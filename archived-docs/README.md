# Archived Docs

**196 documents** (plus this index). Historical, shipped, superseded and
abandoned documentation from before the current sprint structure. The
archaeology — why a decision was made, how a shipped feature was designed,
what a dead end looked like — is worth keeping.

> ⚠️ **Most of this folder is not in git.** `.gitignore` line 47 excludes
> `archived-docs/` with the comment *"already in repo, no longer tracking
> changes"*. That comment is no longer true: only **22** files are tracked —
> the ones `git mv`'d in from tracked locations at 28f23c8 and f37adfe
> (`Final-MVP-Sprint/`, `Wallet-Hardening/`, `Future-Features/`, plus five
> root docs). The other ~175 predate the ignore rule and exist **only in a
> local working copy.** This index and its `git add -f` are tracked so the
> record survives even where the files do not. Whether to un-ignore the folder
> and commit the rest is an open owner decision.

> **Do not treat anything in this folder as current.** Much of it predates the
> Hodos rename, the Rust permission engine, the profile system, the Rust
> service layer and the V23 schema. Several docs describe a Go wallet backend
> that was never shipped. For current state read `/CLAUDE.md`, the per-layer
> `CLAUDE.md` files, and `development-docs/`.

**Why this index exists.** For a long time this folder had 194 files and no
index, so nothing recorded that a doc had moved here. The root `README.md`
pointed at `IMPLEMENTATION_STATUS.md` for months after it was archived. One
line per file, below, so that cannot happen again. When you archive something,
add its row in the same commit.

---

## Root level (112 files)

### Project snapshots, architecture & process

| Doc | Reason archived |
|-----|-----------------|
| `ARCHITECTURE.md` | Oct 2025 HTTP-interception architecture snapshot; predates the Rust permission engine and the IPC bridge entirely. |
| `ASSESSMENT_REPORT.md` | Nov 2025 architecture assessment; a point-in-time review, all findings long since actioned or overtaken. |
| `IMPLEMENTATION_STATUS.md` | Dec 2025 status snapshot. **The doc the root README kept pointing at after it moved** — the reason this index exists. |
| `FEATURES.md` | Nov 2025 feature roadmap; superseded by the sprint folders in `development-docs/`. |
| `TECHNICAL_DECISIONS.md` | Consolidated early architecture/debugging reference; contents dispersed into `/CLAUDE.md` and the layer docs. |
| `TECH_STACK_INTEGRATION.md` | Early three-layer integration write-up; the shape is now in `/CLAUDE.md`. |
| `WALLET_ARCHITECTURE.md` | Pre-repository-pattern wallet architecture; superseded by `rust-wallet/CLAUDE.md`. |
| `RUST_WALLET_DB_ARCHITECTURE.md` | JSON→SQLite transition-era design ("support both during transition"); transition finished. |
| `BUILD_INSTRUCTIONS.md` | Superseded by `build-instructions/WINDOWS_BUILD_INSTRUCTIONS.md` + `MACOS_BUILD_INSTRUCTIONS.md`. |
| `CEF_WRAPPER_CMakeLists.txt` | Copy of the CEF wrapper CMakeLists kept as a reference during early build bring-up; the real file ships in `cef-binaries/`. |
| `RENAMING_PLAN.md` | Babbage → Hodos rename plan; executed. |
| `GIT_WORKFLOW_GUIDE.md` | Generic multi-contributor git guide; superseded by `development-docs/DevOps-CICD/README.md`. |
| `TESTING_STRATEGY.md` | "Babbage Browser" era testing strategy; superseded by the Testing Standards table in `/CLAUDE.md`. |
| `doc-discrepancies.md` | Tracker for doc-vs-code drift; superseded by the 2026-08-03 docs truth pass, which fixed the drift rather than tracking it. |
| `REORGANIZATION_PLAN.md` | Proposal to reorganize the `UX_UI/` folder. Never applied; that folder is itself archived now. |

### BRC protocol research

