"""Shared OES encoding helpers and TCP server for sport simulators.

Provides encoding functions (reverse of protocol.py decoders) and a
multi-client TCP server loop for streaming OES packets.
"""

import socket
import threading
import time
import argparse

STX = 0x02
CR = 0x0D
BLANK = 0x3A  # used for blank/zero digit positions


# --- Encoding helpers (reverse of protocol.py decoders) ---

def encode_score(value):
    """Encode an integer score (0-199) into 2 OES bytes."""
    value = max(0, int(value))
    if value >= 100:
        remainder = value - 100
        tens = ord(str(remainder // 10)) | 0x80
        ones = ord(str(remainder % 10)) | 0x80
        return [tens, ones]
    tens_digit = value // 10
    ones_digit = value % 10
    tens = BLANK if tens_digit == 0 else ord(str(tens_digit))
    ones = ord(str(ones_digit))
    return [tens, ones]


def encode_clock(minutes, seconds, tenths=None):
    """Encode a clock value into 4 OES bytes.

    If minutes == 0 and tenths is provided, uses tenths format.
    """
    minutes = max(0, int(minutes))
    seconds = max(0, int(seconds))

    if minutes == 0 and tenths is not None:
        # Under 1 minute: tenths format
        sec_tens = seconds // 10
        sec_ones = seconds % 10
        tenths = max(0, int(tenths)) % 10
        if sec_tens == 0:
            min_tens = BLANK
        else:
            min_tens = ord(str(sec_tens))
        min_ones = ord(str(sec_ones))
        sec_tens_byte = ord(str(tenths))
        sec_ones_byte = BLANK  # signals tenths mode
        return [min_tens | 0x80, min_ones | 0x80, sec_tens_byte | 0x80, sec_ones_byte]

    # Normal MM:SS format
    min_tens = minutes // 10
    min_ones = minutes % 10
    sec_tens = seconds // 10
    sec_ones = seconds % 10
    if min_tens == 0:
        mt = BLANK
    else:
        mt = ord(str(min_tens))
    return [
        mt | 0x80,
        ord(str(min_ones)) | 0x80,
        ord(str(sec_tens)) | 0x80,
        ord(str(sec_ones)),
    ]


def encode_shot_clock(value):
    """Encode a shot clock value (0-99) into 2 OES bytes."""
    value = max(0, min(99, int(value)))
    tens = value // 10
    ones = value % 10
    tens_byte = BLANK if tens == 0 else ord(str(tens))
    return [tens_byte, ord(str(ones))]


def encode_penalty_time(minutes, sec_tens, sec_ones):
    """Encode a penalty timer into 3 OES bytes: min|0x80, sec_tens|0x80, sec_ones."""
    return [
        ord(str(int(minutes) % 10)) | 0x80,
        ord(str(int(sec_tens) % 10)) | 0x80,
        ord(str(int(sec_ones) % 10)),
    ]


def encode_simple_time(minutes, seconds):
    """Encode a simple time (advantage/injury) into 3 OES bytes."""
    minutes = max(0, int(minutes)) % 10
    sec_tens = int(seconds) // 10
    sec_ones = int(seconds) % 10
    return [
        ord(str(minutes)) | 0x80,
        ord(str(sec_tens)) | 0x80,
        ord(str(sec_ones)),
    ]


def encode_digit(value, blank_zero=True):
    """Encode a single digit (0-9) into 1 OES byte. 0 -> BLANK if blank_zero."""
    value = max(0, int(value)) % 10
    if value == 0 and blank_zero:
        return BLANK
    return ord(str(value))


def encode_two_digit(value, blank_tens=True):
    """Encode a two-digit value into 2 OES bytes."""
    value = max(0, int(value)) % 100
    tens = value // 10
    ones = value % 10
    tens_byte = BLANK if (tens == 0 and blank_tens) else ord(str(tens))
    ones_byte = ord(str(ones))
    return [tens_byte, ones_byte]


def encode_three_digit(value):
    """Encode a three-digit value (e.g. weight class, pitch speed) into 3 OES bytes."""
    value = max(0, int(value)) % 1000
    h = value // 100
    t = (value % 100) // 10
    o = value % 10
    return [
        BLANK if h == 0 else ord(str(h)),
        BLANK if (h == 0 and t == 0) else ord(str(t)),
        ord(str(o)),
    ]


def build_packet(type_byte, data_bytes):
    """Wrap data with STX + type_byte + data + CR."""
    return bytes([STX, type_byte] + list(data_bytes) + [CR])


# --- TCP Server ---

def run_server(port, packet_builder_fn, interval=1.0, sport_name="OES"):
    """TCP server that streams OES packets to connected clients.

    Args:
        port: TCP port to listen on
        packet_builder_fn: callable returning bytes (the next packet)
        interval: seconds between packets
        sport_name: display name for logging
    """
    clients = []
    lock = threading.Lock()

    def accept_loop(server_sock):
        while True:
            try:
                conn, addr = server_sock.accept()
                conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                with lock:
                    clients.append(conn)
                print(f"  [{sport_name}] Client connected: {addr}")
            except OSError:
                break

    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("0.0.0.0", port))
    server.listen(5)
    print(f"[{sport_name}] Listening on port {port}")

    accept_thread = threading.Thread(target=accept_loop, args=(server,), daemon=True)
    accept_thread.start()

    try:
        while True:
            packet = packet_builder_fn()
            with lock:
                dead = []
                for c in clients:
                    try:
                        c.sendall(packet)
                    except (BrokenPipeError, ConnectionResetError, OSError):
                        dead.append(c)
                for c in dead:
                    clients.remove(c)
                    try:
                        c.close()
                    except OSError:
                        pass
                    print(f"  [{sport_name}] Client disconnected")
            time.sleep(interval)
    except KeyboardInterrupt:
        print(f"\n[{sport_name}] Shutting down")
    finally:
        with lock:
            for c in clients:
                try:
                    c.close()
                except OSError:
                    pass
        server.close()


def make_arg_parser(sport_name, default_port):
    """Create a standard argparse parser for a simulator script."""
    parser = argparse.ArgumentParser(description=f"OES {sport_name} Simulator")
    parser.add_argument("--port", type=int, default=default_port,
                        help=f"TCP port (default: {default_port})")
    return parser
