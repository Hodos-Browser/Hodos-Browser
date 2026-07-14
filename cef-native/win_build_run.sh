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

# Kill any existing DEV instance ONLY. Match by exe path under THIS build dir, never
# by bare image name: dev and installed-prod both ship the image name HodosBrowser.exe
# (CMakeLists OUTPUT_NAME), so `taskkill //IM HodosBrowser.exe` force-killed the running
# INSTALLED production browser too (dev/prod deconfliction audit 2026-07-14, gap C2).
# Fail-safe: if the path can't be resolved we kill nothing rather than risk killing prod.
DEV_EXE_DIR="$(cygpath -w "$SCRIPT_DIR/build/bin/Release" 2>/dev/null || echo "")"
if [ -n "$DEV_EXE_DIR" ]; then
    HODOS_DEV_EXE_DIR="$DEV_EXE_DIR" powershell.exe -NoProfile -Command '
        $dir = $env:HODOS_DEV_EXE_DIR
        Get-CimInstance Win32_Process |
            Where-Object { $_.Name -eq "HodosBrowser.exe" -and $_.ExecutablePath -and $_.ExecutablePath.StartsWith($dir, [System.StringComparison]::OrdinalIgnoreCase) } |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    ' 2>/dev/null || true
fi

# Launch in dev mode (separate data directory from installed app)
export HODOS_DEV=1
echo "Launching HodosBrowser (DEV MODE)..."
cd build/bin/Release
./HodosBrowser.exe
