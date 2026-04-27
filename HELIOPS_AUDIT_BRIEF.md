# HeliOps Audit Brief — Hodos Browser

> A scoped introduction to this repo for the HeliOps review team.

## 1. Intro — why we're sending this to you

Hi HeliOps team,

We met at a local tech meetup recently and talked about your code-review / audit product. Thanks for agreeing to look at this repo — this document is meant to orient your auditors to the stack, set clear scope, and flag where off-the-shelf rulesets land well vs. where the BSV / BRC-100 surface has no established rules.

**What Hodos Browser is (in three sentences):** a desktop Web3 browser built on CEF (Chromium Embedded Framework) with a native Rust wallet backend. It implements the BRC-100 protocol suite so users can authenticate to Bitcoin SV–native web apps, send micropayments, and browse normally (with adblock + tracking protection) in one binary. It handles real money, so correctness outranks speed.

**The three layers:**
1. **React + Vite + TypeScript** frontend (UI only — no keys, no signing)
2. **C++17 + CEF 136** browser shell (IPC, overlays, HTTP interception, adblock glue)
3. **Rust + Actix-web + SQLite** wallet backend (crypto, keys, BRC-100 handlers, DB)

**Where to start reading (in order):**
1. `README.md` — product summary + quick start
2. `PROJECT_OVERVIEW.md` — consolidated architecture
3. `SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md` — process-level threat model
4. **This file** — review scope and asks
5. `CLAUDE.md` (root) — detailed code conventions, invariants, and extension points
6. `rust-wallet/src/CLAUDE.md` — endpoint handler groups and key types

What we hope to get out of this: (a) findings we can act on, (b) help calibrating your rulesets against a non-trivial BSV stack that has no well-established public audit rules yet (§8 has more on this).

**⚠️ Honest caveat on our documentation.** The docs in this repo (`README.md`, `PROJECT_OVERVIEW.md`, `THE_WHY.md`, `SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md`, root `CLAUDE.md`, per-directory `CLAUDE.md` files, and the sprint notes under `development-docs/`) have **not** been rigorously updated as the code evolved. Treat them as directional — the code is the source of truth. As a concrete example: different doc files give the monitor task count as "7," "8," and "9" while there are in fact 12 `task_*.rs` files; the root CLAUDE.md links `development-docs/WALLET_SERVICE_FEE_IMPLEMENTATION.md` which no longer exists. Identifying this kind of doc/code drift is explicitly one of the things we'd like reviewed — see §12.

---

## 2. Product overview

**What a user does:** installs Hodos, creates or recovers a wallet (12-word mnemonic, PIN-encrypted at rest), browses the web, and — on BSV-native sites — authenticates via BRC-100, approves micropayments via BRC-29 PeerPay, and uses identity certificates via BRC-52. Non-BSV sites work like a normal Chromium browser with built-in adblock, tracker blocking, and fingerprint farbling.

**Why BSV-native matters:** BRC-100 is a relatively young wallet-interface protocol. There are a handful of implementations and few published security reviews. Getting it wrong leaks keys, lets sites spend funds they shouldn't, or breaks cross-domain privacy guarantees. Our Rust backend is the sole holder of key material — the C++ shell and JS frontend must not be able to extract or forge signatures.

**Distribution:**
- **Windows:** Inno Setup installer, code-signed via Azure Trusted Signing, auto-update via WinSparkle with DSA signature verification
- **macOS:** DMG, code-signed with Apple Developer ID + notarized, auto-update via Sparkle 2 with EdDSA signature verification
- CI pipeline: `.github/workflows/release.yml` handles build, sign, notarize, appcast generation, and GitHub Release publish

---

## 3. Architecture map

```
┌─────────────────────────────────────────────────┐
│ React Frontend (Vite dev :5137, bundled in prod)│
│   window.hodosBrowser.* / window.cefMessage     │
└─────────────────────────────────────────────────┘
                     │ IPC
                     ▼
┌─────────────────────────────────────────────────┐
│ C++ CEF Shell (CEF 136)                          │
│   HTTP interception, overlay subprocesses,       │
│   adblock, fingerprint farbling, history/bookmarks│
└─────────────────────────────────────────────────┘
                     │ HTTP localhost:31301 (wallet) / :31302 (adblock)
                     ▼
┌─────────────────────────────────────────────────┐
│ Rust Wallet Backend + Rust Adblock Engine       │
│   crypto, keys, BRC-100 handlers, SQLite        │
└─────────────────────────────────────────────────┘
                     │
                     ▼
Bitcoin SV chain (WhatsOnChain, GorillaPool/ARC)
+ BRC-2/103 MessageBox (messagebox.babbage.systems)
+ BRC-52 overlay services + Paymail hosts
```

