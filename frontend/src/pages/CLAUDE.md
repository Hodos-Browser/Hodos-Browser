# Pages
> Top-level React page components and overlay roots for the Hodos Browser UI.

## Overview

This directory contains all routable page components. Pages fall into two categories: **main views** rendered in the primary browser window (`MainBrowserView`, `NewTabPage`, `HistoryPage`, `SettingsPage`, `SendPage`, `CertErrorPage`) and **overlay roots** rendered in isolated CEF subprocess windows (`*OverlayRoot.tsx`). Overlays communicate with the C++ shell exclusively through `window.cefMessage.send()` IPC and `window.addEventListener('message', ...)` callbacks.

See root [CLAUDE.md](/CLAUDE.md) for overlay architecture rules, close prevention patterns, and CEF input requirements.

---

## Files

| File | Lines | Purpose |
|------|-------|---------|
| `MainBrowserView.tsx` | 987 | Main browser chrome: tab bar, address bar, nav buttons, toolbar icons. Orchestrates overlay creation via IPC triggers. |
| `WalletPanelPage.tsx` | 1677 | Wallet setup overlay: create, recover (12-word mnemonic), backup import, Centbee legacy recovery, PIN creation/unlock. |
| `BRC100AuthOverlayRoot.tsx` | 970 | BRC-100 auth notification modals: domain approval, payment confirmation, rate limit, certificate disclosure, no-wallet prompt. |
| `SettingsOverlayRoot.tsx` | 551 | Settings overlay with tabs: browser, privacy, wallet auto-approval limits, profile import. |
| `NewTabPage.tsx` | 382 | New tab page: search bar, quick-access tile grid with cached favicons. |
| `BackupOverlayRoot.tsx` | 371 | Backup modal: displays mnemonic, requires checkbox confirmation before close. |
| `ProfilePickerOverlayRoot.tsx` | 354 | Profile picker dropdown: list profiles, switch, create new (with avatar file upload). |
| `OmniboxOverlayRoot.tsx` | 308 | Address bar autocomplete dropdown: history matches + search suggestions. |
| `DownloadsOverlayRoot.tsx` | 219 | Downloads panel: active/completed downloads with progress bars, pause/resume/cancel. |
| `MenuOverlayRoot.tsx` | 218 | Three-dot menu dropdown: new tab, find, print, zoom controls, bookmarks, history, devtools, settings, exit. |
| `CertErrorPage.tsx` | 214 | SSL certificate error interstitial: warning display, "go back" / "proceed (unsafe)" actions. |
| `SendPage.tsx` | 145 | Legacy transaction send page (balance, send form, transaction history). |
| `SettingsPage.tsx` | 131 | Full-page settings with sidebar navigation (general, privacy, downloads, wallet, about). |
| `WalletOverlayRoot.tsx` | 115 | Wallet dashboard overlay: lazy-loaded tabs (dashboard, activity, certificates, approved sites, settings). |
| `PrivacyShieldOverlayRoot.tsx` | 101 | Privacy shield panel: per-domain ad/cookie block stats. Domain set via C++ JS injection. |
| `CookiePanelOverlayRoot.tsx` | 82 | Cookie management panel overlay (delegates to `CookiePanelOverlay` component). |
| `HistoryPage.tsx` | 78 | Browser data page with tabs: history, cookies, cache panels. |

---

## Overlay vs Main View

### Main Views (rendered in primary browser window)

- **`MainBrowserView`** — The browser chrome itself. Contains tab bar, address bar, nav buttons, and toolbar icon buttons that *trigger* overlays. Never add panels or dropdowns directly here (see root CLAUDE.md).
- **`NewTabPage`** — Rendered in the content area when opening a new tab.
- **`HistoryPage`** — Full-page browser data view (history, cookies, cache tabs).
- **`SettingsPage`** — Full-page settings (routed from menu, not an overlay).
- **`CertErrorPage`** — Shown when a site has SSL errors. Reads `domain`, `error`, `url`, `code` from URL query params.
- **`SendPage`** — Legacy transaction page (uses `useBalance()`, `useTransaction()` hooks).

### Overlay Roots (isolated CEF subprocess windows)

Every `*OverlayRoot.tsx` file is a standalone React app root for an overlay subprocess. Overlays are created by C++ when triggered by toolbar icon clicks or IPC messages.

