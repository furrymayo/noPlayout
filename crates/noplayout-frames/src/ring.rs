//! The 4-slot paint ring (PLAN §5.3, E-042), laid out in one shared-memory object.
//!
//! Layout: a 4 KiB header page (`RingHeader`), then four slots of `slot_bytes` each, every slot
//! starting on a 4 KiB boundary. Slot headers live in the header page and are changed only by
//! atomic operations; a slot's pixels are written only by the writer that moved it to
//! `WRITING` and read only by the reader that moved it to `READING`.
//!
//! The state word is `gen << 8 | state`. `gen` grows on every publish, so a reader's
//! compare-and-swap on a slot it inspected fails if the writer reclaimed and republished the
//! slot in between (no ABA).

use crate::shm::{self, Shm};
use std::io;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub const SLOTS: usize = 4;
const MAGIC: u64 = u64::from_le_bytes(*b"NPRING01");
const PAGE: usize = 4096;

pub const FREE: u64 = 0;
pub const WRITING: u64 = 1;
pub const READY: u64 = 2;
pub const READING: u64 = 3;

#[repr(C)]
struct SlotHeader {
    state: AtomicU64,
    inc: AtomicU64,
    paint_seq: AtomicU64,
    painted_at_ns: AtomicU64,
    width: AtomicU32,
    height: AtomicU32,
    stride: AtomicU32,
    _pad: AtomicU32,
}

#[repr(C)]
struct RingHeader {
    magic: AtomicU64,
    inc: AtomicU64,
    slot_bytes: AtomicU64,
    max_width: AtomicU32,
    max_height: AtomicU32,
    /// Writer-side: a READY slot was reclaimed because no slot was FREE.
    overwrites: AtomicU64,
    /// Reader-side: READY paints freed unread because a newer one was taken.
    superseded: AtomicU64,
    slots: [SlotHeader; SLOTS],
}

const _: () = assert!(std::mem::size_of::<RingHeader>() <= PAGE);

/// Ring geometry for a `w × h` BGRA canvas.
pub fn slot_bytes(w: u32, h: u32) -> usize {
    (w as usize * h as usize * 4).div_ceil(PAGE) * PAGE
}

/// Total object size for a `w × h` ring.
pub fn ring_bytes(w: u32, h: u32) -> usize {
    PAGE + SLOTS * slot_bytes(w, h)
}

/// What a published paint is called (§5.3 identity) and how it is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintMeta {
    pub inc: u64,
    pub paint_seq: u64,
    pub painted_at_ns: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

struct Mapped {
    shm: Shm,
}

impl Mapped {
    fn header(&self) -> &RingHeader {
        // SAFETY: the object is at least one page, page-aligned, and RingHeader is all atomics.
        unsafe { &*(self.shm.as_ptr() as *const RingHeader) }
    }
    fn slot_ptr(&self, i: usize) -> *mut u8 {
        let sb = self.header().slot_bytes.load(Ordering::Relaxed) as usize;
        // SAFETY: offset within the mapping (checked in `validate`).
        unsafe { self.shm.as_ptr().add(PAGE + i * sb) }
    }
    fn slot_len(&self) -> usize {
        self.header().slot_bytes.load(Ordering::Relaxed) as usize
    }
    fn validate(&self) -> io::Result<()> {
        let h = self.header();
        let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
        if self.shm.len() < PAGE || h.magic.load(Ordering::Acquire) != MAGIC {
            return Err(bad("not a noPlayout paint ring"));
        }
        let sb = h.slot_bytes.load(Ordering::Relaxed) as usize;
        if sb == 0 || PAGE + SLOTS * sb > self.shm.len() {
            return Err(bad("paint ring is smaller than its header says"));
        }
        Ok(())
    }
}

fn state(word: u64) -> u64 {
    word & 0xff
}
fn gen_of(word: u64) -> u64 {
    word >> 8
}

/// The renderer's side. Never blocks, never waits on the reader.
pub struct Writer {
    m: Mapped,
    name: String,
    inc: u64,
    seq: u64,
}

/// A slot the writer owns until [`Writer::publish`] or drop (drop puts it back to FREE).
pub struct WriteSlot<'a> {
    w: &'a mut Writer,
    i: usize,
    era: u64,
    published: bool,
}

