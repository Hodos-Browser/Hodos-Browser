#!/usr/bin/env bash
# ============================================
# CEF Build Script for Hodos Browser (macOS)
# Builds CEF 136 (branch 7103) with proprietary codecs (H.264, AAC, MP3)
#
# WHAT THIS DOES:
#   1. Checks prerequisites (Xcode CLI tools, Python, git, disk space)
#   2. Downloads depot_tools and automate-git.py if not present
#   3. Runs automate-git.py to download Chromium/CEF source and build
#   4. Outputs a CEF binary distribution with proprietary codecs enabled
#
# REQUIREMENTS:
#   - macOS 13+ (Ventura or later recommended)
#   - Xcode Command Line Tools (full Xcode NOT required)
#   - Python 3.9 - 3.11 (NOT 3.12+ due to compatibility issues)
#   - git (comes with Xcode CLI tools)
#   - ~100 GB free disk space (SSD strongly recommended)
#   - 16 GB RAM minimum, 32 GB recommended
#   - Build time: 4-6 hours first build, 30-60 min incremental
#
# USAGE:
#   chmod +x build_hodos_cef_mac.sh
#   ./build_hodos_cef_mac.sh
#
# OUTPUT:
#   ~/cef/chromium_git/chromium/src/cef/binary_distrib/
#   Look for: cef_binary_136.*_macos{arm64,x86_64}/
# ============================================

set -euo pipefail

# --------------------------------------------------
# Configuration
# --------------------------------------------------

CEF_BASE_DIR="$HOME/cef"
CEF_AUTOMATE_DIR="$CEF_BASE_DIR/automate"
CEF_DEPOT_TOOLS_DIR="$CEF_BASE_DIR/depot_tools"
CEF_CHROMIUM_DIR="$CEF_BASE_DIR/chromium_git"
CEF_BRANCH="7103"

# GN build defines for proprietary codecs
export GN_DEFINES="is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0"

# Archive format
export CEF_ARCHIVE_FORMAT="tar.bz2"

# --------------------------------------------------
# Helper functions
# --------------------------------------------------

log_info() {
    echo ""
    echo "=== $1 ==="
    echo ""
}

log_error() {
    echo ""
    echo "ERROR: $1" >&2
    echo ""
}

log_warn() {
    echo ""
    echo "WARNING: $1"
    echo ""
}

# --------------------------------------------------
# Step 0: Detect architecture
# --------------------------------------------------

log_info "Detecting architecture"

ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    echo "Detected Apple Silicon (ARM64) - M1/M2/M3/M4"
    BUILD_ARCH_FLAG="--arm64-build"
    ARCH_LABEL="arm64"
elif [ "$ARCH" = "x86_64" ]; then
    echo "Detected Intel x86_64"
    BUILD_ARCH_FLAG="--x64-build"
    ARCH_LABEL="x86_64"
else
    log_error "Unknown architecture: $ARCH. Expected arm64 or x86_64."
    exit 1
fi

# --------------------------------------------------
# Step 1: Check prerequisites
# --------------------------------------------------

log_info "Checking prerequisites"

# Check Xcode Command Line Tools
if ! xcode-select -p &>/dev/null; then
    log_error "Xcode Command Line Tools not installed."
    echo "Install with: xcode-select --install"
    echo "Then re-run this script."
    exit 1
fi
echo "[OK] Xcode Command Line Tools: $(xcode-select -p)"

# Check Python version (need 3.9 - 3.11)
if ! command -v python3 &>/dev/null; then
    log_error "Python 3 not found. Install Python 3.9-3.11."
    echo "  brew install python@3.11"
    exit 1
fi

PYTHON_VERSION=$(python3 --version 2>&1 | sed 's/Python //')
PYTHON_MAJOR=$(echo "$PYTHON_VERSION" | cut -d. -f1)
PYTHON_MINOR=$(echo "$PYTHON_VERSION" | cut -d. -f2)

if [ "$PYTHON_MAJOR" -ne 3 ] || [ "$PYTHON_MINOR" -lt 9 ] || [ "$PYTHON_MINOR" -gt 11 ]; then
    log_warn "Python $PYTHON_VERSION detected. Recommended: 3.9-3.11."
    echo "Python 3.12+ has known compatibility issues with Chromium builds."
    echo "Install 3.11 with: brew install python@3.11"
    echo "Then: export PATH=\"\$(brew --prefix python@3.11)/bin:\$PATH\""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
else
    echo "[OK] Python: $PYTHON_VERSION"
fi

# Check git
if ! command -v git &>/dev/null; then
    log_error "git not found. Should come with Xcode CLI tools."
    exit 1
fi
echo "[OK] git: $(git --version)"

# Check disk space (~100GB needed)
AVAILABLE_GB=$(df -g "$HOME" 2>/dev/null | tail -1 | awk '{print $4}' || echo "0")
# Fallback for systems where df -g doesn't work
if [ "$AVAILABLE_GB" = "0" ]; then
    AVAILABLE_GB=$(df -Pk "$HOME" | tail -1 | awk '{print int($4/1048576)}')
fi

if [ "$AVAILABLE_GB" -lt 100 ]; then
    log_warn "Only ${AVAILABLE_GB}GB free disk space. 100GB+ recommended."
    echo "The build may fail due to insufficient disk space."
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
else
    echo "[OK] Disk space: ${AVAILABLE_GB}GB available"
fi

# Check RAM
TOTAL_RAM_GB=$(sysctl -n hw.memsize 2>/dev/null | awk '{print int($1/1073741824)}' || echo "0")
if [ "$TOTAL_RAM_GB" -lt 16 ]; then
    log_warn "Only ${TOTAL_RAM_GB}GB RAM detected. 16GB minimum, 32GB recommended."
