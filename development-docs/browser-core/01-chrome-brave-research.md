# Chrome & Brave Browser Research

**Created**: 2026-02-19
**Status**: Complete (Phase B)
**Purpose**: Research findings on how Chrome and Brave handle core browser features, with focus on what open source code we can reuse and what CEF APIs are available.

---

## B.1 Chrome Browser Internals

### Chrome's `User Data\Default\` File Structure

**SQLite Databases:**

| File | Contents |
|------|----------|
| `History` | Browsing history (`urls`, `visits`, `downloads`, `keyword_search_terms`) |
| `Network/Cookies` | All cookies with DPAPI-encrypted values |
| `Web Data` | Autofill, form data, credit cards, addresses |
| `Login Data` | Saved passwords (DPAPI-encrypted) |
| `Favicons` | Website favicons + URLs |
| `Top Sites` | Most-visited sites with thumbnails |
| `Shortcuts` | Omnibox autocomplete learned suggestions |

**LevelDB Stores:**

| Directory | Contents |
|-----------|----------|
| `Local Storage/leveldb/` | Per-origin `localStorage` |
| `Session Storage/` | Per-origin `sessionStorage` |
| `IndexedDB/` | Per-origin IndexedDB (each origin gets its own LevelDB) |

**JSON / Other:**

| File | Contents |
|------|----------|
| `Bookmarks` | Full bookmark tree (JSON) |
| `Preferences` | Browser settings, per-site permissions, content settings |
| `Secure Preferences` | HMAC-verified subset of preferences |
| `Visited Links` | Bloom filter of visited URL fingerprints |

### Cookie Database Schema

Located at `Default/Network/Cookies`. Key columns:

```sql
CREATE TABLE cookies (
    creation_utc     INTEGER NOT NULL,   -- Microseconds since 1601-01-01
    host_key         TEXT NOT NULL,       -- Domain (e.g., ".example.com")
    name             TEXT NOT NULL,
    value            TEXT NOT NULL,       -- Plaintext (or empty if encrypted)
    encrypted_value  BLOB DEFAULT '',    -- DPAPI-encrypted value
    path             TEXT NOT NULL,
    expires_utc      INTEGER NOT NULL,
    is_secure        INTEGER NOT NULL,
    is_httponly       INTEGER NOT NULL,
    samesite         INTEGER NOT NULL,
    -- ... other fields
);
```

**Encryption**: `encrypted_value` is AES-128-CBC encrypted, key protected by Windows DPAPI. The key is in `User Data/Local State` as `os_crypt.encrypted_key`. We already have DPAPI support in `crypto/dpapi.rs`, so cookie import is feasible.

### History Database Schema

Located at `Default/History`. Key tables:

```sql
CREATE TABLE urls (
    id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR,
    visit_count INTEGER, typed_count INTEGER,
    last_visit_time INTEGER NOT NULL  -- Microseconds since 1601-01-01
);
CREATE TABLE visits (
    id INTEGER PRIMARY KEY, url INTEGER NOT NULL,  -- FK to urls.id
    visit_time INTEGER NOT NULL, from_visit INTEGER, transition INTEGER,
    visit_duration INTEGER DEFAULT 0  -- Microseconds
);
```

**Timestamp conversion**: `(chrome_timestamp / 1000000) - 11644473600` = Unix epoch.

### Bookmarks Format

`Default/Bookmarks` is JSON:

```json
{
    "roots": {
        "bookmark_bar": { "children": [...], "type": "folder" },
        "other": { "children": [...], "type": "folder" },
        "synced": { "children": [...], "type": "folder" }
    }
}
```

Each entry has `type` ("url" or "folder"), `name`, `url`, `date_added` (Chrome timestamp), `guid` (UUID). Straightforward JSON parsing for import.

### Permissions Storage

Per-site permissions are in `Preferences` JSON at `profile.content_settings.exceptions`:

```json
{
    "media_stream_camera": { "https://meet.google.com:443,*": { "setting": 1 } },
    "notifications": { "https://example.com:443,*": { "setting": 2 } }
}
```

Settings: `1` = Allow, `2` = Block, `3` = Ask (default).

### Profile Import Feasibility

