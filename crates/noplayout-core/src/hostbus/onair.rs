//! `shows/<show-id>.onair.jsonl` (PLAN §2.13.2): one JSON line per journal record, appended on a
//! dedicated thread, flushed per line, `fsync` debounced to 200 ms. Never capped or truncated;
//! a disposed instance's records stay in the file and the reader drops the instance.

use super::state::OnAirRecord;
use super::wire::{CheckpointRec, JournalEntry, PageResult};
use crate::time::Ts;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const FSYNC_DEBOUNCE: Duration = Duration::from_millis(200);

/// Sender half of the journal writer. Cheap to clone; dropping every clone stops the thread
/// after a final `fsync`.
#[derive(Clone, Debug)]
pub struct OnAirLog {
    tx: mpsc::Sender<OnAirRecord>,
}

impl OnAirLog {
    pub fn open(path: impl Into<PathBuf>) -> std::io::Result<(Self, std::thread::JoinHandle<()>)> {
        let path = path.into();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let (tx, rx) = mpsc::channel::<OnAirRecord>();
        let handle = std::thread::Builder::new()
            .name("onair-journal".into())
            .spawn(move || writer(file, rx))?;
        Ok((Self { tx }, handle))
    }

    pub fn append(&self, record: OnAirRecord) {
        if self.tx.send(record).is_err() {
            tracing::error!("on-air journal writer stopped; record not persisted");
        }
    }
}

fn writer(file: std::fs::File, rx: mpsc::Receiver<OnAirRecord>) {
    let mut out = std::io::BufWriter::new(file);
    let mut dirty_since: Option<Instant> = None;
    loop {
        let wait = dirty_since.map_or(Duration::from_secs(3600), |t| {
            FSYNC_DEBOUNCE.saturating_sub(t.elapsed())
        });
        match rx.recv_timeout(wait) {
            Ok(r) => {
                let line = match serde_json::to_string(&r) {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("on-air record not serializable: {e}");
                        continue;
                    }
                };
                if let Err(e) = writeln!(out, "{line}").and_then(|_| out.flush()) {
                    tracing::error!("on-air journal write failed: {e}");
                }
                dirty_since.get_or_insert_with(Instant::now);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = out.flush();
                let _ = out.get_ref().sync_data();
                return;
            }
        }
        if dirty_since.is_some_and(|t| t.elapsed() >= FSYNC_DEBOUNCE) {
            if let Err(e) = out.get_ref().sync_data() {
                tracing::error!("on-air journal fsync failed: {e}");
            }
            dirty_since = None;
        }
    }
}

/// One live instance as rebuilt from the file (cold restore input, §2.13.3).
#[derive(Debug, Clone, PartialEq)]
pub struct RestoredInstance {
    pub channel: String,
    pub instance_id: String,
    pub generation: u64,
    pub layer: u32,
    pub cued: bool,
    pub graphic_url: String,
    pub data_at_load: Value,
    pub render_characteristics: Value,
    pub load_seq: u64,
    pub load_ts: Ts,
    pub journal: Vec<(JournalEntry, Option<PageResult>)>,
    pub checkpoint: Option<CheckpointRec>,
}

/// Rebuild the live instances from a journal file. A torn last line (power loss mid-write) is
/// ignored; any other unparseable line is an error.
pub fn read(path: &Path) -> std::io::Result<Vec<RestoredInstance>> {
    let file = std::fs::File::open(path)?;
    let mut live: BTreeMap<String, RestoredInstance> = BTreeMap::new();
    let lines: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .collect::<Result<_, _>>()?;
    let n = lines.len();
    for (idx, line) in lines.into_iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let rec: OnAirRecord = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) if idx + 1 == n => break,
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("on-air journal line {}: {e}", idx + 1),
                ));
            }
        };
        match rec {
            OnAirRecord::Load {
                channel,
                instance_id,
                generation,
                layer,
                cued,
                graphic_url,
                data_at_load,
                render_characteristics,
                seq,
                ts,
            } => {
                live.insert(
                    instance_id.clone(),
                    RestoredInstance {
                        channel,
                        instance_id,
                        generation,
                        layer,
                        cued,
                        graphic_url,
                        data_at_load,
                        render_characteristics,
                        load_seq: seq,
                        load_ts: ts,
                        journal: vec![],
                        checkpoint: None,
                    },
                );
            }
            OnAirRecord::Action {
                instance_id,
                generation,
                entry,
                ..
            } => {
                if let Some(i) = live
                    .get_mut(&instance_id)
                    .filter(|i| i.generation == generation)
                {
                    if entry.method == super::wire::Method::PlayAction && i.cued {
                        i.cued = false;
                    }
                    i.journal.push((entry, None));
                }
            }
            OnAirRecord::Result { result, .. } => {
                if let Some(i) = live
                    .get_mut(&result.instance_id)
                    .filter(|i| i.generation == result.generation)
                    && let Some(e) = i.journal.iter_mut().find(|(e, _)| e.seq == result.seq)
                {
                    e.1 = Some(result);
                }
            }
            OnAirRecord::Checkpoint {
                instance_id,
                generation,
                checkpoint,
                ..
            } => {
                if let Some(i) = live
                    .get_mut(&instance_id)
                    .filter(|i| i.generation == generation)
                {
                    i.checkpoint = Some(checkpoint);
                }
            }
            OnAirRecord::Dispose { instance_id, .. } => {
                live.remove(&instance_id);
            }
        }
    }
    let mut v: Vec<_> = live.into_values().collect();
    v.sort_by_key(|i| i.load_seq);
    Ok(v)
}
