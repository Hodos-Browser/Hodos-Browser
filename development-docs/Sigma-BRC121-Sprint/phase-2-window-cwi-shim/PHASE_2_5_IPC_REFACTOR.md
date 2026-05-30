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
| **2.5-A — Planning (next session)** | **pending** | Fill `WALLET_API_MAP.md`; finalize extraction interface design; lock acceptance criteria per commit |
| 2.5-B — Commit 5 extraction | pending | Reusable gate helpers; HTTP-path behavior unchanged |
| 2.5-C — Commit 6 IPC wiring | pending | `wallet_call` runs full engine cascade |
| 2.5-D — Commit 7 smoke | pending | End-to-end on github + Treechat with engine in path |

Each sub-phase is one focused session. Plan a clean handoff doc between
sessions so context-clear ↔ context-load is lossless.

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
`HttpRequestInterceptor.cpp:1656-1681`'s `AsyncWalletResourceHandler` after
every successfully-auto-approved payment. Under IPC dispatch, **the request
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
