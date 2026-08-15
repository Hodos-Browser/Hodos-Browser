# Phase 0.2 — `window.yours` / `window.panda` Shim Translation Spec

**Date:** 2026-05-06.
**Status:** Spec — gates Phase 2 implementation. Incorporates findings from the 2026-05-06 architecture-reviewer critique (see "Risks identified by review" at the end of this document).
**Scope:** `window.yours` + `window.panda` (alias) translation to the BRC-100 backend. Does NOT cover `window.CWI` (that's a canonical pass-through; specced in Phase 2 README).

> **Visual:** see `../ARCHITECTURE.md` for the diagram showing where this shim layer lands in the Hodos stack.

---

## Design posture (post-review)

The architecture-reviewer surfaced several risks that materially changed the design. Where the phase-0.2 README proposed an option, this spec adopts the position below — usually a stronger guardrail than the README's initial proposal. Each adopted decision is cross-referenced to the risk it resolves.

| Decision area | README proposal | This spec adopts | Resolves |
|---|---|---|---|
| `signMessage` security level | level 2 (forces prompt) | **level 1 (silent, anyone-verifiable)** | R1 |
| `signMessage` convention | unilateral Hodos string | **published as `yours-legacy-v1` shim convention with cross-wallet verifier helper** | R1 |
| `getAddresses` | Option A (identity-key P2PKH) | **Option C (BRC-42 fresh-address generator)** | R2 |
| Multi-recipient `encrypt` | reject | **fetch yours-wallet@v4.5.6 to confirm legacy semantics; reject only if vestigial; typed error either way** | R3 |
| `inscribe`/`transfer`/`purchase` ordinal methods | forward to bare `createAction` | **return typed `NOT_IMPLEMENTED_PRE_PHASE_3` error in v1** (Phase 3 may bundle templates) | R4 |
| Permission gate | "every path goes through" | **enumerated per-method routing diagram + unit test asserting `SessionManager` increment parity with canonical** | R5 |
| Auto-approve under shim | inherit domain settings | **disabled by default for shim paths regardless of whitelist; explicit prompt every time** | R6 |
| Multi-provider discovery | not addressed | **ship `bsv:announceProvider` CustomEvent alongside the shim (EIP-6963 equivalent)** | R7 |
| Deprecation/removal | informal | **feature flag, console warn, release-process milestone, opt-in telemetry** | R8 |
| Re-entrancy + arg overflow | not addressed | **per-frame mutex on legacy methods; cap data length at backend max minus margin; typed errors** | R9 |

---

## Cross-platform parity

The shim itself is V8/JS — runs identically across Windows and macOS. The injection plumbing is platform-dependent:

- **Windows:** V8 inject in `cef-native/src/handlers/simple_render_process_handler.cpp::OnContextCreated()`; IPC dispatch in `cef-native/src/handlers/simple_handler.cpp` (Windows path).
- **macOS:** Same `simple_render_process_handler.cpp` is used (CEF render process is platform-agnostic on the C++ side), but overlay creation for any prompt flow goes through `cef_browser_shell_mac.mm` instead of `cef_browser_shell.cpp`.

**Implication:** Phase 2 implementation must build and smoke-test on both platforms before merging. The Phase 2 acceptance criteria should include "Treechat login works on both Windows and macOS builds against the auth-category test sites listed in root `CLAUDE.md`."

---

## Surface to translate

Per `YOURS_CWI_MIGRATION.md` §3 + the FACT_CHECK_RESULTS.md Q2 enumeration of methods Treechat actually calls. Total: 17 entries in the legacy provider object (including `isReady` flag). All but `isReady` translate to BRC-100 backend calls or typed errors.

| Legacy method | Treechat? | Status in this spec |
|---|---|---|
| `isReady` | yes (boolean flag) | local property, no backend |
| `connect()` | yes | translate |
| `isConnected()` | yes | translate |
| `disconnect()` | no | translate |
| `getAddresses()` | yes | translate (fresh-address gen) |
| `getBalance()` | yes | translate |
| `getPubKeys()` | no | translate (mapped to multiple `getPublicKey` calls) |
| `signMessage(opts)` | yes | translate (silent, level 1, doc'd convention) |
| `getSignatures(opts)` | no | translate (mapped to `signAction` reference) |
| `sendBsv(payments)` | no (stubbed by Treechat) | translate |
| `broadcast(opts)` | no | translate (best-effort) |
| `encrypt(opts)` | no | translate (with multi-recipient verification — see below) |
| `decrypt(opts)` | no | translate |
| `getExchangeRate()` | no | typed error (`YOURS_LEGACY_REMOVED`) |
| `getSocialProfile()` | no | typed error |
| `inscribe(items)` | no | typed error (`NOT_IMPLEMENTED_PRE_PHASE_3`) |
| `transferOrdinal(opts)` | no | typed error |
| `purchaseOrdinal(opts)` | no | typed error |

`window.panda` is exposed as a property alias to the same object — no separate code path.

---

## Per-method translation

Each block follows: **Backend call → Translation rule → Edge cases → Lossy? → Verifier interop note (where relevant).**

### `isReady` (boolean property)

**Backend call:** none — local property.
**Translation rule:** Returns `true` once the V8 context is initialized and the IPC channel is up. Set after `OnContextCreated` finishes wiring `window.cefMessage`.
**Lossy?** No.

---

### `connect()`

**Backend call:** `waitForAuthentication({})`.
**Translation rule:** If the wallet is unauthenticated, this opens the existing connect/auth flow (overlay subprocess; same path the canonical CWI uses). On approval, returns `{ identityKey: <hex>, addresses: { ... } }`-shaped legacy response, derived from `getPublicKey({ identityKey: true })` + the new fresh-address generator (see `getAddresses`).
**Edge cases:** If the user denies, throw a typed `YOURS_LEGACY_USER_DENIED` error. If the wallet is locked, throw `YOURS_LEGACY_WALLET_LOCKED`.
**Lossy?** No (the legacy shape is built from canonical responses).

---

### `isConnected()`

**Backend call:** `isAuthenticated({})`.
**Translation rule:** Returns the boolean from `{ authenticated }`. **Permission-gate note:** this is a read-only call but **must still pass the `check_domain_approved` gate** — an unapproved domain calling `isConnected()` to fingerprint the user's wallet state is a real privacy concern. (Resolves R5.)
**Lossy?** No.

---

### `disconnect()`

**Backend call:** internal `domain_permission_revoke(origin)` — the same path the user clicks in the wallet panel's "Connected sites" UI.
**Translation rule:** Removes the origin from the domain-permission table; future calls require re-authorization.
**Lossy?** No.

---

### `getAddresses()` — **CHANGED from README**

**Backend call:** Hodos-specific: `address_repo::generate_legacy_receive_address(origin)`.
**Translation rule:** Generate a fresh BRC-42-derived P2PKH address using **protocolID `[2, "yours-legacy-receive"]`, keyID = monotonic counter stored in `address_repo` keyed on origin**. Stable across sessions for the same origin. Documented as the `yours-legacy-v1` receive convention so other shim implementers can recover funds.
**Edge cases:** Returns three addresses to match the legacy shape — `{ bsvAddress, ordAddress, identityAddress }`. The first two come from the new generator with different protocolID strings (`yours-legacy-receive` and `yours-legacy-ord-receive`); only `identityAddress` is derived from `getPublicKey({ identityKey: true })`. **The receive addresses must NOT be the identity-key address** — sending funds there pollutes identity-key UTXOs and creates fund-loss + privacy risk. (Resolves R2.)
**Lossy?** Yes-ish — the legacy method had no notion of per-origin separation; we add it for safety. Document as "addresses are isolated per-origin in Hodos by design."
**Verifier interop note:** Other shim implementers should adopt the same convention so a user's funds are recoverable across shims.

> **Phase 1.5 dependency:** This requires `address_repo` extensions that may need a new column (`legacy_origin TEXT NULLABLE`). Fold into Phase 1.5 schema review per CLAUDE.md invariant 2.

---

### `getBalance()`

**Backend call:** `listOutputs({ basket: 'default' })` then sum `outputs[].satoshis`.
**Translation rule:** Match the legacy `{ bsv: number, satoshis: number, usdInCents: number }` shape. Pull USD via the existing `price_cache.rs`. Cache the sum at the shim layer for 5 seconds to avoid hammering the wallet for repeated reads.
**Edge cases:** If the wallet is locked or syncing, return `null` for `bsv`/`satoshis` rather than `0` (legacy clients distinguish "unknown" from "empty").
**Lossy?** No.

---

### `getPubKeys()`

**Backend call:** Multiple `getPublicKey(...)` calls.
**Translation rule:** Returns the legacy `{ bsvPubKey, ordPubKey, identityPubKey }` triple. `identityPubKey` ← `getPublicKey({ identityKey: true })`. `bsvPubKey` and `ordPubKey` are derived via the same `yours-legacy-receive` / `yours-legacy-ord-receive` invoice conventions used by `getAddresses`.
**Lossy?** No (just three canonical calls instead of one bulk legacy call).

---

### `signMessage({ message, encoding })` — **CHANGED from README**

**Backend call:** `createSignature({ data, protocolID: [1, "yours-legacy-message"], keyID: "1", counterparty: "anyone" })`.
**Translation rule:**
- Security level **1** (silent — no prompt). The README originally proposed level 2 but level 2 forces a permission popup that the legacy `signMessage` UX never had. Treechat's silent-login flow would break under level 2.
- Encoding handling: `encoding === "utf8"` → UTF-8 encode the string to bytes. `encoding === "hex"` → hex-decode. `encoding === "base64"` → base64-decode. Default: `"utf8"`. Document: signatures over the same logical message under different encodings WILL diverge — the spec defines `encoding: "utf8"` as the canonical form and clients omitting `encoding` get utf8 implicitly.
- Counterparty `"anyone"` makes signatures verifiable by any holder of the identity pubkey, matching legacy semantics.
**Edge cases:**
- Cap `message.length` at backend's `data` max minus a 64-byte margin. Reject overlength messages with typed `YOURS_LEGACY_MESSAGE_TOO_LONG` error rather than silently truncating. (Resolves R9b.)
- Re-entrancy: per-frame mutex on `signMessage` calls. If a `signMessage` is in-flight (waiting on `createAction` callback), reject the new call with `YOURS_LEGACY_REENTRANT`. (Resolves R9a.)
**Lossy?** Conceptually no, but BRC-100 signatures bind to (protocolID, keyID, counterparty), so verifying these signatures requires the verifier to know the convention.
**Verifier interop note:** Convention published as `yours-legacy-v1`. Hodos ships a `verifyLegacyMessage(message, signature, identityKey)` helper in the shim layer for browser-side use, plus a documented JS snippet for server-side verifiers (using `@bsv/sdk` `verifySignature` under the same convention). Recommended action: **publish as a BRC draft before Phase 2 ships** so other shims interop. (Resolves R1.)

---

### `getSignatures({ rawtx, sigRequests })`

**Backend call:** `signAction({ reference, spends })`.
**Translation rule:** The legacy method takes a partial raw tx + a list of `(inputIndex, derivation, sighashType)`. The shim must first call `createAction({ outputs, inputs, lockTime, version, ... })` to get a `signableTransaction` reference, then `signAction({ reference, spends: { inputIndex: { unlockingScript, sequenceNumber } } })`. The `unlockingScript` is computed by the wallet using the requested derivation paths.
**Edge cases:** If any sigRequest references a derivation the wallet can't perform, return a typed `YOURS_LEGACY_DERIVATION_NOT_AVAILABLE` error.
**Lossy?** Mildly — the legacy method exposed sighash flags directly; BRC-100 uses sighash policy embedded in the action. Map the legacy SIGHASH_ALL+SIGHASH_FORKID to BRC-100 default; reject other flags with a typed error.

---

### `sendBsv(payments[])`

**Backend call:** `createAction({ outputs: payments.map(toOutput) })`.
**Translation rule:** Each `PaymentObject` (`{ to, amount, currency? }`) becomes an output (`{ satoshis, lockingScript: P2PKH(to) }`). If `currency === "BSV"`, multiply by 1e8 to get satoshis. If `currency` is absent or "satoshis", use `amount` directly.
**Edge cases:**
- Inscription-bearing payments (legacy `payments[i].inscription` field): return typed `NOT_IMPLEMENTED_PRE_PHASE_3` error. **Do not silently forward to `createAction`** — the inscription script construction is dApp-side in BRC-100 and silent forwarding produces malformed scripts. (Resolves R4.)
- The Hodos service-fee output (`HODOS_SERVICE_FEE_SATS` to `HODOS_FEE_ADDRESS`, see root `CLAUDE.md`) is added by `create_action_internal` automatically — the shim does not need to manage this.
**Lossy?** No for plain BSV sends. Yes for inscription-bearing sends (rejected with explicit error).

---

### `broadcast({ rawtx })`

**Backend call:** Best-effort `internalizeAction({ tx: rawtx, ... })` if the rawtx contains outputs claimable by the wallet; otherwise, raw broadcast via `wallet/broadcast` endpoint.
**Translation rule:** Try `internalizeAction` first. If the wallet can't classify any output as its own, fall back to the existing raw-broadcast path used by the wallet panel.
**Edge cases:** Legacy `broadcast` returned `{ txid, message? }`; preserve that shape. On internalize failure, surface the underlying error.
**Lossy?** No.

---

### `encrypt({ message, encoding, pubKeys })` — **CHANGED from README**

**Backend call:** `encrypt({ plaintext, protocolID: [1, "yours-legacy-encrypt"], keyID: "1", counterparty: pubKeys[0] })`.
**Translation rule:**
- Security level 1 (silent), matching `signMessage`.
- For single-recipient (`pubKeys.length === 1`), translate directly. counterparty = `pubKeys[0]`.
- For multi-recipient (`pubKeys.length > 1`): **action depends on what the legacy backend actually did.** Before Phase 2 ships, fetch `yours-wallet@v4.5.6` source and inspect the actual implementation. (Resolves R3.)
  - If legacy was vestigial (only ever invoked with one pubKey): reject with typed `YOURS_LEGACY_MULTI_RECIPIENT_UNSUPPORTED` error.
  - If legacy used a real semantic (e.g., a single ECDH-derived key with multiple recipients as additional data): replicate that semantic. Document the convention.
- Encoding: same as `signMessage`. Output ciphertext base64-encoded.
**Edge cases:** counterparty pubkey validation — reject malformed pubkeys with typed error before hitting the backend.
**Lossy?** TBD pending v4.5.6 source inspection.
**Verifier interop note:** decrypt-side must use the same convention. Document in the `yours-legacy-v1` BRC draft.

---

### `decrypt({ message, encoding, pubKeys })`

**Backend call:** `decrypt({ ciphertext, protocolID: [1, "yours-legacy-encrypt"], keyID: "1", counterparty: pubKeys[0] })`.
**Translation rule:** Mirror `encrypt` translation. counterparty = the *sender's* pubkey (the encryptor), passed in as `pubKeys[0]` per legacy convention.
**Edge cases:** Same as `encrypt`. If the ciphertext was produced by a different shim using a different convention, decryption fails with `YOURS_LEGACY_DECRYPT_FAILED`.

---

### `getExchangeRate()` — REMOVED

**Action:** Throw typed `YOURS_LEGACY_REMOVED` error with explanation pointing to BRC-100: dApps should fetch BSV/USD from a public price source themselves (e.g., CryptoCompare). Console-warn on first call.

---

### `getSocialProfile()` — REMOVED

**Action:** Throw typed `YOURS_LEGACY_REMOVED` error. Phase 4 `Sigma OAuth` demo can show the BRC-100 alternative for identity profile fetch.

---

### `inscribe(items)` / `transferOrdinal(opts)` / `purchaseOrdinal(opts)` — DEFERRED to Phase 3

**Action:** Throw typed `NOT_IMPLEMENTED_PRE_PHASE_3` error in v1. **Do NOT silently forward to `createAction`** — the dApp ships no template code, so silent forwarding produces malformed transactions that fail validation with confusing error messages. (Resolves R4.)
**Phase 3 revisit:** Decide whether to bundle `@1sat/templates` (or an equivalent) inside the shim and synthesize the locking scripts wallet-side. Recommended: yes, because legacy callers shipped no template code. Discussion deferred to Phase 3 design.

---

## Permission gate routing

Every `window.yours` / `window.panda` method MUST enter the BRC-100 backend via the same IPC dispatch the canonical `window.CWI` uses, with `originator` derived from the V8 frame's `window.location.host`. **No internal fast paths.** Composite calls (e.g., `signMessage` → `createSignature`) must re-enter the IPC, not call the Rust handler directly. (Resolves R5.)

The full dispatch:

```
window.yours.signMessage(opts)
  → window.cefMessage.send('yours_legacy', ['signMessage', opts])      [V8 → C++ IPC]
  → simple_handler::OnProcessMessageReceived (Windows + macOS, same path)
  → translate → IPC dispatch as if a canonical CWI call
  → window.cefMessage.send('cwi_call', ['createSignature', { ... translated ... }])
  → HttpRequestInterceptor::OnBeforeResourceLoad
  → check_domain_approved(origin)         [C++ side, defense-in-depth]
  → check_protocol_approved(origin, protocolID, keyID)   [Phase 1.5: per-protocol gate]
  → forward to localhost:31301/createSignature
  → handlers::create_signature
  → check_domain_approved (Rust side, second defense-in-depth)
  → execute, return
```

Every read-only legacy method (`isConnected`, `getAddresses`, `getBalance`, `getPubKeys`) follows this same path — no shortcuts that bypass `check_domain_approved`. Domain-fingerprinting via "harmless" reads is a real privacy concern; spec mandates the gate fires for every call.

### SessionManager parity

`SessionManager` (in `cef-native/include/core/SessionManager.h`) tracks per-tab spending and call counts. The shim must increment the same counter as the canonical method:

| Legacy method | Counter incremented |
|---|---|
| `signMessage` | same as `createSignature` |
| `sendBsv` | same as `createAction` |
| `getSignatures` | same as `signAction` |
| `encrypt` | same as `encrypt` |
| `decrypt` | same as `decrypt` |
| `broadcast` | same as `internalizeAction` |
| `getAddresses` / `getBalance` / `isConnected` | shared "read" bucket |

**SessionManager keys are scoped to operation type, not method name.** This prevents an attacker from splitting spend across `window.CWI.createAction` and `window.yours.sendBsv` to launder the rate limit. (Resolves R6a.)

### Acceptance test

Phase 2 implementation must include a unit test (location: `cef-native/tests/test_shim_session_parity.cpp` or equivalent) that asserts:

- For each legacy method, calling it under a fresh origin increments the same `SessionManager` counter as its canonical equivalent.
- For each legacy method, calling it under an unapproved origin triggers a permission prompt (or rejection) before reaching the Rust backend.
- **Every successful payment routed through a shim method (`window.yours.sendBsv`, `window.panda.sendBsv`, etc.) fires the `payment_success_indicator` IPC** that drives the tab payment badge animation (chain: `HttpRequestInterceptor.cpp:1656-1681` → `simple_render_process_handler.cpp:1020` → `useTabManager.ts:141`). This is non-negotiable — every payment leaving the wallet, regardless of which surface initiated it, MUST trigger the green-dot animation so users never miss an outgoing payment.

## Auto-approve under the shim

Hodos's 3-layer auto-approve (per `AUTO_APPROVE_RATIONALE.md`) applies to canonical BRC-100 calls. **For shim paths, auto-approve is OFF by default regardless of domain whitelist status.** (Resolves R6b.)

Rationale: Legacy callers are by definition not actively maintained. Users likely cannot distinguish a legitimate Treechat call from a malicious page mimicking it. Concentrating auto-approve trust in unmaintained code is exactly what Brave's "no auto-approve" stance exists to prevent.

User-facing behavior: every shim call that would have been auto-approved on the canonical surface gets a prompt instead. The prompt explicitly notes "this is a legacy `window.yours` call from <origin>" so users understand why they're seeing a prompt where they don't on canonical sites.

Settings knob (Phase 2 deliverable): `Settings → Privacy → Allow auto-approve for legacy provider calls` (default OFF, can be enabled per-domain by power users).

---

## Multi-provider discovery (`bsv:announceProvider`)

Hodos injects three globals: `window.CWI`, `window.yours`, `window.panda`. Yours v5+ also injects `window.CWI`. Babbage MetaNet Client also injects `window.CWI`. Without coordination, dApps are stuck with first-write-wins on unguarded globals. (Resolves R7.)

**Spec:** Hodos ships a `bsv:announceProvider` CustomEvent contract analogous to EIP-6963.

```js
window.dispatchEvent(new CustomEvent('bsv:announceProvider', {
  detail: {
    info: {
      uuid: '<random per-page>',
      name: 'Hodos',
      icon: 'data:image/svg+xml;base64,...',
      rdns: 'browser.hodos',
    },
    provider: window.CWI,  // canonical; window.yours/panda not announced
  },
}));
```

dApps listen for `bsv:announceProvider` events and present a chooser when multiple providers respond. Hodos announces only on first call to `bsv:requestProvider`, matching EIP-6963 cadence.

**Property descriptors** (per `BRAVE_WALLET_REFERENCE.md`):
- `window.CWI` — non-writable, non-configurable. Other extensions cannot overwrite.
- `window.yours`, `window.panda` — writable. Other wallets can override if user prefers.

**Recommended action:** publish `bsv:announceProvider` as a BRC draft for ecosystem agreement; do NOT wait for the BRC to land before shipping (the shim is the forcing function).

---

## Lifetime, deprecation, and removal

Useful lifetime: months, not years (per `phase-0-research/FACT_CHECK_RESULTS.md` Q1). Without a removal mechanism this becomes dead-weight maintenance. (Resolves R8.)

**Mechanisms baked in from v1:**

1. **Feature flag:** `hodos.legacy_shim_enabled` in settings. Default ON for Hodos v1 with shim. Default OFF in a Hodos vN release ~6 months after Yours v5 ships and Treechat-class apps have migrated. Removable in vN+1.
2. **Console warning:** every legacy invocation logs `[Hodos] window.yours.signMessage is deprecated. Please migrate to window.CWI. See <docs URL>.` Pushes ecosystem migration while shim is still supported.
3. **Opt-in telemetry:** anonymous counter for `window.yours.*` invocations per origin, gated on user opt-in. Gives the team data on when removal is safe. Defaults OFF; user-facing toggle in Settings → Privacy.
4. **Release-process milestone:** add a checklist item to the release-process doc: "On Hodos vN, default `hodos.legacy_shim_enabled = false`. On Hodos vN+1, remove shim code." This survives sprint-doc archival.

---

## Lifecycle: re-entrancy and overflow

(R9 mitigations.)

- **Per-frame mutex.** Each V8 frame has at most one in-flight legacy call at a time. New calls while one is pending reject with typed `YOURS_LEGACY_REENTRANT` error. Implementation: `WeakMap<Frame, Promise>` in the shim layer.
- **Argument size caps.** `signMessage` / `encrypt` / `decrypt` cap `message`/`plaintext`/`ciphertext` length at the BRC-100 backend's max minus 64-byte margin. Reject overlength input with typed `YOURS_LEGACY_DATA_TOO_LARGE` error rather than silently truncating. Truncation could result in signing/encrypting different content than the user thinks they consented to — a phishing primitive.

---

## Open Phase 1.5 dependencies

This spec assumes Phase 1.5 (BRC-100 Surface Completion — see `../phase-0.1-brc100-audit/AUDIT_RESULTS.md`) lands before Phase 2 begins. Specifically:

- **`address_repo` extension** for the legacy fresh-address generator (new `legacy_origin` column or equivalent) — needs user approval per CLAUDE.md invariant 2.
- **Per-protocol permission gate** (`check_protocol_approved`) — used in the dispatch chain.
- **Per-counterparty permission gate** (`check_counterparty_approved`) — used for shim methods like `encrypt`/`decrypt` that take a counterparty.

If Phase 1.5 is descoped or split, this spec needs revision.

---

## Risks identified by review

The architecture-reviewer agent (2026-05-06 read-only critique) surfaced 9 structural risks. The 9 risks below have all been addressed in the spec above; this section preserves them as the as-found audit trail.

### R1 — `signMessage` convention is unilateral and lacks verifier strategy. **High.**
Adopted: level 1 not level 2; convention published as `yours-legacy-v1`; ship `verifyLegacyMessage` helper; document encoding handling.

### R2 — `getAddresses` Option A pollutes identity-key UTXOs. **High.**
Adopted: switched to Option C (BRC-42 fresh-address generator with `[2, "yours-legacy-receive"]` and per-origin monotonic keyID).

### R3 — Multi-recipient `encrypt` rejection is a guess. **Medium.**
Adopted: fetch `yours-wallet@v4.5.6` source before Phase 2 finalizes; reject with typed error if vestigial; replicate semantic if real.

### R4 — Forwarding ordinal methods to bare `createAction` will produce malformed transactions. **High.**
Adopted: typed `NOT_IMPLEMENTED_PRE_PHASE_3` error in v1; bundling templates is a Phase 3 decision.

### R5 — Permission gate has plausible bypass paths in the proposed routing. **High.**
Adopted: enumerated per-method routing diagram; mandate same IPC entry as canonical; acceptance test asserting `SessionManager` counter parity.

### R6 — Shim widens auto-approve attack surface. **High.**
Adopted: SessionManager keyed on operation type not method name; auto-approve OFF by default for shim paths regardless of whitelist; user-visible setting.

### R7 — No multi-provider discovery scheme. **Medium.**
Adopted: ship `bsv:announceProvider` CustomEvent contract alongside the shim; non-writable `window.CWI`, writable `window.yours`/`panda`; publish as BRC draft.

### R8 — No deprecation/removal mechanism. **Medium.**
Adopted: feature flag, console warning, opt-in telemetry, release-process milestone.

### R9 — Re-entrancy + argument overflow. **Medium.**
Adopted: per-frame mutex; data-length caps with typed errors.

### Reviewer's top 3 priority changes — all adopted

1. ✅ Replace `getAddresses` Option A with fresh-address generator (Option C).
2. ✅ Legacy ordinal methods return typed errors, not silent forwards.
3. ✅ Auto-approve disabled by default for shim paths.

---

## Verification

Research deliverable. Verification path:

1. **Cross-check vs comparison table** — every row in `YOURS_CWI_MIGRATION.md` §3 has a corresponding block above. ✓
2. **Reviewer findings fully integrated** — all 9 risks addressed in the design (see decision posture table at the top). ✓
3. **Phase 1.5 dependency callouts** — explicit, surfaced for user approval where DB schema is involved. ✓
4. **Wallet-safety self-check** — every backend call routes through existing handlers. The shim does NOT propose modifying any of the 26 implemented BRC-100 methods, modifying the wallet DB schema, or modifying crypto/signing logic. The fresh-address generator (R2) requires `address_repo` extension, flagged for user approval. ✓
5. **Cross-platform parity** — flagged in dedicated section; Phase 2 acceptance criteria must include both Windows and macOS smoke tests. ✓

---

## References

- `../YOURS_CWI_MIGRATION.md` §3 — comparison table
- `../BRAVE_WALLET_REFERENCE.md` — V8 Proxy / property descriptor patterns; EIP-6963 motivation
- `../AUTO_APPROVE_RATIONALE.md` — Hodos's 3-layer model and why it diverges from Brave
- `../phase-0.1-brc100-audit/AUDIT_RESULTS.md` — handler coverage; Phase 1.5 dependencies
- `../phase-0-research/FACT_CHECK_RESULTS.md` Q1, Q2 — Yours migration timeline; Treechat surface
- `../OPEN_QUESTIONS.md` Q11, Q14, Q15, Q18 — answered by adoptions in this spec
- `https://yours-wallet.gitbook.io/provider-api` — legacy `window.yours` reference
- 2026-05-06 architecture-reviewer critique — see "Risks identified by review" above
