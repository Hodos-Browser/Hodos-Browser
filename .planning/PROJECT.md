# HodosBrowser - Address Box Feature

## What This Is

A unified address bar enhancement for HodosBrowser that provides Chrome-style autocomplete suggestions. When users type in the existing address bar, an overlay appears below showing up to 6 suggestions combining browser history and Google search suggestions, ranked by relevance. Users can navigate suggestions with keyboard or click to navigate immediately.

## Core Value

Users can quickly navigate to URLs or search the web by typing in one place, with intelligent suggestions that learn from their browsing history.

## Requirements

### Validated

Existing browser capabilities that are already working:

- ✓ Three-layer architecture (React → CEF Shell → Rust Wallet) — existing
- ✓ Overlay system with isolated CEF subprocesses — existing
- ✓ Browser history tracking in SQLite database — existing
- ✓ Address bar that accepts URLs and syncs with current page — existing
- ✓ Tab management and navigation — existing
- ✓ V8 injection for `window.hodosBrowser.*` API — existing
- ✓ BRC-100 authentication with overlay pattern — existing

### Active

New HodosAddressBox feature:

- [ ] Overlay appears below address bar when user starts typing
- [ ] Overlay shows up to 6 suggestions (history + Google mixed by relevance)
- [ ] History suggestions query SQLite database and match on URL/title
- [ ] Google autocomplete API integration for search suggestions
- [ ] Relevance ranking prioritizes: URL suggestions > search suggestions, frequency + recency for history
- [ ] Visual indicators distinguish history suggestions from Google search suggestions
- [ ] Keyboard navigation (up/down arrows to select, Enter to navigate, Escape to close)
- [ ] Click suggestion to navigate immediately
- [ ] URL vs search query detection: valid URL/localhost/IP → navigate, else → Google search
- [ ] Overlay dismisses after navigation or when clicking outside or pressing Escape
- [ ] Overlay positioned aligned with address bar left edge and matches width

### Out of Scope

- Bookmark suggestions — explicitly excluded for initial version
- Private/incognito mode handling — no incognito mode exists yet
- macOS compatibility — Windows build first, Mac later
- Real-time synchronization across devices — future feature
- Search engine selection — Google only for now

## Context

**Existing Architecture:**
- Address bar exists in TypeScript header (runs in separate window from webview)
- Overlay system proven working with WalletPanelPage.tsx and WalletPanel.tsx pattern
- Deprecated wallet panel exists in codebase but should not be used as reference
- History stored in SQLite at `%APPDATA%/HodosBrowser/Default/History` (inferred from architecture)
- C++ CEF shell handles window management and IPC between layers

**Technical Environment:**
- Windows development environment (Visual Studio 2022, vcpkg)
- React 19.1.0 + TypeScript 5.8.3 frontend on Vite dev server (localhost:5137)
- CEF 136 browser shell (C++17)
- Existing V8 injection pattern for exposing APIs to JavaScript

**Design Decisions:**
- Follow WalletPanelPage.tsx overlay pattern (proven working)
- Use standard browser relevance algorithm (exact match > frequency > recency)
- 6 suggestions total (flexible mix of history and Google results)

## Constraints

- **Platform**: Windows build only — macOS compatibility deferred to future work
- **Tech Stack**: Must use existing React/TypeScript/CEF/C++ architecture — no new frameworks
- **UI Architecture**: Cannot render TypeScript components over webview — must use overlay subprocess pattern
- **API Dependencies**: Requires Google autocomplete API (cost unknown, user will evaluate)
- **History Database**: Must work with existing SQLite history schema — no schema changes without approval

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use overlay system instead of inline dropdown | TypeScript header can't render over webview; overlay proven with wallet panel | — Pending |
| Mix history + Google suggestions by relevance (not separate sections) | Cleaner UX, prioritizes most useful results regardless of source | — Pending |
| 6 total suggestions | Balance between useful options and visual clutter | — Pending |
| URL suggestions take precedence over Google suggestions | Users navigating to known sites is higher priority than search discovery | — Pending |
| Navigate immediately on suggestion click | Standard browser behavior, reduces friction | — Pending |

---
*Last updated: 2026-01-28 after initialization*
