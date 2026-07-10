# Wallet-Hardening Roadmap — Program Plan

> One program, **three independent, sequenced workstreams.** Each is separately
> designed, adversarially reviewed, built in small revertible commits, and
> **independently shippable** — deliberately NOT interleaved on one branch (coupling
> money-critical reconcile with lower-urgency changes widens blast radius). All land
> on `0.4.0`; reconcile ships in a `0.3.0-beta.XX` **before** the chromium bump.

**Owner:** Matt · **Created:** 2026-07-10 · **Status:** WS1 build-ready; WS2/WS3 need design.
**Discipline (all workstreams):** research-first → design → **adversarial review at every
phase (design AND implementation)** → phased build → unit+integration+smoke, Windows/macOS
parity. Verify every cited file:line before relying on it. No production-code change without
owner sign-off on the design.

---

## Sequencing & ship strategy

| # | Workstream | Urgency | Blast radius | Ships |
|---|---|---|---|---|
| **WS1** | **Reconcile (spent-input)** | **HIGH** — users stuck, can't spend now | UTXO selection + broadcast-failure paths + backup | `0.3.0-beta.XX` (pre-chromium) — **FIRST** |
| **WS2** | **Reorg handling (reprove-on-reorg)** | Medium — rare on BSV, real correctness hole | Monitor + proof/confirmation machinery | a later `0.3.0-beta` or with `0.4.0` |
| **WS3** | **Next-index unification** | Low — privacy/cleanliness, no money risk | Address gen + backup/restore + recovery | with `0.4.0` (no rush) |

**Order:** WS1 first — build, ship, **verify on the real diverged dev wallet**, then move on.
WS2 and WS3 are independent of each other and of WS1's *code* (they touch different files), so
after WS1 they can proceed in either order or in parallel sessions. Recommend WS2 before WS3
(correctness > cleanliness). **Fresh session per workstream build** for context headroom;
WS1 commit 4 (the `create_action` wiring) especially deserves full focus.

**Decoupling rules:** each workstream = its own feature branch + its own commits; no workstream
depends on another's code; each builds green and is independently revertible; WS2 backstops WS1
(reorg safety) but WS1 does not require WS2 to be correct (it spends optimistically at depth-0
like the rest of the wallet).

---

## WS1 — Reconcile (spent-input) · **DESIGN DONE, BUILD-READY**
Fixes the `"Missing inputs"` stuck-spend loop AND the backup-token divergence via one
`reconcile_spent_inputs` primitive. Full design + adversarial-review log:
[`RECONCILE_PHASE2_DESIGN.md`](./RECONCILE_PHASE2_DESIGN.md) (decisions A/B/C settled).

| Phase | Content | State |
|---|---|---|
| P0 Research | BRC-42 derivation, providers, spent-signal semantics, reorg model | ✅ done (5+1 agents) |
| P1 Design | primitive + 3 triggers + guardrails | ✅ done (v2) |
| P2 Adversarial review (design) | 4-agent red-team → 4 CRITICALs fixed | ✅ done |
| **P3 Build c1–3** | extract `check_outpoint_spent` (total table, BananaBlocks, no-"unknown", txid-verify); extract `recover_change_index` (candidate + derivation-re-verify + bounded gap-scan, skip −3/backup/BIP32); add unwired `reconcile_spent_inputs` + unit tests | **behavior-neutral — start here** |
| P4 Adversarial review (impl) | red-team the extractions | pending |
| P5 Build c4 | wire `create_action` (restore-first → reconcile → full cleanup → `ERR_RECONCILED_RETRY`) + frontend one-retry hook + status/toast | pending — the money fix |
| P6 Build c5 | wire cert (replace fail-open) + `do_onchain_backup` (funding-only, `utxo_selection_lock`) | pending |
| P7 Verify | smoke on the diverged dev wallet (2d worked example) + beta.26 send fixture; parity Win/mac | pending — **gates ship** |
| P8 (opt) | proactive startup/Monitor self-healer (deferred; reuses primitive) | later |

**Open dependency:** beta.26 user's send logs (the ARC line) for the regular-send test fixture.

---

## WS2 — Reorg handling (general reprove-on-reorg) · **NEEDS DESIGN**
Closes a wallet-wide gap: after a tx is `completed`, nothing re-checks its block is still
canonical. Ticket + sketch: [`FOLLOWUP_REORG_HANDLING.md`](./FOLLOWUP_REORG_HANDLING.md).
**Not reconcile-specific** — fixes all txs; WS1 doesn't need it (spends optimistically like
everything else). We already own the hard primitive (`verify_tsc_proof_against_block`); missing
only the ongoing loop.