impl Writer {
    /// Create the ring object `name` for incarnation `inc`, `max_w × max_h` BGRA. Record the
    /// name in the engine's [`shm::Registry`] first so a crash can't leak it.
    pub fn create(name: &str, inc: u64, max_w: u32, max_h: u32) -> io::Result<Self> {
        let shm = shm::create(name, ring_bytes(max_w, max_h))?;
        let m = Mapped { shm };
        let h = m.header();
        h.inc.store(inc, Ordering::Relaxed);
        h.slot_bytes
            .store(slot_bytes(max_w, max_h) as u64, Ordering::Relaxed);
        h.max_width.store(max_w, Ordering::Relaxed);
        h.max_height.store(max_h, Ordering::Relaxed);
        for s in &h.slots {
            s.state.store(FREE, Ordering::Relaxed);
        }
        h.magic.store(MAGIC, Ordering::Release);
        Ok(Self {
            m,
            name: name.to_string(),
            inc,
            seq: 0,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Take a slot to paint into: a FREE one, else the oldest READY one (`overwrites += 1`),
    /// never a READING one. A failed compare-and-swap means the reader just took or freed a
    /// slot, so the next pass finds one; the loop is lock-free and allocation-free. `None` only
    /// if every slot stays WRITING/READING, which the protocol rules out (one writer, a reader
    /// holding at most one slot).
    pub fn begin(&mut self) -> Option<WriteSlot<'_>> {
        let h = self.m.header();
        let claim = |i: usize, w: u64| {
            h.slots[i]
                .state
                .compare_exchange(
                    w,
                    (gen_of(w) << 8) | WRITING,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
        };
        let mut found = None;
        'passes: for _ in 0..64 {
            for (i, s) in h.slots.iter().enumerate() {
                let w = s.state.load(Ordering::Relaxed);
                if state(w) == FREE && claim(i, w) {
                    found = Some((i, gen_of(w), false));
                    break 'passes;
                }
            }
            // No FREE slot: reclaim the oldest READY one.
            let mut oldest: Option<(u64, usize, u64)> = None;
            for (i, s) in h.slots.iter().enumerate() {
                let w = s.state.load(Ordering::Acquire);
                let seq = s.paint_seq.load(Ordering::Relaxed);
                if state(w) == READY && oldest.is_none_or(|o| seq < o.0) {
                    oldest = Some((seq, i, w));
                }
            }
            if let Some((_, i, w)) = oldest
                && claim(i, w)
            {
                found = Some((i, gen_of(w), true));
                break;
            }
        }
        let (i, era, reclaimed) = found?;
        if reclaimed {
            h.overwrites.fetch_add(1, Ordering::Relaxed);
        }
        Some(WriteSlot {
            era,
            i,
            w: self,
            published: false,
        })
    }

    /// Writer-side counter (`ring_overwrites`).
    pub fn overwrites(&self) -> u64 {
        self.m.header().overwrites.load(Ordering::Relaxed)
    }
}

impl WriteSlot<'_> {
    /// The slot's pixel memory (`slot_bytes`): CEF's paint buffer is copied straight in, the
    /// only copy between Chromium and the output process.
    pub fn pixels(&mut self) -> &mut [u8] {
        // SAFETY: this writer owns the slot in WRITING; nobody else touches its pixels.
        unsafe { std::slice::from_raw_parts_mut(self.w.m.slot_ptr(self.i), self.w.m.slot_len()) }
    }

    /// Stamp and publish: header fields, then `WRITING → READY` with release ordering.
    pub fn publish(
        mut self,
        painted_at_ns: u64,
        width: u32,
        height: u32,
        stride: u32,
    ) -> PaintMeta {
        self.w.seq += 1;
        let meta = PaintMeta {
            inc: self.w.inc,
            paint_seq: self.w.seq,
            painted_at_ns,
            width,
            height,
            stride,
        };
        let s = &self.w.m.header().slots[self.i];
        s.inc.store(meta.inc, Ordering::Relaxed);
        s.paint_seq.store(meta.paint_seq, Ordering::Relaxed);
        s.painted_at_ns.store(painted_at_ns, Ordering::Relaxed);
        s.width.store(width, Ordering::Relaxed);
        s.height.store(height, Ordering::Relaxed);
        s.stride.store(stride, Ordering::Relaxed);
        s.state
            .store(((self.era + 1) << 8) | READY, Ordering::Release);
        self.published = true;
        meta
    }
}

impl Drop for WriteSlot<'_> {
    fn drop(&mut self) {
        if !self.published {
            self.w.m.header().slots[self.i]
                .state
                .store((self.era << 8) | FREE, Ordering::Release);
        }
    }
}

/// The output process's side (the only reader).
pub struct Reader {
    m: Mapped,
}

