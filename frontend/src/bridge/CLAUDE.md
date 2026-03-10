# frontend/src/bridge
> JavaScript-to-native IPC bridge: defines `window.hodosBrowser.*` APIs that React uses to communicate with the C++ CEF shell and Rust wallet.

## Overview

This module provides the `window.hodosBrowser` API surface that all React components and hooks use to interact with native functionality. It bridges the gap between the React frontend and the C++ CEF shell using `window.cefMessage.send()` for outbound messages and `window.onXxxResponse` / `window.onXxxError` global callbacks for responses.

There are two files with distinct roles: `initWindowBridge.ts` populates `window.hodosBrowser` with imperative IPC wrappers (navigation, overlay, wallet, cookies, bookmarks), while `brc100.ts` provides a typed singleton class for BRC-100 protocol operations that delegates to `window.hodosBrowser.brc100` methods injected by V8 on the C++ side.

## Files

| File | Lines | Purpose |
|------|-------|---------|
| `initWindowBridge.ts` | 1020 | Populates `window.hodosBrowser` with IPC wrappers for navigation, overlay control, wallet, cookies, cookie blocking, omnibox, and bookmarks |
| `brc100.ts` | 479 | `BRC100Bridge` singleton class + TypeScript interfaces for BRC-100 identity, auth, BEEF transactions, and SPV verification |

## IPC Pattern

All bridge methods follow the same request/response pattern over `cefMessage`:

1. **Outbound**: Call `window.cefMessage.send(messageName, [args])` to send a message to C++
2. **Response**: Register a one-shot `window.onXxxResponse` callback before sending
3. **Error**: Register a one-shot `window.onXxxError` callback for failures
4. **Cleanup**: Both callbacks delete themselves after firing

```ts
// Pattern used throughout initWindowBridge.ts
window.onFooResponse = (data: any) => {
  resolve(data);
  delete window.onFooResponse;
  delete window.onFooError;
};
window.onFooError = (error: string) => {
  reject(new Error(error));
  delete window.onFooResponse;
  delete window.onFooError;
};
window.cefMessage?.send('foo_action', [args]);
```

Newer APIs (cookies, cookie blocking, bookmarks) add a 5-second timeout that auto-resolves or auto-rejects if no response arrives.

## API Namespaces in `initWindowBridge.ts`

| Namespace | IPC Messages | Description |
|-----------|-------------|-------------|
| `navigation` | `navigate` | URL navigation from React to CEF |
| `overlay` | `overlay_show_settings`, `overlay_show_brc100_auth`, `overlay_close`, `overlay_hide`, `overlay_input` | Overlay lifecycle control (show/hide/close/input toggle) |
| `address` | `address_generate` | BSV address generation |
| `wallet` | `wallet_status_check`, `create_wallet`, `load_wallet`, `get_wallet_info`, `address_generate`, `get_current_address`, `get_addresses`, `mark_wallet_backed_up`, `get_backup_modal_state`, `set_backup_modal_state`, `get_balance`, `send_transaction`, `get_transaction_history` | Full wallet operations |
| `omnibox` | `omnibox_show`, `omnibox_hide`, `omnibox_create_or_show` | Address bar overlay control |
| `cookies` | `cookie_get_all`, `cookie_delete`, `cookie_delete_domain`, `cookie_delete_all`, `cache_clear`, `cache_get_size` | Cookie and cache management |
| `cookieBlocking` | `cookie_block_domain`, `cookie_unblock_domain`, `cookie_get_blocklist`, `cookie_allow_third_party`, `cookie_remove_third_party_allow`, `cookie_get_block_log`, `cookie_clear_block_log`, `cookie_get_blocked_count`, `cookie_reset_blocked_count` | Cookie blocking rules and analytics |
| `bookmarks` | `bookmark_add`, `bookmark_get`, `bookmark_update`, `bookmark_remove`, `bookmark_search`, `bookmark_get_all`, `bookmark_is_bookmarked`, `bookmark_get_all_tags`, `bookmark_update_last_accessed` | Bookmark CRUD and search |
| `bookmarks.folders` | `bookmark_folder_create`, `bookmark_folder_list`, `bookmark_folder_update`, `bookmark_folder_remove`, `bookmark_folder_get_tree` | Bookmark folder management |

