# macOS Platform Support Plan

**Created**: 2025-12-31 (original)
**Updated**: 2026-02-19 (comprehensive rewrite based on codebase audit + research)
**Status**: Planning — target near end of MVP development
**Goal**: Full feature parity with Windows build

---

## Executive Summary

The macOS port is in a **strong foundation state**. The hard architectural work is done — CMakeLists.txt cross-platform build, NSWindow/NSView hierarchy, 5 overlay windows, event forwarding, CALayer rendering, and 5 CEF helper app bundles are all implemented. What remains is **connecting the plumbing**: HTTP interception, WinHTTP singleton replacements, data paths, and Keychain integration. The Rust wallet and React frontend are already cross-platform and work on macOS today.

**Estimated effort**: 5-7 days for a focused macOS sprint.

---

## 1. Current State Assessment

### What's Done (December 2025 port)

| Component | Status | Notes |
|-----------|--------|-------|
| **CMakeLists.txt** | ✅ Complete | Platform detection, Homebrew packages, framework linking, helper bundles. Portable build (Phase 1): hardcoded vcpkg paths removed, auto-detects via `VCPKG_ROOT` env var or `-DCMAKE_TOOLCHAIN_FILE` flag. No breaking changes to existing builds. |
| **cef_browser_shell_mac.mm** | ✅ Complete (1754 lines) | NSWindow, header/webview NSViews, 5 overlays, event forwarding |
| **my_overlay_render_handler.mm** | ✅ Complete | CALayer rendering with transparency |
| **process_helper_mac.mm** | ✅ Complete | 5 helper .app bundles (GPU, Renderer, Plugin, Alerts, base) |
| **TabManager_mac.mm** | ✅ Partial | Basic tab lifecycle with NSView, needs integration |
| **WalletService_mac.cpp** | ⚠️ Partial | libcurl-based HTTP, basic connection only |
| **simple_handler.cpp** | ✅ Wrapped | 75+ `#ifdef _WIN32` blocks, macOS stubs for wallet/tab/history |
| **simple_app.cpp** | ✅ Wrapped | Platform-split `OnContextInitialized` |
| **simple_render_process_handler.cpp** | ✅ Wrapped | V8 injection cross-platform |
| **Info.plist** | ✅ Created | Basic bundle config, missing usage descriptions |
| **Rust wallet** | ✅ Works | `cargo run --release` works natively on macOS |
| **React frontend** | ✅ Works | `npm run dev` works natively on macOS |

### What's Missing

| Component | Gap | Effort |
|-----------|-----|--------|
| **HTTP Request Interception** | Windows WinHTTP singletons not ported | 2-3 days |
| **Data directory paths** | Hardcoded `APPDATA` env var, falls back to `.` | 0.5 day |
| **DPAPI → Keychain** | Stub returns error on macOS | 1 day |
| **History/Bookmark managers** | Windows-only SQLite + Win32 paths | 0.5 day (path fix) |
| **Notification overlay** | Not in Dec 2025 port (added in Phase 2, Feb 2026) | 0.5 day |
| **Code signing + notarization** | Not set up | 0.5-1 day |
| **Entitlements** | Missing camera/mic/location/network | 0.5 day |
| **Info.plist usage descriptions** | Missing NSCamera/Mic/LocationUsageDescription | Trivial |
| **Keyboard shortcuts** | Cmd instead of Ctrl (macOS convention) | 0.5 day |

---

## 2. macOS File System Conventions

### Where Chrome and Brave Store Data on macOS

| Data | Chrome Path | Brave Path |
|------|-------------|------------|
| **Profile root** | `~/Library/Application Support/Google/Chrome/` | `~/Library/Application Support/BraveSoftware/Brave-Browser/` |
| **Default profile** | `.../Chrome/Default/` | `.../Brave-Browser/Default/` |
| **Cookies** | `Default/Network/Cookies` (SQLite) | Same layout |
| **History** | `Default/History` (SQLite) | Same layout |
| **Bookmarks** | `Default/Bookmarks` (JSON) | Same layout |
| **Cache** | `~/Library/Caches/Google/Chrome/Default/Cache/` | `~/Library/Caches/BraveSoftware/Brave-Browser/Default/Cache/` |
| **Encryption key** | macOS Keychain ("Chrome Safe Storage") | macOS Keychain ("Brave Safe Storage") |

