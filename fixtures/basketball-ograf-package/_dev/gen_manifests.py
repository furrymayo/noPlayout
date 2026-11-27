"""Generates every *.ograf.json manifest and matching defaults.mjs (single source of truth for data fields)."""
import json, os, sys

PKG = sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCHEMA_URL = 'https://ograf.ebu.io/v1/specification/json-schemas/graphics/schema.json'
COLOR_PATTERN = '^#[0-9a-f]{6}$'
AUTHOR = {'name': 'Basketball OGraf Package'}
VERSION = '1.0.0'


class Order:
    def __init__(self):
        self.n = 0

    def __call__(self):
        self.n += 1
        return self.n


def text(label, default, o, desc=None, multi=False, hidden=False, maxlen=None):
    p = {'type': 'string', 'label': label, 'gddType': 'multi-line' if multi else 'single-line', 'default': default, 'order': o}
    if desc: p['description'] = desc
    if hidden: p['hidden'] = True
    if maxlen: p['maxLength'] = maxlen
    return p


def color(label, default, o, desc=None):
    p = {'type': 'string', 'label': label, 'gddType': 'color-rrggbb', 'pattern': COLOR_PATTERN, 'default': default, 'order': o, 'hidden': True}
    if desc: p['description'] = desc
    return p


def image(label, default, o, desc='Path relative to this graphic folder, or an absolute URL.'):
    return {'type': 'string', 'label': label, 'gddType': 'file-path/image-path',
            'gddOptions': {'extensions': ['png', 'svg', 'webp', 'jpg', 'jpeg', 'gif']},
            'default': default, 'order': o, 'hidden': True, 'description': desc}


def integer(label, default, o, lo=None, hi=None, desc=None, hidden=False):
    p = {'type': 'integer', 'label': label, 'default': default, 'order': o}
    if lo is not None: p['minimum'] = lo
    if hi is not None: p['maximum'] = hi
    if desc: p['description'] = desc
    if hidden: p['hidden'] = True
    return p


def number(label, default, o, lo=None, hi=None, desc=None, hidden=True):
    p = {'type': 'number', 'label': label, 'default': default, 'order': o}
    if lo is not None: p['minimum'] = lo
    if hi is not None: p['maximum'] = hi
    if desc: p['description'] = desc
    if hidden: p['hidden'] = True
    return p


def boolean(label, default, o, desc=None):
    p = {'type': 'boolean', 'label': label, 'default': default, 'order': o, 'hidden': True}
    if desc: p['description'] = desc
    return p


def select(label, default, o, options, desc=None, hidden=True):
    p = {'type': 'string', 'label': label, 'gddType': 'select', 'enum': list(options.keys()),
         'gddOptions': {'labels': options}, 'default': default, 'order': o}
    if desc: p['description'] = desc
    if hidden: p['hidden'] = True
    return p


def array(label, items_props, default, o, max_items=None, desc=None):
    p = {'type': 'array', 'label': label, 'items': {'type': 'object', 'properties': items_props},
         'default': default, 'order': o, 'hidden': True}
    if max_items: p['maxItems'] = max_items
    if desc: p['description'] = desc
    return p


ACCENT = '#f5b700'
AWAY = dict(name='RIVERTON', mascot='Hawks', abbr='RIV', color='#1f4fa3', logo='assets/away-logo.svg', rank='', record='14-6, 6-3 CONF')
HOME = dict(name='CENTRAL STATE', mascot='Lions', abbr='CSU', color='#b3202a', logo='assets/home-logo.svg', rank='12', record='17-3, 8-1 CONF')

# ------------------------------------------------------------------ graphics

graphics = []

