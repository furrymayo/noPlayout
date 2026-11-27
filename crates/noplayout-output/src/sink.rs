//! Snapshots, per-plane sender threads and sinks (PLAN §2.4a, §5.4).
//!
//! The pacer converts a paint once into an immutable [`Snapshot`] and offers it to each plane's
//! [`Lane`]: a one-deep queue, newest wins. Each lane's thread owns one [`FrameSink`] and is
//! the only caller of `submit`; the pacer never calls a sink, so a blocked sink cannot delay
//! the clock or the other plane. M1 has [`DumpSink`]; M2 adds `NdiSink` behind the same lanes.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plane {
    Fill,
    Key,
}

impl Plane {
    pub fn label(self) -> &'static str {
        match self {
            Plane::Fill => "fill",
            Plane::Key => "key",
        }
    }
}

/// Counters shared by the pacer, the lanes and the snapshots.
#[derive(Debug, Default)]
pub struct Counters {
    pub submit_blocked: AtomicU64,
    pub pair_partial: AtomicU64,
}

/// One output slot: `k`, `tc(k)` and the paint it carries, converted once (§5.4).
#[derive(Debug)]
pub struct Snapshot {
    pub k: u64,
    pub tc_100ns: i64,
    pub inc: u64,
    pub paint_seq: u64,
    pub painted_at_ns: u64,
    /// Resent because no newer paint was ready (§5.3).
    pub held: bool,
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<Converted>,
    /// Planes this snapshot was dropped from (displaced before `submit`).
    dropped: AtomicU8,
    planes: u8,
    counters: Arc<Counters>,
}

/// FILL (BGRA, straight alpha, alpha forced opaque) and KEY (8-bit, `Y = alpha`) of a paint.
#[derive(Debug)]
pub struct Converted {
    pub fill: Vec<u8>,
    pub key: Vec<u8>,
}

impl Snapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        k: u64,
        tc_100ns: i64,
        meta: noplayout_frames::PaintMeta,
        held: bool,
        pixels: Arc<Converted>,
        planes: u8,
        counters: Arc<Counters>,
    ) -> Self {
        Self {
            k,
            tc_100ns,
            inc: meta.inc,
            paint_seq: meta.paint_seq,
            painted_at_ns: meta.painted_at_ns,
            held,
            width: meta.width,
            height: meta.height,
            pixels,
            dropped: AtomicU8::new(0),
            planes,
            counters,
        }
    }

    pub fn plane(&self, p: Plane) -> &[u8] {
        match p {
            Plane::Fill => &self.pixels.fill,
            Plane::Key => &self.pixels.key,
        }
    }
}

