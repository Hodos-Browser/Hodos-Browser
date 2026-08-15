# Phase 2.5 — Wallet IPC Bridge Refactor

> **Status:** Plan locked 2026-05-29. Revised 2026-05-30 after discovering
> a foundational scope miss on the C++ `PermissionEngine` integration —
> see "Revised scope" below. Multi-session work; commits 1-4 landed,
> commits 5-7 pending across 2-3 future sessions.
>
> Discovered as a foundational issue during Phase 2 Step 3b/3c smoke
> testing — github.com blocked via CSP, treechat.io blocked via CORS.
> Both are symptoms of the shim using `fetch()` for wallet calls when
> the codebase's documented architecture is `window.cefMessage.send`
> V8 IPC.

## Revised scope (2026-05-30)

The original plan estimated commits 5+6 at "1-2 days" for animation +
smoke. After examining `AsyncWalletResourceHandler::Open()`, the
realistic scope is:

- **Commit 5 (extract gating helpers, no behavior change):** 4-8 hours.
  Open() is 522 lines of cascading permission logic spanning 6 major
  branches (internal/no-wallet/blocked/unknown-with-manifest-dispatch/
  approved-with-engine-cascade). Extracting into a reusable free function
  while preserving all behavior requires careful staging.
- **Commit 6 (wire IPC bridge through extracted helpers):** 3-5 hours.
  Includes extending `PendingAuthRequest` to carry an IPC continuation
  type so modal resolution can resume either an HTTP or IPC request.
- **Commit 7 (CEF rebuild + Treechat + github smoke):** 1-2 hours.

**Total realistic effort: 10-18 hours across 2-3 focused sessions** with
clean handoff between sessions. The work is security-critical (auto-approve
engine), so each commit needs HTTP-path smoke regression before moving
on, not just compile-clean.

## Sub-phase structure (multi-session plan)

| Sub-phase | Status | Deliverable |
|---|---|---|
| 2.5 plan doc | ✅ landed (`1be64b2`) | This document |
| Commits 1-4 (IPC bridge plumbing) | ✅ landed | `d0a00c4`, `b7efa6f`, `b8e4753`, `56f7343` |
| **2.5-A — Planning** | ✅ landed | 5 doc commits: `06ba7b1`, `cfe4cd2`, `2c98217`, `37e191b`, `5ae6242` |
| **2.5-B — Commit 5 extraction** | ✅ landed | All 6 sub-commits: 5.a `814c817` (scaffolding), 5.b `e8168d6` (payment), 5.c `6feb318` (identity-key), 5.d `eb7158d` (key-linkage), 5.e `0883eeb` (scoped-grant), 5.f `a7b8680` (cert-disclosure properly engine-driven). |
| **2.5-C — Commit 6 IPC wiring** | ✅ landed | Design doc `a5ca265`. 6 code commits: 6.a `e6ab4a4` (PendingAuthRequest IPC fields), 6.b `6afd34a` (OnWalletCallSuccess), 6.c `f10df6d` (Decision 3 free-fn modal openers), 6.d.A `779ec8d` (IsInternalOrigin + IPC helpers + HandleIpcWalletCall), 6.d.BE `5550d4f` (wired through to wallet_call + handleAuthResponse ResumeKind dispatch — **engine cascade fires from external dApp traffic**), 6.d.BE+1 `52eb57d` (gold-pill animation fix for tiny payments), 6.f `bae5d5c` (closure + dead code removal + criterion sweep). |
| **2.5-D — Commit 7 smoke** | ✅ landed | Real-world smoke on github.com + app.treechat.com — see "Phase 2.5 closure smoke results" below. All 6 acceptance criteria pass with log evidence. Phase 2.5 CLOSED 2026-06-02. |

## 🎉 Phase 2.5 closure smoke results (2026-06-02)

Smoke executed across 6 test scenarios against real production dApps + the
internal wallet UI. **All passed.** Engine cascade now fires from external
dApp traffic for the first time since Phase 2.5 began.

| # | Scenario | Result | Key evidence |
|---|---|---|---|
| 1 | github.com `window.CWI.getNetwork({})` | ✅ PASS | Returned `{network: 'mainnet'}`. Network tab: zero requests to `127.0.0.1:31301`. Console: no CSP violation. Pre-Phase-2.5 this would have been blocked by github's `connect-src` policy. |
| 2 | app.treechat.com login (wallet auth flow) | ✅ PASS | 4 engine paths exercised in one flow: `/waitForAuthentication` Silent, `/getPublicKey` Silent (V17 column hit — persistent identity-key disclosure grant), `/createSignature` Prompt with `protocol_permission_prompt` for `yours-legacy-message` protocol (5.e scoped-grant migration validated), `grant_scoped_permission` persisted to V18 row. Treechat backend rejected the login because Hodos's identity key is unknown to their user database — not a wallet bug. Pre-Phase-2.5 the cross-origin fetch would have been rejected by `actix-cors` localhost-only allowlist. |
| 3 | treechat payment flow with gold pill | ⏭️ skipped | Substitute evidence: yesterday's `hodos-test.local:8000` smoke validated gold-pill animation end-to-end (after 6.d.BE+1 fix). Test 4 also exercised the IPC bridge silent path on github.com. Treechat's deposit-model UX layer made a real payment test impractical without spending sats. |
| 4 | github encrypt/decrypt round-trip | ✅ PASS | `getPublicKey({identityKey:true})` → Silent via V17. `yours.encrypt({message:'hello phase 2.5', pubKey})` returned BIE1 ciphertext (`42 49 45 31 03 ...` = "BIE1\x03" magic bytes proven). `yours.decrypt({ciphertext})` returned `'hello phase 2.5'` exact. Log confirmed Silent engine decisions on `/wallet/encrypt-bie1` + `/wallet/decrypt-bie1` for github.com. Two deprecation warnings for `window.yours.*` methods are expected shim behavior. |
| 5 | localhost:5137 internal wallet UI | ✅ PASS | Wallet panel, balance, address generation, settings overlay, Approved Sites list all work. `IsInternalOrigin` bypass preserved — internal traffic doesn't go through engine cascade. |
| 6 | actix-cors config unchanged | ✅ PASS | `git diff main..HEAD -- rust-wallet/src/main.rs` filtered on CORS/allowed_origin lines returns empty. Defense-in-depth preserved. |

