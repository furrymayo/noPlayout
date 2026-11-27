/**
 * Basketball – Scorebug (OGraf v1)
 *
 *  [logo|RIV  ▮▮▮▮|  54 ][logo|12 CSU ▮▮▮▮ BONUS| 61 ][ 2ND | 8:31 | 17 ]
 *
 * - updateAction: any field (scores, clock, period, possession, bonus ...)
 * - Score increases flash the score box and pop a "+N" badge.
 * - The graphic can run its own game + shot clock (startClock / stopClock /
 *   setClock / resetShotClock) or simply display clock values pushed in by
 *   an external feed through updateAction.
 */
import {
  OGrafGraphic, EASE, esc, contrastText, shade, safeColor,
  parseClock, formatGameClock, formatShotClock,
} from './lib/ograf-base.mjs';
import defaults from './defaults.mjs';

const TEAM_W = 330;
const LIFT_PX = 74;

const teamMarkup = (side) => `
  <div class="team ${side}">
    <div class="logo"><div class="logo-in"><img alt=""><span class="logo-fb"></span></div></div>
    <div class="mid">
      <div class="name"><span class="rank"></span><span class="abbr"></span></div>
      <div class="sub"><div class="pips"></div><div class="tag"></div></div>
    </div>
    <div class="score"><span class="num">0</span></div>
    <div class="poss"></div>
  </div>`;

export default class BasketballScorebug extends OGrafGraphic {
  static stepCount = 1;
  static defaults = defaults;
  static moduleUrl = import.meta.url;

  styles() {
    return `
.bugpos {
  position: absolute; left: 50%; bottom: 54px;
  transform: translateX(-50%);
  filter: drop-shadow(0 6px 18px rgba(0,0,0,.45));
}
.msgwrap {
  position: absolute; left: 0; right: 0; bottom: 100%;
  height: 44px; overflow: hidden;
}
.msg {
  height: 100%; display: flex; align-items: center; justify-content: center;
  background: var(--msg-bg); color: var(--msg-fg);
  font-weight: 800; font-size: 28px; letter-spacing: .12em; text-transform: uppercase;
  transform: translateY(105%);
}
.bar {
  position: relative; display: flex; height: 76px;
  background: #111317;
  clip-path: inset(0 50% 0 50%);
}
.team { position: relative; display: flex; width: ${TEAM_W}px; height: 100%; }
.team + .team { border-left: 2px solid #000; }
.logo { width: 76px; height: 100%; background: var(--c); overflow: hidden; position: relative; }
.logo-in { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; }
.logo img { width: 60px; height: 60px; filter: drop-shadow(0 2px 3px rgba(0,0,0,.35)); }
.logo-fb { font-weight: 800; font-size: 46px; color: var(--c-fg); display: none; }
.logo.nologo .logo-fb { display: block; }
.logo.nologo img { display: none; }
.mid {
  width: 150px; padding: 8px 0 0 14px;
  background: linear-gradient(180deg, var(--c-dk) 0%, var(--c-dk2) 100%);
  display: flex; flex-direction: column; justify-content: space-between;
}
.name { display: flex; align-items: baseline; gap: 6px; line-height: 1; white-space: nowrap; }
.abbr { font-weight: 800; font-size: 42px; letter-spacing: .02em; text-transform: uppercase; }
.rank { font-weight: 600; font-size: 22px; color: rgba(255,255,255,.75); }
.rank:empty { display: none; }
.sub { display: flex; align-items: center; justify-content: space-between; height: 22px; padding: 0 10px 8px 0; }
.pips { display: flex; gap: 3px; }
.pip { width: 13px; height: 5px; background: #fff; border-radius: 1px; }
.pip.used { background: rgba(255,255,255,.22); }
.tag { font-weight: 800; font-size: 16px; letter-spacing: .06em; color: var(--accent); white-space: nowrap; }
.tag.fouls { color: rgba(255,255,255,.8); font-weight: 600; }
.score {
  width: 104px; display: flex; align-items: center; justify-content: center;
  background: #f4f5f7; color: #0c0e12; overflow: hidden;
}
.num { font-weight: 800; font-size: 58px; line-height: 1; display: block; }
.poss {
  position: absolute; left: 76px; right: 0; bottom: 0; height: 5px;
  background: var(--accent); transform: scaleX(0); transform-origin: 50% 50%;
}
.clock { display: flex; align-items: stretch; border-left: 2px solid #000; }
.period, .gclock {
  display: flex; align-items: center; justify-content: center;
  font-weight: 700; line-height: 1;
}
.period { width: 78px; font-size: 30px; color: var(--accent); text-transform: uppercase; letter-spacing: .02em; }
.gclock { width: 136px; font-size: 48px; font-weight: 800; }
.shot {
  width: 76px; display: flex; align-items: center; justify-content: center;
  background: #23262c; font-weight: 800; font-size: 40px; color: var(--accent);
}
.shot.low { color: #ff3b30; }
.shot.off { display: none; }
.fx { position: absolute; left: 0; top: 0; width: 100%; height: 76px; pointer-events: none; }
.plus {
  position: absolute; top: -40px; width: 72px; margin-left: -36px; height: 34px;
  display: flex; align-items: center; justify-content: center;
  background: var(--accent); color: var(--accent-fg);
  font-weight: 800; font-size: 28px; opacity: 0;
}
.plus.away { left: ${76 + 150 + 52}px; }
.plus.home { left: ${TEAM_W + 2 + 76 + 150 + 52}px; }
`;
  }

