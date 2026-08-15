# Phase 0.2 — `window.yours` Shim Design

**Type:** Research / spec deliverable. Gates Phase 2 (window.CWI shim implementation).

## Purpose

Design per-method semantic translations from the legacy `window.yours` API surface to the BRC-100 backend. Several legacy methods don't have clean BRC-100 equivalents — design decisions required before implementation.

## Background

Yours Wallet has *removed* `window.yours` entirely on `brc100-remote`. Sites still using the legacy API (e.g., Treechat targeting `window.panda`) will break on Yours v5+. Hodos's `window.yours`/`window.panda` shim becomes a real translation layer, not a thin alias. See `../YOURS_CWI_MIGRATION.md` §3 for the full comparison table.

## Deliverable

`SHIM_TRANSLATION_SPEC.md` (in this folder, to be created during design) — per-method translation rules.

## Key design decisions

### `signMessage({ message, encoding? })` → `createSignature(...)`
BRC-100 signatures bind to `(protocolID, keyID, counterparty)`. Need to invent a fixed convention so legacy callers (e.g., Treechat) get reproducible signatures.

**Proposal:** protocolID `[2, "yours-legacy-message"]`, keyID `"1"`, counterparty `"anyone"`. Document this so other shim implementers can interop.

### `getAddresses()` → ?
No clean BRC-100 equivalent. Options:
- (a) Fall back to identity-key-derived P2PKH (`getPublicKey({ identityKey: true })` → derive address). **Semantically wrong** — identity key isn't a fresh receive address — but functional for Treechat-style display use.
- (b) Return error / undefined.
- (c) Extend our shim with a Hodos-specific fresh-receive-address generator (won't exist on canonical BRC-100).

**Proposal:** (a) for the narrow Treechat-compat use case. Document the semantic mismatch clearly.

### `encrypt`/`decrypt` argument translation
Legacy: `{ message, encoding, pubKeys[] }`. BRC-100: `{ plaintext, protocolID, keyID, counterparty }`. Need protocolID/keyID convention for the legacy form.

**Proposal:** protocolID `[2, "yours-legacy-encrypt"]`, keyID `"1"`, counterparty = the first entry in `pubKeys[]` (with documented behavior if `pubKeys.length > 1`).

### `getBalance()` → sum of `listOutputs({ basket: 'default' }).outputs[].satoshis`
Direct sum. Cache for performance? Per-call?

### `sendBsv(payments[])` → `createAction({ outputs })`
Direct mapping. Each `PaymentObject` becomes an output entry. Inscription-bearing payments (with `inscription` field) need basket=`'1sat'` handling — but that's Phase 3 territory.

### Methods removed entirely
`getExchangeRate`, `getSocialProfile`, `getPubKeys` bulk → return error or fall back where reasonable.

### Ordinal methods (`inscribe`, `transferOrdinal`, `purchaseOrdinal`)
These collapse into `createAction` calls in Yours's new model, with the dApp building the locking script via `@1sat/templates`. Our shim has two options:
- (a) Forward to `createAction` and assume the dApp handles its own templating (matches Yours's posture)
- (b) Detect legacy-method calls and synthesize the inscription/transfer/purchase scripts in our shim (**more compat, more work**)

**Proposal:** (a) for v1. Phase 3 (ordinals) can revisit.

## Reference sources

- `../YOURS_CWI_MIGRATION.md` §3 — comparison table
- `https://yours-wallet.gitbook.io/provider-api` — legacy `window.yours` docs
- `../BRAVE_WALLET_REFERENCE.md` — V8 Proxy patterns + property-descriptor defenses

## Status

Not started.