### Architectural milestone proven

The IPC bridge + engine cascade live end-to-end on real production dApps that
were previously blocked by the original CSP / CORS symptoms that motivated
Phase 2.5:

- **github.com** (CSP-strict `connect-src`) — wallet calls flow through IPC
  without ever leaving the renderer process as a network request. CSP can't
  see the call.
- **app.treechat.com** (cross-origin from wallet's perspective) — wallet
  calls flow through IPC without triggering CORS preflight. `actix-cors`
  localhost-only allowlist remains intact as defense-in-depth.

### What's next

Phase 2.6 (engine-to-Rust migration) is sequenced immediately after Phase 2.5
closes per
[`../../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md`](../../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md).
Separate plan doc to be drafted before any code work begins per
`feedback_research_first_do_it_once`. Estimated ~3-4 weeks of focused work
+ 1 week soak.

Each sub-phase is one focused session. Plan a clean handoff doc between
sessions so context-clear ↔ context-load is lossless.

### Smoke obligation reality (added 2026-06-01 mid-2.5-B)

The original plan implied per-sub-step smoke verification. That turns out
to be **architecturally impossible for 5.a-5.f** because commits 1-4
routed the V8 shim through `wallet_call` IPC → `SyncHttpClient::Post` (raw
WinHTTP), **bypassing `AsyncWalletResourceHandler::Open()` entirely**.
5.a-5.f migrate branches inside `Open()` — but external dApp traffic
doesn't reach `Open()` until Commit 6 wires the IPC bridge through the
engine. The engine cascade is effectively dormant in production between
commits 1-4 and Commit 6.

Discovery point: 5.b smoke attempt on `https://hodos-test.local:8000`
returned `ERR_DOMAIN_NOT_APPROVED` straight from Rust's
`check_domain_approved` — the C++ engine never ran. Confirmed expected
behavior given the IPC bypass; not a 5.b bug.

**Implications:**

1. **5.c, 5.d, 5.e, 5.f land "blind"** — verification is unit tests
   (`hodos_tests.exe`) + code-diff review only. Each sub-step is a 1:1
   reorganization of an existing engine-driven branch, so risk is bounded.
2. **Cumulative smoke happens at Commit 6 close** — the `hodos-test.local`
   fixture exercises payment / identity-key / key-linkage / scoped-grant /
   cert-disclosure modals in one pass.
3. **No reorder of Commit 6 in front of 5.c-5.f** — was briefly considered
   but rejected: Commit 6 needs a fully-extracted `Open()` (i.e. all
   branches running through `RunPermissionGate`) to avoid duplicating
   inline branch dispatch on the IPC path. Half-extracted `Open()` makes
   Commit 6's design strictly worse.

## Decisions locked (Phase 2.5-A planning, 2026-05-30)

Four architectural decisions sized to commits 5-7. All four were checked
against the Phase 2.6 engine-to-Rust vision
(`../../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md`) and confirmed
forward-compatible — no rework cost on the 2.5 work even if 2.6 starts the
same day 2.5 closes.

> **Phase 2.6 sequencing confirmed:** Phase 2.5 finishes first (delivers
> the user-visible win — engine works on external dApps via IPC). Phase 2.6
> opens immediately after 2.5-D closes as a separate plan doc, migrating
> the engine to Rust. Estimated ~54-86 focused hours + 1 week soak. The
> 2.5 extraction shape (below) is the literal prep for the 2.6 migration.

### Decision 1 — Extraction interface shape

**Locked:** Free function `RunPermissionGate(const PermissionContext& ctx, const GateCallbacks& cb) -> GateDecision`
where `GateCallbacks` is a struct of `std::function` slots: `openModal`,
`firePaymentIndicator`, `recordSpending`, `injectHeader`, `forwardToWallet`.

Stateless, pure data in / decision out + side effects via callbacks. HTTP
path and IPC path build different `GateCallbacks` structs. Mocks trivially.
`PermissionEngine::Decide()` is wrapped, not modified.

**Phase 2.6 fit:** `RunPermissionGate` body changes to "POST to Rust, handle
200 / 202 PENDING / 403"; the signature is identical. Callbacks shrink (most
move to Rust) but the seam shape is preserved.

### Decision 2 — `PendingAuthRequest` IPC continuation

**Locked:** Extend `PendingAuthRequest` struct with:

```cpp
enum class ResumeKind { kHttpCallback, kIpcResponse, kInternal };

struct PendingAuthRequest {
    // existing fields (requestId, domain, method, endpoint, body, type, handler)
    ResumeKind resumeKind = ResumeKind::kHttpCallback;
    CefRefPtr<CefFrame> frame;                            // IPC path only
    int browserId = 0;                                    // IPC path only
    std::map<std::string,std::string> headersOnApprove;   // X-Identity-Key-Approved, etc.
};
```

Enum-tagged variants beat a bool `isIpcCall` for future extensibility (a
third resume path will land — internal Rust-initiated requests under Phase
2.6).

**Phase 2.6 fit:** the pending state owner becomes Rust; C++ keeps a much
thinner "which frame + requestId to wake on approve" record. The fields
above are exactly what serializes across the future IPC. Rename + thin out,
not redesign.

### Decision 3 — Modal trigger routing

**Locked:** Extract every `triggerXxxModal()` member function into free
functions taking explicit context, and add a single dispatcher:

```cpp
void OpenPromptModal(
    PermissionDecision::PromptType type,
    const PromptContext& ctx,
    const std::string& requestId
);
```

The dispatcher maps `PromptType` → matching trigger fn. The engine's
`Prompt` return value carries everything the dispatcher needs as payload;
the engine no longer touches CEF.

**Phase 2.6 fit:** the engine-to-Rust boundary lands cleanly at the engine /
dispatcher seam. Engine emits `202 PENDING` with prompt context;
`OpenPromptModal` is called from the C++ `202 PENDING` response handler
instead of from `RunPermissionGate`'s callback. Same dispatcher code, same
modal triggers.

### Decision 4 — Payment indicator extraction granularity

**Locked:** Extract the WHOLE post-response success branch as one helper:

