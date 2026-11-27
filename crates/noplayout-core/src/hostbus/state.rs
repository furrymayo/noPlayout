//! The channel state machine behind the host bus (PLAN §2.13.2 journal, §2.14 ordering and
//! authority, §4.3 snapshots). Pure and single-threaded: the actor in `mod.rs` owns one per
//! channel, which makes registration atomic with respect to new actions (snapshot at `S`,
//! subscribed from `S + 1` in the same step).

use super::wire::*;
use crate::time::{TimeSource, Ts};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub type ConnId = u64;

/// What an engine caller asks the channel to do (the dev CLI now, the rundown from M4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "cmd",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    Load {
        layer: u32,
        graphic_url: String,
        #[serde(default)]
        data: Option<Value>,
        #[serde(default)]
        cued: bool,
    },
    Play {
        target: Target,
        #[serde(default)]
        delta: Option<i64>,
        #[serde(default)]
        goto: Option<u64>,
        #[serde(default)]
        skip_animation: bool,
    },
    Update {
        target: Target,
        data: Value,
        #[serde(default)]
        skip_animation: bool,
    },
    Stop {
        target: Target,
        #[serde(default)]
        skip_animation: bool,
    },
    Custom {
        target: Target,
        id: String,
        #[serde(default)]
        payload: Option<Value>,
        #[serde(default)]
        skip_animation: bool,
    },
    Dispose {
        target: Target,
    },
}

/// A layer's on-air instance, or the cue slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    Layer(u32),
    Cued,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dispatched {
    pub seq: u64,
    pub instance_id: String,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CmdError {
    #[error("nothing is loaded on {0:?}")]
    NoInstance(Target),
    #[error("layer {0} is outside 1..=999")]
    BadLayer(u32),
    #[error("goto and delta are mutually exclusive")]
    BadPlay,
}

/// Instance phase as committed from the authoritative page's results (seq order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Loading,
    Loaded,
    LoadFailed,
}

#[derive(Debug, Clone)]
struct Entry {
    entry: JournalEntry,
    result: Option<PageResult>,
}

#[derive(Debug, Clone)]
struct Inst {
    id: String,
    generation: u64,
    layer: u32,
    cued: bool,
    graphic_url: String,
    data_at_load: Value,
    data: Value,
    render_characteristics: Value,
    load_seq: u64,
    load_ts: Ts,
    journal: Vec<Entry>,
    /// Dispatched seqs (load included) not yet committed, with any result already received.
    pending: BTreeMap<u64, Option<PageResult>>,
    phase: Phase,
    current_step: Option<u64>,
    at_end: bool,
    checkpoint: Option<CheckpointRec>,
}

#[derive(Debug)]
struct Conn {
    role: Role,
    inc: Option<u64>,
    synced: Option<u64>,
    last_beat: Option<(u64, f64, u64)>,
    /// A standby program page's results for still-pending seqs, used at promotion (§2.14).
    held: BTreeMap<u64, PageResult>,
}

#[derive(Debug)]
struct Incarnation {
    token: String,
    program_conn: Option<ConnId>,
}

/// Side effects the actor performs after a state step.
#[derive(Debug, Default)]
pub struct Out {
    pub sends: Vec<(ConnId, HostDown)>,
    pub close: Vec<(ConnId, u16, String)>,
    pub log: Vec<OnAirRecord>,
    /// Results as they arrive from the authoritative page (API callers wait on these).
    pub results: Vec<PageResult>,
    /// Renderer incarnations whose process the engine must kill (superseded, §2.14).
    pub kill: Vec<u64>,
}

/// One line of `shows/<show-id>.onair.jsonl` (§2.13.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum OnAirRecord {
    Load {
        channel: String,
        instance_id: String,
        generation: u64,
        layer: u32,
        cued: bool,
        graphic_url: String,
        data_at_load: Value,
        render_characteristics: Value,
        seq: u64,
        ts: Ts,
    },
    Action {
        channel: String,
        instance_id: String,
        generation: u64,
        entry: JournalEntry,
    },
    Result {
        channel: String,
        result: PageResult,
    },
    Checkpoint {
        channel: String,
        instance_id: String,
        generation: u64,
        checkpoint: CheckpointRec,
    },
    Dispose {
        channel: String,
        instance_id: String,
        generation: u64,
    },
}

