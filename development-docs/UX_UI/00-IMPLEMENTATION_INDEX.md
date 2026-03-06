# Web3 BitcoinSV UI/UX Implementation Index

## Purpose

This document serves as the master index and tracker for the Web3 BitcoinSV UI/UX interfaces in Hodos Browser. It tracks both original implementation status and the current **Verification & Branding Alignment Pass (V&B)** — a systematic re-test of the entire wallet flow from Phase 0 onwards, aligning all interfaces with Hodos branding.

**Document Version:** 2.0
**Last Updated:** 2026-03-03
**Target Audience:** UI/UX Developers, Full-Stack Developers

---

## Implementation Overview

The Web3 BitcoinSV UI/UX includes a startup flow (foundation) and five key interfaces:

**Foundation:**
- **Startup Flow and Wallet Checks** - Browser startup sequence, wallet file detection, and triggers for Initial Setup

**Interfaces:**
1. **Initial Setup/Recovery** - Modal or full shell window for wallet creation and recovery
2. **User Notifications** - Permission/consent modals governed by whitelist settings and escalation levels
3. **Light Wallet** - Modal or panel for quick wallet access (polish + BRC-29 peer payments)
4. **Full Wallet** - Full webview window for complete wallet management
5. **Activity Status Indicator** - Passive UI element showing background activity and permission status

---

## Implementation Status

### UX/UI Phases — Original Implementation

| Interface | Status | Implementation File | Last Updated |
|-----------|--------|-------------------|--------------|
| 0. Startup Flow and Wallet Checks | COMPLETE | [phase-0-startup-flow-and-wallet-checks.md](./phase-0-startup-flow-and-wallet-checks.md) | 2026-02-13 |
| 1. Initial Setup/Recovery | COMPLETE | [phase-1-initial-setup-recovery.md](./phase-1-initial-setup-recovery.md) | 2026-02-15 |
| 1a. Mnemonic Recovery + PIN | COMPLETE | *(see IMPLEMENTATION_STATUS.md)* | 2026-02-14 |
| 1b. Encrypted File Backup | COMPLETE | [WALLET_BACKUP_AND_RECOVERY_PLAN.md](../WALLET_BACKUP_AND_RECOVERY_PLAN.md) | 2026-02-14 |
| 1c. Centbee External Recovery | COMPLETE | *(see IMPLEMENTATION_STATUS.md)* | 2026-02-15 |
| 1d. Raw Private Key Recovery | PLANNING | [phase-1d-raw-private-key-recovery.md](./phase-1d-raw-private-key-recovery.md) | 2026-03-03 |
| 2. User Notifications (2.0–2.3.7) | COMPLETE | [phase-2-research-findings.md](./phase-2-research-findings.md) | 2026-02-25 |
| 2. User Notifications (remaining) | TODO | [phase-2-research-findings.md](./phase-2-research-findings.md) | 2026-02-25 |
| 3. Light Wallet (Polish) | PLANNING | [phase-3-light-wallet.md](./phase-3-light-wallet.md) | 2026-02-11 |
| 3a. BRC-29 Peer Payments | PLANNING | [phase-3a-brc29-peer-payments.md](./phase-3a-brc29-peer-payments.md) | 2026-02-26 |
| **Phase 3 Master Plan** | PLANNING | [PHASE_3_IMPLEMENTATION_PLAN.md](./PHASE_3_IMPLEMENTATION_PLAN.md) | 2026-02-26 |
| 4. Full Wallet | PLANNING | [phase-4-full-wallet.md](./phase-4-full-wallet.md) | 2026-02-11 |
| 5. Activity Status Indicator | PLANNING (Low Priority) | [phase-5-activity-status-indicator.md](./phase-5-activity-status-indicator.md) | - |

### Phase 2 — Detailed Sub-Phase Status

| Step | Description | Status |
|------|-------------|--------|
| 2.0 | Price Cache Migration | COMPLETE |
| 2.1 | Domain Permissions DB + Repository (6 REST endpoints) | COMPLETE |
| 2.2 | CR-2 Interceptor Refactor (PendingRequestManager, DomainPermissionCache) | COMPLETE |
| DPAPI | Auto-Unlock Sprint (mnemonic auto-decrypt on Windows) | COMPLETE |
| Bug Fix | 6 fixes from first-time user testing | COMPLETE |
| 2.3.1 | Auto-approve engine | COMPLETE |
| 2.3.2 | Notification overlays | COMPLETE |
| 2.3.3 | Payment confirmation variant | COMPLETE |
| 2.3.4 | Rate limiting | COMPLETE |
| 2.3.5 | Session block | COMPLETE |
| 2.3.6 | Advanced settings | COMPLETE |
| 2.3.7 | (sub-phase complete) | COMPLETE |
| **2.3.8** | **Certificate field disclosure notification** | **TODO** |
| **2.4.1** | **BRC-104 nonce fix + Rust defense-in-depth permission checks** | **TODO** |
| **2.4.2** | **Domain permissions management UI in wallet settings** | **TODO** |
| **2.4.4** | **Documentation updates** | **TODO** |

