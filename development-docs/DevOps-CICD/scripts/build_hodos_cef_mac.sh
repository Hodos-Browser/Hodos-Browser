#!/usr/bin/env bash
# ============================================
# CEF Build Script for Hodos Browser (macOS)
# Builds CEF 150 (branch 7871) with proprietary codecs (H.264, AAC, MP3, VP9, AV1)
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
#   - Python 3.9 - 3.11. The real ceiling is per-branch: 7871's .vpython3
#     pins 3.11 (same as 7103). depot_tools also ships its own Python.
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
#   ~/cef/cef150/chromium/src/cef/binary_distrib/
#   Look for: cef_binary_150.*_macos{arm64,x86_64}/
#
# ⛔ BEFORE THE 10-12 h BUILD: run the gn-args codec pre-flight gate. A flipped
#    or renamed codec default produces a GREEN build with NO codecs, and finding
#    that out afterwards is the expensive failure. See PLAN_codecs.md §7.
# ============================================

set -euo pipefail

# --------------------------------------------------
# Configuration
# --------------------------------------------------

# VER-1: branch 7871 = CEF 150 = Chromium 150 (the M150 LTS line).
# Pinned point-release 150.0.17+g94c1726+chromium-150.0.7871.187 -> CEF commit
# 94c1726, which pins Chromium refs/tags/150.0.7871.187 transitively via
# cef/CHROMIUM_BUILD_COMPATIBILITY.txt.
#
# Give 7871 its OWN tree and depot_tools; do not reuse an M136 tree.
# automate-git.py hard-checkouts depot_tools to the commit its branch pins, so a
# shared depot_tools ends up pinned to whichever branch ran last.
CEF_BASE_DIR="$HOME/cef"
CEF_AUTOMATE_DIR="$CEF_BASE_DIR/automate"
CEF_DEPOT_TOOLS_DIR="$CEF_BASE_DIR/cef150/depot_tools"
CEF_CHROMIUM_DIR="$CEF_BASE_DIR/cef150"
CEF_BRANCH="7871"

# Pin an exact FORK commit, not the moving hodos/7871 branch tip: a build must be
# reproducible, and patch content is part of the build. BUMP THIS every time a
# patch lands on hodos/7871, and record the new SHA in the fork's
# HODOS_PATCHES.md. Upstream content is unchanged -- f82b3aae0 is 94c1726
# (upstream 7871 head) plus our patch commits.
CEF_CHECKOUT="f82b3aae0"

# ⚠️ <tree>/chromium/src/cef is a COPY of the standalone checkout, refreshed ONLY
# when the CEF checkout HASH changes (automate-git.py:1358-1360). If you manually
# git-checkout or git-pull "$CEF_CHROMIUM_DIR/cef" to the target commit before
# running this, the hashes already match, the copy never refreshes, and the build
# silently uses the STALE in-tree patch set -- right fork, right pin, green run,
# ZERO Hodos patches compiled in. --force-cef-update is passed unconditionally at
# the invocation below precisely so this cannot happen silently.
# Measured on Windows 2026-08-05; the mechanism is platform-independent.
#
# ⛔ VERIFY BY PRESENCE, NOT BY TOTAL. The gate is "at least one hodos_*.patch is
# in the tree", which is invariant. Do NOT assert "N patches total" -- that number
# changes on every landing, so it needs hand-editing each time, and a gate that
# must be hand-updated is one that eventually gets updated wrongly. Use:
#     ls "$CEF_CHROMIUM_DIR"/chromium/src/cef/patch/patches/hodos_*.patch | wc -l
# or, better, cef_patch_drift_audit.sh, whose "hodos_*.patch files" /
# "hodos_* patch.cfg entries" lines ARE the presence gate (HODOS_MIN_PATCHES).
# The patcher's "N patches total" line remains useful as a cross-check and as the
# cheapest stale-copy tell in a raw build log -- but it is not the gate.

# P3: our CEF fork. Source patches live in it under patch/patches/hodos_*.patch,
# registered in patch/patch.cfg, applied by gclient_hook.py -> patcher.py during
# the BUILD step (NOT by run_patch_updater, which never applies on a pinned
# checkout -- so --force-build alone re-applies them).
#
# automate-git.py validates this against the existing checkout's
# remote.origin.url and hard-errors on a mismatch. A clean dir is NOT required:
# our fork shares upstream's object graph, so
#   git -C "$CEF_CHROMIUM_DIR/cef" remote set-url origin "$CEF_URL"
# satisfies the check and CEF_CHECKOUT still resolves. Mac must run that once,
# before its first fork build (Windows did on 2026-08-05).
#
# WARNING: pointing CEF_CHECKOUT at a NEW commit makes automate-git delete
# <tree>/chromium/src/cef -- which contains binary_distrib/. Move the tarballs
# out first. See PLAN_patch_toolchain.md R9 / P3_BASELINE_94c1726.md.
CEF_URL="https://github.com/Hodos-Browser/cef.git"

# Single condition gate for the whole C1-C7 farbling patch set. patcher.py
# applies a patch only if its 'condition' env var EXISTS, so unsetting this
# produces a farbling-free binary with no patch.cfg edit and no revert. Never
# gate C1/C2 separately from C3-C7 -- a half-applied set is worse than none.
export HODOS_FARBLING=1

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

