# Frontend Source Root
> React/TypeScript application root: routing, bridge initialization, and directory organization for the Hodos Browser UI layer.

**Last Updated:** 2026-08-03

## Overview

This is the source root for the Hodos Browser frontend — a React SPA that runs inside CEF (Chromium Embedded Framework) to provide browser chrome, wallet UI, and overlay panels. The application never handles private keys or signing directly; all sensitive operations are delegated to the Rust wallet backend via `window.hodosBrowser.*`, `window.cefMessage.send()`, and the `window.__hodos_walletCall` wallet bridge — all injected by C++ V8 bindings.

The frontend serves two distinct roles simultaneously:
1. **Main browser chrome** — tab bar, address bar, navigation controls, toolbar icons (route `/`)
2. **Overlay subprocesses** — each overlay (wallet, settings, auth, downloads, etc.) is a separate CEF process loading the same React app at a different route

## Entry Points

| File | Lines | Purpose |
|------|-------|---------|
| `main.tsx` | 31 | React entry point; mounts `<BrowserRouter>` + `<App />`; imports `bridge/initWindowBridge` as side effect; removes the `#splash` element immediately on overlay routes and exposes `window.removeSplash()` so `MainBrowserView` can dismiss it after first paint on `/` |
| `App.tsx` | 205 | Route definitions for all 21 routes; BRC-100 auth modal state; registers `window.showBRC100AuthApprovalModal` global |
| `vite-env.d.ts` | 1 | Vite client type reference |

## Routes

Defined in `App.tsx` — **21 routes**. All are lazy-loaded via `React.lazy` inside a `React.Suspense` boundary **except** `MainBrowserView`, `NewTabPage`, `PaymentPendingPage` and `PaymentFailedPage`, which are eager-loaded (the payment pair deliberately so, to avoid a chunk-fetch flicker when the placeholder/error page swaps in).

| Route | Component | Lines | Context |
|-------|-----------|-------|---------|
| `/` | `MainBrowserView` | 1075 | Main browser window (tab bar, address bar, toolbar) |
| `/newtab` | `NewTabPage` | 387 | New tab content page with search + quick tiles |
| `/browser-data` | `HistoryPage` | 151 | Browser data page (history, cookies, cache tabs) |
| `/settings-page` | `SettingsPage` | 158 | Full-page settings with sidebar nav |
| `/settings-page/:section` | `SettingsPage` | 158 | Settings with specific section pre-selected |
| `/cert-error` | `CertErrorPage` | 214 | SSL certificate error interstitial |
| `/payment-pending` | `PaymentPendingPage` | 70 | BRC-121 background placeholder rendered behind the domain-approval modal for a paywalled article (query params: `domain`, `sats`) |
| `/payment-failed` | `PaymentFailedPage` | 142 | BRC-121 failure page after `Async402ResourceHandler` exhausts retries; nosend tx was never broadcast so funds are preserved (query params: `domain`, `sats`, `originalUrl`, `status`) |
| `/wallet-panel` | `WalletPanelPage` | 1619 | Wallet setup/management overlay |
| `/settings` | `SettingsOverlayRoot` | 577 | Settings overlay subprocess |
| `/wallet` | `WalletOverlayRoot` | 126 | Wallet dashboard overlay |
| `/backup` | `BackupOverlayRoot` | 361 | Mnemonic backup modal overlay |
| `/brc100-auth` | `BRC100AuthOverlayRoot` | 2580 | BRC-100 authentication / permission notifications (type-dispatched prompt overlay) |
| `/omnibox` | `OmniboxOverlayRoot` | 300 | Address bar autocomplete dropdown |
| `/privacy-shield` | `PrivacyShieldOverlayRoot` | 92 | Per-domain privacy stats overlay |
| `/downloads` | `DownloadsOverlayRoot` | 212 | Downloads panel overlay |
| `/bookmarks` | `BookmarksOverlayRoot` | 256 | Bookmarks list/manage dropdown overlay (`useBookmarks`) |
| `/site-info` | `SiteInfoOverlayRoot` | 341 | Site-info hub: TLS state, privacy shield, storage, wallet permissions, per-site capability toggles (`useSitePermissions`, `usePrivacyShield`) |
| `/tab-list` | `TabListOverlayRoot` | 195 | Tab-list caret dropdown + recently-closed list (`useTabManager`) |
| `/profile-picker` | `ProfilePickerOverlayRoot` | 783 | Profile picker dropdown overlay |
| `/menu` | `MenuOverlayRoot` | 215 | Three-dot menu dropdown overlay |

### Pages on disk with no route

