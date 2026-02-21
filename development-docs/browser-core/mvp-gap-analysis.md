# MVP Gap Analysis & Prioritization

**Created**: 2026-02-19
**Status**: Complete (Phase C)
**Purpose**: Synthesize Phase A audit + Phase B research into an ordered priority list of what must be built for a quality MVP browser. Every gap includes implementation approach, effort, and dependencies.

---

## Definitions

- **MVP-Blocking**: Users will abandon the browser without this. Must ship.
- **MVP-Important**: Users will notice the absence. Should ship.
- **Nice-to-Have**: Improves experience. Ship if time allows.

---

## C.1 Core Browser Gaps

### C.1.1 SSL Certificate Error Handling — MVP-Blocking

**Current state**: No `OnCertificateError` handler. CEF default blocks all invalid certs silently. Users cannot log into x.com (likely SSL-related or FedCM — needs testing).

**What's needed**:
- Implement `CefRequestHandler::OnCertificateError` in `simple_handler.cpp`
- For most errors: show a warning interstitial page ("Your connection is not private")
- User can choose "Go back" (default) or "Proceed anyway" (advanced)
- Store callback via `CefRefPtr<CefCallback>` for async proceed
- Add padlock icon / connection security indicator in header bar

**CEF API**: `OnCertificateError(browser, cert_error, request_url, ssl_info, callback)` — UI thread. Return `true` + store callback for async UI. `ssl_info->GetX509Certificate()` provides cert chain for display.

**Note**: The x.com login issue may be FedCM, not SSL. Need to test with the SSL handler in place first.

**Effort**: Low (1 day)
**Dependencies**: None
**Files**: `simple_handler.h/cpp`, new interstitial HTML (can be inline or React route)

---

### C.1.2 Download Handler — MVP-Blocking

**Current state**: No `CefDownloadHandler`. Downloads may trigger a system save dialog but there's no progress UI, download history, or cancel/pause/resume.

**What's needed**:
- Implement `CefDownloadHandler` with 3 methods: `CanDownload`, `OnBeforeDownload`, `OnDownloadUpdated`
- `OnBeforeDownload`: Call `callback->Continue("", true)` to show system Save As dialog (simplest MVP approach)
- `OnDownloadUpdated`: Track progress via `GetPercentComplete()`, `GetReceivedBytes()`, `GetCurrentSpeed()`, `IsComplete()`
- Downloads panel (React overlay) showing active + completed downloads with progress bars
- Ctrl+J keyboard shortcut to open downloads panel
- Cancel/Pause/Resume controls per download

**Effort**: Medium (2-3 days)
**Dependencies**: None
**Files**: `simple_handler.h/cpp` (add handler), new `DownloadsPanel` React component, IPC messages for download state

---

### C.1.3 Media Permissions (Camera/Mic) — MVP-Blocking

**Current state**: No `OnRequestMediaAccessPermission` handler. CEF default denies all camera/mic access. Video calls (Zoom, Teams, Discord, Google Meet) completely broken.

**What's needed**:
- Implement `CefPermissionHandler` on `SimpleHandler`
- Override `OnRequestMediaAccessPermission` — return `false` to get Chrome's native permission prompt UI for free (Chrome bootstrap mode)
- That's it for MVP. Chrome shows its own "Allow/Block" bubble.
- Future: persist permission decisions per-origin in our own storage

**Effort**: Low (0.5 day — literally return `false` from the handler)
**Dependencies**: None
**Files**: `simple_handler.h/cpp`

---

### C.1.4 General Permission Prompts (Geo, Notifications) — MVP-Important

**Current state**: No `OnShowPermissionPrompt` handler. CEF default denies geolocation, web notifications, MIDI, etc.

**What's needed**:
- Override `OnShowPermissionPrompt` on `CefPermissionHandler` — return `false` for Chrome's native permission bubble
- Covers: geolocation, web notifications, MIDI, storage access, and more
- Same pattern as C.1.3

**Effort**: Trivial (part of C.1.3 implementation — same handler)
**Dependencies**: C.1.3 (same `CefPermissionHandler` addition)
**Files**: `simple_handler.h/cpp`

---

### C.1.5 Find-in-Page — MVP-Important

**Current state**: No find handler. Ctrl+F behavior unknown (may do nothing or show minimal CEF default).

