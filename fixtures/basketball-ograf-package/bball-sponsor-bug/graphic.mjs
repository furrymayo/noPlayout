/**
 * Basketball – Sponsor Bug (OGraf v1)
 * Top-right "PRESENTED BY" corner bug with a logo, or the sponsor name as text.
 */
import { OGrafGraphic, EASE, contrastText, safeColor } from './lib/ograf-base.mjs';
import defaults from './defaults.mjs';

export default class BasketballSponsorBug extends OGrafGraphic {
  static stepCount = 1;
  static defaults = defaults;
  static moduleUrl = import.meta.url;

  styles() {
    return `
.sp {
  position: absolute; right: 96px; top: 54px;
  display: flex; opacity: 0;
  filter: drop-shadow(0 6px 18px rgba(0,0,0,.4));
}
.bar { width: 7px; background: var(--accent); transform-origin: 50% 0; }
.box {
  display: flex; flex-direction: column; align-items: flex-start; justify-content: center; gap: 6px;
  padding: 12px 24px 14px 20px; min-width: 220px;
  background: var(--bg); color: var(--fg);
}
.lbl { font-family: 'Barlow', sans-serif; font-weight: 700; font-size: 15px; letter-spacing: .24em; color: var(--muted); text-transform: uppercase; }
.lbl:empty { display: none; }
.logo { height: 64px; max-width: 300px; object-position: left center; }
.name { font-weight: 800; font-size: 40px; line-height: 1; text-transform: uppercase; letter-spacing: .02em; }
.sp.haslogo .name { display: none; }
`;
  }

  template() {
    return `<div class="sp"><div class="bar"></div><div class="box"><div class="lbl"></div><img class="logo" alt=""><div class="name"></div></div></div>`;
  }

  apply(s) {
    const light = s.panel !== 'dark';
    const accent = safeColor(s.accentColor, defaults.accentColor);
    this.vars('.sp', {
      '--accent': accent,
      '--bg': light ? '#f4f5f7' : '#111317',
      '--fg': light ? '#0c0e12' : '#ffffff',
      '--muted': light ? '#5b616b' : 'rgba(255,255,255,.65)',
      '--accent-fg': contrastText(accent),
    });
    this.setText('.lbl', s.label);
    this.setText('.name', s.sponsorName);
    const img = this.$('.logo');
    const has = this.setImage(img, s.logo);
    this.$('.sp').classList.toggle('haslogo', has);
    img.onerror = () => { img.classList.add('hidden'); this.$('.sp').classList.remove('haslogo'); };
    img.onload = () => { img.classList.remove('hidden'); this.$('.sp').classList.add('haslogo'); };
  }

  animateIn(_step, skip) {
    return Promise.all([
      this.anim(this.$('.sp'), [{ opacity: 1 }, { opacity: 1 }], { duration: 1 }, skip),
      this.anim(this.$('.bar'), [{ transform: 'scaleY(0)' }, { transform: 'scaleY(1)' }], { duration: 350 }, skip),
      this.anim(this.$('.box'), [{ clipPath: 'inset(0 0 0 100%)' }, { clipPath: 'inset(0 0 0 0%)' }], { delay: 150, duration: 600 }, skip),
      this.stagger(this.$$('.box > *'), [{ opacity: 0, transform: 'translateX(20px)' }, { opacity: 1, transform: 'none' }], { delay: 300, duration: 500, stagger: 80 }, skip),
    ]);
  }

  animateOut(skip) {
    return Promise.all([
      this.anim(this.$('.box'), [{ clipPath: 'inset(0 0 0 0%)' }, { clipPath: 'inset(0 0 0 100%)' }], { duration: 400, easing: EASE.in }, skip),
      this.anim(this.$('.bar'), [{ transform: 'scaleY(1)' }, { transform: 'scaleY(0)' }], { delay: 300, duration: 200, easing: EASE.in }, skip),
      this.anim(this.$('.sp'), [{ opacity: 1 }, { opacity: 0 }], { delay: 499, duration: 1 }, skip),
    ]);
  }
}
