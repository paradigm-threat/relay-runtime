#!/bin/bash
# Build relay-runtime for Android (all ABIs) and copy to android/jniLibs
# Usage: ./scripts/build-android.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
JNI_LIBS="$REPO_ROOT/android/jniLibs"

cd "$REPO_ROOT"

if [ ! -f "Cargo.toml" ]; then
  echo "❌ Error: Cargo.toml not found"
  exit 1
fi

echo "🔨 Building relay-runtime for Android..."
echo ""

# Build aarch64 (arm64-v8a) and x86_64 (emulator)
echo "  Building aarch64-linux-android (arm64-v8a)..."
cargo build --target aarch64-linux-android --lib --release
echo "  Building x86_64-linux-android (emulator)..."
cargo build --target x86_64-linux-android --lib --release 2>/dev/null || echo "    (skipped - needs .cargo/config.toml linker)"

echo ""
echo "📦 Copying libs to android/jniLibs..."

mkdir -p "$JNI_LIBS"/arm64-v8a "$JNI_LIBS"/armeabi-v7a "$JNI_LIBS"/x86 "$JNI_LIBS"/x86_64

cp -f "$REPO_ROOT/target/aarch64-linux-android/release/librelay_runtime.so" "$JNI_LIBS/arm64-v8a/" || true
[ -f "$REPO_ROOT/target/armv7-linux-androideabi/release/librelay_runtime.so" ] && cp -f "$REPO_ROOT/target/armv7-linux-androideabi/release/librelay_runtime.so" "$JNI_LIBS/armeabi-v7a/" || true
[ -f "$REPO_ROOT/target/i686-linux-android/release/librelay_runtime.so" ] && cp -f "$REPO_ROOT/target/i686-linux-android/release/librelay_runtime.so" "$JNI_LIBS/x86/" || true
[ -f "$REPO_ROOT/target/x86_64-linux-android/release/librelay_runtime.so" ] && mkdir -p "$JNI_LIBS/x86_64" && cp -f "$REPO_ROOT/target/x86_64-linux-android/release/librelay_runtime.so" "$JNI_LIBS/x86_64/" || true

echo ""
echo "✅ Android build complete. Libraries in $JNI_LIBS"
