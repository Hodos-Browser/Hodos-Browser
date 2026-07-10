# Follow-up Ticket — General reorg handling (reprove-on-reorg) for confirmed txs

> **Type:** Wallet-hardening — closes a real gap for **all** transactions (not just
> reconcile). Low probability on BSV, but currently a **total** gap.
> **Priority:** Medium. **Out of scope** for the spent-input reconcile sprint
> ([`RECONCILE_PHASE2_DESIGN.md`](./RECONCILE_PHASE2_DESIGN.md)); reconcile is safe
> without it because it spends optimistically at depth-0 like everything else.
> **Status:** identified 2026-07-10 (reorg-model research); not started.

---

## The gap (verified in code)

Our wallet has **no reorg handling for any `completed` transaction.** Once a tx is
`completed` with an immutable `proven_txs` row, nothing ever re-checks that its block
is still on the canonical chain:

- `proven_txs` is INSERT-OR-IGNORE + immutable; mutators (`delete_by_txid`/`replace_proof`,
  `proven_tx_repo.rs:178-209`) are used only to correct corrupt ARC/WoC proofs at
  **acquisition**, never on a reorg.
- The Monitor tasks that could react are scoped to *pre-confirmation / failed* states
  only: `TaskCheckForProofs` (`status IN ('sending','unproven','nosend')`,
  `task_check_for_proofs.rs:64`), `TaskUnFail` (`status='failed'`, `task_unfail.rs:40`),
  `TaskReviewStatus` ("never un-completes a tx"). **Nothing queries `completed`.**
- `verify_tsc_proof_against_block` (`cache_helpers.rs:168-237`) *is* a genuine canonical
  check (fetches the block by height, compares merkle roots) — but it runs **once**, at
  proof-acquisition (`task_check_for_proofs.rs:579,714`), and the result is stored
  immutably. No Chaintracks, no header subscription, no reprove loop.

**What we DO have (adjacent):** pre-confirmation orphan/stale reactions —
`MINED_IN_STALE_BLOCK` / `SEEN_IN_ORPHAN_MEMPOOL` → `mark_failed` — but only for txs
still in `sending`/`unproven`/`nosend` (`task_check_for_proofs.rs:282-317`).

**Consequence:** a deep reorg that orphaned a `completed` tx's block would leave us
silently believing a now-invalid tx is confirmed (inputs permanently spent, change
permanently spendable). Astronomically unlikely on BSV, but a real correctness gap.

## How the reference stack handles it (target model)

- **`@bsv/sdk`:** stateless — `MerklePath.verify()` re-checks `isValidRootForHeight`
  every verification; an orphaned proof simply fails.
- **`@bsv/wallet-toolbox`:** stateful — a **Chaintracks** header service
  `subscribeReorgs → deactivatedHeaders → TaskReorg → reproveProven`: finds every
  `proven_tx` whose `blockHash` orphaned, fetches a **fresh** merkle path, re-checks the
  root, and **updates the proof row in place** — the tx stays `completed` the whole time.
  It never un-marks or kicks the tx back to pending. Spending stays optimistic (depth-0).

## Proposed fix (sketch — reuses existing primitives; no crypto/schema change)

Add a **`TaskReproveOnReorg`** to the Monitor:
- **Trigger:** long interval (e.g. hourly) and/or the cheap reorg signal — a new tip whose
  height ≤ our last-seen tip height.
- **Scope:** walk `proven_txs` within a **bounded recent-height window** (deep reorgs are
  astronomically unlikely; bounding keeps it cheap).
- **Check:** re-run the existing `verify_tsc_proof_against_block` **by height** against the
  *current* canonical header (fetch fresh — do **not** trust the passive `block_headers`
  cache, which is never proactively invalidated on reorg).
- **On root match:** no-op (still canonical).
- **On mismatch (block orphaned):** demote — fetch a fresh merkle path via the Services
  chain; if a valid new proof exists → `replace_proof` in place (wallet-toolbox
  `reproveProven` behavior, tx stays confirmed); if the tx was genuinely dropped in the
  reorg → route through the existing `mark_failed` path so inputs restore + change disables,
  letting normal recovery / UTXO-sync reconcile funds.

Reuses `verify_tsc_proof_against_block`, `replace_proof`, `mark_failed` — **no new crypto,
no schema change** beyond possibly a `last_reproved_at` column. Closes the gap symmetrically
with the reference stack and backstops the reconcile feature.

## Open questions for its design
- Recent-height window size (how deep a reorg to defend against) vs. cost.
- Whether to add the cheap `new-tip-height ≤ last-tip` reorg trigger or rely on the interval.
- `block_headers` cache: confirm the reprove task fetches fresh (the cache is
  `INSERT OR REPLACE` by hash and never invalidated on reorg — don't trust it).
- Interaction with `TaskCheckForProofs`' `maxAcceptableHeight`-style guard (avoid proving
  into a not-yet-validated tip), which we may also want.

## Unverified assumptions (from research)
- Chaintracks *server-side* reorg-detection internals (only the toolbox Monitor
  subscription was confirmed).
- Whether a canonical-hash refresh of our `block_headers` cache is needed for the task.
