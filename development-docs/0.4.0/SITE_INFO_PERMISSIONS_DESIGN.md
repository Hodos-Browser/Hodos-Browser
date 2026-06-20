# Site Info + Site Permissions — Design

**Status:** DESIGN — awaiting owner go/no-go. No code written.
**Sprint:** 0.4.0 (Shell release). This is the expanded **(b) chunk** of the Header/Omnibox UX pass.
**Scope grew on purpose:** the owner wants **real, Chrome/Brave-quality site permissions**, not a stub — so (b) is now a full **Site Info + Site Permissions** phase, not a single dropdown.

> All file:line anchors below were re-verified against the current `0.4.0` branch at authoring time. `simple_handler.cpp` shifts often — re-verify at implementation time. Confirmed live: `GetPermissionHandler` @ `simple_handler.cpp:7156`; `MENU_ID_MANAGE_PERMISSIONS` def `:7528`, label `:7612`, handler `:7821`, overlay call `:7838` (Win) / `:7842-7843` (mac); `OnCertificateError` `:831`; `securityState` useMemo `MainBrowserView.tsx:391-405`; `CookieBlockManager` @ `cef-native/{include,src}/core/CookieBlockManager.{h,cpp}`.

---

## 1. Goal & Scope

### IN (this phase)
1. **Real site permissions** — a proper Chrome/Brave-grade system with three parts:
   - **Request PROMPT** ("example.com wants to use your camera — Allow / Block"), replacing CEF's stock popup.
   - **Per-site STORAGE** of the choice (Allow / Block / Ask) in a new per-profile SQLite store.
   - **Management UI** in the site-info hub (see/change/reset per type).
2. **Left site-info "Site Controls" HUB** — a left-anchored dropdown opened by a `TuneIcon` (MUI sliders, **not** a padlock) inside the address bar. Contains: connection status badge, permissions summary (→ deep view), a Shields quick-toggle, a "this site's data" link, and a "Manage Wallet Permissions" link.
3. **Connection STATUS badge** — Secure / Not secure / Certificate problem, informational, reusing the existing `securityState` boolean derivation.
4. **Reuse of the wallet-perms modal** — the hub's "Manage Wallet Permissions" entry opens the EXACT existing `edit_permissions` overlay that the right-click context menu opens. Rename the right-click label to "Manage Wallet Permissions"; keep the right-click entry.
5. **Reuse of the Shields toggle** — the hub's Shields on/off delegates to the existing per-site privacy-shield blocking (`usePrivacyShield`). The existing right-side 🛡 overlay STAYS unchanged.
6. **"This site's data" link** — opens the existing `/browser-data` page (cookies/history) filtered to the current origin.

### OUT / DEFERRED
- **Full valid-certificate detail viewer** (issuer / expiry / chain). See §4c for the nuance — research found CEF's `CefSSLStatus`/`CefX509Certificate` *can* surface this without a patch, but Hodos today only captures a boolean and there is no per-load valid-cert hook wired. **v1 ships status-only; cert detail is a fast-follow / Engine-release item** (see Open Question 5).
- **v2 permission types** (USB/Serial/HID, Bluetooth, MIDI, payment handlers, file-system, DRM, AR/VR) — schema supports them; v1 wires only the 6 daily-driver capabilities.
- **No new Rust endpoints** unless unavoidable. This entire feature is C++ + React + SQLite + reuse of existing IPC. Cert/permission state never touches the wallet backend.
- **No Chromium patch.** Confirmed below.

---

## 2. The "No Chromium Patch" Confirmation (load-bearing)

`CefPermissionHandler` is **first-class embedder API** — the header ships in the stock distribution (`cef-binaries/include/cef_permission_handler.h`), and `SimpleHandler::GetPermissionHandler()` **already returns `this`** (`simple_handler.cpp:7156`). It is wired exactly like the `CefDownloadHandler` / `CefContextMenuHandler` interfaces Hodos already overrides. Today it overrides **none** of the three callbacks, so the base-class defaults (`return false`) run → Chromium shows its **stock permission popup**.

To make permissions real we override three callbacks on `SimpleHandler` (all fire on the **browser-process UI thread**):

| Callback | Use for | Callback object |
|---|---|---|
| `OnRequestMediaAccessPermission` | camera + mic (`getUserMedia`) | `CefMediaAccessCallback` → `Continue(allowed_mask)` / `Cancel()` |
| `OnShowPermissionPrompt` | everything else: geolocation, notifications, clipboard, MIDI, sensors… | `CefPermissionPromptCallback` → `Continue(cef_permission_request_result_t)` |
| `OnDismissPermissionPrompt` | cleanup when the prompt closes | — |

