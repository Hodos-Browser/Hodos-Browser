# TICKET — `createSignature` rejects a request with no `counterparty` (suspected BRC-100 conformance gap)

**Opened** 2026-09-21, noticed during the B live test — **not chased** (owner's standing rule: note, don't chase).
**Status:** ⬜ OPEN — **not verified against the spec yet**. **Severity:** unknown until checked.

## Observed

A `window.CWI.createSignature({ protocolID: [1, "hodosbtest"], keyID: "1", data: [1,2,3] })` — no `counterparty`
field — was answered:

```
[Hodos] createSignature failed: Invalid JSON: missing field `counterparty` at line 1 column 58
```

⚠️ The caller was our own test script, so this proves only that **the field is required by our handler's request
struct**, not that any real site sends it without one.

## Why it might matter

To my understanding BRC-100 / `@bsv/sdk` `WalletInterface.createSignature` treats `counterparty` as **optional**
(default `'anyone'` for signing). If so, a conforming dApp that omits it gets a JSON-parse error from us, not a
signature. ⛔ **Unverified** — check the `@bsv/sdk` `CreateSignatureArgs` type and `wallet-toolbox` before changing
anything; this is signing code (root `CLAUDE.md` invariant 3 — ask first).

## Also noticed

The permission prompt for that call was shown and approved **before** the handler rejected the body — the gate runs
ahead of request validation. Probably fine (nothing is signed), but a user can be asked to approve a request that
was always going to fail.

## First step

Read `CreateSignatureArgs` in `@bsv/sdk`; compare with `rust-wallet/src/handlers.rs :: create_signature`'s request
struct. Same check for `createHmac`, `encrypt`, `decrypt`, which share the shape.
