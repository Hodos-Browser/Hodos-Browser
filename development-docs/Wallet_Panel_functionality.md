# Wallet Panel Functionality Restoration Plan

## Current Status

**Date**: 2026-01-27
**Status**: Light wallet opens correctly, but missing critical functionality

### What's Working ✅
- ✅ Light wallet overlay opens when wallet button clicked
- ✅ Balance displays in sats
- ✅ Advanced button opens full wallet in new tab

### What's Broken ❌
- ❌ Balance does NOT display in USD (missing USD conversion)
- ❌ Refresh balance button doesn't work properly
- ❌ Receive button doesn't work (address generation/copying broken)
- ❌ Send button doesn't work (transaction form missing or broken)
- ⚠️ Window positioning issue (opens on wrong side - Windows only)

## What We're Doing

Restoring all functionality from the main branch wallet to the current branch light wallet (`WalletPanel.tsx`).

**Requirements:**
- Keep Ishaan's CSS styling (`WalletPanel.css`, `TransactionComponents.css`)
- Keep Ishaan's component structure (overlay panel type)
- Add ALL missing functionality from main branch
- Fix window positioning issue
- Remove miner fee option from send form (as requested)
- Keep USD/sats toggle in send form

## Two Wallet Interfaces

1. **Light Wallet** (`WalletPanel.tsx`) - Small overlay panel, simple UI with balance, send, receive buttons. This is what opens from the wallet button.
2. **Full Wallet** (`WalletOverlayRoot.tsx`) - Large overlay with tabs for transactions, addresses, outputs, certificates. This opens from the light wallet's "Advanced" button.

## Previous Functionality (Main Branch)

The main branch had a working wallet panel with:
- ✅ Balance display in both BSV (8 decimals) and USD ($X.XX format)
- ✅ Exchange rate fetching from CryptoCompare API
- ✅ Functional refresh balance button
- ✅ Receive button: Generates address → saves to DB → displays → copies to clipboard
- ✅ Send button: Full transaction form with:
  - Address input
  - Amount input with USD/sats toggle
  - Transaction creation and broadcasting
  - Success/error display with WhatsOnChain link
- ✅ Transaction result display with copy link functionality

## Files Analyzed

### Current Branch Files:
1. `frontend/src/components/WalletPanel.tsx` - Simple light wallet (240x200px, basic buttons) - **NEEDS UPDATE**
2. `frontend/src/components/panels/WalletPanelContent.tsx` - More complete wallet panel with full functionality - **USE AS REFERENCE**
3. `frontend/src/pages/WalletPanelPage.tsx` - Wrapper page for WalletPanel
4. `frontend/src/hooks/useBalance.ts` - Balance fetching hook (has USD conversion) - **USE THIS**
5. `frontend/src/hooks/useAddress.ts` - Address generation hook - **USE THIS**
6. `frontend/src/hooks/useTransaction.ts` - Transaction sending hook - **USE THIS**
7. `frontend/src/components/TransactionForm.tsx` - Transaction form component - **NEEDS UPDATE** (remove fee rate)
8. `frontend/src/components/WalletPanel.css` - Styling (keep from Ishaan)
9. `frontend/src/components/TransactionComponents.css` - Transaction styling (keep from Ishaan)
10. `cef-native/src/handlers/simple_app.cpp` - Overlay creation (Windows) - **NEEDS FIX** (window positioning)
11. `cef-native/cef_browser_shell_mac.mm` - Overlay creation (Mac)

### Main Branch Files (for comparison):
1. `frontend/src/pages/WalletOverlayRoot.tsx` - Simple wrapper that loaded WalletPanelLayout
2. `frontend/src/components/panels/WalletPanelLayout.tsx` - Drawer layout wrapper
3. `frontend/src/components/panels/WalletPanelContent.tsx` - Full wallet functionality

## Mac vs Windows Considerations

### Message System Architecture

**Frontend → CEF Native Communication:**
1. Frontend calls: `window.cefMessage.send('message_name', [args])`
2. CEF Render Process Handler (`simple_render_process_handler.cpp`) receives the message
3. Message is sent to Browser Process via `SendProcessMessage(PID_BROWSER, message)`
4. Browser Process Handler (`simple_handler.cpp`) receives via `OnProcessMessageReceived()`
5. Handler processes the message and calls appropriate service (e.g., `WalletService`)

**Message Examples:**
- `get_balance` → Returns `{ balance: number }`
- `address_generate` → Returns address data
- `send_transaction` → Returns transaction result

### CEF Native → Rust Wallet Communication

**Windows (`WalletService.cpp`):**
- Uses **WinHTTP API** for HTTP requests
- Communicates with Rust wallet HTTP API at `http://localhost:3301`
- Endpoints: `/wallet/balance`, `/wallet/addresses`, `/wallet/transactions`, etc.

**Mac (`WalletService_mac.cpp`):**
- Uses **libcurl** for HTTP requests
- Same HTTP API endpoints (`http://localhost:3301`)
- Same JSON request/response format

### Platform-Specific Files

