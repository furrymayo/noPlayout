"""OES Football Simulator — type 0x66, 24-byte packet."""

import random
from oes_helpers import (
    build_packet, encode_clock, encode_score, encode_shot_clock,
    run_server, make_arg_parser, BLANK,
)

TP = 0x66
DEFAULT_PORT = 6002

QUARTER_MINUTES = 15
PLAY_CLOCK_RESET = 25


def make_game():
    return {
        "quarter": 1,
        "clock_sec": QUARTER_MINUTES * 60,
        "home_score": 0,
        "visitor_score": 0,
        "home_tol": 3,
        "visitor_tol": 3,
        "possession": "home",
        "down": 1,
        "yards_to_go": 10,
        "ball_on": 25,
        "play_clock": PLAY_CLOCK_RESET,
    }


game = make_game()


def tick():
    g = game

    if g["clock_sec"] > 0:
        g["clock_sec"] -= 1
        g["play_clock"] = max(0, g["play_clock"] - 1)

        # Play happens when play clock expires
        if g["play_clock"] <= 0:
            # Simulate a play
            yards = random.choice([-2, 0, 1, 2, 3, 4, 5, 6, 8, 12, 20, 40])

            g["ball_on"] = max(1, min(99, g["ball_on"] + yards))
            g["yards_to_go"] -= yards

            if g["yards_to_go"] <= 0:
                # First down
                g["down"] = 1
                g["yards_to_go"] = 10
            else:
                g["down"] += 1

            # Touchdown
            if g["ball_on"] >= 99 or yards >= 40:
                team = g["possession"]
                g[f"{team}_score"] += 7
                g["ball_on"] = 25
                g["down"] = 1
                g["yards_to_go"] = 10
                g["possession"] = "visitor" if g["possession"] == "home" else "home"

            # Turnover on downs
            if g["down"] > 4:
                g["down"] = 1
                g["yards_to_go"] = 10
                g["ball_on"] = max(1, 100 - g["ball_on"])
                g["possession"] = "visitor" if g["possession"] == "home" else "home"

            # Field goal attempt (~10% on 4th down)
            if g["down"] == 4 and g["ball_on"] > 55 and random.random() < 0.5:
                if random.random() < 0.7:
                    g[f"{g['possession']}_score"] += 3
                g["ball_on"] = 25
                g["down"] = 1
                g["yards_to_go"] = 10
                g["possession"] = "visitor" if g["possession"] == "home" else "home"

            g["play_clock"] = PLAY_CLOCK_RESET

        # Timeout
        if random.random() < 0.005:
            key = f"{g['possession']}_tol"
            if g[key] > 0:
                g[key] -= 1

    else:
        if g["quarter"] < 4:
            g["quarter"] += 1
            g["clock_sec"] = QUARTER_MINUTES * 60
            g["home_tol"] = 3
            g["visitor_tol"] = 3
            g["play_clock"] = PLAY_CLOCK_RESET
        else:
            game.update(make_game())


def build_football_packet():
    tick()
    g = game
    minutes = g["clock_sec"] // 60
    seconds = g["clock_sec"] % 60

    if minutes == 0 and seconds < 60:
        clock = encode_clock(0, seconds, tenths=random.randint(0, 9))
    else:
        clock = encode_clock(minutes, seconds)

    quarter_byte = ord(str(min(g["quarter"], 9)))
    hs = encode_score(g["home_score"])
    vs = encode_score(g["visitor_score"])
    ht = ord(str(g["home_tol"])) | 0x80
    vt = ord(str(g["visitor_tol"])) | 0x80

    # Possession: byte 13 or 14 = 0xB8
    hm_poss = 0xB8 if g["possession"] == "home" else 0x30
    vs_poss = 0xB8 if g["possession"] == "visitor" else 0x30

    down_byte = ord(str(min(g["down"], 4)))

    # Yards to go
    ytg = max(1, min(99, g["yards_to_go"]))
    ytg_tens = ytg // 10
    ytg_ones = ytg % 10
    ytg_tens_byte = BLANK if ytg_tens == 0 else ord(str(ytg_tens))
    ytg_ones_byte = ord(str(ytg_ones))

    # Ball on
    bo = max(1, min(50, g["ball_on"] if g["ball_on"] <= 50 else 100 - g["ball_on"]))
    bo_tens = bo // 10
    bo_ones = bo % 10
    bo_tens_byte = BLANK if bo_tens == 0 else ord(str(bo_tens))
    bo_ones_byte = ord(str(bo_ones))

    play_clock = encode_shot_clock(g["play_clock"])

    # Football packet: STX(1) + type(1) + data(21) + CR(1) = 24
    # data indices 2-22 (21 bytes)
    data = (
        clock                     # bytes 2-5 (4)
        + [quarter_byte]          # byte 6 (1)
        + hs                      # bytes 7-8 (2)
        + vs                      # bytes 9-10 (2)
        + [ht, vt]                # bytes 11-12 (2)
        + [hm_poss, vs_poss]      # bytes 13-14 (2)
        + [down_byte]             # byte 15 (1)
        + [ytg_tens_byte, ytg_ones_byte]  # bytes 16-17 (2)
        + [bo_tens_byte, bo_ones_byte]    # bytes 18-19 (2)
        + play_clock              # bytes 20-21 (2)
        + [0x30]                  # byte 22 padding (1)
    )

    return build_packet(TP, data)


if __name__ == "__main__":
    parser = make_arg_parser("Football", DEFAULT_PORT)
    args = parser.parse_args()
    run_server(args.port, build_football_packet, interval=1.0, sport_name="Football")
