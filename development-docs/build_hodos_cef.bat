@echo off
REM ============================================
REM CEF Build Script for Hodos Browser
REM Builds CEF 136 with proprietary codecs (H.264, AAC, MP3)
REM
REM USAGE: Copy this file to C:\cef\chromium_git\ and run from there
REM        Run in a normal cmd or PowerShell (NOT Developer Command Prompt)
REM ============================================

REM Set Visual Studio version and path
set GYP_MSVS_VERSION=2022

REM CRITICAL: Use local VS install, not Google's internal toolchain
set DEPOT_TOOLS_WIN_TOOLCHAIN=0

REM Tell Chromium where BuildTools edition is installed
set GYP_MSVS_OVERRIDE_PATH=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools
set vs2022_install=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools

REM GN build defines for proprietary codecs
set GN_DEFINES=is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0

REM Archive format
set CEF_ARCHIVE_FORMAT=tar.bz2

REM Ensure depot_tools is on PATH
set PATH=C:\cef\depot_tools;%PATH%

REM Step 1: Run gclient sync to download all dependencies (ninja, node, etc.)
echo.
echo === Step 1: Running gclient sync to download dependencies ===
echo.
cd /d C:\cef\chromium_git\chromium
call C:\cef\depot_tools\gclient.bat sync --nohooks --no-history
if %ERRORLEVEL% NEQ 0 (
  echo.
  echo WARNING: gclient sync had errors, continuing...
  echo.
)

REM Step 1b: Run hooks
echo.
echo === Step 1b: Running gclient runhooks ===
echo.
call C:\cef\depot_tools\gclient.bat runhooks
if %ERRORLEVEL% NEQ 0 (
  echo.
  echo WARNING: gclient runhooks had errors, attempting build anyway...
  echo.
)

REM Step 2: Run the automated build
echo.
echo === Step 2: Running automate-git.py build ===
echo.
cd /d C:\cef\chromium_git
python C:\cef\automate\automate-git.py ^
  --download-dir=C:\cef\chromium_git ^
  --depot-tools-dir=C:\cef\depot_tools ^
  --branch=7103 ^
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
  echo Output: C:\cef\chromium_git\chromium\src\cef\binary_distrib\
)

pause