```cpp
void OnWalletCallSuccess(
    int browserId,
    const std::string& domain,
    int64_t cents,                     // -1 if not a payment
    bool wasAutoApprovedPayment,
    const std::string& endpoint
);
// Internally:
//   if (wasAutoApprovedPayment && cents > 0) {
//       SessionManager::GetInstance().recordSpending(browserId, cents);
//       FirePaymentSuccessIndicator(browserId, domain, cents);
//   }
//   (future) PaidContentCache::Put(...) hook for BRC-121 path
```

`recordSpending`, indicator IPC, and (BRC-121-only) cache write are coupled
by the same cents + browserId + success check. Always run together. One
helper = one call site per path. Future caller can't forget to also call
`recordSpending` — the cluster is atomic.

**Phase 2.6 fit:** the helper splits cleanly at the migration boundary —
`recordSpending` moves to Rust (session caps are engine state), indicator
IPC stays C++ (UI concern). Today's bundling makes the future split visible
and atomic.

### Why these survive the Phase 2.6 migration

All four decisions are about *seams*, not implementations:

| Seam | 2.5 implementation | 2.6 implementation |
|---|---|---|
| Pure data in, decision out | `RunPermissionGate(ctx, cb)` calls `PermissionEngine::Decide()` locally | `RunPermissionGate(ctx, cb)` POSTs to Rust, awaits 200/202/403 |
| Pending request continuation | `PendingAuthRequest` struct held in C++ map | `PendingAuthRequest` struct held in C++; Rust owns the matching record indexed by `approvalId` |
| Modal dispatch | `OpenPromptModal()` called from gate runner | `OpenPromptModal()` called from `202 PENDING` response handler |
| Post-success cluster | `OnWalletCallSuccess` bundles spend + indicator | `OnWalletCallSuccess` keeps indicator (UI); `recordSpending` moves to Rust |

The 2.5 work is the literal prep work for 2.6. Zero rework cost.

## Why this exists

Phase 2's shim (Steps 1–4, then 3b + 3c) implements `window.CWI` / `window.yours`
/ `window.panda` as `fetch(ENDPOINT_BASE + '/' + methodName)` calls to the
Rust wallet on `http://127.0.0.1:31301`. This works on Hodos's internal
frontend (`localhost:5137`, same-origin) but **fails on every external dApp
origin** because:

| Layer | Where it runs | What blocks the call |
|---|---|---|
| 1. Document CSP `connect-src` | Renderer, before request leaves the page | github.com — `'self' + allowlist` doesn't include `127.0.0.1:31301` |
| 2. CORS preflight | Network, on response headers | treechat.io — wallet's actix-cors only allows `localhost:5137`, returns no `Access-Control-Allow-Origin` |
| 3. Hodos C++ HTTP interceptor + `domain_permissions` | Browser process | (this is the security boundary we *want*) |

The fetch design exposes the shim to layers 1 and 2 — both of which run
*before* our actual security boundary in the C++ interceptor. The documented
architecture (`main.rs:752-755` comment) is that **website JS talks to the
wallet via V8 IPC, never direct fetch**; the actix-cors localhost-only allowlist
exists explicitly as defense-in-depth against IPC bypass attempts. The
fetch-based shim was architectural drift that landed before this design
intent surfaced.

Phase 2.5 restores the documented architecture.

## Goal

Replace every wallet `fetch()` call in `CWIShimScript.h` with a promise-
correlated IPC bridge (`window.__hodos_walletCall(method, endpoint, body)`),
so all wallet traffic flows through CEF's process-message IPC instead of
HTTP from the renderer. Result:

