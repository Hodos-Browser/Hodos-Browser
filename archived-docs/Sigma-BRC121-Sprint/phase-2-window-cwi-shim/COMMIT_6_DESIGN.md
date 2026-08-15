# Phase 2.5 Commit 6 — IPC Bridge Wiring Through PermissionEngine

> **Status:** Design draft 2026-06-01. Awaiting approval before any source code is touched.
>
> **Scope:** Implementation design for Commit 6 (sub-phase 2.5-C). Sister doc to
> [`PHASE_2_5_IPC_REFACTOR.md`](./PHASE_2_5_IPC_REFACTOR.md) — that one carries the
> acceptance criteria, this one carries the *how*.
>
> **Approval gate:** Per `feedback_research_first_do_it_once` memory, no code lands
> until the user signs off on this design. Open questions in §11 require explicit
> answers before code work begins.

## 0. Glossary

| Term | Meaning |
|---|---|
| HTTP path | Today's flow when something calls the wallet via CEF resource interception (`AsyncWalletResourceHandler::Open()` fires). Used by anything that triggers CEF's network stack. |
| IPC path | The `wallet_call` IPC bridge installed in Phase 2.5 commits 1-4. Used by the V8 shim on external dApps. Currently bypasses the engine. |
| Engine cascade | `PermissionEngine::Decide(ctx)` + the matching `RunPermissionGate` dispatch. After 5.a-5.f this is unified for all gates on the HTTP path. |
| Gate | One of the seven decision branches: payment / identity-key / key-linkage / cert-disclosure / protocol-grant / basket-grant / counterparty-grant. |
| Resume | The act of resolving a pending modal: user clicks Approve/Deny → `handleAuthResponse` looks up the pending request → re-issues the wallet call (Approve) or sends an error (Deny). |
| 4 Decisions | The architectural decisions locked in 2.5-A planning (`5ae6242`). Numbered D1-D4. |

## 1. Architecture before vs. after

### Before Commit 6 (today, post-5.f)

```
External dApp page on https://github.com:
    window.CWI.createAction({...})
        │
        ▼  (V8 shim → IPC bridge from commits 1-4)
    cefMessage.send("wallet_call", [reqId, method, endpoint, body, httpMethod])
        │
        ▼  (UI thread → TID_FILE_USER_BLOCKING)
    SyncHttpClient::Post("http://127.0.0.1:31301/createAction", ...)
        │                          ★ ENGINE CASCADE BYPASSED ★
        ▼                          ★ Only Rust check_domain_approved fires ★
    Rust wallet → 403 ERR_DOMAIN_NOT_APPROVED (if domain not pre-approved)
        │
        ▼  (worker thread → UI thread)
    SendProcessMessage(wallet_response with error)

Internal frontend on http://localhost:5137 OR any fetch-based wallet caller:
    fetch("http://127.0.0.1:31301/createAction", ...)
        │
        ▼  (CEF resource interception)
    AsyncWalletResourceHandler::Open()
        │
        ├─ requestDomain_ starts with 127.0.0.1 or localhost ──→  Bypass to wallet (internal)
        └─ Approved domain
            │
            ▼  (per 5.a-5.f)
        Build PermissionContext → RunPermissionGate(ctx, cb)
            │
            ├─ Silent ─→ forwardToWallet callback fires
            ├─ Prompt ─→ openModal callback fires (PendingRequestManager + CreateNotificationOverlayTask)
            └─ Deny   ─→ denyWithError callback fires
```

The internal-frontend path works for our wallet UI but doesn't help external dApps,
because anything served from `localhost:5137` hits the internal-origin bypass at
the top of `Open()`. **External dApps go through the IPC bridge, which bypasses
the engine.**

### After Commit 6

```
External dApp page on https://github.com:
    window.CWI.createAction({...})
        │
        ▼
    cefMessage.send("wallet_call", [reqId, method, endpoint, body, httpMethod])
        │
        ▼  (UI thread arrives here)
    wallet_call handler
        │
        ├─ origin starts with 127.0.0.1 or localhost ──→  bypass to wallet (internal IPC)
        ├─ wallet does not exist ──→  send wallet_response with NO_WALLET error
        ├─ blocked domain ──→  send wallet_response with blocked error
        ├─ unknown domain ──→  fire domain_approval modal (or manifest_connect_bundle),
        │                     enroll PendingAuthRequest as kIpcResponse
        └─ approved domain
            │
            ▼
        Build PermissionContext → RunPermissionGate(ctx, ipcCb)
            │
            ├─ Silent ─→ ipcCb.forwardToWallet:
            │              CefPostTask(TID_FILE_USER_BLOCKING):
            │                  inject any headers (X-Identity-Key-Approved etc.)
            │                  SyncHttpClient::Post → Rust wallet
            │                  on success: OnWalletCallSuccess (record + indicator IPC)
            │                  CefPostTask(TID_UI): send wallet_response
            │
            ├─ Prompt ─→ ipcCb.openModal:
            │              enroll PendingAuthRequest (kIpcResponse, frame, browserId, headersOnApprove)
            │              OpenPromptModal(promptType, modalCtx)  ← Decision 3 dispatcher
            │              postIpcAuthTimeout(reqId, frame, errorJson)
            │
            └─ Deny   ─→ ipcCb.denyWithError:
                           send wallet_response with engine error

User clicks Approve in modal:
    React sends approve_xxx IPC
        │
        ▼
    handleAuthResponse(requestId, responseData)
        │
        ├─ pop PendingAuthRequest
        ├─ resumeKind == kHttpCallback ─→  existing behavior unchanged
        │                                  (call handler->onAuthResponseReceived or kick off StartAsyncHTTPRequestTask)
        ├─ resumeKind == kIpcResponse  ─→  CefPostTask(TID_FILE_USER_BLOCKING):
        │                                      inject req.headersOnApprove + standard headers
        │                                      SyncHttpClient::Post → Rust
        │                                      on success: OnWalletCallSuccess
        │                                      CefPostTask(TID_UI): send wallet_response
        │
        └─ drain sibling requests for the same domain (already works — IPC siblings get
           the same resume treatment)
```

