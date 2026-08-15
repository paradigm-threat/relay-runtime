#!/bin/bash
# Tester harness: keep android-demo hooks identical to wasm-test.
# Source of truth: examples/wasm-test/hooks/
# Run from relay-runtime: ./scripts/sync-hooks.sh
#
# Do not copy from relay-client-web. Do not touch hook-transpiler.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SOURCE="$RUNTIME_ROOT/examples/wasm-test/hooks"
DEST="$RUNTIME_ROOT/android-demo/src/main/assets/hooks"

if [[ ! -d "$SOURCE" ]]; then
  echo "Source not found: $SOURCE" >&2
  exit 1
fi

mkdir -p "$DEST"
copied=0
for f in "$SOURCE"/*.jsx; do
  [[ -f "$f" ]] || continue
  cp "$f" "$DEST/$(basename "$f")"
  echo "  $(basename "$f")"
  copied=$((copied + 1))
done

if [[ -f "$RUNTIME_ROOT/examples/wasm-test/theme.yaml" ]]; then
  cp "$RUNTIME_ROOT/examples/wasm-test/theme.yaml" "$RUNTIME_ROOT/android-demo/src/main/assets/theme.yaml"
  echo "  theme.yaml"
fi

echo "Synced $copied hooks wasm-test → android-demo"
echo "Source: $SOURCE"
