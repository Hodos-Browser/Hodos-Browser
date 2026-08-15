# Phase 2.6-H — Execution spec (verified 2026-06-15)

> Supersedes the stale §2.6-H in `PHASE_2_6_ENGINE_TO_RUST.md` (which assumed a "delete 4 caches
> in one commit" state that does not match reality). Written after the OQ5 migration closed +
> a full reachability sweep. **Scope locked: 2.6-H.1 now; caches → 2.6-H.2 (audited separately).**

> **RE-VERIFIED 2026-06-15 (execution kickoff).** Every cited line/boundary re-greped against the
> current tree. Boundaries are correct. Line-number drift + three under-specified details noted
> inline below (marked `↻`). No showstoppers.

## Reachability (verified by grep against current tree)

### Cleanly dead → 2.6-H.1 deletes
| Target | Evidence it's dead |
|---|---|
| C++ `PermissionEngine.{h,cpp}`, `PermissionGate.{h,cpp}` | `PermissionEngine::Decide`/`RunPermissionGate` callers are ONLY `HttpRequestInterceptor.cpp:2839` + `3161` (both in dead cascades) + the unit tests |
| `EngineShadow.{h,cpp}` | `grep EngineShadow::` → **no live invocation** (only an `#include` + comments). No C++ POST to `/engine/shadow-decide` remains. |
| `runIpcEngineCascade` dead tail | After the live forward-to-Rust block (~2541+, `CefPostTask` that POSTs to Rust and returns), the `buildPermissionContext`+`RunPermissionGate`+shadow path is unreachable |
| `buildPermissionContext` (1208) | Called only at 2513 (dead tail of runIpcEngineCascade) + 3126 (dead Open cert cascade) |
| dead Open() cert cascade (~3077-3196) | superseded by the Rust-authoritative forward; comments mark it dead |
| `SessionManager.{h,cpp}` | zero live readers after OQ5; remaining refs only at 2490-2493 + 2648-2649 (dead tail) + `TabManager(.cpp/_mac.mm)` clearSession |
| Rust shadow infra | `shadow_decide` handler (`permission_service/handlers.rs:68`) + route (`main.rs:1060`) + `engine_shadow_repo.rs` + `engine_shadow_log` table + `EngineFlags` (only field is `shadow_log_enabled`) |

### ⚠️ Live → 2.6-H.1 KEEPS (the stale doc was wrong)
| Target | Why it's live |
|---|---|
| **`DomainPermissionCache`** | BRC-121 domain-trust gate (5664) + IPC trust logging (2908) + `GetDomainIdentityKeyDisclosureAllowed` (1458) |
| **`IdentityKeyApprovalCache` / `KeyLinkageApprovalCache` / `SubPermissionCache`** | reads mostly in the dead `buildPermissionContext`, BUT still **written by live** approve/revoke IPC handlers (`approve` 3523/3575, `isApproved` 4601, revoke/invalidate 3662-3679). Deleting needs an audit that Rust fully owns this state → **2.6-H.2**. |

## ⚠️ The hard part — live/dead are interleaved in one function

`runIpcEngineCascade` is NOT wholly dead. Structure:
- **LIVE (keep):** payment-context compute (satoshis/cents/priceAvailable — feeds the X-Payment-* headers) + the forward-to-Rust `CefPostTask` block (~2534+) that is how every external IPC wallet call reaches Rust.
- **DEAD (delete):** the `SessionManager` reads (2490-2493) + cert pre-compute + `buildPermissionContext` call (2513) that fed it + everything after the forward block returns (the `RunPermissionGate` at 2839 + shadow).

`AsyncWalletResourceHandler::Open()` (2946) is the LIVE direct-fetch handler.
↻ **Corrected range:** the dead tail is the WHOLE block **3025–3253** (the `DEAD CODE below`
banner at 3025 through the fallback `return true;` at 3253), not just "3077-3196". It is
unreachable after the live unconditional forward at **3021–3023** (`handle_request=true;
CefPostTask(StartAsyncHTTPRequestTask); return true;`). The dead block contains the
`if (perm.trustLevel=="approved")` cascade (cert sub-block w/ `buildPermissionContext` 3126 +
`RunPermissionGate` 3161 + `SubmitShadowComparison` 3162, then the dead payment sub-block
3217-3239) AND the unknown-trust fallback `triggerDomainApprovalModal` (3248-3252). Cut 3025-3253;
keep 3021-3023 + the closing `}` at 3254. The LIVE Open() keeps: internal bypass, no-wallet,
BRC-100 auth modal (2995-3002), and the payment-cost stash (3008-3019).

↻ **runIpcEngineCascade exact cut:** keep 2472-2488 (payment-ctx compute) + the forward block
2522-2622 (returns at 2621). Delete: 2490-2493 (SessionManager reads + `sessionSpent/rateCount/
txCount`), 2495-2511 (cert pre-compute), 2513-2521 (`buildPermissionContext`/`ctx`/`callKind`),
and the whole tail 2624-2854 (`GateCallbacks cb` + `RunPermissionGate` 2839 + `SubmitShadowComparison`
2846 + log). `bsvPrice` stays (feeds `cents`). Confirmed: the ONLY live `SessionManager::` calls in
the file are 2490-2493 + 2648-2649 (both in this dead tail). All other SessionManager mentions
(1388/1447/1482/3214/4296/5482/5691) are comments.

**A wrong cut here breaks the live wallet IPC path.** This is the reason 2.6-H.1 is surgery, not a delete.

↻ **Line-drift on the KEEP table above (cosmetic):** DomainPermissionCache live reads are now at
**5655** (BRC-121 gate), **2911** (IPC trust log), **1459-1460** (identity-disclosure) — spec said
5664/2908/1458. Approval-cache live writers: approve **3523/3575**, isApproved **4601**,
revoke/invalidate **3662-3679** (unchanged).

↻ **ManifestFetcher is OUT of 2.6-H.1 scope.** `ManifestFetcher.{h,cpp}` sit adjacent to the engine
files in CMake (APPLE 216, WIN32 263-264) and the 2026-06-14 handoff lumped it under "2.6-H", but
this spec deliberately does NOT list it. It is still depended on by `tests/manifest_fetcher_test.cpp`.
Leave it in place for 2.6-H.1 (its C++→Rust deletion is a separate follow-up). Do NOT touch CMake
lines 216 / 263-264.

## 2.6-H.1 execution order (for a clean build)
1. **C++ `HttpRequestInterceptor.cpp`:** excise the dead tail of `runIpcEngineCascade` (keep payment-ctx + forward block) + `buildPermissionContext` + the dead Open() cert cascade + the `SessionManager` reads/increments + the `#include`s of `PermissionEngine.h`/`PermissionGate.h`/`EngineShadow.h`.
2. **C++ `TabManager.cpp` + `TabManager_mac.mm`:** drop the `SessionManager::clearSession` call (keep `ClearRustPaymentSessionForBrowser` + `OnTabClosed`) + the `SessionManager.h` include.
3. Delete files: `PermissionEngine.{h,cpp}`, `PermissionGate.{h,cpp}`, `EngineShadow.{h,cpp}`, `SessionManager.{h,cpp}`, `tests/permission_engine_test.cpp`, `tests/permission_gate_test.cpp`.
4. **`CMakeLists.txt`:** remove the 4 engine `.cpp` + their headers from BOTH the `if(APPLE)` block
   (`SessionManager.cpp` 205; `PermissionEngine.cpp` 213; `PermissionGate.cpp` 214; `EngineShadow.cpp`
   215 — **NOT** `ManifestFetcher.cpp` 216) and the `WIN32` block (257-262 = PermissionEngine/Gate/
   EngineShadow .h+.cpp; 265-266 = SessionManager .h+.cpp — **NOT** ManifestFetcher 263-264).
   ↻ **`tests/CMakeLists.txt` (refined):** remove ONLY `permission_engine_test.cpp` (34) +
   `permission_gate_test.cpp` (35) + the `../src/core/PermissionEngine.cpp` (49) +
   `../src/core/PermissionGate.cpp` (50) deps. The target STAYS ALIVE — `manifest_fetcher_test.cpp` +
   `sensitive_cert_fields_test.cpp` remain and still need `ManifestFetcher.cpp`/`SyncHttpClient.cpp`/
   `Logger.cpp` (51-53). Update the comment block (41-48) that describes PermissionEngine/Gate.
5. **Rust (↻ refined — NOT a wholesale `EngineFlags` drop; `audit.rs` is partially kept):**
   - Delete `permission_service/handlers.rs` (shadow_decide only — confirm file has nothing else) +
     its `mod.rs` wiring; remove the `/engine/shadow-decide` route at `main.rs:1060`.
   - Delete `database/engine_shadow_repo.rs` + the two `database/mod.rs` lines (33 `pub mod`, 71 `pub use`).
   - Delete `permission_service/flags.rs` entirely (`EngineFlags`/`FlagClass`/`class_name_for`/`from_env`)
     + remove `pub mod flags;` + `pub use flags::EngineFlags;` from `permission_service/mod.rs`.
     `class_name_for` was used ONLY by `build_shadow_entry`, so it dies cleanly.
   - `permission_service/audit.rs`: delete `build_shadow_entry` (+2 tests) + `context_hash` (+1 test) +
     the `EngineShadowEntry`/`EngineFlags` imports. **KEEP** `build_audit_entry`, `body_hash`,
     `decision_to_strings`, `prompt_type_string`, `call_kind_string`, `hex_encode` (+ their tests).
     NOTE: `build_audit_entry`/`permission_audit_log` currently has **no live caller** (dormant
     scaffolding) but is the long-lived audit surface — keep it (it's `pub`, no dead-code warning).
   - `permission_service/state.rs`: drop `flags` field (126) + `flags()` method (188-190) +
     ctor arg (`new(flags)`→`new()`, remove `flags,` init) + the `use super::flags::EngineFlags`
     import (20); update ~28 test sites `PermissionService::new(EngineFlags::default())`→`new()`
     and remove the 2 tests asserting `svc.flags().shadow_log_enabled` (~598/603).
   - `main.rs`: delete 645 (`EngineFlags::from_env`) + 648-650 (shadow-log info block);
     change 646 to `PermissionService::new()`; keep the C.2 log line 647.
   - Migration: current latest is **V22** (connection.rs runs `migrate_v21_to_v22` at 947 — the
     in-code "(V21)" comments at migrations.rs:1097/1130 are stale). Add **V23** `migrate_v22_to_v23`
     = `DROP TABLE IF EXISTS engine_shadow_log` (+ register in connection.rs runner). KEEP
     `permission_audit_log`.
6. **Build:** `cargo build --release` + `cargo test` (clean) ; `cmake --build build --config Release` (Windows). macOS CMake edits can't be verified here → Mac build.
7. **Co-test (user):** full thorough smoke (CLAUDE.md all categories) — internal wallet UI + external dApp connect (teragun/socialcert) + BRC-121 paid content + **gold pill** + right-click Manage Site Permissions. Then memory + close.

## Risk
- Live-IPC-path dissection (step 1) — a wrong cut breaks all external wallet calls. Build catches compile errors; the **co-test is mandatory** (can't curl-test C++).
- DB migration (step 5) — schema change; user pre-approved the shadow-log drop.
- Two-platform CMake — macOS edits unverifiable on Windows (Mac build confirms).

## 2.6-H.2 (separate, later)
Audit each of `IdentityKeyApprovalCache` / `KeyLinkageApprovalCache` / `SubPermissionCache`: are the live C++ approve/revoke handlers now redundant (Rust owns the session-opt-in + scoped-grant state)? If yes, remove caches + handlers; if a handler still serves a live C++ purpose, keep. `DomainPermissionCache` stays regardless (genuinely live).
