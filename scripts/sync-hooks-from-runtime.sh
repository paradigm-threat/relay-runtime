#!/bin/bash
# Sync hook files FROM relay-runtime (source of truth) to relay-client-web and wasm-test.
# Use when editing hooks in relay-runtime for CleverScript-compatible, cross-platform hooks.
#
# Run from relay-runtime: ./scripts/sync-hooks-from-runtime.sh

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SOURCE="$RUNTIME_ROOT/android-demo/src/main/assets/hooks"
DEST_WEB="$RUNTIME_ROOT/../relay-client-web/public/hooks"
DEST_WASM="$RUNTIME_ROOT/examples/wasm-test/hooks"

if [[ ! -d "$SOURCE" ]]; then
  echo "Source not found: $SOURCE"
  exit 1
fi

mkdir -p "$DEST_WEB" "$DEST_WASM"

for f in "$SOURCE"/*.jsx; do
  [[ -f "$f" ]] || continue
  name=$(basename "$f")
  cp "$f" "$DEST_WEB/$name"
  cp "$f" "$DEST_WASM/$name"
  echo "  $name"
done

echo "Synced hooks from relay-runtime to relay-client-web, wasm-test"
echo "Source: $SOURCE"