Internal-frontend fetch path is **unchanged**. Internal-origin IPC requests (rare
but possible) get an internal bypass path that matches `Open()`'s.

## 2. Threading model

All work happens on three CEF threads:

| Thread | What runs there |
|---|---|
| `TID_UI` | IPC arrives (`OnProcessMessageReceived`). All CefFrame methods. Modal dispatch (`CreateNotificationOverlayTask`). `handleAuthResponse` continuation. |
| `TID_FILE_USER_BLOCKING` | Blocking HTTP via `SyncHttpClient::Post/Get`. The only thread that may block for hundreds of ms. |
| `TID_IO` | Existing CEF resource handler thread for the HTTP path. Not used by IPC path. |

### IPC path threading trace

```
TID_UI: wallet_call arrives
  ↓ (synchronous, all on TID_UI)
  Read frame.GetURL() → origin
  Read DomainPermissionCache, WalletStatusCache, BSVPriceCache, SessionManager
  Build PermissionContext
  Build IPC GateCallbacks (lambdas capture: requestId, frame, browserId, origin, method, httpMethod, body, sat/cents for payment)
  Call RunPermissionGate(ctx, cb)
      ↓
    Silent branch (forwardToWallet lambda runs synchronously on TID_UI):
      CefPostTask(TID_FILE_USER_BLOCKING, [...])
        ↓ TID_FILE_USER_BLOCKING
        SyncHttpClient::Post / Get / Request
          ↓ blocks for response
        Got HttpResponse
          ↓
        Decide if was-auto-approved payment + cents
        CefPostTask(TID_UI, [...])
          ↓ TID_UI
          If frame still valid:
            OnWalletCallSuccess(browserId, domain, cents, wasAutoApprovedPayment, endpoint)
            frame->SendProcessMessage(wallet_response)
    Prompt branch (openModal lambda runs synchronously on TID_UI):
      PendingRequestManager::addRequest(...)
      CefPostTask(TID_UI, new CreateNotificationOverlayTask(...))
        ↓ TID_UI later
        Modal overlay created
      postIpcAuthTimeout(requestId, frame, errorJson, kPromptAuthTimeoutMs)
        ↓ scheduled with CefPostDelayedTask(TID_UI, ...)
        After delay if still pending: send wallet_response with timeout error
    Deny branch (denyWithError lambda runs synchronously on TID_UI):
      Send wallet_response immediately

Later: user clicks Approve in React modal
TID_UI: approve_xxx IPC arrives
  ↓
  Look up domain → requestId → PendingAuthRequest
  handleAuthResponse(requestId, responseData)
    ↓
    Pop PendingAuthRequest
    switch (req.resumeKind):
      case kHttpCallback: existing behavior (untouched)
      case kIpcResponse:
        CefPostTask(TID_FILE_USER_BLOCKING, [...])
          ↓ TID_FILE_USER_BLOCKING
          headers = {Content-Type, X-Requesting-Domain} ∪ req.headersOnApprove
          SyncHttpClient::Post(...)
          decide payment + cents
          CefPostTask(TID_UI, [...])
            ↓ TID_UI
            If frame valid:
              OnWalletCallSuccess(...)
              frame->SendProcessMessage(wallet_response)
```

### Concurrency guarantees / hazards

| Item | Guarantee | Risk |
|---|---|---|
| Cache reads (DomainPermissionCache etc.) | Mutex-protected, any thread | None |
| CefFrame methods | TID_UI only | If we slip a frame method onto a worker thread, hang or crash. **Must verify all frame calls happen on TID_UI.** |
| `CefRefPtr<CefFrame>` capture into worker lambda | Safe (refcounted) | Frame may be destroyed before worker runs — must check `IsValid()` before SendProcessMessage |
| `PendingRequestManager` | Mutex-protected, any thread | None |
| Lambda captures of `int`, `int64_t`, `std::string` | Safe by-value | None |
| Lambda capture of `int& handle_request` | Not applicable for IPC path — that's an HTTP-path-only ref-param |

## 3. PendingAuthRequest extension (Decision 2)

### Today's struct

```cpp
struct PendingAuthRequest {
    std::string requestId;
    std::string domain;
    std::string method;
    std::string endpoint;
    std::string body;
    std::string type;
    CefRefPtr<CefResourceHandler> handler;
};
```

### Post-Commit-6 struct

