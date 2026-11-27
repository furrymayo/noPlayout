"""OES Basketball Simulator — type 0x74, 23-byte packet."""

import random
from oes_helpers import (
    build_packet, encode_clock, encode_score, encode_shot_clock,
    run_server, make_arg_parser, BLANK,
)

TP = 0x74
DEFAULT_PORT = 6001

PERIOD_MINUTES = 10
SHOT_CLOCK_RESET = 30


def make_game():
    return {
        "period": 1,
        "clock_sec": PERIOD_MINUTES * 60,
        "shot_clock": SHOT_CLOCK_RESET,
        "home_score": 0,
        "visitor_score": 0,
        "home_tol": 4,
        "visitor_tol": 4,
        "home_fouls": 0,
        "visitor_fouls": 0,
        "home_bonus": False,
        "visitor_bonus": False,
        "possession": "home",
    }


game = make_game()


def tick():
    g = game

    # Clock countdown
    if g["clock_sec"] > 0:
        g["clock_sec"] -= 1
        g["shot_clock"] = max(0, g["shot_clock"] - 1)

        # Scoring events (~every 20s on average)
        if random.random() < 0.05:
            pts = random.choice([2, 2, 2, 3])
            if g["possession"] == "home":
                g["home_score"] += pts
            else:
                g["visitor_score"] += pts
            g["shot_clock"] = SHOT_CLOCK_RESET
            g["possession"] = "visitor" if g["possession"] == "home" else "home"

        # Shot clock reset on possession change
        if g["shot_clock"] <= 0:
            g["shot_clock"] = SHOT_CLOCK_RESET
            g["possession"] = "visitor" if g["possession"] == "home" else "home"

        # Fouls (~every 40s)
        if random.random() < 0.025:
            team = random.choice(["home", "visitor"])
            g[f"{team}_fouls"] = min(10, g[f"{team}_fouls"] + 1)
            if g[f"{team}_fouls"] >= 7:
                g[f"{team}_bonus"] = True

        # Timeouts (~every 2 min)
        if random.random() < 0.008:
            team = random.choice(["home", "visitor"])
            key = f"{team}_tol"
            if g[key] > 0:
                g[key] -= 1

    else:
        # Period transition
        if g["period"] < 4:
            g["period"] += 1
            g["clock_sec"] = PERIOD_MINUTES * 60
            g["shot_clock"] = SHOT_CLOCK_RESET
            g["home_fouls"] = 0
            g["visitor_fouls"] = 0
            g["home_bonus"] = False
            g["visitor_bonus"] = False
            g["home_tol"] = 4
            g["visitor_tol"] = 4
        else:
            # Game over — restart
            game.update(make_game())


def build_basketball_packet():
    tick()
    g = game
    minutes = g["clock_sec"] // 60
    seconds = g["clock_sec"] % 60

    if minutes == 0 and seconds < 60:
        clock = encode_clock(0, seconds, tenths=random.randint(0, 9))
    else:
        clock = encode_clock(minutes, seconds)

    period_byte = ord(str(min(g["period"], 9)))
    hs = encode_score(g["home_score"])
    vs = encode_score(g["visitor_score"])
    ht = ord(str(g["home_tol"])) | 0x80
    vt = ord(str(g["visitor_tol"])) | 0x80

    # Fouls
    hf = g["home_fouls"]
    if hf >= 10:
        hf_byte = BLANK + 1  # >0x3A triggers "10"
    elif hf == 0:
        hf_byte = BLANK
    else:
        hf_byte = ord(str(hf))

    vf = g["visitor_fouls"]
    if vf >= 10:
        vf_byte = BLANK + 1
    elif vf == 0:
        vf_byte = BLANK
    else:
        vf_byte = ord(str(vf))

    # Byte 15 (unused in parse, just padding)
    pad15 = 0x30

    # Bytes 16-17: possession/bonus packed
    hm_val = 0x30
    if g["possession"] == "home":
        hm_val |= 0x01
    if g["home_bonus"]:
        hm_val |= 0x02

    vs_val = 0x30
    if g["possession"] == "visitor":
        vs_val |= 0x01
    if g["visitor_bonus"]:
        vs_val |= 0x02

    sc = encode_shot_clock(g["shot_clock"])

    # Pad to total 21 data bytes (packet = STX + type + 21 data + CR = 23+CR... wait)
    # Basketball packet length = 23 total including STX and CR
    # STX(1) + type(1) + data(20) + CR(1) = 23
    # data bytes: indices 2-21 (20 bytes)
    # clock[4] + period[1] + hs[2] + vs[2] + ht[1] + vt[1] + hf[1] + vf[1] + pad[1] + hm_val[1] + vs_val[1] + sc[2] + pad[2] = 20
    data = (
        clock          # bytes 2-5 (4)
        + [period_byte]  # byte 6 (1)
        + hs             # bytes 7-8 (2)
        + vs             # bytes 9-10 (2)
        + [ht, vt]       # bytes 11-12 (2)
        + [hf_byte, vf_byte]  # bytes 13-14 (2)
        + [pad15]        # byte 15 (1)
        + [hm_val, vs_val]  # bytes 16-17 (2)
        + sc             # bytes 18-19 (2)
        + [0x30, 0x30]   # bytes 20-21 padding (2)
    )

    return build_packet(TP, data)


if __name__ == "__main__":
    parser = make_arg_parser("Basketball", DEFAULT_PORT)
    args = parser.parse_args()
    run_server(args.port, build_basketball_packet, interval=1.0, sport_name="Basketball")
