# Frontend UI/UX Cleanup & Optimization Report

**Date**: March 5, 2026
**Project**: Hodos Browser
**Status**: Strategic Audit & Recommendations

---

## 1. Executive Summary

This document provides a comprehensive audit of the Hodos Browser frontend (React/TypeScript) and its integration with the C++ CEF Shell. It outlines optimizations for performance, consistency, and responsive design while maintaining strict security integrity (private keys never in JS).

The current architecture is robust but suffers from minor "integration friction" between the native shell and the containerized UI, specifically regarding event-driven communication and layout responsiveness across varying window sizes.

---

## 2. Technical Categorization & Deep Dive

### 2.1 Communication Patterns (IPC & JS Bridge)

#### 2.1a — Global Callback Race Conditions
**Current Implementation**: The React frontend communicates with the C++ shell via `window.cefMessage.send`. Responses are handled by assigning global callbacks to the `window` object (e.g., `window.onCookieBlocklistResponse = (data) => {...}`).
*   **Why it was built this way**: This is the simplest way to implement asynchronous communication in CEF. It avoids complex JS-side message routing and allows C++ to simply execute `window.onSomething(data)` via V8.
*   **The Problem**: Global callbacks are extremely fragile. If two React components request the cookie blocklist simultaneously, the second component will overwrite the first's callback, causing the first component to hang indefinitely.
*   **Risk of Changing (High)**: Modifying this requires coordinated changes across the React hooks and the C++ Render Process (`CefV8Handler` or IPC message handlers). Missing a single C++ callback string will break that specific UI feature.
*   **Implementation Recommendation**:
    1.  **Advantage**: Eliminates race conditions and allows standard `async/await` syntax in React.
    2.  **Disadvantage**: Requires touching dozens of files across C++ and JS.
    3.  **How to Implement**: Create a unified dispatcher in `initWindowBridge.ts`.
        ```typescript
        const pendingRequests = new Map();
        window.hodosBrowser.invoke = (action, data) => {
            return new Promise((resolve) => {
                const id = crypto.randomUUID();
                pendingRequests.set(id, resolve);
                window.cefMessage.send(action, { id, ...data });
            });
        };
        window.__hodos_ipc_resolve = (id, data) => {
            if (pendingRequests.has(id)) {
                pendingRequests.get(id)(data);
                pendingRequests.delete(id);
            }
        };
        ```
        Update the C++ side to extract the `id` from the IPC message and include it in the `__hodos_ipc_resolve` execution.

#### 2.1b — Synchronous WinHTTP Blocking the UI Thread (Discovered 2026-03-07)

**Observed Bug**: User clicks Send in wallet panel → screen goes black for 3-6 seconds → wallet overlay closes → no success/failure feedback shown.

**Root Cause Chain**:
1.  `send_transaction` IPC handler in `simple_handler.cpp:4079` runs on CEF **UI thread** (via `OnProcessMessageReceived`)
2.  Calls `walletService.sendTransaction()` → synchronous WinHTTP POST to `localhost:31301/transaction/send`
3.  Rust handler takes 3-6 seconds (BEEF ancestry chain building for unconfirmed parent txs — see note below)
4.  UI thread blocked entire time → **no rendering, no event processing** → black screen
5.  When unblocking, queued `WM_ACTIVATE` events fire → wallet overlay closes before success modal renders
6.  User gets zero feedback on transaction outcome

**The Problem**: Any IPC handler in `OnProcessMessageReceived` that calls `walletService.*` (synchronous WinHTTP) blocks the entire CEF UI thread. This affects every handler that makes a WinHTTP call to the Rust wallet, not just `send_transaction`. Short calls (<200ms) are invisible to the user; long calls (>500ms) cause visible freezing.

**IPC Handlers to Audit** (all in `simple_handler.cpp`, all call synchronous WinHTTP):

