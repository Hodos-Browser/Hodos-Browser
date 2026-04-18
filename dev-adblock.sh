#!/bin/bash
# Launch adblock engine in dev mode (uses HodosBrowserDev data directory)
export HODOS_DEV=1
echo "DEV MODE: Launching adblock engine (data -> HodosBrowserDev)"
cd "$(dirname "$0")/adblock-engine"
cargo run --release