| Doc | Reason archived |
|-----|-----------------|
| `API_REFERENCES.md` | Written against the **BSV Go SDK** — a backend direction that was abandoned for Rust. Historical only. |
| `BRC100_IMPLEMENTATION_GUIDE.md` | Implementation guide from the schema-V14 era; the schema is now V23 and the surface is documented in `rust-wallet/CLAUDE.md`. |
| `BRC2_RESEARCH_FINDINGS.md` | Pre-implementation research for BRC-2 encryption; shipped as `rust-wallet/src/crypto/brc2.rs`. |
| `BRC33_MESSAGE_RELAY_IMPLEMENTATION_GUIDE.md` | Pre-implementation guide for the message relay; shipped (`sendMessage` / `listMessages` / `acknowledgeMessage`). |
| `BRC42_PRIVACY_EXPLANATION.md` | Explainer on BRC-42/43 per-app identity; conceptual background, now covered by the Glossary in `/CLAUDE.md`. |
| `BRC52_SIGNATURE_VERIFICATION_FINDINGS.md` | Pre-implementation research; shipped as `rust-wallet/src/certificate/verifier.rs`. |
| `BRC_DOCUMENTS_TO_REVIEW.md` | Reading list of BRC specs relevant to Paymail architecture; consumed. |
| `BSV_ACADEMY_CONCEPTS_SUMMARY.md` | Notes from BSV Academy material; background reading, not project state. |
| `ENDPOINT_ANALYSIS.md` | Comparison of our endpoint surface against the reference wallet; superseded by the live roster in `rust-wallet/CLAUDE.md`. |

### BEEF, transactions & broadcasting

| Doc | Reason archived |
|-----|-----------------|
| `BEEF_FORMATS_SUMMARY.md` | BEEF format primer; shipped as `rust-wallet/src/beef.rs`. |
| `BEEF_GENERATION_PLAN.md` | Plan for BEEF generation in `listOutputs`; implemented. |
| `BEEF_IMPLEMENTATION.md` | Oct 2025 BEEF Phase 2 implementation log; complete. |
| `INPUTBEEF_IMPLEMENTATION_GUIDE.md` | `inputBEEF` handling guide; implemented (and the 100 MB payload cap that came with it). |
| `PHASE_6_BEEF_SPV_CACHING_IMPLEMENTATION.md` | BEEF/SPV caching plan; superseded by the proven-tx repositories and the Monitor proof tasks. |
| `TRANSACTION_IMPLEMENTATION_GUIDE.md` | Early unsigned-transaction build guide; superseded by `create_action` and the four builders documented in `/CLAUDE.md`. |
| `ARC_MIGRATION_PLAN.md` | Plan to replace GorillaPool mAPI with ARC; executed — ARC is the primary broadcast path. |
| `MINER_RESPONSE_HANDLER_AND_SELF_HEAL.md` | Design for miner-response classification and self-heal; shipped as `arc_status.rs` + the Monitor tasks. |
| `NOSEND_TWOPHASE_INVESTIGATION.md` | Apr 2026 investigation into two-phase nosend signing; resolved into the BRC-121 broadcast-after-200 architecture. |
| `CREATE_ACTION_LOCK_RESTRUCTURE.md` | Plan to restructure the createAction lock; implemented (`AppState.create_action_lock`). |
| `CHECKPOINT_TRANSACTION_ERROR_HANDLING.md` | Point-in-time checkpoint on transaction error handling and UI feedback. |
| `REQUEST_FORMAT_COMPARISON.md` | Rust wallet vs TypeScript SDK request-shape diffing during interop bring-up. |
| `FINAL_REQUEST_COMPARISON.md` | The closing entry of that same diffing exercise. |
| `capture_working_ts_request.md` | How-to for capturing a known-good TypeScript SDK request as a fixture. |
| `README_CAPTURE.md` | Companion: capturing Metanet-Client requests for comparison. |
| `INTEROPERABILITY_TEST_README.md` | Runbook for the early interop test; superseded by `rust-wallet/tests/sdk_interop_test.rs`. |

