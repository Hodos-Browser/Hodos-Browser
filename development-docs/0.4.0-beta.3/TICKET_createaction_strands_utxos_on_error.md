# 🚨 A failed `createAction` permanently strands the UTXOs it selected

**Found 2026-08-21**, during Phase 0.5 Task C, by noticing the dev wallet was ~$3.67 light and
refusing to accept the recorded "429 mempool under-report" explanation — there were **zero** 429
events in the log that day.

**Status: FIXED 2026-08-22 (Phase 0.7, Windows).** Pre-existing — not introduced by Phase 0.5.
Evidence, including every negative control, is in
`phase-0.7-utxo-reservation-leak/PHASE_CONTRACT.md` §3a.

> ⚠️ **Two claims in this ticket did not survive code re-reading; they are left below as written
> for provenance.**
>
> 1. *"`grep` finds no expiry, rollback, release or staleness sweep."* There was one:
>    `output_repo.rs :: restore_pending_placeholders()`, a blanket
>    `UPDATE … WHERE spending_description LIKE 'pending-%'` with no age filter and **no on-chain
>    check**, run unconditionally at every startup from `main.rs`. So a restart *did* release these
>    reservations — blindly. That is worse than it sounds: a row can still hold a `pending-`
>    placeholder while its transaction is already broadcast (the placeholder→txid update is only a
>    `warn!` on failure and the broadcast proceeds anyway), so the shipped recovery path could
>    un-spend a real spend. It has been deleted and replaced by the verified sweeper.
> 2. *"`sign_action` is the only current release."* The placeholder is also resolved inside
>    `create_action_internal` and restored on the broadcast-failure path.
>
> The core finding — that the 19 in-window `return` sites strand the reservation for the life of
> the process — was confirmed exactly, and re-measured on a pre-fix binary before the fix landed.

**The three UTXOs listed below were already recovered before Phase 0.7 began** (most likely by that
same blanket startup restore, on a restart during Phase 0.5). At kickoff the wallet held **zero**
`pending-%` rows, so there was nothing left for the new sweeper to reclaim; A3 was demonstrated on a
freshly staged strand instead.

## What happens

`rust-wallet/src/handlers.rs :: create_action_internal` reserves the UTXOs it selects by marking
them spent against a placeholder:

```rust
let placeholder_txid = format!("pending-{}", chrono::Utc::now().timestamp_millis());
output_repo.mark_multiple_spent(&utxos_to_reserve, &placeholder_txid)
//  "🔒 Reserved {} UTXOs (preventing concurrent selection)"
```

The placeholder is only ever resolved on the **success** path — `sign_action` calls
`update_spending_description_batch(placeholder, &txid)` once the transaction is signed.

Between the reservation and the point where the pending transaction is stored there are
**19 `return HttpResponse::…` sites**, and **not one of them releases the reservation.**
Ten of those are in the output-building stretch alone, including the most ordinary failure the
product has:

```
handlers.rs :: create_action_internal
    Failed to convert address '1BvB…aW': Address checksum mismatch
    -> 400 Invalid address
    -> the selected UTXOs stay spendable=0, spending_description='pending-<ts>', forever
```

**Nothing ever cleans them up.** `grep` finds no expiry, rollback, release or staleness sweep for a
`pending-%` reservation anywhere in `output_repo.rs` or `monitor/`.

**An ordinary `/wallet/sync` does NOT release them.** Measured immediately after the incident:
`{"synced_addresses":50,"new_utxos":0,"reconciled":0,"balance":18025202}` — balance unchanged.

## Measured instance

Three probes that died at address-checksum validation stranded three UTXOs:

| Reserved at | Outpoint | Satoshis | On-chain |
|---|---|---|---|
| 09:07:38 | `447060147dcd2576…:2` | 13,954,646 | **UNSPENT** |
| 09:26:30 | `d03d8e9af60663a5…:0` | 5,000,000 | **UNSPENT** |
| 09:26:30 | `8012239517dc9c60…:2` | 1,448,668 | **UNSPENT** |
| | **total** | **20,403,314** (**$3.67** @ $18.00) | |

All three verified against WhatsOnChain's per-address unspent set. **The money is not lost** — the
outputs exist and are unspent on chain. The wallet simply will not select them.

## Why this matters beyond a test artifact

The trigger is not exotic. **A user who typos a destination address hits it**: address→script
conversion happens *inside* `create_action_internal`, i.e. after UTXO selection and reservation.
One typo and the coins that would have funded the send become unspendable, silently, with a
`400 Invalid address` as the only feedback. Repeat it a few times on a wallet with few large UTXOs
and the wallet reports "Insufficient funds" while holding the money.

Every other early return in that window has the same effect: basket/tag validation failures,
description-length failures, script-hex failures, DB errors, BEEF parse failures.

## Suggested fix

An RAII/guard-shaped release, not a `return` audit — the 19 sites are exactly why a per-site fix
will rot. Options, in order of preference:

1. **Scope guard.** Hold the placeholder in a struct whose `Drop` releases the reservation unless
   explicitly `commit()`ed once the pending transaction is stored. Correct under `?`, early return
   and panic alike.
2. **Sweeper.** A `monitor/` task that releases any `pending-%` reservation older than N minutes
   with no matching row in `PENDING_TRANSACTIONS`. Cheap, and also cleans up reservations lost to a
   process restart — which the guard alone would not.
3. Both. (2) is the safety net for (1).

⚠️ **Do not simply release on every error path by hand.** That is the enumerate-don't-gate pattern
this sprint has already been bitten by three times.

## Recovery for an already-stranded wallet

Setting `spendable=1` and clearing `spending_description` for rows whose `spending_description`
matches `pending-%` and whose outpoint is confirmed unspent on-chain restores them. ⛔ Verify
on-chain first — a `pending-` row whose transaction *did* broadcast under a real txid must not be
un-spent, or the wallet will try to double-spend it.

## Related

- `development-docs/0.4.0-beta.3/phase-0.5-money-path/PHASE_CONTRACT.md` §4r (the run that found it)
- The recorded 429 confirmed-only under-report is a **different** mechanism and was correctly ruled
  out here: zero 429 events in the log for that day.
