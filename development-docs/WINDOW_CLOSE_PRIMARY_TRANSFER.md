# Primary Window Close — Transfer to Surviving Window (IMPLEMENTED 2026-04-02)

## Problem

When the user tears off a tab into a new window (window 1) and then closes the original window (window 0), the entire application shuts down — both windows close. This is because `WM_CLOSE` on window 0 triggers `ShutdownApplication()` regardless of how many other windows exist.

### Root Cause

`cef_browser_shell.cpp` `ShellWindowProc` WM_CLOSE handler (line ~1073):

```cpp
if (wid == 0 || WindowManager::GetInstance().GetWindowCount() <= 1) {
    // Primary window or last window — full graceful shutdown
    ShutdownApplication();
}
```

The `wid == 0` condition always triggers shutdown for the primary window.

### MVP Fix (Option A — Implemented)

Changed the condition to only check window count:
```cpp
if (WindowManager::GetInstance().GetWindowCount() <= 1) {
    // Last window — full shutdown
} else {
    // Close this window only
}
```

When window 0 closes with other windows alive, all 11 overlay HWNDs are destroyed and their globals nulled. Surviving windows lose overlay access (wallet panel, settings, cookie panel, downloads, etc.) until app restart. This is acceptable for MVP since the scenario is uncommon.

## Full Fix (Option B — Post-MVP)

Transfer the "primary window" role from window 0 to the next surviving window so overlays continue working.

### What Needs to Transfer

When window 0 closes and window N survives:

**1. Global HWND reassignment:**
```
g_hwnd         → window N's hwnd
g_header_hwnd  → window N's header_hwnd
g_webview_hwnd → nullptr (legacy, unused)
```

**2. BrowserWindow 0 reassignment:**
WindowManager currently hardcodes window 0 as the primary. Need to either:
- Renumber window N to become window 0, or
- Remove the assumption that window 0 is special

**3. Overlay HWND re-parenting:**
All 11 overlay HWNDs are `WS_POPUP` windows positioned relative to `g_hwnd`/`g_header_hwnd`. They need:
- `SetWindowPos` to reposition relative to the new primary window
- Their `WM_MOVE`/`WM_SIZE` repositioning code in `ShellWindowProc` already uses `g_hwnd`/`g_header_hwnd` globals — if we reassign the globals, repositioning should self-correct on next move/resize

**4. Overlay browser handler retargeting:**
Each overlay's `SimpleHandler` has a `window_id_` field. These need to update to the new primary window's ID.

**5. Mouse hooks:**
Overlay mouse hooks (`g_settings_mouse_hook`, `g_cookie_panel_mouse_hook`, etc.) track clicks relative to the overlay HWND, not the main window. These should work unchanged after re-parenting.

**6. WM_ACTIVATEAPP handler:**
The main WndProc's `WM_ACTIVATEAPP` handler manages overlay dismissal on app focus loss. It references overlay globals directly — should work after reassignment.

**7. Session save:**
`SaveSession()` iterates all windows and tabs via `WindowManager`. No window 0 assumption — should work.

### Implementation Plan

**Step 1: Extract overlay cleanup into a reusable function**
Move the overlay destruction code from `ShutdownApplication()` into `DestroyAllOverlays()`. Also create `DetachOverlaysFromWindow()` that hides (not destroys) overlays.

**Step 2: Create `TransferPrimaryWindow(int new_wid)`**
```cpp
void TransferPrimaryWindow(int new_wid) {
    BrowserWindow* newPrimary = WindowManager::GetInstance().GetWindow(new_wid);
    if (!newPrimary) return;

    // Reassign globals
    g_hwnd = newPrimary->hwnd;
    g_header_hwnd = newPrimary->header_hwnd;
    g_webview_hwnd = nullptr;

    // Hide all visible overlays (they'll reposition on next show)
    HideAllOverlays();

    // Update overlay handler window IDs
    // (each overlay SimpleHandler's window_id_ → new_wid)
    RetargetOverlayHandlers(new_wid);

    // Update WindowManager: mark new_wid as primary
    WindowManager::GetInstance().SetPrimaryWindow(new_wid);
}
```

**Step 3: Update `WM_CLOSE` handler**
```cpp
case WM_CLOSE: {
    BrowserWindow* bw = ...;
    int wid = bw ? bw->window_id : 0;
    int windowCount = WindowManager::GetInstance().GetWindowCount();

    if (windowCount <= 1) {
        // Last window — full shutdown
        ShutdownApplication();
    } else if (wid == WindowManager::GetInstance().GetPrimaryWindowId()) {
        // Primary window closing with other windows alive — transfer primary role
        int nextWid = WindowManager::GetInstance().GetNextWindowId(wid);
        TransferPrimaryWindow(nextWid);
        // Then close this window like a secondary
        CloseSecondaryWindow(bw, hwnd, wid);
    } else {
        // Secondary window — close normally
        CloseSecondaryWindow(bw, hwnd, wid);
    }
    return 0;
}
```

**Step 4: Add `SetPrimaryWindow()` and `GetPrimaryWindowId()` to WindowManager**
Remove the assumption that window 0 is always primary. Track primary via a member variable.

**Step 5: Audit all `wid == 0` checks**
Search for code that assumes window 0 is special:
- `ShellWindowProc` WM_MOVE: `if (bw && bw->window_id != 0) break;` — overlay repositioning skipped for non-primary windows
- `ShellWindowProc` WM_SIZE: same pattern
- Overlay creation functions: reference `g_hwnd` globally
- `simple_handler.cpp`: various overlay show handlers

Replace `window_id != 0` with `window_id != GetPrimaryWindowId()`.

### Files Involved

| File | Changes |
|------|---------|
| `cef_browser_shell.cpp` | `WM_CLOSE` handler, `TransferPrimaryWindow()`, `DestroyAllOverlays()` extraction |
| `WindowManager.h/.cpp` | `primaryWindowId_` member, `SetPrimaryWindow()`, `GetPrimaryWindowId()`, `GetNextWindowId()` |
| `simple_handler.cpp` | `RetargetOverlayHandlers()`, overlay show handlers audit |
| `simple_app.cpp` | Overlay creation functions — use `GetPrimaryWindowId()` instead of hardcoded 0 |

### Testing

1. Open browser (window 0), open a few tabs
2. Tear off a tab → window 1 created
3. Close window 0 → window 1 survives, becomes primary
4. Open wallet panel in window 1 → positions correctly
5. Open all overlays → all work
6. Close window 1 → app shuts down cleanly
7. Repeat with 3+ windows, close in various orders
8. Verify session save/restore works after primary transfer
