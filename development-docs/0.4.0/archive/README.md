# 0.4.0 Archive

Historical, shipped, superseded, and shelved docs from the 0.4.0 line — moved here on **2026-07-09** during the doc-consolidation pass so the active `../` set stays lean. Nothing here is deleted; `git mv` preserves full history. If you need the archaeology (why a decision was made, how a shipped feature was designed), it's here.

**Do not treat anything in this folder as current.** For current state see `../README.md` and the canonical process docs in `../../DevOps-CICD/`.

## What's here and why

### Shipped features / completed design docs
| Doc | Reason archived |
|-----|-----------------|
| `HEADER_UX_PHASE.md` | Header/Omnibox UX pass complete — chunks a–e landed+pushed (origin/0.4.0 @ 9ba4b7f). |
| `SITE_INFO_PERMISSIONS_DESIGN.md` | Site Info + Site Permissions phase fully implemented (b1a/b1b/b1b.1 + b2a/b2b/b3, smoke-passed). |
| `B3-bookmarks.md` | Bookmarks feature shipped 2026-06-19 (favicon/delete landed beta.23); historical item card. |
| `STARTUP_OPTIMIZATION.md` | Windows first-paint fix shipped (b9542aa + manifest 957962f); completed investigation log. |
| `WALLET_UI_BRIDGE_MIGRATION.md` | Wallet-UI→C++ bridge / dev-prod port deconfliction commits landed; OQ-1 beta.16 target long past. |
| `F5_SINGLE_INSTANCE_HANDOFF_DESIGN.md` | F5 single-instance handoff designed, implemented (SHIP verdict) and shipped in beta.23. |
| `WIN10_OVERLAY_ROOTCAUSE.md` | Win10 overlay cluster root-cause; F1/F2/F3/F5 all shipped in beta.23; investigation complete. |
| `WINDOW_INSTANCE_DECONFLICTION.md` | Window/single-instance/AUMID deconfliction plan LANDED (commits 1aeaedd/f9408fd). |

### macOS one-shot briefs / results (executed)
| Doc | Reason archived |
|-----|-----------------|
| `MACOS_0_4_0_EXECUTION_BRIEF_2026_07_07.md` | Completed macOS execution brief — M1–M3 done; picker parity landed, silent update live. |
| `MACOS_DROPDOWN_BUTTON_CONSISTENCY_BRIEF.md` | Mac dropdown button consistency implemented + shipped (commit 3958cff). |
| `MACOS_EXECUTION_RESULTS_2026_07_07.md` | Completed session results — all items PASS/FIXED and merged. |
| `MACOS_UPDATE_STABILITY_EXECUTION.md` | beta.16 macOS min-version regression fixed in beta.17 with real-update gate GREEN; superseded. |

### Superseded plans (replaced by the beta train + the CEF rebuild kickoff)
| Doc | Reason archived |
|-----|-----------------|
| `SPRINT_0_4_0_MASTER_PLAN.md` | Mid-June pre-code DRAFT snapshot; sprint reshaped and shipped through beta.26. **Still the richest source for the reconstructed source-edit list** (FEAT-B1-*, PIPE-A1) — mine it, but its statuses are stale. |
| `ORCHESTRATION_PLAN_0_4_0.md` | Sprint shipped via the beta train, not this wave graph; CEF-rebuild half now owned by `../CHROMIUM_CEF_SPRINT_KICKOFF.md`. |
| `PHASE2_BUGHUNT_2026_06_12.md` | Phase 2.6 closed; findings resolved/deferred; historical resolution log. |
| `B1-farbling-in-source.md` | Superseded stub — content folded into the canonical `../B1-farbling-design.md`. |
| `B2-header-to-cpp.md` | Header→C++ native-port proposal dropped; B2 resolved as "optimize React (no native port)". |

### Cleared backlogs
| Doc | Reason archived |
|-----|-----------------|
| `POST_BETA16_PLAN.md` | Post-beta.16 hardening plan; all Track-0 threads shipped across beta.17–26. |
| `POST_BETA22_FEEDBACK.md` | beta.22 feedback backlog; all P0/P1 shipped in beta.23–26. |
| `POST_SILENT_FIXLIST.md` | beta.19/20 polish backlog; all items shipped across beta.20–26 or explicitly shelved. |

### Shelved profile-picker / per-profile-wallet direction (owner decision)
| Doc | Reason archived |
|-----|-----------------|
| `PICKER_TASKBAR_INVESTIGATION.md` | Diagnostic feeding the now-shelved same-process picker / taskbar-pin effort; no code landed. |
| `PROFILE_PICKER_SAME_PROCESS_PLAN.md` | Same-process (Chrome-model) picker refactor SHELVED by owner; never implemented. |
| `PROFILE_STARTUP_PICKER_DESIGN.md` | Profile startup/pre-window picker shipped (live through beta.26); further evolution shelved. |
| `PROFILE_MANAGER_REVIEW.md` | Priority/security cluster (R1/R2/R3/R9) landed; remaining R-items deferred with the shelved direction. |

### Superseded auto-update design/build journals (from `../../DevOps-CICD/`)
Living behavior for all of these now lives in `../../DevOps-CICD/AUTO_UPDATE.md` + `../../DevOps-CICD/WINDOWS_AUTOUPDATE_PLAN.md`. These are the pre-code design + build journals, kept for archaeology.
| Doc | Reason archived |
|-----|-----------------|
| `AUTOUPDATE_6B_SUPERVISOR_DESIGN.md` | Build/review journal for the now-shipped, proven-live external rollback supervisor. |
| `AUTOUPDATE_PICKER_GATE_DESIGN.md` | Design for the picker-gate fix (v2 exact picker-exit wait) shipped at ae5beb6, proven live beta.25→26. |
| `AUTOUPDATE_SILENT_STATE_WRITER_DESIGN.md` | Commit #1 pre-code design for the silent-eligibility writer; flip is done + proven live. |
| `AUTO_UPDATE_AND_SIGNING_0_4_0.md` | 2026-06-22 research/design snapshot; designs implemented + shipped-live. Retains signing/SmartScreen-reputation reference material (the active signing *gate* is `../../DevOps-CICD/ORG_IDENTITY_SIGNING_MIGRATION.md`). |
| `A6_SILENT_UPDATE_TEST_PLAN.md` | Pre-implementation test plan replaced by the real silent-update sprint rigs; WinSparkle path dropped for the hybrid custom updater. |

> **Not archived (kept in place):** the `../HelicOps/` audit sub-archive — it is a self-contained cluster and `../HelicOps/AUDIT_FIX_TRACKER.md` is still the live security backlog.
