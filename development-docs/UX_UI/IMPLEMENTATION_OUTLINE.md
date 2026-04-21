# Web3 UX/UI Implementation Outline

## Purpose

This document is the **single progression guide** for implementing Web3 BitcoinSV UX/UI in Hodos Browser. It walks through each phase in order, points to detailed docs, and describes the current **Verification & Branding Alignment Pass (V&B)** that re-tests the entire wallet flow while aligning all interfaces with Hodos branding.

**Document Version:** 2.0
**Last Updated:** 2026-03-03
**Target Audience:** Implementers, PM, AI assistants

---

## How to Use This Doc

1. **We are currently in the V&B pass** — re-testing Phases 0–2 and aligning with branding.
2. **Before any UI work**: Read the helper docs (especially helper-2 and helper-4).
3. **During implementation**: Use [helper-1 Implementation Guide & Checklist](helper-1-implementation-guide-checklist.md) for frontend/CEF/backend checklist and [HTTP Interceptor Flow Guide](HTTP_INTERCEPTOR_FLOW_GUIDE.md) for interceptor patterns.
4. **For design decisions**: Use [Design Philosophy](helper-2-design-philosophy.md) and [UX Considerations](helper-3-ux-considerations.md). Use [Branding & Logos](helper-4-branding-colors-logos.md) for logo/color usage.
5. **Track progress**: Use [00-IMPLEMENTATION_INDEX.md](00-IMPLEMENTATION_INDEX.md) for the V&B checklist and status tracking.

---

## Progression Overview

### Original Implementation (Completed or Planned)

```
Phase 0: Startup Flow & Wallet Checks .................. COMPLETE
    |
Phase 1: Initial Setup / Recovery ...................... COMPLETE (1a-1c)
    |                                                    PLANNING (1d)
Phase 2: User Notifications (2.0-2.3.7) ............... COMPLETE
    |                    (2.3.8, 2.4.x) ............... TODO
CR-2: Interceptor Architecture ......................... COMPLETE
    |
Phase 3: Light Wallet (polish + BRC-29) ............... PLANNING
    |
Phase 4: Full Wallet .................................. PLANNING
    |
Phase 5: Activity Status Indicator .................... PLANNING (Low Priority)
```

### Current: Verification & Branding Pass (V&B)

After completing browser-core sprints 8–13 (ad blocking, profiles, scriptlets, fingerprint protection, multi-window, tab tear-off), we are re-testing the entire wallet flow from Phase 0 and aligning all interfaces with Hodos branding.

```
V&B-0: Verify + Brand Phase 0 (startup flow)
    |
V&B-1: Verify + Brand Phase 1 (setup/recovery) + Implement Phase 1d
    |
V&B-2: Verify + Brand Phase 2 (notifications/permissions)
    |
Phase 2 remaining: Implement 2.3.8, 2.4.1, 2.4.2, 2.4.4
    |
V&B-3 / Phase 3: Light Wallet (new work, branded from start)
    |
Phase 4: Full Wallet (future)
    |
Phase 5: Activity Status Indicator (future, low priority)
```

---

## V&B-0: Startup Flow and Wallet Checks (Phase 0)

**Goal:** Verify startup flow still works after browser-core changes. Apply Hodos branding to any startup UI.

**Status:** COMPLETE (original), NOT STARTED (V&B verification)

**Detail doc:** [phase-0-startup-flow-and-wallet-checks.md](phase-0-startup-flow-and-wallet-checks.md)

**Verify:**
- Browser launches without blocking
- Wallet server starts as subprocess
- No-wallet state correctly triggers create/recover flow
- Multi-window: startup works from any window

**Brand:**
- Replace default blue with gold/teal in NoWallet prompt
- Hodos logo in startup states
- Loading states follow helper-2 feedback patterns

**Helper refs:** [Design Philosophy](helper-2-design-philosophy.md), [Branding](helper-4-branding-colors-logos.md), [Implementation Guide](helper-1-implementation-guide-checklist.md)

---

## V&B-1: Initial Setup / Recovery (Phase 1 + 1d)

**Goal:** Verify all recovery methods still work. Implement Phase 1d (raw private key recovery). Apply branding.

**Status:** COMPLETE (1a-1c original), PLANNING (1d), NOT STARTED (V&B verification)

**Detail docs:** [phase-1-initial-setup-recovery.md](phase-1-initial-setup-recovery.md), [phase-1d-raw-private-key-recovery.md](phase-1d-raw-private-key-recovery.md)

**Verify:**
- Wallet creation (mnemonic, backup, PIN, DPAPI)
- Mnemonic recovery (12/24 words)
- Encrypted backup file recovery
- Centbee import (mnemonic + 4-digit PIN)
- `<input type="file">` in CEF overlays (regression check)

**Implement (1d):**
- Raw private key recovery (hex or WIF)
- Primary vs privileged key type selection (default: primary)
- BRC-100 migration guidance in UI
- Database migration V15

**Brand:**
- Gold primary buttons, amber warnings, teal info text
- All button states, input validation, error patterns per helper-2
- Native `<input>` elements (CEF overlay compatibility)

