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
| `panels/` | Wallet panel layout + backup modal | [panels/CLAUDE.md](panels/CLAUDE.md) |
| `settings/` | Settings page section components (General, Privacy, Downloads, Wallet, About) | [settings/CLAUDE.md](settings/CLAUDE.md) |
| `wallet/` | Full wallet page tabs (Dashboard, Activity, Certificates, Approved Sites, Settings) | [wallet/CLAUDE.md](wallet/CLAUDE.md) |

## Components

| Component | Purpose | Used In |
|-----------|---------|---------|
| `TabBar` | Browser tab strip with drag-reorder and tear-off | `MainBrowserView` |
| `TabComponent` | Individual tab (favicon, title, close) | `TabBar` |
| `FindBar` | Ctrl+F find-in-page bar | `MainBrowserView` |
| `MenuOverlay` | Three-dot dropdown menu (dark theme) | `MainBrowserView` |
| `SettingsMenu` | Legacy dropdown (History + Settings) | `MainBrowserView` |
| `WalletPanel` | Main wallet overlay: balance, send, receive, identity | `WalletPanelPage` overlay |
| `BalanceDisplay` | BSV/USD balance display (stateless) | `WalletPanel` |
| `TransactionForm` | Send BSV form with paymail/PeerPay support | `WalletPanel` |
| `TransactionHistory` | Transaction list with detail modal | `WalletPanel`, wallet page |
| `AddressManager` | Generate and display BSV addresses | Wallet page |
| `BRC100AuthModal` | MUI dialog for BRC-100 auth approval | Auth overlay |
| `DomainPermissionForm` | Auto-approve limit configuration form | `DomainPermissionsTab` |
| `DomainPermissionsTab` | Table of approved BRC-100 domains | Settings/wallet page |
| `HistoryPanel` | Browsing history with search/filter/pagination | Settings page |
| `CookiesPanel` | Full cookie manager (accordion per domain) | Settings page |
| `CookiePanelOverlay` | Compact cookie panel (450px dropdown) | Cookie overlay |
| `CachePanel` | Cache/cookie overview with clear actions | Settings page |
| `PrivacyShieldPanel` | Per-site privacy toggles (adblock, cookies, scriptlets) | Privacy shield overlay |
| `SimplePanel` | Debug/test panel for overlay z-index verification | Development only |

## Component Details

### TabBar
- **Props:** `tabs`, `activeTabId`, `isLoading`, `onCreateTab`, `onCloseTab`, `onSwitchTab`, `onReorderTabs?`, `onTearOff?`
- **Drag behavior:** 5px horizontal threshold to start drag, 40px vertical for tear-off. Uses `setPointerCapture()` to track pointer across HWND boundaries (CEF requirement).
- **IPC:** `tab_ghost_show` / `tab_ghost_hide` for native ghost window during tear-off
- **Styling:** MUI Box, gold drop indicators, custom webkit scrollbar

### TabComponent
- **Props:** `tab`, `isActive`, `showDivider`, `onClose`, `onClick`, `tabRef?`, `isDragged?`, `dropIndicator?`, `onPointerDown?`
- Fully controlled/stateless. Shows favicon (or PublicIcon fallback), title with ellipsis, close button fades in on hover/active.

### FindBar
- **Props:** `onClose`, `findResult: { count, activeMatch } | null`
- **IPC:** Sends `find_text [text, forward, findNext]` and `find_stop`. Red background when count=0.
- **Keyboard:** Enter=next, Shift+Enter=prev, Escape=close

