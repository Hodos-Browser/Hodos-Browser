# Scripts

Utility scripts for development, testing, and CI/CD.

## Test Runner

### `test-all.ps1`

Unified test runner for all stacks.

```powershell
# Quick test (all stacks)
./scripts/test-all.ps1

# Verbose output (see test details)
./scripts/test-all.ps1 -Verbose

# Overnight run with saved logs
./scripts/test-all.ps1 -NightlyReport

# Filter specific tests (Rust only)
./scripts/test-all.ps1 -Filter "brc42"

# Skip frontend (Rust-focused work)
./scripts/test-all.ps1 -SkipFrontend

# With coverage reports
./scripts/test-all.ps1 -Coverage
```

### Nightly Reports

When using `-NightlyReport`, results are saved to:

```
test-reports/
└── 2026-02-27/
    ├── summary.json       # Pass/fail + timing per stack
    ├── rust-wallet.log    # Full cargo test output
    ├── adblock-engine.log
    └── frontend.log
```

## Planned Scripts

- `build-installer.ps1` — Build NSIS/WiX installer (pre-release)
- `sign-binary.ps1` — Code signing with OV certificate
- `release.ps1` — Tag + build + sign + upload to GitHub Releases
