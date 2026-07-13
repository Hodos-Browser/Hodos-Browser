# ⚠️ MAC BACKUP FAILURE — INVESTIGATION HANDOFF (for Mac Claude)

**Created 2026-07-13 by Windows Claude. Priority: HIGH. This gates the 0.4.0 clean build.**

## What happened (owner report)

- Owner installed the **beta.27 draft** on his **Mac** (installed app → production data dir
  `~/Library/Application Support/HodosBrowser/`, NOT the Dev dir).
- The Mac wallet's on-chain backup was **healthy / working before** beta.27 — nothing wrong with it.
- Owner clicked **manual "Backup Now"** (wallet dashboard) **after** the reconcile/c5b hardening was
  installed → the backup **FAILED and is now broken**.
- Owner ran it **again** → it did **NOT** recover. The backup is stuck broken.
- Owner's framing (correct): a broken backup = a broken wallet. This is a regression, most likely
  introduced by our reconcile work.

## Prime suspect: our reconcile/c5b changes to the backup path (on branch `0.4.0`, head ~`b42edb0`)

These are the changes we made to the backup path — grep/verify current locations, they may have moved:
- **c5b Part 1 — backup-token phantom SWEEP** in `adopt_onchain_backup` (`rust-wallet/src/handlers.rs`,
  new "Step 1.5" ~line 12813, BEFORE the hash-check). Gathers all spendable `1-wallet-backup` outpoints
  via `get_spendable_by_derivation("1"/"marker")`; **gate: if len > 2** → `utxo_selection_lock` +
  `reconcile_spent_inputs(state, backup_outpoints)`. Intended to no-op on a healthy wallet (len==2).
- **c5b Part 2 — funding reconcile at the backup broadcast-failure arm** (~`handlers.rs:13476`, after
  `rollback_backup` restores inputs) → `reconcile_missing_inputs(state, &e, funding_outpoints)`.
- **c1 — `do_onchain_backup` adopt-branch repointed to `reconcile::check_outpoint_spent`**
  (`rust-wallet/src/reconcile.rs`) — WoC-primary spent probe; GorillaPool route is known-dead.
- `do_onchain_backup` at `handlers.rs:12688/12789`; endpoint `POST /wallet/backup/onchain`.

**Leading hypotheses to confirm/refute (do NOT assume):**
1. The sweep fired (len > 2 on this wallet unexpectedly) and `reconcile_spent_inputs` **wrongly marked
   the CURRENT good backup token spent** → wallet now has no valid backup → every backup fails. (The
   prior adversarial review claimed the sweep "can't harm the current valid backup" — that guarantee may
   be FALSE; the owner's real result trumps the review.)
2. `check_outpoint_spent` returned a **false Spent** on the current backup token/marker → adopt logic broke.
3. The reconcile probe threw / errored and **aborted the backup** (network probe failure propagating).
4. `utxo_selection_lock` held/contended.

## What Mac Claude must do (READ-ONLY — do NOT run another backup or mutate the wallet)

1. **Pull `origin/0.4.0`** to get this note + the current code.
2. **Grab the wallet log** and paste the relevant lines verbatim into the findings file (below):
   `~/Library/Application Support/HodosBrowser/logs/wallet_rCURRENT.log`
   Look around the two backup attempts — grep for: `backup`, `ERROR`, `reconcile`, `sweep`, `Step 1.5`,
   `check_outpoint`, `adopt`, `mark_spent`, `utxo_selection_lock`, `Insufficient`, `Missing inputs`,
   `ARC`, `broadcast`. Include the exact failure/error string(s).
3. **Read-only inspect the wallet DB** (`~/Library/Application Support/HodosBrowser/wallet/wallet.db`,
   use `sqlite3` read-only — do NOT write):
   - List all `1-wallet-backup` outputs (token + `-3` marker): their `spendable` flag, txid:vout,
     `spending_description`, `spent_by`, satoshis.
   - Cross-check each against chain: WoC `/tx/{txid}` (exists?) and the spent probe — is the CURRENT
     backup token actually spent on-chain, or is it unspent but wrongly `spendable=0` in the DB?
   - Determine: **did our sweep mark a GOOD (chain-unspent) backup token/marker as spent?** That would
     be the smoking gun.
   - How many spendable `1-wallet-backup` outpoints exist (was the `> 2` gate tripped)?
4. Note the wallet's balance + whether it's unlocked (rule out the mundane insufficient-funds / locked
   / hash-unchanged causes).

## Report back

Write findings to `development-docs/Wallet-Hardening/MAC_BACKUP_FAILURE_FINDINGS.md` and **push to
`origin/0.4.0`**. Include: the verbatim log lines, the DB backup-token state vs chain, which hypothesis
the evidence supports, and whether our c5b sweep/reconcile is confirmed as the cause. Windows Claude will
read it and design the fix.

**Do not attempt a fix on the Mac. Diagnose only. Do not run more backups (may further perturb state).**
