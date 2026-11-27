/**
 * Basketball – Player Stat Card (OGraf v1)
 *
 *  ▌[23][ JORDAN MITCHELL                 | TONIGHT ]
 *  ▌    [ 18  ][  9  ][  4  ][ 7-12 ][ 2-4 ]
 *  ▌    [ PTS ][ REB ][ AST ][  FG  ][ 3PT ]
 *  ▌    [ CAREER-HIGH 9 REBOUNDS            ]
 *
 * Numeric stat values count up when the card animates in or a value changes.
 */
import { OGrafGraphic, EASE, esc, contrastText, shade, safeColor } from './lib/ograf-base.mjs';
import defaults from './defaults.mjs';

const NUMERIC = /^-?\d+(\.\d+)?$/;
const decimalsOf = (v) => (String(v).split('.')[1] || '').length;

export default class BasketballStatCard extends OGrafGraphic {
  static stepCount = 1;
  static defaults = defaults;
  static moduleUrl = import.meta.url;

  styles() {
    return `
.card {
  position: absolute; left: 96px; bottom: 196px;
  display: grid; grid-template-columns: 8px auto auto; grid-template-rows: 96px auto auto;
  opacity: 0;
  filter: drop-shadow(0 8px 22px rgba(0,0,0,.45));
}
.accentbar { grid-row: 1 / span 3; background: var(--accent); transform-origin: 50% 100%; }
.numbox { width: 96px; background: var(--team); display: flex; align-items: center; justify-content: center; overflow: hidden; }
.num { font-weight: 800; font-size: 62px; color: var(--team-fg); line-height: 1; }
.head {
  display: flex; align-items: stretch;
  background: linear-gradient(90deg, #111317, #1a1d22);
}
.names { display: flex; align-items: baseline; gap: 14px; padding: 0 28px; align-self: center; flex: 1; white-space: nowrap; }
.first { font-weight: 600; font-size: 36px; letter-spacing: .1em; color: rgba(255,255,255,.72); text-transform: uppercase; }
.last { font-weight: 800; font-size: 60px; line-height: 1; text-transform: uppercase; }
.wm { width: 64px; height: 64px; align-self: center; margin-right: 22px; }
.title {
  display: flex; align-items: center; padding: 0 26px;
  background: var(--accent); color: var(--accent-fg);
  font-weight: 800; font-size: 28px; letter-spacing: .12em; text-transform: uppercase; white-space: nowrap;
}
.title:empty { display: none; }
.stats { grid-column: 2 / span 2; display: flex; gap: 2px; }
.stat {
  flex: 1 1 0; min-width: 150px; padding: 10px 0 12px;
  display: flex; flex-direction: column; align-items: center;
  background: #f4f5f7; color: #0c0e12; overflow: hidden;
}
.val { font-weight: 800; font-size: 74px; line-height: 1; }
.lbl { font-family: 'Barlow', sans-serif; font-weight: 700; font-size: 20px; letter-spacing: .14em; color: var(--team-ink); margin-top: 2px; }
.note {
  grid-column: 2 / span 2; height: 46px; padding: 0 28px;
  display: flex; align-items: center;
  background: var(--team-dk);
  font-family: 'Barlow', sans-serif; font-weight: 600; font-size: 22px; letter-spacing: .1em; text-transform: uppercase;
}
.card.nonote .note { display: none; }
`;
  }

  template() {
    return `
<div class="card">
  <div class="accentbar"></div>
  <div class="numbox"><span class="num"></span></div>
  <div class="head">
    <div class="names"><span class="first"></span><span class="last"></span></div>
    <img class="wm" alt="">
    <div class="title"></div>
  </div>
  <div class="stats"></div>
  <div class="note"></div>
</div>`;
  }

  _stats(s) {
    return (Array.isArray(s.stats) ? s.stats : []).filter((x) => x && (x.label || x.value)).slice(0, 5);
  }

