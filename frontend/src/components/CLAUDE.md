# frontend/src/components
> React components for the Hodos Browser UI: browser chrome, wallet, privacy, and settings panels.

## Overview

This directory contains all React components for the browser frontend. Components fall into three categories:

1. **Browser chrome** — tab bar, find bar, menus (rendered in `header_hwnd`)
2. **Overlay panels** — wallet, cookies, privacy shield, downloads (rendered in separate CEF subprocesses)
3. **Settings/management** — history, cache, domain permissions (rendered in settings page or overlays)

All wallet operations go through hooks that call the Rust backend at `127.0.0.1:31301`. Browser operations use `window.cefMessage.send()` IPC to the C++ shell. Private keys never touch JavaScript — see root CLAUDE.md.

## Subdirectories

| Directory | Purpose | CLAUDE.md |
|-----------|---------|-----------|
| `panels/` | Legacy wallet panel layout + backup modal (dead code — superseded by overlay architecture) | [panels/CLAUDE.md](panels/CLAUDE.md) |
| `settings/` | Settings page section components (General, Privacy, Downloads, Wallet, About) | [settings/CLAUDE.md](settings/CLAUDE.md) |
| `wallet/` | Full wallet page tabs (Dashboard, Activity, Certificates, Approved Sites, Settings) | [wallet/CLAUDE.md](wallet/CLAUDE.md) |

## Components

| Component | Purpose | Used In |
|-----------|---------|---------|
| `TabBar` | Browser tab strip with drag-reorder and tear-off | `MainBrowserView` |
| `TabComponent` | Individual tab (favicon, title, close) | `TabBar` |
| `FindBar` | Ctrl+F find-in-page bar | `MainBrowserView` |
| `MenuOverlay` | Three-dot dropdown menu (dark theme) | `MenuOverlayRoot` overlay |
| `SettingsMenu` | Legacy dropdown (History + Settings) — **unused, dead code** | None |
| `WalletPanel` | Main wallet overlay: balance, send, receive, identity, sync status | `WalletPanelPage` overlay |
| `BalanceDisplay` | BSV/USD balance display (stateless) | `WalletPanel` |
| `TransactionForm` | Send BSV form with paymail/PeerPay support | `WalletPanel` |
| `TransactionHistory` | Transaction list with detail modal | `WalletPanel`, wallet page |
| `AddressManager` | Generate and display BSV addresses (dev/debug) | Wallet page |
| `BRC100AuthModal` | MUI dialog for BRC-100 auth approval | Auth overlay |
| `DomainPermissionForm` | Auto-approve limit configuration form | `DomainPermissionsTab` |
| `DomainPermissionsTab` | Table of approved BRC-100 domains | Settings/wallet page |
| `HistoryPanel` | Browsing history with search/filter/pagination | Settings page |
| `CookiesPanel` | Full cookie manager (accordion per domain) | Settings page |
| `CookiePanelOverlay` | Compact cookie panel (450px dropdown) | Cookie overlay |
| `CachePanel` | Cache/cookie overview with clear actions | Settings page |
| `PrivacyShieldPanel` | Per-site privacy toggles (adblock, cookies, scriptlets) | Privacy shield overlay |
| `SimplePanel` | Debug/test panel for overlay z-index verification — **unused, dead code** | None |

## Component Details

### TabBar
- **Props:** `tabs`, `activeTabId`, `isLoading`, `onCreateTab`, `onCloseTab`, `onSwitchTab`, `onReorderTabs?`, `onTearOff?`
- **Drag behavior:** 5px horizontal threshold to start drag, 40px vertical for tear-off. Uses `setPointerCapture()` to track pointer across HWND boundaries (CEF requirement).
- **IPC:** `tab_ghost_show` / `tab_ghost_hide` for native ghost window during tear-off
- **Styling:** MUI Box, gold drop indicators (`#a67c00`), custom webkit scrollbar

### TabComponent
- **Props:** `tab`, `isActive`, `showDivider`, `onClose`, `onClick`, `tabRef?`, `isDragged?`, `dropIndicator?`, `onPointerDown?`
- Fully controlled/stateless. Shows favicon (or PublicIcon fallback), title with ellipsis, close button fades in on hover/active.
- **Loading timeout:** 8s spinner timeout for sites with persistent loading states (e.g. investing.com)

