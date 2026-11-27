/**
 * ograf-base.mjs — shared foundation for the Basketball OGraf package.
 *
 * Implements the OGraf v1 Graphics web-component interface
 * (load / dispose / playAction / stopAction / updateAction / customAction)
 * so each graphic only has to supply markup, styles, state rendering and
 * its in / out / step animations.
 *
 * Design rules used throughout the package:
 *  - Everything is laid out on a 1920x1080 stage that is scaled to whatever
 *    size the renderer gives the element (720p, 4K, etc. all work).
 *  - Animations use the Web Animations API (no third-party libraries).
 *    Each animate* hook starts all of its animations at once (staggered with
 *    `delay`), so a new play/stop can "skip ahead" by finishing them instantly.
 *  - skipAnimation is honoured everywhere: animations run with 0 duration.
 *
 * Spec: https://ograf.ebu.io/v1/specification/docs/Specification
 */

/* ------------------------------------------------------------------ fonts */

const FONT_FILES = [
  ['Barlow Condensed', 500, 'barlow-condensed-latin-500-normal.woff2'],
  ['Barlow Condensed', 600, 'barlow-condensed-latin-600-normal.woff2'],
  ['Barlow Condensed', 700, 'barlow-condensed-latin-700-normal.woff2'],
  ['Barlow Condensed', 800, 'barlow-condensed-latin-800-normal.woff2'],
  ['Barlow', 500, 'barlow-latin-500-normal.woff2'],
  ['Barlow', 600, 'barlow-latin-600-normal.woff2'],
  ['Barlow', 700, 'barlow-latin-700-normal.woff2'],
];

let fontsPromise = null;