**What's needed**:
- Custom find bar UI (text input + "X of Y matches" + prev/next/close buttons)
- Wire Ctrl+F to show find bar, Escape to close
- Call `browser->GetHost()->Find(id, text, forward, matchCase, findNext)` on text change
- Implement `CefFindHandler::OnFindResult` for match count display
- Call `browser->GetHost()->StopFinding(true)` on close

**Effort**: Medium (1-2 days)
**Dependencies**: None
**Files**: `simple_handler.h/cpp` (add CefFindHandler), find bar can be React component in header or native HWND

---

### C.1.6 Context Menu Enhancement — MVP-Important

**Current state**: Right-click menu has "Inspect Element" and "Open in new tab" only. Missing basic browser context menu items.

**What's needed**:
- Add standard items to `OnBeforeContextMenu` / `OnContextMenuCommand`:
  - Copy / Cut / Paste (using `CefFrame::ExecuteCommand`)
  - Copy Link Address (for links)
  - Save Image As (for images)
  - Copy Image (for images)
  - View Page Source (open `view-source:` URL in new tab)
  - Back / Forward / Reload (when not on a link)
  - Select All
- Context-sensitive display (links show link items, images show image items, etc.)

**Effort**: Medium (1-2 days)
**Dependencies**: None
**Files**: `simple_handler.cpp` (existing `OnBeforeContextMenu` / `OnContextMenuCommand`)

---

### C.1.7 JavaScript Dialog Handler — MVP-Important

**Current state**: No `CefJsDialogHandler`. `alert()`, `confirm()`, `prompt()` may use Chromium's basic default dialogs.

**What's needed**:
- Test current behavior first — Chrome bootstrap may already show native dialogs
- If not: implement `CefJsDialogHandler` with `OnJSDialog` returning `false` for default handling
- Suppress `onbeforeunload` dialogs from malicious sites (common annoyance)

**Effort**: Low (0.5 day — test first, may already work)
**Dependencies**: None
**Files**: `simple_handler.h/cpp`

---

### C.1.8 Print Support — Not Needed

**Current state**: No `CefPrintHandler`. Windows uses native print dialog automatically.

**Assessment**: Per Phase B research, `CefPrintHandler` is Linux-only. Windows CEF already shows the native print dialog for `window.print()` and Ctrl+P. **No implementation needed.**

---

### C.1.9 Keyboard Shortcuts — Partial (Review Needed)

**Current state**: 8 shortcuts implemented (F12, Ctrl+T/W/Tab/Shift+Tab/L/R, F5).

**Missing for MVP**:
- Ctrl+F → Find-in-page (depends on C.1.5)
- Ctrl+J → Downloads panel (depends on C.1.2)
- Ctrl+H → History page
- Ctrl+D → Bookmark current page
- Ctrl+P → Print (may already work via CEF)
- Ctrl+N → New window (if we support multiple windows)
- Ctrl++ / Ctrl+- / Ctrl+0 → Zoom in/out/reset
- Alt+Left/Right → Back/Forward (in addition to existing buttons)

**Effort**: Low (0.5 day — most are simple dispatches)
**Dependencies**: C.1.2, C.1.5 for their respective shortcuts
**Files**: `simple_handler.cpp` (`OnPreKeyEvent` / `OnKeyEvent`)

---

## C.2 Security & Privacy Gaps

### C.2.1 Ad & Tracker Blocking — MVP-Important

**Current state**: `CookieBlockManager` with 24 hardcoded tracker domains in `DefaultTrackerList.h`. Blocks cookies but not requests.

**What's needed**:
- Integrate `adblock-rust` as FFI static library linked into C++ CEF process
- Load EasyList + EasyPrivacy (~123,000 rules)
- Hook into `GetResourceRequestHandler` (same location as wallet HTTP interception)
- Block matching network requests before they load
- Serialize compiled engine to disk for fast startup (~15-25 MB RAM)
- Background daily update of filter lists
- Per-site toggle ("turn off ad blocking for this site")
- Replaces `DefaultTrackerList.h` with proper filter lists

**Effort**: High (3-5 days for FFI integration + filter management + UI toggle)
**Dependencies**: None (standalone)
**Files**: New `adblock/` directory for FFI bridge, `HttpRequestInterceptor.cpp` (hook point), React shield/toggle UI

---

### C.2.2 WebRTC Leak Prevention — MVP-Important

