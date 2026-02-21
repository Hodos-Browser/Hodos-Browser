# Documentation Discrepancy Tracker

**Created**: 2026-02-19
**Purpose**: Track discrepancies between top-level project docs and the actual state of the codebase. To be reviewed at end of Phase D for consolidation/update recommendations.
**Action**: Do NOT fix these now. Just track them. Fix after Phase D.

---

## 1. README.md

**Overall staleness**: Very stale. Last meaningful update ~Dec 2025. Missing all Phase 0-2 work, DB migration consolidation, DPAPI, domain permissions, notification system.

| # | Discrepancy | Current Reality |
|---|-------------|-----------------|
| 1 | "Current Status - Production Ready (Dec 2025)" | Should reflect Feb 2026, Phase 2.4 complete |
| 2 | References `wallet.json`, `actions.json`, `domainWhitelist.json` as file storage | All deprecated/removed. `wallet.db` is sole source of truth since Phase 9 |
| 3 | File tree shows `json_storage.rs`, `utxo_fetcher.rs` | `json_storage.rs` deprecated, `utxo_fetcher.rs` may be removed. Missing: `database/`, `monitor/`, `crypto/dpapi.rs`, `price_cache.rs`, `recovery.rs` |
| 4 | File tree doesn't show `database/` subdirectory | Major subsystem: `connection.rs`, `migrations.rs`, `wallet_repo.rs`, `address_repo.rs`, `output_repo.rs`, `certificate_repo.rs`, `domain_permission_repo.rs`, `proven_tx_repo.rs`, `backup.rs` |
| 5 | File tree doesn't show `monitor/` subdirectory | 7-task background scheduler: `mod.rs`, `task_check_for_proofs.rs`, etc. |
| 6 | No mention of DPAPI auto-unlock | Implemented and working (V4 migration) |
| 7 | No mention of domain permissions system | Full system: DB + repo + 6 REST endpoints + C++ cache + auto-approve engine |
| 8 | No mention of notification overlay system | Keep-alive HWND, JS injection, 4 notification types |
| 9 | No mention of price cache or fee rate cache | `price_cache.rs` (CryptoCompare + CoinGecko), `fee_rate_cache` |
| 10 | No mention of mnemonic recovery or PIN encryption | Phase 1 complete: recover from mnemonic, AES-256-GCM with PBKDF2 |
| 11 | "early-stage rewrite" at bottom | Significantly past early stage |
| 12 | `BRC-100 Groups A & B complete` messaging accurate | But missing Groups C (partial), monitoring, backup/recovery |
| 13 | Setup instructions say `cargo build` / `cargo run` | Should specify `cargo build --release` for wallet, note WSL considerations |
| 14 | References `BUILD_INSTRUCTIONS.md` | Need to verify this exists and is current |
| 15 | BRC-100 compatibility section references `window.hodosBrowser.brc100.getPublicKey()` | API shape has changed; these specific methods may not exist |
| 16 | References MetanetDesktop identity.json compatibility | `identity.json` is deprecated; identity is in wallet.db |
| 17 | `cef-native/include/core/` described as "Wallet, identity, and navigation headers" | Now also contains `PendingAuthRequest.h`, `SessionManager.h`, `DomainPermissionCache.h`, `BSVPriceCache.h` |

---

## 2. PROJECT_OVERVIEW.md

**Overall staleness**: Very stale. Describes Phase 4 (original DB migration era) state. Significant inaccuracies about file storage, encryption, and window architecture.

| # | Discrepancy | Current Reality |
|---|-------------|-----------------|
| 1 | "File Location: `%APPDATA%/HodosBrowser/wallet/wallet.json`" | `wallet.json` deprecated. All data in `wallet.db` |
| 2 | "Encryption: AES-256-CBC with hardcoded key" | Now AES-256-GCM with PBKDF2 (600K iterations) + DPAPI dual encryption |
| 3 | Identity file structure shows `privateKey` in JSON | Private keys NEVER stored. Derived on-demand from mnemonic |
| 4 | File system shows `wallet.json`, `actions.json`, `domainWhitelist.json` | All removed. Only `wallet.db` in wallet folder |
| 5 | Window hierarchy diagram missing notification overlay HWND | `g_notification_overlay_hwnd` added in Phase 2.3, keep-alive pattern |
| 6 | Overlay Window section doesn't describe BRC100Auth or notification overlays | Multiple overlay types now: wallet, settings, backup, brc100auth, notification |
| 7 | "Not Yet Implemented" lists Group C, D, E as missing | Group C partially done. Certificates (Group E) partially done. Encryption (Group D) still TODO |
| 8 | "Partially Implemented" lists backup modal | Backup modal fully complete since Phase 1b |
| 9 | "History management: In progress" / "Bookmarks: Planned" | History and bookmarks have HistoryManager/BookmarkManager singletons (partial) |
| 10 | References `window.bitcoinBrowser` | Should be `window.hodosBrowser` |
| 11 | "22 HTTP API Endpoints" listing | Significantly more endpoints now (domain permissions, wallet balance, sync, price, etc.) |
| 12 | Data Storage Architecture table says "History (in progress)" | HistoryManager exists with SQLite DB |
| 13 | "IdentityHandler" / "PanelHandler" / "NavigationHandler" class descriptions | These may be stale; V8 injection has evolved significantly |
| 14 | Message passing section doesn't mention HTTP interception to Rust | Primary communication path for BRC-100 is HTTP interception, not V8 messages |
| 15 | Missing: entire domain permission system, auto-approve engine, session management |
| 16 | Missing: price cache, fee rate cache, balance cache, sync status |
| 17 | Missing: DPAPI, PIN encryption, mnemonic recovery, Centbee sweep |
| 18 | Missing: Monitor pattern (background tasks) |
| 19 | Next Steps section describes DB migration as future | DB migration complete since Phase 9 (consolidated V1) |

