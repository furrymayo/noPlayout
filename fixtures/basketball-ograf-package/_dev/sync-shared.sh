#!/bin/bash
# Copies the shared base library, fonts and placeholder assets into every graphic folder.
set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
for g in "$ROOT"/bball-*/; do
  mkdir -p "$g/lib/fonts" "$g/assets"
  cp "$ROOT/_dev/shared/ograf-base.mjs" "$g/lib/"
  cp "$ROOT/_dev/shared/fonts/"* "$g/lib/fonts/"
  cp "$ROOT/_dev/shared/assets/"* "$g/assets/"
  echo "synced $(basename "$g")"
done
