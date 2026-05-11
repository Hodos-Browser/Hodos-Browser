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

**Landed:** 2026-05-11
**Branch:** `feature/brc121-phase1` (uncommitted at time of writing)
**TL;DR:** **No macOS-specific implementation work.** Step 1 lives entirely in cross-platform Rust, cross-platform CEF C++ (no Win/Mac branches added), React/TypeScript, and a single SQLite migration. Macros, headers, and APIs used are all platform-neutral. Verification is visual + behavioral; no Mac-only build changes.

#### Files modified — platform impact matrix

| File | Change summary | Platform impact |
|---|---|---|
| `rust-wallet/src/crypto/key_linkage.rs` (NEW) | BRC-72 linkage primitives: `compute_counterparty_linkage` (33-byte ECDH point), `compute_specific_linkage` (32-byte HMAC). Reuses `brc42` helpers. 6 unit tests. | None — pure Rust |
| `rust-wallet/src/crypto/mod.rs` | Register `pub mod key_linkage;` | None — pure Rust |
| `rust-wallet/src/handlers.rs` | Two new handlers (`reveal_counterparty_key_linkage`, `reveal_specific_key_linkage`); identity-key gate added to `get_public_key` (accepts X-Identity-Key-Approved header OR persistent DB column); `SetDomainPermissionRequest` accepts `identityKeyDisclosureAllowed`; all three `*domain_permission*` endpoints serialize new field | None — pure Rust |
| `rust-wallet/src/main.rs` | Register `POST /revealCounterpartyKeyLinkage` + `POST /revealSpecificKeyLinkage` in Identity routes block | None — pure Rust |
| `rust-wallet/src/database/migrations.rs` | New `migrate_v16_to_v17` adds `identity_key_disclosure_allowed INTEGER NOT NULL DEFAULT 0` to `domain_permissions`. Idempotent (PRAGMA table_info check) | None — SQLite |
| `rust-wallet/src/database/connection.rs` | Wire V17 into migration runner | None — pure Rust |
| `rust-wallet/src/database/models.rs` | `DomainPermission.identity_key_disclosure_allowed: bool` field + `defaults()` sets false | None — pure Rust |
| `rust-wallet/src/database/domain_permission_repo.rs` | SELECT/INSERT/UPDATE/list_all all read/write the new column | None — pure Rust |
| `cef-native/include/core/HttpRequestInterceptor.h` | Declared `MarkIdentityKeyRevealApproved`, `MarkKeyLinkageRevealApproved`, `ForwardPendingWalletRequest` free functions | Header — cross-platform |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | New file-local singletons `IdentityKeyApprovalCache` + `KeyLinkageApprovalCache` (both `std::set<std::string>` + `std::mutex`); `DomainPermissionCache::Permission` gains `identityKeyDisclosureAllowed` bool, parsed from JSON response; new isWalletEndpoint entries `/revealCounterpartyKeyLinkage` + `/revealSpecificKeyLinkage`; new privacy-perimeter gates in `Open()` for identity-key and key-linkage reveal; new `triggerIdentityKeyRevealModal` + `triggerKeyLinkageRevealModal` helpers (mirror `triggerCertificateDisclosureModal`); `startAsyncHTTPRequest` adds `X-Identity-Key-Approved` header on cache hit AND runs a safety-net privacy-perimeter gate for drain/sibling-forwarded requests that bypassed `Open()`; `addDomainPermission` + `addDomainPermissionAdvanced` accept `identityKeyDisclosureAllowed` and mirror it into both `DomainPermissionCache` and `IdentityKeyApprovalCache`; `DomainPermissionTask` + `AdvancedDomainPermissionTask` POST the new field to Rust; `ForwardPendingWalletRequest` helper exposed for `simple_handler.cpp` | **Pure C++ — uses only CefPostTask, std::mutex/set, nlohmann::json, and the existing `Win32 #ifdef _WIN32 / #else` WinHTTP/SyncHttpClient path that's already cross-platform.** No new `#ifdef` blocks added. |
| `cef-native/src/handlers/simple_handler.cpp` | Two new IPC dispatchers (`approve_identity_key_reveal`, `approve_key_linkage_reveal`); `add_domain_permission` + `add_domain_permission_advanced` parse `identityKeyDisclosureAllowed` (default true) and pass through; drain paths replaced `popAllForDomain`-and-discard with `ForwardPendingWalletRequest`-forwarding-real-handlers (BRC-121 nullptr handlers still flow through `TriggerPendingBrc121Reloads`) | Cross-platform CEF APIs only |
| `frontend/src/pages/BRC100AuthOverlayRoot.tsx` | Two new prompt branches (`identity_key_reveal`, `key_linkage_reveal`) using `hodosTheme.prompt.privacyPerimeter` framing; locked minimal-neutral copy; "Always allow for this site" checkbox; new approve/deny IPCs (`approve_identity_key_reveal`, `approve_key_linkage_reveal` + `brc100_auth_response`); new "Allow this site to identify you" checkbox on `domain_approval` modal (default ON) wires into `add_domain_permission` + `add_domain_permission_advanced` payloads; `allowIdentityKey` state resets to ON for each fresh prompt | None — React |