/// A paint the reader holds in READING; drop returns the slot to FREE.
pub struct ReadSlot<'a> {
    r: &'a Reader,
    i: usize,
    word: u64,
    pub meta: PaintMeta,
}

impl Reader {
    /// Open an existing ring. A respawned reader resets any READING slot to FREE (§5.3).
    pub fn open(name: &str) -> io::Result<Self> {
        let m = Mapped {
            shm: shm::open(name)?,
        };
        m.validate()?;
        for s in &m.header().slots {
            let w = s.state.load(Ordering::Acquire);
            if state(w) == READING {
                let _ = s.state.compare_exchange(
                    w,
                    (gen_of(w) << 8) | FREE,
                    Ordering::Release,
                    Ordering::Relaxed,
                );
            }
        }
        Ok(Self { m })
    }

    /// The incarnation that created this ring.
    pub fn inc(&self) -> u64 {
        self.m.header().inc.load(Ordering::Relaxed)
    }

    /// Selection at `T_k` (§5.3): the newest READY paint with `painted_at ≤ cutoff_ns`, moved
    /// to READING; every older READY paint is freed (`superseded`). WRITING slots (including a
    /// dead writer's) are ignored. `None` if nothing qualifies: the caller holds its last frame.
    pub fn take_newest(&self, cutoff_ns: u64) -> Option<ReadSlot<'_>> {
        let h = self.m.header();
        loop {
            let mut best: Option<(usize, u64, PaintMeta)> = None;
            for (i, s) in h.slots.iter().enumerate() {
                let w = s.state.load(Ordering::Acquire);
                if state(w) != READY {
                    continue;
                }
                let meta = self.meta(i);
                // Re-check: a reclaim + republish in between bumps gen.
                if s.state.load(Ordering::Acquire) != w || meta.painted_at_ns > cutoff_ns {
                    continue;
                }
                if best.as_ref().is_none_or(|b| meta.paint_seq > b.2.paint_seq) {
                    best = Some((i, w, meta));
                }
            }
            let (i, w, meta) = best?;
            let reading = (gen_of(w) << 8) | READING;
            if h.slots[i]
                .state
                .compare_exchange(w, reading, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                continue; // reclaimed under us; choose again
            }
            for (j, s) in h.slots.iter().enumerate() {
                if j == i {
                    continue;
                }
                let w2 = s.state.load(Ordering::Acquire);
                if state(w2) == READY
                    && s.paint_seq.load(Ordering::Relaxed) < meta.paint_seq
                    && s.state.load(Ordering::Acquire) == w2
                    && s.state
                        .compare_exchange(
                            w2,
                            (gen_of(w2) << 8) | FREE,
                            Ordering::Release,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                {
                    h.superseded.fetch_add(1, Ordering::Relaxed);
                }
            }
            return Some(ReadSlot {
                r: self,
                i,
                word: reading,
                meta,
            });
        }
    }

    fn meta(&self, i: usize) -> PaintMeta {
        let s = &self.m.header().slots[i];
        PaintMeta {
            inc: s.inc.load(Ordering::Relaxed),
            paint_seq: s.paint_seq.load(Ordering::Relaxed),
            painted_at_ns: s.painted_at_ns.load(Ordering::Relaxed),
            width: s.width.load(Ordering::Relaxed),
            height: s.height.load(Ordering::Relaxed),
            stride: s.stride.load(Ordering::Relaxed),
        }
    }

    /// Slot states, for tests and Health.
    pub fn states(&self) -> [u64; SLOTS] {
        std::array::from_fn(|i| state(self.m.header().slots[i].state.load(Ordering::Acquire)))
    }

    pub fn overwrites(&self) -> u64 {
        self.m.header().overwrites.load(Ordering::Relaxed)
    }
    pub fn superseded(&self) -> u64 {
        self.m.header().superseded.load(Ordering::Relaxed)
    }
}

impl ReadSlot<'_> {
    /// The paint's pixels (`stride × height` bytes).
    pub fn pixels(&self) -> &[u8] {
        let len = (self.meta.stride as usize * self.meta.height as usize).min(self.r.m.slot_len());
        // SAFETY: the reader owns the slot in READING; the writer never touches READING slots.
        unsafe { std::slice::from_raw_parts(self.r.m.slot_ptr(self.i), len) }
    }
}

impl Drop for ReadSlot<'_> {
    fn drop(&mut self) {
        let s = &self.r.m.header().slots[self.i];
        let _ = s.state.compare_exchange(
            self.word,
            (gen_of(self.word) << 8) | FREE,
            Ordering::Release,
            Ordering::Relaxed,
        );
    }
}
