# Tab Management Testing Guide - Phase 1

**Date**: December 15, 2025
**Status**: Backend Complete, Console Testing Available
**Next**: Phase 3 React Tab Bar UI

---

## ✅ What Was Implemented

### Backend (C++ CEF)
- ✅ TabManager class - manages all tabs
- ✅ Process-per-tab architecture - each tab isolated
- ✅ Tab creation/closing/switching
- ✅ HWND management (show/hide for switching)
- ✅ Navigation routing to active tab
- ✅ Tab state tracking (title, URL, loading)
- ✅ Message handlers for tab operations

### Frontend API
- ✅ `window.cefMessage.send("tab_create", url)`
- ✅ `window.cefMessage.send("tab_close", tabId)`
- ✅ `window.cefMessage.send("tab_switch", tabId)`
- ✅ `window.cefMessage.send("get_tab_list")`

### Not Yet Implemented
- ❌ Visual tab bar UI (Phase 3)
- ❌ Keyboard shortcuts (Phase 3)
- ❌ Drag-and-drop reordering (Phase 3)

---

## 🧪 Testing Instructions

### Prerequisites
1. ✅ Rust wallet backend running on port 3301
2. ✅ React frontend running on port 5137
3. ✅ HodosBrowserShell.exe running

### Step 1: Open DevTools

Press **F12** or **Ctrl+Shift+I** to open the browser console.

---

## Test Scenarios

### Test 1: Verify Initial Tab

**Check**: Browser should start with one tab automatically

**Console Command**:
```javascript
window.cefMessage.send("get_tab_list");
```

**Expected Output** (in CEF logs or console):
```json
{
  "tabs": [
    {
      "id": 1,
      "title": "...",
      "url": "https://metanetapps.com/",
      "isActive": true,
      "isLoading": false
    }
  ],
  "activeTabId": 1
}
```

---

### Test 2: Create New Tab

**Console Command**:
```javascript
window.cefMessage.send("tab_create", "https://google.com");
```

**Expected Behavior**:
- New tab created with ID 2
- Tab 2 becomes active (visible)
- Tab 1 is hidden
- Console logs: "Tab created: ID 2"

**Verify**:
- Check Task Manager - should see additional HodosBrowserShell.exe process (new tab process)
- Browser content should change to Google

---

### Test 3: Switch Between Tabs

**Console Commands**:
```javascript
// Switch to tab 1
window.cefMessage.send("tab_switch", 1);

// Wait 2 seconds, then switch to tab 2
setTimeout(() => {
    window.cefMessage.send("tab_switch", 2);
}, 2000);
```

**Expected Behavior**:
- Tab 1 becomes visible (metanetapps.com)
- After 2 seconds, tab 2 becomes visible (google.com)
- Only one tab visible at a time
- Console logs: "Tab switch: ID X succeeded"

---

### Test 4: Create Multiple Tabs

**Console Commands**:
```javascript
window.cefMessage.send("tab_create", "https://github.com");
window.cefMessage.send("tab_create", "https://stackoverflow.com");
window.cefMessage.send("tab_create", "https://example.com");

// Get list of all tabs
window.cefMessage.send("get_tab_list");
```

**Expected Behavior**:
- 3 new tabs created (IDs 3, 4, 5)
- Tab 5 (example.com) is active
- Task Manager shows 5+ HodosBrowserShell processes (1 main + 5 tabs)

---

### Test 5: Close a Tab

**Console Commands**:
```javascript
// Close the active tab (tab 5)
window.cefMessage.send("tab_close", 5);

// Check remaining tabs
window.cefMessage.send("get_tab_list");
```

**Expected Behavior**:
- Tab 5 closes
- Automatically switches to tab 4 (most recently accessed)
- Task Manager shows one fewer process
- Console logs: "Tab close: ID 5 succeeded"

---

### Test 6: Navigation in Active Tab

**Console Commands**:
```javascript
// Make sure you're on a specific tab (e.g., tab 1)
window.cefMessage.send("tab_switch", 1);

// Navigate
window.cefMessage.send("navigate", "https://example.com");

// Wait for page to load, then test back/forward
setTimeout(() => {
    window.cefMessage.send("navigate_back");
}, 3000);

setTimeout(() => {
    window.cefMessage.send("navigate_forward");
}, 6000);

// Reload
setTimeout(() => {
    window.cefMessage.send("navigate_reload");
}, 9000);
```

