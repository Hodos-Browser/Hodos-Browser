# Run the ALREADY-BUILT HodosBrowser C++ shell in dev mode -- NO rebuild.
# Use this for smoke testing / launching extra instances when the build is current.
# (Use win_build_run.ps1 instead when you changed C++ and need a rebuild.)
#
# This does NOT kill existing instances -- each profile runs as its own process,
# so you can have several profiles open at once. Launching the SAME profile twice
# just opens a new window in the instance that is already running (by design).
#
# Usage:
#   .\win_run.ps1                       # no --profile (normal resolution / picker)
#   .\win_run.ps1 --profile=Profile_1   # launch straight into the "Test" profile

$ErrorActionPreference = "Stop"
Push-Location $PSScriptRoot
try {
    $exe = "build\bin\Release\HodosBrowser.exe"
    if (-not (Test-Path $exe)) {
        throw "Not built yet. Run .\win_build_run.ps1 once first."
    }
    $env:HODOS_DEV = "1"
    Write-Host "DEV MODE (run-only): launching HodosBrowser (data: HodosBrowserDev)" -ForegroundColor Cyan
    Set-Location build\bin\Release
    & ".\HodosBrowser.exe" @args
}
finally {
    Pop-Location
}
