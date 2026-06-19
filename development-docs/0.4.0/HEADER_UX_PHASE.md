# Header / Omnibox UX Pass — Design (0.4.0 Shell release, item 2)

**Status:** DESIGN — awaiting go/no-go from owner.
**Branch:** `0.4.0`
**Author:** synthesis of cross-browser research (Chrome/Brave/Vivaldi/Firefox) + backend inventory + macOS-parity audit.

> This is a design doc. No code is written until the owner signs off on the open questions in §9.

---

## 0. Decisions LOCKED (owner sign-off 2026-06-19)

All §9 open questions resolved + two placement overrides from the owner:

- **Placement override — Bookmarks button → LEFT.** Bookmark button is a standalone toolbar button **between `⟳` Reload and the address bar** (left of the in-URL site-info glyph), NOT in the right cluster. `[◀][▶][⟳] [🔖] ┌ ⚙ url … [logo][🛡] ┐ [⬇][💰][👤][⋮]`. Its dropdown anchors below-left of the button → hand-rolled X (not right-anchored) on both platforms.
- **Placement override — Tab-list caret → LEFT of the tab strip.** `[⌄][tab][tab][+]`. **Windows:** far-left, clean. **macOS:** the caret starts AFTER the 86px traffic-light reservation (`●●● [⌄][tab][tab][+]`); the inset is derived at RUNTIME from `NSWindow.standardWindowButton` frames — never hardcoded. (Supersedes the §3/§9-Q6 right-edge recommendation.)
- **Q1** — Ctrl+D = **silent toggle + brief confirmation** (omnibox star fills solid). No Chrome-style edit popup.
- **Q2** — Bookmarks v1 = **flat searchable list + tag chips**; folder tree **deferred** (folder CRUD exists in C++ but has no IPC bridge).
- **Q3** — Cert viewer v1 = **connection-status string + error-cert OpenSSL parse**; full good-cert viewer **deferred** to the Engine release (needs a `cef/patch/` Chromium patch). Accepted.
- **Q4** — **Keep the Brave LEFT/RIGHT split.** Site-info (left) = connection/cert + cookies (view + "Manage" deep-link) + permissions + wallet-perms. Privacy Shield (right) = ad/tracker/fingerprint/block-stats. No duplicated cookie UI; no merge.
- **Q5** — Adopt **Ctrl/Cmd+Shift+A** to open the tab-list overlay.
- **Q7** — `browser.showDownloadsWhenDone` default **ON**.
- **Q8** — **Pull B2-FILL in as a pre-chunk** (correctness prerequisite for anchoring the new controls). FEAT-DPI stays a separate parallel investigation; centralize the offset math into one helper now + mixed-scale multi-monitor smoke per chunk.
- **Q9** — Site-info glyph = **tune/sliders** (not a padlock); lock state shown as a row inside the flyout.
- **Q9/Q10 — FINAL 2026-06-19:** tune `⚙` icon (MUI `TuneIcon`) is the single clickable site-info control; **no standalone positive lock, no pipe divider**. Connection state shown via the tune icon's color + a "Not secure" pill for HTTP/cert-error. Removes the old passive `securityState` Lock indicator. (Modern Chrome/Brave pattern — see §5b.)

### Progress
- **Pre-chunk B2-FILL — DONE (pending commit).** Root cause: a vestigial `width/height: calc(100% + 16px); margin: -8px` hack in `MainBrowserView.tsx` root Box compensating for an 8px UA body margin that `index.css`/`index.html` already reset to 0 → 16px overflow + `-8,-8` shift (top-clip + ~9px dark strip under the toolbar). Fixed to `width/height: 100%; margin: 0`. Frontend builds clean; owner confirmed header fills better. Not yet committed.
- **Next chunk:** (d) Downloads auto-hide.

---

## 1. Goal & scope

A single coordinated pass over the browser header / omnibox that adds five user-facing controls, reusing existing backends wherever they already exist (most do). The five features:

| # | Feature | One-line |
|---|---------|----------|
| **a** | **Bookmarks** | A non-star bookmark **button** (Firefox/Edge precedent — Brave has none) right of the address bar opening a `BookmarksOverlayRoot` dropdown (current page + star toggle on top, searchable list below). Ctrl+D toggles the star. Un-stub the three-dot-menu Bookmarks action. |
| **b** | **Site-info button** | A leading-icon button **inside the LEFT of the address bar** (Chrome "tune"/sliders style) opening a `SiteInfoOverlayRoot`: connection/cert state, cookies & site data, site permissions. Delineated against the existing right-side Privacy Shield. |
| **c** | **Wallet permissions** | A read-only wallet-permission **summary + deep-link** section inside the site-info dropdown that opens the **existing** `MENU_ID_MANAGE_PERMISSIONS` modal. Keep the right-click entry. Rename label → "Manage Wallet Permissions". |
| **d** | **Downloads auto-hide** | Restore Chrome-like behavior: NO download icon until a download starts; animate it in with a progress ring; show a complete indicator; click to open/clear; once cleared the icon disappears again. (Regression — was removed by commit `ec18d29`.) |
| **e** | **Tab-list caret** | A small caret at the **right edge of the tab strip** opening a `TabListOverlayRoot`: search box + open tabs (TabManager) + recently-closed (HistoryManager). |

