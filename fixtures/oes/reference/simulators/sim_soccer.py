"""OES Soccer Simulator — type 0x73, 28-byte packet."""

import random
from oes_helpers import (
    build_packet, encode_clock, encode_score,
    run_server, make_arg_parser,
)

TP = 0x73
DEFAULT_PORT = 6004

HALF_MINUTES = 45


def make_game():
    return {
        "period": 1,
        "clock_sec": HALF_MINUTES * 60,
        "home_score": 0,
        "visitor_score": 0,
        "home_shots": 0,
        "home_saves": 0,
        "home_corners": 0,
        "home_penalties": 0,
        "visitor_shots": 0,
        "visitor_saves": 0,
        "visitor_corners": 0,
        "visitor_penalties": 0,
    }


game = make_game()


def tick():
    g = game

    if g["clock_sec"] > 0:
        g["clock_sec"] -= 1

        # Goal (~1 every 20 minutes = 0.0008/sec)
        if random.random() < 0.001:
            team = random.choice(["home", "visitor"])
            g[f"{team}_score"] += 1

        # Shots (~1 per minute)
        if random.random() < 0.017:
            team = random.choice(["home", "visitor"])
            g[f"{team}_shots"] = min(99, g[f"{team}_shots"] + 1)

        # Saves (~1 per 2 minutes)
        if random.random() < 0.008:
            team = random.choice(["home", "visitor"])
            g[f"{team}_saves"] = min(99, g[f"{team}_saves"] + 1)

        # Corners (~1 per 5 minutes)
        if random.random() < 0.003:
            team = random.choice(["home", "visitor"])
            g[f"{team}_corners"] = min(99, g[f"{team}_corners"] + 1)

    else:
        if g["period"] < 2:
            g["period"] = 2
            g["clock_sec"] = HALF_MINUTES * 60
        else:
            game.update(make_game())


def build_soccer_packet():
    tick()
    g = game
    minutes = g["clock_sec"] // 60
    seconds = g["clock_sec"] % 60

    clock = encode_clock(minutes, seconds)
    period_byte = ord(str(g["period"]))
    hs = encode_score(g["home_score"])
    vs = encode_score(g["visitor_score"])

    hm_shots = encode_score(g["home_shots"])
    hm_saves = encode_score(g["home_saves"])
    hm_corners = encode_score(g["home_corners"])
    hm_penalties = encode_score(g["home_penalties"])

    vs_shots = encode_score(g["visitor_shots"])
    vs_saves = encode_score(g["visitor_saves"])
    vs_corners = encode_score(g["visitor_corners"])
    vs_penalties = encode_score(g["visitor_penalties"])

    # Soccer packet: STX(1) + type(1) + data(25) + CR(1) = 28
    data = (
        clock            # bytes 2-5 (4)
        + [period_byte]  # byte 6 (1)
        + hs             # bytes 7-8 (2)
        + vs             # bytes 9-10 (2)
        + hm_shots       # bytes 11-12 (2)
        + hm_saves       # bytes 13-14 (2)
        + hm_corners     # bytes 15-16 (2)
        + hm_penalties   # bytes 17-18 (2)
        + vs_shots       # bytes 19-20 (2)
        + vs_saves       # bytes 21-22 (2)
        + vs_corners     # bytes 23-24 (2)
        + vs_penalties   # bytes 25-26 (2)
    )

    return build_packet(TP, data)


if __name__ == "__main__":
    parser = make_arg_parser("Soccer", DEFAULT_PORT)
    args = parser.parse_args()
    run_server(args.port, build_soccer_packet, interval=1.0, sport_name="Soccer")
