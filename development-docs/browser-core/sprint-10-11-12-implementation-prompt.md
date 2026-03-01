# Sprints 10-12 Implementation Prompt

**Purpose**: Copy-paste this as a prompt to start a fresh Claude Code session. It provides all context needed to implement Sprints 10, 11, and 12 autonomously.

**Date**: 2026-02-25

---

## Prompt

Implement Browser Core Sprints 10, 11, and 12 for Hodos Browser. Work autonomously — implement everything, build after each sprint, and create a comprehensive test checklist document at the end. Do NOT attempt to run the browser — the user will test manually when they return.

### What to implement

**Sprint 10: Scriptlet Compatibility System** (plan: `development-docs/browser-core/sprint-10-scriptlet-compatibility-plan.md`)

Phases:
- **10a**: Create `hodos-unbreak.txt` exception filter list + load in adblock engine. **CRITICAL FIRST STEP**: Write a unit test verifying `#@#+js()` works in adblock-rust 0.10.3 BEFORE anything else. If it doesn't work, implement C++ domain-check bypass in `simple_handler.cpp` instead.
- **10b**: Per-site scriptlet toggle backend — V6 migration (`scriptlets_enabled` column), Rust endpoints, C++ `DomainPermissionCache` integration, `skip_scriptlets` param on `/cosmetic-resources`.
- **10c**: Privacy Shield UI update — add scriptlet toggle row to PrivacyShieldPanel, `useScriptlets` hook, `usePrivacyShield` composition update, IPC handler.
- 10d is testing — skip (user will test manually).

**Sprint 11: Menu Button + Full-Page Settings** (plan: `development-docs/browser-core/sprint-11-menu-settings-plan.md`)

REVISED phasing (different from the plan doc):
- **11a**: Three-dot menu button (React component in MainBrowserView, NOT a separate overlay) + settings page skeleton with sidebar + 4 critical settings sections: General, Privacy & Security, Downloads, About Hodos. Route in App.tsx. C++ IPC handlers for new menu actions (print, devtools, zoom, exit, new_window, view_source). Remove dedicated History/Downloads/Settings toolbar icons. Ensure single-settings-tab enforcement.
- **11b**: Remaining settings sections (Appearance, Wallet, Import, Profiles) + wire settings to actual behavior (homepage, search engine, zoom, DNT header, downloads path, clear-on-exit) + retire settings overlay HWND.

**Sprint 12: Third-Party Cookie Blocking + Fingerprinting** (plan: `development-docs/browser-core/sprint-12-cookie-fingerprint-plan.md`)

Phases per plan, with this modification:
- **REMOVE screen resolution spoofing from Standard mode** in 12d. Brave removed it because breakage > entropy benefit (only 3-4 bits). Canvas + WebGL + Navigator + Audio farbling is sufficient.
- 12f is testing — skip (user will test manually).

### Build commands

After each sprint, build all affected components:
- **Rust adblock**: `cd /mnt/c/Users/archb/Hodos-Browser/adblock-engine && /mnt/c/Users/archb/.cargo/bin/cargo.exe build --release`
- **Rust wallet**: `cd /mnt/c/Users/archb/Hodos-Browser && /mnt/c/Users/archb/.cargo/bin/cargo.exe build --release`
- **Frontend**: `cd /mnt/c/Users/archb/Hodos-Browser/frontend && PATH="/mnt/c/Program Files/nodejs:$PATH" "/mnt/c/Program Files/nodejs/node.exe" ./node_modules/vite/bin/vite.js build` (run tsc first: `"/mnt/c/Program Files/nodejs/node.exe" node_modules/typescript/bin/tsc --noEmit`)
- **C++**: `cd /mnt/c/Users/archb/Hodos-Browser/cef-native && "/mnt/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe" --build build --config Release`

### Key constraints

1. **Rust crate pinning**: `adblock = "=0.10.3"`, `rmp = "=0.8.14"`, `actix-web = "=4.11.0"`. Do NOT upgrade these — newer versions need unstable Rust features.
2. **Cross-platform**: All C++ must have `#ifdef _WIN32` / `#elif defined(__APPLE__)` conditionals. macOS stubs are OK.
3. **CEF input patterns**: Use native `<input>` elements in overlays, NOT MUI TextField. See CLAUDE.md "CEF Input Patterns" section.
4. **Overlay model**: Never add panels directly to MainBrowserView. Settings page is the ONE exception (it's a tab, not an overlay). The three-dot menu IS rendered inside MainBrowserView (it's trusted UI, not web content).
5. **DB migration numbering**: Current schema is V5 (adblock_enabled). Sprint 10 uses V6 (scriptlets_enabled).
6. **Do NOT run the browser** — user will test manually.
7. **Read files before editing** — always use Read tool before Edit.
8. Check `~/.claude/projects/-mnt-c-Users-archb-Hodos-Browser/memory/MEMORY.md` for build environment details and past patterns.
9. **Settings page uses existing `useSettings` hook** — the IPC to `SettingsManager` is already wired from Sprint 9a.

### Final deliverable

After all three sprints are implemented and built, create a comprehensive test document at `development-docs/browser-core/sprint-10-11-12-test-checklist.md`. Include:
- Per-sprint verification steps (what to test, expected behavior)
- Test site matrix (which sites to test, what to look for)
- Known risks and things to watch for
- Build verification steps
- Regression checks (make sure existing features still work)

### Key files to read first

Before starting, read these files to understand current state:
- `CLAUDE.md` (root) — project rules and architecture
- `development-docs/browser-core/CLAUDE.md` — sprint context
- `development-docs/browser-core/sprint-10-scriptlet-compatibility-plan.md`
- `development-docs/browser-core/sprint-11-menu-settings-plan.md`
- `development-docs/browser-core/sprint-12-cookie-fingerprint-plan.md`
- `adblock-engine/src/engine.rs` — current adblock engine
- `frontend/src/pages/MainBrowserView.tsx` — current toolbar layout
- `frontend/src/components/PrivacyShieldPanel.tsx` — current shield panel
- `cef-native/src/handlers/simple_handler.cpp` — IPC dispatch hub
- `cef-native/include/core/SettingsManager.h` — settings singleton
- `rust-wallet/src/database/migrations.rs` — migration numbering

### Implementation order

1. Sprint 10a (verify `#@#+js()`, create exception list)
2. Sprint 10b (backend: migration, endpoints, C++ integration)
3. Sprint 10c (frontend: shield panel update)
4. Build all, fix any errors
5. Sprint 11a (menu button + 4 settings sections + C++ IPC)
6. Build all, fix any errors
7. Sprint 11b (remaining settings + wiring + overlay retirement)
8. Build all, fix any errors
9. Sprint 12a (eTLD+1 cookie detection upgrade)
10. Sprint 12b (cookie policy engine + exceptions)
11. Sprint 12c (fingerprint session seed infrastructure)
12. Sprint 12d (canvas/WebGL/navigator/audio farbling — NO screen resolution)
13. Sprint 12e (privacy shield + settings integration)
14. Build all, fix any errors
15. Create test checklist document
16. Update `development-docs/browser-core/CLAUDE.md` sprint status tracker
17. Update `development-docs/browser-core/00-SPRINT-INDEX.md`
18. Update root `CLAUDE.md` Key Files table if new key files added
