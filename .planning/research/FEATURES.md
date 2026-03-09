# Feature Landscape

**Domain:** macOS CEF overlay porting (Windows parity for dropdown/popup overlay windows)
**Researched:** 2026-03-09

## Table Stakes

Features users expect from macOS overlay panels. Missing any of these and macOS users will perceive the app as broken or non-native.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Click-outside dismissal | Apple HIG: popovers close when users click outside bounds. Every native macOS app does this. Users will be confused if overlays stay open. | Medium | Use `NSEvent addLocalMonitorForEventsMatchingMask:` for in-app clicks. Also observe `NSApplicationDidResignActiveNotification` for app-level focus loss (clicking another app). Local monitors cannot detect clicks sent to other apps. |
| Escape key closes overlay | macOS convention: Escape dismisses transient UI (popovers, sheets, panels). Users instinctively press Escape to dismiss any floating panel. | Low | Forward Escape key event from NSEvent monitor or overlay WndProc to hide function. Notification overlay is exempt (requires explicit approve/deny). |
| Cmd instead of Ctrl for shortcuts | macOS users never use Ctrl for app shortcuts. Cmd+T, Cmd+W, Cmd+L, etc. are muscle memory. Using Ctrl feels immediately wrong. | Low | Already planned: `HODOS_MOD_FLAG` macro. DevTools must be Cmd+Option+I (not Cmd+Shift+I). Cmd+H is "Hide" on macOS (system-level), so History shortcut may conflict. |
| Retina/HiDPI rendering | Every modern Mac has a Retina display (2x or 3x). Blurry overlays are immediately noticeable and look amateurish. | Medium | CEF OSR rendering must account for `backingScaleFactor`. The `MyOverlayRenderHandler` needs to render at device pixel resolution and the overlay NSView must use `convertRectToBacking:` for proper coordinates. Verify paint buffer dimensions match backing store. |
| Correct overlay positioning (Cocoa Y-axis) | Cocoa uses bottom-left origin. Overlays positioned with Windows math (top-left origin) will appear in wrong locations -- often off-screen or flipped. | Medium | All 6 dropdown overlays need Y-axis conversion: `overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - headerOffset`. Already handled correctly in Settings overlay reference implementation. |
| Overlay anchored to triggering icon | Apple HIG: popover should point "as directly as possible to the element that revealed it." Dropdown appearing disconnected from its icon looks broken. | Low | Icon right-offset already passed via IPC parameter. Retina scaling factor must be applied to icon offset values since React reports logical pixels. |
| Keyboard input in OSR overlays | Omnibox and Profile Panel have text fields. If typing doesn't work, these overlays are non-functional. | High | CEF OSR on macOS requires manual keyboard forwarding (NSEventTypeKeyDown/Up/FlagsChanged -> CefKeyEvent -> SendKeyEvent). Reference pattern exists in WalletOverlayWindow class but has known crash issues (CEF issue #3666). Must handle IME for non-Latin keyboards. |
| Cmd+V paste in text fields | macOS users paste with Cmd+V exclusively. If paste doesn't work in Omnibox or Profile name input, the overlay is broken. | Medium | Requires `javascript_access_clipboard = STATE_ENABLED` and `javascript_dom_paste = STATE_ENABLED` in browser settings. Also need NSPasteboard integration in the keyboard forwarding layer. Already enabled in Settings overlay reference. |
| Cmd+A select-all in text fields | Standard text editing shortcut. Users expect it in any input field. | Low | Comes for free if keyboard forwarding is correct. Verify CefKeyEvent modifier flags map Cmd to the correct CEF modifier. |
| Overlay moves with main window | If user moves the main window, overlays must follow. Orphaned floating overlays look like bugs. | Low | Using `addChildWindow:ordered:NSWindowAbove` on `g_main_window` handles this automatically. Already implemented in Settings overlay. Critical: all overlays must be child windows. |
| Download panel shows real downloads | Users expect to see active downloads with progress, pause/resume/cancel, and open/show-in-folder. | Low | CEF's CefDownloadHandler is cross-platform. The React frontend and IPC are already cross-platform. Only the overlay window container needs porting. |
| Notification overlay for BRC-100 auth | When a site requests authentication, a notification must appear. Without this, the core BSV authentication flow is broken. | Medium | Different lifecycle from dropdown overlays: not triggered by icon click, positioned top-right (not anchored to icon), closes via approve/deny buttons (not click-outside). Must handle Y-axis for top-right positioning. |

## Differentiators