| Phase | Content | State |
|---|---|---|
| P0 Research | ✅ reference model (Chaintracks `subscribeReorgs→TaskReorg→reproveProven`; optimistic depth-0) + our gap audited | ✅ done (this session) |
| P1 Design | `TaskReproveOnReorg`: trigger (interval and/or new-tip-height ≤ last-tip), bounded recent-height window, per-tx `verify_tsc_proof_against_block` vs current canonical header, match→no-op / mismatch→`replace_proof` (reprove in place) or `mark_failed` (dropped). Reuse-first; ≤1 new column (`last_reproved_at`). | **next** |
| P2 Adversarial review (design) | red-team: window bounds, cache-trust (`block_headers` must fetch fresh, not trust cache), interaction with `TaskCheckForProofs` `maxAcceptableHeight`, false-orphan fail-closed, idempotency | pending |
| P3 Build | new Monitor task + registration; reuse `verify_tsc_proof_against_block`/`replace_proof`/`mark_failed` | pending |
| P4 Adversarial review (impl) | red-team the task | pending |
| P5 Verify | simulated-reorg test (orphan a block → assert reprove-in-place or mark_failed→restore); parity | pending |

**Design must decide:** recent-height window depth; interval vs tip-height trigger; whether to
also add the `maxAcceptableHeight` guard so we don't prove into an unvalidated tip.

---

## WS3 — Next-index unification · **NEEDS DESIGN**
Unify the two "next self-derivation index" sources (`wallet.current_index+1` for receive vs
`MAX(addresses.index)+1` for change) to one source of truth. Ticket + open questions:
[`FOLLOWUP_NEXT_INDEX_UNIFICATION.md`](./FOLLOWUP_NEXT_INDEX_UNIFICATION.md). Privacy/cleanliness,
**no money risk** (same index → same key). Touches receive-gen + backup/restore + recovery, so
its own small design.

| Phase | Content | State |
|---|---|---|
| P0 Research | ✅ dual-source identified + severity (address-reuse/gap, not fund-loss) | ✅ done (this session) |
| P1 Design | make `MAX(addresses.index>=0)` the single next-index source; decide keep `current_index` as cache-of-MAX (backward-compat, no schema change) vs retire (migration + backup-payload change → invariant #2). Grep every `current_index` reader; reconcile with recovery's `update_current_index`. | **needs decision + design** |
| P2 Adversarial review (design) | red-team: no consumer relies on `current_index` being ahead of MAX; backup/restore round-trip; recovery can't regress the counter | pending |
| P3 Build | unify; keep or retire column per P1 | pending |
| P4 Adversarial review (impl) + Verify | address-gen + backup/restore + recovery regression; parity | pending |

**Adjacent (separate, even smaller):** advanced-wallet address-display UX (show-fresh vs
show-most-recent) — frontend, privacy-only; tracked in the WS3 ticket, do independently.

---

## Cross-cutting
- **Register:** this roadmap + the three design/ticket docs live in `development-docs/Wallet-Hardening/`; link from its `README.md`.
- **Testing standard:** each workstream needs unit + integration + smoke with explicit Windows/macOS parity before its own ship (per root CLAUDE.md).
- **Invariants held by all three:** no crypto/derivation change; DB-schema change only if a workstream's design explicitly asks and owner approves (WS3 P1 may; WS1/WS2 do not).

## Doc index
- [`RECONCILE_SPENT_INPUTS_PLAN.md`](./RECONCILE_SPENT_INPUTS_PLAN.md) — WS1 parent plan
- [`RECONCILE_PHASE2_DESIGN.md`](./RECONCILE_PHASE2_DESIGN.md) — WS1 buildable design + review log
- [`FIX_A_RECONCILE_PLAN.md`](./FIX_A_RECONCILE_PLAN.md) / [`FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md`](./FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md) — WS1 precursors (backup-specific)
- [`ONCHAIN_BACKUP_REVIEW.md`](./ONCHAIN_BACKUP_REVIEW.md) — backup subsystem review + field bug
- [`FOLLOWUP_REORG_HANDLING.md`](./FOLLOWUP_REORG_HANDLING.md) — WS2 ticket + sketch
- [`FOLLOWUP_NEXT_INDEX_UNIFICATION.md`](./FOLLOWUP_NEXT_INDEX_UNIFICATION.md) — WS3 ticket
