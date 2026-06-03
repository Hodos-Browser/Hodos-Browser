# Canary A1 — Wallet API Compatibility Report

**Date:** 2026-05-29
**Scope:** Can Hodos's Rust wallet (`localhost:31301`) serve every call Dolphin Milk's agent makes (today against `bsv-wallet-cli` on `localhost:3322`)?
**Method:** Static analysis of both Rust codebases. No services started, no live calls made.

---

## Methodology

Read Dolphin Milk's wallet abstraction trait (`src/wallet/mod.rs`) and its single HTTP implementation (`src/wallet/http.rs`) to enumerate every endpoint, request shape, and response shape the agent expects. Cross-referenced against Hodos's route table (`rust-wallet/src/main.rs` lines 750-957) and the request/response structs / handler bodies in `rust-wallet/src/handlers.rs` and `rust-wallet/src/handlers/certificate_handlers.rs`. Sampled the on-wire shape of every endpoint pair (~35 endpoints) and verified domain-permission gating behaviour against the `X-Requesting-Domain` header check at `handlers.rs:572-612`. The embedded backend at `src/wallet/embedded.rs` was acknowledged but is not on the integration path (Hodos integration uses HTTP — confirmed by `WalletClient = HttpWalletClient` at `src/wallet/mod.rs:19`). No live tests were run.

---

## Findings — TL;DR

- **Yellow-light, leaning green.** ~95% of the wire surface drops in. Endpoint coverage is essentially complete (every method Dolphin Milk calls exists on Hodos with a compatible JSON shape).
- **Three concrete blockers** between you and a working drop-in: (1) Hodos REJECTS `basket: "default"` in `listOutputs` — Dolphin Milk's `get_balance()` calls it exactly that way; (2) four status endpoints are POST-only on Hodos but Dolphin Milk calls them as GET (`getHeight`, `getNetwork`, `isAuthenticated`, `waitForAuthentication`); (3) `internalizeAction` response shape mismatch — Dolphin Milk's `fund_from_woc` checks `result.accepted == true`, Hodos returns `{txid, status: "unconfirmed"}` with no `accepted` field.
- **One biggest gap:** the `basket: "default"` rejection. That single line at `handlers.rs:15141-15145` will fail Dolphin Milk's balance check on the very first agent boot. Fix is one line of Rust (allow `default` OR map it to Hodos's actual default basket name).

---

## Dolphin Milk's wallet API expectations

All calls flow through `HttpWalletClient` (`Dolphin_Milk/rust-bsv-worm/src/wallet/http.rs`). HTTP method is POST with JSON body for everything except status reads (`call_get` → GET, no body). Auth headers: `Origin: http://localhost` (configurable, set on every request, line 105) and `Content-Type: application/json`. No bearer, no BRC-31 / BRC-103 handshake initiated by the client itself.

