#!/bin/bash
# Build relay-runtime WASM and start test server
# Usage: ./scripts/test-wasm.sh

set -e

# Get script directory (works with both bash and sh)
if [ -n "$BASH_SOURCE" ]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
else
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
fi
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WASM_TEST_DIR="$REPO_ROOT/examples/wasm-test"
PKG_DIR="$WASM_TEST_DIR/pkg"

# Verify we're in the right place
if [ ! -f "$REPO_ROOT/Cargo.toml" ]; then
  echo "❌ Error: Cargo.toml not found in $REPO_ROOT"
  echo "   Make sure you're running this from the relay-runtime repository root"
  exit 1
fi

# Theme styling is now built into relay-runtime (no separate themed-styler build)
echo ""
echo "🔨 Building relay-runtime WASM package..."
echo "   Working directory: $REPO_ROOT"
echo "   For debugging with breakpoints, use: wasm-pack build --target web --out-dir pkg -- --features wasm -g"
cd "$REPO_ROOT"
wasm-pack build --target web --out-dir "$PKG_DIR"

# Copy web wrapper (bootstrap, vnodeToDom) into pkg for zero-config integration
cp "$REPO_ROOT/js/relay-runtime-web.js" "$PKG_DIR/relay-runtime-web.js"
cp "$REPO_ROOT/js/relay-runtime-web.d.ts" "$PKG_DIR/relay-runtime-web.d.ts" 2>/dev/null || true

# Update package.json: main=relay-runtime-web.js, add exports, include wrapper in files
(cd "$PKG_DIR" && node -e "
const fs = require('fs');
const p = require('./package.json');
p.main = 'relay-runtime-web.js';
if (!p.files.includes('relay-runtime-web.js')) p.files.push('relay-runtime-web.js');
if (fs.existsSync('./relay-runtime-web.d.ts') && !p.files.includes('relay-runtime-web.d.ts')) p.files.push('relay-runtime-web.d.ts');
p.exports = { '.': './relay-runtime-web.js', './wasm': './relay_runtime.js' };
if (fs.existsSync('./relay-runtime-web.d.ts')) p.types = 'relay-runtime-web.d.ts';
fs.writeFileSync('./package.json', JSON.stringify(p, null, 2));
")

echo ""
echo "✅ Build complete. Starting test server..."
echo "📦 Package: $PKG_DIR"
echo "🌐 Server: http://localhost:8765/examples/wasm-test/"
echo ""

# Check if port is already in use
if lsof -ti:8765 > /dev/null 2>&1; then
  echo "⚠️  Port 8765 is already in use. Attempting to free it..."
  lsof -ti:8765 | xargs kill -9 2>/dev/null || true
  sleep 1
fi

echo "Press Ctrl+C to stop the server"
echo ""

cd "$REPO_ROOT"
python3 "$SCRIPT_DIR/serve.py" 8765