```cpp
enum class ResumeKind {
    kHttpCallback,   // Resume via handler->onAuthResponseReceived (today's behavior)
    kIpcResponse,    // Resume via frame->SendProcessMessage(wallet_response)
    kInternal,       // Reserved for Phase 2.6 (Rust-initiated resume)
};

struct PendingAuthRequest {
    // Existing fields
    std::string requestId;
    std::string domain;
    std::string method;
    std::string endpoint;
    std::string body;
    std::string type;
    CefRefPtr<CefResourceHandler> handler;   // valid iff resumeKind == kHttpCallback

    // New for Commit 6 (Decision 2)
    ResumeKind resumeKind = ResumeKind::kHttpCallback;   // default preserves HTTP semantics
    CefRefPtr<CefFrame> frame;                            // valid iff resumeKind == kIpcResponse
    int browserId = 0;                                    // valid iff resumeKind == kIpcResponse
    std::map<std::string, std::string> headersOnApprove;  // injected by handleAuthResponse on Approve
    std::string httpMethod = "POST";                      // GET/POST/DELETE/PUT/PATCH for IPC re-issue
};
```

### addRequest signatures after Commit 6

```cpp
// Existing — preserved for backward compat with HTTP-path call sites in 5.b-5.f.
// Default-constructs the new fields. Used by Open()'s lambdas.
std::string addRequest(const std::string& domain,
                       const std::string& method,
                       const std::string& endpoint,
                       const std::string& body,
                       CefRefPtr<CefResourceHandler> handler,
                       const std::string& type = "domain_approval");

// New — takes a fully-constructed PendingAuthRequest. Used by IPC path.
std::string addRequest(PendingAuthRequest req);
```

The new overload assigns a `requestId` internally (same generator as before) and
returns it.

## 4. GateCallbacks for IPC path

Concrete shape the IPC-path `wallet_call` handler builds. Each lambda closes
over by-value snapshots of UI-thread state at the moment the gate runs.

### forwardToWallet (Silent decision)

```cpp
cb.forwardToWallet =
    [requestId, methodName, endpoint, bodyJson, httpMethod,
     origin, capturedFrame, browserId,
     cents, isPayment, sendIdentityKeyApproved, sendKeyLinkageApproved
    ]() {
        // We're on TID_UI. Increment counters now for payment (matches 5.b shape).
        if (isPayment) {
            SessionManager::GetInstance().incrementRateCounter(browserId);
            SessionManager::GetInstance().incrementPaymentCount(browserId);
        }

        CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](
            std::string requestId, std::string methodName, std::string endpoint,
            std::string bodyJson, std::string httpMethod, std::string origin,
            CefRefPtr<CefFrame> capturedFrame, int browserId,
            int64_t cents, bool isPayment, bool sendIdentityKeyApproved, bool sendKeyLinkageApproved
        ) {
            std::map<std::string, std::string> headers = {
                {"Content-Type", "application/json"},
            };
            if (!origin.empty()) headers["X-Requesting-Domain"] = origin;
            if (sendIdentityKeyApproved) headers["X-Identity-Key-Approved"] = "true";
            if (sendKeyLinkageApproved)  headers["X-Key-Linkage-Approved"]  = "true";

            HttpResponse resp = ...;  // same Get/Post/Request dispatch as today

            bool ok = resp.success && resp.statusCode >= 200 && resp.statusCode < 300;
            std::string payload = buildIpcPayload(resp, ok);  // factored helper

            // Hop to TID_UI for IPC + indicator
            CefPostTask(TID_UI, base::BindOnce([](
                std::string requestId, bool ok, std::string payload,
                CefRefPtr<CefFrame> capturedFrame, int browserId,
                std::string methodName, std::string origin, std::string endpoint,
                int64_t cents, bool isPayment
            ) {
                if (!capturedFrame || !capturedFrame->IsValid()) { ... drop with debug log ... }
                if (ok && isPayment && cents > 0) {
                    OnWalletCallSuccess(browserId, origin, cents,
                                        /*wasAutoApprovedPayment=*/true, endpoint);
                }
                sendWalletResponse(capturedFrame, requestId, ok, payload);  // factored helper
            }, ...));
        }, ...));
    };
```

### openModal (Prompt decision)

```cpp
cb.openModal =
    [requestId, methodName, endpoint, bodyJson, httpMethod, origin,
     capturedFrame, browserId,
     /* per-branch context: certInfo, scope, satoshis, cents, perm, ... */
    ](const std::string& promptType, const std::string& engineExtraParams) {
        // 1. Enroll PendingAuthRequest with kIpcResponse + frame + browserId + headersOnApprove.
        PendingAuthRequest req;
        req.domain = origin;
        req.method = methodName;
        req.endpoint = endpoint;
        req.body = bodyJson;
        req.type = promptType;
        req.handler = nullptr;
        req.resumeKind = ResumeKind::kIpcResponse;
        req.frame = capturedFrame;
        req.browserId = browserId;
        req.httpMethod = httpMethod;
        req.headersOnApprove = headersToInjectOnApprove(promptType);
        const std::string reqId = PendingRequestManager::GetInstance().addRequest(std::move(req));

        // 2. Build branch-specific extraParams (same logic as 5.b-5.f's HTTP lambdas).
        std::string extraParams = buildExtraParamsForPrompt(promptType, /*context*/);

        // 3. Dispatch via Decision 3's OpenPromptModal.
        OpenPromptModal(promptType, ModalContext{origin, methodName, endpoint, bodyJson},
                        reqId, extraParams);

        // 4. Arm IPC-side timeout.
        postIpcAuthTimeout(reqId, capturedFrame, timeoutErrorFor(promptType),
                           kPromptAuthTimeoutMs);
    };
```

