#!/bin/bash
# Frame-oracle run on the M1 dump path: engine + output + CEF renderer play fixtures/frame-oracle,
# the output process dumps N slots (patch crops only), then noplayout-oracle judges the dump.
# Usage: scripts/dev-oracle-run.sh [A|B] [frames]   (logs and dump under $TMPDIR/np-oracle-<mode>)
# Timing numbers from a gateway/SSH shell on a Mac with timer coalescing are not evidence; the
# S1a harness runs this from a Terminal .command (scripts/s1a.command).
set -u
cd "$(dirname "$0")/.."
MODE=${1:-A}
FRAMES=${2:-1200}
D=${TMPDIR:-/tmp}/np-oracle-$MODE
BIN=${NOPLAYOUT_BIN:-target/release}
rm -rf "$D" && mkdir -p "$D"
export NOPLAYOUT_DATA_DIR="$D/data" NOPLAYOUT_UI_TOKEN=oracle NOPLAYOUT_HOST_DIR="${NOPLAYOUT_HOST_DIR:-$PWD/web/dist/host}"
export NOPLAYOUT_DRIVE_MODE="$MODE"
# The output process's 1 Hz counters (slots_skipped, held, submit_blocked, ...) go to engine.log.
export RUST_LOG="${RUST_LOG:-info,noplayout_engine=debug}"
# The dump starts with the output clock; the oracle drops the lead-in before the first readable
# frame (the page loading), so dump 10 s more than asked.
export NOPLAYOUT_DUMP_PAINTS="$D/dump" NOPLAYOUT_DUMP_FRAMES=$((FRAMES + 600))
NOPLAYOUT_DUMP_CROP=$("$BIN/noplayout-oracle" --print-crops 1920x1080) || exit 2
export NOPLAYOUT_DUMP_CROP
E=$BIN/noplayout-engine
PORT=${PORT:-18688}
$E --port "$PORT" >"$D/engine.log" 2>&1 &
PID=$!
for _ in $(seq 1 100); do grep -q "program page configured" "$D/engine.log" && break; sleep 0.2; done
ctl() { $E ctl --port "$PORT" "$@"; }
M=$(ctl import fixtures/frame-oracle | sed -n 's/.*"manifestUrl":"\([^"]*\)".*/\1/p')
echo "load: $(ctl load 10 "$M")"
echo "play: $(ctl play 10)"
for _ in $(seq 1 600); do grep -q "DumpComplete" "$D/engine.log" && break; sleep 0.5; done
curl -s -X POST -H "x-noplayout-token: oracle" "http://127.0.0.1:$PORT/api/v1/internal/shutdown" >/dev/null
wait $PID
echo "engine exit $?"
"$BIN/noplayout-oracle" "$D/dump" --mode "$MODE" --rate "${NOPLAYOUT_RATE:-60000/1001}" | tee "$D/report.json"