`pages/` holds two components that `App.tsx` does **not** route. They are reachable only by direct import:

| File | Lines | Status |
|------|-------|--------|
| `CookiePanelOverlayRoot.tsx` | 73 | Cookie panel overlay root; wraps the `CookiePanelOverlay` component. No `<Route>` — the cookie panel is currently surfaced through the site-info / privacy-shield path instead. |
| `SendPage.tsx` | 145 | Legacy standalone transaction send page (`useBalance()` + `useTransaction()`). Superseded by the wallet overlay's send flow. |

## Directory Structure

```
src/
├── bridge/           # 1 module: initWindowBridge.ts (IPC to C++, 8 window.hodosBrowser namespaces)
├── components/       # 18 .tsx components + HodosButton.module.css + 2 plain .css
│   ├── __tests__/    #   EMPTY (no test files present)
│   ├── panels/       #   EMPTY of code — only its CLAUDE.md remains
│   ├── settings/     #   5 settings sub-pages
│   └── wallet/       #   7 wallet tabs/sidebar + WalletDashboard.css
├── hooks/            # 23 custom hooks for CEF/wallet communication
├── pages/            # 22 page/overlay components (21 routed, 2 orphaned — see above)
├── services/         # 2 modules: balanceCache.ts, walletApi.ts
├── styles/           # hodosTheme.ts (colors / fonts / prompt tiers)
├── theme/            # tokens.css + tokens.ts (design tokens, TS + CSS-var forms)
├── types/            # 10 type modules (8 .d.ts + TabTypes.ts + omnibox.ts)
└── utils/            # 3 modules: urlDetection.ts, suggestionRanker.ts, bip21.ts
```

Top-level loose files: `main.tsx`, `App.tsx`, `App.css`, `index.css`, `vite-env.d.ts`.

## Subdirectory CLAUDE.md Index

Ten subdirectory docs exist. `services/` and `theme/` have **no** CLAUDE.md — their contents are documented here instead.

| Directory | Doc | Key Content |
|-----------|-----|-------------|
| `bridge/` | [bridge/CLAUDE.md](bridge/CLAUDE.md) | IPC pattern, guard pattern, 8 `window.hodosBrowser` namespaces defined in `initWindowBridge.ts`: `navigation`, `overlay`, `address`, `wallet`, `omnibox`, `cookies`, `cookieBlocking`, `bookmarks` |
| `components/` | [components/CLAUDE.md](components/CLAUDE.md) | 18 components: browser chrome (`TabBar`, `TabComponent`, `FindBar`, `MenuOverlay`), wallet (`WalletPanel`, `BalanceDisplay`, `AddressManager`, `TransactionForm`, `TransactionHistory`), privacy (`PrivacyShieldPanel`, `CookiesPanel`, `CookiePanelOverlay`, `CachePanel`), permissions (`DomainPermissionForm`, `DomainPermissionsTab`), plus `BRC100AuthModal`, `HistoryPanel`, `HodosButton` |
| `components/panels/` | [components/panels/CLAUDE.md](components/panels/CLAUDE.md) | ⚠️ Directory contains **no source files** — only the doc. The `WalletPanelContent` / `WalletPanelLayout` / `BackupModal` components it describes no longer exist; that layout now lives in `pages/WalletPanelPage.tsx` and `pages/BackupOverlayRoot.tsx` |
| `components/settings/` | [components/settings/CLAUDE.md](components/settings/CLAUDE.md) | 5 files: `GeneralSettings`, `PrivacySettings`, `DownloadSettings`, `AboutSettings`, `SettingsCard` (shared card shell). There is **no** `WalletSettings` component — wallet settings live in `components/wallet/SettingsTab.tsx` |
| `components/wallet/` | [components/wallet/CLAUDE.md](components/wallet/CLAUDE.md) | 7 components: `DashboardTab`, `ActivityTab`, `CertificatesTab`, `ApprovedSitesTab`, `TokensTab`, `SettingsTab`, `WalletSidebar` |
| `hooks/` | [hooks/CLAUDE.md](hooks/CLAUDE.md) | 23 hook modules with communication patterns, polling intervals, return types |
| `pages/` | [pages/CLAUDE.md](pages/CLAUDE.md) | Page/overlay catalog, IPC message reference, close prevention patterns |
| `styles/` | [styles/CLAUDE.md](styles/CLAUDE.md) | `hodosTheme.ts`: `colors`, `fonts`, `prompt` (per-prompt-tier theming incl. the heightened-gold `privacy_perimeter` tier), default `hodosTheme` |
| `types/` | [types/CLAUDE.md](types/CLAUDE.md) | Type definitions, Window API surface, timestamp conventions |
| `utils/` | [utils/CLAUDE.md](utils/CLAUDE.md) | URL detection, suggestion ranking/merging, BIP-21 URI parsing |

