import type { AddressData } from './address';
import type { TransactionResponse, BroadcastResponse } from './transaction';
import type { HistoryEntry, HistorySearchParams, HistoryGetParams, ClearRangeParams, HistoryEntryWithFrecency } from './history';
import type { CookieData, CookieDeleteResponse, CacheSizeResponse } from './cookies';
import type { BlockedDomainEntry, BlockLogEntry, BlockDomainResponse, UnblockDomainResponse, AllowThirdPartyResponse, BlockedCountResponse, ClearBlockLogResponse } from './cookieBlocking';
import type { BookmarkData, FolderData, BookmarkAddResponse, BookmarkUpdateResponse, BookmarkRemoveResponse, BookmarkSearchResponse, BookmarkGetAllResponse, BookmarkIsBookmarkedResponse, FolderCreateResponse, FolderUpdateResponse, FolderRemoveResponse } from './bookmarks';

declare global {
  interface Window {
    hodosBrowser: {
      history: {
        get: (params?: HistoryGetParams) => HistoryEntry[];
        search: (params: HistorySearchParams) => HistoryEntry[];
        searchWithFrecency: (params: { query: string; limit?: number }) => HistoryEntryWithFrecency[];
        delete: (url: string) => boolean;
        clearAll: () => boolean;
        clearRange: (params: ClearRangeParams) => boolean;
      };
      cookies: {
        getAll: () => Promise<CookieData[]>;
        deleteCookie: (url: string, name: string) => Promise<CookieDeleteResponse>;
        deleteDomainCookies: (domain: string) => Promise<CookieDeleteResponse>;
        deleteAllCookies: () => Promise<CookieDeleteResponse>;
        clearCache: () => Promise<{ success: boolean }>;
        getCacheSize: () => Promise<CacheSizeResponse>;
      };
      cookieBlocking: {
        blockDomain: (domain: string, isWildcard: boolean) => Promise<BlockDomainResponse>;
        unblockDomain: (domain: string) => Promise<UnblockDomainResponse>;
        getBlockList: () => Promise<BlockedDomainEntry[]>;
        allowThirdParty: (domain: string) => Promise<AllowThirdPartyResponse>;
        removeThirdPartyAllow: (domain: string) => Promise<AllowThirdPartyResponse>;
        getBlockLog: (limit: number, offset: number) => Promise<BlockLogEntry[]>;
        clearBlockLog: () => Promise<ClearBlockLogResponse>;
        getBlockedCount: () => Promise<BlockedCountResponse>;
        resetBlockedCount: () => Promise<void>;
      };
      bookmarks: {
        add: (url: string, title: string, folderId?: number, tags?: string[]) => Promise<BookmarkAddResponse>;
        get: (id: number) => Promise<BookmarkData>;
        update: (id: number, fields: { title?: string; url?: string; folderId?: number | null; tags?: string[] }) => Promise<BookmarkUpdateResponse>;
        remove: (id: number) => Promise<BookmarkRemoveResponse>;
        search: (query: string, limit?: number, offset?: number) => Promise<BookmarkSearchResponse>;
        getAll: (folderId?: number, limit?: number, offset?: number) => Promise<BookmarkGetAllResponse>;
        isBookmarked: (url: string) => Promise<BookmarkIsBookmarkedResponse>;
        getAllTags: () => Promise<string[]>;
        updateLastAccessed: (id: number) => Promise<BookmarkUpdateResponse>;
        folders: {
          create: (name: string, parentId?: number) => Promise<FolderCreateResponse>;
          list: (parentId?: number) => Promise<{ folders: FolderData[] }>;
          update: (id: number, name: string) => Promise<FolderUpdateResponse>;
          remove: (id: number) => Promise<FolderRemoveResponse>;
          getTree: () => Promise<FolderData[]>;
        };
      };
      wallet: {
        getStatus: () => Promise<{ exists: boolean; needsBackup: boolean }>;
        create: () => Promise<{ success: boolean; wallet?: { mnemonic: string; address?: string; version?: string; backedUp?: boolean }; error?: string }>;
        load: () => Promise<{ success: boolean; address: string; mnemonic: string; version: string; backedUp: boolean }>;
        getInfo: () => Promise<{ version: string; mnemonic: string; address: string; backedUp: boolean }>;
        generateAddress: () => Promise<AddressData>;
        getCurrentAddress: () => Promise<AddressData>;
        getAddresses: () => Promise<AddressData[]>;
        markBackedUp: () => Promise<{ success: boolean }>;
        getBackupModalState: () => Promise<{ shown: boolean }>;
        setBackupModalState: (shown: boolean) => Promise<{ success: boolean }>;
        getBalance: () => Promise<{ balance: number }>;
        sendTransaction: (data: { recipient: string; amount: number }) => Promise<TransactionResponse>;
        getTransactionHistory: () => Promise<any[]>;
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
      send: (channel: string, args: any[]) => void;
    };
    triggerPanel?: (panelName: string) => void;
    onAddressGenerated?: (data: AddressData) => void;
    onAddressError?: (error: string) => void;
    onSendTransactionResponse?: (data: TransactionResponse) => void;
    onSendTransactionError?: (error: string) => void;
    onGetBalanceResponse?: (data: { balance: number }) => void;
    onGetBalanceError?: (error: string) => void;
    onGetTransactionHistoryResponse?: (data: any[]) => void;
    onGetTransactionHistoryError?: (error: string) => void;
    onWalletStatusResponse?: (data: { exists: boolean; needsBackup: boolean }) => void;
    onWalletStatusError?: (error: string) => void;
    onCreateWalletResponse?: (data: { success: boolean; mnemonic: string; address: string; version: string }) => void;
    onCreateWalletError?: (error: string) => void;
    onLoadWalletResponse?: (data: { success: boolean; address: string; mnemonic: string; version: string; backedUp: boolean }) => void;
    onLoadWalletError?: (error: string) => void;
    onGetWalletInfoResponse?: (data: { version: string; mnemonic: string; address: string; backedUp: boolean }) => void;
    onGetWalletInfoError?: (error: string) => void;
    onGetCurrentAddressResponse?: (data: AddressData) => void;
    onGetCurrentAddressError?: (error: string) => void;
    onGetAddressesResponse?: (data: AddressData[]) => void;
    onGetAddressesError?: (error: string) => void;
    onMarkWalletBackedUpResponse?: (data: { success: boolean }) => void;
    onMarkWalletBackedUpError?: (error: string) => void;
    onGetBackupModalStateResponse?: (data: { shown: boolean }) => void;
    onSetBackupModalStateResponse?: (data: { success: boolean }) => void;
    onCookieGetAllResponse?: (data: CookieData[]) => void;
    onCookieGetAllError?: (error: string) => void;
    onCookieDeleteResponse?: (data: CookieDeleteResponse) => void;
    onCookieDeleteError?: (error: string) => void;
    onCookieDeleteDomainResponse?: (data: CookieDeleteResponse) => void;
    onCookieDeleteDomainError?: (error: string) => void;
    onCookieDeleteAllResponse?: (data: CookieDeleteResponse) => void;
    onCookieDeleteAllError?: (error: string) => void;
    onCacheClearResponse?: (data: { success: boolean }) => void;
    onCacheClearError?: (error: string) => void;
    onCacheGetSizeResponse?: (data: CacheSizeResponse) => void;
    onCacheGetSizeError?: (error: string) => void;
    onCookieBlockDomainResponse?: (data: BlockDomainResponse) => void;
    onCookieBlockDomainError?: (error: string) => void;
    onCookieUnblockDomainResponse?: (data: UnblockDomainResponse) => void;
    onCookieUnblockDomainError?: (error: string) => void;
    onCookieBlocklistResponse?: (data: BlockedDomainEntry[]) => void;
    onCookieBlocklistError?: (error: string) => void;
    onCookieAllowThirdPartyResponse?: (data: AllowThirdPartyResponse) => void;
    onCookieAllowThirdPartyError?: (error: string) => void;
    onCookieRemoveThirdPartyAllowResponse?: (data: AllowThirdPartyResponse) => void;
    onCookieRemoveThirdPartyAllowError?: (error: string) => void;
    onCookieBlockLogResponse?: (data: BlockLogEntry[]) => void;
    onCookieBlockLogError?: (error: string) => void;
    onCookieClearBlockLogResponse?: (data: ClearBlockLogResponse) => void;
    onCookieClearBlockLogError?: (error: string) => void;
    onCookieBlockedCountResponse?: (data: BlockedCountResponse) => void;
    onCookieBlockedCountError?: (error: string) => void;
    onCookieResetBlockedCountResponse?: () => void;
    onCookieResetBlockedCountError?: (error: string) => void;
    onBookmarkAddResponse?: (data: BookmarkAddResponse) => void;
    onBookmarkGetResponse?: (data: BookmarkData) => void;
    onBookmarkUpdateResponse?: (data: BookmarkUpdateResponse) => void;
    onBookmarkRemoveResponse?: (data: BookmarkRemoveResponse) => void;
    onBookmarkSearchResponse?: (data: BookmarkSearchResponse) => void;
    onBookmarkGetAllResponse?: (data: BookmarkGetAllResponse) => void;
    onBookmarkIsBookmarkedResponse?: (data: BookmarkIsBookmarkedResponse) => void;
    onBookmarkGetAllTagsResponse?: (data: string[]) => void;
    onBookmarkUpdateLastAccessedResponse?: (data: BookmarkUpdateResponse) => void;
    onBookmarkFolderCreateResponse?: (data: FolderCreateResponse) => void;
    onBookmarkFolderListResponse?: (data: { folders: FolderData[] }) => void;
    onBookmarkFolderUpdateResponse?: (data: FolderUpdateResponse) => void;
    onBookmarkFolderRemoveResponse?: (data: FolderRemoveResponse) => void;
    onBookmarkFolderGetTreeResponse?: (data: FolderData[]) => void;
    allSystemsReady?: boolean;
     __overlayReady?: boolean;
  }
}

export {};