| Endpoint | Method | Request shape | Response shape | Auth | Source location |
|---|---|---|---|---|---|
| `/getVersion` | GET (call_get) | — | `{version}` | Origin | http.rs:555 |
| `/getPublicKey` | POST | `{identityKey: true}` OR `{protocolID, keyID, counterparty, forSelf}` | `{publicKey: hex}` | Origin | http.rs:225, 237 |
| `/createSignature` | POST | `{data: u8[], protocolID, keyID, counterparty}` | `{signature: u8[]}` | Origin | http.rs:263 |
| `/verifySignature` | POST | `{data, signature, protocolID, keyID, counterparty}` | `{valid: bool}` | Origin | http.rs:572 |
| `/createHmac` | POST | `{data: u8[], protocolID, keyID, counterparty}` | `{hmac: u8[]}` | Origin | http.rs:643 |
| `/verifyHmac` | POST | `{data, hmac, protocolID, keyID, counterparty}` | `{valid: bool}` | Origin | http.rs:665 |
| `/encrypt` | POST | `{plaintext: u8[], protocolID, keyID, counterparty}` | `{ciphertext: u8[]}` | Origin | http.rs:599 |
| `/decrypt` | POST | `{ciphertext: u8[], protocolID, keyID, counterparty}` | `{plaintext: u8[]}` | Origin | http.rs:621 |
| `/createAction` | POST | `{description, outputs[], options{acceptDelayedBroadcast, randomizeOutputs, trustSelf?}, inputs?[{outpoint: "txid.vout", inputDescription, unlockingScriptLength}]}` | `{txid, tx: u8[], ...}` | Origin | http.rs:302, 353 |
| `/signAction` | POST | `{reference}` | (raw) | Origin | http.rs:708 |
| `/abortAction` | POST | `{reference}` | (raw) | Origin | http.rs:714 |
| `/internalizeAction` | POST | `{tx: u8[], outputs[{outputIndex, protocol: "wallet payment", paymentRemittance{derivationPrefix, derivationSuffix, senderIdentityKey}}], description}` | `{accepted: bool, ...}` | Origin | http.rs:405, 1075 |
| `/listOutputs` | POST | `{basket: "default", include: "locking scripts", limit, offset}` | `{outputs[{spendable, satoshis, ...}]}` | Origin | http.rs:436, 474 |
| `/relinquishOutput` | POST | `{basket, output: {txid, vout}}` | (raw) | Origin | http.rs:728 |
| `/listActions` | POST | `{labels[], labelQueryMode: "any", includeLabels, includeInputs, includeOutputs, limit, offset}` | (raw) | Origin | http.rs:494 |
| `/isAuthenticated` | GET (call_get) | — | (raw) | Origin | http.rs:531 |
| `/waitForAuthentication` | GET (call_get) | — | (raw) | Origin | http.rs:565 |
| `/getHeight` | GET (call_get) | — | `{height: u64}` | Origin | http.rs:536 |
| `/getNetwork` | GET (call_get) | — | `{network: str}` | Origin | http.rs:545 |
| `/getHeaderForHeight` | POST | `{height}` | `{header: hex}` | Origin | http.rs:694 |
| `/acquireCertificate` | POST | cert JSON spread at top: `{certificateType, type, subject, certifier, serialNumber, revocationOutpoint, signature, fields, acquisitionProtocol: "direct"}` | (raw) | Origin | http.rs:745; lifecycle.rs:208, 290 |
| `/listCertificates` | POST | `{certifiers[], types[], limit, offset}` | (raw) | Origin | http.rs:750 |
| `/proveCertificate` | POST | `{certificate, fieldsToReveal[]}` | (raw) | Origin | http.rs:770 |
| `/relinquishCertificate` | POST | `{certificateType, serialNumber, certifier}` | (raw, may be empty) | Origin | http.rs:788 |
| `/discoverByIdentityKey` | POST | `{identityKey, limit, type?}` | (raw) | Origin | http.rs:817 |
| `/discoverByAttributes` | POST | `{attributes, limit, type?}` | (raw) | Origin | http.rs:831 |
| `/revealCounterpartyKeyLinkage` | POST | `{counterparty, verifier, privileged}` | (raw) | Origin | http.rs:847 |
| `/revealSpecificKeyLinkage` | POST | `{counterparty, verifier, protocolID, keyID, privileged}` | (raw) | Origin | http.rs:865 |
| (any) `raw_call(method, params)` | POST or GET | passthrough | passthrough | Origin | http.rs:521 — escape hatch for `wallet_call` tool |

Funding helpers (`receive_address`, `fund_from_woc`) are built ON TOP of the above — they don't introduce new endpoints; they call `getPublicKey` + `internalizeAction` and pull BEEF straight from WhatsOnChain.

---

## Hodos wallet's available endpoints

Route table at `rust-wallet/src/main.rs:785-957`. Listener: `127.0.0.1:31301`. All endpoints accept JSON.

