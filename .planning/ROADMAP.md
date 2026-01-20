# Roadmap: HodosBrowser macOS Compatibility

## Overview

Bringing the HodosBrowser macOS build to feature parity with Windows through a unified overlay system. The journey starts with completing the wallet UI on macOS, adds DevTools access for both platforms, migrates the improved overlay system to Windows, and finishes with cross-platform testing and polish.

## Domain Expertise

None

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Complete macOS Wallet UI** - Finish wallet panel and advanced features UI, wire to Rust backend
- [x] **Phase 2: DevTools Integration** - Add keyboard shortcuts and UI access for CEF DevTools
- [ ] **Phase 3: Windows Overlay Migration** - Port new overlay system from Mac to Windows
- [ ] **Phase 4: Cross-Platform Testing & Polish** - Test both platforms, fix issues, validate parity

## Phase Details

### Phase 1: Complete macOS Wallet UI
**Goal**: Deliver fully functional wallet panel and advanced features overlay on macOS
**Depends on**: Nothing (first phase)
**Research**: Unlikely (internal UI work, React patterns already established, wallet bridge API already tested)
**Status**: Complete
**Completed**: 2026-01-20
**Plans**: 2 total, 2 complete

Plans:
- [x] 01-01: Core Wallet Panel Operations (balance, send, receive)
- [x] 01-02: Advanced Features and Final Verification

### Phase 2: DevTools Integration
**Goal**: Enable CEF DevTools access via keyboard shortcuts and UI on both platforms
**Depends on**: Phase 1
**Research**: Completed (CEF DevTools configuration)
**Status**: Complete
**Completed**: 2026-01-20
**Plans**: 1 total, 1 complete

Plans:
- [x] 02-01: DevTools keyboard shortcuts and context menu access

### Phase 3: Windows Overlay Migration
**Goal**: Replace Windows' old overlay with the new unified system from macOS
**Depends on**: Phase 2
**Research**: Unlikely (porting existing Mac overlay to Windows, patterns already established in codebase)
**Status**: In progress
**Plans**: 2 total, 1 complete

Plans:
- [x] 03-01: Port Overlay Rendering System
- [ ] 03-02: Build and Functional Verification

### Phase 4: Cross-Platform Testing & Polish
**Goal**: Validate cross-platform consistency and fix remaining issues
**Depends on**: Phase 3
**Research**: Unlikely (testing and bug fixes using established patterns)
**Plans**: TBD

Plans:
- [ ] TBD during phase planning

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Complete macOS Wallet UI | 2/2 | Complete | 2026-01-20 |
| 2. DevTools Integration | 1/1 | Complete | 2026-01-20 |
| 3. Windows Overlay Migration | 1/2 | In progress | - |
| 4. Cross-Platform Testing & Polish | 0/TBD | Not started | - |
