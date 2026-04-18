# Launch adblock engine in dev mode (uses HodosBrowserDev data directory)
$env:HODOS_DEV = "1"
Write-Host "DEV MODE: Launching adblock engine (data -> HodosBrowserDev)" -ForegroundColor Cyan
Set-Location "$PSScriptRoot\adblock-engine"
cargo run --release
