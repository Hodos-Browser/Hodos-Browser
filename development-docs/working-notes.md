# Working Notes — Research & Decisions Backlog

**Purpose**: Track things that come up during sprints but shouldn't derail current work. Each item needs research, a decision, and eventual implementation.

---

## 1. Proprietary Codec Support (AAC, H.264, MP3)

**Discovered**: Sprint 2 testing — `zedvibe.org` fails with "AAC audio codec not supported"

**Problem**: Default CEF binaries from `cef-builds.spotifycdn.com` ship without proprietary codecs (AAC, H.264, MP3) due to patent licensing. Only open codecs included (VP8, VP9, AV1, Opus, Vorbis). This is a compile-time flag (`proprietary_codecs=true`, `ffmpeg_branding="Chrome"`), not a runtime toggle.

**Impact**: Most audio/video streaming sites broken. YouTube works (VP9/AV1 fallback) but most other sites don't.

**Options to research**:
- Does Spotify CDN offer a CEF 136 build variant with proprietary codecs enabled?
- Can we swap in a proprietary-enabled binary without rebuilding the CEF wrapper?
- Should we build CEF from source with the flag? (Hours of compile time, ~50GB disk)
- Patent/licensing implications for distributing a browser with proprietary codecs

**Decision needed**: Which option, and when to implement (before or after MVP launch)

---

## 2. CEF Binary Strategy — Prebuilt vs Source Build

**Context**: Currently using prebuilt CEF binaries from `cef-builds.spotifycdn.com`. Need to decide on long-term strategy.

**Questions to research**:
- **Version selection**: Always use newest CEF/Chromium? Pin to LTS? What's the upgrade cadence? How do we track security patches?
- **Prebuilt vs source**: Prebuilt is fast but limited (no proprietary codecs, no custom patches). Source build gives full control but is heavy (~50GB, hours to compile). What do other CEF-based browsers (Brave, Vivaldi, etc.) do?
- **Wrapper compatibility**: When upgrading CEF versions, what breaks? Does the `libcef_dll_wrapper` need rebuilding? Do API signatures change between versions?
- **Testing**: How to validate a new CEF version doesn't break existing functionality? Need a regression test checklist (media playback, WebRTC, SSL, permissions, cookie blocking, HTTP interception, V8 injection).
- **Full Chromium build**: Is it worth building Chromium directly (like Brave does) instead of using CEF? Pros: full control, all codecs, custom patches. Cons: massive build infrastructure, ongoing merge burden.

---

## 3. Production Installation & Distribution

**Questions to research**:
- **Installer format**: MSI? NSIS? WiX? MSIX (Windows Store)? What do other browsers use?
- **Code signing**: Need a code signing certificate for Windows (SmartScreen warnings without one). Cost, providers, process.
- **Installation directory**: `Program Files` vs `AppData\Local`? Chrome installs per-user in AppData. Affects auto-update permissions.
- **Uninstaller**: What needs cleanup? Registry entries, file associations, default browser registration.
- **macOS**: DMG with drag-to-Applications? Notarization with Apple? Separate research item.

---

## 4. Auto-Update Mechanism

**Questions to research**:
- **Update delivery**: How to push updates to users who have downloaded the browser?
- **Update protocol**: Squirrel (Electron-style)? Google's Omaha/Sparkle? Custom solution? What does Brave use?
- **Differential updates**: Full download vs delta patches? CEF binaries are ~100MB+.
- **Update cadence**: Tied to CEF/Chromium releases? Independent app updates?
- **Rollback**: If an update breaks something, can users roll back? How?
- **Deployment infrastructure**: Dedicated GitHub repo for releases? S3/CDN for binaries? GitHub Releases has file size limits (2GB).
- **Channels**: Stable, beta, canary? Worth the complexity for MVP?

---

## 5. Open Source Dependency Updates (CEF, Ad-blocker, etc.)

**Context**: We depend on CEF binaries, and plan to use Brave's open-source ad-blocker (`adblock-rust`). These dependencies will need periodic updates.

**Questions to research**:
- **CEF upgrades**: How to upgrade without breaking the wrapper, HTTP interception, V8 injection, or other customizations? What's the typical breakage surface between CEF versions?
- **Ad-blocker filter lists**: How are EasyList/EasyPrivacy/etc. updated? At build time? At runtime (download on schedule)? Where are they stored? How does Brave handle this?
- **Data migration**: When upgrading CEF, does the profile data format (`%APPDATA%/HodosBrowser/Default/`) change? Could an upgrade corrupt cookies, history, permissions, or bookmarks?
- **Wallet DB**: Our SQLite wallet DB has its own migration system. CEF upgrades shouldn't affect it, but need to verify isolation.
- **Breaking changes**: How to detect if a CEF API we use was removed or changed signature? Can we pin to a CEF API version?
- **Dependency pinning**: Lock exact versions in build scripts? Use ranges? How to balance security patches vs stability?

---

## 6. User-Agent String

**Discovered**: Sprint 2 — discussing Google login compatibility

**Context**: Google and other sites check the user-agent string to decide whether to allow login. CEF includes a Chrome-like UA but may append custom identifiers.

