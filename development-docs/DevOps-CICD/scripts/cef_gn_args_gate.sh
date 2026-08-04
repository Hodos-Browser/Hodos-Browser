#!/usr/bin/env bash
# ⛔ HARD GATE — run BEFORE the 10-12 h build (PLAN_codecs.md §7 step 1-2).
#
# A flipped/renamed codec default ships a GREEN build with NO codecs. Catching
# that after 12 hours is the expensive failure. This generates the GN projects
# and asserts the resolved values, then inspects the HEVC derivations.
set -uo pipefail

SRC=/c/cef/cef150/chromium/src
OUT_REL="out/Release_GN_x64"

export GYP_MSVS_VERSION=2022
export DEPOT_TOOLS_WIN_TOOLCHAIN=0
export GYP_MSVS_OVERRIDE_PATH='C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools'
export vs2022_install='C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools'
export GN_DEFINES='is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0'
export CEF_ARCHIVE_FORMAT=tar.bz2
export PATH="/c/cef/cef150/depot_tools:$PATH"

cd "$SRC" || { echo "GATE_FAIL: no chromium/src"; exit 1; }

# Hooks must have run at least once: every gclient sync so far used --nohooks,
# and automate-git.py only runs hooks as part of its BUILD step. Without them
# there is no src/build/util/LASTCHANGE, no toolchain, and gn gen cannot work.
if [ ! -f "$SRC/build/util/LASTCHANGE" ]; then
  echo "=== Running gclient runhooks (never run — all syncs were --nohooks) ==="
  ( cd /c/cef/cef150/chromium && gclient runhooks ) 2>&1 | tail -30
  echo "RUNHOOKS_EXIT=$?"
fi

echo "=== Generating GN projects (gclient_hook.py) ==="
# cef_create_projects.bat is literally `python3.bat tools\gclient_hook.py` — a
# RELATIVE path, so it must run from src/cef. But invoking it through cmd.exe
# from git-bash does not inherit the POSIX working directory and fails with
# "'cef_create_projects.bat' is not recognized". Same family as the cmd.exe /c
# trap in the Lessons section.
#
# So skip the wrapper and call the python directly from bash, which is what the
# .bat does anyway. One fewer shell, real exit codes, correct cwd.
( cd "$SRC/cef" && python tools/gclient_hook.py ) 2>&1 | tail -30
echo "GCLIENT_HOOK_EXIT=$?"

echo
echo "=== args.gn as written ==="
cat "$OUT_REL/args.gn" 2>/dev/null || echo "(no args.gn)"

echo
echo "=== Resolved GN args (gn args --list --short) ==="
gn args "$OUT_REL" --list --short > /c/cef/cef150/gn_args_full.txt 2>&1
NARGS=$(wc -l < /c/cef/cef150/gn_args_full.txt)
echo "resolved args: $NARGS"

# ── SELF-CHECK ────────────────────────────────────────────────────────────────
# A gate that cannot tell ITS OWN failure from the failure it is looking for is
# worse than no gate. When gn gen silently does nothing, every flag reads
# MISSING — indistinguishable from "the codec flags were renamed on M150".
# So prove the gate ran before trusting any MISSING verdict:
#   - a real `gn args --list` emits HUNDREDS of lines (a broken one emitted 5)
#   - a control flag unrelated to codecs must resolve
if [ "$NARGS" -lt 100 ]; then
  echo "GATE_FAIL: only $NARGS args resolved — gn gen did not run properly."
  echo "           This is a GATE malfunction, NOT a codec finding."
  echo "           Check: cef_create_projects.bat must run from src/cef (relative path),"
  echo "           and gclient runhooks must have run (src/build/util/LASTCHANGE)."
  exit 2
fi
if ! grep -qE "^target_cpu = " /c/cef/cef150/gn_args_full.txt; then
  echo "GATE_FAIL: control flag target_cpu did not resolve — gate malfunction, not a codec finding."
  exit 2
fi
echo "self-check OK: $NARGS args, control flag target_cpu present"

echo
echo "=== GATE ROWS ==="
FAIL=0
check() { # name  expected
  local key="$1" want="$2"
  local got
  got=$(grep -E "^${key} = " /c/cef/cef150/gn_args_full.txt | head -1 | sed "s/^${key} = //")
  if [ -z "$got" ]; then
    echo "  MISSING  ${key}  (flag renamed or removed?)  <-- GATE FAIL"
    FAIL=1
  elif [ "$got" = "$want" ]; then
    echo "  OK       ${key} = ${got}"
  else
    echo "  MISMATCH ${key} = ${got}  (want ${want})  <-- GATE FAIL"
    FAIL=1
  fi
}
check proprietary_codecs true
check ffmpeg_branding '"Chrome"'
check chrome_pgo_phase 0
check is_official_build true

echo
echo "=== enable_widevine / CDM (must RESOLVE, value recorded not gated) ==="
grep -E "^(enable_widevine|enable_cdm_host_verification|enable_cdm_storage_id|enable_library_cdms) = " \
  /c/cef/cef150/gn_args_full.txt || echo "  (none found)"

echo
echo "=== HEVC derivations (recorded, NON-gating) ==="
grep -E "^(enable_platform_hevc|enable_hevc_parser_and_hw_decoder|enable_platform_encrypted_hevc|enable_hevc_hw_encoding) = " \
  /c/cef/cef150/gn_args_full.txt || echo "  (none found)"

echo
echo "=== AV1 / other media derivations (recorded) ==="
grep -E "^(enable_av1_decoder|enable_dav1d_decoder|enable_platform_dolby_vision|enable_platform_ac3_eac3_audio|enable_mse_mpeg2ts_stream_parser) = " \
  /c/cef/cef150/gn_args_full.txt || echo "  (none found)"

echo
echo "=== media/BUILD.gn coupling guard (fail-loud safety net) ==="
grep -n "ffmpeg_branding" "$SRC/media/BUILD.gn" 2>/dev/null | head -10 || echo "  (not found — note in tracker)"

echo
if [ "$FAIL" -eq 0 ]; then
  echo "GATE_RESULT: PASS — safe to start the build"
else
  echo "GATE_RESULT: FAIL — DO NOT START THE BUILD"
fi
exit "$FAIL"
