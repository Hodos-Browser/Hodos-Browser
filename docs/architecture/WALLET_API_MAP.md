# Wallet API Gate Map

> **Last reviewed: 2026-09-30** (accuracy pass against the website audit of
> 2026-09-30). **2026-08-03:** rewritten. The previous version was a second
> copy of the endpoint roster with a "which C++ gate fires" column describing
> the deleted C++ `PermissionEngine`. Both problems are fixed: the roster now
> lives in one place, and the gate column describes the Rust dispatchers that
> actually run.

## What this doc is — and is not

**Not** an endpoint roster. The route registrations, their handlers and
their HTTP verbs live in **`rust-wallet/CLAUDE.md`**, which is generated
against `main.rs` and is the only place that list should exist. Duplicating it
here is what made the old version rot. (As of 2026-09-30 that roster is one
route behind `main.rs`: it lacks `/wallet/release-nosend`, and its counts
need regenerating.)

This doc answers the two questions that are genuinely cross-layer and live
nowhere else:

1. **Which wallet endpoints are permission-gated, and by which dispatcher?**
2. **Which page-facing shim method reaches which endpoint?**

## 1. How gating works, in one paragraph

Two layers of gate, both in Rust. `main.rs :: domain_trust_mw` wraps every
route and runs Matrix C branch 1 (domain trust) on any request carrying an
`X-Requesting-Domain` header. Individual handlers then call a `dispatch_*`
function from `permission_service/request_gate.rs` for their own `CallKind`,
which runs branches 2–6. A request with no `X-Requesting-Domain` is internal
and skips both. The full cascade is in
[`AUTO_APPROVE_ENGINE.md`](./AUTO_APPROVE_ENGINE.md).

> ⚠️ **Known issue — headerless loopback requests are trusted as the wallet
> UI.** The C++ shell stamps the header, so anything that reaches
> `127.0.0.1:<wallet port>` without passing through the shell's stamp is
> treated as first-party: any local process, and any page on `localhost:*` or
> `127.0.0.1:*`, which `IsInternalOrigin` classifies as Hodos's own UI. The
> only protection is the loopback bind. A fix, including a per-launch secret on
> the wallet port, is planned for a later release.

So an endpoint is in one of four states:

| State | Meaning |
|---|---|
| **Kind-gated** | Middleware + a `dispatch_*` call. Listed in §2. |
| **Trust-gated only** | Middleware only. Reachable by an approved domain without a further prompt. |
| **First-party only** | The middleware answers 403 to any POST or DELETE carrying `X-Requesting-Domain` (`main.rs :: domain_trust_mw`, `is_permission_surface`): the `/domain/` and `/wallet/session` subtrees, `/wallet/reveal-mnemonic`, `/wallet/settings`, and the `/wallet/debug`, `/wallet/delete`, `/wallet/consolidate-dust`, `/wallet/backup`, `/wallet/recover`, `/wallet/broadcast-nosend` and `/wallet/held-outputs` subtrees. One carve-out: a site may `DELETE /domain/permissions` for its own domain. |
| **Internal-only** | Never called with `X-Requesting-Domain`, so never gated at all. |

## 2. The kind-gated endpoints

Every `dispatch_*` call site in the tree. **20 sites across 19 handlers**
(measured 2026-09-30) — `prove_certificate` calls two.

### `dispatch_privacy_perimeter` — 5 sites

Matrix C branch 2. Prompts unless a persistent column or session opt-in
authorizes silence.

| Endpoint | Handler | CallKind | Silent when |
|---|---|---|---|
| `POST /getPublicKey` | `handlers.rs :: get_public_key` | `IdentityKeyReveal` | `domain_permissions.identity_key_disclosure_allowed=1` (V17, default ON) or session opt-in |
| `POST /revealCounterpartyKeyLinkage` | `handlers.rs :: reveal_counterparty_key_linkage` | `CounterpartyKeyLinkage` | session opt-in only |
| `POST /revealSpecificKeyLinkage` | `handlers.rs :: reveal_specific_key_linkage` | `SpecificKeyLinkage` | session opt-in only |
| `POST /wallet/yours-legacy-addresses` | `handlers.rs :: yours_legacy_addresses` | `IdentityKeyReveal` | same as `/getPublicKey` — the identity slot is what triggers it |
| `POST /proveCertificate` | `certificate_handlers.rs :: prove_certificate` | `SensitiveCertField` | **never** |