# Check Python version (7871's .vpython3 pins 3.11)
if ! command -v python3 &>/dev/null; then
    log_error "Python 3 not found. Install Python 3.11 (7871 .vpython3 pin)."
    echo "  brew install python@3.11"
    exit 1
fi

PYTHON_VERSION=$(python3 --version 2>&1 | sed 's/Python //')
PYTHON_MAJOR=$(echo "$PYTHON_VERSION" | cut -d. -f1)
PYTHON_MINOR=$(echo "$PYTHON_VERSION" | cut -d. -f2)

if [ "$PYTHON_MAJOR" -ne 3 ] || [ "$PYTHON_MINOR" -lt 9 ] || [ "$PYTHON_MINOR" -gt 11 ]; then
    log_warn "Python $PYTHON_VERSION detected. 7871 .vpython3 pins 3.11."
    echo "The ceiling is per-branch - re-read .vpython3 on any branch bump."
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
    # Recover a previously-shallow clone (see the FULL-clone note below).
    if [ -f "$CEF_DEPOT_TOOLS_DIR/.git/shallow" ]; then
        echo "depot_tools is a shallow clone - unshallowing..."
        git fetch --unshallow
    fi
else
    echo "Cloning depot_tools from chromium.googlesource.com..."
    # Remove directory contents if it exists but isn't a git repo
    rm -rf "${CEF_DEPOT_TOOLS_DIR:?}/"*
    # MUST be a FULL clone, never --depth 1. CEF pins an exact depot_tools
    # commit in cef/CHROMIUM_BUILD_COMPATIBILITY.txt and automate-git.py hard-
    # checkouts it; a shallow clone dies with "reference is not a tree: <sha>"
    # AFTER cloning cef/ - far enough in to look like progress.
    git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git "$CEF_DEPOT_TOOLS_DIR"
fi

echo "[OK] depot_tools ready"

# Add depot_tools to PATH for this session
export PATH="$CEF_DEPOT_TOOLS_DIR:$PATH"

# --------------------------------------------------
# Step 4: Download automate-git.py (if not present)
# --------------------------------------------------

log_info "Setting up automate-git.py"

# automate-git.py is VERSIONED WITH CEF. The copy inside the branch checkout is
# authoritative and differs from master's (verified on 7871). Prefer it; only
# fall back to master for the chicken-and-egg first run, where cef/ does not
# exist yet and something has to clone it.
CHECKOUT_AUTOMATE="$CEF_CHROMIUM_DIR/cef/tools/automate/automate-git.py"
AUTOMATE_SCRIPT="$CEF_AUTOMATE_DIR/automate-git.py"

if [ -f "$CHECKOUT_AUTOMATE" ]; then
    AUTOMATE_SCRIPT="$CHECKOUT_AUTOMATE"
    echo "[OK] using the branch-matched automate-git.py from the CEF checkout"
else
    echo "No CEF checkout yet - bootstrapping with the master copy."
    echo "     Re-run this script after the checkout exists to pick up the"
    echo "     branch-matched copy."
    curl -fsSL \
        "https://raw.githubusercontent.com/chromiumembedded/cef/master/tools/automate/automate-git.py" \
        -o "$AUTOMATE_SCRIPT"
    echo "[OK] automate-git.py downloaded (bootstrap copy)"
fi

# --------------------------------------------------
# Step 5: Print build configuration
# --------------------------------------------------

log_info "Build Configuration"

echo "  CEF Branch:     $CEF_BRANCH (CEF 150 / Chromium 150, M150 LTS)"
echo "  CEF Checkout:   $CEF_CHECKOUT (150.0.17 -> chromium 150.0.7871.187)"
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

# --force-cef-update: MANDATORY, not optional. <tree>/chromium/src/cef is a COPY,
# and automate-git refreshes it only when cef_checkout_changed
# (automate-git.py:1358-1360), computed as
#     get_git_hash(<standalone cef dir>, 'HEAD') != get_git_hash(..., --checkout)
# Landing a patch REQUIRES committing in that standalone checkout, which moves its
# HEAD to exactly the SHA you then pin -- so current == desired and the copy is
# NEVER refreshed on the normal patch-landing workflow. Measured on Windows
# 2026-08-05 while landing C1: the build reported "114 patches total" instead of
# 115 and would have compiled ZERO Hodos patches with a fully green run. The older
# note claiming this "self-corrects" on the normal workflow is WRONG -- it
# self-corrects only if you never commit locally, which is not a real workflow.
# The refresh is a directory copy (seconds), so it is always passed rather than
# remembered.
python3 "$AUTOMATE_SCRIPT" \
    --download-dir="$CEF_CHROMIUM_DIR" \
    --depot-tools-dir="$CEF_DEPOT_TOOLS_DIR" \
    --url="$CEF_URL" \
    --branch="$CEF_BRANCH" \
    --checkout="$CEF_CHECKOUT" \
    "$BUILD_ARCH_FLAG" \
    --minimal-distrib \
    --client-distrib \
    --no-debug-build \
    --force-cef-update \
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
    echo "  - Python version incompatibility (7871 .vpython3 pins 3.11)"
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
echo "  cef_binary_150.*_macos${ARCH_LABEL}/"
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
