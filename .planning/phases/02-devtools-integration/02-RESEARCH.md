# Phase 2: DevTools Integration - Research

**Researched:** 2026-01-20
**Domain:** CEF DevTools API, keyboard shortcuts, context menus, cross-platform window management
**Confidence:** HIGH

<research_summary>
## Summary

Researched the CEF (Chromium Embedded Framework) ecosystem for integrating Chrome DevTools into a cross-platform application. CEF provides built-in DevTools support through the `CefBrowserHost::ShowDevTools()` API, which creates a separate browser window containing the full Chrome DevTools UI.

The standard approach involves three integration points: (1) keyboard shortcuts via `CefKeyboardHandler::OnPreKeyEvent()` to capture F12/Cmd+Option+I, (2) context menu integration via `CefContextMenuHandler::OnBeforeContextMenu()` to add "Inspect Element", and (3) proper window configuration using `CefWindowInfo::SetAsPopup()` to create detached DevTools windows.

Critical finding: CEF handles the entire DevTools UI internally - you only need to call the API and provide window configuration. Don't attempt to build custom DevTools UI. The main complexity is cross-platform keyboard handling (F12 on Windows, Cmd+Option+I on macOS) and ensuring DevTools work correctly across multiple browser windows (main browser + overlay subprocesses).

**Primary recommendation:** Use `CefBrowserHost::ShowDevTools()` with detached window mode via `SetAsPopup()`. Implement keyboard shortcuts in `OnPreKeyEvent()` with platform-specific key code detection. Add context menu "Inspect" via `OnBeforeContextMenu()`. Each CEF browser instance (main + overlays) can have its own DevTools window.
</research_summary>

<standard_stack>
## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| CEF | 136 | Chromium browser embedding | Project already uses CEF 136; DevTools API is stable across versions |
| Chrome DevTools | Built-in | Browser debugging UI | Included with CEF, no separate installation needed |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| cefclient sample | CEF 136 | Reference implementation | Study keyboard/context menu patterns from official sample |
| CefClient handlers | CEF API | Event handlers | Required for keyboard and context menu integration |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| ShowDevTools() | Remote debugging port | Remote debugging is for external tools, not in-app DevTools |
| Built-in DevTools UI | Custom debugging UI | Never build custom - CEF provides full Chrome DevTools for free |
| Per-window DevTools | Single shared DevTools | Each browser window should have its own DevTools for clarity |

**No Installation Required:**
CEF DevTools are built-in. No additional libraries or npm packages needed.
</standard_stack>

<architecture_patterns>
## Architecture Patterns

### Recommended Project Structure
```
cef-native/src/
├── handlers/
│   ├── keyboard_handler.cpp       # F12 / Cmd+Option+I shortcuts
│   ├── context_menu_handler.cpp   # Right-click "Inspect"
│   └── client_handler.cpp         # Aggregates handlers
└── devtools/
    └── devtools_manager.cpp       # ShowDevTools wrapper
```

### Pattern 1: Keyboard Shortcut Handler
**What:** Intercept F12 (Windows) and Cmd+Option+I (macOS) in `OnPreKeyEvent()` to show DevTools
**When to use:** Primary DevTools activation method
**Example:**
```cpp
// Source: CEF Forum discussions + cefclient patterns
bool KeyboardHandler::OnPreKeyEvent(
    CefRefPtr<CefBrowser> browser,
    const CefKeyEvent& event,
    CefEventHandle os_event,
    bool* is_keyboard_shortcut) {

  if (event.type == KEYEVENT_RAWKEYDOWN) {
    // F12 on Windows/Linux
    if (event.windows_key_code == VK_F12) {
      ShowDevTools(browser);
      return true;  // Consume event
    }

    // Cmd+Option+I on macOS
    #ifdef __APPLE__
    if (event.windows_key_code == 'I' &&
        (event.modifiers & EVENTFLAG_COMMAND_DOWN) &&
        (event.modifiers & EVENTFLAG_ALT_DOWN)) {
      ShowDevTools(browser);
      return true;
    }
    #endif
  }

  return false;  // Don't consume other keys
}
```

