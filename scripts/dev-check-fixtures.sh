#!/bin/bash
# Import every own fixture through the engine's library validation and load each once in the
# real CEF renderer. Usage: scripts/dev-check-fixtures.sh [graphic-dir...]  (logs under $TMPDIR/np-fixtures)
set -u
cd "$(dirname "$0")/.."
D=${TMPDIR:-/tmp}/np-fixtures
BIN=${NOPLAYOUT_BIN:-target/release}
rm -rf "$D" && mkdir -p "$D"
export NOPLAYOUT_DATA_DIR="$D/data" NOPLAYOUT_UI_TOKEN=fixtures NOPLAYOUT_HOST_DIR="${NOPLAYOUT_HOST_DIR:-$PWD/web/dist/host}"
E=$BIN/noplayout-engine
PORT=${PORT:-18689}
$E --port "$PORT" >"$D/engine.log" 2>&1 &
PID=$!
for _ in $(seq 1 100); do grep -q "program page configured" "$D/engine.log" && break; sleep 0.2; done
ctl() { $E ctl --port "$PORT" "$@"; }
fail=0
layer=10
for g in ${@:-fixtures/frame-oracle fixtures/recovery-probe/probe-static fixtures/recovery-probe/probe-timed}; do
  M=$(ctl import "$g" | sed -n 's/.*"manifestUrl":"\([^"]*\)".*/\1/p')
  if [ -z "$M" ]; then echo "FAIL import $g"; fail=1; continue; fi
  R=$(ctl load "$layer" "$M")
  case "$R" in *'"statusCode":200'*) echo "ok   $g" ;; *) echo "FAIL load $g: $R"; fail=1 ;; esac
  layer=$((layer + 10))
done
curl -s -X POST -H "x-noplayout-token: fixtures" "http://127.0.0.1:$PORT/api/v1/internal/shutdown" >/dev/null
wait $PID
if grep -q "console error" "$D/engine.log"; then grep "console error" "$D/engine.log" | head -5; fail=1; fi
exit $fail
