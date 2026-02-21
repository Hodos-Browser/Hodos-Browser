# Browser Core Sprint — Context for Claude

**Scope**: Sprints 0-10 from [implementation-plan.md](./implementation-plan.md). Browser features, security/privacy, and wallet polish.

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| [implementation-plan.md](./implementation-plan.md) | Sprint-by-sprint implementation details |
| [mvp-gap-analysis.md](./mvp-gap-analysis.md) | Prioritized gap list with effort estimates |
| [browser-capabilities.md](./browser-capabilities.md) | What works/doesn't/is missing |
| [architecture-assessment.md](./architecture-assessment.md) | Communication patterns, thread safety, debt |
| [01-chrome-brave-research.md](./01-chrome-brave-research.md) | Chrome/Brave research findings |
| [doc-discrepancies.md](./doc-discrepancies.md) | Stale documentation tracker |
| [file-inventory.md](./file-inventory.md) | Complete runtime file/DB/singleton catalog |

---

## Sprint Status Tracker

| Sprint | Name | Status |
|--------|------|--------|
| 0 | Safety & Quick Wins | Pending |
| 1 | SSL Certificate Handling + Secure Indicator | Pending |
| 2 | Permission Handler | Pending |
| 3 | Download Handler | Pending |
| 4 | Find-in-Page | Pending |
| 5 | Context Menu Enhancement | Pending |
| 6 | JS Dialog Handler + Keyboard Shortcuts | Pending |
| 7 | Light Wallet Polish | Pending |
| 8 | Ad & Tracker Blocking | Pending |
| 9 | Settings Persistence + Profile Import | Pending |
| 10 | Third-Party Cookie Blocking + Fingerprinting | Pending |

---

## Cross-Platform Rules (CRITICAL)

Every sprint MUST follow these rules to avoid accumulating macOS debt:

1. **Platform conditionals on all new C++ code**:
   ```cpp
   #ifdef _WIN32
       // Windows implementation
   #elif defined(__APPLE__)
       // macOS implementation (stub or TODO is OK)
   #endif
   ```

2. **No raw WinHTTP in new singletons** — use a `SyncHttpClient` abstraction (or at minimum, wrap with `#ifdef` and a macOS `#elif` using libcurl/NSURLSession). Existing WinHTTP singletons (`DomainPermissionCache`, `BSVPriceCache`, `WalletStatusCache`) will be refactored during the macOS sprint.

3. **File paths**: Use `SHGetFolderPathW(CSIDL_APPDATA)` on Windows (already in use), `NSSearchPathForDirectoriesInDomains` on macOS. Never hardcode `%APPDATA%` in new code — use the existing path resolution helper.

4. **New overlays**: Any new HWND-based overlay (e.g., downloads panel) needs a corresponding macOS creation stub in `cef_browser_shell_mac.mm`.

5. **Keyboard shortcuts**: Define with both `Ctrl+X` (Windows) and `Cmd+X` (macOS) variants. Use the platform key macro.

6. **CEF helper bundles**: No action needed per-sprint, but be aware: macOS requires 5 helper `.app` bundles. Any new subprocess types need entries.

---

## Key Architecture Patterns for This Sprint

### SimpleHandler is the Hub
`simple_handler.cpp` already implements 8 CEF handler interfaces. New handlers (CefDownloadHandler, CefPermissionHandler, CefFindHandler, CefJsDialogHandler) are added here by:
1. Adding `public CefXxxHandler` to the class declaration in `simple_handler.h`
2. Adding `GetXxxHandler() override { return this; }` method
3. Implementing the handler methods

### HTTP Interception Flow
```
Web request → OnBeforeResourceLoad (HttpRequestInterceptor.cpp)
  → Is it a wallet endpoint? → AsyncWalletResourceHandler
    → Is domain approved? → DomainPermissionCache check
      → Is it a payment endpoint? → Auto-approve engine (spending limits, rate limits)
        → Forward to Rust (localhost:3301) via CefURLRequest on IO thread
```

### IPC Pattern (C++ ↔ React)
```
React → cefMessage.send("command_name", data)
  → CefProcessMessage to browser process
    → simple_handler.cpp OnProcessMessageReceived
      → dispatch by message name
```

### CEF Threading Model
- **UI thread**: Browser creation, window management, IPC dispatch
- **IO thread**: HTTP interception, CefURLRequest, resource handlers
- **Renderer process**: V8 injection, JavaScript context (separate process!)
- **RULE**: Never block the UI thread. Use `CefPostTask(TID_IO, ...)` for async work.

---

## Sprint-Specific Notes

### Sprint 0 (Safety)
- BSVPriceCache at `HttpRequestInterceptor.cpp` ~line 50 — the `fetchPrice()` method returns 0.0 on error. Fix: cache `lastSuccessfulPrice_`, return sentinel -1.0 if never fetched.
- `price_cache.rs` — similar pattern. `get_cached()` should return `Option<f64>`.
- WebRTC flag goes in `cef_browser_shell.cpp` where other `--switches` are set.

### Sprint 1 (SSL)
- `OnCertificateError` uses `CefCallback` (not `CefRequestCallback`). Check CEF 136 API.
- Padlock indicator: header bar is React (port 5137), SSL status is in C++. Need IPC bridge.

### Sprint 2 (Permissions)
- CEF 136 Chrome bootstrap: returning `false` from `OnShowPermissionPrompt` shows native Chrome permission UI. Test this FIRST before building custom UI.

### Sprint 3 (Downloads)
- MVP: Just implement `OnBeforeDownload` with `callback->Continue("", true)` for Save As dialog. Skip progress overlay initially.

### Sprint 8 (Ad Blocking)
- `adblock-rust` is a separate crate, NOT in the Rust wallet workspace.
- FFI static library linked into C++ — NOT HTTP to Rust.
- Filter lists stored in `%APPDATA%/HodosBrowser/adblock/`.
- Hook point: `OnBeforeResourceLoad` in `HttpRequestInterceptor.cpp`, BEFORE wallet interception check.

---

## Files Most Likely to Change

| File | Sprints |
|------|---------|
| `simple_handler.h` | 1, 2, 3, 4, 6 |
| `simple_handler.cpp` | 1, 2, 3, 4, 5, 6 |
| `HttpRequestInterceptor.cpp` | 0, 8, 10 |
| `cef_browser_shell.cpp` | 0, 1, 6, 9 |
| `simple_render_process_handler.cpp` | 10 |
| `frontend/src/components/MainBrowserView.tsx` | 1, 4 |
| `frontend/src/components/WalletPanel.tsx` | 7 |
| `frontend/src/components/TransactionForm.tsx` | 7 |
| `rust-wallet/src/price_cache.rs` | 0 |

---

## Testing Approach

- User runs the browser to test — do not attempt to run it
- After C++ changes: `cmake --build build --config Release` in `cef-native/`
- After Rust changes: `cargo check` or `cargo build --release` in `rust-wallet/`
- After frontend changes: `npm run build` in `frontend/`
- Each sprint has a verification checklist in `implementation-plan.md`