1. **`WalletService.cpp`** (Windows) - Uses WinHTTP
2. **`WalletService_mac.cpp`** (Mac) - Uses libcurl
3. **`simple_app.cpp`** - Has `#ifdef _WIN32` blocks for Windows overlay creation
4. **`cef_browser_shell_mac.mm`** - Mac-specific overlay creation

### Message System Compatibility

✅ **The message system is cross-platform:**
- CEF process messages work identically on both platforms
- Message names are identical (`get_balance`, `address_generate`, `send_transaction`)
- Response format is identical (JSON strings)
- Only the HTTP client implementation differs (WinHTTP vs libcurl)

### Potential Issues & Solutions

**Issue 1: Message handlers in overlay process**
- **Problem**: Wallet overlay runs in a separate CEF process. Message handlers must be registered in that process.
- **Solution**: ✅ Handlers are already registered in `simple_handler.cpp` and work in all processes (browser, render, overlay).

**Issue 2: WalletService initialization**
- **Problem**: `WalletService` needs to connect to Rust wallet HTTP API.
- **Solution**: ✅ Both `WalletService.cpp` and `WalletService_mac.cpp` handle HTTP connections. Ensure Rust wallet service is running on `localhost:3301`.

**Issue 3: Response callbacks**
- **Problem**: Frontend uses window callbacks (`window.onGetBalanceResponse`, etc.) that must be set up correctly.
- **Solution**: ✅ The bridge (`initWindowBridge.ts`) sets up these callbacks. Ensure they're initialized in the overlay process.

**Issue 4: Process isolation**
- **Problem**: Overlay runs in a separate process, so it has its own JavaScript context.
- **Solution**: ✅ The render process handler injects the bridge code, so `window.cefMessage` and `window.hodosBrowser` are available in the overlay.

### Recommendations

1. ✅ **No changes needed to message system** - It's already cross-platform compatible
2. ⚠️ **Ensure Rust wallet service is running** - Both platforms require `localhost:3301` to be accessible
3. ⏳ **Test message handlers in overlay** - Verify `get_balance`, `address_generate`, `send_transaction` work in overlay process
4. ⏳ **Verify bridge initialization** - Ensure `initWindowBridge.ts` runs in overlay process

## Implementation Plan

### Phase 1: Update WalletPanel.tsx

**File**: `frontend/src/components/WalletPanel.tsx`

**Changes**:
1. Replace entire component with functionality from `WalletPanelContent.tsx`
2. Keep the component name as `WalletPanel` (for compatibility)
3. Use `useBalance()` hook for balance + USD display
4. Use `useAddress()` hook for receive functionality
5. Use `TransactionForm` component for send functionality
6. Remove navigation grid (keep only balance, send, receive, advanced)
7. Keep Ishaan's CSS classes and styling
8. Adjust size to fit all functionality (increase from 240x200px)

**Key Features to Implement**:
- ✅ Balance display: BSV amount + USD value (from `useBalance()`)
- ✅ Refresh balance button (calls `refreshBalance()` from `useBalance()`)
- ✅ Send button: Toggles `TransactionForm` component
- ✅ Receive button: Generates address, displays it, copies to clipboard
- ✅ Advanced button: Opens `/wallet` route in new tab
- ✅ Transaction result display: Shows success/error with WhatsOnChain link
- ✅ Address display: Shows generated address with "Copy Again" button

### Phase 2: Update TransactionForm Component

**File**: `frontend/src/components/TransactionForm.tsx`

**Changes**:
1. Remove fee rate input field (as requested)
2. Keep USD/sats toggle functionality
3. Ensure proper validation
4. Ensure proper error handling

### Phase 3: Fix Window Positioning (Windows Only)

**File**: `cef-native/src/handlers/simple_app.cpp`

**Changes**:
- Review overlay window creation and positioning
- Ensure overlay appears on correct side of screen
- May need to adjust `mainRect` calculations or window positioning logic

## Success Criteria

1. ✅ Light wallet opens when wallet button clicked
2. ✅ Balance displays in both BSV (8 decimal places) and USD ($X.XX format)
3. ✅ Refresh button updates balance
4. ✅ Receive button generates address, displays it, copies to clipboard
5. ✅ Send button opens transaction form
6. ✅ Transaction form has USD/sats toggle
7. ✅ Transaction form does NOT have fee rate option
8. ✅ Send transaction works end-to-end
9. ✅ Transaction result shows with WhatsOnChain link
10. ✅ Advanced button opens full wallet in new tab
11. ✅ Window positioning correct (Windows)
12. ✅ All styling matches Ishaan's design
13. ✅ Message system works correctly on both platforms
14. ✅ Rust wallet HTTP API accessible from overlay process

## Next Steps

1. ⏳ Implement Step 1: Rewrite `WalletPanel.tsx` with full functionality
2. ⏳ Implement Step 2: Verify/update `TransactionForm.tsx` (remove fee rate)
3. ⏳ Implement Step 3: Fix window positioning
4. ⏳ Final testing and verification
