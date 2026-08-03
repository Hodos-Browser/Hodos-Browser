# Hooks — Frontend React Hooks
> Custom React hooks providing the bridge between React UI and CEF/Rust backend via IPC and V8 injection.

**Last Updated:** 2026-08-03

## Overview

This directory holds **23 `.ts` hook files** (22 live + 1 dead duplicate). They encapsulate all communication between the React frontend and the C++ CEF shell / Rust wallet backend, using three communication patterns:

1. **V8 / bridge calls** (`window.hodosBrowser.*`) — Namespaced functions either injected into the V8 JavaScript context by C++ (`simple_render_process_handler.cpp`) or, where C++ hasn't injected them, defined as `cefMessage` promise wrappers by `bridge/initWindowBridge.ts` under its guard pattern. Used for wallet, history, bookmarks, address generation, navigation, and Google Suggest.
2. **IPC Window Callbacks** (`cefMessage.send()` → `window.onXxxResponse`) — Asynchronous message passing to C++ with responses delivered via global window callbacks. Used for adblock, cookies, settings, profiles, site permissions, paid-content cache, and imports.
3. **PostMessage Events** (`window.addEventListener('message', …)`) — Continuous state pushes from C++. Used for downloads and the tab manager.

**No hook opens an HTTP connection to the Rust wallet and no hook hardcodes a wallet port.** Hooks reach the wallet through `window.hodosBrowser.wallet.*`; that IPC lands in C++, and **C++ owns the wallet port** — `127.0.0.1:31301` release / `31401` under `HODOS_DEV=1` (source of truth: `cef-native/include/core/PortConfig.h`; adblock is `31302` / `31402`).

> **Wallet-call path (current).** There are two wallet transports, and hooks use only the first:
> - **Hooks** → `window.hodosBrowser.wallet.*` → `cefMessage` IPC → C++ → Rust wallet.
> - **First-party wallet pages/components** (`WalletPanelPage`, `DashboardTab`, `TransactionForm`, `ActivityTab`, `CertificatesTab`, `TokensTab`, `ApprovedSitesTab`, `SettingsTab`, `DomainPermission*`, `BRC100AuthOverlayRoot`, `MainBrowserView`) → `services/walletApi.ts :: walletFetch` → `window.__hodos_walletCall` → the **`wallet_call` CefProcessMessage**, answered by the paired **`wallet_response`** message handled in `simple_render_process_handler.cpp`. This replaced the old direct `fetch('http://127.0.0.1:31301' + path)` so the dev browser and the installed browser can run simultaneously.
>
> No hook in this directory calls `walletFetch` / `__hodos_walletCall` today. If you add a hook that needs an arbitrary wallet endpoint, use `walletFetch` — do **not** reintroduce a hardcoded port.

Permission decisions are **not** made here or anywhere in C++. The decision engine is Rust (`rust-wallet/crates/hodos_permission_engine`); the frontend only renders prompts and reads/writes limits.

## Hooks Reference

| Hook | Purpose | Communication | Polling |
|------|---------|---------------|---------|
| `useHodosBrowser` | Navigation, identity, address generation | V8 + IPC | No |
| `useWallet` | Wallet lifecycle (create/load/status/balance/send) | `window.hodosBrowser.wallet.*` | No |
| `useBalance` | Balance + USD conversion | V8 + localStorage cache | No |
| `useBackgroundBalancePoller` | Keeps balance cache warm for overlays | V8 → localStorage | 30s |
| `useAddress` | BSV address generation + clipboard | V8 | No |
| `useTransaction` | Send BSV transactions | V8 | No |
| `useAdblock` | Ad blocking toggle + blocked count | IPC window callbacks | 10s |
| `useCookieBlocking` | Cookie domain blocking + third-party control | IPC window callbacks | 10s |
| `useCookies` | Cookie CRUD + browser cache management | IPC window callbacks | No |
| `usePrivacyShield` | Composite: adblock + cookie blocking + per-site fingerprinting | Composed hooks + IPC | No |
| `useSitePermissions` | Web-content (OS-capability) permissions tri-state | IPC window callback | No |
| `useSettings` | Settings CRUD (browser/privacy/wallet) | IPC window callback | No |
| `useProfiles` | Browser profile management | IPC window callback | No |
| `useHistory` | Browsing history CRUD | V8 (synchronous) | No |
| `useBookmarks` | Bookmark list state over the bookmark bridge | `window.hodosBrowser.bookmarks` | No |
| `useDownloads` | Download tracking + controls | IPC postMessage | No |
| `useTabManager` | Tab lifecycle, reordering, payment badge | IPC postMessage | 30s (safety net) |
| `usePaidCache` | BRC-121 paid-content cache size + clear | IPC window callbacks | No |
| `useImport` | Import bookmarks/history from other browsers | IPC window callbacks | No |
| `useOmniboxSuggestions` | History + Google autocomplete for omnibox | V8 + custom events | No |
| `useKeyboardShortcuts` | Global keyboard shortcut registration | DOM events | No |
| `useDebounce` | Generic callback debouncing utility | N/A | No |
| `useBitcoinBrowser` | **Dead file** — byte-identical copy of `useHodosBrowser.ts`, and it even exports the symbol `useHodosBrowser`. Zero importers. | — | — |

