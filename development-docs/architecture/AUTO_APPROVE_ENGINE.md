# Permission Decision Flow (the "auto-approve engine")

> **Last reviewed: 2026-08-03.** Rewritten against code. The previous version
> of this file described the C++ `PermissionEngine`, which was **deleted in
> Phase 2.6-H**, and documented the Matrix C branch order incorrectly. Both
> are corrected below.

## 1. Where the decision is made

**The decision engine is Rust.** There is exactly one.

| Piece | Location | Nature |
|---|---|---|
| Decision logic | `rust-wallet/crates/hodos_permission_engine/src/matrix_c.rs :: decide` | Pure function. No I/O, no globals, no clock. 40 unit tests. |
| Public entry | `hodos_permission_engine/src/lib.rs :: decide` | Thin delegate to `matrix_c::decide` |
| Input type | `hodos_permission_engine/src/context.rs :: PermissionContext` | Plain data — `CallKind`, `TrustLevel`, grant flags, cap numbers |
| Output type | `hodos_permission_engine/src/decision.rs :: PermissionDecision` | `#[serde(tag="kind")]` sum: `Silent{reason}` / `Prompt{prompt_type, reason}` / `Deny{reason}` |
| Context assembly + HTTP envelopes | `rust-wallet/src/permission_service/` (`context_builder.rs`, `request_gate.rs`, `state.rs`, `audit.rs`) | Reads the DB, calls `decide`, turns the answer into 200 / 202 / 403 |
| Middleware wiring | `rust-wallet/src/main.rs :: domain_trust_mw` | `.wrap(middleware::from_fn(domain_trust_mw))` — runs domain trust before every handler |

**C++ makes no permission decisions.** `cef-native/src/core/PermissionEngine.cpp`
and `SessionManager.cpp` do not exist. Any comment in the tree still naming
them is a dead reference, not code. What C++ does own is listed in §4.

### The three answers

| Decision | Wire form | What happens |
|---|---|---|
| `Silent` | Rust runs the handler, returns `200` | Call proceeds; if it was a payment, the **gold pill** fires (§5) |
| `Prompt(type)` | `202` + PENDING envelope | C++ opens the modal named by `promptType`, waits, replays with `X-User-Approved` |
| `Deny` | `403` + `{error, code: "ERR_PERMISSION_DENIED", reason}` | Error surfaces to the page. No modal, no retry |

## 2. Matrix C — the cascade

Top-down, **first match wins**. This is `matrix_c.rs :: decide`, read in order.

> ⚠️ **Domain trust is branch 1, not privacy perimeter.** The pre-2026-08
> version of this doc had these swapped. The order is load-bearing: a
> `blocked` domain must reach `Deny` before any branch that could return
> `Prompt`. With privacy-perimeter first, a blocked domain asking for an
> identity key would get *prompted* instead of *denied* — a real security
> difference, and one the tests
> `blocked_wins_over_identity_key_opt_in` / `unknown_wins_over_privacy_perimeter`
> exist to pin down.

### Branch 1 — Domain trust (`decide_domain_trust`)

The only branch that can fall through (`Option<PermissionDecision>`).

| `trust_level` | Decision |
|---|---|
| `Blocked` | `Deny(TrustBlocked)` |
| `Unknown` + `manifest_present` | `Prompt(ManifestConnectBundle, NewDomainWithManifest)` |
| `Unknown`, no manifest | `Prompt(DomainApproval, NewDomainNoManifest)` |
| `Approved` | fall through to branch 2 |

Manifest presence is resolved *before* the engine runs, in Rust —
`rust-wallet/src/manifest.rs :: fetch_manifest` (3 s cap, 64 KB cap), called
from `request_gate.rs`. The C++ `ManifestFetcher::Fetch` is no longer on the
production path; `ManifestFetcher::ParseFromJson` still is, because the
interceptor re-parses the manifest bytes Rust embeds in the
`manifest_connect_bundle` 202 payload to populate the modal.

### Branch 2 — Privacy perimeter (`decide_privacy_perimeter`)

Runs for `IdentityKeyReveal`, `CounterpartyKeyLinkage`, `SpecificKeyLinkage`,
`SensitiveCertField`. Reached only on `Approved` trust.

