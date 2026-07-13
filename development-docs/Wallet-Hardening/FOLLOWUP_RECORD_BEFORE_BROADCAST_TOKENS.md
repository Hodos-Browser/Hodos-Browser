# Follow-up: record-before-broadcast (crash-safety) for token outputs

> **Status:** FUTURE thread — captured 2026-07-13, not scheduled. **No action needed for the
> current 0.4.0 build.** Trigger to revisit: when we add **non-self-derivable token outputs**
> (1Sat ordinals, BSV21, richer cert PushDrops). Extends
> [`FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md`](./FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md).

## The thread
Does every action write its outputs to the DB **before** broadcasting (a write-ahead
discipline), so a crash *after* broadcast is recoverable without reconstructing the output
from the chain? The BRC-100 / `@bsv/wallet-toolbox` model does: `createAction` persists the
tx + outputs (with locking script, basket, derivation/`customInstructions`) before broadcast;
a monitor confirms in-flight actions against the chain on restart. It never *reconstructs* an
output from raw script — annotation happens at creation, when full context exists.

## Where we are today (verified)
- **Sends** (`create_action_internal`): reserve inputs **and** record the tx before broadcast;
  `TaskFailAbandoned`/`TaskSendWaiting`/`TaskUnFail` reconcile on restart. Matches the model.
- **On-chain backup** (`do_onchain_backup`): reserves inputs **before** broadcast (Step 7) but
  records **outputs after** broadcast (Step 12) — **deliberately**, so a *failed* backup leaves
  "no ghost outputs" (`handlers.rs:13508` comment). The crash-after-broadcast window is covered
  because the backup token/marker/change are **fully self-derived**: adopt re-discovers the
  token, `/wallet/sync` re-discovers the change, and the WS1 reconcile self-heals any
  crash-induced phantom input. **This path is correct as-is and does NOT need changing.**
- **Cert publish/unpublish**: reserve + restore-on-failure; audit against this model when the
  broader work happens.

## The key distinction (why the backup is fine but this thread still matters)
- **Self-derivable outputs** (backup token, plain P2PKH change): recoverable from *either*
  ordering — the wallet can always re-derive/re-discover them. Record-after is safe.
- **Non-self-derivable / unique outputs** (1Sat ordinals, BSV21, unique-data PushDrops): the
  chain does **not** let you re-derive them. A crash *after* broadcast with record-*after* can
  lose the DB annotation permanently. **These need record-before-broadcast (or a durable WAL).**
  We have none of these on-chain-as-wallet-UTXOs today (ordinals unimplemented), hence FUTURE.

## Design principle to carry in (the under- vs over-approximation argument)
Prefer the DB to be a **conservative under-approximation** of on-chain truth, never an
over-approximation:
- **Record-after (current)** errs toward **under-count** on crash — the DB is *missing* a real
  output. Benign: invisible money, no failed operation, recovered by *affirmative* re-discovery
  (the chain proves the UTXO exists at our address → safe to add).
- **Record-before** errs toward **over-count** on failure/crash — the DB has a **ghost** output.
  Dangerous: a ghost can be *selected* by a spend → "Missing inputs" (the loop this program
  fixes), and cleanup is **absence-based** (must prove non-existence to delete — the exact
  "never infer from absence" trap that made us delete `TaskValidateUtxos`).
So record-before-broadcast is only worth its cost for outputs we **cannot re-derive** (where the
under-count recovery isn't available), and even then must pair with a safe ghost-cleanup that
never deletes on mere absence.

## TODO when this is picked up
1. Read `@bsv/wallet-toolbox` source: exact storage-write vs broadcast ordering, token-output
   storage shape, monitor reconciliation on restart. Confirm before designing (don't build on
   recollection).
2. Decide the per-token-type recovery contract (self-derivable → re-discover; unique → WAL).
3. Ensure any ghost-cleanup is affirmative (proof-of-non-existence), never absence-based.