fi
echo "[OK] RAM: ${TOTAL_RAM_GB}GB"

echo ""
echo "All prerequisites satisfied."

# --------------------------------------------------
# Step 2: Create directory structure
# --------------------------------------------------

log_info "Creating directory structure at $CEF_BASE_DIR"

mkdir -p "$CEF_AUTOMATE_DIR"
mkdir -p "$CEF_DEPOT_TOOLS_DIR"
mkdir -p "$CEF_CHROMIUM_DIR"

echo "  $CEF_AUTOMATE_DIR"
echo "  $CEF_DEPOT_TOOLS_DIR"
echo "  $CEF_CHROMIUM_DIR"

# --------------------------------------------------
# Step 3: Download depot_tools (if not present)
# --------------------------------------------------

log_info "Setting up depot_tools"

if [ -d "$CEF_DEPOT_TOOLS_DIR/.git" ]; then
    echo "depot_tools already cloned. Updating..."
    cd "$CEF_DEPOT_TOOLS_DIR"
    git pull --quiet
else
    echo "Cloning depot_tools from chromium.googlesource.com..."
    # Remove directory contents if it exists but isn't a git repo
    rm -rf "${CEF_DEPOT_TOOLS_DIR:?}/"*
    git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git "$CEF_DEPOT_TOOLS_DIR"
fi

echo "[OK] depot_tools ready"

# Add depot_tools to PATH for this session
export PATH="$CEF_DEPOT_TOOLS_DIR:$PATH"

# --------------------------------------------------
# Step 4: Download automate-git.py (if not present)
# --------------------------------------------------

log_info "Setting up automate-git.py"

AUTOMATE_SCRIPT="$CEF_AUTOMATE_DIR/automate-git.py"

if [ -f "$AUTOMATE_SCRIPT" ]; then
    echo "automate-git.py already exists. Downloading fresh copy..."
fi

curl -fsSL \
    "https://raw.githubusercontent.com/chromiumembedded/cef/master/tools/automate/automate-git.py" \
    -o "$AUTOMATE_SCRIPT"

echo "[OK] automate-git.py downloaded"

# --------------------------------------------------
# Step 5: Print build configuration
# --------------------------------------------------

log_info "Build Configuration"

echo "  CEF Branch:     $CEF_BRANCH (CEF 136 / Chromium 136)"
echo "  Architecture:   $ARCH_LABEL ($BUILD_ARCH_FLAG)"
echo "  GN_DEFINES:     $GN_DEFINES"
echo "  Archive Format: $CEF_ARCHIVE_FORMAT"
echo "  Download Dir:   $CEF_CHROMIUM_DIR"
echo "  depot_tools:    $CEF_DEPOT_TOOLS_DIR"
echo ""
echo "This will take 4-6 hours for a first build."
echo "Chromium source download is ~30GB."
echo ""
read -p "Start the build? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 0
fi

# --------------------------------------------------
# Step 6: Run automate-git.py
# --------------------------------------------------

log_info "Starting CEF build (branch $CEF_BRANCH, $ARCH_LABEL)"

BUILD_START=$(date +%s)

python3 "$AUTOMATE_SCRIPT" \
    --download-dir="$CEF_CHROMIUM_DIR" \
    --depot-tools-dir="$CEF_DEPOT_TOOLS_DIR" \
    --branch="$CEF_BRANCH" \
    "$BUILD_ARCH_FLAG" \
    --minimal-distrib \
    --client-distrib \
    --no-debug-build \
    --force-build

BUILD_EXIT_CODE=$?
BUILD_END=$(date +%s)
BUILD_DURATION=$(( (BUILD_END - BUILD_START) / 60 ))

# --------------------------------------------------
# Step 7: Report results
# --------------------------------------------------

echo ""
echo "============================================"

if [ $BUILD_EXIT_CODE -ne 0 ]; then
    echo "BUILD FAILED (exit code $BUILD_EXIT_CODE)"
    echo "Build duration: ${BUILD_DURATION} minutes"
    echo ""
    echo "Common issues:"
    echo "  - Python version incompatibility (need 3.9-3.11)"
    echo "  - Insufficient disk space (need ~100GB)"
    echo "  - Insufficient RAM (need 16GB+)"
    echo "  - Network interruption during source download"
    echo ""
    echo "Try re-running the script. automate-git.py supports"
    echo "incremental builds and will resume where it left off."
    echo "============================================"
    exit $BUILD_EXIT_CODE
fi

echo "BUILD SUCCEEDED"
echo "Build duration: ${BUILD_DURATION} minutes"
echo ""
echo "Output directory:"
echo "  $CEF_CHROMIUM_DIR/chromium/src/cef/binary_distrib/"
echo ""
echo "Look for a folder named:"
echo "  cef_binary_136.*_macos${ARCH_LABEL}/"
echo ""
echo "Inside you will find:"
echo "  Release/                    - CEF framework and libraries"
echo "    Chromium Embedded Framework.framework/"
echo "  Resources/                  - CEF resources (pak files, locales)"
echo "  include/                    - CEF C/C++ headers"
echo "  libcef_dll_wrapper/         - Wrapper source to build"
echo ""
echo "Next steps:"
echo "  1. Copy the output to Hodos-Browser/cef-binaries/"
echo "  2. Rebuild libcef_dll_wrapper (cmake .. && make)"
echo "  3. Rebuild cef-native (cmake .. && make)"
echo "  4. Verify codecs: video.canPlayType('video/mp4; codecs=\"avc1.42E01E\"')"
echo "     Should return 'probably' (not empty string)"
echo "============================================"