### Certificates (BRC-52) & PushDrop

| Doc | Reason archived |
|-----|-----------------|
| `CERTIFICATE_DEBUGGING_ANALYSIS.md` | CSR debugging analysis; concluded — custom AES-GCM verified by roundtrip tests. |
| `CERTIFICATE_DEBUGGING_SUMMARY.md` | Summary of that same debugging arc. |
| `CERTIFICATE_TRANSACTION_CREATION_GUIDE.md` | Guide written while confirming our PushDrop matches the TypeScript SDK; confirmed. |
| `CERTIFICATE_TRANSACTION_IMPLEMENTATION_DESIGN.md` | Dec 2025 design; implemented as `create_certificate_transaction`. |
| `CERTIFICATE_UI_AND_LIFECYCLE.md` | Cert UI + lifecycle plan from when overlay submit still rejected; publish/unpublish shipped and were later optimized in Phase 1.6. |
| `CSR_FORMAT_AND_IMPLEMENTATION.md` | CSR wire-format notes; implemented. |
| `ENCRYPTION_AND_DB_STORAGE.md` | Cert field encryption + storage design; implemented. |
| `ENCRYPTION_VERIFICATION.md` | Verification pass over that encryption work; passed. |
| `PART3_CERTIFICATE_MANAGEMENT_IMPLEMENTATION.md` | Part 3 cert-management guide, still marked "Research Phase"; the whole surface shipped. |
| `PART3_REMAINING_TASKS_AND_TESTING.md` | Its task list, written when the endpoint returned `501 Not Implemented`. |
| `PHASE4_RESEARCH_CHECKLIST.md` | Research checklist for cert parsing/verification infrastructure; complete. |
| `PUSHDROP_TEST_RESULTS.md` | PushDrop test results from the SDK-parity push. |
| `PUSHDROP_TRANSACTION_OVERVIEW.md` | Dec 2025 PushDrop overview; shipped as `rust-wallet/src/script/pushdrop.rs`. |
| `test_pushdrop_strategy.md` | The test strategy those results came from. |
| `TYPESCRIPT_SDK_MASTERKEYRING_COMPARISON.md` | MasterKeyring parity comparison; concluded matching. |
| `MESSAGE_TO_CERTIFIER_OPERATORS.md` | Draft outreach to certifier server operators; a one-time communication, not a spec. |
| `SOCIALCERT_DEEP_DIVE.md` | Mar 2026 deep dive on SocialCert social identity certificates; research, no code owner. |
| `APP_SCOPED_IDENTITY_IMPLEMENTATION.md` | App-scoped identity key design; the live behavior is the BRC-42/43 derivation documented in the Glossary. |

### Database & storage

| Doc | Reason archived |
|-----|-----------------|
| `DATABASE_IMPLEMENTATION_GUIDE.md` | Marked "Deferred to a dedicated browser-focused sprint"; that sprint happened and the schema is now V23. |
| `DATABASE_MIGRATION_IMPLEMENTATION_GUIDE.md` | Bring-up guide for the migration infrastructure; the live ledger is `rust-wallet/src/database/CLAUDE.md`. |
| `DATABASE_SCHEMA_DECISIONS.md` | Schema decision log from the V1 consolidation era. |
| `IMPLEMENTATION_DECISIONS_SUMMARY.md` | Companion decision summary ("tables ready, caching to follow"); caching shipped. |
| `BASKET_IMPLEMENTATION_PLAN.md` | Basket support plan; implemented (`BasketRepository`, `listOutputs`). |
| `ISSUE8_BASKET_TAG_IMPLEMENTATION.md` | Issue #8 basket + tag implementation notes; closed. |
| `ISSUE8_SDK_COMPARISON.md` | SDK comparison for the same issue; closed. |
| `GROUP_C_EXECUTION_PLAN.md` | Group C (output/basket/certificate management) execution plan; endpoints all live. |
| `PHASE4_UTXO_MANAGEMENT_RESEARCH.md` | UTXO management research; superseded by the `outputs` table model and `TaskSyncPending`. |

