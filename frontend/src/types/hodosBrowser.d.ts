import type { AddressData } from './address';
import type { IdentityResult } from './identity';
import type { TransactionResponse, BroadcastResponse } from './transaction';
import type { HistoryEntry, HistorySearchParams, HistoryGetParams, ClearRangeParams, HistoryEntryWithFrecency } from './history';
import type { CookieData, CookieDeleteResponse, CacheSizeResponse } from './cookies';
import type { BlockedDomainEntry, BlockLogEntry, BlockDomainResponse, UnblockDomainResponse, AllowThirdPartyResponse, BlockedCountResponse, ClearBlockLogResponse } from './cookieBlocking';
import type { BookmarkAddResponse, BookmarkRemoveResponse, BookmarkSearchResponse, BookmarkGetAllResponse, BookmarkIsBookmarkedResponse } from './bookmarks';

declare global {
  interface Window {
    hodosBrowser: {
      // Phase 8c — the per-request-id wallet bridge. Native promise-returning
      // functions bound in C++ (`WalletBridgeV8Handler`), one entry per migrated
      // method. ⚠️ Deliberately NOT merged into `wallet` below: C++ binds these
      // before the page's JS runs, and `initWindowBridge.ts` guards its whole wallet
      // block with `if (!window.hodosBrowser.wallet)`.
      bridge?: {
        getStatus: () => Promise<{ exists: boolean; needsBackup: boolean }>;
        // ⛔ Takes a JSON STRING, not an object — the caller stringifies, matching what
        // the legacy path already put on the wire.
        sendTransaction: (payloadJson: string) => Promise<TransactionResponse>;
        getBalance: () => Promise<{ balance: number; bsvPrice?: number }>;
        // Stage 3 batch 2. Backs `address.generate()`.
        generateAddress: () => Promise<AddressData>;
        // (getInfo / markBackedUp / getBackupModalState / setBackupModalState were deleted
        // with the backup overlay, Phase 8c O8.)
        // Stage 3 batch 3 — cookies + cookie blocking, called by `useCookies` and
        // `useCookieBlocking`. ⛔ Every argument is a STRING, as the legacy IPC already
        // sent it (booleans and numbers stringified); the browser handlers parse them.
        cookieGetAll: () => Promise<CookieData[]>;
        cookieDelete: (url: string, name: string) => Promise<CookieDeleteResponse>;
        cookieDeleteDomain: (domain: string) => Promise<CookieDeleteResponse>;
        cookieDeleteAll: () => Promise<CookieDeleteResponse>;
        cacheClear: () => Promise<{ success: boolean }>;
        cacheGetSize: () => Promise<CacheSizeResponse>;
        cookieBlockDomain: (domain: string, isWildcard: string) => Promise<BlockDomainResponse>;
        cookieUnblockDomain: (domain: string) => Promise<UnblockDomainResponse>;
        cookieGetBlocklist: () => Promise<BlockedDomainEntry[]>;
        cookieAllowThirdParty: (domain: string) => Promise<AllowThirdPartyResponse>;
        cookieRemoveThirdPartyAllow: (domain: string) => Promise<AllowThirdPartyResponse>;
        cookieGetBlockLog: (limit: string, offset: string) => Promise<BlockLogEntry[]>;
        cookieClearBlockLog: () => Promise<ClearBlockLogResponse>;
        cookieGetBlockedCount: () => Promise<BlockedCountResponse>;
        cookieResetBlockedCount: () => Promise<{ success: boolean }>;
        // Stage 3 batch 4 — bookmarks. Strings only, as the legacy IPC sent them.
        bookmarkAdd: (url: string, title: string, folderId: string, tagsJson: string) => Promise<BookmarkAddResponse>;
        bookmarkRemove: (id: string) => Promise<BookmarkRemoveResponse>;
        bookmarkSearch: (query: string, limit: string, offset: string) => Promise<BookmarkSearchResponse>;
        bookmarkGetAll: (folderId: string, limit: string, offset: string) => Promise<BookmarkGetAllResponse>;
        bookmarkIsBookmarked: (url: string) => Promise<BookmarkIsBookmarkedResponse>;
        // Stage 3 batch 5 — adblock + privacy shield (used by useAdblock / usePrivacyShield).
        // Booleans cross as "true" / "false" strings, as the legacy IPC sent them.
        adblockGetBlockedCount: () => Promise<{ count: number }>;
        adblockResetBlockedCount: () => Promise<{ success: boolean }>;
        adblockSiteToggle: (domain: string, enabled: string) => Promise<{ domain: string; adblockEnabled: boolean; success: boolean }>;
        adblockScriptletToggle: (domain: string, enabled: string) => Promise<{ domain: string; scriptletsEnabled: boolean; success: boolean }>;
        adblockCheckSiteEnabled: (domain: string) => Promise<{ domain: string; adblockEnabled: boolean }>;
        adblockCheckScriptletsEnabled: (domain: string) => Promise<{ domain: string; scriptletsEnabled: boolean }>;
        cookieCheckSiteAllowed: (domain: string) => Promise<{ domain: string; allowed: boolean }>;
        fingerprintGetSiteEnabled: (domain: string) => Promise<{ domain: string; enabled: boolean }>;
      };
      // Promise-based since the history-over-IPC move: the render process no longer
      // opens the history database itself, so every call is a round-trip to the
      // browser process. Matches the shape `cookies` below has always had.
      history: {
        get: (params?: HistoryGetParams) => Promise<HistoryEntry[]>;
        search: (params: HistorySearchParams) => Promise<HistoryEntry[]>;
        searchWithFrecency: (params: { query: string; limit?: number }) => Promise<HistoryEntryWithFrecency[]>;
        delete: (url: string) => Promise<boolean>;
        clearAll: () => Promise<boolean>;
        clearRange: (params: ClearRangeParams) => Promise<boolean>;
      };
      // ⛔ The `cookies` and `cookieBlocking` namespaces were DELETED in Phase 8c stage 3
      // batch 3: they had no callers (the hooks send IPC themselves) and duplicated the
      // hooks' single-slot globals. The hooks now use `bridge.cookie*` above.
      // Phase 8c batch 4: the five live methods, each a one-line wrapper over
      // `bridge.bookmark*`. `get`, `update`, `getAllTags`, `updateLastAccessed` and
      // `folders.*` were DELETED (no caller).
      bookmarks: {
        add: (url: string, title: string, folderId?: number, tags?: string[]) => Promise<BookmarkAddResponse>;
        remove: (id: number) => Promise<BookmarkRemoveResponse>;
        search: (query: string, limit?: number, offset?: number) => Promise<BookmarkSearchResponse>;
        getAll: (folderId?: number, limit?: number, offset?: number) => Promise<BookmarkGetAllResponse>;
        isBookmarked: (url: string) => Promise<BookmarkIsBookmarkedResponse>;
      };
      identity: {
        get: () => Promise<IdentityResult>;
        markBackedUp: () => Promise<string>;
      };
      wallet: {
        getStatus: () => Promise<{ exists: boolean; needsBackup: boolean }>;
        // ⛔ `create`, `load`, `generateAddress`, `getCurrentAddress`, `getAddresses` and
        // `getTransactionHistory` were DELETED in Phase 8c stage 3 batch 2 (no reachable
        // caller; C++ round trips removed). Address generation is `address.generate`.
        // ⛔ `getInfo`, `markBackedUp`, `getBackupModalState`, `setBackupModalState` were
        // DELETED with the backup overlay (Phase 8c O8, 2026-09-12).
        getBalance: () => Promise<{ balance: number; bsvPrice?: number }>;
        sendTransaction: (data: { recipient: string; amount: number }) => Promise<TransactionResponse>;
      };
      address: {
        generate: () => Promise<AddressData>;
      };
      navigation: {
        navigate: (path: string) => void;
      };
      overlay: {
        show: () => void;
        hide: () => void;
        toggleInput: (enable: boolean) => void;
        close: () => void;
      };
      overlayPanel: {
        open: (panelName: string) => void;
        toggleInput: (enable: boolean) => void;
      };
      omnibox: {
        show: (query: string) => void;
        hide: () => void;
        createOrShow: () => void;
        getSuggestions: (query: string) => Promise<any[]>;
      };
      googleSuggest: {
        fetch: (query: string) => number;
      };
    };
    cefMessage?: {
      // Variadic: the C++ CefMessageSendHandler requires the message name and accepts
      // any number of additional args of any type (string, number, array, object, or none).
      send: (channel: string, ...args: any[]) => void;
    };
    triggerPanel?: (panelName: string) => void;
    // ⛔ REMOVED by Phase 8c stage 3 batch 2 — address.generate / wallet.generateAddress
    // are routed by request id. They used to SHARE this one slot pair.
    //   onAddressGenerated / onAddressError
    // ⛔ REMOVED by Phase 8c stage 2 — wallet.sendTransaction is routed by request id.
    //   onSendTransactionResponse / onSendTransactionError
    // ⛔ REMOVED by Phase 8c stage 3 — getBalance is routed by request id.
    //   onGetBalanceResponse / onGetBalanceError
    // ⛔ REMOVED by Phase 8c stage 3 batch 2 — the methods were DELETED, not migrated
    // (no reachable caller):
    //   onGetTransactionHistoryResponse / onGetTransactionHistoryError
    //   onCreateWalletResponse / onCreateWalletError
    //   onLoadWalletResponse / onLoadWalletError
    //   onGetCurrentAddressResponse / onGetCurrentAddressError
    //   onGetAddressesResponse / onGetAddressesError
    // ⛔ REMOVED by Phase 8c stage 1 — `wallet.getStatus` no longer uses a global
    // callback slot. Re-declaring these would invite a caller to reintroduce the race.
    //   onWalletStatusResponse / onWalletStatusError
    // ⛔ REMOVED — the four methods behind these were deleted with the backup overlay
    // (Phase 8c O8): onGetWalletInfoResponse / onGetWalletInfoError,
    // onMarkWalletBackedUpResponse / onMarkWalletBackedUpError,
    // onGetBackupModalStateResponse / onSetBackupModalStateResponse.
    // ⛔ REMOVED by Phase 8c stage 3 batch 3 — the 15 cookie / cache / cookie-blocking
    // calls are routed by request id through `hodosBrowser.bridge`. The `*Error`
    // variants had never been emitted by C++ at all. Re-declaring any of these would
    // invite a hook to reintroduce the single-slot race:
    //   onCookieGetAllResponse/Error, onCookieDeleteResponse/Error,
    //   onCookieDeleteDomainResponse/Error, onCookieDeleteAllResponse/Error,
    //   onCacheClearResponse/Error, onCacheGetSizeResponse/Error,
    //   onCookieBlockDomainResponse/Error, onCookieUnblockDomainResponse/Error,
    //   onCookieBlocklistResponse/Error, onCookieAllowThirdPartyResponse/Error,
    //   onCookieRemoveThirdPartyAllowResponse/Error, onCookieBlockLogResponse/Error,
    //   onCookieClearBlockLogResponse/Error, onCookieBlockedCountResponse/Error,
    //   onCookieResetBlockedCountResponse/Error
    // ⛔ REMOVED by Phase 8c stage 3 batch 4 — the five live bookmark calls are routed by
    // request id through `hodosBrowser.bridge`; the other nine were deleted outright.
    //   onBookmarkAddResponse, onBookmarkGetResponse, onBookmarkUpdateResponse,
    //   onBookmarkRemoveResponse, onBookmarkSearchResponse, onBookmarkGetAllResponse,
    //   onBookmarkIsBookmarkedResponse, onBookmarkGetAllTagsResponse,
    //   onBookmarkUpdateLastAccessedResponse, onBookmarkFolder{Create,List,Update,Remove,GetTree}Response
    allSystemsReady?: boolean;
     __overlayReady?: boolean;
  }
}

export {};
