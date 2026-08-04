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
REM ============================================
echo.
echo === Running automate-git.py build (branch 7871) ===
echo.
cd /d C:\cef\cef150

python C:\cef\cef150\cef\tools\automate\automate-git.py ^
  --download-dir=C:\cef\cef150 ^
  --depot-tools-dir=C:\cef\cef150\depot_tools ^
  --branch=7871 ^
  --checkout=94c1726 ^
  --x64-build ^
  --minimal-distrib ^
  --client-distrib ^
  --no-debug-build ^
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