### Hodos Browser — Correct macOS Paths

```
~/Library/Application Support/HodosBrowser/           # Root (equivalent of %APPDATA%/HodosBrowser/)
├── wallet/
│   └── wallet.db                                      # Rust wallet database
├── Default/                                           # CEF profile
│   ├── HodosHistory                                   # Custom history DB
│   ├── bookmarks.db                                   # Custom bookmarks DB
│   ├── cookie_blocks.db                               # Cookie blocking rules
│   ├── Cookies                                        # CEF cookie jar
│   └── ...                                            # CEF auto-created files
├── adblock/                                           # Future: ad block filter lists + engine
│   ├── lists/
│   └── engine.dat
└── settings.json                                      # Future: browser settings

~/Library/Caches/HodosBrowser/                         # HTTP cache (separate, not backed up)
~/Library/Logs/HodosBrowser/                           # Application logs
```

### Implementation: Cross-Platform Path Resolution

**Rust (main.rs)** — Currently broken on macOS:
```rust
// CURRENT (Windows-only):
let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());

// CORRECT (cross-platform using `dirs` crate):
let appdata = dirs::data_dir()       // ~/Library/Application Support/ on macOS
    .unwrap_or_else(|| PathBuf::from("."))  // %APPDATA% on Windows
    .to_string_lossy()
    .to_string();
```

Add `dirs = "5"` to `Cargo.toml` dependencies. The `dirs` crate uses Apple's `NSSearchPathForDirectoriesInDomains` on macOS and `SHGetKnownFolderPath` on Windows — both are the correct OS APIs.

**C++ (various files)** — Path resolution:
```cpp
#ifdef _WIN32
    // Windows: Use APPDATA environment variable
    std::string appdata = std::getenv("APPDATA");
    std::string dataDir = appdata + "\\HodosBrowser";
#elif defined(__APPLE__)
    // macOS: Use ~/Library/Application Support/
    NSArray* paths = NSSearchPathForDirectoriesInDomains(
        NSApplicationSupportDirectory, NSUserDomainMask, YES);
    NSString* appSupport = [paths firstObject];
    std::string dataDir = [appSupport UTF8String] + std::string("/HodosBrowser");
#endif
```

---

## 3. DPAPI → macOS Keychain

### The Problem

Windows uses DPAPI (`CryptProtectData`/`CryptUnprotectData`) to encrypt the mnemonic at rest. On macOS, the equivalent is the **macOS Keychain**. Our `dpapi.rs` currently returns an error on non-Windows.

### Architecture Difference

| Aspect | Windows DPAPI | macOS Keychain |
|--------|--------------|----------------|
| Model | Encrypt blob → caller stores blob | Store secret → OS stores it securely |
| Storage | Encrypted blob in `wallet.db` column `mnemonic_dpapi` | Secret lives in Keychain, not in our DB |
| Retrieval | Read blob from DB → decrypt | Query Keychain by service+account name |
| User gating | Windows login unlocks DPAPI | macOS login unlocks Keychain |

### Recommended Implementation

Use the `keyring` crate (v3.x) with the `apple-native` feature for macOS:

```toml
# Cargo.toml
[target.'cfg(target_os = "macos")'.dependencies]
keyring = { version = "3", features = ["apple-native"] }
```

