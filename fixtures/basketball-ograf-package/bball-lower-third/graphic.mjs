/**
 * Basketball – Lower Third (OGraf v1)
 *
 *  ▌[ 23 ][ JORDAN                    (logo) ]
 *  ▌      [ MITCHELL                         ]
 *  ▌      [ GUARD • SENIOR • RALEIGH, NC     ]
 *
 * Empty number -> number box is hidden (coaches, commentators, guests).
 * Empty detail -> detail bar is hidden.
 */
import { OGrafGraphic, EASE, contrastText, shade, safeColor } from './lib/ograf-base.mjs';
import defaults from './defaults.mjs';

export default class BasketballLowerThird extends OGrafGraphic {
  static stepCount = 1;
  static defaults = defaults;
  static moduleUrl = import.meta.url;

  styles() {
    return `
.l3 {
  position: absolute; left: 96px; bottom: 196px;
  display: grid; grid-template-columns: 8px auto 1fr; grid-template-rows: 116px auto;
  opacity: 0;
  filter: drop-shadow(0 8px 22px rgba(0,0,0,.45));
}
.accentbar { grid-row: 1 / span 2; background: var(--accent); transform-origin: 50% 100%; }
.numbox {
  width: 116px; background: var(--team);
  display: flex; align-items: center; justify-content: center; overflow: hidden;
}
.num { font-weight: 800; font-size: 76px; line-height: 1; color: var(--team-fg); }
.l3.nonum .numbox { display: none; }
.l3.nonum .detail { grid-column: 2 / span 2; }
.panel {
  min-width: 560px; padding: 0 34px 0 30px;
  display: flex; align-items: center; gap: 36px;
  background: linear-gradient(90deg, #111317 0%, #1a1d22 100%);
}
.names { display: flex; flex-direction: column; justify-content: center; flex: 1; }
.first { font-weight: 600; font-size: 32px; letter-spacing: .14em; color: rgba(255,255,255,.72); line-height: 1.05; text-transform: uppercase; }
.last { font-weight: 800; font-size: 70px; line-height: .92; letter-spacing: .01em; text-transform: uppercase; white-space: nowrap; }
.wm { width: 84px; height: 84px; flex: 0 0 auto; }
.detail {
  grid-column: 3; height: 46px; padding: 0 30px;
  display: flex; align-items: center;
  background: var(--team-dk); color: #fff;
  font-family: 'Barlow', sans-serif; font-weight: 600; font-size: 22px; letter-spacing: .1em; text-transform: uppercase;
  white-space: pre;
}
.l3.nodetail .detail { display: none; }
`;
  }

  template() {
    return `
<div class="l3">
  <div class="accentbar"></div>
  <div class="numbox"><span class="num"></span></div>
  <div class="panel">
    <div class="names"><div class="first"></div><div class="last"></div></div>
    <img class="wm" alt="">
  </div>
  <div class="detail"></div>
</div>`;
  }

  apply(s) {
    const team = safeColor(s.teamColor, defaults.teamColor);
    const accent = safeColor(s.accentColor, defaults.accentColor);
    this.vars('.l3', { '--team': team, '--team-fg': contrastText(team), '--team-dk': shade(team, -0.4), '--accent': accent });
    this.setText('.num', s.number);
    this.setText('.first', s.firstName);
    this.setText('.last', s.lastName);
    this.setText('.detail', s.detail);
    this.setImage('.wm', s.teamLogo);
    const l3 = this.$('.l3');
    l3.classList.toggle('nonum', !String(s.number ?? '').trim());
    l3.classList.toggle('nodetail', !String(s.detail ?? '').trim());
    l3.classList.toggle('nofirst', !String(s.firstName ?? '').trim());
    this.$('.first').classList.toggle('hidden', !String(s.firstName ?? '').trim());
  }

  animateIn(_step, skip) {
    const wipe = [{ clipPath: 'inset(0 100% 0 0)' }, { clipPath: 'inset(0 0% 0 0)' }];
    return Promise.all([
      this.anim(this.$('.l3'), [{ opacity: 1 }, { opacity: 1 }], { duration: 1 }, skip),
      this.anim(this.$('.accentbar'), [{ transform: 'scaleY(0)' }, { transform: 'scaleY(1)' }], { duration: 400 }, skip),
      this.anim(this.$('.numbox'), wipe, { delay: 140, duration: 500 }, skip),
      this.anim(this.$('.num'), [{ opacity: 0, transform: 'scale(.6)' }, { opacity: 1, transform: 'scale(1)' }], { delay: 360, duration: 500, easing: EASE.outBack }, skip),
      this.anim(this.$('.panel'), wipe, { delay: 220, duration: 700 }, skip),
      this.stagger([this.$('.first'), this.$('.last')], [{ opacity: 0, transform: 'translateX(-40px)' }, { opacity: 1, transform: 'none' }], { delay: 380, duration: 650, stagger: 70 }, skip),
      this.anim(this.$('.wm'), [{ opacity: 0, transform: 'translateX(24px)' }, { opacity: 1, transform: 'none' }], { delay: 560, duration: 600 }, skip),
      this.anim(this.$('.detail'), [{ clipPath: 'inset(0 0 100% 0)', transform: 'translateY(-20px)' }, { clipPath: 'inset(0 0 0% 0)', transform: 'none' }], { delay: 560, duration: 450 }, skip),
    ]);
  }

  animateOut(skip) {
    const out = [{ clipPath: 'inset(0 0% 0 0)' }, { clipPath: 'inset(0 0% 0 100%)' }];
    return Promise.all([
      this.anim(this.$('.detail'), [{ clipPath: 'inset(0 0 0% 0)' }, { clipPath: 'inset(0 0 100% 0)' }], { duration: 250, easing: EASE.in }, skip),
      this.anim(this.$('.panel'), out, { delay: 100, duration: 400, easing: EASE.in }, skip),
      this.anim(this.$('.numbox'), out, { delay: 160, duration: 350, easing: EASE.in }, skip),
      this.anim(this.$('.accentbar'), [{ transform: 'scaleY(1)' }, { transform: 'scaleY(0)' }], { delay: 300, duration: 250, easing: EASE.in }, skip),
      this.anim(this.$('.l3'), [{ opacity: 1 }, { opacity: 0 }], { delay: 549, duration: 1 }, skip),
    ]);
  }
}
