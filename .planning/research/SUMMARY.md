# Project Research Summary

**Project:** Hodos Browser -- macOS UI & Overlay Parity
**Domain:** macOS Cocoa porting of CEF off-screen-rendered overlay windows
**Researched:** 2026-03-09
**Confidence:** HIGH

## Executive Summary

This project ports 6 Windows overlay windows (Menu, Cookie Panel, Download Panel, Omnibox, Profile Panel, Notification) to macOS using Cocoa/AppKit APIs with CEF off-screen rendering, plus remapping keyboard shortcuts from Ctrl to Cmd. The stack is fixed (CEF + Cocoa + React), so this is an API mapping and pattern replication problem, not a technology selection problem. The existing codebase already has 5 working macOS overlays (Settings, Wallet, Backup, BRC-100 Auth, Settings Menu), providing proven reference implementations for both overlay categories.

The critical architectural insight -- discovered during wallet overlay development and documented in codebase comments at line 1117 of `cef_browser_shell_mac.mm` -- is that macOS child windows cannot become key windows. This forces a two-category split: display-only overlays use `addChildWindow:ordered:` (auto-positioning, no keyboard), while keyboard-input overlays use independent floating windows with `NSFloatingWindowLevel` and manual position syncing. This distinction is the single most important design decision and must be applied correctly per-overlay or keyboard input silently fails with no error.