**Updated `crypto/dpapi.rs`**:
```rust
// Cross-platform mnemonic protection

#[cfg(target_os = "windows")]
pub fn platform_encrypt_mnemonic(mnemonic: &[u8]) -> Result<Vec<u8>, String> {
    dpapi_encrypt(mnemonic)  // Returns encrypted blob for DB storage
}

#[cfg(target_os = "windows")]
pub fn platform_decrypt_mnemonic(encrypted: &[u8]) -> Result<Vec<u8>, String> {
    dpapi_decrypt(encrypted)  // Decrypts blob from DB
}

#[cfg(target_os = "macos")]
pub fn platform_encrypt_mnemonic(mnemonic: &[u8]) -> Result<Vec<u8>, String> {
    let entry = keyring::Entry::new("HodosBrowser", "wallet-mnemonic")
        .map_err(|e| format!("Keychain error: {}", e))?;
    entry.set_secret(mnemonic)
        .map_err(|e| format!("Keychain store error: {}", e))?;
    Ok(vec![1])  // Sentinel: "stored in Keychain" (not the actual data)
}

#[cfg(target_os = "macos")]
pub fn platform_decrypt_mnemonic(_encrypted: &[u8]) -> Result<Vec<u8>, String> {
    let entry = keyring::Entry::new("HodosBrowser", "wallet-mnemonic")
        .map_err(|e| format!("Keychain error: {}", e))?;
    entry.get_secret()
        .map_err(|e| format!("Keychain retrieve error: {}", e))
}
```

**Key difference**: On macOS, the `mnemonic_dpapi` column in `wallet.db` stores a sentinel value (not the actual encrypted data). The real secret lives in the Keychain. This is fine — the column just indicates "Keychain entry exists, try auto-unlock."

**Requires code signing**: Full Keychain access on macOS requires the app to be signed. During development, unsigned apps can still use Keychain but may trigger additional system prompts.

---

## 4. C++ Singleton Porting — WinHTTP → libcurl/CFNetwork

### The Core Problem

Five C++ singletons use synchronous **WinHTTP** to call the Rust backend:

| Singleton | Purpose | macOS Status |
|-----------|---------|-------------|
| `DomainPermissionCache` | Check domain trust level | ❌ Returns "unknown" |
| `BSVPriceCache` | BSV/USD price for auto-approve | ❌ Returns 0.0 |
| `WalletStatusCache` | Check if wallet exists/locked | ❌ Returns false |
| `fetchCertFieldsFromBackend()` | Check cert field permissions | ❌ Not available |
| `X-Requesting-Domain` header | Added in `startAsyncHTTPRequest` | ❌ Not added |

### Recommended Approach: libcurl (Already Linked)

The macOS build already links libcurl (via WalletService_mac.cpp). Use the same pattern for all singletons.

**Option A: Platform-conditional in each singleton** (simpler, more duplication)
```cpp
#ifdef _WIN32
    // Existing WinHTTP code
#elif defined(__APPLE__)
    CURL* curl = curl_easy_init();
    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 5L);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, writeCallback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &response);
    CURLcode res = curl_easy_perform(curl);
    curl_easy_cleanup(curl);
#endif
```

**Option B: Shared HTTP helper** (cleaner, less duplication)
Create `SyncHttpClient.h/cpp` that wraps WinHTTP on Windows and libcurl on macOS behind a single interface:
```cpp
class SyncHttpClient {
public:
    static std::string get(const std::string& url, int timeoutMs = 5000);
    static std::string post(const std::string& url, const std::string& body, int timeoutMs = 5000);
};
```

**Recommendation**: Option B. It reduces the 5 platform conditionals to 1 shared helper, and it's the right abstraction for our codebase (these singletons all do the same thing — sync HTTP to localhost:3301).

---

## 5. Notification Overlay (Post-December Port)

The notification overlay system (keep-alive HWND, JS injection, `window.showNotification()`) was added in Phase 2 (February 2026), after the December macOS port. It needs to be ported.

**What's needed**:
- macOS equivalent of `CreateNotificationOverlay` — new NSWindow with offscreen CEF browser
- Keep-alive pattern: hide/show NSWindow instead of destroy/recreate
- JS injection via `ExecuteJavaScript("window.showNotification('...')")` — same as Windows
- Pre-creation on header load (existing pattern)

**Effort**: 0.5 day — the pattern already exists in `cef_browser_shell_mac.mm` for the other 5 overlays. The notification overlay is the 6th, following the same pattern.

---

## 6. Keyboard Shortcuts (Cmd vs Ctrl)

