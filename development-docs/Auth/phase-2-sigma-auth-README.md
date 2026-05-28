# Phase 2 — Sigma Auth

Two sub-phases:

- **2A — Signing primitives** (~300 LOC): `crypto/bsm.rs`, `crypto/brc77.rs`, `/signMessage` handler. Independent of Phase 0, ship immediately.
- **2B — Sigma interception** (gated): strategy A (V8 monkey-patch) or B (HTTP interception), decided by Phase 0 evidence.

Seed docs to move into this folder (Step 3 of skeleton plan):

- `BRC103_SIGMA_AUTH_GUIDE.md`
- `BRC103_SIGMA_COMPARISON_AND_IMPLEMENTATION.md`

Currently still at `development-docs/` root.
