# Auto-Approve Engine

> **Status:** Filled in Phase 2.5 sub-phase A planning (2026-05-30). Reference
> doc for the current C++ `PermissionEngine` as it actually runs today.
>
> Covers CallKind classification, decision matrix, modal dispatch, cache
> behavior, header propagation, and the post-success payment indicator chain.
>
> For the future "engine in Rust" vision and the planned Phase 2.6 migration,
> see [`../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md`](../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md).

## What the engine is (current state)

The auto-approve engine is the cross-cutting permission decision system in the
C++ browser process. Today its decision logic lives in two places that must
stay aligned:

1. **`cef-native/src/core/PermissionEngine.cpp`** — pure-logic
   `PermissionEngine::Decide(ctx)` returning `PermissionDecision{Kind, promptType, reason}`.
   No CEF dependencies. Unit-tested in `cef-native/tests/permission_engine_test.cpp`
   (46+ tests).
2. **`cef-native/src/core/HttpRequestInterceptor.cpp::AsyncWalletResourceHandler::Open()`** —
   lines 1879-2400, the 522-line orchestration cascade that builds the
   `PermissionContext`, calls the engine, dispatches modals, and resumes
   pending requests after user resolution.

Phase 1.5 Step 6 (commits A through E) migrated five gates from inline cascade
to engine-driven (payment, identity-key, key-linkage, scoped grants). The
remaining gates still run inline; the engine runs in shadow mode for those
(decision logged, not enforced). Step 6 Commit F will close that gap.

The engine produces one of three decisions for every shim-reachable wallet
call:

- **Silent** — auto-approve, forward to wallet immediately
- **Prompt(promptType)** — show a modal; wait for user resolution before forwarding
- **Deny** — refuse outright; return an error to renderer

## 1. CallKind classification

The engine's first job is mapping the incoming endpoint + body to one of these
`PermissionCallKind` values (`PermissionEngine.h:32-55`):

| `PermissionCallKind` | Triggered by | Classifier source |
|---|---|---|
| `IdentityKeyReveal` | `/getPublicKey` with identity-key body shape | `isGetPublicKeyEndpoint` + `isIdentityKeyStyleGetPublicKey` |
| `CounterpartyKeyLinkage` | `/revealCounterpartyKeyLinkage` | `isKeyLinkageEndpoint` |
| `SpecificKeyLinkage` | `/revealSpecificKeyLinkage` | `isKeyLinkageEndpoint` |
| `SensitiveCertField` | `/proveCertificate` referencing high-sensitivity fields | `isProveCertificateEndpoint` + field-sensitivity check |
| `CertificateDisclosure` | `/proveCertificate` non-sensitive fields | `isProveCertificateEndpoint` |
| `Payment` | `/createAction`, `/acquireCertificate`, `/sendMessage` | `isPaymentEndpoint` |
| `ProtocolUse` | Body has `protocolID` + `keyID` (createSignature, encrypt, decrypt, createHmac, verifyHmac, encrypt-bie1, decrypt-bie1) | body-peek in context builder |
| `BasketAccess` | `/listOutputs`, `/relinquishOutput` with basket field | body-peek |
| `CounterpartyUse` | Level-2 protocol calls naming a specific counterparty | body-peek |
| `DomainTrust` | First BRC-100 call from a fresh origin (no `domain_permissions` row) | `DomainPermissionCache` miss |
| `GenericApproved` | Anything else on an approved domain | catch-all |

**Where this classification happens today:** inline in
`AsyncWalletResourceHandler::Open()` lines 1929-2210, scattered across the
endpoint-classifier `if/else if` cascade. **Phase 2.5 Commit 5 extracts this
into a `buildPermissionContext(request) -> PermissionContext` helper** so both
the HTTP path and the IPC path can build the same context shape.

The classifiers `isPaymentEndpoint`, `isProveCertificateEndpoint`,
`isGetPublicKeyEndpoint`, `isKeyLinkageEndpoint`, `isWalletEndpoint` are at
`HttpRequestInterceptor.cpp:1673-1692` and `:4470`. See
[`WALLET_API_MAP.md`](./WALLET_API_MAP.md) Appendix A.

## 2. Decision Matrix C (the engine's core)

Decision flow as it runs today (`PermissionEngine::Decide()` body in
`PermissionEngine.cpp`). Top-down — first matching branch wins:

### Branch 1 — Privacy perimeter (`DecidePrivacyPerimeter`)