**Current state**: No WebRTC configuration. Local IP addresses may leak to websites via WebRTC.

**What's needed**:
- Add one CEF command-line switch: `--force-webrtc-ip-handling-policy=default_public_interface_only`
- This prevents local IP address leakage while keeping WebRTC functional for video calls

**Effort**: Trivial (5 minutes — one line in `cef_browser_shell.cpp`)
**Dependencies**: None
**Files**: `cef_browser_shell.cpp` (command-line setup)

---

### C.2.3 Secure Connection Indicator — MVP-Important

**Current state**: No padlock icon or HTTPS indicator. Users can't tell if connection is secure.

**What's needed**:
- Parse URL in header to determine protocol (https/http)
- Show padlock icon (locked = HTTPS, warning = HTTP, error = cert error)
- Click on padlock shows connection info popup (issuer, cert validity, etc.)
- Ties into C.1.1 (SSL cert handling) for cert info display

**Effort**: Low-Medium (1 day for icon + basic info popup)
**Dependencies**: C.1.1 (for full cert info display)
**Files**: Header bar React component, potentially `OnCertificateError` data forwarding

---

### C.2.4 Cookie Controls — Nice-to-Have

**Current state**: Per-domain cookie blocking via `CookieBlockManager` + default tracker list. No third-party cookie blocking.

**What's needed for MVP**:
- Third-party cookie blocking is the most impactful addition
- Can be implemented via `CefCookieAccessFilter::CanSendCookie` / `CanSaveCookie`
- Check if cookie domain differs from page domain → block if third-party
- Exception list for sites that break

