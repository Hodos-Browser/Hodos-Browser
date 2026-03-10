# frontend/src/types
> TypeScript type definitions for the Hodos Browser frontend: window API declarations, browser data models, wallet/transaction types, and UI state interfaces.

## Overview

This directory contains all shared TypeScript type definitions used across the frontend. Types fall into two categories:

1. **Window API declarations** (`hodosBrowser.d.ts`) — augments the global `Window` interface with CEF-injected APIs and IPC callbacks
2. **Data models** (all other files) — interfaces for browser data (tabs, history, bookmarks, cookies) and wallet data (addresses, transactions, identity)

All types are consumed by hooks, components, and pages. No runtime code lives here except `omnibox.ts` and `TabTypes.ts` (which use `.ts` instead of `.d.ts` because they export values or are imported directly by non-declaration-aware code).

## Files

| File | Types Exported | Domain |
|------|---------------|--------|
| `hodosBrowser.d.ts` | Global `Window` augmentation | CEF bridge API surface |
| `TabTypes.ts` | `Tab`, `TabListResponse`, `TabManagerState` | Tab management |
| `transaction.d.ts` | `TransactionData`, `Transaction`, `TransactionResponse`, `BroadcastResponse`, `BalanceData` | Wallet transactions |
| `bookmarks.d.ts` | `BookmarkData`, `FolderData`, 7 response types | Bookmark system |
| `history.d.ts` | `HistoryEntry`, `HistoryEntryWithFrecency`, `HistorySearchParams`, `HistoryGetParams`, `ClearRangeParams` | Browsing history |
| `cookies.d.ts` | `CookieData`, `DomainCookieGroup`, `CookieDeleteResponse`, `CacheSizeResponse` | Cookie management |
| `cookieBlocking.d.ts` | `BlockedDomainEntry`, `BlockLogEntry`, 5 response types | Cookie blocking/privacy |
| `identity.d.ts` | `IdentityData`, `BackupCheck`, `IdentityResult` | Wallet identity |
| `address.d.ts` | `AddressData` | BSV addresses |
| `omnibox.ts` | `Suggestion`, re-exports `HistoryEntryWithFrecency` | Omnibox autocomplete |

## Core Types

### Window API (`hodosBrowser.d.ts`)

The central type file. Augments `window` with the full `hodosBrowser` API surface injected by C++ V8 bindings:

```typescript
window.hodosBrowser.history    // Synchronous history queries
window.hodosBrowser.cookies    // Async cookie CRUD
window.hodosBrowser.cookieBlocking  // Domain block/allow management
window.hodosBrowser.bookmarks  // Bookmark + folder CRUD with tags
window.hodosBrowser.wallet     // Wallet status, create, load, send, balance
window.hodosBrowser.address    // Address generation
window.hodosBrowser.navigation // URL navigation
window.hodosBrowser.overlay    // Overlay show/hide/close
window.hodosBrowser.overlayPanel  // Named panel open
window.hodosBrowser.omnibox    // Omnibox show/hide/suggestions
window.hodosBrowser.googleSuggest // Google search suggestions
window.cefMessage.send()       // Raw IPC to C++ shell
```

Also declares ~50 optional `window.onXxxResponse` / `window.onXxxError` callback properties used by the V8 bridge for async IPC responses (wallet operations, cookie operations, bookmark operations).

**Used by:** Every hook and bridge file that calls CEF APIs.

### Tab (`TabTypes.ts`)

```typescript
interface Tab {
  id: number;
  title: string;
  url: string;
  isActive: boolean;
  isLoading: boolean;
  favicon?: string;
  hasCertError?: boolean;
}
```
- **Purpose:** Represents a browser tab, synced with C++ `TabManager` backend
- **Used by:** `TabBar`, `TabComponent`, `useTabManager`

### Transaction Types (`transaction.d.ts`)

```typescript
interface TransactionData {        // Send form input
  recipient: string;
  amount: string;
  feeRate: string;
  memo?: string;
  sendMax?: boolean;
}

interface Transaction {            // Stored transaction record
  txid: string;
  status: 'pending' | 'confirmed' | 'failed';
  amount: number;
  recipient: string;
  timestamp: number;
  confirmations: number;
  fee: number;
  memo?: string;
}

interface TransactionResponse {    // Send result from Rust backend
  txid: string;
  rawTx?: string;
  fee?: number;
  success?: boolean;
  error?: string;
  whatsOnChainUrl?: string;
}

interface BalanceData {             // Balance display state
  balance: number;
  usdValue: number;
  isLoading: boolean;
  isRefreshing?: boolean;
}
```
- **Used by:** `TransactionForm`, `TransactionHistory`, `WalletPanel`, `BalanceDisplay`, `useTransaction`, `SendPage`, `DashboardTab`

### History Types (`history.d.ts`)

