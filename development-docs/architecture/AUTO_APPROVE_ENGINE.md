# Auto-Approve Engine

> **Status:** Skeleton — to be filled in Phase 2.5 sub-phase A (planning).
>
> Reference doc for the current C++ `PermissionEngine`. Covers decision
> matrix, call-kind classification, modal dispatch, cache behavior, and
> the gate cascade as it actually runs today.
>
> For the future "engine in Rust" vision, see
> [`../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md`](../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md).

## What the engine is (current state)

The auto-approve engine is the cross-cutting permission decision system in
the C++ browser process. It runs inside
`cef-native/src/core/HttpRequestInterceptor.cpp::AsyncWalletResourceHandler::Open()`
on every wallet HTTP request from a renderer. It decides whether the call
should:

- **Silent** — auto-approve, forward to the wallet immediately
- **Prompt** — show a modal to the user before forwarding
- **Deny** — refuse and return an error to the renderer

The decision logic lives in `cef-native/src/core/PermissionEngine.cpp` as
`PermissionEngine::Decide(const PermissionContext& ctx) -> PermissionDecision`.

## Sections to fill (Phase 2.5 sub-phase A)

### 1. CallKind classification

How endpoint + body is mapped to a `CallKind` enum. Identity, Payment,
ProtocolUse, BasketAccess, CounterpartyReveal, KeyLinkageReveal,
CertificateDisclosure, etc.

Source: `PermissionEngine::ClassifyCallKind()` or equivalent.

### 2. Decision Matrix C (the engine's core)

The actual decision tree:

- Domain trust level (unknown / approved / blocked) ← from
  `domain_permissions` table
- Identity key disclosure flag (column `identity_key_disclosure_allowed`)
- Per-protocol / per-basket / per-counterparty grants (V18 child tables)
- Per-tx limit / per-session cap / rate limit
- Manifest-declared permissions (Phase 1.5 Step 5)
- Special endpoints: `proveCertificate`, `revealCounterpartyKeyLinkage`, etc.

Source: `PermissionEngine.cpp` + the inline cascade in
`AsyncWalletResourceHandler::Open()` (lines ~1879-2400).

### 3. Modal dispatch flow

Each `Prompt` decision triggers a modal:

| Prompt type | Trigger fn | Modal page | Resolution path |
|---|---|---|---|
| `domain_approval` | `triggerDomainApprovalModal` | TBD | TBD |
| `manifest_connect_bundle` | `triggerManifestConnectBundleModal` | TBD | TBD |
| `brc100_auth` | `triggerBRC100AuthApprovalModal` | TBD | TBD |
| `payment_confirmation` | `triggerPaymentConfirmationModal` | TBD | TBD |
| `identity_key_reveal` | `triggerIdentityKeyRevealModal` | TBD | TBD |
| `key_linkage_reveal` | `triggerKeyLinkageRevealModal` | TBD | TBD |
| `certificate_disclosure` | `triggerCertificateDisclosureModal` | TBD | TBD |
| `rate_limit_exceeded` | TBD | TBD | TBD |
| Scoped grants (Protocol/Basket/Counterparty) | TBD | TBD | TBD |

Each row needs:
- Modal page React component path
- What context fields are passed to the modal
- How user resolution flows back (via `PendingRequestManager` →
  `handleAuthResponse`)
- What header(s) get injected on the re-issued request after approval

### 4. Pending request lifecycle

How modal flow uses `PendingRequestManager`:

- `addRequest(domain, method, endpoint, body, handler, type)` registers
- Modal opens, user resolves
- React sends IPC (`brc100_auth_response`, `add_domain_permission`, etc.)
  with the requestId
- `handleAuthResponse(requestId, approved, ...)` pops the pending,
  resumes the handler

### 5. Caches involved

- `DomainPermissionCache` — in-memory mirror of `domain_permissions` table,
  invalidated on `domain_permission_invalidate` IPC from the wallet UI
- `SubPermissionCache` — sub-permission cache (V18 child tables) — same
  pattern
- `IdentityKeyApprovalCache` — in-memory one-shot / session opt-in for
  identity-key reveal
- `KeyLinkageApprovalCache` — same for BRC-72
- `WalletStatusCache` — wallet-exists flag
- `BSVPriceCache` — BSV/USD price for payment-cents computation
- `SessionManager` — per-browser-tab spending + rate tracking

### 6. Payment cents computation

How `extractOutputSatoshis()` + `BSVPriceCache::getPrice()` produce
`preCalculatedCents_` for the payment gate.

### 7. Auto-approval flags propagated to Rust

When the engine decides Silent for identity-key reveal or key-linkage,
the resumed request injects:

- `X-Identity-Key-Approved: true`
- `X-Key-Linkage-Approved: true` (?)

So the Rust wallet's defense-in-depth gate passes through silently.

### 8. Payment success indicator (green-dot animation)

After silent-approved payment succeeds: `payment_success_indicator` IPC
fires from `AsyncWalletResourceHandler::AsyncHTTPClient::onHTTPResponseReceived`
to the header browser, which dispatches a postMessage that the React tab
manager picks up. See memory `project_payment_animation_safeguard`.

### 9. Phase 2.5 IPC bridge impact

Phase 2.5's `wallet_call` IPC bridge currently bypasses this entire
engine (commits 1-4 of Phase 2.5). Commits 5-7 of Phase 2.5 extract the
engine logic into reusable helpers and call them from both the HTTP path
(unchanged) and the new IPC path. See
`../Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md`.

### 10. Known gaps / future work

(To be filled — likely includes: post-MVP work to consolidate two
permission systems into one — see
[`../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md`](../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md).)
