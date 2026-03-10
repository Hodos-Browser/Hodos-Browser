# Frontend Source Root
> React/TypeScript application root: routing, bridge initialization, and directory organization for the Hodos Browser UI layer.

## Overview

This is the source root for the Hodos Browser frontend — a React SPA that runs inside CEF (Chromium Embedded Framework) to provide browser chrome, wallet UI, and overlay panels. The application never handles private keys or signing directly; all sensitive operations are delegated to the Rust wallet backend via `window.hodosBrowser.*` and `window.cefMessage.send()` APIs injected by C++ V8 bindings.

The frontend serves two distinct roles simultaneously:
1. **Main browser chrome** — tab bar, address bar, navigation controls, toolbar icons (route `/`)
2. **Overlay subprocesses** — each overlay (wallet, settings, auth, downloads, etc.) is a separate CEF process loading the same React app at a different route

## Entry Points

| File | Lines | Purpose |
|------|-------|---------|
| `main.tsx` | 12 | React entry point; mounts `<BrowserRouter>` + `<App />`; imports `bridge/initWindowBridge` as side effect |
| `App.tsx` | 201 | Route definitions for all 15 routes; BRC-100 auth modal state; registers `window.showBRC100AuthApprovalModal` global |
| `vite-env.d.ts` | 1 | Vite client type reference |

## Routes

Defined in `App.tsx`:

| Route | Component | Context |
|-------|-----------|---------|
| `/` | `MainBrowserView` | Main browser window (tab bar, address bar, toolbar) |
| `/newtab` | `NewTabPage` | New tab content page with search + quick tiles |
| `/browser-data` | `HistoryPage` | Browser data page (history, cookies, cache tabs) |
| `/settings-page` | `SettingsPage` | Full-page settings with sidebar nav |
| `/settings-page/:section` | `SettingsPage` | Settings with specific section pre-selected |
| `/cert-error` | `CertErrorPage` | SSL certificate error interstitial |
| `/wallet-panel` | `WalletPanelPage` | Wallet setup/management overlay (1677 lines) |
| `/settings` | `SettingsOverlayRoot` | Settings overlay subprocess |
| `/wallet` | `WalletOverlayRoot` | Wallet dashboard overlay |
| `/backup` | `BackupOverlayRoot` | Mnemonic backup modal overlay |
| `/brc100-auth` | `BRC100AuthOverlayRoot` | BRC-100 authentication notifications |
| `/omnibox` | `OmniboxOverlayRoot` | Address bar autocomplete dropdown |
| `/privacy-shield` | `PrivacyShieldOverlayRoot` | Per-domain privacy stats overlay |
| `/downloads` | `DownloadsOverlayRoot` | Downloads panel overlay |
| `/profile-picker` | `ProfilePickerOverlayRoot` | Profile picker dropdown overlay |
| `/menu` | `MenuOverlayRoot` | Three-dot menu dropdown overlay |

## Directory Structure

```
src/
├── bridge/           # Window bridge + BRC-100 bridge (IPC to C++)
├── components/       # Reusable React components
│   ├── panels/       #   Wallet panel layout + backup modal
│   ├── settings/     #   Settings sub-pages (general, privacy, downloads, wallet, about)
│   └── wallet/       #   Wallet tabs (dashboard, activity, certificates, approved sites, settings)
├── hooks/            # 20 custom hooks for CEF/wallet communication
├── pages/            # Route-level pages and overlay roots
├── services/         # Shared services (balance cache)
├── types/            # TypeScript type definitions
└── utils/            # Pure utility functions (URL detection, suggestion ranking)
```

## Subdirectory CLAUDE.md Index

Each subdirectory has its own detailed CLAUDE.md:

| Directory | Doc | Key Content |
|-----------|-----|-------------|
| `bridge/` | [bridge/CLAUDE.md](bridge/CLAUDE.md) | IPC pattern, API namespaces (8 namespace groups), BRC-100 bridge class, guard pattern |
| `components/` | [components/CLAUDE.md](components/CLAUDE.md) | 20+ components: browser chrome, wallet, privacy, settings |
| `components/panels/` | [components/panels/CLAUDE.md](components/panels/CLAUDE.md) | `WalletPanelContent`, `WalletPanelLayout`, `BackupModal` |
| `components/settings/` | [components/settings/CLAUDE.md](components/settings/CLAUDE.md) | Settings sub-pages: `GeneralSettings`, `PrivacySettings`, etc. |
| `components/wallet/` | [components/wallet/CLAUDE.md](components/wallet/CLAUDE.md) | Wallet tabs: `DashboardTab`, `ActivityTab`, `CertificatesTab`, etc. |
| `hooks/` | [hooks/CLAUDE.md](hooks/CLAUDE.md) | 20 hooks with communication patterns, polling intervals, return types |
| `pages/` | [pages/CLAUDE.md](pages/CLAUDE.md) | Page/overlay catalog, IPC message reference, close prevention patterns |
| `types/` | [types/CLAUDE.md](types/CLAUDE.md) | Type definitions, Window API surface, timestamp conventions |
| `utils/` | [utils/CLAUDE.md](utils/CLAUDE.md) | URL detection, suggestion ranking/merging |