macOS convention uses **Cmd** (⌘) instead of **Ctrl** for most shortcuts:

| Action | Windows | macOS |
|--------|---------|-------|
| New tab | Ctrl+T | Cmd+T |
| Close tab | Ctrl+W | Cmd+W |
| Next tab | Ctrl+Tab | Cmd+Option+Right (or Ctrl+Tab) |
| Find | Ctrl+F | Cmd+F |
| Reload | Ctrl+R / F5 | Cmd+R |
| DevTools | Ctrl+Shift+I / F12 | Cmd+Option+I |
| Address bar | Ctrl+L | Cmd+L |
| Bookmark | Ctrl+D | Cmd+D |
| Print | Ctrl+P | Cmd+P |
| Quit | Alt+F4 | Cmd+Q |
| Preferences | — | Cmd+, |

**Implementation**: In the keyboard handler, check for `event.modifiers & EVENTFLAG_COMMAND_DOWN` on macOS instead of `EVENTFLAG_CONTROL_DOWN`.

---

## 7. App Bundle, Code Signing & Notarization

### App Bundle Structure (Current vs Required)

**Current** (from CMakeLists.txt):
```
HodosBrowserShell.app/
  Contents/
    MacOS/HodosBrowserShell
    Frameworks/
      Chromium Embedded Framework.framework/
      HodosBrowserShell Helper.app/
      HodosBrowserShell Helper (GPU).app/
      HodosBrowserShell Helper (Renderer).app/
      HodosBrowserShell Helper (Plugin).app/
      HodosBrowserShell Helper (Alerts).app/
    Resources/ (pak files, locales)
    Info.plist
```

**Required additions**:
- `Resources/app.icns` — Application icon
- Entitlements files for main app and helpers
- Usage description strings in Info.plist

### Info.plist Updates Needed

```xml
<!-- Camera/mic/location permission prompts (required by macOS) -->
<key>NSCameraUsageDescription</key>
<string>Websites may request access to your camera for video calls.</string>
<key>NSMicrophoneUsageDescription</key>
<string>Websites may request access to your microphone for audio calls.</string>
<key>NSLocationUsageDescription</key>
<string>Websites may request access to your location.</string>
```

### Entitlements (app.entitlements)

```xml
<dict>
    <!-- V8 JIT compilation -->
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    <!-- Load CEF framework -->
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
    <!-- V8 JIT on Intel -->
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <!-- WebRTC camera/mic -->
    <key>com.apple.security.device.camera</key>
    <true/>
    <key>com.apple.security.device.microphone</key>
    <true/>
    <!-- Geolocation -->
    <key>com.apple.security.personal-information.location</key>
    <true/>
    <!-- Network (localhost wallet + internet) -->
    <key>com.apple.security.network.client</key>
    <true/>
    <key>com.apple.security.network.server</key>
    <true/>
    <!-- File download/upload -->
    <key>com.apple.security.files.user-selected.read-write</key>
    <true/>
    <key>com.apple.security.files.downloads.read-write</key>
    <true/>
</dict>
```

### Code Signing Process

1. **Apple Developer ID** — requires Apple Developer Program membership ($99/year)
2. **Sign inside-out**: helpers → framework → main app
3. **Hardened Runtime** required for notarization
4. **Notarize**: `xcrun notarytool submit HodosBrowser.zip --apple-id X --team-id Y --password Z`
5. **Staple**: `xcrun stapler staple HodosBrowser.app`

Without code signing, macOS Gatekeeper blocks the app. Users can override via System Preferences, but this is not acceptable for production.

---

## 8. Sprint Plan: macOS Port Completion

### Prerequisites
- Apple Developer ID certificate (needed for code signing)
- macOS machine with Xcode, Homebrew, CEF binaries
- App icon (.icns format)

### Day 1: Data Paths + Keychain (Foundation)