| Endpoint | Method | Request shape | Response shape | Auth | main.rs line |
|---|---|---|---|---|---|
| `/getVersion` | **POST + GET** | empty | `{version, capabilities[], brc100, timestamp}` | none | 790-791 |
| `/getPublicKey` | POST | `{identityKey?, protocolID?, keyID?, counterparty?, forSelf?}` | `{publicKey: hex}` | identity-key gate via `X-Requesting-Domain` + `X-Identity-Key-Approved` (handlers.rs:307-352) | 792 |
| `/createSignature` | POST | `{data\|hashToDirectlySign, protocolID, keyID, counterparty}` | `{signature: u8[]}` (DER) | `check_domain_approved` | 800 |
| `/verifySignature` | POST | `{data, signature, protocolID, keyID, counterparty}` | `{valid: bool}` | (none on this gate) | 799 |
| `/createHmac` | POST | `{data, protocolID, keyID, counterparty}` | `{hmac: u8[]}` | `check_domain_approved` | 795 |
| `/verifyHmac` | POST | `{data, hmac, protocolID, keyID, counterparty}` | `{valid: bool}` | none | 796 |
| `/encrypt` | POST | `{plaintext, protocolID, keyID, counterparty?}` | `{ciphertext: u8[]}` (BRC-2 IV+CT+TAG) | `check_domain_approved` | 797 |
| `/decrypt` | POST | `{ciphertext, protocolID, keyID, counterparty?}` | `{plaintext: u8[]}` | `check_domain_approved` | 798 |
| `/createAction` | POST | `{inputs?[{outpoint: STRING\|OBJECT, ...}], outputs[{satoshis, script\|lockingScript, address?, customInstructions?, basket?, tags?}], description?, labels?, options{signAndProcess?, acceptDelayedBroadcast?, returnTXIDOnly?, noSend?, randomizeOutputs?, sendMax?, sendWith?}, inputBEEF?}` | `{reference, version, lockTime, inputs[], outputs[], txid, tx: u8[] (Atomic BEEF), signableTransaction?, sendWithResults?, noSendChange?}` | `check_domain_approved` + per-tx USD limit (handlers.rs:4108-4147) | 805 |
| `/signAction` | POST | `{reference, spends?, options?}` | `{txid, rawTx, unsignedInputs?}` | none | 811 |
| `/abortAction` | POST | `{reference}` | `{status, ...}` | none | 817 |
| `/internalizeAction` | POST | `{tx: u8[]\|string, outputs?[{outputIndex, protocol, paymentRemittance{senderIdentityKey, derivationPrefix, derivationSuffix}, insertionRemittance?}], description?, labels?}` | **`{txid, status: "unconfirmed"}`** | none | 819 |
| `/listOutputs` | POST | `{basket, include?, limit?, offset?, tags?, tagQueryMode?, includeCustomInstructions?, includeTags?, includeLabels?, includeOutputDescription?}` | `{totalOutputs, outputs[{outpoint: "txid.vout", satoshis, spendable, lockingScript?, ...}], BEEF?}` | **REJECTS `basket: "default"` (handlers.rs:15141)** | 821 |
| `/relinquishOutput` | POST | `{basket, output: {txid, vout}}` | (raw) | none | 822 |
| `/listActions` | POST | `{labels?, labelQueryMode?, includeLabels?, includeInputs?, includeOutputs?, limit?, offset?}` | `{totalActions, actions[]}` | none | 818 |
| `/isAuthenticated` | **POST only** | empty | `{authenticated: true}` | none | 793 |
| `/waitForAuthentication` | **POST only** | empty | `{authenticated: bool, error?}` | none | 794 |
| `/getHeight` | **POST only** | empty | `{height: u32}` | none | 826 |
| `/getNetwork` | **POST only** | empty | `{network: "main"\|"test"}` | none | 828 |
| `/getHeaderForHeight` | POST | `{height}` | `{header: hex}` | none | 827 |
| `/acquireCertificate` | POST | `{acquisitionProtocol?, type\|certificateType-via-`certificate_handlers.rs:679`-?, certifier, fields, serialNumber, revocationOutpoint, signature, keyringForSubject, subject?, certifierUrl?}` | `{certificate: JSON}` | none | 831 |
| `/listCertificates` | POST | `{certifiers, types, limit, offset}` | (raw) | none | 832 |
| `/proveCertificate` | POST | `{certificate, fieldsToReveal}` | (raw) | none | 833 |
| `/relinquishCertificate` | POST | `{certificateType, serialNumber, certifier}` | (raw) | none | 834 |
| `/discoverByIdentityKey` | POST | `{identityKey, ...}` | (raw) | none | 835 |
| `/discoverByAttributes` | POST | `{attributes, ...}` | (raw) | none | 836 |
| `/revealCounterpartyKeyLinkage` | POST | `{counterparty, verifier, privileged}` | (raw) | none | 802 |
| `/revealSpecificKeyLinkage` | POST | `{counterparty, verifier, protocolID, keyID, privileged}` | (raw) | none | 803 |
| `/.well-known/auth` | POST | BRC-103 messages (`{version, messageType, identityKey, initialNonce}`) | BRC-104 responses | `check_domain_approved` | 846 |

