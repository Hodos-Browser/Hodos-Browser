# Dolphin Milk + Edwin Integration — Technical Docs (Hodos side)

> **This folder holds the engineering/technical half of the Dolphin Milk + Edwin
> integration doc set.** The pitch, product-framing, meeting-prep, and outreach
> half lives in the marketing intelligence vault — see **Companion folder** below.

## Split principle

- **Dev / technical docs** (architecture, security model, wallet compat, setup
  feedback, exploration) **stay here** in the Hodos-Browser repo, version-controlled
  alongside the code they describe.
- **Pitch / product / admin / outreach docs** live in the marketing intelligence
  vault. They are not in this repo.

## Congruence rule

The two halves are a **coupled set**. When a technical doc here changes in a way
that alters product implications, claims, or messaging, update the corresponding
marketing-side doc so the two stay congruent — and vice versa. Cross-references
between docs are by filename (prose), not clickable links; this README is the map.

## Companion folder (marketing / pitch half)

```
C:\Users\archb\Marston Enterprises\Hodos\marketing\intelligence\features\Dolphin Milk + Edwin Integration\
```

See that folder's `README.md` for the full pitch/product/outreach index.

## Docs in this folder (technical)

| Doc | What it is | Notes |
|-----|------------|-------|
| `INTEGRATION_PLAN_v1.md` | Technical architecture + implementation sequencing for bundling Edwin / Dolphin Milk into Hodos | **External-send** (Jake + Calhoun) |
| `EDWIN_SETUP_FEEDBACK_FOR_JAKE.md` | Engineering feedback to Jake: Edwin install/setup, Windows/WSL friction, Shad/qmd heaviness, 9P perf finding | **External-send** (Jake + Calhoun) |
| `ARCHITECTURE_TECHNICAL.md` | Three-party technical architecture (Hodos + Edwin + Dolphin Milk); cross-layer detail | |
| `EDWIN_VS_DOLPHIN_MILK_SECURITY.md` | Security-model comparison: Edwin's signed-envelope model vs Dolphin Milk's agent wallet | |
| `CANARY_A1_WALLET_COMPAT.md` | Wallet-compatibility verification (canary A1) between the Hodos wallet and the integrated agents | |
| `DOLPHIN_MILK_INTEGRATION.md` | Research doc: what Dolphin Milk is (x402 LLM payments, BRC-18 proofs, embedded BSV wallet) + bundling approach | |
| `THRESHOLD_ECDSA_EXPLORATION.md` | Exploration / future-tracking of John + BINARY's threshold-MPC (CGGMP'24) signing network; not committed direction | |

> **External-send docs** are written to be shared with external partners. Do **not**
> add cross-references to internal marketing/pitch material inside them.

## Related

- `../DevOps-CICD/WSL_HYBRID_WORKSPACE.md` — workspace/sync strategy for running Edwin in WSL
- Project memory: `project_edwin_install_session_2026_06_06`, `project_dolphin_milk_edwin_handoff_2026_06_03`