`headersToInjectOnApprove(promptType)` maps:
- `identity_key_reveal` → `{X-Identity-Key-Approved: true}`
- `key_linkage_reveal`  → `{X-Key-Linkage-Approved: true}`
- everything else        → `{}`

### denyWithError (Deny decision from gate)

```cpp
cb.denyWithError =
    [requestId, capturedFrame, browserId, methodName](const std::string& errorJson) {
        if (!capturedFrame || !capturedFrame->IsValid()) {
            LOG_DEBUG_BROWSER("wallet_call deny dropped — frame invalid");
            return;
        }
        sendWalletResponse(capturedFrame, requestId, /*ok=*/false, errorJson);
    };
```

## 5. handleAuthResponse extension

Existing signature stays:

```cpp
void handleAuthResponse(const std::string& requestId, const std::string& responseData);
```

Body branches on `req.resumeKind`:

```cpp
void handleAuthResponse(const std::string& requestId, const std::string& responseData) {
    PendingAuthRequest req;
    if (!PendingRequestManager::GetInstance().popRequest(requestId, req)) return;

    bool isRejection = parseHasError(responseData);

    // Domain-approval rejection: blocklist the domain (existing logic, untouched).
    if (isRejection && isDomainApproval(req.type)) {
        DomainPermissionCache::GetInstance().setBlocked(req.domain);
    }

    switch (req.resumeKind) {
        case ResumeKind::kHttpCallback: {
            // Existing behavior. UNCHANGED from today's code.
            // (calls walletHandler->onAuthResponseReceived OR
            //  CefPostTask(TID_IO, new StartAsyncHTTPRequestTask(walletHandler))
            //  for approved sibling drain — same as today)
            resumeHttpCallback(req, responseData, isRejection);
            break;
        }
        case ResumeKind::kIpcResponse: {
            resumeIpcResponse(req, responseData, isRejection);
            break;
        }
        case ResumeKind::kInternal:
            // Phase 2.6 placeholder. Log + drop.
            LOG_WARNING_HTTP("handleAuthResponse: kInternal not yet implemented");
            break;
    }

    // Drain sibling requests for the same domain — works for mixed kinds (each
    // sibling's resumeKind is honored independently).
    auto siblings = PendingRequestManager::GetInstance().popAllForDomain(req.domain);
    for (auto& sib : siblings) {
        switch (sib.resumeKind) {
            case ResumeKind::kHttpCallback: resumeHttpCallback(sib, responseData, isRejection); break;
            case ResumeKind::kIpcResponse:  resumeIpcResponse(sib, responseData, isRejection); break;
            case ResumeKind::kInternal:     /* drop */ break;
        }
    }
}
```

`resumeIpcResponse`:

```cpp
void resumeIpcResponse(PendingAuthRequest& req, const std::string& responseData, bool isRejection) {
    if (isRejection) {
        // User Denied. Send wallet_response with the error envelope. No wallet call.
        if (req.frame && req.frame->IsValid()) {
            sendWalletResponse(req.frame, req.requestId, /*ok=*/false, responseData);
        }
        return;
    }

    // User Approved. Re-issue the wallet call on a worker thread, then send wallet_response.
    CefPostTask(TID_FILE_USER_BLOCKING, base::BindOnce([](PendingAuthRequest req) {
        std::map<std::string, std::string> headers = {
            {"Content-Type", "application/json"},
        };
        if (!req.domain.empty()) headers["X-Requesting-Domain"] = req.domain;
        for (auto& kv : req.headersOnApprove) headers[kv.first] = kv.second;

        std::string url = "http://127.0.0.1:31301" + req.endpoint;
        HttpResponse resp = dispatchByMethod(req.httpMethod, url, req.body, headers);

        bool ok = resp.success && resp.statusCode >= 200 && resp.statusCode < 300;
        std::string payload = buildIpcPayload(resp, ok);
        bool wasPayment = isPaymentEndpoint(req.endpoint);
        int64_t cents = wasPayment ? extractCentsFromBody(req.body) : 0;  // recompute, BSV price may have moved

        CefPostTask(TID_UI, base::BindOnce([](PendingAuthRequest req, bool ok, std::string payload, bool wasPayment, int64_t cents) {
            if (!req.frame || !req.frame->IsValid()) return;
            if (ok && wasPayment && cents > 0) {
                OnWalletCallSuccess(req.browserId, req.domain, cents,
                                    /*wasAutoApprovedPayment=*/true, req.endpoint);
            }
            sendWalletResponse(req.frame, req.requestId, ok, payload);
        }, std::move(req), ok, payload, wasPayment, cents));
    }, std::move(req)));
}
```

## 6. OnWalletCallSuccess helper (Decision 4)

```cpp
// In HttpRequestInterceptor.cpp anonymous namespace (or a small new TU)
void OnWalletCallSuccess(int browserId,
                        const std::string& domain,
                        int64_t cents,
                        bool wasAutoApprovedPayment,
                        const std::string& endpoint) {
    if (!wasAutoApprovedPayment || cents <= 0) return;

    SessionManager::GetInstance().recordSpending(browserId, cents);

    CefRefPtr<CefBrowser> headerBrowser = SimpleHandler::GetHeaderBrowser();
    if (!headerBrowser || !headerBrowser->GetMainFrame()) return;

    int tabId = TabManager::GetInstance().GetTabIdForBrowserIdentifier(browserId);
    nlohmann::json payload;
    payload["browserId"] = tabId;
    payload["domain"] = domain;
    payload["cents"] = cents;

    CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("payment_success_indicator");
    msg->GetArgumentList()->SetString(0, payload.dump());
    headerBrowser->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);

    LOG_DEBUG_HTTP("💰 OnWalletCallSuccess fired (" + std::to_string(cents) + " cents from " + domain
                   + ", cefBrowserId=" + std::to_string(browserId) + " → tabId=" + std::to_string(tabId)
                   + ", endpoint=" + endpoint + ")");
}
```