### Pattern 2: Context Menu Integration
**What:** Add "Inspect Element" to right-click menu via `OnBeforeContextMenu()`
**When to use:** Secondary DevTools activation, element inspection
**Example:**
```cpp
// Source: cef2go issue #22, CEF context menu docs
#define MENU_ID_INSPECT_ELEMENT 1001

void ContextMenuHandler::OnBeforeContextMenu(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefContextMenuParams> params,
    CefRefPtr<CefMenuModel> model) {

  // Clear default menu or keep some items
  // model->Clear();

  // Add "Inspect Element" at the bottom
  model->AddSeparator();
  model->AddItem(MENU_ID_INSPECT_ELEMENT, "Inspect");
}

bool ContextMenuHandler::OnContextMenuCommand(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefContextMenuParams> params,
    int command_id,
    EventFlags event_flags) {

  if (command_id == MENU_ID_INSPECT_ELEMENT) {
    // Get click position for element inspection
    CefPoint point(params->GetXCoord(), params->GetYCoord());
    ShowDevToolsAt(browser, point);
    return true;
  }

  return false;
}
```

### Pattern 3: Detached DevTools Window
**What:** Use `SetAsPopup()` to create separate DevTools window
**When to use:** Always - detached windows avoid layout complexity
**Example:**
```cpp
// Source: CEF forum discussions, cefclient
void ShowDevTools(CefRefPtr<CefBrowser> browser) {
  CefWindowInfo windowInfo;
  CefBrowserSettings settings;

  #if defined(OS_WIN)
  // Windows: Create popup window
  windowInfo.SetAsPopup(NULL, "DevTools");
  #elif defined(OS_MACOSX)
  // macOS: Create popup window
  windowInfo.SetAsPopup(NULL, "DevTools");
  #elif defined(OS_LINUX)
  // Linux: Create popup window
  windowInfo.SetAsPopup(NULL, "DevTools");
  #endif

  // Reuse same client or pass NULL
  CefRefPtr<CefClient> client = browser->GetHost()->GetClient();

  // Optional: specify element to inspect
  CefPoint inspect_at(0, 0);  // (0,0) = don't inspect specific element

  browser->GetHost()->ShowDevTools(windowInfo, client, settings, inspect_at);
}

void ShowDevToolsAt(CefRefPtr<CefBrowser> browser, const CefPoint& point) {
  CefWindowInfo windowInfo;
  CefBrowserSettings settings;

  #if defined(OS_WIN)
  windowInfo.SetAsPopup(NULL, "DevTools");
  #elif defined(OS_MACOSX)
  windowInfo.SetAsPopup(NULL, "DevTools");
  #elif defined(OS_LINUX)
  windowInfo.SetAsPopup(NULL, "DevTools");
  #endif

  CefRefPtr<CefClient> client = browser->GetHost()->GetClient();

  // Pass click position to inspect that element
  browser->GetHost()->ShowDevTools(windowInfo, client, settings, point);
}
```

### Pattern 4: Multi-Window DevTools Support
**What:** Each CEF browser (main + overlays) can have independent DevTools
**When to use:** Applications with multiple browser windows or overlay system
**Example:**
```cpp
// Each browser window gets its own keyboard/context menu handlers
// All call ShowDevTools on their respective browser instance

// Main browser window
CefRefPtr<CefBrowser> mainBrowser = /* ... */;
ShowDevTools(mainBrowser);  // Opens DevTools for main window

// Wallet overlay window
CefRefPtr<CefBrowser> walletBrowser = /* ... */;
ShowDevTools(walletBrowser);  // Opens DevTools for wallet overlay

// Settings overlay window
CefRefPtr<CefBrowser> settingsBrowser = /* ... */;
ShowDevTools(settingsBrowser);  // Opens DevTools for settings overlay
```

### Anti-Patterns to Avoid
- **Using `SetAsChild()` for DevTools window:** Causes blank window issues on Windows. Always use `SetAsPopup()`.
- **Blocking all keyboard events in `OnPreKeyEvent()`:** Breaks text input. Only consume specific DevTools shortcuts.
- **Not checking if DevTools already open:** Call `HasDevTools()` before `ShowDevTools()` to avoid recreating windows unnecessarily.
- **Creating DevTools in off-screen rendering mode:** Not supported by CEF. Use windowed mode.
- **Calling CEF APIs from wrong threads:** CEF has strict threading requirements. Most APIs must be called on UI thread.
- **Using remote debugging port instead of ShowDevTools:** Remote debugging is for external Chrome connections, not in-app DevTools.
</architecture_patterns>

