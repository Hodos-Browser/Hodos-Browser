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

### Real-World Impact (confirmed 2026-02-27)

Testing confirms proprietary codecs are the **root cause** of media failures on multiple sites:

| Site | Static Images | Video | Root Cause |
|------|--------------|-------|------------|
| **x.com** | ✅ JPEG/PNG work | ❌ No playback (0:00, blank) | x.com converts ALL animated GIFs to H.264 MP4 (`<video>` elements). HLS via MSE requires H.264. No fallback codec. |
| **Reddit** | ✅ Thumbnails work | ❌ Spinner, never plays | Reddit video player uses H.264 MP4. No VP9/AV1 fallback. |
| **YouTube** | ✅ All work | ✅ Plays fine | YouTube has VP9/AV1 fallback — doesn't need H.264. |

**x.com "intermittent images" explained**: Static JPEG/PNG images load fine. But x.com converts animated GIFs to MP4 and renders them as `<video>` elements — these appear as broken "images" because the H.264 codec is missing. The "intermittent" behavior is actually: real `<img>` tags work, `<video>` tags (disguised as images) don't.

**Diagnostic command** (run in DevTools console on any page):
```javascript
const v = document.createElement('video');
console.log('H.264:', v.canPlayType('video/mp4; codecs="avc1.42E01E"'));
console.log('VP9:', v.canPlayType('video/webm; codecs="vp9"'));
console.log('AV1:', v.canPlayType('video/webm; codecs="av01.0.01M.08"'));
console.log('AAC:', v.canPlayType('audio/mp4; codecs="mp4a.40.2"'));
// H.264 and AAC will be empty string (unsupported), VP9/AV1 will be "probably"
```

**Priority**: This is the **single biggest UX issue** for the browser. Sites that don't work without H.264: x.com (videos+GIFs), Reddit (videos), Twitch (some streams), Instagram, TikTok, most news sites with embedded video. Should be addressed before MVP launch.

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

**Status**: ⚠️ CRITICAL BUG FOUND — Cookie/cache isolation is broken. See details below.

**See**: `development-docs/browser-core/multi-instance-profile-testing.md` for testing strategy.

### Critical Bug: CefSettings.cache_path Not Profile-Aware (found 2026-02-25)

In `cef_browser_shell.cpp`, `CefSettings.cache_path` is hardcoded to `"HodosBrowser\\Default"` at line 2115 **before** `CefInitialize()` at line 2276. The `--profile=` argument is parsed **after** `CefInitialize()` at line 2290. This means **CEF's built-in cookie store, localStorage, IndexedDB, and HTTP cache all go to the `Default` directory regardless of which profile is selected.**

**Impact**: Profiles do NOT provide true cookie isolation. Logging into x.com on Profile A means you're also logged in on Profile B. Only custom managers (HistoryManager, BookmarkManager, CookieBlockManager) are actually profile-isolated.

**Fix**: Parse `--profile=` argument BEFORE `CefInitialize()` and set `CefSettings.cache_path` to the profile-specific directory:
```cpp
// Parse --profile= FIRST (before CefInitialize)
std::string profileId = ProfileManager::ParseProfileArgument(GetCommandLineW());
ProfileManager::GetInstance().Initialize(user_data_path);
ProfileManager::GetInstance().SetCurrentProfileId(profileId);
std::string profile_cache = ProfileManager::GetInstance().GetCurrentProfileDataPath();

CefString(&settings.root_cache_path).FromString(user_data_path);
CefString(&settings.cache_path).FromString(profile_cache);  // Profile-specific!

CefInitialize(main_args, settings, app, nullptr);
```

Since each profile already launches a separate OS process, this fix naturally isolates all CEF data per profile.

### Summary of decisions

- **Multiple instances**: Use Chrome's model — each profile runs as separate process. Profile switching launches `HodosBrowserShell.exe --profile="Name"`. Implemented in Sprint 9d.
- **Profile locking**: Add lock file per profile (`FILE_FLAG_DELETE_ON_CLOSE`) to prevent same profile running twice. NOT YET IMPLEMENTED — risk of SQLite corruption if two instances share a profile.
- **Startup profile picker**: Show picker before main browser if user has 2+ profiles and didn't check "Remember my choice".
- **Dev vs Stage vs Prod testing**: Add "Stage mode" with `HODOS_USE_BUNDLED=1` env var to test bundled files without full installer.