# 1. Scorebug --------------------------------------------------------------
o = Order()
graphics.append(dict(
    id='bball-scorebug',
    name='Basketball – Scorebug',
    description='Persistent score bug: scores, period, game clock, shot clock, timeouts, bonus and possession. '
                'Runs its own clock (start/stop/set) or follows an external data feed via updates. Score increases animate automatically.',
    stepCount=1,
    durations=[('playAction', 1100), ('stopAction', 600), ('updateAction', -1)],
    props={
        'awayAbbr': text('Away abbreviation', AWAY['abbr'], o(), maxlen=5),
        'awayScore': integer('Away score', 0, o(), 0, 999),
        'homeAbbr': text('Home abbreviation', HOME['abbr'], o(), maxlen=5),
        'homeScore': integer('Home score', 0, o(), 0, 999),
        'period': text('Period', '1ST', o(), 'Free text, e.g. 1ST, 2ND, 3RD, 4TH, HALF, OT, 2OT, FINAL.', maxlen=6),
        'gameClock': text('Game clock', '20:00', o(), 'M:SS or SS.t – sets the clock. Leave the clock running or stopped with Start/Stop Clock.', hidden=True),
        'shotClock': text('Shot clock', '30', o(), 'Seconds.', hidden=True),
        'clockRunning': boolean('Clock running', False, o(), 'Run the game and shot clocks inside the graphic.'),
        'showShotClock': boolean('Show shot clock', True, o()),
        'shotClockTenths': boolean('Shot clock tenths under 5s', True, o()),
        'possession': select('Possession', 'none', o(), {'none': 'None', 'away': 'Away', 'home': 'Home'}),
        'awayTimeouts': integer('Away timeouts left', 4, o(), 0, 8, hidden=True),
        'homeTimeouts': integer('Home timeouts left', 4, o(), 0, 8, hidden=True),
        'maxTimeouts': integer('Timeouts per game', 4, o(), 0, 8, hidden=True),
        'awayBonus': select('Away bonus', 'none', o(), {'none': 'None', 'bonus': 'Bonus', 'double': 'Double bonus'}),
        'homeBonus': select('Home bonus', 'none', o(), {'none': 'None', 'bonus': 'Bonus', 'double': 'Double bonus'}),
        'showFouls': boolean('Show team fouls', False, o(), 'Shows "FOULS n" when the team is not in the bonus.'),
        'awayFouls': integer('Away team fouls', 0, o(), 0, 50, hidden=True),
        'homeFouls': integer('Home team fouls', 0, o(), 0, 50, hidden=True),
        'awayRank': text('Away rank', AWAY['rank'], o(), 'Leave empty for unranked.', hidden=True, maxlen=3),
        'homeRank': text('Home rank', HOME['rank'], o(), 'Leave empty for unranked.', hidden=True, maxlen=3),
        'awayColor': color('Away colour', AWAY['color'], o()),
        'homeColor': color('Home colour', HOME['color'], o()),
        'awayLogo': image('Away logo', AWAY['logo'], o()),
        'homeLogo': image('Home logo', HOME['logo'], o()),
        'accentColor': color('Accent colour', ACCENT, o()),
        'lift': boolean('Lift for ticker', False, o(), 'Raises the bug so the score ticker can run underneath.'),
        'scoreFlash': boolean('Animate score changes', True, o()),
    },
    actions=[
        dict(id='addPoints', name='Add points', description='Add points to a team (animates).',
             schema={'type': 'object', 'properties': {
                 'team': select('Team', 'home', 1, {'away': 'Away', 'home': 'Home'}, hidden=False),
                 'points': integer('Points', 2, 2, -3, 3, 'Use a negative number to correct a score.')}}),
        dict(id='startClock', name='Start clock', description='Start the game and shot clocks.', schema=None),
        dict(id='stopClock', name='Stop clock', description='Stop the game and shot clocks.', schema=None),
        dict(id='setClock', name='Set clock', description='Set the game and/or shot clock. Empty fields are left unchanged.',
             schema={'type': 'object', 'properties': {
                 'gameClock': text('Game clock', '', 1),
                 'shotClock': text('Shot clock', '', 2)}}),
        dict(id='resetShotClock', name='Reset shot clock', description='Reset the shot clock (30 or 20 in NCAA).',
             schema={'type': 'object', 'properties': {'seconds': integer('Seconds', 30, 1, 0, 99)}}),
        dict(id='showMessage', name='Show message', description='Slide a message strip out of the top of the bug (TIMEOUT, REVIEW, etc.).',
             schema={'type': 'object', 'properties': {
                 'text': text('Text', 'TIMEOUT', 1),
                 'color': select('Colour', 'accent', 2, {'accent': 'Accent', 'away': 'Away team', 'home': 'Home team'}, hidden=False),
                 'autoHideSeconds': number('Auto hide after (s, 0 = stay)', 0, 3, 0, 600, hidden=False)}}),
        dict(id='hideMessage', name='Hide message', description='Hide the message strip.', schema=None),
    ],
))

