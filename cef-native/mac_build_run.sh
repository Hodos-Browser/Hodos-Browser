#!/bin/bash
# Build and run HodosBrowser on macOS
# Usage: ./build_and_run.sh [--clean]

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# Clean build if --clean flag passed
if [ "$1" = "--clean" ]; then
    echo "Cleaning build directory..."
    rm -rf build
fi

# Configure if needed
if [ ! -f build/Makefile ] && [ ! -f build/build.ninja ]; then
    echo "Configuring CMake..."
    cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
fi

# Build
echo "Building..."
cmake --build build --config Release

# Copy helpers into app bundle
echo "Copying helper bundles..."
cd build/bin
cp -r "HodosBrowser Helper.app" \
      "HodosBrowser Helper (Alerts).app" \
      "HodosBrowser Helper (GPU).app" \
      "HodosBrowser Helper (Plugin).app" \
      "HodosBrowser Helper (Renderer).app" \
      HodosBrowser.app/Contents/Frameworks/

# Kill any existing instance
pkill -9 HodosBrowser 2>/dev/null || true

# Launch in dev mode (separate data directory from installed app)
export HODOS_DEV=1
# Enable in-process GPU for ad-hoc signed dev builds (GPU Helper subprocess
# requires proper code signing that only the release build has)
export HODOS_MAC_DEV_FLAGS=1
echo "Launching HodosBrowser (DEV MODE)..."
./HodosBrowser.app/Contents/MacOS/HodosBrowser
