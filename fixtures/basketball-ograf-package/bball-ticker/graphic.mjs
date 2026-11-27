/**
 * Basketball – Score Ticker (OGraf v1)
 *
 *  [AROUND THE CONFERENCE ◣ WESTFIELD 71   NORTH VALLEY 64   FINAL            ]
 *
 * Flips through `items` every `dwellSeconds` while on air. Each item is either a
 * score line (team1/score1/team2/score2/status) or a free `text` line.
 * On FINAL scores the losing side is dimmed.
 */
import { OGrafGraphic, EASE, esc, contrastText, safeColor } from './lib/ograf-base.mjs';
import defaults from './defaults.mjs';

const isNum = (v) => /^\d+$/.test(String(v ?? '').trim());

export default class BasketballTicker extends OGrafGraphic {
  static stepCount = 1;
  static defaults = defaults;
  static moduleUrl = import.meta.url;

  styles() {
    return `
.tk {
  position: absolute; left: 96px; right: 96px; bottom: 54px; height: 58px;
  display: flex; opacity: 0;
  filter: drop-shadow(0 6px 18px rgba(0,0,0,.45));
}
.label {
  flex: 0 0 auto; display: flex; align-items: center; padding: 0 46px 0 26px;
  background: var(--accent); color: var(--accent-fg);
  font-weight: 800; font-size: 28px; letter-spacing: .1em; text-transform: uppercase; white-space: nowrap;
  clip-path: polygon(0 0, 100% 0, calc(100% - 22px) 100%, 0 100%);
  position: relative; z-index: 1;
}
.track { flex: 1; position: relative; overflow: hidden; background: rgba(17,19,23,.94); margin-left: -22px; }
.item {
  position: absolute; inset: 0; padding: 0 30px 0 52px;
  display: flex; align-items: center; gap: 16px; white-space: nowrap;
}
.t { font-weight: 700; font-size: 32px; text-transform: uppercase; letter-spacing: .02em; }
.sc { font-weight: 800; font-size: 36px; min-width: 44px; text-align: right; }
.gap { width: 30px; }
.dim { opacity: .5; }
.st { margin-left: 26px; font-weight: 700; font-size: 26px; color: var(--accent); letter-spacing: .08em; text-transform: uppercase; }
.txt { font-family: 'Barlow', sans-serif; font-weight: 600; font-size: 26px; letter-spacing: .08em; text-transform: uppercase; }
.count { position: absolute; right: 16px; bottom: 6px; display: flex; gap: 5px; }
.dot { width: 6px; height: 6px; border-radius: 50%; background: rgba(255,255,255,.25); }
.dot.on { background: var(--accent); }
`;
  }

  template() {
    return `<div class="tk"><div class="label"></div><div class="track"><div class="count"></div></div></div>`;
  }

  _items(s = this.state) {
    return (Array.isArray(s.items) ? s.items : []).filter((i) => i && (i.text || i.team1 || i.team2));
  }

  _itemHtml(it) {
    if (it.text && !it.team1 && !it.team2) return `<span class="txt">${esc(it.text)}</span>`;
    const final = /final/i.test(it.status || '') && isNum(it.score1) && isNum(it.score2);
    const s1 = +it.score1, s2 = +it.score2;
    const d1 = final && s1 < s2 ? ' dim' : '';
    const d2 = final && s2 < s1 ? ' dim' : '';
    return `
      <span class="t${d1}">${esc(it.team1)}</span><span class="sc${d1}">${esc(it.score1)}</span>
      <span class="gap"></span>
      <span class="t${d2}">${esc(it.team2)}</span><span class="sc${d2}">${esc(it.score2)}</span>
      <span class="st">${esc(it.status)}</span>`;
  }

  _renderCurrent() {
    const items = this._items();
    const track = this.$('.track');
    let cur = track.querySelector('.item.current');
    if (!cur) {
      cur = document.createElement('div');
      cur.className = 'item current';
      track.prepend(cur);
    }
    const html = items.length ? this._itemHtml(items[this.index % items.length]) : '';
    if (cur.dataset.html !== html) { cur.innerHTML = html; cur.dataset.html = html; }
    this._renderDots();
  }

  _renderDots() {
    const n = this._items().length;
    const dots = this.$('.count');
    dots.innerHTML = n > 1 ? Array.from({ length: n }, (_, i) => `<div class="dot${i === this.index % n ? ' on' : ''}"></div>`).join('') : '';
  }