  apply(s, { prev, skip, initial }) {
    const team = safeColor(s.teamColor, defaults.teamColor);
    const accent = safeColor(s.accentColor, defaults.accentColor);
    this.vars('.card', {
      '--team': team, '--team-fg': contrastText(team), '--team-dk': shade(team, -0.4),
      '--team-ink': contrastText(team) === '#ffffff' ? team : '#3a3f46',
      '--accent': accent, '--accent-fg': contrastText(accent),
    });
    this.setText('.num', s.number);
    this.$('.numbox').classList.toggle('hidden', !String(s.number ?? '').trim());
    this.setText('.first', s.firstName);
    this.setText('.last', s.lastName);
    this.setText('.title', s.title);
    this.setText('.note', s.note);
    this.setImage('.wm', s.teamLogo);
    this.$('.card').classList.toggle('nonote', !String(s.note ?? '').trim());

    const stats = this._stats(s);
    const box = this.$('.stats');
    const oldStats = initial ? [] : this._stats(prev);
    const sameShape = stats.length === box.children.length;
    if (!sameShape) {
      box.innerHTML = stats.map((st) => `<div class="stat"><div class="val">${esc(st.value)}</div><div class="lbl">${esc(st.label)}</div></div>`).join('');
      if (!skip) this.stagger([...box.children], [{ opacity: 0, transform: 'translateY(30px)' }, { opacity: 1, transform: 'none' }], { duration: 500, stagger: 60 });
      return;
    }
    stats.forEach((st, i) => {
      const el = box.children[i];
      this.setText(el.querySelector('.lbl'), st.label);
      const val = el.querySelector('.val');
      const before = oldStats[i]?.value;
      if (String(before) === String(st.value) && !initial) return;
      if (!skip && NUMERIC.test(String(st.value)) && NUMERIC.test(String(before ?? ''))) {
        this.countUp(val, parseFloat(before), parseFloat(st.value), { decimals: decimalsOf(st.value), duration: 700 });
        this.anim(el, [{ backgroundColor: getComputedStyle(this.$('.card')).getPropertyValue('--accent').trim() }, { backgroundColor: '#f4f5f7' }], { duration: 1000, easing: EASE.linear });
      } else {
        val.textContent = st.value;
        if (!skip) this.anim(val, [{ opacity: 0 }, { opacity: 1 }], { duration: 400 });
      }
    });
  }

  animateIn(_step, skip) {
    const wipe = [{ clipPath: 'inset(0 100% 0 0)' }, { clipPath: 'inset(0 0% 0 0)' }];
    const statEls = [...this.$('.stats').children];
    const counts = this._stats(this.state).map((st, i) => {
      const v = String(st.value);
      const valEl = statEls[i]?.querySelector('.val');
      if (!valEl) return null;
      if (NUMERIC.test(v)) return this.countUp(valEl, 0, parseFloat(v), { decimals: decimalsOf(v), delay: 650 + i * 90, duration: 900 }, skip);
      valEl.textContent = v;
      return this.anim(valEl, [{ opacity: 0 }, { opacity: 1 }], { delay: 650 + i * 90, duration: 500 }, skip);
    });
    return Promise.all([
      this.anim(this.$('.card'), [{ opacity: 1 }, { opacity: 1 }], { duration: 1 }, skip),
      this.anim(this.$('.accentbar'), [{ transform: 'scaleY(0)' }, { transform: 'scaleY(1)' }], { duration: 450 }, skip),
      this.anim(this.$('.numbox'), wipe, { delay: 120, duration: 450 }, skip),
      this.anim(this.$('.head'), wipe, { delay: 200, duration: 700 }, skip),
      this.stagger([this.$('.first'), this.$('.last')], [{ opacity: 0, transform: 'translateX(-30px)' }, { opacity: 1, transform: 'none' }], { delay: 350, duration: 600, stagger: 70 }, skip),
      this.anim(this.$('.wm'), [{ opacity: 0 }, { opacity: 1 }], { delay: 500, duration: 500 }, skip),
      this.anim(this.$('.title'), [{ transform: 'translateX(100%)', opacity: 0 }, { transform: 'none', opacity: 1 }], { delay: 450, duration: 550 }, skip),
      this.stagger(statEls, [{ clipPath: 'inset(100% 0 0 0)' }, { clipPath: 'inset(0% 0 0 0)' }], { delay: 500, duration: 550, stagger: 90 }, skip),
      this.anim(this.$('.note'), [{ clipPath: 'inset(0 0 100% 0)', transform: 'translateY(-16px)' }, { clipPath: 'inset(0 0 0% 0)', transform: 'none' }], { delay: 900, duration: 450 }, skip),
      ...counts,
    ]);
  }

  animateOut(skip) {
    const statEls = [...this.$('.stats').children];
    return Promise.all([
      this.anim(this.$('.note'), [{ clipPath: 'inset(0 0 0% 0)' }, { clipPath: 'inset(0 0 100% 0)' }], { duration: 250, easing: EASE.in }, skip),
      this.stagger(statEls.reverse(), [{ clipPath: 'inset(0% 0 0 0)' }, { clipPath: 'inset(100% 0 0 0)' }], { delay: 80, duration: 300, stagger: 40, easing: EASE.in }, skip),
      this.anim(this.$('.head'), [{ clipPath: 'inset(0 0% 0 0)' }, { clipPath: 'inset(0 0% 0 100%)' }], { delay: 200, duration: 380, easing: EASE.in }, skip),
      this.anim(this.$('.numbox'), [{ clipPath: 'inset(0 0% 0 0)' }, { clipPath: 'inset(0 0% 0 100%)' }], { delay: 260, duration: 320, easing: EASE.in }, skip),
      this.anim(this.$('.accentbar'), [{ transform: 'scaleY(1)' }, { transform: 'scaleY(0)' }], { delay: 380, duration: 250, easing: EASE.in }, skip),
      this.anim(this.$('.card'), [{ opacity: 1 }, { opacity: 0 }], { delay: 639, duration: 1 }, skip),
    ]);
  }
}