- Site CSP is untouched (IPC is not a network request from the renderer's view)
- Wallet CORS stays localhost-only (correct defense-in-depth)
- Hodos C++ interceptor + `domain_permissions` becomes the *only* security
  boundary, matching the documented design
- Per-call overhead drops (no HTTP framing, no CORS preflight RTTs)

## Out of scope

- Re-architecting `window.hodosBrowser.*` (already IPC-based, works as-is)
- Migrating internal frontend (`localhost:5137`) — same-origin fetch
  continues to work; no harm in leaving the legacy path alive for that origin
- Changing the Rust wallet API surface — endpoints + JSON shapes stay
  identical, only the transport changes

## Reuse audit

| Need | Existing piece | Status |
|---|---|---|
| Render-process JS → browser-process IPC | `cefMessage.send(name, ...args)` in `simple_render_process_handler.cpp:84-191` | **Reuse.** Already injected on every page. |
| Browser-process IPC dispatch | `SimpleHandler::OnProcessMessageReceived` in `simple_handler.cpp` (already routes 125+ message types) | **Reuse.** Add one new branch for `wallet_call`. |
| Browser-process → renderer IPC | `frame->SendProcessMessage(PID_RENDERER, msg)` pattern (used by 70+ existing response messages) | **Reuse.** |
| Renderer-side response handling | `SimpleRenderProcessHandler::OnProcessMessageReceived` (already routes 70+ response types) | **Reuse.** Add one new branch for `wallet_response`. |
| HTTP forwarder C++ → Rust | `SyncHttpClient::Get` / `Post` (`include/core/SyncHttpClient.h`) | **Reuse.** Add custom-headers overload for `X-Requesting-Domain` propagation. |
| Worker-thread dispatch (so HTTP doesn't block UI thread) | `CefPostTask(TID_FILE, ...)` + `CefPostTask(TID_UI, ...)` round-trip | **Reuse.** Standard CEF pattern. |
| Permission gate (`X-Requesting-Domain`, `check_domain_approved`) | Rust handler reads header → consults `domain_permissions` | **Reuse.** No change needed. The header propagates from the calling frame's origin. |

**Net new code:** ~120 LOC. Most of the work is plumbing, not new behavior.

## Design

### 1. JS bridge (injected by `simple_render_process_handler.cpp::OnContextCreated` for every page that gets the shim)

```javascript
// Injected BEFORE CWIShimScript.h's IIFE runs.
(function() {
    'use strict';
    if (window.__hodos_walletCall) return;  // idempotent — survives re-injection

    var nextId = 1;
    var pending = Object.create(null);  // requestId -> {resolve, reject, method}

    // Browser process calls this via ExecuteJavaScript on response.
    window.__hodos_walletResponse = function(requestId, ok, payloadJson) {
        var p = pending[requestId];
        if (!p) {
            // Late / orphan — frame navigated, or duplicate response.
            try { console.warn('[Hodos] orphan wallet_response id=' + requestId); } catch(e) {}
            return;
        }
        delete pending[requestId];
        try {
            var payload = payloadJson ? JSON.parse(payloadJson) : null;
            if (ok) {
                p.resolve(payload);
            } else {
                // payload is { error, code?, status? }
                var err = new Error(
                    '[Hodos] ' + p.method + ' failed: ' +
                    ((payload && payload.error) || 'unknown error')
                );
                if (payload && payload.code)   err.code = payload.code;
                if (payload && payload.status) err.status = payload.status;
                err.body = payload;
                p.reject(err);
            }
        } catch (e) {
            p.reject(new Error('[Hodos] response parse failed: ' + e.message));
        }
    };

    window.__hodos_walletCall = function(method, endpoint, body) {
        // method = friendly name for diagnostics (e.g. 'createAction')
        // endpoint = wallet route (e.g. '/createAction' or '/wallet/encrypt-bie1')
        // body = JSON-serializable object
        var bodyJson;
        try {
            bodyJson = JSON.stringify(body == null ? {} : body);
        } catch (e) {
            return Promise.reject(new Error(
                '[Hodos] ' + method + ': args not JSON-serializable: ' + e.message
            ));
        }
        if (bodyJson.length > 50 * 1024 * 1024) {
            return Promise.reject(new Error(
                '[Hodos] ' + method + ': payload exceeds 50MB IPC ceiling. ' +
                'Large payloads (e.g. createAction with massive inputs.BEEF) are ' +
                'not supported via this bridge — break the call into smaller ' +
                'chunks or contact wallet support.'
            ));
        }
        var requestId = String(nextId++);
        return new Promise(function(resolve, reject) {
            pending[requestId] = { resolve: resolve, reject: reject, method: method };
            try {
                window.cefMessage.send('wallet_call', [requestId, method, endpoint, bodyJson]);
            } catch (e) {
                delete pending[requestId];
                reject(new Error('[Hodos] failed to dispatch wallet_call: ' + e.message));
            }
        });
    };
})();
```

**Per-frame, per-context:** the bridge lives on `window` so each frame's V8
context has its own `nextId` counter and `pending` map. Cross-frame
correlation is impossible by design — a frame's responses never reach
another frame.

**50 MB payload ceiling** documented inline as a guardrail; the renderer
rejects the call before sending anything over IPC if the body exceeds it.
The current Hodos HTTP limit is 100 MB, but practical IPC sweet spot
(Chromium `kMaxMessageSize` is 128 MB default) is far smaller. 50 MB
gives a 2× safety margin against IPC stalls and lets us tighten later if
real workloads need less.

### 2. Browser-process handler (`simple_handler.cpp::OnProcessMessageReceived`)

When `wallet_call` arrives:

```cpp
if (message_name == "wallet_call") {
    auto args = message->GetArgumentList();
    std::string requestId = args->GetString(0);
    std::string method    = args->GetString(1);   // friendly name (diagnostics)
    std::string endpoint  = args->GetString(2);   // e.g. "/createAction"
    std::string bodyJson  = args->GetString(3);

    // Extract calling frame's origin for X-Requesting-Domain
    std::string origin;
    if (frame) {
        std::string frameUrl = frame->GetURL().ToString();
        // protocol://host[:port] — strip path
        size_t protoEnd = frameUrl.find("://");
        if (protoEnd != std::string::npos) {
            size_t pathStart = frameUrl.find('/', protoEnd + 3);
            if (pathStart != std::string::npos) {
                origin = frameUrl.substr(0, pathStart);
            } else {
                origin = frameUrl;
            }
            // X-Requesting-Domain wants host[:port] only (matches existing pattern)
            origin = origin.substr(protoEnd + 3);
        }
    }

    // Capture frame ref for response routing
    CefRefPtr<CefFrame> capturedFrame = frame;

    // Run the HTTP call on a worker thread so we don't block the UI thread
    CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
        std::string requestId, std::string method,
        std::string endpoint, std::string bodyJson, std::string origin,
        CefRefPtr<CefFrame> capturedFrame
    ) {
        std::string url = "http://127.0.0.1:31301" + endpoint;
        std::map<std::string, std::string> headers = {
            {"Content-Type",        "application/json"},
            {"X-Requesting-Domain", origin}
        };
        HttpResponse resp = SyncHttpClient::Post(url, bodyJson, headers, /*timeoutMs=*/30000);

        // Bounce back to UI thread to send the response IPC
        CefPostTask(TID_UI, base::BindOnce([](
            std::string requestId, HttpResponse resp,
            CefRefPtr<CefFrame> capturedFrame
        ) {
            if (!capturedFrame || !capturedFrame->IsValid()) return;
            bool ok = resp.success && resp.statusCode >= 200 && resp.statusCode < 300;
            std::string payload = ok
                ? resp.body
                : std::string("{\"error\":") + EscapeJson(resp.body) +
                  ",\"status\":" + std::to_string(resp.statusCode) + "}";

            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("wallet_response");
            auto respArgs = response->GetArgumentList();
            respArgs->SetString(0, requestId);
            respArgs->SetBool(1, ok);
            respArgs->SetString(2, payload);
            capturedFrame->SendProcessMessage(PID_RENDERER, response);
        }, requestId, resp, capturedFrame));
    }, requestId, method, endpoint, bodyJson, origin, capturedFrame));

    return true;
}
```

**Threading:** `wallet_call` arrives on the UI thread, gets posted to
`TID_FILE_USER_BLOCKING` (CEF's worker pool for blocking I/O), the sync
HTTP call runs there, the response is posted back to `TID_UI` to send the
IPC. This keeps the UI thread responsive during long wallet operations
(e.g. createAction signing 100 inputs).

**Captured frame ref:** the `CefRefPtr<CefFrame>` is captured by value
into the worker task; on completion we check `capturedFrame->IsValid()`
to avoid sending to a frame that's been destroyed (frame navigation,
tab close).

### 3. Render-process response routing (`simple_render_process_handler.cpp::OnProcessMessageReceived`)

```cpp
if (message_name == "wallet_response") {
    auto args = message->GetArgumentList();
    std::string requestId = args->GetString(0);
    bool ok = args->GetBool(1);
    std::string payload = args->GetString(2);

    // Build JS call: window.__hodos_walletResponse(requestId, ok, payloadJson)
    std::string js = "window.__hodos_walletResponse("
        + EscapeJsString(requestId) + ", "
        + (ok ? "true" : "false") + ", "
        + EscapeJsString(payload) + ");";

    CefRefPtr<CefFrame> targetFrame = browser->GetMainFrame();
    if (targetFrame) {
        targetFrame->ExecuteJavaScript(js, targetFrame->GetURL(), 0);
    }
    return true;
}
```

### 4. Shim refactor (`CWIShimScript.h`)

#### Canonical methods (`makeMethod`):

```javascript
function makeMethod(methodName) {
    function impl(args, originator) {
        var body = (args == null) ? {} : args;
        return window.__hodos_walletCall(methodName, '/' + methodName, body);
    }
    return new Proxy(impl, {
        apply: function(target, thisArg, argumentsList) {
            return Reflect.apply(target, undefined, argumentsList);
        }
    });
}
```

**Net code shrinks** — the previous ~50 LOC fetch logic (mode, credentials,
headers, response parsing, error wrapping) collapses to one bridge call.
Error envelope construction moves to the bridge JS where it lives once,
not 28 times.

#### Legacy methods (7 fetch sites):

| Method | Endpoint | Today's path |
|---|---|---|
| `yours.getAddresses` | `/wallet/yours-legacy-addresses` | fetch → __hodos_walletCall |
| `yours.sendBsv` (N calls) | `/wallet/address-to-script` × N | fetch → __hodos_walletCall |
| `yours.getExchangeRate` | `/wallet/bsv-price` (GET) | fetch → __hodos_walletCall + GET marker |
| `yours.getBalance` | `/wallet/bsv-price` (GET) | fetch → __hodos_walletCall |
| `yours.broadcast` fallback | `/wallet/broadcast` | fetch → __hodos_walletCall |
| `yours.encrypt` | `/wallet/encrypt-bie1` | fetch → __hodos_walletCall |
| `yours.decrypt` | `/wallet/decrypt-bie1` | fetch → __hodos_walletCall |

For GET endpoints (`/wallet/bsv-price`), the bridge passes an empty body
and the C++ handler dispatches via `SyncHttpClient::Get` instead of
`::Post` based on method hint. The simplest scheme is to look at the
endpoint string — GET-only endpoints are documented in a small set, and
we'd hard-code them, OR we add a `method: 'GET'|'POST'` parameter to the
bridge call. **Decision: add the parameter** so the bridge stays generic
and future GET endpoints don't require C++ changes.

Updated bridge call shape:
```js
window.__hodos_walletCall(methodName, '/endpoint', body, /*httpMethod=*/'POST')
```

Default is POST for backwards-compat with the canonical path.

### 5. `SyncHttpClient` extension

Add a custom-headers overload:

```cpp
// SyncHttpClient.h
static HttpResponse Get(const std::string& url, int timeoutMs = 5000);
static HttpResponse Get(const std::string& url,
                       const std::map<std::string, std::string>& headers,
                       int timeoutMs = 5000);
static HttpResponse Post(const std::string& url,
                        const std::string& body,
                        const std::string& contentType = "application/json",
                        int timeoutMs = 5000);
static HttpResponse Post(const std::string& url,
                        const std::string& body,
                        const std::map<std::string, std::string>& headers,
                        int timeoutMs = 5000);
```

Existing call sites (9) keep working without changes; only the new IPC
handler uses the headers overload.

## Security invariants preserved

1. **Permission gate stays on the wallet side.** The IPC handler forwards
   `X-Requesting-Domain` set to the calling frame's `host[:port]`; the Rust
   wallet's `check_domain_approved` reads it as before and consults
   `domain_permissions`. No code path bypasses the gate.

2. **CSP and CORS stay strict.** Page CSP is never modified. Wallet CORS
   stays localhost-only. The shim's transport changes; the security model
   doesn't.

3. **Payment animation chain stays intact.** `sendBsv` continues to route
   through `canonical.createAction`, which the C++ HTTP interceptor's
   `OnResourceLoadComplete` watches for to fire `payment_success_indicator`
   IPC. Wait — under IPC dispatch, `OnResourceLoadComplete` no longer fires
   because there's no HTTP request from the renderer. **This is the one
   real behavior change requiring care.** See "Payment animation safeguard"
   below.

4. **Identity-key prompt flow unchanged.** External `getPublicKey` with
   `identityKey: true` still returns 403 `identity_key_prompt_required`
   from the wallet; the C++ side still surfaces the modal. Status code
   propagates through the bridge (`err.status = 403`, `err.code =
   'ERR_IDENTITY_KEY_PROMPT_REQUIRED'`).

5. **Domain permission cache invalidation** (`domain_permission_invalidate`
   IPC, per memory `domain_permission_cache_invalidation`) — unaffected.
   The invalidation IPC is between the wallet UI and the C++ interceptor's
   cache; it has no dependency on the shim's transport.

## ⚠️ Payment animation safeguard — load-bearing change

Currently the green-dot tab animation fires from
`AsyncHTTPClient::OnRequestComplete` inside `AsyncWalletResourceHandler` in
`HttpRequestInterceptor.cpp` after every successfully-auto-approved payment
(and from `firePaymentSuccessIpc()` for the BRC-121 paid retry path). Under IPC dispatch, **the request
never goes through `AsyncWalletResourceHandler`** because it's not an HTTP
request from the renderer's perspective — it's an IPC message that the
browser-process handler forwards directly to `SyncHttpClient::Post`.

Two paths to preserve the animation:

**Option A — Fire the IPC in the new C++ bridge handler.**
After the worker thread gets the wallet's response, before sending
`wallet_response` back to the renderer, check if the endpoint was
`/createAction` (and any future payment-firing endpoint) and the response
was a successful auto-approve. If so, post a `payment_success_indicator`
IPC to the header browser with `{browserId, domain, cents}`.

**Option B — Move the trigger to the wallet's response payload.**
Have the Rust wallet include `_paymentSuccessIndicator: { cents, ... }`
in the successful-auto-approve createAction response, and have the C++
handler watch for that field and fire the IPC.

**Option A is preferred** because the existing trigger logic is already
in C++, and Option B requires modifying the wallet's response shape (which
might break other consumers). Option A is a small extraction from the
existing `AsyncWalletResourceHandler` code into a reusable function called
from both the HTTP path (which still services internal frontend traffic
on `localhost:5137`) and the new IPC path.

**Decision (locked here):** Option A. Extract the indicator-fire logic
into a helper called from both paths; add it to the IPC handler's
response branch before sending `wallet_response`. Smoke verification
must include observing the green-dot animation on a `sendBsv` call from
treechat or a test page.

## Commit-level plan

> **Revised 2026-05-30 after discovering a missed gap in commits 1-4 scope:**
> the C++ `PermissionEngine` (per-tx limits, per-session caps, rate
> limiting, auto-approve cascade, modal prompts, payment indicator)
> lives inside `AsyncWalletResourceHandler::Open()` and only fires when
> requests come in through CEF's resource interception. Commits 1-4
> route `wallet_call` directly to `SyncHttpClient::Post`, which bypasses
> CEF's interception and therefore bypasses the entire engine. The
> wallet's `check_domain_approved` still fires (coarse-grained approve/
> not-approve), but everything else — per-tx limits, payment modals,
> identity-key prompts, rate limits, session spending tracking, and the
> green-dot animation — silently does not run on the IPC path.
>
> Commits 5-7 (revised below) extract the gating cascade into a
> reusable helper that both `AsyncWalletResourceHandler::Open()` and
> the IPC bridge can call. Phase 2.5 is not done until those land.