/// Health counters for the channel.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counters {
    pub mirror_divergence: u64,
    pub dropped_results: u64,
    pub refused_registrations: u64,
}

/// A read-only view for Health and tests.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceView {
    pub instance_id: String,
    pub generation: u64,
    pub layer: u32,
    pub cued: bool,
    pub phase: Phase,
    pub current_step: Option<u64>,
    pub at_end: bool,
    pub journal_len: usize,
    pub pending: usize,
    pub data: Value,
}

pub struct ChannelState {
    channel: String,
    time: TimeSource,
    render_characteristics: Value,
    revision: u64,
    last_seq: u64,
    next_generation: u64,
    instances: BTreeMap<String, Inst>,
    conns: BTreeMap<ConnId, Conn>,
    incarnations: BTreeMap<u64, Incarnation>,
    max_inc: u64,
    active_inc: Option<u64>,
    barrier: u64,
    pub counters: Counters,
}

/// 1080p59.94 (the default format; per-channel formats arrive with Output Setup in M4).
pub fn default_render_characteristics() -> Value {
    json!({"resolution": {"width": 1920, "height": 1080}, "frameRate": 60000.0 / 1001.0})
}

/// Constant-time equality for launch tokens.
pub fn token_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl ChannelState {
    pub fn new(channel: impl Into<String>, time: TimeSource) -> Self {
        Self {
            channel: channel.into(),
            time,
            render_characteristics: default_render_characteristics(),
            revision: 0,
            last_seq: 0,
            next_generation: 1,
            instances: BTreeMap::new(),
            conns: BTreeMap::new(),
            incarnations: BTreeMap::new(),
            max_inc: 0,
            active_inc: None,
            barrier: 0,
            counters: Counters::default(),
        }
    }

    pub fn channel(&self) -> &str {
        &self.channel
    }
    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }
    pub fn active_inc(&self) -> Option<u64> {
        self.active_inc
    }
    pub fn barrier(&self) -> u64 {
        self.barrier
    }
    pub fn now(&self) -> Ts {
        self.time.now()
    }

    pub fn instances(&self) -> Vec<InstanceView> {
        self.instances
            .values()
            .map(|i| InstanceView {
                instance_id: i.id.clone(),
                generation: i.generation,
                layer: i.layer,
                cued: i.cued,
                phase: i.phase,
                current_step: i.current_step,
                at_end: i.at_end,
                journal_len: i.journal.len(),
                pending: i.pending.len(),
                data: i.data.clone(),
            })
            .collect()
    }

    /// Full journal of an instance (tests, cold restore).
    pub fn journal(&self, instance_id: &str) -> Option<Vec<JournalEntry>> {
        self.instances
            .get(instance_id)
            .map(|i| i.journal.iter().map(|e| e.entry.clone()).collect())
    }

    // ---- renderer incarnations (§2.14) ----

    /// Register a new renderer spawn; returns its `inc` (strictly increasing within the run).
    pub fn new_incarnation(&mut self, token: String) -> u64 {
        self.max_inc += 1;
        self.incarnations.insert(
            self.max_inc,
            Incarnation {
                token,
                program_conn: None,
            },
        );
        self.max_inc
    }

    /// The renderer process is gone or replaced: its pages are superseded.
    pub fn remove_incarnation(&mut self, inc: u64) -> Out {
        let mut out = Out::default();
        self.incarnations.remove(&inc);
        if self.active_inc == Some(inc) {
            self.active_inc = None;
        }
        let gone: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.inc == Some(inc))
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            self.conns.remove(&id);
            out.close
                .push((id, close::SUPERSEDED, "renderer superseded".into()));
        }
        out
    }

    /// Make `inc` the authoritative renderer. Records barrier `B` = last dispatched seq and
    /// takes the new active's results for still-pending `seq ≤ B` (§2.14).
    pub fn promote(&mut self, inc: u64) -> Out {
        let mut out = Out::default();
        if !self.incarnations.contains_key(&inc) {
            return out;
        }
        self.active_inc = Some(inc);
        self.barrier = self.last_seq;
        let conn = self.incarnations.get(&inc).and_then(|i| i.program_conn);
        if let Some(held) = conn
            .and_then(|c| self.conns.get_mut(&c))
            .map(|c| std::mem::take(&mut c.held))
        {
            for r in held.into_values() {
                self.commit(r, &mut out);
            }
        }
        out
    }

    fn authoritative_conn(&self) -> Option<ConnId> {
        self.active_inc
            .and_then(|i| self.incarnations.get(&i))
            .and_then(|i| i.program_conn)
    }

    // ---- registration ----

    pub fn register(
        &mut self,
        conn: ConnId,
        loopback: bool,
        reg: &Register,
    ) -> Result<Snapshot, (u16, String)> {
        let refused = |s: &mut Self, code: u16, why: &str| {
            s.counters.refused_registrations += 1;
            Err((code, why.to_string()))
        };
        if reg.channel != self.channel {
            return refused(self, close::NO_CHANNEL, "unknown channel");
        }
        let mut inc = None;
        if reg.role == Role::Program {
            if !loopback {
                return refused(self, close::FORBIDDEN, "program role requires loopback");
            }
            let (Some(i), Some(token)) = (reg.inc, reg.token.as_deref()) else {
                return refused(
                    self,
                    close::FORBIDDEN,
                    "program role requires a launch token",
                );
            };
            let verdict = match self.incarnations.get(&i) {
                None if i <= self.max_inc => Some((close::SUPERSEDED, "superseded")),
                None => Some((close::FORBIDDEN, "unknown incarnation")),
                Some(n) if !token_eq(&n.token, token) => {
                    Some((close::FORBIDDEN, "bad launch token"))
                }
                Some(n) if n.program_conn.is_some() => {
                    Some((close::FORBIDDEN, "program already registered"))
                }
                Some(_) => None,
            };
            if let Some((code, why)) = verdict {
                return refused(self, code, why);
            }
            if let Some(n) = self.incarnations.get_mut(&i) {
                n.program_conn = Some(conn);
            }
            inc = Some(i);
        }
        self.conns.insert(
            conn,
            Conn {
                role: reg.role,
                inc,
                synced: None,
                last_beat: None,
                held: BTreeMap::new(),
            },
        );
        Ok(self.snapshot())
    }

    /// A refused superseded program page: the engine kills that process if it is alive.
    pub fn superseded_kill(&self, reg: &Register) -> Option<u64> {
        (reg.role == Role::Program).then_some(reg.inc).flatten()
    }

    pub fn unregister(&mut self, conn: ConnId) {
        if let Some(c) = self.conns.remove(&conn)
            && let Some(n) = c.inc.and_then(|i| self.incarnations.get_mut(&i))
            && n.program_conn == Some(conn)
        {
            n.program_conn = None;
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let layer = |i: &Inst| {
            let frontier = i.checkpoint.as_ref().map_or(0, |c| c.frontier_seq);
            SnapshotLayer {
                layer: i.layer,
                instance_id: i.id.clone(),
                generation: i.generation,
                graphic_url: i.graphic_url.clone(),
                data_at_load: i.data_at_load.clone(),
                render_characteristics: i.render_characteristics.clone(),
                load_ts: i.load_ts.clone(),
                journal: i
                    .journal
                    .iter()
                    .filter(|e| e.entry.seq > frontier)
                    .map(|e| JournalEntry {
                        replay: Some(true),
                        ..e.entry.clone()
                    })
                    .collect(),
                checkpoint: i.checkpoint.clone(),
            }
        };
        let mut layers: Vec<_> = self
            .instances
            .values()
            .filter(|i| !i.cued)
            .map(layer)
            .collect();
        layers.sort_by_key(|l| l.layer);
        Snapshot {
            revision: self.revision,
            last_seq: self.last_seq,
            server_now: self.time.now(),
            layers,
            cued: self.instances.values().find(|i| i.cued).map(layer),
            flags: vec![],
        }
    }

    pub fn time_sync(&self) -> Out {
        let now = self.time.now();
        Out {
            sends: self
                .conns
                .keys()
                .map(|c| {
                    (
                        *c,
                        HostDown::TimeSync {
                            server_now: now.clone(),
                        },
                    )
                })
                .collect(),
            ..Out::default()
        }
    }

    // ---- commands ----

    fn target(&self, t: Target) -> Result<String, CmdError> {
        self.instances
            .values()
            .find(|i| match t {
                Target::Cued => i.cued,
                Target::Layer(l) => !i.cued && i.layer == l,
            })
            .map(|i| i.id.clone())
            .ok_or(CmdError::NoInstance(t))
    }

    fn drop_instance(&mut self, id: &str, out: &mut Out) {
        if let Some(i) = self.instances.remove(id) {
            out.log.push(OnAirRecord::Dispose {
                channel: self.channel.clone(),
                instance_id: i.id,
                generation: i.generation,
            });
        }
    }

    pub fn dispatch(&mut self, cmd: Command) -> Result<(Dispatched, Out), CmdError> {
        let mut out = Out::default();
        let skip = |p: &mut Value, s: bool| {
            if s {
                p["skipAnimation"] = Value::Bool(true);
            }
        };
        let (id, method, params) = match cmd {
            Command::Load {
                layer,
                graphic_url,
                data,
                cued,
            } => {
                if !(1..=999).contains(&layer) {
                    return Err(CmdError::BadLayer(layer));
                }
                // The page retires what this load replaces (runtime `make`); so does the engine.
                let replaced: Vec<String> = self
                    .instances
                    .values()
                    .filter(|i| {
                        if cued {
                            i.cued
                        } else {
                            !i.cued && i.layer == layer
                        }
                    })
                    .map(|i| i.id.clone())
                    .collect();
                for r in replaced {
                    self.drop_instance(&r, &mut out);
                }
                let id = uuid::Uuid::now_v7().to_string();
                let generation = self.next_generation;
                self.next_generation += 1;
                let data = data.unwrap_or_else(|| json!({}));
                let seq = self.last_seq + 1;
                let ts = self.time.now();
                self.instances.insert(
                    id.clone(),
                    Inst {
                        id: id.clone(),
                        generation,
                        layer,
                        cued,
                        graphic_url: graphic_url.clone(),
                        data_at_load: data.clone(),
                        data: data.clone(),
                        render_characteristics: self.render_characteristics.clone(),
                        load_seq: seq,
                        load_ts: ts,
                        journal: vec![],
                        pending: BTreeMap::new(),
                        phase: Phase::Loading,
                        current_step: None,
                        at_end: false,
                        checkpoint: None,
                    },
                );
                let params = json!({
                    "graphicUrl": graphic_url, "layer": layer, "data": data,
                    "renderCharacteristics": self.render_characteristics, "cued": cued,
                });
                (id, Method::Load, params)
            }
            Command::Play {
                target,
                delta,
                goto,
                skip_animation,
            } => {
                if delta.is_some() && goto.is_some() {
                    return Err(CmdError::BadPlay);
                }
                let id = self.target(target)?;
                let mut p = match goto {
                    Some(g) => json!({"goto": g}),
                    None => json!({"delta": delta.unwrap_or(1)}),
                };
                skip(&mut p, skip_animation);
                // A cued instance goes on air at its layer: retire whatever holds that layer.
                if target == Target::Cued {
                    let layer = self.instances[&id].layer;
                    p["layer"] = json!(layer);
                    let others: Vec<String> = self
                        .instances
                        .values()
                        .filter(|i| !i.cued && i.layer == layer)
                        .map(|i| i.id.clone())
                        .collect();
                    for o in others {
                        self.drop_instance(&o, &mut out);
                    }
                    if let Some(i) = self.instances.get_mut(&id) {
                        i.cued = false;
                    }
                }
                (id, Method::PlayAction, p)
            }
            Command::Update {
                target,
                data,
                skip_animation,
            } => {
                let id = self.target(target)?;
                if let (Some(i), Value::Object(d)) = (self.instances.get_mut(&id), &data) {
                    if !i.data.is_object() {
                        i.data = json!({});
                    }
                    for (k, v) in d {
                        i.data[k] = v.clone();
                    }
                }
                let mut p = json!({"data": data});
                skip(&mut p, skip_animation);
                (id, Method::UpdateAction, p)
            }
            Command::Stop {
                target,
                skip_animation,
            } => {
                let id = self.target(target)?;
                let mut p = json!({});
                skip(&mut p, skip_animation);
                (id, Method::StopAction, p)
            }
            Command::Custom {
                target,
                id: action,
                payload,
                skip_animation,
            } => {
                let id = self.target(target)?;
                let mut p = json!({"id": action, "payload": payload.unwrap_or_else(|| json!({}))});
                skip(&mut p, skip_animation);
                (id, Method::CustomAction, p)
            }
            Command::Dispose { target } => (self.target(target)?, Method::Dispose, json!({})),
        };
        self.last_seq += 1;
        self.revision += 1;
        let seq = self.last_seq;
        let ts = if method == Method::Load {
            self.instances[&id].load_ts.clone()
        } else {
            self.time.now()
        };
        let inst = self.instances.get_mut(&id).expect("target resolved above");
        let generation = inst.generation;
        let entry = JournalEntry {
            seq,
            ts: ts.clone(),
            method,
            params: params.clone(),
            quarantined: false,
            replay: None,
        };
        inst.pending.insert(seq, None);
        match method {
            Method::Load => out.log.push(OnAirRecord::Load {
                channel: self.channel.clone(),
                instance_id: id.clone(),
                generation,
                layer: inst.layer,
                cued: inst.cued,
                graphic_url: inst.graphic_url.clone(),
                data_at_load: inst.data_at_load.clone(),
                render_characteristics: inst.render_characteristics.clone(),
                seq,
                ts: ts.clone(),
            }),
            Method::Dispose => {}
            _ => {
                inst.journal.push(Entry {
                    entry: entry.clone(),
                    result: None,
                });
                out.log.push(OnAirRecord::Action {
                    channel: self.channel.clone(),
                    instance_id: id.clone(),
                    generation,
                    entry,
                });
            }
        }
        let action = HostDown::Action(Action {
            revision: self.revision,
            seq,
            ts,
            instance_id: id.clone(),
            generation,
            method,
            params,
        });
        out.sends
            .extend(self.conns.keys().map(|c| (*c, action.clone())));
        if method == Method::Dispose {
            self.drop_instance(&id, &mut out);
        }
        Ok((
            Dispatched {
                seq,
                instance_id: id,
                generation,
            },
            out,
        ))
    }

    /// Ask the authoritative page for a quiescent checkpoint (declared instances only, §2.13.4).
    pub fn request_checkpoint(&self, instance_id: &str) -> Out {
        let mut out = Out::default();
        if let (Some(c), Some(i)) = (self.authoritative_conn(), self.instances.get(instance_id)) {
            out.sends.push((
                c,
                HostDown::Checkpoint {
                    instance_id: i.id.clone(),
                    generation: i.generation,
                },
            ));
        }
        out
    }

    // ---- page messages ----

    pub fn page(&mut self, conn: ConnId, msg: HostUp) -> Out {
        let mut out = Out::default();
        let authoritative = self.authoritative_conn() == Some(conn);
        let Some(role) = self.conns.get(&conn).map(|c| c.role) else {
            return out;
        };
        match msg {
            HostUp::Register(_) => {}
            HostUp::Beat {
                n,
                lag_ms,
                last_applied_seq,
                ..
            } => {
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.last_beat = Some((n, lag_ms, last_applied_seq));
                }
            }
            HostUp::Synced { seq } => {
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.synced = Some(seq);
                }
            }
            HostUp::PageError { text } => {
                tracing::warn!(channel = %self.channel, ?role, "page error: {text}");
            }
            HostUp::CheckpointDeferred { .. } => {}
            HostUp::CheckpointResult {
                instance_id,
                generation,
                frontier_seq,
                cut_ts,
                state,
            } => {
                if !authoritative {
                    return out;
                }
                if let Some(i) = self.instances.get_mut(&instance_id)
                    && i.generation == generation
                    && frontier_seq <= self.last_seq
                    && frontier_seq >= i.load_seq
                {
                    let cp = CheckpointRec {
                        state,
                        frontier_seq,
                        cut_ts,
                    };
                    i.checkpoint = Some(cp.clone());
                    out.log.push(OnAirRecord::Checkpoint {
                        channel: self.channel.clone(),
                        instance_id,
                        generation,
                        checkpoint: cp,
                    });
                }
            }
            HostUp::Result(r) => {
                if authoritative {
                    self.commit(r, &mut out);
                } else if role == Role::Program {
                    // Standby: keep results for pending seqs, for promotion; compare otherwise.
                    let pending = self
                        .instances
                        .get(&r.instance_id)
                        .filter(|i| i.generation == r.generation)
                        .is_some_and(|i| i.pending.contains_key(&r.seq));
                    if pending {
                        if let Some(c) = self.conns.get_mut(&conn) {
                            c.held.insert(r.seq, r);
                        }
                    } else {
                        self.compare_mirror(&r);
                    }
                } else {
                    self.compare_mirror(&r);
                }
            }
        }
        out
    }

    fn compare_mirror(&mut self, r: &PageResult) {
        let committed = self
            .instances
            .get(&r.instance_id)
            .filter(|i| i.generation == r.generation)
            .and_then(|i| i.journal.iter().find(|e| e.entry.seq == r.seq))
            .and_then(|e| e.result.as_ref());
        if let Some(c) = committed
            && (c.class() != r.class() || c.current_step != r.current_step)
        {
            self.counters.mirror_divergence += 1;
        }
    }

    /// Record an authoritative result; update engine state only in seq order per instance.
    fn commit(&mut self, r: PageResult, out: &mut Out) {
        let Some(i) = self
            .instances
            .get_mut(&r.instance_id)
            .filter(|i| i.generation == r.generation)
        else {
            tracing::debug!(seq = r.seq, "result for a gone generation: discarded");
            self.counters.dropped_results += 1;
            return;
        };
        match i.pending.get_mut(&r.seq) {
            Some(slot @ None) => *slot = Some(r.clone()),
            _ => {
                self.counters.dropped_results += 1;
                return;
            }
        }
        if let Some(e) = i.journal.iter_mut().find(|e| e.entry.seq == r.seq) {
            e.result = Some(r.clone());
        }
        out.results.push(r.clone());
        out.log.push(OnAirRecord::Result {
            channel: self.channel.clone(),
            result: r,
        });
        while let Some(entry) = i.pending.first_entry() {
            let Some(res) = entry.get().clone() else {
                break;
            };
            let seq = *entry.key();
            entry.remove();
            let method = if seq == i.load_seq {
                Method::Load
            } else {
                i.journal
                    .iter()
                    .find(|e| e.entry.seq == seq)
                    .map_or(Method::Dispose, |e| e.entry.method)
            };
            match method {
                Method::Load => {
                    i.phase = if res.ok() {
                        Phase::Loaded
                    } else {
                        Phase::LoadFailed
                    }
                }
                Method::PlayAction if res.ok() => {
                    i.current_step = res.current_step;
                    // O7: `currentStep: undefined` means "at end".
                    i.at_end = res.current_step.is_none();
                }
                Method::StopAction if res.ok() => {
                    i.current_step = None;
                    i.at_end = true;
                }
                _ => {}
            }
        }
    }

    /// Pages registered on this channel, with their last beat, for Health.
    pub fn pages(&self) -> Vec<PageView> {
        self.conns
            .values()
            .map(|c| PageView {
                role: c.role,
                inc: c.inc,
                synced_seq: c.synced,
                last_beat: c.last_beat.map(|b| b.0),
                lag_ms: c.last_beat.map(|b| b.1),
                last_applied_seq: c.last_beat.map(|b| b.2),
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageView {
    pub role: Role,
    pub inc: Option<u64>,
    pub synced_seq: Option<u64>,
    pub last_beat: Option<u64>,
    pub lag_ms: Option<f64>,
    pub last_applied_seq: Option<u64>,
}