Call sites after Commit 6:
- HTTP path's `AsyncHTTPClient::OnRequestComplete` (current L2911-2931) → replace inline with `OnWalletCallSuccess(...)`
- BRC-121's `firePaymentSuccessIpc` body → ALREADY similar; leaves alone for Commit 6 minimum scope (deferred consolidation note in §11 Q4).
- IPC path's silent-approve worker callback (new)
- IPC path's resume-after-approve worker callback (new)

**Acceptance criterion #9 grep**: `grep -c 'CefProcessMessage::Create("payment_success_indicator")' HttpRequestInterceptor.cpp` should drop from 2 → 1 after this refactor (only BRC-121's firePaymentSuccessIpc keeps the inline; createAction's L2926 becomes a call to OnWalletCallSuccess which contains the single new fire site). **Net fire-site count across the file: still 2** (one inside OnWalletCallSuccess, one in firePaymentSuccessIpc). The grep regex on the doc needs updating to count function calls (`OnWalletCallSuccess(` plus the BRC-121 `Create("payment_success_indicator")`).

**Acceptance criterion #10 (no double-fire)**: ensured by helper-only-call discipline. Each request flows through exactly one path (HTTP or IPC), and within that path exactly one success callback runs.

## 7. Decision 3 — OpenPromptModal dispatcher + free-function trigger extraction

Decision 3 lands here. Today's `triggerXxxModal` member functions become free
functions taking explicit context. A single dispatcher routes by promptType.

### ModalContext struct

```cpp
struct ModalContext {
    std::string domain;
    std::string method;
    std::string endpoint;
    std::string body;
    // For payment-cap-class prompts, extraParams gets the pre-built scope/cents/etc.
    // For domain_approval / brc100_auth / manifest_connect_bundle, extraParams is "".
    // The trigger fn reads what it needs.
};
```

### Free-function trigger signatures

```cpp
// Used by both HTTP and IPC paths. Each fn dispatches the CreateNotificationOverlayTask
// for its modal type. None of them touch handler instance state.
void openDomainApprovalModal(const ModalContext& ctx);
void openManifestConnectBundleModal(const ModalContext& ctx, const hodos::Manifest& m);
void openPaymentConfirmationModal(const ModalContext& ctx, const std::string& extraParams);
void openIdentityKeyRevealModal(const ModalContext& ctx);
void openKeyLinkageRevealModal(const ModalContext& ctx);
void openCertificateDisclosureModal(const ModalContext& ctx, const CertDisclosureInfo& info);
void openProtocolPermissionPromptModal(const ModalContext& ctx, const std::string& extraParams);
void openBasketPermissionPromptModal(const ModalContext& ctx, const std::string& extraParams);
void openCounterpartyPermissionPromptModal(const ModalContext& ctx, const std::string& extraParams);
void openBRC100AuthApprovalModal(const ModalContext& ctx);
```

### OpenPromptModal dispatcher

```cpp
void OpenPromptModal(const std::string& promptType,
                     const ModalContext& ctx,
                     const std::string& requestId,
                     const std::string& extraParams = "") {
    // Most modals only need ctx + extraParams. Cert disclosure + manifest bundle
    // have extra typed payloads (CertDisclosureInfo, Manifest) so they don't go
    // through this dispatcher — callers invoke the matching opener directly.
    if      (promptType == "domain_approval")               openDomainApprovalModal(ctx);
    else if (promptType == "payment_confirmation"
          || promptType == "rate_limit_exceeded")           openPaymentConfirmationModal(ctx, extraParams);
    else if (promptType == "identity_key_reveal")           openIdentityKeyRevealModal(ctx);
    else if (promptType == "key_linkage_reveal")            openKeyLinkageRevealModal(ctx);
    else if (promptType == "protocol_permission_prompt")    openProtocolPermissionPromptModal(ctx, extraParams);
    else if (promptType == "basket_permission_prompt")      openBasketPermissionPromptModal(ctx, extraParams);
    else if (promptType == "counterparty_permission_prompt") openCounterpartyPermissionPromptModal(ctx, extraParams);
    else if (promptType == "brc100_auth")                   openBRC100AuthApprovalModal(ctx);
    else LOG_WARNING_HTTP("OpenPromptModal: unknown promptType " + promptType);
}
```

### HTTP-path migration

The 5.b-5.f lambdas in `Open()` switch from calling `this->triggerXxxModal(...)`
to calling the matching free function. PendingRequestManager::addRequest is also
moved out of the openers — the caller (either Open() or the IPC handler) controls
the resume kind via the new `addRequest(PendingAuthRequest)` overload, then calls
the matching opener for the modal dispatch.

This is a structural refactor of 5.b-5.f's `openModal` lambda bodies. Each
lambda gets shorter — it builds the PendingAuthRequest (with `kHttpCallback`),
calls `addRequest`, then calls `OpenPromptModal` or a direct opener.

## 8. postIpcAuthTimeout

New free function, mirrors `AsyncWalletResourceHandler::postAuthTimeout` but for
the IPC path (no handler instance to call back).

