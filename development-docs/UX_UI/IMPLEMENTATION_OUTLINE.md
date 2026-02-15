# Web3 UX/UI Implementation Outline

## Purpose

This document is the **single progression guide** for implementing Web3 BitcoinSV UX/UI in Hodos Browser. It walks through each phase in order and points to the detailed doc for that phase plus the helper docs (design philosophy, considerations, enhancement guide, branding).

**Document Version:** 1.0
**Last Updated:** 2026-02-11
**Target Audience:** Implementers, PM, AI assistants

---

## How to Use This Doc

1. **Implement in order**: Phase 0 → 1 → 2 → 3 → 4 → 5.
2. **Before each phase**: Read the linked phase doc and the helper docs it references.
3. **During implementation**: Use [helper-1 Implementation Guide & Checklist](helper-1-implementation-guide-checklist.md) for frontend/CEF/backend checklist and [HTTP Interceptor Flow Guide](HTTP_INTERCEPTOR_FLOW_GUIDE.md) for interceptor patterns.
4. **For design decisions**: Use [Design Philosophy](helper-2-design-philosophy.md) and [UX Considerations](helper-3-ux-considerations.md). Use [Branding & Logos](helper-4-branding-colors-logos.md) for logo/color usage.

**Done:** Reorganization applied; phase and helper doc names updated. `1-initial-setup-recovery.md`, etc., and helpers as `helper-1-...`, `helper-2-...`. Update this outline’s links then.

---

## Progression Overview

```
Phase 0: Startup Flow & Wallet Checks (foundation)
    ↓
Phase 1: Initial Setup / Recovery (wallet creation & recovery)
    ↓
Phase 2: User Notifications (permission modals, escalation consent)
    ↓
Phase 3: Light Wallet (quick balance, send, receive)
    ↓
Phase 4: Full Wallet (full wallet management UI)
    ↓
Phase 5: Activity Status Indicator (passive activity/permission indicator)
```

---

## Phase 0: Startup Flow and Wallet Checks

**Goal:** Browser starts without blocking; wallet existence is checked; Initial Setup is triggered only when appropriate (e.g. user clicks Wallet and no wallet exists).

**Detail doc:** [phase-0-startup-flow-and-wallet-checks.md](phase-0-startup-flow-and-wallet-checks.md)

**Key outcomes:**
- C++ checks wallet file on startup (no auto-create).
- Wallet server starts only when wallet exists (or on first use, per design).
- Frontend does not block startup; wallet check on Wallet button click can trigger Phase 1.

**Helper refs:** Design Philosophy (non-blocking, user-driven); Implementation Guide (CEF + frontend startup).

**Next:** Phase 1 is shown when no wallet exists and user tries to use wallet.

---

## Phase 1: Initial Setup / Recovery

**Goal:** User can create a new wallet or recover from mnemonic/backup in a clear, secure flow.

**Detail doc:** [phase-1-initial-setup-recovery.md](phase-1-initial-setup-recovery.md)

**Key outcomes:**
- Create new wallet (mnemonic, backup confirmation).
- Recover from mnemonic or backup file.
- Integration with wallet backend; no wallet until user completes flow.

**Helper refs:** Design Philosophy (security, feedback, human-readable); UX Considerations (trust, clarity).

**Next:** Once wallet exists, Phase 2 (notifications) and Phase 3 (light wallet) become relevant.

---

## Phase 2: User Notifications

**Goal:** Permission and consent modals (Escalation Consent) for payments, signing, certificates, identity; governed by whitelist and escalation level so we don’t over-prompt.

**Detail doc:** [phase-2-user-notifications.md](phase-2-user-notifications.md)

**Before starting Phase 2:** Do a detailed analysis of the HTTP interceptor flow. **Key reference (helper for this phase):** [HTTP Interceptor Flow Guide](HTTP_INTERCEPTOR_FLOW_GUIDE.md) — keep as a standalone reusable doc; do not consolidate into phase-2.

**Key outcomes:**
- Modal prompts for payment, sign, encrypt, certificate (and optional “Block site”).
- Optional lightweight notifications for first-time/low-risk.
- Integration with HTTP interceptor (pause request → show overlay → user choice → resume/block).
- Outline only for permissions research (payment requests, certificates); full research in this phase or when needed.

**Helper refs:** Design Philosophy (escalation levels, non-annoying permissions); UX Considerations (notification system, guard rails); [HTTP_INTERCEPTOR_FLOW_GUIDE.md](HTTP_INTERCEPTOR_FLOW_GUIDE.md).

**Next:** Phase 3 (Light Wallet) can be built in parallel or after; Phase 4 builds on Light Wallet patterns.

---

## Phase 3: Light Wallet

**Goal:** Quick-access UI for balance, send, receive, and recent transactions (modal or panel).

**Detail doc:** [phase-3-light-wallet.md](phase-3-light-wallet.md)

**Key outcomes:**
- Balance, send (simplified form), receive (address/QR), recent tx list.
- Opens from toolbar/header; may open Full Wallet (Phase 4) for advanced features.

**Helper refs:** Design Philosophy (clean + simple, human-readable, feedback); Implementation Guide (overlay, routing, components).

**Next:** Phase 4 (Full Wallet) reuses or extends patterns from Light Wallet.

---

## Phase 4: Full Wallet

**Goal:** Full wallet management in a dedicated window: addresses, history, settings, backup, etc.

**Detail doc:** [phase-4-full-wallet.md](phase-4-full-wallet.md)

**Key outcomes:**
- Full webview/window for wallet; all management features.
- Uses User Notifications for transaction confirmations where needed.

**Helper refs:** Design Philosophy; UX Considerations; Implementation Guide (window management, wallet APIs).

**Next:** Phase 5 (Activity Status Indicator) can be added last.

---

## Phase 5: Activity Status Indicator

**Goal:** Passive UI element showing background activity and permission usage; user can open a review/change-permissions view without constant popups.

**Detail doc:** [phase-5-activity-status-indicator.md](phase-5-activity-status-indicator.md)

**Key outcomes:**
- Indicator in header/toolbar (or status bar); click opens activity/permission summary.
- Complements User Notifications (passive vs active).

**Helper refs:** Design Philosophy (minimal interruption, informed choice); UX Considerations (notification vs indicator).

---

## Helper Documents (Reference for All Phases)

| Doc | Purpose |
|-----|--------|
| [helper-1 Implementation Guide & Checklist](helper-1-implementation-guide-checklist.md) | Frontend architecture, checklist (Frontend / CEF / Rust / DB / Testing). |
| [helper-2 Design Philosophy](helper-2-design-philosophy.md) | Philosophy, escalation levels, security, clarity. |
| [helper-3 UX Considerations](helper-3-ux-considerations.md) | Notifications, guard rails, auto-approve, risk. |
| [HTTP Interceptor Flow Guide](HTTP_INTERCEPTOR_FLOW_GUIDE.md) | How interception and overlay prompts work. |
| [helper-4 Color Guidelines & Logos](helper-4-branding-colors-logos.md) | Brand colors, logo usage (frontend/public). |

---

## Index and Status

For a compact status table and shared resources list, see [00-IMPLEMENTATION_INDEX.md](00-IMPLEMENTATION_INDEX.md).

---

**End of Document**