For `IdentityKeyReveal`, `CounterpartyKeyLinkage`, `SpecificKeyLinkage`,
`SensitiveCertField`:

| Condition | Decision |
|---|---|
| `identityKeyDisclosureAllowed=true` AND kind is `IdentityKeyReveal` | `Silent` (persistent opt-in via V17 column) |
| `identityKeySessionOptIn=true` AND kind is `IdentityKeyReveal` | `Silent` (one-shot session cache) |
| `keyLinkageSessionOptIn=true` AND kind is key-linkage | `Silent` (`KeyLinkageApprovalCache`) |
| Otherwise | `Prompt(matching type)` |

These prompts **fire regardless of `trustLevel`** — even approved domains see
the identity-key reveal modal unless the persistent column or session cache
authorizes it. This is the user's primary privacy safeguard against blanket
"approve everything" auto-grants.

### Branch 2 — Domain trust (`DecideDomainTrust`)

For `DomainTrust` and as a fallback for anything not yet decided:

| `trustLevel` | Decision |
|---|---|
| `"blocked"` | `Deny` |
| `"unknown"` AND no manifest | `Prompt(domain_approval)` |
| `"unknown"` AND manifest exists | `Prompt(manifest_connect_bundle)` (Phase 1.5 Step 5; bundles all manifest-declared permissions into one modal) |
| `"approved"` | continue to next branch |

### Branch 3 — Scoped grants (`DecideScopedGrant`)

For `ProtocolUse`, `BasketAccess`, `CounterpartyUse`:

| Condition | Decision |
|---|---|
| `scopedGrantExists=true` (caller checked V18 child tables) | `Silent` |
| Protected basket (`default`, `backup-*`, `admin *`) | `Prompt(BasketAccess)` always — never auto-granted |
| Otherwise | `Prompt(protocol_permission_prompt / basket_permission_prompt / counterparty_permission_prompt)` |

### Branch 4 — Payment (`DecidePayment`)

For `Payment` kind on `"approved"` trust:

```
1. paymentScopeKindMissing != ""  →  Prompt(matching scope permission)
   (createAction body references a protocol/basket/counterparty without grant)

2. bsvPriceAvailable == false  →  Prompt(payment_confirmation, reason="price_unavailable")
   (silent forward refused because cents can't be verified)

3. requestedCents > perTxLimitCents  →  Prompt(payment_confirmation, reason="per_tx_limit")

4. sessionSpentCents + requestedCents > perSessionLimitCents
                                       →  Prompt(payment_confirmation, reason="session_cap")

5. paymentRequestsThisMinute >= rateLimitPerMin
                                       →  Prompt(rate_limit_exceeded)

6. paymentCountThisSession >= maxTxPerSession
                                       →  Prompt(payment_confirmation, reason="max_tx_per_session")

7. Otherwise  →  Silent
```

### Branch 5 — Certificate disclosure

For `CertificateDisclosure`:

| Condition | Decision |
|---|---|
| Every requested field has a matching `cert_field_permissions` row | `Silent` |
| Any requested field missing a permission row | `Prompt(certificate_disclosure)` per-field |
| Sensitive field (email, dob — see `SensitiveCertField` kind) | Always `Prompt(certificate_disclosure)` regardless of stored permissions |

### Branch 6 — Generic approved

Catch-all: `Silent` for any approved-domain call that didn't match Branches 1-5.

## 3. Modal dispatch flow

Each `Prompt` decision triggers a modal via a `triggerXxxModal()` member
function on `AsyncWalletResourceHandler`. All modals share one HWND
(`notification_browser_`) that hosts `BRC100AuthOverlayRoot.tsx`, which
dispatches on the `type` field to render the right page.

