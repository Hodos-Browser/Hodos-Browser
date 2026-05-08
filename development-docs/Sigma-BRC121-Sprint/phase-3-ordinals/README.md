# Phase 3 — 1Sat Ordinals (Deferred)

**Status:** Deferred to a later sprint.

Sigma auth (Phase 2) alone unlocks app discovery in Cluster B (users can sign in to 1sat.market with their Hodos identity). Ordinal *transfer* requires substantial new work and is a separate sprint:

- New UTXO classification (1-sat outputs with inscribed data)
- BSV20/21 token indexer integration
- New ordinal transfer transaction builder
- New monitor task for indexer sync
- UI for inscriptions

## Existing research to evaluate

`development-docs/BSV-Tokens/` contains pre-existing research:

- `BSV_TOKEN_PROTOCOLS_COMPARISON.md` — likely trusted (raw protocol facts)
- `BSV21_1SAT_ORDINALS_ANALYSIS.md` — likely trusted (raw analysis)
- `MNEE_STABLECOIN_IMPLEMENTATION.md` — mixed trust
- `BSV21_PLAN_A_BACKEND.md` / `BSV21_PLAN_B_FRONTEND.md` — design decisions, **do not trust without re-review**
- `BSV21_UX_DESIGN_OUTLINE.md` — **do not trust**, redo from scratch when Phase 3 activates

When Phase 3 activates, extract trusted raw research into `research-extracted/` here. Folder will be moved into this sprint at that time.