| IPC Message | WalletService Method | Typical Duration | Risk |
|-------------|---------------------|-----------------|------|
| `send_transaction` | `sendTransaction()` | 1-6s (BEEF building) | **HIGH** — causes black screen |
| `get_balance` | `getBalance()` | 2-50ms | Low |
| `create_transaction` | `createTransaction()` | variable | Medium |
| `sign_transaction` | `signTransaction()` | variable | Medium |
| `broadcast_transaction` | `broadcastTransaction()` | 1-5s (network) | **HIGH** |
| Other wallet calls | various | variable | Audit needed |

**Risk of Changing (Medium-High)**: Threading bugs are subtle. If `SendProcessMessage` is called from the wrong thread, it silently fails or crashes. The `CefRefPtr<CefBrowser>` must remain valid across thread boundaries.

**Implementation Recommendation**:
1.  **Pattern**: Move WinHTTP call to background thread via `CefPostTask(TID_FILE_USER_VISIBLE, ...)`, capture `CefRefPtr<CefBrowser>` + request ID, return result via `SendProcessMessage` from callback.
2.  **Existing precedent**: `HttpRequestInterceptor.cpp` already uses `CefPostTask` with `CefTask` subclasses for async WinHTTP work. Use same pattern.
3.  **Phased approach**: Convert `send_transaction` first (highest impact), then audit and convert remaining handlers.
4.  **V8 side**: The render process already handles async IPC responses — `SendProcessMessage(PID_RENDERER)` from a callback should resolve the JS Promise correctly.

**Key Files**:
| File | What to Change |
|------|----------------|
| `simple_handler.cpp:4079-4160` | `send_transaction` IPC → async pattern |
| `simple_handler.cpp` (other handlers) | Audit all `walletService.*` calls for >200ms duration |
| `WalletService.cpp:797` | `sendTransaction()` — the sync WinHTTP call itself |
| `HttpRequestInterceptor.cpp` | Reference implementation for CefPostTask async pattern |

**Note — Why `send_transaction` is slow**: The Rust `/transaction/send` handler builds BEEF (BRC-62) which requires a complete ancestry chain back to confirmed transactions. When the parent transaction is unconfirmed, the code fetches grandparent transactions + merkle proofs from WhatsOnChain API (multiple network round-trips). This is a separate optimization tracked outside this document — see "BEEF Ancestry Cache Optimization" in development-docs.

### 2.2 Performance & Latency (Response Filtering)
**Current Implementation**: `AdblockResponseFilter` (a `CefResponseFilter`) buffers the *entire* HTTP response for YouTube before outputting it. It does this to safely regex and rename JSON keys (e.g., `adPlacements` -> `adPlacements_`).
*   **Why it was built this way**: CEF's filter API receives data in unpredictable chunk sizes. If a target string like `"adPlacements"` spans across two chunks (`"adPlac"` in chunk 1, `"ements"` in chunk 2), a simple string replace will miss it. Buffering the whole response is a brute-force but guaranteed way to avoid missing boundary strings.
*   **The Problem**: Buffering adds significant latency (Time to First Byte is delayed until the *last* byte is received) and increases memory pressure for large HTML/JSON payloads.
*   **Risk of Changing (High)**: If a streaming filter drops characters or misaligns chunk boundaries, it will corrupt the JSON/HTML response, breaking YouTube entirely.
*   **Implementation Recommendation**:
    1.  **Advantage**: Massively improves page load times on YouTube.
    2.  **Disadvantage**: Complex C++ state machine required.
    3.  **How to Implement**: Implement a "rolling window" buffer. The filter should hold back a buffer equal to the length of the longest search string (e.g., 30 chars). As new data arrives, append it, search the window, output the safe portion, and keep the last 30 characters for the next chunk. See CEF's `FindReplaceResponseFilter` unit test for the exact algorithm.

