# 🚨 Every output found by address sync stores a **fabricated** locking script

**Found 2026-09-08**, during the Phase 8 dust-guard kickoff (`D-5` in
`phase-8a-dust-guard/PHASE_CONTRACT.md`). Not found by a failure — found while checking whether
`task_consolidate_dust`'s `is_p2pkh_script()` guard had teeth. It does not.

**Status: OPEN.** ⛔ **Not urgent for beta.3 — see §"What Phase 8 already closed".** It is filed
because of §"Why this matters to beta.4", which is the real cost.

> ⭐ **Method note.** Unlike most of this sprint's tickets, the core of this one is **measured
> against mainnet**, not read from code. The live outpoints and byte counts in §2 were fetched on
> 2026-09-08 and are reproducible. The code-reading parts say so.

## 1. What happens

The wallet never records the locking script it actually saw on chain. It **generates one from the
address** and stores that:

| Path | Site | Script stored |
|---|---|---|
| WhatsOnChain (primary) | `utxo_fetcher.rs :: fetch_utxos_woc` | `generate_p2pkh_script_from_address(address)` |
| GorillaPool ordinals (fallback) | `utxo_fetcher.rs :: fetch_utxos_gorillapool` | same |
| WhatsOnChain bulk | `utxo_fetcher.rs :: fetch_utxos_bulk` | same (pre-generated per address) |

⚠️ **This is not the wallet discarding data it was given.** Neither API returns a per-UTXO locking
script — `WhatsOnChainUTXO` is `{tx_hash, tx_pos, value, height, isSpentInMempoolTx}` and
`GorillaPoolUTXO` is `{txid, vout, satoshis, owner}`. The synthesis fills a genuine gap. It is
still a fabrication, and nothing downstream knows it is one.

`generate_p2pkh_script_from_address` always returns exactly 25 bytes in exactly the P2PKH pattern.
So for every synced output:

- **`is_p2pkh_script()` in `task_consolidate_dust` cannot return false.** Its comment claims it
  *"guards against accidentally trying to spend PushDrop tokens or other non-standard scripts."*
  For the dominant ingest path that is untrue by construction. *(Comment corrected in place by
  Phase 8, `383bf4f`; the underlying issue is this ticket.)*
- **`calculate_sighash(tx, i, prev_script, …)`** takes the stored script as the scriptCode. A wrong
  scriptCode produces a wrong digest, so the signature does not verify and **the whole transaction
  is rejected** — a failure, not a loss, but it takes every other input with it.

## 2. Measured — the guess is wrong for two of the three real cases

Fetched from mainnet, 2026-09-08:

| Output kind | Live outpoint | Real script | We store | Correct? |
|---|---|---|---|---|
| **Transferred ordinal** | `59fbd7b0…03d5:0` (1 sat) | bare P2PKH, **25 B** | 25 B P2PKH | ✅ correct |
| **Fresh inscription** | `7faac48b…6473:0` (1 sat) | P2PKH **+ ord envelope, 2,596,810 B** | 25 B P2PKH | ⛔ **wrong by 2.6 MB** |
| **OrdLock market listing** | `6af57766…f997:0` (1 sat) | contract, **860 B**, not P2PKH-shaped | 25 B P2PKH | ⛔ **wrong** |

🚨 **The inscription's first 25 bytes are a valid P2PKH** — `P2PKHshape = true`, then
`00 63 03 6f 72 64 51 09 "image/png"` (`OP_FALSE OP_IF "ord" OP_1 …`). **That is why address
indexers return it under the owner's address**, and why this reaches us at all.

**Two providers, disjoint answers for the same address** (`18ydEoCY9sPR7DzE6iX3pNkc4pZ7mngqf4`):

- WoC `/address/{a}/unspent/all` → `59fbd7b0`, `9db78c50`, `080c6262` — three bare-P2PKH 1-sat ordinals
- GorillaPool `/txos/address/{a}/unspent` → `63dfd808` — an OrdLock listing WoC does **not** list

⇒ `fetch_utxos` is a fallback chain (WoC → GorillaPool). **A WoC outage changes which set we
ingest**, and the fallback is an *ordinals indexer*. Neither sync path reconciles rows away
(`task_sync_pending` and `wallet_sync` are both discovery-only, deliberately), so anything ingested
during an outage becomes a **permanent** `spendable = 1` row.

## 3. What Phase 8 already closed — read this before sizing anything

⭐ **All three measured cases are 1-satoshi outputs, and `383bf4f` now excludes 1-satoshi outputs
from every spend path.** The signature-failure scenario is therefore **not reachable today** through
any of the shapes actually observed on chain. This ticket is **not** a live money bug.

What is left, in order of real cost:

1. ⛔ **A trap laid directly under beta.4 sprint 1** — §4. This is the reason to file.
2. ⚠️ **Permanent phantom rows.** OrdLock listings ingested via the fallback stay as spendable
   rows forever. 1 sat each; they inflate balance and UTXO count, never break a spend.