  apply(s, { prev, initial }) {
    const accent = safeColor(s.accentColor, defaults.accentColor);
    this.vars('.tk', { '--accent': accent, '--accent-fg': contrastText(accent) });
    this.setText('.label', s.label);
    this.$('.label').classList.toggle('hidden', !String(s.label ?? '').trim());
    if (initial) this.index = 0;
    const n = this._items(s).length;
    if (n) this.index = this.index % n;
    this._renderCurrent();
    if (!initial && (s.paused !== prev.paused || s.dwellSeconds !== prev.dwellSeconds) && this.visible) this._schedule();
  }

  _schedule() {
    if (this._timer) this.clearTimer(this._timer);
    this._timer = null;
    if (!this.visible || this.state.paused || this._items().length < 2) return;
    const ms = Math.max(1, Number(this.state.dwellSeconds) || 6) * 1000;
    this._timer = this.setTimer(() => this._flip(1, false), ms);
  }

  async _flip(dir, skip) {
    const items = this._items();
    if (items.length < 2) { this._schedule(); return; }
    this.index = (this.index + dir + items.length) % items.length;
    const track = this.$('.track');
    const old = track.querySelector('.item.current');
    const next = document.createElement('div');
    next.className = 'item current';
    next.innerHTML = this._itemHtml(items[this.index]);
    next.dataset.html = next.innerHTML;
    track.prepend(next);
    this._renderDots();
    if (old) old.classList.remove('current');
    const from = dir > 0 ? '100%' : '-100%';
    const to = dir > 0 ? '-100%' : '100%';
    this._schedule();
    await Promise.all([
      old ? this.anim(old, [{ transform: 'translateY(0)', opacity: 1 }, { transform: `translateY(${to})`, opacity: 0 }], { duration: 500, easing: EASE.inOut }, skip) : null,
      this.anim(next, [{ transform: `translateY(${from})`, opacity: 0 }, { transform: 'translateY(0)', opacity: 1 }], { duration: 500, easing: EASE.inOut }, skip),
    ]);
    old?.remove();
  }

  onShown() { this._schedule(); }
  onHidden() { if (this._timer) this.clearTimer(this._timer); this._timer = null; }

  animateIn(_step, skip) {
    return Promise.all([
      this.anim(this.$('.tk'), [{ opacity: 1 }, { opacity: 1 }], { duration: 1 }, skip),
      this.anim(this.$('.label'), [{ transform: 'translateX(-40px)', opacity: 0 }, { transform: 'none', opacity: 1 }], { duration: 450 }, skip),
      this.anim(this.$('.track'), [{ clipPath: 'inset(0 100% 0 0)' }, { clipPath: 'inset(0 0% 0 0)' }], { delay: 150, duration: 650 }, skip),
      this.anim(this.$('.item.current'), [{ transform: 'translateY(100%)', opacity: 0 }, { transform: 'none', opacity: 1 }], { delay: 450, duration: 500 }, skip),
    ]);
  }

  animateOut(skip) {
    return Promise.all([
      this.anim(this.$('.item.current'), [{ opacity: 1 }, { opacity: 0 }], { duration: 200 }, skip),
      this.anim(this.$('.track'), [{ clipPath: 'inset(0 0% 0 0)' }, { clipPath: 'inset(0 100% 0 0)' }], { delay: 100, duration: 400, easing: EASE.in }, skip),
      this.anim(this.$('.label'), [{ transform: 'none', opacity: 1 }, { transform: 'translateX(-40px)', opacity: 0 }], { delay: 350, duration: 250, easing: EASE.in }, skip),
      this.anim(this.$('.tk'), [{ opacity: 1 }, { opacity: 0 }], { delay: 599, duration: 1 }, skip),
    ]).then(() => {
      const cur = this.$('.item.current');
      if (cur) cur.style.opacity = '';
    });
  }

  actions() {
    return {
      nextItem: async (_p, skip) => { if (this.visible) await this._flip(1, skip); else { this.index++; this._renderCurrent(); } },
      previousItem: async (_p, skip) => { if (this.visible) await this._flip(-1, skip); else { this.index = Math.max(0, this.index - 1); this._renderCurrent(); } },
    };
  }
}