### Explicitly IN
- Non-star bookmark button + dropdown overlay; omnibox star toggle kept separate.
- Ctrl+D **toggle** (currently add-only — see §5a).
- Site-info dropdown overlay (LEFT) — connection summary, cookies/site-data, site permissions, wallet-perms deep-link.
- Wallet-perms reuse + the "Manage Site Permissions" → "Manage Wallet Permissions" rename (`simple_handler.cpp:7436`).
- Downloads auto-hide/animate-in/complete-indicator restoration.
- Tab-list caret + dropdown (right-edge placement on BOTH platforms).

### Explicitly OUT (scoped out this pass)
- **No horizontal bookmarks bar.** (`browser.showBookmarkBar` setting exists but stays unused.)
- **No HTML bookmark import/export.** Browser-profile import already exists in Settings (`import_bookmarks` → `ProfileImporter`).
- **No bookmark folder tree in v1.** Backend has folder CRUD but **no IPC bridge** exists; v1 uses a flat searchable list with optional tag chips (`bookmark_get_all_tags`).
- **No full good-cert certificate viewer in v1.** Stock CEF only delivers `CefSSLInfo` in `OnCertificateError` (bad certs); a real good-cert viewer needs a `cef/patch/` Chromium patch. v1 = connection-status string + (for error certs) OpenSSL-parsed issuer/expiry. Full viewer is a flagged follow-up (§9 Q3).
- **No Vivaldi-style merge of site-info + privacy shield.** We keep Brave's split deliberately (§5b).

---

## 2. Current header icon inventory ("before" map)

Source: `frontend/src/pages/MainBrowserView.tsx` (note: lives in `pages/`, NOT `views/` — research drift), left → right.

```
WINDOWS — TODAY
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ [tab][tab][tab][+]                                                                      │  ← TabBar (no caret)
├──────────────────────────────────────────────────────────────────────────────────────┤
│ [◀ Back][▶ Fwd][⟳ Reload]   ····· flex ·····                                            │
│   ┌────────────────────────────────────────────────────────┐                           │
│   │ 🔒  https://example.com/...            [Hodos logo] [🛡] │   ····· flex ·····        │
│   └────────────────────────────────────────────────────────┘                           │
│      ▲ security indicator        ▲ logo (right,inside)  ▲ Privacy Shield (right,inside)  │
│      (Lock/LockOpen/Error,        decorative           → cookie_panel_show               │
│       pointerEvents:none)                              → loads /privacy-shield           │
│                                                                                          │
│        ····· flex ·····   [⬇ Download][💰 Wallet][👤 Profile][⋮ Menu]                    │
│                            ▲ ALWAYS VISIBLE (regression)                                 │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

Right-cluster (existing, `MainBrowserView.tsx:815-933`): **Download** (`download_panel_show`, lines 817-852, always visible) → **Wallet** (`toggle_wallet_panel`, 854-885) → **Profile** (`profile_panel_show`, 887-915) → **Menu** (`menu_show`, 917-931). Inline **FindBar** (936+).

Inside the address bar: **security indicator** (left, `pointerEvents:none`, Lock/LockOpen/Error driven by `securityState`), **Hodos logo** (right, decorative), **Privacy Shield `SecurityIcon`** (right, sends `cookie_panel_show` → C++ loads `/privacy-shield`).

> ⚠️ The existing in-omnibox `SecurityIcon` already opens the Privacy Shield panel. The site-info button (b) overlaps it heavily on cookies/site-data — the delineation rule in §5b is load-bearing.

---

## 3. Target header/omnibox icon inventory ("after" map)

Legend: `[NEW]` = added this pass · `[EXIST]` = already present · `[RESTORE]` = behavior restored.

```
WINDOWS — AFTER  (placement per §0 owner override)
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ [⌄][tab][tab][tab][+]                                                                       │  ← tab-list caret, LEFT of strip (e)
│  ▲ [NEW]                                                                                     │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ [◀][▶][⟳]  [🔖]  ┌──────────────────────────────────────────────┐                          │
│             ▲NEW │ ⚙  https://example.com/...      [logo] [🛡]   │  ····· flex ·····         │
│             (a)  └──────────────────────────────────────────────┘                          │
│             book   ▲ SITE-INFO [NEW] (b)        [EXIST]  [EXIST] Privacy Shield (RIGHT,stays) │
│             glyph    replaces passive Lock;    decorative                                    │
│                      CLICKABLE; tune/sliders;                                                │
│                      secure/not-secure/mixed/internal                                        │
│                                                                                             │
│   ····· flex ·····                                  [⬇ Download][💰 Wallet][👤][⋮]           │
│                                                      ▲ [RESTORE] (d)  [EXIST] [EXIST][EXIST] │
│                                                      hidden until first download; animates   │
│                                                      in; progress ring; complete pulse;      │
│                                                      hides when cleared                      │
└──────────────────────────────────────────────────────────────────────────────────────────┘
   ⭐ omnibox STAR toggle (Ctrl+D) — kept SEPARATE from the bookmark button. Star = "bookmark THIS page";
      bookmark button = "open my bookmarks". (Chrome/Firefox split.)
