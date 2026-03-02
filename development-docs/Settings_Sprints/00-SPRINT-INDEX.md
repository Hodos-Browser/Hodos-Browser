# Settings — Sprint Index

**Created**: 2026-02-28
**Purpose**: Track implementation of settings features across all tabs. Each non-working setting has its own planning doc and sprint.

---

## Agreed Priority Order (2026-03-01)

| Priority | Sprint | Effort | Rationale |
|----------|--------|--------|-----------|
| ~~1~~ | ~~**PS1** (Global Shield Toggles)~~ | ~~Low~~ | **Done** (2026-03-01) |
| ~~2~~ | ~~**D1** (Download Settings)~~ | ~~Low~~ | **Done** (2026-03-01) |
| ~~3~~ | ~~**G1** (Search Engine + Suggest Swap)~~ | ~~Low~~ | **Done** (2026-03-02) |
| ~~4~~ | ~~**G4 Phase 1** (New Tab Page)~~ | ~~Medium~~ | **Done** (2026-03-02) |
| ~~4a~~ | ~~**G4 Phases 1b-3** (Ctrl+T fix, Homepage separation, Set as Homepage)~~ | ~~Medium~~ | **Done** (2026-03-02) |
| ~~4b~~ | ~~**G4 Phase 4** (Tab Drag-Reorder)~~ | ~~Low-Medium~~ | **Done** (2026-03-02) |
| 4c | **G4 Phase 5** (Tab Tear-Off) | High | Drag tab out → new window. Multi-window architecture refactor. |
| ~~5~~ | ~~**G2 Phase 1+2** (Session Restore)~~ | ~~Medium~~ | **Done** (2026-03-02) |
| ~~6~~ | ~~**PS3** (Clear on Exit)~~ | ~~Medium~~ | **Done** (2026-03-02) |
| ~~7~~ | ~~**OB1** (Omnibox Arrow Keys)~~ | ~~Low~~ | **Done** (2026-03-02) |
| — | ~~**G3**~~ (Bookmark Bar) | ~~High~~ | **Deferred** — remove placeholder toggle. |
| — | ~~**G5**~~ (Default Browser) | ~~Low~~ | **Deferred** — needs installer first (see working-notes.md A3). |

---

## General Tab

| Setting | UI Exists | Persists | Behavior Works | Sprint | Status |
|---------|-----------|----------|---------------|--------|--------|
| **Homepage** | Yes | Yes | Yes (launch only) | N/A — complete (Sprint 11b) | Done |
| **Search Engine** | Yes | Yes | Yes — DDG default, Google option | ~~G1~~ | **Done** (2026-03-02) |
| **Restore Previous Session** | Yes | Yes | Yes — save on shutdown, restore on startup | ~~G2~~ | **Done** (2026-03-02) |
| **Bookmark Bar** | No | Yes | **No** — no bookmark bar UI | ~~G3~~ | **Deferred** — placeholder toggle removed |
| **New Tab Page** | Yes | Yes (cache) | Yes — branded NTP with search + tiles + favicon cache | ~~G4 Phase 1~~ | **Done** (2026-03-02) |
| **Set as Default Browser** | No | N/A | **No** — no button to open OS default browser settings | ~~G5~~ | **Deferred** — needs installer |
| **Right-click "Set as Homepage"** | Yes | Yes | Yes — context menu calls `SettingsManager::SetHomepage()` | ~~G4 (Phase 3)~~ | **Done** (2026-03-02) |

## Privacy & Security Tab

| Setting | UI Exists | Persists | Behavior Works | Sprint | Status |
|---------|-----------|----------|---------------|--------|--------|
| **Ad & tracker blocking** | Yes | Yes | Yes — global master switch + per-site toggle | PS1 | **Done** |
| **Third-party cookie blocking** | Yes | Yes | Yes — global master switch, trackers always blocked | PS1 | **Done** |
| **Fingerprint protection** | Yes | Yes | Yes | N/A — complete (Sprint 12) | Done |
| **Do Not Track / GPC headers** | Yes | Yes | Yes | N/A — complete (Sprint 11b) | Done |
| **Manage browsing data** | Yes (link) | N/A | **Needs testing** — links to Browser Data page | N/A | **Test needed** |
| **Clear data on exit** | Yes | Yes | Yes — clears history, cookies, cache, session on clean shutdown | ~~PS3~~ | **Done** (2026-03-02) |
| **Blocked Domains list** | Yes | Yes | Yes | N/A — complete (Sprint 12) | Done |
| **Block Log** | Yes | Yes | Yes | N/A — complete (Sprint 12) | Done |

## Downloads Tab

| Setting | UI Exists | Persists | Behavior Works | Sprint | Status |
|---------|-----------|----------|---------------|--------|--------|
| **Default download folder** | Yes (Browse) | Yes | Yes — Win32 IFileSaveDialog opens in configured folder | D1 | **Done** |
| **Folder picker (Browse)** | Yes | N/A | Yes — Win32 IFileOpenDialog with "Select Folder" | D1 | **Done** |
| **Ask where to save each file** | Yes (toggle) | Yes | Yes — controls Save As vs silent download | D1 | **Done** |

