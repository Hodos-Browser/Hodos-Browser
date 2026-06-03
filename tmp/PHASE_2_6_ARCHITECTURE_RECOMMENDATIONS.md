# Phase 2.6 — Architecture Recommendations (research brief)

> Produced by architecture-reviewer subagent 2026-06-02 in support of Phase 2.6
> planning. Read this BEFORE the architecture discussion session. Every
> load-bearing claim is cited file:line.

## TL;DR

Phase 2.6 should ship as a **dual-layer Rust crate** — a pure `permission_engine` crate that owns decision logic + a thin `permission_service` actix integration that owns approval state + audit. The C++ side keeps **modal dispatch, payment indicator, and IsInternalOrigin bypass** as UI concerns; everything else (cache mirrors, SessionManager, BRC-121 cap cascade) collapses into Rust. The `202 PENDING` payload carries **JSON URL-style query params** (not query strings) so the Phase 2.5 React `applyParams` consumer keeps working unchanged. Migration runs with **one feature flag per CallKind class** (5 flags, matching the engine's branch structure) and shadow-mode runs **C++-authoritative + Rust-async-compare** so production traffic doesn't double-roundtrip.

## What I read

Read end-to-end and cited:
- `development-docs/FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md` (vision, full)
- `development-docs/architecture/AUTO_APPROVE_ENGINE.md` (full)
- `development-docs/architecture/WALLET_API_MAP.md` (full — 13 clusters, 95 endpoints)
- `development-docs/Sigma-BRC121-Sprint/phase-2-window-cwi-shim/COMMIT_6_DESIGN.md` (full — the seam reference)
- `development-docs/Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md` (full)
- `development-docs/Sigma-BRC121-Sprint/phase-1.5-brc100-surface-completion/PERMISSION_UX_DESIGN.md` §1–§3 (Matrix C source)

Code spot-reads (file:line cited inline below):
- `cef-native/include/core/PendingAuthRequest.h` (full, 169 lines)
- `cef-native/include/core/PermissionEngine.h` (full, 162 lines)
- `cef-native/include/core/SessionManager.h` (full, 84 lines)
- `cef-native/src/core/HttpRequestInterceptor.cpp` — `IsInternalOrigin` L1365-1374, `OnWalletCallSuccess` L1320-1350, `HandleIpcWalletCall` L2538-2585, `Open()` internal-origin branch L2616-2697, IPC silent worker `wasAutoApprovedPayment` derivation L3683-3709, BRC-121 `TryHandleBrc121_402` L4946-5108, `firePaymentSuccessIpc` L4835-4855, `CreateNotificationOverlayTask` L684-705, `IdentityKeyApprovalCache` L737-758
- `cef-native/src/handlers/simple_handler.cpp` — wallet_call IPC dispatch L1634-1686, `domain_permission_invalidate` IPC L4411-4447
- `cef-native/src/handlers/simple_render_process_handler.cpp` — CWI shim injection L880-898 (unconditional inject on every https external main frame)
- `frontend/src/pages/BRC100AuthOverlayRoot.tsx` — `applyParams` query-string consumer L335-440
- `rust-wallet/src/main.rs` — AppState L80-152 with `Arc<services::WalletServices>` facade; actix-cors localhost-allowlist L756-763; route registrations L785-820+
- `rust-wallet/src/handlers.rs` — `check_domain_approved` L572-612; `pay_402` L16585-16660 (already runs check_domain_approved — defense-in-depth in place)

## Cross-cutting observations

These shape multiple answers — read first:

1. **The shim injects on every external https main frame unconditionally.** `simple_render_process_handler.cpp:889-898` — there is no per-domain pre-flight check today. The vision doc's "`should_inject_shim_on_this_page(host)` → `Set<approved_host>` mirror" is **net-new architecture, not a preservation of an existing pattern.** This makes Q4 less load-bearing than the vision implies (we're not preserving sub-ms behavior; we're adding it). Consider deferring the mirror until a real performance signal materializes.

2. **`wasAutoApprovedPayment` is derived locally in C++ from `isPaymentKind` (URL classifier) + HTTP success + error-body parse.** `HttpRequestInterceptor.cpp:3683-3694`. It does NOT require Rust to tell C++ "this was a payment that auto-approved." This means Q3's signaling problem is smaller than the vision suggests — Rust doesn't need to round-trip a `_paymentSuccess` field; C++ already has the inputs from the response code + the endpoint string.

3. **The React modal layer consumes URL-style query strings, not JSON.** `BRC100AuthOverlayRoot.tsx:335-440` builds state from `URLSearchParams`. The C++ `CreateNotificationOverlayTask` ships data through `extraParams_` in the notification overlay URL. This is the path of least resistance for Q1: keep the same shape from Rust → C++ → React.

4. **Rust's `AppState` already has the `Arc<services::WalletServices>` facade pattern (Phase 1.6).** `main.rs:87`. This is the natural insertion point for an `Arc<PermissionService>` field — it's a proven pattern in the codebase, not novel.

5. **Defense-in-depth is already pervasive in Rust** — every shim-reachable handler runs `check_domain_approved` (`handlers.rs:572-612`), and Phase 1.5 Step 1 added `X-Identity-Key-Approved` / `X-Key-Linkage-Approved` header gates. **Q8 answers itself**: internal-only endpoints don't need migration because Rust already enforces "no X-Requesting-Domain header → internal, allow through" (`handlers.rs:577-582`).

6. **PendingAuthRequest already has `ResumeKind::kInternal` reserved as a placeholder for 2.6.** `PendingAuthRequest.h:23-27`. The 2.5 work was deliberately forward-compatible. Q2's audit/retention questions are NOT blocking — the C++ struct is ready.

7. **BRC-121 has a parallel inline cap cascade that bypasses the engine TODAY in C++** (`TryHandleBrc121_402` at `HttpRequestInterceptor.cpp:4946-5108`), but `pay_402` in Rust already runs `check_domain_approved` (`handlers.rs:16612`). Under 2.6, the engine moves to Rust and `pay_402` becomes an internal Rust→engine call — the BRC-121 path can be unified for free if we sequence it right. **Q7 is mostly a sequencing question, not a design question.**

---

## Per-question recommendations

### Q1 — 202 PENDING payload schema

**Recommendation:** Response body is JSON with this shape (versioned):
```json
{
  "status": "pending",
  "approvalId": "<nonce>",
  "promptType": "payment_confirmation|identity_key_reveal|...",
  "promptPayload": { /* type-specific fields matching today's URL query params */ },
  "engineReason": "per_tx_limit|session_cap|...",
  "ttlMs": 600000,
  "schemaVersion": 1
}
```
`promptPayload` carries the **same field set that `applyParams` consumes today** (BRC100AuthOverlayRoot.tsx:335-440): `satoshis`, `cents`, `exceededLimit`, `perTxLimit`, `perSessionLimit`, `sessionSpent`, `rateLimit`, `maxTxPerSession`, `fields`, `certType`, `certifier`, `kind`, `verifier`, `protocol`, `keyID`, `protocolLevel`, `protocolName`, `protocolKeyId`, `protocolCounterparty`, `basket`, `basketAccess`, `counterparty`, `manifest`. C++ translates JSON → URL query string when calling `CreateNotificationOverlayTask` so the React layer is **completely unchanged**.

`engineReason` is a separate field (not nested in promptPayload) so audit consumers and telemetry don't have to know prompt-type-specific shapes. `manifest` (the manifest_connect_bundle subtree) stays a stringified JSON value inside promptPayload since that's what `applyParams` already parses (BRC100AuthOverlayRoot.tsx:425-434).

**Reasoning:**
- C++ modal trigger functions today consume URL query strings via `extraParams_` (`HttpRequestInterceptor.cpp:686-704`). The cheapest migration keeps that shape; C++ becomes a JSON→query-string translator (~30 lines).
- The React `applyParams` consumer (`BRC100AuthOverlayRoot.tsx:335-440`) is 100+ lines of param-reading. Rewriting it to consume JSON is a separate, dangerous refactor that should NOT be coupled to 2.6.
- Versioning the schema upfront is cheap insurance — without it the first cross-version skew between C++ build and Rust build produces silent prompt-data corruption.
- `engineReason` separated from `promptPayload` because audit log + cents-rolling telemetry need a typed reason field; nesting it would force every consumer to know the prompt-type-specific shapes.

**Severity:** **BLOCKER**. The schema is the contract between Rust engine and C++ modal layer; getting it wrong forces a doc-level rewrite mid-migration.

**2nd-order consequences:** Q3 — `engineReason` field at the top level gives Rust a natural place to also carry `_paymentSuccess` info on the 200 response (analogous shape, paired with the 202 shape — avoids one-off telemetry plumbing). Q5 — schema versioning lets per-CallKind feature flags ship without breaking other CallKinds. Q10 — pure engine crate emits a `PermissionDecision` enum; the service layer serializes it to this 202 shape.

---

### Q2 — `approvalId` lifecycle (TTL + replay)

**Recommendation:**
- **TTL: 10 minutes** (matches `kPromptAuthTimeoutMs = 600_000` already in production — `COMMIT_6_DESIGN.md` §12 risks table cites this value).
- **Single-use.** approvalId is consumed atomically by the first request carrying `X-User-Approved: <approvalId>`. The Rust `pending_approvals` map removes the entry on first lookup.
- **No re-use within a session.** Re-use makes audit ambiguous and adds a separate replay-window threat model for nothing — when a user grants "Always allow for this site" they get a persistent V18 row instead.
- **Orphan policy: timeout-driven.** If the user closes the tab or the dApp never re-issues, the approval simply expires after TTL. No active cancellation. Matches Q5 lean in COMMIT_6_DESIGN (deferred unless we see a real problem).
- **Audit log: SQLite table `permission_audit_log`** with `(approval_id, domain, endpoint, call_kind, engine_reason, decision, user_decision, body_hash, created_at, resolved_at, resolved_via_method)`. **Retain 90 days** (matches `monitor_events` purge pattern at 7d → can be more generous since this is much lower volume). `body_hash` not raw body — privacy + size.
- **Replay protection: the single-use rule handles it.** A leaked approvalId can't be reused once the request succeeds.

**Reasoning:**
- `PendingRequestManager` in C++ today is single-use (`PendingAuthRequest.h:95-104` — `popRequest` erases on read). Rust pattern should mirror.
- 10-minute timeout already proven in production (`COMMIT_6_DESIGN.md` §12 — recently bumped from 60s after SocialCert cert acquire failed mid-modal).
- Body-hash (sha256) is the standard wallet-toolbox audit shape and avoids storing potentially-sensitive payloads.
- 90-day retention is a guess — needs user confirmation. Could be longer (regulatory) or shorter (storage). **Flagged in §risks below.**

**Severity:** **OPEN-QUESTION-OK.** The struct fields are already in `PendingAuthRequest`; lifecycle policy can be decided per-commit during the migration. The audit table schema needs to land before the first endpoint flips, but the schema itself isn't a foundational decision.

**2nd-order consequences:** Q3 — the audit log naturally holds `cents` + `wasAutoApprovedPayment` for the 200 path too, eliminating the need for an out-of-band IPC channel.

---

### Q3 — `SessionManager` migration boundary + payment success indicator

**Recommendation:** **(d) Keep `wasAutoApprovedPayment` derivation in C++; Rust only returns enough that C++ can compute it.**

Specifically: Rust returns 200 with the wallet response body; C++ derives `isPaymentKind` from the endpoint URL (the same classifier it uses today — `isPaymentEndpoint` at `HttpRequestInterceptor.cpp:~1673`), and derives `wasAutoApprovedPayment = ok && isPaymentKind && !errorInResponseBody` (the exact derivation already running on the IPC silent path at `HttpRequestInterceptor.cpp:3683-3694`).

`cents` extraction: keep in C++ for the indicator path. Rust's response body already carries the satoshi amount in the createAction response shape; C++ multiplies by BSV price (BSVPriceCache stays C++) the same way it does today.

`SessionManager`: **migrate to Rust** because session caps are engine state. The 200/202/403 contract is the natural seam — Rust increments `paymentRequestsThisMinute` and `paymentCountThisSession` when it returns 200 to a Silent payment decision, and reads them when deciding subsequent calls. C++'s SessionManager singleton becomes a write-through cache for the indicator UI to read recent spending (or, simpler, delete it from C++ entirely — `OnWalletCallSuccess` can pull the cents from the response body and the indicator IPC needs `(tabId, domain, cents)` only).

**Reasoning:**
- The derivation `wasAutoApprovedPayment = ok && isPaymentKind && !isErrorInResponse` (`HttpRequestInterceptor.cpp:3693-3694`) does NOT need anything from Rust. Adding a response field would be redundant signaling.
- `OnWalletCallSuccess` at `HttpRequestInterceptor.cpp:1320-1350` is a 30-line UI helper. Splitting `recordSpending` (engine state) off to Rust and leaving the indicator-fire in C++ matches `COMMIT_6_DESIGN.md` Q2 + the locked seam doc (PHASE_2_5_IPC_REFACTOR.md "Phase 2.6 fit" row for D4).
- Memory `payment_animation_safeguard` is the load-bearing constraint. The chain is `OnWalletCallSuccess` → `TabManager::GetTabIdForBrowserIdentifier` → IPC to header browser → `useTabManager.ts:141`. Every step except `recordSpending` is UI-thread / browser-state code. Forcing it to Rust would mean Rust serializes a tab-ID → C++ deserializes — pure indirection.
- BRC-121's `firePaymentSuccessIpc` at `HttpRequestInterceptor.cpp:4835-4855` already increments `incrementRateCounter` + `incrementPaymentCount` at success time (not gate time). Under 2.6 these increments move to Rust's pay_402 path — same call-class generalization.

**Severity:** **BLOCKER for the SessionManager piece** (the migration boundary directly affects every payment-path commit). **OPEN-QUESTION-OK for the indicator detail** (we'll either keep `OnWalletCallSuccess` as-is or thin it further during migration).

**2nd-order consequences:** Q4 — `SessionManager` going to Rust eliminates one of the C++ cache singletons; the "Set<approved_host> mirror" gets smaller. Q5 — the Payment CallKind flag can flip independently of identity-key/cert-disclosure flags because their state migrates separately.

---

### Q4 — `DomainPermissionCache` → `Set<approved_host>` collapse + invalidation channel

**Recommendation:** **(c) Pull-style refresh on shim-injection — and don't ship the `Set<approved_host>` mirror in Phase 2.6 at all.**

Rationale: The shim is injected today on **every external https main frame unconditionally** (`simple_render_process_handler.cpp:889-898`). There is no per-domain pre-flight check today. The "tiny C++ mirror" in the vision doc is **adding** a new behavior, not preserving an existing one. Under 2.6:

- `OnContextCreated` calls `cefMessage.send('shim_should_inject', [origin])` synchronously (it's already a UI-thread blocking IPC pattern in CEF — `wallet_status_check` etc.).
- The browser process handler routes that to Rust over HTTP `GET /wallet/should-inject-shim?domain=X`. Localhost RTT is <1ms — same order as a cache hit, no architectural change.
- Rust returns `{ inject: bool }`. Decision: still inject for ALL external https today (no UX change from 2.6); the endpoint exists as a forward seam for future per-domain shim gating.
- **Skip the mirror entirely.** No long-poll, no SSE, no bidirectional IPC. The mirror is a premature optimization addressing a problem that doesn't exist today (shim is injected unconditionally).

Invalidation chain stays as-is: `domain_permission_invalidate` IPC at `simple_handler.cpp:4411-4447` continues to drop the four C++ caches (DomainPermissionCache, SubPermissionCache, IdentityKeyApprovalCache, KeyLinkageApprovalCache). After 2.6, those caches **no longer exist in C++** — invalidation IPC fires from wallet UI to Rust directly via `POST /domain/permissions/invalidate`, and Rust's engine reads from SQLite (or its own in-memory mirror, an engine-side concern).

**Reasoning:**
- The mirror addresses a "pre-flight" decision (`should_inject_shim_on_this_page`) that **isn't being made today** — verified at `simple_render_process_handler.cpp:889-898`. Adding it to Phase 2.6 conflates two refactors.
- Bidirectional IPC for a `Set<approved_host>` push channel is a meaningful net-new mechanism. CEF's IPC is per-frame; pushing to header browser, then having header browser broadcast to renderer subprocesses, is a couple hundred lines of plumbing.
- Pull-style is consistent with how every other localhost call in Hodos works (`SyncHttpClient::Get` everywhere).
- Memory `domain_permission_cache_invalidation` notes the current chain works and has tests; preserving it through 2.6 is lower risk than rebuilding it bidirectionally.
- Sub-permission level: the IPC carries the domain only today. SubPermissionCache + IdentityKeyApprovalCache + KeyLinkageApprovalCache all key by domain too (`HttpRequestInterceptor.cpp:737-758`). The boolean "is X in `domain_permissions` with trust=approved" is the canonical truth; sub-permissions can be resolved on the wallet call itself, not pre-flight.

**Severity:** **OPEN-QUESTION-OK.** This is a "what to defer" decision, not a "must answer before sub-phase 1" decision. If we follow the recommendation, Phase 2.6 deletes the four C++ caches and the new shim-gating endpoint is a follow-on item with its own plan doc.

**2nd-order consequences:** Q5 — fewer migration units (caches don't migrate, they delete). Q10 — Rust engine owns its own in-memory cache shape; no contract with C++ to align on. Q6 — shadow-mode comparison gets easier because there's no C++ cache state to drift from.

---

### Q5 — Per-endpoint migration unit + feature flag scope

**Recommendation:** **5 feature flags, one per CallKind class**, matching the engine's branch structure exactly:

| Flag | Covers | Endpoints |
|---|---|---|
| `engine_rust_payment` | `Payment` kind | `/createAction`, `/signAction`, `/processAction`, `/acquireCertificate`, `/sendMessage`, `/transaction/send`, `/wallet/pay402` |
| `engine_rust_privacy_perimeter` | `IdentityKeyReveal`, `CounterpartyKeyLinkage`, `SpecificKeyLinkage`, `SensitiveCertField` | `/getPublicKey` (identity-key shape), `/revealCounterpartyKeyLinkage`, `/revealSpecificKeyLinkage`, `/proveCertificate` (sensitive fields) |
| `engine_rust_scoped_grant` | `ProtocolUse`, `BasketAccess`, `CounterpartyUse` | createSignature, createHmac, encrypt, decrypt, encrypt-bie1, decrypt-bie1, listOutputs, relinquishOutput, sendMessage (counterparty subset), listMessages, acknowledgeMessage |
| `engine_rust_cert_disclosure` | `CertificateDisclosure` (non-sensitive) | `/proveCertificate` (non-sensitive fields) |
| `engine_rust_domain_trust` | `DomainTrust`, `GenericApproved` | everything else (first-touch domain_approval / manifest_connect_bundle paths) |

**Mechanism:** environment variables read at Rust process start, exposed through `AppState.engine_flags: EngineFlags` (a struct of `bool` fields). The flags affect **both production AND shadow-mode** behavior (one set of flags, two modes — see Q6).

**Reasoning:**
- Matches `PermissionEngine.h:32-55` `PermissionCallKind` enum exactly — no impedance mismatch between flag layer and engine layer.
- 5 flags is the right granularity: per-endpoint is 28+ flags (too many — operational nightmare); per-phase is 1 flag (defeats the purpose — no rollback for one CallKind class at a time). Per-gate-type is conceptually similar to per-CallKind but the engine internally already groups by branch, so per-CallKind matches the implementation.
- Env var matches existing dev/prod isolation pattern (CLAUDE.md "Dev Runbook" section — `HODOS_DEV=1` is the precedent).
- The PHASE_2_5_IPC_REFACTOR per-commit acceptance criteria already grouped tests by gate type (Commit 5 done-when criteria #1 — see PHASE_2_5_IPC_REFACTOR.md §"Done when"). Continuity with the 2.5 sub-phase shape.
- AppState already takes facade structs (`Arc<services::WalletServices>` at `main.rs:87`) — adding an `engine_flags: EngineFlags` field is idiomatic.

**Severity:** **BLOCKER.** Flag count + scope reshapes the sub-phase structure. With 5 flags, you get 5 ordered sub-phases (Privacy Perimeter → Scoped Grant → Payment → Cert Disclosure → Domain Trust, ordered from highest-value/lowest-volume to lowest-value/highest-volume so rollback is cheap if something breaks).

**2nd-order consequences:** Q6 — shadow mode runs per-flag (each flag flip moves one CallKind from C++-authoritative to Rust-authoritative). Q10 — pure engine crate's API surface is the CallKind enum + Decide(); flags only affect which **path** the wallet handler takes to call Decide(), not the engine itself.

---

### Q6 — Shadow-mode parity comparison

**Recommendation:** **(a) C++ authoritative, Rust runs in shadow on every request, agreements/disagreements logged async to a new audit-style table.**

Concretely:
- For each call where the relevant `engine_rust_<class>` flag is OFF (shadow mode), C++ runs its inline gate, decides, fires the modal / forwards normally — production semantics unchanged.
- C++ ALSO builds a `PermissionContext` (already does for shadow logging today per `phase15_step6_commit_a` memory) and POSTs it to Rust's `POST /engine/shadow-decide` endpoint **fire-and-forget on a worker thread** (TID_FILE_USER_BLOCKING).
- Rust's shadow handler runs the engine, compares the result to a hint C++ sent (`{ cppDecision, cppPromptType, cppReason }`), and writes either an "agreement" row or a "disagreement" row to SQLite `engine_shadow_log`.
- When a flag flips ON, the same wallet call now goes through Rust authoritatively. C++ still runs its inline gate **in shadow**, posts its result + Rust's actual decision, and the same comparison fires (just inverted).

**Disagreement log shape:**
```
(call_kind, endpoint, domain, cpp_decision, rust_decision, cpp_reason, rust_reason, context_hash, observed_at)
```
Reviewed via wallet UI under Settings > Engine Shadow Log (gated by a debug build flag — not user-facing).

**Reasoning:**
- Option (d) — sequential — doubles latency on critical path. Hard reject for production.
- Option (c) — 1% sample — too slow to surface drift before flag flip.
- Option (e) — post-hoc — requires storing per-request context, more storage churn than just logging the decision.
- Option (b) — Rust authoritative in shadow — has the inverse latency problem: we're now ENABLED on a path that wasn't supposed to be production yet.
- Option (a) — async fire-and-forget — adds zero critical-path latency (the worker post is ~10μs; the HTTP round-trip happens off-thread). Memory `phase15_step6_commit_a` confirms shadow-mode logging is a proven pattern in Hodos (commit `1aeb878`).
- The existing `monitor_events` table is the pattern for audit-style logs (CLAUDE.md V20 migration). New table `engine_shadow_log` follows the same pattern with the same purge cadence.

**Critical detail:** the C++ → Rust shadow POST is fire-and-forget; if Rust's shadow endpoint is slow or down, **production is unaffected**. The shadow run NEVER blocks the critical path.

**Severity:** **OPEN-QUESTION-OK** for the table shape; **BLOCKER for the fire-and-forget rule.** Anything sequential is a non-starter in production.

**2nd-order consequences:** Q5 — flag flips become "observe shadow log for N hours → flip → observe disagreement log inverted for M hours → finalize." Q10 — pure engine crate is what makes shadow trivial; if engine is coupled to Actix state we'd pay more to construct shadow contexts.

---

### Q7 — BRC-121 paid retry path (`TryHandleBrc121_402`)

**Recommendation:** **Defer BRC-121 cap migration to a post-2.6 polish commit (sequenced as the LAST migration unit), and migrate it Rust-internal — never round-trip through C++.**

Specifically:
- Phase 2.6 main migration covers all 5 CallKind flags (Q5).
- After all 5 flip ON and C++ engine is dead code, kick off "Phase 2.6 polish G" (the renamed Phase 1.5 polish item G from memory `phase16_d_d_3_landed` notes).
- `TryHandleBrc121_402` at `HttpRequestInterceptor.cpp:4946-5108` becomes a thin "is this a 402 with BRC-121 headers? if so, call Rust's pay_402; otherwise return false." Rust's pay_402 handler ALREADY calls `check_domain_approved` (`handlers.rs:16608-16614`); it gets extended to run the **same Payment-CallKind engine decision** as createAction, internally.
- The BRC-121-specific inline cascade at `HttpRequestInterceptor.cpp:5022-5108` (modal-firing for unapproved-domain / price-unavailable / per-tx-limit / etc.) gets DELETED. The engine produces the same prompt types and the existing modal dispatcher handles them. The handler stays C++-side for one reason only — receiving the 402 response and re-issuing the paid retry is a CEF resource-handler thing, not a Rust HTTP thing.

**Reasoning:**
- Memory `brc121_bypasses_permission_engine` already flags this as the planned Phase 1.5 polish item G.
- Rust-internal engine call means BRC-121 inherits all 5 CallKind classes for free — no header serialization, no separate header injection rules, no second TOCTOU window.
- Deferring after the main migration avoids coupling two different cascade paths during the same flag flips. BRC-121's modal-firing branches (`HttpRequestInterceptor.cpp:5039`, `5085`, etc.) currently use the same prompt types as the engine (`domain_approval`, `payment_confirmation`) but with different parameter structures. Migrating these in the SAME commit as a CallKind flag risks both regressions at once.
- Memory `brc121_no_send_required` notes the broadcast-after-200 architecture; that's orthogonal to the engine decision and stays as-is.
- Memory `brc121_handler_dispatch` notes the 402 detection must live in every `CefResourceRequestHandler` — that constraint is a CEF concern, not an engine concern, so doesn't affect the Rust side.

**Severity:** **OPEN-QUESTION-OK** — explicitly deferred. The 2.6 plan doc should list it in "out of scope, sequenced after main migration."

**2nd-order consequences:** Q5 — if we DON'T defer, the `engine_rust_payment` flag has to flip both the createAction path and the pay_402 path together, doubling regression risk per flip. Deferring isolates risk.

---

### Q8 — Wallet endpoint surface — what doesn't need migration

**Recommendation:** **Internal-only endpoints stay non-engine-routed.** Don't wrap them.

Concretely:
- Clusters 1/3/4/6/8/11/13/14 from WALLET_API_MAP (Health/System, Cert publish/admin, Debug, Custom wallet CRUD, Price/sync/activity/settings, PeerPay, Paymail, Recipient resolution) — ~50 endpoints — continue to do `check_domain_approved` only.
- The threat model is already handled: `check_domain_approved` at `handlers.rs:577-582` returns `Ok(None)` (allow) when `X-Requesting-Domain` header is absent, and `Err(403)` when the header is present but unapproved. The IPC bridge sets `X-Requesting-Domain` from the calling frame's origin (`COMMIT_6_DESIGN.md` §1 + `simple_handler.cpp:1648-1662`); internal callers from wallet UI omit the header.
- A malicious dApp cannot inject a forged X-Requesting-Domain header through the IPC bridge — `HandleIpcWalletCall` sets the header from `frame->GetURL()` (`HttpRequestInterceptor.cpp:2538-2585` + `simple_handler.cpp:1648-1662`), not from caller-controlled data.

**Audit log angle:** if 2.6 introduces structured audit logging anyway (Q2), it's cheap to log internal-endpoint hits to the same table. But this is a different concern from "should the engine gate them" — and the answer is no.

**Reasoning:**
- `check_domain_approved` is already defense-in-depth for the shim-reachable surface. Adding engine gating to internal-only endpoints is performance overhead with no security benefit.
- The wallet UI calling its own backend through `/wallet/balance` etc. shouldn't ever fire a modal. If we engine-gate these, we'd need the `IsInternalOrigin` bypass to fire on every call.
- Memory `wallet_toolbox_divergence` is the precedent: Hodos diverges from toolbox in places where the cost/benefit doesn't justify it. Engine-gating internal endpoints is one of those places.

**Severity:** **OPEN-QUESTION-OK.** Codified in the plan doc as "out of scope" rather than as a foundational decision.

**2nd-order consequences:** Q9 — internal-origin bypass stays critical (Rust still needs to know what to do for header-less internal calls).

---

### Q9 — Internal-origin bypass

**Recommendation:** **Yes, preserve `IsInternalOrigin` bypass on the C++ side AND mirror it in Rust** (header-less requests skip the engine).

Specifically:
- C++ keeps `IsInternalOrigin` at `HttpRequestInterceptor.cpp:1365-1374` exactly as is — exact-or-port-suffix match closes the prefix-match weakness. **Don't propose reverting.**
- For requests where C++ has bypassed (internal frontend at localhost:5137), Rust receives a request with NO `X-Requesting-Domain` header → `check_domain_approved` returns `Ok(None)` (allow through) → engine is never called.
- For requests where C++ has NOT bypassed (external dApp), Rust receives a request WITH `X-Requesting-Domain` → engine runs.

This is exactly the current pattern. **No change.** The only new thing in 2.6 is the engine running in Rust instead of C++ — the bypass mechanism is identical.

**Edge cases:**
- A third-party extension running on localhost:5137 spoofing the wallet origin: extensions aren't supported today (memory `cef_self_build_reason`). Future work.
- A malicious dApp hosting at `127.0.0.1.evil.com` or similar: blocked by the exact-or-port-suffix tightening in `IsInternalOrigin` (`HttpRequestInterceptor.cpp:1370-1372`).
- Wallet UI calling a BRC-100 endpoint AS IF it were the dApp (e.g. for testing): wallet UI must set `X-Requesting-Domain` explicitly if it wants engine treatment. Make this an explicit dev-only mode behind a settings flag — NOT the default.

**Reasoning:**
- Phase 2.5 Commit 6 closure smoke #5 (PHASE_2_5_IPC_REFACTOR.md "Phase 2.5 closure smoke results") confirmed `IsInternalOrigin` bypass is the key seam that keeps wallet UI usable. Reverting would force the wallet UI itself to surface modals — which it can't do (the wallet UI IS the modal surface).
- Memory note `IsInternalOrigin uses exact-or-port-suffix match` is the security tightening — must survive 2.6.
- Defense-in-depth: even if `IsInternalOrigin` had a bug, Rust's `check_domain_approved` is the second layer.

**Severity:** **OPEN-QUESTION-OK** — the answer is mechanical (preserve current pattern). Document as a locked decision in the plan doc.

**2nd-order consequences:** none significant.

---

### Q10 — Rust engine module shape — pure module vs Actix integration

**Recommendation:** **(d) Two layers: `permission_engine` pure crate + `permission_service` actix module.**

Structure:
```
rust-wallet/
├── crates/
│   └── permission_engine/        ← pure crate (no actix, no sqlite, no http)
│       ├── src/
│       │   ├── lib.rs            ← public: Decide(ctx) → Decision
│       │   ├── context.rs        ← PermissionContext struct (data in)
│       │   ├── decision.rs       ← PermissionDecision struct (data out)
│       │   └── matrix_c.rs       ← branch helpers (privacy_perimeter, payment, etc.)
│       └── tests/                ← decision logic unit tests (port C++ engine tests verbatim)
└── src/
    ├── permission_service/       ← actix-integrated service module
    │   ├── mod.rs
    │   ├── state.rs              ← PermissionService struct (holds approval map + session counters + audit log handle)
    │   ├── handlers.rs           ← shadow-decide endpoint, approval lookup, etc.
    │   ├── context_builder.rs    ← assembles PermissionContext from AppState (DB + caches + price)
    │   └── audit.rs              ← writes to engine_shadow_log + permission_audit_log
    └── main.rs                   ← AppState gains `Arc<PermissionService>` field
```

The pure crate has **no dependencies on actix, sqlite, reqwest, or AppState.** It takes a `PermissionContext` (data) and returns a `PermissionDecision` (data). Mirrors today's `cef-native/src/core/PermissionEngine.{h,cpp}` exactly — same shape, same test surface.

The service module owns:
- The pending-approval map (`HashMap<approval_id, PendingApprovalRecord>` with TTL)
- Session counters (the migrated `SessionManager` equivalent)
- The audit log SQLite writes
- The context builder that reads from `AppState` (DB, caches, BSVPriceCache equivalent) to assemble what the pure crate needs

**Reasoning:**
- Pure crate is what enabled the 46+ engine tests in C++ today (`cef-native/tests/permission_engine_test.cpp`). Same pattern in Rust gives us the same test ergonomics — and lets us **port the C++ test vectors verbatim** to validate the Rust engine matches.
- Two layers separates "decision logic" from "infrastructure," which is the standard Rust pattern for testable cores.
- AppState already takes facade structs (`main.rs:87`); adding one more is idiomatic.
- Option (a) — pure crate with plumbed-in context per call — is OK but the plumbing per CallKind is repetitive enough that you want a context-builder layer above the engine. So (a) decays into (d) anyway.
- Option (b) — module inside wallet with `&AppState` — couples engine to actix and loses test isolation. Hard reject.
- Option (c) — trait-with-EngineContext — over-engineered for a wallet that has one production context and one test context. The two-layer split gives us the same benefit with less ceremony.

**Severity:** **BLOCKER.** Module structure is the foundation; getting it wrong forces sub-phase 1 (pure crate build) to land twice.

**2nd-order consequences:** Q5 — flag handling lives in `permission_service`, not the engine crate. Q6 — shadow mode runs by calling `permission_engine::Decide(ctx)` and comparing to a C++-supplied hint; pure crate makes this trivial. Q2 — audit table lives in `permission_service`; pure crate stays untouched. Q1 — `PermissionDecision` is defined in the pure crate; `permission_service` serializes to the 202 PENDING shape.

---

## Recommended discussion order

Discuss in dependency order (blockers first, then questions whose answers depend on the prior ones):

1. **Q10** — module structure (defines the seams everything else hangs on)
2. **Q1** — 202 PENDING payload schema (the contract C++ and Rust speak)
3. **Q5** — feature flag scope (defines sub-phase structure)
4. **Q3** — SessionManager migration boundary (load-bearing for payment-CallKind flag)
5. **Q6** — shadow-mode mechanism (gates each flag flip)
6. **Q2** — approvalId lifecycle (mechanical but needs the user's retention/policy input)
7. **Q4** — DomainPermissionCache collapse (recommend deferring the mirror)
8. **Q9** — internal-origin bypass (mechanical confirmation)
9. **Q8** — wallet endpoint surface (mechanical confirmation)
10. **Q7** — BRC-121 path (explicitly deferred to post-main-migration polish)

## What I'd write into the plan doc as locked decisions vs open questions

**Lock these from this brief:**
- **Q3 a/b**: `wasAutoApprovedPayment` derivation stays in C++. SessionManager migrates to Rust. Indicator IPC fire site stays C++. (3-line section, references file:line.)
- **Q5**: 5 feature flags, one per CallKind class. Env-var-driven. AppState gets `engine_flags: EngineFlags`.
- **Q6 mechanism**: C++ authoritative, Rust shadow async via fire-and-forget. NEVER sequential.
- **Q7**: BRC-121 migration sequenced AFTER all 5 main flags flip.
- **Q8**: Internal-only endpoints stay non-engine-routed; defense-in-depth `check_domain_approved` already sufficient.
- **Q9**: `IsInternalOrigin` bypass preserved exactly. Header-less Rust requests skip engine.
- **Q10**: Two-layer split — `crates/permission_engine` pure + `src/permission_service` actix module.

**Keep open in §X of the 2.6-A plan doc:**
- **Q1 schema versioning**: confirm `schemaVersion: 1` is the right starting point, or whether we should defer until first cross-version drift surfaces.
- **Q2 audit retention**: 90 days is a guess. Need user input on regulatory / forensics requirements.
- **Q2 audit table column set**: `body_hash` vs `body_preview` vs nothing. Privacy/forensics tradeoff.
- **Q4 mirror deferral**: if user agrees the shim is injected unconditionally today, mirror is deferred. If user wants per-domain shim gating in 2.6, mirror question reopens.
- **Q6 disagreement review UI**: is there a wallet UI surface for the shadow log? Or CLI-only?

## Risks / things I'm unsure about

1. **Q2 retention.** 90 days is a guess. Could be regulatory-driven (longer) or storage-driven (shorter). Needs user input.
2. **Q4 shim-injection assumption.** I verified at `simple_render_process_handler.cpp:889-898` that the shim is unconditionally injected on external https main frames. If there's a per-domain gating path I missed, my recommendation to defer the mirror changes.
3. **`SessionManager` migration mechanics.** Moving session counters to Rust means a tab close (which clears C++ SessionManager today) needs a Rust hook. Today's `SessionManager::clearSession(browserId)` is called from somewhere in CEF — needs grep to confirm where, and to design the equivalent IPC. Out of scope for this brief; flagged as Phase 2.6 sub-phase 1 design item.
4. **C++ engine tests as the Rust test vector.** I assume `cef-native/tests/permission_engine_test.cpp` can be ported verbatim — verified the engine is pure-logic (PermissionEngine.h:18-21 "PURE LOGIC. No CEF dependencies"). Confirmed in source.
5. **Schema versioning** at the JSON contract level — `schemaVersion: 1` is cheap to add but might be over-engineered if we never bump it. The forward-compat answer is to add it; the YAGNI answer is to skip it. Lean toward adding because cross-version skew between C++ and Rust builds is a real operational risk.
6. **`OnWalletCallSuccess` cents source.** If we keep cents extraction in C++ for the indicator, but `SessionManager.recordSpending` moves to Rust, we have to either (a) Rust extracts cents and returns them on the 200, or (b) C++ extracts cents and POSTs them to Rust for recording. (a) is cleaner architecturally; (b) avoids the extra round-trip. Lean toward (a) but flagging.
7. **Untested**: macOS parity for the migration. I did not deep-read `cef_browser_shell_mac.mm`. All cross-platform constraints should be carried forward from CLAUDE.md "macOS cross-platform readiness" invariant. The plan doc needs an explicit macOS parity check matrix per sub-phase.
8. **Pure crate name**: `permission_engine` works; could also be `hodos_permission_engine` to avoid namespace collision in a future workspace. Bikeshed-level concern; flagging only.
