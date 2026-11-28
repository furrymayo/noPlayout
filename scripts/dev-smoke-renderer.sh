#!/bin/bash
# Dev smoke for M1 task 4: engine + output + CEF renderer, a real program page, ctl load/play.
# Usage: scripts/dev-smoke-renderer.sh [A|B]   (logs under $TMPDIR/np-e2e)
set -u
cd "$(dirname "$0")/.."
MODE=${1:-A}
D=${TMPDIR:-/tmp}/np-e2e
rm -rf "$D" && mkdir -p "$D"
export NOPLAYOUT_DATA_DIR="$D/data" NOPLAYOUT_UI_TOKEN=smoke NOPLAYOUT_HOST_DIR="$PWD/web/dist/host"
export NOPLAYOUT_DRIVE_MODE="$MODE"
E=target/release/noplayout-engine
PORT=18687
$E --port $PORT >"$D/engine.log" 2>&1 &
PID=$!
for _ in $(seq 1 50); do grep -q "program page configured" "$D/engine.log" && break; sleep 0.2; done
sleep 2
ctl() { $E ctl --port $PORT "$@"; }
HASH=$(ctl import third_party/ograf/v1/examples/minimal | sed -n 's/.*"manifestUrl":"\([^"]*\)".*/\1/p')
echo "manifest: $HASH"
echo "load:    $(ctl load 10 "$HASH")"
echo "play:    $(ctl play 10)"
sleep 1
echo "inspect: $(ctl inspect)"
echo "hang:    $(ctl hang 300)"
sleep 1
echo "health:  $(curl -s http://127.0.0.1:$PORT/api/v1/health | tr -d "\n" | grep -o "\"paints\":[0-9]*")"
curl -s -X POST -H "x-noplayout-token: smoke" http://127.0.0.1:$PORT/api/v1/internal/shutdown >/dev/null
wait $PID
echo "engine exit $?"
grep -E "renderer event|console error|program page|hanging|ERROR|WARN" "$D/engine.log" | cut -c1-220 | head -20
