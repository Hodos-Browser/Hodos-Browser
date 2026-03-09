# UX_UI — AI Assistant Context

> **Purpose:** This folder contains the UI/UX implementation plans for Hodos Browser's Web3 wallet interfaces. Use this document as your starting point when working on frontend, overlay, notification, or wallet UI tasks.

---

## Quick Orientation

**What is this?** Five phased UX interfaces for the native BSV wallet embedded in Hodos Browser, plus helper docs for design philosophy, branding, and technical patterns.

**Tech Stack:**
- React 19 + TypeScript + Vite
- Material-UI v7 (primary component library)
- CEF (Chromium Embedded Framework) for native browser shell
- Rust Actix-web backend (`localhost:3301`)
- Process-per-overlay architecture (each overlay = separate CEF render process)

**Frontend Location:** `frontend/src/`

---

## Current Status: Phase 4 Complete — Testing & Refinement Needed

**Phases 3, 3a, 3b, and 4** are all IMPLEMENTED. All need real-world testing and refinement before MVP.

**See [00-IMPLEMENTATION_INDEX.md](./00-IMPLEMENTATION_INDEX.md)** for full status and V&B schedule.

### Phase 3: Light Wallet Polish + BRC-29 PeerPay (COMPLETE)

| Sprint | Description | Status |
|--------|-------------|--------|
| 3.1a | Button audit (native buttons, hover/active states) | COMPLETE |
| 3.1b | QR code for receive addresses (BIP21 format) | COMPLETE |
| 3.2 | Notification badge + PeerPay auto-accept setting | COMPLETE |
| 3.3 | PeerPay send via BRC-29 MessageBox | COMPLETE |
| 3.4 | PeerPay receive (background poller + polling badge) | COMPLETE |
| 3.5 | Error handling, labels, documentation | COMPLETE |

### Phase 3a: BRC-29 PeerPay Rebuild (COMPLETE — 2026-03-05)
Full rewrite with correct BRC-29 + MessageBox protocol. See `phase-3a-brc29-peer-payments.md`.

### Phase 3b: Paymail + Identity Resolution (COMPLETE — 2026-03-07)

| Sprint | Description | Status |
|--------|-------------|--------|
| 3b.1 | Paymail backend (`paymail.rs`, `paymail_send`, `paymail_resolve`) | COMPLETE |
| 3b.2 | Paymail frontend (detection, debounced resolve, send routing) | COMPLETE |
| 3b.3-4 | Unified recipient resolution (`identity_resolver.rs`, `/wallet/recipient/resolve`, dropdown UI) | COMPLETE |

### Phase 4: Advanced Wallet Dashboard (COMPLETE — 2026-03-08)

| Sprint | Description | Status |
|--------|-------------|--------|
| 4.1 | Layout shell — sidebar nav, DashboardTab (balance, QR, send, recent activity) | COMPLETE |
| 4.2 | ActivityTab — unified sent/received, pagination, USD, copy txid, WoC link | COMPLETE |
| 4.3 | CertificatesTab, ApprovedSitesTab (domain permissions + default limits) | COMPLETE |
| 4.4 | SettingsTab (display name, PIN mnemonic reveal, backup export, wallet delete) | COMPLETE |
| 4.5 | Dark theme CSS, responsive layout, lazy-loaded tabs | COMPLETE |
| 4.6 | Unified activity history — V11 migration, `/wallet/activity` endpoint, price snapshots | COMPLETE |

### V&B Pass (Pending — Lower Priority)

| Step | Status |
|------|--------|
| V&B-0 (Startup) | NOT STARTED |
| V&B-1 (Setup/Recovery + 1d) | NOT STARTED |
| V&B-2 (Notifications) | NOT STARTED |

---

## MANDATORY: Read Before Any UI Work

Before writing or modifying ANY wallet UI code, read these docs:

| Doc | What You Learn | Why It Matters |
|-----|---------------|----------------|
| **[helper-2-design-philosophy.md](./helper-2-design-philosophy.md)** | 8 design principles, interaction rules, escalation model, micro UX patterns | Every button, input, modal, error, and empty state must follow these rules |
| **[helper-4-branding-colors-logos.md](./helper-4-branding-colors-logos.md)** | Brand palette, typography, logo usage | Single source of truth for visual identity — NO deprecated blue (`#1a73e8`) |
| **[helper-3-ux-considerations.md](./helper-3-ux-considerations.md)** | Wallet creation/recovery UX, privileged identity, guard rails | Security-sensitive UX decisions that are already made |
| **[helper-1-implementation-guide-checklist.md](./helper-1-implementation-guide-checklist.md)** | Frontend architecture, components, bridges, CSS | Technical patterns that must be followed |

### Branding Rules (Quick Reference from helper-4)

| Element | Color | Hex |
|---------|-------|-----|
| Primary buttons, links, headers | **Gold** | `#a67c00` |
| Info accents, secondary links | **Deep Teal** | `#1a6b6a` |
| Borders, secondary text | **Slate** | `#4a5568` |
| Success states | **Forest Green** | `#2e7d32` |
| Warning/caution states | **Amber** | `#e6a200` |
| Error/destructive states | **Deep Red** | `#c62828` |
| ~~Old default blue~~ | **DEPRECATED — REMOVE** | ~~`#1a73e8`~~ |

### Interaction Rules (Quick Reference from helper-2)

- **Buttons**: Must have hover + pressed + disabled + loading states
- **Inputs**: Real-time validation, error text under field (not alerts)
- **Copy actions**: Show "Copied" feedback (2 seconds)
- **Modals**: Close button + Escape key support
- **Long actions**: Descriptive progress ("Broadcasting transaction..." not "Loading...")
- **Errors**: Explain what happened + what user can do next (never raw HTTP errors)
- **Empty states**: Icon + message + action button (never blank areas)

---

## Implementation Status (as of 2026-03-09)

| Phase | Name | Status | Key Doc |
|-------|------|--------|---------|
| **0** | Startup Flow & Wallet Checks | COMPLETE | `phase-0-startup-flow-and-wallet-checks.md` |
| **1** | Initial Setup/Recovery (1a, 1b, 1c) | COMPLETE | `phase-1-initial-setup-recovery.md` |
| **1d** | Raw Private Key Recovery | PLANNING | `phase-1d-raw-private-key-recovery.md` |
| **2** | User Notifications (2.0–2.3.7) | COMPLETE | `phase-2-research-findings.md` |
| **2** | Remaining (2.3.8, 2.4.x) | TODO | `phase-2-research-findings.md` |
| **CR-2** | Interceptor Architecture | COMPLETE | (done as part of Phase 2.2) |
| **3** | Light Wallet (Polish + BRC-29) | COMPLETE | `phase-3-light-wallet.md`, `PHASE_3_IMPLEMENTATION_PLAN.md` |
| **3a** | BRC-29 PeerPay Rebuild | COMPLETE (needs testing) | `phase-3a-brc29-peer-payments.md` |
| **3b** | Paymail + Identity Resolution | COMPLETE (needs testing) | `phase-3b-paymail-identity-resolution.md` |
| **4** | Full Wallet Dashboard | COMPLETE (needs testing) | `phase-4-full-wallet.md` |
| **5** | Activity Status Indicator | PLANNING (Low Priority) | `phase-5-activity-status-indicator.md` |

---

## Critical Files & Patterns

### Frontend -> Backend Communication

All wallet operations use message passing through CEF:

```typescript
// Frontend sends
window.cefMessage?.send('message_type', [JSON.stringify(payload)]);

// Frontend receives via callback
window.onMessageTypeResponse = (data) => { ... };
window.onMessageTypeError = (error) => { ... };
```

**Bridge setup:** `frontend/src/bridge/initWindowBridge.ts`
**Types:** `frontend/src/types/hodosBrowser.d.ts`

