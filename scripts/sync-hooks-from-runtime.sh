#!/bin/bash
# Deprecated name. Tester SoT is wasm-test → android-demo via sync-hooks.sh
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sync-hooks.sh" "$@"
