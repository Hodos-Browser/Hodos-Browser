# Wallet API Map

> **Status:** Filled in Phase 2.5 sub-phase A planning (2026-05-30). Source-of-truth
> table mapping every Rust wallet endpoint to: what it does, which permission
> gate(s) fire on the C++ side, which shim call(s) reach it, and which engine
> `PermissionDecision::Kind` is expected at each trust level.

## How to use this doc

- **Adding an endpoint:** add a row in the relevant category table below in the
  same commit that adds the Rust handler + route registration.
- **Changing gate behavior:** update the "Gate(s) fired" + "Engine decision"
  columns in the same commit.
- **Auditing shim coverage:** scan the "Shim call(s)" column to confirm every
  externally-reachable wallet endpoint has a documented caller pattern.
- **Cross-layer review:** the row should make sense to someone who hasn't read
  the implementation. If it doesn't, the row needs more detail.

## Schema (used by every cluster)

| Column | Meaning |
|---|---|
| Endpoint | HTTP method + path |
| Handler | Rust function in `handlers.rs` (or submodule) |
| What it does | One-line semantic description |
| Gate(s) fired | C++ permission gate triggered by `AsyncWalletResourceHandler::Open()` cascade. One of: `none`, `domain_approval`, `payment_confirmation`, `identity_key_reveal`, `key_linkage_reveal`, `certificate_disclosure`, `scoped_grant` (protocol/basket/counterparty), `generic` (catch-all unknown-trust prompt) |
| Engine decision (approved) | Decision returned by `PermissionEngine::Decide()` for an already-approved domain at the highest trust tier. `Silent` / `Prompt(type)` / `Deny` |
| Shim call(s) | Which `window.CWI.*` / `window.yours.*` / `window.panda.*` (alias) / internal-only |
| Notes | Rate-limit class, body classification specifics, header requirements, etc. |

## Trust levels referenced in "Engine decision"

Three coarse trust tiers materialize on the `domain_permissions` row:

- **Approved** — `trust_level = 'approved'`, all sub-permission caches consulted
- **Blocked** — `trust_level = 'blocked'`, every call denied
- **Unknown** — no row OR `trust_level = 'unknown'`, every shim-reachable call
  surfaces a `domain_approval` or `manifest_connect_bundle` modal first

The "Engine decision (approved)" column captures the highest-trust path; the
unknown / blocked paths can be inferred from the gate type.

---

## 1. Health & system (3 endpoints)

Internal-only. Not reachable from shims. No gates fire.

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `GET /health` | `health` | Liveness probe | none | n/a | internal only | Used by CEF startup wait loop |
| `POST /shutdown` | `shutdown` | Graceful wallet shutdown | none | n/a | internal only | Triggered by CEF on app exit |
| `GET /brc100/status` | `brc100_status` | BRC-100 implementation status banner | none | n/a | internal only | Surface check |

## 2. BRC-100 standard endpoints (29 routes / 28 endpoints)

