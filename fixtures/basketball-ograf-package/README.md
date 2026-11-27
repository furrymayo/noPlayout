# Basketball OGraf Package (v1.0.0)

A broadcast graphics package for basketball, built as **EBU OGraf v1** graphics (HTML web components plus `.ograf.json` manifests). It is designed at 1920×1080, uses a neutral template look, and you set team colours and logos per show through the data fields.

| Folder | Graphic | Steps | Custom actions |
|---|---|---|---|
| `bball-scorebug` | Scorebug: scores, period, game clock, shot clock, timeouts, bonus/fouls, possession, message strip | 1 | Add points, Start / Stop / Set clock, Reset shot clock, Show / Hide message |
| `bball-lower-third` | Player/person ID: number, first/last name, detail line, team logo | 1 | – |
| `bball-matchup` | Full-screen open. **Step 1:** team vs team. **Step 2:** starting lineups and coaches | 2 | – |
| `bball-stat-card` | Player stat panel with up to 5 stats. Numbers count up, with an optional note line | 1 | – |
| `bball-ticker` | Bottom ticker that flips through scores or text items | 1 | Next item, Previous item |
| `bball-sponsor-bug` | Top-right "Presented by" bug with a logo or text | 1 | – |

Every graphic folder is **self-contained** (manifest, `graphic.mjs`, `lib/` with the shared code and fonts, `assets/`, thumbnails). You can copy the folders into your renderer's graphics directory together or one at a time.

---

## Quick start

1. Copy the `bball-*` folders into the graphics folder of your OGraf renderer or controller.
2. In your controller, load a graphic, fill in its fields and **Play**. **Next** advances steps. **Stop** takes it off.

### Previewing without a renderer
Graphics are ES modules, so they must be served over HTTP (not `file://`):

```bash
cd Basketball-OGraf-Package
python3 -m http.server 8000
# open http://localhost:8000/preview.html
```

`preview.html` is a small controller. It can load several graphics as layers, play, stop and step them, send updates (JSON), fire custom actions, toggle skipAnimation, and show a safe-area overlay.
The [EBU OGraf Devtool](https://ograf.ebu.io) works too.

---

## Branding

- **Colours** use lowercase `#rrggbb`, which the OGraf GDD colour type requires. Text on team colours automatically switches between black and white for contrast.
- **Logos:** put files in a graphic's `assets/` folder and set the field to `assets/yourteam.png`. An absolute `https://…` URL also works. PNG, SVG and WebP with transparency work best, and square logos look best. If a logo is missing, the scorebug shows the team's first letter.
- **Accent colour:** every graphic has an *Accent colour* field (default amber `#f5b700`). It is used for the possession bar, period, shot clock, ticker label, etc.
- **Fonts:** Barlow and Barlow Condensed are bundled (SIL Open Font License, see `lib/fonts/OFL-Barlow.txt`). No internet connection is needed at playout.
- The placeholder teams and logos (Riverton / Central State) are fictional.

---

## Operator notes

### Scorebug
- **Clock, two ways:**
  - *Internal clock:* set **Game clock** / **Shot clock** in the fields (e.g. `20:00`, `30`), then use **Start clock / Stop clock / Set clock / Reset shot clock**. The display shows `M:SS` and switches to `SS.t` under one minute. The shot clock shows tenths under 5 s (optional) and turns red at 5 s.
  - *External feed:* push `gameClock` / `shotClock` strings through updateAction as often as you like. A clock value only resets when the incoming value changes, so unrelated updates never jump a running clock.
- **Scores:** any increase animates (score box flash plus a "+N" badge). **Add points** (−3 … +3) is handy without a data feed.
  ⚠️ If your controller re-sends the *whole form* on every update, it will overwrite scores changed with *Add points*. Pick one method per show: the form/feed **or** Add points.
- **Possession** (None/Away/Home) draws an accent bar under the team. **Bonus** shows `BONUS` / `DBL BONUS`. With *Show team fouls* on, `FOULS n` shows when a team is not in the bonus.
- **Timeouts:** set *Timeouts per game* (default 4) and the timeouts left for each team. Used timeouts are shown as dimmed pips.
- **Show message** slides a strip out of the top of the bug, for example `TIMEOUT • CENTRAL STATE`, `UNDER REVIEW` or `END OF 1ST HALF`. You can colour it with the accent or either team's colour, and auto-hide it after N seconds.
- **Lift for ticker** raises the bug 74 px so the score ticker can run underneath. Lifting and lowering are animated.

### Matchup / lineups
*Play* shows the matchup, *Next* moves to the starting lineups, and *Next* again (or Stop) takes it off. You can jump straight to lineups with `playAction({ goto: 1 })`. Lineups take up to 6 players per team (number, name, position, info such as `6-3 • SR`). Long school and player names shrink automatically to fit.

### Stat card
Up to 5 stats. Whole-number and decimal values (`18`, `51.2`) count up. Other values (`7-12`) fade in. When values change on air, the changed boxes count to the new number and flash.

### Ticker
Each item is either a score (`team1`, `score1`, `team2`, `score2`, `status`) or a plain `text` line. On `FINAL` items the losing team is dimmed. *Seconds per item* sets the dwell time. *Paused* holds the current item. Next/Previous flip by hand.

---

## Layout & safe areas
- Everything sits inside 5 % action-safe (96 px left/right, 54 px top/bottom at 1080p).
- Lanes: **scorebug** bottom centre · **lower third / stat card** bottom left, above the scorebug · **ticker** bottom, full width (lift the bug) · **sponsor bug** top right · **matchup** full frame.
- The graphics fill whatever box the renderer gives them and scale the 1920×1080 design to fit. They work at 720p, 1080p, 4K, and in letterboxed or pillarboxed output.

---

## Technical notes
- Built to OGraf Graphics spec **v1** (`$schema: https://ograf.ebu.io/v1/specification/json-schemas/graphics/schema.json`). All six manifests validate against the official JSON schema. Fields use GDD types (`single-line`, `color-rrggbb`, `select`, `file-path/image-path`, arrays) so controllers can build proper forms.
- Real-time only (`supportsRealTime: true`, `supportsNonRealTime: false`).
- Implements `load`, `dispose`, `playAction` (`goto` / `delta`, returns `currentStep`), `stopAction`, `updateAction` (partial data is merged) and `customAction`. Unknown custom actions return status 400.
- `skipAnimation` is honoured everywhere.
- **Concurrency:** if a new play or stop arrives mid-animation, the running animation jumps to its end and the new one starts. Calls are never ignored, as the spec requires.
- No third-party libraries: animations use the Web Animations API. Graphics render in a Shadow DOM, so their styles don't collide with the renderer or other graphics.
- Tested in headless Chrome: every action, rapid overlapping commands, skipAnimation, dispose, and 720p scaling. Run it in your own playout before air, since renderers differ in how they size and composite the element.

## Customising
- `_dev/shared/ograf-base.mjs` is the shared base class. After editing it, run `_dev/sync-shared.sh` to copy it, the fonts and the assets into every graphic.
- `_dev/gen_manifests.py` is the single source of truth for every field and default. Edit it, then run `python3 _dev/gen_manifests.py` to regenerate each `*.ograf.json` and `defaults.mjs`.
- Each graphic's look and animation is in its own `graphic.mjs` (CSS at the top, animations in `animateIn` / `animateOut` / `animateStep`).
