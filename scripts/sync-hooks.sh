#!/bin/bash
# Sync hook files from relay-client-web to relay-runtime and hook-transpiler.
# Source of truth: relay-client-web/public/hooks/
# Run after editing hooks in relay-client-web: ./scripts/sync-hooks.sh

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

SOURCE="$REPO_ROOT/relay-client-web/public/hooks"
DEST_RUNTIME="$REPO_ROOT/relay-runtime/android-demo/src/main/assets/hooks"
DEST_TRANSPILER_WEB="$REPO_ROOT/hook-transpiler/tests/web/public/hooks"
DEST_TRANSPILER_ANDROID="$REPO_ROOT/hook-transpiler/tests/android/app/src/main/assets/hooks"

if [[ ! -d "$SOURCE" ]]; then
  echo "Source not found: $SOURCE"
  exit 1
fi

mkdir -p "$DEST_RUNTIME" "$DEST_TRANSPILER_WEB" "$DEST_TRANSPILER_ANDROID"

# errorBoundary.jsx removed: relay-runtime has built-in error boundary; do not sync
for f in "$SOURCE"/*.jsx; do
  [[ -f "$f" ]] || continue
  name=$(basename "$f")
  [[ "$name" == "errorBoundary.jsx" ]] && continue
  cp "$f" "$DEST_RUNTIME/$name"
  cp "$f" "$DEST_TRANSPILER_WEB/$name"
  cp "$f" "$DEST_TRANSPILER_ANDROID/$name"
  echo "  $name"
done

echo "Synced hooks from relay-client-web to relay-runtime, hook-transpiler (web + android)"
