/**
 * Basketball – Matchup / Starting Lineups (OGraf v1, 2 steps)
 *
 * Step 0: full-screen "AWAY vs HOME" with logos, ranks, mascots and records.
 * Step 1: starting lineups for both teams (number, name, position, info) + coaches.
 * Next from step 1 animates the graphic off (stepCount = 2).
 */
import { OGrafGraphic, EASE, esc, contrastText, shade, safeColor } from './lib/ograf-base.mjs';
import defaults from './defaults.mjs';

const side = (s) => `
  <div class="team ${s}">
    <img class="logo" alt="">
    <div class="tname"><span class="rank"></span><span class="school"></span></div>
    <div class="mascot"></div>
    <div class="record"></div>
  </div>`;

const lineupCol = (s) => `
  <div class="col ${s}">
    <div class="banner"><img class="blogo" alt=""><div class="bname"></div></div>
    <div class="rows"></div>
    <div class="coach"></div>
  </div>`;

export default class BasketballMatchup extends OGrafGraphic {
  static stepCount = 2;
  static defaults = defaults;
  static moduleUrl = import.meta.url;

  styles() {
    return `
.fs { position: absolute; inset: 0; opacity: 0; overflow: hidden; background: #0b0d10; }
.panel { position: absolute; inset: 0; }
.panel.away { background: linear-gradient(160deg, var(--away) 0%, var(--away-dk) 80%); clip-path: polygon(0 0, 58% 0, 42% 100%, 0 100%); }
.panel.home { background: linear-gradient(200deg, var(--home) 0%, var(--home-dk) 80%); clip-path: polygon(58% 0, 100% 0, 100% 100%, 42% 100%); }
.panel::after {
  content: ''; position: absolute; inset: 0;
  background: repeating-linear-gradient(115deg, rgba(255,255,255,.035) 0 2px, transparent 2px 22px),
              radial-gradient(ellipse at 50% 40%, transparent 30%, rgba(0,0,0,.55) 100%);
}
.divider { position: absolute; inset: 0; background: var(--accent); clip-path: polygon(57.6% 0, 58.4% 0, 42.4% 100%, 41.6% 100%); }
.header { position: absolute; left: 0; right: 0; top: 54px; display: flex; justify-content: center; }
.plate { padding: 14px 48px 16px; background: rgba(8,10,13,.82); border-bottom: 3px solid var(--accent); text-align: center; }
.etitle { font-weight: 800; font-size: 44px; letter-spacing: .24em; padding-left: .24em; color: var(--accent); text-transform: uppercase; line-height: 1.1; }
.esub { font-family: 'Barlow', sans-serif; font-weight: 600; font-size: 24px; letter-spacing: .14em; color: rgba(255,255,255,.85); margin-top: 6px; text-transform: uppercase; }
.footer { position: absolute; left: 0; right: 0; bottom: 62px; display: flex; justify-content: center; }
.footer span {
  padding: 8px 26px; background: rgba(0,0,0,.55); border-top: 3px solid var(--accent);
  font-family: 'Barlow', sans-serif; font-weight: 600; font-size: 22px; letter-spacing: .16em; text-transform: uppercase;
}
.footer span:empty { display: none; }

/* step 0 */
.match { position: absolute; inset: 0; opacity: 0; }
.team {
  position: absolute; top: 230px; width: 760px;
  display: flex; flex-direction: column; align-items: center; text-align: center;
}
.team.away { left: 130px; }
.team.home { right: 130px; }
.logo { width: 330px; height: 330px; filter: drop-shadow(0 10px 30px rgba(0,0,0,.5)); }
.tname { margin-top: 26px; display: flex; align-items: baseline; gap: 14px; white-space: nowrap; }
.rank { font-weight: 700; font-size: 48px; color: rgba(255,255,255,.75); }
.rank:empty { display: none; }
.school { font-weight: 800; font-size: 92px; line-height: 1; text-transform: uppercase; text-shadow: 0 4px 18px rgba(0,0,0,.4); }
.mascot { font-weight: 600; font-size: 42px; letter-spacing: .3em; text-transform: uppercase; color: rgba(255,255,255,.8); margin-top: 6px; padding-left: .3em; }
.record {
  margin-top: 22px; padding: 6px 22px; background: rgba(0,0,0,.45);
  font-family: 'Barlow', sans-serif; font-weight: 600; font-size: 26px; letter-spacing: .1em; text-transform: uppercase;
}
.record:empty { display: none; }
.vs {
  position: absolute; left: 960px; top: 540px; width: 150px; height: 150px; margin: -75px 0 0 -75px;
  border-radius: 50%; background: #0b0d10; border: 5px solid var(--accent);
  display: flex; align-items: center; justify-content: center;
  font-weight: 800; font-size: 66px; color: #fff; letter-spacing: .02em;
  box-shadow: 0 10px 40px rgba(0,0,0,.6);
}

/* step 1 */
.lineups { position: absolute; inset: 0; opacity: 0; }
.ltitle {
  position: absolute; left: 50%; top: 190px; transform: translateX(-50%); white-space: nowrap;
  padding: 6px 30px 6px calc(30px + .3em); background: rgba(8,10,13,.82);
  font-weight: 800; font-size: 32px; letter-spacing: .3em; text-transform: uppercase;
}
.col { position: absolute; top: 262px; width: 780px; }
.col.away { left: 110px; }
.col.home { right: 110px; }
.banner { display: flex; align-items: center; gap: 20px; height: 96px; padding: 0 22px; background: rgba(0,0,0,.55); border-bottom: 4px solid var(--c); }
.blogo { width: 72px; height: 72px; }
.bname { font-weight: 800; font-size: 54px; text-transform: uppercase; white-space: nowrap; }
.rows { display: flex; flex-direction: column; gap: 8px; margin-top: 12px; }
.row { display: flex; align-items: stretch; height: 98px; background: rgba(8,10,13,.78); }
.rnum { width: 104px; display: flex; align-items: center; justify-content: center; background: var(--c); color: var(--c-fg); font-weight: 800; font-size: 58px; }
.rname span { display: inline-block; }
.rname { flex: 1; display: flex; align-items: center; padding: 0 24px; font-weight: 700; font-size: 52px; text-transform: uppercase; white-space: nowrap; overflow: hidden; }
.rpos { width: 76px; display: flex; align-items: center; justify-content: center; font-weight: 800; font-size: 36px; color: var(--accent); }
.rinfo { width: 170px; display: flex; align-items: center; justify-content: flex-end; padding-right: 24px; font-family: 'Barlow', sans-serif; font-weight: 600; font-size: 22px; letter-spacing: .06em; color: rgba(255,255,255,.75); white-space: pre; }
.coach { margin-top: 14px; padding: 0 4px; font-family: 'Barlow', sans-serif; font-weight: 600; font-size: 24px; letter-spacing: .12em; text-transform: uppercase; color: rgba(255,255,255,.85); white-space: pre; }
`;
  }