### Hook roster (23)

`useAdblock`, `useAddress`, `useBackgroundBalancePoller`, `useBalance`, `useBitcoinBrowser`, `useBookmarks`, `useCookieBlocking`, `useCookies`, `useDebounce`, `useDownloads`, `useHistory`, `useHodosBrowser`, `useImport`, `useKeyboardShortcuts`, `useOmniboxSuggestions`, `usePaidCache`, `usePrivacyShield`, `useProfiles`, `useSettings`, `useSitePermissions`, `useTabManager`, `useTransaction`, `useWallet`.

> `useBitcoinBrowser.ts` exports a function named `useHodosBrowser()` — a duplicate of `hooks/useHodosBrowser.ts`. Nothing imports it; treat it as dead and import from `useHodosBrowser.ts`.

## Communication Architecture

The frontend uses four communication patterns to talk to the C++ CEF shell:

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

### 4. Wallet-call bridge (`walletFetch()` → `window.__hodos_walletCall`)
The first-party React wallet UI reaches the Rust wallet backend through the C++ `window.__hodos_walletCall` IPC bridge — **never** through a direct `fetch()` to a hardcoded port. C++ owns the wallet port (release `127.0.0.1:31301`, `HODOS_DEV=1` `127.0.0.1:31401`; source of truth is `cef-native/include/core/PortConfig.h`), which is what lets a dev browser and an installed browser run simultaneously. C++ also gates the call by the un-forgeable frame origin: a first-party loopback origin sends no `X-Requesting-Domain`, so Rust treats it as an internal call.

`services/walletApi.ts :: walletFetch(path, init)` is a drop-in for `fetch('<walletOrigin>' + path, init)` — same call shape, returns a fetch-like `WalletResponse` (`ok` / `status` / `statusText` / `json()` / `text()`), resolves (not throws) on a non-2xx wallet response, rejects on genuine transport failure, and honors `init.signal` with an `AbortError`.

```typescript
import { walletFetch } from '../services/walletApi';
const res = await walletFetch('/wallet/status');
if (res.ok) { const data = await res.json(); }
```

> The bridge is injected on every internal/overlay page by `simple_render_process_handler.cpp` (`WALLET_CALL_BRIDGE_SCRIPT`). If `window.__hodos_walletCall` is absent, `walletFetch` throws rather than falling back to a hardcoded port — deliberately loud.

## Key Exports

| Export | File | Description |
|--------|------|-------------|
| `App` (default) | `App.tsx` | Root component with router and BRC-100 auth modal |
| `walletFetch()`, `WalletResponse` | `services/walletApi.ts` | First-party wallet transport over the `window.__hodos_walletCall` bridge |
| `useHodosBrowser()` | `hooks/useHodosBrowser.ts` | Primary bridge hook: navigate, identity, address gen |
| `useTabManager()` | `hooks/useTabManager.ts` | Tab CRUD, switching, reordering, tear-off |
| `useKeyboardShortcuts()`, `KeyboardShortcutHandlers` | `hooks/useKeyboardShortcuts.ts` | Chrome-like keyboard shortcut handler |
| `useDownloads()` | `hooks/useDownloads.ts` | Download state and control functions |
| `useWallet()` | `hooks/useWallet.ts` | Wallet lifecycle (create/load/status/balance/send) |
| `useBalance()`, `calculateUsdValue()` | `hooks/useBalance.ts` | Balance with localStorage caching + USD conversion |
| `usePrivacyShield()` | `hooks/usePrivacyShield.ts` | Composite: adblock + cookie blocking state |
| `useSitePermissions()`, `SitePermState`, `SitePermission` | `hooks/useSitePermissions.ts` | Tri-state (`ask`/`allow`/`block`) web-content capability permissions; codes must match `kSitePermCaps` in `simple_handler.cpp` |
| `usePaidCache()` | `hooks/usePaidCache.ts` | BRC-121 paid-content cache size/clear over `paid_cache_get_size` / `paid_cache_clear` IPC |
| `useSettings()`, `AllSettings`, `BrowserSettings`, `PrivacySettings`, `WalletSettings` | `hooks/useSettings.ts` | Settings CRUD with dot-notation keys |
| `useProfiles()`, `ProfileInfo`, `ProfilesState` | `hooks/useProfiles.ts` | Profile list / create / switch |
| `useImport()`, `DetectedProfile`, `ImportResult` | `hooks/useImport.ts` | Chrome/Edge profile import |
| `isUrl()`, `normalizeUrl()`, `toSearchUrl()`, `toGoogleSearchUrl()` | `utils/urlDetection.ts` | URL vs search query detection |
| `rankAndMergeSuggestions()`, `getAutocompleteSuggestion()` | `utils/suggestionRanker.ts` | Omnibox suggestion merging + inline autocomplete |
| `parseBIP21()` | `utils/bip21.ts` | BIP-21 payment URI → `{ address, amount?, label? }` |
| `colors`, `fonts`, `prompt`, `hodosTheme` (default) | `styles/hodosTheme.ts` | Canonical brand palette / font stacks / per-prompt-tier theming |
| `tokens` | `theme/tokens.ts` | Design tokens in TS form; must match `theme/tokens.css` |
| `DownloadItem` | `hooks/useDownloads.ts` | Download item interface |
| `Tab`, `TabListResponse`, `TabManagerState` | `types/TabTypes.ts` | Browser tab interfaces |
| `Suggestion` | `types/omnibox.ts` | Omnibox suggestion interface |