All shim-reachable as canonical `window.CWI.*` methods. Gate cascade fires per
endpoint classifier (see Appendix A).

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /getVersion` | `get_version` | Returns BRC-100 wallet version + capabilities | `domain_approval` | Silent | `CWI.getVersion` | Also accepts GET (line 791) |
| `POST /getPublicKey` | `get_public_key` | Derive a public key — identity-key OR BRC-42 child key depending on body shape | `identity_key_reveal` if body is identity-key-style (per `isIdentityKeyStyleGetPublicKey`); else `domain_approval` | Silent if `domain_permissions.identity_key_disclosure_allowed=1` OR `X-Identity-Key-Approved: true` header injected post-modal; else `Prompt(IdentityKeyReveal)` | `CWI.getPublicKey` | Phase 1.5 Step 1 — identity-key-style = `{identityKey: true}` OR missing protocolID/keyID |
| `POST /isAuthenticated` | `is_authenticated` | Returns `{authenticated: bool}` based on `domain_permissions` row | `domain_approval` | Silent | `CWI.isAuthenticated`, `yours.isConnected` (legacy alias) | No body data, never prompts in approved path |
| `POST /waitForAuthentication` | `wait_for_authentication` | Long-poll for auth; opens connect modal if not approved | `domain_approval` (cascades to manifest connect bundle) | Silent if approved, else `Prompt(ManifestConnectBundle)` or `Prompt(DomainApproval)` | `CWI.waitForAuthentication`, `yours.connect` (legacy wraps this + getPublicKey) | BRC-100 Call Code 24. First-contact entry point for most dApps |
| `POST /createHmac` | `create_hmac` | Compute HMAC-SHA256 with BRC-42-derived key | `scoped_grant` (Protocol) | Silent if protocol granted; else `Prompt(ProtocolUse)` | `CWI.createHmac` | ProtocolID extracted from body |
| `POST /verifyHmac` | `verify_hmac` | Verify HMAC against derived key | `scoped_grant` (Protocol) | Silent if protocol granted; else `Prompt(ProtocolUse)` | `CWI.verifyHmac` | Symmetric with createHmac |
| `POST /encrypt` | `encrypt` | BRC-2 AES-256-GCM encrypt using BRC-42-derived key | `scoped_grant` (Protocol) | Silent if protocol granted; else `Prompt(ProtocolUse)` | `CWI.encrypt` | NOT BIE1 — see `/wallet/encrypt-bie1` for that |
| `POST /decrypt` | `decrypt` | BRC-2 AES-256-GCM decrypt | `scoped_grant` (Protocol) | Silent if protocol granted; else `Prompt(ProtocolUse)` | `CWI.decrypt` | Symmetric with encrypt |
| `POST /verifySignature` | `verify_signature` | Verify ECDSA signature with BRC-42-derived key | `scoped_grant` (Protocol) | Silent if protocol granted; else `Prompt(ProtocolUse)` | `CWI.verifySignature` | TBD whether verify-only triggers protocol prompt or always Silent |
| `POST /createSignature` | `create_signature` | Sign data with BRC-42-derived key | `scoped_grant` (Protocol) | Silent if protocol granted; else `Prompt(ProtocolUse)` | `CWI.createSignature` | ProtocolID extracted from body |
| `POST /revealCounterpartyKeyLinkage` | `reveal_counterparty_key_linkage` | BRC-72 — produce linkage proof revealing keys for a counterparty | `key_linkage_reveal` | `Prompt(KeyLinkageReveal)` unless `KeyLinkageApprovalCache` opt-in for this session | `CWI.revealCounterpartyKeyLinkage` | Session-only approval cache (no persistent column) |
| `POST /revealSpecificKeyLinkage` | `reveal_specific_key_linkage` | BRC-72 — reveal specific protocol+counterparty+keyID linkage | `key_linkage_reveal` | `Prompt(KeyLinkageReveal)` unless `KeyLinkageApprovalCache` opt-in | `CWI.revealSpecificKeyLinkage` | Session-only approval cache |
| `POST /createAction` | `create_action` | Build + sign + broadcast a BSV transaction; emits service fee | `payment_confirmation` | Silent if `cents ≤ per_tx_cents_limit` AND `session_spent + cents ≤ per_session_cap` AND rate ≤ limit; else `Prompt(PaymentConfirmation)` | `CWI.createAction`, `yours.sendBsv` (legacy maps N×{address,sats} → outputs via `/wallet/address-to-script`) | 100MB payload limit (large inputBEEF). Service fee 1000sat to `HODOS_FEE_ADDRESS`. Payment success indicator IPC fires from `AsyncHTTPClient::OnRequestComplete` on auto-approved success |
| `POST /signAction` | `sign_action` | Sign a previously-staged action (multi-step signing) | `payment_confirmation` (if produces outputs) | Same logic as createAction | `CWI.signAction` | 100MB payload limit |
| `POST /processAction` | `process_action` | Finalize a pre-built action | `payment_confirmation` | Same logic as createAction | (canonical-only) | |
| `POST /abortAction` | `abort_action` | Cancel a staged action | `domain_approval` | Silent | `CWI.abortAction` | No payment side-effect |
| `POST /listActions` | `list_actions` | Enumerate this domain's actions | `domain_approval` | Silent | `CWI.listActions` | Filtered by `X-Requesting-Domain` |
| `POST /internalizeAction` | `internalize_action` | Accept a tx originated externally (e.g. PeerPay receive) | `domain_approval` | Silent | `CWI.internalizeAction` | Used by `TaskCheckPeerPay` internally too |
| `POST /updateConfirmations` | `update_confirmations_endpoint` | Refresh proof status for tracked tx | `domain_approval` | Silent | (canonical-only) | |
| `POST /listOutputs` | `list_outputs` | List UTXOs in a basket | `scoped_grant` (Basket) | Silent if basket granted; else `Prompt(BasketAccess)` | `CWI.listOutputs` | Basket name in body. Protected baskets (`default`, `backup-*`, `admin *`) never auto-grant |
| `POST /relinquishOutput` | `relinquish_output` | Remove output from a basket | `scoped_grant` (Basket) | Silent if basket granted; else `Prompt(BasketAccess)` | `CWI.relinquishOutput` | |
| `POST /getHeight` | `get_height` | Current chain tip height | `domain_approval` | Silent | `CWI.getHeight` | |
| `POST /getHeaderForHeight` | `get_header_for_height` | Block header at given height | `domain_approval` | Silent | `CWI.getHeaderForHeight` | |
| `POST /getNetwork` | `get_network` | Returns `'mainnet'` | `domain_approval` | Silent | `CWI.getNetwork` | |
| `POST /acquireCertificate` | `acquire_certificate` | Request a BRC-52 cert from a certifier | `payment_confirmation` (acquire is a paid call per `isPaymentEndpoint`) | Silent if within caps; else `Prompt(PaymentConfirmation)` | `CWI.acquireCertificate` | Service fee applies. 240s timeout (cert hosts slow). Auto-publishes on acquire (Phase 1.6 polish) |
| `POST /listCertificates` | `list_certificates` | List held certs | `domain_approval` | Silent | `CWI.listCertificates` | |
| `POST /proveCertificate` | `prove_certificate` | BRC-52 selective disclosure — reveal field subset | `certificate_disclosure` | Silent if every requested field has matching `cert_field_permissions` row; else `Prompt(CertificateDisclosure)` per missing field | `CWI.proveCertificate` | Field-level granularity. Sensitive fields (email, dob) prompt even for approved domain |
| `POST /relinquishCertificate` | `relinquish_certificate` | Delete a cert and auto-unpublish from overlay | `domain_approval` | Silent | `CWI.relinquishCertificate` | Triggers unpublish-on-relinquish |
| `POST /discoverByIdentityKey` | `discover_by_identity_key` | BRC-100 cert discovery by identity key | `domain_approval` | Silent | `CWI.discoverByIdentityKey` | Reads from overlay services + IdentityResolver |
| `POST /discoverByAttributes` | `discover_by_attributes` | BRC-100 cert discovery by attributes | `domain_approval` | Silent | `CWI.discoverByAttributes` | |

## 3. Certificate publish/admin (4 endpoints)

Internal-only — invoked by wallet UI overlay, not by shim. No external gates.

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /wallet/certificate/publish` | `publish_certificate` | Broadcast cert advert to overlay SHIP hosts | none (internal) | n/a | internal only | Service fee. Parallel submit to 3+ hosts (Phase 1.6 polish) |
| `POST /wallet/certificate/unpublish` | `unpublish_certificate` | Spend SHIP advert UTXO | none (internal) | n/a | internal only | Service fee. Parallel lookup + early-return-on-any |
| `POST /wallet/certificate/cleanup` | `cleanup_overlay_certificates` | Purge stale cert adverts | none (internal) | n/a | internal only | Background task |
| `POST /admin/prepare-unpublish` | `admin_prepare_unpublish` | DB-only repair for unpublish flow | none (internal) | n/a | internal only | Recovery helper |

