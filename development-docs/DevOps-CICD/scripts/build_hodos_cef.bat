@echo off
REM ============================================
REM CEF Build Script for Hodos Browser (Windows)
REM
REM VER-1: branch 7871 = CEF 150 = Chromium 150 (the M150 LTS line).
REM Pinned point-release: 150.0.17+g94c1726+chromium-150.0.7871.187
REM   -> CEF commit 94c1726, which pins Chromium refs/tags/150.0.7871.187
REM      transitively via cef\CHROMIUM_BUILD_COMPATIBILITY.txt.
REM
REM Builds with proprietary codecs (H.264, AAC, MP3, VP9, AV1).
REM
REM TREE LAYOUT (D11): the 7871 tree lives at C:\cef\cef150\ and has its OWN
REM depot_tools. C:\cef\chromium_git (the 175 GB M136 tree) is PRESERVED and
REM must not be reused: automate-git.py hard-checkouts depot_tools to the
REM commit its branch pins, so a shared depot_tools ends up pinned to whichever
REM branch ran last.
REM
REM THIS SCRIPT IS PHASE 3 OF 3. Run them in order:
REM   1. checkout : automate-git.py ... --no-build --no-distrib
REM   2. GATE     : gn_args_gate.sh    <-- codec pre-flight, DO NOT SKIP
REM   3. build    : this script (--force-build)
REM The gate exists because a flipped or renamed codec default produces a GREEN
REM build with NO codecs, and discovering that after 10-12 hours is the
REM expensive failure mode.
REM
REM Run from a normal cmd or PowerShell (NOT a Developer Command Prompt).
REM Do NOT invoke via `cmd.exe /c` from git-bash: MSYS rewrites /c into a path,
REM so cmd opens interactively and exits 0 having done nothing.
REM ============================================

REM ============================================
REM P3: our CEF fork + the farbling patch gate.
REM
REM CEF_URL points automate-git.py at Hodos-Browser/cef instead of upstream.
REM Our source patches live in that fork under patch/patches/hodos_*.patch,
REM registered in patch/patch.cfg. They are applied by gclient_hook.py ->
REM patcher.py during the BUILD step (NOT by run_patch_updater -- that never
REM applies on a pinned checkout). Consequence: --force-build alone re-applies.
REM
REM automate-git.py validates --url against the existing checkout's
REM remote.origin.url and hard-errors on a mismatch. It does NOT require a clean
REM dir -- our fork shares upstream's object graph, so:
REM   git -C C:\cef\cef150\cef remote set-url origin <fork>
REM satisfies the check and the pin still resolves. Done once, 2026-08-05.
REM
REM WARNING: changing CEF_CHECKOUT to a NEW commit makes automate-git delete
REM chromium\src\cef -- which contains binary_distrib\. Move the tarballs out
REM first (see P3_BASELINE_94c1726.md). THIS IS TRIGGERED by the C3 pin bump.
set CEF_URL=https://github.com/Hodos-Browser/cef.git

REM Pin an exact FORK commit, not the moving hodos/7871 branch tip: a build must
REM be reproducible, and patch content is part of the build. BUMP THIS every time
REM a patch lands on hodos/7871, and record the new SHA in the fork's
REM HODOS_PATCHES.md. Upstream content is unchanged -- dfe5a2343 is
REM 94c1726 (upstream 7871 head) plus our patch commits.
set CEF_CHECKOUT=9ccef044f

REM ⚠️ chromium\src\cef is a COPY of the standalone checkout, refreshed ONLY when
REM the CEF checkout HASH changes (automate-git.py:1358-1360). If you manually
REM git-checkout or git-pull C:\cef\cef150\cef to the target commit before running
REM this, the hashes already match, the copy never refreshes, and the build
REM silently uses the STALE in-tree patch set -- right fork, right pin, green run,
REM ZERO Hodos patches compiled in. --force-cef-update is passed unconditionally
REM below precisely so this cannot silently happen.
REM
REM ⛔ VERIFY BY PRESENCE, NOT BY TOTAL. The gate is "at least one
REM hodos_*.patch is in the tree", which is invariant. Do NOT assert
REM "N patches total" -- that number changes on every landing, so it needs
REM hand-editing each time, and a gate that must be hand-updated is one that
REM eventually gets updated wrongly. Use:
REM     dir /b chromium\src\cef\patch\patches\hodos_*.patch
REM or, better, cef_patch_drift_audit.sh, whose "hodos_*.patch files" /
REM "hodos_* patch.cfg entries" lines ARE the presence gate (HODOS_MIN_PATCHES).
REM The patcher's "N patches total" line remains useful as a cross-check and as
REM the cheapest stale-copy tell in a raw build log -- but it is not the gate.

REM HODOS_FARBLING is the single condition gate on the whole C1-C7 farbling
REM patch set (never gate them separately -- a half-applied set is worse than
REM none). patcher.py applies a patch only if its 'condition' env var EXISTS.
REM Unset this line to ship a farbling-free binary with no patch.cfg edit and no
REM revert -- the patches are simply skipped. Upstream 7871 carries zero
REM 'condition' entries, so ours is the first.
set HODOS_FARBLING=1