---

## 3. THE_WHY.md

**Overall staleness**: Moderate. Philosophy is still valid but some technical details outdated.

| # | Discrepancy | Current Reality |
|---|-------------|-----------------|
| 1 | "Last Updated: 2025-01-XX" | Placeholder date, never filled in |
| 2 | References `window.bitcoinBrowser` (Section 1.3, 4.2) | Should be `window.hodosBrowser` |
| 3 | "Express.js" mentioned as comparison | Still valid comparison but feels oddly specific |
| 4 | No mention of DPAPI as concrete example of OS-level security integration | DPAPI is a real implementation of the OS integration they theorize about |
| 5 | No mention of domain permissions as concrete permission model | The permission system described theoretically is now implemented |
| 6 | Trust Wallet migration reference still valid | Good reference, keep |

**Assessment**: Mostly sound. Needs date fix, API name fix, and could benefit from adding concrete examples from our actual implementation (DPAPI, domain permissions) to strengthen the arguments.

---

## 4. ARCHITECTURE.md (TECH_STACK_INTEGRATION.md is the actual content)

**Overall staleness**: Very stale. Describes architecture from ~Oct 2025. Major components missing.

| # | Discrepancy | Current Reality |
|---|-------------|-----------------|
| 1 | HTTP interception flow shows "IO Thread → UI Thread Task → Rust Wallet" | UI thread hop removed in CR-2. Now: IO Thread → direct CefURLRequest on IO thread |
| 2 | References `URLRequestCreationTask` | Removed in CR-2.2 |
| 3 | References `localhost:8080` for interception | Pattern has changed; intercepts any BRC-100 endpoint regardless of port |
| 4 | "In Development" lists completed items | Window management, frontend sync, frontend BRC-100 integration all done |
| 5 | Process-per-overlay diagram shows only 3 overlays | Now 5+: settings, wallet, backup, brc100auth, notification |
| 6 | V8 injection section references `window.bitcoinAPI.sendTransaction()` | API is `window.hodosBrowser.*` |
| 7 | Missing: DomainPermissionCache singleton | Replaced DomainVerifier, reads from Rust DB |
| 8 | Missing: PendingRequestManager singleton | Replaced g_pendingAuthRequest |
| 9 | Missing: SessionManager singleton | Per-browser session tracking |
| 10 | Missing: BSVPriceCache singleton | C++ price cache for auto-approve |
| 11 | Missing: WalletStatusCache singleton | Cached wallet exists/locked status |
| 12 | Missing: Notification overlay keep-alive pattern | HWND reuse, JS injection, pre-creation |
| 13 | BRC-100 endpoint listing is incomplete | Missing: domain permissions, wallet balance, sync, price, recover, etc. |
| 14 | Monitor pattern section is accurate | One of the few current sections |
| 15 | "Wallet-Toolbox Alignment" through V24 note is accurate | Schema consolidation happened after (V1 consolidated) |

---

## 5. TECH_STACK_INTEGRATION.md

**Overall staleness**: Moderate-to-stale. CEF deep dive section is useful reference material. Integration descriptions outdated.

