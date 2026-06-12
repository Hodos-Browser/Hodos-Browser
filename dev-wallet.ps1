# Launch Rust wallet in dev mode (uses HodosBrowserDev data directory)
$env:HODOS_DEV = "1"
Write-Host "DEV MODE: Launching wallet (data -> HodosBrowserDev)" -ForegroundColor Cyan
Write-Host "Logs (rotating file + this console): $env:APPDATA\HodosBrowserDev\logs\" -ForegroundColor DarkGray
# Optional: set $env:RUST_LOG to override the default "info" level (e.g. "debug").
Set-Location "$PSScriptRoot\rust-wallet"
cargo run --release
