# Hooks — Frontend React Hooks
> Custom React hooks providing the bridge between React UI and CEF/Rust backend via IPC and V8 injection.

## Overview

These 20 hooks encapsulate all communication between the React frontend and the C++ CEF shell / Rust wallet backend. They use two primary communication patterns:

1. **V8 Bridge** (`window.hodosBrowser.*`) — Direct calls to functions injected into the V8 JavaScript context by C++ (`simple_render_process_handler.cpp`). Used for wallet operations, history, navigation, and address generation.
2. **IPC Window Callbacks** (`cefMessage.send()` → `window.onXxxResponse`) — Asynchronous message passing to C++ with responses delivered via global window callbacks. Used for privacy controls, cookies, settings, profiles, and imports.

No hook ever calls the Rust wallet directly — all wallet operations go through the V8 bridge.

## Hooks Reference

| Hook | Purpose | Communication | Polling |
|------|---------|---------------|---------|
| `useHodosBrowser` | Navigation, identity, address generation | V8 + IPC | No |
| `useWallet` | Wallet lifecycle (create/load/status) | V8 | No |
| `useBalance` | Balance + USD conversion | V8 + localStorage cache | No |
| `useBackgroundBalancePoller` | Keeps balance cache warm for overlays | V8 → localStorage | 30s |
| `useAddress` | BSV address generation + clipboard | V8 | No |
| `useTransaction` | Send BSV transactions | V8 | No |
| `useAdblock` | Ad blocking toggle + blocked count | IPC window callbacks | 2s |
| `useCookieBlocking` | Cookie domain blocking + third-party control | IPC window callbacks | 2s |
| `useCookies` | Cookie CRUD + cache management | IPC window callbacks | No |
| `usePrivacyShield` | Composite: adblock + cookie blocking | Composed hooks | No |
| `useSettings` | Settings CRUD (browser/privacy/wallet) | IPC window callbacks | No |
| `useProfiles` | Browser profile management | IPC window callbacks | No |
| `useHistory` | Browsing history CRUD | V8 (synchronous) | No |
| `useDownloads` | Download tracking + controls | IPC postMessage | No |
| `useTabManager` | Tab lifecycle + reordering | IPC postMessage | 2s |
| `useImport` | Import bookmarks/history from other browsers | IPC window callbacks | No |
| `useOmniboxSuggestions` | History + Google autocomplete for omnibox | V8 + custom events | No |
| `useKeyboardShortcuts` | Global keyboard shortcut registration | DOM events | No |
| `useDebounce` | Generic callback debouncing utility | N/A | No |
| `useBitcoinBrowser` | Dead file — identical copy of `useHodosBrowser` | — | — |

## Communication Patterns

### V8 Bridge Pattern (`window.hodosBrowser.*`)
Used by: `useWallet`, `useBalance`, `useAddress`, `useTransaction`, `useHistory`, `useHodosBrowser`

```typescript
// Async V8 call (most hooks)
const result = await window.hodosBrowser.wallet.getBalance();

// Sync V8 call (useHistory only)
const entries = window.hodosBrowser.history.get(params);
```

The V8 bridge is injected by `simple_render_process_handler.cpp` in `OnContextCreated`. Functions return Promises (async) or direct values (sync). Always check availability before calling:

```typescript
if (!window.hodosBrowser?.wallet?.getBalance) {
  throw new Error('Bridge not available');
}
```

### IPC Window Callback Pattern (`cefMessage.send()`)
Used by: `useAdblock`, `useCookieBlocking`, `useCookies`, `useSettings`, `useProfiles`, `useImport`

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
Primary bridge hook. `generateAddress` has special logic: overlays use direct V8 calls while the main browser uses V8 + `cefMessageResponse` event listener fallback with 10s timeout. Navigation IPC: `navigate_back`, `navigate_forward`, `navigate_reload`.

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
Full wallet lifecycle. `createWallet` returns mnemonic (display once, security). All methods go through `window.hodosBrowser.wallet.*`.

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
Seeds state from `localStorage` cache on mount for instant display. Updates cache on refresh. Exports `calculateUsdValue(satoshis, bsvPrice)` helper.

### useBackgroundBalancePoller
```typescript
function useBackgroundBalancePoller(): void
```
Runs in `MainBrowserView` only. Polls `window.hodosBrowser.wallet.getBalance()` every 30s (500ms initial delay). Writes to `localStorage` via `balanceCache` service so wallet overlay subprocesses (separate CEF processes, same origin) can read fresh data without their own V8 bridge.

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
IPC messages: `adblock_get_blocked_count`, `adblock_reset_blocked_count`, `adblock_site_toggle`, `adblock_scriptlet_toggle`, `adblock_check_site_enabled`, `adblock_check_scriptlets_enabled`. Polls blocked count every 2s.

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
Largest IPC surface (9 message types). Supports wildcard domain blocking and third-party cookie allow/deny per domain. Polls blocked count every 2s. Optimistic state updates on block/unblock.