# 2. Lower third -----------------------------------------------------------
o = Order()
graphics.append(dict(
    id='bball-lower-third',
    name='Basketball – Lower Third',
    description='Player / person ID lower third with jersey number, two-line name and a detail line. Leave the number empty for coaches and commentators.',
    stepCount=1,
    durations=[('playAction', 1000), ('stopAction', 500), ('updateAction', 0)],
    props={
        'firstName': text('First name', 'JORDAN', o()),
        'lastName': text('Last name', 'MITCHELL', o()),
        'number': text('Number', '23', o(), 'Leave empty to hide the number box.', maxlen=3),
        'detail': text('Detail line', 'GUARD  •  SENIOR  •  RALEIGH, NC', o()),
        'teamColor': color('Team colour', HOME['color'], o()),
        'teamLogo': image('Team logo', HOME['logo'], o()),
        'accentColor': color('Accent colour', ACCENT, o()),
    },
    actions=[],
))

# 3. Matchup full screen ---------------------------------------------------
o = Order()
player = {'number': text('No.', '', 1), 'name': text('Name', '', 2), 'position': text('Pos', '', 3), 'info': text('Info', '', 4)}
away_lineup = [
    {'number': '1', 'name': 'Marcus Hale', 'position': 'G', 'info': '6-3  •  SR'},
    {'number': '5', 'name': 'Devin Ortiz', 'position': 'G', 'info': '6-1  •  JR'},
    {'number': '11', 'name': 'Tre Wallace', 'position': 'F', 'info': '6-7  •  SO'},
    {'number': '24', 'name': 'Isaiah Brooks', 'position': 'F', 'info': '6-8  •  SR'},
    {'number': '33', 'name': 'Noah Petersen', 'position': 'C', 'info': '7-0  •  JR'},
]
home_lineup = [
    {'number': '2', 'name': 'Chris Dawson', 'position': 'G', 'info': '6-2  •  SR'},
    {'number': '10', 'name': 'Andre Simmons', 'position': 'G', 'info': '6-4  •  FR'},
    {'number': '15', 'name': 'Elijah Grant', 'position': 'F', 'info': '6-6  •  JR'},
    {'number': '23', 'name': 'Jordan Mitchell', 'position': 'F', 'info': '6-9  •  SR'},
    {'number': '44', 'name': 'Samuel Okafor', 'position': 'C', 'info': '6-11  •  SO'},
]
graphics.append(dict(
    id='bball-matchup',
    name='Basketball – Matchup / Starting Lineups',
    description='Full-screen open. Step 1: team-vs-team matchup with records and ranks. Step 2: starting lineups. Next again takes it off.',
    stepCount=2,
    durations=[('playAction', 1400), ('stopAction', 800), ('updateAction', 0)],
    props={
        'eventTitle': text('Event title', "MEN'S BASKETBALL", o()),
        'eventSubtitle': text('Event subtitle', 'CENTRAL STATE ARENA  •  TIP-OFF 7:00 PM', o()),
        'awayName': text('Away school', AWAY['name'], o()),
        'awayMascot': text('Away mascot', AWAY['mascot'], o(), hidden=True),
        'awayRecord': text('Away record', AWAY['record'], o(), hidden=True),
        'awayRank': text('Away rank', AWAY['rank'], o(), hidden=True, maxlen=3),
        'homeName': text('Home school', HOME['name'], o()),
        'homeMascot': text('Home mascot', HOME['mascot'], o(), hidden=True),
        'homeRecord': text('Home record', HOME['record'], o(), hidden=True),
        'homeRank': text('Home rank', HOME['rank'], o(), hidden=True, maxlen=3),
        'lineupTitle': text('Lineup title', 'STARTING LINEUPS', o(), hidden=True),
        'awayLineup': array('Away starters', player, away_lineup, o(), 6),
        'homeLineup': array('Home starters', player, home_lineup, o(), 6),
        'awayCoach': text('Away head coach', 'Head Coach  Ray Thompson', o(), hidden=True),
        'homeCoach': text('Home head coach', 'Head Coach  Lisa Moreno', o(), hidden=True),
        'footer': text('Footer / sponsor line', '', o(), 'Optional, e.g. "PRESENTED BY YOUR SPONSOR".', hidden=True),
        'awayColor': color('Away colour', AWAY['color'], o()),
        'homeColor': color('Home colour', HOME['color'], o()),
        'awayLogo': image('Away logo', AWAY['logo'], o()),
        'homeLogo': image('Home logo', HOME['logo'], o()),
        'accentColor': color('Accent colour', ACCENT, o()),
    },
    actions=[],
))

