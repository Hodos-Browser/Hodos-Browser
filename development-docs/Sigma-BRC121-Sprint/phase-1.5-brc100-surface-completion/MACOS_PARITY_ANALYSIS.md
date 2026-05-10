# Phase 1.5 BRC-100 Surface Completion — macOS parity analysis

**Date:** 2026-05-10 (started during Step 0)
**Scope:** All Phase 1.5 changes shipped on `feature/brc121-phase1` branch.
**Goal:** Single source of truth for everything Phase 1.5 needs to be tested or verified on macOS before merge. Mirrors the Phase 1 doc pattern in `../phase-1-brc121/MACOS_PARITY_ANALYSIS.md`.

This doc is **updated as each step lands.** Step 0 entries are filled in below. Later sections are placeholders for Steps 1–7 (populate when they land).

---

## TL;DR (rolling — updated per step)

Through Step 0: **No macOS-specific implementation work required.** All changes are cross-platform React/MUI/TypeScript except for one cross-platform C++ helper that needed an implementation in both `TabManager.cpp` (Windows) and `TabManager_mac.mm` (macOS). Both files patched in tandem so the build is clean on both platforms. Verification risks are visual-only and listed at the bottom of each step section.

---

## Step 0 — Cosmetic pre-flight + critical bug fixes

**Landed:** 2026-05-10
**Branch:** `feature/brc121-phase1` (uncommitted at time of writing)

### Files modified — platform impact matrix

| File | Change summary | Platform impact |
|---|---|---|
| `frontend/src/styles/hodosTheme.ts` (NEW, 92 lines) | Shared color/font/prompt-tier tokens | None — React |
| `frontend/src/styles/CLAUDE.md` (NEW, 89 lines) | Theme docs | None — docs |
| `frontend/src/hooks/useTabManager.ts` | Animation match fix (Tab::id), merge instead of replace on `tab_list_response` to preserve `paymentIndicator` | None — React |
| `frontend/src/pages/BRC100AuthOverlayRoot.tsx` | `HodosWalletHeader` injected on all 6 prompt branches; revoke-permissions confirmation panel with explicit OK button | None — React |
| `frontend/src/components/DomainPermissionForm.tsx` | Brighter input + checkbox borders, bolder "Always notify me" label, new `borderInput: '#555'` token | None — React |
| `frontend/src/components/DomainPermissionsTab.tsx` | Dark-theme MUI `sx` overrides on Edit + Revoke dialogs | None — React/MUI |
| `frontend/src/pages/PaymentFailedPage.tsx` | Inline hex → theme tokens; gold Hodos Wallet icon top-left inside card; card anchored at 28vh | None — React |
| `frontend/src/pages/PaymentPendingPage.tsx` | Inline hex → theme tokens (no layout change — spinning icon top-left preserved) | None — React |
| `frontend/src/components/settings/WalletSettings.tsx` | **DELETED** (dead code — Q1 resolution: ApprovedSitesTab is single source of truth) | None |
| `frontend/src/components/settings/CLAUDE.md` | Documented deletion + correct location | None — docs |
| `cef-native/include/core/TabManager.h` | Declared `GetTabIdForBrowserIdentifier(int)` | Header — cross-platform |
| `cef-native/src/core/TabManager.cpp` | **Windows** implementation of `GetTabIdForBrowserIdentifier` | Windows-only file; impl is pure C++, no platform code |
| `cef-native/src/core/TabManager_mac.mm` | **macOS** implementation of `GetTabIdForBrowserIdentifier` | macOS-only file; identical impl to Windows |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | Added include for `TabManager.h`; both `firePaymentSuccessIpc` sites now translate CEF identifier → Tab::id before sending IPC; log lines show both IDs | Cross-platform CEF APIs only |
| `development-docs/Sigma-BRC121-Sprint/CHECKLIST.md` | Line-reference fixes | None — docs |
| `development-docs/Sigma-BRC121-Sprint/phase-1.5-brc100-surface-completion/README.md` | Line-reference fixes | None — docs |

### CEF APIs used — all cross-platform

Step 0's C++ changes use only:
- `CefBrowser::GetIdentifier()` — cross-platform CEF
- `CefProcessMessage` (already in use for `payment_success_indicator`)
- `nlohmann::json` — cross-platform

No new platform-specific APIs introduced.

### macOS verification checklist (Step 0)

**Visual** (run on Mac and confirm parity with Windows behavior):