| Task | Effort |
|------|--------|
| Add `dirs` crate to Cargo.toml, fix `main.rs` path resolution | 1 hr |
| Implement `platform_encrypt_mnemonic`/`platform_decrypt_mnemonic` via `keyring` crate | 2 hrs |
| Update startup flow to use Keychain on macOS (mirror DPAPI backfill logic) | 1 hr |
| Test: Rust wallet creates `~/Library/Application Support/HodosBrowser/wallet/wallet.db` | 0.5 hr |
| Test: Mnemonic stored in Keychain, auto-unlocks on restart | 0.5 hr |

### Day 2: SyncHttpClient + Singleton Porting

| Task | Effort |
|------|--------|
| Create `SyncHttpClient.h/cpp` (WinHTTP on Windows, libcurl on macOS) | 3 hrs |
| Port `DomainPermissionCache` to use SyncHttpClient | 1 hr |
| Port `BSVPriceCache` to use SyncHttpClient | 0.5 hr |
| Port `WalletStatusCache` to use SyncHttpClient | 0.5 hr |
| Port `fetchCertFieldsFromBackend()` to use SyncHttpClient | 0.5 hr |
| Verify `X-Requesting-Domain` header added on macOS path | 0.5 hr |

### Day 3: HTTP Interception + Notification Overlay

