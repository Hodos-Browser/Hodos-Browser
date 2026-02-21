# Security & Process Isolation Analysis

**Date**: October 9, 2025 (Updated: February 19, 2026)
**Focus**: Current security model and process isolation architecture

## 🔐 Current Process Architecture

### Process Map

Your browser currently runs **9 distinct processes**:

```
┌──────────────────────────────────────────────────────────────┐
│  PROCESS 1: Main Browser Process (cef_browser_shell.cpp)    │
│  - Window management (WM_SIZE, WM_MOVE, WM_CLOSE)           │
│  - HWND creation and coordination                            │
│  - Logger initialization                                     │
│  - Graceful shutdown orchestration                           │
│  - NO web content rendering                                  │
│  - NO JavaScript execution                                   │
└──────────────────────────────────────────────────────────────┘
                              │
                              ├─────────────────────────────────┐
                              ▼                                 ▼
┌────────────────────────────────────┐    ┌─────────────────────────────────┐
│ PROCESS 2: Header Browser          │    │ PROCESS 3: Webview Browser      │
│ Role: "header"                     │    │ Role: "webview"                 │
│ URL: http://127.0.0.1:5137         │    │ URL: https://metanetapps.com/   │
│                                    │    │                                 │
│ - React UI rendering               │    │ - External website rendering    │
│ - Navigation controls              │    │ - Web content from internet     │
│ - Wallet/Settings buttons          │    │ - HTTP interception active      │
│ - Own V8 context                   │    │ - Domain whitelisting applied   │
│ - hodosBrowser API injected      │    │ - hodosBrowser API injected   │
│ - WS_CHILD window                  │    │ - WS_CHILD window               │
└────────────────────────────────────┘    └─────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┬───────────────────┐
        ▼                     ▼                     ▼                   ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐   ┌────────────────┐
│ PROCESS 4:    │    │ PROCESS 5:    │    │ PROCESS 6:    │   │ PROCESS 7:     │
│ Settings      │    │ Wallet        │    │ Backup        │   │ BRC100 Auth    │
│ Overlay       │    │ Overlay       │    │ Modal         │   │ Modal          │
│               │    │               │    │               │   │                │
│ Role:         │    │ Role:         │    │ Role:         │   │ Role:          │
│ "settings"    │    │ "wallet"      │    │ "backup"      │   │ "brc100auth"   │
│               │    │               │    │               │   │                │
│ Own V8        │    │ Own V8        │    │ Own V8        │   │ Own V8         │
│ WS_POPUP      │    │ WS_POPUP      │    │ WS_POPUP      │   │ WS_POPUP       │
│ Layered       │    │ Layered       │    │ Layered       │   │ Layered        │
└───────────────┘    └───────────────┘    └───────────────┘   └────────────────┘
                                                                      │
                                                            ┌────────────────┐
                                                            │ PROCESS 8:     │
                                                            │ Notification   │
                                                            │ Overlay        │
                                                            │                │
                                                            │ Role:          │
                                                            │ "notification" │
                                                            │                │
                                                            │ Own V8         │
                                                            │ Keep-alive     │
                                                            │ HWND (reused)  │
                                                            └────────────────┘

┌────────────────────────────────────────────────────────────┐
│ PROCESS 9: Rust Wallet Backend (Separate Process)          │
│ - HD wallet management                                     │
│ - Transaction creation/signing/broadcasting                │
│ - UTXO management                                          │
│ - BRC100 authentication                                    │
│ - Domain permission enforcement                            │
│ - HTTP API server (localhost:3301)                          │
│ - DPAPI-encrypted mnemonic storage                         │
│ - Runs independently, started by C++                       │
└────────────────────────────────────────────────────────────┘
```

### Process Isolation Benefits

**Header Browser (React UI):**
- ✅ Isolated from web content
- ✅ Cannot be compromised by malicious websites
- ✅ Always trustworthy UI
- ✅ Controls navigation/wallet access

**Webview Browser (Web Content):**
- ✅ Isolated from UI controls
- ✅ Can't modify navigation bar
- ✅ Can't intercept wallet button clicks
- ✅ Limited to HTTP interceptor communication

**Overlay Browsers:**
- ✅ Each overlay isolated from others
- ✅ Settings can't access wallet state
- ✅ Fresh V8 context prevents state pollution
- ✅ Independent lifecycle (can close without affecting others)