Hodos also exposes ~50 wallet-management endpoints (`/wallet/*`, `/domain/*`, `/sendMessage`, paymail, peerpay, pay402, etc.) that Dolphin Milk never calls.

---

## The diff

### Endpoints Hodos provides + Dolphin Milk uses, with compatible shapes (drop-in)

These ~25 endpoints share byte-compatible request and response shapes and will work without any change:

- `/getVersion` — Hodos accepts both POST and GET (route is registered twice). Response field `version` matches.
- `/getPublicKey` — same field names, same response. Hodos's BRC-42 derivation logic mirrors the bsv-wallet-cli semantic.
- `/createSignature` — both sides use `Vec<u8>` (JSON array) for `signature`, DER-encoded.
- `/verifySignature` — same `{valid: bool}` response.
- `/createHmac` — `{hmac: u8[]}` matches.
- `/verifyHmac` — same `{valid: bool}` response.
- `/encrypt` — `{ciphertext: u8[]}` BRC-2 format matches.
- `/decrypt` — `{plaintext: u8[]}` matches.
- `/createAction` — Hodos's `CreateActionOutpoint` deserializer accepts BOTH `"txid.vout"` string AND `{txid, vout}` object (handlers.rs:3807-3870); the `script` field aliases to `lockingScript`; all `options` fields are optional with sane defaults; `inputBEEF` accepts both hex string and byte array. Dolphin Milk's call pattern slots in cleanly.
- `/signAction` — `{reference}` only, optional `spends` is unused by Dolphin Milk's flow. Compatible.
- `/abortAction` — `{reference}`. Compatible.
- `/listActions` — every field Dolphin Milk sends is parsed; pagination semantics match.
- `/relinquishOutput` — `{basket, output: {txid, vout}}` matches exactly (note: a non-`default` basket name).
- `/getHeaderForHeight` — `{height}` → `{header: hex}`. Compatible.
- `/acquireCertificate` — Dolphin Milk sends both `certificateType` AND `type` (lifecycle.rs:208-218); Hodos reads `type`. Other field names match.
- `/listCertificates` — `{certifiers, types, limit, offset}` matches.
- `/proveCertificate` — `{certificate, fieldsToReveal}` matches.
- `/relinquishCertificate` — `{certificateType, serialNumber, certifier}` matches.
- `/discoverByIdentityKey`, `/discoverByAttributes` — passthrough JSON, no schema friction.
- `/revealCounterpartyKeyLinkage`, `/revealSpecificKeyLinkage` — exact field-name match.

### Endpoints Hodos provides + Dolphin Milk uses, with shape mismatches (need fixes)