### 2.3 React Polling (Adblock & Cookies)
**Current Implementation**: `useAdblock.ts` and `useCookieBlocking.ts` use `setInterval` to fetch updated blocked counts every 2-3 seconds.
*   **Why it was built this way**: It's the fastest way to get data into a React component without requiring the C++ backend to track which UI windows are open and push events to them.
*   **The Problem**: Causes continuous React re-renders (leading to UI flickering, like the Privacy Shield badge) and unnecessary IPC/HTTP traffic even when nothing is happening.
*   **Risk of Changing (Low)**: The UI might temporarily fail to update if the push event is missed, but it won't break core browser functionality.
*   **Implementation Recommendation**:
    1.  **Advantage**: Zero idle CPU usage; stops UI flickering.
    2.  **How to Implement**: 
        *   C++ Side: When `AdblockCache::incrementBlockedCount` is called, trigger an IPC message `broadcast_adblock_count` to all header/overlay browsers.
        *   React Side: Replace `setInterval` with a `useEffect` that listens for a custom window event dispatched by the IPC bridge.

---

## 3. UI/UX Consistency & Design Philosophy

### 3.1 Responsive Design: Pixels vs. Percentages
**The Problem**: Interfaces behave differently on different screen sizes because they often use fixed pixel widths or arbitrary percentages that don't account for aspect ratios.

**Best Practices & Recommendations**:
*   **Use `rem` / `em` over `px`**: This allows the UI to scale with the browser's base font size, which is critical for accessibility and high-DPI displays.
*   **CSS Grid & Flexbox**: Move away from absolute positioning (`top`, `left`) where possible and use `flex` for toolbars and `grid` for dashboard-style views (like the wallet history).
*   **Viewport Clamping**: Implement a unified `actualHeight = min(preferredHeight, windowBottom - anchorTop - margin)` logic in the C++ shell for ALL overlays to prevent content from falling off-screen.
*   **Media Queries**: Define standardized breakpoints (e.g., `< 1024px` for "Compact Mode") where the URL bar and toolbar icons gracefully collapse or hide labels.

### 3.2 Interaction Polish
*   **Overlay Standard**: Every overlay (`/wallet`, `/settings`, `/brc100-auth`) must implement:
    1.  **Escape key** to dismiss.
    2.  **Click-outside** to dismiss (handled in C++ via `WM_LBUTTONDOWN` detection on parent window).
    3.  **100ms Feedback**: Every button must have a visual "pressed" or "loading" state.
*   **Badge Flashing**: Resolve the `Privacy Shield` badge re-renders by comparing counts to a navigation-baseline rather than a global total.

---

## 4. Security Integrity

**Requirement**: Private keys and signing logic must remain in the Rust backend.
*   **Audit Result**: Current implementation adheres strictly to this. The UI only receives transaction metadata and public keys.
*   **Recommendation**: Ensure that sensitive modals (like `BRC100AuthModal`) use the "Keep-alive HWND" pattern to prevent "re-render flicker" that could be exploited for click-jacking or UI spoofing.

---

## 5. Implementation Roadmap & Research

### 5.1 Immediate Cleanups (Low Effort/High Impact)
- [ ] **Standardize Padding**: Align all overlays to use the brand-standard 16px/24px padding scale.
- [ ] **Typography Audit**: Ensure all UI text uses the `Inter` stack and monospace for blockchain data.
- [ ] **Icon Cleanup**: Replace the generic profile icon with the active profile's avatar/initial (as spec'd in `helper-4-branding-colors-logos.md`).

### 5.2 Areas for Research (Higher Effort)
1.  **Streaming Find-Replace**: Investigate the CEF `FindReplaceResponseFilter` example to replace the current buffering logic in `simple_handler.cpp`.
2.  **JS Bridge Refactor**: Research `window.crypto.randomUUID()` usage for creating a robust, typed IPC bridge that eliminates global `window` callbacks.
3.  **Layout Engine**: Evaluate if a "Fluid Typography" system (using `clamp()`) can solve the pixel-scaling issue more effectively than media queries.

### 5.3 Responsive Layout Benchmarks
- **Target Sizes**: 
  - **Compact**: 800x600 (Overlays must use scrollbars).
  - **Standard**: 1366x768 (Optimal).
  - **High-Res**: 1920x1080+ (Scale without looking "lost").

---

*This document replaces `ux-ui-cleanup.md`. All future UI/UX and performance tasks should be tracked here.*