| Task | Effort |
|------|--------|
| Ensure `HttpRequestInterceptor.cpp` macOS `#elif` blocks call the ported singletons | 2 hrs |
| Port the auto-approve engine path (payment/cert checks use singletons) | 1 hr |
| Add notification overlay to `cef_browser_shell_mac.mm` (6th overlay, keep-alive pattern) | 2 hrs |
| Port `SessionManager` macOS path (should work — it's pure C++ with mutex) | 0.5 hr |

### Day 4: History/Bookmarks Paths + Keyboard Shortcuts

| Task | Effort |
|------|--------|
| Fix HistoryManager macOS path (`~/Library/Application Support/HodosBrowser/Default/`) | 1 hr |
| Fix BookmarkManager macOS path | 0.5 hr |
| Fix CookieBlockManager macOS path | 0.5 hr |
| Map Ctrl shortcuts to Cmd on macOS in keyboard handler | 2 hrs |
| Add Cmd+Q quit, Cmd+, preferences shortcuts | 0.5 hr |
| Test all keyboard shortcuts on macOS | 1 hr |

### Day 5: Entitlements, Info.plist, Code Signing

| Task | Effort |
|------|--------|
| Add camera/mic/location usage descriptions to Info.plist | 0.5 hr |
| Create `app.entitlements` and `helper.entitlements` files | 1 hr |
| Update CMakeLists.txt to apply entitlements during build | 1 hr |
| Set up code signing in build script (or Makefile) | 2 hrs |
| Test notarization pipeline (sign → submit → staple) | 2 hrs |

### Day 6-7: Integration Testing + Bug Fixes

| Task | Effort |
|------|--------|
| Full integration test: wallet create/recover/send/receive | 4 hrs |
| Test domain approval flow (notification overlay) | 1 hr |
| Test auto-approve engine (spending limits, rate limiting) | 1 hr |
| Test all overlays (settings, wallet, backup, BRC-100 auth, notification) | 2 hrs |
| Test tab management (create, switch, close) | 1 hr |
| Test history recording and search | 0.5 hr |
| Test bookmarks CRUD | 0.5 hr |
| Bug fixes from testing | 4 hrs (buffer) |

---

## 9. Long-Term Considerations

### Things That Will "Just Work" Once the Sprint Is Done
- All Rust wallet features (cross-platform by default)
- All React frontend features (cross-platform by default)
- All CEF browser features implemented in browser-core sprints (SSL, downloads, permissions, find-in-page, context menus) — because they're implemented on `SimpleHandler` which is already cross-platform wrapped
- Ad blocking (adblock-rust FFI library compiles on macOS natively)

### Things That Need Ongoing Attention
- **New C++ singletons**: Any new WinHTTP singleton added on Windows must also use `SyncHttpClient` (the abstraction prevents this from being forgotten)
- **New overlays**: Any new overlay window type needs a macOS creation function in `cef_browser_shell_mac.mm`
- **File paths**: All new file I/O must use the cross-platform path helper, not hardcoded Windows paths
- **Keyboard shortcuts**: New shortcuts need both Ctrl (Windows) and Cmd (macOS) mappings

### macOS-Specific Enhancements (Post-MVP)
- Native macOS dark mode integration
- Touch Bar support (MacBook Pro)
- macOS menu bar integration (standard Edit/View/Window/Help menus)
- Retina (@2x) icon assets
- macOS Handoff/Continuity integration
- Spotlight indexing of bookmarks/history
- DMG installer with drag-to-Applications
- Auto-update mechanism (Sparkle framework is the standard for non-App Store apps)

---

## 10. Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| CEF framework changes between versions | Build may break on macOS | Pin CEF version, test both platforms on update |
| Keychain access issues with unsigned app | DPAPI equivalent fails in dev | Use `security` CLI tool for dev testing; prioritize code signing |
| libcurl SSL certificate issues on macOS | Singletons can't reach Rust backend | Use `--insecure` for localhost only, or link against system SSL |
| Notification overlay timing differences | JS injection race on macOS | Use same keep-alive + preload pattern as Windows |
| Code signing certificate expiry | App stops launching for new users | Set calendar reminder, 5-year cert available |

---

## 11. Portable Build Configuration (Complete)

The Windows build system was made portable, removing all developer-specific hardcoded paths from `cef-native/CMakeLists.txt`. This was a prerequisite for cross-platform builds.

**What was done**:
- Removed hardcoded vcpkg paths (`C:/Users/archb/Dev/vcpkg/...`)
- Added automatic vcpkg detection via `VCPKG_ROOT` environment variable
- Falls back to `-DCMAKE_TOOLCHAIN_FILE=...` command-line parameter
- Automatic package discovery (OpenSSL, nlohmann-json, sqlite3)
- No breaking changes to existing Windows builds

**Build options (Windows)**:
```powershell
# Option 1: Environment variable (recommended)
$env:VCPKG_ROOT = "C:/Users/<YourUsername>/Dev/vcpkg"
cmake -S . -B build -G "Visual Studio 17 2022" -A x64

# Option 2: Command-line parameter
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 `
  -DCMAKE_TOOLCHAIN_FILE=C:/Users/<YourUsername>/Dev/vcpkg/scripts/buildsystems/vcpkg.cmake
```

---

## 12. Settings Sprints — macOS Considerations

Several settings sprints in `development-docs/Settings_Sprints/` include platform-specific C++ code. When porting to macOS, review these sprints for platform conditionals:

| Sprint | macOS Notes |
|--------|------------|
| **G4** (New Tab Page) | Context menu "Set as homepage" needs macOS equivalent |
| **G5** (Default Browser) | `ms-settings:defaultapps` → macOS System Settings + `LSSetDefaultHandlerForURLScheme` |
| **D1** (Download Settings) | `CefBrowserHost::RunFileDialog` should work cross-platform; verify on macOS |
| **PS3** (Clear Data on Exit) | Shutdown hook: `WM_CLOSE` → `applicationShouldTerminate:` or `windowWillClose:` |

---

## Related Documents

- [MACOS_IMPLEMENTATION_COMPLETE.md](/MACOS_IMPLEMENTATION_COMPLETE.md) — Dec 2025 port details
- [MACOS_PORT_SUCCESS.md](/MACOS_PORT_SUCCESS.md) — Build success summary
- [PHASE2_MACOS_SUPPORT_SUMMARY.md](/PHASE2_MACOS_SUPPORT_SUMMARY.md) — Phase 2 CMake details
- [build-instructions/MACOS_BUILD_INSTRUCTIONS.md](/build-instructions/MACOS_BUILD_INSTRUCTIONS.md) — Build guide
- [browser-core/implementation-plan.md](../browser-core/implementation-plan.md) — MVP sprint plan (Windows-first)
- [Settings_Sprints/00-SPRINT-INDEX.md](../Settings_Sprints/00-SPRINT-INDEX.md) — Settings implementation sprints (Windows-first, macOS notes flagged)

---

**End of Document**
