# Wallet API Gate Map

> **Last reviewed: 2026-08-03.** Rewritten. The previous version was a second
> copy of the endpoint roster with a "which C++ gate fires" column describing
> the deleted C++ `PermissionEngine`. Both problems are fixed: the roster now
> lives in one place, and the gate column describes the Rust dispatchers that
> actually run.

## What this doc is — and is not

**Not** an endpoint roster. The 110 route registrations, their handlers and
their HTTP verbs live in **`rust-wallet/CLAUDE.md`**, which is generated
against `main.rs` and is the only place that list should exist. Duplicating it
here is what made the old version rot.

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

So an endpoint is in exactly one of three states:

| State | Meaning |
|---|---|
| **Kind-gated** | Middleware + a `dispatch_*` call. Listed in §2. |
| **Trust-gated only** | Middleware only. Reachable by an approved domain without a further prompt. |
| **Internal-only** | Never called with `X-Requesting-Domain`, so never gated at all. |

## 2. The kind-gated endpoints

Every `dispatch_*` call site in the tree. **16 sites across 15 handlers** —
`prove_certificate` calls two.

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
prefix — `is_protected_basket`) always prompt: the dispatcher forces both
`scoped_grant_exists` and `bundled_scope_grant` to false before calling the
engine.

> **`verifyHmac` and `verifySignature` are NOT gated.** They have no
> `dispatch_*` call and no `check_domain_approved`. The old version of this
> doc claimed both fired a protocol scoped-grant; they do not. Whether
> verify-only operations *should* be gated is a live question — they derive a
> key from a protocol the site may not hold a grant for — but today they are
> trust-gated only.

### `dispatch_payment` — 4 sites

Matrix C branch 4. Caps, rate limit, session counters.

| Endpoint | Handler | Note |
|---|---|---|
| `POST /createAction` | `handlers.rs :: create_action` | The main spend path |
| `POST /sendMessage` | `handlers.rs :: send_message` | BRC-33 relay — a paid call |
| `POST /wallet/pay402` | `handlers.rs :: pay_402` | BRC-121. Called by C++ `Async402ResourceHandler`, not by the shim |
| `POST /acquireCertificate` | `certificate_handlers.rs :: acquire_certificate` | Certifiers charge |

The payment context (`X-Browser-Id`, `X-Payment-Satoshis`, `X-Payment-Cents`,
`X-Bsv-Price-Available`) is computed in C++ and sent as headers; C++ injects
those headers only when `isPaymentEndpoint(endpoint)` matches, and that
function matches exactly `/createAction`, `/acquireCertificate`, `/sendMessage`.
**`/wallet/pay402` is not in `isPaymentEndpoint`** — the BRC-121 path builds
its own headers inside the 402 chain.

> **`signAction` and `processAction` have no payment dispatcher.** A
> multi-step sign flow is gated when the action is created, not when it is
> signed. Flagged in `AUTO_APPROVE_ENGINE.md` §7.

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
nothing more. That includes the BRC-100 read surface (`listActions`,
`listCertificates`, `getHeight`, `getNetwork`, `discoverBy*`, `getVersion`,
`isAuthenticated`, `waitForAuthentication`), the action lifecycle
(`signAction`, `processAction`, `abortAction`, `internalizeAction`) and
`DELETE /domain/permissions` (self-revoke).

Internal-only clusters — wallet CRUD, backup/restore, PeerPay, Paymail,
recipient resolution, price/sync/activity/settings, certificate publish/admin,
debug — are never called with `X-Requesting-Domain` and are outside the
permission system entirely. Their protection is the CORS allowlist plus the
fact that no page-facing shim method reaches them.

## 3. The shim surface

Injected by `cef-native/include/core/CWIShimScript.h`. Every call goes over
the IPC bridge ([`IPC_BRIDGE.md`](./IPC_BRIDGE.md)) — no page ever fetches the
wallet directly.

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
| `verifyLegacyMessage(msg, sig, idKey)` | `POST /verifySignature` | Interop helper. Ungated — see §2 |
| `sendBsv([{address, satoshis}])` | N × `POST /wallet/address-to-script` → `POST /createAction` | Translates the legacy shape to BRC-100 outputs |
| `broadcast({rawtx})` | `POST /internalizeAction`, falling back to `POST /wallet/broadcast` | ⚠️ **`/wallet/broadcast` is not a registered route** — the fallback 404s |
| `encrypt()` / `decrypt()` | `POST /wallet/encrypt-bie1` / `/wallet/decrypt-bie1` | BIE1 (ECIES Electrum), **not** BRC-2. For Yours-era ciphertexts |
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

A scripted check could diff §2 against
`grep -n 'dispatch_' rust-wallet/src/handlers*.rs` and §3 against the
`METHODS` array. Not built.

## Related

- `rust-wallet/CLAUDE.md` — the endpoint roster this doc deliberately does not repeat
- [`AUTO_APPROVE_ENGINE.md`](./AUTO_APPROVE_ENGINE.md) — what each dispatcher decides
- [`IPC_BRIDGE.md`](./IPC_BRIDGE.md) — how shim calls travel
- `cef-native/include/core/CLAUDE.md` — `CWIShimScript.h` in its header context