3. ⚠️ **Non-1-satoshi wrong scripts are possible in principle.** An inscription on a >1-sat output
   is legal, just uncommon; that one *would* still be selected and *would* break every transaction
   it entered. **Not observed — code reading, not a measurement.**
4. ⚠️ **Backup bloat, pre-existing and unrelated to the fix.** `cache_parent_transactions` already
   stores the full raw parent tx, and `parent_transactions.raw_hex` is in the on-chain backup
   payload (`backup.rs :: collect_payload`). Syncing one 2.6 MB inscription already puts 2.6 MB
   into the wallet DB **and** into the next on-chain backup. Worth its own look; not this ticket.

## 4. 🚨 Why this matters to beta.4 — the actual reason to fix it

beta.4 sprint 1 is the UTXO safety guard: *"a general seam, one classifier —
`Spendable` / `Token` / `Unknown`, fail closed on `Unknown`"* (beta.4 `README.md`, decision 3), and
it is the **real** fix that Phase 8's value floor stands in for.

⛔ **If that classifier inspects `outputs.locking_script`, it will read a fabricated bare P2PKH for
every output address sync ever found, and confidently classify all of them `Spendable`.** The ord
envelope — the exact signal it exists to detect — is *erased at ingest*, before the classifier runs.

The failure mode is the dangerous one: it does not error. **It fails closed on nothing, passes
everything, and looks like it works.** It is the same shape as the three 0.4.0 farbling harnesses
that would have passed with the feature absent, and as `R-INTEXT` reading zero lines because the log
was at `debug` — an instrument reading a value the system fabricated for it.

⇒ **Fix this before sprint 1's classifier is designed, not after.** Otherwise sprint 1 needs its own
negative control proving the classifier can see an envelope at all — which is a harder thing to
build than this fix.

## 5. The fix — the data is already local

⭐ **No new fetch, no new API, no schema change.**

- `monitor/task_sync_pending.rs :: cache_parent_transactions` **already fetches and stores the full
  raw parent transaction** for every newly-discovered UTXO (both tiers, `:237` and `:348`).
- `reconcile.rs :: parse_tx_outputs(raw_tx) -> Vec<(i64, Vec<u8>)>` **already returns value and
  locking script per output**, and is already used elsewhere.
- `outputs.script_length INTEGER` **already exists** (`migrations.rs:237`) and is currently **never
  written** for synced outputs — a free slot for the true length. ⇒ **CLAUDE.md invariant #2 is not
  engaged.**

Sketch: after `cache_parent_transactions`, read `parse_tx_outputs(raw)[vout]`, and
1. **verify the value matches** what the indexer claimed — a strong, free integrity check that also
   catches a provider lying or a vout mismatch;
2. write the **true `script_length`** always;
3. write the true `locking_script` — **but see the size trap below.**

### ⚠️ The size trap — do not write the naive version

A real inscription script measured **2.6 MB**. Blindly copying it into `outputs.locking_script`
(BLOB, no size guard) puts megabytes per row into the wallet DB and into every on-chain backup.

Options, to be decided when this is scoped — **not** decided here:

| Option | Shape | Note |
|---|---|---|
| **A** | Store the true `script_length` + a bounded **prefix** (say first 64 B); keep the synthesised script for the P2PKH case | Enough for a classifier (`len != 25` ⇒ not plain P2PKH; the `ord` envelope is within the first ~40 B). Cheap. **Recommended starting point.** |
| **B** | Store the full script under a size cap, flag oversize rows `Unknown` | Fail-closed, matches beta.4's stated posture. Costs DB + backup size. |
| **C** | Store nothing extra; have the classifier re-derive from `parent_transactions` on demand | No storage cost, but the classifier depends on a cache that `TaskPurge` prunes after 7 days |

⛔ **Whatever is chosen, stop synthesising silently.** A synthesised script must be *marked* as
synthesised, so no downstream consumer can mistake a guess for an observation. That, not the storage
format, is the defect.

## 6. Test / negative control

- **GREEN:** given a raw parent tx containing an output with a P2PKH prefix followed by an `ord`
  envelope, ingest records `script_length != 25` and the classifier does **not** call it plain P2PKH.
- **RED:** feed the same output through the current synthesis path — `script_length` is 25 and it
  classifies as plain P2PKH. ⛔ **Assert the length differs**, not merely that classification
  succeeded; a classifier that returns the same verdict either way is the whole bug.
- **SUBJECT:** the stored row, **not** the API response and **not** a log line — the fabrication
  happens between them.
- ⭐ The fixture is free: the byte prefix in §2
  (`76a914…88ac` + `0063036f7264 5109 image/png`) is real mainnet data and needs no network.

## 7. Links

- `phase-8a-dust-guard/PHASE_CONTRACT.md` §0.5 (`D-5`) — where this was found, and §7 `D-9`
- `TICKET_token_outputs_destroyed_by_dust_paths.md` — the sibling ticket; its floor is what makes
  this non-urgent today
- `../0.4.0-beta.4/README.md` decision 3 — the classifier this would blind
- `../0.4.0-beta.4/sprint-2-1sat-ordinals/README.md` §"Two rules from BRC-147" — why 1-sat outputs
  are assets rather than change