**Post-MVP**: Ephemeral third-party storage (Brave's approach), CNAME uncloaking, bounce tracking protection.

**Effort**: Medium (1-2 days)
**Dependencies**: None (can be done independently)
**Files**: `HttpRequestInterceptor.cpp` (cookie filter), settings UI for exceptions

---

### C.2.5 Fingerprinting Protection — Nice-to-Have

**Current state**: No fingerprinting protection.

**What's needed**:
- Phase 1 (low effort): Block third-party access to Canvas/WebGL/AudioContext APIs via V8 injection. Check `document.referrer` or use a permissions policy.
- Phase 2 (medium effort): Per-session "farbling" — randomize Canvas/WebGL output using a per-site seed via V8 injection.
- Phase 3 (not practical): Blink-level modifications require patching CEF source.

**Recommendation for MVP**: Phase 1 only — block third-party fingerprinting APIs. Ship Phase 2 post-MVP.

**Effort**: Medium (1-2 days for Phase 1)
**Dependencies**: None
**Files**: V8 injection in `simple_render_process_handler.cpp`

---

### C.2.6 Mixed Content Handling — Nice-to-Have

**Current state**: CEF allows insecure content (`--allow-running-insecure-content` flag is set).

**What's needed**:
- Remove `--allow-running-insecure-content` flag (or make it dev-only)
- CEF's default mixed content behavior (block active mixed content, warn on passive) is already good
- No custom handler needed — just remove the flag

**Effort**: Trivial (remove one flag)
**Dependencies**: Verify no internal pages break without it
**Files**: `cef_browser_shell.cpp`

---

### C.2.7 BSVPriceCache 0.0 Fix — MVP-Blocking (Safety)

**Current state**: `BSVPriceCache` returns `0.0` if both CryptoCompare and CoinGecko fail. This breaks the auto-approve engine — every transaction converts to $0.00 and auto-approves regardless of spending limits.

**What's needed**:
- Cache last successful price; only return 0.0 if never fetched successfully
- If no price has ever been fetched, treat as "price unavailable" and require user approval for all payment endpoints
- Same fix needed in Rust `price_cache.rs` (defense-in-depth)

**Effort**: Low (0.5 day)
**Dependencies**: None
**Files**: `HttpRequestInterceptor.cpp` (BSVPriceCache), `rust-wallet/src/price_cache.rs`

---

## C.3 User Data & Profile Gaps

### C.3.1 Profile Import (Bookmarks) — Nice-to-Have

**Current state**: No import capability. Users must manually recreate bookmarks.

**What's needed**:
- Read Chrome's `Bookmarks` JSON file from `%LOCALAPPDATA%/Google/Chrome/User Data/Default/Bookmarks`
- Parse the JSON tree structure (roots → bookmark_bar/other/synced → children)
- Map to our `BookmarkManager` schema (bookmark_folders + bookmarks tables)
- Import UI: file picker or auto-detect Chrome profile location
- Support both Chrome and Brave (same format)

**Effort**: Low (1 day)
**Dependencies**: None
**Files**: New import handler (IPC in `simple_handler.cpp`), React import UI in Settings

---

### C.3.2 Profile Import (History) — Nice-to-Have

**Current state**: No import capability.

**What's needed**:
- Read Chrome's `History` SQLite database (may need to copy since Chrome locks it)
- Convert Chrome timestamps: `(chrome_ts / 1000000) - 11644473600` = Unix epoch
- Map `urls` + `visits` tables to our `HodosHistory` schema
- Merge with existing history (skip duplicates)

**Effort**: Low (1 day)
**Dependencies**: None (can be bundled with C.3.1)
**Files**: C++ import handler, React import UI

---

### C.3.3 Profile Import (Cookies) — Nice-to-Have

**Current state**: No import capability.

**What's needed**:
- Read Chrome's `Network/Cookies` SQLite database
- Decrypt `encrypted_value` using DPAPI (we have `dpapi.rs`, but cookie decrypt needs C++ DPAPI)
- Import into CEF's cookie store via `CefCookieManager::SetCookie()`
- Handle expired cookies, duplicates

**Effort**: Medium (1-2 days — DPAPI decrypt in C++ is the complex part)
**Dependencies**: None
**Files**: C++ import handler with DPAPI, CefCookieManager calls

---

### C.3.4 History Management — Mostly Complete

**Current state**: Full history recording, search, deletion, domain grouping. All working.

**Gaps**:
- No import from other browsers (see C.3.2)
- No export capability
- No auto-clearing (e.g., "clear history older than 30 days")

**MVP impact**: Low — what exists is sufficient for MVP.

---

### C.3.5 Bookmark Management — Mostly Complete

**Current state**: Create, edit, delete, folder organization. All working.

**Gaps**:
- No import from other browsers (see C.3.1)
- No export (HTML bookmark format)
- No drag-and-drop reordering
- No bookmark bar (favorites bar below header)

**MVP impact**: Low for basic use. A bookmark bar would be a nice UX improvement but not blocking.

---

### C.3.6 Settings Persistence — Partial

**Current state**: Wallet settings persist in `wallet.db` settings table. Browser settings (ad blocking toggle, cookie preferences, etc.) have no unified persistence.

**What's needed for MVP**:
- Settings model in C++ that persists to a JSON file or SQLite table
- Key settings to persist: default search engine, ad blocking on/off, cookie blocking mode, preferred homepage, zoom level defaults
- Settings overlay already exists — needs to be wired to persist

**Effort**: Medium (1-2 days)
**Dependencies**: None
**Files**: New settings persistence layer (JSON or SQLite), `SettingsOverlayRoot.tsx`

---

## C.4 Remaining Wallet UX

### C.4.1 Phase 3: Light Wallet Polish — MVP-Important

**Current state**: Wallet overlay works but lacks polish. Missing: button hover/pressed/loading states, send progress indicator, QR code for receive, "Copied" feedback, inline validation.

**What's needed**:
- Button state feedback (hover, pressed, disabled, loading) across all wallet buttons
- Send transaction progress indicator ("Broadcasting...", "Confirmed")
- QR code on receive section (need `qrcode.react` or similar library)
- "Copied!" toast on address copy
- Inline validation on send form (address format, amount range)
- Empty state messages where appropriate
- Ensure full Hodos branding consistency (gold #a67c00)

**Effort**: Medium (2-3 days)
**Dependencies**: None
**Files**: `WalletPanel.tsx`, `WalletPanelContent.tsx`, `TransactionForm.tsx`, `WalletPanel.css`, new QR code dependency

---

### C.4.2 Phase 4: Full Wallet View — Nice-to-Have for MVP

**Current state**: Wallet overlay has 5 tabs (Overview, Send, Receive, Transactions, Approved Sites). Phase 4 envisions a full-window wallet with address management, certificate management, UTXO display, etc.

**Assessment**: The current wallet overlay already covers the essential operations. Phase 4's expanded features (address labeling, certificate management, basket-grouped outputs) are important for power users and BRC-100 developers but not MVP-blocking.

**Recommendation**: Defer to post-MVP. The current 5-tab overlay is sufficient for first release.

---

### C.4.3 Phase 5: Activity Status Indicator — Nice-to-Have

**Current state**: Not started. Planning doc exists with pre-phase tab session infrastructure design.

**Assessment**: The domain permissions system + notification overlays already handle approval flows. An activity indicator adds passive monitoring but isn't needed for basic browser+wallet use.

**Recommendation**: Defer to post-MVP.

---

### C.4.4 Certificate Testing — Blocked

**Current state**: Certificate acquisition and proving endpoints exist in Rust. Frontend has certificate disclosure notification. Untested end-to-end.

**Assessment**: BRC-52 certificates are needed for identity-heavy BRC-100 apps but not for basic wallet send/receive or general browsing. Testing requires a running certifier service.

**Recommendation**: Defer comprehensive testing to post-MVP, but include a basic smoke test if a certifier becomes available.

---

## C.5 Priority Ordering

### Tier 0 — Ship-Blocking (Must complete for MVP)

| # | Item | Ref | Effort | Dependencies |
|---|------|-----|--------|-------------|
| 1 | BSVPriceCache 0.0 safety fix | C.2.7 | 0.5 day | None |
| 2 | SSL certificate error handling | C.1.1 | 1 day | None |
| 3 | Download handler | C.1.2 | 2-3 days | None |
| 4 | Media permissions (camera/mic) + general permissions | C.1.3 + C.1.4 | 0.5 day | None |

**Rationale**: Without these, the browser is broken for common workflows (downloading files, video calls, SSL sites). The price cache bug is a safety issue that could let payments auto-approve.

---

### Tier 1 — Core Quality (Should complete for MVP)

| # | Item | Ref | Effort | Dependencies |
|---|------|-----|--------|-------------|
| 5 | WebRTC leak prevention | C.2.2 | 5 min | None |
| 6 | Mixed content flag removal | C.2.6 | 5 min | Verify internal pages |
| 7 | Find-in-page | C.1.5 | 1-2 days | None |
| 8 | Context menu enhancement | C.1.6 | 1-2 days | None |
| 9 | Secure connection indicator | C.2.3 | 1 day | C.1.1 (SSL handler) |
| 10 | JS dialog handler (test first) | C.1.7 | 0.5 day | None |
| 11 | Additional keyboard shortcuts | C.1.9 | 0.5 day | C.1.2, C.1.5 |
| 12 | Light wallet polish (Phase 3) | C.4.1 | 2-3 days | None |

**Rationale**: These make the browser feel complete. Find-in-page and context menus are deeply ingrained browser expectations. The security items (WebRTC, mixed content) are trivial wins. Wallet polish improves the core differentiator.

---

### Tier 2 — Differentiating (Complete if time allows)

| # | Item | Ref | Effort | Dependencies |
|---|------|-----|--------|-------------|
| 13 | Ad & tracker blocking (adblock-rust) | C.2.1 | 3-5 days | None |
| 14 | Settings persistence | C.3.6 | 1-2 days | None |
| 15 | Third-party cookie blocking | C.2.4 | 1-2 days | None |
| 16 | Basic fingerprinting protection | C.2.5 | 1-2 days | None |
| 17 | Profile import (bookmarks + history) | C.3.1 + C.3.2 | 2 days | None |

**Rationale**: Ad blocking is the headline privacy feature that differentiates from Chrome. Settings persistence is needed for a production browser. Profile import lowers the switching barrier.

---

### Tier 3 — Post-MVP

| # | Item | Ref | Effort | Notes |
|---|------|-----|--------|-------|
| 18 | Full wallet view (Phase 4) | C.4.2 | 5+ days | Power user feature |
| 19 | Activity status indicator (Phase 5) | C.4.3 | 3-5 days | Passive monitoring |
| 20 | Cookie import from Chrome | C.3.3 | 1-2 days | C++ DPAPI needed |
| 21 | Bookmark bar | C.3.5 | 1-2 days | UI feature |
| 22 | Cosmetic filtering (CSS hide rules) | — | 2-3 days | Needs adblock-rust |
| 23 | Private browsing tabs | — | 2-3 days | CefRequestContext per-tab |
| 24 | Advanced fingerprinting (farbling) | C.2.5 | 3+ days | V8 injection |
| 25 | Certificate end-to-end testing | C.4.4 | 2-3 days | Needs certifier |
| 26 | FedCM support | — | High | No CEF API, low ROI |

---

## Dependency Map

```
C.2.7 BSVPriceCache fix ──────────────────────── standalone
C.1.1 SSL cert handling ───┬──────────────────── standalone
                           └── C.2.3 Secure connection indicator
C.1.2 Downloads ───────────┬──────────────────── standalone
                           └── C.1.9 Ctrl+J shortcut
C.1.3+C.1.4 Permissions ──────────────────────── standalone (combined)
C.2.2 WebRTC fix ──────────────────────────────── standalone
C.2.6 Mixed content ───────────────────────────── standalone (verify first)
C.1.5 Find-in-page ───────┬──────────────────── standalone
                           └── C.1.9 Ctrl+F shortcut
C.1.6 Context menus ───────────────────────────── standalone
C.1.7 JS dialogs ──────────────────────────────── standalone (test first)
C.4.1 Wallet polish ───────────────────────────── standalone
C.2.1 Ad blocking ─────────┬──────────────────── standalone
                            └── C.2.4 (shares filter infrastructure)
C.3.1+C.3.2 Import ────────────────────────────── standalone (bundle together)
C.3.6 Settings persistence ────────────────────── standalone
```

**Parallelization opportunities**: Almost everything is independent. The only real dependency chain is SSL → secure connection indicator, and downloads/find → their keyboard shortcuts.

---

## Shared Infrastructure

Items that should be built together because they share code:

1. **C.1.3 + C.1.4 (Permissions)**: Same `CefPermissionHandler` implementation. Do both at once.
2. **C.1.1 + C.2.3 (SSL + Padlock)**: Cert error handler provides data for connection indicator.
3. **C.1.9 (Keyboard shortcuts)**: Depends on C.1.2 and C.1.5 being done first.
4. **C.3.1 + C.3.2 (Import bookmarks + history)**: Same UI, same detection logic, same settings panel.
5. **C.2.2 + C.2.6 (WebRTC + Mixed content)**: Both are one-line config changes. Do together.

---

## Effort Summary

| Tier | Items | Total Effort |
|------|-------|-------------|
| **Tier 0** (Ship-blocking) | 4 items | ~4-5 days |
| **Tier 1** (Core quality) | 8 items | ~8-11 days |
| **Tier 2** (Differentiating) | 5 items | ~9-13 days |
| **Tier 3** (Post-MVP) | 9 items | ~20+ days |

**Minimum MVP** (Tier 0 only): ~1 week
**Quality MVP** (Tier 0 + Tier 1): ~2-3 weeks
**Full MVP** (Tier 0 + Tier 1 + Tier 2): ~4-5 weeks

---

## Architectural Debt to Address During Implementation

These items from Phase A should be resolved alongside the gap work:

| Item | When to Fix | Why |
|------|------------|-----|
| BSVPriceCache 0.0 bug | Tier 0 (C.2.7) | Safety-critical |
| `--allow-running-insecure-content` flag | Tier 1 (C.2.6) | Security |
| Hardcoded timeout/limit constants | During any C++ work | Extract to `Config.h` |
| `json_storage.rs` rename | During any Rust cleanup | Misleading file name |
| SessionManager reference-after-unlock | During wallet work | Minor race condition |

---

## Documentation Fixes (Phase D)

Per `doc-discrepancies.md`, 66+ discrepancies across 6 documents. Recommend:

1. **Consolidate** PROJECT_OVERVIEW.md + ARCHITECTURE.md → single updated ARCHITECTURE.md
2. **Rewrite** README.md (concise: current state, setup, links)
3. **Update** CLAUDE.md (fix 8 stale key file references)
4. **Update** CEF_REFINEMENT_TRACKER.md (check off all CR-2/CR-3 items verified in Phase A.3)
5. **Archive** FEATURES.md (critically stale, replaced by this gap analysis + browser-capabilities.md)
6. **Minor edit** THE_WHY.md (fix date, API names)
7. **Update** TECH_STACK_INTEGRATION.md (remove stale integration descriptions, keep CEF reference sections)
8. **Update** UX_UI/00-IMPLEMENTATION_INDEX.md (mark CR-2 complete, mark Phase 2 complete)

**Effort**: 1-2 days for all doc fixes.

---

**End of Document**
