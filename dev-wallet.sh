#!/bin/bash
# Launch Rust wallet in dev mode (uses HodosBrowserDev data directory)
export HODOS_DEV=1
echo "DEV MODE: Launching wallet (data -> HodosBrowserDev)"
cd "$(dirname "$0")/rust-wallet"
cargo run --release