**Questions to research**:
- What is our current UA string? (Check in DevTools or debug logs)
- Does it include "HodosBrowser" or other non-standard identifiers that trigger blocks?
- Should we match Chrome's UA exactly? Legal/ethical considerations?
- Chrome is deprecating the UA string in favor of Client Hints — do we need to support `Sec-CH-UA` headers?

---

## 7. CEF Wrapper (`libcef_dll_wrapper`) — Rebuild & macOS Readiness

**Discovered**: Sprint 4 (Find-in-Page) — `CefBrowserHost::Find()` silently no-ops, `GetFindHandler()` never called

**Root Cause**: The wrapper's `CMakeCache.txt` contained a stale path (`D:\BSVProjects\Browser-Project\Babbage-Browser\...`) from a previous machine/location. All wrapper rebuilds silently output to the old (nonexistent) path, so the `.lib` hadn't been updated in 5 months. Fresh `cmake` reconfigure fixed it — all 175 source files now compile correctly (60 cpptoc + 70 ctocpp + 15 views ctocpp + 7 views cpptoc + base/wrapper/utils).

**CEF Find() API still non-functional**: Even after the wrapper rebuild, `CefBrowserHost::Find()` does not trigger `GetFindHandler()` or `OnFindResult` callbacks. The reason is unknown — possibly a CEF 136 regression or windowed-mode limitation. Sprint 4 was completed using a JavaScript-based fallback (`window.find()` + DOM counting).

**Action items**:
- [ ] **Investigate CEF Find() API**: Test with the cefclient sample app to determine if this is a CEF 136 bug or specific to our setup. If cefclient's find works, diff their handler registration against ours.
- [ ] **Wrapper CMakeLists.txt — macOS readiness**: Current `wrapper/CMakeLists.txt` is Windows-only (`MSVC_RUNTIME_LIBRARY`, `WIN32_LEAN_AND_MEAN`, `NOMINMAX`). Needs platform conditionals:
  - Wrap `MSVC_RUNTIME_LIBRARY` in `if(MSVC)`
  - Wrap `WIN32_LEAN_AND_MEAN`/`NOMINMAX` in `if(WIN32)`
  - Add macOS deployment target (`-mmacosx-version-min=10.15`)
- [ ] **macOS wrapper rebuild required**: The wrapper MUST be rebuilt on macOS (static `.a` is platform-specific). The wrapper source is pure C++ and compiles cleanly cross-platform.
- [ ] **macOS wrapper path**: Main `cef-native/CMakeLists.txt` already has separate paths (Windows: `cef-binaries/libcef_dll/wrapper/build/Release/`, macOS: `cef-binaries/build/libcef_dll_wrapper/`). Verify these are correct when setting up macOS.
- [ ] **Never let CMakeCache go stale again**: If the project is moved/cloned to a new location, the wrapper CMakeCache must be deleted and reconfigured. Add this to onboarding/setup docs.

---

## 8. CEF Built-In Menu Command IDs — Auto-Disable Quirk

**Discovered**: Sprint 5 (Context Menu Enhancement)

**Problem**: When building a custom context menu by calling `model->Clear()` then re-adding items using CEF's built-in command IDs (`MENU_ID_BACK`, `MENU_ID_COPY`, `MENU_ID_PASTE`, `MENU_ID_SELECT_ALL`, etc. from `cef_types.h`), CEF's internal command state manager auto-disables them. All items appear greyed out and unclickable, even though they were explicitly added.

**Root Cause**: CEF ties its built-in menu IDs to internal "command updater" infrastructure inherited from Chromium. This infrastructure manages enabled/disabled state based on browser state (e.g., clipboard contents, selection, navigation history). When the menu model is cleared and rebuilt, the state tracking gets out of sync — it sees the IDs but doesn't recognize them as freshly-added, so it applies stale disabled state.

**Fix**: Use custom command IDs in the `MENU_ID_USER_FIRST` (26500+) range for ALL menu items and handle every command manually in `OnContextMenuCommand`. Navigation: `browser->GoBack()`, `browser->GoForward()`, `browser->Reload()`. Editing: `frame->ExecuteJavaScript("document.execCommand('copy')")`, etc. This gives us full control and avoids CEF's internal state management entirely.

**Impact on future work**:
- If we upgrade CEF binaries or build from source, this behavior may or may not change — worth re-testing with newer CEF versions.
- If CEF ever fixes this (or if we find a different `model->Clear()` alternative), we could switch back to built-in IDs to get automatic state management for free.
- The cefclient sample app does NOT call `model->Clear()` — it appends to the default menu, which is why their built-in IDs work. Our approach (full custom menu) is fundamentally different.

---

## 9. Dev Environment vs Production / Multi-Instance Behavior

