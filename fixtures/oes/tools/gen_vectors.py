"""Regenerate fixtures/oes/vectors/ from the reference implementation in fixtures/oes/reference/.

    python3 fixtures/oes/tools/gen_vectors.py

Deterministic (fixed seeds): running it again must leave `git diff fixtures/oes/vectors` empty.
The reference decoder (protocol.py) is the oracle; the Rust OES decoder (M4.5) must reproduce
every `decoded` object from every `hex` packet, and the framing results in streams.json.
Standard library only; Python 3.9+.
"""

import importlib
import json
import random
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OES = HERE.parent
REF = OES / "reference"
OUT = OES / "vectors"
sys.path.insert(0, str(REF))
sys.path.insert(0, str(REF / "simulators"))

import protocol  # noqa: E402  (fixtures/oes/reference/protocol.py)

PER_SPORT = 240  # packets per sport: covers period changes, scoring, clock under one minute

SPORTS = [
    # (simulator module, packet builder, expected sport name, seed)
    ("sim_basketball", "build_basketball_packet", "Basketball", 1001),
    ("sim_baseball", "build_baseball_packet", "Baseball", 1002),
    ("sim_softball", "build_softball_packet", "Softball", 1003),
    ("sim_football", "build_football_packet", "Football", 1004),
    ("sim_volleyball", "build_volleyball_packet", "Volleyball", 1005),
    ("sim_lacrosse", "build_lacrosse_packet", "Lacrosse", 1006),
    ("sim_hockey", "build_hockey_packet", "Hockey", 1007),
    ("sim_wrestling", "build_wrestling_packet", "Wrestling", 1008),
    ("sim_soccer", "build_soccer_packet", "Soccer", 1009),
]

# A real football datagram captured from an OES controller behind a serial-to-Ethernet bridge
# (2026-08-12): Football, 10-3, Q2, 2nd and 10 on the 25. Venue and addresses removed.
CAPTURED_FOOTBALL = bytes.fromhex("026634b8b3393231303a3330303a3a32313032353430382a0d")

# Documentation addresses (RFC 5737) stand in for the two real senders of the original test.
SENDER_A = "192.0.2.10:5002"
SENDER_B = "192.0.2.11:5002"


def decode(packet):
    sport, fields = protocol.identify_and_parse(list(packet))
    return sport, fields


def sport_vectors():
    index = []
    for module, builder, sport, seed in SPORTS:
        random.seed(seed)
        mod = importlib.import_module(module)
        build = getattr(mod, builder)
        rows = []
        for i in range(PER_SPORT):
            pkt = bytes(build())
            framed = protocol.PacketStreamParser().feed_bytes(pkt)
            assert [bytes(p) for p in framed] == [pkt], f"{sport} #{i} does not frame alone"
            got_sport, fields = decode(pkt)
            assert got_sport == sport, f"{sport} #{i} decoded as {got_sport}"
            rows.append({"i": i, "hex": pkt.hex(), "decoded": fields})
        name = f"{sport.lower()}.jsonl"
        with open(OUT / name, "w", encoding="utf-8", newline="\n") as f:
            for r in rows:
                f.write(json.dumps(r, sort_keys=True, ensure_ascii=False) + "\n")
        index.append({"sport": sport, "file": name, "packets": len(rows), "seed": seed,
                      "length": len(bytes.fromhex(rows[0]["hex"])), "type": rows[0]["hex"][2:4]})
    return index


def football_with_home(score):
    p = bytearray(CAPTURED_FOOTBALL)
    p[7], p[8] = ord(score[0]), ord(score[1])
    return bytes(p)


def run_stream(datagrams, per_sender):
    """Feed datagrams through one parser per sender (or one shared parser) and decode."""
    parsers = {}
    results = []
    for sender, data in datagrams:
        key = sender if per_sender else "*"
        parser = parsers.setdefault(key, protocol.PacketStreamParser())
        for pkt in parser.feed_bytes(data):
            sport, fields = decode(bytes(pkt))
            results.append({"sender": sender, "hex": bytes(pkt).hex(), "sport": sport,
                            "decoded": fields})
    return results


def stream_vectors():
    cases = []

    def case(name, note, datagrams):
        cases.append({
            "name": name,
            "note": note,
            "datagrams": [{"sender": s, "hex": d.hex()} for s, d in datagrams],
            "expected_per_sender": run_stream(datagrams, per_sender=True),
        })

    p10, p21 = football_with_home("10"), football_with_home("21")
    case("captured_datagram", "the captured football datagram alone", [(SENDER_A, CAPTURED_FOOTBALL)])
    case("two_senders", "two senders are framed and attributed separately",
         [(SENDER_A, p10), (SENDER_B, p21)])
    case("split_frame_with_interloper",
         "A's frame split around B's datagram; per-sender framing keeps both",
         [(SENDER_A, p10[:12]), (SENDER_B, p21), (SENDER_A, p10[12:])])
    case("foreign_fragment_not_spliced",
         "a bare fragment from B mid-frame of A must not land in A's packet (A stays 10-3)",
         [(SENDER_A, p10[:7]), (SENDER_B, b"99"), (SENDER_A, p10[7:])])
    case("repeat_datagrams", "one sender, two datagrams, latest wins",
         [(SENDER_A, p10), (SENDER_A, football_with_home("14"))])

    # Byte-stream framing (serial/TCP): garbage, a bad type byte, a control byte mid-frame, and
    # all sports concatenated, delivered in uneven chunks.
    random.seed(2001)
    blob = bytearray(b"\xff\x10\x00")
    blob += bytes([protocol.STX, 0x01, 0x30, 0x30, protocol.CR])  # invalid type: dropped
    blob += CAPTURED_FOOTBALL[:10] + b"\x07" + CAPTURED_FOOTBALL[10:]  # control byte: dropped
    for module, builder, _sport, seed in SPORTS:
        random.seed(seed + 500)
        mod = importlib.reload(importlib.import_module(module))
        for _ in range(3):
            blob += bytes(getattr(mod, builder)())
    chunks, i = [], 0
    while i < len(blob):
        n = random.randint(1, 40)
        chunks.append(("serial", bytes(blob[i:i + n])))
        i += n
    case("byte_stream_chunked", "serial/TCP stream in uneven chunks with garbage and bad frames",
         chunks)
    return cases


def main():
    OUT.mkdir(exist_ok=True)
    index = sport_vectors()
    streams = stream_vectors()
    with open(OUT / "streams.json", "w", encoding="utf-8", newline="\n") as f:
        json.dump({"cases": streams}, f, indent=1, sort_keys=True, ensure_ascii=False)
        f.write("\n")
    with open(OUT / "index.json", "w", encoding="utf-8", newline="\n") as f:
        json.dump({"generator": "fixtures/oes/tools/gen_vectors.py", "per_sport": index,
                   "stream_cases": [c["name"] for c in streams]}, f, indent=1, sort_keys=True)
        f.write("\n")
    total = sum(s["packets"] for s in index)
    print(f"wrote {total} packets in {len(index)} sports and {len(streams)} stream cases to {OUT}")


if __name__ == "__main__":
    main()