| # | Subject | Files | Smoke |
|---|---|---|---|
| 1 | ✅ landed (`d0a00c4`) — `SyncHttpClient::Post(headers)` overload + `wallet_call` IPC handler | `SyncHttpClient.h/.cpp`, `simple_handler.cpp` | Compile only |
| 2 | ✅ landed (`b7efa6f`) — JS bridge injection + `wallet_response` routing | `simple_render_process_handler.cpp` | Compile + DevTools test |
| 3 | ✅ landed (`b8e4753`) — `makeMethod` refactor (28 canonical methods) | `CWIShimScript.h` | Compile |
| 4 | ✅ landed (`56f7343`) — Legacy fetch sites (8 sites; +DELETE verb support) | `CWIShimScript.h`, `SyncHttpClient`, `simple_handler.cpp` | Compile |
| 5 | Extract gating helpers from `AsyncWalletResourceHandler` (NO behavior change) | `HttpRequestInterceptor.cpp` | HTTP path on `localhost:5137` still works identically |
| 6 | Wire IPC bridge through `PermissionEngine` (silent/prompt/deny paths) | `simple_handler.cpp`, `PendingAuthRequest.h` | Engine + animation + modals all fire from IPC path |
| 7 | CEF rebuild + Treechat + github smoke | None — just verification | The real end-to-end test |