# 4. Player stat card ------------------------------------------------------
o = Order()
graphics.append(dict(
    id='bball-stat-card',
    name='Basketball – Player Stat Card',
    description='Player stat panel with up to five animated stat boxes (numbers count up) and an optional note line.',
    stepCount=1,
    durations=[('playAction', 1400), ('stopAction', 500), ('updateAction', -1)],
    props={
        'firstName': text('First name', 'JORDAN', o()),
        'lastName': text('Last name', 'MITCHELL', o()),
        'number': text('Number', '23', o(), maxlen=3),
        'title': text('Title', 'TONIGHT', o(), 'e.g. TONIGHT, FIRST HALF, SEASON AVERAGES.'),
        'stats': array('Stats', {'label': text('Label', '', 1), 'value': text('Value', '', 2)},
                       [{'label': 'PTS', 'value': '18'}, {'label': 'REB', 'value': '9'},
                        {'label': 'AST', 'value': '4'}, {'label': 'FG', 'value': '7-12'}, {'label': '3PT', 'value': '2-4'}], o(), 5,
                       'Numeric values count up; others (e.g. 7-12) fade in.'),
        'note': text('Note line', 'CAREER-HIGH 9 REBOUNDS', o(), 'Optional.'),
        'teamColor': color('Team colour', HOME['color'], o()),
        'teamLogo': image('Team logo', HOME['logo'], o()),
        'accentColor': color('Accent colour', ACCENT, o()),
    },
    actions=[],
))

# 5. Score ticker ----------------------------------------------------------
o = Order()
ticker_item = {'team1': text('Team 1', '', 1), 'score1': text('Score 1', '', 2), 'team2': text('Team 2', '', 3),
               'score2': text('Score 2', '', 4), 'status': text('Status', '', 5), 'text': text('Text (instead of a score)', '', 6)}