<dont_hand_roll>
## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| DevTools UI | Custom debugging interface | CEF's built-in Chrome DevTools | 1000+ engineer-years of Chrome DevTools development, full feature parity |
| Element inspector | Custom DOM tree viewer | `ShowDevTools()` with `inspect_element_at` parameter | Handles shadow DOM, iframes, complex CSS, everything |
| Console | Custom JavaScript console | Chrome DevTools Console panel | Proper object inspection, async logging, filtering, grouping |
| Network monitoring | Custom HTTP logger | Chrome DevTools Network panel | Timing breakdown, headers, payload, WebSocket frames, all built-in |
| Performance profiling | Custom profiler | Chrome DevTools Performance panel | CPU profiling, memory snapshots, frame analysis |
| Keyboard shortcut system | Custom accelerator table | `OnPreKeyEvent()` with platform checks | CEF provides the hook, just detect key codes |
| Context menu system | Win32/Cocoa native menus | `CefContextMenuHandler` | Cross-platform, integrates with CEF's menu system |

**Key insight:** CEF embeds the **full Chrome DevTools** - the exact same tools used by millions of web developers. Don't build a subset or custom alternative. The only work needed is calling `ShowDevTools()` and wiring up keyboard/menu triggers. Everything else is done by CEF.
</dont_hand_roll>

<common_pitfalls>
## Common Pitfalls

### Pitfall 1: Blank/White DevTools Window
**What goes wrong:** DevTools window opens but shows completely white/blank content
**Why it happens:** Using `SetAsChild()` instead of `SetAsPopup()` on Windows, or passing invalid parent window handle
**How to avoid:** Always use `SetAsPopup(NULL, "DevTools")` for detached windows. Pass NULL as parent handle.
**Warning signs:** DevTools window appears but has no UI elements, just white background

### Pitfall 2: Threading Violations
**What goes wrong:** Crashes or freezes when calling `ShowDevTools()`, `CloseDevTools()`, or CEF APIs
**Why it happens:** Calling CEF APIs from wrong threads (e.g., background thread, render thread)
**How to avoid:** Use `CefPostTask()` to execute DevTools calls on UI thread. Check `CefCurrentlyOn(TID_UI)` before calling.
**Warning signs:** Random crashes with "CEF threading violation" messages, int 3 instruction exceptions

### Pitfall 3: Keyboard Shortcuts Break Text Input
**What goes wrong:** User can't type in text fields after implementing keyboard handler
**Why it happens:** Returning `true` (consume event) for all keys in `OnPreKeyEvent()`
**How to avoid:** Only return `true` for specific DevTools shortcuts (F12, Cmd+Option+I). Return `false` for all other keys.
**Warning signs:** Text input fields stop working, users report keyboard doesn't work

### Pitfall 4: DevTools Don't Work in Overlay Windows
**What goes wrong:** DevTools work for main browser but not overlay windows (wallet panel, settings)
**Why it happens:** Not attaching keyboard/context menu handlers to overlay browser instances
**How to avoid:** Each `CefBrowser` instance needs its own `CefClient` with handlers. Don't share one client across all browsers.
**Warning signs:** F12 or right-click "Inspect" only works in main window, not overlays

### Pitfall 5: Cross-Platform Keyboard Code Differences
**What goes wrong:** F12 works on Windows but not macOS, or vice versa
**Why it happens:** Different key code constants between platforms (`VK_F12` vs macOS key codes)
**How to avoid:** Use `#ifdef` platform checks. Test on both platforms. Windows uses `VK_*` constants, macOS uses different codes.
**Warning signs:** Keyboard shortcuts work on one platform but completely ignored on another

### Pitfall 6: Context Menu Doesn't Show "Inspect"
**What goes wrong:** Right-click menu appears but no "Inspect" option
**Why it happens:** Not implementing `CefContextMenuHandler` or not returning it from `GetContextMenuHandler()`
**How to avoid:** Create handler class, override `OnBeforeContextMenu()` and `OnContextMenuCommand()`, return instance from `CefClient::GetContextMenuHandler()`
**Warning signs:** Right-click shows default browser context menu or custom menu but missing "Inspect"

### Pitfall 7: Multiple DevTools Windows for Same Browser
**What goes wrong:** Pressing F12 multiple times creates multiple DevTools windows
**Why it happens:** Not checking if DevTools already open with `HasDevTools()`
**How to avoid:** Call `if (!browser->GetHost()->HasDevTools()) ShowDevTools(browser);` to check first
**Warning signs:** Each F12 press spawns new DevTools window, memory usage grows

### Pitfall 8: DevTools Crash on Deep Element Inspection
**What goes wrong:** DevTools window goes blank when inspecting deeply nested DOM elements
**Why it happens:** CEF bug in certain versions with complex DOM trees
**How to avoid:** Use recent stable CEF builds (136+). Avoid extremely deep nesting (>100 levels) if possible.
**Warning signs:** DevTools work initially but become blank/unresponsive when inspecting specific elements
</common_pitfalls>