```

```
macOS — AFTER  (deltas: traffic lights top-left; TabBar reserves 86px left inset)
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ ●●●  [⌄][tab][tab][tab][+]                                                                  │  ← caret LEFT, but inset AFTER the
│  ▲ traffic lights      ▲ caret starts at the 86px traffic-light reservation (TabBar.tsx:275)  │     86px traffic-light reservation;
│    (left ~70px)          inset DERIVED AT RUNTIME from NSWindow.standardWindowButton frames    │     NOT hardcoded
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ [◀][▶][⟳]  [🔖]  ┌──────────────────────────────────────────────┐                          │
│             ▲NEW │ ⚙  https://example.com/...      [logo] [🛡]   │  ····· flex ·····         │
│             (a)  └──────────────────────────────────────────────┘                          │
│             both (a) bookmark btn + (b) site-info sit well right of the 86px inset → NO       │
│             traffic-light conflict. BOTH dropdowns anchor LEFT/below-button → mac creation    │
│             fns must HAND-ROLL X (omnibox pattern, cef_browser_shell_mac.mm:3787);            │
│             CalculateToolbarOverlayFrame is right-only (use only for the ⬇/💰/👤/⋮ cluster). │
│                                                                                             │
│   ····· flex ·····                                  [⬇][💰][👤][⋮]  (right-cluster flush-right│
│                                                                       via CalculateToolbar…)  │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

**Placement rules (both platforms):**
- **Tab-list caret (e)** = **LEFT of the tab strip**. Windows: far-left. macOS: starts after the 86px traffic-light reservation (runtime-derived from `NSWindow.standardWindowButton`, never hardcoded).
- **Bookmarks (a)** = standalone toolbar **icon button between `⟳` Reload and the address bar** (left of the in-URL site-info glyph). Dropdown anchors below-left of the button → hand-rolled X on both platforms (NOT the right-anchored helper).
- **Site-info (b)** = leading-icon slot **inside the LEFT of the address bar** — unaffected by mac traffic lights (URL field sits well right of the 86px inset). Replaces the passive `securityState` indicator with a clickable button.
- **Download (d)** = right-cluster toolbar icon button (allowed — triggers an overlay), restored to hidden-until-download.
- **Privacy Shield (🛡)** stays exactly where it is (right-inside address bar) — see §5b delineation.
- The remaining right cluster (`⬇ 💰 👤 ⋮`) keeps `CalculateToolbarOverlayFrame` right-anchoring on mac.

---

## 4. Architecture-rule compliance

Per root CLAUDE.md: the header (`MainBrowserView.tsx`) may ONLY hold tab bar, nav buttons, address-bar input, toolbar icon **buttons that trigger overlays**, and the inline find bar. Every panel is an overlay in its own CEF subprocess.

