# Roadmap: Hodos Browser Omnibox Feature

## Overview

Transform the existing address bar into a Chrome-like omnibox that seamlessly combines URL navigation with Google search. Starting with investigation of current implementation, we'll build a React component with Material-UI that integrates SQLite history autocomplete and Google search suggestions, implementing intelligent URL vs search detection and full keyboard navigation. The journey progresses from understanding existing architecture through core functionality to cross-platform polish.

## Domain Expertise

None

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Foundation & Investigation** - Understand current address bar and history database architecture
- [ ] **Phase 2: Core Input Component** - Build base omnibox React component with Material-UI
- [ ] **Phase 3: History Autocomplete** - Integrate SQLite history database with search and ranking
- [ ] **Phase 4: Google Search Integration** - Set up Google API and implement search vs URL detection
- [ ] **Phase 5: Keyboard Navigation** - Implement Chrome-like keyboard shortcuts and navigation
- [ ] **Phase 6: Cross-Platform Testing & Polish** - Verify Windows compatibility and optimize performance

## Phase Details

### Phase 1: Foundation & Investigation
**Goal**: Understand existing address bar implementation, history database schema, and establish architecture for omnibox replacement
**Depends on**: Nothing (first phase)
**Research**: Likely (need to understand existing implementation)
**Research topics**: Current address bar code location and structure, SQLite history database schema (fields, indexes, query patterns), CEF browser data storage patterns in C++ layer (HistoryManager.cpp)
**Plans**: TBD

Plans:
- TBD (determined during phase planning)

### Phase 2: Core Input Component
**Goal**: Build the foundational omnibox React component with input handling, basic dropdown UI, and Material-UI styling
**Depends on**: Phase 1
**Research**: Unlikely (React + Material-UI component using established patterns)
**Plans**: TBD

Plans:
- TBD (determined during phase planning)

### Phase 3: History Autocomplete
**Goal**: Query SQLite history database from frontend, implement ranking algorithm (frequency + recency), and display history suggestions in dropdown
**Depends on**: Phase 2
**Research**: Likely (database integration and ranking)
**Research topics**: Querying SQLite history from React frontend (via C++ layer), autocomplete ranking algorithms (FrecencyScore = frequency × recency_weight), efficient SQL queries for prefix matching
**Plans**: TBD

Plans:
- TBD (determined during phase planning)

### Phase 4: Google Search Integration
**Goal**: Set up Google API credentials, implement URL vs search query detection, and merge Google suggestions with history results
**Depends on**: Phase 3
**Research**: Likely (external API and setup)
**Research topics**: Google Custom Search API vs Autocomplete API (which to use), Google API project setup and credential management, URL detection heuristics (protocol, TLD, whitespace patterns)
**Plans**: TBD

Plans:
- TBD (determined during phase planning)

### Phase 5: Keyboard Navigation
**Goal**: Implement arrow key navigation through suggestions, Tab to autocomplete, Enter to navigate/search with Chrome-like behavior
**Depends on**: Phase 4
**Research**: Unlikely (standard React keyboard event handling)
**Plans**: TBD

Plans:
- TBD (determined during phase planning)

### Phase 6: Cross-Platform Testing & Polish
**Goal**: Verify Windows compatibility, optimize performance (query speed, dropdown rendering), and refine UI/UX
**Depends on**: Phase 5
**Research**: Unlikely (testing and refinement of existing functionality)
**Plans**: TBD

Plans:
- TBD (determined during phase planning)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation & Investigation | 0/TBD | Not started | - |
| 2. Core Input Component | 0/TBD | Not started | - |
| 3. History Autocomplete | 0/TBD | Not started | - |
| 4. Google Search Integration | 0/TBD | Not started | - |
| 5. Keyboard Navigation | 0/TBD | Not started | - |
| 6. Cross-Platform Testing & Polish | 0/TBD | Not started | - |
