# 0.4.0 — macOS Port Delta Log

> **Purpose:** Windows-first execution. As each Windows change lands, its macOS-parity delta is recorded here. When the **Mac sprint** starts, pull this doc and implement straight from it, then run Mac smoke + Mac B1-VERIFY.
>
> **How it's filled:** step 6 of the per-chunk harness lifecycle (`ORCHESTRATION_PLAN_0_4_0.md` §1) + a periodic mac-parity sweep workflow (§6).
>
> **Created 2026-06-17. Status: skeleton — populated as Windows work lands.**

---

## How to use this doc (Mac-sprint agent boot)

1. Read `ORCHESTRATION_PLAN_0_4_0.md` §6 + this whole log.
2. For each entry below, open the cited Windows file:fn and the Mac counterpart, implement the delta.
3. Honor CLAUDE.md Invariant #9 (platform conditionals) — Mac code lives in `*_mac.mm` / `#elif defined(__APPLE__)`.
4. Run Mac smoke (Authentication + Video/Media + News categories) + the B1 cross-session login test on macOS.

## Known macOS parity anchors (from prior review)

- Overlays: macOS uses `NSPanel` + `NSWindowDelegate` (`cef_browser_shell_mac.mm`), NOT `WS_POPUP`. Any new overlay needs a Mac creation fn.
- Tabs/windows: `TabManager_mac.mm`, `WindowManager_mac.mm` mirror the Windows APIs.
- HTTP singletons: macOS uses libcurl (`*_mac.cpp` / `SyncHttpClient` libcurl path), not WinHTTP.
- Auto-update: macOS = Sparkle (EdDSA already); Windows = WinSparkle (DSA→EdDSA this sprint, Q9). Mac side mostly unchanged — verify appcast-decouple (Q13) applies to both.

---

## Delta log

> Format per entry:
> ### <chunk id> — <short title> (date, Windows commit)
> - **Windows change:** `file:fn` — what changed.
> - **Mac equivalent:** `file_mac.mm:fn` (or `#elif __APPLE__` block) — what to do.
> - **Risk / notes:** platform-specific gotchas, test to run.

### Wave 0 — secret-log removal (2026-06-17, branch `0.4.0`)
- **Windows change:** `WalletService.cpp::createWallet` — deleted mnemonic `std::cout`. Plus Rust deletions in `crypto/brc2.rs`, `certificate/verifier.rs`, `handlers/certificate_handlers.rs`, `handlers.rs`.
- **Mac equivalent:** **None required.** `WalletService_mac.cpp` (libcurl) never logged the mnemonic — swept all `*_mac.*` + `*.mm` for secret `cout`/`NSLog`/`os_log`, zero siblings. Rust is single cross-platform source (no `_mac` variant).
- **Risk / notes:** Nothing to port. Verified by grep over `*_mac.*` and `*.mm`.

### Wave 0 follow-up — AddressHandler phantom-`privateKey` removal (2026-06-17, branch `0.4.0`)
- **Windows change:** `AddressHandler.cpp` (delete phantom `privateKey` cout + V8 `SetValue`), `simple_app.cpp:479` (legacy injected debug-JS), `frontend/src/types/address.d.ts:4` (type field).
- **Mac equivalent:** **None required.** `AddressHandler.cpp` and `simple_app.cpp` are single cross-platform files; injected JS + TS type are platform-agnostic. No Mac-specific address-gen path.
- **Risk / notes:** Zero functional impact — the `privateKey` field is never returned by Rust nor consumed by JS (phantom).