graphics.append(dict(
    id='bball-ticker',
    name='Basketball – Score Ticker',
    description='Bottom ticker that flips through other scores or text items. Use with the scorebug "Lift for ticker" option.',
    stepCount=1,
    durations=[('playAction', 900), ('stopAction', 500), ('updateAction', 0)],
    props={
        'label': text('Label', 'AROUND THE CONFERENCE', o()),
        'items': array('Items', ticker_item, [
            {'team1': 'WESTFIELD', 'score1': '71', 'team2': 'NORTH VALLEY', 'score2': '64', 'status': 'FINAL', 'text': ''},
            {'team1': 'BAYSIDE', 'score1': '38', 'team2': '12 LAKEMONT', 'score2': '41', 'status': 'HALF', 'text': ''},
            {'team1': 'EASTERN TECH', 'score1': '55', 'team2': 'PINE RIDGE', 'score2': '57', 'status': '2ND 4:12', 'text': ''},
            {'team1': 'SUMMIT', 'score1': '', 'team2': 'HARBOR CITY', 'score2': '', 'status': '9:00 PM', 'text': ''},
            {'team1': '', 'score1': '', 'team2': '', 'score2': '', 'status': '', 'text': 'NEXT HOME GAME: SATURDAY VS. WESTFIELD  •  2:00 PM'},
        ], o(), 40),
        'dwellSeconds': number('Seconds per item', 6, o(), 1, 120),
        'paused': boolean('Paused', False, o(), 'Hold on the current item.'),
        'accentColor': color('Accent colour', ACCENT, o()),
    },
    actions=[
        dict(id='nextItem', name='Next item', description='Flip to the next item now.', schema=None),
        dict(id='previousItem', name='Previous item', description='Flip to the previous item now.', schema=None),
    ],
))

# 6. Sponsor bug -----------------------------------------------------------
o = Order()
graphics.append(dict(
    id='bball-sponsor-bug',
    name='Basketball – Sponsor Bug',
    description='Top-right "presented by" corner bug with a sponsor logo or name.',
    stepCount=1,
    durations=[('playAction', 800), ('stopAction', 500), ('updateAction', 0)],
    props={
        'label': text('Label', 'PRESENTED BY', o()),
        'sponsorName': text('Sponsor name', 'YOUR SPONSOR', o(), 'Shown when no logo is set.'),
        'logo': image('Sponsor logo', '', o()),
        'panel': select('Panel style', 'light', o(), {'light': 'Light', 'dark': 'Dark'}),
        'accentColor': color('Accent colour', ACCENT, o()),
    },
    actions=[],
))

# ------------------------------------------------------------------ write

for g in graphics:
    folder = os.path.join(PKG, g['id'])
    os.makedirs(folder, exist_ok=True)
    manifest = {
        '$schema': SCHEMA_URL,
        'id': g['id'],
        'version': VERSION,
        'name': g['name'],
        'description': g['description'],
        'author': AUTHOR,
        'main': 'graphic.mjs',
        'supportsRealTime': True,
        'supportsNonRealTime': False,
        'stepCount': g['stepCount'],
        'renderRequirements': [{'resolution': {'width': {'ideal': 1920}, 'height': {'ideal': 1080}}}],
        'schema': {'type': 'object', 'properties': g['props']},
    }
    thumbs = []
    for fn, w, h in (('thumbnail.png', 1280, 720), ('thumbnail-small.png', 480, 270)):
        if os.path.exists(os.path.join(folder, fn)):
            thumbs.append({'file': fn, 'resolution': {'width': w, 'height': h}})
    if thumbs:
        manifest['thumbnails'] = thumbs
    if g['actions']:
        manifest['customActions'] = g['actions']
    durations = [{'type': t, 'duration': d} for t, d in g['durations']]
    manifest['actionDurations'] = durations
    with open(os.path.join(folder, f"{g['id']}.ograf.json"), 'w') as f:
        json.dump(manifest, f, indent=2, ensure_ascii=False)
        f.write('\n')
    defaults = {k: v['default'] for k, v in g['props'].items()}
    with open(os.path.join(folder, 'defaults.mjs'), 'w') as f:
        f.write('// Generated from the manifest schema defaults — do not edit by hand.\n')
        f.write('export default ' + json.dumps(defaults, indent=2, ensure_ascii=False) + ';\n')
    print('wrote', g['id'])