Returning `true` from the first two **takes over** the decision; `Continue`/`Cancel` may be called **async** — which is exactly what we need to round-trip through a React overlay. Enums (`cef_media_access_permission_types_t`, `cef_permission_request_types_t`, `cef_permission_request_result_t`) all live in `cef-binaries/include/internal/cef_types.h`.

> **Zero `cef/patch/` changes. Zero CEF rebuild. Zero runbook impact.** The entire permissions + hub feature is application-layer C++ + React + SQLite. The **only** patch-gated piece anywhere near this work is the optional valid-cert detail viewer, which is scoped OUT of v1.

---

## 3. Cross-Browser Standard (the bar we're meeting)

### Permission PROMPT
| | Anchor | Options | Persistence model |
|---|---|---|---|
| **Chrome 116+** | bubble under the left site-info icon | "Allow this time" / "Allow on every visit" / "Don't allow" (cam/mic/geo); older Allow/Block for the rest | duration baked into the **button** |
| **Brave** | same Chromium prompt | same trio + more aggressive defaults (sensors blocked) | same |
| **Firefox** | door-hanger at the permission glyph | "Allow" / "Block" + **"Remember this decision"** checkbox | persistence via the **checkbox** |

Convergent semantics everywhere: **one-time vs persistent**, plus **dismiss ≠ block** (clicking-away is a temporary dismissal, the site may re-prompt). After a block, all three show a **crossed-out capability glyph** in the address bar, clickable to re-open the controls.

### Site-info HUB (what the left icon opens)
Chrome (post-117 tune icon), Brave, and Edge all open a dropdown ordered: **connection status → cookies & site data → permissions summary (only perms the site actually used) → "Site settings" deep-link**. The hub **links out**; it is not the deep view. Brave additionally runs a **separate right-side Shields panel** (lion icon) for ad/tracker/script/fingerprint blocking — this two-panel split is the **direct precedent for Hodos's left-hub + right-shield design**. (Brave's own 2026 unification effort, issue #49746, is evidence two sources of truth cause friction → Hodos's hub Shields toggle should **delegate** to the right shield, not re-implement it.) Firefox fragments into several address-bar icons (lock / permissions / ETP shield).

### Connection
All majors: top row badge ("Connection is secure" / "Not secure" / "Certificate problem") that expands to connection detail → certificate viewer. The **badge + verdict** is the cheap, standard-meeting part; the **full cert viewer** is the historically expensive part.