### Wave 1 Track A — F7 backup/restore path-traversal + internal-only gate; F9 cert malformed-fields panic (2026-06-18, branch `0.4.0`)
- **Windows change:** Pure Rust (platform-agnostic backend). `backup.rs` (`backups_dir_for_db`, `lexical_normalize_abs`, `validate_backup_path`), `handlers.rs` (`wallet_backup` + `wallet_restore`: internal-only `X-Requesting-Domain` gate + path validation before any FS touch), `handlers/certificate_handlers.rs` (`acquire_certificate_issuance` `is_object()` guard).
- **Mac equivalent:** **None required.** Single cross-platform Rust source — no `_mac` variant. Path logic is cross-platform: the `\\?\`/UNC/`\\.\`-rejection test is `#[cfg(windows)]`; the POSIX accept/reject variants already run on the macOS leg.
- **Risk / notes:** At Mac smoke, sanity-check `lexical_normalize_abs`/`validate_backup_path` against a real macOS data path (`~/Library/Application Support/HodosBrowser/backups`) — the unit tests cover the POSIX shape but confirm `data_root()` resolution end-to-end. No Mac code to port.
- **Future (deferred, not built):** the user-facing "copy the file"/cloud-backup buttons must obtain the destination from the **OS save dialog driven by the C++ shell** (authenticated path), not an HTTP body — at which point the `backups/` confinement relaxes for that dialog-returned path. Mac side: native save dialog via `cef_browser_shell_mac.mm`.

### Wave 1 Track A — F6 JS-string-injection hardening (2026-06-18, branch `0.4.0`)
- **Windows change:** New header-only `cef-native/include/core/JsStringEscape.h` (hardened `escapeJsonForJs`); `simple_render_process_handler.cpp` deletes its local `static` copy, `#include`s the header, and routes 3 sites through it (`brc100_auth_request` 5 dApp fields, `tab_list_response`, `omnibox_select`). New GoogleTest `tests/js_string_escape_test.cpp` (15 tests) + `tests/CMakeLists.txt` entry.
- **Mac equivalent:** **None required.** `simple_render_process_handler.cpp` is a single cross-platform file (per `handlers/CLAUDE.md`, all 5 handler files are cross-platform); the new header is pure C++ (no platform code). The encoder behaves identically on macOS.
- **Risk / notes:** Build verified on Windows (encoder 54/54 GoogleTest green; full `HodosBrowserShell` recompiles clean). On the Mac build, the same `hodos_tests` target compiles + runs (no Mac-specific wiring). **Live smoke (deferred to next dev run, both platforms):** BRC-100 auth overlay still populates domain/method/body on a real dApp; a tab whose title contains an apostrophe still renders (was the `tab_list_response` breakout); omnibox arrow-key nav still works.

### Wave 1 Track A — F5 / R1 profile-launch cmd-injection (2026-06-18, branch `0.4.0`)
- **Windows change:** `ProfileManager.h` adds inline `IsValidProfileId` (cross-platform; 9 GoogleTests). `ProfileManager.cpp::LaunchWithProfile` gains a cross-platform validation guard at the top. `simple_handler.cpp` `profiles_switch` IPC validates the id (defense-in-depth). All compile-clean on Windows; the Windows `CreateProcessW` branch is unchanged (validation now guarantees a safe id).
- **⚠️ Mac equivalent — COMPILE-VERIFY REQUIRED ON MAC (this is the core of F5):** `ProfileManager.cpp` `#elif defined(__APPLE__)` branch (~`:435`) replaces `system(cmd)` with **`posix_spawn("/usr/bin/open", argv…)`** (argv `{"/usr/bin/open","-n","-a",appPath,"--args","--profile="+id,nullptr}`) + `waitpid`. New mac includes added: `<spawn.h>`, `<sys/wait.h>`, `<cstring>`, `<cerrno>`, `extern char** environ;`. **This branch is `#elif`-gated so the Windows build did NOT compile it** — on the first Mac build, confirm it compiles and that profile switching still launches a new instance with the right `--profile`. argv is byte-identical to the old shell string, so behavior should match.
- **Risk / notes:** `IsValidProfileId` accepts the legacy `"Profile N"` (space) id form — verified against `GenerateProfileId` — so existing profiles are not locked out. Live smoke (next dev run, **especially macOS**): create/switch profiles; confirm a new instance launches with the correct profile and a malformed id is rejected.

### Profile review R2/R3 — clean shutdown / safe immediate restart (2026-06-18, branch `0.4.0`)