### MenuOverlay
- **Props:** `onClose`, `onAction(action: string)`, `currentZoom?`
- **Sections:** New Tab | History/Downloads | Zoom (+/−/reset/fullscreen) | Print/Find | DevTools | Settings/About/Exit
- Dark theme (#1e1e1e), ClickAwayListener for dismiss

### WalletPanel
- **Props:** `onClose?`
- **State:** Manages send/receive views, identity key display, sync status (polled every 3s from `/wallet/sync-status`), PeerPay notifications (from URL params `ppc`/`ppa`)
- **IPC sent:** `wallet_payment_dismissed`, `tab_create`, `wallet_prevent_close`, `wallet_allow_close`
- **Hooks:** `useBalance()`, `useAddress()`
- **Close prevention:** Sends `wallet_prevent_close` / `wallet_allow_close` IPC to guard overlay during sensitive operations (see root CLAUDE.md overlay lifecycle section)

### TransactionForm
- **Props:** `onTransactionCreated(result)`, `balance`, `bsvPrice`, `isLoading?`, `error?`
- **Dual amount:** USD input auto-calculates BSV and vice versa
- **Recipient types:** BSV address (`^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$`), identity key (`^(02|03)[0-9a-fA-F]{64}$`), paymail (`user@domain` or `$handle`)
- **Send paths:** Paymail (bsalias) | PeerPay (BRC-29 via MessageBox) | Standard (P2PKH)
- **Paymail resolution:** 500ms debounce to `/wallet/paymail/resolve`, displays avatar + P2P capability
- Fee rate hardcoded to 5 sat/byte

### BalanceDisplay
- **Props:** Extends `BalanceData { balance, usdValue, isLoading, isRefreshing }` + `onRefresh?`
- Stateless. Formats satoshis → BSV (÷ 100,000,000), USD to 2 decimals.

### BRC100AuthModal
- **Props:** `open`, `onClose`, `onApprove(whitelist: boolean)`, `onReject`, `request: BRC100AuthRequest`
- MUI Dialog showing domain, requested permissions (as Chips), whitelist checkbox

### DomainPermissionForm
- **Props:** `domain`, `currentSettings?`, `onSave(settings)`, `onCancel`
- Configures per-TX limit, per-session limit, rate limit (requests/min)
- "Always notify" toggle zeros out all limits
- Warning banner if limits exceed $5/tx or $50/session
- Gold theme (#a67c00), native inputs

### DomainPermissionsTab
- Fetches from `http://127.0.0.1:31301/domain/permissions/*`
- MUI Table with edit (opens DomainPermissionForm in Dialog) and revoke (DELETE with confirmation)

### HistoryPanel
- **Hooks:** `useHistory()` for fetch/search/delete/clear
- Time range filter (hour/day/week/all), search, pagination (20/page)
- Click entry → `window.hodosBrowser.navigation.navigate(url)`

### CookiesPanel
- **Hooks:** `useCookies()`, `useCookieBlocking()`
- Full view: accordion per domain, per-cookie detail (name, value, domain, path, expires, httpOnly, secure, sameSite)
- Sort options: default, blocked, count, size
- Block exact or wildcard via context menu
- Pagination (20/page), blocking log section

### CookiePanelOverlay
- Compact 450px version of CookiesPanel
- Three tabs: Cookies by domain, Blocked list, Block log
- Same hooks as CookiesPanel

### CachePanel
- **Hooks:** `useCookies()` for cache/cookie stats
- Stat cards (cache size + cookie count), confirmation dialogs for destructive actions

### PrivacyShieldPanel
- **Props:** `domain`, `showCount?`
- **Hooks:** `usePrivacyShield(domain)`, `useSettings()`
- Toggles: master protection, ad blocking, scriptlet injection, cookie blocking
- Fingerprint shield shown as always-on (no toggle)
- Respects global settings: if adblock disabled globally, per-site toggle is ineffective

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
Per root CLAUDE.md, overlays must use **native `<input>` elements**, not MUI `TextField`. MUI's extra DOM layers break CEF focus handling. Components like `TransactionForm` and `DomainPermissionForm` follow this pattern. `FindBar` also uses a native input.

### IPC Communication
- **Overlay → C++:** `window.cefMessage.send(messageName, args)` for tab creation, overlay control, find operations
- **C++ → Overlay:** URL parameters (PeerPay notification counts) or IPC message listeners
- **React → Wallet backend:** Direct HTTP fetch to `127.0.0.1:31301` via hooks

### Styling Approaches
- **MUI components:** CachePanel, CookiesPanel, DomainPermissionsTab, HistoryPanel, PrivacyShieldPanel, BRC100AuthModal
- **Inline CSS objects:** TabBar, TabComponent, MenuOverlay, DomainPermissionForm, SimplePanel
- **CSS classes:** TransactionForm, TransactionHistory, WalletPanel (class-based stylesheets)
- **Hybrid:** Some components mix MUI layout with inline overrides

### Portal Rendering
Components that need to escape CEF z-index constraints render via `ReactDOM.createPortal(content, document.body)` — see `SettingsMenu` and `SimplePanel`.

### Validation Patterns
BSV address, identity key, and paymail each have specific regex patterns. `TransactionForm` validates all three and routes to the appropriate send path. Minimum output is 546 satoshis (dust limit).

## Related

- [Root CLAUDE.md](../../../CLAUDE.md) — Architecture, overlay lifecycle, CEF input rules, close prevention
- [panels/CLAUDE.md](panels/CLAUDE.md) — Wallet panel layout and backup modal
- [settings/CLAUDE.md](settings/CLAUDE.md) — Settings page section components
- [wallet/CLAUDE.md](wallet/CLAUDE.md) — Full wallet page tab components
- [hooks/](../hooks/) — All custom hooks used by these components
- [pages/](../pages/) — Overlay root pages that host these components