**Process model:** a running Hodos instance has multiple processes — the main CEF browser process, several CEF render-process-per-overlay subprocesses (wallet panel, settings, omnibox, menu, backup, etc.), the Rust wallet server, and the Rust adblock server. Details in `SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md`.

**Overlay model (important for threat modeling):** panels like the Wallet Panel, Settings, Omnibox, Menu, Backup, and BRC-100 Auth each run as *separate CEF subprocesses with isolated V8 contexts*. They communicate with the main process over CEF IPC. See the "Overlay Lifecycle & Close Prevention" section of the root `CLAUDE.md` for the subtle close-prevention logic (race conditions live here).

**Inter-layer boundaries:**
- JS → C++: `cefMessage.send(name, args)` → `CefMessageSendHandler` → `simple_handler.cpp` dispatch
- C++ → Rust: HTTP via `SyncHttpClient` (WinHTTP on Windows, libcurl on macOS) to `localhost:31301`
- Rust → chain: HTTPS to WhatsOnChain, ARC, price oracles, MessageBox, BSV overlay, paymail hosts

---

## 4. Stack & scale inventory

Figures verified against the current tree (April 2026):

| Component              | Tech                              | Scale                              | Purpose |
|---|---|---|---|
| `rust-wallet/`         | Rust, Actix-web, SQLite (rusqlite) | ~85 `.rs` files, 76 HTTP handlers  | crypto, keys, BRC-100 handlers, DB |
| `cef-native/`          | C++17, CEF 136, WinAPI / Cocoa     | 36 `.cpp/.mm` + 33 headers         | browser shell, IPC, overlays, adblock glue |
| `frontend/`            | React 18 + Vite + TS + MUI         | ~85 `.ts/.tsx` files               | UI only, no crypto |
| `adblock-engine/`      | Rust wrapper around `adblock-rust` (0.10.3) | 3 `.rs` files                    | filter-list compile + cosmetic resources, HTTP :31302 |

**Rust wallet — internal module breakdown:**
- `src/crypto/` — 9 code modules: `brc42`, `brc43`, `signing`, `aesgcm_custom`, `dpapi` (Windows DPAPI / macOS Keychain stub), `pin`, `keys`, `brc2`, `ghash` (+ `mod.rs` + `aesgcm_custom_test.rs`)
- `src/database/` — 19 repo modules (`wallet_repo`, `address_repo`, `output_repo`, `certificate_repo`, `proven_tx_repo`, `proven_tx_req_repo`, `domain_permission_repo`, `peerpay_repo`, `user_repo`, `settings_repo`, `commission_repo`, `message_relay_repo`, `sync_state_repo`, `tx_label_repo`, `tag_repo`, `basket_repo`, `block_header_repo`, `parent_transaction_repo`, `transaction_repo`) + `migrations.rs`, `connection.rs`, `models.rs`, `helpers.rs`, `mod.rs`
- `src/monitor/` — **12** background task modules (see §6a)
- `src/transaction/` — tx building + ForkID SIGHASH (BSV-specific)
- `src/certificate/` — BRC-52 certificate parser, verifier, selective disclosure
- `src/script/` — BRC-48 PushDrop encoding + tests
- `src/handlers.rs` — ~13 KLOC, 70 endpoints
- `src/handlers/certificate_handlers.rs` — 6 certificate endpoints
- External-API clients: `authfetch.rs` (BRC-103), `messagebox.rs` (BRC-2 encrypted relay), `paymail.rs`, `identity_resolver.rs`, `overlay.rs`
- Caches: `balance_cache.rs`, `fee_rate_cache.rs`, `price_cache.rs`
- Recovery: `recovery.rs` (BIP32 legacy `m/{index}` + BRC-42 self-derivation scan)

