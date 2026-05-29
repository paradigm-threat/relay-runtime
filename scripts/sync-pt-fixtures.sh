#!/bin/bash
# Sync Paradigm Threat files hooks from the PT repo into relay-runtime test fixtures.
# Source: ../pt/paradigm-threat-files/hooks
# Dest:   tests/fixtures/paradigm-threat-files/hooks
#
# Run after editing hooks in paradigm-threat-files:
#   ./scripts/sync-pt-fixtures.sh

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SOURCE="$RUNTIME_ROOT/../pt/paradigm-threat-files/hooks"
DEST="$RUNTIME_ROOT/tests/fixtures/paradigm-threat-files/hooks"

if [[ ! -d "$SOURCE" ]]; then
  echo "Source not found: $SOURCE"
  echo "Expected sibling checkout: pt/paradigm-threat-files next to relay-runtime"
  exit 1
fi

mkdir -p "$(dirname "$DEST")"
rm -rf "$DEST"
mkdir -p "$DEST"
cp -a "$SOURCE/." "$DEST/"
echo "Synced PT hooks: $SOURCE -> $DEST"