The previously highest-risk item -- CEF `SendKeyEvent` crash (issue #3666) -- is verified resolved in CEF M145+. The remaining risks are: (1) incomplete `CefKeyEvent` field mapping breaking non-printable keys and Cmd shortcuts, (2) Cocoa Y-axis coordinate flips causing mispositioned overlays, (3) an existing `GetScreenPoint` Y-coordinate bug in the render handler affecting popup positioning, and (4) NSWindow lifecycle crashes from premature destruction. All have known prevention strategies. The main mitigation is building shared infrastructure (generic overlay view base class, keyboard event mapper, click-outside helper) before porting individual overlays.

## Key Findings

### Recommended Stack

The stack is CEF 136 + Cocoa/AppKit + React, all already in use. No new dependencies needed. All required APIs are stable (most since macOS 10.6+) and have working reference code in the existing 5 macOS overlays.

**Core technologies:**
- `NSWindow` (borderless): Overlay container -- two variants: plain for display-only, custom `KeyboardOverlayWindow` subclass (overrides `canBecomeKeyWindow:YES`) for text input
- `NSEvent addLocalMonitorForEventsMatchingMask:`: Click-outside dismissal -- replaces Windows `WH_MOUSE_LL` hooks, app-local only, no special permissions needed
- `CALayer` + `MyOverlayRenderHandler`: CEF OSR rendering pipeline -- BGRA pixel buffers to CGImage, already working
- `addChildWindow:ordered:` vs `NSFloatingWindowLevel`: Window management -- child windows for non-keyboard overlays (auto-positioning), floating windows for keyboard overlays (manual sync via MainWindowDelegate)

### Expected Features

**Must have (table stakes):**
- Click-outside dismissal for all dropdown overlays
- Escape key closes overlays (except Notification which requires approve/deny)
- Cmd instead of Ctrl for all shortcuts (Cmd+T, Cmd+W, Cmd+L, etc.)
- Retina/HiDPI rendering at correct backing scale factor
- Correct Y-axis overlay positioning (Cocoa bottom-left origin)
- Keyboard input in Omnibox and Profile Panel
- Cmd+V paste and Cmd+A select-all in overlay text fields
- Overlays move with main window
- Download panel with progress, pause/resume/cancel
- Notification overlay for BRC-100 auth flow

**Should have (differentiators):**
- Overlay entrance/exit animation (fade, subtle scale)
- Drop shadows on overlay windows
- Overlay repositions on window resize
- Screen edge clamping
- Smooth focus return to main window on overlay close
- Cmd+, opens Settings (macOS convention)

**Defer (v2+):**
- Animations (add after all overlays are functional)
- Screen edge clamping (low risk in practice)
- IME support for non-Latin keyboards

### Architecture Approach

Two-category overlay system. Display-only overlays (Menu, Cookie, Download, Notification) are child windows of `g_main_window` using `addChildWindow:ordered:NSWindowAbove` at `NSNormalWindowLevel`. Keyboard-input overlays (Omnibox, Profile Panel) are independent floating windows at `NSFloatingWindowLevel` with a custom NSWindow subclass and manual position syncing. All overlays use CEF off-screen rendering to CALayer for transparency.

**Major components:**
1. `GenericOverlayView` (new) -- shared NSView base class with mouse/keyboard/scroll forwarding, eliminating duplication across 11+ overlay views
2. `KeyboardOverlayWindow` (new) -- reusable NSWindow subclass with `canBecomeKeyWindow:YES` for Omnibox and Profile Panel
3. `InstallClickOutsideMonitor` / `RemoveClickOutsideMonitor` (new) -- reusable helper combining NSEvent local monitor with `NSApplicationDidResignActiveNotification` for app-level focus loss
4. `NSEventToCefKeyEvent` helper (new) -- shared function mapping macOS key events to CefKeyEvent with correct `windows_key_code`, `unmodified_character`, and modifier flags
5. `MainWindowDelegate` extensions -- repositioning logic for all new overlays in `windowDidMove:` and `windowDidResize:`

### Critical Pitfalls

1. **Borderless NSWindow keyboard death** -- Borderless windows refuse key status by default. Overlays with text input MUST use a custom `canBecomeKeyWindow:YES` subclass. Silent failure, no crash, no error.
2. **Child window cannot become key** -- `addChildWindow:` overlays cannot receive keyboard events on macOS (framework constraint). Keyboard overlays must be independent floating windows. Non-keyboard overlays should be child windows (floating level stays above ALL apps, which is wrong for dropdowns).
3. **Cocoa Y-axis flip** -- Origin is bottom-left. Every position calculation must flip: `overlayY = mainFrame.origin.y + mainFrame.size.height - panelHeight - headerOffset`. Build a helper and validate with the first overlay.
4. **Incomplete CefKeyEvent mapping** -- Must set `windows_key_code` and `unmodified_character` for non-printable keys (Backspace, Enter, arrows) and Cmd shortcuts. Build a shared mapping function from CEF's cefclient sample.
5. **NSWindow lifecycle crashes** -- Always set `releasedWhenClosed:NO`. Use `orderOut:` (hide) not `close` (destroy) for keep-alive overlays. Close CEF browser before NSWindow. Remove event monitors in ALL destruction paths.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 0: Pre-Port Fixes
**Rationale:** Two existing bugs affect all overlays and must be fixed first. Building on a broken foundation wastes time.
**Delivers:** Fixed `GetScreenPoint` Y-flip in render handler; wallet overlay crash diagnosis and fix.
**Addresses:** Pitfall 11 (GetScreenPoint bug affects popup positioning in ALL overlays), wallet crash (contains reference keyboard pattern needed for Omnibox/Profile).
**Avoids:** Building 6 overlays on a broken rendering foundation; proceeding to keyboard overlays without a working reference.

### Phase 1: Foundation & Shared Infrastructure
**Rationale:** All 6 overlays depend on click-outside detection, correct keyboard mapping, and a shared view base class. Building these first prevents 6x code duplication and ensures fixes propagate everywhere.
**Delivers:** Keyboard shortcut remapping (Ctrl to Cmd, History to Cmd+Y not Cmd+H), click-outside detection helper (local monitor + app resignation observer), `GenericOverlayView` base class with mouse/keyboard/scroll forwarding, `NSEventToCefKeyEvent` helper, `KeyboardOverlayWindow` subclass.
**Addresses:** Cmd shortcuts (table stakes), Pitfall 14 (code duplication), Pitfall 5 (key mapping), Pitfall 7 (missing global clicks), Pitfall 8 (Cmd+H conflict), Pitfall 12 (monitor leak).

### Phase 2a: Simple Overlays (No Keyboard)
**Rationale:** Menu and Download Panel are the simplest overlays -- no keyboard input, no special state. They validate the full Pattern 1 (child window + OSR + click-outside + IPC + Y-axis positioning) with minimal risk. Lessons feed directly into subsequent overlays.
**Delivers:** Menu overlay, Download Panel overlay.
**Uses:** `GenericOverlayView` (mouse-only), `InstallClickOutsideMonitor`, Pattern 1 (child window).
**Avoids:** Pitfall 3 (build Y-axis helper here, validate), Pitfall 6 (establish lifecycle pattern), Pitfall 13 (set `acceptsMouseMovedEvents:YES`).

### Phase 2b: Medium Overlays
**Rationale:** Cookie Panel introduces Retina icon-offset math that all subsequent dropdowns will reuse. Notification has a unique lifecycle (programmatic trigger, button-close, top-right positioning) but still no keyboard input.
**Delivers:** Cookie Panel overlay, Notification overlay.
**Addresses:** Cookie blocking UI, BRC-100 auth notification (critical for BSV features).
**Avoids:** Pitfall 4 (Retina scale factor), Pitfall 9 (add scroll forwarding).

### Phase 2c: Keyboard Overlays
**Rationale:** Omnibox and Profile Panel are the hardest overlays -- they require keyboard input, clipboard access, and the floating window pattern. Omnibox first (keyboard only), Profile Panel last (keyboard + clipboard + profile creation). Both benefit from all prior pattern validation.
**Delivers:** Omnibox overlay, Profile Panel overlay.
**Uses:** `KeyboardOverlayWindow`, `NSEventToCefKeyEvent`, Pattern 2 (floating window + manual position sync).
**Avoids:** Pitfall 1 (must use key-capable window), Pitfall 2 (must NOT use child window), Pitfall 5 (must set `windows_key_code` and `unmodified_character`).

### Phase 3: Polish
**Rationale:** Animations, shadows, and edge clamping are differentiators that should only be added after all overlays are functional. Never block core functionality for polish.
**Delivers:** Overlay entrance/exit animations, drop shadows, screen edge clamping, Cmd+, for Settings.

### Phase Ordering Rationale

- Phase 0 before everything because existing bugs in `GetScreenPoint` and the wallet overlay affect all subsequent work.
- Phase 1 before any overlays because click-outside detection, shared view class, and keyboard helper are dependencies for all 6 overlays.
- Phase 2a before 2b/2c because the simplest overlays validate the pattern with lowest risk. Lessons feed forward.
- Phase 2c last among ports because keyboard overlays are highest complexity and benefit from all prior validation.
- Phase 3 after functional parity because polish should never block core functionality.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 0 (Wallet Crash):** Root cause unknown. May be lifecycle, memory, or keyboard-event-related. Needs hands-on debugging, not more research.
- **Phase 2c (Keyboard Overlays):** `windows_key_code` mapping table must be extracted from CEF's `cefclient/browser/osr_window_mac.mm`. IME for non-Latin keyboards is undocumented territory.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Foundation):** All APIs are well-documented Apple APIs with existing reference code in the codebase.
- **Phase 2a (Simple Overlays):** Direct application of Pattern 1, already working for Settings overlay.
- **Phase 2b (Medium Overlays):** Same as 2a with minor additions (Retina math, top-right positioning).
- **Phase 3 (Polish):** Standard Cocoa animation APIs, no CEF interaction.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Not a selection problem -- stack is fixed. All APIs documented and already in use in existing codebase. |
| Features | HIGH | Feature list defined by Windows parity. Apple HIG confirms macOS expectations. |
| Architecture | HIGH | Two-category overlay pattern proven by 5 existing overlays. Child-vs-floating distinction verified via Apple docs and codebase experience. |
| Pitfalls | HIGH | 15 pitfalls identified from code analysis, CEF issue tracker, and Apple docs. Existing bugs found (GetScreenPoint Y-flip, missing scroll forwarding). |

**Overall confidence:** HIGH

### Gaps to Address

- **Wallet overlay crash root cause:** Needs hands-on debugging. Verified NOT the same as CEF #3666 (resolved in M145+). Likely lifecycle or memory issue. Handle during Phase 0.
- **CefKeyEvent `windows_key_code` mapping:** Need to extract from CEF's cefclient macOS sample and build shared helper. Reference: `cefclient/browser/osr_window_mac.mm`.
- **`GetScreenPoint` Y-flip bug:** Existing bug in `my_overlay_render_handler.mm:292-303`. Must fix in Phase 0 before porting overlays.
- **Scroll event forwarding:** No existing overlay implements `scrollWheel:`. Any scrollable overlay content will not scroll. Add to `GenericOverlayView` base class.
- **IME support:** Input Method Editor for non-Latin keyboards not researched. Defer to post-MVP unless user base requires it.
- **Cmd+H conflict:** Confirmed Chrome does not bind Cmd+H. Recommend Cmd+Y for History on macOS. Decide in Phase 1.

## Sources

### Primary (HIGH confidence)
- Apple Developer Documentation: NSWindow, NSEvent, CALayer, addChildWindow, canBecomeKeyWindow, addLocalMonitorForEventsMatchingMask, Cocoa coordinate systems, backingScaleFactor
- CEF Issue #3666 (SendKeyEvent crash) -- verified resolved in M145+ by CEF maintainer
- CEF Issue #3602 (shutdown crash from zombie NSWindow)
- CEF Bitbucket Issue #2150 (macOS OSR display scale change)
- Existing codebase: `cef_browser_shell_mac.mm` (1824 lines, 5 working overlays), `my_overlay_render_handler.mm`, `simple_handler.cpp`
- Track B project doc: `development-docs/Final-MVP-Sprint/macos-port/Track-B-UI-Overlays.md`

### Secondary (MEDIUM confidence)
- CEF Forum posts on keyboard forwarding and transparent overlays
- Apple HIG: Popovers
- CocoaDev: KeyEventsInBorderlessWindow, BorderlessWindow
- Noodlesoft: Understanding Flipped Coordinate Systems

---
*Research completed: 2026-03-09*
*Ready for roadmap: yes*