- [ ] Theme tokens render identically — gold (#a67c00), dark surfaces (#1a1a1a / #252525), Inter font stack falls back to BlinkMacSystemFont as expected
- [ ] `HodosWalletHeader` icon (`/Hodos_Gold_Wallet_Icon.svg`) loads and displays at 36px height on every BRC-100 prompt
- [ ] `DomainPermissionsTab` Edit + Revoke dialogs render with readable dark-theme MUI overrides (the bug they fix was unreadable text on macOS too — confirm fixed)
- [ ] `DomainPermissionForm` brighter borders + bolder checkbox label readable on macOS
- [ ] `PaymentFailedPage` card sits at 28vh from top with wallet icon top-left of card
- [ ] `PaymentPendingPage` spinning icon top-left unchanged from previous look
- [ ] `EditPermissionsForm` revoke confirmation panel ("Permissions revoked / OK") renders correctly

**Behavioral:**

- [ ] Right-click → "Manage Site Permissions" opens the `edit_permissions` overlay on Mac (via `cef_browser_shell_mac.mm` notification overlay creation path — should work, but verify)
- [ ] Settings → Wallet sidebar entry routes correctly after `WalletSettings.tsx` deletion (no broken section)
- [ ] **Payment animation badge fires on auto-approved BRC-121 payments** — the IPC chain is `Async402ResourceHandler::OnRequestComplete` → `firePaymentSuccessIpc` → `TabManager::GetTabIdForBrowserIdentifier` → `CefProcessMessage` → render process → React `useTabManager`. All cross-platform. The `_mac.mm` patch ensures the C++ helper links on Mac.
- [ ] `tab_list_response` merge logic preserves `paymentIndicator` across C++ tab list pushes during article load on Mac (same React code path as Windows)

**Build verification:**

- [ ] `cef-native` builds clean on Mac with the new `TabManager::GetTabIdForBrowserIdentifier` declared in the header and implemented in `TabManager_mac.mm`

### Known secondary issues to also check on Mac

- **`cents=0` in every BRC-121 fire log** — `bsvPrice` likely 0 at compute time. React renders `"< $0.01"` fallback so the badge still appears, but the displayed amount is wrong. Separately tracked. On Mac, badge should also show `"< $0.01"` until the root cause is fixed.

---

## Steps 1–7 — placeholders (populate when each step lands)

### Step 1 — Missing handlers + privacy perimeter
_To be filled in when Step 1 lands. Likely items: new `identity_key_reveal` + `key_linkage_reveal` prompt types in `BRC100AuthOverlayRoot.tsx` (React, cross-platform). Rust handlers (`revealCounterpartyKeyLinkage`, `revealSpecificKeyLinkage`) in `rust-wallet/src/handlers.rs` (cross-platform Rust). Possible new key-linkage logic in `rust-wallet/src/crypto/key_linkage.rs` (cross-platform Rust)._

### Step 2 — DB schema
_To be filled in. Three new child tables of `domain_permissions` + optional `sensitivity` column on `cert_field_permissions`. Pure SQLite/Rust — no platform impact._

### Step 3 — Permission engine
_To be filled in. New `PermissionEngine.h/.cpp` in `cef-native/src/core/`. Should be cross-platform C++; verify no Win-specific APIs added._

### Step 4 — Manifest fetcher
_To be filled in. Likely uses `SyncHttpClient` which is already cross-platform (WinHTTP / libcurl)._

### Step 5 — Extend existing UI
_To be filled in. Manifest connect bundle prompt + two new prompt types (`protocol_permission_prompt`, `counterparty_permission_prompt`) in shared overlay. Likely React-only, cross-platform._

### Step 6 — Rewire existing handlers
_To be filled in. One-line gate at top of 26+2 BRC-100 handlers. Pure C++ in `HttpRequestInterceptor.cpp` — cross-platform._

### Step 7 — Demo prep
_To be filled in. Demo dApp + dev guide. No platform impact._

---

## Cross-step verification (run once before merge)

After all steps land, do a single Mac smoke pass:

- [ ] Build cef-native on Mac (clean rebuild from scratch)
- [ ] Standard verification basket on Mac per root CLAUDE.md testing table (x.com, google.com, github.com, youtube.com, etc.)
- [ ] All 11 prompt types in `BRC100AuthOverlayRoot.tsx` render correctly on Mac (the 6 existing + 5 new from Steps 1 + 5)
- [ ] BRC-121 payment to `now.bsvblockchain.tech` succeeds + badge fires on Mac
- [ ] Settings, Approved Sites, right-click Manage Site Permissions all functional on Mac

---

## Related

- [Phase 1 macOS parity doc](../phase-1-brc121/MACOS_PARITY_ANALYSIS.md) — pattern this doc mirrors
- [Phase 1.5 README](README.md) — implementation spine
- [Phase 1.5 PERMISSION_UX_DESIGN](PERMISSION_UX_DESIGN.md) — UX rationale
- [Sprint CHECKLIST](../CHECKLIST.md) — overall sprint progress
- Root CLAUDE.md invariant #9 — macOS cross-platform readiness requirement
