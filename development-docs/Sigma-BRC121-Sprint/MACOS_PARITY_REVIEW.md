# macOS Parity Review — BRC-121 Sprint (Phases 0–2.6)

> **Status:** ✅ COMPLETE — reviewed 2026-06-15 via an 18-agent workflow (baseline +
> 8 phase readers + 8 file auditors + adversarial verify + synthesis). One confirmed
> code gap; the rest of the sprint's C++ is already cross-platform.
> **Created:** 2026-06-15 (end of the 2.6-G session, before 2.6-H cleanup).
> **Goal:** A single authoritative gap-analysis + implementation-spec for making
> the macOS build compile and work with everything the BRC-121 sprint added.

---

## Executive summary (2026-06-15)

**The macOS parity gap for the entire BRC-121 sprint is one missing pair of calls.**

A multi-agent review audited every C++ delta across phases 0 / 0.1 / 0.2 (research,
no code) / 1 / 1.5 / 1.6 / 2 / 2.5 / 2.6, plus a Windows-only-pattern sweep of every
major C++ file (`HttpRequestInterceptor.cpp`, `simple_handler.cpp`, `simple_app.cpp`,
`cef_browser_shell.cpp`, `simple_render_process_handler.cpp`, `SessionManager`,
`PendingAuthRequest.h`, the new singletons) and the CMake source set. Raw findings: 1
candidate gap → 1 confirmed (0 spurious).

Why so clean: the sprint rode the established cross-platform rails throughout —
`SyncHttpClient` (WinHTTP/libcurl) for all wallet HTTP, `CefPostTask`/`CefTask` for
threading (never `Sleep()`/raw threads), `std::mutex`/`std::chrono` (never
`CRITICAL_SECTION`), and the **shared notification overlay**. `CreateNotificationOverlayTask`
is properly `#ifdef _WIN32` / `#elif defined(__APPLE__)` paired, and the macOS
`CreateNotificationOverlay` forwards any `type` + `extraParams` straight to React with
**no per-type allowlist** — so all the new prompt types (payment_confirmation,
certificate_disclosure, identity_key_reveal, key_linkage_reveal, the three scoped-grant
prompts, manifest_connect_bundle, domain_approval, rate_limit_exceeded) need **zero**
per-type macOS C++. Every sprint `.cpp` carrying cross-platform logic is already in the
CMake `if(APPLE)` source set, so there is **no build/compile gap**.