## 4. Debug endpoints (3 endpoints)

Internal-only. Used by dev tools and Phase 1.6 recovery flows.

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /wallet/debug/validate-beef` | `debug_validate_beef` | Manual BEEF ancestry validation | none | n/a | internal only | |
| `POST /wallet/debug/repair-nosend` | `debug_repair_nosend` | Repair DB after failed nosend broadcast | none | n/a | internal only | |
| `POST /wallet/debug/broadcast-nosend` | `debug_broadcast_nosend` | Broadcast a staged nosend tx | none | n/a | internal only | NB: distinct from `/wallet/broadcast-nosend` (BRC-121) |

## 5. BRC-103/104 authentication (1 endpoint)

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /.well-known/auth` | `well_known_auth` | BRC-103 mutual-auth challenge/response from external services | `domain_approval` | Silent | (server-side endpoint, external clients call us) | Cross-direction: Hodos exposes this for BRC-103-aware peers to authenticate Hodos itself |

## 6. Custom wallet endpoints — internal CRUD / lifecycle (22 endpoints)

All internal-only. Used by wallet UI overlay (`WalletPanelPage.tsx`, `SettingsPage.tsx`), not by external shims.

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `GET /wallet/status` | `wallet_status` | Wallet-exists + locked flag | none (internal) | n/a | internal only | Cached in C++ `WalletStatusCache` |
| `POST /wallet/create` | `wallet_create` | Generate new HD wallet | none | n/a | internal only | |
| `POST /wallet/delete` | `wallet_delete` | Destroy wallet | none | n/a | internal only | |
| `GET /wallet/balance` | `wallet_balance` | Spendable + pending balances | none | n/a | internal only | |
| `POST /wallet/sync` | `wallet_sync` | Sync UTXOs from indexer | none | n/a | internal only | |
| `POST /wallet/address/generate` | `generate_address` | Mint new receive address | none | n/a | internal only | |
| `GET /wallet/addresses` | `get_all_addresses` | List all addresses | none | n/a | internal only | |
| `GET /wallet/address/current` | `get_current_address` | Current receive address | none | n/a | internal only | |
| `POST /wallet/backup` | `wallet_backup` | Encrypted local backup file | none | n/a | internal only | |
| `POST /wallet/backup/onchain` | `wallet_backup_onchain` | On-chain encrypted backup | none | n/a | internal only | Service fee |
| `POST /wallet/backup/onchain/verify` | `wallet_backup_onchain_verify` | Verify on-chain backup is retrievable | none | n/a | internal only | |
| `POST /wallet/recover/onchain` | `wallet_recover_onchain` | Restore wallet from on-chain backup | none | n/a | internal only | |
| `POST /wallet/restore` | `wallet_restore` | Restore from local backup file | none | n/a | internal only | |
| `POST /wallet/unlock` | `wallet_unlock` | PIN-unlock encrypted wallet | none | n/a | internal only | |
| `POST /wallet/recover` | `wallet_recover` | Recover from mnemonic | none | n/a | internal only | |
| `POST /wallet/recover-external` | `wallet_recover_external` | Recover from external wallet's mnemonic format | none | n/a | internal only | |
| `POST /wallet/rescan` | `wallet_rescan` | Full chain rescan | none | n/a | internal only | |
| `POST /wallet/cleanup` | `wallet_cleanup` | DB maintenance | none | n/a | internal only | |
| `POST /wallet/consolidate-dust` | `wallet_consolidate_dust` | Merge small UTXOs | none | n/a | internal only | Service fee |
| `POST /wallet/export` | `wallet_export` | Export backup payload | none | n/a | internal only | |
| `POST /wallet/import` | `wallet_import` | Import backup payload (100MB limit) | none | n/a | internal only | |
| `POST /wallet/reveal-mnemonic` | `reveal_mnemonic` | Mnemonic disclosure after PIN re-auth | none | n/a | internal only | Privacy-sensitive UI flow |
| `GET /wallet/tokens` | `list_token_outputs` | Token UTXOs categorized by basket | none | n/a | internal only | Wallet UI token list |