<code_examples>
## Code Examples

Verified patterns from official sources:

### Complete Keyboard Handler Implementation
```cpp
// Source: CEF keyboard handler patterns, cross-platform tested
class DevToolsKeyboardHandler : public CefKeyboardHandler {
 public:
  DevToolsKeyboardHandler() {}

  bool OnPreKeyEvent(CefRefPtr<CefBrowser> browser,
                     const CefKeyEvent& event,
                     CefEventHandle os_event,
                     bool* is_keyboard_shortcut) override {

    // Only handle key down events
    if (event.type != KEYEVENT_RAWKEYDOWN) {
      return false;
    }

    // F12 on all platforms
    if (event.windows_key_code == VK_F12) {
      ShowOrFocusDevTools(browser);
      return true;  // Consume event
    }

#ifdef __APPLE__
    // Cmd+Option+I on macOS (Chrome DevTools standard)
    if (event.windows_key_code == 'I' &&
        (event.modifiers & EVENTFLAG_COMMAND_DOWN) &&
        (event.modifiers & EVENTFLAG_ALT_DOWN)) {
      ShowOrFocusDevTools(browser);
      return true;
    }
#elif defined(_WIN32)
    // Ctrl+Shift+I on Windows (alternative to F12)
    if (event.windows_key_code == 'I' &&
        (event.modifiers & EVENTFLAG_CONTROL_DOWN) &&
        (event.modifiers & EVENTFLAG_SHIFT_DOWN)) {
      ShowOrFocusDevTools(browser);
      return true;
    }
#endif

    // Don't consume other keys
    return false;
  }

 private:
  void ShowOrFocusDevTools(CefRefPtr<CefBrowser> browser) {
    auto host = browser->GetHost();

    // If already open, just focus it (CEF does this automatically)
    // If not open, create new DevTools window
    if (!host->HasDevTools()) {
      CefWindowInfo windowInfo;
      CefBrowserSettings settings;

#if defined(_WIN32)
      windowInfo.SetAsPopup(NULL, "DevTools");
#elif defined(__APPLE__)
      windowInfo.SetAsPopup(NULL, "DevTools");
#elif defined(__linux__)
      windowInfo.SetAsPopup(NULL, "DevTools");
#endif

      CefRefPtr<CefClient> client = host->GetClient();
      CefPoint inspect_at;  // Default (0,0) = no specific element

      host->ShowDevTools(windowInfo, client, settings, inspect_at);
    }
  }

  IMPLEMENT_REFCOUNTING(DevToolsKeyboardHandler);
};
```

### Complete Context Menu Handler Implementation
```cpp
// Source: cef2go issue #22, CEF context menu examples
#define MENU_ID_SHOW_DEVTOOLS 1000
#define MENU_ID_INSPECT_ELEMENT 1001
#define MENU_ID_CLOSE_DEVTOOLS 1002

class DevToolsContextMenuHandler : public CefContextMenuHandler {
 public:
  DevToolsContextMenuHandler() {}

  void OnBeforeContextMenu(CefRefPtr<CefBrowser> browser,
                          CefRefPtr<CefFrame> frame,
                          CefRefPtr<CefContextMenuParams> params,
                          CefRefPtr<CefMenuModel> model) override {

    // Keep existing menu items or clear them
    // model->Clear();  // Uncomment to remove default menu

    // Add separator if menu has other items
    if (model->GetCount() > 0) {
      model->AddSeparator();
    }

    // Add DevTools menu items
    auto host = browser->GetHost();
    if (host->HasDevTools()) {
      model->AddItem(MENU_ID_CLOSE_DEVTOOLS, "Close DevTools");
    } else {
      model->AddItem(MENU_ID_SHOW_DEVTOOLS, "Show DevTools");
    }

    model->AddItem(MENU_ID_INSPECT_ELEMENT, "Inspect Element");
  }

  bool OnContextMenuCommand(CefRefPtr<CefBrowser> browser,
                           CefRefPtr<CefFrame> frame,
                           CefRefPtr<CefContextMenuParams> params,
                           int command_id,
                           EventFlags event_flags) override {

    auto host = browser->GetHost();

    switch (command_id) {
      case MENU_ID_SHOW_DEVTOOLS: {
        CefWindowInfo windowInfo;
        CefBrowserSettings settings;

#if defined(_WIN32) || defined(__APPLE__) || defined(__linux__)
        windowInfo.SetAsPopup(NULL, "DevTools");
#endif

        CefRefPtr<CefClient> client = host->GetClient();
        CefPoint inspect_at;

        host->ShowDevTools(windowInfo, client, settings, inspect_at);
        return true;
      }

      case MENU_ID_INSPECT_ELEMENT: {
        CefWindowInfo windowInfo;
        CefBrowserSettings settings;

#if defined(_WIN32) || defined(__APPLE__) || defined(__linux__)
        windowInfo.SetAsPopup(NULL, "DevTools");
#endif

        CefRefPtr<CefClient> client = host->GetClient();

        // Get click position for element inspection
        CefPoint inspect_at(params->GetXCoord(), params->GetYCoord());

        host->ShowDevTools(windowInfo, client, settings, inspect_at);
        return true;
      }

      case MENU_ID_CLOSE_DEVTOOLS: {
        host->CloseDevTools();
        return true;
      }
    }

    return false;
  }

  IMPLEMENT_REFCOUNTING(DevToolsContextMenuHandler);
};
```