  template() {
    return `
<div class="bugpos">
  <div class="msgwrap"><div class="msg"><span class="msg-text"></span></div></div>
  <div class="bar">
    ${teamMarkup('away')}
    ${teamMarkup('home')}
    <div class="clock">
      <div class="period"></div>
      <div class="gclock">20:00</div>
      <div class="shot"><span class="shot-num">30</span></div>
    </div>
  </div>
  <div class="fx"><div class="plus away"></div><div class="plus home"></div></div>
</div>`;
  }

  /* ------------------------------------------------------------ state */

  apply(s, { prev, skip, initial }) {
    const accent = safeColor(s.accentColor, defaults.accentColor);
    this.vars('.bugpos', { '--accent': accent, '--accent-fg': contrastText(accent) });

    for (const side of ['away', 'home']) this._applyTeam(side, s, prev, skip, initial);

    this.setText('.period', s.period);

    // clocks: only (re)set when the incoming value changed, so a running clock isn't reset by unrelated updates
    if (initial || s.gameClock !== prev.gameClock) this.gameMs = parseClock(s.gameClock);
    if (initial || s.shotClock !== prev.shotClock) this.shotMs = parseClock(s.shotClock);
    if (initial || s.clockRunning !== prev.clockRunning) this._setRunning(!!s.clockRunning);
    this.$('.shot').classList.toggle('off', !s.showShotClock);
    this._renderClock();

    // lift for ticker
    if (!initial && !!s.lift !== !!prev.lift) {
      const to = s.lift ? `0 -${LIFT_PX}px` : '0 0';
      this.anim(this.$('.bugpos'), [{ translate: s.lift ? '0 0' : `0 -${LIFT_PX}px` }, { translate: to }],
        { duration: 600, easing: EASE.inOut }, skip);
    } else if (initial) {
      this.$('.bugpos').style.translate = s.lift ? `0 -${LIFT_PX}px` : '0 0';
    }

    if (initial && !this._tick) {
      this._last = performance.now();
      this._tick = this.setRepeat(() => this._onTick(), 50);
    }
  }

