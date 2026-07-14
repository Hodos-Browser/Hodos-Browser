# Build and run HodosBrowser C++ shell in dev mode
# Usage: .\win_build_run.ps1 [--clean]

$ErrorActionPreference = "Stop"

Push-Location $PSScriptRoot

try {
    # Clean build if --clean flag passed
    if ($args -contains "--clean") {
        Write-Host "Cleaning build directory..."
        Remove-Item -Recurse -Force build -ErrorAction SilentlyContinue
    }

    # Configure if needed
    if (-not (Test-Path "build\HodosBrowser.sln") -and -not (Test-Path "build\build.ninja")) {
        Write-Host "Configuring CMake..."
        cmake -S . -B build -G "Visual Studio 17 2022" -A x64
    }

    # Build
    Write-Host "Building..."
    cmake --build build --config Release
    if ($LASTEXITCODE -ne 0) { throw "Build failed" }

    # Kill any existing DEV instance ONLY. Match by exe path under THIS build dir, never
    # by bare image name: dev and installed-prod both ship the image name HodosBrowser.exe
    # (CMakeLists OUTPUT_NAME), so `Stop-Process -Name HodosBrowser` force-killed the running
    # INSTALLED production browser too (dev/prod deconfliction audit 2026-07-14, gap C2).
    $devExeDir = Join-Path $PSScriptRoot "build\bin\Release"
    Get-CimInstance Win32_Process |
        Where-Object { $_.Name -eq "HodosBrowser.exe" -and $_.ExecutablePath -and $_.ExecutablePath.StartsWith($devExeDir, [System.StringComparison]::OrdinalIgnoreCase) } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }

    # Launch in dev mode
    $env:HODOS_DEV = "1"
    Write-Host "DEV MODE: Launching HodosBrowser (data -> HodosBrowserDev)" -ForegroundColor Cyan
    Set-Location build\bin\Release
    .\HodosBrowser.exe
}
finally {
    Pop-Location
}
