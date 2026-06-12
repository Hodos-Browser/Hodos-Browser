#!/bin/bash
# Launch Rust wallet in dev mode (uses HodosBrowserDev data directory)
export HODOS_DEV=1
echo "DEV MODE: Launching wallet (data -> HodosBrowserDev)"
echo "Logs (rotating file + this console): ~/Library/Application Support/HodosBrowserDev/logs/"
# Optional: export RUST_LOG to override the default "info" level (e.g. "debug").
cd "$(dirname "$0")/rust-wallet"
cargo run --release