| Prompt type | Trigger fn (line) | Modal page | Resolution IPC |
|---|---|---|---|
| `domain_approval` | `triggerDomainApprovalModal` (L1458) | `DomainPermissionForm` via `BRC100AuthOverlayRoot` | `add_domain_permission` / `deny_domain_permission` |
| `manifest_connect_bundle` | `triggerManifestConnectBundleModal` (L1507) | `ManifestConnectBundle` panel | `manifest_connect_bundle_approve` / `_deny` |
| `brc100_auth` | (legacy) | older auth modal | `brc100_auth_response` |
| `payment_confirmation` | `triggerPaymentConfirmationModal` (L1650) | `PaymentConfirmation` panel | `approve_payment` / `deny_payment` |
| `rate_limit_exceeded` | (same trigger fn, different reason) | `PaymentConfirmation` with rate-limit context | same |
| `identity_key_reveal` | `triggerIdentityKeyRevealModal` (L1596) | `IdentityKeyReveal` panel | `approve_identity_key_reveal` / `deny_identity_key_reveal` |
| `key_linkage_reveal` | `triggerKeyLinkageRevealModal` (L1606) | `KeyLinkageReveal` panel | `approve_key_linkage_reveal` / `deny_key_linkage_reveal` |
| `certificate_disclosure` | `triggerCertificateDisclosureModal` (L1786) | `CertificateDisclosure` panel | `approve_cert_disclosure` / `deny_cert_disclosure` |
| `protocol_permission_prompt` | (scoped-grant trigger, added in Step 6 Commit E) | scoped-grant modal | `grant_scoped_permission` (with kind=protocol) |
| `basket_permission_prompt` | (same — scoped grant) | scoped-grant modal | `grant_scoped_permission` (with kind=basket) |
| `counterparty_permission_prompt` | (same — scoped grant) | scoped-grant modal | `grant_scoped_permission` (with kind=counterparty) |

Each trigger function:

1. Calls `PendingRequestManager::addRequest(domain, method, endpoint, body, this, type)`
   which returns a `requestId`. Reference stored in the manager keyed by
   `requestId`. `this` is the `AsyncWalletResourceHandler` instance — the handler
   stays alive because the manager holds a `CefRefPtr`.
2. Sends `wallet_auth_request_data` IPC to the notification browser with
   `requestId + type + payload`.
3. Returns from `Open()` without calling the CEF callback — CEF holds the
   request open until the handler eventually invokes `callback->Continue()` or
   `callback->Cancel()`.

## 4. Pending request lifecycle

The full request → modal → resolution cycle:

```
1. AsyncWalletResourceHandler::Open() fires for /createAction on unknown domain
2. Open() builds PermissionContext, calls PermissionEngine::Decide() → Prompt(payment_confirmation)
3. Open() calls triggerPaymentConfirmationModal() → addRequest() returns req-12345
4. Browser process sends wallet_auth_request_data IPC to notification_browser_
5. React BRC100AuthOverlayRoot.tsx dispatches on type='payment_confirmation'
   → renders PaymentConfirmation panel with body data + req-12345 in scope
6. User clicks Approve
7. React sends approve_payment IPC with requestId=req-12345 + headers
8. SimpleHandler::OnProcessMessageReceived dispatches → handleAuthResponse(requestId, approved=true, ...)
9. handleAuthResponse pops the PendingAuthRequest, retrieves the handler ref,
   sets header X-Identity-Key-Approved/etc. on the original request as needed,
   and calls handler->resume() (or equivalent)
10. The handler proceeds with the original request through the rest of Open()
    (skipping the gate now that approval headers are set)
11. Request forwards to Rust wallet
12. Wallet returns 200; AsyncHTTPClient::OnRequestComplete fires
13. If wasAutoApprovedPayment: SessionManager::recordSpending() + fire payment_success_indicator IPC
14. Response forwards to renderer
```

The `PendingRequestManager` supports per-domain queuing
(`hasPendingForDomain`, `popAllForDomain`) so multiple in-flight wallet calls
from the same domain only fire one modal — when the user approves, every
queued request resumes. See `PendingAuthRequest.h:71-96`.

## 5. Caches involved

The engine itself is pure (no caches). Its `PermissionContext` is built from
these caches living in `HttpRequestInterceptor.cpp` and adjacent:

| Cache | Purpose | Source of truth | Invalidation |
|---|---|---|---|
| `DomainPermissionCache` | In-memory mirror of `domain_permissions` SQLite table | `GET /domain/permissions` | `domain_permission_invalidate` IPC after any wallet UI mutation; auto-poison-resistant (does NOT cache on fetch failure — see `cache_no_poison_on_failure` memory) |
| `SubPermissionCache` | In-memory mirror of `domain_protocol_permissions`, `domain_basket_permissions`, `domain_counterparty_permissions` (V18 child tables) | `GET /domain/permissions/{protocol,basket,counterparty}` | Same IPC; same no-poison policy |
| `IdentityKeyApprovalCache` | Session-only opt-in for identity-key reveals | (in-memory only; never persisted) | Cleared on tab close + on `domain_permission_invalidate` for the domain |
| `KeyLinkageApprovalCache` | Session-only opt-in for BRC-72 reveals | (in-memory only) | Same as IdentityKeyApprovalCache |
| `WalletStatusCache` | Wallet-exists + locked flag | `GET /wallet/status` | TTL + on wallet lifecycle events |
| `BSVPriceCache` | BSV/USD price for `requestedCents` computation | `GET /wallet/bsv-price` (5-min TTL inside Rust) | Stale-bypass when payment gate fires with `bsvPriceAvailable=false` |
| `SessionManager` (`SessionManager.h`) | Per-`browserId` spend cents + payment count + rate counter | (in-memory only) | `clearSession()` on tab close; rate counter rolls every 60 seconds (`minuteWindowStart`) |

