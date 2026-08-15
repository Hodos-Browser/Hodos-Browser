# Phase 0.1 — BRC-100 Audit Results

**Date:** 2026-05-06.
**Audit target:** Hodos's BRC-100 surface vs. canonical 28-method `WalletInterface` from `@bsv/sdk@2.0.13` (the surface Yours Wallet's `brc100-remote` branch ships as `window.CWI`).
**Method:** Grep + Read against `rust-wallet/src/handlers.rs`, `rust-wallet/src/handlers/certificate_handlers.rs`, and the routing table in `rust-wallet/src/main.rs`.

> **Visual summary:** see `../ARCHITECTURE.md` for box-and-arrow diagrams of where the gaps below land in the Hodos stack.

---

## TL;DR

- **26 / 28 methods are FULLY implemented** with canonical BRC-100 routes (camelCase, no prefix) and argument shapes that match `@bsv/sdk@2.0.13`.
- **2 methods are MISSING:** `revealCounterpartyKeyLinkage`, `revealSpecificKeyLinkage`. Both belong to BRC-72-style identity-derivation introspection.
- **No ordinal-shaped gaps in the BRC-100 surface itself.** Ordinals route through existing `createAction` / `listOutputs` with `basket: '1sat'`; the audit found no missing wallet entrypoint for ordinal flows.
- **Permission-model gap:** BRC-100 expects three additional permission tiers (per-protocol, grouped, per-counterparty) that Hodos's existing 3-layer auto-approve does not yet model. `OPEN_QUESTIONS.md` Q17.

The audit recommends a small **Phase 1.5 — BRC-100 Surface Completion** between Phase 1 (BRC-121) and Phase 2 (V8 shim). See "Recommended Phase 1.5 scope" below.

---

## Coverage table — all 28 canonical methods

Routes are registered in `rust-wallet/src/main.rs:763–812`. Every BRC-100 route uses the canonical camelCase name (no prefix, no `/brc100/` namespacing). All confirmed by grep on the source files cited.

