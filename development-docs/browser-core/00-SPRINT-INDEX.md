# Browser Core Audit & MVP Sprint

**Created**: 2026-02-19
**Status**: All Phases Complete
**Purpose**: Comprehensive review of browser functionality, security, privacy, and architecture cohesiveness. Prioritize remaining work for a quality MVP browser.

---

## Sprint Goal

Assess what we have, identify gaps, research how Chrome and Brave handle core browser features, and produce an ordered implementation plan that minimizes future refactoring. The output is a prioritized MVP roadmap covering both browser features and remaining wallet UX.

---

## Sprint Phases

| Phase | Name | Status | Description |
|-------|------|--------|-------------|
| **A** | Audit & Inventory | ✅ Complete | Catalog all files/databases created, review architecture, update stale docs |
| **B** | Research | ✅ Complete | Chrome/Brave file structures, security, privacy, open source components |
| **C** | Gap Analysis & Prioritization | ✅ Complete | What's missing for MVP, ordered priority list |
| **D** | Implementation Planning | ✅ Complete | Break into sprint-sized chunks with dependencies |

## Implementation Sprints

| Sprint | Name | Status |
|--------|------|--------|
| 0 | Safety & Quick Wins | ✅ Complete |
| 1 | SSL Certificate Handling + Secure Indicator | ✅ Complete |
| 2 | Permission Handler | Pending |
| 3 | Download Handler | ✅ Complete |
| 4 | Find-in-Page | ✅ Complete |
| 5 | Context Menu Enhancement | ✅ Complete |
| 6 | JS Dialog Handler + Keyboard Shortcuts | ✅ Complete |
| 7 | Light Wallet Polish | Pending |
| 8 | Ad & Tracker Blocking (8a-8f) | ✅ Complete |
| 9 | Settings + Import + Clear Data + Multi-Profile | ✅ Complete (2026-02-25) |
| 10 | Scriptlet Compatibility System (10a-10c) | ✅ Complete (2026-02-25) |
| 11 | Menu Button UX + Full-Page Settings (11a-11b) | ✅ Complete (2026-02-25) |
| 12 | Fingerprint Protection (12c-12e) | ✅ Complete (2026-02-25) |
| 13 | Tab Tear-Off (Multi-Window) | Planning |

---

## Phase A: Audit & Inventory

### A.1 Documentation Discrepancy Review
- [x] Read all top-level docs (README, PROJECT_OVERVIEW, THE_WHY, ARCHITECTURE, TECH_STACK_INTEGRATION)
- [x] Create discrepancy tracker: [doc-discrepancies.md](./doc-discrepancies.md)
- [x] Cross-reference CLAUDE.md against current code — 8 discrepancies found (added to tracker)
- **Action**: Defer fixes to end of Phase D; just note issues now