**Currently unconsumed by any component/page:** `useWallet` (no importers anywhere in `frontend/src`) and `useBitcoinBrowser` (dead). `useDebounce` has exactly one consumer — `useOmniboxSuggestions`. Everything else is imported by at least one `.tsx` page or component.

## Communication Patterns

### V8 / Bridge Pattern (`window.hodosBrowser.*`)
Used by: `useWallet`, `useBalance`, `useBackgroundBalancePoller`, `useAddress`, `useTransaction`, `useHistory`, `useBookmarks`, `useHodosBrowser`, `useOmniboxSuggestions`

```typescript
// Async bridge call (most hooks)
const result = await window.hodosBrowser.wallet.getBalance();

// Sync V8 call (useHistory + useOmniboxSuggestions' frecency search only)
const entries = window.hodosBrowser.history.get(params);
```

Namespaces are populated by C++ V8 injection in `OnContextCreated` and/or by `bridge/initWindowBridge.ts` (which only defines a namespace when C++ hasn't). Functions return Promises (async) or direct values (sync). Always check availability before calling:

```typescript
if (!window.hodosBrowser?.wallet?.getBalance) {
  throw new Error('Bridge not available');
}
```

### IPC Window Callback Pattern (`cefMessage.send()`)
Used by: `useAdblock`, `useCookieBlocking`, `useCookies`, `usePaidCache`, `useSettings`, `useProfiles`, `useSitePermissions`, `useImport`, and the direct-IPC parts of `usePrivacyShield`

```typescript
// 1. Register callback on window
window.onCookieBlocklistResponse = (data: string) => {
  const parsed = JSON.parse(data);
  resolve(parsed);
};
// 2. Send IPC message
window.cefMessage.send('cookie_get_blocklist', []);
// 3. Timeout fallback (3-5 seconds typical)
setTimeout(() => reject(new Error('Timeout')), 5000);
```

C++ dispatches IPC in `simple_handler.cpp`, calls `frame->ExecuteJavaScript()` to invoke the window callback.

Two hooks deliberately keep a **persistent** (not one-shot) callback because C++ re-emits the authoritative state after every mutation: `useSitePermissions` (`onSitePermissionsResponse`) and `useSettings` (`onSettingsResponse`).

### PostMessage Pattern
Used by: `useDownloads`, `useTabManager`

```typescript
window.addEventListener('message', (event) => {
  if (event.data?.type === 'download_state_update') {
    setDownloads(JSON.parse(event.data.data));
  }
});
```

## Hook Details

### useAddress
```typescript
function useAddress(): {
  currentAddress: string;
  isGenerating: boolean;
  error: string | null;
  generateAddress: () => Promise<string>;
  copyToClipboard: (text: string) => Promise<void>;
  generateAndCopy: () => Promise<string>;
}
```
Generates BSV addresses via `window.hodosBrowser.address.generate()`. `generateAndCopy` is a convenience wrapper that generates then copies to clipboard via `navigator.clipboard.writeText()`.

### useTransaction
```typescript
function useTransaction(): {
  transactions: Transaction[];  // always empty — unused state
  isLoading: boolean;
  error: string | null;
  sendTransaction: (data: TransactionData) => Promise<TransactionResponse>;
}
```
Sends BSV transactions via `window.hodosBrowser.wallet.sendTransaction()`. Converts `amount` from BSV string to satoshis (`Math.round(parseFloat(amount) * 1e8)`) and maps `recipient` → `toAddress`. Supports `sendMax` flag and custom `feeRate`. Types imported from `types/transaction.d.ts`.

### useHodosBrowser
```typescript
function useHodosBrowser(): {
  getIdentity: () => Promise<IdentityResult>;
  markBackedUp: () => Promise<string>;
  generateAddress: () => Promise<AddressData>;
  navigate: (path: string) => void;
  goBack: () => void;
  goForward: () => void;
  reload: () => void;
}
```
Primary bridge hook. `generateAddress` has special logic: overlays use direct V8 calls while the main browser uses V8 + `cefMessageResponse` event listener fallback with a 10s timeout. Navigation IPC: `navigate_back`, `navigate_forward`, `navigate_reload`.

### useWallet
```typescript
function useWallet(): WalletState & {
  checkWalletStatus: () => Promise<any>;
  createWallet: () => Promise<any>;
  loadWallet: () => Promise<any>;
  getWalletInfo: () => Promise<any>;
  generateAddress: () => Promise<any>;
  getCurrentAddress: () => Promise<any>;
  markBackedUp: () => Promise<any>;
  getBalance: () => Promise<any>;
  sendTransaction: (recipient: string, amount: number) => Promise<any>;
}

interface WalletState {
  address: string | null;
  mnemonic: string | null;
  isInitialized: boolean;
  backedUp: boolean;
  version: string | null;
}
```
Full wallet lifecycle; every method goes through `window.hodosBrowser.wallet.*`. `createWallet()` unwraps the nested `{ success, wallet: { mnemonic, address, version } }` response shape into `WalletState`.

> **Security invariant:** the BIP39 mnemonic held in `WalletState.mnemonic` is display-once material. It must never be written to the console, to `localStorage`, or to any log sink — a `console.log` of it was removed from `createWallet` and must not come back. Only `loadWallet` / `markBackedUp` / `generateAddress` / `getBalance` / `sendTransaction` log, and none of them touch the mnemonic.

**No current importers** — this hook is presently unconsumed; wallet UI goes through `services/walletApi.ts` instead. Treat it as a supported-but-idle surface, not as the live wallet path.

### useBalance
```typescript
function useBalance(): {
  balance: number;       // satoshis
  usdValue: number;      // calculated
  bsvPrice: number;      // USD per BSV
  isLoading: boolean;
  isRefreshing: boolean;
  error: string | null;
  refreshBalance: () => Promise<void>;
}
```
Seeds state from the `localStorage` cache synchronously on mount for instant display; the background poller keeps that cache fresh, so there is **no interval here**. Auto-fetches once on mount only when the cache is empty (e.g. right after wallet recovery). Updates the cache on refresh. Exports `calculateUsdValue(satoshis, bsvPrice)` helper.

### useBackgroundBalancePoller
```typescript
function useBackgroundBalancePoller(): void
```
Runs in `MainBrowserView` only. Polls `window.hodosBrowser.wallet.getBalance()` every 30s (`BALANCE_POLL_MS = 30_000`, 500ms initial delay). Writes to `localStorage` via the `balanceCache` service so wallet overlay subprocesses (separate CEF processes, same origin) can read fresh data without their own bridge. Price piggybacks on the balance response (backend holds a 5-min TTL price cache), so there is no separate price poll. Errors are swallowed silently by design.

### useAdblock
```typescript
function useAdblock(): {
  blockedCount: number;
  adblockEnabled: boolean;
  scriptletsEnabled: boolean;
  fetchBlockedCount: () => Promise<number>;
  resetBlockedCount: () => Promise<void>;
  toggleSiteAdblock: (domain: string, enabled: boolean) => Promise<boolean>;
  checkSiteAdblock: (domain: string) => Promise<boolean>;
  toggleScriptlets: (domain: string, enabled: boolean) => Promise<boolean>;
  checkScriptlets: (domain: string) => Promise<boolean>;
}
```
IPC messages (6): `adblock_get_blocked_count`, `adblock_reset_blocked_count`, `adblock_site_toggle`, `adblock_scriptlet_toggle`, `adblock_check_site_enabled`, `adblock_check_scriptlets_enabled`. Polls blocked count on a **10s** interval.

### useCookieBlocking
```typescript
function useCookieBlocking(): {
  blockedDomains: BlockedDomainEntry[];
  blockLog: BlockLogEntry[];
  blockedCount: number;
  loading: boolean;
  error: string | null;
  fetchBlockList: () => Promise<BlockedDomainEntry[]>;
  blockDomain: (domain: string, isWildcard: boolean) => Promise<BlockDomainResponse>;
  unblockDomain: (domain: string) => Promise<UnblockDomainResponse>;
  allowThirdParty: (domain: string) => Promise<AllowThirdPartyResponse>;
  removeThirdPartyAllow: (domain: string) => Promise<AllowThirdPartyResponse>;
  fetchBlockLog: (limit?: number, offset?: number) => Promise<BlockLogEntry[]>;
  clearBlockLog: () => Promise<ClearBlockLogResponse>;
  fetchBlockedCount: () => Promise<BlockedCountResponse>;
  resetBlockedCount: () => Promise<void>;
}
```
Largest IPC surface in this directory — **9 message types**: `cookie_block_domain`, `cookie_unblock_domain`, `cookie_get_blocklist`, `cookie_allow_third_party`, `cookie_remove_third_party_allow`, `cookie_get_block_log`, `cookie_clear_block_log`, `cookie_get_blocked_count`, `cookie_reset_blocked_count`. Supports wildcard domain blocking and third-party cookie allow/deny per domain. Polls blocked count on a **10s** interval. Optimistic state updates on block/unblock.

### usePrivacyShield
```typescript
function usePrivacyShield(domain: string, refreshKey: number = 0): {
  masterEnabled: boolean;
  toggleMaster: (d: string, enable: boolean) => Promise<void>;
  totalBlockedCount: number;
  adblockEnabled: boolean;
  adblockBlockedCount: number;
  toggleSiteAdblock: (d: string, enable: boolean) => Promise<boolean>;
  scriptletsEnabled: boolean;
  toggleScriptlets: (d: string, enable: boolean) => Promise<boolean>;
  cookieBlockingEnabled: boolean;
  cookieBlockedCount: number;
  toggleCookieBlocking: (d: string, enable: boolean) => Promise<void>;
  fingerprintSiteEnabled: boolean;
  toggleFingerprintSite: (d: string, enabled: boolean) => void;
  fingerprintNeedsReload: boolean;
  blockedDomains: BlockedDomainEntry[];
  blockLog: BlockLogEntry[];
  fetchBlockList: () => Promise<BlockedDomainEntry[]>;
  fetchBlockLog: () => Promise<BlockLogEntry[]>;
  clearBlockLog: () => Promise<ClearBlockLogResponse>;
  blockDomain: (domain: string, isWildcard: boolean) => Promise<BlockDomainResponse>;
  unblockDomain: (domain: string) => Promise<UnblockDomainResponse>;
}
```
**Composite hook** — wraps `useAdblock()` + `useCookieBlocking()` and adds per-site fingerprint protection on top. Master toggle drives all three (adblock, scriptlets, cookie blocking). Own IPC: `cookie_check_site_allowed`, `fingerprint_get_site_enabled`, `fingerprint_set_site_enabled`. Inversion logic: `cookieBlockingEnabled = !cookieSiteAllowed`. `fingerprintNeedsReload` tells the UI a page reload is required for a farbling change to take effect.

The second argument `refreshKey` exists because the shield overlay is keep-alive: bumping it forces a re-fetch on every re-show, so a toggle changed in one surface (e.g. the site-info hub) isn't shown stale in another (e.g. the Shield panel) for the same domain. `useSitePermissions` uses the same convention.

### useSitePermissions
```typescript
type SitePermState = 'ask' | 'allow' | 'block';
interface SitePermission { code: string; state: SitePermState; }

function useSitePermissions(host: string, refreshKey: number = 0): {
  permissions: SitePermission[];
  setPermission: (code: string, state: SitePermState) => void;
  resetPermissions: () => void;
}
```
Web-content (OS-capability) permissions surfaced in the site-info hub — camera, mic, location, etc. Mirrors the C++ `SitePermissionStore` tri-state; the `code` strings must match `kSitePermCaps` in `simple_handler.cpp`. IPC: `site_permissions_get`, `site_permissions_set`, `site_permissions_reset`. Uses a **persistent** `window.onSitePermissionsResponse` handler because C++ re-emits the full authoritative list after every get/set/reset. Optimistic local update on set/reset, confirmed by the re-emit. Consumer: `pages/SiteInfoOverlayRoot.tsx`.

### useCookies
```typescript
function useCookies(): {
  cookies: CookieData[];
  domainGroups: DomainCookieGroup[];
  loading: boolean;
  error: string | null;
  cacheSize: number;
  fetchAllCookies: () => Promise<CookieData[]>;
  deleteCookie: (url: string, name: string) => Promise<CookieDeleteResponse>;
  deleteDomainCookies: (domain: string) => Promise<CookieDeleteResponse>;
  deleteAllCookies: () => Promise<CookieDeleteResponse>;
  clearCache: () => Promise<{ success: boolean }>;
  getCacheSize: () => Promise<CacheSizeResponse>;
  groupByDomain: (cookieList: CookieData[]) => DomainCookieGroup[];
}
```
Cookie management (read/delete) plus browser cache operations. `groupByDomain` normalizes leading dots and sorts by count descending. IPC: `cookie_get_all`, `cookie_delete`, `cookie_delete_domain`, `cookie_delete_all`, `cache_clear`, `cache_get_size`.

### usePaidCache
```typescript
function usePaidCache(): {
  totalBytes: number;
  enabled: boolean;
  loading: boolean;
  refresh: () => Promise<PaidCacheSizeResponse>;
  clear: () => Promise<PaidCacheClearResponse>;
}
```
Phase 1 BRC-121 **Paid Content Cache** control surface — a thin hook over the IPC pair `paid_cache_get_size` / `paid_cache_clear` (3s timeouts on both). Shaped to mirror `useCookies`' `getCacheSize` / `clearCache` so `components/CachePanel.tsx` can render it beside the existing Cache and Cookies cards. `enabled` reflects the `privacy.paidContentCacheEnabled` setting. Backed by `cef-native/src/core/PaidContentCache.cpp`. Auto-refreshes once on mount.

### useSettings
```typescript
function useSettings(): {
  settings: AllSettings;
  loading: boolean;
  error: string | null;
  updateSetting: (key: string, value: string | number | boolean) => void;
  refresh: () => void;
}

interface AllSettings {
  version: number;
  browser: BrowserSettings;   // homepage, searchEngine, zoomLevel, showBookmarkBar,
                              // downloadsPath, restoreSessionOnStart, askWhereToSave,
                              // autoUpdateMode: 'off' | 'notify' | 'silent'
  privacy: PrivacySettings;   // adBlockEnabled, thirdPartyCookieBlocking, doNotTrack,
                              // clearDataOnExit, fingerprintProtection, paidContentCacheEnabled
  wallet: WalletSettings;     // autoApproveEnabled, defaultPerTxLimitCents,
                              // defaultPerSessionLimitCents, defaultRateLimitPerMin
}
```
Dot-notation key paths for `updateSetting` (e.g. `"browser.zoomLevel"`). Optimistic local state updates; booleans are stringified for IPC. IPC: `settings_get_all`, `settings_set`; response callback `window.onSettingsResponse`.

The `defaultSettings` object in this file is only a pre-load placeholder — it is overwritten the moment `settings_get_all` returns. **The authoritative defaults live in C++** (`cef-native/include/core/SettingsManager.h` :: `WalletSettings`): `defaultPerTxLimitCents = 100` (**$1.00 per transaction**), `defaultPerSessionLimitCents = 1000` (**$10.00 per session**), `defaultRateLimitPerMin = 30`, plus `defaultMaxTxPerSession = 100` and `peerpayAutoAccept = true`, neither of which the frontend `WalletSettings` interface models. Never quote this file's placeholder numbers as the product defaults.

### useHistory
```typescript
function useHistory(): {
  history: HistoryEntry[];
  loading: boolean;
  error: string | null;
  fetchHistory: (params?: HistoryGetParams) => void;
  searchHistory: (params: HistorySearchParams) => void;
  deleteEntry: (url: string) => boolean;
  clearAllHistory: () => boolean;
  clearHistoryRange: (startTime: number, endTime: number) => boolean;
  chromiumTimeToDate: (chromiumTime: number) => Date;
  dateToChromiumTime: (date: Date) => number;
}
```
**Synchronous V8 calls** (not async like other hooks). Chromium timestamps use microseconds since 1601-01-01 (epoch offset: 11644473600 seconds). Utility converters exported for UI display.

### useBookmarks
```typescript
function useBookmarks(): {
  bookmarks: BookmarkData[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  search: (query: string) => Promise<void>;
  isBookmarked: (url: string) => Promise<boolean>;
  add: (url: string, title: string) => Promise<boolean>;
  remove: (id: number) => Promise<boolean>;
  removeByUrl: (url: string) => Promise<boolean>;
}
```
React state wrapper over the canonical bookmark bridge (`window.hodosBrowser.bookmarks`, defined in `bridge/initWindowBridge.ts`). Mirrors the `useHistory` pattern: **no raw IPC here** — the bridge is the single source of truth and this hook only adds list state plus ergonomics. Page size is 200 for both `getAll` and `search`.

`refresh()` retries **3 times with 400ms backoff**: on a saturated UI thread (slow Win10) the `getAll` round-trip can outlive its bridge timeout and the late response is dropped, which would otherwise leave the list silently empty.

`removeByUrl` exists for the current-page star toggle, which has a URL but not a bookmark id — it searches, matches on exact `url`, then delegates to `remove(id)`.

Known wart documented in the file: `Window.hodosBrowser` is declared in two places (`types/hodosBrowser.d.ts` and `bridge/brc100.ts`); TS resolves the `brc100.ts` shape, which omits `bookmarks`. The hook reaches the typed surface via a local `BookmarksBridge` interface + one cast, the same workaround `initWindowBridge.ts` uses. Consumer: `pages/BookmarksOverlayRoot.tsx`.

### useDownloads
```typescript
function useDownloads(): {
  downloads: DownloadItem[];
  hasDownloads: boolean;
  hasActiveDownloads: boolean;
  cancelDownload: (id: number) => void;
  pauseDownload: (id: number) => void;
  resumeDownload: (id: number) => void;
  openFile: (id: number) => void;
  showInFolder: (id: number) => void;
  clearCompleted: () => void;
}

export interface DownloadItem {
  id: number;
  url: string;
  filename: string;
  fullPath: string;
  receivedBytes: number;
  totalBytes: number;
  percentComplete: number;
  currentSpeed: number;
  isInProgress: boolean;
  isComplete: boolean;
  isCanceled: boolean;
  isPaused: boolean;
}
```
Uses `postMessage` for state updates (not window callbacks). Control functions are fire-and-forget `cefMessage.send()` calls. IPC: `download_get_state`, `download_cancel`, `download_pause`, `download_resume`, `download_open`, `download_show_folder`, `download_clear_completed`.

### useTabManager
```typescript
function useTabManager(): {
  tabs: TabInfo[];
  activeTabId: number;
  isLoading: boolean;
  createTab: (url?: string) => void;
  closeTab: (tabId: number) => void;
  switchToTab: (tabId: number) => void;
  nextTab: () => void;
  prevTab: () => void;
  switchToTabByIndex: (index: number) => void;
  closeActiveTab: () => void;
  reorderTabs: (fromIndex: number, toIndex: number) => void;
  tearOffTab: (tabId: number, screenX: number, screenY: number) => void;
  refreshTabList: () => void;
}
```
IPC: `get_tab_list`, `tab_create`, `tab_close`, `tab_switch`, `tab_reorder`, `tab_tearoff`; responses arrive as `tab_list_response` postMessages. C++ now **pushes** tab updates on create/close/title change, so the interval is a **30s safety net** only (it catches favicon/loading-state edges) — not a 2s poll. New tabs default to `http://127.0.0.1:5137/newtab`.

Optimistic local updates for `switchToTab`, `closeTab`, and `reorderTabs`. Closing keeps a `recentlyClosedRef` suppression set for 3s so an in-flight `tab_list_response` can't resurrect a just-closed tab before C++'s `OnBeforeClose` cleanup lands; closing the *last* tab intentionally does not empty local state, because C++ creates a replacement NTP and pushes a new list.

**⚠️ Load-bearing safeguard — the GOLD PILL payment indicator.** This hook owns the React half of the auto-approved-payment badge (never call it a "green dot"). It listens for the `payment_success_indicator` postMessage, parses `{ cents, browserId }`, formats the amount (`< $0.01` under a cent), and stamps `tab.paymentIndicator = { amount, timestamp }` for `PAYMENT_BADGE_DURATION_MS = 6000`. Two invariants must survive any refactor here:
- **`browserId` in the payload is a `Tab::id`, not `CefBrowser::GetIdentifier()`.** C++ translates via `TabManager::GetTabIdForBrowserIdentifier()` before sending (fire sites: `HttpRequestInterceptor.cpp :: AsyncHTTPClient::OnRequestComplete` and `HttpRequestInterceptor.cpp :: firePaymentSuccessIpc`). The field keeps the old name for compat.
- **The `tab_list_response` handler merges, it does not replace.** C++ knows nothing about `paymentIndicator`; a wholesale replace wipes the badge the instant C++ pushes any unrelated tab update (title, loading state) — which happens during the very page load that triggered the badge. `paymentIndicator` is carried across for tabs present in both the old and new lists; the 6s auto-clear still does eventual cleanup.

### useOmniboxSuggestions
```typescript
function useOmniboxSuggestions(): {
  suggestions: Suggestion[];
  loading: boolean;
  autocomplete: string | null;
  search: (query: string) => void;
}
```
Two-phase search. History results are immediate (synchronous V8 via `window.hodosBrowser.history.searchWithFrecency`, `limit: 6`); Google suggestions fire from `window.hodosBrowser.googleSuggest.fetch` for queries ≥ 2 chars and return asynchronously on the `googleSuggestResponse` custom event. Results are merged/ranked by `utils/suggestionRanker.ts`. Request-ID tracking discards stale Google responses.

Two caveats worth knowing before editing: the `useDebounce(…, 200)` call here is currently an **empty stub** kept "for future use" — the Google fetch actually happens un-debounced inside `performSearch`. And this hook does **not** send an `omnibox_autocomplete` IPC; inline completion is returned in-band as the `autocomplete` string.

### useImport
```typescript
function useImport(): {
  profiles: DetectedProfile[];
  loading: boolean;
  importing: boolean;
  lastResult: ImportResult | null;
  refresh: () => void;
  importBookmarks: (profilePath: string) => void;
  importHistory: (profilePath: string, maxEntries?: number) => void;
  importAll: (profilePath: string, maxHistoryEntries?: number) => void;
}
```
Auto-detects browser profiles on mount. IPC: `import_detect_profiles`, `import_bookmarks`, `import_history`, `import_all`. Exports `DetectedProfile` and `ImportResult` interfaces.

### useProfiles
```typescript
function useProfiles(): {
  profiles: ProfileInfo[];
  currentProfile: ProfileInfo | undefined;
  currentProfileId: string;
  defaultProfileId: string;
  loading: boolean;
  fetchProfiles: () => void;
  createProfile: (name: string, color: string, avatarImage?: string) => void;
  renameProfile: (id: string, newName: string) => void;
  deleteProfile: (id: string) => void;
  switchProfile: (id: string) => void;
  setProfileColor: (id: string, color: string) => void;
  setProfileAvatar: (id: string, avatarImage: string) => void;
  setDefaultProfile: (id: string) => void;
}

export interface ProfileInfo { id; name; color; avatarInitial; avatarImage?: string /* base64 data URL */ }
export interface ProfilesState { currentProfileId; defaultProfileId; profiles }
```
Optimistic state updates for rename/delete/color/avatar/default. `switchProfile` opens a new window. IPC (**8 messages**): `profiles_get_all`, `profiles_create`, `profiles_rename`, `profiles_delete`, `profiles_switch`, `profiles_set_color`, `profiles_set_avatar`, `profiles_set_default`; response callback `window.onProfilesResult`.

### useKeyboardShortcuts
```typescript
function useKeyboardShortcuts(handlers: KeyboardShortcutHandlers): void
```
Registers a global `keydown` listener. `KeyboardShortcutHandlers` has **9 optional handlers**: `onNewTab`, `onCloseTab`, `onNextTab`, `onPrevTab`, `onSwitchToTab(index)`, `onFocusAddressBar`, `onReload`, `onFindInPage`, `onToggleDevTools`.

Bindings (`ctrl` = `ctrlKey || metaKey`): `Ctrl/Cmd+T` (new tab), `Ctrl/Cmd+W` (close tab), `Ctrl+Tab` / `Ctrl+Shift+Tab` (next/prev tab), `Ctrl+1-9` (switch to tab by index), `Ctrl/Cmd+L` or `F6` (focus address bar), `Ctrl/Cmd+R` or `F5` (reload), `Ctrl/Cmd+F` (find in page), `F12` or `Ctrl+Shift+I` (devtools).

### useDebounce
```typescript
function useDebounce<T extends (...args: any[]) => any>(
  callback: T,
  delay: number
): (...args: Parameters<T>) => void
```
Generic debounce utility. Stores latest callback in a ref to avoid stale closures. Creates the debounced function once via `useMemo`. Only consumer: `useOmniboxSuggestions`.

## Shared Patterns

- **Optimistic updates**: cookie deletion, profile changes, site permissions, tab switch/close/reorder, and settings updates modify local state immediately before IPC confirmation
- **Timeout fallbacks**: window-callback IPC calls include 3-5 second timeouts to prevent hanging promises (`usePaidCache` and the `usePrivacyShield`/`useSitePermissions` checks use 3s; adblock toggles use 5s)
- **Retry-on-slow-machine**: `useBookmarks.refresh()` retries 3× at 400ms because a dropped late response would otherwise present as an empty list
- **Mounted ref cleanup**: long-running hooks (`useBackgroundBalancePoller`, `useBalance`) track mount state to avoid state updates after unmount
- **Error extraction**: standard pattern `err instanceof Error ? err.message : 'Operation failed'`
- **Bridge availability checks**: bridge hooks validate `window.hodosBrowser?.module?.method` before calling
- **`refreshKey` second argument**: keep-alive overlays pass an incrementing key (`usePrivacyShield`, `useSitePermissions`) to force a re-fetch on re-show rather than only on subject change
- **Never log secrets**: no hook may log a mnemonic, private key, or seed

## Dependencies

| Service / Module | Used By | Purpose |
|---------|---------|---------|
| `services/balanceCache.ts` | `useBalance`, `useBackgroundBalancePoller` | localStorage-based balance/price cache shared across CEF subprocesses |
| `hooks/useDebounce.ts` | `useOmniboxSuggestions` | intra-directory dependency (currently a no-op stub at that call site) |
| `hooks/useAdblock.ts`, `hooks/useCookieBlocking.ts` | `usePrivacyShield` | composed sub-hooks |
| `types/identity.d.ts` | `useHodosBrowser`, `useBitcoinBrowser` (dead) | `IdentityResult` type |
| `types/address.d.ts` | `useHodosBrowser`, `useBitcoinBrowser` (dead) | `AddressData` type |
| `types/transaction.d.ts` | `useTransaction` | `TransactionData`, `Transaction`, `TransactionResponse` |
| `types/cookieBlocking.d.ts` | `useCookieBlocking` | `BlockedDomainEntry`, `BlockLogEntry`, + response types |
| `types/cookies.d.ts` | `useCookies` | `CookieData`, `DomainCookieGroup`, `CookieDeleteResponse`, `CacheSizeResponse` |
| `types/history.d.ts` | `useHistory` | `HistoryEntry`, `HistorySearchParams`, `HistoryGetParams` |
| `types/bookmarks.d.ts` | `useBookmarks` | `BookmarkData` + 5 response types (`Add`, `Remove`, `Search`, `GetAll`, `IsBookmarked`) |
| `types/TabTypes.ts` | `useTabManager` | `TabListResponse`, `TabManagerState` |
| `types/omnibox.ts` | `useOmniboxSuggestions` | `HistoryEntryWithFrecency`, `Suggestion` |
| `utils/suggestionRanker.ts` | `useOmniboxSuggestions` | `rankAndMergeSuggestions()`, `getAutocompleteSuggestion()` |

`usePaidCache`, `useSitePermissions`, `useSettings`, `useProfiles`, `useImport`, `useDownloads`, `useAdblock`, `useKeyboardShortcuts`, `useAddress`, `useWallet` and `useDebounce` declare their own local interfaces and import nothing from `types/`.

## Related

- [Frontend CLAUDE.md](../../../frontend/CLAUDE.md) — Frontend layer overview, entry points, invariants
- [Root CLAUDE.md](../../../CLAUDE.md) — Full architecture, CEF input patterns, overlay lifecycle
- [Bridge CLAUDE.md](../bridge/CLAUDE.md) — `window.hodosBrowser` namespaces these hooks call into
- [Wallet Components CLAUDE.md](../components/wallet/CLAUDE.md) — Wallet UI components (these use `services/walletApi.ts`, not `useWallet`)
- [Settings Components CLAUDE.md](../components/settings/CLAUDE.md) — Settings UI consuming `useSettings`