## Communication Architecture

The frontend uses three communication patterns to talk to the C++ CEF shell:

### 1. V8 Bridge (`window.hodosBrowser.*`)
Functions injected by C++ `simple_render_process_handler.cpp` into the V8 context. Used for wallet, history, navigation, and address operations.

```typescript
// Async (most operations)
const balance = await window.hodosBrowser.wallet.getBalance();

// Sync (history only)
const entries = window.hodosBrowser.history.get({ limit: 50 });
```

### 2. IPC Callbacks (`cefMessage.send()` → `window.onXxxResponse`)
Asynchronous message passing with one-shot global callbacks. Used for cookies, cookie blocking, settings, profiles, bookmarks.

```typescript
window.onCookieGetAllResponse = (data) => { resolve(data); };
window.cefMessage?.send('cookie_get_all', []);
```

### 3. PostMessage Events
Used by downloads and tab manager for continuous state updates.

```typescript
window.addEventListener('message', (event) => {
  if (event.data?.type === 'download_state_update') { ... }
});
```

## Key Exports

| Export | File | Description |
|--------|------|-------------|
| `App` (default) | `App.tsx` | Root component with router and BRC-100 auth modal |
| `brc100` | `bridge/brc100.ts` | `BRC100Bridge` singleton for BRC-100 protocol operations |
| `useHodosBrowser()` | `hooks/useHodosBrowser.ts` | Primary bridge hook: navigate, identity, address gen |
| `useTabManager()` | `hooks/useTabManager.ts` | Tab CRUD, switching, reordering, tear-off |
| `useKeyboardShortcuts()` | `hooks/useKeyboardShortcuts.ts` | Chrome-like keyboard shortcut handler |
| `useDownloads()` | `hooks/useDownloads.ts` | Download state and control functions |
| `useWallet()` | `hooks/useWallet.ts` | Wallet lifecycle (create/load/status/balance/send) |
| `useBalance()` | `hooks/useBalance.ts` | Balance with localStorage caching + USD conversion |
| `usePrivacyShield()` | `hooks/usePrivacyShield.ts` | Composite: adblock + cookie blocking state |
| `useSettings()` | `hooks/useSettings.ts` | Settings CRUD with dot-notation keys |
| `isUrl()`, `normalizeUrl()`, `toSearchUrl()` | `utils/urlDetection.ts` | URL vs search query detection |
| `rankAndMergeSuggestions()` | `utils/suggestionRanker.ts` | Omnibox suggestion merging |
| `DownloadItem` | `hooks/useDownloads.ts` | Download item interface |
| `Tab` | `types/TabTypes.ts` | Browser tab interface |
| `Suggestion` | `types/omnibox.ts` | Omnibox suggestion interface |

## Services

| File | Purpose |
|------|---------|
| `services/balanceCache.ts` | localStorage-based cache for wallet balance (60s TTL) and BSV price (10min TTL). Shared across CEF overlay subprocesses via same-origin `localhost:5137` |

Functions: `getCachedBalance()`, `setCachedBalance()`, `getCachedPrice()`, `setCachedPrice()`, `isBalanceStale()`, `isPriceStale()`

## Initialization Flow

1. `main.tsx` imports `bridge/initWindowBridge` (side effect — populates `window.hodosBrowser` APIs)
2. `main.tsx` renders `<BrowserRouter><App /></BrowserRouter>`
3. `App.tsx` registers `window.showBRC100AuthApprovalModal` for C++ to call
4. `App.tsx` initializes `BRC100Bridge` singleton via `brc100.isAvailable()`
5. React Router renders the appropriate page based on URL path
6. For main view (`/`): `MainBrowserView` initializes tab manager, keyboard shortcuts, balance poller
7. For overlays: the overlay root component reads query params and communicates via IPC

## Invariants

1. **No private keys in JavaScript** — all signing happens in Rust
2. **No direct Rust wallet calls** — everything goes through `window.hodosBrowser.*` or `cefMessage.send()`
3. **No new routes without C++ HWND setup** — overlay routes need creation functions in `cef_browser_shell.cpp` (Windows) and `cef_browser_shell_mac.mm` (macOS)
4. **No MUI TextField in overlays** — use native `<input>` elements for CEF compatibility
5. **No hidden file inputs in overlays** — use visible `<input type="file">` elements
6. **Guard pattern for bridge init** — `initWindowBridge.ts` checks `if (!window.hodosBrowser.xxx)` before defining, to avoid overwriting V8-injected methods

## Related

- [../CLAUDE.md](../CLAUDE.md) — Frontend layer overview, build commands, entry points
- [../../CLAUDE.md](../../CLAUDE.md) — Root project context, full architecture, overlay lifecycle, CEF input patterns