| # | Discrepancy | Current Reality |
|---|-------------|-----------------|
| 1 | HTTP interceptor routes to "port 3301" — correct | But description of routing pattern outdated |
| 2 | "forwards original headers including BRC-31 Authrite authentication headers" | Now also adds `X-Requesting-Domain` header for defense-in-depth |
| 3 | "handles domain whitelisting" | Domain whitelist replaced by DomainPermissionCache |
| 4 | Build output says `cef_browser_shell.exe` | Should be `HodosBrowserShell.exe` (or verify actual name) |
| 5 | CEF deep dive sections (APIs, downloads, cookies) still accurate | Good reference material, keep |
| 6 | Security/Privacy implementation guide is accurate overview | Could be expanded with our actual implementation experience |
| 7 | Cross-platform section is forward-looking and still relevant | macOS sprint hasn't happened yet |
| 8 | Missing: all Phase 2 additions to the tech stack | Domain permissions, auto-approve, notifications, DPAPI |

---

## 6. CLAUDE.md

**Assessment**: Most current document. Updated regularly. Code-verified 2026-02-19 — 8 discrepancies found.

| # | Discrepancy | Current Reality |
|---|-------------|-----------------|
| 1 | Key Files: `HttpRequestInterceptor.cpp` lists `DomainVerifier` class | `DomainVerifier` removed in CR-2. Now `DomainPermissionCache` (HttpRequestInterceptor.cpp:51) |
| 2 | Key Files: `HttpRequestInterceptor.cpp` lists `g_pendingAuthRequest` global | Replaced by `PendingRequestManager` singleton (PendingAuthRequest.h:20-123) |
| 3 | Key Files: `handlers.rs` lists 8 endpoints | File contains **68+ public functions**. Missing: `wallet_create`, `wallet_unlock`, `wallet_balance`, `wallet_backup`, `wallet_recover`, `send_transaction`, domain permissions, price, sync, etc. Also `list_certificates` and `acquire_certificate` not found by name. |
| 4 | Key Files: Missing `PendingAuthRequest.h` | Contains `PendingRequestManager` singleton — critical for request tracking |
| 5 | Key Files: Missing `SessionManager.h` | Contains `SessionManager` singleton + `BrowserSession` struct — critical for auto-approve |
| 6 | Key Files: `crypto/` lists 4 modules | Actually 11 files. Missing: `dpapi.rs`, `pin.rs`, `keys.rs`, `brc2.rs`, `ghash.rs`, `aesgcm_custom_test.rs` |
| 7 | Key Files: `database/` lists 5 repos + 2 helpers | Actually 23 files with 18+ repos. Missing: `domain_permission_repo`, `user_repo`, `settings_repo`, `sync_state_repo`, `tag_repo`, `tx_label_repo`, `commission_repo`, `basket_repo`, etc. |
| 8 | Key Files: `useHodosBrowser.ts` lists 4 methods | Also exports `goBack`, `goForward`, `reload` |

**Verified accurate**: Architecture diagram, port numbers (3301/5137), all 9 invariants, monitor task list (7 tasks), glossary terms.

---

## Summary: Severity Assessment

| Document | Staleness | Effort to Fix | Priority |
|----------|-----------|---------------|----------|
| **README.md** | Critical | Medium | High — first thing users/contributors see |
| **PROJECT_OVERVIEW.md** | Critical | High | High — comprehensive but very wrong |
| **ARCHITECTURE.md** | Critical | High | High — core reference for developers |
| **TECH_STACK_INTEGRATION.md** | Moderate | Low-Medium | Medium — CEF reference sections still valuable |
| **THE_WHY.md** | Low | Low | Low — philosophy still valid, minor fixes |
| **CLAUDE.md** | Low | Low | Low — mostly current, minor updates |

---

## 7. macOS Documentation (Scattered & Partially Stale)

**Assessment**: 5 documents spread across 3 locations. The Dec 2025 port docs are accurate for what they describe but miss all Phase 2 work (Feb 2026). The updated `MAC_PLATFORM_SUPPORT_PLAN.md` is now comprehensive.

### Files and Locations

| File | Location | Status |
|------|----------|--------|
| `MACOS_IMPLEMENTATION_COMPLETE.md` | Repo root | Accurate for Dec 2025 scope; misses Phase 2 features |
| `MACOS_PORT_SUCCESS.md` | Repo root | Largely duplicates the above; celebratory |
| `PHASE2_MACOS_SUPPORT_SUMMARY.md` | Repo root | Describes CMake phase only; overlaps with above |
| `MAC_PLATFORM_SUPPORT_PLAN.md` | `development-docs/macos-port/` | **Updated 2026-02-19** — comprehensive rewrite |
| `MACOS_BUILD_INSTRUCTIONS.md` | `build-instructions/` | Useful build guide, still valid |

### Discrepancies

