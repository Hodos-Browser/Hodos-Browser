# Wallet Panel Regression Analysis

## Problem Summary

**Status**: Wallet panel no longer opens after Ishaan's Mac migration merge

**Previous Functionality** (Working on main branch):
- ✅ Wallet button click creates new CEF process
- ✅ Creates overlay_hwnd window
- ✅ Injects correct JSX component
- ✅ Displays balance in USD and sats
- ✅ Fetches exchange rate and calculates USD value
- ✅ Functional refresh balance button
- ✅ Receive button: routes through CEF-native → wallet → generates address → saves to DB → returns to frontend → displays and copies to clipboard
- ✅ Send button: form with address/amount entry, sats/dollars toggle, routes through CEF-native → wallet → creates/broadcasts transaction → saves to DB → returns txid → frontend shows WhatsOnChain link

**Current State**:
- ❌ Wallet panel does not open when wallet button is clicked

## Investigation Plan

### Phase 1: Identify What Changed

#### 1.1 Frontend Changes
- [ ] Compare wallet button click handler (main vs Ishaan's branch)
- [ ] Compare wallet route/component rendering
- [ ] Check for changes to wallet overlay creation messages
- [ ] Verify JSX component still exists and is correct

#### 1.2 CEF-Native Changes
- [ ] Compare overlay creation code (`CreateWalletOverlayWithSeparateProcess`)
- [ ] Check message handlers for wallet overlay messages
- [ ] Verify CEF browser creation for wallet overlay
- [ ] Check for changes to overlay window management

#### 1.3 Process/Window Management
- [ ] Compare HWND creation and management
- [ ] Check overlay window properties/styles
- [ ] Verify overlay positioning and visibility
- [ ] Check for changes to overlay render handler

### Phase 2: Root Cause Analysis

#### Areas to Investigate:
1. **Overlay Creation Method**
   - Did Ishaan change from separate process to something else?
   - Did he change the overlay window type/class?
   - Did he modify the overlay creation timing?

2. **Message Routing**
   - Are wallet messages still being sent correctly?
   - Are message handlers still registered?
   - Did message names/format change?

3. **Window Management**
   - Did overlay HWND creation change?
   - Are overlays being created but not shown?
   - Did window styles/properties change?

4. **CEF Browser Creation**
   - Is the CEF browser being created for wallet overlay?
   - Is the URL correct?
   - Is the render handler attached?

### Phase 3: Comparison Checklist

#### Files to Compare:
- [ ] `frontend/src/...` - Wallet button/component code
- [ ] `cef-native/cef_browser_shell.cpp` - Overlay creation
- [ ] `cef-native/src/handlers/simple_handler.cpp` - Message handling
- [ ] `cef-native/src/handlers/simple_app.cpp` - Overlay setup
- [ ] `cef-native/src/handlers/my_overlay_render_handler.cpp` - Render handler
- [ ] Any Mac-specific overlay files

### Phase 4: Fix Strategy

#### Approach:
1. **Identify the breaking change** - What exactly broke it?
2. **Assess Ishaan's changes** - Were they intentional improvements or accidental breaks?
3. **Preserve Mac functionality** - Ensure Mac code still works
4. **Restore wallet functionality** - Fix the breaking change
5. **Improve if possible** - Keep any UI/UX/speed improvements Ishaan made

#### Fix Options:
- **Option A**: Revert Ishaan's overlay changes, restore original working code
- **Option B**: Fix Ishaan's changes to work correctly
- **Option C**: Hybrid - Keep Mac improvements, restore Windows wallet functionality

## Analysis Notes

### Date: 2026-01-27
### Analyst: Initial investigation

#### Findings:
- ✅ **Message name changed**: Frontend now sends `toggle_wallet_panel` instead of `overlay_show_wallet`
- ✅ **Handler exists but incomplete**: `toggle_wallet_panel` handler exists in `simple_handler.cpp` but only implements Mac version
- ✅ **Windows handler missing**: Windows path just logs "not implemented on Windows yet" and returns
- ✅ **Old handler still exists**: `overlay_show_wallet` handler still exists and works, but frontend doesn't call it anymore

#### Root Cause:
- ✅ **Identified breaking change**: Ishaan changed frontend to use `toggle_wallet_panel` message (probably for Mac panel integration), but didn't implement the Windows version. The Windows code path just logs a warning and does nothing.

**Location**: `cef-native/src/handlers/simple_handler.cpp` line 1756-1767

**Code**:
```cpp
if (message_name == "toggle_wallet_panel") {
    LOG_DEBUG_BROWSER("💰 Toggle wallet panel requested");
#ifdef __APPLE__
    // Mac implementation exists
    extern void ToggleWalletPanel();
    ToggleWalletPanel();
#else
    LOG_DEBUG_BROWSER("⚠️ toggle_wallet_panel not implemented on Windows yet");
#endif
    return true;
}
```

**Frontend**: `frontend/src/pages/MainBrowserView.tsx` line 233 sends `toggle_wallet_panel`

#### Solution:
- ✅ **Proposed fix**: Add Windows implementation to `toggle_wallet_panel` handler that calls `CreateWalletOverlayWithSeparateProcess(g_hInstance)` (same as old `overlay_show_wallet` handler does)
- ✅ **Fix implemented**: 2026-01-27
  - **Issue Found**: Two `toggle_wallet_panel` handlers existed - first one (line 1425) was Mac-only and returned early on Windows, blocking the second handler
  - **Solution**:
    1. Removed first handler (line 1425-1454) that was blocking Windows
    2. Updated second handler (line 1725) to use `CreateWalletOverlayWithSeparateProcess()` on both platforms
    3. Both Mac and Windows now use separate overlay windows (consistent security model)
  - **Changes**:
    - Removed: First `toggle_wallet_panel` handler with embedded panel toggle (Mac-only)
    - Updated: Second handler to call `CreateWalletOverlayWithSeparateProcess()` on Mac instead of `ToggleWalletPanel()`
    - Result: Both platforms use separate overlay windows with process isolation
- ✅ **Testing plan**:
  1. ✅ Fix Windows handler to call overlay creation - DONE
  2. ✅ Remove duplicate handler blocking execution - DONE
  3. ⏳ Test wallet button click opens overlay - PENDING
  4. ⏳ Test all wallet functionality (balance, receive, send) - PENDING
  5. ⏳ Verify Mac functionality still works - PENDING

## Testing Checklist

After fix is applied:
- [ ] Wallet button opens wallet panel
- [ ] Balance displays correctly (USD + sats)
- [ ] Refresh balance button works
- [ ] Receive button generates address and copies to clipboard
- [ ] Send button form appears and works
- [ ] Transactions create and broadcast correctly
- [ ] Mac functionality still works (if applicable)

## Related Files

### Main Branch (Working):
- Wallet overlay creation: `cef-native/cef_browser_shell.cpp`
- Message handlers: `cef-native/src/handlers/simple_handler.cpp`
- Frontend wallet component: `frontend/src/...`

### Ishaan's Branch (Current):
- Same files - need to compare line by line

## Next Steps

1. Compare main branch to current branch for wallet-related code
2. Identify exact breaking change
3. Discuss with Ishaan about his changes
4. Implement fix
5. Test all wallet functionality
6. Verify Mac functionality still works