| Overlay | Style | Close Mechanism |
|---------|-------|-----------------|
| `WalletPanelPage` | Side panel (full height) | Click-outside (React `handleBackgroundClick`), guarded by `preventClose` |
| `WalletOverlayRoot` | Side panel | Click-outside (React) |
| `BRC100AuthOverlayRoot` | Centered modal | C++ manages lifecycle; `overlay_close` IPC |
| `BackupOverlayRoot` | Centered modal | Checkbox confirmation + `overlay_close` IPC |
| `SettingsOverlayRoot` | Full page | `settings_close` IPC |
| `MenuOverlayRoot` | Dropdown | C++ mouse hook (click-outside), Escape key sends `menu_hide` |
| `OmniboxOverlayRoot` | Dropdown below address bar | C++ mouse hook, auto-hide after navigation |
| `DownloadsOverlayRoot` | Dropdown panel | `download_panel_hide` IPC |
| `CookiePanelOverlayRoot` | Right-side panel | `cookie_panel_hide` IPC |
| `PrivacyShieldOverlayRoot` | Right-side panel | `cookie_panel_hide` IPC |
| `ProfilePickerOverlayRoot` | Dropdown | `profile_panel_hide` IPC |

---

## IPC Message Reference

### MainBrowserView → C++ (overlay triggers)

| IPC Message | Args | Purpose |
|-------------|------|---------|
| `omnibox_create` | — | Pre-create omnibox overlay on address bar focus |
| `omnibox_show` / `omnibox_hide` | — | Show/hide omnibox dropdown |
| `omnibox_update_query` | `[query]` | Send typed text to omnibox |
| `omnibox_select` | `[direction]` | Arrow key nav (up/down) |
| `cookie_panel_show` | `[rightOffset, domain]` | Show privacy shield overlay |
| `download_panel_show` | `[rightOffset]` | Show downloads overlay |
| `toggle_wallet_panel` | `[rightOffset, unreadCount]` | Toggle wallet panel |
| `profile_panel_show` | `[rightOffset]` | Show profile picker |
| `menu_show` | `[rightOffset]` | Show three-dot menu |
| `settings_get_all` | — | Fetch browser settings |

### Overlay → C++ (actions)

| IPC Message | Sent From | Purpose |
|-------------|-----------|---------|
| `overlay_close` | Wallet, Backup, BRC100Auth | Close the overlay HWND |
| `wallet_prevent_close` / `wallet_allow_close` | WalletPanelPage | Toggle close prevention flag |
| `settings_close` | SettingsOverlayRoot | Close settings overlay |
| `menu_hide` | MenuOverlayRoot | Hide menu on Escape |
| `menu_action` | MenuOverlayRoot, SettingsPage | Trigger action (new_tab, print, find, zoom, devtools, settings, exit, etc.) |
| `download_panel_hide` | DownloadsOverlayRoot | Hide downloads panel |
| `cookie_panel_hide` | CookiePanelOverlay, PrivacyShield | Hide cookie/privacy panel |
| `profile_panel_hide` | ProfilePickerOverlayRoot | Hide profile picker |
| `add_domain_permission` | BRC100AuthOverlayRoot | Whitelist domain (simple) |
| `add_domain_permission_advanced` | BRC100AuthOverlayRoot | Whitelist with spending limits |
| `approve_cert_fields` | BRC100AuthOverlayRoot | Remember cert field selections |
| `brc100_auth_response` | BRC100AuthOverlayRoot | Approve/deny auth request |
| `cert_error_go_back` / `cert_error_proceed` | CertErrorPage | Handle SSL error decision |
| `navigate` | OmniboxOverlayRoot | Navigate to selected URL |
| `omnibox_autocomplete` | OmniboxOverlayRoot | Send selected suggestion back |

### C++ → React (injected callbacks)

| Callback | Overlay | Purpose |
|----------|---------|---------|
| `window.showNotification(queryString)` | BRC100AuthOverlayRoot | Show/update auth notification with params |
| `window.hideNotification()` | BRC100AuthOverlayRoot | Hide notification |
| `window.setMenuZoomLevel(level)` | MenuOverlayRoot | Set current zoom % for display |
| `window.setShieldDomain(domain)` | PrivacyShieldOverlayRoot | Set domain for privacy stats |

### C++ → React (window message events)

| Event / Detail | Received By | Purpose |
|----------------|-------------|---------|
| `find_show` | MainBrowserView | Show find bar (Ctrl+F from C++) |
| `find_result` | MainBrowserView | Find match count + ordinal |
| `omniboxQueryUpdate` (CustomEvent) | OmniboxOverlayRoot | New query from address bar |
| `omniboxSelect` (CustomEvent) | OmniboxOverlayRoot | Arrow key selection |
| `most_visited_response` | NewTabPage | Most-visited sites data |
| `allSystemsReady` | BackupOverlayRoot | System initialization complete |
| `download_state_update` | (via useDownloads) | Download progress updates |

---

## Key Hooks Used

