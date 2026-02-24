# UX/UI Cleanup & Optimization Tracker

**Purpose**: Collect non-critical UI/UX issues and performance optimization opportunities discovered during feature sprints. Dedicate a focused sprint near production to clean up the UI, polish the UX, and optimize speed/responsiveness across the entire app.

**Priority**: Low — none of these block functionality. Address after core feature sprints are complete.

---

## 1. Overlay Panel Viewport Clamping

**Status**: Partially fixed (privacy shield + download panels clamped), needs audit of all overlays.

**Problem**: Icon-anchored overlay panels (privacy shield, downloads, notifications) can extend below the visible window area when the browser window is small. Users can't see or interact with content that's off-screen.

**Current fix**: Added `mainRect.bottom` clamping in `simple_app.cpp` (Create/Show) and `cef_browser_shell.cpp` (WM_MOVE/WM_SIZE) for privacy shield and download panels.

**Remaining work**:
- Audit ALL overlay panels (settings, wallet, notification, BRC-100 auth) for consistent viewport clamping
- Consider minimum panel height (e.g., 200px) below which content gets too cramped — may need scrollable layout
- Test at various window sizes: 800x600, 1024x768, 1366x768, 1920x1080
- Industry standard: content-driven sizing with viewport clamping (`actualHeight = min(preferredHeight, windowBottom - anchorTop - margin)`)

**Files**: `simple_app.cpp` (overlay Create/Show functions), `cef_browser_shell.cpp` (WM_MOVE/WM_SIZE handlers)

---

## 2. Privacy Shield Badge Flashing

**Status**: Open — not resolved.

**Problem**: The green dot badge on the privacy shield icon flashes every 2-3 seconds, even when no new trackers/cookies are being blocked. This is because:
- `useAdblock` polls every 2 seconds, `useCookieBlocking` polls every 3 seconds
- Each poll response triggers a React re-render
- The "settled" pattern (show dot for 5s after count increases) may be re-triggered by poll timing artifacts

**Attempted fix**: "Settled" dot pattern — dot appears when `totalBlocked` increases, auto-hides after 5 seconds. Didn't fully resolve because the polling cadence causes the count to appear to change.

**Potential solutions**:
- Only show badge when count has changed since last navigation (compare to a "baseline" captured on page load)
- Debounce badge visibility with a larger window (e.g., 10s)
- Only poll when panel is open; use IPC push notifications for badge updates instead of polling
- Simplify: static dot when count > 0 (no animation/flashing), or no dot at all

**Files**: `frontend/src/pages/MainBrowserView.tsx` (badge logic), `frontend/src/hooks/useAdblock.ts`, `frontend/src/hooks/useCookieBlocking.ts`

---

## 3. OSR Overlay Mouse Wheel Scrolling

**Status**: Fixed for privacy shield panel. Needs audit of other overlays.

**Problem**: `WM_MOUSEWHEEL` provides screen coordinates in `lParam`, but CEF expects client coordinates. Without `ScreenToClient()` conversion, mouse wheel events land at wrong positions.

**Fix applied**: Added `ScreenToClient(hwnd, &pt)` in `CookiePanelOverlayWndProc` before sending to CEF.

**Remaining work**: Verify all overlay WndProcs handle `WM_MOUSEWHEEL` correctly:
- Notification overlay
- Download panel overlay
- Settings overlay
- Wallet overlay

**Files**: `cef_browser_shell.cpp` (overlay WndProc functions)

---

## 4. Nested Scrollbars in Overlay Panels

**Status**: Fixed for privacy shield panel. Pattern to follow elsewhere.

**Problem**: When expandable sections each have their own `maxHeight + overflow: auto`, users get nested scrollbars that are confusing and hard to use.

**Solution**: Single scrollable container at the panel root level. Remove per-section `maxHeight` and `overflow` — let content expand naturally within the single scroll context.

**Pattern to audit**: Any overlay with expandable/collapsible sections (MUI Collapse, Accordion, etc.)

---

## 5. General Overlay Consistency

**Items to standardize across all overlay panels**:
- Consistent close behavior (click outside, Escape key, close button)
- Consistent header/title styling
- Consistent padding and typography scales
- Dark theme consistency (some panels may have mismatched backgrounds)
- Transition animations (show/hide) — currently none, may want subtle fade

---

## 6. Responsive Layout at Small Window Sizes

**Problem**: Browser at small sizes (< 1024px width) may have toolbar items overlapping or truncated.

**To investigate**:
- Header bar at narrow widths — do icons overlap?
- Tab bar behavior when many tabs open
- URL bar minimum width before it becomes unusable
- Overlay panels at minimum window size