**Rust Wallet:**
- ✅ Separate process = can't be directly memory-exploited from web
- ✅ Only accessible via HTTP localhost API
- ✅ Domain whitelisting enforced
- ✅ Private keys never exposed to browser processes

## 🛡️ Security Boundaries

### Boundary 1: UI ↔ Web Content

**Separation:**
- Header browser: Trusted React UI
- Webview browser: Untrusted web content

**Communication:**
- ❌ NO direct JavaScript access between them
- ✅ Communication only via CEF process messages
- ✅ Main process mediates all communication

**Security:**
- ✅ Malicious website can't modify UI
- ✅ Malicious website can't intercept wallet button
- ✅ Phishing protection (can't fake wallet modal)

### Boundary 2: Browser ↔ Wallet Daemon

**Separation:**
- CEF browsers: JavaScript execution environment
- Rust wallet backend: Wallet operations

**Communication:**
- ✅ Only HTTP requests to localhost:3301
- ✅ All requests intercepted by HttpRequestInterceptor
- ✅ Domain whitelisting enforced before allowing request
- ✅ User approval required for sensitive operations

**Security:**
- ✅ Private keys never in browser process memory
- ✅ Transaction signing in separate process
- ✅ Domain-based access control
- ✅ HTTP-only communication (no shared memory exploits)

### Boundary 3: Tab ↔ Tab (With Process-Per-Tab)

**Separation:**
- Each tab: Own render process
- Each tab: Own V8 context

**Communication:**
- ❌ Tabs CANNOT communicate directly
- ✅ Can only communicate via main process
- ✅ No shared memory between tabs

**Security:**
- ✅ Tab 1 can't read Tab 2's cookies/localStorage
- ✅ Tab 1 can't intercept Tab 2's HTTP requests
- ✅ Tab 1 can't steal Tab 2's BRC100 session
- ✅ Complete isolation between websites

## 🔒 Current Security Strengths

### 1. Process Isolation ✅

**What You Have:**
- ✅ UI in separate process from web content
- ✅ Each overlay in own process
- ✅ Wallet operations in Rust wallet
- ✅ No shared memory between security boundaries

**Attack Surface:**
- ❌ Malicious website can't access wallet directly
- ❌ Malicious website can't modify UI
- ❌ Compromised tab can't affect other tabs (once you add tabs)

### 2. HTTP Request Interception ✅

**What You Have:**
- ✅ All HTTP requests go through CEF interceptor
- ✅ Domain whitelisting before processing
- ✅ User approval for new domains
- ✅ Wallet endpoints only accessible from approved domains

**Code Location:**
```cpp
// cef-native/src/core/HttpRequestInterceptor.cpp
bool HttpRequestInterceptor::isWalletEndpoint(const std::string& url) {
    return (url.find("/socket.io/") != std::string::npos ||
            url.find("/.well-known/auth") != std::string::npos ||
            // ... other wallet endpoints
    );
}
```

### 3. API Injection Control ✅

**What You Have:**
- ✅ `hodosBrowser` API only injected into specific browsers
- ✅ Each browser gets fresh injection
- ✅ No global shared API object
- ✅ API scoped to browser's V8 context

**Code Location:**
```cpp
// cef-native/src/handlers/simple_app.cpp - InjectBitcoinBrowserAPI()
// Called independently for each browser in OnLoadingStateChange
```

### 4. Domain Permission System ✅

**What You Have:**
- ✅ Persistent domain permissions (domain_permissions table (SQLite))
- ✅ Check before processing wallet requests
- ✅ User approval modal for new domains
- ✅ Domain extracted from main frame URL

**Code Location:**
```cpp
// cef-native/src/core/HttpRequestInterceptor.cpp (DomainPermissionCache)
// rust-wallet/src/database/domain_permission_repo.rs
```

## ⚠️ Current Security Gaps

### 1. Single Webview = No Tab Isolation ⚠️

**Current:**
- Only one webview browser
- Navigating to new site replaces current site
- **If you open two sites sequentially, they share NO state** (different page loads)

**Risk:**
- ✅ Actually LOW risk currently (only one site at a time)
- ⚠️ WOULD be high risk if you implement tabs without process isolation

### 2. No Content Security Policy (CSP) ⚠️

**Missing:**
- No CSP headers enforced
- Websites can include any external scripts
- No XSS protection beyond browser defaults

**Recommendation:**
- Consider adding CSP headers in HTTP interceptor
- Block inline scripts on approved domains
- Restrict external script sources

### 3. No Request Size Limits ⚠️

**Missing:**
- No limits on HTTP request size
- Could be used for DoS attacks
- Memory exhaustion possible

**Recommendation:**
- Add request size limits in HTTP interceptor
- Timeout for long-running requests
- Rate limiting per domain

## 🎯 Security Impact of Adding Tabs

### Without Process-Per-Tab (DON'T DO THIS)

```
❌ BAD APPROACH:
┌─────────────────────────────────────┐
│    Single Webview Process           │
│  - Tab 1: peerpay.com  ─┐           │
│  - Tab 2: malicious.com ├─ Same V8  │
│  - Tab 3: thryll.com   ─┘           │
│                                     │
│  All tabs share JavaScript context! │
│  Malicious site can access others!  │
└─────────────────────────────────────┘
```

**Risks:**
- ❌ Tab 2 can read Tab 1's cookies/localStorage
- ❌ Tab 2 can intercept Tab 1's wallet API calls
- ❌ Tab 2 can steal Tab 3's BRC100 session
- ❌ **CRITICAL SECURITY VULNERABILITY**

**Verdict**: ❌ **NEVER DO THIS FOR BITCOIN WALLET BROWSER**

### With Process-Per-Tab (DO THIS)

```
✅ GOOD APPROACH:
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   Tab 1     │  │   Tab 2     │  │   Tab 3     │
│  Process    │  │  Process    │  │  Process    │
│             │  │             │  │             │
│ peerpay.com │  │malicious.com│  │ thryll.com  │
│ Own V8      │  │ Own V8      │  │ Own V8      │
│ Isolated    │  │ Isolated    │  │ Isolated    │
└─────────────┘  └─────────────┘  └─────────────┘
       │                │                │
       └────────────────┴────────────────┘
                        │
                        ▼
              ┌──────────────────┐
              │ HTTP Interceptor │
              │ Domain Permissions │
              └──────────────────┘
                        │
                        ▼
              ┌──────────────────┐
              │   Rust Wallet      │
              │   Wallet Ops     │
              └──────────────────┘
```

**Security:**
- ✅ Tab 2 can't access Tab 1's memory
- ✅ Tab 2 can't steal Tab 1's sessions
- ✅ Tab crash doesn't affect other tabs
- ✅ Complete isolation between websites

**Verdict**: ✅ **REQUIRED FOR SECURE BITCOIN WALLET BROWSER**

## 📋 Security Checklist for Tabs

Before implementing tabs, ensure:

- [ ] **Process-per-tab architecture** (like Chrome/Brave)
- [ ] **Tab-specific session management** (track which tab is authenticated)
- [ ] **UTXO locking** (prevent double-spend from concurrent tabs)
- [ ] **Auth request queuing** (handle multiple simultaneous auth requests)
- [ ] **Tab context in messages** (know which tab sent request)
- [ ] **Tab-specific domain tracking** (each tab has own domain context)
- [ ] **Proper cleanup on tab close** (release sessions, locks, resources)

## 🎓 Summary

### Your Current Security: EXCELLENT ✅

**Strengths:**
- ✅ Process isolation between UI and web content
- ✅ Each overlay in separate process
- ✅ Wallet in separate Rust wallet backend
- ✅ HTTP interception with domain permission checking
- ✅ No shared memory between security boundaries

**For Tabs:**
- ✅ **MUST use process-per-tab** for security
- ✅ Your architecture already supports this pattern
- ✅ Wallet/BRC100 will work independently per tab
- ✅ Implement back/forward/refresh first (essential features)

### Recommended Implementation Order:

1. **Week 1**: Back/Forward/Refresh buttons ⭐⭐⭐⭐⭐ **DO FIRST**
2. **Week 2**: Tab architecture design & planning
3. **Week 3-4**: Tab implementation with process-per-tab
4. **Week 5**: Testing & security validation

---

**Bottom Line**: Yes, implement navigation buttons first. They're essential, quick to build, and will help you understand the system better before tackling tabs. Tabs WILL work with wallet/BRC100, but MUST use process-per-tab for security.