| Data Type | Source | Method | Difficulty |
|-----------|--------|--------|------------|
| Bookmarks | `Bookmarks` JSON | JSON parse | **Easy** |
| History | `History` SQLite | Read `urls` + `visits` | **Easy** |
| Cookies | `Network/Cookies` SQLite | Read + DPAPI decrypt | **Medium** (we have DPAPI) |
| Passwords | `Login Data` SQLite | DPAPI decrypt | **Hard** (security concerns) |
| Form data | `Web Data` SQLite | Read autofill tables | Easy |
| Permissions | `Preferences` JSON | Parse `content_settings` | Easy |
| Extensions | `Extensions/` directory | Not compatible | **Not feasible** |

---

## B.2 Brave Browser Architecture

### How `brave-core` Overlays Chromium

Brave does NOT fork Chromium. `brave-core` is a separate repo that layers on top via two mechanisms:

1. **`chromium_src` overlays**: Files in `brave-core/chromium_src/` mirror the Chromium source tree. Build compiles the Brave file *instead of* the original. Uses `#define Original Original_ChromiumImpl` + `#include "src/..."` to wrap/extend functions.

2. **Patch files**: For changes that can't use overlays (build files, class visibility changes).

**Relevance to Hodos**: We use CEF, which exposes handler interfaces (`CefResourceRequestHandler`, etc.) that serve the same purpose as Brave's overlays. Our `HttpRequestInterceptor.cpp` is the Hodos equivalent.

### Brave Shields

Unified per-site privacy controls stored via Chromium's `ContentSettingsProvider` system.

| Feature | Default |
|---------|---------|
| Ad & tracker blocking | Standard |
| HTTPS upgrades | On |
| Script blocking | Off (allow) |
| Fingerprinting protection | Standard (farbling) |
| Cookie control | Block 3rd-party |
| Referrer stripping | On |

**Always-on (not per-site)**: Global Privacy Control (GPC), De-AMP, redirect tracking URL cleanup, Client Hints reduction.

Our domain permission system is architecturally similar but scoped to wallet operations. Could extend to a Shields-like panel using existing notification overlay infrastructure.

### Cookie Controls

Brave blocks third-party cookies by default. Advanced features:
- **Ephemeral third-party storage**: Temporary cookies deleted when last tab for site closes (better compatibility than hard blocking)
- **CNAME uncloaking**: Resolves DNS CNAME chains to detect trackers disguised as first-party
- **Bounce tracking protection**: Tracks redirect chains, clears storage for suspected trackers

Most practical near-term improvement for Hodos: adopt filter-list-based blocking via `adblock-rust` (EasyPrivacy covers tracking cookies).

### Fingerprinting Protection ("Farbling")