**C++ shell — notable singletons (header files under `cef-native/include/core/`):**
`AdblockCache`, `BookmarkManager`, `CookieBlockManager`, `EphemeralCookieManager`, `CookieManager`, `FingerprintProtection`, `GoogleSuggestService`, `HistoryManager`, `HttpRequestInterceptor`, `PendingAuthRequest`, `ProfileImporter`, `ProfileLock`, `ProfileManager`, `SessionManager`, `SettingsManager`, `SingleInstance`, `SyncHttpClient`, `TabManager`, `WindowManager`, `WalletService`, `AutoUpdater`, `NavigationHandler`, `IdentityHandler`, `AddressHandler`, `BRC100Bridge`, `BRC100Handler`, `BrowserWindow`, `Logger`.

**C++ main entry points:**
- `cef-native/cef_browser_shell.cpp` — Windows entry, overlay WndProcs, globals, close-prevention flags
- `cef-native/cef_browser_shell_mac.mm` — macOS entry, NSWindow/NSPanel overlays, event forwarding
- `cef-native/src/handlers/simple_handler.cpp` — CEF client handler, IPC dispatch, context menus, downloads, find-in-page
- `cef-native/src/handlers/simple_render_process_handler.cpp` — render-process side, V8 injection, scriptlet pre-cache

---

## 5. Documentation map — what already exists

To avoid you re-deriving what's written down, here's where material lives. **Remember the §1 caveat — these may be stale.**

| Topic | File |
|---|---|
| Product summary + quick start | `README.md` |
| Consolidated architecture | `PROJECT_OVERVIEW.md` |
| Design rationale | `THE_WHY.md` |
| Process isolation + threat model | `SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md` |
| Session handoff notes | `HANDOFF.md` |
| Windows build quick-start | `Hodos_Build_Windows.md` |
| Code conventions, invariants, CEF patterns | `CLAUDE.md` (root) |
| Rust wallet layer overview | `rust-wallet/CLAUDE.md` |
| Rust wallet source modules (endpoint groups, types, constants, external APIs) | `rust-wallet/src/CLAUDE.md` |
| Build guides | `build-instructions/BUILD_INSTRUCTIONS.md`, `WINDOWS_BUILD_INSTRUCTIONS.md`, `MACOS_BUILD_INSTRUCTIONS.md` |
| Sprint-level context | `development-docs/Final-MVP-Sprint/` (AI-HANDOFF.md, SECURITY_MINDSET.md, OPTIMIZATION_PRIORITIES.md, TESTING_GUIDE.md, post-beta3-cleanup.md, backup-double-spend-incident-2026-04-11.md) |
| UX / wallet phases | `development-docs/UX_UI/` (phase-0 through phase-5) |
| Roadmap / research | `development-docs/Possible-MVP-Features/`, `development-docs/BSV-Tokens/` |
| Known bugs | `development-docs/MVP_BETA_ISSUES.md`, `RECOVERY_BALANCE_BUG_INVESTIGATION.md`, `NOSEND_TWOPHASE_INVESTIGATION.md` |
| Chain-truth / fee / miner handling | `development-docs/CHAIN_TRUTH_HARDENING_RESEARCH.md`, `MINER_RESPONSE_HANDLER_AND_SELF_HEAL.md` |
| Ecosystem comparison | `development-docs/BSV_RUST_ECOSYSTEM_COMPARISON.md`, `BSV_BROWSER_COMPARISON.md` |
| macOS port tracking | `development-docs/Final-MVP-Sprint/macos-port/` |
| BRC-100 implementation notes | `archived-docs/BRC100_IMPLEMENTATION_GUIDE.md` |
| BRC-42 key-derivation privacy | `archived-docs/BRC42_PRIVACY_EXPLANATION.md` |
| BRC-33 message relay | `archived-docs/BRC33_MESSAGE_RELAY_IMPLEMENTATION_GUIDE.md` |
| BRC-52 certificate findings | `archived-docs/BRC52_SIGNATURE_VERIFICATION_FINDINGS.md` |
| BRC-2 research | `archived-docs/BRC2_RESEARCH_FINDINGS.md` |
| BRC spec index + priority | `archived-docs/BRC_DOCUMENTS_TO_REVIEW.md` |