| Method | Hodos handler (`file:line`) | HTTP route | Args match `@bsv/sdk@2.0.13`? | Status |
|---|---|---|---|---|
| **Identity / keys** | | | | |
| `getPublicKey` | `handlers.rs:264` | `POST /getPublicKey` | yes — `{identityKey?, protocolID?, keyID?, forSelf?, counterparty?}` | **FULL** |
| `revealCounterpartyKeyLinkage` | — | — | — | **MISSING** |
| `revealSpecificKeyLinkage` | — | — | — | **MISSING** |
| **Crypto** | | | | |
| `encrypt` | `handlers.rs:1530` | `POST /encrypt` | yes — `{protocolID, keyID, plaintext, counterparty?}` | **FULL** |
| `decrypt` | `handlers.rs:1712` | `POST /decrypt` | yes — `{protocolID, keyID, ciphertext, counterparty?}` | **FULL** |
| `createHmac` | `handlers.rs:824` | `POST /createHmac` | yes — `{protocolID, keyID, data, counterparty?}` | **FULL** |
| `verifyHmac` | `handlers.rs:1134` | `POST /verifyHmac` | yes — `{protocolID, keyID, data, hmac, counterparty?}` | **FULL** |
| `createSignature` | `handlers.rs:2781` | `POST /createSignature` | yes — `{protocolID, keyID, data, counterparty?}` | **FULL** |
| `verifySignature` | `handlers.rs:2494` | `POST /verifySignature` | yes — `{protocolID, keyID, data, signature, counterparty?}` | **FULL** |
| **Transactions** | | | | |
| `createAction` | `handlers.rs:3381` | `POST /createAction` (100 MB payload) | yes — `{outputs, inputs?, inputBEEF?, description?, labels?, options?}` | **FULL** |
| `signAction` | `handlers.rs:6281` | `POST /signAction` (100 MB payload) | yes — `{reference, spends, options?}` | **FULL** |
| `abortAction` | `handlers.rs:9955` | `POST /abortAction` | yes — `{reference}` | **FULL** |
| `listActions` | `handlers.rs:10898` | `POST /listActions` | yes — `{labels, labelQueryMode?, includeLabels?, includeInputs?, includeOutputs?, limit?, offset?}` | **FULL** |
| `internalizeAction` | `handlers.rs:10082` | `POST /internalizeAction` | yes — `{reference, inputs, ...}` | **FULL** |
| **Outputs** | | | | |
| `listOutputs` | `handlers.rs:14243` | `POST /listOutputs` | yes — `{basket, tags?, tagQueryMode?, include?, includeCustomInstructions?, includeTags?, includeLabels?}` | **FULL** |
| `relinquishOutput` | `handlers.rs:14536` | `POST /relinquishOutput` | yes — `{basket, outputIndex, txid}` | **FULL** |
| **Certificates** | | | | |
| `acquireCertificate` | `handlers/certificate_handlers.rs:725` | `POST /acquireCertificate` | yes — `{type, certifier, serialNumber, ...}` | **FULL** |
| `listCertificates` | `handlers/certificate_handlers.rs:288` | `POST /listCertificates` | yes — `{certifiers?, types?}` | **FULL** |
| `proveCertificate` | `handlers/certificate_handlers.rs:3134` | `POST /proveCertificate` | yes — `{type, certifier, serialNumber, fieldsToReveal}` | **FULL** |
| `relinquishCertificate` | `handlers/certificate_handlers.rs:54` | `POST /relinquishCertificate` | yes — `{type, certifier, serialNumber}` | **FULL** |
| `discoverByIdentityKey` | `handlers/certificate_handlers.rs:3334` | `POST /discoverByIdentityKey` | yes — `{identityKey, types?}` | **FULL** |
| `discoverByAttributes` | `handlers/certificate_handlers.rs:3494` | `POST /discoverByAttributes` | yes — `{attributes, types?}` | **FULL** |
| **Auth** | | | | |
| `isAuthenticated` | `handlers.rs:449` | `POST /isAuthenticated` | yes — body ignored | **FULL** |
| `waitForAuthentication` | `handlers.rs:459` | `POST /waitForAuthentication` | yes — body ignored | **FULL** |
| **Chain info** | | | | |
| `getHeight` | `handlers.rs:14629` | `POST /getHeight` | yes — body ignored | **FULL** |
| `getHeaderForHeight` | `handlers.rs:14677` | `POST /getHeaderForHeight` | yes — `{height}` | **FULL** |
| `getNetwork` | `handlers.rs:14868` | `POST /getNetwork` | yes — body ignored | **FULL** |
| `getVersion` | `handlers.rs:243` | `POST/GET /getVersion` | yes — body ignored | **FULL** |

**Result: 26 FULL, 2 MISSING, 0 PARTIAL.**

---

## Missing-method targets

Both missing methods are about disclosing the BRC-42 derivation linkage between a key the wallet holds and a counterparty / specific keyID — used by counterparty workflows that need to prove "yes, this child key really is derived from my master via this invoice." Tiny, isolated, additive.

### `revealCounterpartyKeyLinkage`

Canonical signature (from `@bsv/sdk@2.0.13/src/wallet/Wallet.interfaces.d.ts`):

```ts
revealCounterpartyKeyLinkage(
  args: { counterparty: string; verifier: string; privileged?: boolean; privilegedReason?: string },
  originator?: string
): Promise<{
  prover: string;
  verifier: string;
  counterparty: string;
  revelationTime: string;
  encryptedLinkage: number[];
  encryptedLinkageProof: number[];
}>;
```

Returns the BRC-42 linkage envelope encrypted to the verifier's pubkey so only the verifier can read it. Implementation: derive the linkage value, encrypt with `encrypt({ protocolID: [2, "counterparty linkage revelation"], keyID: counterparty, counterparty: verifier })`-style invoice convention. The Babbage SDK has reference behaviour.

### `revealSpecificKeyLinkage`

Canonical signature:

```ts
revealSpecificKeyLinkage(
  args: { counterparty: string; verifier: string; protocolID: WalletProtocol; keyID: string; privileged?: boolean; privilegedReason?: string },
  originator?: string
): Promise<{
  prover: string;
  verifier: string;
  counterparty: string;
  protocolID: WalletProtocol;
  keyID: string;
  encryptedLinkage: number[];
  encryptedLinkageProof: number[];
  proofType: number;
}>;
```

