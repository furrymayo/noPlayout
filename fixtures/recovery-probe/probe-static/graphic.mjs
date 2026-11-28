// noPlayout recovery probe, static variant (PLAN §2.13.6, rev 4). No timer, no rAF loop, no
// time read: it repaints only from load, updateAction, playAction, stopAction and customAction.
// Its sidecar may honestly declare `time: none`. probe-timed/graphic.mjs is the same graphic plus
// a clock; keep the two in step.
//
// Patch P (top-left), the frame-oracle patch geometry (crates/noplayout-oracle/src/layout.rs):
// 9 × 8 cells of 16 px, opaque black/white. Bits 0–15 the custom-action counter, 16–47 the
// FNV-1a 32 hash of JSON.stringify(data), 48–55 currentStep + 1 (0 = undefined), 56–63 zero,
// 64 even parity over 0–63, 65–71 sync 1010101. Decoded as the oracle's (n, s):
// n = counter | (hash & 0xff) << 16, s = hash >> 8 | step << 24.

const CELL = 16;
const COLS = 9;
const SYNC = [1, 0, 1, 0, 1, 0, 1];

function fnv1a(text) {
  let h = 0x811c9dc5;
  for (const b of new TextEncoder().encode(text)) {
    h ^= b;
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h;
}

function field(bits, value, width) {
  for (let i = 0; i < width; i++) bits.push(Math.floor(value / 2 ** i) % 2);
}

function drawPatch(ctx, x0, y0, words) {
  const b = [];
  for (const [v, w] of words) field(b, v, w);
  while (b.length < 64) b.push(0);
  b.push(b.reduce((x, y) => x + y, 0) % 2);
  b.push(...SYNC);
  b.forEach((bit, i) => {
    ctx.fillStyle = bit ? "#ffffff" : "#000000";
    ctx.fillRect(x0 + (i % COLS) * CELL, y0 + Math.floor(i / COLS) * CELL, CELL, CELL);
  });
}

class RecoveryProbe extends HTMLElement {
  constructor() {
    super();
    this.currentStep = undefined;
    this.counter = 0;
    this.hash = 0;
  }

  paint() {
    const { ctx, canvas } = this;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    const step = this.currentStep === undefined ? 0 : this.currentStep + 1;
    drawPatch(ctx, 0, 0, [
      [this.counter, 16],
      [this.hash, 32],
      [step, 8],
    ]);
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
    this.hash = fnv1a(JSON.stringify(params.data ?? {}));
    this.paint();
    return { statusCode: 200 };
  }

  async updateAction(params) {
    this.hash = fnv1a(JSON.stringify(params.data ?? {}));
    this.paint();
    return { statusCode: 200 };
  }

  async playAction() {
    this.currentStep = 0;
    this.paint();
    return { statusCode: 200, currentStep: this.currentStep };
  }

  async stopAction() {
    this.currentStep = undefined;
    this.paint();
    return { statusCode: 200 };
  }

  async customAction(params) {
    if (params.id !== "bump") return { statusCode: 404, statusMessage: "unknown action" };
    this.counter = (this.counter + 1) % 65536;
    this.paint();
    return { statusCode: 200, result: { counter: this.counter } };
  }

  async dispose() {
    this.innerHTML = "";
    return { statusCode: 200 };
  }
}

export default RecoveryProbe;
