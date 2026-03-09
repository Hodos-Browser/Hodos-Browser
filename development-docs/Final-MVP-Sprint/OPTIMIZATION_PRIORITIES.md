# Optimization Priorities — Final MVP Sprint

**Purpose**: What to fix before vs. after testing, and why.

**Context**: The CEF Refinement Tracker (CR-1/CR-2/CR-3) is complete. These items come from the frontend/UX optimization audit and represent the remaining performance work.

---

## Before Testing

These are bugs or visible jank that will interfere with tester experience.

### 1. Async IPC for send_transaction (Critical)

**Problem**: Clicking "Send" in the wallet freezes the entire browser for 1-6 seconds (black screen, no feedback). The wallet overlay often closes before the success modal renders.

**Root cause**: `send_transaction` IPC handler in `simple_handler.cpp:4079` calls `walletService.sendTransaction()` — a synchronous WinHTTP POST — on the CEF UI thread. While waiting for the Rust wallet to respond, nothing renders.

**Note**: The BEEF Ancestry Cache optimization (completed 2026-03-07) should have reduced the Rust-side delay from 3-6s to under 1s for most sends. **Verify this first** — if sends now complete fast enough that the freeze is imperceptible (<300ms), this drops to post-testing priority.

**Fix**: Move WinHTTP call to background thread via `CefPostTask(TID_FILE_USER_VISIBLE, ...)`, return result via `SendProcessMessage`. The exact pattern already exists in `HttpRequestInterceptor.cpp` (CR-2.1).

**Files**:
| File | Change |
|------|--------|
| `simple_handler.cpp:4079-4160` | `send_transaction` handler → async pattern |
| `HttpRequestInterceptor.cpp` | Reference implementation (copy this pattern) |

**After send_transaction, audit these** (same issue, lower severity):

| IPC Handler | Typical Duration | Priority |
|-------------|-----------------|----------|
| `broadcast_transaction` | 1-5s (network) | High |
| `create_transaction` | Variable | Medium |
| `sign_transaction` | Variable | Medium |
| `get_balance` | 2-50ms | Low (fast enough) |

---

### 2. Polling → Push Events (Low effort, visible improvement)

**Problem**: `useAdblock.ts` and `useCookieBlocking.ts` poll via `setInterval` every 2-3 seconds. This causes the Privacy Shield badge to flicker/re-render continuously, even when nothing is happening.

**Fix**:
- C++ side: When `AdblockCache::incrementBlockedCount` is called, send an IPC message to the header browser
- React side: Replace `setInterval` with a `useEffect` listener for the IPC event

**Files**:
| File | Change |
|------|--------|
| `AdblockCache.h` | Add IPC broadcast on count increment |
| `simple_handler.cpp` or `simple_render_process_handler.cpp` | Forward count event to JS |
| `frontend/src/hooks/useAdblock.ts` | Replace polling with event listener |
| `frontend/src/hooks/useCookieBlocking.ts` | Same |

---

## After Testing (informed by tester feedback)

### 3. Streaming Response Filter (YouTube)

**Problem**: `AdblockResponseFilter` buffers the entire HTTP response before outputting it. This delays YouTube's Time to First Byte until the last byte is received.

**Why wait**: This is complex C++ (rolling window state machine) with high risk of breaking YouTube. Only worth doing if testers report YouTube feeling noticeably slower than Chrome. Current approach works — it's just not optimal.

**Reference**: CEF's `FindReplaceResponseFilter` unit test has the exact rolling-window algorithm.

### 4. Responsive Design & Layout Polish

**Problem**: Fixed pixel widths, inconsistent padding, no media queries for small windows.

**Why wait**: Testers will tell us which layouts actually break at real screen sizes. The testing guide "vibe check" ratings will surface the worst offenders. Fix what testers flag rather than guessing.

### 5. Promise-based IPC Bridge

**Problem**: React communicates with C++ via global callbacks (`window.onCookieBlocklistResponse = ...`). If two components request the same data simultaneously, the second callback overwrites the first.

**Why wait**: This is a code quality / developer ergonomics improvement, not a performance fix. The global callbacks don't add latency — they're just fragile. The race condition is theoretical in practice because overlays are isolated V8 contexts. Worth doing for codebase health, but not before MVP stabilization.

**Note**: This is NOT the same as item #1 above. Item #1 fixes the C++ UI thread blocking on WinHTTP calls. This item fixes the JavaScript callback pattern. Different layers, different problems.

---

## Already Done (for reference)

These were tracked in the CEF Refinement Tracker and are complete:

- CR-1 (7 items): JS injection fix, auth hang fixes, OnPaint buffer overflow, timeouts
- CR-2 (6 items): Async interceptor, per-request map, mutex safety, whitelist cache, thread races, raw pointer fix
- CR-3 (13/15 items): DB migration, overlay lifecycle, debug cleanup, logging, port restriction
- BEEF Ancestry Cache: Parent tx data cached at UTXO discovery time (address sync + PeerPay)
- TSC proof retry: Reduced from 2s → 500ms

---

*Last updated: 2026-03-09*
