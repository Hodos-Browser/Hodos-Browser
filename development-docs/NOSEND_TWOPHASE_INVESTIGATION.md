# nosend Two-Phase Signing Investigation

**Date:** 2026-04-17  
**Status:** ACTIVE — needs code trace before any changes  
**Priority:** HIGH — all PushDrop token operations fail

## Summary

Two-phase signed transactions (PushDrop token spends, possibly certificate publish/unpublish) are set to `nosend` status even when the calling app did NOT request `noSend=true`. The wallet never broadcasts to miners. The transaction times out after 48h and gets failed by TaskCheckForProofs.

## Observed Behavior (ToDo token spend, 2026-04-17)

### App's request
```
acceptDelayedBroadcast=Some(true)
signAndProcess=None
noSend=None          ← NOT requested
randomizeOutputs=Some(false)
```

### createAction flow
1. App provides 1 user input (PushDrop `1c6d67f3:0`, 1 sat) + inputBEEF (4 txs, 4 BUMPs)
2. Wallet selects 1 additional UTXO (6,075,334 sats)
3. Wallet signs its input (index 1), leaves PushDrop (index 0) unsigned
4. Status set to `nosend`: `💾 Transaction status: nosend (app will broadcast to overlay)`
5. Returns `signableTransaction` with Atomic BEEF (5,713 bytes)

### First signAction (returns unsigned PushDrop)
- Input 0: `left unsigned (SDK signs via two-phase)`
- Input 1: signed by wallet (BRC-42 derivation)
- Status stays `nosend`
- Warning: `⚠️ 1 input(s) remain unsigned: [0]`
- Returns `signableTransaction` (Atomic BEEF)

### createSignature (SDK signs the PushDrop data)
- Protocol: `0-todo list`, Key: `1`, Counterparty: `self`
- Returns 71-byte DER signature

### Second signAction (applies SDK's unlocking script)
- Input 0: `applying SDK-provided unlocking script (73 bytes)` via `spends` parameter
- Input 1: re-signed by wallet (same key)
- All inputs now signed
- Status set to `nosend` AGAIN: `💾 Transaction status: nosend (app will broadcast to overlay)`
- Returned Atomic BEEF (5,786 bytes, fully signed)

### Result
- Transaction never broadcast to miners
- WoC returns 404
- TaskCheckForProofs eventually fails it
- Token restored to spendable

## Expected Behavior (per SDK spec)

After second `signAction` completes with all inputs signed:
- If `noSend=false` (default): wallet SHOULD broadcast to ARC
- If `acceptDelayedBroadcast=true`: broadcast can be async/background
- Then return Atomic BEEF to app for overlay submission
- App submits to overlay via TopicBroadcaster/SHIPBroadcaster

## SDK Spec Reference

From `@bsv/sdk` Wallet.interfaces.ts:

| `noSend` | `acceptDelayedBroadcast` | Expected behavior |
|----------|--------------------------|-------------------|
| `false` (default) | `false` | Wallet broadcasts synchronously, waits for confirmation |
| `false` (default) | `true` | Wallet broadcasts in background, returns immediately |
| `true` | either | Wallet does NOT broadcast. Returns BEEF. App handles broadcast. |

Key types:
```typescript
CreateActionResult {
  txid?: TXIDHexString           // Set when wallet broadcasts
  tx?: AtomicBEEF                // Always returned
  noSendChange?: OutpointString[] // Change outputs from noSend txs
  signableTransaction?: SignableTransaction // When two-phase needed
  sendWithResults?: SendWithResult[]        // Batch broadcast results
}
```

After external broadcast of a `noSend` tx, app should call `internalizeAction()` to sync wallet state.

## Investigation Steps

### 1. Trace signAction handler
File: `rust-wallet/src/handlers.rs` — find the `signAction` / `sign_action` handler.

Questions:
- Where exactly is `nosend` status set?
- Is it set unconditionally for two-phase, or does it check the `noSend` option?
- After the second signAction (all inputs signed), is there a code path that broadcasts?
- Does the handler check if `noSend` was originally requested?

### 2. Trace createAction handler  
File: `rust-wallet/src/handlers.rs` — find `create_action` / `create_action_internal`.

Questions:
- Where is the `noSend` option stored? Is it persisted with the transaction?
- Is `acceptDelayedBroadcast` stored?
- When `signAndProcess=true` and all inputs are signed, does it broadcast?

### 3. Check certificate publish/unpublish
Files: `rust-wallet/src/certificate/certificate_handlers.rs`

Questions:
- Do certificate operations use `noSend`?
- Do they go through the same two-phase flow?
- Could this bug affect certificate publish/unpublish too?

### 4. Compare with wallet-toolbox
File: `reference/ts-brc100/node_modules/@bsv/sdk/`

Questions:
- How does the SDK's built-in wallet handle two-phase completion?
- Does it broadcast after second sign when `noSend=false`?

### 5. Check broadcast safety
Before adding broadcast after two-phase:
- Will this double-broadcast if the app also broadcasts?
- Is ARC idempotent for duplicate broadcasts?
- What if broadcast fails — do we leave status as `nosend` or mark `failed`?

## Key Constraint
Do NOT change any code until the full flow is traced and understood. We have gotten this wrong multiple times. The fix must be based on reading the actual code paths, not assumptions.

## Related Files
- `rust-wallet/src/handlers.rs` — createAction, signAction handlers
- `rust-wallet/src/monitor/task_check_for_proofs.rs` — nosend timeout handling
- `development-docs/CERTIFICATE_UI_AND_LIFECYCLE.md` — certificate flow docs
- `reference/ts-brc100/node_modules/@bsv/sdk/src/wallet/Wallet.interfaces.ts` — SDK types
