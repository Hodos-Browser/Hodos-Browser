# Startup Flow and Wallet Checks

## Purpose

This document describes the startup sequence of Hodos Browser, including wallet server initialization, wallet existence checking, and user-driven wallet creation.

**Status: ✅ COMPLETE (2026-02-13)**

**Document Version:** 2.0
**Last Updated:** 2026-02-13

---

## Table of Contents

1. [Overview](#overview)
2. [Startup Sequence](#startup-sequence)
3. [Wallet Server Startup](#wallet-server-startup)
4. [Wallet Creation Flow](#wallet-creation-flow)
5. [Frontend Wallet Check](#frontend-wallet-check)
6. [Shutdown](#shutdown)
7. [Files Modified](#files-modified)

---

## Overview

The browser always launches the Rust wallet server as a subprocess. The server starts without auto-creating a wallet — it returns `{ exists: false }` from `/wallet/status` until the user explicitly creates one via the frontend. This avoids the chicken-and-egg problem (frontend can't call the API if the server isn't running) and keeps the architecture simple.

### Key Principles

1. **Non-blocking startup**: Browser launches regardless of wallet status
2. **Always-on server**: Wallet server always starts; returns `exists: false` if no wallet
3. **User-driven creation**: Wallet creation only happens via `POST /wallet/create`
4. **Auto-cleanup**: Job Object ensures wallet server dies when browser exits

### Wallet States

| State | Description |
|-------|-------------|
| No Wallet | DB file exists (SQLite schema created) but no wallet record |
| Wallet Exists | DB has wallet record with keys, mnemonic, addresses |
| Server Running | `hodos-wallet.exe` is running on port 3301 |

---

## Startup Sequence

### C++ Entry Point (`cef_browser_shell.cpp` → `WinMain`)

1. Initialize Logger
2. Set up CEF settings (cache paths, subprocess paths)
3. Create windows (shell, header, webview)
4. `CefInitialize()`
5. Initialize HistoryManager, CookieBlockManager, BookmarkManager
6. **`StartWalletServer()`** — launch or detect wallet server
7. `SetWindowHandles()`, `CefRunMessageLoop()`

### Wallet Server Startup (`StartWalletServer()`)

1. **Dev mode detection**: `QuickHealthCheck()` probes `GET /health` with a 2-second WinHTTP timeout. If healthy, server is already running (developer ran `cargo run` separately) — skip launch.
2. **Exe resolution**: Resolves `hodos-wallet.exe` path relative to the browser executable (`..\..\..\..\rust-wallet\target\release\hodos-wallet.exe`). If not found, logs warning and continues without wallet.
3. **Process launch**: `CreateProcessA()` with `CREATE_NO_WINDOW`.
4. **Job Object**: Creates a Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and assigns the wallet process. This guarantees the child is killed when the browser exits for any reason (normal close, crash, Task Manager kill).
5. **Health polling**: Polls `QuickHealthCheck()` up to 10 times at 500ms intervals (5 seconds total). Logs success or warning.

### Rust Server Startup (`rust-wallet/src/main.rs`)

1. Creates wallet directory and SQLite database (schema migrations run)
2. Checks for existing wallet via `WalletRepository::get_primary_wallet()`
3. If **wallet exists**: runs maintenance (master address, default basket, stale tx cleanup, placeholder restoration), seeds balance cache, starts Monitor
4. If **no wallet**: logs "server ready for user-initiated creation", skips all maintenance and Monitor
5. Starts Actix-web HTTP server on port 3301

---

## Wallet Creation Flow

### Endpoint: `POST /wallet/create` (`rust-wallet/src/handlers.rs`)

1. Checks if wallet already exists → 409 Conflict if so
2. Calls `db.create_wallet_with_first_address()` (creates wallet, keys, user, basket, address)
3. Returns `{ success: true, mnemonic: "...", address: "...", walletId: N }`

### Frontend: `WalletPanelPage.tsx`

On mount:
1. Checks `localStorage('hodos_wallet_exists')` — if set, skips status fetch and shows WalletPanel immediately (no spinner on subsequent opens)
2. Otherwise, fetches `GET /wallet/status` — if `exists: true`, sets localStorage flag and shows WalletPanel; if `exists: false`, shows NoWallet prompt

NoWallet prompt:
1. "Create New Wallet" button → `POST /wallet/create`
2. Shows 12-word mnemonic with numbered words, copy button
3. "I have backed up my mnemonic" checkbox gates "Continue to Wallet" button
4. On confirm: sets localStorage flag, transitions to WalletPanel
5. "Recover Wallet (Coming Soon)" — disabled, placeholder for Phase 1

---

## Frontend Wallet Check

The wallet panel uses `localStorage` to avoid showing the loading spinner on every open:

| Scenario | Behavior |
|----------|----------|
| First open, no wallet | Spinner → fetch status → "No Wallet Found" prompt |
| After wallet creation | localStorage flag set → instant WalletPanel |
| Subsequent opens | localStorage flag cached → skip fetch, instant WalletPanel |

---

## Shutdown

### Normal Close (X button)

1. `WM_CLOSE` → `ShutdownApplication()` → `StopWalletServer()` (explicit `TerminateProcess`)
2. After `CefRunMessageLoop()` returns → `StopWalletServer()` again (belt-and-suspenders)

### Abnormal Exit (crash, Task Manager kill)

The Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` ensures `hodos-wallet.exe` is terminated automatically when the browser process handle closes — regardless of how the browser exits.

---

## Files Modified

| File | Changes |
|------|---------|
| `rust-wallet/src/main.rs` | Disabled auto-creation; `wallet_exists` guards maintenance, balance cache, Monitor; registered `/wallet/create` route |
| `rust-wallet/src/handlers.rs` | Added `wallet_create` handler (POST, returns mnemonic, 409 if exists) |
| `cef-native/cef_browser_shell.cpp` | Added `QuickHealthCheck()`, `StartWalletServer()`, `StopWalletServer()` with Job Object; forward declarations; calls in WinMain and ShutdownApplication |
| `cef-native/src/core/WalletService.cpp` | Fixed health check (`"ok"` not `"healthy"`); updated daemon path to `rust-wallet` |
| `frontend/src/pages/WalletPanelPage.tsx` | Added status check, NoWallet prompt, mnemonic backup flow, localStorage caching |

---

## Integration with Phase 1

Phase 1 (Initial Setup & Recovery) will add:
- Mnemonic recovery flow ("Recover Wallet" button)
- Local file backup/restore
- The NoWallet prompt's disabled "Recover Wallet" button will be enabled

---

**End of Document**