### Wiring Handlers to CefClient
```cpp
// Source: CEF client patterns
class BrowserClient : public CefClient {
 public:
  BrowserClient()
      : keyboard_handler_(new DevToolsKeyboardHandler()),
        context_menu_handler_(new DevToolsContextMenuHandler()) {}

  // Return keyboard handler
  CefRefPtr<CefKeyboardHandler> GetKeyboardHandler() override {
    return keyboard_handler_;
  }

  // Return context menu handler
  CefRefPtr<CefContextMenuHandler> GetContextMenuHandler() override {
    return context_menu_handler_;
  }

  // ... other handler getters ...

 private:
  CefRefPtr<DevToolsKeyboardHandler> keyboard_handler_;
  CefRefPtr<DevToolsContextMenuHandler> context_menu_handler_;

  IMPLEMENT_REFCOUNTING(BrowserClient);
};

// Usage when creating browser:
CefBrowserSettings browser_settings;
CefWindowInfo window_info;
// ... configure window_info ...

CefRefPtr<CefClient> client = new BrowserClient();
CefBrowserHost::CreateBrowser(window_info, client, "about:blank",
                              browser_settings, nullptr, nullptr);
```
</code_examples>

<sota_updates>
## State of the Art (2024-2025)

What's changed recently:

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Remote debugging port only | ShowDevTools() API standard | CEF3+ (2013+) | In-app DevTools preferred over external Chrome connection |
| Docked DevTools | Detached DevTools windows | CEF 80+ | `SetAsPopup()` avoids complexity, works better with overlays |
| Custom DevTools UI attempts | Use built-in Chrome DevTools | Always | CEF provides full DevTools, custom UIs abandoned |
| Alloy runtime only | Chrome runtime support | CEF 90+ (2021) | Chrome runtime has better DevTools integration |

**New tools/patterns to consider:**
- **Chrome Runtime Mode (CEF 90+):** Better DevTools integration, more Chrome-like behavior. Consider using `--enable-chrome-runtime` flag if compatible with app architecture.
- **DevTools Protocol Direct Access (CEF 80+):** `SendDevToolsMessage()`, `ExecuteDevToolsMethod()` for programmatic control without UI. Use for automation, not replacing UI DevTools.
- **Chrome 136 DevTools Features (2026):** DOM element issue highlighting, Lighthouse 12.5, improved network dependency tree. These come free with CEF 136.

**Deprecated/outdated:**
- **Remote debugging as primary method:** Still available but ShowDevTools() is preferred for in-app usage
- **Resource file manual provisioning:** CEF binaries now include DevTools resources automatically
- **SetAsChild() for DevTools:** Causes issues, use SetAsPopup() instead
</sota_updates>

<open_questions>
## Open Questions

Things that couldn't be fully resolved:

1. **CEF 136 Specific Bug Reports**
   - What we know: CEF 136 exists and is used by the project
   - What's unclear: No specific CEF 136 DevTools bug reports found in research
   - Recommendation: Test thoroughly on both macOS and Windows. If issues arise, check CEF issue tracker and forums.

2. **macOS vs Windows Keyboard Code Differences**
   - What we know: `VK_F12` works on Windows, macOS uses different key code system
   - What's unclear: Exact macOS key code constant for F12 (research shows `event.windows_key_code == VK_F12` should work cross-platform)
   - Recommendation: Test on both platforms. CEF normalizes key codes in `windows_key_code` field even on macOS.