### Shared Services Across Profile Instances

When multiple profile instances are running simultaneously, they share:
- **Wallet backend** (port 3301) — first instance starts it, subsequent instances must detect it's already running (not start a second one). Currently uses `CreateProcessA` + Job Object — need to check if second launch attempt fails gracefully or crashes.
- **Adblock engine** (port 3302) — same concern. `StartAdblockServer()` should check if port is already bound before launching.
- **Frontend dev server** (port 5137, dev only) — shared, no conflict since it's read-only.

### Dev Environment Testing for Multiple Instances

**Dev mode (current)**:
- All profile instances connect to the same Vite dev server on `localhost:5137` — fine for UI development.
- Profile switching spawns new `HodosBrowserShell.exe` process — works but doesn't test bundled frontend.
- **Test gap**: No way to verify cookie isolation in dev mode due to the `CefSettings.cache_path` bug above.

**Stage mode (recommended, not yet implemented)**:
```powershell
# Build frontend to static files
cd frontend && npm run build

# Run with bundled flag
$env:HODOS_USE_BUNDLED = "1"
cd cef-native/build/bin/Release

# Instance 1 (Profile A)
./HodosBrowserShell.exe --profile="Default"

# Instance 2 (different profile, different terminal)
./HodosBrowserShell.exe --profile="Work"
```

