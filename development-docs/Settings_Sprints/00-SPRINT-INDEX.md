# Settings — Sprint Index

**Created**: 2026-02-28
**Purpose**: Track implementation of settings features across all tabs. Each non-working setting has its own planning doc and sprint.

---

## Agreed Priority Order (2026-03-01)

| Priority | Sprint | Effort | Rationale |
|----------|--------|--------|-----------|
| 1 | **PS1** (Global Shield Toggles) | Low | Broken UI — toggles do nothing. Must-fix. |
| 2 | **D1** (Download Settings) | Low | Quick win, basic expected functionality. |
| 3 | **G1** (Search Engine + Suggest Swap) | Low | ~30 min, unblocks G4, includes suggest API swap. |
| 4 | **G4 Phase 1** (New Tab Page) | Medium | Highest brand impact. |
| 5 | **G2 Phase 1+2** (Session Restore) | Medium | Core browser expectation. Lazy tab loading. |
| 6 | **PS3** (Clear on Exit) | Medium | Privacy feature. |
| 7 | **OB1** (Omnibox Arrow Keys) | Low | Keyboard navigation through dropdown. |
| — | ~~**G3**~~ (Bookmark Bar) | ~~High~~ | **Deferred** — remove placeholder toggle. |
| — | ~~**G5**~~ (Default Browser) | ~~Low~~ | **Deferred** — needs installer first (see working-notes.md A3). |

---

## General Tab

| Setting | UI Exists | Persists | Behavior Works | Sprint | Status |
|---------|-----------|----------|---------------|--------|--------|
| **Homepage** | Yes | Yes | Yes (launch only) | N/A — complete (Sprint 11b) | Done |
| **Search Engine** | Yes | Yes | **No** — hardcoded to Google | G1 | Not Started |
| **Restore Previous Session** | Yes | Yes | **No** — no save/restore logic | G2 | Not Started |
| **Bookmark Bar** | Yes | Yes | **No** — no bookmark bar UI | ~~G3~~ | **Deferred** — remove placeholder |
| **New Tab Page** | No | No | **No** — new tabs open external URL, no branded page | G4 | Not Started |
| **Set as Default Browser** | No | N/A | **No** — no button to open OS default browser settings | ~~G5~~ | **Deferred** — needs installer |
| **Right-click "Set as Homepage"** | No | N/A | **No** — no context menu option | G4 (Phase 3) | Not Started |

## Privacy & Security Tab

| Setting | UI Exists | Persists | Behavior Works | Sprint | Status |
|---------|-----------|----------|---------------|--------|--------|
| **Ad & tracker blocking** | Yes | Yes | **No** — global toggle ignored; only per-site toggle works | PS1 | Not Started |
| **Third-party cookie blocking** | Yes | Yes | **No** — always on (ephemeral CM); toggle has no effect | PS1 | Not Started |
| **Fingerprint protection** | Yes | Yes | Yes | N/A — complete (Sprint 12) | Done |
| **Do Not Track / GPC headers** | Yes | Yes | Yes | N/A — complete (Sprint 11b) | Done |
| **Manage browsing data** | Yes (link) | N/A | **Needs testing** — links to Browser Data page | N/A | **Test needed** |
| **Clear data on exit** | Yes | Yes | **No** — stored but never read on shutdown | PS3 | Not Started |
| **Blocked Domains list** | Yes | Yes | Yes | N/A — complete (Sprint 12) | Done |
| **Block Log** | Yes | Yes | Yes | N/A — complete (Sprint 12) | Done |

## Downloads Tab

| Setting | UI Exists | Persists | Behavior Works | Sprint | Status |
|---------|-----------|----------|---------------|--------|--------|
| **Default download folder** | Yes (text input) | Yes | **No** — stored but never read by OnBeforeDownload | D1 | Not Started |
| **Folder picker (Browse)** | No | N/A | **No** — user must type path manually | D1 | Not Started |
| **Ask where to save each file** | No | No | **No** — hardcoded to always show Save As | D1 | Not Started |

---

## Sprint Docs — General

| Sprint | Feature | Doc | Complexity | Dependencies |
|--------|---------|-----|------------|--------------|
| **G1** | Default Search Engine + Suggest Swap | [G1-search-engine.md](./G1-search-engine.md) | Low | None. DDG default, Google fallback. Drop Bing/Brave. |
| **G2** | Session Restore | [G2-session-restore.md](./G2-session-restore.md) | Medium | None. Lazy tab loading for Phase 2. |
| ~~**G3**~~ | ~~Bookmark Bar~~ | ~~[G3-bookmark-bar.md](./G3-bookmark-bar.md)~~ | ~~High~~ | **Deferred** — remove placeholder toggle from settings UI. |
| **G4** | New Tab Page & Homepage | [G4-new-tab-page.md](./G4-new-tab-page.md) | Medium-High (4 phases) | G1 (search engine for search bar), HistoryManager |
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
| **OB1** | Arrow Key Navigation | (inline — no separate doc) | Low | None. Add Up/Down arrow key navigation through omnibox dropdown. |

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

---

**Last Updated**: 2026-03-01