| # | Discrepancy | Current Reality |
|---|-------------|-----------------|
| 1 | `MACOS_IMPLEMENTATION_COMPLETE.md` lists 5 overlay types | Now 6+ (notification overlay added in Phase 2.3) |
| 2 | Feature matrix shows Tab/Wallet/History/HTTP Interception as "TODO" | Tab partially done (TabManager_mac.mm exists). Others still TODO. |
| 3 | All 3 root docs pre-date Phase 2 | Missing: domain permissions, auto-approve, notification overlay, session manager, BSVPriceCache, WalletStatusCache, DPAPI, mnemonic recovery |
| 4 | `MACOS_PORT_SUCCESS.md` and `MACOS_IMPLEMENTATION_COMPLETE.md` heavily overlap | Should consolidate into one document |
| 5 | None mention macOS Keychain as DPAPI equivalent | `MAC_PLATFORM_SUPPORT_PLAN.md` now covers this |
| 6 | None mention correct macOS file paths | `MAC_PLATFORM_SUPPORT_PLAN.md` now covers this |

### Recommendation

1. **Move** the 3 root macOS docs into `development-docs/macos-port/` (reduce root clutter)
2. **Consolidate** `MACOS_IMPLEMENTATION_COMPLETE.md` + `MACOS_PORT_SUCCESS.md` + `PHASE2_MACOS_SUPPORT_SUMMARY.md` into a single `port-history.md` (archive of what was done in Dec 2025)
3. **Keep** `MAC_PLATFORM_SUPPORT_PLAN.md` as the living plan document (already updated)
4. **Keep** `build-instructions/MACOS_BUILD_INSTRUCTIONS.md` as the build guide

---

## 8. Stale Top-Level Documents (Repo Root)

**Assessment**: Several top-level docs have not been inventoried. Some may be stale or redundant.

| File | Likely Status | Recommendation |
|------|--------------|----------------|
| `Developer_notes.md` | Unknown — check content | Review, potentially archive |
| `IMPLEMENTATION_STATUS.md` | Likely stale (pre-Phase 2) | Review, merge into browser-core docs or archive |
| `SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md` | May still be accurate | Review, keep if valid |
| `UX_FEATURE_COMPARISON_AND_ROADMAP.md` | Likely stale | Review, superseded by browser-capabilities.md |
| `WALLET_ARCHITECTURE.md` | May overlap with PROJECT_OVERVIEW | Review, merge into ARCHITECTURE.md or archive |

---

## Summary: Severity Assessment

| Document | Staleness | Effort to Fix | Priority |
|----------|-----------|---------------|----------|
| **README.md** | Critical | Medium | High — first thing users/contributors see |
| **PROJECT_OVERVIEW.md** | Critical | High | High — comprehensive but very wrong |
| **ARCHITECTURE.md** | Critical | High | High — core reference for developers |
| **TECH_STACK_INTEGRATION.md** | Moderate | Low-Medium | Medium — CEF reference sections still valuable |
| **THE_WHY.md** | Low | Low | Low — philosophy still valid, minor fixes |
| **CLAUDE.md** | Low | Low | Low — mostly current, minor updates |
| **macOS docs (3 in root)** | Moderate | Low | Medium — consolidate into macos-port/ |
| **Other root docs (5)** | Unknown | Low | Low — review and archive/merge |

### Recommendation (Updated) — Status as of 2026-02-19

**Phase 1: Immediate** -- COMPLETE
1. ~~**Update** CLAUDE.md~~ -- Done. 8 fixes, 2 new invariants (macOS, doc updates), macOS path
2. ~~**Create** `development-docs/browser-core/CLAUDE.md`~~ -- Done. Sprint context + cross-platform rules
3. ~~**Move** 3 macOS root docs into `development-docs/macos-port/`~~ -- Done. git mv

**Phase 2: Consolidation** -- COMPLETE
4. ~~**Consolidate** PROJECT_OVERVIEW + ARCHITECTURE + WALLET_ARCHITECTURE~~ -- Done. Single PROJECT_OVERVIEW.md; others archived with pointers
5. ~~**Rewrite** README.md~~ -- Done. Concise landing page
6. ~~**Archive** FEATURES.md, TECH_STACK_INTEGRATION.md, UX_FEATURE_COMPARISON_AND_ROADMAP.md~~ -- Done. Archive headers added

**Phase 3: Polish** -- COMPLETE
7. ~~**Minor edit** THE_WHY.md~~ -- Done. Dates fixed, `bitcoinBrowser` -> `hodosBrowser`, concrete examples added
8. ~~**Update** SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md~~ -- Done. Go->Rust, ports, notification overlay, domain permissions
9. ~~**Update** CEF_REFINEMENT_TRACKER.md~~ -- Done. All CR-2/CR-3 checkboxes updated
10. **Keep** Developer_notes.md, IMPLEMENTATION_STATUS.md -- Active development logs, no changes needed

**Remaining**:
- UX_UI/00-IMPLEMENTATION_INDEX.md -- Mark CR-2 complete, Phase 2.4 complete (do during next UX session)

---

**End of Document**