  _applyTeam(side, s, prev, skip, initial) {
    const team = this.$(`.team.${side}`);
    const c = safeColor(s[`${side}Color`], defaults[`${side}Color`]);
    this.vars(team, { '--c': c, '--c-fg': contrastText(c), '--c-dk': shade(c, -0.45), '--c-dk2': shade(c, -0.65) });

    const abbr = s[`${side}Abbr`] ?? '';
    this.setText(team.querySelector('.abbr'), abbr);
    this.setText(team.querySelector('.rank'), s[`${side}Rank`]);
    this.setText(team.querySelector('.logo-fb'), String(abbr).charAt(0));
    const hasLogo = this.setImage(team.querySelector('img'), s[`${side}Logo`]);
    team.querySelector('.logo').classList.toggle('nologo', !hasLogo);
    const img = team.querySelector('img');
    const logoBox = team.querySelector('.logo');
    img.onerror = () => logoBox.classList.add('nologo');
    img.onload = () => logoBox.classList.remove('nologo');

    // timeouts
    const max = Math.max(0, s.maxTimeouts | 0);
    const left = Math.max(0, Math.min(max, s[`${side}Timeouts`] | 0));
    const key = `${max}/${left}`;
    const pips = team.querySelector('.pips');
    if (pips.dataset.key !== key) {
      pips.dataset.key = key;
      pips.innerHTML = Array.from({ length: max }, (_, i) => `<div class="pip${i >= left ? ' used' : ''}"></div>`).join('');
    }

    // bonus / fouls tag
    const tag = team.querySelector('.tag');
    const bonus = s[`${side}Bonus`];
    let tagText = '';
    if (bonus === 'double') tagText = 'DBL BONUS';
    else if (bonus === 'bonus') tagText = 'BONUS';
    else if (s.showFouls) tagText = `FOULS ${s[`${side}Fouls`] | 0}`;
    tag.classList.toggle('fouls', !bonus || bonus === 'none');
    if (tag.textContent !== tagText) {
      tag.textContent = tagText;
      if (tagText && !initial) this.anim(tag, [{ opacity: 0, transform: 'translateY(8px)' }, { opacity: 1, transform: 'none' }], { duration: 400 }, skip);
    }

    // possession
    const has = s.possession === side;
    const was = !initial && prev.possession === side;
    const poss = team.querySelector('.poss');
    if (initial) poss.style.transform = has ? 'scaleX(1)' : 'scaleX(0)';
    else if (has !== was) this.anim(poss, [{ transform: `scaleX(${was ? 1 : 0})` }, { transform: `scaleX(${has ? 1 : 0})` }], { duration: 350 }, skip);

    // score
    const score = s[`${side}Score`] | 0;
    const num = team.querySelector('.num');
    const old = initial ? score : prev[`${side}Score`] | 0;
    num.textContent = String(score);
    if (!initial && score !== old && s.scoreFlash !== false) this._flash(side, score - old, skip);
  }

  _flash(side, diff, skip) {
    if (skip) return;
    const team = this.$(`.team.${side}`);
    const box = team.querySelector('.score');
    const num = team.querySelector('.num');
    const accent = getComputedStyle(this.$('.bugpos')).getPropertyValue('--accent').trim() || '#f5b700';
    this.anim(box, [{ backgroundColor: accent }, { backgroundColor: accent, offset: 0.35 }, { backgroundColor: '#f4f5f7' }], { duration: 1100, easing: EASE.linear });
    this.anim(num, [{ transform: 'translateY(-100%)', opacity: 0 }, { transform: 'translateY(0)', opacity: 1 }], { duration: 450 });
    if (diff > 0) {
      const plus = this.$(`.plus.${side}`);
      plus.textContent = `+${diff}`;
      this.anim(plus, [
        { opacity: 0, transform: 'translateY(16px) scale(.8)' },
        { opacity: 1, transform: 'translateY(0) scale(1)', offset: 0.18 },
        { opacity: 1, transform: 'translateY(-4px) scale(1)', offset: 0.78 },
        { opacity: 0, transform: 'translateY(-14px) scale(1)' },
      ], { duration: 1500, easing: EASE.linear });
    }
  }

  /* ------------------------------------------------------------ clock */

  _setRunning(on) {
    this.running = on;
    this._last = performance.now();
  }

  _onTick() {
    const now = performance.now();
    const dt = now - (this._last ?? now);
    this._last = now;
    if (!this.running) return;
    this.gameMs = Math.max(0, this.gameMs - dt);
    this.shotMs = Math.max(0, this.shotMs - dt);
    if (this.gameMs <= 0) this.running = false;
    this._renderClock();
  }

  _renderClock() {
    this.setText('.gclock', formatGameClock(this.gameMs));
    const shot = this.$('.shot');
    // shot clock is hidden by the scoreboard when it exceeds the remaining game time
    const shotOff = this.state.showShotClock && this.gameMs > 0 && this.shotMs > this.gameMs + 999;
    this.setText('.shot-num', shotOff ? '' : formatShotClock(this.shotMs, this.state.shotClockTenths));
    shot.classList.toggle('low', !shotOff && this.shotMs > 0 && this.shotMs <= 5000);
  }

  /* ------------------------------------------------------------ animation */