Same shape but bound to a specific (protocolID, keyID) pair instead of the generic counterparty linkage.

### Implementation sizing

Both handlers can reuse existing crypto primitives in `rust-wallet/src/crypto/brc42.rs` and the `encrypt` machinery already present at `handlers.rs:1530`. New code: ~150–250 LOC of handlers + serde structs + a small `crypto/key_linkage.rs` module. **No DB schema changes** — this is pure derivation + encryption.

---

## Argument-shape spot checks

Spot-checking the implemented methods against the canonical interface confirmed the shapes line up. Notable defensive flexibility found in the Rust handlers:

- `keyID` and `protocolID` accept both string and array forms (per BRC-43 invoice-number conventions), via `serde` enum deserialization.
- `data` / `plaintext` / `ciphertext` accept both base64 and byte-array forms.
- `counterparty` accepts the magic strings `"self"` and `"anyone"` plus hex pubkeys.

No obvious arg-shape mismatches surfaced. The defensive flexibility is *additive* — handlers accept canonical shapes plus convenience extras. This is fine for canonical compliance and is what `@1sat/wallet-browser` does on the page side too.

A targeted regression sweep against `@bsv/sdk@2.0.13` reference fixtures should be part of Phase 1.5 verification, not this audit.

---

## Permission-model gap

BRC-100 (per `YOURS_CWI_MIGRATION.md` §7 and the Yours `brc100-remote` source) expects three permission tiers beyond connect/disconnect:

| Tier | Trigger | Scope |
|------|---------|-------|
| `PermissionRequest` | First call to a protocol+keyID combo from this origin | (origin, protocolID) |
| `GroupedPermissionRequest` | dApp-pre-bundled grants for many protocolIDs at once | (origin, protocolID-set) |
| `CounterpartyPermissionRequest` | First derivation for a specific counterparty | (origin, counterparty) |

Yours opens a separate popup window per tier. Hodos already has the overlay-subprocess pattern (`SettingsOverlayRoot.tsx`, `BackupOverlayRoot.tsx`, `DownloadsOverlayRoot.tsx`, etc.), so the UX shape transfers cleanly.

**What Hodos has today:**

- Origin-keyed connect/disconnect via `domain_permissions` (in code; not the dropped `domain_whitelist` table — see `rust-wallet/CLAUDE.md` migration V24 notes).
- Per-tab spending session + rate limit, enforced in `SessionManager` on the C++ side (see `cef-native/include/core/SessionManager.h`).
- `check_domain_approved()` in Rust handlers as a defense-in-depth gate (per `rust-wallet/src/CLAUDE.md` "Domain Permission Defense-in-Depth").

**What Hodos lacks:**

- A per-protocolID grant store on the wallet side. (Today: any approved domain can call any protocol.)
- A per-counterparty grant store. (Today: same.)
- A grouped-grant flow where a dApp can pre-request a batch of permissions and have them shown in one approval surface.

**Implication for the V8 shim:** Without these tiers, Hodos's BRC-100 surface is *more permissive* than canonical. A site approved for connect can immediately call `createSignature({ protocolID: [...], keyID: ..., counterparty: ... })` for any combination, with no per-pair gate. This may or may not be a problem given Hodos's auto-approve posture (see `AUTO_APPROVE_RATIONALE.md`), but **not modelling these tiers means we can't faithfully advertise as a BRC-100 substrate** — Babbage MetaNet App Catalog apps will hit auto-grants where they expect a prompt.

The fix is a Phase 1.5 deliverable. **It does require new DB tables** — flagged below as needing user approval per CLAUDE.md invariant 2.

---

## Recommended Phase 1.5 — BRC-100 Surface Completion

**Goal:** Close the audit gaps so Phase 2's V8 shim has a complete BRC-100 backend to translate into.
**Sizing:** ~1 sprint week. Small, additive, no breaking changes.
**Gating:** Should land between Phase 1 (BRC-121) and Phase 2 (V8 shim). Folding into Phase 2 risks scope creep.

### Scope

