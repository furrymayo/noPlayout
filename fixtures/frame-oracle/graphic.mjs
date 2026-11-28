// noPlayout frame oracle (PLAN §5.5). Geometry and bit order must match
// crates/noplayout-oracle/src/layout.rs (the Rust side is the reference encoder its tests use).
//
// Each requestAnimationFrame callback increments n and records s = the callback's rAF
// timestamp in whole µs (source domain), then draws:
// - patches A (top-left) and B (bottom-right): 9 × 8 cells of 16 px, opaque black/white:
//   bits 0–23 n (LSB first), 24–63 s, 64 even parity over 0–63, 65–71 sync 1010101;
// - the alpha patch one cell below A: 5 × 5 cells, white at alpha 255 / 0: bits 0–23 n, 24 parity;
// - the ramp one cell right of A: 16 cells, white at alpha 17·i;
// - a moving 64 × 64 white box at mid-height.
// Drawing starts at playAction and stops at stopAction; load paints nothing.

const CELL = 16;
const COLS = 9;
const ROWS = 8;
const PATCH_W = COLS * CELL;
const PATCH_H = ROWS * CELL;
const ALPHA_COLS = 5;
const SYNC = [1, 0, 1, 0, 1, 0, 1];

function bits(n, s) {
  const b = [];
  for (let i = 0; i < 24; i++) b.push(Math.floor(n / 2 ** i) % 2);
  for (let i = 0; i < 40; i++) b.push(Number((s >> BigInt(i)) & 1n));
  b.push(b.reduce((x, y) => x + y, 0) % 2);
  return b.concat(SYNC);
}

function alphaBits(n) {
  const b = [];
  for (let i = 0; i < 24; i++) b.push(Math.floor(n / 2 ** i) % 2);
  b.push(b.reduce((x, y) => x + y, 0) % 2);
  return b;
}

class FrameOracle extends HTMLElement {
  constructor() {
    super();
    this.currentStep = undefined;
    this.n = 0;
    this.raf = 0;
  }

  async load(params) {
    if (params.renderType !== "realtime") {
      return { statusCode: 400, statusMessage: "realtime only" };
    }
    this.canvas = document.createElement("canvas");
    this.canvas.width = window.innerWidth;
    this.canvas.height = window.innerHeight;
    this.canvas.style.cssText = "position:absolute;left:0;top:0;width:100%;height:100%";
    this.appendChild(this.canvas);
    this.ctx = this.canvas.getContext("2d", { alpha: true });
    return { statusCode: 200 };
  }

  frame(t) {
    this.n = (this.n + 1) % 2 ** 24;
    const s = BigInt(Math.floor(t * 1000)) & ((1n << 40n) - 1n);
    const { ctx, canvas } = this;
    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);
    const b = bits(this.n, s);
    for (const [ox, oy] of [[0, 0], [w - PATCH_W, h - PATCH_H]]) {
      b.forEach((bit, i) => {
        ctx.fillStyle = bit ? "#ffffff" : "#000000";
        ctx.fillRect(ox + (i % COLS) * CELL, oy + Math.floor(i / COLS) * CELL, CELL, CELL);
      });
    }
    alphaBits(this.n).forEach((bit, i) => {
      if (bit) {
        ctx.fillStyle = "#ffffff";
        ctx.fillRect((i % ALPHA_COLS) * CELL, PATCH_H + CELL + Math.floor(i / ALPHA_COLS) * CELL, CELL, CELL);
      }
    });
    for (let i = 0; i < 16; i++) {
      ctx.fillStyle = `rgba(255,255,255,${(17 * i) / 255})`;
      ctx.fillRect(PATCH_W + CELL + i * CELL, 0, CELL, CELL);
    }
    const x0 = PATCH_W + CELL;
    const span = Math.max(1, w - x0 - 64);
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(Math.min(w - 64, x0 + ((this.n * 8) % span)), h / 2 - 32, 64, 64);
    this.raf = requestAnimationFrame((t2) => this.frame(t2));
  }

  async playAction() {
    this.currentStep = 0;
    if (!this.raf) this.raf = requestAnimationFrame((t) => this.frame(t));
    return { statusCode: 200, currentStep: this.currentStep };
  }

  async stopAction() {
    cancelAnimationFrame(this.raf);
    this.raf = 0;
    this.currentStep = undefined;
    this.ctx?.clearRect(0, 0, this.canvas.width, this.canvas.height);
    return { statusCode: 200 };
  }

  async updateAction() {
    return { statusCode: 200 };
  }

  async customAction() {
    return { statusCode: 404, statusMessage: "no custom actions" };
  }

  async dispose() {
    cancelAnimationFrame(this.raf);
    this.raf = 0;
    this.innerHTML = "";
    return { statusCode: 200 };
  }
}

export default FrameOracle;