| Hook | Used By | Purpose |
|------|---------|---------|
| `useHodosBrowser()` | MainBrowserView, BackupOverlayRoot | Navigation, wallet API, reload |
| `useTabManager()` | MainBrowserView | Tab CRUD, switching, reordering, drag tear-off |
| `useKeyboardShortcuts()` | MainBrowserView | Ctrl+T/W/F, tab switching shortcuts |
| `useDownloads()` | MainBrowserView, DownloadsOverlayRoot | Download list, progress, actions |
| `useCookieBlocking()` | MainBrowserView | Cookie block count polling |
| `useAdblock()` | MainBrowserView | Ad block count + site toggle state |
| `useProfiles()` | MainBrowserView, ProfilePickerOverlayRoot | Profile list, switch, create |
| `useBackgroundBalancePoller()` | MainBrowserView | Keep wallet balance cache warm |
| `useSettings()` | SettingsOverlayRoot | Load/update browser settings |
| `useImport()` | SettingsOverlayRoot | Browser profile import (Chrome, Brave, Edge) |
| `useOmniboxSuggestions()` | OmniboxOverlayRoot | Fetch history + search suggestions |
| `useBalance()` | SendPage | Balance + USD value |
| `useTransaction()` | SendPage | Transaction list + send |
| `usePrivacyShield()` | (via PrivacyShieldPanel) | Composed adblock + cookie state |

---

## Close Prevention Patterns

### WalletPanelPage
```tsx
// preventClose is true during mnemonic display or PIN entry
const preventClose = mnemonic !== null || (pinStep !== null && pendingAction !== null);

useEffect(() => {
  window.cefMessage?.send(preventClose ? 'wallet_prevent_close' : 'wallet_allow_close', []);
}, [preventClose]);
```
The C++ side also sets `g_wallet_overlay_prevent_close = true` at overlay creation time (synchronous, no race). React sends `wallet_allow_close` once the user reaches a safe state.

### BackupOverlayRoot
Close is gated by a "I have backed up my recovery phrase" checkbox. The "Done" button only enables after checking the box and calling `markBackedUp()`.

### BRC100AuthOverlayRoot
No React-side close prevention. C++ manages the overlay lifecycle — it stays visible until the user clicks Allow/Deny.

---

## CEF Input Patterns (enforced here)

These pages follow the CEF input rules from root CLAUDE.md:

- **Native `<input>` elements** — `ProfilePickerOverlayRoot` uses `<input type="text">` for profile name, `<input type="file">` (visible) for avatar upload.
- **WalletPanelPage** — Uses native inputs for 12-word recovery grid, PIN input (`PinInput` component with auto-advance via refs), and backup password field.
- **No MUI TextField** in any overlay — all text inputs are native HTML with inline styles.

---

## BRC100AuthOverlayRoot Notification Types

The auth overlay handles 5 notification variants, selected by `type` query param:

| Type | UI | User Action |
|------|-----|-------------|
| `domain_approval` | Domain card + advanced settings (spending limits) | Allow / Deny |
| `payment_confirmation` | Gold amount box (satoshis + USD) | Approve / Deny |
| `rate_limit_exceeded` | Limit explanation + current limits display | Update Limits / Deny |
| `certificate_disclosure` | Field checkboxes + "remember for site" | Share / Deny |
| `no_wallet` | Setup prompt | Setup Wallet / Deny |

Advanced settings on domain approval include:
- Per-transaction limit (cents)
- Per-session limit (cents)
- Rate limit (requests/min)

---

## Adding a New Overlay

Follow the pattern from root CLAUDE.md:

1. Create `<Name>OverlayRoot.tsx` in this directory
2. Add route in `frontend/src/App.tsx`
3. Add C++ handler in `simple_handler.cpp` (IPC show/hide)
4. Add C++ creation function in `cef_browser_shell.cpp` (Windows) and `cef_browser_shell_mac.mm` (macOS)
5. Trigger from `MainBrowserView.tsx` toolbar icon via `window.cefMessage.send('<name>_show', [offset])`

Use native `<input>` elements (not MUI), visible file inputs, and follow the close prevention patterns above.

---

## Related

- [Root CLAUDE.md](/CLAUDE.md) — Overlay architecture rules, CEF input patterns, close prevention details
- `frontend/src/components/wallet/CLAUDE.md` — Wallet tab components (dashboard, activity, certificates)
- `frontend/src/components/panels/CLAUDE.md` — Panel components (cookie, privacy, history)
- `frontend/src/components/settings/CLAUDE.md` — Settings sub-components
- `frontend/src/hooks/` — All custom hooks referenced above
- `frontend/src/bridge/initWindowBridge.ts` — `window.hodosBrowser` API definitions