## 7. Legacy Yours-shim helper endpoints (4 endpoints)

Shim-reachable via `window.yours.*` (and `window.panda.*` alias) only. Created
specifically as backing translators for the legacy shim surface — not part of
the canonical BRC-100 API.

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /wallet/yours-legacy-addresses` | `yours_legacy_addresses` | Derive `{bsvAddress, ordAddress, identityAddress}` via BRC-42 in one round-trip | `domain_approval` (identity slot returns null unless identity-key disclosure granted) | Silent for bsv/ord; identity slot null if no disclosure permission | `yours.getAddresses` | Phase 2 Step 3b.1. Uses yours-legacy-v1 protocol IDs + `yours-{origin}` keyID |
| `POST /wallet/address-to-script` | `address_to_script` | Convert BSV address → P2PKH locking script | `domain_approval` | Silent | `yours.sendBsv` (N× per payment) | Phase 2 Step 3b.2. Then routes through `/createAction` |
| `POST /wallet/encrypt-bie1` | `encrypt_bie1_handler` | BIE1 (ECIES Electrum) legacy encrypt — NOT BRC-2 | `scoped_grant` (Protocol) | Silent if protocol granted; else `Prompt(ProtocolUse)` | `yours.encrypt` | Phase 2 Step 3c.2. For Yours-era dApps storing pre-BRC-2 ciphertexts |
| `POST /wallet/decrypt-bie1` | `decrypt_bie1_handler` | BIE1 (ECIES Electrum) legacy decrypt | `scoped_grant` (Protocol) | Silent if protocol granted; else `Prompt(ProtocolUse)` | `yours.decrypt` | Symmetric with encrypt-bie1 |

## 8. Price / sync status / activity / settings (8 endpoints)

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `GET /wallet/bsv-price` | `get_bsv_price` | BSV/USD price (5-min TTL cache) | `domain_approval` (when called from shim); none (when called from C++ engine for cents calc) | Silent | `yours.getExchangeRate`, `yours.getBalance` (legacy uses this for USD conversion) | C++ engine also consults this for `preCalculatedCents_` in payment gate via `BSVPriceCache` |
| `GET /wallet/sync-status` | `get_sync_status` | Recovery progress (height, addresses scanned) | none (internal) | n/a | internal only | |
| `POST /wallet/sync-status/seen` | `mark_sync_seen` | Acknowledge sync banner dismissal | none (internal) | n/a | internal only | |
| `POST /transaction/send` | `send_transaction` | Legacy raw-tx send (pre-BRC-100) | `payment_confirmation` | Same as createAction | (canonical/legacy?) | Pre-dates BRC-100; may be deprecation candidate post-2.5 |
| `GET /wallet/activity` | `wallet_activity` | Unified sent/received feed | none (internal) | n/a | internal only | Wallet UI activity tab |
| `GET /wallet/settings` | `wallet_settings_get` | Wallet UI settings | none (internal) | n/a | internal only | |
| `POST /wallet/settings` | `wallet_settings_set` | Persist setting | none (internal) | n/a | internal only | Settings overlay |

## 9. Domain permissions (17 endpoints)

The shape that the engine itself reads. Most are internal-only (UI manages
them); `DELETE /domain/permissions` is shim-reachable via `yours.disconnect`.

After any mutation, the wallet UI sends `domain_permission_invalidate` IPC to
the C++ side so `DomainPermissionCache` + `SubPermissionCache` drop their
cached view of the affected domain.

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `GET /domain/permissions` | `get_domain_permission` | Read row for a given domain | none (internal) | n/a | internal only | `?domain=` query |
| `POST /domain/permissions` | `set_domain_permission` | Upsert row (approve / set caps / etc.) | none (internal) | n/a | internal only | Fires `domain_permission_invalidate` IPC after success |
| `DELETE /domain/permissions` | `delete_domain_permission` | Remove row (full revoke) | none (internal: wallet UI); but also shim-reachable | n/a (called as self-revoke) | `yours.disconnect` | Shim path identifies the domain via `X-Requesting-Domain` |
| `GET /domain/permissions/all` | `list_domain_permissions` | List all permission rows | none (internal) | n/a | internal only | "Manage Site Permissions" UI |
| `GET /domain/permissions/certificate` | `check_cert_permissions` | List per-field cert disclosure permissions | none (internal) | n/a | internal only | |
| `POST /domain/permissions/certificate` | `approve_cert_fields` | Grant field disclosure (one or more cert_field_permissions rows) | none (internal: called by modal resolution) | n/a | internal only | Fires invalidate IPC |
| `DELETE /domain/permissions/certificate` | `revoke_cert_fields` | Revoke field permissions | none (internal) | n/a | internal only | Fires invalidate IPC |
| `POST /domain/permissions/protocol` | `grant_protocol_permission` | Insert row in `domain_protocol_permissions` (V18) | none (internal: called by ProtocolUse modal resolution) | n/a | internal only | Phase 1.5 Step 3 |
| `DELETE /domain/permissions/protocol` | `revoke_protocol_permission` | Remove row | none (internal) | n/a | internal only | |
| `GET /domain/permissions/protocol` | `list_protocol_permissions` | List grants | none (internal) | n/a | internal only | |
| `POST /domain/permissions/basket` | `grant_basket_permission` | Insert row in `domain_basket_permissions` (V18) | none (internal) | n/a | internal only | Phase 1.5 Step 3 |
| `DELETE /domain/permissions/basket` | `revoke_basket_permission` | Remove row | none (internal) | n/a | internal only | |
| `GET /domain/permissions/basket` | `list_basket_permissions` | List grants | none (internal) | n/a | internal only | |
| `POST /domain/permissions/counterparty` | `grant_counterparty_permission` | Insert row in `domain_counterparty_permissions` (V18) | none (internal) | n/a | internal only | Phase 1.5 Step 3 |
| `DELETE /domain/permissions/counterparty` | `revoke_counterparty_permission` | Remove row | none (internal) | n/a | internal only | |
| `GET /domain/permissions/counterparty` | `list_counterparty_permissions` | List grants | none (internal) | n/a | internal only | |
| `POST /domain/permissions/reset-all` | `domain_permissions_reset_all` | Wipe all domain permissions | none (internal) | n/a | internal only | "Reset all permissions" UI button. Fires invalidate IPC across all domains |

## 10. BRC-33 message relay (3 endpoints)

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /sendMessage` | `send_message` | Send BRC-33 encrypted message via MessageBox | `payment_confirmation` (per `isPaymentEndpoint`) + `scoped_grant` (Counterparty) | Silent if counterparty granted AND within caps; else cascading prompts | `CWI.sendMessage` | Counterparty extracted from body |
| `POST /listMessages` | `list_messages` | Poll MessageBox inbox | `scoped_grant` (Counterparty) | Silent if counterparty granted | `CWI.listMessages` | |
| `POST /acknowledgeMessage` | `acknowledge_message` | Mark message seen | `scoped_grant` (Counterparty) | Silent if counterparty granted | `CWI.acknowledgeMessage` | |