Features that add polish beyond minimum viability. Not having these won't break the experience, but having them makes the app feel native.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Overlay entrance/exit animation | Native macOS apps animate popover appearance (fade in, slight scale). Instant show/hide feels jarring compared to Safari, Chrome, Firefox. | Medium | Use `NSAnimationContext` with `setDuration:0.15` and animate alpha from 0->1 and frame scale. Respect system "Reduce Motion" preference via `NSWorkspace.shared.accessibilityDisplayShouldReduceMotion`. |
| Shadow on overlay windows | macOS panels typically have a subtle drop shadow. The reference Settings overlay sets `setHasShadow:NO` -- adding shadows makes overlays feel more grounded. | Low | Single line: `[overlayWindow setHasShadow:YES]`. Test that shadow doesn't clip or cause rendering artifacts with CEF OSR transparent backgrounds. |
| Overlay repositions on window resize | When main window is resized, dropdown overlays should reposition to stay anchored to their icon. Currently overlays are positioned at creation time only. | Medium | Listen for `NSWindowDidResizeNotification` on `g_main_window` and recalculate overlay frame. Since overlays are child windows they move with the parent, but icon-relative positioning gets stale on resize. |
| Screen edge clamping | If the triggering icon is near the screen edge, the overlay should shift to stay fully visible on screen rather than clipping off-screen. | Low | Clamp overlay frame to `[[NSScreen mainScreen] visibleFrame]`. Already noted in Track B doc for Cookie Panel. Apply to all overlays. |
| Multi-monitor awareness | Overlay should appear on the correct screen when main window spans or moves between monitors. | Low | Using `[g_main_window frame]` for positioning (not `[NSScreen mainScreen]`) handles this. The child window relationship also ensures correct screen. Verify with `backingScaleFactor` which can differ per screen. |
| Smooth keyboard focus transition | When overlay opens, focus should move to it. When it closes, focus should return to the main browser. Avoids focus getting "lost." | Medium | On show: `[overlayWindow makeKeyAndOrderFront:nil]` + `browser->GetHost()->SetFocus(true)`. On hide: `[g_main_window makeKeyWindow]`. Must not steal focus from main window when showing non-interactive overlays (Menu, Cookie Panel). |
| Cmd+, opens Settings | macOS convention: Cmd+Comma opens Preferences/Settings. Chrome, Safari, Firefox all support this. | Low | Already in the shortcut plan (Task 1). Just map to the existing settings overlay show IPC. |
| Context-aware Cmd+H handling | Cmd+H is macOS system "Hide Application." If we bind it to History, it conflicts. Need to either use a different shortcut or let the system handle it. | Low | Recommendation: Do NOT override Cmd+H on macOS. Use the Menu overlay's History item instead. Let macOS handle Cmd+H as "Hide." This is what Chrome does. |

## Anti-Features

Features to explicitly NOT build. These would be wrong for this port or actively harmful.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Native NSPopover for overlays | NSPopover is the "right" macOS component for popovers, but it cannot host a CEF OSR browser. The entire overlay architecture is NSWindow-based with CEF OSR rendering. Switching to NSPopover would require a complete architecture change. | Continue using borderless NSWindow + OSR CEF browser. This is the established pattern that works on Windows and already has a working macOS reference (Settings overlay). |
| Native macOS menu bar integration | macOS apps have a system menu bar. Adding Hodos menu items there would be a large scope expansion beyond parity and would create a second menu system alongside the in-browser three-dot menu. | Keep the three-dot menu as the primary menu, matching Windows behavior. The macOS menu bar already has the standard App menu (Quit, Hide, etc.) which is sufficient. |
| Touch Bar support | Touch Bar is deprecated (removed from all current Macs). Building for it is wasted effort. | Do not implement. No modern Mac has a Touch Bar. |
| Window-level modal overlays | Making overlays modal (blocking interaction with main window) would feel wrong. macOS users expect popovers to be non-modal and dismissible. | Keep overlays non-modal. Exception: Notification overlay should demand attention but still allow the user to interact with the browser (they might need to check the URL). |
| Detachable popovers | Apple HIG mentions detachable popovers (drag to become separate panel). This is complex and provides no value for browser chrome overlays that are small and transient. | Do not implement. Overlays should be anchored and non-draggable. |
| Global mouse monitoring | `NSEvent addGlobalMonitorForEventsMatchingMask:` can detect clicks in other apps. This requires accessibility permissions and feels invasive. | Use local monitors (`addLocalMonitorForEventsMatchingMask:`) for in-app clicks. For app-level deactivation, observe `NSApplicationDidResignActiveNotification`. This covers all cases without needing special permissions. |
| Overriding Cmd+H for History | Cmd+H is a system-level shortcut for "Hide Application" on macOS. Overriding it breaks a deeply ingrained user expectation and may not even work reliably. | Let macOS handle Cmd+H natively. Access History via the three-dot menu or consider an alternative shortcut (Cmd+Y, used by Chrome/Safari for history). |
| Custom overlay chrome/titlebar | Adding custom close buttons, drag handles, or titlebars to overlay windows. These are transient dropdowns, not persistent panels. | Keep overlays borderless and chrome-free, matching the Windows implementation. Close via click-outside or Escape. |