### Wallet backend, state & hardening

| Doc | Reason archived |
|-----|-----------------|
| `WALLET_ASSESSMENT_27JAN.md` | Jan 2026 assessment + cleanup sprint; the untested methods it flagged have since been exercised against real dApps. |
| `WALLET_BACKUP_AND_RECOVERY_PLAN.md` | Backup/recovery outline; implemented as `backup.rs` + the on-chain backup endpoints + `TaskBackup`. |
| `WALLET_INFO_STORAGE_GUIDE.md` | Pre-SQLite wallet-info storage guide (the `identity.json` era). |
| `WALLET_PHASE1_RESEARCH_FINDINGS.md` | Phase 1 wallet research; consumed. |
| `WALLET_SERVICE_FEE_IMPLEMENTATION.md` | Deliberately superseded: its content was folded into the **Wallet Service Fee** section of `/CLAUDE.md` at 3a7007d, which is now the implementation doc. |
| `WALLET_TOOLBOX_RS_COMPARISON.md` | Feature comparison against `wallet-toolbox-rs`; the deliberate divergences are recorded in memory instead. |
| `BSV_RUST_ECOSYSTEM_COMPARISON.md` | Survey of the Rust BSV ecosystem (noting `bsv-rs` was not yet on crates.io); a moment-in-time landscape scan. |
| `GO_WALLET_TOOLKIT_MIGRATION.md` | Plan to migrate onto `go-wallet-toolbox` — **a direction not taken.** The wallet is Rust. |
| `RUST_WALLET_SESSION_SUMMARY.md` | Session summary from the initial Rust wallet build-out. |
| `Wallet_Panel_functionality.md` | Jan 2026 plan to restore wallet-panel functionality; the panel was rebuilt in the UX_UI sprint. |
| `waitForAuthentication_implementation_guide.md` | Implementation guide, marked Implemented. Endpoint is live (BRC-100 Call Code 24). |
| `RECOVERY_BALANCE_BUG_INVESTIGATION.md` | Apr 2026 recovery balance-inflation investigation; resolved. |
| `STATE_MAINTENANCE_AND_RECONCILIATION_TRANSITION_PLAN.md` | Transition plan left at "Phases 1-6 complete, 7-8 pending"; superseded by the Monitor task set and the Wallet-Hardening WS1 work. |
| `STATE_RECONCILIATION_ASSESSMENT.md` | Feb 2026 assessment feeding that plan. |
| `CHAIN_TRUTH_HARDENING_RESEARCH.md` | Research project behind the chain-truth hardening commit (1fa686f) — backup adoption, double-spend verification, output safety. Shipped. |
| `PHASE_7_PERFORMANCE_OPTIMIZATION_PLAN.md` | Wallet performance plan; overtaken by the Phase 1.6 latency work and the 0.4.0 startup optimization. |
| `PHASE_9_CLEANUP_AND_DOCUMENTATION_PLAN.md` | Marked "DEFERRED — will coordinate with open source protocol developers"; never resumed. |
| `PART2_RESEARCH_SUMMARY.md` | Blockchain-query research (`getHeight` and friends); all three endpoints shipped. |

### Paymail

| Doc | Reason archived |
|-----|-----------------|
| `PAYMAIL_IMPLEMENTATION_GUIDE.md` | Implementation guide; shipped as `rust-wallet/src/paymail.rs` + the two Paymail endpoints. |
| `VENDOR_NEUTRAL_PAYMAIL_ANALYSIS.md` | Analysis of vendor-neutral Paymail architecture; informed the above, no longer a plan. |

### Browser shell, UI & privacy