### FindBar
- **Props:** `onClose`, `findResult: { count, activeMatch } | null`
- **IPC:** Sends `find_text [text, forward, findNext]` and `find_stop`. Red background when count=0.
- **Keyboard:** Enter=next, Shift+Enter=prev, Escape=close
- Uses native `<input>` for CEF compatibility

### MenuOverlay
- **Props:** `onClose`, `onAction(action: string)`, `currentZoom?`
- **Sections:** New Tab | History/Downloads | Zoom (+/-/reset/fullscreen) | Print/Find | DevTools | Settings/About/Exit
- **Actions:** `new_tab`, `history`, `downloads`, `zoom_in`, `zoom_out`, `zoom_reset`, `fullscreen`, `print`, `find`, `devtools`, `settings`, `about`, `exit`
- Dark theme (#1e1e1e), ClickAwayListener for dismiss

### WalletPanel
- **Props:** `onClose?`
- **State:** Manages send/receive views, identity key display, sync status (polled every 3s from `/wallet/sync-status`), PeerPay notifications (from URL params `ppc`/`ppa`)
- **IPC sent:** `wallet_payment_dismissed`, `tab_create` (for Advanced/Manage Sites links)
- **Hooks:** `useBalance()`, `useAddress()`
- **Key sections:**
  1. Balance display (Hodos logo, USD + BSV amounts, refresh)
  2. Sync status banner (addresses scanned, UTXOs found, auto-dismiss)
  3. PeerPay notification (auto-accepted payment count + total, Details/Dismiss)
  4. Identity key section (copy/show toggle)
  5. Action buttons (Receive with QR, Send with TransactionForm)
  6. Advanced/Manage Sites links (open full wallet page via `tab_create`)
- **Note:** Close prevention (`wallet_prevent_close`/`wallet_allow_close` IPC) is handled by `WalletPanelPage` (the overlay root), not this component

### TransactionForm
- **Props:** `onTransactionCreated(result)`, `balance`, `bsvPrice`, `isLoading?`, `error?`
- **Dual amount:** USD input auto-calculates BSV and vice versa
- **Recipient types:** BSV address (`^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$`), identity key (`^(02|03)[0-9a-fA-F]{64}$`), paymail (`user@domain` or `$handle`)
- **Send paths:** Paymail (bsalias) | PeerPay (BRC-29 via MessageBox) | Standard (P2PKH)
- **Paymail resolution:** 500ms debounce to `/wallet/paymail/resolve`, displays avatar + P2P capability
- Fee rate hardcoded to 5 sat/byte

### BalanceDisplay
- **Props:** Extends `BalanceData { balance, usdValue, isLoading, isRefreshing }` + `onRefresh?`
- Stateless. Formats satoshis -> BSV (/ 100,000,000 to 8 decimals), USD to 2 decimals.

### BRC100AuthModal
- **Props:** `open`, `onClose`, `onApprove(whitelist: boolean)`, `onReject`, `request: BRC100AuthRequest`
- MUI Dialog showing domain, requested permissions (as Chips), whitelist checkbox

### DomainPermissionForm
- **Props:** `domain`, `currentSettings?`, `onSave(settings)`, `onCancel`
- **Exports:** `DomainPermissionSettings` interface (perTxLimitCents, perSessionLimitCents, rateLimitPerMin)
- Configures per-TX limit, per-session limit, rate limit (requests/min)
- "Always notify" toggle zeros out all limits
- Warning banner if limits exceed $5/tx or $50/session
- Gold theme (#a67c00), native inputs

### DomainPermissionsTab
- Fetches from `http://127.0.0.1:31301/domain/permissions/all`
- MUI Table with edit (opens DomainPermissionForm in Dialog) and revoke (DELETE with confirmation)

### HistoryPanel
- **Hooks:** `useHistory()` for fetch/search/delete/clear
- Time range filter (hour/day/week/all), search, pagination (20/page)
- Click entry -> `window.hodosBrowser.navigation.navigate(url)`
- Helpers: `chromiumTimeToDate()`, `dateToChromiumTime()` for timestamp conversion

### CookiesPanel
- **Hooks:** `useCookies()`, `useCookieBlocking()`
- Full view: accordion per domain, per-cookie detail (name, value, domain, path, expires, httpOnly, secure, sameSite)
- Sort options: default (alphabetical), blocked first, most cookies, largest
- Block exact or wildcard via context menu
- Pagination (20/page), blocking log section, managed domains section

### CookiePanelOverlay
- Compact 450px version of CookiesPanel for dropdown-style overlay
- Three tabs: Cookies by domain, Blocked list, Block log
- Same hooks as CookiesPanel but simpler layout (no accordion expansion of individual cookies)

### CachePanel
- **Hooks:** `useCookies()` for cache/cookie stats
- Stat cards (cache size + cookie count), confirmation dialogs for destructive actions, toast notifications

### PrivacyShieldPanel
- **Props:** `domain`, `showCount?`
- **Hooks:** `usePrivacyShield(domain)`, `useSettings()`
- Toggles: master protection, ad blocking, scriptlet injection, cookie blocking
- Fingerprint shield shown as always-on (no toggle)
- Respects global settings: if adblock disabled globally, per-site toggle shows "disabled in settings"
- **IPC:** `menu_action['settings_privacy']` to open Privacy Settings page

## Hook Dependencies

| Hook | Components Using It | Backend |
|------|-------------------|---------|
| `useBalance` | WalletPanel | `127.0.0.1:31301/wallet/balance` |
| `useAddress` | WalletPanel | `127.0.0.1:31301/wallet/address` |
| `useTransaction` | TransactionForm | `127.0.0.1:31301/wallet/send` |
| `useHodosBrowser` | AddressManager | `window.hodosBrowser.*` |
| `useCookies` | CookiesPanel, CookiePanelOverlay, CachePanel | `127.0.0.1:31301` cookie endpoints |
| `useCookieBlocking` | CookiesPanel, CookiePanelOverlay | `127.0.0.1:31301` cookie blocking endpoints |
| `useHistory` | HistoryPanel | `127.0.0.1:31301` history endpoints |
| `usePrivacyShield` | PrivacyShieldPanel | `127.0.0.1:31302` adblock + `31301` cookies |
| `useSettings` | PrivacyShieldPanel | `127.0.0.1:31301/settings/*` |

## Patterns

### CEF Input Rules
Per root CLAUDE.md, overlays must use **native `<input>` elements**, not MUI `TextField`. MUI's extra DOM layers break CEF focus handling. Components like `TransactionForm`, `DomainPermissionForm`, and `FindBar` follow this pattern.

### IPC Communication
- **Overlay -> C++:** `window.cefMessage.send(messageName, args)` for tab creation, overlay control, find operations
- **C++ -> Overlay:** URL parameters (PeerPay notification counts) or IPC message listeners
- **React -> Wallet backend:** Direct HTTP fetch to `127.0.0.1:31301` via hooks

### Styling Approaches
- **MUI components:** CachePanel, CookiesPanel, DomainPermissionsTab, HistoryPanel, PrivacyShieldPanel, BRC100AuthModal, TabBar, TabComponent
- **Inline CSS objects:** MenuOverlay, DomainPermissionForm
- **CSS classes:** TransactionForm, TransactionHistory, WalletPanel (class-based stylesheets: `TransactionComponents.css`, `WalletPanel.css`)
- **Hybrid:** Some components mix MUI layout with inline overrides

### Portal Rendering
Components that need to escape CEF z-index constraints render via `ReactDOM.createPortal(content, document.body)` — see `SettingsMenu` and `SimplePanel` (both legacy/unused).

### Validation Patterns
BSV address, identity key, and paymail each have specific regex patterns. `TransactionForm` validates all three and routes to the appropriate send path. Minimum output is 546 satoshis (dust limit).

## Dead Code

The following components exist but are not imported anywhere:
- **`SettingsMenu.tsx`** — Legacy dropdown, superseded by `MenuOverlay`
- **`SimplePanel.tsx`** — Debug/test panel for z-index verification
- **`panels/BackupModal.tsx`** — Superseded by `BackupOverlayRoot.tsx` overlay
- **`panels/WalletPanelContent.tsx`** — Superseded by wallet tab components
- **`panels/WalletPanelLayout.tsx`** — Superseded by overlay architecture

These can be safely deleted if cleanup is desired.

## Related

- [Root CLAUDE.md](../../../CLAUDE.md) — Architecture, overlay lifecycle, CEF input rules, close prevention
- [panels/CLAUDE.md](panels/CLAUDE.md) — Legacy wallet panel layout and backup modal
- [settings/CLAUDE.md](settings/CLAUDE.md) — Settings page section components
- [wallet/CLAUDE.md](wallet/CLAUDE.md) — Full wallet page tab components
- [hooks/](../hooks/) — All custom hooks used by these components
- [pages/](../pages/) — Overlay root pages that host these components
