# Web3 BitcoinSV UI/UX Implementation Index

## Purpose

This document serves as the master index and tracker for the five Web3 BitcoinSV UI/UX interfaces being implemented for Hodos Browser.

**Document Version:** 1.1
**Last Updated:** 2026-02-11
**Target Audience:** UI/UX Developers, Full-Stack Developers

---

## Implementation Overview

The Web3 BitcoinSV UI/UX enhancement includes the startup flow (foundation) and five key interfaces:

**Foundation:**
- **Startup Flow and Wallet Checks** - Browser startup sequence, wallet file detection, and triggers for Initial Setup

**Interfaces:**
1. **Initial Setup/Recovery** - Modal or full shell window for wallet creation and recovery
2. **User Notifications** - Popup modal governed by whitelist settings
3. **Light Wallet** - Modal or panel for quick wallet access
4. **Full Wallet** - Full webview window for complete wallet management
5. **Activity Status Indicator** - Passive UI element showing background activity and permission status

---

## Implementation Status

### UX/UI Phases

| Interface | Status | Implementation File | Last Updated |
|-----------|--------|-------------------|--------------|
| 0. Startup Flow and Wallet Checks | 📋 Planning | [phase-0-startup-flow-and-wallet-checks.md](./phase-0-startup-flow-and-wallet-checks.md) | 2026-02-11 |
| 0.1 Domain Permissions Research | 📋 Planning | *(no dedicated doc yet — research sprint)* | 2026-02-11 |
| 1. Initial Setup/Recovery | 📋 Planning | [phase-1-initial-setup-recovery.md](./phase-1-initial-setup-recovery.md) | 2026-02-11 |
| B1. Local File Backup (parallel w/ Phase 1) | 📋 Planning | [WALLET_BACKUP_AND_RECOVERY_PLAN.md](../WALLET_BACKUP_AND_RECOVERY_PLAN.md) | 2026-02-11 |
| 2. User Notifications | 📋 Planning | [phase-2-user-notifications.md](./phase-2-user-notifications.md) | 2026-02-11 |
| 3. Light Wallet (Polish) | 📋 Planning | [phase-3-light-wallet.md](./phase-3-light-wallet.md) | 2026-02-11 |
| 4. Full Wallet | 📋 Planning | [phase-4-full-wallet.md](./phase-4-full-wallet.md) | 2026-02-11 |
| 5. Activity Status Indicator | 📋 Planning (Low Priority) | [phase-5-activity-status-indicator.md](./phase-5-activity-status-indicator.md) | - |

### CEF Refinement Phases

Stability, security, and architecture improvements to the C++ native layer. Tracked in **[CEF_REFINEMENT_TRACKER.md](../CEF_REFINEMENT_TRACKER.md)**.

| Phase | Name | Status | UX Dependency |
|-------|------|--------|---------------|
| CR-1 | Critical Stability & Security | 📋 Planning | Before/alongside UX Phase 0 |
| CR-2 | Interceptor Architecture | 📋 Planning | Must complete before UX Phase 2 |
| CR-3 | Polish & Lifecycle | 📋 Planning | Alongside UX Phase 2–3 |

**Status Legend:**
- 📋 Planning - Design and planning phase
- 🔨 In Progress - Implementation started
- ✅ Complete - Implementation finished
- 🧪 Testing - In testing phase

---

## Implementation Order

The interfaces should be implemented in the following order to ensure dependencies are met:

0. **Startup Flow and Wallet Checks** (Foundation - always start server, triggers Initial Setup)
   - See: [Startup Flow and Wallet Checks](./phase-0-startup-flow-and-wallet-checks.md)
   - Status: 📋 Planning
   - **Prerequisites**: Disable wallet auto-creation in Rust server
   - **Decision**: Option A — always start server (returns `exists: false` if no wallet)
   - **Testing**: Verify `<input type="file">` works in CEF overlays (needed for Phase 1)
0.1. **Domain Permissions Research & Design** (Research sprint — feeds Phase 2 and Phase 4)
   - Study: per-domain spending limits (per-tx, per-day, per-session), certificate auto-approve levels
   - Design: `domain_permissions` DB table schema, sensible defaults, simple MVP model
   - **Prerequisites**: None — can start anytime, should complete before Phase 2
1. **Initial Setup/Recovery** (Foundation - enables wallet functionality, triggered by Startup Flow)
   - Parallel: **Backup Phase B1** (local file export/import backend for "Recover from file")
   - **Decision**: User PIN for file backup encryption; mnemonic for on-chain backup
2. **User Notifications** (Depends on Phase 0.1 domain permissions design)
   - **Decision**: Simplified auto-approve for MVP (trusted domains + sat threshold)
   - Phase 2 also triggers create/recover modal if HTTP intercept finds no wallet
3. **Light Wallet (Polish)** (Polish of existing wallet overlay — branding, feedback, QR code)
4. **Full Wallet** (Most complex — route namespace, WalletContext, domain permission settings)
5. **Activity Status Indicator** (Low Priority - Passive monitoring, can be added last)

---

## Shared Resources

### Design Principles
- **[Design Philosophy](./helper-2-design-philosophy.md)** - Foundational design guidelines

### Related Documentation
- **[Implementation Guide & Checklist](./helper-1-implementation-guide-checklist.md)** - Frontend architecture + per-interface checklist
- **[Startup Flow and Wallet Checks](./phase-0-startup-flow-and-wallet-checks.md)** - Startup sequence
- **[HTTP Interceptor Flow Guide](./HTTP_INTERCEPTOR_FLOW_GUIDE.md)** - Request interception patterns; **primary reference for Phase 2 (User Notifications)** — use as a helper during that phase
- **[CEF Refinement Tracker](../CEF_REFINEMENT_TRACKER.md)** - Phased checklist for C++ stability, security, and architecture fixes. **Consult during pre-phase planning** to identify CR prerequisites for each UX phase.
- **[UX Design Considerations](./helper-3-ux-considerations.md)** - User experience considerations
- **[Color Guidelines & Logos](./helper-4-branding-colors-logos.md)** - Brand colors and logo usage

---

## Implementation Checklist

See **[helper-1-implementation-guide-checklist.md](./helper-1-implementation-guide-checklist.md)** for the full per-interface checklist (Frontend, CEF-Native, Rust, Database, Testing).

---

## Reorganization

A phased layout (0–5, helper-1–4) and link updates are proposed in [REORGANIZATION_PLAN.md](./REORGANIZATION_PLAN.md). The progression guide is [IMPLEMENTATION_OUTLINE.md](./IMPLEMENTATION_OUTLINE.md).

## Notes

- Each interface should be implemented, tested, and reviewed before moving to the next
- Design principles should be established before detailed implementation planning
- All interfaces should follow the established architecture patterns
- Database changes should be carefully planned to avoid breaking existing functionality

---

**End of Document**
