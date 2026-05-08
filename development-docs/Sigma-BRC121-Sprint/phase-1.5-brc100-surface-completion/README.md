# Phase 1.5 — BRC-100 Surface Completion

**Type:** Implementation. Gates Phase 2 (V8 shim) and Phase 3 (ordinals).
**Status:** Scoped 2026-05-06. Awaiting DB schema walkthrough before migration is written.
**Sizing:** ~1 sprint week. Rust + C++ + React touches, all additive.

> **Read this first:** `PERMISSION_UX_DESIGN.md` in this folder. It explains why this phase looks the way it does — the research, the matrices, and the design intent. This README is implementation-shaped.

---

## Why this phase exists

Two gaps surfaced from `phase-0.1-brc100-audit/AUDIT_RESULTS.md`:

1. **Two BRC-100 methods are missing** — `revealCounterpartyKeyLinkage`, `revealSpecificKeyLinkage`. Canonical (not Yours-specific). Small, additive.
2. **Hodos's permission model is coarser than canonical BRC-100.** Today, a domain approved via `domain_permissions` gets implicit access to any protocolID, counterparty, or basket. BRC-100 expects per-(origin, scope) gating across these axes. Phase 2 (V8 shim) needs the finer-grained gating to be a faithful BRC-100 substrate.

Closing both gaps before Phase 2 ships is what makes Hodos faithful to BRC-100 and unlocks the manifest-driven first-visit bundle UX that the design doc proposes.

---

## Architectural divergence from `@bsv/wallet-toolbox` (deliberate)

The canonical `@bsv/wallet-toolbox` (used by Yours's `brc100-remote` and Babbage's reference) stores permissions **on-chain as PushDrop UTXOs** in four named admin baskets — `admin protocol-permission`, `admin basket-access`, `admin certificate-access`, `admin spending-authorization`. There is **no SQL `permissions` table in the toolbox at all.**

**Hodos diverges deliberately.** We store grants in local SQLite. Trade-offs:

| Lose | Gain |
|---|---|
| Native expiry via UTXO spending | Zero on-chain transaction cost per grant |
| Cross-device sync via UTXO sync (which isn't shipping anywhere yet — research found this is infrastructure debt) | Faster query path (5-minute cache hit ≪ on-chain check) |
| Cryptographic revocation (UTXO spent = grant gone) | Simpler implementation; no UTXO management for grants |
| Direct toolbox interop (their permissions don't show up in our DB and vice versa) | No upstream migration risk |

**Mitigations baked into the schema:**

- **`expires_at` column** — explicit expiry instead of relying on UTXO lifecycle.
- **`revoked_at TEXT` column** (soft-delete) — revocations stay queryable for audit instead of disappearing with a UTXO spend.
- **No claim of toolbox interop** — `wallet-manifest.json` and our grant tables are Hodos-native; if/when we want toolbox-format mirroring, that's a Phase 4+ option.

This trade-off is documented up-front so future readers know the divergence is intentional, not an oversight.

---

## What's already in place (no need to build)

Hodos has substantial existing infrastructure that Phase 1.5 builds on, not parallel to:

| What exists today | Where |
|---|---|
| `domain_permissions` table with `trust_level`, `per_tx_limit_cents` (default $1), `per_session_limit_cents` (default $10), `rate_limit_per_min` (default 30), `max_tx_per_session` (default 100) | `rust-wallet/src/database/migrations.rs:468-481` |
| `cert_field_permissions` child table joined by FK to `domain_permissions` | `rust-wallet/src/database/migrations.rs:486-494` |
| Domain permission CRUD endpoints (`/domain/permissions/*`) | `rust-wallet/src/handlers.rs` (around lines 9230, 9267, 9332, 9446, 16370) |
| `ApprovedSitesTab.tsx` — Default Limits + per-site management | `frontend/src/components/wallet/ApprovedSitesTab.tsx` |
| `DomainPermissionsTab.tsx` — table with sort/paginate/edit/revoke | `frontend/src/components/DomainPermissionsTab.tsx` |
| `DomainPermissionForm.tsx` — edit form with **"Always notify" toggle** that zeros all limits + warning if limits exceed $5/tx or $50/session | `frontend/src/components/DomainPermissionForm.tsx` |
| `BRC100AuthOverlayRoot.tsx` — connect / payment / cert-disclosure prompt | `frontend/src/pages/BRC100AuthOverlayRoot.tsx` |
| **Right-click "Manage Site Permissions"** context menu opens the form quickly to revoke | `cef-native/src/handlers/simple_handler.cpp:6696, 6780, 6989` (`MENU_ID_MANAGE_PERMISSIONS`) |
| Token UI grouped by basket name | `frontend/src/components/wallet/TokensTab.tsx` |
| Global default limits in settings | `/wallet/settings` GET/POST → `default_per_tx_limit_cents`, etc. |

**The phase preserves all of the above unchanged** in shape. New columns and new child tables hang off `domain_permissions` via FK with CASCADE delete, mirroring how `cert_field_permissions` already does it.

---

## Scope — three deliverables

### A. The two missing BRC-100 handlers

**Files:**
- New: `rust-wallet/src/crypto/key_linkage.rs` (BRC-72 linkage encryption, BRC-42 derivation reuse)
- Edited: `rust-wallet/src/handlers.rs` — add `reveal_counterparty_key_linkage`, `reveal_specific_key_linkage`
- Edited: `rust-wallet/src/main.rs` — register routes `POST /revealCounterpartyKeyLinkage`, `POST /revealSpecificKeyLinkage` near line 779 (Identity group)
- Edited: `cef-native/src/core/HttpRequestInterceptor.cpp` — add the 2 routes to `isWalletEndpoint()`
- New overlays: `KeyLinkageRevealOverlayRoot.tsx`, `IdentityKeyRevealOverlayRoot.tsx` (always-prompt privacy perimeter)

These are **always-prompt privacy perimeter calls** regardless of `domain_permissions` settings — never auto-approve. `getPublicKey({ identityKey: true })` similarly routes to the new identity-key reveal overlay (today it's silent on approved domains; Phase 1.5 fixes that).

### B. Three new child tables of `domain_permissions`

Added shape, mirroring `cert_field_permissions` (FK + CASCADE delete + UNIQUE constraint):

```sql
-- Per-protocol grants (BRC-100 PermissionRequest type='protocol')
CREATE TABLE domain_protocol_permissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain_permission_id INTEGER NOT NULL,
    protocol_security_level INTEGER NOT NULL,    -- 0, 1, or 2
    protocol_name TEXT NOT NULL,
    key_id TEXT NOT NULL DEFAULT '*',             -- '*' = wildcard
    counterparty TEXT,                            -- NULL = any
    expires_at INTEGER,                           -- UNIX seconds; NULL = never (warned UX)
    created_at INTEGER NOT NULL,
    FOREIGN KEY (domain_permission_id) REFERENCES domain_permissions(id) ON DELETE CASCADE,
    UNIQUE(domain_permission_id, protocol_security_level, protocol_name, key_id, counterparty)
);
CREATE INDEX idx_domain_protocol_perms_domain ON domain_protocol_permissions(domain_permission_id);

-- Per-basket grants (BRC-100 PermissionRequest type='basket')
CREATE TABLE domain_basket_permissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain_permission_id INTEGER NOT NULL,
    basket TEXT NOT NULL,
    access TEXT NOT NULL,                         -- 'read' | 'read_write'
    expires_at INTEGER,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (domain_permission_id) REFERENCES domain_permissions(id) ON DELETE CASCADE,
    UNIQUE(domain_permission_id, basket)
);
CREATE INDEX idx_domain_basket_perms_domain ON domain_basket_permissions(domain_permission_id);

-- Per-counterparty grants (BRC-100 CounterpartyPermissionRequest, level-2 protocols)
CREATE TABLE domain_counterparty_permissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain_permission_id INTEGER NOT NULL,
    counterparty TEXT NOT NULL,                   -- hex pubkey
    expires_at INTEGER,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (domain_permission_id) REFERENCES domain_permissions(id) ON DELETE CASCADE,
    UNIQUE(domain_permission_id, counterparty)
);
CREATE INDEX idx_domain_counterparty_perms_domain ON domain_counterparty_permissions(domain_permission_id);

-- Optional: extend cert_field_permissions with sensitivity classification
ALTER TABLE cert_field_permissions ADD COLUMN sensitivity TEXT NOT NULL DEFAULT 'unknown';
-- Values: 'low' | 'medium' | 'high' | 'highest' | 'unknown'
```

Migration version: V25 (or whatever is current at implementation time).

**`expires_at` semantics (revised after schema research):** Default is **NULL = never expires**. The existing `certificates` table has no `expiry`/`valid_until` column (verified in `migrations.rs:170-185`) and the canonical `@bsv/wallet-toolbox` schema has no expiry column either — BRC-52 cert lifecycle is anchored on `revocation_outpoint`, spending it revokes. We follow the same convention for our domain sub-permissions: NULL = never, with explicit `revoked_at TEXT` column for soft-delete (so revocations stay queryable).

If a future cert type ships an explicit expiry timestamp in its `fields` payload, the cert disclosure flow can read and surface it — no DB column change needed. Periodic check of `revocation_outpoint` on-chain status is a long-term enhancement (separate sprint).

**No new top-level tables.** No `protocol_permissions` parallel to `domain_permissions`. No `permission_audit_log`. No `user_tier_preset`. No `spending_grants`. The audit-log idea is **deferred to a future browser-history-and-permissions sprint**, per user direction.

### C. Permission engine + manifest fetcher

**Permission engine (C++):** new `cef-native/include/core/PermissionEngine.h` and `.cpp`. Sits between `HttpRequestInterceptor::AsyncWalletResourceHandler` and the forward to `localhost:31301`. For each BRC-100 call from an origin:

1. Fetch domain permission row (cache → SQLite).
2. Classify call (privacy perimeter? bundled at connect? new sub-permission?).
3. Check sub-permission tables (protocol/basket/counterparty/cert-field).
4. Check counters in existing `SessionManager`.
5. Decide: `SILENT` / `PROMPT(kind)` / `DENY`.

Decision logic per `PERMISSION_UX_DESIGN.md` §3 Matrix C.

**Manifest fetcher:** on first connect, fetch `/.well-known/wallet-manifest.json`. Parse and cache. Render bundled connect prompt if present; lightweight per-call prompts if absent. The manifest format is documented in `PERMISSION_UX_DESIGN.md` §5.

**Defense-in-depth:** the same permission gates also live in Rust handlers (additive top-of-handler calls). If C++ misroutes, Rust still rejects.

---

## UI changes (extend, don't replace)

Existing components that get **extended**, not redesigned:

### `DomainPermissionForm.tsx` — extend existing form

- Keep all current fields ($/tx, $/session, rate, max tx/session, "Always notify" toggle).
- Add new "Allow without limits (advanced)" button — sets per_tx and per_session to a very high value (e.g. 100000 cents = $1000) with the existing warning banner adapted, plus a don't-show-again checkbox.
- Add new collapsible section: **"Specific permissions"** — lists per-protocol, per-basket, per-counterparty grants for this domain. Each entry shows scope + expiry + revoke button.
- Add new collapsible section: **"Certificate fields"** — extends the existing cert-field handling. Shows fields with their sensitivity tier (low/medium/high/highest/unknown); user can override sensitivity per-field for this domain.

### `ApprovedSitesTab.tsx` — extend existing Default Limits section

- Keep all current default-limits inputs.
- Add new "Allow without limits (advanced)" button alongside "Reset All."
- Add new editable JSON view of the global sensitivity classifier (regex map → tier). Default seed values from the conservative mapping below; user can edit. Validation prevents malformed regex.

### `BRC100AuthOverlayRoot.tsx` — extend existing connect prompt

- Keep all current behavior (domain approval, payment confirmation, rate limit prompts, cert disclosure).
- Add new code path for **manifest-driven connect bundle** when the manifest fetcher has results. Renders the bundled permissions in plain language. Three buttons: Connect / Customize / Decline.
- Add new cert-disclosure path that respects sensitivity tiers (low = bundled, medium/high/highest = always individual prompt).

### NEW prompt types — added to the shared `notification_browser_` overlay

The existing `notification_browser_` overlay (HWND on Windows, NSPanel on macOS) already multiplexes 6 prompt types via the `type` query param: `domain_approval`, `payment_confirmation`, `certificate_disclosure`, `rate_limit_exceeded`, `no_wallet`, `edit_permissions`. Lives in `BRC100AuthOverlayRoot.tsx` (renders by type), `simple_app.cpp::CreateNotificationOverlay()` (Win), and `cef_browser_shell_mac.mm` (Mac).

**Phase 1.5 adds 5 NEW types to this same overlay** — no new HWNDs, no new NSPanels, no new platform-creation functions. Just new cases in the React dispatch and new triggers in `HttpRequestInterceptor.cpp`:

| New type | Trigger | UX |
|---|---|---|
| `manifest_connect_bundle` | First-visit when `wallet-manifest.json` is fetched | Bundled connect prompt with plain-language permissions list (replaces/augments `domain_approval` when manifest exists) |
| `identity_key_reveal` | `getPublicKey({ identityKey: true })` from any origin | Always prompt; extra-prominent warning |
| `key_linkage_reveal` | `revealCounterpartyKeyLinkage` or `revealSpecificKeyLinkage` | Always prompt; names the verifier in plain language |
| `protocol_permission_prompt` | Manifest-less site requests new (origin, protocolID, keyID) | Lightweight; "Allow this and others from this site" option |
| `counterparty_permission_prompt` | Level-2 protocol asks about new counterparty | Same shape |

**Why this matters for sizing and platform parity:** Earlier draft proposed 4 new HWND/NSPanel overlays = 4 × 2 platforms = 8 new creation paths. Shared-overlay approach is 0 new creation paths — just React component cases. **Significant scope reduction.** Step 5 is faster and the Win/Mac parity surface is much smaller.

### What we explicitly DO NOT change

- **Right-click "Manage Site Permissions"** context menu (`MENU_ID_MANAGE_PERMISSIONS` at `simple_handler.cpp:6696`) — preserved exactly as-is. We test that it still opens the form correctly after every UI change.
- **Payment success animation pipeline** — every auto-approved payment fires the **tab payment badge animation** so the user has a visible signal even when no prompt appears. Pipeline:
  - `HttpRequestInterceptor.cpp:1656-1681` sends `payment_success_indicator` IPC after `UR_SUCCESS` on auto-approved payments, with `{ browserId, domain, cents }`.
  - `simple_render_process_handler.cpp:1020` receives and dispatches via `window.postMessage`.
  - `useTabManager.ts:141` listens and triggers the green-dot animation on the tab.
  - **Phase 1.5 must keep this firing** when handlers are rewired through the new permission engine. Engine's silent-approve path MUST send the same IPC. Add a regression test that asserts the animation fires for an auto-approved payment going through the engine.
  - **Phase 2 (V8 shim) must also fire this** for `window.CWI` / `window.yours` / `window.panda` payments. As long as shim methods route through the canonical IPC path (per `SHIM_TRANSLATION_SPEC.md` permission-gate routing diagram), the indicator fires automatically — but call it out explicitly in the shim acceptance test.
- Per-session counter behavior (resets on tab close). Per user direction, no change.
- Existing 26 BRC-100 handler bodies (additive permission-gate calls only at the top).
- Existing `crypto/brc42.rs`, `crypto/signing.rs`, `crypto/keys.rs` — invariant 3.
- Existing core tables (`wallets`, `users`, `addresses`, `outputs`, `transactions`, `certificates`, `domain_permissions`).
- CEF lifecycle/threading/message loop.
- HTTP interception routing semantics.

---

## Default sensitivity classifier (initial seed)

Editable globally and per-site post-Phase 1.5. Initial seed (from the conservative mapping the research surfaced):

| Tier | Field-name regex examples | Connect behavior |
|---|---|---|
| `low` | `name`, `username`, `displayName`, `profilePhoto`, `avatar`, `bio` | Bundle into connect prompt (auto-approve OK) |
| `medium` | `email`, `country`, `age`, `language` | Show in connect prompt with opt-out required |
| `high` | `phone`, `address`, `employer`, `street` | Always prompt individually |
| `highest` | `dob`, `dateOfBirth`, `nationalId`, `passportNumber`, `ssn`, `bankAccount` | Never auto-include; explicit prompt + confirm |
| `unknown` (no match) | (anything not matching above) | Treat as `high` — always prompt |

Stored as JSON in settings (e.g. `cert_field_sensitivity_classifier`). User can edit globally in `ApprovedSitesTab`. Per-site overrides stored in a small JSON column on `cert_field_permissions` row, edited in `DomainPermissionForm`.

---

## Implementation order (each step is independently mergeable)

### Step 1 — Missing handlers + privacy-perimeter overlays

- `crypto/key_linkage.rs` + 2 handlers + 2 routes.
- New overlays: `IdentityKeyRevealOverlayRoot`, `KeyLinkageRevealOverlayRoot` (Win + Mac).
- Hard-code "always prompt" — no engine integration yet.

**Test:** test page calls `/revealCounterpartyKeyLinkage`; prompt fires on Win and Mac; on approve, encrypted linkage returns; on deny, typed error returns. `getPublicKey({ identityKey: true })` now prompts where it was silent before.

### Step 2 — DB schema (after walkthrough + approval)

- Migration V25 with the three new child tables + the `sensitivity` column.
- New repos following existing pattern (`domain_protocol_permission_repo.rs`, etc.) or one combined permission repo file.
- No handler changes yet — repos isolated.

**Test:** unit tests on each repo for insert/query/revoke/expiry. CASCADE delete from `domain_permissions` removes child rows.

### Step 3 — Permission engine (C++)

- New `PermissionEngine.h` + `.cpp`.
- New IPC types in `simple_handler.cpp`.
- New endpoints on Rust side: `/wallet/permissions/check` (combined gate query), `/wallet/permissions/save`, `/wallet/permissions/revoke`, `/wallet/permissions/list`. Engine calls these via existing localhost:31301 path.

**Test:** unit tests for the engine's decision logic against fixture inputs covering all four sub-types (protocol/basket/counterparty/cert-field) and the privacy perimeter.

### Step 4 — Manifest fetcher

- C++ module fetches `/.well-known/wallet-manifest.json` on first connect.
- Parses + caches in memory; optionally persists in SQLite for offline.
- Falls back gracefully when missing.

**Test:** test against a manifest-shipping demo dApp + a manifest-less site; both produce reasonable connect prompts.

### Step 5 — Extend existing UI

- `BRC100AuthOverlayRoot` — manifest-driven connect bundle path + sensitivity-aware cert disclosure.
- `DomainPermissionForm` — "Allow without limits" button, "Specific permissions" section, "Certificate fields" section.
- `ApprovedSitesTab` — "Allow without limits" globally, sensitivity classifier editor.
- New overlays: `ProtocolPermissionPromptOverlayRoot`, `CounterpartyPermissionPromptOverlayRoot`.

**Test:** smoke against auth-category sites (x.com, google.com, github.com) on both Win + Mac. Verify right-click "Manage Site Permissions" still works.

### Step 6 — Rewire existing handlers through the engine

- Add permission-gate calls at the top of each Rust BRC-100 handler.
- Bodies untouched; gate is one line above existing logic.
- All 26 + 2 new methods route through the engine.

**Test:** existing test suite passes. New fresh-origin tests assert the gate fires.

### Step 7 — Demo prep

- Build a minimal demo dApp shipping a `wallet-manifest.json`. Used as the smoke target for Step 4.
- Document app-dev best practices in a draft for `phase-4-demos/DEV_GUIDE.md`.

---

## Test plan (per-phase, runs on every step's PR)

| Test type | Where | What it covers |
|---|---|---|
| **Rust unit tests** | `rust-wallet/tests/` | Each new repo's CRUD; `crypto/key_linkage.rs` round-trip vs known-good fixtures from `@bsv/sdk` if available |
| **C++ unit tests** | `cef-native/tests/` (new dir) | `PermissionEngine` decision logic; manifest parser |
| **Integration tests** | `cef-native/tests/integration/` | Engine → Rust handler → DB round-trip |
| **Manual smoke (Win)** | local | Auth-category sites still log in; right-click Manage Site Permissions still works; new overlays render; **green-dot tab payment animation fires on every auto-approved payment** |
| **Manual smoke (Mac)** | local | Same as Win — every overlay must work on macOS too before merge; same payment animation check |
| **BRC-100 conformance** | `rust-wallet/tests/brc100_conformance/` | Each of the 28 methods accepts canonical args from `@bsv/sdk@2.0.13` and returns canonical responses |
| **Regression** | manual | Existing connect / payment / cert flows still work; existing `domain_permissions` rows still respected |
| **Cross-platform parity** | manual | Each new overlay opens, accepts input, returns the right decision on both platforms |

---

## Cross-platform parity

Every new overlay needs both Win and Mac creation paths before merge:

| Platform | Path | Window class |
|---|---|---|
| Windows | `cef-native/src/handlers/simple_app.cpp` `CreateXxxOverlay()` functions | `WS_POPUP` |
| macOS | `cef-native/src/cef_browser_shell_mac.mm` equivalents | `NSPanel` |

Same React component on both — only the C++ creation path differs.

Smoke-test sites (per root `CLAUDE.md`):
- Auth: x.com, google.com, github.com
- Video: youtube.com (sanity)
- BSV: 1sat.market, treechat.com (manifest-shipping target if available; otherwise the manifest-less fallback path)

---

## Files this phase touches

### NEW

**Rust:**
- `rust-wallet/src/crypto/key_linkage.rs`
- `rust-wallet/src/database/domain_protocol_permission_repo.rs`
- `rust-wallet/src/database/domain_basket_permission_repo.rs`
- `rust-wallet/src/database/domain_counterparty_permission_repo.rs`
- `rust-wallet/src/database/migrations/v25_*.sql` (after schema walkthrough)

**CEF / C++:**
- `cef-native/include/core/PermissionEngine.h`
- `cef-native/src/core/PermissionEngine.cpp`
- `cef-native/src/core/ManifestFetcher.cpp` (or equivalent)

**React:**
- (no new overlay files — new prompt types are added to existing `BRC100AuthOverlayRoot.tsx` via type dispatch)

### EDITED (additive only)

**Rust:**
- `rust-wallet/src/handlers.rs` — 2 new handlers + permission-gate calls atop 26 existing
- `rust-wallet/src/main.rs` — 2 new BRC-100 routes + 4 new permission-management routes
- `rust-wallet/src/database/connection.rs` — register new migration

**CEF / C++:**
- `cef-native/src/handlers/simple_handler.cpp` — new IPC dispatchers for permission engine; new permission-prompt type triggers
- `cef-native/src/core/HttpRequestInterceptor.cpp` — 2 new wallet endpoints + 4 permission endpoints, route through `PermissionEngine`; trigger new prompt types via `CreateNotificationOverlayTask` with new `type` strings
- (no new overlay creation functions — reuse existing `CreateNotificationOverlay`)

**React:**
- `frontend/src/components/DomainPermissionForm.tsx` — "Allow without limits" + Specific permissions + Cert fields sections
- `frontend/src/components/wallet/ApprovedSitesTab.tsx` — "Allow without limits" + sensitivity classifier editor
- `frontend/src/pages/BRC100AuthOverlayRoot.tsx` — manifest-bundle path + sensitivity-aware disclosure
- `frontend/src/App.tsx` — register new routes

### EXPLICITLY UNTOUCHED

- All 26 existing BRC-100 handler bodies
- `rust-wallet/src/crypto/brc42.rs`, `crypto/brc43.rs`, `crypto/signing.rs`, `crypto/keys.rs`
- `wallets`, `users`, `addresses`, `outputs`, `transactions`, `certificates` tables
- Existing `domain_permissions` table shape (we add child tables, not modify)
- `MENU_ID_MANAGE_PERMISSIONS` right-click context menu
- CEF lifecycle / threading / message loop
- Per-session counter reset behavior

---

## Sizing

| Step | Days |
|---|---|
| 1 — Missing handlers + privacy perimeter prompt types (shared overlay) | 0.75 |
| 2 — DB migration + repos | 1 |
| 3 — Permission engine + IPC + Rust gate calls | 1.5 |
| 4 — Manifest fetcher | 0.5 |
| 5 — Extend existing UI + new prompt types in shared overlay | 1 |
| 6 — Rewire existing handlers through engine | 0.5 |
| 7 — Demo prep | 0.5 |
| Cross-platform parity testing (smaller surface — shared overlay) | 0.5 |
| **Total** | **~6.25 days** |

Buffer to 8–10 days for integration / debugging / platform quirks. Shared-overlay refactor saved ~1.5 days vs the original draft.

---

## What this phase does NOT do (deferred work)

- **Audit log** of every permission decision — deferred to future browser-history-and-permissions sprint per user direction.
- **Tier preset abstraction** (Cautious / Balanced / Power) — dropped; existing Default Limits + "Always notify" toggle + new "Allow without limits" button cover the user-tier UX with no new tables.
- **On-chain permission tokens** — deferred (research found Babbage's on-chain grants are infrastructure debt; UTXO sync isn't robust).
- **Action registry** (translating protocolIDs to plain verbs in connect prompts) — **deferred to research phase right after Phase 1.5**, per user direction. Open questions: is it standardized via BSVA? de-facto in the ecosystem? long-term adoption likely?
- **`bsv:announceProvider` BRC submission** — drafted as part of design, ship in Phase 2; formal BRC submission deferred until demo phase shows ecosystem appetite.
- **`wallet-manifest.json` BRC submission** — same.
- **Ordinal-specific UI** — Phase 3 / a separate UI phase.
- **Phase 4 demo apps** — preps here, builds in Phase 4.

---

## Dependencies

**Inbound:**
- DB schema walkthrough + approval per CLAUDE.md invariant 2 (table-by-table review of the three child tables and the `sensitivity` column).
- Confirmation of `expires_at` default (proposed: 1 year, with "never" available as user choice with warning).

**Outbound:**
- **Phase 2** — V8 shim assumes the permission engine exists.
- **Phase 3** — ordinal classification can hook into `domain_basket_permissions` cleanly.
- **Phase 4** — demo apps using the manifest format need the backend to honor it.

---

## Status

- [x] `PERMISSION_UX_DESIGN.md` written (then trimmed)
- [x] User reviewed open questions
- [x] Existing infrastructure mapped and acknowledged
- [ ] DB schema table-by-table walkthrough + approval
- [ ] `expires_at` default confirmed
- [ ] Implementation begins (Step 1 first)

---

## Notes for future work

Captured during this phase, deferred to later sprints:

1. **BRC submission for `wallet-manifest.json`** — propose to the ecosystem after demo phase. Pattern is BSV-native (no EVM-specific primitives like ERC-7715's `to: Address` or `token: address`), so it stands on its own.
2. **BRC submission for cert-field `sensitivityHints`** — propose to certifier-facing BRC. Hodos ships its own classifier in the meantime; certifier-provided hints become the override path.
3. **Action registry research** — three questions to answer before deciding: Is there a BSVA standard already? Is one becoming de-facto? What's long-term adoption likely? Decide implementation approach after research.
4. **Audit log of permission decisions** — fits a future "browser data + permissions refactor" sprint.
5. **On-chain permission token mirroring** — revisit when UTXO sync is more mature.

---

## References

- `PERMISSION_UX_DESIGN.md` — design doc with research, matrices, manifest format, UX mock-ups
- `../phase-0.1-brc100-audit/AUDIT_RESULTS.md` — coverage gaps this phase fills
- `../phase-0.2-window-yours-shim-design/SHIM_TRANSLATION_SPEC.md` — what Phase 2 needs from this phase
- `../ARCHITECTURE.md` — sprint-level diagrams
- Root `CLAUDE.md` — invariants, testing standards, platform notes
- `rust-wallet/CLAUDE.md`, `rust-wallet/src/CLAUDE.md`, `rust-wallet/src/database/CLAUDE.md` — Rust context
- `cef-native/CLAUDE.md` and subfolder CLAUDE.md files — CEF context
- `frontend/src/components/wallet/CLAUDE.md` — wallet UI tabs context