| # | Endpoint | Mismatch | Severity | Suggested shim |
|---|---|---|---|---|
| 1 | `/listOutputs` | Dolphin Milk's `get_balance()` calls `listOutputs({basket: "default", ...})`. Hodos returns HTTP 400 `"Basket name 'default' is prohibited by BRC-100 specification"` at handlers.rs:15141-15145. | **Critical** — breaks first-call boot (`get_balance()` is the canonical health check). | Drop the rejection OR alias `"default"` → wallet's actual default basket. One-line change in Hodos. |
| 2 | `/getHeight`, `/getNetwork`, `/isAuthenticated`, `/waitForAuthentication` | Dolphin Milk calls each as GET (`call_get`, http.rs:531-567). Hodos registers POST-only (main.rs:793-794, 826, 828). Result: HTTP 405 Method Not Allowed. | **High** — `is_authenticated()` and `wait_for_authentication()` are commonly called at agent startup. | Add a `web::get()` route for each, pointing at the same handler. Four one-line additions to main.rs. |
| 3 | `/internalizeAction` response | Dolphin Milk's `fund_from_woc` reads `result.get("accepted").as_bool() == true` (http.rs:1083-1091). Hodos returns `InternalizeActionResponse { txid, status: "unconfirmed" }` — no `accepted` field. | **High** — every funding internalization will be reported as "wallet rejected internalization" by the agent even when it succeeded. | Add `accepted: true` to `InternalizeActionResponse` (handlers.rs:10964-10968 + 11735-11738). Two-line change. |

### Endpoints Dolphin Milk needs that Hodos does NOT provide

**None.** Every endpoint Dolphin Milk calls (`HttpWalletClient` inherent methods, `wallet/http.rs:225-883`) maps to a Hodos route. Hodos's surface is a strict superset of Dolphin Milk's needs.

The closest thing to a gap is the `raw_call` escape hatch — Dolphin Milk exposes a `wallet_call` tool to the agent's LLM that lets it call any method by name. Whatever it calls will land on Hodos. As long as it's a BRC-100 method, Hodos has it. We can't enumerate the closure across LLM behavior, but mechanically the dispatcher is just `client.call(method, params)`.

### Endpoints Hodos provides that Dolphin Milk does NOT use

Out of scope, but for context — Hodos exposes a much larger surface than vanilla BRC-100:

- Wallet-management: `/wallet/create`, `/wallet/delete`, `/wallet/balance`, `/wallet/sync`, `/wallet/backup`, `/wallet/restore`, `/wallet/unlock`, `/wallet/recover`, `/wallet/export`, `/wallet/import`, `/wallet/cleanup`, `/wallet/activity`, `/wallet/settings`
- Domain permissions: `/domain/permissions[/all|/protocol|/basket|/counterparty|/certificate|/reset-all]`
- PeerPay (BRC-29): `/wallet/peerpay/*`
- BRC-121 paid HTTP: `/wallet/pay402`, `/wallet/broadcast-nosend`
- Paymail: `/wallet/paymail/*`
- BRC-33 messages: `/sendMessage`, `/listMessages`, `/acknowledgeMessage`
- Certificate publishing (overlay): `/wallet/certificate/{publish,unpublish,cleanup}`
- Legacy compatibility: `/wallet/yours-legacy-addresses`, `/wallet/address-to-script`, `/wallet/{encrypt,decrypt}-bie1`

These don't affect Dolphin Milk integration. They're just there.

---

## Authentication shape

**No handshake collision.** Dolphin Milk's `HttpWalletClient` does NOT initiate BRC-31 / BRC-103 mutual auth with the wallet. It just sends `Origin: http://localhost` (configurable per `dolphin-milk.toml` `[wallet] origin`). It does, however, USE the wallet's identity to AUTHENTICATE with external x402 servers — via `/createSignature` + the BRC-103 nonce protocol applied at the OUTBOUND layer (`src/auth/` in Dolphin Milk).

Hodos's permission gate (`check_domain_approved` at `handlers.rs:572-612`) only fires when the request carries an `X-Requesting-Domain` header — which CEF adds before forwarding to Rust. Dolphin Milk's reqwest client never sets that header, so `check_domain_approved` returns `Ok(None)` — i.e. it treats Dolphin Milk's requests as INTERNAL (wallet-UI-like) and bypasses the domain gate entirely. That's accidentally perfect for this integration.

The other gate — the identity-key disclosure prompt at handlers.rs:307-352 — keys off `X-Requesting-Domain` too, so it also won't trip Dolphin Milk.