**The one confirmed gap (Phase 2.6-E):** the tab-close session-reset hook
(`ClearRustPaymentSessionForBrowser` + legacy `SessionManager::clearSession`) was wired
only into Windows `TabManager.cpp::CloseTab`, not macOS `TabManager_mac.mm::CloseTab`.
On macOS, closing/reopening a tab never resets Rust's per-`(browser_id,domain)` payment
counters → a domain that hit its per-session cap keeps that spent total across a tab
reopen, causing spurious `payment_confirmation` prompts (or counters that never reset).
Fix is two added calls + one `#include` (medium risk, see Gap #1).

**Everything else is runtime-verification debt, not code.** None of the macOS BRC-121
surface has ever been exercised on a Mac device (port gates #28–30 still open). The
WinHTTP↔libcurl behavioral parity, the prompt-type rendering, the gold-pill chain, and
the Phase-1 runtime risks (zstd auto-decompress, deferred `Open()` callback, `LoadURL`
reload hitting the PaidContentCache read-side) all need a **macOS smoke pass**, not new
code. See Open Questions.

---

## Why this doc exists

The existing macOS port (`development-docs/Final-MVP-Sprint/macos-port/`:
`MACOS-PORT-HANDOVER.md`, `PROGRESS.md`, `Track-B-UI-Overlays.md`) is dated
**2026-04-21** — *before* almost all of the BRC-121 sprint. Phases **1.5
(permission UX), 2 (window.CWI shim), 2.5 (IPC bridge), and 2.6 (engine→Rust)**
all landed **May–June 2026**, on top of that port baseline. None of that C++
has been built or tested on macOS.

This review captures the **macOS delta introduced by the BRC-121 sprint** —
what phases 0/1/2 changed in the C++ layer that the April port doesn't cover —
so the macOS build can be brought to parity. It is a companion to (NOT a
replacement for) the `Final-MVP-Sprint/macos-port/` tracking; cross-reference
that for the broader port status and the established macOS patterns.

**Sequencing:** done now, against the current Windows-verified tree, BEFORE
2.6-H deletes the C++ `PermissionEngine` + dead cascades. Note: the deleted
engine code does NOT need macOS porting (Rust replaced it and is already
cross-platform); the macOS-relevant C++ is the KEPT thin-proxy / overlay /
IPC code. Git history keeps the old code reachable either way.

---

## Locked approach decisions (2026-06-15)

- **Review + spec HERE (Windows); implement + test on the Mac.** Writing
  NSWindow/NSPanel/Core-Animation Objective-C++ blind on Windows (no compiler,
  no macOS CEF build, no way to verify APIs) churns. This doc is the spec: for
  each gap, the Windows reference code inline + the proposed macOS change
  (written as actual code where confident) + risk + test. The Mac session
  applies and validates it. **This doc IS the notes** that survive the C++
  deletion.
- **Scope = the C++ CEF layer.** `rust-wallet/` (incl. the 2.6-G actix
  middleware) is platform-agnostic; the React frontend is platform-agnostic.
  macOS work is almost entirely in `cef-native/`.
- **Reuse the notification overlay.** Permission prompts (payment, cert
  disclosure, identity reveal, key linkage, scoped grants, domain_approval,
  manifest_connect_bundle) multiplex through the existing `notification_browser_`
  overlay via `BRC100AuthOverlayRoot.tsx` type dispatch — which already has a
  macOS creation path. So most "new modal types" need NO new macOS overlay,
  just verification that the type/param/JS-injection path works on macOS.

---

## Methodology (for the review session)

1. Read `Final-MVP-Sprint/macos-port/` first — establish the April baseline +
   the established macOS patterns (NSWindow/NSPanel, NSWindowDelegate close,
   Core Animation OSR, libcurl via SyncHttpClient, event forwarding).
2. For each sprint phase (`phase-0*`, `phase-1*`, `phase-2*`), read the
   README + design docs AND the commits, and extract the **C++ changes**
   (ignore Rust + frontend — cross-platform).
3. Audit each major C++ file for Windows-only patterns lacking an
   `#elif defined(__APPLE__)` branch: `HttpRequestInterceptor.cpp`,
   `simple_handler.cpp`, `simple_app.cpp`, `cef_browser_shell.cpp`,
   `PendingAuthRequest.h`, and the new core singletons (DomainPermissionCache,
   SessionManager, ManifestFetcher, PaidContentCache, SubPermissionCache,
   FingerprintProtection). Confirm each macOS counterpart file
   (`cef_browser_shell_mac.mm`, `simple_handler_mac.mm`, `WindowManager_mac.mm`,
   `TabManager_mac.mm`, `WalletService_mac.cpp`, `my_overlay_render_handler.mm`,
   `OverlayHelpers_mac.mm`) covers the new code.
4. Check the macOS build wiring: does the CMake/`.mm` set include the new
   files? Will clang/Objective-C++ compile the new C++?
5. Synthesize into the gap table below.

**Optional:** run this as a multi-agent workflow (one reader per phase + one
auditor per major C++ file → synthesis). Requires explicit opt-in.

---

## Known macOS-specific touchpoints to verify (hypotheses — confirm in review)

| Area | Windows | macOS concern |
|---|---|---|
| Overlay/window lifecycle | HWND/WS_POPUP/GDI (`simple_app.cpp`, `cef_browser_shell.cpp`) | NSWindow/NSPanel + NSWindowDelegate (`cef_browser_shell_mac.mm`). Any new overlay or lifecycle change needs parity. |
| Close prevention | `WM_ACTIVATE`/`WM_ACTIVATEAPP` guards | `resignKey`/`resignMain` delegate. Verify any new guard flags. |
| OSR rendering | `UpdateLayeredWindow` | `CALayer.contents`/`CGImageCreate` (`my_overlay_render_handler.mm`) |
| HTTP to wallet | should be `SyncHttpClient` (WinHTTP) | `SyncHttpClient` (libcurl). Flag any RAW WinHTTP added this sprint. |
| CSPRNG / crypto | `CryptGenRandom`, DPAPI | `SecRandomCopyBytes`, Keychain. (FingerprintProtection + Rust dpapi already have Mac branches — confirm.) |
| File paths | `%APPDATA%\...` | `~/Library/Application Support/...`. Flag hardcoded Windows paths. |
| IPC dispatch | cross-platform CEF | should be cross-platform; confirm no Windows-only assumptions. |

---

## Gap analysis

> One confirmed code gap. Detailed spec below the summary table.

| # | Phase | File (macOS) | Gap | Risk | Status |
|---|---|---|---|---|---|
| 1 | 2.6-E | `cef-native/src/core/TabManager_mac.mm` (`CloseTab`) | Tab-close session-reset hook (`ClearRustPaymentSessionForBrowser` + `SessionManager::clearSession`) wired only on Windows; macOS `CloseTab` only calls `OnTabClosed`. Rust payment counters never reset on tab close/reopen on macOS. | medium | ✅ **fix applied on Windows branch** (mirrors Windows verbatim; **not yet compile-verified** — needs macOS build) |

### Gap #1 detail — TabManager_mac.mm::CloseTab missing session-reset

**Phase:** 2.6-E · **Risk:** medium · **macOS coverage today:** none (two separate
translation units, not a shared `#ifdef` pair, so the macOS path genuinely lacks both clears).

**Consequence:** On macOS, closing then reopening a tab on the same domain never resets
Rust's `sessionSpentCents` / `paymentCountThisSession` / 60s rate window for that
`browser_id`. A domain that hit its per-session cap can carry that spent total across a
tab close/reopen → spurious `payment_confirmation` prompts (or counters that never
reset). Violates 2.6-E done-when #9/#10. The gold-pill / `payment_success_indicator`
chain is C++-local and **unaffected** by this gap.

**Windows reference** — `cef-native/src/core/TabManager.cpp:181-187` (has all three calls):

```cpp
// (2.6-E migrated payment counters to Rust; legacy SessionManager still used by the
//  BRC-121 inline cascade, so both clears happen here in tandem.)
if (tab.browser) {
    int browserId = tab.browser->GetIdentifier();
    SessionManager::GetInstance().clearSession(browserId);
    extern void ClearRustPaymentSessionForBrowser(int);
    ClearRustPaymentSessionForBrowser(browserId);
    EphemeralCookieManager::GetInstance().OnTabClosed(browserId);
}
```

**macOS current** — `cef-native/src/core/TabManager_mac.mm:186-189` (only `OnTabClosed`):

```objc
// Notify ephemeral cookie manager before closing
if (tab.browser) {
    EphemeralCookieManager::GetInstance().OnTabClosed(tab.browser->GetIdentifier());
}
```

**Proposed macOS change** — replace the block above with the Windows version, and add
the missing include. `ClearRustPaymentSessionForBrowser` is an `extern` free function in
the cross-platform `HttpRequestInterceptor.cpp` (in the APPLE source list; impl at
`HttpRequestInterceptor.cpp:1449` → `fireSessionCloseToRust` :1390, all `SyncHttpClient`
+ `CefPostTask`, no platform gating); `SessionManager.cpp` is in the cross-platform
sources. Both link on macOS unchanged — **no new platform-specific code**.

```objc
// Notify ephemeral cookie manager + reset Rust/legacy payment session before closing
if (tab.browser) {
    int browserId = tab.browser->GetIdentifier();
    SessionManager::GetInstance().clearSession(browserId);
    extern void ClearRustPaymentSessionForBrowser(int);
    ClearRustPaymentSessionForBrowser(browserId);
    EphemeralCookieManager::GetInstance().OnTabClosed(browserId);
}
```

Add to the include block at the top of `TabManager_mac.mm` (currently absent — Windows
`TabManager.cpp` has it):

```objc
#include "../../include/core/SessionManager.h"
```

**Test (macOS dev build):** (1) Approve a domain with a low per-session cap; make a
payment approaching the cap (silent, gold pill fires). (2) Close the tab. (3) Reopen the
same domain in a new tab and pay again. **After fix:** silent (Rust counters reset).
**Before fix:** prompts, because Rust still holds prior `sessionSpentCents` for that
`browser_id`. Cross-check the wallet log for `POST /wallet/session/close` on tab close
(absent pre-fix). Confirm the gold-pill chain is unaffected.

---

## Build-bring-up checklist

- [x] **macOS CMake / source set includes all new C++ files** — confirmed. Every
  sprint `.cpp` with cross-platform logic is in the macOS build: `simple_handler.cpp` /
  `simple_render_process_handler.cpp` / `simple_app.cpp` + `PaidContentCache.cpp` /
  `SyncHttpClient.cpp` / `CookieBlockManager.cpp` (cross-platform `SOURCES`);
  `HttpRequestInterceptor.cpp` / `PermissionEngine.cpp` / `PermissionGate.cpp` /
  `EngineShadow.cpp` / `ManifestFetcher.cpp` / `EphemeralCookieManager.cpp` /
  `SettingsManager.cpp` / `SessionManager.cpp` (`if(APPLE)` block). Header-only sprint
  files (`CachedContentResourceHandler.h`, `CWIShimScript.h`, `SensitiveCertFields.h`)
  compile via inclusion. No CMake changes needed.
- [x] **Add `#include "../../include/core/SessionManager.h"` to `TabManager_mac.mm`** — DONE on the Windows branch (2026-06-15).
- [x] **Apply the Gap #1 `CloseTab` fix** — DONE on the Windows branch (2026-06-15), mirrors Windows verbatim. Still needs a macOS clean build to confirm symbol resolution (next item).
- [ ] **Confirm `sqlite3` links on macOS** for PaidContentCache (`find_library SQLITE3_LIBRARY` on main + helper targets — already configured per the CMake audit; confirm at build time).
- [ ] **Full macOS clean build** of the C++ shell after the `TabManager_mac.mm` edit — confirm `SessionManager` symbol resolution + no include regressions.
- [ ] Wallet UI overlays open/close correctly.
- [ ] Permission prompts (each of the 11 types) render + resolve via the notification overlay.
- [ ] Domain-trust connect flow (direct-fetch + shim) works.
- [ ] Gold pill fires on auto-approved payment.
- [ ] BRC-121 paid content works.
- [ ] Full thorough smoke (CLAUDE.md Testing Standards) on macOS.

---

## Open questions / runtime-verification debt

1. **Legacy `SessionManager::clearSession` on macOS** — Windows still calls it because
   the BRC-121 inline cascade may still use C++ SessionManager counters. Decision:
   mirror Windows exactly (include both calls) unless the Mac session confirms the C++
   `SessionManager` is fully dead on the BRC-121 path. (This intersects 2.6-H's OQ5
   SessionManager-deletion question — if 2.6-H removes C++ SessionManager, drop the
   `clearSession` call on both platforms together.)
2. **Runtime verification (gates #28–30 still open)** — no macOS BRC-121 surface has
   ever run on a Mac. Before any release: all 11 prompt types render via the shared
   `CreateNotificationOverlay`; BRC-121 gold-pill `payment_success_indicator` fires;
   right-click *Manage Site Permissions* opens the `edit_permissions` overlay.
3. **Cache WinHTTP↔libcurl behavioral parity** (DomainPermissionCache, WalletStatusCache,
   BSVPriceCache, CertFieldCache) — confirm timeout semantics + NotFound handling match
   between the branches when the Mac build runs.
4. **Phase-1 runtime risks (test, not code)** — zstd auto-decompression in `CefURLRequest`,
   deferred `Open()` callback semantics, `frame->LoadURL` reload hitting
   `GetResourceRequestHandler` (PaidContentCache read-side) on macOS.
5. **Out-of-scope macOS-port backlog (NOT a sprint regression)** — `wallet_delete_cancel`
   raw-WinHTTP block in `simple_handler.cpp` (~3643-3692) is `#ifdef _WIN32` with no
   `__APPLE__` branch (dates to April-port commit `ae07ee6`) → wallet-delete does nothing
   on macOS. Decide whether to fold into the same Mac session.
6. **Cosmetic doc-comment fix** — `simple_render_process_handler.cpp` HistoryManager
   `#else` comment says "not available on macOS - stubbed", which is stale: macOS DOES
   init HistoryManager via `mac/process_helper_mac.mm`. Not a functional gap.

---

## What was audited and found clean (durable record)

This is the record of *what was checked* — so the C++ deletion in 2.6-H doesn't erase
the knowledge that these surfaces were reviewed.

- **Phases 0 / 0.1 / 0.2** — research/design only, zero C++ deltas (confirmed via git log;
  the shim is V8/JS in the cross-platform `simple_render_process_handler.cpp`).
- **Phase 1 (BRC-121 402)** — `Async402ResourceHandler` / `Async402HTTPClient` /
  `PaidContentCache` / `CachedContentResourceHandler` / gold-pill IPC / broadcast-nosend
  all pure CEF + `SyncHttpClient` + `CefPostDelayedTask` (not `Sleep()`). `PaidContentCache::Initialize`
  called on both platforms. The 4 cache WinHTTP blocks each have a paired `#else`
  `SyncHttpClient` branch.
- **Phase 1.5 (surface completion)** — `PermissionEngine.cpp` / `ManifestFetcher.cpp`
  cross-platform; 5 new modal triggers ride the shared `CreateNotificationOverlayTask`
  (no new HWND/NSPanel); IPC additions platform-neutral; `tests/CMakeLists.txt` correctly
  `if(WIN32) winhttp elseif(APPLE) CURL`.
- **Phase 1.6 (indexer resilience)** — Rust-only except two cross-platform constants in
  `HttpRequestInterceptor.cpp` (`kPromptAuthTimeoutMs`, cert endpoint 120s cap).
- **Phase 2 (CWI/yours/panda shim)** — `CWIShimScript.h` is a pure JS string constant
  (MSVC literal-cap split is harmless under clang); injection is one cross-platform
  `frame->ExecuteJavaScript` block.
- **Phase 2.6 (engine→Rust)** — `EngineShadow.cpp` / `SensitiveCertFields.h` /
  202-PENDING handler / `fireSession*ToRust` / thin-proxy reshape all cross-platform.
  `PermissionEngine`/`PermissionGate`/`EngineShadow` are 2.6-H-doomed and contain no
  platform code regardless.
- **File sweeps** — `HttpRequestInterceptor.cpp`, `simple_handler.cpp`, `simple_app.cpp`,
  `cef_browser_shell.cpp`, `simple_render_process_handler.cpp`, `SessionManager.{h,cpp}`,
  `PendingAuthRequest.h`, the new singletons, and `CMakeLists.txt` all swept for
  HWND/WinHTTP/`WM_*`/`CryptGenRandom`/DPAPI/`CRITICAL_SECTION`/`Sleep()`/hardcoded paths
  — every match was either pre-sprint (April-port era, out of scope) or already inside a
  proper `#ifdef _WIN32` / `#elif defined(__APPLE__)` pair.

---

## Related

- `Final-MVP-Sprint/macos-port/` — the April baseline + established patterns (reuse)
- `phase-2.6-engine-to-rust/PHASE_2_6_ENGINE_TO_RUST.md` — 2.6 plan (each sub-phase had a macOS done-when criterion that was deferred)
- Root `CLAUDE.md` — Invariant #9 (macOS cross-platform readiness), overlay lifecycle (Windows vs macOS), CEF input patterns
