//! Patch geometry shared by the `fixtures/frame-oracle` graphic (JS) and the decoder. A change
//! here must be made in `fixtures/frame-oracle/graphic.mjs` too; the encoder below is the
//! reference the decoder's tests use.
//!
//! Cells are 16 × 16 px, opaque black (0) or white (1), large enough to survive NDI.
//! - **A** at the top-left and **B** at the bottom-right: 9 × 8 cells. Bits 0–23 `n`
//!   (LSB first), 24–63 `s` (rAF time in whole µs, 40 bits), 64 even parity over 0–63,
//!   65–71 the sync pattern `1010101`.
//! - **Alpha patch** below A (top half, so a bottom-half tear does not touch it): 5 × 5
//!   cells of white with alpha 255 (1) or 0 (0): bits 0–23 `n`, 24 parity. It is visible in
//!   KEY only.
//! - **Ramp** right of A: 16 steps of 16 × 16 px, white with alpha `17·i`.
//! - A moving 64 × 64 box (not decoded) so a frozen page is visible to a person.

pub const CELL: u32 = 16;
pub const PATCH_COLS: u32 = 9;
pub const PATCH_ROWS: u32 = 8;
pub const PATCH_W: u32 = PATCH_COLS * CELL;
pub const PATCH_H: u32 = PATCH_ROWS * CELL;
pub const ALPHA_COLS: u32 = 5;
pub const ALPHA_ORIGIN: (u32, u32) = (0, PATCH_H + CELL);
pub const RAMP_ORIGIN: (u32, u32) = (PATCH_W + CELL, 0);
pub const RAMP_STEPS: u32 = 16;
pub const SYNC: [bool; 7] = [true, false, true, false, true, false, true];
pub const N_BITS: u32 = 24;
pub const S_BITS: u32 = 40;

pub fn patch_a() -> (u32, u32) {
    (0, 0)
}

pub fn patch_b(w: u32, h: u32) -> (u32, u32) {
    (w - PATCH_W, h - PATCH_H)
}

/// The rectangles a dump must keep for the decoder (`--dump-crop`), `x,y,w,h`.
pub fn crops(w: u32, h: u32) -> Vec<[u32; 4]> {
    let (bx, by) = patch_b(w, h);
    vec![
        [0, 0, PATCH_W, PATCH_H],
        [
            ALPHA_ORIGIN.0,
            ALPHA_ORIGIN.1,
            ALPHA_COLS * CELL,
            ALPHA_COLS * CELL,
        ],
        [RAMP_ORIGIN.0, RAMP_ORIGIN.1, RAMP_STEPS * CELL, CELL],
        [bx, by, PATCH_W, PATCH_H],
    ]
}

/// The 72 cell values of patch A/B for `(n, s)`.
pub fn patch_bits(n: u32, s_us: u64) -> [bool; 72] {
    let mut b = [false; 72];
    for i in 0..N_BITS {
        b[i as usize] = (n >> i) & 1 == 1;
    }
    for i in 0..S_BITS {
        b[(N_BITS + i) as usize] = (s_us >> i) & 1 == 1;
    }
    b[64] = b[..64].iter().filter(|x| **x).count() % 2 == 1;
    b[65..72].copy_from_slice(&SYNC);
    b
}

/// The 25 cell values of the alpha patch for `n`.
pub fn alpha_bits(n: u32) -> [bool; 25] {
    let mut b = [false; 25];
    for i in 0..N_BITS {
        b[i as usize] = (n >> i) & 1 == 1;
    }
    b[24] = b[..24].iter().filter(|x| **x).count() % 2 == 1;
    b
}

/// Reference encoder: paint one oracle frame into premultiplied BGRA (as CEF delivers it).
/// `n_alpha` is normally `n`; tests pass another value to model a KEY from another paint.
pub fn paint(buf: &mut [u8], w: u32, h: u32, n: u32, s_us: u64, n_alpha: u32) {
    buf.fill(0);
    let mut rect = |x0: u32, y0: u32, cw: u32, ch: u32, px: [u8; 4]| {
        for y in y0..(y0 + ch).min(h) {
            for x in x0..(x0 + cw).min(w) {
                let i = ((y * w + x) * 4) as usize;
                buf[i..i + 4].copy_from_slice(&px);
            }
        }
    };
    let bits = patch_bits(n, s_us);
    for origin in [patch_a(), patch_b(w, h)] {
        for (i, bit) in bits.iter().enumerate() {
            let (cx, cy) = (i as u32 % PATCH_COLS, i as u32 / PATCH_COLS);
            let v = if *bit { 255 } else { 0 };
            rect(
                origin.0 + cx * CELL,
                origin.1 + cy * CELL,
                CELL,
                CELL,
                [v, v, v, 255],
            );
        }
    }
    for (i, bit) in alpha_bits(n_alpha).iter().enumerate() {
        let (cx, cy) = (i as u32 % ALPHA_COLS, i as u32 / ALPHA_COLS);
        let a = if *bit { 255 } else { 0 };
        rect(
            ALPHA_ORIGIN.0 + cx * CELL,
            ALPHA_ORIGIN.1 + cy * CELL,
            CELL,
            CELL,
            [a, a, a, a],
        );
    }
    for i in 0..RAMP_STEPS {
        let a = (17 * i) as u8;
        rect(
            RAMP_ORIGIN.0 + i * CELL,
            RAMP_ORIGIN.1,
            CELL,
            CELL,
            [a, a, a, a],
        );
    }
    let bx = (RAMP_ORIGIN.0 + (n * 8) % (w.saturating_sub(RAMP_ORIGIN.0 + 64)).max(1)).min(w - 64);
    rect(bx, h / 2 - 32, 64, 64, [255, 255, 255, 255]);
}