  template() {
    return `
<div class="fs">
  <div class="panel away"></div>
  <div class="panel home"></div>
  <div class="divider"></div>
  <div class="header"><div class="plate"><div class="etitle"></div><div class="esub"></div></div></div>
  <div class="match">${side('away')}${side('home')}<div class="vs">VS</div></div>
  <div class="lineups"><div class="ltitle"></div>${lineupCol('away')}${lineupCol('home')}</div>
  <div class="footer"><span></span></div>
</div>`;
  }

  apply(s) {
    const accent = safeColor(s.accentColor, defaults.accentColor);
    const vars = { '--accent': accent };
    for (const t of ['away', 'home']) {
      const c = safeColor(s[`${t}Color`], defaults[`${t}Color`]);
      vars[`--${t}`] = c;
      vars[`--${t}-dk`] = shade(c, -0.6);
      this.vars(`.col.${t}`, { '--c': c, '--c-fg': contrastText(c) });

      const team = this.$(`.team.${t}`);
      this.setImage(team.querySelector('.logo'), s[`${t}Logo`]);
      this.setText(team.querySelector('.rank'), s[`${t}Rank`]);
      this.setText(team.querySelector('.school'), s[`${t}Name`]);
      this.setText(team.querySelector('.mascot'), s[`${t}Mascot`]);
      this.setText(team.querySelector('.record'), s[`${t}Record`]);
      this._fit(team.querySelector('.tname'), 740);

      const col = this.$(`.col.${t}`);
      this.setImage(col.querySelector('.blogo'), s[`${t}Logo`]);
      this.setText(col.querySelector('.bname'), [s[`${t}Rank`], s[`${t}Name`]].filter(Boolean).join('  '));
      this.setText(col.querySelector('.coach'), s[`${t}Coach`]);
      const players = (Array.isArray(s[`${t}Lineup`]) ? s[`${t}Lineup`] : []).slice(0, 6);
      col.querySelector('.rows').innerHTML = players.map((p) => `
        <div class="row">
          <div class="rnum">${esc(p.number)}</div>
          <div class="rname"><span>${esc(p.name)}</span></div>
          <div class="rpos">${esc(p.position)}</div>
          <div class="rinfo">${esc(p.info)}</div>
        </div>`).join('');
      for (const n of col.querySelectorAll('.rname')) this._fit(n.firstElementChild, (n.clientWidth || 440) - 48, true);
    }
    this.vars('.fs', vars);
    this.setText('.etitle', s.eventTitle);
    this.setText('.esub', s.eventSubtitle);
    this.setText('.ltitle', s.lineupTitle);
    this.setText('.footer span', s.footer);
  }

  /** Shrink text that would overflow its box (long school names). */
  _fit(el, maxWidth, inner = false) {
    if (!el) return;
    el.style.transform = '';
    el.style.transformOrigin = inner ? '0 50%' : '50% 50%';
    const w = el.scrollWidth;
    if (w > maxWidth) el.style.transform = `scale(${(maxWidth / w).toFixed(3)})`;
  }