```cpp
void postIpcAuthTimeout(const std::string& requestId,
                        CefRefPtr<CefFrame> frame,
                        const std::string& errorJson,
                        int delayMs) {
    CefPostDelayedTask(TID_UI, base::BindOnce([](
        std::string requestId, CefRefPtr<CefFrame> frame, std::string errorJson
    ) {
        // Only fire if the request is still pending (user hasn't resolved yet).
        PendingAuthRequest req;
        if (!PendingRequestManager::GetInstance().popRequest(requestId, req)) return;
        if (!frame || !frame->IsValid()) return;
        sendWalletResponse(frame, requestId, /*ok=*/false, errorJson);
        LOG_DEBUG_HTTP("⏰ IPC auth timeout fired for " + requestId);
    }, requestId, frame, errorJson), delayMs);
}
```

## 9. Internal-origin + no-wallet + blocked + unknown handling on IPC path

Today's wallet_call handler skips ALL these checks. After Commit 6:

```cpp
// In wallet_call IPC handler, BEFORE building the gate context:

// 1. Internal origin bypass (matches Open() L1886).
bool isInternalOrigin = origin.empty()
    || origin.find("127.0.0.1") == 0
    || origin.find("localhost")  == 0;
if (isInternalOrigin) {
    // Skip the engine, run the call directly (today's behavior).
    runIpcCallDirect(...);
    return true;
}

// 2. Wallet existence check (matches Open() L1897).
if (!WalletStatusCache::GetInstance().walletExists()) {
    sendWalletResponse(frame, requestId, false,
        "{\"error\":\"No wallet exists\",\"code\":\"NO_WALLET\",\"status\":\"error\"}");
    return true;
}

// 3. DomainPermissionCache lookup.
auto perm = DomainPermissionCache::GetInstance().getPermission(origin);

if (perm.trustLevel == "blocked") {
    sendWalletResponse(frame, requestId, false,
        "{\"error\":\"Domain blocked\",\"status\":\"error\"}");
    return true;
}

if (perm.trustLevel == "unknown") {
    // Mirror Open() L1923-1971: try manifest first, then domain_approval.
    // The opener handles addRequest + CreateNotificationOverlayTask.
    handleIpcUnknownTrust(origin, /*ctx*/, requestId, frame, browserId, ...);
    return true;
}

// 4. Approved trust → engine cascade (the new code path described in §4-5).
runIpcGate(...);
return true;
```

The "approved → engine cascade" branch is where most of the new IPC-path code
lives. The other branches are short-circuits that map 1:1 to `Open()`'s existing
precondition checks.

## 10. Sub-commit breakdown

Total commit count for Commit 6: **6 sub-commits**, mirroring 5.a-5.f's discipline.

| Sub-commit | Scope | Test |
|---|---|---|
| 6.a | Extend `PendingAuthRequest` struct + add `addRequest(PendingAuthRequest)` overload. ResumeKind enum. No consumers. | Build clean; existing tests pass. |
| 6.b | New `OnWalletCallSuccess` helper. Refactor HTTP path's `AsyncHTTPClient::OnRequestComplete` L2911-2931 to call it. Grep verifies fire-site count and behavior preservation. | Build clean; tests pass; grep count update. |
| 6.c | Extract `triggerXxxModal` member fns to free `openXxxModal` functions + `OpenPromptModal` dispatcher (Decision 3). Update 5.b-5.f's openModal lambdas to call the new free functions. **No behavior change on HTTP path.** | Build clean; tests pass; visual check that modals still fire for the HTTP path's 6 gates. |
| 6.d | IPC path entry: rewrite `wallet_call` handler to do internal/wallet/blocked/unknown checks + build PermissionContext + run RunPermissionGate with IPC callbacks. **Engine cascade fires from external dApp traffic for the first time.** New `postIpcAuthTimeout` helper. | Build clean; partial cumulative smoke (Silent + Deny paths from gate). Prompt path fires modal but user-resolve won't work until 6.e. |
| 6.e | `handleAuthResponse` extension for `ResumeKind::kIpcResponse` dispatch. Sibling drain handles mixed-kind requests. **Full approve/deny resume loop works end-to-end via IPC.** | **Full cumulative smoke** — payment / identity-key / key-linkage / cert-disclosure / scoped-grant all fire properly on external dApps. |
| 6.f | Cleanup: final line-count verification, criterion #1-10 check, regression run against HTTP path, doc updates. | Acceptance criteria sweep. |

Estimated time: 4-6 hours of focused work across the six sub-commits, with the
cumulative smoke at 6.e being the load-bearing verification.

## 11. Open questions — RESOLVED 2026-06-01

All 8 questions answered. Decisions locked. Code work for sub-commits 6.a-6.f
proceeds against these answers.