| CallKind | Silent when | Otherwise |
|---|---|---|
| `IdentityKeyReveal` | `identity_key_disclosure_allowed` (persistent, V17 column) **or** `identity_key_session_opt_in` | `Prompt(IdentityKeyReveal)` |
| `CounterpartyKeyLinkage` / `SpecificKeyLinkage` | `key_linkage_session_opt_in` (session only — no persistent column) | `Prompt(KeyLinkageReveal)` |
| `SensitiveCertField` | **never** | `Prompt(CertificateDisclosure, SensitiveCertField)` |

Sensitive certificate fields are the one gate with no opt-out anywhere in the
system. Every other perimeter gate has a user-facing way to go quiet.
`identity_key_disclosure_allowed` defaults **ON** globally
(`default_identity_key_disclosure_allowed`, V19), so identity-key reveal is
silent for most users unless they turn it off.

### Branch 3 — Scoped grants (`decide_scoped_grant`)

Runs for `ProtocolUse`, `BasketAccess`, `CounterpartyUse`. In order:

1. `CounterpartyUse` → `Silent(SilentCounterpartyDefault)` **unconditionally**
   (Phase 2.6-D Fix #3). BRC-42 counterparty derivation is one-sided and
   reveals nothing the dApp does not already hold; prompting per counterparty
   collapsed UX on token dApps that use one counterparty per recipient.
2. `bundled_scope_grant` → `Silent(SilentBundledScopeGrant)` (Fix #4). Set
   when the user ticked the "without prompting each time" box on the connect
   modal (`domain_permissions.bundled_scope_grant`, V22).
3. `scoped_grant_exists` → `Silent(SilentScopedGrantExists)`. A matching row
   in the V18 child tables from a prior "Always allow".
4. Otherwise → `Prompt(ProtocolPermissionPrompt | BasketPermissionPrompt)`.

**Protected baskets never reach step 2 with a grant.**
`request_gate.rs :: dispatch_scoped_grant` forces `scoped_grant_exists=false`
*and* clears `bundled_scope_grant` when the basket matches
`is_protected_basket` — `default`, `backup-*`, or `admin ` prefix. The engine
itself has no basket-name knowledge; this is a caller-side override.

### Branch 4 — Payment (`decide_payment`)

Runs for `CallKind::Payment`, in this order:

1. `payment_scope_kind_missing` is `Some(_)` → `Prompt` for that scope
   (Protocol / Basket / Counterparty). **Scope beats caps.** If a call is both
   missing a scope and over cap, the user approves the scope, the call is
   re-issued, and the cap prompt fires on the second pass.
2. `!bsv_price_available && requested_cents == 0` → `Prompt(PaymentConfirmation, PriceUnavailable)`.
   Note the conjunction: a known cents value computed before the price cache
   lapsed is still evaluated against the caps.
3. `payment_requests_this_minute >= rate_limit_per_min` **and** `rate_limit_per_min > 0`
   → `Prompt(RateLimitExceeded, RateLimit)`.
4. `payment_count_this_session >= max_tx_per_session` **and** `max_tx_per_session > 0`
   → `Prompt(RateLimitExceeded, MaxTxPerSession)`. Same modal shape as rate
   limit, on purpose.
5. `requested_cents > per_tx_limit_cents` → `Prompt(PaymentConfirmation, PerTxLimit)`.
6. `session_spent_cents + requested_cents > per_session_limit_cents`
   → `Prompt(PaymentConfirmation, SessionCap)`.
7. Otherwise → `Silent(SilentWithinCaps)`.

Both cap comparisons are **strict greater-than** — exactly at the cap is
allowed (`payment_at_boundary_is_silent`). The zero-guards on steps 3 and 4
mean "0 = no limit configured", not "0 = block everything".

**Default limits: $1.00 per transaction, $10.00 per session** (100 / 1000 USD
cents), seeded by `migrate_v11_to_v12` onto `settings` as
`default_per_tx_limit_cents` / `default_per_session_limit_cents`, alongside
`default_rate_limit_per_min = 30`. Mirrored on the C++ side as
`WalletSettings::defaultPerTxLimitCents` / `defaultPerSessionLimitCents`
(`SettingsManager.h`) for the settings UI only — Rust holds the enforced copy.

The **"Always notify" toggle** in `DomainPermissionForm` zeroes the three
spending limits so every payment prompts. It deliberately leaves
`rate_limit_per_min` alone (floored to 1 by `parseInt(rateLimitPerMin) || 1`).

### Branch 5 — Certificate disclosure

`CallKind::CertificateDisclosure` (non-sensitive fields only — sensitive ones
were caught by branch 2):

- `scoped_grant_exists` → `Silent(SilentAllCertFieldsApproved)`. The caller
  resolves "every requested field has a `cert_field_permissions` row" and
  signals it through that one flag.
- Otherwise → `Prompt(CertificateDisclosure, CertFieldUnapproved)`.

### Branch 6 — Generic

Anything else on an approved domain → `Silent(SilentGenericApproved)`.

## 3. The cross-layer round trip

```
page JS  window.CWI.createAction({...})
  │
  ├─ (IPC path)  window.__hodos_walletCall → "wallet_call" process message
  │                → simple_handler.cpp :: OnProcessMessageReceived
  │                → HandleIpcWalletCall → runIpcEngineCascade
  └─ (HTTP path) direct request to 127.0.0.1:<WalletPort()>
                   → HttpRequestInterceptor :: isWalletEndpoint
                   → AsyncWalletResourceHandler
  │
  ▼  both paths converge: C++ enriches headers, forwards unconditionally
     X-Requesting-Domain: <host[:port]>              (always, external calls)
     X-Browser-Id / X-Payment-Satoshis /
     X-Payment-Cents / X-Bsv-Price-Available          (payment endpoints only)
  │
  ▼  Rust
     domain_trust_mw            → branch 1, on every request carrying a domain
     handler's dispatch_*()     → branches 2–6 for that CallKind
     hodos_permission_engine::decide(&ctx)
  │
  ├─ Silent → handler runs → 200 ────────────────────► deliver + gold pill (if payment)
  ├─ Prompt → 202 { status, approvalId, promptType,
  │                 engineReason, ttlMs, schemaVersion,
  │                 promptPayload }
  │            → C++ OpenPromptModal(promptType, …)
  │            → PendingRequestManager enrollment
  │            → BRC100AuthOverlayRoot.tsx renders by type
  │            → user resolves
  │            → C++ re-issues the SAME call with X-User-Approved: <approvalId>
  │            → Rust consumes the approval (single-use, body-sha256 bound) → 200
  │            → resume*Response delivers + gold pill (if payment)
  └─ Deny  → 403 ─────────────────────────────────────► error to page
```

**Which layer builds the context.** Rust builds `PermissionContext`
(`permission_service/context_builder.rs`) from the DB plus the headers C++
supplies. C++ contributes exactly the parts it alone can compute: the
originating frame's origin, the CEF browser id, and the satoshi→cents
conversion via `BSVPriceCache`. Everything else — trust level, grant rows,
session counters, caps — is read server-side.

**Session counters live in Rust.** `PermissionService.session_counters`
(`permission_service/state.rs`), keyed by browser id, holds per-session spend,
payment count and the rolling rate window. They are cleared by
`POST /wallet/session/close`, which C++ fires from `TabManager::CloseTab` via
`ClearRustPaymentSessionForBrowser`. Counters resetting on tab close is
deliberate, not a leak.

**Internal calls skip the engine.** No `X-Requesting-Domain` header means the
wallet's own UI is calling, and both `domain_trust_mw` and every `dispatch_*`
return `Proceed` immediately. The trust boundary is the header, injected by
C++ from the calling frame's URL and never settable by page script.

**The approval is single-use and body-bound.** `X-User-Approved` carries an
`approvalId` minted with the 202; `state.rs :: consume_and_verify` checks it
against a sha256 of the request body and burns it. A replay with a different
body fails closed. The certificate path is the one exception — approving a
narrowed field subset changes the body, so that path consumes the id and
instead verifies `requested ⊆ approved` from the C++-injected
`X-Cert-Approved-Fields` header.

**Every decision is audited.** `permission_service/audit.rs` writes to
`permission_audit_log` (V20) via `PermissionAuditRepository`.

## 4. What C++ still owns

Not decisions — everything around them:

| Responsibility | Symbol |
|---|---|
| Deciding a URL is a wallet call at all | `HttpRequestInterceptor :: isWalletEndpoint` — the route table; new endpoints go through it, never around it |
| Origin attribution | `X-Requesting-Domain` from the calling frame; `IsInternalOrigin` for the internal/external split |
| Payment context arithmetic | `AsyncWalletResourceHandler :: extractOutputSatoshis` + `BSVPriceCache` → the `X-Payment-*` headers |
| Opening the modal Rust asked for | `OpenPromptModal` fronting 11 free-function openers (`openDomainApprovalModal`, `openPaymentConfirmationModal`, …) |
| Tracking the in-flight request across the modal | `PendingRequestManager` (`include/core/PendingAuthRequest.h`) |
| Replaying after resolution | `resumeInternalResponse` / `resumeHttpCallbackResponse` / `resumeIpcResponse` |
| The gold pill | `OnWalletCallSuccess` (§5) |
| Session teardown | `ClearRustPaymentSessionForBrowser` → `POST /wallet/session/close` |
| Pre-flight cache of trust level | `DomainPermissionCache`, invalidated by the `domain_permission_invalidate` IPC after any wallet-UI mutation |

`DomainPermissionCache` is an optimization, not a gate. It never grants
anything Rust would refuse — Rust re-reads the row on every call. It also
refuses to cache on fetch failure, so a transient wallet outage cannot poison
it into a wrong answer.

## 5. The gold pill

Every payment that is auto-approved without a modal flashes a **gold pill**
badge on the paying tab. It is the user's only visual signal that money moved
silently, and it must survive every refactor. It is **not** a "green dot" —
do not rename it in code, comments, logs, or docs.

Single emit point:
`HttpRequestInterceptor.cpp :: OnWalletCallSuccess(browserId, domain, cents, wasAutoApprovedPayment, endpoint)`.
Six call sites feed it — the HTTP path, the BRC-121 paid retry, the IPC
silent-approve, and the three modal-resume deliveries. The full chain, the
call-site table and the invariants (no cents floor; mandatory
`Tab::id` translation; ordering ahead of `PaidContentCache::Put`) are
documented once, in `cef-native/src/core/CLAUDE.md`.

`OnWalletCallSuccess` does **no spend accounting**. Spend, rate and count are
recorded in Rust at decision time inside `dispatch_payment`.

## 6. BRC-121 — the paid-content path

The 402 paid-retry chain (`TryHandleBrc121_402`, `InstallAsync402HandlerIfPending`,
`Async402ResourceHandler`) is the one C++ subsystem that spends money on its
own. It is **no longer** a parallel decision cascade: since OQ5 it calls
`POST /wallet/pay402`, whose handler runs `dispatch_payment` like any other
payment. The 202 → modal → `X-User-Approved` replay works the same way; the
armed `approvalId` is stashed and replayed on the paid retry.

After a `2xx` from the upstream retry the handler fires the gold pill, calls
`POST /wallet/broadcast-nosend`, and writes the response into
`PaidContentCache` so a reload serves bytes from disk instead of re-paying.

## 7. Known gaps

- **Dead classifiers in `HttpRequestInterceptor.cpp`.**
  `isProveCertificateEndpoint`, `isGetPublicKeyEndpoint`,
  `isKeyLinkageEndpoint` and `isIdentityKeyStyleGetPublicKey` are still
  defined but have **zero callers** — the gate selection they fed moved to
  Rust in 2.6-G. Only `isPaymentEndpoint` and `isWalletEndpoint` are live.
  Removal is safe and unscheduled.
- **`handleIpcUnknownTrust` is dead code**, and is the last caller of
  `ManifestFetcher::Fetch`. Slated for removal.
- **`ENDPOINT_BASE = 'http://127.0.0.1:31301'` in `CWIShimScript.h`** is
  declared and never read. It is also wrong under `HODOS_DEV=1`. Delete it
  rather than "fixing" it — the shim reaches the wallet over IPC, not HTTP.
- **`signAction` / `processAction` carry no payment dispatcher.** Only
  `create_action`, `send_message`, `pay_402` and `acquire_certificate` call
  `dispatch_payment`. A multi-step sign flow is gated when the action is
  created, not when it is signed. Whether that is sufficient has not been
  re-argued since 2.6-E.

## Related

- [`WALLET_API_MAP.md`](./WALLET_API_MAP.md) — which endpoint gets which dispatcher
- [`IPC_BRIDGE.md`](./IPC_BRIDGE.md) — how page calls reach C++ in the first place
- `rust-wallet/CLAUDE.md` — endpoint roster, migration ledger, default limits
- `cef-native/src/core/CLAUDE.md` — gold-pill chain, interceptor internals
- `development-docs/Sigma-BRC121-Sprint/phase-2.6-engine-to-rust/` — the port that produced this architecture
- `development-docs/FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md` — the original vision doc; read as history, it is now largely realized
