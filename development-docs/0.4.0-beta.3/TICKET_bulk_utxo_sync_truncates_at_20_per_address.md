# The wallet never sees more than 20 UTXOs per address

**Found:** 2026-09-16, during the payment sitting (`PAYMENT_TEST_BATCH.md` M5).
**Status:** 🔴 OPEN — measured, not fixed. **Severity:** high for beta.4, moderate today.
**Found by:** a dust-consolidation test that had nothing to do with syncing.

---

## What happens

`WhatsOnChain`'s **bulk** unspent endpoint returns at most **20 UTXOs per address**. Its
single-address endpoint returns them all. Our sync uses the bulk endpoint, so every coin past the
twentieth on a given address is **never marked confirmed**, and every confirmed-only path therefore
cannot see it.

📏 Measured on one address, same moment, same indexer:

| Call | UTXOs returned |
|---|---|
| `POST /v1/bsv/main/addresses/unspent` (bulk — what sync uses) | **20** |
| `GET /v1/bsv/main/address/{addr}/unspent` (single) | **27** |

## How it surfaced

A test transaction paid 22 outputs to one of our own addresses — 21 × 700 sats and one 1-satoshi
carrier. After the transaction was mined (4 confirmations, verified on chain):

```
vout 0..19   700 sats   confirmed = 1
vout 20      700 sats   confirmed = 0     <-- stranded
vout 21        1 sat    confirmed = 0     <-- stranded
```

⛔ **It is a flat cap, not a value rule.** The stranded pair is "the 21st and 22nd", and the bulk
response contained twenty 700-satoshi entries and nothing else. The two indexer entries for a kept
and a stranded output are byte-identical in shape, both carrying `"height": 967061`.

⚠️ **The first hypothesis was wrong and worth recording.** Because one of the two stranded outputs was
the 1-satoshi carrier, this looked exactly like a value filter — the wallet failing to see token
carriers. It is not. Only listing every output of the transaction, rather than the two that were
missing, showed the boundary at 20.

## Why it matters

- **Silent.** Nothing errors. The coins are stored as rows, they simply never become `confirmed = 1`,
  so `get_spendable_confirmed_by_user` skips them, dust consolidation skips them, and any future
  confirmed-only path skips them.
- **It gets worse with use.** An address accumulates UTXOs; the cap is fixed.
- **🚨 beta.4's asset layer is exactly this shape.** 1Sat Ordinals put *many* 1-satoshi outputs on an
  address. A wallet holding more than 20 of them cannot see the rest through bulk sync. The ordinals
  sprint should not be built on top of this.
- The single-address path already exists as the **fallback when bulk fails** — so the fix may be
  routing rather than new code.

## What is NOT wrong

- Our fetcher applies no cap of its own (`utxo_fetcher.rs`: `BULK_BATCH_SIZE = 20` is *addresses per
  request*, a different thing, and the per-address loop reads every entry returned).
- The ingest path applies no value filter — `output_repo`'s own test
  `ingest_applies_no_value_filter` is green and stays true.
- `confirmed` is derived honestly from `u.height > 0`; the height simply never arrives for the
  truncated entries.

## Suggested shape of a fix (not implemented)

1. **Detect the truncation** rather than trusting the count: if a bulk response returns exactly 20
   UTXOs for an address, treat it as "possibly truncated" and re-fetch that address singly.
2. Or page the bulk endpoint if it supports an offset (unverified).
3. ⛔ Do **not** simply switch everything to single-address fetches — bulk exists because the sweep
   covers many addresses and the rate limit is real. The narrow rule above keeps the bulk path and
   pays the cost only for addresses that are actually at the boundary.
4. Whatever is built, the **negative control** is this: put 22 UTXOs on one address, and assert the
   count the wallet marks confirmed is 22 and not 20.

## Cross-references

- `PAYMENT_TEST_BATCH.md` M5 — the row that surfaced it; its carrier had to be marked confirmed by
  hand to finish, and that seeding is disclosed there.
- `development-docs/0.4.0-beta.4/` — the asset layer. ⚠️ This should be read before the ordinals
  sprint starts.
- `rust-wallet/src/utxo_fetcher.rs` — `WOC_BULK_CONFIRMED`, the per-chunk loop, and the
  single-address fallback that already exists.
