#!/bin/bash
# Build relay-runtime android-demo APK and install on connected emulator/device.
#
# Run after each change: ./scripts/build-install-demo.sh
# Requires: adb, device/emulator connected

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$SCRIPT_DIR/.."
cd "$REPO"

echo "Building native lib (arm64 + x86_64)..."
bash scripts/build-android.sh 2>/dev/null || true

echo "Building APK..."
./gradlew :android-demo:assembleDebug -q

echo "Installing on device..."
adb install -r android-demo/build/outputs/apk/debug/android-demo-debug.apk

echo "Launching..."
adb shell am start -n com.clevertree.relayruntime.demo/.MainActivity

echo "Done. Run: adb logcat -s RelayDemo:D"
