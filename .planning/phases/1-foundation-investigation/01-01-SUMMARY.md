---
phase: 1-foundation-investigation
plan: 01
subsystem: ui
tags: [react, mui, sqlite, cef, history, autocomplete, architecture]

# Dependency graph
requires:
  - phase: codebase-mapping
    provides: STRUCTURE.md and ARCHITECTURE.md documenting three-layer system
provides:
  - Complete address bar React component documentation (MainBrowserView.tsx InputBase)
  - HistoryManager C++ API reference and SQLite schema documentation
  - End-to-end data flow diagrams from React → IPC → C++ → SQLite
  - Replace vs Reuse decision matrix for all major components
  - Clear guidance on what to replace (InputBase), keep (database schema, HistoryManager), and extend (state management, IPC protocol)
affects: [2-core-input-component, 3-history-autocomplete, 4-google-search-integration, 5-url-detection, 6-integration-testing]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "React address bar uses Material-UI Paper + InputBase with pill-shaped styling"
    - "Tab synchronization via useEffect that syncs address from activeTabId when not editing"
    - "IPC via window.cefMessage.send() from React to C++ browser process"
    - "HistoryManager singleton with SQLite WAL mode and Chromium timestamp format (µs since 1601)"
    - "History schema: urls table (url, title, visit_count, typed_count, last_visit_time) + visits table (url FK, visit_time, transition)"

key-files:
  created:
    - .planning/phases/1-foundation-investigation/INVESTIGATION.md
  modified: []

key-decisions:
  - "REPLACE: MainBrowserView InputBase (lines 183-227) - simple text input has no autocomplete dropdown UI"
  - "KEEP: HistoryManager C++ class and SQLite schema - perfect for autocomplete (url, title, visit_count, typed_count, last_visit_time)"
  - "EXTEND: State management - need suggestions[], selectedIndex, showDropdown in addition to address string"
  - "EXTEND: IPC protocol - add autocomplete_query and autocomplete_response messages"
  - "EXTEND: HistoryManager - add GetAutocompleteSuggestions() method for optimized autocomplete queries"
  - "KEEP: Navigation flow and tab synchronization - existing logic works correctly for omnibox"

patterns-established:
  - "Investigation produces INVESTIGATION.md with: (1) React Component Architecture, (2) History Database Architecture, (3) Data Flow Diagram, (4) IPC Protocol, (5) Replace vs Reuse Decision Matrix"
  - "Decision matrix categorizes every component as REPLACE, KEEP, or EXTEND with rationale"
  - "Documentation includes code snippets, ASCII diagrams, and SQL examples for future reference"

issues-created: []

# Metrics
duration: 45min
completed: 2026-01-24
---

# Phase 1: Foundation Investigation Summary

**Mapped existing address bar and history database architecture with comprehensive replace-vs-reuse decision matrix identifying 1 component to replace, 12 to keep, and 7 to extend for omnibox implementation.**

## Performance

- **Duration:** 45 min
- **Started:** 2026-01-24T10:00:00Z
- **Completed:** 2026-01-24T10:45:00Z
- **Tasks:** 3
- **Files modified:** 0 (investigation only)

## Accomplishments

- Documented complete React address bar architecture with component tree, state flow, event handlers, and Material-UI styling patterns
- Documented HistoryManager C++ API, SQLite schema (urls + visits tables), indexes, Chromium timestamp format, and SQL query examples for autocomplete
- Traced end-to-end data flow from React InputBase → useHodosBrowser → IPC → C++ SimpleHandler → TabManager/HistoryManager → SQLite
- Created comprehensive replace-vs-reuse decision matrix covering 20 components with explicit decisions and rationale
- Identified key insight: Database schema is already ideal for autocomplete, only need 1 new method in HistoryManager

## Task Commits

Since this is investigation with no code changes, a single commit will be made after documentation is complete:

- **docs(01-01): complete foundation investigation** - Investigation and summary files created

## Files Created/Modified

- `.planning/phases/1-foundation-investigation/INVESTIGATION.md` - Comprehensive 800+ line investigation covering React components, C++ database layer, IPC protocol, and decision matrix
- `.planning/phases/1-foundation-investigation/01-01-SUMMARY.md` - This summary file

## Decisions Made

**Replace Decisions (1 component)**

1. **MainBrowserView InputBase (lines 183-227)**: Current simple text input has no autocomplete dropdown UI, no suggestion rendering, no keyboard navigation through suggestions. Must be replaced with full Omnibox component.

**Keep Decisions (12 components)**

2. **Paper container styling**: Pill-shaped design (borderRadius 20px) with hover/focus states is reusable for omnibox visual wrapper.
3. **SQLite urls table schema**: Perfect for autocomplete with url, title, visit_count, typed_count, last_visit_time fields.
4. **SQLite visits table schema**: Useful for history page, doesn't interfere with autocomplete queries.
5. **HistoryManager C++ class structure**: Solid singleton foundation, just needs 1 new method.
6. **HistoryManager::AddVisit()**: Write logic is correct, autocomplete only reads existing data.
7. **HistoryManager::SearchHistory()**: Can be used as-is or adapted for autocomplete queries.
8. **Chromium timestamp format**: Standard int64 microseconds since 1601, works correctly.
9. **Tab sync useEffect (lines 45-53)**: Syncs address from activeTabId when not editing, logic is correct.
10. **useHodosBrowser navigation hooks**: navigate(), goBack(), goForward(), reload() API is sufficient.
11. **useKeyboardShortcuts**: Ctrl+L works, just need to update focus reference to new omnibox.
12. **IPC navigate message**: Existing message works for omnibox navigation.
13. **window.cefMessage.send() transport**: IPC mechanism works, just send new autocomplete_query message.

**Extend Decisions (7 components)**

14. **address state**: Need additional state: suggestions[], selectedIndex, showDropdown.
15. **isEditingAddress flag**: Need multi-state for dropdown interaction (idle/typing/navigating/selected).
16. **handleNavigate()**: Same navigate() call, but determine URL from typed input vs selected suggestion.
17. **handleKeyDown()**: Add ArrowUp, ArrowDown, Tab handling for suggestion navigation.
18. **handleAddressFocus()**: Same select-all behavior, optionally show suggestions on focus.
19. **handleAddressBlur()**: Add dropdown close logic before existing URL reset.
20. **IPC protocol**: Add autocomplete_query (React → C++) and autocomplete_response (C++ → React) messages.
21. **HistoryManager**: Add GetAutocompleteSuggestions(input, max_results) method with frecency ranking.

## Deviations from Plan

None - plan executed exactly as written. All three tasks completed:
1. ✓ Documented React component architecture
2. ✓ Documented HistoryManager database schema and API
3. ✓ Mapped data flow and created decision matrix

## Issues Encountered

None - investigation proceeded smoothly. All necessary files were accessible and well-documented.

## Next Phase Readiness

**Ready for Phase 2: Core Input Component**

Foundation is complete with clear guidance:
- Replace: Only the simple InputBase component
- Keep: Database, HistoryManager structure, navigation API, most React state logic
- Extend: Add suggestion state, arrow key handlers, 2 IPC messages, 1 HistoryManager method

**Key Insight for Phase 2**: The existing SQLite schema is already perfect for autocomplete. No database changes needed. Only need to add GetAutocompleteSuggestions() method to HistoryManager and build the React Omnibox component with dropdown UI.

**Blockers**: None

**Concerns**: None - investigation provided complete picture of existing architecture.

---
*Phase: 1-foundation-investigation*
*Completed: 2026-01-24*