### A.2 File & Database Inventory
- [x] Catalog everything in `%APPDATA%\HodosBrowser\` at runtime
- [x] Catalog everything CEF creates in `Default\` profile folder
- [x] Identify what Rust wallet creates in `wallet\`
- [x] Identify C++ singletons and their in-memory state
- [x] Identify frontend localStorage keys
- [x] Document: [file-inventory.md](./file-inventory.md)

### A.3 CEF Refinement Tracker Update
- [x] Review [CEF_REFINEMENT_TRACKER.md](../CEF_REFINEMENT_TRACKER.md) against actual code
- [x] Mark completed items — see findings below
- [x] Identify remaining CR-3 items still relevant
- [x] Note any new issues discovered during Phase 2

**CR-2/CR-3 Verification Findings** (code-verified 2026-02-19):

| Item | Tracker Status | Actual Status | Notes |
|------|---------------|---------------|-------|
| CR-2.1 | Unchecked | **DONE** | Async HTTP via `CefURLRequest` on IO thread; `StartAsyncHTTPRequestTask` defers creation |
| CR-2.2 | Unchecked | **DONE** | `PendingRequestManager` singleton, `map<requestId, PendingAuthRequest>`, unique IDs |
| CR-2.3 | Unchecked | **DONE** | `std::mutex` + `lock_guard` on all PendingRequestManager methods |
| CR-2.4 | Unchecked | **DONE** | `DomainPermissionCache` replaced `DomainVerifier` (JSON file I/O eliminated) |
| CR-2.5 | Unchecked | **DONE** | `std::atomic<bool> httpCompleted_` + `compare_exchange_strong` in both response paths |
| CR-2.6 | Unchecked | **DONE** | `CefRefPtr<AsyncWalletResourceHandler> parent_` (was raw pointer) |
| CR-3.1 | Unchecked | **DONE** | `domain_whitelist.rs` deleted; `domain_permissions` table in SQLite |
| CR-3.2 | Unchecked | **PARTIAL** | State spread across `AsyncWalletResourceHandler` + `SessionManager`; no unified context struct |
| CR-3.3 | Unchecked | **DONE** | All deferred callbacks use `CefRefPtr` (ref-counted) |
| CR-3.4 | Unchecked | **DONE** | `OnBeforeClose` properly nullifies browser refs; HWND cleanup gated |
| CR-3.5 | Unchecked | **DONE** | Debug overlay not enabled in production paths |
| CR-3.6 | Unchecked | **PARTIAL** | Hardcoded 200 — by design (errors in JSON body). Could revisit for standard compliance. |
| CR-3.7 | Unchecked | **DONE** | HWND validity check + explicit `nullptr` on destroy |
| CR-3.8 | Unchecked | **DONE** | Singleton Logger, minimal conditional logging |
| CR-3.9 | Unchecked | **DONE** | Only redirects localhost/127.0.0.1 → 3301; external BRC-104 passes through |
| CR-3.10 | Unchecked | **N/A** | macOS-only; no macOS build yet |

**Summary**: 13/15 DONE, 2/15 PARTIAL (both by design), 1/15 N/A. Tracker document is very stale — all CR-2 and most CR-3 items were completed during Phase 2 but never checked off.
**Action**: Tracker checkboxes should be updated (deferred to Phase D with other doc fixes).

### A.4 Current Browser Capabilities Assessment
- [x] What works today (navigation, cookies, audio, video)
- [x] What doesn't work (x.com login, camera/mic permissions, etc.)
- [x] What's partially working (history, bookmarks)
- [x] What's completely missing (tabs, downloads UI, ad blocking, etc.)
- [x] Document: [browser-capabilities.md](./browser-capabilities.md)

**Key findings**: Navigation/tabs/cookies/history/bookmarks all fully working. SSL cert handler, camera/mic permissions, downloads UI, find-in-page, print handler NOT implemented. SimpleHandler implements 8 CEF handlers; missing 5 needed for MVP (CefDownloadHandler, CefPrintHandler, CefPermissionHandler, CefFindHandler, CefJsDialogHandler). FEATURES.md critically stale.

### A.5 Architecture Cohesiveness Assessment
- [x] C++ ↔ Rust communication patterns (HTTP, IPC, WinHTTP)
- [x] Data flow consistency across layers
- [x] Singleton/global state inventory in C++
- [x] Thread safety audit (IO thread, UI thread, renderer)
- [x] Identify architectural debt from rapid Phase 2 development
- [x] Document: [architecture-assessment.md](./architecture-assessment.md)

**Key findings**: 3 communication patterns (WinHTTP sync, CefURLRequest async, direct frontend fetch). BSVPriceCache returns 0.0 on error (HIGH risk — breaks auto-approve USD conversion). 10+ C++ singletons all properly mutex-protected. SessionManager has minor reference-after-unlock race (LOW). 15+ unprotected global HWNDs (LOW — Windows kernel thread-safe). Direct frontend->Rust fetch calls bypass C++ interceptor (architectural debt, not security risk).

---

## Phase B: Research

Detailed findings go in: [01-chrome-brave-research.md](./01-chrome-brave-research.md)

### B.1 Chrome Browser Internals
- [x] File/database structure (`User Data\Default\`)
- [x] Cookie storage and management
- [x] History and bookmarks databases
- [x] Certificate and SSL handling
- [x] Permission model (camera, mic, notifications, geolocation)
- [x] Profile management and data isolation
- [x] Cache structure and eviction

**Key findings**: Chrome stores 7 SQLite DBs + 3 LevelDB stores + JSON files in `Default/`. Cookies use AES-128-CBC with DPAPI-protected key. History timestamp is microseconds since 1601-01-01. Bookmarks are straightforward JSON. Profile import feasible for bookmarks (easy), history (easy), cookies (medium — we have DPAPI).

### B.2 Brave Browser Architecture
- [x] `brave-core` overlay on Chromium — what they customize
- [x] Shields system (unified per-site toggle for ads/trackers/cookies/fingerprinting)
- [x] `adblock-rust` crate — API, license (MPL 2.0), integration pattern
- [x] Brave's Rust usage — what runs in Rust vs C++
- [x] Cookie controls implementation
- [x] Fingerprinting protection approach
- [x] WebRTC leak prevention
- [x] Open source components we can reuse (license-compatible)

**Key findings**: Brave's `brave-core` overlays Chromium (similar pattern to our CEF handler approach). `adblock-rust` is 5.7μs/check, MPL-2.0, battle-tested. Fingerprinting uses per-session "farbling" seeds. WebRTC leak prevention is a one-line CEF flag. 4 reusable OSS repos identified (adblock-rust, adblock-lists, adblock-resources, easylist).

### B.3 CEF-Specific Research
- [x] SSL certificate store behavior in subprocesses
- [x] `CefRequestContext` configuration for SSL/cookies/permissions
- [x] Permission APIs (camera, mic, geolocation, notifications)
- [x] FedCM support status and workarounds
- [x] Profile import capabilities (can CEF read Chrome/Brave profile data?)
- [x] `OnBeforeResourceLoad()` performance characteristics

**Key findings**: `OnCertificateError` uses `CefCallback` (not `CefRequestCallback`). CEF 136 Chrome bootstrap: returning `false` from `OnShowPermissionPrompt` shows native Chrome permission UI for free. Print handler NOT needed on Windows. FedCM has no CEF API — low ROI. `CefRequestContext` per-tab with empty `cache_path` = incognito mode.

### B.4 Rust Daemon Architecture
- [x] Evaluate expanding `rust-wallet/` to `hodos-core/` workspace
- [x] FFI vs HTTP for ad block queries (latency analysis)
- [x] `adblock-rust` crate evaluation (build, test, API review)
- [x] Workspace structure if we expand

**Key findings**: FFI overhead is ~3ns (negligible) vs HTTP 50-500μs (10-100x worse). Recommended: adblock-rust as FFI static lib linked into C++ (Brave's proven architecture). Cargo workspace refactor premature — revisit with 3rd Rust component. adblock-rust API: `FilterSet` → `Engine` → `check_network_request()` with disk serialization for fast startup.

---

## Phase C: Gap Analysis & Prioritization

Document: [mvp-gap-analysis.md](./mvp-gap-analysis.md)

### C.1 Core Browser Gaps (MVP-blocking)
- [x] Tab management — **Already fully working** (Phase A.4 confirmed)
- [x] SSL/TLS (x.com and similar sites) — Gap identified: no `OnCertificateError` handler (Tier 0)
- [x] Permission prompts (camera, mic, geolocation, notifications) — Gap: return `false` for Chrome native UI (Tier 0)
- [x] Downloads manager — **COMPLETE** (Sprint 3): `CefDownloadHandler` + overlay panel + progress icon
- [x] Find in page — Gap: no `CefFindHandler` (Tier 1)
- [x] Print support — **NOT NEEDED** on Windows (CEF handles natively)
- [x] Context menus — Gap: only 2 items, needs Copy/Paste/Save Image/View Source (Tier 1)
- [x] Keyboard shortcuts — 8 exist, ~8 more needed (Tier 1, depends on downloads + find)

### C.2 Security & Privacy Gaps
- [x] Ad blocking — Gap: `adblock-rust` FFI integration needed (Tier 2)
- [x] Tracker blocking — Combined with ad blocking via EasyPrivacy list
- [x] Cookie controls — Gap: third-party cookie blocking (Tier 2)
- [x] Fingerprinting protection — Gap: V8 injection for 3rd-party API blocking (Tier 2)
- [x] Secure connection indicators — Gap: no padlock/HTTPS indicator (Tier 1)
- [x] Mixed content handling — Gap: `--allow-running-insecure-content` flag should be removed (Tier 1, trivial)
- [x] **NEW**: BSVPriceCache 0.0 safety bug — returns 0.0 on error, breaks auto-approve (Tier 0)

### C.3 User Data & Profile Gaps
- [x] Profile import from Chrome/Brave — Feasible: bookmarks (easy JSON), history (easy SQLite), cookies (medium DPAPI) (Tier 2)
- [x] History management (current state assessment) — **Fully working**, no MVP gaps
- [x] Bookmark management (current state assessment) — **Fully working**, bookmark bar would be nice-to-have
- [x] Settings persistence — Gap: no unified browser settings persistence (Tier 2)

### C.4 Remaining Wallet UX (from UX_UI roadmap)
- [x] Phase 3: Light wallet polish — Tier 1 (button states, progress, QR, validation)
- [x] Phase 4: Full wallet view — Deferred to post-MVP (current overlay sufficient)
- [x] Phase 5: Activity status indicator — Deferred to post-MVP
- [x] Certificate testing — Deferred (needs certifier service)

### C.5 Priority Ordering
- [x] Rank all gaps by: MVP-blocking vs nice-to-have — 4 tiers defined
- [x] Identify dependencies between items — Dependency map in document
- [x] Identify items that share infrastructure (build together) — 5 groups identified
- [x] Produce ordered implementation list — 26 items across 4 tiers

**Summary**: 4 Tier-0 items (~4-5 days), 8 Tier-1 items (~8-11 days), 5 Tier-2 items (~9-13 days), 9 post-MVP items. Almost all items are independent (highly parallelizable). Quality MVP = Tier 0 + Tier 1 ≈ 2-3 weeks.

---

## Phase D: Implementation Planning

Document: [implementation-plan.md](./implementation-plan.md)

- [x] Break ordered list into sprint-sized chunks (1-3 days each) — 11 sprints (0-10)
- [x] Identify which sprints can be parallelized — Sprints 0-3 parallel, 4-5+7 parallel, 8 standalone
- [x] Note architecture decisions needed before implementation — 7 decisions documented
- [x] Flag items requiring user testing — Post-implementation checklist with Tier 0/1/2 verification
- [x] Review doc-discrepancies.md and recommend doc consolidation/updates — 8 doc fixes recommended

**Summary**: 11 sprints across 4 weeks. Week 1: safety fixes + SSL + permissions + downloads (ship-blocking). Week 2: find-in-page + context menus + shortcuts + wallet polish (core quality). Week 3: ad blocking (differentiating). Week 4: settings + import + cookie/fingerprint protection (if time). Doc fixes as final sprint.

---

## Related Documents

### In This Folder
- [doc-discrepancies.md](./doc-discrepancies.md) — Stale documentation tracker (66+ discrepancies across 6 docs)
- [file-inventory.md](./file-inventory.md) — Complete file/DB/singleton inventory (Phase A.2)
- [browser-capabilities.md](./browser-capabilities.md) — What works, what's broken, what's missing (Phase A.4)
- [architecture-assessment.md](./architecture-assessment.md) — Communication patterns, thread safety, debt (Phase A.5)
- [01-chrome-brave-research.md](./01-chrome-brave-research.md) — Research findings (Phase B)
- [mvp-gap-analysis.md](./mvp-gap-analysis.md) — Prioritized gap list with effort estimates (Phase C)
- [implementation-plan.md](./implementation-plan.md) — Sprint-sized implementation chunks with schedule (Phase D)

### macOS Port
- [macos-port/MAC_PLATFORM_SUPPORT_PLAN.md](../macos-port/MAC_PLATFORM_SUPPORT_PLAN.md) — Comprehensive macOS porting plan (5-7 day sprint, near end of MVP)

### Existing Documents (Referenced, Not Moved)
- [CEF_REFINEMENT_TRACKER.md](../CEF_REFINEMENT_TRACKER.md) — CR-1/2/3 checklist (needs update)
- [data-storage-and-encryption-review.md](../data-storage-and-encryption-review.md) — Storage, encryption, security sprint planning
- [FEATURES.md](../FEATURES.md) — Feature roadmap (needs update)
- [UX_UI/00-IMPLEMENTATION_INDEX.md](../UX_UI/00-IMPLEMENTATION_INDEX.md) — UX phase tracker

### Top-Level Docs (Discrepancies Noted)
- `/README.md`
- `/PROJECT_OVERVIEW.md`
- `/THE_WHY.md`
- `/ARCHITECTURE.md`
- `/TECH_STACK_INTEGRATION.md`

---

**End of Document**
