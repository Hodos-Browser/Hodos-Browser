#!/usr/bin/env bash
# VER-5 / Step-5.5 — CEF distribution file-manifest drift audit.
#
# WHY THIS EXISTS, and why it is not the audit you might assume:
#
# The plan originally said to diff the new CEF dist against "hardcoded copy-lists
# in cef-native/CMakeLists.txt". There is no such list — CMake does a wholesale
# copy_directory, so it can never drop a file. The real gate is the INSTALLER's
# extension whitelist, installer/hodos-browser.iss:
#
#     Source: "{StagingDir}\*.dll"   ...
#     Source: "{StagingDir}\*.bin"   ...
#     Source: "{StagingDir}\*.dat"   ...
#     Source: "{StagingDir}\*.pak"   ...
#     Source: "{StagingDir}\*.json"  ...
#     Source: "{StagingDir}\locales\*"
#
# A CEF file with any OTHER extension is copied by CMake, works perfectly in a
# from-source smoke test, and is then SILENTLY DROPPED when Inno packages the
# installer. The failure only appears for installed users — and, because a
# silent update ships the same incomplete file set, it is exactly the class of
# change that breaks an auto-update.
#
# 14 milestones of drift (M136 -> M150) makes at least one changed resource
# likely. This script turns that from a hope into a check.
#
# Usage:
#   cef_dist_drift_audit.sh <release-dir> [<resources-dir>]
# e.g. baseline:  cef_dist_drift_audit.sh cef-binaries/Release
#      new build: cef_dist_drift_audit.sh <distrib>/Release <distrib>/Resources
#
# Exit 1 if an extension outside the whitelist is found. Exit 0 = clean.

set -uo pipefail

WHITELIST_EXT="dll bin dat pak json"

# Extensions that are LINK-TIME ONLY and correctly absent from the installer.
# .lib = MSVC import libraries (libcef.lib, cef_sandbox.lib) — consumed when
# building cef-native, never shipped to users. Excluding them is what makes the
# M136 baseline read CLEAN, so that a genuinely new extension on the next branch
# stands out instead of drowning in known-good noise.
IGNORE_EXT="lib"

REL_DIR="${1:-}"
RES_DIR="${2:-}"

if [ -z "$REL_DIR" ] || [ ! -d "$REL_DIR" ]; then
  echo "usage: $0 <release-dir> [<resources-dir>]"
  exit 2
fi

echo "=== CEF dist drift audit ==="
echo "Release dir:   $REL_DIR"
[ -n "$RES_DIR" ] && echo "Resources dir: $RES_DIR"
echo "Whitelist:     $WHITELIST_EXT (+ locales/*)"
echo

scan() {
  local d="$1"
  [ -d "$d" ] || return 0
  # Top-level files only; locales/ is whitelisted wholesale via recursesubdirs.
  find "$d" -maxdepth 1 -type f -printf '%f\n' 2>/dev/null
}

ALL=$(mktemp); { scan "$REL_DIR"; [ -n "$RES_DIR" ] && scan "$RES_DIR"; } | sort -u > "$ALL"

echo "--- file inventory (top level, $(wc -l < "$ALL") files) ---"
cat "$ALL"
echo

echo "--- extension histogram ---"
sed -n 's/.*\.\([A-Za-z0-9_]*\)$/\1/p' "$ALL" | sort | uniq -c | sort -rn
echo

echo "--- files NOT covered by the installer whitelist ---"
FAIL=0
while IFS= read -r f; do
  ext="${f##*.}"
  # No dot at all, or an extension outside the whitelist.
  if [ "$ext" = "$f" ]; then
    echo "  !! $f   (no extension — NOT packaged)"
    FAIL=1
    continue
  fi
  skip=0
  for i in $IGNORE_EXT; do [ "$ext" = "$i" ] && skip=1 && break; done
  if [ "$skip" -eq 1 ]; then
    echo "  -- $f   (.$ext link-time only, intentionally not shipped)"
    continue
  fi
  covered=0
  for w in $WHITELIST_EXT; do [ "$ext" = "$w" ] && covered=1 && break; done
  if [ "$covered" -eq 0 ]; then
    echo "  !! $f   (.$ext outside whitelist — SILENTLY DROPPED by the installer)"
    FAIL=1
  fi
done < "$ALL"
[ "$FAIL" -eq 0 ] && echo "  (no unexpected extensions — every shipped file is covered)"
rm -f "$ALL"

echo
if [ "$FAIL" -eq 0 ]; then
  echo "DRIFT_RESULT: CLEAN"
else
  echo "DRIFT_RESULT: ACTION REQUIRED — add the extension(s) to installer/hodos-browser.iss [Files]"
  echo "               and re-check release.yml's staging step + CEF_HELPER_APP_SUFFIXES (mac)."
fi
exit "$FAIL"