### Permission catalog + defaults (verified against Chrome/Brave/Firefox)
| Permission | Chrome | Firefox | Brave | **Hodos default** | Phase |
|---|---|---|---|---|---|
| Camera | Ask | Ask | Ask | **Ask** | v1 |
| Microphone | Ask | Ask | Ask | **Ask** | v1 |
| Location | Ask | Ask | Ask | **Ask** | v1 |
| Notifications | Ask | Ask | Ask | **Ask** | v1 |
| Clipboard (read) | Ask | Ask | Ask | **Ask** | v1 |
| Motion/orientation sensors | Ask | n/a | **Block** | **Block** (privacy-forward, matches Brave) | v1 |
| Pop-ups & redirects | **Block** | Block | Block | **Block** | v1 |
| Sound / autoplay | Ask/Allow | Allow | Ask | **Allow** (don't break video) | v1 |
| MIDI (sysex) | Ask | Ask | Ask | Ask | v2 |
| USB / Serial / HID | Ask | n/a | Ask | Ask | v2 |
| Bluetooth | Ask | n/a | Ask | Ask | v2 |
| Automatic downloads | Ask | n/a | Ask | Ask | v2 |
| Payment handlers | Ask | n/a | Ask | Ask | v2 |
| File-system editing | Ask | n/a | Ask | Ask | v2 |
| Protected content / DRM | Ask | Ask | Ask | Ask | v2 |
| AR / VR | Ask | n/a | Ask | Ask | v2 |
| JavaScript / Images | Allowed | Allowed | Allowed | Allowed | — |
| Insecure content | Block | Block | Block | Block | — |

**v1 ships the 6 daily-driver capability prompts** — camera, microphone, location, notifications, clipboard, motion-sensors — wired through the real `CefPermissionHandler`. Everything else falls through to a default decision until v2.

---

## 4. The Hodos Design

### 4a. Site-Info "Site Controls" HUB

Left-anchored dropdown under the `TuneIcon` (inside the address bar, left side). Clones the just-landed bookmarks dropdown overlay (new role `"siteinfopanel"`). **The hub links out; only the Shields toggle is an inline action.**

```
   [ ⚙ example.com  ▾ ]  ← TuneIcon (MUI sliders) inside address bar, LEFT          NEW (icon)
    │
    ▼  (left-anchored dropdown, clone of bookmarks overlay — role "siteinfopanel")  NEW
 ┌─ Site controls — example.com ────────────────────────┐
 │  🔒  Connection is secure                       ›     │  (1) CONNECTION STATUS   EXISTING derivation
 │      Your data is private between you and this site   │      securityState useMemo
 │                                                       │      (MainBrowserView.tsx:391-405)
 │                                                       │      boolean only — Secure/Not-secure/Cert-problem
 ├───────────────────────────────────────────────────── ┤
 │  ⚙  Site permissions                            ›     │  (2) PERMISSIONS         NEW
 │      Camera, mic, location, notifications…            │      summary of used perms;
 │      (only perms this site has touched)               │      links OUT to mgmt deep-view
 ├───────────────────────────────────────────────────── ┤
 │  🛡  Shields for this site            [ ON  ●——]      │  (3) SHIELDS QUICK-TOGGLE EXISTING blocking
 │      Blocking ads & trackers                          │      delegates to usePrivacyShield
 │                                                       │      (redundant w/ right shield — accepted)
 ├───────────────────────────────────────────────────── ┤
 │  🍪  This site's data                           ›     │  (4) SITE-DATA LINK      EXISTING plumbing
 │      N cookies · view / clear                         │      → /browser-data?domain=<host>
 ├───────────────────────────────────────────────────── ┤
 │  💼  Manage Wallet Permissions                  ›     │  (5) WALLET-PERMS REUSE  EXISTING modal
 │                                                       │      → CreateNotificationOverlay
 │                                                       │        ("edit_permissions", domain)
 └───────────────────────────────────────────────────────┘
```

**Order rationale** mirrors Chrome/Brave: connection (trust anchor scanned first) → permissions → shields → site-data → wallet (Hodos-specific extra, visually last). Rows (1)(2)(4)(5) are navigation rows ("›"); row (3) is the only inline action.

**Win vs mac:** the hub React page is identical cross-platform. Only the native overlay shell differs (HWND/`WS_POPUP` + mouse-hook on Win; `NSWindow`/`DropdownOverlayView` + NSEvent monitor on mac — see §7).

### Left-hub vs Right-shield delineation (load-bearing split)

| | **LEFT hub** (TuneIcon, in address bar) — NEW | **RIGHT shield** (🛡 overlay) — EXISTING, unchanged |
|---|---|---|
| Identity | "what is this site / what can it do here" | "what am I blocking on this site" |
| Connection status | ✅ owns it (informational badge) | ✗ |
| Site permissions (cam/mic/loc/notif) | ✅ owns it (summary + deep-link) | ✗ |
| Cookies & site data | ✅ owns it (count + view/clear link) | ✗ |
| Shields on/off | ⚠️ quick-toggle ONLY (delegates to right) | ✅ full control |
| Blocking detail (counts, cookie-block, scriptlets) | ✗ (links out) | ✅ owns it (`usePrivacyShield`) |
| Wallet permissions | ✅ link to existing modal | ✗ |
| Link to "Privacy & Security" page | (optional) | ✅ keeps existing link |

**Rule of thumb:** the left hub answers *"tell me / let me manage this site"*; the right shield answers *"how hard am I blocking this site."* The only intentional overlap is the Shields on/off toggle in both places (Brave-style multi-path familiarity — explicitly accepted). Everything blocking-granular stays right; identity/permission/data stays left. The hub Shields toggle is a thin **delegating** shortcut to the existing per-site blocking state, not a parallel flag.

### 4b. SITE PERMISSIONS system

#### CefPermissionHandler wiring (see §2 for the no-patch confirmation)
- Override `OnRequestMediaAccessPermission` (camera/mic), `OnShowPermissionPrompt` (everything else), `OnDismissPermissionPrompt` on `SimpleHandler`.
- On a request: normalize the origin host, look up the per-site store.
  - **Stored Allow** → resolve `Continue(ACCEPT)` immediately (silent, no prompt).
  - **Stored Block** → resolve `Cancel()` / `Continue(DENY)` immediately (silent).
  - **Ask (no row)** → store the pending CEF callback in `PendingRequestManager` (already supports per-domain queuing + 6 request types — add a permission type), fire the prompt overlay, resolve on the React Allow/Block/AllowOnce IPC.
- **Secure-context rule:** camera/mic/geolocation only over HTTPS — refuse on insecure origins (tie to the existing `securityState` derivation), matching all browsers.
- Store a **Hodos-stable enum** for `permission_type`, mapping to/from the CEF bitflags at the callback boundary — so a Chromium bump that renumbers `cef_permission_request_types_t` can't corrupt stored rows.

#### Per-site SQLite store (NEW — clone `CookieBlockManager`)
New singleton `SitePermissionStore` mirroring `CookieBlockManager` (SQLite, `Initialize(user_data_path)` per-profile, `Shutdown()` checkpoint+close on the exit path). **Thread note:** permission callbacks fire on the **UI thread only**, so a plain `std::mutex` suffices (no `shared_mutex` needed unless the React-IPC read path reads concurrently — either is fine). Per-profile DB `site_permissions.db`.

```sql
CREATE TABLE IF NOT EXISTS site_permissions (
    domain          TEXT    NOT NULL,   -- normalized host (reuse CookieBlockManager::NormalizeDomain)
    permission_type INTEGER NOT NULL,   -- Hodos-stable enum, NOT raw CEF bit
    state           INTEGER NOT NULL,   -- 0=ask (default/absent), 1=allow, 2=block
    updated_at      INTEGER NOT NULL,   -- unix epoch ms
    PRIMARY KEY (domain, permission_type)
);
```
- **Ask = absence of a row** (or `state=0`) → matches Chrome's tri-state.
- **Reset for a site** = `DELETE` the domain's rows (mirrors `RemoveBlockedDomain`).
- **One-time grants** ("Allow this time") are **in-memory only**, cleared on tab close / navigate-away — never written to the DB. This aligns naturally with Hodos's existing per-session-resets-on-tab-close pattern.

#### Request PROMPT (REUSE the notification overlay — recommended)
Add a new `permission_request` type to the existing shared notification overlay (`BRC100AuthOverlayRoot.tsx` type-dispatch). Rationale: the overlay already keep-alives a warm React bundle, `CreateNotificationOverlay(type, domain, extraParams)` already exists on **both** platforms (Win `simple_handler.cpp:7838` / mac `:7842-7843`), and the async `CefPermissionPromptCallback::Continue` model maps cleanly to the existing IPC-response pattern (mirror `brc100_auth_response`). A brand-new overlay would duplicate all create/show/hide/mouse-hook/keyboard plumbing for no benefit.

Adopt **Chrome-style duration-in-the-button** (clearest, fewest clicks):

```
 ┌─────────────────────────────────────────────┐   NEW (new type on EXISTING overlay)
 │  📷  example.com wants to use your camera     │
 │                                               │
 │   ┌─────────────────┐  ┌──────────────────┐  │
 │   │ Allow this time │  │ Allow every visit│  │
 │   └─────────────────┘  └──────────────────┘  │
 │   ┌─────────────────────────────────────┐    │
 │   │            Don't allow               │    │
 │   └─────────────────────────────────────┘    │
 │                                          [✕]  │
 └─────────────────────────────────────────────┘
```
- **X / click-outside = temporary DISMISS** (`Continue(DISMISS)`; do NOT persist) — map the existing dropdown mouse-hook close to "dismissed," not "blocked."
- **"Allow this time"** → in-memory grant (`Continue(ACCEPT)`), cleared on tab close / navigate.
- **"Allow every visit" / "Don't allow"** → `Continue(ACCEPT/DENY)` + persistent row write.

> **Placement trade-off:** centered-modal reuse = ~zero new C++. A Chrome-faithful *anchored* bubble under the TuneIcon would need a small dropdown-style overlay (clone of bookmarks). **Default to reuse; anchored-bubble is a polish item** (Open Question 2).

#### Post-decision omnibox indicator
After **Don't allow**, show a **crossed-out capability glyph** (crossed camera/bell/pin) on the left of the address bar, clickable → re-opens the hub. Standard re-entry across all browsers.

#### Management UI (in the hub's "Site permissions" deep-view)
Show only permissions the site has **actually touched** inline (Chrome pattern), each a **3-state dropdown (Allow / Block / Ask)** — superset of every browser, clean map to the DB tri-state (a toggle can't express "Ask"). Plus **"Reset permissions for this site"** (`DELETE` all rows) and **"All site settings"** deep-link to the full catalog.

```
 ┌─ Permissions — example.com ─────────────────────────┐   NEW
 │   📷  Camera ............... [ Allow ▾ ]            │
 │   🎤  Microphone ........... [ Ask   ▾ ]            │
 │   📍  Location ............. [ Block ▾ ]            │
 │   🔔  Notifications ........ [ Ask   ▾ ]            │
 │                                                     │
 │   ↺  Reset permissions for this site                │
 │   →  All site settings                              │
 └──────────────────────────────────────────────────────┘
```

### 4c. Connection status presentation

**v1 = status-only**, reusing the existing `securityState` useMemo (`MainBrowserView.tsx:391-405`) which yields `'secure' | 'insecure' | 'error' | 'none'` from URL scheme + `activeTab.hasCertError`. `OnCertificateError` (`simple_handler.cpp:831`) only flips the boolean `tab->has_cert_error` — issuer/expiry are never read.

| State | Source | Badge | Sub-line |
|---|---|---|---|
| Secure | scheme `https` && no cert error | 🔒 **Connection is secure** | "Your data is private between you and this site." |
| Not secure | scheme `http` | ⓘ **Not secure** | "This site doesn't use a private connection." |
| Certificate problem | `hasCertError` | ⚠ **Certificate problem** | "There's an issue with this site's certificate." |
| Internal page | `hodos://` / `about:` | 🔧 **Hodos page** | (no cert row) |

Wording mirrors Chrome ("Connection is secure" / "Not secure") so it reads as familiar/standard.

**Why full cert detail is deferred:** research (Section 3) found CEF's embedder API (`GetVisibleNavigationEntry()->GetSSLStatus()` → `CefSSLStatus`/`CefX509Certificate`) **can** surface issuer/validity/serial/chain + mixed-content + TLS version for valid HTTPS **without a patch** — so the cost is lower than the original "needs a patch" framing. BUT: (1) it's currently **unverified** that `GetX509Certificate()` is populated on non-error pages (docs didn't confirm; needs a smoke test on e.g. github.com), and (2) Hodos has no per-load valid-cert capture wired today. **Decision for v1: ship status-only; treat the cert-detail view as a fast-follow** that, if the smoke test passes, needs **no patch** (only an in-overlay React cert card fed from a new small `Tab` struct cached on nav-commit). The only genuinely patch-gated thing is OS-native cert-dialog cosmetic parity, which we never need (we render fields ourselves). See Open Question 5.

---

## 5. Reuse Map (file:line)

| Area | Status | Anchor | Action |
|---|---|---|---|
| PermissionHandler wiring | PARTIAL (stub) | `simple_handler.cpp:7156` (`GetPermissionHandler` returns `this`, no overrides) | Override 3 callbacks — embedder API, no patch |
| CEF permission enums | EXISTS | `cef-binaries/include/internal/cef_types.h` (`cef_media_access_permission_types_t`, `cef_permission_request_types_t`, `cef_permission_request_result_t`) | Map to Hodos-stable enum |
| Per-site store | MISSING (clone) | `cef-native/include/core/CookieBlockManager.h` + `src/core/CookieBlockManager.cpp` | New `SitePermissionStore` singleton, UI-thread, `std::mutex`, `site_permissions.db` |
| Prompt overlay | EXISTS (reuse) | `BRC100AuthOverlayRoot.tsx` type-dispatch + `CreateNotificationOverlay` (`simple_handler.cpp:7838` Win / `:7842-7843` mac) | Add `permission_request` type |
| Pending-request queue | EXISTS | `PendingAuthRequest.h` (`PendingRequestManager`, 6 types, per-domain queue) | Add permission request type; park CEF callback |
| Hub overlay shell | EXISTS (clone) | bookmarks: `BookmarksOverlayRoot.tsx` + `Create/Show/HideBookmarksPanelOverlay` (`simple_app.cpp`) + `BookmarksPanelOverlayWndProc` (`cef_browser_shell.cpp`) + role in `BrowserWindow.cpp` | New role `"siteinfopanel"` |
| Connection status | EXISTS | `MainBrowserView.tsx:391-405` (`securityState`); `Tab::has_cert_error` (`Tab.h`); `OnCertificateError` (`simple_handler.cpp:831`) | Reuse derivation; pass into hub |
| Site cookies/data | EXISTS | `useCookies.ts` (`cookie_get_all` → JS domain-group; `deleteDomainCookies` → `cookie_delete_domain`) | Client-side filter to host; no new backend |
| Site history filter | PARTIAL | `HistoryManager::SearchHistory(HistorySearchParams)` (`HistoryManager.h`) — `search_term` substring only, no `domain` column | Pass host as search term (good enough); exact-host param optional |
| Site-data deep view | EXISTS | `/browser-data` (`HistoryPage.tsx`, cookies/history tabs) | Read `?domain=<host>` query param, pre-select tab/filter (small FE wiring) |
| Wallet-perms modal | EXISTS | `MENU_ID_MANAGE_PERMISSIONS` def `:7528`, label `:7612`, handler `:7821`, overlay `:7838`/`:7842-7843` | Rename label → "Manage Wallet Permissions"; add hub trigger to same path |

---

## 6. Implementation Plan / Phasing

Three ordered, independently-committable sub-chunks for the per-chunk harness.

> **Progress (2026-06-20):** b1 was split into **b1a (engine + store, no prompt — non-regressive)** and **b1b (the Hodos-branded prompt)**. **b1a ✅ committed** — `SitePermissionStore` + the two `CefPermissionHandler` overrides (honor stored Allow/Block silently; defer "Ask" to Chromium's stock prompt). Adversarial review (reviewer+skeptic) found no critical bugs; fixed 5 confirmed findings (secure-context guard, unanimous-only collapse, IPv6/trailing-dot keying, PTZ→Camera). v1 capability set is **5 (camera, mic, location, notifications, clipboard)** — motion-sensors dropped (not exposed by `cef_permission_request_types_t`; would need a patch). **NEXT: b1b** = replace Chromium's "Ask" prompt with the Hodos-branded one (notification overlay), capture the choice into the store, lighting up the silent-honor path.

### b1a — Permission engine + store (no prompt)  · RISK: MEDIUM · ✅ DONE
### b1b — Hodos-branded prompt + parked-callback lifecycle  · RISK: MEDIUM · ✅ DONE (smoke-passed)
> Replaces Chromium's "Ask" prompt with the Hodos prompt (notification overlay, `permission_request` type) + `PendingPermissionManager` (park/resolve), `permission_response` IPC, persist always/block to the store. Focused adversarial review found real lifecycle defects (media-callback leak on close/nav, no watchdog, shared-overlay modal collision, unconditional hide) — ALL fixed: parking `browserId` + `OnBeforeClose` drain, 60s `SweepStalePermissions`, wallet-modal guard (`g_pendingModalDomain`), guarded hides, button-disable, `popByPromptId` guard. Live smoke confirmed allow_always→silent, navigate-away→no hang. Residual (bounded): wallet-modal firing over a permission prompt cancels it via the watchdog (full prevention = wallet-side guard, deferred). **NEXT: b1b.1** = allow-once SESSION memory (per-tab, cleared on host-change/close) so "Allow this time" matches Chrome instead of re-prompting every request.

### b1b.1 — Allow-once session memory  · RISK: LOW · ✅ DONE (smoke-passed)
> Ephemeral per-tab "Allow this time" grants (Chrome parity): re-requests silently allowed until navigate-away (host change) or tab close; never persisted. In-memory in `PendingPermissionManager` + `EffectiveState` (Ask→Allow promotion) + clears in `OnAddressChange`/`OnBeforeClose`. **b1 (engine + prompt) COMPLETE. NEXT: b2** = site-info hub + in-UI management/reset.

### b1 — Permission engine + store + prompt  · RISK: MEDIUM
The core new system; the only chunk that touches the live permission hot path.
- Override `OnRequestMediaAccessPermission` / `OnShowPermissionPrompt` / `OnDismissPermissionPrompt` on `SimpleHandler`.
- New `SitePermissionStore` SQLite singleton (clone `CookieBlockManager`; UI-thread `std::mutex`; `site_permissions.db`; `Shutdown()` checkpoint).
- Hodos-stable permission enum + CEF-bit mapping at the callback boundary.
- Park pending CEF callback in `PendingRequestManager` (new permission type).
- New `permission_request` type on the notification overlay (`BRC100AuthOverlayRoot.tsx`) + Allow-this-time / Allow-every-visit / Don't-allow + dismiss=`DISMISS`.
- IPC response (mirror `brc100_auth_response`) resolves the parked callback.
- Secure-context refusal for cam/mic/geo on insecure origins.
- **Risk callout:** this is the one chunk on the live permission path. Until it ships, CEF's stock popup shows (current behavior) — so b1 must be feature-complete-and-tested before the override goes live (a half-wired override could swallow real permission requests). Smoke against a getUserMedia + geolocation + notifications test page. Verify the 1-time grant clears on tab close.

### b2 — Site-info hub + management UI + connection badge  · RISK: LOW
Pure additive overlay; no hot-path impact.
- New `SiteInfoOverlayRoot.tsx` + route; clone bookmarks plumbing → role `"siteinfopanel"` (`simple_app.cpp` create/show/hide, `cef_browser_shell.cpp` WndProc + HWND global + mouse-hook, `BrowserWindow.cpp` role dispatch, `App.tsx` route).
- TuneIcon in the address bar (left) → `siteinfo_panel_show` IPC.
- Hub layout (§4a): connection badge (reuse `securityState`), permissions summary + deep-view (3-state dropdown + reset), Shields delegating toggle (`usePrivacyShield`), site-data link, wallet-perms link.
- Post-decision crossed-out omnibox glyph (re-opens hub).
- **Risk:** low — additive overlay, reuses existing state. Main risk is left-anchor positioning math (shared with bookmarks; already solved on Win).

### b3 — Wallet-perms rename + site-data deep link  · RISK: LOW
Smallest chunk; pure reuse + wiring.
- Rename right-click label `"Manage Site Permissions"` → `"Manage Wallet Permissions"` (`simple_handler.cpp:7612`); keep the entry; hub fires the same `CreateNotificationOverlay("edit_permissions", domain)` path.
- `/browser-data` reads `?domain=<host>` + `?tab=` query params, pre-selects tab and filters to origin (cookies via client-side group; history via `search_term=host`).
- **Risk:** low — no new backend, no new overlay.

> Order: **b1 → b2 → b3.** b2/b3 can technically land before b1 (hub renders the management UI reading the store even while the live override still defers to CEF's popup), but b1's store must exist first for b2's management UI to read/write. Recommend b1 first.

---

## 7. macOS Deltas (for MACOS_PORT_0_4_0.md)

**Already cross-platform — no mac work:**
- **CefPermissionHandler overrides** — `simple_handler.cpp` is a single cross-platform file; the handler + overrides are platform-neutral embedder API. No `.mm` work for the handler, no patch.
- **`SitePermissionStore`** — SQLite + cross-platform per-profile path resolution, like every other browser-DB manager. Per R2/R3, the **mac DB-cascade in `main()` shutdown is deferred** (not all SQLite managers are wired into the mac shutdown yet) — fold this manager's `Shutdown()` into that future mac cascade, not now.
- **Prompt overlay** (if it reuses the notification overlay — recommended) — `CreateNotificationOverlay(type, domain, extraParams)` already exists on mac (`cef_browser_shell_mac.mm:~3454`); adding a `permission_request` React case is pure cross-platform. **No new mac overlay for the prompt.**
- **"Manage Wallet Permissions"** — already wired cross-platform with a working `#elif defined(__APPLE__)` arm (`:7842-7843`). Right-click label rename is shared source. **Zero mac delta.**
- **"This site's data" link** — cookie panel exists on mac; `/browser-data` is a `navigate`. Link is IPC + navigate only. **No new mac overlay.**

**New mac work — the `"siteinfopanel"` LEFT-anchored dropdown** (near-exact clone of the bookmarks-overlay mac checklist in `MACOS_PORT_0_4_0.md`):
- [ ] `NSWindow* g_siteinfo_panel_overlay_window` + click-outside monitor + last-hide-time globals (alongside profile/bookmarks, `cef_browser_shell_mac.mm:~256-266`).
- [ ] `CreateSiteInfoPanelOverlayMacOS(int iconLeftOffset)` / `Show` / `Hide` / `IsVisible` / `WasJustHidden` + `Install/RemoveSiteInfoPanelClickOutsideMonitor`, cloned from the profile block (`:3991-4118`). Use `DropdownOverlayView`, `browserAccessor = ^{ return SimpleHandler::GetSiteInfoPanelBrowser(); }`, URL `/site-info`, role `"siteinfopanel"`, `makeKeyAndOrderFront` + `makeFirstResponder:contentView`.
- [ ] **LEFT-anchor X** hand-rolled like the omnibox (`:3787`: `overlayX = contentScreen.origin.x + iconLeftOffset`), **NOT** `CalculateToolbarOverlayFrame` (right-only). `iconLeftOffset` is **CSS px / points — apply directly, NO `ScalePx`** (double-offsets on Retina); then `ClampOverlayToScreen`.
- [ ] Forward declarations in the extern block (`~:579-581`).
- [ ] Replace the no-op `#elif defined(__APPLE__)` arm for `siteinfo_panel_show` in `simple_handler.cpp` (+ `_hide` + menu trigger).
- [ ] Cross-platform browser-process bits (run on mac once consumed): role `"siteinfopanel"` in `SetBrowserForRole`/`GetBrowserForRole` (`BrowserWindow.cpp`); `GetSiteInfoPanelBrowser()` static; `BrowserWindow` mac fields in the `__APPLE__` section of `BrowserWindow.h`.
- [ ] Add `g_siteinfo_panel_overlay_window` to `InstallAppFocusLossHandler`'s close list (`OverlayHelpers_mac.mm:~216`) + shutdown cleanup (`cef_browser_shell_mac.mm:~4282-4293`).

**macOS-specific permission nuance — THE real risk (flag for owner):**
- **macOS TCC double-prompt.** macOS gates camera/mic/location/screen-recording at the **OS level via TCC**, independent of Chromium. After our `OnRequestMediaAccessPermission` allows, **macOS shows its own native TCC prompt** ("HodosBrowser would like to access the Camera"), granted **once per app** (not per-site). So a mac user can see **two prompts** on first camera use (our per-site prompt, then the OS TCC prompt). This is normal Chromium-on-mac behavior (Chrome/Brave do the same), but:
  1. The mac bundle `Info.plist` **MUST** declare `NSCameraUsageDescription` / `NSMicrophoneUsageDescription` / `NSLocationUsageDescription` or OS access **silently fails / crashes** instead of prompting — verify before shipping mac.
  2. If our store says **Block**, deny in `OnRequestMediaAccessPermission` so the TCC prompt never fires (good).
  3. If the user later revokes in System Settings → Privacy, our store still says "Allow" but capture fails — our UI's "Allow" **cannot override TCC**. Document the divergence; don't try to reconcile the two layers.
- Other mac checks: Retina points-vs-pixels on the left X (no `ScalePx`); `makeFirstResponder:contentView` for OSR keyboard; verify the new `permission_request` type doesn't collide with in-flight `edit_permissions`/BRC-100 types sharing the single `g_notification_overlay_window` keep-alive (sequence them).

---

## 8. Open Questions / Decisions for the Owner

1. **Permission defaults sign-off** — confirm the §3 table, especially **motion-sensors = Block** (privacy-forward, matches Brave but differs from Chrome's "Ask") and **autoplay = Allow** (don't break video). And confirm the **v1 set = 6 capabilities** (camera, mic, location, notifications, clipboard, motion-sensors) with the rest deferred to v2.
2. **Prompt-overlay choice** — **reuse the notification overlay** (centered modal, ~zero new C++, recommended) vs a **new anchored bubble** under the TuneIcon (Chrome-faithful placement, but a new overlay HWND + mac creation fn). Recommend reuse for v1, anchored-bubble as later polish.
3. **"Site data" link target** — deep-link to `/browser-data?domain=<host>` with cookies tab pre-selected and history filtered by `search_term=host` (substring). Acceptable, or do we want an exact-host history filter (small new `HistorySearchParams.domain` param)?
4. **Both Shields paths?** — keep the Shields on/off toggle in BOTH the left hub (delegating) AND the right shield? Owner already accepted the redundancy (multi-path familiarity); confirming the hub toggle **delegates** to existing per-site state rather than introducing a parallel flag.
5. **Cert scope** — confirm v1 ships **connection status only** (Secure / Not secure / Cert problem). Research found full cert detail is likely achievable **without a patch** via `CefSSLStatus`/`CefX509Certificate` (pending one smoke test on a valid HTTPS page). Do we want to **schedule that smoke test now** to decide whether basic cert detail can be a cheap fast-follow, or hard-defer the entire cert-detail view to the Engine release?
6. **Wallet-perms label rename** — confirm renaming the right-click `"Manage Site Permissions"` → `"Manage Wallet Permissions"` (keeping the entry), so "Site Permissions" unambiguously means the new OS-capability permissions.