`/getPublicKey` only routes through the gate for identity-key-shaped requests
from an external domain. The classification lives in Rust now
(`permission_service/context_builder.rs`); the C++
`isIdentityKeyStyleGetPublicKey` that used to make this call is dead code.
The legacy `X-Identity-Key-Approved` header is ignored and no longer injected —
the only approval channel is `X-User-Approved`.

### `dispatch_scoped_grant` — 6 sites

Matrix C branch 3. Protocol / basket / counterparty grants.

| Endpoint | Handler | Scope |
|---|---|---|
| `POST /createHmac` | `handlers.rs :: create_hmac` | Protocol |
| `POST /encrypt` | `handlers.rs :: encrypt` | Protocol |
| `POST /decrypt` | `handlers.rs :: decrypt` | Protocol |
| `POST /createSignature` | `handlers.rs :: create_signature` | Protocol |
| `POST /listOutputs` | `handlers.rs :: list_outputs` | Basket |
| `POST /relinquishOutput` | `handlers.rs :: relinquish_output` | Basket |

Basket calls against a **protected** basket (`default`, `backup-*`, `admin `
prefix — `is_protected_basket`) always prompt: the dispatcher forces
`scoped_grant_exists` to false before calling the engine. `bundled_scope_grant`
plays no part: beta.3 Phase 7c removed the engine arm that read it.

> **`verifyHmac` and `verifySignature` are NOT gated.** They have no
> `dispatch_*` call and no `check_domain_approved`. The old version of this
> doc claimed both fired a protocol scoped-grant; they do not. Whether
> verify-only operations *should* be gated is a live question — they derive a
> key from a protocol the site may not hold a grant for — but today they are
> trust-gated only.

### `dispatch_payment` — 8 sites

Matrix C branch 4. Caps, rate limit, session counters.

| Endpoint | Handler | Note |
|---|---|---|
| `POST /createAction` | `handlers.rs :: create_action` | The main spend path |
| `POST /processAction` | `handlers.rs :: process_action` | Create, sign and broadcast in one call |
| `POST /transaction/send` | `handlers.rs :: send_transaction` | Calls `dispatch_payment_with_amount` |
| `POST /wallet/peerpay/send` | `handlers.rs :: peerpay_send` | |
| `POST /wallet/paymail/send` | `handlers.rs :: paymail_send` | |
| `POST /sendMessage` | `handlers.rs :: send_message` | BRC-33 (PeerServ Message Relay Interface). Passes the payment gate and spends nothing; it writes to the local relay store (note below) |
| `POST /wallet/pay402` | `handlers.rs :: pay_402` | BRC-121 (Simple 402 Payments). Called by C++ `Async402ResourceHandler`, not by the shim |
| `POST /acquireCertificate` | `certificate_handlers.rs :: acquire_certificate` | Certifiers charge |

The payment context (`X-Browser-Id`, `X-Payment-Satoshis`, `X-Payment-Cents`,
`X-Bsv-Price-Available`) is computed in C++ and sent as headers; C++ injects
those headers only when `isPaymentEndpoint(endpoint)` matches. That function
(`cef-native/include/core/PaymentCost.h :: IsPaymentEndpoint`) normalizes the
path first and matches eight: `/createAction`, `/acquireCertificate`,
`/sendMessage`, `/transaction/send`, `/wallet/peerpay/send`,
`/wallet/paymail/send`, `/processAction` and `/wallet/pay402`.