  /* ------------------------------------------------------------ content per step */

  _contentIn(step, delay, skip) {
    if (step === 0) {
      return Promise.all([
        this.anim(this.$('.match'), [{ opacity: 0 }, { opacity: 1 }], { delay, duration: 1 }, skip),
        this.stagger(this.$$('.logo'), [{ opacity: 0, transform: 'scale(.6) translateY(40px)' }, { opacity: 1, transform: 'none' }], { delay, duration: 800, stagger: 120 }, skip),
        this.stagger(this.$$('.team .tname, .team .mascot, .team .record'), [{ opacity: 0, transform: 'translateY(30px)' }, { opacity: 1, transform: 'none' }], { delay: delay + 150, duration: 650, stagger: 45 }, skip),
        this.anim(this.$('.vs'), [{ opacity: 0, transform: 'scale(0) rotate(-90deg)' }, { opacity: 1, transform: 'none' }], { delay: delay + 300, duration: 650, easing: EASE.outBack }, skip),
      ]);
    }
    const rows = (t) => [...this.$$(`.col.${t} .row`)];
    return Promise.all([
      this.anim(this.$('.lineups'), [{ opacity: 0 }, { opacity: 1 }], { delay, duration: 1 }, skip),
      this.anim(this.$('.ltitle'), [{ opacity: 0, letterSpacing: '.5em' }, { opacity: 1, letterSpacing: '.3em' }], { delay, duration: 700 }, skip),
      this.stagger(this.$$('.banner'), [{ clipPath: 'inset(0 100% 0 0)' }, { clipPath: 'inset(0 0% 0 0)' }], { delay, duration: 600, stagger: 100 }, skip),
      this.stagger(rows('away'), [{ opacity: 0, transform: 'translateX(-60px)' }, { opacity: 1, transform: 'none' }], { delay: delay + 200, duration: 600, stagger: 70 }, skip),
      this.stagger(rows('home'), [{ opacity: 0, transform: 'translateX(60px)' }, { opacity: 1, transform: 'none' }], { delay: delay + 260, duration: 600, stagger: 70 }, skip),
      this.stagger(this.$$('.coach'), [{ opacity: 0 }, { opacity: 1 }], { delay: delay + 700, duration: 500, stagger: 60 }, skip),
    ]);
  }

  _contentOut(step, skip, delay = 0) {
    const el = this.$(step === 0 ? '.match' : '.lineups');
    return this.anim(el, [{ opacity: 1, transform: 'none' }, { opacity: 0, transform: 'scale(.97)' }], { delay, duration: 350, easing: EASE.in }, skip)
      .then(() => { el.style.transform = ''; });
  }

  animateIn(step, skip) {
    return Promise.all([
      this.anim(this.$('.fs'), [{ opacity: 0 }, { opacity: 1 }], { duration: 300, easing: EASE.linear }, skip),
      this.anim(this.$('.panel.away'), [{ transform: 'translateX(-100%)' }, { transform: 'none' }], { duration: 800 }, skip),
      this.anim(this.$('.panel.home'), [{ transform: 'translateX(100%)' }, { transform: 'none' }], { duration: 800 }, skip),
      this.anim(this.$('.divider'), [{ transform: 'scaleY(0)' }, { transform: 'scaleY(1)' }], { delay: 350, duration: 600 }, skip),
      this.anim(this.$('.header'), [{ opacity: 0, transform: 'translateY(-24px)' }, { opacity: 1, transform: 'none' }], { delay: 400, duration: 600 }, skip),
      this.anim(this.$('.footer'), [{ opacity: 0, transform: 'translateY(24px)' }, { opacity: 1, transform: 'none' }], { delay: 650, duration: 600 }, skip),
      this._contentIn(step, 450, skip),
    ]);
  }

  animateStep(from, to, skip) {
    return Promise.all([this._contentOut(from, skip), this._contentIn(to, 300, skip)]);
  }

  animateOut(skip) {
    return Promise.all([
      this._contentOut(0, skip),
      this._contentOut(1, skip),
      this.anim(this.$('.header'), [{ opacity: 1 }, { opacity: 0 }], { duration: 300 }, skip),
      this.anim(this.$('.footer'), [{ opacity: 1 }, { opacity: 0 }], { duration: 300 }, skip),
      this.anim(this.$('.divider'), [{ transform: 'scaleY(1)' }, { transform: 'scaleY(0)' }], { delay: 100, duration: 350, easing: EASE.in }, skip),
      this.anim(this.$('.panel.away'), [{ transform: 'none' }, { transform: 'translateX(-100%)' }], { delay: 200, duration: 550, easing: EASE.in }, skip),
      this.anim(this.$('.panel.home'), [{ transform: 'none' }, { transform: 'translateX(100%)' }], { delay: 200, duration: 550, easing: EASE.in }, skip),
      this.anim(this.$('.fs'), [{ opacity: 1 }, { opacity: 0 }], { delay: 550, duration: 250, easing: EASE.linear }, skip),
    ]);
  }
}