  animateIn(_step, skip) {
    const bar = this.$('.bar');
    const p = [
      this.anim(bar, [{ clipPath: 'inset(0 50% 0 50%)' }, { clipPath: 'inset(0 0% 0 0%)' }], { duration: 650, easing: EASE.inOut }, skip),
      this.stagger(this.$$('.logo-in'), [{ transform: 'translateY(100%)' }, { transform: 'translateY(0)' }], { delay: 320, duration: 600, stagger: 90 }, skip),
      this.stagger(this.$$('.mid > *'), [{ opacity: 0, transform: 'translateX(-24px)' }, { opacity: 1, transform: 'none' }], { delay: 380, duration: 600, stagger: 50 }, skip),
      this.stagger(this.$$('.num'), [{ opacity: 0, transform: 'translateY(60%)' }, { opacity: 1, transform: 'none' }], { delay: 460, duration: 600, stagger: 90 }, skip),
      this.stagger(this.$$('.clock > *'), [{ opacity: 0 }, { opacity: 1 }], { delay: 520, duration: 500, stagger: 70 }, skip),
    ];
    if (this._msgShown) p.push(this._msgAnim(true, skip, 700));
    return Promise.all(p);
  }

  animateOut(skip) {
    const bar = this.$('.bar');
    return Promise.all([
      this._msgShown ? this._msgAnim(false, skip) : null,
      this.stagger([...this.$$('.logo-in'), ...this.$$('.mid > *'), ...this.$$('.num'), ...this.$$('.clock > *')],
        [{ opacity: 1 }, { opacity: 0 }], { duration: 200, stagger: 0, easing: EASE.linear }, skip),
      this.anim(bar, [{ clipPath: 'inset(0 0% 0 0%)' }, { clipPath: 'inset(0 50% 0 50%)' }], { delay: 120, duration: 450, easing: EASE.inOut }, skip),
    ]).then(() => {
      // restore content opacity for the next play (bar stays clipped)
      for (const el of [...this.$$('.logo-in'), ...this.$$('.mid > *'), ...this.$$('.num'), ...this.$$('.clock > *')]) el.style.opacity = '';
    });
  }

  /* ------------------------------------------------------------ message */

  _msgColors(which) {
    const s = this.state;
    const c = which === 'away' ? s.awayColor : which === 'home' ? s.homeColor : s.accentColor;
    const bg = safeColor(c, defaults.accentColor);
    this.vars('.msg', { '--msg-bg': bg, '--msg-fg': contrastText(bg) });
  }

  _msgAnim(show, skip, delay = 0) {
    return this.anim(this.$('.msg'),
      [{ transform: `translateY(${show ? 105 : 0}%)` }, { transform: `translateY(${show ? 0 : 105}%)` }],
      { duration: show ? 500 : 350, delay, easing: show ? EASE.out : EASE.in }, skip);
  }

  /* ------------------------------------------------------------ custom actions */

  actions() {
    return {
      addPoints: async ({ team = 'home', points = 2 }, skip) => {
        if (team !== 'home' && team !== 'away') return { statusCode: 400, statusMessage: 'team must be "home" or "away"' };
        const key = `${team}Score`;
        const next = Math.max(0, (this.state[key] | 0) + (Number(points) | 0));
        await this.updateAction({ data: { [key]: next }, skipAnimation: skip });
      },
      startClock: async () => { this._setRunning(true); },
      stopClock: async () => { this._setRunning(false); this._renderClock(); },
      setClock: async ({ gameClock, shotClock }) => {
        if (gameClock !== undefined && String(gameClock).trim() !== '') this.gameMs = parseClock(gameClock);
        if (shotClock !== undefined && String(shotClock).trim() !== '') this.shotMs = parseClock(shotClock);
        this._renderClock();
      },
      resetShotClock: async ({ seconds = 30 }) => {
        this.shotMs = Math.max(0, Number(seconds) || 0) * 1000;
        this._renderClock();
      },
      showMessage: async ({ text = '', color = 'accent', autoHideSeconds = 0 }, skip) => {
        this.setText('.msg-text', text);
        this._msgColors(color);
        if (this._msgTimer) this.clearTimer(this._msgTimer);
        if (autoHideSeconds > 0) this._msgTimer = this.setTimer(() => this.actions().hideMessage({}, false), autoHideSeconds * 1000);
        if (this._msgShown) {
          await this.anim(this.$('.msg-text'), [{ opacity: 0 }, { opacity: 1 }], { duration: 300 }, skip);
          return;
        }
        this._msgShown = true;
        if (this.visible) await this._msgAnim(true, skip);
      },
      hideMessage: async (_p, skip) => {
        if (this._msgTimer) this.clearTimer(this._msgTimer);
        if (!this._msgShown) return;
        this._msgShown = false;
        if (this.visible) await this._msgAnim(false, skip);
      },
    };
  }
}