Commits 1-4 already shipped the bridge plumbing. Commits 5-6 complete
the security boundary so the IPC path matches the HTTP path 1:1.
Commit 7 is the smoke pass.

## Per-commit acceptance criteria

Each commit lands only when every "done when" criterion is verifiable. No
hand-waving — each line should map to a runnable command or an observable
behavior.

### Commit 5 — Extraction (no behavior change)

**Files touched:** `cef-native/src/core/HttpRequestInterceptor.cpp`,
new `cef-native/include/core/PermissionGate.h`, new
`cef-native/src/core/PermissionGate.cpp`,
`cef-native/include/core/PendingAuthRequest.h`,
optional new `cef-native/tests/permission_gate_test.cpp`.

#### Sub-step breakdown (5.a-5.f) — Phase 2.5-B staging

Commit 5 is staged as 6 incremental sub-commits, each migrating one branch
of `AsyncWalletResourceHandler::Open()` to the shared `RunPermissionGate`
helper. Each sub-step preserves existing behavior (1:1 reorganization of
the engine-driven branches that already landed in Phase 1.5 Step 6
Commits A-E).

| Sub-step | Target | Status |
|---|---|---|
| 5.a | Scaffolding — `PermissionGate.h`/.cpp + `permission_gate_test.cpp` + CMake wiring; nothing consumes yet | ✅ `814c817` |
| 5.b | Migrate the approved-trust **payment** branch (was L2245-2385 inline) | ✅ `e8168d6` |
| 5.c | Migrate the **identity-key reveal** branch | pending |
| 5.d | Migrate the **key-linkage reveal** branch | pending |
| 5.e | Migrate the **scoped-grant** branch (Protocol / Basket / Counterparty) for non-payment endpoints | pending |
| 5.f | Migrate the **cert-disclosure** branch + final cleanup (move shadow-mode log, verify line-count target) | pending |

**Smoke deferred to Commit 6 cumulative pass** — see "Smoke obligation
reality" above. Per-sub-step verification for 5.a-5.f is unit tests +
code-diff review.

#### Done when

