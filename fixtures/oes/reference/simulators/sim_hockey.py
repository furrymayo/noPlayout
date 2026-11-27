"""OES Hockey (Field Hockey) Simulator — type 0x6C, 51-byte packet."""

import random
from oes_helpers import (
    build_packet, encode_clock, encode_score,
    encode_penalty_time, encode_two_digit,
    run_server, make_arg_parser, BLANK,
)

TP = 0x6C
DEFAULT_PORT = 6006

HALF_MINUTES = 35


def make_game():
    return {
        "period": 1,
        "clock_sec": HALF_MINUTES * 60,
        "home_score": 0,
        "visitor_score": 0,
        "home_saves": 0,
        "visitor_saves": 0,
        "home_shots": 0,
        "visitor_shots": 0,
        "home_corners": 0,
        "visitor_corners": 0,
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

        # Count down penalties
        for team in ["home_pen", "visitor_pen"]:
            for pen in g[team]:
                if pen["time_sec"] > 0:
                    pen["time_sec"] -= 1

        # Goal (~1 per 20 minutes)
        if random.random() < 0.001:
            team = random.choice(["home", "visitor"])
            g[f"{team}_score"] += 1

        # Shots
        if random.random() < 0.01:
            team = random.choice(["home", "visitor"])
            g[f"{team}_shots"] = min(99, g[f"{team}_shots"] + 1)

        # Saves
        if random.random() < 0.007:
            team = random.choice(["home", "visitor"])
            g[f"{team}_saves"] = min(99, g[f"{team}_saves"] + 1)

        # Corners
        if random.random() < 0.003:
            team = random.choice(["home", "visitor"])
            g[f"{team}_corners"] = min(99, g[f"{team}_corners"] + 1)

        # Penalty
        if random.random() < 0.002:
            team = random.choice(["home_pen", "visitor_pen"])
            for pen in g[team]:
                if pen["time_sec"] == 0:
                    pen["player"] = random.randint(1, 30)
                    pen["time_sec"] = random.choice([60, 120, 300])
                    break

    else:
        if g["period"] < 2:
            g["period"] = 2
            g["clock_sec"] = HALF_MINUTES * 60
        else:
            game.update(make_game())


def encode_pen_player(number):
    if number == 0:
        return [BLANK, BLANK]
    return encode_two_digit(number, blank_tens=True)


def encode_pen_time(time_sec):
    if time_sec <= 0:
        return encode_penalty_time(0, 0, 0)
    minutes = time_sec // 60
    secs = time_sec % 60
    return encode_penalty_time(minutes, secs // 10, secs % 10)


def encode_saves(value):
    """Encode saves as 2 bytes (same as score but handles 0 as BLANK+0x3A)."""
    value = max(0, int(value)) % 100
    tens = value // 10
    ones = value % 10
    tens_byte = BLANK if tens == 0 else ord(str(tens))
    ones_byte = BLANK if (tens == 0 and ones == 0) else ord(str(ones))
    return [tens_byte, ones_byte]


def build_hockey_packet():
    tick()
    g = game
    minutes = g["clock_sec"] // 60
    seconds = g["clock_sec"] % 60

    if minutes == 0 and seconds < 60:
        clock = encode_clock(0, seconds, tenths=random.randint(0, 9))
    else:
        clock = encode_clock(minutes, seconds)

    period_byte = ord(str(g["period"]))
    hs = encode_score(g["home_score"])
    vs = encode_score(g["visitor_score"])

    hm_saves = encode_saves(g["home_saves"])
    vs_saves = encode_saves(g["visitor_saves"])

    # Bytes 15-17: padding (3 bytes)
    pad = [0x30, 0x30, 0x30]

    hm_shots = encode_score(g["home_shots"])
    vs_shots = encode_score(g["visitor_shots"])

    # Home penalties
    hp1 = encode_pen_player(g["home_pen"][0]["player"]) + encode_pen_time(g["home_pen"][0]["time_sec"])
    hp2 = encode_pen_player(g["home_pen"][1]["player"]) + encode_pen_time(g["home_pen"][1]["time_sec"])

    # Visitor penalties
    vp1 = encode_pen_player(g["visitor_pen"][0]["player"]) + encode_pen_time(g["visitor_pen"][0]["time_sec"])
    vp2 = encode_pen_player(g["visitor_pen"][1]["player"]) + encode_pen_time(g["visitor_pen"][1]["time_sec"])

    hm_corners = encode_saves(g["home_corners"])
    vs_corners = encode_saves(g["visitor_corners"])

    # Hockey packet: STX(1) + type(1) + data(48) + CR(1) = 51
    data = (
        clock            # bytes 2-5 (4)
        + [period_byte]  # byte 6 (1)
        + hs             # bytes 7-8 (2)
        + vs             # bytes 9-10 (2)
        + hm_saves       # bytes 11-12 (2)
        + vs_saves       # bytes 13-14 (2)
        + pad            # bytes 15-17 (3)
        + hm_shots       # bytes 18-19 (2)
        + vs_shots       # bytes 20-21 (2)
        + hp1            # bytes 22-26 (5)
        + hp2            # bytes 27-31 (5)
        + vp1            # bytes 32-36 (5)
        + vp2            # bytes 37-41 (5)
        + hm_corners     # bytes 42-43 (2)
        + vs_corners     # bytes 44-45 (2)
    )

    # Ensure exactly 48 data bytes
    while len(data) < 48:
        data.append(0x30)
    data = data[:48]

    return build_packet(TP, data)


if __name__ == "__main__":
    parser = make_arg_parser("Hockey", DEFAULT_PORT)
    args = parser.parse_args()
    run_server(args.port, build_hockey_packet, interval=1.0, sport_name="Hockey")
