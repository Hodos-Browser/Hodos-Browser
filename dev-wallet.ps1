# Launch Rust wallet in dev mode (uses HodosBrowserDev data directory)
$env:HODOS_DEV = "1"
Write-Host "DEV MODE: Launching wallet (data -> HodosBrowserDev)" -ForegroundColor Cyan
Set-Location "$PSScriptRoot\rust-wallet"
cargo run --release