## 6. Payment cents computation

For `CallKind::Payment`, the context builder extracts satoshi total from the
request body and converts to cents:

```cpp
int64_t satoshis = extractOutputSatoshis(body_);  // sums all output.satoshis
double bsvUsd = BSVPriceCache::GetInstance().getPrice();
if (bsvUsd <= 0) {
    ctx.bsvPriceAvailable = false;
    ctx.requestedCents = 0;
} else {
    ctx.bsvPriceAvailable = true;
    ctx.requestedCents = static_cast<int64_t>(
        (satoshis * bsvUsd) / 1'000'000.0  // satoshis → BSV → USD → cents
    );
}
```

The result feeds `DecidePayment` (Branch 4 above). `bsvPriceAvailable=false`
forces a `payment_confirmation` prompt so the user can review the satoshi
amount manually instead of silently forwarding a tx with unverifiable USD
cost. See `PermissionEngine.h:83-91` comment.

## 7. Auto-approval flags propagated to Rust

When the engine decides `Silent` for a privacy-perimeter call, the handler
injects authorization headers on the request before forwarding to Rust. Rust's
defense-in-depth gates require these to be present.

| Header | When set | Rust gate that consumes it |
|---|---|---|
| `X-Identity-Key-Approved: true` | Silent decision on identity-key reveal | `get_public_key` rejects identity-key-style calls from external domains without either this header OR `domain_permissions.identity_key_disclosure_allowed=1` (Phase 1.5 Step 1) |
| `X-Key-Linkage-Approved: true` | Silent decision on key-linkage reveal | `reveal_counterparty_key_linkage` / `reveal_specific_key_linkage` gates (Phase 1.5 Step 1) |
| `X-Requesting-Domain: <host[:port]>` | Always — set from calling frame's origin | Every shim-reachable handler reads it for `check_domain_approved` |

Header injection in `Open()` at line ~3030 (and similar for key-linkage). When
the engine prompts and the user approves, `handleAuthResponse` adds the header
to the resumed request so the now-approved call passes Rust's gate.

## 8. Payment success indicator (green-dot animation)

Critical UX safeguard — every auto-approved payment fires the green-dot tab
badge animation so users have a visible signal even when no modal appeared.

**Fire sites (2):**

| Fire site | When | Path |
|---|---|---|
| `AsyncWalletResourceHandler::AsyncHTTPClient::OnRequestComplete` (~L2900) | `/createAction` silent-approve success | The standard payment path |
| `firePaymentSuccessIpc()` helper (~L3876) | BRC-121 paid retry success (after `/wallet/pay402` → server 200) | The 402-paid-content path |

**IPC chain:**
```
Wallet returns 200
  → AsyncHTTPClient::OnRequestComplete (or firePaymentSuccessIpc)
    → SessionManager::recordSpending(browserId, cents)
    → CefBrowser headerBrowser = SimpleHandler::GetHeaderBrowser()
    → TabManager::GetTabIdForBrowserIdentifier(browserId)  ← CEF browserId → Tab::id translation
    → CefProcessMessage("payment_success_indicator", {browserId: tabId, domain, cents})
    → headerBrowser->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg)
  → simple_render_process_handler.cpp:1051 receives, dispatches via window.postMessage
  → useTabManager.ts:141 listens, matches by tab.id, triggers green-dot animation
```

**Why the Tab::id translation matters:** React's tab list keys by `Tab::id`,
not by CEF `CefBrowser::GetIdentifier()` — these are different counters. Phase
1.5 Step 0 fixed a subtle bug where the indicator fired but the animation
showed on the wrong tab (or no tab) because the domain-match heuristic was
poisoned by `/payment-pending` and `data:` URLs. See memory
`payment_animation_safeguard` and `payment_animation_ids`.