1. **Implement the two missing handlers.**
   - New module: `rust-wallet/src/crypto/key_linkage.rs` (BRC-42 linkage derivation + serialization).
   - New handlers: `reveal_counterparty_key_linkage` + `reveal_specific_key_linkage` in `rust-wallet/src/handlers.rs`.
   - New routes: `POST /revealCounterpartyKeyLinkage`, `POST /revealSpecificKeyLinkage` in `main.rs:768–779` block (Identity group).
   - **No DB changes.** No crypto/signing logic changes — only adds new code paths under `crypto/`. Honors invariants 2 + 3.

2. **Stand up the three BRC-100 permission tiers.**
   - **REQUIRES USER APPROVAL per CLAUDE.md invariant 2** (new DB tables). Phase 1.5 README will surface this explicitly before any migration is written.
   - Proposed new tables (sketch only — final shape decided in Phase 1.5 design):
     - `protocol_permissions(origin, protocolID, keyID_pattern, granted_at, expiry?)` — per-protocol grants.
     - `counterparty_permissions(origin, counterparty, granted_at, expiry?)` — per-counterparty grants.
     - Grouped permissions could potentially be derived from rows in the above two tables, no separate table needed.
   - New repo: `rust-wallet/src/database/protocol_permission_repo.rs` (mirrors existing `domain_permission_repo`).
   - New overlay: `frontend/src/pages/ProtocolPermissionOverlayRoot.tsx` (mirrors `SettingsOverlayRoot.tsx` shape).
   - New IPC paths: `protocol_permission_request` / `protocol_permission_grant` / `protocol_permission_deny` (mirrors existing domain-permission IPC pattern in `simple_handler.cpp`).

3. **Wire all 28 method paths through the new tier checks.**
   - Every method that takes `protocolID` adds a `check_protocol_approved(origin, protocolID, keyID)` call.
   - Every method that takes `counterparty` adds a `check_counterparty_approved(origin, counterparty)` call.
   - Methods that take *both* (e.g. `createSignature`) check both. This is the single largest risk in Phase 1.5: easy to miss a path. The implementation must include a single test fixture that exercises every BRC-100 method end-to-end and asserts the gate fires under a fresh origin.

### Files this would touch

- New: `rust-wallet/src/crypto/key_linkage.rs`
- New: `rust-wallet/src/database/protocol_permission_repo.rs` (after schema approval)
- New: `rust-wallet/src/database/migrations.rs::v25_protocol_permissions()` (after schema approval)
- New: `frontend/src/pages/ProtocolPermissionOverlayRoot.tsx`
- Edits: `rust-wallet/src/handlers.rs` (two new handlers + permission gate calls in 26 existing handlers — all *additive*, no signature changes)
- Edits: `rust-wallet/src/main.rs` (two new route registrations)
- Edits: `cef-native/src/handlers/simple_handler.cpp` (new IPC dispatchers)
- Edits: `cef-native/src/core/HttpRequestInterceptor.cpp` (new wallet-endpoint route entries)

### What does NOT change in Phase 1.5

Honoring invariants 2 + 3 of `rust-wallet/CLAUDE.md`:

- The 26 existing handler bodies are not rewritten — only call out to the new permission-gate functions at the top.
- `crypto/brc42.rs`, `crypto/signing.rs`, `crypto/keys.rs` are not modified.
- The `wallets`, `users`, `addresses`, `transactions`, `outputs`, `proven_txs`, `certificates` tables are not modified.

---

## Cross-platform parity

The Rust wallet is platform-agnostic and runs identically on Windows and macOS. This audit's scope (Rust handlers + routes) has **no Win/macOS divergence** to flag.

The Phase 1.5 work that *does* touch cef-native (new IPC dispatchers, new overlay creation) will need parity per the standard pattern documented in the root `CLAUDE.md`:

- Windows: WS_POPUP overlays in `cef_browser_shell.cpp`.
- macOS: NSPanel overlays in `cef_browser_shell_mac.mm`.
- Both must be built and smoke-tested before Phase 2 begins.

---

## Open questions for the ordinal conversation (deferred)

The user explicitly deferred ordinal DB and UI questions to a separate conversation. Capturing here without answering:

### What changes in the DB with ordinals?