## 11. PeerPay BRC-29 (5 endpoints)

Internal-only — UI overlay (`PeerPayPanel.tsx`) is the only caller. Background
task `TaskCheckPeerPay` polls + auto-accepts via `internalize_action`.

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /wallet/peerpay/send` | `peerpay_send` | Send BRC-29 PaymentToken via encrypted MessageBox | none (internal) | n/a | internal only | Service fee; uses BRC-103 AuthFetch |
| `POST /wallet/peerpay/check` | `peerpay_check` | Poll MessageBox for incoming PeerPay | none (internal) | n/a | internal only | |
| `GET /wallet/peerpay/status` | `peerpay_status` | Recent PeerPay activity | none (internal) | n/a | internal only | |
| `POST /wallet/peerpay/dismiss` | `peerpay_dismiss` | Hide PeerPay notification | none (internal) | n/a | internal only | |
| `POST /wallet/peerpay/outbox-retry` | `peerpay_outbox_retry` | Retry stuck outbound PeerPay | none (internal) | n/a | internal only | |

## 12. BRC-121 paid content (2 endpoints)

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /wallet/pay402` | `pay_402` | Build BRC-29 nosend BEEF + emit 5 retry headers for HTTP 402 paid retry | inline cap-check cascade in `TryHandleBrc121_402` (NOT the engine — see `brc121_bypasses_permission_engine` memory) | Silent if within caps; else inline modals (DomainApproval / PaymentConfirmation) | (not shim-reachable — C++ `Async402ResourceHandler` calls this from paid-content interception) | Phase 1 BRC-121. Parallel cap-check cascade lives at `HttpRequestInterceptor.cpp:3717-3791` — Phase 1.5 polish item G is to migrate this to the engine |
| `POST /wallet/broadcast-nosend` | `broadcast_nosend` | Broadcast a previously-built nosend BEEF after the paid retry returned 200 | none | n/a | internal only (called from `Async402ResourceHandler` after success) | Broadcast-after-200 architecture (see `brc121_no_send_required` memory) |