| Doc | Reason archived |
|-----|-----------------|
| `TABBED_BROWSING_IMPLEMENTATION_PLAN.md` | Oct 2025 tabbed-browsing plan; shipped as `TabManager` (plus tear-off and multi-window since). |
| `TABBED_IMPLEMENTATION_STATUS.md` | Its status tracker; all rows closed. |
| `POPUP_TO_TAB_FIX_PLAN.md` | Plan to route popups into tabs; fixed. |
| `POPUP_TO_TAB_DEBUGGING.md` | The debugging notes behind it. |
| `HEADER_UI_IMPROVEMENTS_PLAN.md` | Early header plan; superseded by the 0.4.0 Header/Omnibox UX pass. |
| `OVERLAY_DPI_SCALING.md` | Overlay DPI scaling, marked IMPLEMENTED 2026-04-02. The live DPI gate is `development-docs/DevOps-CICD/DPI_RESOLUTION_TEST_MATRIX.md`. |
| `WINDOW_CLOSE_PRIMARY_TRANSFER.md` | Primary-window-close transfer, marked IMPLEMENTED 2026-04-02; behavior now lives in `WindowManager`. |
| `CEF_REFINEMENT_TRACKER.md` | Feb 2026 CEF refinement phase tracker; all phases closed. |
| `cef-native-performance-audit.md` | Jul 2025 performance audit of the C++ shell; findings actioned or overtaken. |
| `frontend-ui-ux-cleanup-optimization.md` | Mar 2026 frontend cleanup report; actioned. |
| `UX_FEATURE_COMPARISON_AND_ROADMAP.md` | Dec 2025 UX comparison + roadmap; the roadmap shipped through the browser-core and 0.4.0 sprints. |
| `BSV_BROWSER_COMPARISON.md` | Competitive analysis of other BSV browsers; a market snapshot. |
| `cloudflare-bot-bypass-strategy.md` | Mar 2026 plan for Cloudflare/bot-detection handling; not implemented as written — no owner. |
| `data-storage-and-encryption-review.md` | Comprehensive storage/encryption/TLS review; superseded by `SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md`. |
| `CORS_EXPLANATION.md` | Explainer written while diagnosing the CORS block; the resolution is the IPC bridge (`development-docs/architecture/IPC_BRIDGE.md`). |

### Testing logs & session records

| Doc | Reason archived |
|-----|-----------------|
| `REAL_WORLD_TESTING_LOG.md` | Dec 2025 real-world testing log; a point-in-time record. |
| `TEDIOUS_TEST_LOG.md` | Mar 2026 test log for issue #49. |
| `MVP_BETA_ISSUES.md` | MVP beta issue tracker; superseded by GitHub issues and the beta train. |
| `SESSION_SUMMARY_2025-10-27.md` | Single-session summary; the pattern was replaced by the memory files. |
| `Developer_notes.md` | Loose developer notes, ending in "✅ FIXED"; a scratchpad. |
| `phase-5-github-issue.md` | A copy/paste-ready GitHub issue body; filed. |

### macOS & external reviewers