**Discovered**: Sprint 6 — user noticed Ctrl+H and Ctrl+J open separate Chromium windows (chrome://history, chrome://downloads) rather than our custom UI. DevTools also opens in a separate window.

**Questions to investigate**:
- **Separate windows**: Chromium's built-in shortcuts (Ctrl+H, Ctrl+J, Ctrl+D) open `chrome://` pages in new Chromium windows because we don't intercept them. We need to intercept these in `OnPreKeyEvent` and route to our own UI. DevTools opening in a separate window is normal CEF behavior (not a dev-only thing).
- **Multiple instances**: Can the user run multiple instances of HodosBrowserShell.exe simultaneously? CEF uses a `cache_path` for the profile — if two instances share the same path, the second instance may fail or corrupt data (SQLite locking on wallet.db, CEF cookie DB conflicts). Need to either: (a) detect and block second instance (single-instance lock file/mutex), or (b) support multiple profiles with separate data directories.
- **Dev vs production differences**: In production, the browser won't connect to `localhost:5137` (frontend dev server). Instead, the React build will be served from bundled files (or an embedded server). Need to decide: embed static files via `CefRegisterSchemeHandlerFactory` (custom scheme), or bundle a lightweight HTTP server? This affects how `http://127.0.0.1:5137/history` URLs work in production.
- **Installation considerations**: See working-notes.md #3 (installer format, code signing) and #4 (auto-update). Additionally: how do we handle CEF subprocess executables in the installer? CEF requires specific helper processes alongside the main exe.
- **Default browser registration**: Should we register as a default browser candidate? What system hooks are needed (Windows: registry entries, protocol handlers)?

**Priority**: Post-MVP, but worth keeping in mind during development to avoid assumptions that break in production.

---

## 10. Ad & Tracker Blocking — Architecture & Implementation Notes

**Discovered**: Sprint 8 research (2026-02-22). **Implemented**: Sprint 8 Phases 8a-8b (2026-02-23).

**See also**: `ci-cd-testing-strategy.md` Section 7 (architectural decision), Section 9j (tests), `sprint-8-adblock-research.md` (full design doc).

### Architecture Decision

**Separate standalone project** at `adblock-engine/` — NOT inside `rust-wallet/` or a workspace. Runs as independent process on port 3302 (wallet is port 3301). C++ starts it via `CreateProcessA` + Job Object. Non-critical: if it fails, browsing continues unblocked.

### Key Implementation Details

**Crate version pinning** (critical):
- `adblock = "=0.10.3"` — last version compatible with stable Rust 1.85.1. v0.10.4+ uses unstable `unsigned_is_multiple_of` (needs Rust 1.87+)
- `rmp = "=0.8.14"` — required for rmp-serde 0.15 compat with adblock 0.10.x
- `actix-web = "=4.11.0"` — 4.13+ requires Rust 1.88
- `default-features = false` — disables `unsync-regex-caching` feature to enable `Send+Sync` for `RwLock<Engine>` (in v0.10.x the feature name is `unsync-regex-caching`, NOT `single-thread` as in newer versions)
- `engine.serialize()` — NOT `serialize_raw()` (that's a newer API)

**C++ integration**:
- `AdblockCache.h` (header-only singleton): URL→bool cache + sync WinHTTP POST to `/check`
- `AdblockBlockHandler`: `CefResourceRequestHandler` returning `RV_CANCEL`
- Hook in `GetResourceRequestHandler()` BEFORE wallet interception
- `shouldSkipAdblockCheck()`: skips localhost, data:, blob:, chrome:, devtools: URLs
- macOS stubs: `fetchFromBackend()` returns false, `StartAdblockServer()`/`StopAdblockServer()` need `#elif defined(__APPLE__)` implementation

**Two-phase startup**: HTTP server starts immediately (/health returns "loading"), engine loads async in background (deserialize engine.dat or download filter lists). C++ health poll checks for `"ready"`.

**Unit tests**: 9 tests in `adblock-engine/src/engine.rs` — run with `cargo test --manifest-path adblock-engine/Cargo.toml`.

### Brave's Approach (for reference)

- Uses Chromium's **Component Updater** system (CRX packages, signed, distributed via S3)
- Checks for filter list updates every **~5 hours** via `go-updater.brave.com`
- Key repos: `brave/adblock-rust`, `brave/adblock-resources` (list catalog), `brave/adblock-lists` (brave-specific rules)
- Major release every ~4 weeks mapped 1:1 to Chromium milestones

### Our Approach (simpler)

- Fetch lists directly from upstream URLs:
  - EasyList: `https://easylist.to/easylist/easylist.txt` (expires: 4 days)
  - EasyPrivacy: `https://easylist.to/easylist/easyprivacy.txt` (expires: 4 days)
- Store raw lists in `%APPDATA%/HodosBrowser/adblock/lists/`
- Compile with `adblock::Engine`, serialize to `engine.dat` for fast startup
- Background task checks for updates every 6 hours (Phase 8d — not yet implemented)

### Open Questions for Future Research

- Do we need our own "unbreak" list (like Brave's `brave-unbreak.txt`)?
- Should we bundle pre-compiled `engine.dat` with the installer?
- How do we handle the crate's serialization format changing between versions?
- Do we ever want cosmetic filtering (CSS element hiding)? Network blocking covers ~90% of ads.

---

*Add new items below as they come up during sprints.*