impl Drop for Snapshot {
    /// A slot that reached some planes but not all is a partial pair (§5.4).
    fn drop(&mut self) {
        let d = *self.dropped.get_mut();
        if d > 0 && d < self.planes {
            self.counters.pair_partial.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Premultiplied BGRA (CEF) → FILL + KEY. The one conversion per slot (S2 measures it).
pub fn convert(src: &[u8], width: u32, height: u32, stride: u32) -> Converted {
    let (w, h, stride) = (width as usize, height as usize, stride as usize);
    let mut fill = vec![0u8; w * h * 4];
    let mut key = vec![0u8; w * h];
    for y in 0..h {
        let row = &src[y * stride..y * stride + w * 4];
        let out = &mut fill[y * w * 4..(y + 1) * w * 4];
        let kout = &mut key[y * w..(y + 1) * w];
        for x in 0..w {
            let p = &row[x * 4..x * 4 + 4];
            let a = u32::from(p[3]);
            kout[x] = p[3];
            let o = &mut out[x * 4..x * 4 + 4];
            if a == 0 {
                o.copy_from_slice(&[0, 0, 0, 255]);
            } else if a == 255 {
                o.copy_from_slice(&[p[0], p[1], p[2], 255]);
            } else {
                for c in 0..3 {
                    o[c] = ((u32::from(p[c]) * 255 + a / 2) / a).min(255) as u8;
                }
                o[3] = 255;
            }
        }
    }
    Converted { fill, key }
}

/// Where a plane's frames go. `submit` may block; only its lane's thread calls it.
pub trait FrameSink: Send {
    fn submit(&mut self, plane: Plane, snap: &Snapshot) -> io::Result<()>;
    /// Flush and close; called once when the lane stops.
    fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A plane's one-deep queue and its thread's progress (§5.4).
#[derive(Debug, Default)]
pub struct Lane {
    queue: Mutex<LaneQueue>,
    cv: Condvar,
    /// `mono_ns` when the current `submit` started; 0 while idle.
    pub in_flight_since: AtomicU64,
    pub last_submit_k: AtomicU64,
    pub submitted: AtomicU64,
}

#[derive(Debug, Default)]
struct LaneQueue {
    next: Option<Arc<Snapshot>>,
    stop: bool,
}

impl Lane {
    /// Newest wins: a snapshot still queued is dropped from this plane only.
    pub fn offer(&self, snap: Arc<Snapshot>, counters: &Counters) {
        let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(old) = q.next.replace(snap) {
            counters.submit_blocked.fetch_add(1, Ordering::Relaxed);
            old.dropped.fetch_add(1, Ordering::Relaxed);
        }
        self.cv.notify_one();
    }

    /// Nothing queued and nothing in `submit` (tests pace a virtual clock on it).
    #[cfg(test)]
    pub fn is_idle(&self) -> bool {
        let q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        q.next.is_none() && self.in_flight_since.load(Ordering::Acquire) == 0
    }

    pub fn stop(&self) {
        let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        q.stop = true;
        self.cv.notify_one();
    }

    fn take(&self) -> Option<Arc<Snapshot>> {
        let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(s) = q.next.take() {
                return Some(s);
            }
            if q.stop {
                return None;
            }
            q = self.cv.wait(q).unwrap_or_else(|p| p.into_inner());
        }
    }

    /// The sender thread: elevated, takes the newest snapshot, submits it, records progress.
    pub fn spawn(
        self: &Arc<Self>,
        plane: Plane,
        mut sink: Box<dyn FrameSink>,
        hook: Option<crate::fault::SubmitHook>,
    ) -> io::Result<std::thread::JoinHandle<io::Result<()>>> {
        let lane = self.clone();
        std::thread::Builder::new()
            .name(format!("sender-{}", plane.label()))
            .spawn(move || {
                noplayout_frames::clock::elevate_current_thread();
                while let Some(snap) = lane.take() {
                    lane.in_flight_since
                        .store(noplayout_frames::clock::mono_ns().max(1), Ordering::Release);
                    if let Some(h) = &hook {
                        h(plane, snap.k);
                    }
                    let r = sink.submit(plane, &snap);
                    lane.in_flight_since.store(0, Ordering::Release);
                    lane.last_submit_k.store(snap.k, Ordering::Relaxed);
                    lane.submitted.fetch_add(1, Ordering::Relaxed);
                    if let Err(e) = r {
                        eprintln!("noplayout-output: {} sink: {e}", plane.label());
                    }
                }
                sink.finish()
            })
    }
}

/// Rectangles to keep from each frame (`x,y,w,h`); empty keeps the whole frame.
pub type Crop = Vec<[u32; 4]>;

pub fn parse_crop(s: &str) -> Result<[u32; 4], String> {
    let v: Vec<u32> = s
        .split(',')
        .map(|p| p.trim().parse::<u32>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("bad crop {s}: expected x,y,w,h"))?;
    <[u32; 4]>::try_from(v).map_err(|_| format!("bad crop {s}: expected x,y,w,h"))
}

/// Writes each slot to `<dir>/<plane>/<k>.raw` plus one JSON line in `<dir>/<plane>.jsonl`
/// (`k`, `tc`, the paint, `held`, geometry). FILL is BGRA, KEY one byte per pixel. With a crop
/// list, only those rectangles are written, row by row in list order.
pub struct DumpSink {
    dir: PathBuf,
    index: BufWriter<File>,
    crop: Crop,
}

impl DumpSink {
    pub fn create(dir: &Path, plane: Plane, crop: Crop) -> io::Result<Self> {
        std::fs::create_dir_all(dir.join(plane.label()))?;
        let index = BufWriter::new(File::create(dir.join(format!("{}.jsonl", plane.label())))?);
        Ok(Self {
            dir: dir.join(plane.label()),
            index,
            crop,
        })
    }
}

impl FrameSink for DumpSink {
    fn submit(&mut self, plane: Plane, s: &Snapshot) -> io::Result<()> {
        let bpp: usize = match plane {
            Plane::Fill => 4,
            Plane::Key => 1,
        };
        let data = s.plane(plane);
        let (w, h) = (s.width as usize, s.height as usize);
        let mut f = BufWriter::new(File::create(self.dir.join(format!("{:08}.raw", s.k)))?);
        if self.crop.is_empty() {
            f.write_all(data)?;
        } else {
            for &[x, y, cw, ch] in &self.crop {
                let (x, y) = (x as usize, y as usize);
                let cw = (cw as usize).min(w.saturating_sub(x));
                let ch = (ch as usize).min(h.saturating_sub(y));
                for row in y..y + ch {
                    let start = (row * w + x) * bpp;
                    f.write_all(&data[start..start + cw * bpp])?;
                }
            }
        }
        f.flush()?;
        writeln!(
            self.index,
            "{{\"k\":{},\"tc\":{},\"inc\":{},\"paintSeq\":{},\"paintedAt\":{},\"held\":{},\"width\":{},\"height\":{},\"bpp\":{},\"crop\":{:?}}}",
            s.k,
            s.tc_100ns,
            s.inc,
            s.paint_seq,
            s.painted_at_ns,
            s.held,
            s.width,
            s.height,
            bpp,
            self.crop
        )
    }

    fn finish(&mut self) -> io::Result<()> {
        self.index.flush()
    }
}

/// Counts submissions only (no dump requested): keeps the lanes and monitor real.
pub struct NullSink;

impl FrameSink for NullSink {
    fn submit(&mut self, _plane: Plane, _snap: &Snapshot) -> io::Result<()> {
        Ok(())
    }
}