**Helper refs:** [Design Philosophy](helper-2-design-philosophy.md), [Branding](helper-4-branding-colors-logos.md), [UX Considerations](helper-3-ux-considerations.md)

---

## V&B-2: User Notifications (Phase 2)

**Goal:** Verify permission system, notification overlays, and interceptor flow still work. Apply branding.

**Status:** COMPLETE (2.0-2.3.7 original), TODO (2.3.8, 2.4.x), NOT STARTED (V&B verification)

**Detail docs:** [phase-2-user-notifications.md](phase-2-user-notifications.md), [phase-2-research-findings.md](phase-2-research-findings.md)

**Before starting:** Review [HTTP Interceptor Flow Guide](HTTP_INTERCEPTOR_FLOW_GUIDE.md) for interceptor patterns.

**Verify:**
- Domain permission creation (first-time site prompt)
- Auto-approve for trusted sites below spending limits
- Spending/rate limit enforcement
- Notification overlay lifecycle (show/hide, positioning, keyboard)
- Multi-window: notifications appear in correct window

**Brand:**
- Gold header accent in notification overlay
- Domain name prominent, teal for informational links
- Allow = gold, Deny = slate, Block = red
- Error states per helper-2 (explain + next action)

**After V&B-2, implement remaining Phase 2 items:**
- 2.3.8: Certificate field disclosure notification
- 2.4.1: BRC-104 nonce fix + Rust defense-in-depth
- 2.4.2: Domain permissions management UI
- 2.4.4: Documentation updates

**Helper refs:** [Design Philosophy](helper-2-design-philosophy.md), [Branding](helper-4-branding-colors-logos.md), [HTTP Interceptor Flow Guide](HTTP_INTERCEPTOR_FLOW_GUIDE.md)

---

## V&B-3 / Phase 3: Light Wallet

**Goal:** Polish the existing wallet overlay with Hodos branding. Add BRC-29 peer payments.

**Status:** PLANNING (new work — branding built in from the start)

**Detail docs:** [phase-3-light-wallet.md](phase-3-light-wallet.md), [PHASE_3_IMPLEMENTATION_PLAN.md](PHASE_3_IMPLEMENTATION_PLAN.md), [phase-3a-brc29-peer-payments.md](phase-3a-brc29-peer-payments.md)

**Key outcomes:**
- Hodos gold branding on wallet overlay from day one
- Button feedback states, QR code, progress indicators
- BRC-29 peer payments via identity key + MessageBox
- Unified send field (address/paymail/identity auto-detect)

**Helper refs:** [Design Philosophy](helper-2-design-philosophy.md), [Branding](helper-4-branding-colors-logos.md), [Peer Payments Research](phase-3-peer-payments-research.md)

---

## Phase 4: Full Wallet

**Goal:** Full wallet management in a dedicated window: addresses, history, settings, backup.

**Status:** PLANNING (future)

**Detail doc:** [phase-4-full-wallet.md](phase-4-full-wallet.md)

**Key outcomes:**
- Full webview/window for wallet; all management features
- Uses User Notifications for transaction confirmations
- Route namespace conflict must be resolved first

**Helper refs:** [Design Philosophy](helper-2-design-philosophy.md), [Branding](helper-4-branding-colors-logos.md), [Implementation Guide](helper-1-implementation-guide-checklist.md)

---

## Phase 5: Activity Status Indicator

**Goal:** Passive UI element showing background activity and permission usage.

**Status:** PLANNING (Low Priority — future)

**Detail doc:** [phase-5-activity-status-indicator.md](phase-5-activity-status-indicator.md)

**Key outcomes:**
- Indicator in header/toolbar; click opens activity/permission summary
- Complements User Notifications (passive vs active)

**Helper refs:** [Design Philosophy](helper-2-design-philosophy.md)

---

## Helper Documents (MANDATORY Reading Before Any Phase)

| Doc | Purpose | When to Read |
|-----|---------|-------------|
| [helper-2 Design Philosophy](helper-2-design-philosophy.md) | 8 design principles, interaction rules, escalation levels, micro UX | **Before every phase** |
| [helper-4 Color Guidelines & Logos](helper-4-branding-colors-logos.md) | Brand palette, typography, logo usage | **Before every phase** |
| [helper-3 UX Considerations](helper-3-ux-considerations.md) | Wallet creation/recovery UX, privileged identity, guard rails | Before Phases 1, 2, 3 |
| [helper-1 Implementation Guide & Checklist](helper-1-implementation-guide-checklist.md) | Frontend architecture, components, bridges, CSS constraints | During implementation |
| [HTTP Interceptor Flow Guide](HTTP_INTERCEPTOR_FLOW_GUIDE.md) | Interception and overlay prompt patterns | Before Phase 2 |

---

## Index and Status

For the full V&B checklist and detailed status tracking, see [00-IMPLEMENTATION_INDEX.md](00-IMPLEMENTATION_INDEX.md).

---

**End of Document**
