# ⚠️ MAC BACKUP FAILURE — FULL ANALYSIS HANDOFF (for Mac Claude)

**Created 2026-07-13 by Windows Claude. REVISED after owner found the real timeline. Priority: HIGH.
Gates whether reconcile ships in the 0.4.0 clean build.**

## Corrected framing — this is very likely NOT our July regression

Owner installed beta.27 on the **Mac** (installed app → `~/Library/Application Support/HodosBrowser/`),
clicked manual **"Backup Now"** → it FAILED; re-run did NOT recover. Initially suspected our reconcile/
c5b hardening. **But the owner then found the last good on-chain backup is from ~April 15, 2026** — i.e.
the backup has been broken for ~3 MONTHS, since well before any July reconcile work (and before the
May-7 shutdown-backup removal). **Our July changes cannot have started an April break.**

This lines up with a known event: the **Backup double-spend cascade incident of 2026-04-11**. Read it:
- `development-docs/Final-MVP-Sprint/backup-double-spend-incident-2026-04-11.md`
- Related commits: `8dd3d2c` (incident report), `3a6fd2e` (three backup pipeline bugs from the incident),
  `9ba106b` (disabled TaskValidateUtxos + reconcile — false external-spend bug), `1fa686f` (chain-truth
  hardening: backup adoption / double-spend verification).
- Backup system reference: `development-docs/ONCHAIN_BACKUP_SYSTEM.md`,
  `development-docs/Wallet-Hardening/ONCHAIN_BACKUP_REVIEW.md`.

## The likely diagnosis (owner's insight — confirm it)

**Backup-token DIVERGENCE.** The owner observes: the **pre-incident on-chain backup token is still
UNSPENT**, but the wallet's manual backup is **trying to spend something ELSE** (and failing). So the
DB's notion of "the current backup token" (the tip each new backup spends from) drifted away from the
chain during the April incident. The DB tries to spend a token that is spent/nonexistent → backup fails
every time. **This is the exact divergence class the reconcile sprint was meant to HEAL** — so the sharp
question is: **why did adopt_onchain_backup / c5b NOT heal it today?**

**Logs note:** April logs are long gone (rotation). But TODAY's manual-backup attempt IS still in
`~/Library/Application Support/HodosBrowser/logs/wallet_rCURRENT.log` — capture it.

## FULL ANALYSIS — READ-ONLY. Do NOT run another backup. Do NOT mutate the wallet. It may be funded.

**Operational pre-steps (owner shuts the app down first):**
- The owner will **cleanly quit** the Mac app (Cmd+Q / normal quit, NOT force-kill) so the WAL checkpoints
  into `wallet.db` and the logs flush. **Do NOT relaunch the app** — a relaunch restarts the monitor,
  which can mutate the very state we're photographing, and may rotate the log.
- **Copy the forensic files aside and analyze the COPIES** — never touch the live files:
  - `cp ~/Library/Application Support/HodosBrowser/wallet/wallet.db* <scratch>/` (grab `-wal`/`-shm` too if
    present; after a clean quit there should just be `wallet.db`).
  - `cp ~/Library/Application Support/HodosBrowser/logs/wallet_rCURRENT.log <scratch>/`.
  - Open the DB copy with `sqlite3` in read-only mode. Zero writes to anything under HodosBrowser/.

1. **Pull `origin/0.4.0`** (this note + current code).
2. **Today's failed attempt** — in `wallet_rCURRENT.log`, find the manual "Backup Now" run(s). Capture
   verbatim: the exact **outpoint/txid it tried to spend**, the error string, and any `adopt` / `Step 1.5`
   / `sweep` / `reconcile` / `check_outpoint` / `mark_spent` / `Missing inputs` / `Insufficient` lines.
3. **DB backup-token state** (`~/Library/Application Support/HodosBrowser/wallet/wallet.db`, `sqlite3`
   READ-ONLY): list ALL `1-wallet-backup` outputs ever recorded (token + `-3` marker) — txid:vout,
   `spendable`, `spent_by`, `spending_description`, satoshis, created order. Identify which one the DB
   currently treats as the live tip.
4. **Chain truth:** for each backup token in the DB history, WoC `/tx/{txid}` (exists?) + spent status.
   Identify the backup token that is actually **UNSPENT on chain** (owner believes it's the ~April
   pre-incident one). Confirm.
5. **Divergence map:** DB "current" backup tip vs chain "last unspent" backup token. Where did they fork?
   **What is the DB trying to spend, and why is it invalid** (spent / never existed)? — this answers the
   owner's "what is it trying to spend then?"
6. **Why didn't adopt heal it?** Trace `adopt_onchain_backup` / `do_onchain_backup` (`rust-wallet/src/
   handlers.rs` ~12688/12789/12813) for THIS wallet's state: does it DETECT the divergence and try to
   adopt the on-chain unspent token? If yes → why did the subsequent build/broadcast still fail? If no →
   what condition gated adopt OFF for this state? **This is the key shipping question** for whether
   reconcile/adopt actually fixes real divergence.

## Report back

Write findings to `development-docs/Wallet-Hardening/MAC_BACKUP_FAILURE_FINDINGS.md` and **push to
`origin/0.4.0`**. Include: today's attempted-spend outpoint + exact error, the full DB backup-token
history, which token is chain-unspent, the divergence fork point, and the answer to "why adopt didn't
heal." Windows Claude will read it and design the fix.

**Diagnose only. No fixes, no backups, no writes — a real/funded wallet in a fragile diverged state.**