#### CEF APIs used — all cross-platform

Step 1's C++ additions use only:
- `CefPostTask` / `CefResourceHandler` / `CefCallback` (cross-platform CEF)
- `CefURLRequest` + `CefPostData` (cross-platform CEF)
- `std::mutex`, `std::set`, `std::string` (C++ stdlib)
- `nlohmann::json` (cross-platform)
- Existing `SyncHttpClient` abstraction (already has Win/Mac branches)

No new platform-specific APIs introduced. No new `#ifdef _WIN32` / `#elif defined(__APPLE__)` blocks added.

#### macOS verification checklist (Step 1)

**Visual:**

- [ ] `identity_key_reveal` prompt renders with `prompt.privacyPerimeter` framing (gold border + soft halo, 18px/700 header) on macOS
- [ ] `key_linkage_reveal` prompt same — verifier hex truncated to `first4...last2`, kind-aware copy (counterparty vs specific) renders
- [ ] "Always allow for this site" checkbox on both prompts is visible, native styling, default unchecked
- [ ] `domain_approval` modal shows the new "Allow this site to identify you" checkbox between reassurance text and Advanced settings toggle (default ON) on macOS

**Behavioral:**

- [ ] Visit a fresh site → bundle checkbox stays default ON → click Allow → site connects without a second popup. Verify in macOS log: `🔐 Setting domain permission ... identityKeyDisclosure=1` AND `🛡️ identity-key reveal silently approved`.
- [ ] Visit a fresh site → UNCHECK the bundle checkbox → click Allow → the privacy-perimeter prompt fires as a second step (safety-net gate in `startAsyncHTTPRequest` catches drain-forwarded siblings). Verify in macOS log: `🛡️ identity-key-style /getPublicKey bypassed Open() for <site> ... — firing identity_key_reveal prompt`.
- [ ] Approve a site → close app → relaunch → revisit. Expected: silent connect, no identity-key prompt. Verify with `sqlite3 ~/Library/Application\ Support/HodosBrowserDev/wallet/wallet.db "SELECT identity_key_disclosure_allowed FROM domain_permissions WHERE domain='<site>';"` returns 1.
- [ ] Right-click "Manage Site Permissions" still opens the `edit_permissions` overlay correctly (existing behavior, not changed in Step 1).
- [ ] **BRC-121 payment animation non-regression** — gold pill animation still fires on auto-approved payments at `now.bsvblockchain.tech/articles/<slug>`.
- [ ] Drain-forward fix: visit a site that makes parallel BRC-100 calls during connect → after Allow, all queued requests resolve. Verify in macOS log: `🔐 Drained N pending request(s) for <site> after approval (N forwarded, 0 BRC-121)`.

**Build verification:**

- [ ] `rust-wallet` builds clean on macOS (`cargo build --release`)
- [ ] `cef-native` builds clean on macOS (the `#elif defined(__APPLE__)` path in `DomainPermissionCache::fetchFromBackend` parses the new `identityKeyDisclosureAllowed` field via the same nlohmann::json path — verify the new field shows up in the macOS fetch result)
- [ ] `frontend` builds clean (`npm run build`) — Step 1 is pure cross-platform React

**Migration verification:**

- [ ] First launch with a pre-existing dev DB (at V16) shows in log: `Applying migration V17 (identity_key_disclosure_allowed)... ✅ Schema V17 applied`
- [ ] `PRAGMA table_info(domain_permissions);` on macOS dev DB lists `identity_key_disclosure_allowed` as the last column

#### Known follow-ups to also check on Mac

- The Step 1 follow-up bug we fixed (drain-forwarded siblings hitting Rust 403 instead of firing the second prompt) was a logic bug, not a Win-only one. The safety-net check in `startAsyncHTTPRequest` runs on both platforms identically. Verify on Mac by repeating the unchecked-bundle smoke step.
- `KeyLinkageApprovalCache` is in-memory only (no DB persistence in Step 1 per user direction). On Mac this means restarting the browser re-prompts for key-linkage revelation — same as Windows. If a Mac user reports this surprises them, surface as a Step 5+ ask.

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
