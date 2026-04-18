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

    # Kill any existing instance
    Stop-Process -Name "HodosBrowser" -Force -ErrorAction SilentlyContinue

    # Launch in dev mode
    $env:HODOS_DEV = "1"
    Write-Host "DEV MODE: Launching HodosBrowser (data -> HodosBrowserDev)" -ForegroundColor Cyan
    Set-Location build\bin\Release
    .\HodosBrowser.exe
}
finally {
    Pop-Location
}