**Phase 2.5 commit 6 must preserve this:** when the IPC bridge runs the engine
and silent-approves a payment, the indicator IPC must still fire. The
extracted helper `OnWalletCallSuccess(browserId, domain, cents, wasAutoApprovedPayment)`
(per Phase 2.5-A Decision 4) will encapsulate `recordSpending` + indicator
fire as one call, called from both the HTTP path and the new IPC path.

## 9. Phase 2.5 IPC bridge impact

Phase 2.5 commits 1-4 routed the V8 shim through `wallet_call` IPC →
`SyncHttpClient::Post` directly, **bypassing `AsyncWalletResourceHandler::Open()`
entirely.** Effect on this engine:

- `PermissionEngine::Decide()` not called on the IPC path
- No modal dispatch, no PendingRequestManager registration
- No `SessionManager::recordSpending`, no payment indicator IPC
- Only Rust's `check_domain_approved` fires (coarse-grained)
- All sub-permission, per-tx-cap, per-session-cap, rate-limit, identity-key,
  key-linkage, cert-disclosure logic silently skipped

**Phase 2.5 commits 5-7 close this gap:**

- **Commit 5 (extraction, no behavior change):** extract context builder +
  decision orchestration + modal dispatch into reusable free functions per
  Phase 2.5-A Decisions 1-3 (see
  [`../Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md`](../Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md)).
- **Commit 6 (IPC wiring):** call the extracted gate from `wallet_call`'s IPC
  handler; extend `PendingAuthRequest` with `ResumeKind` enum + `requestId` +
  `CefRefPtr<CefFrame>` so modal resolution resumes either an HTTP or IPC
  request.
- **Commit 7 (smoke):** verify github.com + treechat.io see full gate cascade
  including green-dot animation.

## 10. Known gaps / future work

### Phase 2.5 immediate

- **BRC-121 paid retry path bypass** (`brc121_bypasses_permission_engine`
  memory). `TryHandleBrc121_402` has its own inline cap-check cascade in
  `HttpRequestInterceptor.cpp:3717-3791` that mirrors but does NOT call the
  engine. Phase 1.5 polish item G is the planned migration; deferred until
  after Phase 2.5 lands.

- **Inline classifier scatter.** Today's CallKind classification is spread
  across the inline cascade in `Open()` (lines 1929-2210). Commit 5 will
  consolidate into a single `buildPermissionContext()` helper.

### Phase 2.6 — the planned full migration

[`../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md`](../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md)
captures the engine-to-Rust vision. Phase 2.6 implements it. Per Phase 2.5-A
sequencing discussion (2026-05-30), Phase 2.6 starts immediately after Phase
2.5 closes:

- Engine logic ports to Rust as the canonical implementation
- C++ becomes a thin proxy: builds context → asks Rust over IPC → handles
  `202 PENDING` by dispatching modal → re-issues with `X-User-Approved` header
- `DomainPermissionCache` + `SubPermissionCache` + `IdentityKeyApprovalCache` +
  `KeyLinkageApprovalCache` collapse to a single `Set<approved_host>` mirror
  for pre-flight only
- `SessionManager` migrates to Rust (session caps are engine state)
- `PaymentSuccessIndicator` stays C++ (UI concern)
- Modal dispatch stays C++ (UI concern); `OpenPromptModal` is invoked from
  `202 PENDING` response handler instead of from `RunPermissionGate` callback
- Per-endpoint feature-flag migration over ~2-4 weeks; shadow-mode parity
  comparison + 1 week soak; C++ engine deleted last

### Open Phase 1.5 polish items (in priority order)

- **G** — BRC-121 inline cascade migration to engine (mentioned above)
- **H** — Back button = Deny (modal cancellation handling)
- **L** — Fallback indexer chain (Phase 1.6 covered this for broadcast/UTXO;
  still needs polish for ARC txStatus + identity-key resolution)
- **I** — IdentityResolver cache stale after cert unpublish

---

## Related docs

- [`WALLET_API_MAP.md`](./WALLET_API_MAP.md) — every endpoint × gate × shim-call mapping
- [`../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md`](../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md) — Phase 2.6 vision
- [`../Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md`](../Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md) — Phase 2.5 plan + commit acceptance criteria
- [`../Sigma-BRC121-Sprint/phase-1.5-brc100-surface-completion/PERMISSION_UX_DESIGN.md`](../Sigma-BRC121-Sprint/phase-1.5-brc100-surface-completion/PERMISSION_UX_DESIGN.md) — Matrix C source document