| Doc | Reason archived |
|-----|-----------------|
| `MACOS_DEV_SETUP_GUIDE.md` | Step-by-step macOS dev setup; superseded by `build-instructions/MACOS_BUILD_INSTRUCTIONS.md`. |
| `macos-port-track-a-handoff-2026-03-09.md` | Point-in-time session handoff (Track A complete, issues #7/#8/#9/#12/#13/#14/#27 closed). Sat at the repo root until 28f23c8. |
| `questions_for_mac_devs.md` | Question list for the macOS contributors; answered. |
| `Edwin-initial-look.md` | Feb 2026 first-impressions review from an external reviewer. |

---

## `Final-MVP-Sprint/` (6 files)

Retired at f37adfe. The sprint itself lives on at
`development-docs/Final-MVP-Sprint/`; these are the parts that concluded.

| Doc | Reason archived |
|-----|-----------------|
| `AI-HANDOFF.md` | Dormant cross-session AI communication log; superseded by the memory files. |
| `OPTIMIZATION_PRIORITIES.md` | Framing ("what to fix before testers run") expired 26+ betas ago. |
| `post-beta3-cleanup.md` | Point-in-time cleanup sweep, parked at beta.3. |
| `sprint-4-investigation.md` | Apr 2026 investigation into B-6 single-instance + B-3 uninstall cleanup; both shipped. |
| `macos-port/PROGRESS.md` | macOS port progress dashboard; a running log, superseded by the memory files. |
| `macos-port/Track-B-UI-Overlays.md` | Assigns macOS overlay work that is now all implemented (14 overlays, Windows/macOS parity). |

## `Wallet-Hardening/` (8 files)

Retired at f37adfe. The live hardening docs (`WALLET_HARDENING_ROADMAP.md`,
`RECONCILE_PHASE2_DESIGN.md`, `FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md`) stayed in
`development-docs/`; their inbound links were repointed here.

| Doc | Reason archived |
|-----|-----------------|
| `MAC_BACKUP_FAILURE_HANDOFF.md` | Opening handoff of the mac-backup NULLFAIL investigation arc. |
| `MAC_BACKUP_FAILURE_CODE_HUNT.md` | Windows-side code hunt feeding that investigation. |
| `MAC_BACKUP_FAILURE_FINDINGS.md` | Forensic findings from the same arc. |
| `MAC_BACKUP_NULLFAIL_NEXT_STEPS.md` | Diagnostic next-steps brief handed to the Mac session. |
| `MAC_BACKUP_NULLFAIL_RESULTS.md` | The conclusion — SOLVED 2026-07-14, Keychain root cause. Whole arc closed. |
| `FIX_A_RECONCILE_PLAN.md` | Pre-code plan, "design complete, NOT implemented"; superseded by `RECONCILE_PHASE2_DESIGN.md`. |
| `RECONCILE_SPENT_INPUTS_PLAN.md` | Generalized spent-input sprint plan; superseded by the same, plus the commits that landed `reconcile.rs`. |
| `C4_C5_WIRING_PLAN.md` | Wiring plan for spent-input reconcile into the money path; superseded. |

## `Future-Features/` (2 files)

Retired at f37adfe. `development-docs/Future-Features/` remains live.

| Doc | Reason archived |
|-----|-----------------|
| `BRC52_CERTIFICATE_RESEARCH_PLAN.md` | Both gaps it plans against are shipped. |
| `browser-extensions/IMPLEMENTATION_OUTLINE.md` | **Premise refuted** — plans against CEF Chrome-Runtime extension support that a spike disproved. |

## `Mac-port/` (1 file)

| Doc | Reason archived |
|-----|-----------------|
| `PHASE1_PORTABLE_BUILD_SUMMARY.md` | Phase 1 portable-build configuration, marked COMPLETE; the macOS build is documented in `build-instructions/`. |

## `Settings_Sprints/` (18 files)

A settings sprint organized as paired `X.md` / `X-DETAILED.md` docs. The
outcomes landed in `SettingsManager` + `SettingsPage.tsx`; several items were
never started and were absorbed by the 0.4.0 sprint instead. Archived as a
whole because the sprint structure was retired, not because every item shipped.

> ⚠️ The `-DETAILED.md` files all read "Status: Not Started" even where the
> paired summary says COMPLETE — the detail docs were written up front and
> never re-stamped. Trust the summary, not the detail doc, on status.

| Doc | Reason archived |
|-----|-----------------|
| `00-SPRINT-INDEX.md` | The sprint index; sprint retired. |
| `D1-download-settings.md` | COMPLETE 2026-03-01 — download settings shipped. |
| `D1-download-settings-DETAILED.md` | Its implementation detail doc. |
| `G1-search-engine.md` | Never started as a sprint item; default-search-engine now lives in `BrowserSettings.searchEngine`. |
| `G1-search-engine-DETAILED.md` | Its detail doc. |
| `G2-session-restore.md` | COMPLETE (phases 1+2, 2026-03-02) — `restoreSessionOnStart`. |
| `G2-session-restore-DETAILED.md` | Its detail doc. |
| `G3-bookmark-bar.md` | Not started here; bookmarks shipped instead via the 0.4.0 B3 track. |
| `G3-bookmark-bar-DETAILED.md` | Its detail doc. |
| `G4-new-tab-page.md` | Phase 1 complete 2026-03-02; tab drag-reorder and tear-off shipped later under browser-core sprint 13. |
| `G4-new-tab-page-DETAILED.md` | Its detail doc. |
| `G5-default-browser.md` | Not started; set-as-default-browser remains unimplemented. |
| `G5-default-browser-DETAILED.md` | Its detail doc. |
| `PS1-global-shield-toggles.md` | Complete — the privacy shield toggles shipped (`usePrivacyShield`). |
| `PS1-global-shield-toggles-DETAILED.md` | Its detail doc. |
| `PS2-clear-browsing-data.md` | Not started under this sprint; clear-browsing-data shipped via browser-core sprint 9c. |
| `PS3-clear-data-on-exit.md` | Not started here; `clearDataOnExit` shipped and is a known bookmark-loss suspect. |
| `PS3-clear-data-on-exit-DETAILED.md` | Its detail doc. |

## `UX_UI/` (21 files)

The completed wallet UI/UX sprint — setup/recovery, notifications, light
wallet, peer payments, activity indicator. Its output is the wallet overlay
surface that ships today. Archived as a closed sprint.

| Doc | Reason archived |
|-----|-----------------|
| `00-IMPLEMENTATION_INDEX.md` | Sprint index; sprint closed. |
| `CLAUDE.md` | The sprint's own AI-context file; the folder is no longer worked in. |
| `IMPLEMENTATION_OUTLINE.md` | Top-level outline for the whole UX/UI arc. |
| `HTTP_INTERCEPTOR_FLOW_GUIDE.md` | Jan 2026 interceptor flow guide — **stale in a load-bearing way**: it predates the Rust permission engine. Use `development-docs/architecture/AUTO_APPROVE_ENGINE.md`. |
| `helper-1-implementation-guide-checklist.md` | Working checklist for the UI enhancement pass. |
| `helper-2-design-philosophy.md` | Design principles for the Web3 UI; superseded in practice by the shipped surface. |
| `helper-3-ux-considerations.md` | UX considerations for dangerous-operation warnings; realized in the permission modals. |
| `helper-4-branding-colors-logos.md` | Colour + logo guidelines from Feb 2026. |
| `phase-0-startup-flow-and-wallet-checks.md` | COMPLETE 2026-02-13 — startup flow and wallet checks. |
| `phase-1-initial-setup-recovery.md` | Setup/recovery interface plan; shipped as `WalletPanelPage` + `BackupOverlayRoot`. |
| `phase-1d-raw-private-key-recovery.md` | Raw private-key recovery add-on; shipped (`wallet_recover_external`). |
| `phase-2-research-findings.md` | Research behind domain permissions + user notifications. |
| `phase-2-user-notifications.md` | The notification/consent-prompt plan; shipped as the shared notification overlay + `BRC100AuthOverlayRoot`. |
| `phase-3-light-wallet.md` | Light wallet interface plan; shipped. |
| `phase-3-peer-payments-research.md` | Peer-payments research feeding phase 3a. |
| `PHASE_3_IMPLEMENTATION_PLAN.md` | Combined light-wallet + peer-payments implementation plan. |
| `phase-3a-brc29-peer-payments.md` | BRC-29 peer payments; shipped (PeerPay + MessageBox + `TaskCheckPeerPay`). |
| `phase-3b-paymail-identity-resolution.md` | Paymail + identity name resolution; marked IMPLEMENTED. |
| `phase-3-test-checklist.md` | Test checklist for phase 3; executed. |
| `phase-4-full-wallet.md` | Advanced wallet dashboard summary (Mar 2026); shipped. |
| `phase-5-activity-status-indicator.md` | Passive activity indicator plan. Note the **gold pill** payment indicator that ships today came from Phase 1.5, not this doc. |

## `browser-core/` (22 files)

The browser-core audit and MVP sprint (sprints 8–13): adblock, cookies,
fingerprinting, profiles, settings, tab tear-off. Index says "All Phases
Complete". Archived as a closed sprint; its output is most of what
`cef-native/src/core/` does today.

| Doc | Reason archived |
|-----|-----------------|
| `00-SPRINT-INDEX.md` | Sprint index — all phases complete. |
| `CLAUDE.md` | The sprint's own AI-context file. |
| `01-chrome-brave-research.md` | Chrome/Brave research (Phase B); informed the adblock and privacy design. |
| `architecture-assessment.md` | Architecture cohesiveness assessment (Phase A.5). |
| `browser-capabilities.md` | Capabilities assessment (Phase A.4). |
| `file-inventory.md` | File + database inventory (Phase A.2) — superseded by the per-directory `CLAUDE.md` inventories. |
| `mvp-gap-analysis.md` | Gap analysis + prioritization (Phase C). |
| `implementation-plan.md` | The MVP implementation plan (Phase D) those phases produced. |
| `auth-cookies-profiles-guide.md` | Deep reference on web auth, cookies and how Brave handles them; background for the cookie-blocking design. |
| `multi-instance-profile-testing.md` | Multi-instance + profile testing strategy; the profile system shipped in 0.4.0. |
| `sprint-8-adblock-research.md` | Adblock research (Feb 2026); shipped as `adblock-engine/`. |
| `sprint-9-implementation-plan.md` | Settings persistence + profile import — marked ✅ COMPLETE. |
| `sprint-9-profile-account-research.md` | Research on how major browsers handle profiles and account login. |
| `sprint-9c-clear-data-research.md` | Clear-browsing-data research; implemented. |
| `sprint-10-scriptlet-compatibility-plan.md` | Scriptlet compatibility system; shipped (`hodos-unbreak.txt`, `#@#+js()`). |
| `sprint-11-menu-settings-plan.md` | Menu button UX + full-page settings; shipped (`MenuOverlay`, `SettingsPage`). |
| `sprint-12-cookie-fingerprint-plan.md` | Third-party cookie blocking + fingerprint protection; shipped (`CookieBlockManager`, `FingerprintProtection`). |
| `sprint-13-tab-tearoff-plan.md` | Tab tear-off / multi-window; shipped (`MoveTabToWindow`, `WindowManager`). |
| `sprint-10-11-12-implementation-prompt.md` | Copy-paste session-start prompt for those three sprints. |
| `sprint-10-11-12-review.md` | Plan review + recommendations before they ran. |
| `sprint-10-11-12-test-checklist.md` | Their combined test checklist; executed. |
| `test-site-basket.md` | Standard verification-site basket — **superseded, not dead**: the live version is the Testing Standards table in `/CLAUDE.md`. |

## `feature-research/` (6 files)

Early research on browser-data features. All three shipped (history,
bookmarks, cookies), so both the original and "REVISED" passes are historical.

| Doc | Reason archived |
|-----|-----------------|
| `README.md` | Index for the research set. |
| `README_REVISED.md` | Its revised pass, re-scoped to BRC-100 wallet operations exclusively. |
| `HISTORY_FEATURE_REVISED.md` | History feature research; shipped as `HistoryManager`. |
| `BOOKMARKS_FEATURE_REVISED.md` | Bookmarks research; shipped as `BookmarkManager` (UI landed in 0.4.0 B3). |
| `COOKIES_MANAGEMENT_FEATURE.md` | Cookie management research; shipped as `CookieManager` + `CookieBlockManager`. |
| `FAVORITES_COOKIES_REVISED.md` | Combined favorites + cookies revision of the two above. |

---

## Archiving something new

1. `git mv` it here (preserves history — never copy-and-delete).
2. Add its row above, in the right section, in the **same commit**.
3. Repoint any inbound relative links in still-live docs. A dangling link is
   how a reader learns a doc moved, and that is too late.
4. Say *why* in the reason column — shipped, superseded by X, premise refuted,
   abandoned, point-in-time record. "Old" is not a reason.

Do not archive anything under `development-docs/0.4.0/` or
`development-docs/Sigma-BRC121-Sprint/` — both are live or about to be. Those
get archived when each sprint closes, as whole folders.
