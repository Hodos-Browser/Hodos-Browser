# HTTP Interceptor Flow Guide

## Purpose

This document outlines how the HTTP interceptor system works, including domain whitelisting checks, BRC-100 authentication flows, and user approval modals. This serves as a reference implementation for adding similar interceptor-based flows that require database checks and frontend user notifications/prompts.

**Document Version:** 1.0
**Last Updated:** 2026-01-27
**Target Audience:** Developers implementing new HTTP interceptor-based flows

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture Components](#architecture-components)
3. [Domain Whitelist Check Flow (Working Example)](#domain-whitelist-check-flow-working-example)
4. [BRC-100 Authentication Flow](#brc-100-authentication-flow)
5. [Wallet Existence Check (Commented Out - Reference)](#wallet-existence-check-commented-out---reference)
6. [Implementation Patterns](#implementation-patterns)
7. [Adding New Interceptor Flows](#adding-new-interceptor-flows)

---

## Overview

The HTTP interceptor system allows the browser to:

1. **Intercept wallet API requests** from external websites
2. **Check security/permission state** (database checks, whitelists, etc.)
3. **Prompt users** via modal/overlay windows when action is needed
4. **Continue or block requests** based on user approval

### Key Concepts

- **HTTP Interception**: CEF's `OnBeforeResourceLoad()` intercepts all HTTP requests
- **Database Checks**: Rust wallet backend maintains whitelists and state in database
- **Async Flow**: Request is paused, user is prompted, request continues/terminates based on response
- **Process Isolation**: Overlays run in separate CEF processes for security

---

## Architecture Components

### 1. HTTP Interceptor (`cef-native/src/core/HttpRequestInterceptor.cpp`)

**Primary Classes:**
- `HttpRequestInterceptor`: Main interceptor that checks `isWalletEndpoint()`
- `AsyncWalletResourceHandler`: Handles individual wallet API requests
- `DomainVerifier`: Checks domain whitelist status in JSON file

**Key Methods:**
- `GetResourceHandler()`: Called for each intercepted request
- `isWalletEndpoint()`: Determines if URL should be intercepted
- `AsyncWalletResourceHandler::Open()`: Entry point for request processing

### 2. Frontend Components

**Pages:**
- `SettingsOverlayRoot.tsx`: Settings overlay that shows auth modals
- `BRC100AuthOverlayRoot.tsx`: Dedicated BRC-100 auth overlay (alternative)

**Components:**
- `BRC100AuthModal.tsx`: Reusable authentication approval modal

**Global State:**
- `window.pendingBRC100AuthRequest`: Temporary storage for pending auth requests

### 3. Message Passing

**C++ → Frontend:**
- `ExecuteJavaScript()`: Injects `window.pendingBRC100AuthRequest` data
- `window.hodosBrowser.overlay.show()`: Triggers overlay window creation

**Frontend → C++:**
- `window.cefMessage.send('brc100_auth_response', ...)`: Sends user approval/rejection
- `window.cefMessage.send('add_domain_to_whitelist', ...)`: Adds domain to whitelist

**C++ Message Handler:**
- `SimpleHandler::OnProcessMessageReceived()`: Processes messages in `simple_handler.cpp`

### 4. Rust Wallet Backend

**Endpoints:**
- `GET /domain/whitelist/check?domain={domain}`: Check if domain is whitelisted
- `POST /domain/whitelist/add`: Add domain to whitelist (database + JSON file)

**Database:**
- `domain_whitelist` table: Stores whitelisted domains
- `domainWhitelist.json`: JSON file for C++ interceptor (syncs with database)

---

## Domain Whitelist Check Flow (Working Example)

This is the **fully working reference implementation** for interceptor-based flows.

### Flow Diagram

```
External Website Request
    ↓
HTTP Interceptor (OnBeforeResourceLoad)
    ↓
isWalletEndpoint() = true?
    ↓ YES
Create AsyncWalletResourceHandler
    ↓
AsyncWalletResourceHandler::Open()
    ↓
DomainVerifier::isDomainWhitelisted(domain)
    ↓
┌─────────────────────────────────────┐
│ Domain Whitelisted?                 │
└─────────────────────────────────────┘
    │                      │
   YES                    NO
    │                      │
    ↓                      ↓
Continue Request    triggerDomainApprovalModal()
    │                      │
    │                      ├──> Store pending request in g_pendingAuthRequest
    │                      ├──> ExecuteJavaScript() to set window.pendingBRC100AuthRequest
    │                      ├──> Call window.hodosBrowser.overlay.show()
    │                      └──> Pause request (handle_request = true, return true)
    │
    ├──────────────────────────────────────────────────┐
    │                                                  │
    │ Forward to Rust Wallet (localhost:3301)         │
    │                                                  │
    └──────────────────────────────────────────────────┘
                            │
                            │
                    ┌───────┴───────┐
                    │               │
                Approve         Reject
                    │               │
                    ↓               ↓
            Add to whitelist    Block request
                    │               │
                    ↓               ↓
            Continue request    Send error response
```

### Step-by-Step Flow

#### Step 1: Request Interception

**File**: `cef-native/src/core/HttpRequestInterceptor.cpp`
**Location**: `HttpRequestInterceptor::GetResourceHandler()`

```cpp
// Check if this is a wallet endpoint
if (!isWalletEndpoint(url)) {
    return nullptr; // Let CEF handle it normally
}

// Create handler for wallet request
return new AsyncWalletResourceHandler(method, endpoint, body, domain, browser, headers);
```

#### Step 2: Domain Whitelist Check

**File**: `cef-native/src/core/HttpRequestInterceptor.cpp`
**Location**: `AsyncWalletResourceHandler::Open()`

```cpp
// Check if domain is whitelisted - NO BYPASSES
DomainVerifier domainVerifier;
if (!domainVerifier.isDomainWhitelisted(requestDomain_)) {
    // Domain not whitelisted, trigger approval modal
    triggerDomainApprovalModal(requestDomain_, method_, endpoint_);

    // Pause request - wait for user response
    handle_request = true;
    return true; // Don't continue request yet
}

// Domain is whitelisted, proceed with request
domainVerifier.recordRequest(requestDomain_);
startAsyncHTTPRequest(); // Continue with request
```

#### Step 3: Domain Verification

**File**: `cef-native/src/core/HttpRequestInterceptor.cpp`
**Location**: `DomainVerifier::isDomainWhitelisted()`

```cpp
bool isDomainWhitelisted(const std::string& domain) {
    // Read whitelist file: %APPDATA%/HodosBrowser/wallet/domainWhitelist.json
    std::ifstream file(whitelistFilePath);
    nlohmann::json whitelist;
    file >> whitelist;

    // Check if domain exists in whitelist
    for (const auto& entry : whitelist) {
        if (entry["domain"] == domain) {
            return true; // Domain is whitelisted
        }
    }

    return false; // Domain not whitelisted
}
```

#### Step 4: Trigger Approval Modal

**File**: `cef-native/src/core/HttpRequestInterceptor.cpp`
**Location**: `AsyncWalletResourceHandler::triggerDomainApprovalModal()`

```cpp
void triggerDomainApprovalModal(const std::string& domain, const std::string& method, const std::string& endpoint) {
    // Prevent duplicate modals
    if (g_pendingModalDomain == domain) {
        return; // Already showing modal for this domain
    }

    // Store pending request globally
    g_pendingAuthRequest.domain = domain;
    g_pendingAuthRequest.method = method;
    g_pendingAuthRequest.endpoint = endpoint;
    g_pendingAuthRequest.body = "";
    g_pendingAuthRequest.isValid = true;
    g_pendingModalDomain = domain;

    // Send JavaScript to frontend to trigger overlay
    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    std::string js = R"(
        window.pendingBRC100AuthRequest = {
            domain: ')" + domain + R"(',
            method: ')" + method + R"(',
            endpoint: ')" + endpoint + R"(',
            body: '',
            type: 'domain_approval'
        };
        window.hodosBrowser.overlay.show(); // Opens settings overlay
    )";
    header_browser->GetMainFrame()->ExecuteJavaScript(js, "", 0);
}
```

#### Step 5: Frontend Receives Request

**File**: `frontend/src/pages/SettingsOverlayRoot.tsx`
**Location**: `useEffect()` hook

```typescript
useEffect(() => {
    // Check for pending BRC-100 auth request (set by C++ via ExecuteJavaScript)
    const pendingAuthRequest = (window as any).pendingBRC100AuthRequest;
    if (pendingAuthRequest) {
        setAuthRequest({
            domain: pendingAuthRequest.domain,
            appId: pendingAuthRequest.domain,
            purpose: 'Authentication Request',
            challenge: pendingAuthRequest.body || '',
            sessionDuration: 30,
            permissions: ['Access identity certificate']
        });
        setAuthModalOpen(true);
        // Clear the pending request
        (window as any).pendingBRC100AuthRequest = null;
    }
}, []);
```

#### Step 6: User Interacts with Modal

**File**: `frontend/src/components/BRC100AuthModal.tsx`

User sees modal with:
- Domain name
- Request details
- "Whitelist this site" checkbox
- Approve/Reject buttons

#### Step 7: Frontend Sends Response

**File**: `frontend/src/pages/SettingsOverlayRoot.tsx` (or `BRC100AuthOverlayRoot.tsx`)
**Location**: `handleAuthApprove()` / `handleAuthReject()`

```typescript
const handleAuthApprove = async (whitelist: boolean) => {
    // If user checked "whitelist this site", add domain to whitelist
    if (whitelist && authRequest) {
        window.cefMessage?.send('add_domain_to_whitelist', [
            JSON.stringify({
                domain: authRequest.domain,
                permanent: true
            })
        ]);
    }

    // Send approval response to HTTP interceptor
    window.cefMessage?.send('brc100_auth_response', [
        JSON.stringify({
            approved: true,
            whitelist: whitelist
        })
    ]);

    // Close overlay window
    window.hodosBrowser.overlay.close();
};
```

#### Step 8: C++ Processes Response - Add to Whitelist

**File**: `cef-native/src/handlers/simple_handler.cpp`
**Location**: `OnProcessMessageReceived()` - `add_domain_to_whitelist` handler

```cpp
if (message_name == "add_domain_to_whitelist") {
    // Parse JSON: { domain: "...", permanent: true/false }
    nlohmann::json whitelistData = nlohmann::json::parse(whitelistJson);
    std::string domain = whitelistData["domain"];
    bool permanent = whitelistData["permanent"];

    // Call HTTP interceptor function to add domain
    extern void addDomainToWhitelist(const std::string& domain, bool permanent);
    addDomainToWhitelist(domain, permanent);
}
```

**File**: `cef-native/src/core/HttpRequestInterceptor.cpp`
**Location**: `addDomainToWhitelist()`

```cpp
void addDomainToWhitelist(const std::string& domain, bool permanent) {
    // Post task to UI thread (CefURLRequest must be on UI thread)
    CefPostTask(TID_UI, new DomainWhitelistTask(domain, permanent));
}

// DomainWhitelistTask::Execute() creates HTTP request to Rust wallet
// POST http://localhost:3301/domain/whitelist/add
// Body: { "domain": "...", "isPermanent": true/false }
```

#### Step 9: C++ Processes Response - Continue Request

**File**: `cef-native/src/handlers/simple_handler.cpp`
**Location**: `OnProcessMessageReceived()` - `brc100_auth_response` handler

```cpp
if (message_name == "brc100_auth_response") {
    // Parse JSON: { approved: true/false, whitelist: true/false }
    nlohmann::json responseData = nlohmann::json::parse(responseJson);
    bool approved = responseData["approved"];
    bool whitelist = responseData["whitelist"];

    if (approved) {
        // User approved - generate authentication response from wallet
        // Make HTTP request to wallet backend to process request
        CefRefPtr<CefRequest> cefRequest = CefRequest::Create();
        cefRequest->SetURL("http://localhost:3301" + g_pendingAuthRequest.endpoint);
        // ... set method, body, headers ...

        // When response received, call handleAuthResponse()
        handleAuthResponse(responseData_);
    } else {
        // User rejected - clear pending request
        g_pendingAuthRequest.isValid = false;
    }
}
```

#### Step 10: Continue Original Request

**File**: `cef-native/src/core/HttpRequestInterceptor.cpp`
**Location**: `handleAuthResponse()`

```cpp
void handleAuthResponse(const std::string& responseData) {
    // Clear pending modal domain
    g_pendingModalDomain = "";

    if (g_pendingAuthRequest.isValid && g_pendingAuthRequest.handler) {
        // Send response back to original HTTP request
        AsyncWalletResourceHandler* walletHandler =
            static_cast<AsyncWalletResourceHandler*>(g_pendingAuthRequest.handler.get());
        walletHandler->onAuthResponseReceived(responseData);

        // Clear pending request
        g_pendingAuthRequest.isValid = false;
    }
}
```

**File**: `cef-native/src/core/HttpRequestInterceptor.cpp`
**Location**: `AsyncWalletResourceHandler::onAuthResponseReceived()`

```cpp
void onAuthResponseReceived(const std::string& data) {
    responseData_ = data;
    requestCompleted_ = true;

    // Resume request - callback will read response data
    if (readCallback_) {
        readCallback_->Continue();
    }
}
```

---

## BRC-100 Authentication Flow

Similar to domain whitelist check, but for BRC-100 authentication requests.

### Differences from Domain Whitelist Flow

1. **Endpoint Check**: Checks if `endpoint_.find("/brc100/auth/") != std::string::npos`
2. **Trigger Function**: `triggerBRC100AuthApprovalModal()` instead of `triggerDomainApprovalModal()`
3. **Request Body**: BRC-100 requests include authentication challenge in body
4. **Handler Storage**: Stores `handler` pointer in `g_pendingAuthRequest.handler`

### Flow Differences

```cpp
// In AsyncWalletResourceHandler::Open()
if (!domainVerifier.isDomainWhitelisted(requestDomain_)) {
    if (endpoint_.find("/brc100/auth/") != std::string::npos) {
        // BRC-100 auth request
        triggerBRC100AuthApprovalModal(requestDomain_, method_, endpoint_, body_, this);
        // 'this' is the handler pointer, stored for later continuation
    } else {
        // Regular domain approval
        triggerDomainApprovalModal(requestDomain_, method_, endpoint_);
    }
}
```

**Key Point**: BRC-100 requests store the `handler` pointer so the request can be resumed after user approval. The handler's `onAuthResponseReceived()` method is called to continue the request.

---

## Wallet Existence Check (Commented Out - Reference)

**⚠️ IMPLEMENTATION STATUS**:

**Current State**:
- ✅ This wallet check code is **commented out** (as intended)
- ❌ The new wallet check on Wallet button click is **NOT YET IMPLEMENTED**
- ❌ WalletSetupModal component does **NOT YET EXIST**

**Planned Implementation**: The wallet initialization will move wallet checks to user-initiated actions (Wallet button click) rather than automatic startup checks.

**Note**: For the planned wallet initialization flow, see [Startup Flow and Wallet Checks](./phase-0-startup-flow-and-wallet-checks.md) and [Wallet Initialization Flow](./helper-1-implementation-guide-checklist.md#wallet-initialization-flow).

This code was previously used to check wallet status on startup and prompt users to create/backup their wallet. It's currently **commented out** but serves as a reference for similar startup checks.

### Location

**File**: `frontend/src/App.tsx`
**Lines**: 42-110 (within large comment block)

### Code Structure

```typescript
// COMMENTED OUT: Wallet status check and wallet creation/backup prompt on startup
// This was previously used to create wallet.json and prompt users to save mnemonic
// We have changed how wallet.json is created, so this check is disabled for now
// TODO: Re-implement wallet initialization check in the future with new wallet creation flow
/*
const checkWalletStatus = async () => {
  console.log("🔍 checkWalletStatus started");

  // Wait for all systems to be ready (for overlay browsers)
  if (window.location.pathname !== '/') {
    await new Promise<void>((resolve) => {
      if (window.allSystemsReady) {
        console.log("🔍 All systems already ready");
        resolve();
      } else {
        console.log("🔍 Waiting for allSystemsReady event...");
        window.addEventListener('allSystemsReady', () => {
          console.log("🔍 allSystemsReady event received");
          resolve();
        }, { once: true });
      }
    });
  }

  // Wait for cefMessage to be ready
  for (let i = 0; i < 40; i++) {
    if (window.cefMessage && typeof window.cefMessage.send === 'function') {
      console.log("🔍 Backend ready after", i, "attempts");
      break;
    }
    await new Promise((r) => setTimeout(r, 50));
  }

  console.log("🔍 Backend check complete, cefMessage exists:", typeof window.cefMessage?.send);
  console.log("🔍 Current pathname:", window.location.pathname);

  // Only check on main page
  if (window.location.pathname === '/' && window.hodosBrowser?.wallet) {
    console.log("🔍 Running wallet status check via API");

    try {
      const walletStatus = await window.hodosBrowser.wallet.getStatus();
      console.log("🔍 Wallet status response:", walletStatus);

      if (walletStatus.needsBackup) {
        // Wallet needs backup - create wallet first, then show modal
        console.log("🔍 Wallet needs backup, creating wallet first...");
        try {
          await window.hodosBrowser.wallet.create();
          console.log("🔍 Wallet created successfully, showing backup modal");
          window.cefMessage?.send('overlay_show_backup', []);
        } catch (error) {
          console.error("💥 Error creating wallet:", error);
        }
      } else {
        // Wallet is backed up - do nothing
        console.log("🔍 Wallet is backed up, no action needed");
      }
    } catch (error) {
      console.error("💥 Error checking wallet status:", error);
    }

  } else {
    console.log("🔍 Skipping wallet check - path:", window.location.pathname, "wallet API ready:", !!window.hodosBrowser?.wallet);
  }
};

checkWalletStatus();
*/
```

### Pattern Analysis

This code demonstrates a **startup check pattern**:

1. **Wait for Systems**: Waits for `allSystemsReady` event and `cefMessage` availability
2. **Check Condition**: Calls `window.hodosBrowser.wallet.getStatus()`
3. **Take Action**: If `needsBackup`, creates wallet and shows backup overlay
4. **Route-Specific**: Only runs on main page (`pathname === '/'`)

### When to Use This Pattern

- **Startup checks**: Wallet existence, database initialization, first-run setup
- **Proactive prompts**: Backup reminders, security warnings, update notifications
- **Frontend-initiated**: Not triggered by HTTP requests, but by app lifecycle

**⚠️ IMPLEMENTATION STATUS**:

**Current State**:
- ❌ Wallet checks on Wallet button click are **NOT YET IMPLEMENTED**
- ✅ Frontend startup check is commented out (as intended)

**Planned Implementation**: The wallet initialization flow will NOT use this startup pattern. Instead, wallet checks will happen when the user clicks the Wallet button. See [Startup Flow and Wallet Checks](./phase-0-startup-flow-and-wallet-checks.md) for the planned implementation.

### Differences from HTTP Interceptor Flow

| Aspect | Wallet Check (Frontend) | HTTP Interceptor (Backend) |
|--------|------------------------|---------------------------|
| **Trigger** | App startup / component mount | HTTP request interception |
| **Location** | Frontend (`App.tsx`) | C++ (`HttpRequestInterceptor.cpp`) |
| **Timing** | Proactive (on load) | Reactive (on request) |
| **User Action** | Can be delayed/deferred | Blocks request (synchronous) |
| **Database Check** | Via `wallet.getStatus()` API | Direct database/file check |

---

## Implementation Patterns

### Pattern 1: HTTP Interceptor with Database Check

**Use Case**: Check permission/state before allowing request to proceed

**Steps:**
1. Intercept request in `GetResourceHandler()`
2. Check database/whitelist in `AsyncWalletResourceHandler::Open()`
3. If check fails, trigger modal via `trigger*ApprovalModal()`
4. Store request handler for later continuation
5. Frontend shows modal, user approves/rejects
6. Frontend sends response via `cefMessage.send()`
7. C++ receives response, continues/blocks request

**Files:**
- `cef-native/src/core/HttpRequestInterceptor.cpp`: Interception logic
- `frontend/src/pages/*OverlayRoot.tsx`: Modal display
- `frontend/src/components/*Modal.tsx`: Modal UI

### Pattern 2: Frontend Startup Check

**Use Case**: Proactive check on app load (wallet status, first-run, etc.)

**Steps:**
1. Check condition in `useEffect()` on component mount
2. Call API via `window.hodosBrowser.*`
3. If condition requires action, trigger overlay/modal
4. User interacts, system updates state

**Files:**
- `frontend/src/App.tsx`: Startup checks
- `frontend/src/pages/*OverlayRoot.tsx`: Overlay display

### Pattern 3: Database Check in Rust Wallet

**Use Case**: Permission/state stored in database, checked by backend

**Files:**
- `rust-wallet/src/domain_whitelist.rs`: Database operations
- `rust-wallet/src/handlers.rs`: API endpoints

**Example:**
```rust
// In handlers.rs
let is_whitelisted = state.whitelist.is_domain_whitelisted(domain);
if !is_whitelisted {
    // Return error or trigger approval flow
}
```

---

## Adding New Interceptor Flows

### Template: New Permission Check Flow

#### Step 1: Add Check in HTTP Interceptor

**File**: `cef-native/src/core/HttpRequestInterceptor.cpp`
**Location**: `AsyncWalletResourceHandler::Open()`

```cpp
// After domain whitelist check, add your check
if (!someOtherCheck(requestDomain_)) {
    triggerYourApprovalModal(requestDomain_, method_, endpoint_);
    handle_request = true;
    return true; // Pause request
}
```

#### Step 2: Implement Check Function

**File**: `cef-native/src/core/HttpRequestInterceptor.cpp`

```cpp
class YourChecker {
public:
    bool checkSomething(const std::string& domain) {
        // Check database, file, or state
        // Return true if permission granted
    }
};
```

#### Step 3: Implement Trigger Function

**File**: `cef-native/src/core/HttpRequestInterceptor.cpp`

```cpp
void triggerYourApprovalModal(const std::string& domain, const std::string& method, const std::string& endpoint) {
    // Store pending request
    g_pendingYourRequest.domain = domain;
    g_pendingYourRequest.isValid = true;
    g_pendingModalDomain = domain;

    // Send JavaScript to frontend
    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    std::string js = R"(
        window.pendingYourRequest = {
            domain: ')" + domain + R"(',
            method: ')" + method + R"(',
            endpoint: ')" + endpoint + R"('
        };
        window.hodosBrowser.overlay.show();
    )";
    header_browser->GetMainFrame()->ExecuteJavaScript(js, "", 0);
}
```

#### Step 4: Frontend Receives Request

**File**: `frontend/src/pages/SettingsOverlayRoot.tsx` (or new overlay)

```typescript
useEffect(() => {
    const pendingRequest = (window as any).pendingYourRequest;
    if (pendingRequest) {
        setYourModalOpen(true);
        setYourRequestData(pendingRequest);
        (window as any).pendingYourRequest = null;
    }
}, []);
```

#### Step 5: User Response Handler

**File**: `frontend/src/pages/SettingsOverlayRoot.tsx`

```typescript
const handleYourApprove = () => {
    window.cefMessage?.send('your_response', [
        JSON.stringify({ approved: true })
    ]);
    window.hodosBrowser.overlay.close();
};
```

#### Step 6: C++ Message Handler

**File**: `cef-native/src/handlers/simple_handler.cpp`
**Location**: `OnProcessMessageReceived()`

```cpp
if (message_name == "your_response") {
    // Parse JSON response
    nlohmann::json responseData = nlohmann::json::parse(responseJson);
    bool approved = responseData["approved"];

    if (approved) {
        // Continue original request
        handleYourResponse(responseData);
    }
}
```

### Checklist for New Flows

- [ ] Add check function in `HttpRequestInterceptor.cpp`
- [ ] Add trigger function for modal
- [ ] Store pending request in global variable (or extend `g_pendingAuthRequest`)
- [ ] Frontend reads `window.pending*Request` and shows modal
- [ ] Frontend sends response via `cefMessage.send()`
- [ ] C++ message handler processes response
- [ ] Continue or block original request based on response
- [ ] Update `isWalletEndpoint()` if new endpoint type needed
- [ ] Add database/state management if needed in Rust wallet

---

## Key Global Variables

### C++ Side (`HttpRequestInterceptor.cpp`)

```cpp
// Pending authentication/approval request
PendingAuthRequest g_pendingAuthRequest = {
    "",      // domain
    "",      // method
    "",      // endpoint
    "",      // body
    false,   // isValid
    nullptr  // handler (for BRC-100 auth requests)
};

// Track which domain has pending modal (prevents duplicates)
std::string g_pendingModalDomain = "";
```

### Frontend Side

```typescript
// Temporary storage for pending requests (set by C++ via ExecuteJavaScript)
window.pendingBRC100AuthRequest = {
    domain: "...",
    method: "...",
    endpoint: "...",
    body: "..."
};
```

---

## Database Integration Points

### Domain Whitelist Database

**Rust Backend** (`rust-wallet/src/domain_whitelist.rs`):
- `DomainWhitelistManager`: Manages whitelist in database
- `is_domain_whitelisted()`: Check if domain is whitelisted
- `add_to_whitelist()`: Add domain to whitelist

**Database Table**:
```sql
CREATE TABLE domain_whitelist (
    domain TEXT PRIMARY KEY,
    added_at INTEGER,
    last_used INTEGER,
    request_count INTEGER,
    is_permanent INTEGER
);
```

**JSON File** (`%APPDATA%/HodosBrowser/wallet/domainWhitelist.json`):
- Synced with database
- Used by C++ interceptor for fast lookup
- Format:
```json
[
  {
    "domain": "example.com",
    "addedAt": 1234567890,
    "lastUsed": 1234567890,
    "requestCount": 5,
    "isPermanent": true
  }
]
```

### Adding Database Checks for New Flows

1. **Create Database Table** (in `rust-wallet/src/database/migrations.rs`)
2. **Create Manager Class** (in `rust-wallet/src/`)
3. **Add API Endpoint** (in `rust-wallet/src/handlers.rs`)
4. **Sync to JSON File** (if C++ needs fast access)
5. **Call from C++** (via HTTP request to `localhost:3301`)

---

## Summary

The HTTP interceptor flow provides a secure, user-friendly way to:

1. **Intercept requests** before they reach the backend
2. **Check permissions** via database/whitelist
3. **Prompt users** via modal overlays
4. **Continue or block** requests based on user approval

**Key Takeaways:**
- Use `AsyncWalletResourceHandler::Open()` as entry point
- Store pending requests globally for later continuation
- Use `ExecuteJavaScript()` to pass data to frontend
- Use `cefMessage.send()` to send responses back to C++
- Keep database state in sync between Rust backend and C++ interceptor

**Reference Implementations:**
- **Domain Whitelist**: Fully working - use as primary template
- **BRC-100 Auth**: Similar pattern with handler storage
- **Wallet Check**: Frontend pattern for startup checks

When implementing new flows, follow the domain whitelist pattern and adapt as needed for your specific requirements.

---

**End of Document**