> `bridge/brc100.ts` no longer exists. The legacy `window.hodosBrowser.brc100.*` V8 bindings were deleted in the startup-optimization pass (the `brc100.isAvailable()` probe made a synchronous native HTTP call on the renderer main thread and cost ~2s of first paint). Wallet UI calls now go through `walletFetch()`.

## Services

| File | Purpose |
|------|---------|
| `services/balanceCache.ts` | localStorage-based cache for wallet balance (`BALANCE_MAX_AGE_MS` = 60s, balance polls every 30s) and BSV price (`PRICE_MAX_AGE_MS` = 10min, price polls every 5min). Shared across CEF overlay subprocesses via same-origin `localhost:5137` |
| `services/walletApi.ts` | `walletFetch()` — first-party wallet transport over the C++ `window.__hodos_walletCall` bridge (see Communication Architecture §4) |

`balanceCache.ts` exports: `getCachedBalance()`, `setCachedBalance()`, `getCachedPrice()`, `setCachedPrice()`, `isBalanceStale()`, `isPriceStale()`, plus the `CachedBalance` / `CachedPrice` interfaces.

`walletApi.ts` exports: `walletFetch()`, `WalletResponse`.

## Initialization Flow

1. `main.tsx` imports `bridge/initWindowBridge` (side effect — populates the 8 `window.hodosBrowser` namespaces, guarded so V8-injected methods win)
2. `main.tsx` renders `<BrowserRouter><App /></BrowserRouter>`
3. `main.tsx` removes the `#splash` element immediately on any non-`/` route, and exposes `window.removeSplash()` for `MainBrowserView` to call after first paint on `/`
4. `App.tsx` registers `window.showBRC100AuthApprovalModal` for C++ to call
5. React Router renders the appropriate page based on URL path, inside a `React.Suspense` boundary for the lazy routes
6. For main view (`/`): `MainBrowserView` initializes tab manager, keyboard shortcuts, balance poller
7. For overlays: the overlay root component reads query params and communicates via IPC

> There is **no** BRC-100 availability probe at startup any more — it was removed for first paint. Do not reintroduce a synchronous native call on the render path.

## Invariants

1. **No private keys in JavaScript** — all signing happens in Rust
2. **No direct Rust wallet calls** — everything goes through `window.hodosBrowser.*`, `cefMessage.send()`, or `walletFetch()`
3. **No hardcoded wallet port in the frontend** — the wallet origin differs between dev (`31401`) and release (`31301`) and is owned by C++ (`PortConfig.h`). Use `walletFetch()`; never write `http://127.0.0.1:31301` into a `fetch()`
4. **No new routes without C++ HWND setup** — overlay routes need creation functions in `cef_browser_shell.cpp` (Windows) and `cef_browser_shell_mac.mm` (macOS)
5. **No MUI TextField in overlays** — use native `<input>` elements for CEF compatibility
6. **No hidden file inputs in overlays** — use visible `<input type="file">` elements
7. **Guard pattern for bridge init** — `initWindowBridge.ts` checks `if (!window.hodosBrowser.xxx)` before defining, to avoid overwriting V8-injected methods
8. **Nothing blocking on the first-paint path** — no synchronous native bridge calls during module init or the first render pass

## Related

- [../CLAUDE.md](../CLAUDE.md) — Frontend layer overview, build commands, entry points
- [../../CLAUDE.md](../../CLAUDE.md) — Root project context, full architecture, overlay lifecycle, CEF input patterns