**Multi-instance test checklist** (after fixing cache_path bug):
1. Log into x.com on Profile A → verify logged in
2. Open Profile B → navigate to x.com → verify NOT logged in (must be separate session)
3. Log into x.com with different account on Profile B → verify different account
4. Close Profile B → reopen Profile B → verify Profile B's x.com session persists
5. Verify Profile A's x.com session is unaffected
6. Verify history is separate (visit youtube.com on A, check Ctrl+H on B — shouldn't appear)
7. Verify bookmarks are separate
8. Stress test: open both profiles, browse actively on both simultaneously

**Original questions (resolved)**:
- **Separate windows**: ✅ Fixed in Sprint 6 — Ctrl+H/J/D now open our overlays.
- **Multiple instances**: ✅ Decided — Chrome model with `--profile=` arg
- **Dev vs production**: ✅ Planned — Stage mode for testing bundled behavior
- **Cookie isolation**: ❌ BROKEN — CefSettings.cache_path bug (see above)
- **Installation considerations**: Still pending (see #3, #4)
- **Default browser registration**: Still pending — post-MVP

**Priority**: Fix cache_path bug FIRST (blocks all profile testing), then Phase 1 (Stage mode) before Sprint 10.

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

- ~~Do we need our own "unbreak" list (like Brave's `brave-unbreak.txt`)?~~ **YES** — implemented as `hodos-unbreak.txt` (Sprint 10). Contains `#@#+js()` scriptlet exceptions + `$generichide` cosmetic exceptions for auth/banking/e-commerce domains, plus global OAuth SDK network exceptions.
- Should we bundle pre-compiled `engine.dat` with the installer?
- How do we handle the crate's serialization format changing between versions?
- ~~Do we ever want cosmetic filtering (CSS element hiding)?~~ **YES** — implemented in Sprint 8e. Two-phase: hostname-specific CSS selectors on page load, generic selectors after DOM class/ID collection. Suppressed per-domain via `$generichide` rules in `hodos-unbreak.txt`.

### adblock-rust Quirks (discovered 2026-02-27)

- **`$elemhide` NOT supported in adblock-rust 0.10.3**: The `elemhide` option (which should suppress all cosmetic filtering) does NOT set the `generichide` flag in `UrlSpecificResources`. Only `$generichide` works. This is a known limitation — PR #587 in the adblock-rust repo is still open. All rules in `hodos-unbreak.txt` use `$generichide` instead of `$elemhide`.
- **`cosmetic_resources()` Phase 1 fix**: The `url_cosmetic_resources()` API returns hostname-specific CSS selectors in `hide_selectors` even when `generichide=true`. Our `cosmetic_resources()` wrapper now explicitly returns empty selectors when `generichide=true`, suppressing both Phase 1 (hostname-specific) and Phase 2 (generic) CSS. See `engine.rs` `cosmetic_resources()` function. `CONFIG_VERSION` bumped to 6.

---

## 11. Settings Functionality — Making Settings Actually Work

**Discovered**: Sprint 9a (2026-02-25) — Settings persistence works, but settings don't affect behavior yet.

**Context**: Sprint 9a added settings persistence (JSON file + UI), but the settings are just stored — they don't actually change browser behavior. Each setting needs backend integration:

| Setting | What It Should Do | Complexity |
|---------|-------------------|------------|
| `homepage` | New tab opens this URL | Low — wire into new tab creation |
| `searchEngine` | Omnibox uses this for searches | Medium — update GoogleSuggestService + search URL |
| `zoomLevel` | Default zoom for all pages | Low — `CefBrowserHost::SetZoomLevel()` |
| `showBookmarkBar` | Toggle bookmark bar visibility | Medium — UI component, needs bookmark bar first |
| `downloadsPath` | Where downloads go | Low — pass to DownloadHandler |
| `restoreSessionOnStart` | Reopen last tabs on launch | Medium — persist tab URLs, restore on startup |
| `adBlockEnabled` | Toggle ad blocking | Low — flag in AdblockCache check |
| `thirdPartyCookieBlocking` | Block third-party cookies | Low — flag in CookieBlockManager |
| `doNotTrack` | Send DNT header | Low — modify request headers |
| `clearDataOnExit` | Clear history/cache/cookies on close | Medium — call clear functions in OnBeforeClose |
| `autoApproveEnabled` | Toggle wallet auto-approve | Low — flag in SessionManager |
| Spending limits | Enforce limits in auto-approve flow | Low — SessionManager already has this |

**Scope**: This is ~1-2 days of work spread across multiple systems. Not MVP-blocking but important for user expectations.

**Recommendation**: Post-MVP, or as a dedicated sprint after the current browser-core sprints (10-12) complete. Could batch with Sprint 11 (Menu UX) since both touch settings UI.

**See also**: Sprint 9 implementation plan (`sprint-9-implementation-plan.md`)

---

## 12. Multi-Window Same-Profile Support & Instance Management

**Discovered**: Sprint 9d testing (2026-02-25) — Profile lock prevents a second instance from opening the same profile.

**Current behavior**: Each profile runs as a separate OS process with its own `CefInitialize`, `root_cache_path`, and SingletonLock. If you try to launch a second instance with the same `--profile=` argument, a "Profile Locked" error dialog appears and the second process exits. This is correct for data integrity but bad UX.

**What Chrome does (the target UX)**:

1. **Single process per profile, multiple windows**: Chrome runs one process per profile. Opening a second window with the same profile doesn't launch a new `chrome.exe` — it tells the existing process to create another top-level window. All windows share one cookie store, one set of tabs, one CefInitialize.

2. **Instance detection via IPC**: When you run `chrome.exe --profile-directory="Profile 1"` and that profile is already running, Chrome's startup code:
   - Checks for a named pipe / Unix socket at a well-known path (e.g., `<profile_dir>/SingletonSocket`)
   - Sends a message to the existing instance: "open a new window" (or "open these URLs")
   - The existing instance receives the message, creates a new top-level HWND, and optionally brings it to the foreground
   - The launcher process exits silently (no error dialog)

3. **Tab drag-out**: Dragging a tab out of a window creates a new top-level HWND within the same process. The tab's browser instance is re-parented to the new window. This is a natural extension of multi-window support.

**Implementation plan for Hodos Browser**:

### Phase 1: Multi-window within same process (Medium effort)
- Add a `WindowManager` alongside `TabManager` that tracks multiple top-level HWNDs
- Each window has its own header browser, tab bar, and set of tabs
- All windows share the same CefInitialize, cookie store, HistoryManager, etc.
- IPC message `create_new_window` from React opens a new window
- Keyboard shortcut: Ctrl+N opens new window (currently opens new tab)
- Closing the last window triggers CefShutdown

### Phase 2: Single-instance detection (Medium effort)
- On startup, BEFORE showing the lock error:
  - Check if a named pipe exists at `<profile_dir>/hodos_instance_pipe` (Windows) or Unix socket (macOS)
  - If it exists, connect and send a JSON message: `{"action": "new_window"}` or `{"action": "open_urls", "urls": [...]}`
  - If the pipe responds with success, the launcher process exits cleanly (no error)
  - If the pipe is dead/stale, delete it and proceed with normal startup
- The running instance listens on the named pipe in a background thread
  - On receiving `new_window`: calls `WindowManager::CreateWindow()` on the UI thread via `CefPostTask`
  - On receiving `open_urls`: creates a new window with tabs for each URL

### Phase 3: Tab drag-out (High effort, post-MVP)
- Detect tab drag beyond window bounds
- Create a new top-level HWND at the drop position
- Re-parent the tab's CEF browser to the new window
- This is complex because CEF doesn't natively support re-parenting windowed browsers — may need to destroy and recreate

**Files that will need changes**:
- `cef_browser_shell.cpp` — named pipe listener, WindowManager integration, multi-HWND management
- `include/core/TabManager.h` / `TabManager.cpp` — scope tabs per-window instead of global
- New: `include/core/WindowManager.h` / `WindowManager.cpp` — track multiple top-level windows
- `simple_handler.cpp` — OnBeforeClose needs to handle "close window but keep process running"
- `frontend/src/pages/MainBrowserView.tsx` — Ctrl+N shortcut, window-aware tab management

**Priority**: Post-MVP. The current lock behavior is safe and prevents data corruption. The UX improvement is important but not blocking.

---

## 13. Exception List Auto-Update via CDN (Post-MVP)

**Discovered**: Sprint 10 planning (2026-02-25)

**Context**: Sprint 10 creates `hodos-unbreak.txt` — a scriptlet exception filter list embedded in the adblock-engine binary via `include_str!()`. It also checks for an updatable version at `%APPDATA%/HodosBrowser/adblock/hodos-unbreak.txt` (local override).

**Current approach**: Embedded list is sufficient for MVP. When new broken auth domains are discovered, they require an adblock-engine rebuild to update the embedded list (or manual file placement for the local override).

**Post-MVP enhancement**: Add the exception list to the existing filter list auto-update cycle (6-hour background task in `adblock-engine/src/engine.rs`). Implementation:

1. Host `hodos-unbreak.txt` at a stable URL (e.g., `https://updates.hodosbrowser.com/adblock/hodos-unbreak.txt` or a GitHub Pages/raw GitHub URL)
2. Add the URL to the `FILTER_LISTS` array in `engine.rs` alongside EasyList, EasyPrivacy, uBlock Filters, and uBlock Privacy
3. The existing `needs_update()` / `rebuild_engine()` infrastructure handles the rest — version checking, downloading, engine hot-swap under `RwLock`
4. Include `! Expires: 7 days` header so it updates weekly (less frequent than the 4-day filter lists since exception rules change rarely)
5. Keep the embedded `include_str!()` fallback for offline/first-run scenarios

**Infrastructure needed**: A static file hosting endpoint. Options:
- **GitHub Pages** (free, version-controlled, auto-deploys on push) — simplest
- **GitHub raw URL** (`raw.githubusercontent.com/...`) — even simpler but has CDN caching delays
- **S3 + CloudFront** — proper CDN with cache invalidation, cost ~$1/month for our traffic
- **Cloudflare Pages** (free tier) — fast CDN, auto-deploy from repo

**Priority**: Low — implement when we set up the browser update infrastructure (see #4). The same CDN/hosting can serve both browser updates and filter list updates.

---

## 14. Entity-Aware Ad Blocking (disconnect.me Entity Lists)

**Implemented**: 2026-02-27

**Context**: EasyList/EasyPrivacy rules block same-organization CDN domains as "trackers" (e.g. `pbs.twimg.com` on `x.com`, `gstatic.com` on `google.com`). This kills images, videos, and auth flows even when the resources are first-party CDN content. Manual `@@||domain^$domain=` exceptions don't scale.

**Solution**: Entity-aware blocking using the disconnect.me `entities.json` file (~400KB, 1859 organizations, 9500+ domains). When the adblock engine says "block", we check whether the URL domain and source domain belong to the same organization. If same-entity → allow (first-party CDN). If different-entity → block (real tracker).

**Implementation details**:
- `EntityMap` struct in `engine.rs` with `HashMap<String, u16>` (domain → entity_id)
- Suffix-walking lookup: `pbs.twimg.com` → tries `pbs.twimg.com`, then `twimg.com` → matches X Corp entity
- Embedded via `include_str!("entities.json")`, auto-updated alongside filter lists every 6 hours
- Only overrides **blocks**, not redirects (redirects are scriptlet-related)
- Removed 13 manual same-entity network exception lines from `hodos-unbreak.txt`
- `CONFIG_VERSION` bumped to 5 for entity-aware blocking, then to 6 for cosmetic CSS fix (forces engine.dat rebuild)
- 7 new unit tests covering entity matching, subdomain walking, and integration with `check_request()`

**Production build note**: The `entities.json` file (~400KB) is embedded at compile time. This adds ~400KB to the binary size but avoids a runtime dependency on disk. The file is also downloaded to `%APPDATA%/HodosBrowser/adblock/entities.json` for runtime updates — the disk version takes priority over embedded if present and valid.

**License**: disconnect.me entity lists are CC BY-NC-SA 4.0. Include attribution in the About page (same as Brave does).

**Update frequency**: The entity list is very stable (updates every few weeks). Piggybacks on the existing 6-hour filter list update cycle. No separate timer needed.

---

## 15. x.com Media Debugging — Lessons Learned (2026-02-27)

**Summary**: Extended debugging session to fix x.com media (images/videos not displaying). Found and fixed two real bugs, but the primary issue was proprietary codecs (item #1).

### What was fixed

1. **Cosmetic CSS Phase 1 suppression** (real bug): `cosmetic_resources()` in engine.rs returned hostname-specific CSS selectors even when `generichide=true`. EasyList has selectors that match organic tweet containers, not just ads. Fixed by clearing selectors when `generichide=true`.

2. **`$elemhide` → `$generichide` in hodos-unbreak.txt** (real bug): adblock-rust 0.10.3 doesn't support `$elemhide` (only `$generichide` sets the flag). Changed all 16 rules.

3. **Entity-aware blocking** (implemented in prior session): disconnect.me entity list prevents same-org CDN blocking (twimg.com on x.com, gstatic.com on google.com).

### What was NOT the problem

All of these were investigated and ruled out:
- **Network blocking**: `debug_output.log` showed all pbs.twimg.com/media requests going through (200 OK). Entity-aware blocking working correctly.
- **Cookie blocking**: CookieBlockManager only blocks cookies, not requests. EphemeralCookieManager correctly allowing third-party cookies.
- **CORS**: Regular `<img>` tags don't require CORS. CEF handles CORS natively.
- **Fingerprint protection**: x.com is in `IsAuthDomain()` — fingerprint farbling skipped.
- **Scriptlet injection**: x.com has `#@#+js()` exception — no scriptlets injected.
- **CefResponseFilter**: Only applies to YouTube (via `CookieFilterResourceHandler`), not x.com.

### The actual root cause

**Missing proprietary codecs (H.264/AAC)** — see item #1 for full details. x.com converts GIFs to H.264 MP4 rendered as `<video>` elements. Without H.264 support, these appear as missing "images." True static images (JPEG/PNG) always worked fine.

### Debugging approach for future media issues

1. Check `debug_output.log` for "Blocked by adblock" lines for the domain's CDN resources
2. Check DevTools Network tab for failed/blocked requests (filter by domain)
3. Run `canPlayType()` diagnostic in console (see item #1) to confirm codec support
4. Check if the "image" is actually a `<video>` element (inspect element → look for `<video>` tags)
5. Check cosmetic CSS: look for `css=N` in debug log — if N>0, cosmetic selectors may be hiding content

---

*Add new items below as they come up during sprints.*
