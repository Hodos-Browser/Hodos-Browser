# Hodos Browser - Chrome Omnibox Feature

## What This Is

A Web3 browser with integrated BSV wallet functionality, built on CEF (Chromium Embedded Framework). We're adding an intelligent address bar (omnibox) that combines URL navigation with search capabilities, featuring autocomplete from browser history and Google search integration. The omnibox will replace the existing address bar with a multifunctional input that intelligently detects whether to navigate to a URL or perform a Google search.

## Core Value

Fast, seamless switching between URL navigation and web search without the user having to think about which they're doing. The browser should intelligently detect intent and respond instantly.

## Requirements

### Validated

<!-- Shipped and confirmed valuable (inferred from existing codebase) -->

- ✓ Three-layer architecture: React frontend, CEF shell, Rust wallet backend — existing
- ✓ Material-UI React component library for consistent UI — existing
- ✓ SQLite database for browser history storage — existing
- ✓ HTTP interception and routing via C++ CEF layer — existing
- ✓ Cross-platform support (Windows and macOS) — existing
- ✓ V8 injection providing window.hodosBrowser API — existing
- ✓ Overlay management system for browser UI components — existing

### Active

<!-- Current scope for omnibox feature -->

- [ ] URL autocomplete from browser history database
- [ ] Google search integration when input is not a URL
- [ ] Intelligent URL vs search query detection
- [ ] Autocomplete dropdown showing mix of history URLs and Google search suggestions
- [ ] Chrome-like keyboard navigation (arrow keys, Tab to autocomplete, Enter to navigate/search)
- [ ] Google API setup and integration for search suggestions
- [ ] Replace existing address bar with enhanced omnibox component
- [ ] Cross-platform implementation (macOS-first, Windows-compatible)

### Out of Scope

<!-- Explicit boundaries with reasoning -->

- Calculator/unit conversions — Deferred to future version; focus is URL + search
- Custom site searches (e.g., 'wiki', 'yt' shortcuts) — Adds complexity; not essential for v1
- Bookmark integration in autocomplete — Keep scope focused on history + search
- Advanced search filters (date ranges, domain filters) — Unnecessary complexity for initial version
- Offline mode/caching for suggestions — Requires additional infrastructure; online-first is acceptable

## Context

**Implementation Architecture:**
See `.planning/phases/1-foundation-investigation/INVESTIGATION.md` for comprehensive documentation:
- Complete React component architecture (MainBrowserView address bar)
- HistoryManager C++ API reference and SQLite schema
- End-to-end data flow diagrams (React → IPC → C++ → SQLite)
- Replace vs Reuse decision matrix for all components
- IPC protocol documentation
- SQL query examples for autocomplete

**Existing Address Bar:**
- Basic address bar in MainBrowserView.tsx (lines 183-227)
- Simple InputBase component with no autocomplete functionality
- The omnibox will replace this with full dropdown UI

**Browser History Database:**
- History is stored in SQLite database (C++ layer manages it)
- Location: `%APPDATA%/HodosBrowser/Default/` (Windows) or `~/Library/Application Support/HodosBrowser/Default/` (macOS)
- Managed by HistoryManager in C++: `cef-native/src/core/HistoryManager.cpp`
- Schema: urls table (url, title, visit_count, typed_count, last_visit_time) + visits table
- Perfect for autocomplete (no schema changes needed)

**Architecture Considerations:**
- Frontend (React) handles UI and user interactions
- C++ CEF layer handles HTTP interception and native features
- Rust wallet backend is for crypto operations (not relevant for omnibox)
- Omnibox lives primarily in Frontend + C++ layers

**Google API Integration:**
- Need to set up Google API project and obtain credentials
- Consider rate limits and quota management
- May need API key management in application settings

## Constraints

- **Platform**: MacOS-first development and testing, must maintain Windows compatibility — The development environment is macOS, but Windows users exist
- **Tech Stack**: Must use existing stack (React, Material-UI, CEF, C++) — No new frameworks or major dependencies
- **Architecture**: Must follow three-layer pattern (React → CEF → Backend) — Maintain existing architectural boundaries
- **History Database**: Must work with existing SQLite schema — Cannot modify core browser database structure without careful consideration

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Replace existing address bar (not add alongside) | Cleaner UX, avoids confusion about which input to use | ✓ Confirmed (Phase 1) |
| Use Google for search (not DuckDuckGo/Bing) | User explicitly requested Google | — Pending |
| Mix history URLs + Google suggestions in dropdown | Requested by user; provides best of local + remote suggestions | — Pending |
| MacOS-first implementation | User's development platform; can test immediately | — Pending |
| Chrome-like keyboard shortcuts | Leverage familiar UX patterns users already know | — Pending |
| REPLACE: MainBrowserView InputBase component | Simple text input has no autocomplete dropdown UI, suggestion rendering, or keyboard navigation | ✓ Decided (Phase 1) |
| KEEP: HistoryManager C++ class and SQLite schema | Database schema perfect for autocomplete with url, title, visit_count, typed_count, last_visit_time fields | ✓ Decided (Phase 1) |
| EXTEND: State management for suggestions[], selectedIndex, showDropdown | Need autocomplete state in addition to address string | ✓ Decided (Phase 1) |
| EXTEND: IPC protocol with autocomplete_query/response messages | New messages for autocomplete suggestions | ✓ Decided (Phase 1) |
| EXTEND: HistoryManager with GetAutocompleteSuggestions() method | Single new method for optimized autocomplete queries | ✓ Decided (Phase 1) |

---
*Last updated: 2026-01-25 after Phase 1 completion*
