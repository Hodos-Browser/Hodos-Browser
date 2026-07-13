# Windows Claude's CODE-HUNT FINDINGS — read this to orient the Mac analysis

**For Mac Claude. Windows Claude (lead) ran a 5-agent code hunt on our own reconcile/c5b diff. This is the
reasoning behind the checks in `MAC_BACKUP_FAILURE_HANDOFF.md`. Read both. You are still READ-ONLY —
diagnose only, no fixes, no backups, analyze a COPY of the DB.**

## The situation in one paragraph

The owner's Mac wallet: manual "Backup Now" FAILED on beta.27, and running it AGAIN did NOT recover.
**Two things are likely BOTH true:** (a) the wallet's backup has been diverged since ~April 15 — a
PRE-EXISTING condition from the **April 11 backup double-spend cascade incident**
(`development-docs/Final-MVP-Sprint/backup-double-spend-incident-2026-04-11.md`), which predates all our
July reconcile work; AND (b) running "Backup Now" under our new **c5b Step 1.5 sweep** today may have
**permanently made it worse** by marking a still-good backup token spent. Your job is to determine which
arm actually fired, using the DB + today's log. Do not assume — the code hunt gives a mechanism, the
evidence decides.

## The suspect mechanism (what to confirm or rule out)

`do_onchain_backup` now runs a **c5b Step 1.5 sweep** (`rust-wallet/src/handlers.rs:12827-12850`, gate
`backup_outpoints.len() > 2` at :12841) BEFORE broadcast:
1. It gathers ALL spendable `1-wallet-backup` outpoints — **including the current, good token (suffix "1")
   and its "-3" marker** — and passes the whole set to `reconcile_spent_inputs` (:12844).
2. Inside, each outpoint is probed by `check_outpoint_spent`. GorillaPool's route is dead (→ `NoSignal`),
   so WhatsOnChain is the sole voice. **One stale/false WoC `200 + valid txid` yields `Spent{Y}`.**
3. The walk verifies `Y` is a real mined tx (txid-hash + merkle proof) but **NEVER verifies that Y's
   inputs actually spend the candidate outpoint** → `mark_spent(current_token, Y)` writes `spendable=0`.
   The `WHERE spendable=1` guard is useless because the good token *is* spendable=1.
4. **No self-heal undoes this:** `adopt_onchain_backup` is in-memory only; the broadcast-failure reconcile
   heals FUNDING outpoints only, never the token/marker (`:13535-13547`). So the bad `spendable=0` sticks
   → **every subsequent "Backup Now" starts from the corrupted state** = "ran it again, didn't recover."

Note: `fetch_onchain_backup` (`:14259-14330`) is 100% chain-based and never reads the DB → the on-chain
backup coins are recoverable; it's the "Backup Now" *operation* that's bricked.

## YOUR DECISIVE CHECKS (analyze a COPY; do not write anything)

**1. Did the sweep fire?** grep `wallet_rCURRENT.log`: `🧹 c5b backup-sweep: N spendable backup outputs`
- **Absent** → sweep never ran (gate ≤ 2). Our new pre-broadcast code is EXONERATED; the cause is mundane
  (insufficient funds / wallet locked / hash-unchanged skip / network). Report the actual final error.
- **Present** → the sweep ran. grep next for `check_outpoint_spent <curr-token-txid>:0 → Spent { ... }
  (woc=ExplicitSpent, gp=NoSignal)` and `✅ reconcile: marked <curr-token>:0 spent by <Y>`.

**2. DB smoking gun (decisive):**
```sql
SELECT txid, vout, spendable, spent_by, spending_description, satoshis
FROM outputs WHERE derivation_prefix='1-wallet-backup';
```
- A row `spendable=0, spent_by=NULL, spending_description=<Y>` **while WoC `/tx/{txid}/{vout}/spent` shows
  that outpoint UNSPENT** = the sweep KILLED A GOOD TOKEN (our regression: H1/H3).
- A row `spendable=1` that WoC reports SPENT = the sweep left a spent phantom selectable (H2).
- Classify EVERY `1-wallet-backup` row as good-but-killed vs spent-but-trusted via WoC.

**3. Final error string disambiguates the arm:** `Insufficient funds (need ~N sats)` → H3;
`Missing inputs`/`double spend` → H1/H2 (build picked a spent input); generic `Broadcast failed` → check
which input was selected.

## Ranked hypotheses (for classification)

- **H1 (prime):** sweep marks the CURRENT good token/marker `spendable=0` via an unverified successor.
- **H2:** sweep leaves a stale phantom `spendable=1`; Step 5b re-selects a chain-spent token (`.first()`
  arbitrary tie-break, all tokens tie at 1000 sats).
- **H3:** current pair marked spent → lost `previous_sats` → `Insufficient funds` hard error.
- **H5:** the broadcast-failure reconcile heals FUNDING only → the jammed token input is never healed →
  why re-runs never recover (this is the missing safety net, not the cause).
- **H6:** mundane cause (if the sweep never fired). Rule in/out via check #1.

## Report back

Write to `development-docs/Wallet-Hardening/MAC_BACKUP_FAILURE_FINDINGS.md` and push to `origin/0.4.0`:
which arm fired, the DB-vs-chain classification of every `1-wallet-backup` row, today's attempted-spend
outpoint + exact error, and — if you can determine it — the ORIGINAL April divergence fork point (what the
DB thinks the tip is vs the chain's last unspent backup). Windows Claude will reconcile and design the fix.