## Feature Dependencies

```
Keyboard Shortcuts (Cmd mapping) -- independent, no overlay dependency

Click-Outside System (NSEvent monitors)
  |-- Menu Overlay (no keyboard needed)
  |-- Cookie Panel Overlay (no keyboard needed)
  |-- Download Panel Overlay (no keyboard needed)
  |-- Omnibox Overlay (requires keyboard forwarding)
  |-- Profile Panel Overlay (requires keyboard forwarding + clipboard)
  |-- Notification Overlay (may not use click-outside -- closes via buttons)

Keyboard Forwarding (NSEventTypeKeyDown -> CefKeyEvent -> SendKeyEvent)
  |-- Omnibox Overlay (typing URLs/searches)
  |-- Profile Panel Overlay (typing profile names)
  |-- Wallet Overlay fix (existing code crashes -- CEF issue #3666)

Retina Support (backingScaleFactor)
  |-- All overlays (positioning accuracy)
  |-- MyOverlayRenderHandler (paint buffer dimensions)

Escape-to-Close
  |-- All dropdown overlays (Menu, Cookie, Download, Omnibox, Profile)
  |-- NOT Notification overlay (must approve/deny explicitly)

Animation (optional polish)
  |-- All overlays (but can be added incrementally after functional parity)
```

## MVP Recommendation

Prioritize in this order:

1. **Keyboard shortcuts (Cmd mapping)** -- independent, quick win, immediately makes the app feel native. Avoid Cmd+H conflict by not binding it.
2. **Click-outside detection system** -- foundation for all 6 dropdown overlays. Build once as reusable helper, validate with simplest overlay (Menu).
3. **Wallet overlay crash fix** -- existing keyboard forwarding pattern is the reference for Omnibox and Profile Panel. Must work before building those.
4. **Menu overlay** -- simplest overlay (no keyboard input). Validates the full pattern: NSWindow creation, positioning, click-outside, IPC.
5. **Download + Cookie Panel overlays** -- next simplest (no keyboard input). Cookie Panel stubs adblock state.
6. **Omnibox overlay** -- requires keyboard forwarding. Test thoroughly: typing, backspace, Cmd+A, Cmd+V, Escape.
7. **Profile Panel overlay** -- requires keyboard + clipboard. Similar to Omnibox but with profile creation flow.
8. **Notification overlay** -- unique lifecycle (not icon-triggered, button-close instead of click-outside). Do last.

Defer:
- **Animations**: Add after all overlays are functional. Polish pass, not MVP blocker.
- **Overlay reposition on resize**: Nice-to-have. Child windows move with parent automatically; icon-relative positioning getting stale on resize is a minor visual issue.
- **Screen edge clamping**: Low risk on most setups. Add if testing reveals off-screen clipping.

## Sources

- [Apple HIG: Popovers](https://developer.apple.com/design/human-interface-guidelines/popovers) -- MEDIUM confidence (could not render JS-heavy page, but cross-referenced with search results)
- [NSEvent addLocalMonitorForEventsMatchingMask: Apple Docs](https://developer.apple.com/documentation/appkit/nsevent/1534971-addlocalmonitorforeventsmatching) -- HIGH confidence
- [NSEvent addGlobalMonitorForEventsMatchingMask: Apple Docs](https://developer.apple.com/documentation/appkit/nsevent/1535472-addglobalmonitorforeventsmatchin) -- HIGH confidence
- [Apple: Monitoring Events](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html) -- HIGH confidence
- [NSWindow backingScaleFactor Apple Docs](https://developer.apple.com/documentation/appkit/nswindow/1419459-backingscalefactor) -- HIGH confidence
- [CEF Issue #3666: SendKeyEvent crashing on macOS](https://github.com/chromiumembedded/cef/issues/3666) -- HIGH confidence (known bug affecting this project)
- [CEF Forum: Keyboard not working with OSR](https://magpcss.org/ceforum/viewtopic.php?f=6&t=16583) -- MEDIUM confidence
- [Apple: APIs for Supporting High Resolution](https://developer.apple.com/library/archive/documentation/GraphicsAnimation/Conceptual/HighResolutionOSX/APIs/APIs.html) -- HIGH confidence
- Track B project doc (`development-docs/Final-MVP-Sprint/macos-port/Track-B-UI-Overlays.md`) -- HIGH confidence (internal)
- Existing macOS implementation (`cef_browser_shell_mac.mm`) -- HIGH confidence (internal, reference code)