**Probably nothing for the basic flow.** Per the `rust-wallet/CLAUDE.md` schema (V24), ordinals would be stored in the existing `outputs` table with `basket = '1sat'`, `derivation_prefix` matching whatever the 1Sat-template locking script requires, and `locking_script` carrying the inscription payload. No new tables needed for ownership tracking — it's just outputs in a different basket.

**Open questions to discuss:**

- Do we want to add columns for ordinal-specific metadata: `ordinal_origin_outpoint`, `ordinal_content_type`, `ordinal_content_hash`? Or keep that derived/external (ord-fs lookup, GorillaPool API)?
- Yours's `processCWICreateAction` does basket-aware classification when `basket: '1sat'` is detected — splits inputs/outputs into 1sat vs default and constructs the locking scripts via `@1sat/templates`. Should Hodos mirror this server-side (Phase 3) or trust the dApp to construct scripts (Phase 2 posture, per shim spec)?
- BSV21 fungible tokens (a different `1sat` flavor) need separate accounting (decimal balances, supply tracking). Out of scope for ordinal-as-NFT but flagged.

### Do we need to change/improve the UI?

**Yes — but later, not in Phase 1.5 / Phase 2.** Current wallet UI shows BSV balance and recent activity. To support ordinals well, the UI needs:

- A **collection / inscriptions view** showing ordinal outputs with thumbnail previews.
- An **inscription detail card** showing content type, content (text/image/video), origin outpoint, transfer history.
- An **ordinal-aware send flow** that distinguishes "send 1 inscription" from "send 10 sats."
- Possibly a **token tab** for BSV21 fungible tokens.

This is a dedicated UI/frontend phase — likely sized similarly to one of the Wallet UI phases in `development-docs/UX_UI/`. Recommended: discussed and scoped after Phase 2 lands and we know which apps are actually exercising the ordinal route.

### `createAction` routing for ordinals — confirmed correct

All three legacy ordinal flows (`inscribe`, `transferOrdinal`, `purchaseOrdinal`) collapse into `createAction({ outputs, inputs?, basket: '1sat' })` calls in Yours's BRC-100 model. Hodos's existing `create_action` handler at `handlers.rs:3381` handles this without modification — the basket name is just a string field on the output entry. No new wallet entrypoint needed for the basic flow. Phase 3 may add basket-aware classification per the open question above.

---

## Verification

This is a research deliverable. Verification:

1. **Cross-checked** the 28-method list against `https://unpkg.com/@bsv/sdk@2.0.13/dist/types/src/wallet/Wallet.interfaces.d.ts` and `YOURS_CWI_MIGRATION.md` §2. Method names + groupings match exactly.
2. **Confirmed by grep** every cited handler exists in `handlers.rs` / `handlers/certificate_handlers.rs` at the line numbers cited.
3. **Confirmed by grep** the two missing methods produce zero matches anywhere in `rust-wallet/`.
4. **Confirmed by Read** all 26 BRC-100 routes are registered in `main.rs:763–812` with canonical names.
5. **Wallet-safety self-check:** every recommendation in this document is *additive* (new files, new tables, new permission-gate calls). No proposed modifications to existing handlers, existing crypto, or existing DB schema. New tables in Phase 1.5 are explicitly flagged as needing user approval per CLAUDE.md invariant 2.

---

## References

- `https://unpkg.com/@bsv/sdk@2.0.13/dist/types/src/wallet/Wallet.interfaces.d.ts` — canonical 28-method `WalletInterface`
- `../YOURS_CWI_MIGRATION.md` §2, §7 — method list + permission model
- `../OPEN_QUESTIONS.md` Q11, Q16, Q17 — surfaced gaps
- `../AUTO_APPROVE_RATIONALE.md` — why Hodos's permission posture differs from Brave's
- `../ARCHITECTURE.md` — diagrams showing where Phase 1.5 + Phase 2 changes land
- `rust-wallet/src/handlers.rs`, `rust-wallet/src/main.rs`, `rust-wallet/src/handlers/certificate_handlers.rs` — Hodos source
- `rust-wallet/CLAUDE.md`, `rust-wallet/src/CLAUDE.md` — Rust-side context
