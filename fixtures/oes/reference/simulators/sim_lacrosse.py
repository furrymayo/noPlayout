"""OES Lacrosse Simulator — type 0x6C, 47-byte packet."""

import random
from oes_helpers import (
    build_packet, encode_clock, encode_score, encode_shot_clock,
    encode_penalty_time, encode_two_digit,
    run_server, make_arg_parser, BLANK,
)

TP = 0x6C
DEFAULT_PORT = 6005

PERIOD_MINUTES = 15
SHOT_CLOCK_RESET = 80


def make_game():
    return {
        "period": 1,
        "clock_sec": PERIOD_MINUTES * 60,
        "shot_clock": SHOT_CLOCK_RESET,
        "home_score": 0,
        "visitor_score": 0,
        "home_tol": 2,
        "visitor_tol": 2,
        "home_shots": 0,
        "visitor_shots": 0,
        "home_pen": [
            {"player": 0, "time_sec": 0},
            {"player": 0, "time_sec": 0},
        ],
        "visitor_pen": [
            {"player": 0, "time_sec": 0},
            {"player": 0, "time_sec": 0},
        ],
    }


game = make_game()


def tick():
    g = game

    if g["clock_sec"] > 0:
        g["clock_sec"] -= 1
        g["shot_clock"] = max(0, g["shot_clock"] - 1)

        # Count down penalties
        for team in ["home_pen", "visitor_pen"]:
            for pen in g[team]:
                if pen["time_sec"] > 0:
                    pen["time_sec"] -= 1

        # Goal (~1 per 5 minutes)
        if random.random() < 0.003:
            team = random.choice(["home", "visitor"])
            g[f"{team}_score"] += 1
            g["shot_clock"] = SHOT_CLOCK_RESET

        # Shots
        if random.random() < 0.01:
            team = random.choice(["home", "visitor"])
            g[f"{team}_shots"] = min(99, g[f"{team}_shots"] + 1)
            g["shot_clock"] = SHOT_CLOCK_RESET

        # Penalty (~1 per 8 minutes)
        if random.random() < 0.002:
            team = random.choice(["home_pen", "visitor_pen"])
            for pen in g[team]:
                if pen["time_sec"] == 0:
                    pen["player"] = random.randint(1, 45)
                    pen["time_sec"] = random.choice([30, 60, 120])
                    break

        # Shot clock reset
        if g["shot_clock"] <= 0:
            g["shot_clock"] = SHOT_CLOCK_RESET

    else:
        if g["period"] < 4:
            g["period"] += 1
            g["clock_sec"] = PERIOD_MINUTES * 60
            g["shot_clock"] = SHOT_CLOCK_RESET
            g["home_tol"] = 2
            g["visitor_tol"] = 2
        else:
            game.update(make_game())


def encode_pen_player(number):
    """Encode a penalty player number into 2 bytes."""
    if number == 0:
        return [BLANK, BLANK]
    return encode_two_digit(number, blank_tens=True)


def encode_pen_time(time_sec):
    """Encode penalty time from seconds into 3 bytes."""
    if time_sec <= 0:
        return encode_penalty_time(0, 0, 0)
    minutes = time_sec // 60
    secs = time_sec % 60
    return encode_penalty_time(minutes, secs // 10, secs % 10)


def build_lacrosse_packet():
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

    # Bytes 11-15: padding (5 bytes to match protocol)
    pad = [0x30, 0x30, 0x30, 0x30, 0x30]

    ht = ord(str(g["home_tol"])) | 0x80
    vt = ord(str(g["visitor_tol"])) | 0x80

    hm_shots = encode_score(g["home_shots"])
    vs_shots = encode_score(g["visitor_shots"])

    # Home penalties (2 slots × 5 bytes each)
    hp1 = encode_pen_player(g["home_pen"][0]["player"]) + encode_pen_time(g["home_pen"][0]["time_sec"])
    hp2 = encode_pen_player(g["home_pen"][1]["player"]) + encode_pen_time(g["home_pen"][1]["time_sec"])

    # Visitor penalties
    vp1 = encode_pen_player(g["visitor_pen"][0]["player"]) + encode_pen_time(g["visitor_pen"][0]["time_sec"])
    vp2 = encode_pen_player(g["visitor_pen"][1]["player"]) + encode_pen_time(g["visitor_pen"][1]["time_sec"])

    sc = encode_shot_clock(g["shot_clock"])

    # Lacrosse packet: STX(1) + type(1) + data(44) + CR(1) = 47
    # Pad to fill remaining bytes
    data = (
        clock            # bytes 2-5 (4)
        + [period_byte]  # byte 6 (1)
        + hs             # bytes 7-8 (2)
        + vs             # bytes 9-10 (2)
        + pad            # bytes 11-15 (5)
        + [ht, vt]       # bytes 16-17 (2)
        + hm_shots       # bytes 18-19 (2)
        + vs_shots       # bytes 20-21 (2)
        + hp1            # bytes 22-26 (5)
        + hp2            # bytes 27-31 (5)
        + vp1            # bytes 32-36 (5)
        + vp2            # bytes 37-41 (5)
        + sc             # bytes 42-43 (2)
        + [0x30]         # byte 44-45 padding to fill to 44 data bytes
    )

    # Ensure exactly 44 data bytes (indices 2-45)
    while len(data) < 44:
        data.append(0x30)
    data = data[:44]

    return build_packet(TP, data)


if __name__ == "__main__":
    parser = make_arg_parser("Lacrosse", DEFAULT_PORT)
    args = parser.parse_args()
    run_server(args.port, build_lacrosse_packet, interval=1.0, sport_name="Lacrosse")
