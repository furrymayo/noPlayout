//! Decoding the frame-oracle patches from pixels (PLAN §5.5). Works on FILL (straight BGRA,
//! as the output process converts it) and KEY (one byte per pixel) planes, whole or cropped.

use crate::layout::*;

/// A view of one frame's plane: either the whole frame or the dump's crop rectangles.
pub struct Plane<'a> {
    pub width: u32,
    pub height: u32,
    /// Bytes per pixel: 4 (FILL, BGRA) or 1 (KEY).
    pub bpp: u32,
    pub data: &'a [u8],
    /// `x,y,w,h` rectangles stored back to back; empty = the whole frame.
    pub crops: &'a [[u32; 4]],
}

impl Plane<'_> {
    /// The pixel's value: luma-ish (green) for FILL, the byte for KEY.
    pub fn at(&self, x: u32, y: u32) -> Option<u8> {
        let ch = if self.bpp == 4 { 1 } else { 0 };
        if self.crops.is_empty() {
            if x >= self.width || y >= self.height {
                return None;
            }
            let i = ((y * self.width + x) * self.bpp) as usize + ch;
            return self.data.get(i).copied();
        }
        let mut base = 0usize;
        for &[cx, cy, cw, chh] in self.crops {
            let cw = cw.min(self.width.saturating_sub(cx));
            let chh = chh.min(self.height.saturating_sub(cy));
            if x >= cx && x < cx + cw && y >= cy && y < cy + chh {
                let i = base + (((y - cy) * cw + (x - cx)) * self.bpp) as usize + ch;
                return self.data.get(i).copied();
            }
            base += (cw * chh * self.bpp) as usize;
        }
        None
    }

    /// Mean of the 4 × 4 centre of a cell, thresholded at half scale.
    fn cell(&self, x0: u32, y0: u32) -> Option<bool> {
        let mut sum = 0u32;
        for dy in 6..10 {
            for dx in 6..10 {
                sum += u32::from(self.at(x0 + dx, y0 + dy)?);
            }
        }
        Some(sum / 16 >= 128)
    }

    /// Mean of a cell's 8 × 8 centre (for the ramp).
    fn mean(&self, x0: u32, y0: u32) -> Option<f64> {
        let mut sum = 0u32;
        for dy in 4..12 {
            for dx in 4..12 {
                sum += u32::from(self.at(x0 + dx, y0 + dy)?);
            }
        }
        Some(f64::from(sum) / 64.0)
    }
}

fn bits_u64(bits: &[bool]) -> u64 {
    bits.iter()
        .enumerate()
        .fold(0u64, |acc, (i, b)| acc | (u64::from(*b) << i))
}

/// Patch A or B: `(n, s_us)`, or `None` if unreadable (sync or parity wrong).
pub fn read_patch(fill: &Plane<'_>, origin: (u32, u32)) -> Option<(u32, u64)> {
    let mut bits = [false; 72];
    for (i, b) in bits.iter_mut().enumerate() {
        let (cx, cy) = (i as u32 % PATCH_COLS, i as u32 / PATCH_COLS);
        *b = fill.cell(origin.0 + cx * CELL, origin.1 + cy * CELL)?;
    }
    let parity = bits[..64].iter().filter(|x| **x).count() % 2 == 1;
    if bits[65..72] != SYNC || bits[64] != parity {
        return None;
    }
    Some((bits_u64(&bits[..24]) as u32, bits_u64(&bits[24..64])))
}

/// The alpha patch's `n` from KEY, or `None` if unreadable.
pub fn read_alpha(key: &Plane<'_>) -> Option<u32> {
    let mut bits = [false; 25];
    for (i, b) in bits.iter_mut().enumerate() {
        let (cx, cy) = (i as u32 % ALPHA_COLS, i as u32 / ALPHA_COLS);
        *b = key.cell(ALPHA_ORIGIN.0 + cx * CELL, ALPHA_ORIGIN.1 + cy * CELL)?;
    }
    let parity = bits[..24].iter().filter(|x| **x).count() % 2 == 1;
    (bits[24] == parity).then(|| bits_u64(&bits[..24]) as u32)
}

/// Ramp steps whose KEY level is off nominal by more than 1 LSB (only `a ≥ 16` is judged).
pub fn ramp_errors(key: &Plane<'_>) -> u32 {
    (0..RAMP_STEPS)
        .filter(|&i| {
            let nominal = f64::from(17 * i);
            nominal >= 16.0
                && key
                    .mean(RAMP_ORIGIN.0 + i * CELL, RAMP_ORIGIN.1)
                    .is_none_or(|m| (m - nominal).abs() > 1.0)
        })
        .count() as u32
}

/// What a recovery-probe page shows (`fixtures/recovery-probe`, PLAN §2.13.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    pub counter: u16,
    /// FNV-1a 32 of `JSON.stringify(data)`.
    pub hash: u32,
    /// `currentStep`; `None` = undefined (stopped).
    pub step: Option<u8>,
}

/// Probe patch P (top-left, the frame-oracle geometry): `n = counter | (hash & 0xff) << 16`,
/// `s = hash >> 8 | (step + 1) << 24`.
pub fn read_probe(fill: &Plane<'_>) -> Option<Probe> {
    let (n, s) = read_patch(fill, (0, 0))?;
    let step = ((s >> 24) & 0xff) as u8;
    Some(Probe {
        counter: (n & 0xffff) as u16,
        hash: ((n >> 16) & 0xff) | ((s & 0xff_ffff) as u32) << 8,
        step: step.checked_sub(1),
    })
}

/// `probe-timed`'s clock patch (bottom-right): deciseconds since its `load`.
pub fn read_probe_clock(fill: &Plane<'_>) -> Option<u32> {
    read_patch(fill, patch_b(fill.width, fill.height)).map(|(n, _)| n)
}

/// FNV-1a 32, as the probe hashes its data.
pub fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5u32, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(0x0100_0193)
    })
}