### Overlay Window Pattern

Overlays are React apps loaded in separate CEF windows:
- **Route:** `/wallet`, `/settings`, `/backup`, `/brc100-auth`
- **Root components:** `frontend/src/pages/*OverlayRoot.tsx`
- **C++ trigger:** `cefMessage.send('overlay_show_wallet', [])`
- **Close:** `cefMessage.send('overlay_close', [])`

### HTTP Interceptor Flow (Phase 2)

For permission/notification prompts:
1. C++ intercepts wallet API request
2. Checks `DomainPermissionCache` (trust level, spending limits)
3. If prompt needed -> shows notification overlay
4. User responds -> `cefMessage.send('auth_response', ...)`
5. C++ continues or blocks request

**Key file:** `HTTP_INTERCEPTOR_FLOW_GUIDE.md`

---

## Design System

### Brand Colors (from `helper-4-branding-colors-logos.md`)

| Use | Hex | Notes |
|-----|-----|-------|
| Gold (primary) | `#a67c00` | Buttons, links, headers |
| Deep Teal (accent) | `#1a6b6a` | Info states, secondary actions |
| Success | `#2e7d32` | Confirmed transactions |
| Warning | `#e6a200` | Pending states |
| Error | `#c62828` | Failed transactions |

**Logos:** `frontend/public/Hodos_Gold_Icon.svg`, `Hodos_Black_Icon.svg`

### Design Philosophy (from `helper-2-design-philosophy.md`)

1. **Security without friction** -- Never silently approve sensitive actions
2. **Clean + simple** -- Minimal screens, minimal options
3. **Human-readable** -- "$0.02" not "0.00003124 BSV"
4. **Immediate feedback** -- Visual response within 100ms
5. **No ambiguity** -- Always answer "did it work?"
6. **Non-annoying permissions** -- Quiet indicators for trusted sites; modals only for sensitive actions
7. **Consistency** -- Same action looks/behaves the same everywhere
8. **Calm confidence** -- Professional, calm, build trust

### Escalation Levels

| Level | When | UI |
|-------|------|-----|
| **Quiet** | Trusted site, low-risk | No interruption |
| **Notification** | First-time site | Dismissible notice |
| **Modal** | Payments, PII, certificates | Requires action |

---

## Phase 2 Remaining Items

From `phase-2-research-findings.md`:

| Step | Description | Status |
|------|-------------|--------|
| 2.3.8 | Certificate field disclosure notification | TODO |
| 2.4.1 | BRC-104 nonce fix in Rust | TODO |
| 2.4.2 | Domain permissions UI in wallet settings | DONE (ApprovedSitesTab in Phase 4) |
| 2.4.4 | Documentation updates | TODO |

---

## Phase 3: What Was Built

Phase 3 was a polish pass + BRC-29 peer payments + paymail + identity resolution:

- **3.1**: Button audit (native), QR code (BIP21), notification badge
- **3a**: BRC-29 PeerPay rebuild (authfetch, MessageBox, auto-accept, encrypted payments)
- **3b**: Paymail resolution (bsvalias protocol, P2P + basic paths, HandCash $handle support)
- **3b**: Identity name resolution (BSV Overlay Services, PushDrop, BRC-2 anyone-key decryption)
- **3b**: Unified recipient resolution (`/wallet/recipient/resolve` — auto-detects all 4 input types)

---

## Phase 4: What Was Built

Phase 4 replaced the old MUI-tab wallet overlay with a sidebar + content dashboard:

- **4.1**: Sidebar navigation, DashboardTab (balance, QR, send form, recent activity)
- **4.2**: ActivityTab (unified sent+received, pagination, USD at tx time, copy txid, WoC link)
- **4.3**: CertificatesTab, ApprovedSitesTab (domain permissions + default limits)
- **4.4**: SettingsTab (display name, PIN mnemonic reveal, backup export, delete wallet)
- **4.5**: Dark theme CSS, lazy-loaded tabs, responsive layout
- **4.6**: Unified activity history (V11 migration, `/wallet/activity` endpoint, price snapshots)

