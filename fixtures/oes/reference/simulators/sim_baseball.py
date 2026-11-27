"""OES Baseball Simulator — type 0x74, 52-byte packet."""

import random
from oes_helpers import (
    build_packet, encode_score, encode_two_digit, encode_three_digit,
    run_server, make_arg_parser, BLANK,
)

TP = 0x74
DEFAULT_PORT = 6008

NUM_INNINGS = 9


def make_game():
    return {
        "inning": 1,
        "half": "top",  # top or bottom
        "outs": 0,
        "balls": 0,
        "strikes": 0,
        "batter_num": 1,
        "pitch_speed": 0,
        "home_runs": 0,
        "home_hits": 0,
        "home_errors": 0,
        "away_runs": 0,
        "away_hits": 0,
        "away_errors": 0,
        "home_innings": [0] * 10,
        "away_innings": [0] * 10,
    }


game = make_game()


def tick():
    g = game

    # Simulate a pitch
    g["pitch_speed"] = random.randint(72, 98)

    outcome = random.random()

    if outcome < 0.35:
        # Strike
        g["strikes"] += 1
        if g["strikes"] >= 3:
            _record_out()
    elif outcome < 0.55:
        # Ball
        g["balls"] += 1
        if g["balls"] >= 4:
            _advance_batter(hit=False)
    elif outcome < 0.70:
        # Foul (strike if < 2)
        if g["strikes"] < 2:
            g["strikes"] += 1
    elif outcome < 0.90:
        # Hit
        _advance_batter(hit=True)
        # Run scores sometimes
        if random.random() < 0.3:
            _score_run()
    else:
        # Out (groundout, flyout)
        _record_out()


def _record_out():
    g = game
    g["outs"] += 1
    g["balls"] = 0
    g["strikes"] = 0
    g["batter_num"] = (g["batter_num"] % 9) + 1

    if g["outs"] >= 3:
        _next_half_inning()


def _advance_batter(hit=False):
    g = game
    if hit:
        team = "away" if g["half"] == "top" else "home"
        g[f"{team}_hits"] += 1
    g["balls"] = 0
    g["strikes"] = 0
    g["batter_num"] = (g["batter_num"] % 9) + 1


def _score_run():
    g = game
    team = "away" if g["half"] == "top" else "home"
    g[f"{team}_runs"] += 1
    idx = min(g["inning"] - 1, 9)
    g[f"{team}_innings"][idx] += 1


def _next_half_inning():
    g = game
    g["outs"] = 0
    g["balls"] = 0
    g["strikes"] = 0
    g["batter_num"] = 1

    if g["half"] == "top":
        g["half"] = "bottom"
    else:
        g["half"] = "top"
        g["inning"] += 1
        if g["inning"] > NUM_INNINGS:
            game.update(make_game())

    # Error occasionally
    if random.random() < 0.05:
        team = random.choice(["home", "away"])
        g[f"{team}_errors"] += 1


def encode_inning_digit(value):
    """Encode a single inning run value."""
    if value == 0:
        return BLANK
    return ord(str(min(value, 9)))


def build_baseball_packet():
    tick()
    g = game

    # Visitor innings: packet[2,3,4] = innings 1-3, packet[17-23] = innings 4-10
    vi = [encode_inning_digit(g["away_innings"][i]) for i in range(10)]
    # Home innings: packet[5,6,7] = innings 1-3, packet[24-30] = innings 4-10
    hi = [encode_inning_digit(g["home_innings"][i]) for i in range(10)]

    # Batter number (2 bytes at 8-9)
    bn = g["batter_num"]
    bn_tens = BLANK if bn < 10 else ord(str(bn // 10))
    bn_ones = BLANK if bn == 0 else ord(str(bn % 10))

    balls_byte = ord(str(min(g["balls"], 3)))
    strikes_byte = ord(str(min(g["strikes"], 2)))
    outs_byte = ord(str(min(g["outs"], 2)))

    # Bytes 11-16: padding (6 bytes)
    pad1 = [0x30] * 6

    # Runs/hits/errors
    vr = encode_two_digit(g["away_runs"], blank_tens=True)
    vh = encode_two_digit(g["away_hits"], blank_tens=True)
    ve_byte = ord(str(min(g["away_errors"], 9)))

    hr = encode_two_digit(g["home_runs"], blank_tens=True)
    hh = encode_two_digit(g["home_hits"], blank_tens=True)
    he_byte = ord(str(min(g["home_errors"], 9)))

    ps = encode_three_digit(g["pitch_speed"])

    # Padding bytes 31-32, 44-45, 49-50
    pad_byte = 0x30

    # Baseball packet: STX(1) + type(1) + data(49) + CR(1) = 52
    # Build data array indexed 2..50 (49 data bytes)
    data = [pad_byte] * 49

    # Visitor innings 1-3 at data indices 0-2 (packet 2-4)
    data[0] = vi[0]
    data[1] = vi[1]
    data[2] = vi[2]
    # Home innings 1-3 at data indices 3-5 (packet 5-7)
    data[3] = hi[0]
    data[4] = hi[1]
    data[5] = hi[2]
    # Batter num at data 6-7 (packet 8-9)
    data[6] = bn_tens
    data[7] = bn_ones
    # Balls at data 8 (packet 10)
    data[8] = balls_byte
    # Padding 11-16: data 9-14
    for i in range(9, 15):
        data[i] = pad_byte
    # Visitor innings 4-10 at data 15-21 (packet 17-23)
    for i in range(7):
        data[15 + i] = vi[3 + i]
    # Home innings 4-10 at data 22-28 (packet 24-30)
    for i in range(7):
        data[22 + i] = hi[3 + i]
    # Strikes at data 29 (packet 31)
    data[29] = strikes_byte
    # Padding at data 30 (packet 32)
    data[30] = pad_byte
    # Visitor R/H/E at data 31-35 (packet 33-37)
    data[31] = vr[0]
    data[32] = vr[1]
    data[33] = vh[0]
    data[34] = vh[1]
    data[35] = ve_byte
    # Home R/H/E at data 36-40 (packet 38-42)
    data[36] = hr[0]
    data[37] = hr[1]
    data[38] = hh[0]
    data[39] = hh[1]
    data[40] = he_byte
    # Outs at data 41 (packet 43)
    data[41] = outs_byte
    # Padding 44-45: data 42-43
    data[42] = pad_byte
    data[43] = pad_byte
    # Pitch speed at data 44-46 (packet 46-48)
    data[44] = ps[0]
    data[45] = ps[1]
    data[46] = ps[2]
    # Padding 49-50: data 47-48
    data[47] = pad_byte
    data[48] = pad_byte

    return build_packet(TP, data)


if __name__ == "__main__":
    parser = make_arg_parser("Baseball", DEFAULT_PORT)
    args = parser.parse_args()
    run_server(args.port, build_baseball_packet, interval=1.0, sport_name="Baseball")