## BRC-100 Bridge (`brc100.ts`)

### Singleton Access

```ts
import { brc100 } from './bridge/brc100';
// or
const bridge = BRC100Bridge.getInstance();
```

### Key Interfaces

| Interface | Purpose |
|-----------|---------|
| `BRC100Status` | Availability check response (`available`, `version`, `features`) |
| `IdentityData` | BRC-52 identity certificate fields (`issuer`, `subject`, `publicKey`, `certificate`) |
| `AuthChallengeRequest` / `AuthChallenge` | Challenge-response auth initiation |
| `AuthRequest` / `AuthResponse` | Authentication completion with session creation |
| `SessionData` | Active session with permissions |
| `BEEFTransaction` / `BEEFAction` | BEEF format transaction with actions and optional SPV data |
| `SPVData` / `MerkleProof` / `BlockHeader` | SPV proof structures for transaction verification |
| `TransactionData` / `InputData` / `OutputData` | Raw transaction components |
| `IdentityProof` | Identity verification with merkle proof link |
| `SPVVerificationRequest` / `SPVVerificationResponse` | SPV verification request/result |

### Method Groups

| Category | Methods |
|----------|---------|
| **Status** | `status()`, `isAvailable()` |
| **Identity** | `generateIdentity()`, `validateIdentity()`, `selectiveDisclosure()` |
| **Auth** | `generateChallenge()`, `authenticate()`, `deriveType42Keys()` |
| **Sessions** | `createSession()`, `validateSession()`, `revokeSession()` |
| **BEEF** | `createBEEF()`, `verifyBEEF()`, `broadcastBEEF()` |
| **SPV** | `verifySPV()`, `createSPVProof()` |
| **Workflows** | `requestAuthentication()` (challenge + approval + auth), `createAndBroadcastBEEFTransaction()` (create + approval + broadcast) |

The `BRC100Bridge` delegates to `window.hodosBrowser.brc100[methodName]()` — these methods are injected by V8 in the C++ render process, not defined in this module. The bridge class provides TypeScript typing and workflow orchestration on top of the native methods.

## Import Locations

| Consumer | Import | Purpose |
|----------|--------|---------|
| `src/main.tsx` | `import './bridge/initWindowBridge'` | Side-effect import — populates `window.hodosBrowser` on app boot |
| `src/App.tsx` | `import { brc100 } from './bridge/brc100'` | BRC-100 singleton for auth modal integration |

## Guard Pattern (Don't Override V8)

`initWindowBridge.ts` carefully avoids overwriting methods that C++ V8 injection may have already defined:

```ts
if (!window.hodosBrowser.overlay?.show) {
  // Only define if V8 didn't inject it
  window.hodosBrowser.overlay.show = () => { ... };
}
```

The `wallet`, `cookies`, `cookieBlocking`, `bookmarks`, and `omnibox` namespaces use the same guard (`if (!window.hodosBrowser.xxx)`). The `address.generate` method is an exception — it force-overrides to ensure the promise-based wrapper is always present.

## macOS Compatibility

`initWindowBridge.ts` includes defensive stubs for macOS where some C++ APIs may not be available yet:

- `address` — creates empty stub object if not present
- `overlay.toggleInput` — no-op stub with console warning
- `overlay.hide` — no-op stub with console warning

## Related

- [../components/CLAUDE.md](../components/CLAUDE.md) — React components that consume these APIs
- [../hooks/CLAUDE.md](../hooks/CLAUDE.md) — React hooks (especially `useHodosBrowser`) that wrap bridge calls
- [../pages/CLAUDE.md](../pages/CLAUDE.md) — Overlay pages that use bridge for lifecycle control
- Root `CLAUDE.md` — Architecture overview, overlay lifecycle, CEF input patterns