> **`signAction` has no payment dispatcher.** A multi-step sign flow is gated
> when the action is created, not when it is signed. `processAction` is gated
> (table above). Flagged in `AUTO_APPROVE_ENGINE.md` §7.

> **BRC-33 is a local relay store, kept on purpose.** `/sendMessage`,
> `/listMessages` and `/acknowledgeMessage` read and write the wallet's own
> SQLite table (`MessageRelayRepository`). `send_message` takes the sender from
> the `x-bsv-auth-identity-key` header without verifying it, and falls back to
> `anonymous` when the header is absent. PeerPay delivers through the MessageBox
> service instead (`rust-wallet/src/messagebox.rs`): to the recipient's
> advertised host, or to every host in `MESSAGEBOX_HOSTS`, and read from every one. Hodos keeps both: the local relay for
> BRC-33 callers, and MessageBox for delivery between wallets.

### `dispatch_cert_disclosure` — 1 site

Matrix C branch 5. `POST /proveCertificate`
(`certificate_handlers.rs :: prove_certificate`), for the non-sensitive
fields left after the privacy-perimeter dispatch. Silent only when every
requested field already has a `cert_field_permissions` row.

The approval replay for this path is special: approving a narrowed field
subset rewrites the body, which breaks the body-sha256 binding the normal
`X-User-Approved` replay relies on. So this path consumes the approval id and
verifies `requested ⊆ approved` against the C++-injected
`X-Cert-Approved-Fields` header instead.

### Everything else

Every other externally-reachable route runs the domain-trust middleware and
nothing more. That includes the BRC-100 (Unified, Vendor-Neutral, Unchanging,
and Open BSV Blockchain Standard Wallet-to-Application Interface) read surface
(`listActions`, `listCertificates`, `getHeight`, `getNetwork`, `discoverBy*`,
`getVersion`, `isAuthenticated`, `waitForAuthentication`), the action lifecycle
(`signAction`, `abortAction`, `internalizeAction`) and
`DELETE /domain/permissions` (self-revoke).

Internal-only clusters — wallet CRUD, backup/restore, PeerPay and Paymail
(apart from the two send routes in §2), recipient resolution,
price/sync/activity/settings, certificate publish/admin, debug — are meant to
be called only without `X-Requesting-Domain`. CORS does not protect them: the
IPC bridge ([`IPC_BRIDGE.md`](./IPC_BRIDGE.md)) forwards from the browser
process, outside CORS. Their protection is the middleware plus the
first-party-only list in §1. A route on neither list is reachable by an
approved dApp that names its path, because exposure is a deny-list. Scheduled
for beta.7 Track 5 (phase P2):
`development-docs/0.4.0-beta.7/tickets/TICKET_dapp_reachable_surface_is_a_denylist_not_an_allowlist.md`.

## 3. The shim surface

Injected by `cef-native/include/core/CWIShimScript.h`. Every shim call goes
over the IPC bridge ([`IPC_BRIDGE.md`](./IPC_BRIDGE.md)). The HTTP path is
also live: a page request to the wallet port, or to the MetaNet Client (3321)
or HandCash (2121) bridge ports, is caught by
`HttpRequestInterceptor.cpp` (`isWalletEndpoint`), re-pointed at the Hodos
wallet, stamped with `X-Requesting-Domain` and forwarded. `@bsv/sdk`
`WalletClient`'s default transport uses this path.

### `window.CWI` — 28 canonical methods

The `METHODS` array in `CWIShimScript.h`, one-to-one with the BRC-100
endpoints. `makeMethod(name)` maps each to `'/' + name`, so
`CWI.createAction` → `POST /createAction`. No translation, no special cases.
Wrapped in a `Proxy` with an `apply` trap so detached references
(`const fn = CWI.createSignature`) still work.

Grouped as: identity & keys (3), crypto (6), transactions (5), outputs (2),
certificates (6), auth (2), chain info (4).

### `window.yours` — the legacy surface