---

## 6. Review scope — organized by layer

For each layer: **current state** (honest), **what we'd like reviewed**, and known gaps worth your attention.

### 6a. Rust wallet — `rust-wallet/`

**Current state.**
- **Test coverage:** 13 integration test files under `rust-wallet/tests/` (tier3 through tier11, plus diagnostic / interop / sighash / BEEF struct tests). Inline unit tests in a few modules (e.g. `src/crypto/aesgcm_custom_test.rs`, `src/script/pushdrop_tests.rs`, `src/certificate/test_utils.rs`).
- **Tooling:** no `rustfmt.toml`, no `clippy.toml`, no `cargo-audit` or `cargo-deny` configured. No CI job runs tests or lints today.
- **Scale:** ~85 `.rs` files, 76 HTTP handlers on port 31301, 19 DB repositories, 12 background monitor tasks.

**What we'd like reviewed:**
- **Crypto correctness.** `src/crypto/` is security-critical. Specifically:
  - `brc42.rs` — ECDH child-key derivation (BRC-42)
  - `brc43.rs` — invoice-number formatting (BRC-43)
  - `signing.rs` — SHA-256, HMAC-SHA-256, HMAC verification
  - `aesgcm_custom.rs` + `ghash.rs` — AES-GCM implementation (review for nonce reuse, tag forgery)
  - `pin.rs` — PBKDF2 + AES-GCM for PIN-at-rest
  - `dpapi.rs` — Windows DPAPI integration (macOS keychain is currently a stub)
  - `brc2.rs` — symmetric encryption used by MessageBox
- **Signing flows.** `src/transaction/sighash.rs` implements BSV ForkID SIGHASH — this differs from BTC post-2017. Verify the sighash construction, script-code handling, and anyone-can-pay / single / all combinations.
- **Key management.** HD seed handling, PIN-unlock path, DPAPI wrap/unwrap, `recovery.rs` for BIP32 legacy derivation + BRC-42 self-derivation gap-limit scanning.
- **HTTP surface.** 76 endpoints. We especially want:
  - Input validation on every POST body (handlers are ~13 KLOC in one file — easy to miss)
  - Auth state assumptions: when is `check_domain_approved()` required? Are any handlers that need it missing the call?
  - CSRF / cross-origin protection for localhost:31301 (the browser is the only intended client, but any process on the machine can reach it)
  - Domain-permission defense-in-depth (C++ also checks — confirm the Rust-side check is not bypassable by a malformed `X-Requesting-Domain`)