---

## 7. Ad Blocking Response Filter — Buffering Latency

**Status**: Open — known trade-off, not blocking.

**Problem**: `AdblockResponseFilter` (CefResponseFilter in `simple_handler.cpp`) buffers the ENTIRE YouTube response before outputting modified data. This adds latency because the browser can't start rendering/processing until the full response is buffered and ad keys are renamed.

**Observed impact**: YouTube pages load noticeably slower. API JSON responses (40-600KB) buffer quickly, but main page HTML (200KB-1.8MB) adds visible delay.

**Potential optimizations to research**:
- **Streaming find-replace**: Instead of buffering the entire response, use a streaming approach that maintains a small overlap buffer (length of longest search string ~30 chars) and processes/outputs data as it arrives. The CEF test suite has a `FindReplaceResponseFilter` example that handles chunk-boundary matching.
- **Selective API filtering**: Currently filters all `/youtubei/` endpoints, but only `/player` and `/get_watch` consistently contain ad keys. Could narrow to just those endpoints (trade-off: YouTube may add ad data to other endpoints in the future).
- **Skip HTML filtering**: The API response filter may be sufficient on its own — YouTube's player typically uses API data over inline HTML data for ad rendering. Test whether removing the HTML filter eliminates ads or just causes a brief ad flash on initial load.
- **Content-Length pre-check**: For responses with known Content-Length > threshold (e.g., 2MB), skip filtering and rely on scriptlet injection as fallback.

**Files**: `simple_handler.cpp` (`AdblockResponseFilter` class, `CookieFilterResourceHandler::GetResourceResponseFilter`)

---

## 8. Duplicate Scriptlet Pre-Caching

**Status**: Open — harmless, low priority.

**Problem**: Some YouTube pages get scriptlets pre-cached 2-3x in quick succession (13ms apart). Both `OnBeforeBrowse` and `OnLoadingStateChange(isLoading=true)` fire for the same full navigation, each making a sync HTTP call to localhost:3302 and sending an IPC to the renderer.

**Impact**: Minimal — each extra call is <10ms localhost HTTP + negligible IPC. The renderer's `s_scriptCache` overwrites the same key.

**Fix if needed**: Track last pre-cached URL+timestamp in a member variable; skip if same URL was pre-cached within 100ms.

**Files**: `simple_handler.cpp` (`OnBeforeBrowse`, `OnLoadingStateChange`)

---

## 9. General Page Load Performance Audit

**Status**: Not started — research needed for optimization sprint.

**Areas to investigate**:
- **Sync WinHTTP calls on UI thread**: `OnBeforeBrowse` makes a sync HTTP call to fetch scriptlets. This blocks the UI thread during navigation (~5-10ms typically, 3s timeout). Consider async pre-fetch or caching at the browser-process level.
- **Sync WinHTTP calls on IO thread**: `AdblockCache::check()` makes sync HTTP calls for every uncached URL. High cache-hit rate mitigates this, but first-visit to a new domain triggers many cache misses in parallel.
- **CefResponseFilter overhead**: Even when no replacements are made, the buffering adds latency. Profile whether a pass-through filter adds measurable overhead vs no filter.
- **React polling intervals**: `useAdblock` polls every 2s, `useCookieBlocking` every 3s. Consider event-driven push (IPC from C++ when counts change) instead of polling.
- **Overlay pre-creation**: Notification overlay pre-creates after 2s delay. Profile whether this affects initial page load.

---

## Testing Checklist (for the cleanup & optimization sprint)

### UX/UI
- [ ] Resize browser to 800x600 — verify all overlays fit
- [ ] Resize browser to 1920x1080 — verify overlays aren't tiny
- [ ] Open each overlay type and verify mouse wheel scrolls
- [ ] Navigate to tracker-heavy site — verify badge doesn't flash incessantly
- [ ] Click outside each overlay type — verify it dismisses
- [ ] Press Escape in each overlay — verify it dismisses
- [ ] Open overlay, resize window — verify panel repositions correctly
- [ ] Test with 10+ tabs open — verify header doesn't break

### Performance
- [ ] Time YouTube page load with response filter vs without — measure added latency
- [ ] Profile sync HTTP call frequency during typical browsing session
- [ ] Measure cache hit rate on `AdblockCache` after 5 minutes of browsing
- [ ] Compare page load times on ad-heavy sites (Forbes, CNN) vs Chrome/Brave
- [ ] Profile memory usage with multiple YouTube tabs open (response filter buffers)