Slightly randomizes output of identifying browser APIs using a **per-session, per-eTLD+1 seed**:
- Same site, same session: consistent values (pages work correctly)
- Different sites: different values (can't correlate)
- New session: new seed (can't track across sessions)

**APIs protected** (standard mode): Canvas 2D, WebGL, AudioContext, `navigator.hardwareConcurrency`, `navigator.deviceMemory`, `navigator.plugins`, screen dimensions, fonts.

**Implementation options for Hodos** (from easiest to hardest):
1. Block third-party access to fingerprint APIs via V8 injection (high value, low effort)
2. Implement farbling via V8 injection for Canvas/AudioContext (medium effort)
3. Full Blink-level modifications (requires patching CEF — not practical)

### WebRTC Leak Prevention

Four policies via Chromium's `WebRTCIPHandlingPolicy`:

| Policy | Local IP? | Public STUN? | Default When |
|--------|-----------|-------------|--------------|
| Default | Yes | Yes | Fingerprinting OFF |
| Public+Private Interfaces | Yes (default only) | Yes | — |
| **Public Interface Only** | **No** | Yes | — |
| Disable Non-Proxied UDP | No | No (TCP only) | Tor tabs |

**Implementation for Hodos**: One-line CEF config change:
`--force-webrtc-ip-handling-policy=default_public_interface_only`

### `adblock-rust` Crate

- **Repo**: [github.com/brave/adblock-rust](https://github.com/brave/adblock-rust) — 2,200+ stars
- **Crate**: `adblock` on crates.io, latest v0.11.0
- **License**: **MPL-2.0** (file-level copyleft — compatible with proprietary use)
- **Performance**: **5.7 microseconds** per request check (69x faster than their old C++ engine)
- **Memory**: ~15-25 MB for EasyList + EasyPrivacy (~55,000 rules)
- **Filter formats**: ABP syntax, uBlock Origin extensions, hosts files
- **Integration**: Brave uses C FFI via `adblock-rust-ffi` (static library linked into Chromium)

### Brave's Rust-C++ Bridging

| Component | Bridge Method |
|-----------|---------------|
| adblock-rust | Handwritten C FFI (cbindgen) |
| speedreader | C FFI (cbindgen) |
| challenge-bypass-ristretto | `cxx` bridge |
| brave_wallet (partial) | `cxx` bridge |

Brave is migrating toward `cxx` crate for safer interop. `cxx` provides zero-copy bridging with compile-time type checking.

### Reusable Open Source Components

| Repository | License | Use for Hodos | Priority |
|------------|---------|---------------|----------|
| [brave/adblock-rust](https://github.com/brave/adblock-rust) | MPL-2.0 | Ad & tracker blocking engine | **HIGH** |
| [brave/adblock-lists](https://github.com/brave/adblock-lists) | MPL-2.0 | Curated filter lists | **HIGH** |
| [brave/adblock-resources](https://github.com/brave/adblock-resources) | MPL-2.0 | Scriptlets + redirect rules | MEDIUM |
| [easylist/easylist](https://github.com/easylist/easylist) | GPL v3 / CC BY-SA 3.0 | Standard filter lists | **HIGH** |

EasyList/EasyPrivacy as runtime data files (downloaded, not compiled in) is standard practice for all browsers.

---

## B.3 CEF-Specific Research

### SSL Certificate Error Handling

**API**: `CefRequestHandler::OnCertificateError` (called on UI thread)

```cpp
bool OnCertificateError(
    CefRefPtr<CefBrowser> browser,
    cef_errorcode_t cert_error,        // ERR_CERT_AUTHORITY_INVALID, etc.
    const CefString& request_url,
    CefRefPtr<CefSSLInfo> ssl_info,    // X.509 cert chain via GetX509Certificate()
    CefRefPtr<CefCallback> callback    // Continue() to proceed, destroy to cancel
) override;
```

- Return `true` + store callback to show "proceed anyway" UI asynchronously
- Return `false` to cancel immediately (strictest security)
- `ssl_info->GetX509Certificate()` gives issuer, subject, validity dates, DER data
- **Note**: API changed from `CefRequestCallback` to `CefCallback` in recent CEF versions

### Permission APIs

CEF 136 uses Chrome bootstrap (Alloy removed in M128). Key handler: `CefPermissionHandler`.

#### `OnShowPermissionPrompt` (geolocation, notifications, etc.)

```cpp
bool OnShowPermissionPrompt(
    CefRefPtr<CefBrowser> browser,
    uint64_t prompt_id,
    const CefString& requesting_origin,
    uint32_t requested_permissions,     // Bitmask of cef_permission_request_types_t
    CefRefPtr<CefPermissionPromptCallback> callback
) override;
```

Permission types include: `GEOLOCATION`, `NOTIFICATIONS`, `CAMERA_PAN_TILT_ZOOM`, `IDENTITY_PROVIDER` (FedCM), `MIDI_SYSEX`, `MULTIPLE_DOWNLOADS`, `STORAGE_ACCESS`, and more.

**Key insight**: Returning `false` from `OnShowPermissionPrompt` with Chrome bootstrap shows Chrome's **native permission bubble UI** for free. This is the simplest initial implementation.

#### `OnRequestMediaAccessPermission` (camera, mic)

```cpp
bool OnRequestMediaAccessPermission(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    const CefString& requesting_origin,
    uint32_t requested_permissions,     // DEVICE_AUDIO_CAPTURE, DEVICE_VIDEO_CAPTURE, etc.
    CefRefPtr<CefMediaAccessCallback> callback
) override;
```

- Return `true` + `callback->Continue(granted_permissions)` to approve specific permissions
- Return `false` for default handling (Chrome bootstrap shows native UI)

#### Integration

```cpp
CefRefPtr<CefPermissionHandler> GetPermissionHandler() override { return this; }
```

### Download Handler

**3 methods** on `CefDownloadHandler`:

1. **`CanDownload`** — Return `true` to allow, `false` to block
2. **`OnBeforeDownload`** — Call `callback->Continue("", true)` to show Save As dialog. **Download is canceled unless you call the callback.**
3. **`OnDownloadUpdated`** — Progress: `GetPercentComplete()`, `GetReceivedBytes()`, `GetCurrentSpeed()`, `IsComplete()`. Can `Cancel()`/`Pause()`/`Resume()`.

All called on UI thread. `CefDownloadItem` references must not be stored outside callbacks.

### Find-in-Page

Two APIs work together:

```cpp
// Start search
browser->GetHost()->Find(identifier, searchText, forward, matchCase, findNext);

// Stop search
browser->GetHost()->StopFinding(clearSelection);
```

Results via `CefFindHandler::OnFindResult`:
```cpp
void OnFindResult(
    CefRefPtr<CefBrowser> browser,
    int identifier, int count,              // Total matches
    const CefRect& selectionRect,
    int activeMatchOrdinal,                  // Current match (1-based)
    bool finalUpdate
) override;
```

**UI**: Find bar is NOT provided by CEF — we build it (text input + "X of Y" counter + prev/next/close). Ctrl+F shows our custom find bar.

### FedCM Support

**Status: No dedicated CEF API exists.** No `CefFedCMHandler`.

- `CEF_PERMISSION_TYPE_IDENTITY_PROVIDER` exists in the enum, suggesting some awareness
- With Chrome bootstrap, FedCM dialogs might partially work but require Chrome-style windows (not our Alloy-style custom chrome)
- Most sites using FedCM also support redirect-based OAuth fallback

**Recommendation**: Low ROI to implement. Test whether x.com/Twitter falls back to redirect OAuth. If not, investigate Chrome-style windows for specific auth flows.

### CefRequestContext (Per-Tab Isolation)

```cpp
CefRequestContextSettings settings;
CefString(&settings.cache_path).FromString(path);  // Empty = incognito
CefRefPtr<CefRequestContext> context = CefRequestContext::CreateContext(settings, nullptr);
CefBrowserHost::CreateBrowser(windowInfo, handler, url, browserSettings, nullptr, context);
```

- Each context has its own cookie store, cache, network state
- Empty `cache_path` = in-memory ("incognito mode")
- Sharing same context shares cookies; separate contexts are fully isolated
- **For private tabs**: Create new context with empty `cache_path`

### Print Handler

**NOT needed on Windows.** CEF automatically uses the native Windows print dialog when `browser->GetHost()->Print()` or `window.print()` is called. The `CefPrintHandler` interface is **Linux-only**.

### Profile Import

**CEF provides NO built-in import mechanism.** All import is manual:
1. Read Chrome's data files directly (SQLite, JSON)
2. For cookies/passwords: decrypt via DPAPI (we have this capability)
3. Map Chrome schemas to our own data structures

---

## B.4 Rust Daemon Architecture

### adblock-rust Deep Evaluation

**Core API**:

```rust
use adblock::{lists::FilterSet, request::Request, Engine};

// Build filter set from multiple sources
let mut filter_set = FilterSet::new(false);
filter_set.add_filter_list(&easylist_text, Default::default());
filter_set.add_filter_list(&easyprivacy_text, Default::default());

// Create compiled engine
let engine = Engine::from_filter_set(filter_set, true);

// Check a network request (~5 microseconds)
let request = Request::new(&url, &source_url, &request_type).unwrap();
let result = engine.check_network_request(&request);
// result.matched, result.redirect, result.exception, result.filter

// Cosmetic filtering (CSS hiding rules)
let resources = engine.url_cosmetic_resources(&page_url);
// resources.hide_selectors, resources.style_selectors

// Serialize compiled engine to disk for fast reload
let bytes = engine.serialize().unwrap();
let engine2 = Engine::deserialize(&bytes).unwrap();
```

**Dependency conflicts with our Cargo.toml**: Very low risk. Only `base64` has a version mismatch (0.13 vs our 0.22) — Cargo handles this by compiling both. Core deps (`serde`, `reqwest`, `regex`, `once_cell`) are compatible versions.

### FFI vs HTTP Latency Analysis

| Approach | Per-request overhead | 100 requests (typical page) |
|----------|---------------------|---------------------------|
| In-process Rust call | ~5 μs | ~0.5 ms |
| FFI via `cxx` crate | ~5-8 μs | ~0.5-0.8 ms |
| HTTP localhost | ~50-500 μs | **5-50 ms** |

**Key insight**: `cxx` FFI overhead is ~3 nanoseconds — 1,000x less than the matching itself. HTTP adds 10-100x overhead due to TCP/serialization.

### Recommended Architecture: FFI Static Library

Build adblock-rust as a **standalone static library linked into C++ CEF process**, separate from the Rust wallet backend. This matches Brave's battle-tested architecture.

```
C++ CEF Process
├── adblock-rust (static lib via FFI)
│   ├── engine_check_url() in GetResourceRequestHandler   ← ~5μs
│   ├── engine_cosmetic_resources() in OnLoadStart
│   └── Engine loaded from serialized file at startup
│
Rust Wallet Process (port 3301)  ← unchanged
└── wallet operations only
```

**Rationale**:
1. **Performance**: 0.5ms vs 5-50ms per page for ad block checks
2. **Threading**: Synchronous FFI calls are fine on IO thread (5μs). HTTP would need async for every resource request
3. **Separation**: Wallet stays focused on security-critical operations
4. **Proven**: This is exactly how Brave does it

**NOT recommended yet**: Cargo workspace refactor. Premature for adding just ad blocking. Revisit when adding a third Rust component.

### Filter List Management

| List | Rules | Size | URL |
|------|-------|------|-----|
| EasyList | ~72,000 | ~1.5 MB | `https://easylist.to/easylist/easylist.txt` |
| EasyPrivacy | ~51,000 | ~1.0 MB | `https://easylist.to/easylist/easyprivacy.txt` |
| Peter Lowe's | ~231,000 | ~4 MB | `pgl.yoyo.org/adservers/...` |
| EasyList Cookie | ~15,000 | ~0.3 MB | `easylist.to/.../easylist-cookie.txt` |

**Standard config** (EasyList + EasyPrivacy): ~123,000 rules, ~2.5 MB text, ~15-25 MB in-memory.

**Update strategy**:
1. **Startup**: Load cached serialized engine if < 24 hours old; otherwise download + compile
2. **Background**: Download fresh lists every 24 hours, compile, serialize, hot-swap
3. **Bundled**: Ship compiled engine snapshot with installer for immediate blocking on first launch
4. **Storage**: Lists in `%APPDATA%/HodosBrowser/adblock/lists/`, compiled engine in `engine.dat`

### CEF Integration Point

Ad blocking hooks into `GetResourceRequestHandler` (same place as our existing wallet HTTP interception):

```cpp
// In GetResourceRequestHandler or OnBeforeResourceLoad:
if (adblock_engine_check(url, source, type)) {
    // Block: set disable_default_handling = true, return nullptr
}
```

For cosmetic filtering: inject CSS via `CefFrame::ExecuteJavaScript()` after page load.

The existing `CookieBlockManager` with 24 hardcoded domains in `DefaultTrackerList.h` would be superseded by EasyPrivacy's 51,000+ tracking rules.

---

## Implementation Priority Summary

### P0 — Critical for MVP

| Feature | CEF API | Effort | Notes |
|---------|---------|--------|-------|
| SSL certificate errors | `OnCertificateError` | Low | Return `false` for strict; or show warning overlay |
| Download handler | `CefDownloadHandler` (3 methods) | Medium | Users can't download without this |
| Permission handler (media) | `OnRequestMediaAccessPermission` | Low | Return `false` → Chrome shows native UI |

### P1 — High Priority

| Feature | Approach | Effort | Notes |
|---------|----------|--------|-------|
| Ad/tracker blocking | `adblock-rust` via FFI static lib | Medium-High | Replaces `DefaultTrackerList.h` |
| Find-in-page | `CefBrowserHost::Find` + custom UI | Medium | Fundamental browser feature |
| WebRTC leak prevention | CEF command-line switch | **Trivial** | One line |
| Permission handler (geo/notif) | `OnShowPermissionPrompt` | Low | Chrome bootstrap handles UI |

### P2 — Medium Priority

| Feature | Approach | Effort | Notes |
|---------|----------|--------|-------|
| Profile import (bookmarks) | JSON parse | Low | Straightforward mapping |
| Profile import (history) | SQLite read | Low | Timestamp conversion |
| Cosmetic filtering | adblock-rust + JS injection | Medium | Hides ad elements |
| Basic fingerprinting protection | V8 injection | Medium | Block 3rd-party Canvas/WebGL/Audio |

### P3 — Nice to Have

| Feature | Approach | Effort | Notes |
|---------|----------|--------|-------|
| Profile import (cookies) | SQLite + DPAPI decrypt | Medium | We have DPAPI infrastructure |
| Private browsing tabs | `CefRequestContext` per tab | Medium | Empty cache_path = incognito |
| Ephemeral 3rd-party cookies | Per-tab storage cleanup | Medium | Brave-style compatibility approach |
| FedCM support | Chrome-style windows | High | No CEF API; low ROI |

### Not Needed

| Feature | Reason |
|---------|--------|
| Print handler | Windows uses native dialog automatically |
| Cargo workspace refactor | Premature; revisit with 3rd Rust component |
| `libadblockplus` (C++) | Embeds V8 — conflicts with CEF, slower, heavier |

---

**End of Document**
