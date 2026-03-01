# Settings — Sprint Index

**Created**: 2026-02-28
**Purpose**: Track implementation of settings features across all tabs. Each non-working setting has its own planning doc and sprint.

---

## General Tab

| Setting | UI Exists | Persists | Behavior Works | Sprint | Status |
|---------|-----------|----------|---------------|--------|--------|
| **Homepage** | Yes | Yes | Yes (launch only) | N/A — complete (Sprint 11b) | Done |
| **Search Engine** | Yes | Yes | **No** — hardcoded to Google | G1 | Not Started |
| **Restore Previous Session** | Yes | Yes | **No** — no save/restore logic | G2 | Not Started |
| **Bookmark Bar** | Yes | Yes | **No** — no bookmark bar UI | G3 | Not Started |
| **New Tab Page** | No | No | **No** — new tabs open external URL, no branded page | G4 | Not Started |
| **Set as Default Browser** | No | N/A | **No** — no button to open OS default browser settings | G5 | Not Started |
| **Right-click "Set as Homepage"** | No | N/A | **No** — no context menu option | G4 (Phase 3) | Not Started |

## Privacy & Security Tab

| Setting | UI Exists | Persists | Behavior Works | Sprint | Status |
|---------|-----------|----------|---------------|--------|--------|
| **Ad & tracker blocking** | Yes | Yes | **No** — global toggle ignored; only per-site toggle works | PS1 | Not Started |
| **Third-party cookie blocking** | Yes | Yes | **No** — always on (ephemeral CM); toggle has no effect | PS1 | Not Started |
| **Fingerprint protection** | Yes | Yes | Yes | N/A — complete (Sprint 12) | Done |
| **Do Not Track / GPC headers** | Yes | Yes | Yes | N/A — complete (Sprint 11b) | Done |
| **Manage browsing data** | Yes (link) | N/A | Yes — links to Browser Data page (history/cookies/cache clearing) | N/A — complete | Done |
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
| **G1** | Default Search Engine | [G1-search-engine.md](./G1-search-engine.md) | Low-Medium | None |
| **G2** | Session Restore | [G2-session-restore.md](./G2-session-restore.md) | Medium | None |
| **G3** | Bookmark Bar | [G3-bookmark-bar.md](./G3-bookmark-bar.md) | High (multi-phase) | BookmarkManager backend exists |
| **G4** | New Tab Page & Homepage | [G4-new-tab-page.md](./G4-new-tab-page.md) | Medium-High (4 phases) | G1 (search engine for search bar), HistoryManager |
| **G5** | Set as Default Browser | [G5-default-browser.md](./G5-default-browser.md) | Low | OS protocol registration (installer) |

## Sprint Docs — Privacy & Security

| Sprint | Feature | Doc | Complexity | Dependencies |
|--------|---------|-----|------------|--------------|
| **PS1** | Global Shield Toggles | [PS1-global-shield-toggles.md](./PS1-global-shield-toggles.md) | Low-Medium | AdblockCache, EphemeralCookieManager |
| ~~**PS2**~~ | ~~Clear Browsing Data~~ | ~~[PS2-clear-browsing-data.md](./PS2-clear-browsing-data.md)~~ | ~~Medium~~ | Resolved — linked to existing Browser Data page |
| **PS3** | Clear Data on Exit | [PS3-clear-data-on-exit.md](./PS3-clear-data-on-exit.md) | Medium | Reuses Browser Data page clearing logic |

## Sprint Docs — Downloads

| Sprint | Feature | Doc | Complexity | Dependencies |
|--------|---------|-----|------------|--------------|
| **D1** | Download Settings | [D1-download-settings.md](./D1-download-settings.md) | Low-Medium | CEF RunFileDialog, SettingsManager |

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

**Last Updated**: 2026-02-28
