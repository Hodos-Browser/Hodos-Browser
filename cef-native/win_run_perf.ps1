# B2-MEASURE - run the ALREADY-BUILT shell with header perf tracing ON.
#
# Prereqs:
#   1. C++ built with the perf instrumentation
#   2. .\stage_frontend_for_perf.ps1 run (prod transport active)
#   3. Rust wallet running (.\dev-wallet.ps1)
#
# Outputs (in build\bin\Release\):
#   - debug_output.log : the PERF lines (CreateBrowser->OnAfterCreated, ->LoadEnd, PERF_REPORT)
#   - hodos_perf_trace.json : load in Perfetto (ui.perfetto.dev) or chrome://tracing
$ErrorActionPreference = "Stop"
Push-Location $PSScriptRoot
try {
    $exe = "build\bin\Release\HodosBrowser.exe"
    if (-not (Test-Path $exe)) {
        throw "Not built yet. Run .\win_build_run.ps1 once first."
    }
    if (-not (Test-Path "build\bin\Release\frontend\index.html")) {
        Write-Host "WARNING: frontend not staged - you will measure the DEV (Vite) transport, not prod." -ForegroundColor Red
        Write-Host "Run .\stage_frontend_for_perf.ps1 first for a representative measurement." -ForegroundColor Red
    }
    $env:HODOS_DEV = "1"
    $env:HODOS_PERF_TRACE = "1"
    Write-Host "PERF MODE: HODOS_PERF_TRACE=1  (trace -> build\bin\Release\hodos_perf_trace.json)" -ForegroundColor Magenta
    Set-Location build\bin\Release
    & ".\HodosBrowser.exe" @args
}
finally {
    Pop-Location
}
