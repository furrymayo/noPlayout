#!/bin/bash
# noPlayout M1 timing kit: spikes S1a (paint-level pacing, modes A and B, 59.94 and 50) and S2
# (paint-path cost, process-tree memory at 1 and 2 channels with the Basketball on-air set).
#
# Self-contained: run it from the unpacked kit (`cargo xtask kit` builds one) on a Mac with no
# Rust toolchain. Double-click it in Finder or `open -a Terminal m1-timing.command`, so it runs
# in the logged-in GUI session; a gateway or SSH shell coalesces timers and its numbers are
# not evidence (the timer probe below records which it was). Writes one results JSON and one
# log in results/<host>-<UTC time>/ next to this script, then prints DONE.
#
# Knobs (environment): FRAMES (per S1a run; default 10 minutes at each rate, PLAN §5.6), MEM_SECONDS
# (memory sample window per channel count, default 30), SKIP_MEMORY=1, SKIP_S1A=1.
# Run it between shows only: with the defaults it loads the machine for about 45 minutes.
set -u
KIT="$(cd "$(dirname "$0")" && pwd)"
cd "$KIT"
BIN="$KIT/bin"
export NOPLAYOUT_BIN="$BIN" NOPLAYOUT_HOST_DIR="$BIN/host"
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
HOST=$(scutil --get LocalHostName 2>/dev/null || hostname -s)
OUT="$KIT/results/$HOST-$STAMP"
mkdir -p "$OUT"
LOG="$OUT/log.txt"
exec > >(tee -a "$LOG") 2>&1
FRAMES=${FRAMES:-}
MEM_SECONDS=${MEM_SECONDS:-30}
export TMPDIR="$OUT/tmp"
mkdir -p "$TMPDIR"
echo "noPlayout M1 timing kit: $OUT"

json_str() { printf '"%s"' "$(printf '%s' "$1" | tr -d '"\\' | tr '\n' ' ')"; }

# --- Machine and session -----------------------------------------------------------------
SESSION=$(launchctl managername 2>/dev/null || echo unknown)
MACHINE=$(printf '{"model":%s,"cpu":%s,"memBytes":%s,"os":%s,"build":%s,"session":%s,"kit":%s}' \
  "$(json_str "$(sysctl -n hw.model)")" \
  "$(json_str "$(sysctl -n machdep.cpu.brand_string)")" \
  "$(sysctl -n hw.memsize)" \
  "$(json_str "$(sw_vers -productVersion)")" \
  "$(json_str "$(sw_vers -buildVersion)")" \
  "$(json_str "$SESSION")" \
  "$(json_str "$(cat "$KIT/VERSION" 2>/dev/null || echo unknown)")")
echo "machine: $MACHINE"
TIMER=$("$BIN/noplayout-output" --bench-timer)
echo "timer: $TIMER"

# --- S2: paint-path cost -------------------------------------------------------------------
S2=$("$BIN/noplayout-output" --bench-s2)
echo "s2: $S2"

# --- S1a: pacing through the real renderer and the oracle ----------------------------------
S1A="[]"
if [ "${SKIP_S1A:-0}" != 1 ]; then
  rows=()
  for rate in 60000/1001 50; do
    for mode in A B; do
      n=${FRAMES:-$([ "$rate" = 50 ] && echo 30000 || echo 35964)}
      echo "s1a: mode $mode rate $rate, $n frames"
      export NOPLAYOUT_RATE=$rate
      R=$(PORT=18690 bash "$KIT/scripts/dev-oracle-run.sh" "$mode" "$n" | tail -1)
      unset NOPLAYOUT_RATE
      case "$R" in "{"*) ;; *) R='{"error":"no report"}' ;; esac
      rows+=("{\"mode\":\"$mode\",\"rate\":\"$rate\",\"frames\":$n,\"report\":$R}")
      mv "$TMPDIR/np-oracle-$mode" "$OUT/s1a-$mode-${rate%%/*}" 2>/dev/null
      rm -rf "$OUT/s1a-$mode-${rate%%/*}/dump"
    done
  done
  S1A="[$(IFS=,; echo "${rows[*]}")]"
fi

# --- S2 memory: process tree RSS, 1 and 2 channels, Basketball on-air set ------------------
# Sum of RSS over the engine and every descendant (output, renderers, CEF helpers), in KiB.
tree_rss() {
  ps -A -o pid=,ppid=,rss= | awk -v roots="$*" '
    { pid[NR]=$1; ppid[NR]=$2; rss[NR]=$3 }
    END {
      n = split(roots, r, " "); for (i = 1; i <= n; i++) keep[r[i]] = 1
      do { added = 0
        for (i = 1; i <= NR; i++) if (!(pid[i] in keep) && (ppid[i] in keep)) { keep[pid[i]] = 1; added = 1 }
      } while (added)
      for (i = 1; i <= NR; i++) if (pid[i] in keep) total += rss[i]
      print total + 0 }'
}

