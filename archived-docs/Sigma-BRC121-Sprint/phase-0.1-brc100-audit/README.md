# Phase 0.1 — BRC-100 Audit

**Type:** Research / spec deliverable. Gates Phase 2 (window.CWI shim implementation).

## Purpose

Audit Hodos's existing BRC-100 implementation against the canonical 28-method `WalletInterface` from `@bsv/sdk@2.0.13`. Identify gaps, document signature mismatches, plan extension to BRC-100's 3-tier permission model.

## Background

Yours Wallet's `brc100-remote` branch ships `window.CWI` as the canonical 28-method `WalletInterface`. See `../YOURS_CWI_MIGRATION.md` §2 for the method list. Hodos already implements BRC-100 mostly via the HTTP interceptor + Rust handlers, but the surface hasn't been audited method-by-method against the canonical spec.

## Deliverable

`AUDIT_RESULTS.md` (in this folder, to be created during audit) — a checklist listing each of the 28 methods:

| Method | Have? | Status (full / partial / missing) | Hodos handler/file | Argument shape match? | Gap notes |
|--------|-------|-----------------------------------|--------------------|------------------------|-----------|
| `getPublicKey` | ? | ? | ? | ? | ? |
| `revealCounterpartyKeyLinkage` | ? | ? | ? | ? | ? |
| ... (26 more) | | | | | |

Plus a section on permission-model extension: BRC-100 has three tiers beyond connect/disconnect (per-protocol, grouped, per-counterparty). Hodos's 3-layer model handles connect+spending. Plan additions.

## Method list to audit (28 canonical)

- **Identity / keys:** `getPublicKey`, `revealCounterpartyKeyLinkage`, `revealSpecificKeyLinkage`
- **Crypto:** `encrypt`, `decrypt`, `createHmac`, `verifyHmac`, `createSignature`, `verifySignature`
- **Transactions:** `createAction`, `signAction`, `abortAction`, `listActions`, `internalizeAction`
- **Outputs:** `listOutputs`, `relinquishOutput`
- **Certificates:** `acquireCertificate`, `listCertificates`, `proveCertificate`, `relinquishCertificate`, `discoverByIdentityKey`, `discoverByAttributes`
- **Auth:** `isAuthenticated`, `waitForAuthentication`
- **Chain info:** `getHeight`, `getHeaderForHeight`, `getNetwork`, `getVersion`

## Reference sources

- `../YOURS_CWI_MIGRATION.md` — full method list + comparison table + permission-model notes
- `https://unpkg.com/@bsv/sdk@2.0.13/dist/types/src/wallet/Wallet.interfaces.d.ts` — canonical interface
- Hodos's existing BRC-100 handlers: `rust-wallet/src/handlers.rs` (`well_known_auth` at `:564`, `create_action` at `:3381`, `create_action_internal` at `:3577`, plus 70+ other endpoints)

## Status

Not started.