### CEF Refinement Phases

| Phase | Name | Status | UX Dependency |
|-------|------|--------|---------------|
| CR-1 | Critical Stability & Security | COMPLETE (2026-02-12) | Before/alongside UX Phase 0 |
| CR-2 | Interceptor Architecture | COMPLETE (done as part of Phase 2.2) | Before UX Phase 2 |
| CR-3 | Polish & Lifecycle | PLANNING | Alongside UX Phase 2–3 |

### Browser Core Sprints (Context)

All browser core sprints 0–13 are COMPLETE. This is relevant because those changes (multi-window, tab tear-off, adblock, fingerprint protection, profiles, etc.) may have affected wallet UI flows. The V&B pass below verifies everything still works correctly after those changes.

---

## Verification & Branding Alignment Pass (V&B)

### Why This Pass?

Since completing Phases 0–2, significant browser-core work has been done (Sprints 8–13: ad blocking, profiles, scriptlet injection, fingerprint protection, multi-window, tab tear-off). These changes touch CEF lifecycle, overlay management, IPC, and HWND hierarchy — all of which could affect wallet UI flows. Additionally, many interfaces still use default blue (`#1a73e8`) instead of Hodos branding.

**Goals:**
1. **Verify** every wallet flow still works correctly end-to-end after browser-core changes
2. **Align** all interfaces with Hodos branding (gold `#a67c00`, teal `#1a6b6a`, Inter font)
3. **Implement** Phase 1d (raw private key recovery) during the Phase 1 pass
4. **Document** any regressions or issues found

### Mandatory Reading Before V&B Work

Before touching any UI during the V&B pass, read these docs:

| Doc | Why |
|-----|-----|
| **[helper-2-design-philosophy.md](./helper-2-design-philosophy.md)** | 8 core design principles, interaction rules, escalation model — every interface must follow these |
| **[helper-4-branding-colors-logos.md](./helper-4-branding-colors-logos.md)** | Brand colors, typography, logo usage — the branding source of truth |
| **[helper-3-ux-considerations.md](./helper-3-ux-considerations.md)** | Wallet creation/recovery UX, privileged identity, blind message attacks, guard rails |
| **[helper-1-implementation-guide-checklist.md](./helper-1-implementation-guide-checklist.md)** | Frontend architecture, component patterns, bridge methods, CSS constraints |

### V&B Pass Schedule

Each step has two parts: **Verify** (test the existing flow) and **Brand** (align colors/typography/patterns with helper-2 and helper-4).

#### V&B-0: Startup Flow (Phase 0)

**Verify:**
- [ ] Browser launches without blocking
- [ ] Wallet server starts as subprocess (health polling works)
- [ ] `POST /wallet/status` returns `exists: false` when no wallet
- [ ] Clicking Wallet toolbar button triggers create/recover flow
- [ ] Windows Job Object cleanup on abnormal exit
- [ ] Multi-window: startup flow works from any window

**Brand:**
- [ ] Audit NoWallet prompt — replace any default blue with gold/teal
- [ ] Hodos logo in startup/loading states (use `Hodos_Gold_Icon.svg` on dark, `Hodos_Black_Icon.svg` on light)
- [ ] Typography: product name uses Founders Grotesk (if loaded), UI text uses Inter
- [ ] Loading/connecting states follow helper-2 feedback patterns

**Ref:** [phase-0-startup-flow-and-wallet-checks.md](./phase-0-startup-flow-and-wallet-checks.md)

---

#### V&B-1: Initial Setup/Recovery (Phase 1 + 1d)

**Verify — Wallet Creation:**
- [ ] New wallet: mnemonic generated, displayed, confirmed
- [ ] Mnemonic backup warning shown
- [ ] PIN encryption flow (optional)
- [ ] DPAPI auto-unlock stores encrypted mnemonic
- [ ] Wallet accessible after creation (balance, identity key)