start_channel() { # <index>; sets STARTED_PID (runs in this shell so `wait` can reap it)
  local i=$1 port=$((18700 + $1)) d="$TMPDIR/mem-$1"
  mkdir -p "$d"
  NOPLAYOUT_DATA_DIR="$d/data" NOPLAYOUT_UI_TOKEN="mem$i" "$BIN/noplayout-engine" --port "$port" >"$d/engine.log" 2>&1 &
  STARTED_PID=$!
}

load_on_air_set() { # <index>
  local port=$((18700 + $1)) d="$TMPDIR/mem-$1" layer=10
  for _ in $(seq 1 100); do grep -q "PageLoaded" "$d/engine.log" && break; sleep 0.2; done
  sleep 2
  for g in bball-scorebug bball-ticker bball-sponsor-bug; do
    local m
    m=$(NOPLAYOUT_UI_TOKEN="mem$1" "$BIN/noplayout-engine" ctl --port "$port" import "$KIT/fixtures/basketball-ograf-package/$g" |
      sed -n 's/.*"manifestUrl":"\([^"]*\)".*/\1/p')
    r=$(NOPLAYOUT_UI_TOKEN="mem$1" "$BIN/noplayout-engine" ctl --port "$port" load $layer "$m")
    case "$r" in *'"statusCode":200'*) ;; *) echo "memory: load $g on channel $1 FAILED: $r" ;; esac
    NOPLAYOUT_UI_TOKEN="mem$1" "$BIN/noplayout-engine" ctl --port "$port" play $layer >/dev/null
    layer=$((layer + 10))
  done
}

stop_channel() { # <index> <pid>
  curl -s -X POST -H "x-noplayout-token: mem$1" "http://127.0.0.1:$((18700 + $1))/api/v1/internal/shutdown" >/dev/null
  wait "$2" 2>/dev/null
}

pressure() { sysctl -n kern.memorystatus_vm_pressure_level 2>/dev/null || echo 0; }
swap_used() { sysctl -n vm.swapusage 2>/dev/null | sed -n 's/.*used = \([0-9.]*\)M.*/\1/p'; }

MEM="[]"
if [ "${SKIP_MEMORY:-0}" != 1 ]; then
  rows=()
  for channels in 1 2; do
    echo "memory: $channels channel(s), Basketball on-air set, ${MEM_SECONDS}s"
    pids=()
    for i in $(seq 1 "$channels"); do start_channel "$i"; pids+=("$STARTED_PID"); done
    for i in $(seq 1 "$channels"); do load_on_air_set "$i"; done
    swap0=$(swap_used)
    max=0 sum=0 n=0 pmax=0
    for _ in $(seq 1 "$MEM_SECONDS"); do
      sleep 1
      kb=$(tree_rss "${pids[@]}")
      [ "$kb" -gt "$max" ] && max=$kb
      sum=$((sum + kb)) n=$((n + 1))
      p=$(pressure); [ "$p" -gt "$pmax" ] && pmax=$p
    done
    swap1=$(swap_used)
    procs=$(ps -A -o pid=,ppid= | awk -v roots="${pids[*]}" '
      { pid[NR]=$1; ppid[NR]=$2 } END { n=split(roots,r," "); for(i=1;i<=n;i++) k[r[i]]=1
      do { a=0; for(i=1;i<=NR;i++) if(!(pid[i] in k) && (ppid[i] in k)) { k[pid[i]]=1; a=1 } } while(a)
      c=0; for (x in k) c++; print c }')
    for i in $(seq 1 "$channels"); do stop_channel "$i" "${pids[$((i - 1))]}"; done
    rows+=("{\"channels\":$channels,\"processes\":$procs,\"rssMaxMiB\":$((max / 1024)),\"rssMeanMiB\":$((sum / (n > 0 ? n : 1) / 1024)),\"swapUsedMiBBefore\":${swap0:-0},\"swapUsedMiBAfter\":${swap1:-0},\"pressureLevelMax\":$pmax}")
    echo "memory: ${rows[${#rows[@]}-1]}"
  done
  MEM="[$(IFS=,; echo "${rows[*]}")]"
fi

# The memory rows run one engine per channel (M1 has one channel per engine), so a 2-channel
# row counts the engine and UI twice: an upper bound. Preview browsers arrive with the UI (M3)
# and are not in these numbers.
printf '{"kit":"noplayout-m1-timing","stamp":"%s","machine":%s,"timer":%s,"s2":%s,"s1a":%s,"memory":%s,"notes":"memory rows: one engine per channel (upper bound); no preview browsers in M1"}\n' \
  "$STAMP" "$MACHINE" "$TIMER" "$S2" "$S1A" "$MEM" >"$OUT/results.json"
[ "${KEEP_TMP:-0}" = 1 ] || rm -rf "$TMPDIR"
echo "results: $OUT/results.json"
echo DONE