| Feature | Header addition (allowed) | Overlay (own CEF subprocess) | Status |
|---------|---------------------------|------------------------------|--------|
| a | bookmark trigger button | `BookmarksOverlayRoot.tsx` route `/bookmarks` | NEW overlay |
| b | site-info trigger button (inside URL) | `SiteInfoOverlayRoot.tsx` route `/site-info` | NEW overlay |
| c | (none — lives in b's dropdown) | reuse `notification_browser_` `edit_permissions` overlay | REUSE |
| d | download trigger button (restore guard) | `DownloadsOverlayRoot.tsx` route `/downloads` | EXIST overlay |
| e | tab-list caret button | `TabListOverlayRoot.tsx` route `/tab-list` | NEW overlay |

All four new/reused dropdowns follow the **keep-alive dropdown** pattern (Cookie/Download/Menu/Profile family): created once, `ShowWindow(SW_SHOW/SW_HIDE)` on Windows, `WH_MOUSE_LL` click-outside; NSPanel + `NSEvent` click-outside + focus-loss observer on macOS.

---

## 5. Per-feature design

### (a) Bookmarks — button + `BookmarksOverlayRoot`

**UX.** A non-star bookmark button (book/ribbon glyph) as a **standalone toolbar button between `⟳` Reload and the address bar** (§0 owner override — NOT in the right cluster). Click opens a dropdown anchored below-left of the button (hand-rolled X, not the right-anchored helper) copying the `DownloadsOverlayRoot` skeleton:

```
┌─────────────────────────────────────────────┐
│  CURRENT PAGE                                 │
│  Example Page Title              [ ☆ / ★ ]    │  ← star toggles THIS page (bookmark_add/remove)
│  https://example.com/...                      │
│  ───────────────────────────────────────────  │
│  🔍 [ Search bookmarks…                     ] │  ← bookmark_search
│  ───────────────────────────────────────────  │
│  All Bookmarks                                │
│   ● GitHub          github.com         ⋯      │  ← favicon, title, host, kebab(edit/remove)
│   ● WhatsOnChain    whatsonchain.com   ⋯      │
│   …(scrollable, bookmark_get_all)             │
│  ───────────────────────────────────────────  │
│  [ tag-chip ] [ tag-chip ]  (optional, v1)    │  ← bookmark_get_all_tags
└─────────────────────────────────────────────┘
```

**Backends to reuse (all EXIST — no new backend):**
- C++ `BookmarkManager` — `cef-native/include/core/BookmarkManager.h`, impl `cef-native/src/core/BookmarkManager.cpp`. Full SQLite CRUD + folder CRUD (folder CRUD has NO IPC bridge → out of scope v1).
- IPC suite — `simple_handler.cpp` ~5907-6136: `bookmark_add`, `bookmark_get`, `bookmark_update`, `bookmark_remove`, `bookmark_search`, `bookmark_get_all`, `bookmark_is_bookmarked`, `bookmark_get_all_tags`, `bookmark_update_last_accessed`. Each fires a `*_response` IPC (callback pattern §Communication-2).
- Browser-profile bookmark import — `simple_handler.cpp:3009` (`import_bookmarks`) — already in Settings.

**New work:**
- `frontend/src/pages/BookmarksOverlayRoot.tsx` + route in `App.tsx` (`/bookmarks`).
- `frontend/src/hooks/useBookmarks.ts` (NEW — none exists) using the `cefMessage.send → window.onXxxResponse` callback pattern.
- C++ show/hide handler `bookmarks_panel_show` in `simple_handler.cpp` + Windows creation fn in `simple_app.cpp` (clone `CreateDownloadPanelOverlay`) + mac creation fn in `cef_browser_shell_mac.mm` (clone `CreateDownloadPanelOverlayMacOS`, but **left/below-button anchored** → hand-roll X like the omnibox, NOT `CalculateToolbarOverlayFrame`).
- Bookmark trigger button in `MainBrowserView.tsx` **between Reload and the address bar** (left toolbar slot — §0 override).

**Behaviors / shortcuts:**
- **Ctrl+D toggle.** TODAY `simple_handler.cpp` Ctrl+D handler (~7227-7242) is **add-only** — it always `AddBookmark`s. Add an `IsBookmarked` → `RemoveBookmark` branch so Ctrl+D toggles. On add, animate the omnibox star to filled-solid (Chrome precedent — easing is a Hodos choice, §9 Q1).
- **Un-stub menu action.** `simple_handler.cpp:2534` `else if (action == "bookmarks") { /* TODO */ }` → fire `bookmarks_panel_show`. (`MenuOverlay.tsx` already dispatches `handleAction('bookmarks')`.)
- Omnibox **star toggle stays separate** from the bookmark button (Chrome/Firefox split).

**Doc reconciliation:** `development-docs/0.4.0/B3-bookmarks.md` says "Research pending / not source-verified" — **STALE**. Backend + IPC + Ctrl+D + import are all built; only UI (button + overlay + hook), the menu un-stub, and the Ctrl+D toggle branch remain. Update B3 when this lands.

**Risk tier:** Light-Medium (new overlay, but pure reuse of a complete backend).

---

### (b) Site-info button — `SiteInfoOverlayRoot` (LEFT, inside address bar)

**UX.** A clickable leading-icon inside the left of the URL field, opening a LEFT-anchored dropdown.

**Glyph (LOCKED, Q9 + owner confirm 2026-06-19):** the **tune/sliders icon** ("o–" over "–o" — two horizontal sliders) = Chrome's "view site information" control since **Chrome 117**; Brave matches. Use MUI **`TuneIcon`**. NOT a padlock.

**Connection-state model (LOCKED 2026-06-19 — modern Chrome/Brave pattern):** NO standalone positive lock and NO pipe divider. `[◀][▶][⟳] [🔖] ┌ ⚙ https://example.com/… [logo][🛡] ┐`
- **`⚙`** tune button is the single clickable control; **its color/state encodes connection security** (neutral for HTTPS, warning-tinted for HTTP / cert error).
- **HTTP / insecure** → a small **"Not secure"** pill next to the tune button (driven by URL scheme + `has_cert_error`).
- **Rationale:** the positive lock was retired by Chrome 117 / Brave because users misread "encrypted" as "safe/legitimate" and phishing sites all have free HTTPS certs now; HTTPS is the norm (~95%+), so the modern pattern warns on the *exception* (HTTP/errors) rather than badging the norm. The passive lock + pipe are dropped entirely (supersedes the earlier `⚙ | 🔒` refinement). This removes the old `securityState` Lock/LockOpen indicator; the Error/cert-error state becomes the "Not secure" surface.

Click opens a LEFT-anchored dropdown:

```
┌─ Site info ───────────────────────────┐
│  example.com                           │
│  🔒 Connection is secure          ›    │  → expands: cert summary + "View certificate"
│  ───────────────────────────────────   │     (scope: see below)
│  Cookies & site data                   │
│    12 cookies in use         [Manage]  │  → opens existing Cookie/Privacy panel + cookie IPC
│  ───────────────────────────────────   │
│  Permissions (camera, location…)  ›    │  → inline summary OR deep-link
│  ───────────────────────────────────   │
│  WALLET  (feature c)                    │
│    Identity-key disclosure  [Allowed]  │  ← read-only chips from domain_permissions
│    Auto-approve payments    [On·caps]  │
│    Scoped grants (3)            ›       │
│  [ Manage Wallet Permissions ]    ───►  │  → reuse MENU_ID_MANAGE_PERMISSIONS modal
└─────────────────────────────────────────┘
   ⛔ NO ad/tracker/fingerprint controls — those live ONLY in the right Privacy Shield.
```

**Icon state machine** (drive off existing load state): secure / not-secure (HTTP) / mixed-content / internal-or-file — the four Chromium states. Today Hodos derives secure/insecure from URL scheme + `Tab::has_cert_error` boolean (`Tab.h:74`, surfaced as `tab_json["hasCertError"]`, consumed in `MainBrowserView.tsx` `securityState`).

**Backends to reuse (EXIST):**
- Cookie data/management — `CookieManager.h` / `CookieBlockManager.h` / `EphemeralCookieManager.h` + cookie IPC (`cookie_get_all`, `cookie_delete*`, `cache_clear`, `cache_get_size`); hooks `useCookies`, `useCookieBlocking`.
- Connection boolean — `OnCertificateError` (`simple_handler.cpp` ~797-864), `Tab::has_cert_error`.
- Wallet-perms section — see (c).

**Cert viewer — honest scope.** Stock CEF does NOT expose per-navigation SSL on the happy path. `CefSSLInfo` (→ `CefX509Certificate` → `GetDEREncoded()`, parse with OpenSSL — already a vcpkg dep) is only delivered in `OnCertificateError`, i.e. **only for bad certs**. A real good-cert detail viewer requires a `cef/patch/` Chromium source patch (consistent with the "we patch Chromium" model). **v1 scope:** connection-status string + issuer/expiry for *error* certs via OpenSSL; full good-cert viewer is a flagged patch-scale follow-up (§9 Q3).

**Delineation vs Privacy Shield (LOAD-BEARING — §5b rule).**

| LEFT — Site-info (NEW, b) | RIGHT — Privacy Shield (EXIST) |
|---------------------------|--------------------------------|
| Connection / cert state | Ad / tracker blocking |
| Cookies & site data (view + clear) | Fingerprint protection |
| Site permissions (camera/location/…) | Cosmetic/scriptlet toggles |
| Wallet permissions (deep-link, c) | Per-site block counts/stats |

This is **Brave's exact split** (left lock = site-info/permissions; right lion = privacy protections). Do **not** adopt Vivaldi's merged model — Brave's own community complains about the resulting redundancy. The one genuine overlap is *cookies*: site-info shows cookie **count + a "Manage" deep-link** into the existing cookie surface; it does NOT duplicate the cookie-blocking toggles (those stay in the shield). Document this so nobody builds two cookie UIs.

**New work:**
- `frontend/src/pages/SiteInfoOverlayRoot.tsx` + route `/site-info`.
- C++ `siteinfo_panel_show` handler + Windows creation fn (clone Cookie/Download) + mac creation fn (**LEFT-anchored** → hand-roll X like omnibox `cef_browser_shell_mac.mm:3787`; React passes a left-X offset).
- Clickable site-info button replacing the passive indicator in `MainBrowserView.tsx`.
- (follow-up) cert plumbing.

**Risk tier:** Medium (new overlay + delineation discipline + cert scoping; the icon-state machine touches existing `securityState`).

---

### (c) Wallet permissions — inside the site-info dropdown

**UX.** A read-only WALLET summary section (status chips from `domain_permissions`: identity-key disclosure, auto-approve/caps, scoped-grant count) + a single **"Manage Wallet Permissions" ›** deep-link row. Do NOT build inline toggles — the deep-link opens the existing full modal.

**Backends to reuse (EXIST):**
- `MENU_ID_MANAGE_PERMISSIONS` = `MENU_ID_USER_FIRST + 22` (`simple_handler.cpp:7352`).
- Menu item + handler — `simple_handler.cpp:7436` adds the item; handler ~7644-7673 extracts domain → `CreateNotificationOverlay(..., "edit_permissions", domain, "")` (reuses shared `notification_browser_` overlay, dispatched in `BRC100AuthOverlayRoot.tsx`).
- Inline summary populated from the existing `domain_permissions` row — no new backend.

**Changes:**
- **Rename** `simple_handler.cpp:7436` `"Manage Site Permissions"` → `"Manage Wallet Permissions"` (verified anchor; research said ~6696 — drift). Also update the comment at ~4750 and the handler comment ~7644.
- **Keep** the right-click entry (quick-revoke path stays). Add the site-info entry as an additional discoverable surface that fires the **same** overlay — no parallel UI.

**Risk tier:** Light (string rename + reuse of one existing overlay; rides on b).

---

### (d) Downloads auto-hide + animation

**UX (Chrome-like lifecycle):** hidden → download starts → icon animates in (fade/slide) → progress ring fills while active → on completion, brief complete pulse/badge (and optional auto-open of overlay, settings-gated) → user clicks to open/clear → once list cleared/empty, icon animates out and hides again.

**This is a REGRESSION fix.** Commit `575e09b` had `{hasDownloads && (<IconButton .../>)}` (Chrome-like). Commit `ec18d29` replaced it with the always-shown button. All needed state already exists in `MainBrowserView.tsx`: `hasDownloads`, `hasActiveDownloads`, `downloadProgress`, `allComplete` (computed ~330-342); the button + `CircularProgress` ring is at `MainBrowserView.tsx:817-852`; `clearCompleted` exists in `useDownloads.ts`.

**Work (small, mostly React):**
- Re-wrap the download button in a conditional/animated mount: render zero-width when `downloads.length === 0`; fade/slide in on first active `download_state_update`; fade out after the list is empty/cleared.
- Keep the existing progress ring; aggregate concurrent downloads into one overall-% ring (already largely there).
- One-shot CSS completion pulse on transition-to-complete.
- New `SettingsManager` key (e.g. `browser.showDownloadsWhenDone`, default ON) gating the auto-open-overlay-on-complete.
- **Ctrl+J** (already mapped) must force-show the overlay even when the icon is hidden.

**Backends:** all EXIST — `useDownloads.ts` (`download_state_update`), `DownloadsOverlayRoot.tsx`, C++ `CefDownloadHandler` + `download_panel_show` handler.

**Risk tier:** Light (re-wrap + animation + one settings key; no backend).

---

### (e) Tab-list caret — `TabListOverlayRoot`

**UX.** A caret (downward chevron) at the **LEFT of the tab strip** (§0 owner override): Windows far-left; macOS inset to start after the 86px traffic-light reservation (runtime-derived from `NSWindow.standardWindowButton`, never hardcoded). Click opens a dropdown copying the `DownloadsOverlayRoot` skeleton:

```
┌─────────────────────────────────────┐
│ 🔍 [ Search tabs…              ]     │  ← native <input>, autofocus (50ms)
├─────────────────────────────────────┤
│ OPEN TABS (n)                        │
│  ⌾ Tab title — example.com    [×]    │  ← active tab highlighted; row=switch; [×]=close
│  ⌾ Another tab — news.com     [×]    │
│  …(scrolls; live-filtered by search) │
├─────────────────────────────────────┤
│ RECENTLY CLOSED                      │
│  ↩ old-tab.com                       │  ← from HistoryManager
└─────────────────────────────────────┘
```
Width ~320-360px, max-height capped + internal scroll on the open-tabs list. Search filters OPEN TABS live (Chrome behavior).

**Backends to reuse (EXIST — no new backend):**
- Open tabs — `TabManager::GetAllTabs()` (`TabManager.h:129`); React already has `tabs[]` via `useTabManager()` (`MainBrowserView.tsx:135-148`) → reuse `tabs` + `switchToTab` + tab-close directly.
- Recently-closed — `HistoryManager::GetHistory/GetHistorySimple` (`HistoryManager.h:49/56`); React access via `window.hodosBrowser.history.*` (sync V8, `useHistory`).

**New work:**
- `frontend/src/pages/TabListOverlayRoot.tsx` + route `/tab-list`.
- C++ `tablist_panel_show` handler + Windows creation fn + mac creation fn (**left/below-caret anchored** → hand-roll X like omnibox).
- The caret button in the tab strip (`MainBrowserView.tsx` / `TabBar`), **left edge**; on mac, positioned after the runtime-derived traffic-light inset (reuse/extend the existing 86px logic at `TabBar.tsx:275`).

**Behaviors / shortcuts:**
- Click caret → toggle. Click row → switch + close overlay. Click `[×]` → close that tab, stay in overlay. Click recently-closed → reopen + close overlay.
- Bind **Ctrl/Cmd+Shift+A** to open it focused on search (Chrome parity). Esc closes.
- Autofocus search input (native `<input>` + `setTimeout(50ms)` per CEF input rule).
- The caret recomputes position on DPI/monitor change (couples with FEAT-DPI — §7).

**Risk tier:** Medium (new overlay + tab-strip-edge placement + mac-anchor hand-roll + DPI coupling).

---

## 6. Implementation plan / sequence

Each chunk is sized for the per-chunk harness (independent commit, smoke-testable). Dependencies noted.

| Order | Chunk | Depends on | Risk | Notes |
|-------|-------|-----------|------|-------|
| **1** | **(d) Downloads auto-hide** | — | Light | Pure React re-wrap + animation + 1 settings key. Fastest win, restores a known-good behavior, zero new backend. Good warm-up that exercises the header without new overlays. |
| **2** | **(a) Bookmarks** | — | Light-Med | New `BookmarksOverlayRoot` + `useBookmarks` hook (clone Downloads pattern) + Ctrl+D toggle branch + menu un-stub. Backend 100% reuse. Establishes the new-dropdown-overlay template that b and e copy. |
| **3** | **(b) Site-info button** | (after a — reuses overlay template) | Med | New `SiteInfoOverlayRoot` (LEFT-anchored — first left-anchored dropdown), icon-state machine, cookies/permissions reuse, delineation discipline. Cert viewer scoped to status-string + error-cert OpenSSL; full viewer deferred. |
| **4** | **(c) Wallet permissions** | (b — lives in its dropdown) | Light | String rename (`:7436`) + read-only summary section + deep-link to existing modal. Folds into b's commit OR a tiny follow-on. |
| **5** | **(e) Tab-list caret** | (after a — reuses overlay template) | Med | New `TabListOverlayRoot`, right-edge caret, Ctrl/Cmd+Shift+A, mac right-edge mandatory, DPI-coupled position recompute. |

**Rationale for order:** d first (no new overlay, restores regression, lowest risk). a second to build the reusable dropdown-overlay template (hook + Windows/mac creation fns + show/hide IPC) that b, c, e all clone — doing a first de-risks the three that follow. b+c together (c is a section of b's dropdown). e last (most placement nuance + DPI coupling).

**Cross-cutting (do once, during chunk 1 or as a pre-chunk):** factor the repeated `getBoundingClientRect()` + `window.innerWidth - rect.right + rect.width/2` offset math (used by every right-cluster button at `:822-823`, `:860-861`, `:892-893`, `:922-923`) into one shared helper, since this pass adds two more call sites and it's the exact math that interacts with FEAT-DPI (§7).

---

## 7. B2-FILL & FEAT-DPI interaction

**Context.** B2-FILL = the React header doesn't fully fill its header window (visible gap). FEAT-DPI = multi-monitor mouse-offset / hit-testing across differently-scaled monitors.

**How this pass touches them:**
- Every new clickable header control (site-info button, bookmark button, tab-list caret, plus the restored download button) is a **new hit-test target** and a **new overlay-anchor source**.
- Overlay anchoring uses `getBoundingClientRect()` + `window.innerWidth` offset math in React, sent as a px offset to C++. This is the **same** coordinate path FEAT-DPI is about — if the header is mis-scaled on a secondary monitor, every one of these offsets is wrong, and the overlays open in the wrong place.
- A visible header gap (B2-FILL) means the rightmost cluster's `rect.right` and the tab-strip-right caret's geometry may not match the true window edge — directly poisoning the offset math for the NEW right-edge caret and right-cluster buttons.

**Recommendation:**
1. **Pull B2-FILL into this pass (as a pre-chunk).** It is a prerequisite for correct anchoring of the new right-cluster buttons (a, d) and the right-edge caret (e). Shipping new header icons on top of an unfilled header risks anchoring bugs that look like icon bugs. Fix the fill first; it's cheap and de-risks everything downstream.
2. **Keep FEAT-DPI as a SEPARATE, parallel investigation** — but treat THIS pass as the forcing function and validation surface. Do NOT block the header pass on a full DPI fix, but: (a) factor the offset math into one helper (chunk-1 cross-cutting task) so a future DPI fix has a single chokepoint; (b) add a multi-monitor / mixed-scale smoke step to each chunk's test plan so the new icons are validated against the DPI problem as they land; (c) on mac, the analog is Retina `backingScaleFactor` mouse-coordinate mapping in the OSR overlay path (`GenericOverlayView cefMouseEventFromEvent`, `isFlipped=YES`) — verify on a mixed Retina/non-Retina setup.

**One-line:** *Fold B2-FILL in as a pre-chunk (it's a correctness prerequisite for the new anchors); keep FEAT-DPI separate but use this pass as its validation surface and centralize the offset math into one helper now.*

---

## 8. macOS deltas (for MACOS_PORT_0_4_0.md)

> Self-contained section — the owner can copy this into `development-docs/0.4.0/MACOS_PORT_0_4_0.md`. **Do not edit that file from this doc.**

**Shared mac infra (read first).** Every dropdown is a borderless `DropdownOverlayWindow`/`GenericOverlayWindow` child of `g_main_window`, OSR-rendered, dismissed by a per-overlay `NSEvent` click-outside monitor + the shared app-focus-loss observer (`OverlayHelpers_mac.mm:189` `InstallAppFocusLossHandler`). 12 `NSWindow*` overlay globals at `cef_browser_shell_mac.mm:256-266`, each with a `Create…Macos`/`Show…`/`Hide…` triplet. `CalculateToolbarOverlayFrame` (`OverlayHelpers_mac.mm:267`) is **right-anchored only** (header height 96). The only left/center-anchored overlay today is the omnibox, which hand-rolls its X (`cef_browser_shell_mac.mm:3787`). Header is 96px (tabs 42 + toolbar 54); window uses `NSWindowStyleMaskFullSizeContentView` so the React header renders under the traffic lights; `TabBar.tsx:275` reserves **86px** left inset on mac (tab height 46 vs 42).

**Checklist of new mac work:**
- [ ] **(a) BookmarksOverlay:** add `g_bookmarks_overlay_window` global + `CreateBookmarksPanelOverlayMacOS`/`Show`/`Hide` triplet (clone `CreateDownloadPanelOverlayMacOS` `:3918`); URL `…/bookmarks`; button sits **between Reload and the URL field** (§0) → **LEFT/below-button anchored**, cannot use `CalculateToolbarOverlayFrame`; hand-roll X like the omnibox (`:3787`), React passes a left-X offset; add to focus-loss close list (`OverlayHelpers_mac.mm:216`, which has a TODO for exactly this).
- [ ] **(b) SiteInfoOverlay:** new overlay-window global + triplet; **LEFT-anchored** → cannot use `CalculateToolbarOverlayFrame`; hand-roll X like the omnibox (`:3787`), React passes a left-X offset; add to focus-loss list. Cert-viewer SSL plumbing is platform-neutral (no extra mac work beyond the Windows scoping decision).
- [ ] **(c) Wallet-perms:** verify the right-click `MENU_ID_MANAGE_PERMISSIONS` → modal path actually shows on mac via `CreateNotificationOverlay` / BRC100 overlay family (`:3454` / `:3384`). The right-click entry + menu ID are cross-platform (`simple_handler.cpp`); the **overlay-show side is the mac risk**. Rename string is cross-platform, no delta.
- [ ] **(d) Downloads:** the mac downloads **panel** already exists (`CreateDownloadPanelOverlayMacOS` `:3918`); auto-hide/animate/complete logic is React (cross-platform, ports for free). Confirm `g_mac_download_panel_icon_right_offset` (`:3905`) is recomputed when the icon appears/disappears so the panel stays anchored to the now-present icon. Add macOS **Dock-bounce / `requestUserAttention`** on download-complete (small mac-only addition, no Windows analog).
- [ ] **(e) Tab-list caret:** caret at the **LEFT of the tab strip, inset to start AFTER the 86px traffic-light reservation** (`TabBar.tsx:275`) — §0 owner override. **The inset must be derived at runtime from `NSWindow.standardWindowButton(...).frame` (the ~70–78px / 86px figures are UNVERIFIED placeholders), never hardcoded.** New overlay-window global + triplet, **left/below-caret anchored** → hand-roll X like omnibox; add to focus-loss list. `TabManager_mac.mm` + cross-platform `HistoryManager` supply data.
- [ ] **Every new overlay** must be added to `InstallAppFocusLossHandler`'s close list (`OverlayHelpers_mac.mm:189-226`) or it won't dismiss on Cmd+Tab.
- [ ] **Coupled (FEAT-DPI mac analog):** new header icons enlarge the surface where OSR overlay click-coordinate mapping (`GenericOverlayView cefMouseEventFromEvent` `:356`, `isFlipped=YES`) must stay correct across scaled displays. Verify overlay click hit-testing on a mixed Retina / non-Retina multi-monitor setup.

**Key mac files:** `cef-native/cef_browser_shell_mac.mm` (overlay globals `:256-266`, window/header `:2477-2534`, download overlay `:3918`/`:3905`, omnibox left/center anchor `:3787`, cookie panel `:2698`, notification overlay `:3454`); `cef-native/OverlayHelpers_mac.mm` (`CalculateToolbarOverlayFrame` `:267` right-only, focus-loss `:189`, click-outside `:76`); `frontend/src/components/TabBar.tsx` (`isMac` `:41`, 86px inset `:275`).

---

## 9. Open questions / decisions needed from the owner

> ✅ **ALL RESOLVED 2026-06-19 — see §0 for the locked decisions.** Q6 (caret placement) was overridden: caret goes LEFT, not right-edge. The original questions are retained below for the rationale/context that fed each decision.

1. **Ctrl+D add feedback (UNVERIFIED easing).** "Added" confirmation = the omnibox star fills solid (Chrome state-change is verified; exact animation easing is an assumption, no public spec). Also: should Ctrl+D **silently toggle** + brief confirmation (research recommendation), or open an edit pop-up like Chrome (name/folder/Done/Remove)? **Decision needed.**
2. **Bookmark grouping in v1.** Confirm flat searchable list + optional **tag chips** (`bookmark_get_all_tags`) is acceptable for v1, with the folder tree deferred (folder CRUD exists in C++ but has **no IPC bridge** — would need new IPC). **Confirm defer.**
3. **Cert-viewer scope (the biggest risk).** v1 = connection-status string + issuer/expiry for **error certs only** (OpenSSL-parse `CefSSLInfo`); a full **good-cert** detail viewer requires a `cef/patch/` Chromium source patch (patch-scale + per-bump maintenance). **Decision: accept the v1 scope and defer/flag the full viewer, or commit to the patch now?**
4. **Site-info vs Privacy Shield delineation.** Confirm the LEFT/RIGHT split (site-info = connection/cert/cookies-view/permissions/wallet-perms; Privacy Shield = ad/tracker/fingerprint/block-stats) and that cookies appear in BOTH only as a **view+deep-link** on the left vs **toggles** on the right (no duplicated cookie UI). Alternative the owner may prefer: fold Privacy Shield INTO the new site-info dropdown (one panel) — research recommends AGAINST (re-creates Brave's complained-about redundancy). **Confirm split, or request the merge.**
5. **Tab-list shortcut.** Adopt Chrome's **Ctrl/Cmd+Shift+A** to open the tab-list overlay (community-verified, treat as adopt-by-convention)? **Confirm.**
6. **Tab-list caret placement.** Confirm **right edge of the tab strip on BOTH platforms** (mandatory on mac due to the 86px traffic-light inset; Chrome-on-mac matches). The "left-edge with traffic-light inset" option from the prompt is rejected — if the owner ever wants left placement on mac, the inset MUST be derived at runtime from `NSWindow.standardWindowButton` frames, never hardcoded (the ~70-78px figure is **UNVERIFIED**). **Confirm right-edge.**
7. **Downloads auto-open-on-complete default.** New `SettingsManager` key `browser.showDownloadsWhenDone` — default **ON** (Chrome parity)? **Confirm default.**
8. **B2-FILL in-scope?** Recommendation (§7): pull B2-FILL in as a **pre-chunk** (it's a correctness prerequisite for anchoring the new right-cluster buttons + right-edge caret). **Confirm pull-in, or keep separate and accept anchoring risk.**
9. **Site-info glyph.** Use a Chrome-style **tune/sliders** glyph (not a padlock), with the lock state shown as a row *inside* the flyout. **Confirm glyph choice.**
10. **`securityState` indicator replacement.** The site-info button replaces the existing passive (`pointerEvents:none`) Lock/LockOpen/Error indicator with a clickable button driven by the same state. Confirm we **replace** (not add alongside) — adding alongside would put two connection indicators in the URL field.

---

### Verified-anchor corrections vs research material (drift caught this session)
- `MainBrowserView.tsx` is at `frontend/src/pages/` — research said `frontend/src/views/` (wrong path).
- Rename anchor confirmed at `simple_handler.cpp:7436` (`"Manage Site Permissions"`) — research/CLAUDE.md said ~6696.
- Download button uses IPC `download_panel_show` (confirmed `MainBrowserView.tsx:824`).
- Menu bookmarks stub confirmed at `simple_handler.cpp:2534` (research/CLAUDE.md said ~2542).