**Expected Behavior**:
- Only tab 1 navigates (other tabs unchanged)
- Back/forward/reload work on active tab
- Console logs show: "Navigate to ... on active tab 1"

---

### Test 7: Test Tab Isolation

**Console Commands**:
```javascript
// Create 2 tabs with different sites
window.cefMessage.send("tab_create", "https://google.com");
window.cefMessage.send("tab_create", "https://github.com");

// Switch to tab 1 (Google)
window.cefMessage.send("tab_switch", 1);
// Set a variable in tab 1's console
window.testVariable = "Tab 1";

// Switch to tab 2 (GitHub)
window.cefMessage.send("tab_switch", 2);
// Try to access the variable
console.log(window.testVariable);  // Should be undefined
```

**Expected Result**:
- `window.testVariable` is `undefined` in tab 2
- **Confirms process isolation** - tabs have separate V8 contexts

---

### Test 8: Stress Test - Many Tabs

**Console Command**:
```javascript
// Create 10 tabs
for (let i = 0; i < 10; i++) {
    window.cefMessage.send("tab_create", `https://example.com?tab=${i}`);
}

// Get tab count
window.cefMessage.send("get_tab_list");
```

**Expected Behavior**:
- 10 new tabs created
- Task Manager shows 10+ additional processes
- No crashes or memory errors
- Can still switch between tabs

**Check**:
- Memory usage in Task Manager
- CPU usage stays reasonable
- No error logs

---

### Test 9: Close All Tabs Except One

**Console Commands**:
```javascript
// Assuming you have tabs 1-10, close 9 of them
for (let i = 2; i <= 10; i++) {
    window.cefMessage.send("tab_close", i);
}

// Verify only 1 tab remains
window.cefMessage.send("get_tab_list");
```

**Expected Behavior**:
- Only tab 1 remains
- Browser doesn't crash
- Tab 1 is active and functional

---

## 🔍 What to Look For

### In CEF Console Logs
```
✅ TabManager initialized
✅ Creating tab 1 with URL: https://metanetapps.com/
✅ Created HWND for tab 1: [handle]
✅ Created SimpleHandler for tab 1 with role: tab_1
✅ Tab browser registered: ID 1, Browser ID: [id]
✅ Tab 1 title updated to: [page title]
✅ Tab 1 URL updated to: [url]
✅ Tab 1 loading state: loaded
```

### In Task Manager
- **Before tabs**: 6-7 HodosBrowserShell.exe processes (main + overlays)
- **After 1 tab**: 7-8 processes (+ 1 tab render process)
- **After 5 tabs**: 11-12 processes (+ 5 tab render processes)

### In Browser Behavior
- ✅ Only one tab visible at a time
- ✅ Switching tabs changes visible content
- ✅ Navigation affects only active tab
- ✅ Tab state persists when switching away and back
- ✅ No crashes or freezes

---

## 🐛 Troubleshooting

### Issue: "tab_create" doesn't work
**Solution**: Make sure you're running the command in the header browser (main window), not in a tab or overlay.

### Issue: No visual change when switching tabs
**Possible Causes**:
1. Both tabs showing same content
2. Tab HWND not being shown/hidden correctly
3. Check CEF logs for errors

**Debug**: Check logs for "Tab switch: ID X succeeded"

### Issue: Browser crashes when creating tab
**Check**:
1. CEF logs for error messages
2. Windows Event Viewer for crash dumps
3. Make sure CEFHostWindow class is registered

### Issue: Can't see console logs
**Solution**:
- Check debug_output.log file in Release directory
- Check startup_log.txt
- Make sure LOG macros are enabled

---

## 📊 Success Criteria

Phase 1 is successful if:
- [x] Browser starts without crashes
- [ ] Can create at least 5 tabs
- [ ] Can switch between tabs smoothly
- [ ] Can close tabs without crashes
- [ ] Each tab has isolated V8 context
- [ ] Navigation works on active tab only
- [ ] Tab state updates correctly
- [ ] No memory leaks after creating/closing 20 tabs

---

## 🎯 Next Phase: React Tab Bar UI

Once Phase 1 testing is complete, Phase 3 will add:
- Visual tab bar at top of browser
- Click to switch tabs
- Click X to close tabs
- Click + to create new tab
- Keyboard shortcuts (Ctrl+T, Ctrl+W, Ctrl+Tab)
- Tab title display
- Active tab highlighting
- Tab overflow scrolling

**Estimated Time**: 4-6 hours for full UI

---

**Last Updated**: December 15, 2025