`window.panda` is a direct alias; Treechat and other Yours-era dApps target
that name. Same dispatch table, no separate gate handling.

| Method | Reaches | Note |
|---|---|---|
| `isReady` | — | Local boolean, always `true` |
| `isConnected()` | `POST /isAuthenticated` | Via `canonical.isAuthenticated` |
| `connect()` | `POST /waitForAuthentication` → `POST /getPublicKey{identityKey:true}` | ⚠️ Returns `addresses: {bsvAddress: null, ordAddress: null, identityAddress: null}` — hardcoded placeholders that the Step 3b polish never replaced |
| `disconnect()` | `DELETE /domain/permissions?domain=<origin>` | Self-revoke |
| `getAddresses()` | `POST /wallet/yours-legacy-addresses` | One round-trip; identity slot may be null without disclosure permission |
| `getPubKeys()` | 3 × `POST /getPublicKey` | Receive + ord-receive (yours-legacy-v1 protocols, keyID `yours-{host}`) + identity. Each `.catch(() => null)` |
| `getBalance()` | `POST /listOutputs` + `GET /wallet/bsv-price` | Sums outputs, converts to USD |
| `getExchangeRate()` | `GET /wallet/bsv-price` | |
| `signMessage({message, encoding?})` | `POST /createSignature` → `POST /getPublicKey` | yours-legacy-v1 sig protocol, counterparty `anyone` |
| `verifyLegacyMessage(msg, sig, idKey)` | `POST /verifySignature` | Interop helper. Ungated — see §2. ⚠️ Ignores `idKey`: it verifies with counterparty `anyone`, so the result does not depend on the key the site passed. Ticket (beta.8 intake): `development-docs/0.4.0-beta.8/tickets/TICKET_verifyLegacyMessage_ignores_identityKey.md` |
| `sendBsv([{address, satoshis}])` | N × `POST /wallet/address-to-script` → `POST /createAction` | Translates the legacy shape to BRC-100 outputs |
| `broadcast({rawtx})` | `POST /internalizeAction`, falling back to `POST /wallet/broadcast` | ⚠️ **`/wallet/broadcast` is not a registered route** — the fallback 404s |
| `encrypt()` / `decrypt()` | `POST /wallet/encrypt-bie1` / `/wallet/decrypt-bie1` | BIE1 (ECIES Electrum), **not** BRC-2 (Data Encryption and Decryption). For Yours-era ciphertexts |
| `getSignatures()` | — | Typed `NOT_IMPL` with a migration note. No safe translation to BRC-100 `createSignature` semantics |
| `getSocialProfile()` | — | Typed `NOT_IMPL`, deferred |
| Ordinal methods | — | Typed `NOT_IMPL` pointing at Phase 3 |

## 4. Drift detection

| Change | Update |
|---|---|
| New `.route()` in `main.rs` | `rust-wallet/CLAUDE.md` (the roster). Touch this doc only if the endpoint is gated or shim-reachable |
| Adding or removing a `dispatch_*` call | §2 here, same commit |
| New entry in `METHODS` or a new `defineLegacyProp` | §3 here, same commit |
| Change to `is_protected_basket` | §2 here and `AUTO_APPROVE_ENGINE.md` §2 branch 3 |
| Change to `isPaymentEndpoint` | §2 here — the header-injection note |
| Change to `is_permission_surface` in `main.rs` | §1 here — the first-party-only row |

A scripted check could diff §2 against
`grep -n 'dispatch_' rust-wallet/src/handlers*.rs` and §3 against the
`METHODS` array. Not built.

## Related

- `rust-wallet/CLAUDE.md` — the endpoint roster this doc deliberately does not repeat
- [`AUTO_APPROVE_ENGINE.md`](./AUTO_APPROVE_ENGINE.md) — what each dispatcher decides
- [`IPC_BRIDGE.md`](./IPC_BRIDGE.md) — how shim calls travel
- `cef-native/include/core/CLAUDE.md` — `CWIShimScript.h` in its header context