Fixes the "DB held on quick restart" race: the C++ browser DBs were closed only at static-destructor time (after the profile lock was already freed), so a fast relaunch could win the lock and open a live-WAL DB (`SQLITE_BUSY`). Ran the full harness incl. an adversarial design-review gate (invariant #8). **Windows side is compile-verified here; the macOS `.mm` parts below are `#ifdef __APPLE__` and were NOT compiled by the Windows build — they need compile-verify + behaviour-verify on the Mac.**

- **Cross-platform (compiles on Windows, used by both):** added a public inline `void Shutdown() { CloseDatabase(); }` to `HistoryManager`, `BookmarkManager`, `CookieBlockManager`, `PaidContentCache`. Added the missing `PRAGMA wal_checkpoint(RESTART)` to `PaidContentCache::CloseDatabase()` (parity with the other 3).
- **Windows (`cef_browser_shell.cpp`):** removed the early `ReleaseProfileLock()` (~old `:530`); in `main()` final cleanup, after the defensive server-stops and before `Logger::Shutdown()`/`CefShutdown()`, added the 4-manager `Shutdown()` cascade then `ReleaseProfileLock()`.
- **⚠️ macOS `cef_browser_shell_mac.mm` — COMPILE + BEHAVIOUR VERIFY ON MAC:**
  - **DONE (verify compiles):** `StopServers()` — replaced the blind `usleep(1s)` graceful wait with an **adaptive `waitpid(WNOHANG)` poll** (lambda `stopPid`): early-exits the instant the process is reaped, SIGTERM only after a cap (wallet **5s**, adblock 1.5s) — mirrors Windows `WaitForSingleObject(5000)`. Confirm it compiles (lambda + `waitpid`/`kill`/`usleep`/`std::string` — all headers already used in this file) and that on a real quit the wallet exits *fast* when idle and is only SIGTERM'd if a long broadcast overruns 5s.
  - **DEFERRED ON PURPOSE — the macOS DB cascade is NOT added.** Reason: per this file's own notes (`HistoryManager not implemented on macOS yet`, ~`:4914`), not all 4 SQLite managers are initialized/compiled in the mac build, so adding `GetInstance().Shutdown()` calls risks a **link error** I can't catch from Windows — and mac has no live browser-DB race today (DBs aren't all there). **TODO when wiring up mac DB managers:** in mac `main()` shutdown, insert the 4-manager `Shutdown()` cascade **before** `ReleaseProfileLock()` (which already sits after `StopServers()` and before `CefShutdown()`), mirroring the Windows ordering. First confirm which of History/Bookmark/CookieBlock/PaidContent are actually in the mac build/target.
- **Mac live-test (next dev run):** the core scenario — **fire a wallet action (e.g. a send/broadcast), immediately quit, then immediately relaunch** → expect a clean start with no wallet/DB error (wallet WAL auto-recovers; `TaskSendWaiting` reconciles the in-flight tx). Also verify a normal quit is *snappy* (idle wallet exits well under 5s, not a fixed 1s+ stall) and that quitting mid-broadcast doesn't hang the app for the full 5s unless the broadcast is genuinely still running.

### Profile startup resolver + pre-window picker (CHUNK 1 + R5 + R7 + picker) (2026-06-19, branch `0.4.0`)

Design: `development-docs/0.4.0/PROFILE_STARTUP_PICKER_DESIGN.md`. Ran the full harness (kickoff + bounded research + adversarial design review + adversarial code review). Builds clean on Windows; `hodos_tests` 19 resolver/id tests green; frontend TS green.

- **Cross-platform (compiles on Windows, used by both):**
  - `ProfileManager.h` — new pure header-only `ResolveStartup()` resolver + `StartupResolution` struct (the startup decision table, R5/R7); `SetCurrentProfileId(id, persist)` now takes a persist flag; `showPickerOnStartup_` default flipped to `true`.
  - `ProfileManager.cpp` — `RegistryLock` RAII (cross-process registry lock: **Windows named mutex / macOS `flock`** — the `#elif __APPLE__` branch adds `<fcntl.h>` + `<sys/file.h>`), wraps `Load()` + `Save()`; atomic `SaveUnlocked()` (tmp+rename); `Load` default for `showPickerOnStartup` → `true`.
  - The resolver + persist-flag are used by **both** entry points; the mac entry (`cef_browser_shell_mac.mm`) was edited too (below).
- **⚠️ macOS `cef_browser_shell_mac.mm` — COMPILE-VERIFY ON MAC (NOT compiled by the Windows build):** the startup block (~`:4640`) now extracts `--profile` from `NSProcessInfo`, then calls `ProfileManager::ResolveStartup(argProfile, existingIds, lastUsed, defaultId, /*pickerEnabled=*/false)` and `SetCurrentProfileId(id, res.explicitChoice)`. **Picker mode is forced OFF on macOS** (Windows-first), so mac simply gets the CHUNK 1 + R5 + R7 win (no-arg launch opens last-used; coherent invalid-`--profile` fallback; no boot-rewrite of `profiles.json`). Confirm it compiles and that no-arg launch + a native dock/reopen still behaves.
- **⚠️ Picker MODE is Windows-only — TODO for the Mac sprint:** the pre-window picker (neutral `.picker-cache` `CefInitialize` branch, full-window `/profile-picker?mode=window`, spawn-then-`WM_CLOSE`) lives entirely in Windows code (`cef_browser_shell.cpp` `g_picker_mode` branches + `simple_app.cpp` OnContextInitialized + `simple_handler.cpp` `profiles_switch`). macOS has no equivalent yet. To port: add a mac picker path (NSWindow hosting the `/profile-picker?mode=window` browser, no profile lock/DBs, `LaunchWithProfile`-then-quit), respecting invariant #8. macOS single-instance is the `NSApplication` reopen delegate, not the `.picker` pipe.
- **macOS `RegistryLock` flock has no acquire timeout** (Windows uses 5s). A crashed peer holding the flock would block mac startup at the registry read. Acceptable Windows-first; add a timeout (e.g. `flock(LOCK_EX|LOCK_NB)` + bounded retry) during the Mac sprint.
- **Live smoke (next dev run, Windows):** see `PROFILE_STARTUP_PICKER_DESIGN.md` §7 — picker appears on no-arg launch with >1 profile; double no-arg launch = single picker; pick already-running profile → new window in it + picker exits; pick not-running profile → cold start; 1-profile install → no picker; garbage `--profile` → coherent Default; quick-restart still clean (R2/R3 regression).

### Per-profile history isolation fix + R6 picker-payload hardening (2026-06-19, branch `0.4.0`)

Root cause (Windows + macOS): the **render-process** `HistoryManager` init hardcoded `…\Default` for the history DB path, regardless of profile. Because `SimpleApp` eagerly `new`s the render handler in EVERY process (incl. the browser process), and `HistoryManager::OpenDatabase()` has an `if (history_db_) return true;` early-return guard, the hardcoded-Default open won the race and masked the correct per-profile init → every profile's New-Tab tiles + omnibox showed **Default's** history (cookies/logins were always isolated).

- **Windows fix:**
  - `simple_app.cpp/.h` — new `CefBrowserProcessHandler::OnBeforeChildProcessLaunch` appends `--profile=<currentProfileId>` (IsValidProfileId-guarded) to child command lines.
  - `simple_render_process_handler.cpp` — render History init now (1) only runs in real renderer subprocesses (`--type=renderer`), so it no longer poisons the browser process, and (2) reads the propagated `--profile` and binds to the correct profile path (fallback Default only if absent/invalid).
  - `simple_handler.cpp` — **R6**: `profiles_get_all` payload rebuilt with `nlohmann::json` (was hand-concatenated) so an odd profile name can't break the picker.
- **⚠️ macOS — SAME BUG, NEEDS THE PARALLEL FIX (compile + behaviour verify on Mac):**
  - `cef-native/mac/process_helper_mac.mm:~58` and `cef_browser_shell_mac.mm:~4877` init `HistoryManager` with a `cache_path` — verify whether the **render/helper** path is profile-aware or hardcoded like Windows was. The macOS render helper needs to read the active profile (macOS has no `OnBeforeChildProcessLaunch` wired yet — add the equivalent child-arg propagation, or derive the profile in the helper) and bind History to the correct profile dir. Until then, macOS multi-profile history may leak the same way.
  - The `OnBeforeChildProcessLaunch` override is cross-platform (CefBrowserProcessHandler) — it will run on macOS too once the mac render helper consumes `--profile`.
- **Live smoke (Windows, next dev run):** open a non-Default profile → New-Tab tiles show only the 2 placeholders (CoinGeek/MetaNet), NOT Default's tiles; omnibox suggestions don't include Default's history; Default itself unchanged. R6: a profile named with a `"` doesn't break the picker.

### Header/Omnibox UX pass — B2-FILL + (d) Downloads auto-hide (2026-06-19, branch `0.4.0`)

Design: `development-docs/0.4.0/HEADER_UX_PHASE.md`. First two pieces of the header pass; both **frontend-only / cross-platform** — they port to macOS for free (the header React app renders identically under the mac CEF shell).

- **B2-FILL** (`MainBrowserView.tsx` root Box): dropped the vestigial `calc(100% + 16px)` / `margin: -8px` hack (compensated for an 8px UA body margin already reset to 0). No mac-specific work — the same React fix applies under `cef_browser_shell_mac.mm`'s header NSView. Worth an eyeball on mac that the header fills its 96px region (mac header height is also 96).
- **(d) Downloads auto-hide** (`MainBrowserView.tsx`): download toolbar button now hidden until a download exists, `Grow`-animates in/out, pulses green on complete. Pure React. **Optional mac-only nicety deferred:** a Dock bounce / `requestUserAttention:` on download-complete (no Windows analog). Not built; queue for the Mac sprint if wanted.

### Header/Omnibox UX pass — (a) Bookmarks overlay (2026-06-19, branch `0.4.0`)

The Bookmarks overlay is a **left-anchored, keyboard-capable dropdown** (search box). Its closest macOS sibling is the **profile picker** (`CreateProfilePanelOverlayMacOS`/`Show`/`Hide`, `cef_browser_shell_mac.mm:~4051/4038/4020`) — it uses `DropdownOverlayView` (keyboard-forwarding `keyDown:` at `~:943`) + the click-outside-monitor pattern. The one structural difference: bookmarks is **LEFT-anchored**, so positioning must be hand-rolled like the omnibox (`~:3787`), **not** `CalculateToolbarOverlayFrame` (`OverlayHelpers_mac.mm:267`, right-only). Verify all line numbers on the Mac branch before trusting them.

**Already cross-platform (no mac work):**
- `BrowserWindow` mac fields `bookmarks_panel_overlay_window` / `bookmarks_panel_event_monitor` / `bookmarks_icon_left_offset` (`BrowserWindow.h` `__APPLE__` section).
- Role dispatch `"bookmarkspanel"` in `SetBrowserForRole`/`GetBrowserForRole` (`BrowserWindow.cpp`).
- `GetBookmarksPanelBrowser()` — cross-platform static (`simple_handler.cpp`), callable from `.mm` like `GetProfilePanelBrowser()`.
- Deferred `setBookmarkContext` injection in `OnLoadingStateChange` (role-gated, NOT `#ifdef`'d) + the shared `EscapeForSingleQuotedJs` helper + `OnAfterCreated` role branch + IPC arg parse/`pending_bookmark_*` stash (run before the `#ifdef` split).
- React: `BookmarksOverlayRoot.tsx`, `useBookmarks.ts`, route `/bookmarks`, header button — all platform-agnostic.

**New mac work required (checklist):**
- [ ] Add `NSWindow* g_bookmarks_panel_overlay_window = nullptr;` + click-outside monitor + show-tick globals alongside the profile-panel ones (`cef_browser_shell_mac.mm:~256-266`, `~3992-4036`).
- [ ] Implement `CreateBookmarksPanelOverlayMacOS(int iconLeftOffset)` / `Show` / `Hide` / `IsVisible` / `WasJustHidden` + `Install/RemoveBookmarksPanelClickOutsideMonitor`, cloned from the profile block (`~:3991-4118`). Use **`DropdownOverlayView`** (needs `keyDown:` for the search box), `browserAccessor = ^{ return SimpleHandler::GetBookmarksPanelBrowser(); }`, URL `/bookmarks`, role `"bookmarkspanel"`, then `makeKeyAndOrderFront` + `makeFirstResponder:contentView` (required for search focus).
- [ ] **Left-anchor positioning** (hand-roll X like omnibox `~:3787`; do NOT use `CalculateToolbarOverlayFrame`). `iconLeftOffset` arrives as **CSS px / points** — apply directly to `contentScreen.origin.x` with **NO Windows-style `ScalePx`** (would double-offset on Retina); then `ClampOverlayToScreen` (`OverlayHelpers_mac.mm:~235`).
- [ ] Replace the no-op `#elif defined(__APPLE__)` arms: `bookmarks_panel_show` (`simple_handler.cpp`, currently logs+returns) → create/show/hide toggle mirroring the download-panel mac block, **including the immediate `setBookmarkContext` re-open injection** (deferred path only fires on first load); add an `__APPLE__` arm to `bookmarks_panel_hide` (currently `_WIN32`-only); add an `__APPLE__` arm to the menu "bookmarks" action (currently `_WIN32`-only).
- [ ] Add `g_bookmarks_panel_overlay_window` to `InstallAppFocusLossHandler`'s close list (`OverlayHelpers_mac.mm:~189-229`, has a "Future dropdown overlays" TODO) + to shutdown cleanup (`cef_browser_shell_mac.mm:~4210-4293`).

**Risks to verify on mac:** Retina points-vs-pixels for the left X (no scaling); `makeFirstResponder` so the search box is typable (classic OSR-keyboard failure if omitted); first-open (deferred) vs re-open (immediate) context injection both wired; use `DropdownOverlayView` not `GenericOverlayView`.

### Site permissions engine — b1a (2026-06-20, branch `0.4.0`)

`SitePermissionStore` (per-profile SQLite, tri-state) + the two `CefPermissionHandler` overrides (`OnRequestMediaAccessPermission` / `OnShowPermissionPrompt`) that honor stored Allow/Block silently and defer "Ask" to Chromium's prompt. **Almost entirely cross-platform** — `SitePermissionStore.cpp` is in the shared `SOURCES` (compiles + links on mac), and the overrides + secure-context guard live in cross-platform `simple_handler.cpp`. The store is **null-safe** (`GetState` returns `Ask` when `db_==nullptr`), so on mac it compiles, links, and runs safely TODAY: with the store uninitialized, every request falls through to Chromium's prompt — i.e. current mac behavior, nothing broken.

**One-line mac TODO (do during the Mac sprint):** wire `SitePermissionStore::GetInstance().Initialize(profile_cache)` into the mac startup alongside the bookmark init (`cef_browser_shell_mac.mm:~4718`, mirroring `BookmarkManager::GetInstance().Initialize`), and add `SitePermissionStore::GetInstance().Shutdown()` to the mac DB-close cascade (deferred per R2/R3). Until then, stored permission decisions simply don't apply on mac (everything prompts). **macOS TCC nuance still applies** (camera/mic also gated by the OS; `Info.plist` must declare `NSCameraUsageDescription`/etc.) — that's a b1b/Mac-sprint concern, see the SITE_INFO_PERMISSIONS_DESIGN.md macOS section.

### Site-permission prompt — b1b (2026-06-20, branch `0.4.0`)

The Hodos-branded permission prompt (replaces Chromium's stock prompt on the "Ask" path) reuses the shared notification overlay (`BRC100AuthOverlayRoot` `notificationType === 'permission_request'`) — **cross-platform React**, no new overlay. The parked-callback registry (`PendingPermissionManager`), the `OnBeforeClose` cleanup, the 60s watchdog sweep, and the `permission_response` IPC handler are all in cross-platform `simple_handler.cpp`.

**Windows-only today (deliberate):** `FireHodosPermissionPrompt` is `#ifdef _WIN32` — on mac it returns false so the override falls through to Chromium's stock prompt (current mac behavior, nothing broken). To enable the Hodos prompt on mac: add a mac arm that calls the mac `CreateNotificationOverlay(type, domain, extraParams)` (already exists, `cef_browser_shell_mac.mm:~3454`) with `type="permission_request"` + `&requestId=…&perm=…`, and re-check the `g_pendingModalDomain` wallet-modal guard + the overlay-hide guards (the `g_notification_overlay_hwnd` hide blocks are `#ifdef _WIN32`; add `HideNotificationOverlayWindow()` mac equivalents). The React branch + IPC + store all work as-is once the mac fire path exists. **macOS TCC** still applies (camera/mic also OS-gated; `Info.plist` usage strings) — see the SITE_INFO_PERMISSIONS_DESIGN.md macOS section.

**Known residual (both platforms):** if a wallet/auth modal fires while a permission prompt is showing, it replaces it on the shared overlay; the parked permission callback is then cancelled by the 60s watchdog (bounded, not a hang). Full prevention = the wallet-overlay fire path checking `PendingPermissionManager::hasPending()` — deferred follow-up.

### Allow-once session memory — b1b.1 (2026-06-20, branch `0.4.0`)

Ephemeral per-tab "Allow this time" grants (in `PendingPermissionManager`, NOT persisted), promoting Ask→Allow for the granting tab+host until navigate-away or tab close. Entirely cross-platform: the grant logic + the clear hooks (`OnAddressChange` host-change clear, `OnBeforeClose` clear) live in cross-platform `simple_handler.cpp`. No mac-specific work; it activates on mac as soon as the mac permission-prompt fire path exists (b1b mac TODO).

### Site-info hub overlay — b2a (2026-06-21, branch `0.4.0`)

The Site-Info hub is a **left-anchored, NO-keyboard dropdown** (TuneIcon at the address-bar left). Unlike bookmarks (which needed keyboard for its search box → `DropdownOverlayView` + `makeFirstResponder`), the hub has **no text input**, so its macOS sibling is the **download/cookie panel** pattern (mouse-only), NOT the profile/bookmarks keyboard pattern. On Windows it clones the DOWNLOAD panel (`MA_NOACTIVATE` + installed `WH_MOUSE_LL` click-outside hook) with the BOOKMARKS left-anchor math.

**Already cross-platform (no mac work):**
- `BrowserWindow` mac fields `siteinfo_panel_overlay_window` / `siteinfo_panel_event_monitor` / `siteinfo_icon_left_offset` (`BrowserWindow.h` `__APPLE__` section).
- Role dispatch `"siteinfopanel"` in `SetBrowserForRole`/`GetBrowserForRole` (`BrowserWindow.cpp`).
- `GetSiteInfoPanelBrowser()` — cross-platform static (`simple_handler.cpp`), callable from `.mm` like `GetDownloadPanelBrowser()`.
- Deferred `setSiteInfoContext` injection in `OnLoadingStateChange` (role-gated, not `#ifdef`'d) + `OnAfterCreated` role branch (150ms WasResized+Invalidate) + IPC arg parse / `pending_siteinfo_host_`/`_security_` stash (runs before the `#ifdef` split).
- `open_wallet_permissions` IPC already has a working `#elif defined(__APPLE__)` arm (calls the existing mac `CreateNotificationOverlay("edit_permissions", domain)`); the `siteinfo_panel_hide` no-op on mac is harmless (overlay doesn't exist there yet).
- React: `SiteInfoOverlayRoot.tsx`, route `/site-info`, `usePrivacyShield(host)`, the TuneIcon in `MainBrowserView.tsx` — all platform-agnostic (the TuneIcon renders identically under the mac header NSView).

**New mac work required (checklist):**
- [ ] Add `NSWindow* g_siteinfo_panel_overlay_window = nullptr;` + click-outside monitor + a `g_siteinfo_last_hide_tick` analog alongside the download/profile globals (`cef_browser_shell_mac.mm:~256-266`).
- [ ] Implement `CreateSiteInfoPanelOverlayMacOS(int iconLeftOffset)` / `Show` / `Hide` / `IsVisible` / `WasJustHidden` + `Install/RemoveSiteInfoPanelClickOutsideMonitor`, cloned from the **download** mac block (mouse-only — use `GenericOverlayView`, NOT `DropdownOverlayView`; no `makeFirstResponder`/keyboard needed). `browserAccessor = ^{ return SimpleHandler::GetSiteInfoPanelBrowser(); }`, URL `/site-info`, role `"siteinfopanel"`.
- [ ] **Left-anchor positioning** (hand-roll X like omnibox/bookmarks `~:3787`; do NOT use `CalculateToolbarOverlayFrame`, right-only). `iconLeftOffset` arrives as **CSS px / points** — apply directly, **NO `ScalePx`** (double-offsets on Retina); then `ClampOverlayToScreen`. Size 360×480.
- [ ] Replace the no-op `#elif defined(__APPLE__)` arm for `siteinfo_panel_show` in `simple_handler.cpp` (currently logs) → create/show/hide toggle mirroring the download-panel mac block, **including the immediate `setSiteInfoContext` re-open injection** (deferred path only fires on first load) **and a hide-tick toggle guard** if the mac path also installs a global mouse monitor that can pre-hide on the same click (verify whether the mac NSEvent monitor has the same same-click race as the Windows `WH_MOUSE_LL`; if it doesn't fire before the React click, the guard may be unnecessary on mac). Add an `__APPLE__` arm to `siteinfo_panel_hide` (currently `_WIN32`-only).
- [ ] Add `g_siteinfo_panel_overlay_window` to `InstallAppFocusLossHandler`'s close list (`OverlayHelpers_mac.mm:~189-229`) + shutdown cleanup (`cef_browser_shell_mac.mm:~4210-4293`).

**Risks to verify on mac:** Retina points-vs-pixels for the left X (no scaling); the toggle/click-outside race (the Windows fix is the `g_siteinfo_last_hide_tick` guard — confirm whether the mac NSEvent local monitor reproduces the "hook hides before IPC re-shows" sequence); first-open (deferred) vs re-open (immediate) `setSiteInfoContext` both wired. Connection-badge + shields + links are pure React and work as-is once the mac overlay shell exists.

**Known Windows limitation (shared with bookmarks, NOT a regression):** the primary-window-handoff migration block (`cef_browser_shell.cpp` ~`overlayRoles[]`) does not transfer/null the siteinfo (or bookmarks) overlay on multi-window primary reassignment — worst case is a stale HWND. Fix both together if multi-window overlay lifecycle is ever hardened.

### Site-permission management — b2b (2026-06-21, branch `0.4.0`)

The in-hub `Allow | Block | Ask` management UI. **Fully cross-platform — no mac work.** The 3 IPC handlers (`site_permissions_get/set/reset`) + the `SendSitePermissionsToBrowser` helper live in cross-platform `simple_handler.cpp` and operate purely on `SitePermissionStore` (already in the shared SOURCES) + send `site_permissions_response` back to the calling overlay via `browser->GetMainFrame()->SendProcessMessage` (platform-neutral). The render-side route → `window.onSitePermissionsResponse` is in cross-platform `simple_render_process_handler.cpp`. React (`useSitePermissions`, the collapsible segmented UI) is platform-agnostic. So b2b activates on mac automatically once the b2a `"siteinfopanel"` overlay shell exists there (see the b2a checklist above). The only b2a-side mac dependency carries over; b2b itself adds zero mac TODOs.
