#!/bin/bash
# Build and run HodosBrowser on Windows (Git Bash / MSYS2)
# Usage: ./build_and_run_win.sh [--clean]

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# Clean build if --clean flag passed
if [ "$1" = "--clean" ]; then
    echo "Cleaning build directory..."
    rm -rf build
fi

# Configure if needed
if [ ! -f build/HodosBrowser.sln ] && [ ! -f build/build.ninja ]; then
    echo "Configuring CMake..."
    cmake -S . -B build -G "Visual Studio 17 2022" -A x64
fi

# Build
echo "Building..."
cmake --build build --config Release

# Kill any existing instance
taskkill //F //IM HodosBrowser.exe 2>/dev/null || true

# Launch
echo "Launching HodosBrowser..."
cd build/bin/Release
./HodosBrowser.exe