Route namespace resolved: Phase 4 owns `/wallet` as the overlay route. No sub-routes needed — tabs are components within the single overlay.

---

## Key Decisions Already Made

1. **USD-based spending limits** -- Not satoshis (volatility). Stored as cents in DB.
2. **Trust is permanent** -- No auto-expiry. Users manually revoke in settings.
3. **2-state trust model** -- `unknown` (prompts) or `approved` (auto-approve below limits). `blocked` is session-only in C++ memory.
4. **DPAPI auto-unlock** -- No PIN on startup (Windows DPAPI decrypts mnemonic automatically).
5. **Price fetching centralized** -- Rust backend only (`price_cache.rs`). Frontend reads from `/wallet/balance` response.
6. **Primary key for recovery** -- When importing from BRC-100 wallets, always recommend primary (everyday) key to preserve identity. Privileged keyring deferred to post-MVP.
7. **Branding from the start** -- All new UI work uses Hodos branding. No more default blue.

---

## Helper Docs Reference

| Doc | Use For |
|-----|---------|
| `helper-1-implementation-guide-checklist.md` | Frontend architecture, component patterns, bridge methods, window management |
| `helper-2-design-philosophy.md` | Design principles, interaction rules, UX patterns — **READ FIRST** |
| `helper-3-ux-considerations.md` | Permission UI, guard rails, auto-approve, wallet management |
| `helper-4-branding-colors-logos.md` | Colors, typography, logo usage — **READ FIRST** |
| `HTTP_INTERCEPTOR_FLOW_GUIDE.md` | How interceptor + notification flow works (primary reference for Phase 2) |

---

## CSS Constraints (DO NOT MODIFY)

```css
/* Critical for CEF browser -- in frontend/src/index.css */
html, body {
  margin: 0 !important;
  padding: 0 !important;
  overflow: hidden;
  position: fixed;
}
#root {
  overflow: hidden;
  position: absolute;
}
```

---

## When Working on UX_UI Tasks

1. **Read helper-2 and helper-4 FIRST** -- Branding and design principles are mandatory
2. **Read the relevant phase doc** -- Each phase has detailed requirements
3. **Check `phase-2-research-findings.md`** for latest Phase 2 implementation status
4. **Follow existing patterns** -- Look at `WalletPanelLayout.tsx`, `BRC100AuthOverlayRoot.tsx`
5. **Use native `<input>` elements** in CEF overlays (MUI TextField breaks CEF focus)
6. **Test in overlays** -- Different rendering context than main browser
7. **Preserve bridge APIs** -- Don't change message formats without updating C++
8. **Apply Hodos branding** -- Gold `#a67c00`, never default blue

---

## Related Docs Outside This Folder

- `../CEF_REFINEMENT_TRACKER.md` -- C++ stability/architecture issues
- `../WALLET_BACKUP_AND_RECOVERY_PLAN.md` -- Backup data model, encryption
- `../../SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md` -- Security architecture
- `../../PRIVILEGED_KEYRING_ANALYSIS.md` -- BRC-100 dual-keyring assessment
- `../../frontend/src/` -- Actual React code
- `../../cef-native/src/core/HttpRequestInterceptor.cpp` -- Interceptor implementation
- `../../rust-wallet/src/database/domain_permission_repo.rs` -- Permission DB layer

---

## Continuous Improvement Directive

**After each sprint, phase, or sub-phase:**
1. Review this CLAUDE.md -- Is it still accurate? Update stale sections.
2. Update V&B tracking in `00-IMPLEMENTATION_INDEX.md`.
3. Add new patterns/decisions discovered during implementation.
4. Note any regressions found during verification.
5. If a phase doc is significantly out of date, flag it for update.

**Goal:** Context files should always reflect current reality. They're the institutional memory that lets any AI (or human) pick up where the last session left off.

---

**Last Updated:** 2026-03-09