### usePrivacyShield
```typescript
function usePrivacyShield(domain: string): {
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
  blockedDomains: BlockedDomainEntry[];
  blockLog: BlockLogEntry[];
  fetchBlockList: () => Promise<BlockedDomainEntry[]>;
  fetchBlockLog: () => Promise<BlockLogEntry[]>;
  clearBlockLog: () => Promise<ClearBlockLogResponse>;
  blockDomain: (domain: string, isWildcard: boolean) => Promise<BlockDomainResponse>;
  unblockDomain: (domain: string) => Promise<UnblockDomainResponse>;
}
```
**Composite hook** — wraps `useAdblock()` + `useCookieBlocking()`. Master toggle syncs both systems. Checks per-domain cookie allow status via `cookie_check_site_allowed` IPC. Inversion logic: `cookieBlockingEnabled = !cookieSiteAllowed`.

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
  browser: BrowserSettings;   // homepage, searchEngine, zoomLevel, showBookmarkBar, downloadsPath, restoreSessionOnStart, askWhereToSave
  privacy: PrivacySettings;   // adBlockEnabled, thirdPartyCookieBlocking, doNotTrack, clearDataOnExit, fingerprintProtection
  wallet: WalletSettings;     // autoApproveEnabled, defaultPerTxLimitCents, defaultPerSessionLimitCents, defaultRateLimitPerMin
}
```
Dot-notation key paths for `updateSetting` (e.g., `"browser.zoomLevel"`). Optimistic local state updates. Booleans converted to strings for IPC. IPC: `settings_get_all`, `settings_set`.

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
Polls tab list every 2s via `postMessage`. Optimistic local updates for `switchToTab` and `reorderTabs`. Tab create/close triggers delayed refresh (500ms). Circular next/prev tab switching. IPC: `get_tab_list`, `tab_create`, `tab_close`, `tab_switch`, `tab_reorder`, `tab_tearoff`.

### useOmniboxSuggestions
```typescript
function useOmniboxSuggestions(): {
  suggestions: Suggestion[];
  loading: boolean;
  autocomplete: string | null;
  search: (query: string) => void;
}
```
Two-phase search: history results are immediate (synchronous V8 via `window.hodosBrowser.history.searchWithFrecency`), Google suggestions are debounced (200ms, via `window.hodosBrowser.googleSuggest.fetch`). Merges and ranks results. Request ID tracking discards stale responses. Sends `omnibox_autocomplete` IPC for inline completion.

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
  loading: boolean;
  fetchProfiles: () => void;
  createProfile: (name: string, color: string, avatarImage?: string) => void;
  renameProfile: (id: string, newName: string) => void;
  deleteProfile: (id: string) => void;
  switchProfile: (id: string) => void;
}
```
Optimistic state updates for rename/delete. Avatar support via base64 data URL. `switchProfile` opens a new window. IPC: `profiles_get_all`, `profiles_create`, `profiles_rename`, `profiles_delete`, `profiles_switch`.

### useKeyboardShortcuts
```typescript
function useKeyboardShortcuts(handlers: KeyboardShortcutHandlers): void
```
Registers global `keydown` listener. Shortcuts: `Ctrl/Cmd+T` (new tab), `Ctrl/Cmd+W` (close tab), `Ctrl+Tab`/`Ctrl+Shift+Tab` (next/prev tab), `Ctrl+1-9` (switch tab), `Ctrl/Cmd+L`/`F6` (focus address bar), `Ctrl/Cmd+R`/`F5` (reload), `Ctrl/Cmd+F` (find), `F12`/`Ctrl+Shift+I` (devtools).

### useDebounce
```typescript
function useDebounce<T extends (...args: any[]) => any>(
  callback: T,
  delay: number
): (...args: Parameters<T>) => void
```
Generic debounce utility. Stores latest callback in ref to avoid stale closures. Creates debounced function once via `useMemo`.

## Shared Patterns

- **Optimistic updates**: Cookie deletion, profile changes, tab switching, and settings updates modify local state immediately before IPC confirmation
- **Timeout fallbacks**: Window callback IPC calls include 3-5 second timeouts to prevent hanging promises
- **Mounted ref cleanup**: Long-running hooks (`useBackgroundBalancePoller`, pollers) track mount state to avoid state updates after unmount
- **Error extraction**: Standard pattern `err instanceof Error ? err.message : 'Operation failed'`
- **Bridge availability checks**: V8 hooks validate `window.hodosBrowser?.module?.method` before calling

## Dependencies

| Service | Used By | Purpose |
|---------|---------|---------|
| `services/balanceCache.ts` | `useBalance`, `useBackgroundBalancePoller` | localStorage-based balance/price cache shared across CEF subprocesses |
| `types/identity.ts` | `useHodosBrowser` | `IdentityResult` type |
| `types/address.ts` | `useHodosBrowser`, `useAddress` | `AddressData` type |

## Related

- [Frontend CLAUDE.md](../../../frontend/CLAUDE.md) — Frontend layer overview, entry points, invariants
- [Root CLAUDE.md](../../../CLAUDE.md) — Full architecture, CEF input patterns, overlay lifecycle
- [Wallet Components CLAUDE.md](../components/wallet/CLAUDE.md) — Wallet UI components consuming these hooks
- [Settings Components CLAUDE.md](../components/settings/CLAUDE.md) — Settings UI consuming `useSettings`
