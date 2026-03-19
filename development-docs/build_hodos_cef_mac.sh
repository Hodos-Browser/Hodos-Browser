#!/bin/bash
# ============================================
# CEF Build Script for Hodos Browser (macOS)
# Builds CEF 136 with proprietary codecs
# (H.264, AAC, MP3) + Widevine DRM
#
# Usage: ./build_hodos_cef_mac.sh
# Resumable: if interrupted, just re-run it
# ============================================
set -eo pipefail

# --- Configuration ---
CEF_BRANCH="7103"               # CEF 136 / Chromium 136
BASE_DIR="$HOME/cef"
DOWNLOAD_DIR="$BASE_DIR/chromium_git"
DEPOT_TOOLS_DIR="$BASE_DIR/depot_tools"
AUTOMATE_DIR="$BASE_DIR/automate"
LOG_FILE="$BASE_DIR/build.log"

# --- Colors ---
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# --- Auto-detect architecture ---
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    BUILD_FLAG="--arm64-build"
    ARCH_LABEL="ARM64 (Apple Silicon)"
elif [ "$ARCH" = "x86_64" ]; then
    BUILD_FLAG="--x64-build"
    ARCH_LABEL="x64 (Intel)"
else
    error "Unknown architecture: $ARCH"
fi

echo ""
echo "============================================"
echo "  CEF Build for Hodos Browser (macOS)"
echo "  Branch: $CEF_BRANCH (CEF 136)"
echo "  Architecture: $ARCH_LABEL"
echo "============================================"
echo ""

# --- Prerequisite Checks ---
info "Checking prerequisites..."

# Xcode CLI tools
xcode-select -p &>/dev/null || error "Xcode CLI tools not installed. Run: xcode-select --install"
info "  Xcode CLI tools: $(xcode-select -p)"

# Python 3.9-3.11 (required by Chromium build tooling)
PYTHON_CMD=""
if command -v python3.11 &>/dev/null; then
    PYTHON_CMD="python3.11"
elif [ -f "$(brew --prefix python@3.11 2>/dev/null)/bin/python3.11" ]; then
    PYTHON_CMD="$(brew --prefix python@3.11)/bin/python3.11"
elif command -v python3 &>/dev/null; then
    PY_MINOR=$(python3 -c 'import sys; print(sys.version_info.minor)')
    if [ "$PY_MINOR" -ge 9 ] && [ "$PY_MINOR" -le 11 ]; then
        PYTHON_CMD="python3"
    fi
fi
[ -z "$PYTHON_CMD" ] && error "Python 3.9-3.11 required. Install with: brew install python@3.11"
PYTHON_VERSION=$($PYTHON_CMD --version 2>&1)
info "  Python: $PYTHON_VERSION ($PYTHON_CMD)"

# RAM
RAM_GB=$(( $(sysctl -n hw.memsize) / 1073741824 ))
[ "$RAM_GB" -lt 16 ] && error "Need 16+ GB RAM, found ${RAM_GB} GB"
info "  RAM: ${RAM_GB} GB"

# Disk space
FREE_GB=$(( $(df -k "$HOME" | tail -1 | awk '{print $4}') / 1048576 ))
[ "$FREE_GB" -lt 80 ] && error "Need 80+ GB free disk, found ${FREE_GB} GB"
[ "$FREE_GB" -lt 100 ] && warn "  Disk: ${FREE_GB} GB free (tight — 100 GB recommended)"
[ "$FREE_GB" -ge 100 ] && info "  Disk: ${FREE_GB} GB free"

# cmake
command -v cmake &>/dev/null || error "cmake not found. Install with: brew install cmake"
info "  cmake: $(cmake --version | head -1)"

# ninja (optional — depot_tools provides it, but good to check)
command -v ninja &>/dev/null && info "  ninja: $(ninja --version)" || info "  ninja: will use depot_tools version"

echo ""
info "All prerequisites passed!"
echo ""

# --- Confirmation ---
echo "This will:"
echo "  1. Download depot_tools (~500 MB)"
echo "  2. Download Chromium source (~30 GB)"
echo "  3. Build CEF with proprietary codecs (4-6 hours)"
echo ""
echo "  Base directory: $BASE_DIR"
echo "  Log file:       $LOG_FILE"
echo "  Total disk:     ~60-80 GB"
echo ""
echo "  TIP: The build uses caffeinate to prevent sleep."
echo "  TIP: If interrupted, just re-run this script — ninja resumes."
echo ""
read -p "Continue? [y/N] " -n 1 -r
echo ""
[[ ! $REPLY =~ ^[Yy]$ ]] && { echo "Aborted."; exit 0; }

# --- Setup directories ---
mkdir -p "$BASE_DIR" "$DOWNLOAD_DIR" "$AUTOMATE_DIR"

# --- Install/update depot_tools ---
if [ -d "$DEPOT_TOOLS_DIR/.git" ]; then
    info "Updating depot_tools..."
    git -C "$DEPOT_TOOLS_DIR" pull --quiet
else
    info "Cloning depot_tools..."
    rm -rf "$DEPOT_TOOLS_DIR"
    git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git "$DEPOT_TOOLS_DIR"
fi

# --- Set PATH: Python 3.11 first, then depot_tools ---
PYTHON_BIN_DIR=$(dirname "$($PYTHON_CMD -c 'import sys; print(sys.executable)')")
export PATH="$PYTHON_BIN_DIR:$DEPOT_TOOLS_DIR:$PATH"
info "PATH updated: Python at $PYTHON_BIN_DIR, depot_tools at $DEPOT_TOOLS_DIR"

# --- Download automate-git.py ---
AUTOMATE_SCRIPT="$AUTOMATE_DIR/automate-git.py"
info "Downloading automate-git.py..."
curl -sL -o "$AUTOMATE_SCRIPT" \
    "https://raw.githubusercontent.com/chromiumembedded/cef/master/tools/automate/automate-git.py"

# --- Set build environment ---
export GN_DEFINES="is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome enable_widevine=true"
export CEF_ARCHIVE_FORMAT="tar.bz2"

info "GN_DEFINES=$GN_DEFINES"
echo ""
echo "============================================"
echo "  BUILD STARTED: $(date)"
echo "============================================"
echo ""

# --- Run the build (with caffeinate to prevent sleep) ---
caffeinate -dims $PYTHON_CMD "$AUTOMATE_SCRIPT" \
    --download-dir="$DOWNLOAD_DIR" \
    --depot-tools-dir="$DEPOT_TOOLS_DIR" \
    --branch="$CEF_BRANCH" \
    $BUILD_FLAG \
    --minimal-distrib \
    --client-distrib \
    --no-debug-build \
    2>&1 | tee "$LOG_FILE"

echo ""
echo "============================================"
echo "  BUILD SUCCEEDED: $(date)"
echo "============================================"
echo ""
info "Output directory:"
ls -d "$DOWNLOAD_DIR/chromium/src/cef/binary_distrib/cef_binary_"* 2>/dev/null || warn "Output not found — check $LOG_FILE"
echo ""
info "Next steps:"
echo "  cd ~/Hodos-Browser"
echo "  mv cef-binaries cef-binaries-backup"
echo "  cp -r ~/cef/chromium_git/chromium/src/cef/binary_distrib/cef_binary_136.*_macosarm64_minimal cef-binaries"
echo "  # Then rebuild wrapper + cef-native (see issue #37)"