---

## Sprint Docs — General

| Sprint | Feature | Doc | Complexity | Dependencies |
|--------|---------|-----|------------|--------------|
| **G1** | Default Search Engine + Suggest Swap | [G1-search-engine.md](./G1-search-engine.md) | Low | None. DDG default, Google fallback. Drop Bing/Brave. |
| **G2** | Session Restore | [G2-session-restore.md](./G2-session-restore.md) | Medium | None. Lazy tab loading for Phase 2. |
| ~~**G3**~~ | ~~Bookmark Bar~~ | ~~[G3-bookmark-bar.md](./G3-bookmark-bar.md)~~ | ~~High~~ | **Deferred** — remove placeholder toggle from settings UI. |
| **G4** | New Tab Page, Tab Drag-Reorder & Tear-Off | [G4-new-tab-page.md](./G4-new-tab-page.md) | High (6 phases) | G1 (done), HistoryManager, TabManager multi-window |
| ~~**G5**~~ | ~~Set as Default Browser~~ | ~~[G5-default-browser.md](./G5-default-browser.md)~~ | ~~Low~~ | **Deferred** — needs installer plan first (see working-notes.md A3). |

## Sprint Docs — Privacy & Security

| Sprint | Feature | Doc | Complexity | Dependencies |
|--------|---------|-----|------------|--------------|
| **PS1** | Global Shield Toggles | [PS1-global-shield-toggles.md](./PS1-global-shield-toggles.md) | Low-Medium | AdblockCache, EphemeralCookieManager |
| ~~**PS2**~~ | ~~Clear Browsing Data~~ | ~~[PS2-clear-browsing-data.md](./PS2-clear-browsing-data.md)~~ | ~~Medium~~ | Resolved — linked to existing Browser Data page (**needs testing**) |
| **PS3** | Clear Data on Exit | [PS3-clear-data-on-exit.md](./PS3-clear-data-on-exit.md) | Medium | Reuses Browser Data page clearing logic |

## Sprint Docs — Downloads

| Sprint | Feature | Doc | Complexity | Dependencies |
|--------|---------|-----|------------|--------------|
| **D1** | Download Settings | [D1-download-settings.md](./D1-download-settings.md) | Low-Medium | CEF RunFileDialog, SettingsManager |

## Sprint Docs — Omnibox

| Sprint | Feature | Doc | Complexity | Dependencies |
|--------|---------|-----|------------|--------------|
| ~~**OB1**~~ | ~~Arrow Key Navigation~~ | ~~(inline — no separate doc)~~ | ~~Low~~ | **Done** (2026-03-02) |

---

## Workflow

Each sprint follows this lifecycle:

1. **Research** — Understand how Chrome/Brave handle it, identify design decisions
2. **Plan** — Write implementation plan with phases, make decisions
3. **Implement** — Build in small phases, build-verify each (Windows only for now)
4. **Test** — Verify with test site basket
5. **Polish** — UX refinements, edge cases
6. **macOS notes** — If the sprint includes platform-specific C++ code, add notes to `development-docs/macos-port/MAC_PLATFORM_SUPPORT_PLAN.md` for future porting

---

## Design Decisions Log

| Date | Decision | Context |
|------|----------|---------|
| 2026-03-01 | DDG default, Google fallback, drop Bing/Brave | Privacy brand alignment. DDG business model is legitimate (contextual ads, not data selling). |
| 2026-03-01 | Lazy tab loading for session restore (G2) | Only active tab loads content on restore. Others show title/favicon, load on click. Chrome approach. |
| 2026-03-01 | Defer bookmark bar (G3) | Low personal priority. Remove placeholder toggle. Backend works (Ctrl+D, import). |
| 2026-03-01 | Defer default browser (G5) | Useless without installer. Bundle with installer sprint. |
| 2026-03-01 | PS3 + G2 conflict resolution | If both "clear on exit" and "restore session" enabled, disable restore and warn. Don't allow both simultaneously. |

| 2026-03-02 | G1 done — DDG default, Google fallback | DDG search + suggest API wired to behavior. Bing/Brave removed. |
| 2026-03-02 | Tab drag-reorder + tear-off added to G4 | User request. Phases 4-5 added. Tear-off uses HWND reparenting within same process (not multi-instance). |
| 2026-03-02 | G4 Phase 1 done — branded NTP | Logo, search bar, most-visited tiles (8 max), localStorage cache for instant render, base64 favicon caching. Default BSV tiles for first-time users. |
| 2026-03-02 | G2 Phases 1+2 done — Session Restore | SaveSession() in ShutdownApplication(), restore in OnContextInitialized(). All tabs load immediately (lazy deferred). session.json deleted after restore. |

---

**Last Updated**: 2026-03-02