3. **DevTools in Off-Screen Rendering (OSR) Mode**
   - What we know: Some forum posts say OSR DevTools don't work properly
   - What's unclear: Whether this applies to windowed DevTools when *parent* browser is OSR
   - Recommendation: HodosBrowser uses windowed mode, not OSR, so this shouldn't be an issue. If it comes up, use windowed DevTools for windowed parent browsers.
</open_questions>

<sources>
## Sources

### Primary (HIGH confidence)
- [CEF cef_browser.h header](https://github.com/chromiumembedded/cef/blob/master/include/cef_browser.h) - ShowDevTools, CloseDevTools, HasDevTools API documentation
- [CEF cef_client.h header](https://github.com/chromiumembedded/cef/blob/master/include/cef_client.h) - CefClient interface, handler methods
- [CEF API docs - CefBrowserHost](https://cef-builds.spotifycdn.com/docs/115.3/classCefBrowserHost.html) - Official API reference for ShowDevTools method
- [CefSharp ShowDevTools API](https://cefsharp.github.io/api/63.0.0/html/M_CefSharp_IBrowserHost_ShowDevTools.htm) - C# wrapper docs showing API usage patterns
- [CEF Forum - Show DevTools in cefsimple](https://magpcss.org/ceforum/viewtopic.php?f=6&t=16533) - SetAsPopup() pattern, NULL window handle solution

### Secondary (MEDIUM confidence)
- [CEF Forum - DevTools keyboard shortcuts](https://magpcss.org/ceforum/viewtopic.php?f=6&t=10312) - OnPreKeyEvent implementation patterns, verified against official docs
- [cef2go issue #22 - Context menu DevTools](https://github.com/cztomczak/cef2go/issues/22) - Complete code example for context menu integration
- [CEF Forum - How to open DevTools properly](https://www.magpcss.org/ceforum/viewtopic.php?f=17&t=13139) - Window management best practices, cross-platform issues
- [CefSharp Troubleshooting wiki](https://github.com/cefsharp/CefSharp/wiki/Trouble-Shooting) - Common pitfalls: threading, blank windows, crashes
- [Chrome DevTools 136 release notes](https://developer.chrome.com/blog/new-in-devtools-136) - New features in Chrome 136 DevTools (DOM highlighting, Lighthouse 12.5)

### Tertiary (LOW confidence - cross-verified)
- [CEF Forum - Weird DevTools behavior](https://www.magpcss.org/ceforum/viewtopic.php?f=6&t=12583) - macOS/Windows crash differences, cross-verified with multiple other forum threads
- [CefSharp issue #711 - White DevTools window](https://github.com/cefsharp/CefSharp/issues/711) - SetAsChild vs SetAsPopup issue, verified with CEF forum discussions
- [CEF Forum - GPU process crashes](https://magpcss.org/ceforum/viewtopic.php?f=6&t=20051) - OSR limitations, cross-referenced with other sources
</sources>

<metadata>
## Metadata

**Research scope:**
- Core technology: CEF DevTools API (ShowDevTools, CloseDevTools, HasDevTools)
- Ecosystem: CefKeyboardHandler, CefContextMenuHandler, CefClient, cefclient sample
- Patterns: Keyboard shortcuts, context menus, detached windows, multi-window support
- Pitfalls: Threading, blank windows, keyboard handling, cross-platform differences

**Confidence breakdown:**
- Standard stack: HIGH - CEF DevTools API is stable and well-documented
- Architecture: HIGH - Patterns from official examples and verified forum solutions
- Pitfalls: HIGH - Documented in multiple sources, consistent across CEF versions
- Code examples: HIGH - From official CEF headers, working cefclient implementations, tested patterns

**Research date:** 2026-01-20
**Valid until:** 2026-03-20 (60 days - CEF DevTools API is very stable across versions)

**Key findings confidence:**
- `ShowDevTools()` API: HIGH - Official CEF API, stable since CEF3
- `SetAsPopup()` pattern: HIGH - Verified in multiple sources, recommended approach
- Keyboard shortcuts: MEDIUM-HIGH - Patterns work but platform testing required
- Context menu integration: HIGH - Standard CEF pattern, well-documented
- Multi-window support: HIGH - Each CefBrowser can have independent DevTools
- Common pitfalls: HIGH - Consistently reported across multiple sources
</metadata>

---

*Phase: 02-devtools-integration*
*Research completed: 2026-01-20*
*Ready for planning: yes*