**Verify — Recovery Methods:**
- [ ] Mnemonic recovery (12/24 words) — wallet restored, addresses/UTXOs found
- [ ] Encrypted backup file (.hodos/.bsv-wallet) — decrypt, import, verify entities
- [ ] Centbee import — mnemonic + 4-digit PIN, non-standard derivation paths, sweep
- [ ] `<input type="file">` works correctly in CEF overlays (regression check)

**Implement — Phase 1d (Raw Private Key Recovery):**
- [ ] Database migration V15 (`key_source`, `raw_key_encrypted` columns)
- [ ] `POST /wallet/recover-from-key` endpoint with `keyType` field
- [ ] Hex (64 char) and WIF format validation
- [ ] Primary vs privileged key guidance in UI (see phase-1d doc)
- [ ] Warning: "No recovery phrase available"
- [ ] Confirmation checkbox before import
- [ ] BRC-100 migration guidance: "Use your primary key to preserve identity"

**Brand:**
- [ ] `WalletSetupModal.tsx` — replace all default blue with gold primary buttons
- [ ] Recovery method cards — consistent styling per helper-2 (clean + simple)
- [ ] Mnemonic display — monospace font (Courier New), copy button with "Copied" feedback
- [ ] Warning banners — use semantic amber (`#e6a200`), not red (recoverable caution)
- [ ] Success states — forest green (`#2e7d32`)
- [ ] Error states — deep red (`#c62828`) with friendly message + next action
- [ ] All buttons follow helper-2 states: hover, pressed, disabled, loading
- [ ] Inputs follow helper-2: real-time validation, error text under field (not alerts)
- [ ] Modals follow helper-2: clear close button, Escape key support

**Ref:** [phase-1-initial-setup-recovery.md](./phase-1-initial-setup-recovery.md), [phase-1d-raw-private-key-recovery.md](./phase-1d-raw-private-key-recovery.md)

---

#### V&B-2: User Notifications (Phase 2)

**Verify — Permission System:**
- [ ] Domain permission creation (first-time site prompt)
- [ ] Auto-approve for trusted sites below spending limits
- [ ] Spending limit enforcement (per-tx $0.10, per-session $3.00 defaults)
- [ ] Rate limiting (10 req/min default)
- [ ] Session tracking (`SessionManager` per-browser-session)
- [ ] `DomainPermissionCache` syncs with Rust DB
- [ ] `PendingRequestManager` handles concurrent requests

**Verify — Notification Overlays:**
- [ ] Domain trust prompt (Allow/Deny/Block)
- [ ] Payment confirmation modal (amount, recipient, approve/deny)
- [ ] Notification overlay show/hide lifecycle
- [ ] Overlay positioning relative to toolbar
- [ ] Keyboard events forward correctly in notification overlay
- [ ] Multi-window: notifications appear in correct window

**Verify — Interceptor Flow:**
- [ ] HTTP interception → permission check → prompt if needed → resume/block
- [ ] Auto-approve engine respects limits
- [ ] BSV price cache provides USD conversion
- [ ] DPAPI auto-unlock: no PIN prompt on startup (Windows)

**Brand:**
- [ ] Notification overlay — gold header accent, not blue
- [ ] Permission prompt — domain name prominent, teal for informational links
- [ ] Payment confirmation — amount in gold, "Why?" link in teal
- [ ] Allow/Deny buttons — gold primary (Allow), slate secondary (Deny), red for Block
- [ ] Trust indicators — consistent with helper-2 escalation visual language
- [ ] Error states — friendly language per helper-2 (explain + next action)

**Ref:** [phase-2-research-findings.md](./phase-2-research-findings.md), [HTTP_INTERCEPTOR_FLOW_GUIDE.md](./HTTP_INTERCEPTOR_FLOW_GUIDE.md)

---

#### V&B-3: Light Wallet Polish (Phase 3) — New Work

Phase 3 is new implementation work, but it should be built with branding from the start (no retrofit needed).

- [ ] Apply gold branding to wallet overlay from day one
- [ ] Follow helper-2 for all button states, input patterns, feedback
- [ ] Follow helper-4 for all colors and typography
- [ ] See [phase-3-light-wallet.md](./phase-3-light-wallet.md) and [PHASE_3_IMPLEMENTATION_PLAN.md](./PHASE_3_IMPLEMENTATION_PLAN.md)

---

### V&B Tracking