REM Set Visual Studio version and path
set GYP_MSVS_VERSION=2022

REM CRITICAL: Use local VS install, not Google's internal toolchain
set DEPOT_TOOLS_WIN_TOOLCHAIN=0

REM Tell Chromium where BuildTools edition is installed
set GYP_MSVS_OVERRIDE_PATH=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools
set vs2022_install=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools

REM GN build defines for proprietary codecs.
REM Byte-identical to the M136 build's args.gn - carried forward verbatim.
set GN_DEFINES=is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0

REM Archive format
set CEF_ARCHIVE_FORMAT=tar.bz2

REM Use THIS tree's depot_tools, not the M136 tree's.
set PATH=C:\cef\cef150\depot_tools;%PATH%

REM Build-scoped git config. Git for Windows ships core.autocrlf=true in its
REM SYSTEM config, which makes `gclient sync` fail on freshly-cloned third_party
REM repos ("your local changes would be overwritten"). Chromium's own
REM .gitattributes protects the MAIN tree but not the sub-repos. Precedence is
REM system < global < local, so this overrides the installer default WITHOUT
REM editing the machine-wide config or affecting any other repo.
REM Also required for P3: `git apply -p0` is exact-context with no fuzz, so
REM CRLF-contaminated sources would fail to patch.
REM Copy scripts\chromium-build-gitconfig next to the tree first.
set GIT_CONFIG_GLOBAL=C:/cef/cef150/gitconfig

REM ============================================
REM Build. Siso is the default build tool on a fresh 7871 out-dir (Ninja has
REM been unsupported upstream since Sept 2025). Never mix - switching requires
REM `gn clean`.
REM
REM --force-build forces a rebuild but KEEPS existing objects, so an interrupted
REM build resumes. We deliberately do NOT use --force-clean on re-runs.
REM
REM NOTE: automate-git.py is taken from the CEF CHECKOUT, not from master. It is
REM versioned with CEF and the master copy differs from 7871's.
REM
REM --no-depot-tools-update: automate-git.py runs update_depot_tools.bat on
REM EVERY invocation, re-pulling depot_tools OFF the pinned commit and rewriting
REM its files. That re-dirties the tree and makes the very next step -- the
REM checkout of the pinned depot_tools sha from CHROMIUM_BUILD_COMPATIBILITY.txt
REM -- fail with "your local changes would be overwritten", killing the build 3
REM seconds in. Check depot_tools out to the pin FIRST, then skip the update.
REM
REM PREREQUISITE, once per tree (repo-local beats system AND global, and unlike
REM GIT_CONFIG_GLOBAL it is honoured by depot_tools' own bundled git):
REM   for %%r in (depot_tools cef chromium\src) do (
REM     git -C %%r config core.autocrlf false
REM     git -C %%r config core.filemode false
REM     git -C %%r reset --hard
REM   )
REM ============================================
echo.
REM --force-cef-update: MANDATORY, not optional. chromium\src\cef is a COPY, and
REM automate-git refreshes it only when cef_checkout_changed
REM (automate-git.py:1358-1360), which is computed as
REM     get_git_hash(<standalone cef dir>, 'HEAD') != get_git_hash(..., --checkout)
REM Landing a patch REQUIRES committing in that standalone checkout, which moves its
REM HEAD to exactly the SHA you then pin -- so current == desired and the copy is
REM NEVER refreshed on the normal patch-landing workflow. Measured 2026-08-05 while
REM landing C1: the build reported "114 patches total" instead of 115 and would have
REM compiled ZERO Hodos patches with a fully green run. An earlier note claiming this
REM "self-corrects" on the normal workflow is WRONG; it self-corrects only if you
REM never commit locally, which is not a real workflow. The refresh is a directory
REM copy -- seconds -- so it is always passed rather than remembered.
echo === Running automate-git.py build (branch 7871) ===
echo.
cd /d C:\cef\cef150

python C:\cef\cef150\cef\tools\automate\automate-git.py ^
  --download-dir=C:\cef\cef150 ^
  --depot-tools-dir=C:\cef\cef150\depot_tools ^
  --url=%CEF_URL% ^
  --branch=7871 ^
  --checkout=%CEF_CHECKOUT% ^
  --x64-build ^
  --minimal-distrib ^
  --client-distrib ^
  --no-debug-build ^
  --no-depot-tools-update ^
  --force-cef-update ^
  --force-build

REM Exit code check
if %ERRORLEVEL% NEQ 0 (
  echo.
  echo BUILD FAILED with error code %ERRORLEVEL%
  echo Check output above for errors.
) else (
  echo.
  echo BUILD SUCCEEDED
  echo Output: C:\cef\cef150\chromium\src\cef\binary_distrib\
  echo.
  echo Next: Layer-A codec probe, then the VER-5 dist drift audit
  echo       ^(scripts\cef_dist_drift_audit.sh^), then rebuild
  echo       libcef_dll_wrapper + cef-native.
)

pause
