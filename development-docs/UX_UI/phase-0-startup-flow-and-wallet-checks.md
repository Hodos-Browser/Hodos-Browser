# Startup Flow and Wallet Checks

## Purpose

This document outlines the **planned** startup sequence of Hodos Browser, including wallet file checks, wallet server initialization, and how the browser handles wallet existence. This serves as the definitive reference for implementing the new startup logic and wallet initialization.

**⚠️ IMPORTANT: Implementation Status**

**Current State** (as of 2025-01-27):
- ❌ C++ browser does NOT check wallet file on startup
- ❌ C++ browser does NOT start wallet server on startup
- ✅ Rust wallet server auto-creates wallet if none exists (needs to be changed)
- ✅ Frontend wallet check on startup is COMMENTED OUT (as intended)

**Planned Implementation** (not yet implemented):

> **DECISION (2026-02-11): Option A — Always Start Server**
>
> The C++ browser will **always** start the Rust wallet server on launch, regardless of
> whether a wallet DB exists. The server returns `{ exists: false }` from `/wallet/status`
> when no wallet is present. This avoids the chicken-and-egg problem (frontend can't call
> the API if the server isn't running) and is the simplest architecture.
>
> The C++ file-existence check and SQLite validation are **removed** from the startup path.
> The Rust server handles all wallet state detection internally.

- ✅ C++ browser will **always** start the Rust wallet server on startup
- ✅ Rust wallet server will NOT auto-create wallet (user-initiated only)
- ✅ Server returns `{ exists: false }` when no wallet DB exists
- ✅ Frontend will check wallet when user clicks Wallet button
- ✅ Phase 2: If an HTTP request to wallet is intercepted but no wallet exists, the create/recover modal can also be triggered

**Document Version:** 1.1
**Last Updated:** 2026-02-11
**Target Audience:** Developers implementing startup logic and wallet initialization

---

## Table of Contents

1. [Overview](#overview)
2. [Startup Sequence](#startup-sequence)
3. [Wallet File Check](#wallet-file-check)
4. [Wallet Server Startup](#wallet-server-startup)
5. [Browser Initialization](#browser-initialization)
6. [Frontend Startup](#frontend-startup)
7. [Flow Diagrams](#flow-diagrams)
8. [Implementation Details](#implementation-details)
9. [Error Handling](#error-handling)

---

## Overview

**⚠️ This document describes the PLANNED implementation, not the current state.**

The Hodos Browser startup flow is designed to be **non-blocking** and **wallet-optional**. The browser can fully function without a wallet, and wallet initialization is deferred until the user explicitly requests it.

### Current Implementation vs Planned Implementation

| Aspect | Current State | Planned State |
|--------|--------------|---------------|
| **C++ Wallet Server Start** | ❌ Not implemented | ✅ **Always** start server on launch |
| **C++ Wallet File Check** | ❌ Not implemented | ❌ Removed — server handles this |
| **Rust Auto-Create Wallet** | ✅ Auto-creates on server start | ❌ User-initiated only |
| **Frontend Startup Check** | ✅ Commented out (disabled) | ✅ Check on Wallet button click |

### Key Principles (Planned Implementation)

1. **Non-blocking startup**: Browser launches regardless of wallet status
2. **Always-on server**: Wallet server always starts; returns `exists: false` if no DB
3. **User-driven creation**: Wallet creation/recovery only happens via user action
4. **Two modal triggers**: Wallet button click (Phase 1) and HTTP intercept with no wallet (Phase 2)

### Wallet States

- **No Wallet**: Wallet database file doesn't exist
- **Wallet Exists**: Wallet database file exists and is valid
- **Wallet Invalid**: Wallet database file exists but is corrupted
- **Server Running**: Rust wallet server (Actix-web) is running on port 3301
- **Server Stopped**: Rust wallet server is not running

---

## Startup Sequence

### Phase 1: Application Entry Point (C++)

**Location**: `cef-native/cef_browser_shell.cpp`
**Function**: `WinMain()`

**Steps**:
1. Initialize logger (`Logger::Initialize()`)
2. Set up CEF settings (cache paths, subprocess paths)
3. Determine window dimensions
4. Register window classes (main shell, header, webview, overlays)
5. Create main shell window (`g_hwnd`)
6. Create header window (`g_header_hwnd`) - React UI
7. Create webview window (`g_webview_hwnd`) - Web content (hidden, tabs handle content)
8. Initialize CEF framework (`CefInitialize()`)
9. Initialize HistoryManager
10. **Always start wallet server** (see [Wallet Server Startup](#wallet-server-startup))
11. **Wait for server ready** (poll `/health`)
12. Enter message loop

**⚠️ TO BE IMPLEMENTED**

**Current State**: The C++ browser does NOT currently perform wallet file checks or start the wallet server on startup.

**Planned Code** (to be added to `cef-native/cef_browser_shell.cpp:981`):
```cpp
// cef-native/cef_browser_shell.cpp:981
int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE, LPSTR, int nCmdShow) {
    // ... existing initialization ...

    // Always start wallet server — it handles wallet existence internally
    // Server returns { exists: false } from /wallet/status when no DB present
    startWalletServer();
    waitForWalletServerReady();

    // ... continue with browser initialization ...
}
```

---

## Wallet File Check

### Purpose

Quickly determine if a wallet exists without starting the wallet server or opening the database.

### Implementation Status

**⚠️ NOT YET IMPLEMENTED** - This functionality needs to be added to the C++ startup code.

**Location**: C++ startup code (to be implemented in `cef-native/cef_browser_shell.cpp`)
**Method**: File system check

**Wallet Database Path**:
```
%APPDATA%/HodosBrowser/wallet/wallet.db
```

**Check Logic**:
```cpp
bool checkWalletFileExists() {
    std::string appdata = std::getenv("APPDATA") ? std::getenv("APPDATA") : "";
    std::string wallet_path = appdata + "\\HodosBrowser\\wallet\\wallet.db";
    return std::filesystem::exists(wallet_path);
}
```

### Validation

If the file exists, perform basic validation:

1. **File is readable**: Can open file
2. **File is valid SQLite**: Can open as SQLite database
3. **Database schema exists**: Required tables exist
4. **Primary wallet exists**: `wallets` table has a primary wallet record

**Validation Function** (⚠️ TO BE IMPLEMENTED):
```cpp
bool validateWalletDatabase(const std::string& db_path) {
    // Try to open SQLite database
    sqlite3* db;
    if (sqlite3_open(db_path.c_str(), &db) != SQLITE_OK) {
        return false; // Invalid SQLite file
    }

    // Check if primary wallet exists
    const char* query = "SELECT COUNT(*) FROM wallets WHERE is_primary = 1";
    sqlite3_stmt* stmt;
    if (sqlite3_prepare_v2(db, query, -1, &stmt, nullptr) != SQLITE_OK) {
        sqlite3_close(db);
        return false;
    }

    bool has_wallet = false;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        int count = sqlite3_column_int(stmt, 0);
        has_wallet = (count > 0);
    }

    sqlite3_finalize(stmt);
    sqlite3_close(db);

    return has_wallet;
}
```

### Result

- **File exists + valid**: Proceed to start wallet server
- **File exists + invalid**: Log warning, continue without wallet server
- **File doesn't exist**: Continue without wallet server

---

## Wallet Server Startup

### Purpose

Start the Rust wallet server (Actix-web) when a valid wallet exists.

### Implementation Status

**⚠️ NOT YET IMPLEMENTED** - This functionality needs to be added to the C++ startup code.

**Location**: C++ startup code (to be implemented in `cef-native/cef_browser_shell.cpp`)
**Method**: Launch Rust wallet executable as subprocess

**Wallet Server Executable**:
```
rust-wallet/target/release/hodos-wallet.exe
```

**Startup Command**:
```cpp
void startWalletServer() {
    std::string wallet_exe = "rust-wallet\\target\\release\\hodos-wallet.exe";

    STARTUPINFOA si = { sizeof(si) };
    PROCESS_INFORMATION pi;

    // Start wallet server in background
    if (CreateProcessA(
        nullptr,
        const_cast<char*>(wallet_exe.c_str()),
        nullptr,
        nullptr,
        FALSE,
        CREATE_NO_WINDOW,  // Run in background
        nullptr,
        nullptr,
        &si,
        &pi
    )) {
        LOG_INFO("✅ Wallet server started (PID: " + std::to_string(pi.dwProcessId) + ")");
        CloseHandle(pi.hThread);
        CloseHandle(pi.dwProcessId);

        // Wait for server to be ready (poll http://localhost:3301/health)
        waitForWalletServerReady();
    } else {
        LOG_ERROR("❌ Failed to start wallet server");
    }
}
```

**Server Readiness Check**:
```cpp
bool waitForWalletServerReady(int max_attempts = 10, int delay_ms = 500) {
    for (int i = 0; i < max_attempts; i++) {
        // Try to connect to http://localhost:3301/health
        // If successful, server is ready
        if (checkWalletServerHealth()) {
            LOG_INFO("✅ Wallet server is ready");
            return true;
        }
        Sleep(delay_ms);
    }
    LOG_WARNING("⚠️ Wallet server did not become ready in time");
    return false;
}
```

### Rust Wallet Server Behavior

**Location**: `rust-wallet/src/main.rs`

**Current Behavior** (lines 98-124 in `rust-wallet/src/main.rs`):
- ✅ On startup, checks if wallet exists in database
- ❌ **Auto-creates wallet if none exists** (lines 106-120) - **This needs to be changed**

**⚠️ CHANGE REQUIRED**: The auto-creation behavior needs to be **disabled** so wallet creation is user-driven only.

**Planned Change** (to be implemented):
```rust
// rust-wallet/src/main.rs
match wallet_repo.get_primary_wallet() {
    Ok(Some(wallet)) => {
        println!("📋 Wallet found in database (ID: {})", wallet.id.unwrap());
        println!("   Addresses: {}", wallet.current_index + 1);
    }
    Ok(None) => {
        // COMMENTED OUT: Auto-create wallet on startup
        // User must explicitly create wallet via UI
        println!("🔑 No wallet in database - wallet server ready for user-initiated creation");
        // Do NOT auto-create - wait for /wallet/create endpoint call
    }
    Err(e) => {
        eprintln!("   ⚠️  Error checking for wallet: {}", e);
    }
}
```

### Server Endpoints

Once started, the wallet server provides HTTP API on `http://localhost:3301`:

- `GET /wallet/status` - Check if wallet exists
- `POST /wallet/create` - Create new wallet (user-initiated)
- `POST /wallet/recover` - Recover wallet from mnemonic
- `POST /wallet/restore` - Restore wallet from backup file
- `GET /health` - Server health check

---

## Browser Initialization

### CEF Initialization

**Location**: `cef-native/src/handlers/simple_app.cpp`
**Function**: `SimpleApp::OnContextInitialized()`

**Steps**:
1. Create header browser (React UI) - loads `http://127.0.0.1:5137`
2. Create initial tab browser (web content)
3. Set up HTTP request interceptor
4. Initialize message handlers

**Code Reference**:
```cpp
// cef-native/src/handlers/simple_app.cpp:86
void SimpleApp::OnContextInitialized() {
    // Create header browser (React UI)
    CefRefPtr<SimpleHandler> header_handler = new SimpleHandler("header");
    std::string header_url = "http://127.0.0.1:5137";
    CefBrowserHost::CreateBrowser(header_window_info, header_handler, header_url, ...);

    // Create initial tab browser
    // ... tab creation logic ...
}
```

### Frontend Dev Server

**Location**: Vite dev server (separate process)
**URL**: `http://127.0.0.1:5137`

The React frontend runs in a Vite dev server (development) or is built and served statically (production).

---

## Frontend Startup

### React Application Entry

**Location**: `frontend/src/main.tsx`

**Steps**:
1. Import React and ReactDOM
2. Import BrowserRouter
3. Import bridge initialization (`initWindowBridge`)
4. Render App component

**Code Reference**:
```typescript
// frontend/src/main.tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import App from './App';
import './index.css';
import './bridge/initWindowBridge'; // Initialize window.hodosBrowser API

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>
);
```

### App Component Initialization

**Location**: `frontend/src/App.tsx`

**Steps**:
1. Set up routes
2. Initialize global state
3. **No wallet check on startup** (removed - now done on Wallet button click)
4. Set up BRC-100 auth modal state

**Previous Behavior** (commented out):
```typescript
// COMMENTED OUT: Wallet status check on startup
// This was previously used to create wallet.json and prompt users to save mnemonic
// We have changed how wallet.json is created, so this check is disabled for now
// TODO: Re-implement wallet initialization check in the future with new wallet creation flow
/*
const checkWalletStatus = async () => {
  // ... old wallet check logic ...
};
checkWalletStatus();
*/
```

**Current Behavior** (as of 2025-01-27):
- ✅ Browser loads without checking wallet (wallet check is commented out)
- ❌ Wallet button currently opens wallet overlay directly (does NOT check if wallet exists)
- ⚠️ Wallet check on Wallet button click needs to be implemented (see [Wallet Initialization Flow](./helper-1-implementation-guide-checklist.md#wallet-initialization-flow))

---

## Flow Diagrams

### Complete Startup Flow (Option A — Always Start Server)

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Launch                       │
│                  (WinMain entry point)                      │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Initialize Logger & CEF Settings                │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Create Windows (Shell, Header, WebView)         │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Initialize CEF Framework                       │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│   Always Start Wallet Server                                │
│   (Rust Actix-web on :3301)                                 │
│   Server handles wallet existence check internally          │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│   Wait for Server Ready                                     │
│   (Poll /health endpoint)                                   │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Initialize CEF Browsers                        │
│              (Header: React UI, Tabs: Web Content)          │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Enter Message Loop                             │
│              (Browser Ready for User Interaction)           │
└─────────────────────────────────────────────────────────────┘
```

### Wallet Button Click Flow

```
┌─────────────────────────────────────────────────────────────┐
│              User Clicks Wallet Button                      │
│              (MainBrowserView.tsx)                           │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│         Ensure Wallet Server Running                        │
│         (Start if not running)                              │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│         Call wallet.getStatus()                              │
│         (GET /wallet/status)                                 │
└──────────────┬───────────────────────────────┬──────────────┘
               │                               │
        ┌──────▼──────┐                ┌──────▼──────┐
        │ exists:true │                │exists:false │
        └──────┬──────┘                └──────┬──────┘
               │                               │
               ▼                               ▼
┌──────────────────────────┐    ┌──────────────────────────────┐
│   Open Wallet Overlay     │    │   Show Wallet Setup Modal    │
│   (Existing wallet UI)    │    │   (Create/Recover options)   │
└──────────────────────────┘    └──────────────────────────────┘
```

---

## Implementation Details

### Wallet File Path Resolution

**Windows**:
```cpp
std::string getWalletPath() {
    const char* appdata = std::getenv("APPDATA");
    if (!appdata) {
        LOG_ERROR("APPDATA environment variable not set");
        return "";
    }
    return std::string(appdata) + "\\HodosBrowser\\wallet\\wallet.db";
}
```

**Cross-Platform** (future):
```cpp
#ifdef _WIN32
    std::string appdata = std::getenv("APPDATA");
    return appdata + "\\HodosBrowser\\wallet\\wallet.db";
#elif __APPLE__
    std::string home = std::getenv("HOME");
    return home + "/Library/Application Support/HodosBrowser/wallet/wallet.db";
#else // Linux
    std::string home = std::getenv("HOME");
    return home + "/.local/share/HodosBrowser/wallet/wallet.db";
#endif
```

### Wallet Server Process Management

**Starting Server**:
- Launch as background process
- Store process handle for cleanup on shutdown
- Monitor process health

**Stopping Server**:
- Send graceful shutdown signal
- Wait for process termination
- Force kill if necessary

**Code Reference** (to be implemented):
```cpp
class WalletServerManager {
private:
    PROCESS_INFORMATION process_info_;
    bool is_running_ = false;

public:
    bool start() {
        // ... start process ...
        is_running_ = true;
        return true;
    }

    bool stop() {
        if (!is_running_) return true;
        // ... stop process ...
        is_running_ = false;
        return true;
    }

    bool isRunning() const { return is_running_; }
};
```

### Server Health Check

**Endpoint**: `GET http://localhost:3301/health`

**Expected Response**:
```json
{
  "status": "ok",
  "version": "1.0.0"
}
```

**Implementation**:
```cpp
bool checkWalletServerHealth() {
    // Use HTTP client to GET /health
    // Return true if status 200 and response contains "ok"
    // Return false otherwise
}
```

---

## Error Handling

### Wallet File Check Errors

**File Not Found**:
- **Action**: Continue without wallet server
- **Log**: Info level ("No wallet found - browser will continue without wallet")
- **User Impact**: None - browser fully functional

**File Exists But Invalid**:
- **Action**: Log warning, continue without wallet server
- **Log**: Warning level ("Wallet database exists but is invalid")
- **User Impact**: Wallet features unavailable until recovery

**Permission Denied**:
- **Action**: Log error, continue without wallet server
- **Log**: Error level ("Cannot access wallet database - permission denied")
- **User Impact**: Wallet features unavailable

### Wallet Server Startup Errors

**Server Won't Start**:
- **Action**: Log error, continue without wallet server
- **Log**: Error level ("Failed to start wallet server")
- **User Impact**: Wallet features unavailable

**Server Starts But Not Ready**:
- **Action**: Log warning, continue without wallet server
- **Log**: Warning level ("Wallet server did not become ready in time")
- **User Impact**: Wallet features unavailable (user can retry via Wallet button)

**Server Crashes After Start**:
- **Action**: Detect crash, log error
- **Log**: Error level ("Wallet server crashed")
- **User Impact**: Wallet features unavailable (user can retry via Wallet button)

### Frontend Errors

**Bridge Not Initialized**:
- **Action**: Show error message, disable wallet features
- **User Impact**: Wallet button disabled or shows error

**Server Not Available**:
- **Action**: Show "Wallet server unavailable" message
- **User Impact**: Wallet button shows error, user can retry

---

## Integration with Wallet Initialization Flow

This startup flow integrates with the [Wallet Initialization Flow](./helper-1-implementation-guide-checklist.md#wallet-initialization-flow):

1. **Startup**: Checks if wallet exists, starts server if valid
2. **Wallet Button Click**: Checks wallet status, shows setup modal if needed
3. **User Creates Wallet**: Server creates wallet, user backs up mnemonic
4. **User Recovers Wallet**: Server recovers wallet from mnemonic/file
5. **Wallet Ready**: Wallet overlay opens, user can use wallet features

---

## Summary

### Implementation Status Summary

**What Exists Now**:
- ✅ Rust wallet server runs and auto-creates wallet if none exists
- ✅ Frontend wallet check on startup is commented out (as intended)
- ❌ C++ browser does NOT check wallet file on startup
- ❌ C++ browser does NOT start wallet server on startup
- ❌ Wallet button does NOT check wallet existence before opening overlay

**What Needs to be Implemented**:
1. ⚠️ **[PREREQUISITE]** Disable wallet auto-creation in Rust server (`main.rs`) — server must start without creating a wallet
2. ⚠️ Add wallet server startup logic in C++ startup (always start, no conditional)
3. ⚠️ Server `/wallet/status` returns `{ exists: false }` when no wallet DB present
4. ⚠️ Add wallet existence check to Wallet button click handler
5. ⚠️ Create WalletSetupModal component for create/recover flow
6. ⚠️ **[TESTING]** Test `<input type="file">` in CEF overlay subprocess — if it doesn't work, build a C++ bridge method to open a native `OPENFILENAME` dialog (needed for Phase 1 "Recover from file")

### CEF Refinement Prerequisite

**[CR-1 (Critical Stability & Security)](../CEF_REFINEMENT_TRACKER.md#cr-1-critical-stability--security)** should be completed before or alongside Phase 0. CR-1 fixes JS injection, auth hangs, and overlay buffer overflows that affect the current browser. These are independent of UX work and can be done in parallel.

### Planned Behavior (After Implementation)

The startup flow is designed to be:

- ✅ **Fast**: File check is instant, no blocking operations
- ✅ **Resilient**: Browser works even if wallet/server fails
- ✅ **User-friendly**: No prompts or blocking dialogs on startup
- ✅ **Flexible**: Wallet can be created/recovered on-demand

**Key Points** (Planned Implementation):

1. Browser startup does NOT block on wallet
2. Wallet file check is fast (file system only)
3. Wallet server starts only if wallet exists and is valid
4. Frontend does NOT check wallet on startup
5. Wallet check happens when user clicks Wallet button
6. Wallet creation/recovery is user-initiated via modal

---

**End of Document**