## 13. Paymail (bsvalias) (2 endpoints)

Internal-only — UI overlay drives Paymail send/resolve directly.

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `POST /wallet/paymail/send` | `paymail_send` | Pay to `user@paymail.example` | none (internal) | n/a | internal only | Service fee. Wraps createAction |
| `GET /wallet/paymail/resolve` | `paymail_resolve` | Resolve paymail → output script | none (internal) | n/a | internal only | |

## 14. Recipient resolution (2 endpoints)

Internal-only. Helpers for the wallet UI's "send to..." form.

| Endpoint | Handler | What it does | Gate(s) fired | Engine decision (approved) | Shim call(s) | Notes |
|---|---|---|---|---|---|---|
| `GET /wallet/recipient/resolve` | `recipient_resolve` | Resolve identity-key/paymail/BSV address into a unified target | none (internal) | n/a | internal only | |
| `GET /wallet/recipient/suggest` | `recipient_suggest` | Autocomplete suggestions (Issue #38) | none (internal) | n/a | internal only | |

---

## Appendix A — C++ gate classifier reference

Defined in `cef-native/src/core/HttpRequestInterceptor.cpp`. Used by
`AsyncWalletResourceHandler::Open()` to pick which gate fires for the incoming
request before consulting the `PermissionEngine`.

| Classifier | Endpoints matched | Gate triggered |
|---|---|---|
| `isPaymentEndpoint` (L1673) | `/createAction`, `/acquireCertificate`, `/sendMessage` | payment_confirmation cascade |
| `isProveCertificateEndpoint` (L1680) | `/proveCertificate` | certificate_disclosure (per-field) |
| `isGetPublicKeyEndpoint` (L1685) + `isIdentityKeyStyleGetPublicKey` body check (L1696) | `/getPublicKey` IFF body is identity-key shape | identity_key_reveal |
| `isKeyLinkageEndpoint` (L1689) | `/revealCounterpartyKeyLinkage`, `/revealSpecificKeyLinkage` | key_linkage_reveal |
| `isWalletEndpoint` (L4470) | The full union of BRC-100 + `/wallet/*` + `/transaction/*` + `/.well-known/auth` + `/sendMessage` etc. | Master "is this a wallet request" filter — gates the entire `AsyncWalletResourceHandler` engagement |

Scoped-grant gates (Protocol / Basket / Counterparty) are decided inside
`PermissionEngine::Decide()` by looking at the body's `protocolID` /
`basket` / `counterparty` fields, not by URL classification. See
[`AUTO_APPROVE_ENGINE.md`](./AUTO_APPROVE_ENGINE.md) §1 (CallKind classification).

## Appendix B — Shim method → endpoint map

### Canonical methods (`window.CWI.*`) — 28 methods

All routed through `makeMethod(name) → __hodos_walletCall(name, '/' + name, body)`
in `CWIShimScript.h`. One-to-one with BRC-100 endpoints. See cluster 2 above.

### Legacy methods (`window.yours.*` + `window.panda.*` alias) — 11 methods

| Method | Endpoint(s) | Notes |
|---|---|---|
| `yours.isReady` | (local prop, always `true`) | No wallet call |
| `yours.isConnected` | `POST /isAuthenticated` (canonical wrap) | Returns `!!result.authenticated` |
| `yours.connect` | `POST /waitForAuthentication` + `POST /getPublicKey` | Returns `{identityKey, addresses}` |
| `yours.disconnect` | `DELETE /domain/permissions?domain=<origin>` | Self-revoke |
| `yours.getAddresses` | `POST /wallet/yours-legacy-addresses` | Single round-trip; identity slot may be null |
| `yours.getPubKeys` | `POST /getPublicKey` (canonical wrap) | |
| `yours.getBalance` | `GET /wallet/bsv-price` | Returns USD-converted balance |
| `yours.getExchangeRate` | `GET /wallet/bsv-price` | |
| `yours.sendBsv` | N × `POST /wallet/address-to-script` → `POST /createAction` | Converts `[{address, satoshis}]` to BRC-100 outputs |
| `yours.broadcast` | `POST /createAction` (best-effort fallback to internal broadcast) | Pre-BRC-100 |
| `yours.encrypt` | `POST /wallet/encrypt-bie1` | BIE1 (ECIES Electrum), NOT BRC-2 |
| `yours.decrypt` | `POST /wallet/decrypt-bie1` | |
| `yours.getSignatures` | (rejected with NOT_IMPL — see `phase2_step3_ecies_electrum`) | No safe translation to BRC-100 createSignature semantics |

`window.panda` is set as a direct alias to `window.yours` (`CWIShimScript.h:987`)
— Treechat and a few other Yours-era dApps target this name. Same dispatch
table, no separate gate handling.

## Appendix C — Endpoints NOT reachable through the IPC bridge today

Commits 1-4 of Phase 2.5 wired the shim through the IPC bridge for 28 canonical
methods + 8 legacy methods. The following endpoints are still reached only via:

- **Direct C++ resource interception** (CEF resource handler → wallet):
  - `/wallet/pay402` + `/wallet/broadcast-nosend` (BRC-121 paid retry path; `Async402ResourceHandler`)
- **Direct wallet UI overlay → C++ → Rust** (no shim path):
  - All `internal only` endpoints in clusters 1, 3, 4, 6, 8 (price/sync/activity), 9 (most), 11, 13, 14
- **Background tasks inside Rust** (no C++/IPC dispatch):
  - `TaskCheckPeerPay` calls `internalize_action`, `peerpay_check`, etc.

Phase 2.5 commits 5-7 specifically target the **shim-reachable** column —
ensuring those endpoints, when called through the IPC bridge, fire the full
`PermissionEngine` cascade instead of bypassing to `check_domain_approved` alone.

## Appendix D — Drift detection

Whenever any of these change, this doc must update in the same commit:

| Change | Doc impact |
|---|---|
| New `.route()` registration in `main.rs` | Add row in correct cluster |
| New gate classifier in `HttpRequestInterceptor.cpp` | Update Appendix A |
| New shim method in `CWIShimScript.h` | Update Appendix B + relevant cluster row |
| Engine decision logic change for a `CallKind` | Update affected rows' "Engine decision" column |
| New sub-permission table | Update relevant cluster + Appendix A |

A future scripted check could parse the table back into structured data and
diff it against `grep .route\\( main.rs` to catch silent drift; not built yet.
