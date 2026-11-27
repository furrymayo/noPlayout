"""OES Volleyball Simulator — type 0x76, 42-byte packet."""

import random
from oes_helpers import (
    build_packet, encode_clock, encode_score,
    run_server, make_arg_parser,
)

TP = 0x76
DEFAULT_PORT = 6003


def make_game():
    return {
        "set": 1,
        "clock_sec": 0,
        "home_score": 0,
        "visitor_score": 0,
        "home_tol": 2,
        "visitor_tol": 2,
        "home_sets_won": 0,
        "visitor_sets_won": 0,
        "set_scores_home": [0, 0, 0, 0, 0],
        "set_scores_visitor": [0, 0, 0, 0, 0],
        "possession": "home",
    }


game = make_game()


def tick():
    g = game

    # Rally scoring: one point per ~3 seconds
    if random.random() < 0.33:
        scorer = random.choice(["home", "visitor"])
        g[f"{scorer}_score"] += 1
        g["possession"] = scorer

        current_set = g["set"] - 1
        g[f"set_scores_{scorer}"][current_set] = g[f"{scorer}_score"]

        # Check set completion
        target = 15 if g["set"] == 5 else 25
        hs, vs = g["home_score"], g["visitor_score"]
        if (hs >= target or vs >= target) and abs(hs - vs) >= 2:
            winner = "home" if hs > vs else "visitor"
            g[f"{winner}_sets_won"] += 1

            if g[f"{winner}_sets_won"] >= 3:
                # Match over — restart
                game.update(make_game())
                return

            g["set"] += 1
            g["home_score"] = 0
            g["visitor_score"] = 0
            g["home_tol"] = 2
            g["visitor_tol"] = 2

    # Timeout
    if random.random() < 0.005:
        team = random.choice(["home", "visitor"])
        key = f"{team}_tol"
        if g[key] > 0:
            g[key] -= 1


def build_volleyball_packet():
    tick()
    g = game

    clock = encode_clock(0, 0)  # Volleyball often shows 0:00 or count-up
    period_byte = ord(str(min(g["set"], 5)))
    hs = encode_score(g["home_score"])
    vs = encode_score(g["visitor_score"])
    ht = ord(str(g["home_tol"])) | 0x80
    vt = ord(str(g["visitor_tol"])) | 0x80

    # Padding bytes 13-15
    pad = [0x30, 0x30, 0x30]

    # Bytes 16-17: possession
    hm_val = 0x30
    vs_val = 0x30
    if g["possession"] == "home":
        hm_val |= 0x01
    else:
        vs_val |= 0x01

    # Sets won
    hm_sets = ord(str(g["home_sets_won"]))
    vs_sets = ord(str(g["visitor_sets_won"]))

    # Set scores: 5 sets × 2 bytes each for home, then visitor
    hm_set_bytes = []
    for s in g["set_scores_home"]:
        hm_set_bytes += encode_score(s)

    vs_set_bytes = []
    for s in g["set_scores_visitor"]:
        vs_set_bytes += encode_score(s)

    # Volleyball packet: STX(1) + type(1) + data(39) + CR(1) = 42
    # data bytes: indices 2-40 (39 bytes)
    data = (
        clock               # bytes 2-5 (4)
        + [period_byte]     # byte 6 (1)
        + hs                # bytes 7-8 (2)
        + vs                # bytes 9-10 (2)
        + [ht, vt]          # bytes 11-12 (2)
        + pad               # bytes 13-15 (3)
        + [hm_val, vs_val]  # bytes 16-17 (2)
        + [hm_sets, vs_sets]  # bytes 18-19 (2)
        + hm_set_bytes      # bytes 20-29 (10)
        + vs_set_bytes      # bytes 30-39 (10)
        + [0x30]            # byte 40 padding (1)
    )

    return build_packet(TP, data)


if __name__ == "__main__":
    parser = make_arg_parser("Volleyball", DEFAULT_PORT)
    args = parser.parse_args()
    run_server(args.port, build_volleyball_packet, interval=1.0, sport_name="Volleyball")