| Step | Verify Status | Brand Status | Notes |
|------|--------------|-------------|-------|
| V&B-0 (Startup) | NOT STARTED | NOT STARTED | |
| V&B-1 (Setup/Recovery) | NOT STARTED | NOT STARTED | Includes 1d implementation |
| V&B-2 (Notifications) | NOT STARTED | NOT STARTED | |
| V&B-3 (Light Wallet) | N/A (new work) | Built-in | Branding from the start |

---

## Implementation Order

**Original order** (for reference):

0. Startup Flow → 1. Setup/Recovery → 2. Notifications → 3. Light Wallet → 4. Full Wallet → 5. Activity Indicator

**Current work order** (V&B pass):

1. **V&B-0**: Verify + brand Phase 0 (startup flow)
2. **V&B-1**: Verify + brand Phase 1 (setup/recovery) + implement Phase 1d
3. **V&B-2**: Verify + brand Phase 2 (notifications/permissions)
4. **Phase 2 remaining**: Implement 2.3.8, 2.4.1, 2.4.2, 2.4.4
5. **V&B-3 / Phase 3**: Light wallet polish + BRC-29 (new work, branded from start)
6. **Phase 4**: Full wallet (future)
7. **Phase 5**: Activity indicator (future, low priority)

---

## Shared Resources

### Design & Branding (READ BEFORE ANY UI WORK)

- **[Design Philosophy](./helper-2-design-philosophy.md)** - 8 core principles, interaction rules, escalation model, micro UX requirements
- **[Color Guidelines & Logos](./helper-4-branding-colors-logos.md)** - Brand palette, typography, logo usage — the single source of truth for visual identity
- **[UX Design Considerations](./helper-3-ux-considerations.md)** - Wallet creation/recovery UX, privileged identity access, guard rails, auto-approve safety

### Technical Reference

- **[Implementation Guide & Checklist](./helper-1-implementation-guide-checklist.md)** - Frontend architecture, component patterns, bridge methods, CSS constraints
- **[HTTP Interceptor Flow Guide](./HTTP_INTERCEPTOR_FLOW_GUIDE.md)** - Request interception patterns — primary reference for Phase 2 work

### Research Documents

- **[Phase 2 Research Findings](./phase-2-research-findings.md)** - Domain permissions, notification design, implementation status (most current Phase 2 reference)
- **[Phase 3 Peer Payments Research](./phase-3-peer-payments-research.md)** - BRC-29, MessageBox, paymail, primary vs privileged keys
- **[Privileged Keyring Analysis](../../PRIVILEGED_KEYRING_ANALYSIS.md)** - BRC-100 dual-keyring assessment (deferred to post-MVP)

---

## Branding Alignment Checklist

Every UI component in the wallet flow must pass this checklist (derived from helper-2 and helper-4):

### Colors
- [ ] No instances of `#1a73e8` (deprecated Google blue) — replace with gold or teal
- [ ] Primary buttons: gold `#a67c00`
- [ ] Secondary/info accents: teal `#1a6b6a`
- [ ] Neutral/borders: slate `#4a5568`
- [ ] Success: forest green `#2e7d32`
- [ ] Warning: amber `#e6a200` (NOT brand gold)
- [ ] Error: deep red `#c62828`

### Typography
- [ ] Product names ("HODOS BROWSER", "HODOS WALLET"): Founders Grotesk semibold, all-caps
- [ ] All UI text: Inter (body 400, headings/buttons 600-700)
- [ ] Addresses/txids: Courier New / monospace

### Interaction Patterns (helper-2)
- [ ] All buttons: hover + pressed + disabled + loading states
- [ ] All inputs: real-time validation, error text under field
- [ ] All copy actions: "Copied" feedback (icon change or toast, 2s)
- [ ] All modals: close button + Escape key
- [ ] All long actions: descriptive progress text (not just "Loading...")
- [ ] All errors: explain what happened + what user can do next
- [ ] Empty states: icon + message + action button (never blank)

### Logos
- [ ] Dark backgrounds: `Hodos_Gold_Icon.svg`
- [ ] Light backgrounds: `Hodos_Black_Icon.svg`

---

## Notes

- Each V&B step should be completed and tested before moving to the next
- Document any regressions found during verification (browser-core changes that broke wallet flows)
- Branding changes should be applied alongside verification, not as a separate pass
- Database changes (Phase 1d migration) require asking before implementation per CLAUDE.md invariants

---

## Status Legend

- PLANNING - Design and planning phase
- TODO - Designed, not yet started
- IN PROGRESS - Implementation started
- COMPLETE - Implementation finished and verified
- NOT STARTED - V&B verification not yet begun

---

**End of Document**
