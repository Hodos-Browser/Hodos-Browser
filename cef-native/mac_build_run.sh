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
    cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0
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

# Ad-hoc sign with entitlements (required for macOS TCC: camera, mic, etc.)
echo "Code signing with entitlements..."
ENTITLEMENTS="$SCRIPT_DIR/mac/entitlements.plist"
for helper in \
    "HodosBrowser Helper.app" \
    "HodosBrowser Helper (Alerts).app" \
    "HodosBrowser Helper (GPU).app" \
    "HodosBrowser Helper (Plugin).app" \
    "HodosBrowser Helper (Renderer).app"; do
    codesign --force --sign - --entitlements "$ENTITLEMENTS" \
        "HodosBrowser.app/Contents/Frameworks/$helper"
done
codesign --force --sign - \
    "HodosBrowser.app/Contents/Frameworks/Chromium Embedded Framework.framework"
codesign --force --sign - --entitlements "$ENTITLEMENTS" \
    "HodosBrowser.app"

# Kill any existing DEV instance ONLY. Match by the dev BUNDLE PATH, never by bare
# process name: dev and installed-prod both run a process named "HodosBrowser"
# (CMakeLists OUTPUT_NAME), so `pkill -9 HodosBrowser` force-killed the running
# INSTALLED production app too (dev/prod deconfliction audit 2026-07-14, gap C2).
# The dev bundle lives under $SCRIPT_DIR/build/bin; the installed app lives under
# /Applications, so a path-scoped match hits only the dev bundle (main + helpers).
# NOTE (Mac Claude): VERIFY AT RUNTIME that this matches the dev processes and NOT
# the installed app — pkill -f matches against the full argv, which is why the launch
# below uses the ABSOLUTE bundle path so argv[0] contains "build/bin/HodosBrowser.app".
DEV_BUNDLE="$SCRIPT_DIR/build/bin/HodosBrowser.app"
pkill -9 -f "$DEV_BUNDLE" 2>/dev/null || true

# Launch in dev mode (separate data directory from installed app)
export HODOS_DEV=1
# Enable in-process GPU for ad-hoc signed dev builds (GPU Helper subprocess
# requires proper code signing that only the release build has)
export HODOS_MAC_DEV_FLAGS=1
echo "Launching HodosBrowser (DEV MODE)..."
# Absolute path (not ./...) so the process argv is path-scoped and the pkill above
# can distinguish this dev bundle from the installed /Applications app.
#
# --profile= is passed explicitly so the startup picker never appears. Two reasons,
# and the second is the load-bearing one:
#   1. an unattended build+run otherwise blocks forever on the picker;
#   2. CDP only binds for the profile literally named "Default" --
#      cef_browser_shell_mac.mm :: main does
#          settings.remote_debugging_port = (profileId == "Default") ? 9222 : 0;
#      then +100 under HODOS_DEV, giving 9322. Any other profile (and the picker,
#      which has not resolved one yet) leaves the port at 0, so CDP never binds and
#      every harness phase reads as "the browser failed to start" rather than "you
#      have a dialog open".
# Override with HODOS_DEV_PROFILE=<id>, but note that a non-Default profile has no
# CDP port and therefore cannot be driven by the farbling harnesses.
"$DEV_BUNDLE/Contents/MacOS/HodosBrowser" --profile="${HODOS_DEV_PROFILE:-Default}"
