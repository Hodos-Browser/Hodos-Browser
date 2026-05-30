# Wallet API Map

> **Status:** Skeleton — to be filled in Phase 2.5 sub-phase A (planning).
>
> Source-of-truth table mapping every Rust wallet endpoint to: what it does,
> which permission gate(s) fire on the C++ side, which shim call(s) reach
> it, and which engine `PermissionDecision::Kind` is expected for each
> trust level.

## How to use this doc

- **Adding an endpoint:** add a row in the table below in the same commit
  that adds the Rust handler + route registration.
- **Changing gate behavior:** update the "Gate(s) fired" + "Engine decision"
  columns in the same commit.
- **Auditing shim coverage:** scan the "Shim call(s)" column to confirm
  every wallet endpoint has a documented caller pattern (or "internal only").
- **Cross-layer review:** the row should make sense to someone who hasn't
  read the implementation. If it doesn't, the row needs more detail.

## Endpoint table (skeleton — rows TBD)

Columns:

- **Endpoint** — HTTP method + path
- **Handler** — Rust function in `handlers.rs` (or submodule)
- **What it does** — one-line semantic description
- **Permission gate(s) fired** — which sections of
  `AsyncWalletResourceHandler::Open()` apply
  (domain_approval / payment_confirmation / identity_key_reveal /
   key_linkage_reveal / certificate_disclosure / scoped_grant / none)
- **Engine decision under "approved" trust** — Silent / Prompt(type) / Deny
- **Shim call(s)** — which `window.CWI.*` / `window.yours.*` reach it
- **Notes** — anything that doesn't fit above (rate-limit class, payment
  cents calc, special header requirements, etc.)

### BRC-100 standard endpoints

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /getVersion` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /getPublicKey` | TBD | TBD | TBD | TBD | TBD | identityKey-style branch |
| `POST /isAuthenticated` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /waitForAuthentication` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /createHmac` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /verifyHmac` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /encrypt` | TBD | TBD | TBD | TBD | TBD | BRC-2 (canonical) — not BIE1 |
| `POST /decrypt` | TBD | TBD | TBD | TBD | TBD | BRC-2 (canonical) — not BIE1 |
| `POST /verifySignature` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /createSignature` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /revealCounterpartyKeyLinkage` | TBD | TBD | TBD | TBD | TBD | BRC-72 |
| `POST /revealSpecificKeyLinkage` | TBD | TBD | TBD | TBD | TBD | BRC-72 |
| `POST /createAction` | TBD | TBD | payment_confirmation | TBD | TBD | Payment cents extracted from outputs |
| `POST /signAction` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /processAction` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /abortAction` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /listActions` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /internalizeAction` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /updateConfirmations` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /listOutputs` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /relinquishOutput` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /getHeight` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /getHeaderForHeight` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /getNetwork` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /acquireCertificate` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /listCertificates` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /proveCertificate` | TBD | TBD | certificate_disclosure | TBD | TBD | Field-level disclosure |
| `POST /relinquishCertificate` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /discoverByIdentityKey` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /discoverByAttributes` | TBD | TBD | TBD | TBD | TBD | TBD |

### Custom wallet endpoints (`/wallet/*`)

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `GET /wallet/status` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /wallet/create` | TBD | TBD | TBD | TBD | internal only | TBD |
| `POST /wallet/delete` | TBD | TBD | TBD | TBD | internal only | TBD |
| `GET /wallet/balance` | TBD | TBD | TBD | TBD | internal only | TBD |
| `POST /wallet/sync` | TBD | TBD | TBD | TBD | internal only | TBD |
| `POST /wallet/address/generate` | TBD | TBD | TBD | TBD | internal only | TBD |
| `GET /wallet/addresses` | TBD | TBD | TBD | TBD | internal only | TBD |
| `GET /wallet/address/current` | TBD | TBD | TBD | TBD | internal only | TBD |
| `POST /wallet/yours-legacy-addresses` | yours_legacy_addresses | TBD | TBD | TBD | yours.getAddresses | Phase 2 Step 3b.1 |
| `POST /wallet/address-to-script` | address_to_script | TBD | TBD | TBD | yours.sendBsv (N×) | Phase 2 Step 3b.2 |
| `POST /wallet/encrypt-bie1` | encrypt_bie1_handler | TBD | TBD | TBD | yours.encrypt | Phase 2 Step 3c.2; BIE1 not BRC-2 |
| `POST /wallet/decrypt-bie1` | decrypt_bie1_handler | TBD | TBD | TBD | yours.decrypt | Phase 2 Step 3c.2 |
| `GET /wallet/bsv-price` | get_bsv_price | TBD | TBD | TBD | yours.getExchangeRate, yours.getBalance | TBD |
| ...and many more — to be filled | | | | | | |

### Domain permission endpoints (`/domain/permissions/*`)

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `GET /domain/permissions` | TBD | TBD | TBD | TBD | TBD | TBD |
| `POST /domain/permissions` | TBD | TBD | TBD | TBD | TBD | TBD |
| `DELETE /domain/permissions` | TBD | TBD | TBD | TBD | yours.disconnect | TBD |
| ...and the sub-permission CRUD set | | | | | | |

### Auth / messaging / PeerPay / settings

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /.well-known/auth` | TBD | TBD | TBD | TBD | TBD | BRC-103/104 |
| ...and the rest | | | | | | |

## How the table will be filled (Phase 2.5 sub-phase A)

1. Read `rust-wallet/src/main.rs` route table top-to-bottom; one row per
   `.route()` registration. ~75 endpoints.
2. For each row, look at the handler in `handlers.rs` (or the submodule
   it routes to) and fill "What it does" from the doc comment.
3. For each row, search `cef-native/src/core/HttpRequestInterceptor.cpp`
   for the endpoint string — find which gate functions
   (`isPaymentEndpoint`, `isGetPublicKeyEndpoint`, etc.) match it and fill
   "Gate(s) fired".
4. For each row, search `cef-native/include/core/CWIShimScript.h` for
   the endpoint — find which canonical / legacy method dispatches to it
   and fill "Shim call(s)".
5. For each row, look at `PermissionEngine::Decide()`'s decision tree for
   the matching `CallKind` to determine "Engine decision (approved)".

Output: this table fully filled, signed off, committed.

After completion: use this table to verify Phase 2.5 commits 5-7 preserve
every gate's behavior on every endpoint.
