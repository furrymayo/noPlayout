"""OES Softball Simulator — type 0x74, 75-byte packet."""

import random
from oes_helpers import (
    build_packet, encode_two_digit, encode_three_digit, encode_score,
    run_server, make_arg_parser, BLANK,
)

TP = 0x74
DEFAULT_PORT = 6009

NUM_INNINGS = 7


def make_game():
    return {
        "inning": 1,
        "half": "top",  # top=1, bottom=2
        "outs": 0,
        "balls": 0,
        "strikes": 0,
        "batter_num": 1,
        "batter_avg": 275,
        "pitcher_num": 21,
        "pitcher_count": 0,
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

    g["pitch_speed"] = random.randint(55, 72)
    g["pitcher_count"] += 1

    outcome = random.random()

    if outcome < 0.35:
        g["strikes"] += 1
        if g["strikes"] >= 3:
            _record_out()
    elif outcome < 0.55:
        g["balls"] += 1
        if g["balls"] >= 4:
            _advance_batter(hit=False)
    elif outcome < 0.70:
        if g["strikes"] < 2:
            g["strikes"] += 1
    elif outcome < 0.90:
        _advance_batter(hit=True)
        if random.random() < 0.3:
            _score_run()
    else:
        _record_out()


def _record_out():
    g = game
    g["outs"] += 1
    g["balls"] = 0
    g["strikes"] = 0
    g["batter_num"] = (g["batter_num"] % 9) + 1
    g["batter_avg"] = random.randint(180, 400)

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
    g["batter_avg"] = random.randint(180, 400)


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
    g["batter_avg"] = random.randint(180, 400)

    if g["half"] == "top":
        g["half"] = "bottom"
    else:
        g["half"] = "top"
        g["inning"] += 1
        if g["inning"] > NUM_INNINGS:
            game.update(make_game())

    if random.random() < 0.05:
        team = random.choice(["home", "away"])
        g[f"{team}_errors"] += 1


def encode_inning_digit(value):
    if value == 0:
        return BLANK
    return ord(str(min(value, 9)))


def build_softball_packet():
    tick()
    g = game

    # Build 72 data bytes (packet indices 2..73), total = STX+type+72+CR = 75
    data = [0x30] * 72

    # Byte 2: team at bat (1=top/visitor, 2=bottom/home)
    data[0] = ord("1") if g["half"] == "top" else ord("2")

    # Bytes 3-4: inning (2 digits)
    inn = encode_two_digit(g["inning"], blank_tens=True)
    data[1] = inn[0]
    data[2] = inn[1]

    # Bytes 5-6: batter number
    bn = encode_two_digit(g["batter_num"], blank_tens=True)
    data[3] = bn[0]
    data[4] = bn[1]

    # Bytes 7-9: batter average (3 digits)
    avg = encode_three_digit(g["batter_avg"])
    data[5] = avg[0]
    data[6] = avg[1]
    data[7] = avg[2]

    # Bytes 10-11: pitcher number
    pn = encode_two_digit(g["pitcher_num"], blank_tens=True)
    data[8] = pn[0]
    data[9] = pn[1]

    # Bytes 12-13: pitcher count (low 2 digits), byte 71 = hundreds digit
    pc = min(g["pitcher_count"], 999)
    pc_h = pc // 100
    pc_t = (pc % 100) // 10
    pc_o = pc % 10
    data[69] = BLANK if pc_h == 0 else ord(str(pc_h))  # byte 71 (index 69)
    data[10] = BLANK if (pc_h == 0 and pc_t == 0) else ord(str(pc_t))  # byte 12
    data[11] = ord(str(pc_o))  # byte 13

    # Padding bytes 14-21 (indices 12-19)
    for i in range(12, 20):
        data[i] = 0x30

    # Bytes 22-24: pitch speed
    ps = encode_three_digit(g["pitch_speed"])
    data[20] = ps[0]
    data[21] = ps[1]
    data[22] = ps[2]

    # Byte 25: balls
    data[23] = ord(str(min(g["balls"], 3)))
    # Byte 26: strikes
    data[24] = ord(str(min(g["strikes"], 2)))
    # Byte 27: outs
    data[25] = ord(str(min(g["outs"], 2)))

    # Bytes 28-29: last play type/pos
    data[26] = BLANK  # no last play
    data[27] = BLANK

    # Visitor R/H/E (bytes 30-34)
    vr = encode_two_digit(g["away_runs"], blank_tens=True)
    vh = encode_two_digit(g["away_hits"], blank_tens=True)
    data[28] = vr[0]
    data[29] = vr[1]
    data[30] = vh[0]
    data[31] = vh[1]
    data[32] = ord(str(min(g["away_errors"], 9)))

    # Home R/H/E (bytes 35-39)
    hr = encode_two_digit(g["home_runs"], blank_tens=True)
    hh = encode_two_digit(g["home_hits"], blank_tens=True)
    data[33] = hr[0]
    data[34] = hr[1]
    data[35] = hh[0]
    data[36] = hh[1]
    data[37] = ord(str(min(g["home_errors"], 9)))

    # Visitor innings (bytes 40-49, indices 38-47)
    for i in range(10):
        data[38 + i] = encode_inning_digit(g["away_innings"][i])

    # Home innings (bytes 50-59, indices 48-57)
    for i in range(10):
        data[48 + i] = encode_inning_digit(g["home_innings"][i])

    # Remaining bytes 60-71 are padding (already 0x30)
    # except byte 71 already set above for pitcher count hundreds

    return build_packet(TP, data)


if __name__ == "__main__":
    parser = make_arg_parser("Softball", DEFAULT_PORT)
    args = parser.parse_args()
    run_server(args.port, build_softball_packet, interval=1.0, sport_name="Softball")