/** Registers the bundled fonts on the document (fonts declared inside a shadow root are ignored by browsers). */
export function loadFonts() {
  if (!fontsPromise) {
    fontsPromise = Promise.all(
      FONT_FILES.map(async ([family, weight, file]) => {
        const exists = [...document.fonts].some(
          (f) => f.family.replace(/["']/g, '') === family && String(f.weight) === String(weight) && f.status === 'loaded'
        );
        if (exists) return;
        try {
          const face = new FontFace(family, `url("${new URL('./fonts/' + file, import.meta.url).href}")`, {
            weight: String(weight),
            style: 'normal',
          });
          document.fonts.add(face);
          await face.load();
        } catch (err) {
          console.warn('[ograf-bball] could not load font', file, err);
        }
      })
    );
  }
  return fontsPromise;
}

/* ---------------------------------------------------------------- helpers */

export const EASE = {
  out: 'cubic-bezier(0.16, 1, 0.3, 1)', // expo-out: fast start, soft landing
  outBack: 'cubic-bezier(0.34, 1.4, 0.64, 1)',
  in: 'cubic-bezier(0.7, 0, 0.84, 0)',
  inOut: 'cubic-bezier(0.83, 0, 0.17, 1)',
  linear: 'linear',
};

export function esc(value) {
  return String(value ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function hexToRgb(hex) {
  const m = /^#?([0-9a-f]{6})$/i.exec(String(hex || '').trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** Black or white, whichever reads better on the given colour. */
export function contrastText(hex, dark = '#0c0e12', light = '#ffffff') {
  const rgb = hexToRgb(hex);
  if (!rgb) return light;
  const lin = rgb.map((c) => {
    c /= 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  const L = 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
  // contrast against white vs against near-black
  return (1.05 / (L + 0.05)) >= ((L + 0.05) / 0.055) ? light : dark;
}

/** Lighten (amt > 0) or darken (amt < 0) a hex colour, amt in -1..1. */
export function shade(hex, amt) {
  const rgb = hexToRgb(hex);
  if (!rgb) return hex;
  const out = rgb.map((c) => Math.round(amt >= 0 ? c + (255 - c) * amt : c * (1 + amt)));
  return '#' + out.map((c) => c.toString(16).padStart(2, '0')).join('');
}

export function safeColor(hex, fallback) {
  return hexToRgb(hex) ? hex : fallback;
}

/** Resolve an image path relative to the graphic's folder (absolute URLs and data: URIs pass through). */
export function resolveAsset(path, baseUrl) {
  if (!path) return '';
  try {
    return new URL(path, baseUrl).href;
  } catch {
    return '';
  }
}

/** "10:00" -> 600000, "45.3" -> 45300, "1:02.5" -> 62500 */
export function parseClock(text) {
  if (typeof text === 'number') return Math.max(0, text * 1000);
  const s = String(text ?? '').trim();
  if (!s) return 0;
  const parts = s.split(':');
  let secs = 0;
  for (const p of parts) secs = secs * 60 + (parseFloat(p) || 0);
  return Math.max(0, Math.round(secs * 1000));
}

/** Game clock: "M:SS" at a minute or more, "SS.t" under a minute. */
export function formatGameClock(ms) {
  ms = Math.max(0, ms);
  if (ms >= 60000) {
    const total = Math.ceil(ms / 1000);
    return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
  }
  return (Math.floor(ms / 100) / 10).toFixed(1);
}

/** Shot clock: whole seconds, tenths under 5 seconds if enabled. */
export function formatShotClock(ms, tenths = true) {
  ms = Math.max(0, ms);
  if (tenths && ms < 5000 && ms > 0) return (Math.floor(ms / 100) / 10).toFixed(1);
  return String(Math.ceil(ms / 1000));
}

/* ------------------------------------------------------------ base styles */

const BASE_CSS = `
:host {
  display: block;
  position: absolute;
  inset: 0;
  overflow: hidden;
  pointer-events: none;
  contain: strict;
}
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
.stage {
  position: absolute;
  left: 0; top: 0;
  width: 1920px; height: 1080px;
  transform-origin: 0 0;
  font-family: 'Barlow Condensed', 'Arial Narrow', sans-serif;
  color: #fff;
  font-variant-numeric: tabular-nums;
  -webkit-font-smoothing: antialiased;
  text-rendering: geometricPrecision;
}
.hidden { display: none !important; }
img { display: block; object-fit: contain; }
.upper { text-transform: uppercase; }
`;

/* ------------------------------------------------------------- base class */

export class OGrafGraphic extends HTMLElement {
  /** Override in subclasses. */
  static stepCount = 1;
  static defaults = {};
  /** Subclasses set this to their own import.meta.url so asset paths resolve to their folder. */
  static moduleUrl = import.meta.url;

  constructor() {
    super();
    this.root = this.attachShadow({ mode: 'open' });
    this.state = {};
    this.visible = false;
    this.currentStep = undefined;
    this._anims = new Set();
    this._timers = new Set();
    this._intervals = new Set();
  }

  /* ---------- hooks (override) ---------- */
  styles() { return ''; }
  template() { return ''; }
  /** Render state into the DOM. ctx = { prev, skip, initial } */
  apply(_state, _ctx) {}
  animateIn(_step, _skip) { return Promise.resolve(); }
  animateOut(_skip) { return Promise.resolve(); }
  animateStep(_from, _to, _skip) { return Promise.resolve(); }
  /** Return { actionId: (payload, skip) => Promise } */
  actions() { return {}; }
  onShown() {}
  onHidden() {}
  onDispose() {}

  /* ---------- utilities for subclasses ---------- */
  $(sel) { return this.root.querySelector(sel); }
  $$(sel) { return [...this.root.querySelectorAll(sel)]; }
  asset(path) { return resolveAsset(path, this.constructor.moduleUrl); }

  setText(sel, text) {
    const el = typeof sel === 'string' ? this.$(sel) : sel;
    if (el && el.textContent !== String(text ?? '')) el.textContent = String(text ?? '');
    return el;
  }

  setImage(sel, path) {
    const el = typeof sel === 'string' ? this.$(sel) : sel;
    if (!el) return false;
    const url = this.asset(path);
    if (!url) {
      el.removeAttribute('src');
      el.classList.add('hidden');
      return false;
    }
    if (el.getAttribute('src') !== url) {
      el.classList.remove('hidden');
      el.onerror = () => el.classList.add('hidden');
      el.onload = () => el.classList.remove('hidden');
      el.setAttribute('src', url);
    }
    return true;
  }

  vars(el, map) {
    const target = typeof el === 'string' ? this.$(el) : el;
    if (!target) return;
    for (const [k, v] of Object.entries(map)) target.style.setProperty(k, v);
  }

  /**
   * Run a Web Animation. Returns a promise that resolves when it finishes
   * (or is skipped). Final styles are committed inline so they persist.
   */
  anim(el, keyframes, opts = {}, skip = false) {
    if (!el) return Promise.resolve();
    const a = el.animate(keyframes, {
      duration: skip ? 0 : opts.duration ?? 500,
      delay: skip ? 0 : opts.delay ?? 0,
      easing: opts.easing ?? EASE.out,
      fill: 'both',
    });
    this._anims.add(a);
    const last = keyframes[keyframes.length - 1] || {};
    return a.finished
      .then(() => {
        // Persist the end state as inline styles, then drop the animation.
        // (Writing the final keyframe is more reliable than commitStyles() for very short animations.)
        for (const [prop, value] of Object.entries(last)) {
          if (prop === 'offset' || prop === 'easing' || prop === 'composite') continue;
          el.style[prop] = value;
        }
        a.cancel();
      })
      .catch(() => {})
      .finally(() => this._anims.delete(a));
  }

  /** Animate many elements with the same keyframes and a stagger. */
  stagger(els, keyframes, opts = {}, skip = false) {
    const step = opts.stagger ?? 60;
    return Promise.all(
      els.map((el, i) => this.anim(el, keyframes, { ...opts, delay: (opts.delay ?? 0) + i * step }, skip))
    );
  }

  /** Count a number up inside an element, synced to the animation clock so it can be skipped. */
  countUp(el, from, to, opts = {}, skip = false) {
    if (!el) return Promise.resolve();
    const decimals = opts.decimals ?? 0;
    const fmt = (v) => (opts.format ? opts.format(v) : v.toFixed(decimals));
    if (skip || from === to) {
      el.textContent = fmt(to);
      return Promise.resolve();
    }
    const driver = el.animate([{ opacity: 1 }, { opacity: 1 }], {
      duration: opts.duration ?? 900,
      delay: opts.delay ?? 0,
      easing: 'linear',
    });
    this._anims.add(driver);
    const ease = (t) => 1 - Math.pow(1 - t, 3);
    let raf = 0;
    const frame = () => {
      const p = driver.effect.getComputedTiming().progress;
      if (p != null) el.textContent = fmt(from + (to - from) * ease(p));
      raf = requestAnimationFrame(frame);
    };
    el.textContent = fmt(from);
    raf = requestAnimationFrame(frame);
    return driver.finished
      .catch(() => {})
      .finally(() => {
        cancelAnimationFrame(raf);
        el.textContent = fmt(to);
        this._anims.delete(driver);
      });
  }

  /** Jump every running animation to its end state. */
  finishAll() {
    for (const a of [...this._anims]) {
      try { a.finish(); } catch { /* infinite or already done */ }
    }
  }

  setTimer(fn, ms) {
    const id = setTimeout(() => { this._timers.delete(id); fn(); }, ms);
    this._timers.add(id);
    return id;
  }
  clearTimer(id) { clearTimeout(id); this._timers.delete(id); }
  setRepeat(fn, ms) {
    const id = setInterval(fn, ms);
    this._intervals.add(id);
    return id;
  }
  clearRepeat(id) { clearInterval(id); this._intervals.delete(id); }

  async whenImagesReady(timeout = 3000) {
    const imgs = this.$$('img[src]').filter((i) => !i.complete);
    if (!imgs.length) return;
    await Promise.race([
      Promise.all(imgs.map((i) => new Promise((r) => { i.addEventListener('load', r, { once: true }); i.addEventListener('error', r, { once: true }); }))),
      new Promise((r) => setTimeout(r, timeout)),
    ]);
  }

  _fitStage() {
    const stage = this.$('.stage');
    if (!stage) return;
    const w = this.clientWidth || window.innerWidth || 1920;
    const h = this.clientHeight || window.innerHeight || 1080;
    const s = Math.min(w / 1920, h / 1080);
    const ox = (w - 1920 * s) / 2;
    const oy = (h - 1080 * s) / 2;
    stage.style.transform = `translate(${ox}px, ${oy}px) scale(${s})`;
  }

  /* ---------- OGraf interface ---------- */

  async load(params = {}) {
    this.renderType = params.renderType || 'realtime';
    await loadFonts();
    this.root.innerHTML = `<style>${BASE_CSS}${this.styles()}</style><div class="stage">${this.template()}</div>`;
    this._fitStage();
    this._ro = new ResizeObserver(() => this._fitStage());
    this._ro.observe(this);
    this.visible = false;
    this.currentStep = undefined;
    this.state = { ...this.constructor.defaults, ...(params.data || {}) };
    await this.apply(this.state, { prev: {}, skip: true, initial: true });
    await this.whenImagesReady();
    return { statusCode: 200 };
  }

  async dispose() {
    for (const a of [...this._anims]) { try { a.cancel(); } catch { /* noop */ } }
    this._anims.clear();
    for (const id of this._timers) clearTimeout(id);
    for (const id of this._intervals) clearInterval(id);
    this._timers.clear();
    this._intervals.clear();
    this._ro?.disconnect();
    this.onDispose();
    this.root.innerHTML = '';
    this.visible = false;
    this.currentStep = undefined;
    return { statusCode: 200 };
  }

  async playAction(params = {}) {
    const skip = !!params.skipAnimation;
    const count = this.constructor.stepCount;
    let target =
      Number.isInteger(params.goto) && params.goto >= 0
        ? params.goto
        : (this.currentStep ?? -1) + (Number.isInteger(params.delta) ? params.delta : 1);

    if (count >= 0 && target >= count) {
      await this.stopAction({ skipAnimation: skip });
      return { statusCode: 200, currentStep: undefined };
    }
    if (target < 0) target = 0;

    this.finishAll();
    if (!this.visible) {
      this.visible = true;
      this.currentStep = target;
      this.onShown();
      await this.animateIn(target, skip);
    } else if (target !== this.currentStep) {
      const from = this.currentStep;
      this.currentStep = target;
      await this.animateStep(from, target, skip);
    }
    return { statusCode: 200, currentStep: this.currentStep };
  }

  async stopAction(params = {}) {
    const skip = !!params.skipAnimation;
    this.finishAll();
    if (!this.visible) return { statusCode: 200 };
    this.visible = false;
    this.currentStep = undefined;
    this.onHidden();
    await this.animateOut(skip);
    return { statusCode: 200 };
  }

  async updateAction(params = {}) {
    const prev = this.state;
    this.state = { ...prev, ...(params.data || {}) };
    await this.apply(this.state, { prev, skip: !!params.skipAnimation || !this.visible, initial: false });
    await this.whenImagesReady();
    return { statusCode: 200 };
  }

  async customAction(params = {}) {
    const fn = this.actions()[params.id];
    if (!fn) return { statusCode: 400, statusMessage: `Unknown custom action "${params.id}"` };
    const result = await fn.call(this, params.payload || {}, !!params.skipAnimation);
    return result || { statusCode: 200 };
  }
}