| Q | Answer | Note |
|---|---|---|
| Q1 — Internal-origin bypass on IPC | **Yes** | Match Open() L1886 behavior — internal frontend stays trusted, no engine gate |
| Q2 — Consolidate `firePaymentSuccessIpc` into `OnWalletCallSuccess` | **Yes, in 6.b** | Single source of truth for the green-dot fire. BRC-121's counter increments stay adjacent to the call (2-line block) since BRC-121 doesn't have a silent-approve step |
| Q3 — Manifest-aware bundle dispatch on IPC unknown-trust | **Yes** | Mirror HTTP path — same UX whether dApp arrives via shim or fetch |
| Q4 — Re-extract cents at re-issue time | **Yes** | Matches HTTP path freshness behavior; `preCalculatedCents_` equivalent |
| Q5 — Active cancel on tab close | **No** | Timeout handles dead frames; active cancellation deferred unless we see a real problem |
| Q6 — Safety-net re-check on IPC worker | **No** | HTTP safety net solves sibling-bypass (which IPC doesn't have); TOCTOU window exists in both paths and is bounded |
| Q7 — Worker-pool throttle | **No** | Premature optimization without metrics; revisit if production shows saturation |
| Q8 — Logging macros | **File-owns-macro** | `LOG_DEBUG_HTTP` for code in `HttpRequestInterceptor.cpp`; `LOG_DEBUG_BROWSER` for code in `simple_handler.cpp` |

### Original open-question text (kept for archaeology)

### Q1 — Where does the existing wallet_call IPC handler's "internal origin" assumption come from?

Today's handler doesn't check internal-origin — it just routes everything to
Rust. The internal-frontend at `localhost:5137` calls wallet endpoints via
fetch, not via the shim's `__hodos_walletCall`, so this path is effectively
external-only today.

**Q:** Do you want the IPC path to bypass the engine for internal origins (as
§9 proposes), or should it always run the engine (even for internal origins)?

**Lean:** Bypass for internal-origin to match the HTTP path's L1886. The internal
frontend is trusted (by design — it's our wallet UI); subjecting it to the
gate would force domain_approval modals on every wallet UI op.

### Q2 — `OnWalletCallSuccess` consolidation: refactor BRC-121's `firePaymentSuccessIpc` too?

BRC-121's `firePaymentSuccessIpc` (L3876-3900) does roughly the same work
(indicator IPC + recordSpending + increment counters) but ALSO increments rate
counter + payment count (which createAction does in Open() at silent-approve
time, not at OnRequestComplete time).

**Q:** In Commit 6, do we:
- **(a)** Leave `firePaymentSuccessIpc` alone — `OnWalletCallSuccess` is only used by createAction's path and the new IPC path. BRC-121 stays separate.
- **(b)** Refactor both: `firePaymentSuccessIpc` becomes a thin wrapper that calls `OnWalletCallSuccess` + increments BRC-121-specific counters.

**Lean:** (a) for Commit 6 — minimum scope. (b) is a polish item for later.

### Q3 — Modal dispatch on unknown-trust IPC path: include manifest_connect_bundle?

§9 step 3 says unknown-trust on IPC fires `domain_approval`, mirroring Open()
L1923-1971. But that branch ALSO has the manifest-aware path that fires
`manifest_connect_bundle` if `.well-known/wallet-manifest.json` is found.

**Q:** Should the IPC path's unknown-trust handling include the manifest fetch
+ bundle dispatch, or just `domain_approval`?

**Lean:** Include the manifest fetch + bundle dispatch — same UX as HTTP path.
The fetch happens via `ManifestFetcher::Fetch` which is already a sync HTTP
call (handled fine on a worker thread or the UI thread depending on timeout).

### Q4 — IPC re-issue on Approve: re-extract cents at re-issue time, or use the cents captured at modal-open time?

The resume-after-approve worker callback in §5 re-extracts cents from the body
at re-issue time. This means if BSV price moves between modal-open and
modal-resolve (could be minutes for a slow user), the recorded cents reflects
the latest price.

**Q:** Use latest price at re-issue, or freeze the price at modal-open time?

**Lean:** Use latest price at re-issue. Matches the HTTP path's behavior —
`preCalculatedCents_` is set when the user approves, and `OnRequestComplete`
uses it. Equivalent freshness here.

### Q5 — Frame validity in long-pending modals

The acceptance criteria say "if frame navigates or tab closes between modal
opening and Approve, wallet_response is dropped silently with debug log."

**Q:** Do we ALSO want to actively cancel the pending request when the tab
closes (so the modal closes automatically), or let the timeout handle it?

**Lean:** Let the timeout handle it. Active cancellation requires a tab-close
listener that pops PendingRequestManager entries for that frame, which is
extra wiring not strictly required by the acceptance criteria.

### Q6 — Identity-key cache hit on IPC Silent path

When the engine returns Silent for identity-key (because
`identityKeyDisclosureAllowed=true` OR session cache hit), the HTTP path's
StartAsyncHTTPRequestTask injects `X-Identity-Key-Approved: true` (L3152-3157).
The HTTP path ALSO has a "safety net" at L3108-3126 that re-runs the privacy
check and triggers the modal if neither persistent nor cache grant exists
(handles drain-forwarded siblings that bypass Open()).

**Q:** Does the IPC path need a similar safety net at the worker-thread send,
or is the gate's Silent decision sufficient (no need for re-check)?

**Lean:** The gate's decision is sufficient. The HTTP safety net exists because
sibling requests can land in StartAsyncHTTPRequestTask without going through
Open()'s gate. The IPC path has no equivalent sibling-bypass; every call
runs the gate.

### Q7 — Worker-thread queue depth

If 10 IPC calls arrive in 100ms (e.g. a dApp doing 10 createSignatures), each
posts a task to `TID_FILE_USER_BLOCKING`. CEF's worker pool is shared with
other blocking work (DB writes, file I/O). Today's HTTP path doesn't have this
concern because each request gets its own CefURLRequest on TID_IO.

**Q:** Do we want to add a semaphore / queue cap to the IPC path to avoid
saturating the worker pool?

**Lean:** No, not for Commit 6. CEF's `TID_FILE_USER_BLOCKING` pool is sized
for concurrent blocking work. If we observe saturation in production, we can
add throttling later.

### Q8 — Logging discipline

The HTTP path uses `LOG_DEBUG_HTTP` macro. The IPC path's wallet_call handler
uses `LOG_DEBUG_BROWSER`. Different files, different macros.

**Q:** For the IPC path's new engine-cascade code, which logger should I use?

**Lean:** `LOG_DEBUG_HTTP` for the cascade itself (since the logic lives in
`HttpRequestInterceptor.cpp`), `LOG_DEBUG_BROWSER` for the IPC dispatch in
`simple_handler.cpp`. Whichever file owns the code owns the log macro.

## 12. Risks

| Risk | Severity | Mitigation |
|---|---|---|
| Header injection on wrong call (e.g. `X-Identity-Key-Approved` sent for a getNetwork call) | High | Set `sendIdentityKeyApproved` only if engine returned Silent AND callKind is `IdentityKeyReveal`. Same gate for key-linkage. Verified in the lambda capture. |
| Double-fire of payment_success_indicator | High | One success-callback discipline. OnWalletCallSuccess called at exactly one site per request path. Acceptance criterion #10. |
| Worker thread captures a stale frame ref | Medium | `capturedFrame->IsValid()` check before `SendProcessMessage` on every worker→UI hop. |
| Modal opens but timeout fires before user can interact | Low | 10-minute default timeout (`kPromptAuthTimeoutMs = 600000`) per recent bump. |
| Race: user approves while timeout fires concurrently | Low | `popRequest` is atomic — only one of (approve, timeout) wins. The other path no-ops gracefully. |
| Decision 3 refactor breaks 5.b-5.f openModal lambdas in subtle ways | High | 6.c is the structural refactor with no behavior change. Build + test + visual modal check on HTTP path BEFORE moving to 6.d's IPC wiring. |
| handleAuthResponse sibling drain on mixed-kind requests | Medium | Switch on each sibling's resumeKind individually. Unit test the drain mechanism after 6.e. |
| Engine-cascade fires for an endpoint that shouldn't be gated | Low | `isWalletEndpoint` filter still applies upstream. The IPC handler also gates by endpoint to skip wallet-internal endpoints. |
| CSP/CORS still bites on internal frontend | None | Internal frontend uses fetch + same-origin to localhost:5137; not affected by IPC bridge. |
| Phase 2.6 breaks something in Commit 6 | None | Commit 6 design checked against 2.5-A Decisions table; forward-compatible per `seams not implementations`. |

## 13. Acceptance criteria sweep — when is Commit 6 done?

Per `PHASE_2_5_IPC_REFACTOR.md` "Commit 6" criteria:

| # | Criterion | How verified |
|---|---|---|
| 1 | wallet_call builds a PermissionContext before RunPermissionGate | Code review of 6.d |
| 2 | PermissionContext carries X-Requesting-Domain | Code review of 6.d |
| 3 | Silent decision: forwards + OnWalletCallSuccess + recordSpending + indicator + wallet_response | Cumulative smoke at 6.e (green-dot visible on tab) |
| 4 | Prompt decision: modal opens via OpenPromptModal + PendingAuthRequest enrolled + Approve resumes with headers + Deny errors cleanly | Cumulative smoke at 6.e (all 6 modal types) |
| 5 | Deny decision: wallet_response with error, no wallet call | Cumulative smoke at 6.e (use blocked domain) |
| 6 | CSP-bypass + CORS-bypass verified on github.com / treechat.io | Manual smoke after 6.f |
| 7 | HTTP path unchanged (localhost:5137 wallet UI works) | Manual smoke after 6.f |
| 8 | Per-domain queue dedup still works | Test with 2x in-flight IPC calls from same domain after 6.e |
| 9 | Frame validity check: dead frames drop silently | Visible in debug log when tab closes mid-modal |
| 10 | No double-fire of green-dot animation | grep + code review + manual test |

## 14. Out of scope for Commit 6

- Phase 2.6 (engine-to-Rust). Decisions D1-D4 are forward-compatible; Phase 2.6 work happens after Phase 2.5 closes.
- BRC-121 paid retry inline cap-cascade migration to engine (Phase 1.5 polish item G).
- Sensitive cert field classifier (`project_sensitive_cert_field_classifier_gap`).
- `firePaymentSuccessIpc` consolidation with `OnWalletCallSuccess` — Q2.
- Active modal cancellation on tab close — Q5.
- Worker-pool throttling — Q7.

## 15. Related docs / memories

- [`PHASE_2_5_IPC_REFACTOR.md`](./PHASE_2_5_IPC_REFACTOR.md) — acceptance criteria + sub-phase status
- [`../../architecture/AUTO_APPROVE_ENGINE.md`](../../architecture/AUTO_APPROVE_ENGINE.md) — engine state-of-world
- [`../../architecture/WALLET_API_MAP.md`](../../architecture/WALLET_API_MAP.md) — endpoint × gate matrix
- Memory `phase25-session-handoff-2026-06-01` — current sprint state
- Memory `feedback_research_first_do_it_once` — why this doc exists
- Memory `feedback_batch_implement_then_smoke` — smoke deferred to 6.e cumulative
- Memory `payment_animation_safeguard` — green-dot invariant (criterion #10)
- Memory `domain_permission_cache_invalidation` — cache invalidation IPC stays intact