`well_known_auth` (BRC-103 server side) gates on `check_domain_approved`. Since Dolphin Milk doesn't call `/.well-known/auth` from `HttpWalletClient`, this isn't an issue. It DOES use BRC-103 — but only between the agent and external x402 servers, with the wallet acting purely as a signing oracle through `/createSignature`. That path is unchanged.

**Net:** dropping in Hodos as the wallet does not perturb any auth path Dolphin Milk relies on.

---

## Recommended next step

**Drop in with a thin shim. Three concrete fixes to Hodos's Rust wallet, each one-to-three lines:**

1. **`listOutputs` "default" basket rejection** — `rust-wallet/src/handlers.rs:15141-15145`. Either delete the four-line check, or alias `"default"` → the wallet's real default basket name inside the handler before resolution. (Hodos already has a "default" basket internally — `ensure_default_basket_exists` is called at startup, main.rs:283.)

2. **GET aliases for status endpoints** — `rust-wallet/src/main.rs`. Add four lines next to existing POST registrations:
   - `.route("/getHeight", web::get().to(handlers::get_height))`
   - `.route("/getNetwork", web::get().to(handlers::get_network))`
   - `.route("/isAuthenticated", web::get().to(handlers::is_authenticated))`
   - `.route("/waitForAuthentication", web::get().to(handlers::wait_for_authentication))`
   The handlers already accept `_body: web::Bytes` so GET-with-empty-body deserializes correctly.

3. **`internalizeAction` response `accepted: true`** — `rust-wallet/src/handlers.rs:10964-10968` and `:11735-11738`. Add `pub accepted: bool` to `InternalizeActionResponse`; set it `true` in the success path. Status field stays for backward compat with the existing CEF-side consumer.

Total estimated effort: under one hour of code + 30 minutes of testing. Zero schema migrations. Zero ABI-breaking changes for existing Hodos UI consumers.

After those three fixes, run the canary test described in `DOLPHIN_MILK_INTEGRATION.md` line 85: spawn `dolphin-milk` with `DOLPHIN_MILK_WALLET_URL=http://localhost:31301` and run `dolphin-milk status` / a simple x402 LLM call. If it boots and successfully resolves balance + identity key + a single signed request, the integration is real.

---

## Confidence level

**High** that the three fixes above resolve every static mismatch on the call paths the agent's startup and steady-state exercise.

**Medium** that no additional issue surfaces under live load. Static-only blind spots that would raise confidence to "high":

- BEEF interop: Dolphin Milk's `fund_from_woc` builds AtomicBEEF as `[0x01,0x01,0x01,0x01] + reversed_txid(32) + beef_bytes` (http.rs:1049-1054). Hodos's `internalize_action` detects this prefix at line 10987 and parses via `Beef::from_atomic_beef_base64` (line 11018) — but the input there is hex, not base64. Need to confirm Hodos's parser handles both. Annotated as a separate static check in a follow-up.
- BRC-29 derivation prefix/suffix: Dolphin Milk uses literal strings (`"dolphin-milk-fund"` / `"1"`); Hodos's BRC-29 invoice format treats them as opaque strings (handlers.rs:6606-6607), so this should be fine — but PeerPay's monitor task assumes base64. If Dolphin Milk's incoming payments need to be reconciled by `task_check_peerpay`, that might trip on the non-base64 prefix.
- Two-phase signing: Dolphin Milk's `spend_output` calls `createAction` with an unsigned input + `unlockingScriptLength: 73` and expects the wallet to fully sign and broadcast. Hodos's createAction can do that for wallet-known UTXOs (via `derive_key_for_output`), but if the spend is against an OUTPUT not in Hodos's database (e.g., the agent received funds OUTSIDE Hodos's awareness), the path may bail. Worth a real test.
- Live behavior under the `pay402` flow when the agent uses x402 to pay for LLM inference — that's `bsv-x402-server` → `WalletApi::create_action` → Hodos's `/createAction` with `acceptDelayedBroadcast: false`. Should work mechanically; needs one live e2e.

Confidence reaches "high" with a 15-minute live canary against a funded dev wallet.
