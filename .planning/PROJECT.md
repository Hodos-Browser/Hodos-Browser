# HodosBrowser macOS Compatibility

## What This Is

A cross-platform Web3 browser built on CEF with native Rust wallet backend and BRC-100 protocol support. This project brings the macOS build to feature parity with Windows using a new, unified overlay system for both platforms.

## Core Value

Functional cross-platform parity with enhanced wallet UI - users on both macOS and Windows can access wallet operations, BRC-100 authentication, and developer tools through a modern overlay interface.

## Requirements

### Validated

<!-- Shipped and confirmed valuable on Windows build -->

- ✓ Three-layer architecture (React → CEF → Rust wallet) — existing
- ✓ BRC-100 protocol suite (BRC-42, BRC-43, BRC-52, BRC-103/104) — existing
- ✓ HD wallet with BIP39/BIP32 key derivation — existing
- ✓ Private key isolation in Rust process — existing
- ✓ HTTP interception and wallet request routing — existing (Windows)
- ✓ V8 injection for `window.hodosBrowser` API — existing
- ✓ SQLite persistence for wallet and browser data — existing
- ✓ Browser core functionality (navigate, render, tabs) — existing (both platforms)
- ✓ Rust wallet backend runs on localhost:3301 — existing (both platforms)

### Active

<!-- Current scope for Mac branch merge -->

- [ ] Complete wallet panel overlay UI (view balance, addresses, transactions)
- [ ] Complete advanced features page UI (BRC-100 certificate management, auth configuration)
- [ ] Wire wallet panel to Rust backend via `window.hodosBrowser` API
- [ ] Wire advanced features to Rust backend for certificate operations
- [ ] DevTools keyboard shortcut (Cmd+Option+I on Mac, Ctrl+Shift+I on Windows)
- [ ] DevTools UI access (menu item or button)
- [ ] Port new overlay system to Windows (replace old wallet overlay)
- [ ] Testing and polish for cross-platform consistency

### Out of Scope

- Performance optimization — Focus is functional correctness, optimize later
- New features beyond parity — No additional capabilities; just match Windows plus new overlay
- CEF version upgrade — Stick with CEF 136 to minimize risk
- Wallet data migration — Schema and crypto logic remain unchanged

## Context

**Current State:**
- Mac migration branch exists with significant CEF cross-platform work already done
- `#ifdef` blocks throughout `cef-native/` separate Windows and macOS implementations
- Old wallet overlay on Windows is outdated; new overlay built on Mac is the target design
- Wallet bridge tested working via dev console on Mac, but full UI integration incomplete
- DevTools currently accessible only by curling for remote debugging link from external browser

**Technical Environment:**
- CEF 136 (Chromium Embedded Framework)
- Rust 2021 with Actix-web for wallet backend
- React 19 with MUI 7 for frontend
- Multi-process CEF with overlay model (each overlay is a subprocess)
- Storage: `%APPDATA%/HodosBrowser/` (Windows), `~/Library/Application Support/HodosBrowser/` (macOS)

**Known Issues:**
- DevTools not accessible from within app on macOS
- New wallet panel UI incomplete (both platforms)
- Advanced features page incomplete (both platforms)
- Windows still using old overlay system

## Constraints

- **Tech Stack**: Must work with existing CEF 136 binaries — No CEF upgrade; architecture changes must be minimal
- **Cross-Platform**: Can't break Windows build — All changes must maintain Windows compatibility; test both platforms
- **Data Integrity**: Must preserve wallet data format — No SQLite schema changes; no crypto/signing logic changes; wallet.db must remain compatible

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Unify overlay system across platforms | Mac's new overlay is cleaner than Windows' old one; single codebase easier to maintain | — Pending |
| DevTools via both keyboard shortcut and UI | Standard browser UX; supports both developer workflows and casual debugging | — Pending |
| Complete Mac first, then backport to Windows | Mac is the active branch; validate new overlay design before replacing Windows version | — Pending |

---
*Last updated: 2026-01-20 after initialization*
