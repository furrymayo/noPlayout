#!/bin/bash
# Dev probe for the supervisor (M1 task 8): load+play a graphic, then faults; prints the
# supervisor view after each. Usage: scripts/dev-recovery-probe.sh <graphic-dir> [standby 0|1]
set -u
cd "$(dirname "$0")/.."
D=${TMPDIR:-/tmp}/np-sup
rm -rf "$D" && mkdir -p "$D"
export NOPLAYOUT_DATA_DIR="$D/data" NOPLAYOUT_UI_TOKEN=probe NOPLAYOUT_HOST_DIR="$PWD/web/dist/host"
export NOPLAYOUT_STANDBY="${2:-1}" RUST_LOG=info
E=target/release/noplayout-engine
$E --port 18696 >"$D/engine.log" 2>&1 &
P=$!
for _ in $(seq 1 100); do grep -q PageLoaded "$D/engine.log" && break; sleep 0.2; done
sleep 2
c() { $E ctl --port 18696 "$@"; }
sv() { echo "--- $1"; curl -s http://127.0.0.1:18696/api/v1/health | jq -c '.supervisor | del(.records) + {recs: [.records[]? | [.kind,.path,.failedInc,.newInc,(.switchedMs|floor?)]]}'; c inspect | jq -c '{active: .activeInc, barrier, pages: [.pages[] | [.role,.inc,.syncedSeq,.beatAgeMs]], inst: [.instances[] | [.layer,.phase,.currentStep]]}'; }
M=$(c import "$1" | sed -n 's/.*"manifestUrl":"\([^"]*\)".*/\1/p')
c load 10 "$M" >/dev/null; c play 10 >/dev/null
sleep 3; sv "steady"
c kill-renderer; sleep 4; sv "after kill-renderer (K1)"
sleep 6; sv "standby rebuilt?"
c hang 3000; sleep 4; sv "after hang 3000 (K3a)"
sleep 12; sv "after 12 s"
c kill-renderer; sleep 2; c kill-renderer; sleep 8; sv "double kill within 10 s (loop guard)"
c play 10 >/dev/null; sleep 1; sv "play after recovery"
curl -s -X POST -H "x-noplayout-token: probe" http://127.0.0.1:18696/api/v1/internal/shutdown >/dev/null
wait $P
echo "--- engine log (warn/error)"
grep -E "WARN|ERROR" "$D/engine.log" | grep -v "console error" | sed 's/^.*Z //' | cut -c1-170 | head -40
