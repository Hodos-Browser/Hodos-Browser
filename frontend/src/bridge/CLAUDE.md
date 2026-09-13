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

⛔ Phase 8c is replacing this whole pattern with native, per-request-id promise functions on `window.hodosBrowser.bridge` (C++ holds the promise; no `window.on*` global at all). **Nothing in this file uses `window.on*` any more** as of batch 4 (2026-09-13): `wallet.*`, `address.generate`, every cookie / cache / cookie-blocking call, and `bookmarks.*` all go through the bridge. The remaining legacy slots live in hooks (`useAdblock`, `usePrivacyShield`, `usePaidCache`, `useImport`, `useProfiles`, `useSettings`, `useSitePermissions`) and `TabListOverlayRoot`. Do not add new `window.on*` slots.

## API Namespaces in `initWindowBridge.ts`

| Namespace | IPC Messages | Description |
|-----------|-------------|-------------|
| `navigation` | `navigate` | URL navigation from React to CEF |
| `overlay` | `overlay_show_settings`, `overlay_show_brc100_auth`, `overlay_close`, `overlay_hide`, `overlay_input` | Overlay lifecycle control (show/hide/close/input toggle) |
| `address` | `address_generate` | BSV address generation |
| `wallet` | `wallet_status_check`, `get_balance`, `send_transaction` — all three route by request id through `hodosBrowser.bridge` (Phase 8c), not through `window.on*` globals. `create_wallet`, `load_wallet`, `get_current_address`, `get_addresses`, `get_transaction_history` were deleted in 8c batch 2 (no reachable caller); `get_wallet_info`, `mark_wallet_backed_up`, `get/set_backup_modal_state` went with the backup overlay (8c O8) | Wallet operations that are not `walletFetch` |
| `omnibox` | `omnibox_show`, `omnibox_hide`, `omnibox_create_or_show` | Address bar overlay control |
| ~~`cookies`~~ / ~~`cookieBlocking`~~ | — | **Deleted in beta.3 Phase 8c batch 3 (2026-09-12).** They had no callers: `useCookies` / `useCookieBlocking` send their IPC themselves, and now do so through the native `hodosBrowser.bridge.cookie*` / `cache*` functions (15 of them, per-request-id) |
| `bookmarks` | `bookmark_add`, `bookmark_remove`, `bookmark_search`, `bookmark_get_all`, `bookmark_is_bookmarked` — all five route by request id through `hodosBrowser.bridge.bookmark*` (Phase 8c batch 4, 2026-09-13). `bookmark_get`, `bookmark_update`, `bookmark_get_all_tags`, `bookmark_update_last_accessed` and the five `bookmark_folder_*` IPCs were **deleted** (no caller); `BookmarkManager`'s folder/tag SQL remains | Bookmark CRUD and search used by `useBookmarks` |

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
| **Auth** | `generateChallenge()`, `authenticate()` |
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

The `wallet`, `bookmarks`, and `omnibox` namespaces use the same guard (`if (!window.hodosBrowser.xxx)`). The `address.generate` method is an exception — it force-overrides to ensure the promise-based wrapper is always present.

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