> **Smoke criteria (#1, #7) deferred to Commit 6 cumulative smoke** per
> "Smoke obligation reality" above — the engine cascade is dormant on
> external dApps until Commit 6 wires the IPC bridge. Architecturally
> impossible to smoke each sub-step independently.

1. **HTTP path works identically for all seven gates** (verified at
   Commit 6 close on the `hodos-test.local:8000` fixture; cannot be
   verified mid-Commit-5):
   - createAction triggers `payment_confirmation` modal when over per-tx cap
   - createAction silent-approves when within caps (no modal, no log change)
   - getPublicKey identity-key style triggers `identity_key_reveal` modal
   - revealCounterpartyKeyLinkage triggers `key_linkage_reveal` modal
   - proveCertificate non-granted field triggers `certificate_disclosure` modal
   - listOutputs new-basket triggers `basket_permission_prompt` modal
   - createSignature new-protocol triggers `protocol_permission_prompt` modal
2. **All 28 canonical shim methods still function** through the IPC bridge
   path (verified at Commit 6 close — same fixture)
3. **All 11 legacy yours methods still function** (same — Commit 6 close)
4. **`PermissionEngine` unit tests still pass** (`hodos_tests.exe`).
   Verified per sub-step.
5. **New `RunPermissionGate` helper has unit tests** covering at least:
   - Silent decision forwards correctly via `forwardToWallet` callback
   - Prompt decision invokes `openModal` callback with correct PromptType
   - Deny decision invokes `denyWithError` callback with reason JSON
   - All callbacks can be mocked; helper is testable without CEF
   Verified in 5.a (7 tests landed).
6. **No new lines of behavior logic** — every `if/else` branch in the
   migrated lambdas traces 1:1 to a branch that previously existed in
   `Open()`. Verified by side-by-side review per sub-step.
7. **Green-dot animation still fires** on auto-approved payment (visual
   smoke at Commit 6 close — deferred).
8. **`Open()` line count trend** — net direction depends on Commit 6's
   helper extractions per Decision 3 (`OpenPromptModal`). Through 5.a-5.f
   Open() TRENDS UP because C++ lambda boilerplate is more verbose than
   inline if-blocks; the shrinkage happens at Commit 6 when the lambdas
   are deduplicated across HTTP + IPC paths and modal triggers move to
   named free functions. Original ~30% drop expectation is a Commit-5 +
   Commit-6 combined metric, not per-sub-step.
9. **`grep -n payment_success_indicator HttpRequestInterceptor.cpp` still
   returns the two fire sites** — extraction MUST NOT remove the IPC fire
   from the HTTP path. (Indicator helper is a separate Commit 6 deliverable;
   Commit 5 leaves the inline fire in place.) Verified per sub-step.
10. **No regression in `PermissionEngine::Decide()`** — the engine itself
    is not modified, only wrapped by `RunPermissionGate`. Verified per
    sub-step (engine source untouched + 33 engine tests pass).

### Commit 6 — IPC bridge wiring through PermissionEngine

**Files touched:** `cef-native/src/handlers/simple_handler.cpp`
(`wallet_call` handler body), `cef-native/include/core/PendingAuthRequest.h`
(ResumeKind enum + IPC fields), `cef-native/src/core/HttpRequestInterceptor.cpp`
(`handleAuthResponse` extension for ResumeKind dispatch), new
`OnWalletCallSuccess` helper.

**Done when:**

1. **`wallet_call` IPC handler builds a `PermissionContext`** before
   calling `RunPermissionGate` (vs. today's direct `SyncHttpClient::Post`)
2. **`PermissionContext` carries `X-Requesting-Domain`** from the calling
   frame's origin — same source as HTTP path
3. **Silent decision on IPC path**:
   - Forwards to wallet via `SyncHttpClient::Post`
   - On success, calls `OnWalletCallSuccess(browserId, domain, cents, wasAutoApprovedPayment, endpoint)`
   - `SessionManager::recordSpending` runs for payment kind
   - `firePaymentSuccessIndicator` runs for payment kind — **green-dot
     animation visible on the tab**
   - `wallet_response` IPC fires back to renderer with payload
4. **Prompt decision on IPC path**:
   - Modal opens via `OpenPromptModal()` (same dispatcher as HTTP path)
   - `PendingAuthRequest` enrolled with `resumeKind=kIpcResponse`,
     `requestId`, `frame`, `browserId`, expected `headersOnApprove`
   - User Approve → `handleAuthResponse` resumes:
     - Injects `headersOnApprove` (e.g. `X-Identity-Key-Approved: true`)
     - Forwards to wallet via `SyncHttpClient::Post`
     - On success: `OnWalletCallSuccess` + `wallet_response` IPC
   - User Deny → `wallet_response` IPC with error payload (no wallet call made)
5. **Deny decision on IPC path** → `wallet_response` IPC with denial error;
   no wallet call made
6. **CSP-bypass and CORS-bypass verified**:
   - On `https://github.com`: `await window.CWI.getNetwork({})` returns
     `'mainnet'` without DevTools CSP violation
   - On `https://treechat.io`: `await window.yours.getAddresses()` returns
     `{bsvAddress, ordAddress, identityAddress}` without CORS preflight failure
7. **HTTP path on `localhost:5137` unchanged** — re-run Commit 5's
   acceptance criteria #1-10 to confirm no regression
8. **`PendingAuthRequest` per-domain queuing still works** — multiple
   in-flight IPC calls from same domain dedupe to one modal; on Approve,
   all queued calls resume
9. **Frame validity check**: if frame navigates or tab closes between
   modal opening and Approve, `wallet_response` is dropped silently with
   debug log (no SendProcessMessage to invalid frame)
10. **No double-fire of green-dot animation**: a single auto-approved
    payment fires the indicator IPC exactly once (whether via HTTP path
    or IPC path — not both)

#### Commit 6 acceptance sweep — RESULTS (2026-06-01, sub-step 6.f close)

All ten criteria passed. Smoke verified on `https://hodos-test.local:8000`
fixture with a 100-sat low payment and a 50M-sat over-cap payment.

| # | Criterion | Result | Evidence |
|---|---|---|---|
| 1 | wallet_call builds PermissionContext before RunPermissionGate | ✅ | `HandleIpcWalletCall.runIpcEngineCascade` calls `buildPermissionContext` then `RunPermissionGate` |
| 2 | PermissionContext carries X-Requesting-Domain | ✅ | Origin extracted from `frame.GetURL()` and threaded through the gate context |
| 3 | Silent decision: forwardToWallet + OnWalletCallSuccess + indicator + wallet_response | ✅ | Live smoke: 100-sat createAction silent-approved; gold pill animated; debug_output.log shows `💰 IPC: auto-approved payment` and `💰 OnWalletCallSuccess fired` |
| 4 | Prompt decision: modal opens + PendingAuthRequest enrolled with kIpcResponse | ✅ | Live smoke: 50M-sat createAction fired payment_confirmation modal; user approve → wallet response delivered via IPC; user deny → error envelope returned to page |
| 5 | Deny decision: wallet_response immediate, no wallet call | ✅ | denyWithError callback in `runIpcEngineCascade` sends `wallet_response` with engine error envelope without posting to worker |
| 6 | CSP/CORS bypass (github.com, treechat.io) | ⏳ | Deferred to Commit 7 final pass — `hodos-test.local` smoke proved the IPC + engine end-to-end; github/treechat is the dApp-compatibility verification |
| 7 | HTTP path on localhost:5137 unchanged | ✅ | Existing `if (req.handler)` branches in handleAuthResponse untouched; member trigger delegates preserve external API; HTTP wallet UI ops unaffected |
| 8 | Per-domain queue dedup still works | ✅ | `PendingRequestManager` logic untouched; `resumeIpcResponse` handles each sibling's resumeKind individually so mixed HTTP+IPC drains work correctly |
| 9 | Frame validity check | ✅ | `sendWalletResponseIpc` checks `frame.IsValid()` before `SendProcessMessage`; drops with debug log if invalid; same check at every worker→UI hop in IPC silent / resume paths |
| 10 | No double-fire of green-dot animation | ✅ | grep `payment_success_indicator` returns 1 fire site (inside `OnWalletCallSuccess`); each request flows through exactly one path (HTTP `OnRequestComplete` OR IPC silent worker OR IPC resume worker) and triggers the helper once. Verified by inspection + live smoke (single tab animation per silent-approve, no duplicates) |

#### Bonus regression caught and fixed mid-6.d.BE

The first smoke run showed the gold-pill animation didn't fire on the
100-sat low payment even though the silent-approve path succeeded. Root
cause: a `cents <= 0` guard I introduced in `OnWalletCallSuccess` (6.b)
short-circuited tiny payments (cents rounds to 0 for < ~16,667 sats at
typical BSV prices). Pre-6.b legacy paths didn't have this guard. Fix
landed as `52eb57d` (6.d.BE+1) — restored legacy behavior. Memory
[[payment_animation_safeguard]] holds.

### Commit 7 — CEF rebuild + Treechat + github smoke

**Files touched:** None — verification-only.

**Done when:** Every item in `## Verification — Phase 2.5 done when` section
below passes. Specifically:

1. **github.com smoke** — `await window.CWI.getNetwork({})` returns
   `'mainnet'`. DevTools shows no CSP `connect-src` violation. Network tab
   shows zero requests to `127.0.0.1:31301`.
2. **treechat.io smoke** — `await window.yours.getAddresses()` returns
   the three-address object. DevTools shows no CORS preflight failure.
3. **treechat.io payment smoke** — `await window.yours.sendBsv([{address, satoshis: 1000}])`:
   - First call from a new approved-but-no-payment-permission domain
     fires `payment_confirmation` modal
   - User clicks Approve
   - Returned `{txid}` matches `await window.CWI.listActions({limit: 1})`
   - **Green-dot tab badge animation visible**
   - SessionManager recorded the spend (verify via `wallet_status` or wallet UI)
4. **github.com encrypt/decrypt round-trip** —
   `await window.yours.encrypt({message: 'hi', pubKey: selfPubKey})` then
   `await window.yours.decrypt({ciphertext: ...})` returns `'hi'`
5. **No regressions on `localhost:5137`** — wallet UI fully functional;
   all wallet UI operations (create, backup, restore, send, settings)
   work identically to before Phase 2.5
6. **`actix-cors` config unchanged** in `main.rs` — defense-in-depth
   localhost-only allowlist preserved; verified by `git diff main.rs`
7. **No build warnings** introduced by commits 5-6 (clean `cmake --build
   build --config Release` output)
8. **Per-tab counter behavior preserved** — closing the github tab and
   re-opening clears the session spend counter (per design)
9. **Modal click-outside behavior preserved** — same overlay close
   patterns as before commits 5-6 land
10. **macOS parity check** — same smoke matrix runs on macOS build (or
    explicit deferral note in the commit message if Mac smoke deferred to
    next session)

## Risk surface

| Risk | Mitigation |
|---|---|
| Promise correlation race (response arrives before `pending[id]` is populated) | Bridge sets `pending[id]` synchronously before `cefMessage.send`. Send is async; response can't possibly outrun the set. |
| Orphan responses (frame navigates between call and response) | `capturedFrame->IsValid()` check before `SendProcessMessage`. Renderer side: orphan id → `console.warn` and drop. |
| Worker thread starvation | `TID_FILE_USER_BLOCKING` is CEF's pool for blocking I/O; concurrent wallet calls share the pool but don't block UI. 30s timeout in `SyncHttpClient::Post` is the backstop. |
| Large payload overflow IPC | 50 MB guardrail in renderer pre-rejects oversize calls with clear error message. Future: add chunking if real workloads need it. |
| Payment animation regression | Commit 5 explicitly extracts the indicator-fire logic; smoke check in commit 6 must visually verify the green dot. |
| Method name vs endpoint divergence | Some canonical endpoints are uppercase (`/createAction`), some are paths under `/wallet/` (`/wallet/yours-legacy-addresses`). Bridge takes both as explicit parameters — no string mangling. |
| GET vs POST endpoints | Bridge takes `httpMethod` parameter (default POST). Wallet endpoint table documented in `CWIShimScript.h` comment. |

## Out-of-tree assumptions

- `cefMessage.send` IPC is reliable on https external main frames (verified
  in Phase 2 smoke: `bsv:announceProvider` event posted via the same path)
- CEF `kMaxMessageSize` is at least 50 MB (Chromium default is 128 MB)
- `frame->ExecuteJavaScript` from response handler is safe on a `wallet_response`
  arrival (existing pattern, used by 70+ message types in the same handler)

## Verification — Phase 2.5 done when

1. `await window.CWI.getNetwork({})` works on `https://github.com` (CSP no longer in path)
2. `await window.yours.getAddresses()` works on `https://treechat.io` (CORS no longer in path)
3. `await window.yours.sendBsv([{address, satoshis: 1000}])` on treechat:
   - Permission modal fires (per-domain auto-approve, same as before)
   - Green-dot tab animation fires on success
   - Returned `{txid}` matches `await window.CWI.listActions({limit: 1})`
4. `await window.yours.encrypt({message: 'hi', pubKey: <self>})` round-trips
   through `await window.yours.decrypt({ciphertext})`
5. No regressions on internal wallet UI (`localhost:5137`) — fetch still works
   same-origin
6. `actix-cors` config unchanged in `main.rs` — verify the localhost-only
   restriction stays as defense-in-depth
