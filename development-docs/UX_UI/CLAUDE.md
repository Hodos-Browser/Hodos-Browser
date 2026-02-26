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

**Frontend Location:** `/src/frontend/`

---

## Implementation Status (as of 2026-02-18)

| Phase | Name | Status | Key Doc |
|-------|------|--------|---------|
| **0** | Startup Flow & Wallet Checks | ✅ Complete | `phase-0-startup-flow-and-wallet-checks.md` |
| **1** | Initial Setup/Recovery | ✅ Complete | `phase-1-initial-setup-recovery.md` |
| **2** | User Notifications | 🔨 In Progress (2.3.8, 2.4.x remaining) | `phase-2-user-notifications.md`, `phase-2-research-findings.md` |
| **3** | Light Wallet (Polish) | 📋 Planning | `phase-3-light-wallet.md` |
| **4** | Full Wallet | 📋 Planning | `phase-4-full-wallet.md` |
| **5** | Activity Status Indicator | 📋 Planning (Low Priority) | `phase-5-activity-status-indicator.md` |

---

## Critical Files & Patterns

### Frontend → Backend Communication

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
3. If prompt needed → shows notification overlay
4. User responds → `cefMessage.send('auth_response', ...)`
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

**Logos:** `/frontend/public/Hodos_Gold_Icon.svg`, `Hodos_Black_Icon.svg`

### Design Philosophy (from `helper-2-design-philosophy.md`)

1. **Security without friction** — Never silently approve sensitive actions
2. **Clean + simple** — Minimal screens, minimal options
3. **Human-readable** — "$0.02" not "0.00003124 BSV"
4. **Immediate feedback** — Visual response within 100ms
5. **Non-annoying permissions** — Quiet indicators for trusted sites; modals only for sensitive actions
6. **Consistency** — Same action looks/behaves the same everywhere

### Escalation Levels

| Level | When | UI |
|-------|------|-----|
| **Quiet** | Trusted site, low-risk | No interruption |
| **Notification** | First-time site | Dismissible notice |
| **Modal** | Payments, PII, certificates | Requires action |

---

## Current Work: Phase 2 Remaining Items

From `phase-2-research-findings.md`:

| Step | Description | Status |
|------|-------------|--------|
| 2.3.8 | Certificate field disclosure notification | TODO |
| 2.4.1 | BRC-104 nonce fix in Rust | TODO |
| 2.4.1b | Rust defense-in-depth permission checks | TODO |
| 2.4.2 | Domain permissions UI in wallet settings | TODO |
| 2.4.4 | Documentation updates | TODO |

**Key infrastructure already built:**
- `DomainPermissionCache` (C++ singleton, reads from Rust DB)
- `PendingRequestManager` (per-request map, replaces global)
- `SessionManager` (per-browser-session spending/rate tracking)
- `BSVPriceCache` (USD conversion for spending limits)
- Notification overlay with keep-alive (HWND reused, hidden/shown)

---

## Phase 3: What to Know

Phase 3 is a **polish pass** on the existing wallet overlay (`WalletPanelLayout.tsx`), not a new component:

- Apply Hodos gold branding
- Add button feedback states (hover, pressed, loading)
- Add BSV QR code to receive section
- Add progress indicators for transactions
- Micro UX fixes (copy feedback, inline validation, empty states)

---

## Phase 4: Route Namespace Warning

The `/wallet` route is currently used by the wallet overlay. Phase 4 (Full Wallet) proposes sub-routes like `/wallet/overview`, `/wallet/addresses`. **Resolve this conflict during Phase 4 planning:**

- Option A: Move light wallet to overlay-only (no URL route)
- Option B: Use `/wallet-full/*` for Phase 4
- Option C: Phase 4 replaces overlay and owns `/wallet/*`

---

## Key Decisions Already Made

1. **USD-based spending limits** — Not satoshis (volatility). Stored as cents in DB.
2. **Trust is permanent** — No auto-expiry. Users manually revoke in settings.
3. **2-state trust model** — `unknown` (prompts) or `approved` (auto-approve below limits). `blocked` is session-only in C++ memory.
4. **DPAPI auto-unlock** — No PIN on startup (Windows DPAPI decrypts mnemonic automatically).
5. **Price fetching centralized** — Rust backend only (`price_cache.rs`). Frontend reads from `/wallet/balance` response.

---

## Helper Docs Reference

| Doc | Use For |
|-----|---------|
| `helper-1-implementation-guide-checklist.md` | Frontend architecture, component patterns, bridge methods, window management |
| `helper-2-design-philosophy.md` | Design principles, interaction rules, UX patterns |
| `helper-3-ux-considerations.md` | Permission UI, guard rails, auto-approve, wallet management |
| `helper-4-branding-colors-logos.md` | Colors, typography, logo usage |
| `HTTP_INTERCEPTOR_FLOW_GUIDE.md` | How interceptor + notification flow works (primary reference for Phase 2) |

---

## CSS Constraints (DO NOT MODIFY)

```css
/* Critical for CEF browser — in frontend/src/index.css */
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

1. **Read the relevant phase doc first** — Each phase has detailed requirements
2. **Check `phase-2-research-findings.md`** for latest implementation status
3. **Follow existing patterns** — Look at `WalletPanelLayout.tsx`, `BRC100AuthOverlayRoot.tsx`
4. **Use MUI `sx` prop** for styling (preferred over CSS files)
5. **Test in overlays** — Different rendering context than main browser
6. **Preserve bridge APIs** — Don't change message formats without updating C++
7. **Apply Hodos branding** — Gold `#a67c00`, not default blue

---

## Related Docs Outside This Folder

- `../CEF_REFINEMENT_TRACKER.md` — C++ stability/architecture issues
- `../WALLET_BACKUP_AND_RECOVERY_PLAN.md` — Backup data model, encryption
- `../SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md` — Security architecture
- `/src/frontend/` — Actual React code
- `/cef-native/src/core/HttpRequestInterceptor.cpp` — Interceptor implementation
- `/rust-wallet/src/database/domain_permission_repo.rs` — Permission DB layer

---

---

## Continuous Improvement Directive

**After each sprint, phase, or sub-phase:**
1. Review this CLAUDE.md — Is it still accurate? Update stale sections.
2. Check implementation status table — Mark completed work.
3. Add new patterns/decisions discovered during implementation.
4. Note any gotchas or surprises for future reference.
5. If a phase doc is significantly out of date, flag it for update.

**Goal:** Context files should always reflect current reality. They're the institutional memory that lets any AI (or human) pick up where the last session left off.

---

**Last Updated:** 2026-02-25