```typescript
interface HistoryEntry {
  url: string;
  title: string;
  visitCount: number;
  visitTime: number;      // Chromium timestamp (microseconds since 1601-01-01)
  transition: number;
}

interface HistoryEntryWithFrecency {  // Ranked by frecency for omnibox
  url: string;
  title: string;
  visitCount: number;
  lastVisitTime: number;
  frecencyScore: number;
}
```
- **Timestamp format:** Chromium timestamps (microseconds since Jan 1, 1601), not Unix timestamps
- **Used by:** `useHistory`, `useOmniboxSuggestions`, `suggestionRanker`

### Cookie Types (`cookies.d.ts`)

```typescript
interface CookieData {
  name: string;
  value: string;
  domain: string;
  path: string;
  secure: boolean;
  httponly: boolean;
  sameSite: number;    // 0=unspecified, 1=no_restriction, 2=lax, 3=strict
  hasExpires: boolean;
  expires?: number;     // Unix timestamp ms
  size: number;         // name.length + value.length
}

interface DomainCookieGroup {      // UI grouping for cookie panel
  domain: string;
  cookies: CookieData[];
  totalSize: number;
  count: number;
}
```
- **Used by:** `useCookies`, `CookiesPanel`

### Bookmark Types (`bookmarks.d.ts`)

```typescript
interface BookmarkData {
  id: number;
  url: string;
  title: string;
  folder_id: number | null;   // null = root level
  favicon_url: string;
  position: number;
  created_at: number;          // Unix timestamp ms
  updated_at: number;
  last_accessed: number;
  tags: string[];
}

interface FolderData {
  id: number;
  name: string;
  parent_id: number | null;   // null = root level
  position: number;
  created_at: number;
  updated_at: number;
  children?: FolderData[];    // Present in tree responses
}
```
- **Folder tree:** `FolderData.children` is recursive — `getTree()` returns nested folder hierarchy
- **Used by:** `hodosBrowser.d.ts` (window API), bookmark components

### Identity (`identity.d.ts`)

```typescript
type IdentityData = {
  publicKey: string;
  privateKey: string;
  address: string;
  backedUp: boolean;
};

type BackupCheck = { backedUp: true };
type IdentityResult = IdentityData | BackupCheck;
```
- **Discriminated union:** Check `'privateKey' in result` to distinguish `IdentityData` from `BackupCheck`
- **Used by:** `useHodosBrowser`, `useBitcoinBrowser`

### Address (`address.d.ts`)

```typescript
type AddressData = {
  address: string;
  publicKey: string;
  privateKey: string;
  index: number;
};
```
- **Used by:** `useHodosBrowser`, `useBitcoinBrowser`, `AddressManager`, `hodosBrowser.d.ts`

### Omnibox (`omnibox.ts`)

```typescript
interface Suggestion {
  url: string;
  title: string;
  type: 'history' | 'google';
  score: number;
}
```
- **Re-exports:** `HistoryEntryWithFrecency` from `./history` for convenience
- **Used by:** `OmniboxOverlayRoot`, `useOmniboxSuggestions`, `suggestionRanker`

## Type Categories

| Category | Files | Backing System |
|----------|-------|----------------|
| **Browser data** | `TabTypes.ts`, `history.d.ts`, `bookmarks.d.ts`, `cookies.d.ts`, `cookieBlocking.d.ts`, `omnibox.ts` | C++ CEF shell (SQLite storage in `%APPDATA%/HodosBrowser/Default/`) |
| **Wallet data** | `address.d.ts`, `identity.d.ts`, `transaction.d.ts` | Rust wallet backend (port 31301) |
| **Bridge/API** | `hodosBrowser.d.ts` | V8 injection layer |

## Timestamp Conventions

Two different timestamp formats are used across types:

| Format | Value | Used In |
|--------|-------|---------|
| **Chromium timestamp** | Microseconds since 1601-01-01 | `HistoryEntry.visitTime`, `HistorySearchParams`, `ClearRangeParams` |
| **Unix timestamp (ms)** | Milliseconds since 1970-01-01 | `BookmarkData`, `CookieData.expires`, `BlockedDomainEntry`, `BlockLogEntry` |

## Response Pattern

Most C++ IPC operations follow a consistent response pattern:

```typescript
interface XxxResponse {
  success: boolean;
  error?: string;
  // ...operation-specific fields
}
```

The `hodosBrowser.d.ts` file pairs each response type with `window.onXxxResponse` and `window.onXxxError` callbacks for async IPC.

## Related

- [../components/CLAUDE.md](../components/CLAUDE.md) — Components that consume these types
- [../hooks/CLAUDE.md](../hooks/CLAUDE.md) — Hooks that use these types for state management
- [../../CLAUDE.md](../../CLAUDE.md) — Frontend layer overview
- Root [CLAUDE.md](../../../CLAUDE.md) — Full architecture and `window.hodosBrowser` API docs
