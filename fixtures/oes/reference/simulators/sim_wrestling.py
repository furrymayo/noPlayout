"""OES Wrestling Simulator — type 0x77, 42-byte packet."""

import random
from oes_helpers import (
    build_packet, encode_clock, encode_score, encode_simple_time,
    run_server, make_arg_parser,
)

TP = 0x77
DEFAULT_PORT = 6007

WEIGHT_CLASSES = [125, 133, 141, 149, 157, 165, 174, 184, 197, 285]
PERIOD_DURATIONS = [180, 120, 120]  # 3:00, 2:00, 2:00


def make_match():
    return {
        "weight_idx": 0,
        "period": 1,
        "clock_sec": PERIOD_DURATIONS[0],
        "home_score": 0,
        "visitor_score": 0,
        "home_team_points": 0,
        "visitor_team_points": 0,
        "home_adv_sec": 0,
        "visitor_adv_sec": 0,
        "home_inj_sec": 0,
        "visitor_inj_sec": 0,
    }


game = make_match()


def tick():
    g = game

    if g["clock_sec"] > 0:
        g["clock_sec"] -= 1

        # Scoring events (~1 per 30s)
        if random.random() < 0.03:
            team = random.choice(["home", "visitor"])
            pts = random.choice([1, 2, 2, 3])
            g[f"{team}_score"] += pts

        # Advantage time ticks up occasionally
        if random.random() < 0.01:
            team = random.choice(["home", "visitor"])
            g[f"{team}_adv_sec"] = min(599, g[f"{team}_adv_sec"] + random.randint(5, 15))

        # Injury time (rare)
        if random.random() < 0.003:
            team = random.choice(["home", "visitor"])
            g[f"{team}_inj_sec"] = min(599, g[f"{team}_inj_sec"] + random.randint(10, 30))

    else:
        if g["period"] < 3:
            g["period"] += 1
            g["clock_sec"] = PERIOD_DURATIONS[g["period"] - 1]
        else:
            # Match over — award team points
            hs, vs = g["home_score"], g["visitor_score"]
            diff = abs(hs - vs)
            if diff == 0:
                pts = 2  # draw
                g["home_team_points"] += pts
                g["visitor_team_points"] += pts
            else:
                winner = "home" if hs > vs else "visitor"
                if diff >= 8:
                    pts = 5  # major decision
                elif diff >= 4:
                    pts = 4  # decision
                else:
                    pts = 3
                g[f"{winner}_team_points"] += pts

            # Next weight class
            g["weight_idx"] += 1
            if g["weight_idx"] >= len(WEIGHT_CLASSES):
                # Dual meet over — restart
                game.update(make_match())
                return

            g["period"] = 1
            g["clock_sec"] = PERIOD_DURATIONS[0]
            g["home_score"] = 0
            g["visitor_score"] = 0
            g["home_adv_sec"] = 0
            g["visitor_adv_sec"] = 0
            g["home_inj_sec"] = 0
            g["visitor_inj_sec"] = 0


def build_wrestling_packet():
    tick()
    g = game
    minutes = g["clock_sec"] // 60
    seconds = g["clock_sec"] % 60

    if minutes == 0 and seconds < 60:
        clock = encode_clock(0, seconds, tenths=random.randint(0, 9))
    else:
        clock = encode_clock(minutes, seconds)

    period_byte = ord(str(min(g["period"], 3)))
    hs = encode_score(g["home_score"])
    vs = encode_score(g["visitor_score"])

    # Padding bytes 11-17 (7 bytes)
    pad1 = [0x30] * 7

    hm_tp = encode_score(g["home_team_points"])
    vs_tp = encode_score(g["visitor_team_points"])

    # Weight class as 3 ASCII bytes
    wc = WEIGHT_CLASSES[g["weight_idx"]]
    wc_h = wc // 100
    wc_t = (wc % 100) // 10
    wc_o = wc % 10
    wc_bytes = [ord(str(wc_h)), ord(str(wc_t)), ord(str(wc_o))]

    hm_adv = encode_simple_time(g["home_adv_sec"] // 60, g["home_adv_sec"] % 60)
    vs_adv = encode_simple_time(g["visitor_adv_sec"] // 60, g["visitor_adv_sec"] % 60)

    # Padding bytes 31-33 (3 bytes)
    pad2 = [0x30, 0x30, 0x30]

    hm_inj = encode_simple_time(g["home_inj_sec"] // 60, g["home_inj_sec"] % 60)
    vs_inj = encode_simple_time(g["visitor_inj_sec"] // 60, g["visitor_inj_sec"] % 60)

    # Wrestling packet: STX(1) + type(1) + data(39) + CR(1) = 42
    data = (
        clock           # bytes 2-5 (4)
        + [period_byte] # byte 6 (1)
        + hs            # bytes 7-8 (2)
        + vs            # bytes 9-10 (2)
        + pad1          # bytes 11-17 (7)
        + hm_tp         # bytes 18-19 (2)
        + vs_tp         # bytes 20-21 (2)
        + wc_bytes      # bytes 22-24 (3)
        + hm_adv        # bytes 25-27 (3)
        + vs_adv        # bytes 28-30 (3)
        + pad2          # bytes 31-33 (3)
        + hm_inj        # bytes 34-36 (3)
        + vs_inj        # bytes 37-39 (3)
    )

    # Ensure exactly 39 data bytes
    while len(data) < 39:
        data.append(0x30)
    data = data[:39]

    return build_packet(TP, data)


if __name__ == "__main__":
    parser = make_arg_parser("Wrestling", DEFAULT_PORT)
    args = parser.parse_args()
    run_server(args.port, build_wrestling_packet, interval=1.0, sport_name="Wrestling")
