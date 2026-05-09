# Sprint Checklist (revised 2026-05-07)

Cross-phase checklist matching the phase structure in `README.md`. Per-phase detail lives in each phase folder.

## Every phase begins with a kickoff review

Per root `CLAUDE.md` "Phase kickoff workflow":
- [ ] Re-read this phase's docs + linked sources
- [ ] Verify cited file:line references are still current (don't trust prior session memory)
- [ ] **Reuse-first audit:** map every change to existing functions/code. If tempted to write something new, first prove the equivalent doesn't exist
- [ ] Risk assessment: what existing functionality could this touch? Especially the load-bearing UX safeguards (right-click manage permissions, `payment_success_indicator` animation, "Always notify" toggle, privacy perimeter)
- [ ] Confirm the test plan is actionable for this phase
- [ ] Hand back tight summary asking for confirmation before any code is written

## Phase 0.1 — BRC-100 Audit ✅ COMPLETE (2026-05-06)
- [x] Map each of the 28 canonical `WalletInterface` methods to existing Hodos handlers
- [x] Document argument-shape match against `@bsv/sdk@2.0.13` for each method
- [x] Identify gaps (`revealCounterpartyKeyLinkage`, `revealSpecificKeyLinkage` missing)
- [x] Plan extension to BRC-100's permission model (per-protocol, basket, counterparty, certificate)
- [x] Deliverable: `phase-0.1-brc100-audit/AUDIT_RESULTS.md`

## Phase 0.2 — `window.yours` Shim Design ✅ COMPLETE (2026-05-06)
- [x] signMessage protocolID/keyID convention (`yours-legacy-v1`, level 1, anyone counterparty)
- [x] getAddresses fallback (Option C: BRC-42 fresh-address generator with `yours-legacy-receive`)
- [x] encrypt/decrypt argument translation (multi-recipient verification deferred to v4.5.6 source check)
- [x] sendBsv → createAction mapping (inscription-bearing → typed error)
- [x] getBalance computation strategy (sum of `listOutputs({ basket: 'default' })`)
- [x] Removed-methods handling (typed errors with explanation)
- [x] Ordinal-method shim posture (typed `NOT_IMPLEMENTED_PRE_PHASE_3` error in v1)
- [x] Deliverable: `phase-0.2-window-yours-shim-design/SHIM_TRANSLATION_SPEC.md`

## Phase 1 — BRC-121 ✅ COMPLETE (initial scope shipped 2026-05-08 at `0a73b98`; polish pass 2026-05-09 at `c11afbf` + this commit)
**Reuse map (verified at kickoff):** BRC-29 protocol ID `3241645161d8` (`handlers.rs:4348`), `peerpay_send` (`handlers.rs:15224`), `paymail_send` (`handlers.rs:15637`), `create_action_internal` (`handlers.rs:3577`), `OnResourceResponse` stub (`HttpRequestInterceptor.cpp:2056`), `payment_success_indicator` IPC (`HttpRequestInterceptor.cpp:1656-1681`).
- [x] Kickoff review (per cross-phase checklist above)
- [x] Rust handler `pay_402` in `rust-wallet/src/handlers.rs` — mirrors `peerpay_send` shape; reuses BRC-29 invoice format
- [x] Route `/wallet/pay402` registered in `rust-wallet/src/main.rs`
- [x] CEF interception: filled `OnResourceResponse` in `cef-native/src/core/HttpRequestInterceptor.cpp` (production: also `CookieFilterResourceHandler::OnResourceResponse` since 3rd-party HTTP doesn't go through `HttpRequestInterceptor`; both delegate to free `TryHandleBrc121_402`)
- [x] Added `/wallet/pay402` to `isWalletEndpoint` route table
- [x] Auto-approve integration via existing `SessionManager` (no duplicate gate)
- [x] `payment_success_indicator` fires on successful 402 payment (verified `HttpRequestInterceptor.cpp:2462-2480`)
- [x] Localhost 402 demo server is now the real `bsvblockchain.tech` paid news site — see `OPEN_QUESTIONS.md` and the production verification log in commit notes
- [x] Acceptance: round-trip <2s on Windows (4 articles tested, 3 round-tripped cleanly; 1 hit a server-side Cloudflare 431 — see polish work below). macOS parity untested (see `phase-1-brc121/MACOS_PARITY_ANALYSIS.md`)
- [x] Regression: PeerPay flow still works (smoke 2026-05-08)

### Phase 1 polish (post-acceptance, shipped after initial test exposed gaps)
**Why:** the original scope assumed a clean localhost demo. Real-world testing against `now.bsvblockchain.tech` exposed a flaky Cloudflare 431 that broke the UX, plus a "we pay every reload" UX issue that wasn't in the original spec. All polish items preserve load-bearing safeguards (`payment_success_indicator`, "Always notify" toggle, privacy perimeter prompts, per-session counters).
- [x] **Paid Content Cache** — disk-backed SQLite at `<profile>/paid_content_cache.db`, 500 MB LRU, server `Cache-Control: max-age` honored or NULL=forever. Read hook at top of `SimpleHandler::GetResourceRequestHandler`. Hard-reload (Ctrl+Shift+R) bypasses via `Cache-Control: no-cache` request header. Toggle in Privacy Settings + Clear in Cache & Storage panel.
- [x] **PaymentPendingPage placeholder** — `OnLoadError` swaps CEF's data:text/html "Failed to load" for `/payment-pending` when 402 hits an unapproved domain (modal pops over a clean Hodos background instead of the failed-load page).
- [x] **PaymentFailedPage** — Hodos error page with Try Again button when paid retry exhausts retries with non-2xx. Text: "your sats are safe — no broadcast happened." `Async402` registers the URL via `RegisterBrc121FailedUrl`; `OnLoadError` consumes and routes.
- [x] **Auto-retry on 431/5xx** in `Async402ResourceHandler::onUpstreamComplete`. One retry, 250 ms backoff, reuses the same paid retry context (no new nosend tx). Catches Cloudflare flakiness silently.
- [x] **Reuse-don't-recreate** in Rust `pay_402`. (URL, sats) → in-memory cache of full retry context (txid + BEEF + derivation prefix/suffix + time_ms + sender pubkey + vout). Within ~25 s window AND tx still in `nosend` status, return the cached entry instead of minting a new tx. Drained on `broadcast-nosend` success.
- [x] **WalletStatusCache hardening** in C++. Bumped `/wallet/status` timeout from 1 s to 3 s. Three-state result (`Exists`/`DoesNotExist`/`FetchFailed`) with separate cache TTLs: 30 s for definitive answers, 2 s for transient fetch failures. Fixes the "single timeout poisons BRC-121 for 30 s" symptom seen during testing.
- [x] **Eager-load Hodos error pages** — `PaymentPendingPage` + `PaymentFailedPage` moved out of `React.lazy` so the swap renders without a chunk-fetch flicker on first hit (~3 kB cost on the index bundle).
- [x] **Back-button history fix (partial)** — `TriggerPendingBrc121Reloads` uses `window.location.replace` so `/payment-pending` is replaced in history rather than appended. **Known limitation:** the rest of the BRC-121 reload chain (`pay_402` reload, paid-retry reload, `/payment-failed` swap, Try Again navigation) still appends, so back-from-article still walks through 3-5 intermediate entries before reaching the previous real page. Filed as a Phase 1.5 polish task.

## Phase 1.5 — BRC-100 Surface Completion (NEW)
**Reuse map:** existing `domain_permissions` + `cert_field_permissions` tables (`migrations.rs:468, 486`), shared `notification_browser_` overlay (`simple_app.cpp::CreateNotificationOverlay`, `BRC100AuthOverlayRoot.tsx`), existing `DomainPermissionForm` "Always notify" toggle, `MENU_ID_MANAGE_PERMISSIONS` (`simple_handler.cpp:6696`).
**Phase principles** (per user 2026-05-09): Trust > Convenience > Control on first contact; Convenience > Control on repeat. Hide power-user controls behind disclosures. Never overwhelm non-technical users with security/privacy decisions that need domain expertise. See README.md "Phase principles".

### Step 0 — Cosmetic pre-flight sweep (do first)
**Inventory:** 14 auto-approve UI surfaces — see README.md "Auto-approve UI surfaces" table for the full list.
- [ ] Kickoff review
- [ ] Centralize Hodos theme tokens (extract `#1a1a1a` / `#e0e0e0` / `#a67c00` / `Inter` from inline strings to a shared module — `frontend/src/styles/hodosTheme.ts` or similar)
- [ ] Add `Hodos_Gold_Wallet_Icon.svg` to the header of every notification overlay type (#1–#11 in README inventory) and every wallet-panel form (#12–#13)
- [ ] Add domain favicon to every domain-specific modal (mirror existing `BRC100AuthOverlayRoot.tsx` favicon helper — `https://t0.gstatic.com/faviconV2?...` with Google fallback)
- [ ] Fix `ApprovedSitesTab` modal theme bug (Edit/Delete confirms have wrong colors with unreadable text — see `project_phase15_approved_sites_modal_theme` memory)
- [ ] Style Phase 1 polish pages (`PaymentPendingPage.tsx`, `PaymentFailedPage.tsx`) with the centralized theme tokens
- [ ] Fix payment animation domain-match race in `useTabManager.ts:148-167` — match by `tab.id === browserId` (already in IPC payload at `HttpRequestInterceptor.cpp:2466`) instead of by `tabDomain === domain`. Tab URL is often `/payment-pending` or failed-load data URL when IPC fires → domain match silently fails
- [ ] (Optional) visual tuning of payment badge — only if match-by-browserId fix alone doesn't make it noticeable enough
- [ ] Update `frontend/src/styles/CLAUDE.md` (or equivalent) with theme conventions
- [ ] Smoke: every surface renders correctly on Win + Mac with Hodos icon + favicon visible + readable colors. **Critical:** payment animation badge actually appears on the article tab after every BRC-121 payment to bsvblockchain.tech.

### Step 1+ — Architectural work (was the original Phase 1.5 scope)
- [ ] DB schema walkthrough — table-by-table review of three new child tables + `sensitivity` column (per CLAUDE.md invariant 2)
- [ ] `revealCounterpartyKeyLinkage` + `revealSpecificKeyLinkage` Rust handlers + routes + `crypto/key_linkage.rs`
- [ ] Three new child tables of `domain_permissions` (protocol/basket/counterparty) with `expires_at` + `revoked_at`
- [ ] Optional `sensitivity` column on `cert_field_permissions`
- [ ] Permission engine in C++ (decision logic, manifest fetcher)
- [ ] Five new prompt types added to existing `notification_browser_` overlay (no new HWNDs/NSPanels) — `manifest_connect_bundle`, `identity_key_reveal`, `key_linkage_reveal`, `protocol_permission_prompt`, `counterparty_permission_prompt`
- [ ] Extend `DomainPermissionForm` with "Allow without limits" + Specific permissions + Cert fields sections
- [ ] Extend `ApprovedSitesTab` with sensitivity classifier editor
- [ ] Wire all 28 handlers through new permission gates (additive, bodies unchanged)
- [ ] Manifest fetcher for `/.well-known/wallet-manifest.json`
- [ ] Verify: right-click "Manage Site Permissions" still works
- [ ] Verify: payment animation still fires through new engine
- [ ] Acceptance: BRC-100-conforming demo dApp loads, manifest bundle prompt appears, post-grant calls silent

### Open questions (need user direction during kickoff — see README.md "Open questions for kickoff")
- [ ] Settings → Wallet vs in-tab Default settings — keep both / merge / restore inline
- [ ] Sensitivity classifier UX disclosure — visible / Advanced expander / chrome preference
- [ ] "Allow without limits" friction — current binary (always-notify vs allow-without-limits) or add middle "Trust this site (large limits)" preset
- [ ] Per-session counter visibility — surface in wallet panel / DomainPermissionForm / both / neither
- [ ] Privacy-perimeter prompt warning prominence (identity-key, key-linkage) — extra-prominent treatment yes/no, how much

## Phase 2 — `window.CWI` / `window.yours` / `window.panda` Shim
- [ ] V8 injection in `simple_render_process_handler.cpp::OnContextCreated`
- [ ] V8 Proxy wrapper with `apply` traps on each method
- [ ] Non-writable, non-configurable descriptors for `window.CWI`
- [ ] Writable descriptors for `window.yours`/`window.panda` (Brave isMetaMask lesson)
- [ ] No injection in private/incognito tabs
- [ ] Iframe Permissions Policy gating (`allow="bsv-wallet"`)
- [ ] Secure-context-only check
- [ ] Hide-until-user-gesture mode (stricter than Brave; default-on?)
- [ ] IPC routing for all 28 BRC-100 methods → Rust handlers
- [ ] IPC routing for legacy `window.yours` methods → translation layer → Rust handlers
- [ ] Origin + favicon shown on every signing/spending prompt
- [ ] BRC-100 sub-tier permission flows (per-protocol, grouped, per-counterparty)
- [ ] EIP-6963-equivalent announce protocol (propose `metanet:announceProvider`?)
- [ ] "Default wallet" setting for users with other BSV wallet extensions

## Phase 3 — 1Sat Ordinals
- [ ] UTXO classifier flagging ordinal vs fungible (1-sat with envelope)
- [ ] `Ordinal` row type + repo
- [ ] `OrdinalsIndexerClient` (REST + SSE wrapper for GorillaPool)
- [ ] Basket-aware UTXO selection (don't accidentally spend ordinal UTXOs for fees)
- [ ] Separate ordinal address derivation path (`m/44'/236'/1'/0/0` per Yours convention)
- [ ] `createAction` extension for `basket: '1sat'` (recognize, classify outputs correctly)
- [ ] Inscription envelope construction
- [ ] Ordinal Lock covenant script (lock + unlock paths)
- [ ] Signer accepts arbitrary lock scripts (not just P2PKH)
- [ ] UI for inscriptions
- [ ] Acceptance: 1sat.market purchase round-trip succeeds

## Phase 4 — Demos + LLM Dev Guides
- [ ] `demo-brc100-createaction`
- [ ] `demo-brc121-402`
- [ ] `demo-window-cwi-signin` (BRC-100 + Yours-style + Sigma OAuth + HandCash buttons)
- [ ] `demo-1sat-ordinals` (mint + transfer + buy)
- [ ] LLM-ready `.md` integration guides for each
- [ ] Repo location decided (subdir vs separate repo) — Q3
- [ ] Demo video script that explains:
  - Three sign-in surfaces (BRC-100, Sigma OAuth, HandCash)
  - Auto-approve model + why we differ from Brave (per `AUTO_APPROVE_RATIONALE.md`)
  - Keys-in-Rust-process security advantage
  - Domain whitelist + spend cap UX

## Cross-cutting (every phase)
- [ ] Standard verification basket regression tested after each phase
- [ ] No DB schema changes outside Phase 3 (where ordinal classification requires it)
- [ ] All new C++ uses cross-platform conditionals + `SyncHttpClient`
- [ ] Private keys never leave Rust process (signing all Rust-side)
- [ ] Every auto-approve fire produces visible notification
- [ ] Origin + favicon on every prompt (Phase 2 onward)
