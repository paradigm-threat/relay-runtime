#!/usr/bin/env bash
# Build relay-runtime for the web bundle consumed by relay-client-web.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${RELAY_RUNTIME_WASM_OUT:-${ROOT}/../relay-client-web/src/wasm/relay_runtime_pkg}"
command -v wasm-pack >/dev/null || { echo "Install wasm-pack: https://rustwasm.github.io/wasm-pack/installer/"; exit 1; }
mkdir -p "$(dirname "$OUT")"
cd "$ROOT"
wasm-pack build --target web --out-dir "$OUT" --features wasm
echo "WASM package -> $OUT"