- **Database.** SQLite accessed via `rusqlite` under a `Arc<Mutex<WalletDatabase>>`. Review: lock-drop discipline before `.await`, the `utxo_selection_lock` + `create_action_lock` pair for UTXO race prevention, migration safety (current version **V24**), schema in `src/database/migrations.rs`.
- **Background monitor.** `src/monitor/` has 12 tasks (`task_check_for_proofs`, `task_send_waiting`, `task_fail_abandoned`, `task_unfail`, `task_review_status`, `task_purge`, `task_sync_pending`, `task_check_peerpay`, `task_consolidate_dust`, `task_backup`, `task_verify_double_spend`, `task_replay_overlay`). Review failure modes, retry semantics, ghost-output cleanup rules, and the "safe cleanup order" invariants called out in `rust-wallet/CLAUDE.md`.
- **BRC-103 AuthFetch + BRC-2 MessageBox.** `authfetch.rs` (challenge-response, nonce exchange) and `messagebox.rs` (encrypted relay). Review: signing scope (what's covered by the signature), nonce handling, encryption key scoping, and replay protection.
- **BEEF / BUMP parsing.** `src/beef.rs`, `src/beef_helpers.rs`. Review: ancestry-limit enforcement (`MAX_BEEF_ANCESTORS = 50`), V1/V2/Atomic marker handling, proof validation.
- **Service fee correctness.** See §7 — we want the fee-stripping attack surface reviewed explicitly.

**Ruleset expectations.** Clippy, `cargo-audit`, `cargo-deny`, and `rustsec` get you a lot here — we haven't wired any of them, so their findings will be a large first-pass win. BSV-specific crypto (ForkID SIGHASH, BRC-42, BRC-43, BEEF, BUMP) needs human review or custom rules; there are no off-the-shelf analyzers for these.

### 6b. C++ CEF shell — `cef-native/`

**Current state.**
- **Tests:** none. No gtest/Catch2; no integration-test harness for the shell.
- **Tooling:** no `.clang-format`, no `clang-tidy` config.
- **Scale:** 36 `.cpp/.mm/.h` source files, 33 headers in `include/core/`. Cross-platform split via `#ifdef _WIN32` / `#elif defined(__APPLE__)`.

**What we'd like reviewed:**
- **CEF ref-count / lifetime discipline.** CEF uses `IMPLEMENT_REFCOUNTING(...)` intrusive refcounting; misuse is easy and produces use-after-free bugs. Singletons (`Adblock*`, `HttpRequestInterceptor`, `SessionManager`, etc.) and per-browser state (`BrowserWindow`, `TabManager`) are both prone.
- **IPC message validation.** `simple_handler.cpp::OnProcessMessageReceived` dispatches a large number of named IPC messages from the render process. Every message's payload must be validated as untrusted — the render process is theoretically attacker-controllable via a malicious page + Chromium sandbox escape.
- **Overlay lifecycle & close prevention (Windows).** The root `CLAUDE.md` has an extensive section on this. Five destruction paths for the wallet overlay; async IPC-set flags cannot guard synchronous `WM_ACTIVATEAPP` / `WM_ACTIVATE` events. Race conditions have historically hidden here.
- **HTTP interception.** `HttpRequestInterceptor.cpp` routes requests to the wallet backend, runs domain-permission checks, auto-approve decisions for payment / cert flows. Review trust assumptions on `X-Requesting-Domain`, and the auto-approve session spending caps (`SessionManager`).
- **Response body filter.** `AdblockResponseFilter` (in `AdblockCache.h`) is a `CefResponseFilter` that streams and rewrites YouTube ad-configuration JSON keys. Review buffering correctness, memory bounds, and whether malformed input can cause a hang / OOM.
- **Scriptlet pre-cache + fingerprint injection.** `simple_render_process_handler.cpp` caches scriptlets per-domain (`s_scriptCache`) and injects in `OnContextCreated`. Also injects fingerprint farbling JS (`FingerprintScript.h`) with a per-session, per-domain Mulberry32 seed. Review: seed uniqueness, cache poisoning, injection timing relative to page script.
- **Cross-platform parity.** Windows uses raw WndProcs + `WS_POPUP` overlays; macOS uses `NSPanel` + `NSWindowDelegate`. Review that security-relevant behavior (e.g. "keep overlay open during file dialog") is equivalent on both platforms.
- **`SyncHttpClient`.** Cross-platform HTTP client (WinHTTP / libcurl). Review: TLS verification settings, timeout handling, error surface.

**Ruleset expectations.** `clang-tidy` with CERT and Core Guidelines checks + `cppcheck` will surface a lot. CEF-specific lifetime bugs largely require expert human review; pattern-based custom rules (e.g. "any `Cef*::Create` must return through `CefRefPtr`") could help.

### 6c. React frontend — `frontend/`

**Current state.**
- **Tests:** 6 Playwright E2E specs under `frontend/e2e/tests/` (smoke, wallet-dashboard, wallet-activity, wallet-panel, settings-page, cross-cutting). Runs against `http://localhost:5137` with 30s test timeout.
- **Tooling:** ESLint flat config (`frontend/eslint.config.js`) with `typescript-eslint` recommended + `react-hooks` + `react-refresh`. No Prettier. No unit-test framework (Vitest / Jest).
- **Scale:** ~85 `.ts/.tsx` files. 8 overlay root components in `frontend/src/pages/*OverlayRoot.tsx`.

**What we'd like reviewed:**
- **Invariant #1 holds:** confirm no key material, mnemonic, or signing primitive lives in JS. `window.hodosBrowser.*` should only expose capabilities, never secrets.
- **IPC hygiene.** `frontend/src/bridge/initWindowBridge.ts` defines the JS → C++ bridge. Review payload shapes, trust assumptions on responses, and what happens when C++ sends unexpected IPC data.
- **Overlay state machines.** `WalletPanelPage.tsx` has a non-trivial `preventClose` derived state driving `wallet_prevent_close` / `wallet_allow_close` IPC. This guards against the user accidentally closing the overlay during mnemonic display or PIN entry.
- **CEF-specific input quirks.** Native `<input>` instead of MUI `TextField`; visible-file-input instead of hidden+click-trigger. See root `CLAUDE.md` for the rationale.
- **Dependency posture.** `frontend/package.json` — we haven't wired `npm audit` to CI. Known-vulnerable transitive deps, if any.

**Ruleset expectations.** This layer is well covered by existing tooling — `typescript-eslint`, `eslint-plugin-react`, `eslint-plugin-security`, `eslint-plugin-jsx-a11y`, `npm audit`, Snyk, SonarJS.

### 6d. Adblock engine — `adblock-engine/`

**Current state.** Minimal Rust service (3 files in `src/`) that wraps `adblock-rust` 0.10.3 (pinned). Serves HTTP on `localhost:31302` — `/health`, `/check`, `/status`, `/toggle`, `/cosmetic-resources`, `/cosmetic-hidden-ids`. Auto-updates 4 filter lists (EasyList, EasyPrivacy, uBlock Filters, uBlock Privacy) every 6 hours. 6 bundled extra scriptlets.

**What we'd like reviewed:**
- **Thread safety of `RwLock<Engine>`** during filter-list reload while serving checks
- **Filter-list update integrity** — are the fetched lists verified in any way?
- **Cosmetic-resource API** correctness (CSS + scriptlet injection payloads)
- **Denial-of-service surface** of the HTTP endpoints

### 6e. CI/CD, release, and supply chain

**Current state.** *One* GitHub Actions workflow: `.github/workflows/release.yml`. It handles Windows + macOS builds, code signing, macOS notarization, appcast XML generation, and GitHub Release publish.

**No CI test runs. No linting gate. No `cargo-audit`. No pre-commit hooks. No secret-scanning workflow.** Dependabot is enabled via GitHub's UI (no `.github/dependabot.yml` in the repo — config lives in repo settings) and automated security fixes are on, so there are regular dependency-update PRs but no config checked into the tree.

**What we'd like reviewed:**
- **Secret handling** in `release.yml` — Azure Trusted Signing credentials, Apple notarization credentials, Sparkle/WinSparkle signing keys (DSA for Windows, EdDSA for macOS). Who holds these keys, are they rotatable, is the signing step bypassable via workflow edit on a feature branch?
- **Installer safety.** `installer/hodos-browser.iss` (Inno Setup). Review install paths, uninstall cleanup, elevation prompts, symlink handling.
- **Auto-update trust chain.** `appcast.xml` → signature verification in WinSparkle / Sparkle 2 → binary install. Review the key-pinning posture.
- **Recommended minimum CI gates.** We'd welcome a concrete "here is the minimum CI you should be running" output — `cargo test`, `cargo clippy -D warnings`, `cargo audit`, `npm run lint`, `npm audit`, Playwright smoke suite, `cargo-deny`.
- **SBOM / supply chain.** We don't produce one. Worth recommending if low-effort.

### 6f. Dev environment & data isolation

**Current state.**
- Dev and production use **separate** data directories (`HodosBrowserDev/` vs. `HodosBrowser/`), keyed by the `HODOS_DEV=1` environment variable set by launcher scripts
- Dev binaries detect they are running from `target/release/` or `build/bin/Release/` and **refuse to start** without `HODOS_DEV=1`. This is a runtime safeguard, not compile-time
- Launcher scripts: `dev-wallet.ps1` / `dev-wallet.sh`, `cef-native/win_build_run.sh` / `mac_build_run.sh`, `dev-adblock.ps1` / `dev-adblock.sh`

**What we'd like reviewed:**
- **Safeguard robustness.** Can the dev-path check be bypassed? (e.g. copying the exe out of `target/release/`.) What happens if `HODOS_DEV` is set but the binary is actually installed?
- **DPAPI / Keychain at-rest encryption.** `src/crypto/dpapi.rs` — the macOS side is a stub; confirm what that means for macOS users today.
- **Launcher script correctness** — do they reliably set the right env, run from the right CWD, shut down cleanly?

---

## 7. Wallet service fee & commission model (important disclosure)

Before reading transaction-construction code, know this:

Every outgoing transaction includes a **1000-satoshi service fee output** sent to the Hodos treasury address `1Q1A2rq6trBdptd3t6n53vB79mRN6JHEFT`. This is intentional and is part of our revenue model.

**Where it's added (four tx builders):**
1. `create_action_internal` in `rust-wallet/src/handlers.rs` — standard sends, PeerPay, Paymail
2. `publish_certificate` in `rust-wallet/src/handlers/certificate_handlers.rs` — BRC-52 identity certificate publish
3. `unpublish_certificate_core` in `rust-wallet/src/handlers/certificate_handlers.rs` — certificate unpublish
4. `task_consolidate_dust` in `rust-wallet/src/monitor/task_consolidate_dust.rs` — background dust consolidation

**Constants (both `pub`):** `HODOS_FEE_ADDRESS`, `HODOS_SERVICE_FEE_SATS` in `rust-wallet/src/handlers.rs` (lines 43, 47).

**Output order in every tx:** request outputs → **Hodos service fee** → change.

**Excluded from response.** The `CreateActionResponse.outputs` array returns only the caller's request outputs — not the service fee, not change.

**Commission tracking.** Each service-fee output is recorded in the `commissions` table (see `rust-wallet/src/database/commission_repo.rs`). Commission rows should be cleaned up on every broadcast-failure path.

**Please review specifically:**
- Can a malicious caller strip or redirect the fee output? (Check all tx-build paths, not just the "happy" path.)
- Is the fee output consistently excluded from `CreateActionResponse.outputs` and any list-outputs endpoints?
- Are commission rows cleaned up on **every** failure path (create → sign → broadcast → proof-wait)?
- Are any monitor tasks (especially the ghost-output cleanup in `task_fail_abandoned` / `task_unfail`) correctly accounting for the service-fee output when restoring / failing txs?

---

## 8. Where off-the-shelf rulesets apply vs. don't

This is probably the most interesting section for your product, so we've been more detailed here.

**Well-covered by existing rulesets** (findings from these tools will mostly be first-pass cleanup for us, but still valuable):

| Area | Tools / rulesets |
|---|---|
| Rust general safety + style | Clippy (`-D warnings`), `cargo-audit`, `cargo-deny`, RustSec DB |
| C++ general | `clang-tidy` with CERT-C++ + Core Guidelines, `cppcheck`, MSVC `/analyze` |
| TypeScript / React | `typescript-eslint`, `eslint-plugin-react`, `eslint-plugin-security`, SonarJS |
| JS dependency vulns | `npm audit`, Snyk, OSS Review Toolkit |
| Web security (general) | OWASP ASVS, OWASP Top 10, CWE/SANS Top 25 |
| CEF patterns | CEF project samples + issues (no formal ruleset, but strong community patterns) |
| Release signing / supply chain | SLSA, OpenSSF Scorecard, Sigstore |

**Thin or missing public rulesets — these would benefit from custom rules:**

| Area | Why novel |
|---|---|
| BRC-42 child-key derivation (ECDH + HMAC) | Distinct from BIP32; no standard analyzer |
| BRC-43 invoice-number formatting | Formatting rules, collision implications |
| BRC-52 certificates with selective disclosure | Recent spec, few implementations to cross-check |
| BRC-100 wallet interface | Not yet widely reviewed; implementations diverge |
| BRC-103 / 104 mutual auth | Signing scope and nonce handling vary |
| BRC-2 symmetric encryption over MessageBox | Key scoping + AEAD nonce reuse risk is bespoke |
| BRC-29 PeerPay | Payment-token format, output binding, anti-replay |
| BRC-33 message relay | Relay trust model, ack semantics |
| BSV ForkID SIGHASH | Diverged from BTC in 2017 — BTC-focused tools miss it |
| BEEF / BUMP transaction formats | Newer SPV formats, parser complexity |
| HTTP interception for payment-gated resources | Unique to BRC-100 browsers — no precedent for auditing |

**Canonical references we'd recommend your auditors keep open:**
- BRC spec index: `https://bsv.brc.dev/` (authoritative numbered BRC list)
- Our own BRC research files: `archived-docs/BRC*.md` — your auditors may find these useful as a starting point, with the §1 caveat about doc freshness
- Alternate-implementation cross-references: BSV Desktop SDK (ts-sdk), MetaNet Desktop client

**Honest observation for your product team.** There is no consolidated public ruleset for the BRC family today — implementations are scattered across TS / Go / Rust SDKs. If HeliOps builds even a modest ruleset for BRC-42/43/52/100/29/2/103, that is market-differentiating work. We're genuinely interested in whether that's something your product is positioned to do.

---

## 9. How to run locally

Prereqs and detailed setup:
- Windows: `build-instructions/WINDOWS_BUILD_INSTRUCTIONS.md`
- macOS: `build-instructions/MACOS_BUILD_INSTRUCTIONS.md`
- General: `build-instructions/BUILD_INSTRUCTIONS.md`

**Dev run (three processes, must all be running):**
1. Rust wallet: `.\dev-wallet.ps1` (Windows) or `./dev-wallet.sh` (mac/Linux) → `localhost:31301`
2. Frontend: `cd frontend && npm run dev` → `localhost:5137`
3. CEF shell: `cd cef-native && .\win_build_run.sh` (Windows) or `./mac_build_run.sh` (macOS)

**⚠️ Never run the wallet or adblock Rust servers with bare `cargo run`** — the dev-path safeguard will refuse to start without `HODOS_DEV=1`. The launcher scripts set this automatically.

**Tests:**
- Rust wallet: `cd rust-wallet && cargo test` (13 integration test files)
- Frontend E2E: `cd frontend && npm test` (Playwright; needs a running dev server + backends)
- Whole suite runner (Windows): `.\scripts\test-all.ps1` (`-Verbose`, `-Coverage`, `-Filter` supported)

---

## 10. Non-goals for this review

- UI / UX polish feedback. We have opinions on our own design language and aren't looking for stylistic redesigns.
- Feature requests outside the existing roadmap (see `development-docs/Possible-MVP-Features/`).
- Stack rewrites ("use Electron / Tauri / Node"). The CEF + Rust split is load-bearing for security and isn't up for debate.
- Auditing BSV L1 itself, or the BRC spec process. We're asking about *our implementation* of BRC specs, not the specs themselves.

---

## 11. How to report findings back

We'd like findings filed as **GitHub issues on this repo** — one issue per finding, not a single mega-report. Suggested conventions:

- Label: `audit/heliops`
- Severity tag in the title: `[critical]`, `[high]`, `[medium]`, `[low]`, `[informational]`
- File + line references (e.g. `rust-wallet/src/handlers.rs:4484`)
- Reproduction steps or a pointer to the code path when applicable
- Suggested fix, if you have one — not required, but helpful

We're happy to set up an `audit/` label namespace and a triage board if that helps.

---

## 12. Documentation accuracy review (explicit ask)

Because we've been honest in §1 about doc drift, we'd like documentation accuracy treated as **part of the review**, not a nice-to-have:

- Flag specific claims in `README.md`, `PROJECT_OVERVIEW.md`, `THE_WHY.md`, `SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md`, root `CLAUDE.md`, per-directory `CLAUDE.md` files, and `build-instructions/*.md` that don't match the code
- Flag stale file paths, function names, and counts (the root `CLAUDE.md` claims "9 monitor tasks" — there are 12; `src/CLAUDE.md` claims "76 handlers" — worth verifying; the root doc links `development-docs/WALLET_SERVICE_FEE_IMPLEMENTATION.md` which no longer exists)
- Flag architectural claims the code no longer implements (or never did)
- Flag code paths that exist but aren't documented anywhere
- Flag any security-relevant invariant in docs that the code no longer enforces

Light-touch is fine: a list of "doc says X, code does Y" findings is more useful to us than rewritten docs.

We think this is also a genuinely useful rule class for your product — static doc/code-drift detection is underserved by existing analyzers, and an LLM-backed review pass can do it well.

---

## Contact

Repo owner: **Matthew Archbold** — `matthew.archbold@marstonenterprises.com`

Please reach out with questions, access issues, or scope clarifications before starting a large review pass. We'd rather answer five questions up front than get findings scoped to the wrong assumption.

Thanks again for taking a look.
