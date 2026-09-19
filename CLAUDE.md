# HodosBrowser - Project Context for Claude

> **Last reviewed: 2026-08-03.** This root doc carries **shape, contracts and pointers**. Per-directory `CLAUDE.md` files carry **inventory** — file rosters, counts, exhaustive lists. If you need "which repos / which handlers / which overlays exist", go to the layer doc; if the same fact lives in both places, the layer doc wins and the duplicate here is a bug. Prefer `file.rs :: symbol_name` over `file.rs:1234` — symbols survive edits, line numbers do not.

# Guidelines

Build with a production-focused mindset. Do not take shortcuts. If you get stuck do research on proper implementation plans/debugging steps.

## Working rules — every session, not only sprint phases

Six standing rules. They are short on purpose. Adopted 2026-08-30; provenance and the items we deliberately **declined** are in `development-docs/SCOPING_PROCESS.md` §6–§7.

1. **Don't assume. Don't hide confusion. Ask.**
   State your assumptions explicitly. If a request has more than one reasonable reading, **present the readings and ask — do not pick one silently.** If something is unclear, stop and name exactly what is confusing. This applies to *intent* as much as to code: a request carries both **product** intent (what should be true for the user, and why) and **project** intent (order, dependencies, what is in this release vs the next). ⛔ **If either is ambiguous, ask before building.** Guessing which one was meant is the most expensive kind of wrong, because the work looks finished.

2. ⭐ **Minimum code that solves the problem. Nothing speculative.**
   No features beyond what was asked. No abstraction for single-use code. No configurability nobody requested. If you wrote 200 lines and 50 would do, rewrite it.
   ⚠️ **This is not "take shortcuts", and it does not override the line above it.** It means no *unrequested* scope — not less rigour, fewer tests, or thinner error handling. This is a browser that moves real money: handling a failure path you were asked to handle is the requirement, not gold-plating. When the two pull against each other, **rigour wins and you say so.**

3. **Surgical changes. Every changed line traces to the request.**
   Don't "improve" adjacent code, comments or formatting you did not come for. Don't refactor what isn't broken. Remove imports and helpers **your** change orphaned; **report** unrelated dead code rather than deleting it.
   ⛔ This generalises three warnings already in this file — the gold pill "must survive every refactor", `FingerprintProtection.h` "never delete while tidying", `ManifestFetcher` "do not relax it back". Those are three instances of this one rule.
   ⚠️ It does **not** mean "match existing style even if wrong" — invariant 9's platform-conditional requirements and the CEF input patterns below still win.

4. **Read the docs for an API you have not used here before.**
   A call into CEF, Win32, AppKit or our own fork patches that this repo does not already use elsewhere gets its documentation read — and cited in the phase contract — before the diff is accepted. Do not infer a signature from what looks plausible. `FarblingPolicy.h`'s hand-rolled `RegistrableDomainFromUrl` is the live example of an API that must not be independently re-derived.

5. ⭐ **Look at how it has already been solved, before designing it.**
   These problems are not new to the world, only to us. Before designing anything non-trivial, go and read how the people who hit it first did it — and **say in the design what you found and why we are or are not following it.**

   Organised **by our stack**, so nobody reads nine browsers to answer one question. Full record and the lookup ledger: `development-docs/PRIOR_ART.md`.

   **`rust-wallet/` — BSV protocol & wallet behaviour**

   | Source | Good for |
   |---|---|
   | **BRC documentation** — always first | What the spec *requires* vs what it leaves to the implementer |
   | **BSV Association SDKs + `wallet-toolbox`** | ⭐ **The authoritative answer.** How a conforming wallet actually behaves. **TypeScript and Go** are richest. ⚠️ No Rust implementation — port patterns, never code |
   | **Bitcoin BIPs** | Protocol lineage only — BRC-42/43 descend from BIP32, and BIP text is often better argued than the BRC that followed |
   | **BDK / `rust-bitcoin`** | ⛔ **BTC, not BSV. Narrow use only — see the warning below.** |

   > ⛔ **BTC-library warning.** `rust-bitcoin` and BDK are **Bitcoin Core** libraries. BSV diverged, and the divergences are exactly the kind that produce confident, wrong assumptions:
   >
   > - **no SegWit, no Taproot/Schnorr** on BSV — BDK's whole model is descriptor-based (`wpkh`, `wsh`, `tr`) and none of it applies;
   > - **key derivation is different in kind** — BSV uses BRC-42/43 invoice-number derivation, **not** output descriptors;
   > - **no RBF** (first-seen-rule), different dust and standardness rules, no practical transaction-size cap, restored opcodes;
   > - **different SPV model** — BSV uses merkle proofs / BEEF (BRC-62/74).
   >
   > ⭐ **Use it for exactly one thing:** the *data-model* idea of "a UTXO that exists but is deliberately not selectable" (BDK's unspendable set / coin-selection separation). That is a wallet-architecture pattern, not chain semantics, and it is the closest Rust prior art for the sprint-1 classification seam.
   > **Everything else: assume it does not transfer until proven.** ⛔ **`wallet-toolbox` always outranks it** — where they disagree, BSV-native wins.

   **`cef-native/` — engine, privacy & security**

   | Source | Good for |
   |---|---|
   | **Chromium upstream** | The source of truth. Our patches diverge from it and every engine bump re-litigates them — know what upstream does before deciding to differ |
   | **Brave** | ⭐ **Closest to us in intent, and the origin of our approach.** Brave coined *farbling* and shipped randomisation first (prior academic work: PriVaricator, FPRandom). Firefox and Safari adopted it later |
   | **Tor Browser** | ⭐ **The threat model, and the opposing strategy.** Its design document is the canonical statement of *what fingerprinting defence is defending against*. ⚠️ Note Tor's own answer is **uniformity** (everyone looks identical), which is the **opposite** of farbling's **randomisation** (everyone looks different, every time). Two strategies, same threat — reading both is how you tell which residuals actually matter |
   | **Mullvad Browser** | Tor's hardening *without* the Tor network — the closest analogue to "a privacy browser that is not Tor" |
   | **Firefox / Gecko** | An independent engine reaching different answers. Also the standards-conformance reference |
   | **Safari / WebKit** | ITP, and the most aggressive tracking prevention shipped at scale. Platform reference on macOS |
   | **LibreWolf** | Which Firefox defaults a privacy project actually changes, and why |
   | **ungoogled-chromium** | De-Googling patch sets — directly comparable to our fork's maintenance burden across bumps |

   **`frontend/` — browser UI & interaction**

   | Source | Good for |
   |---|---|
   | **Vivaldi** | Chrome-level UI over a Chromium base — closest to what we do |
   | **Brave / Firefox** | Permission and consent surfaces, which are security UI as much as UI |

   ⛔ **This is "look at", not "copy".** We do not default to their answer; we make sure we know it before choosing ours. A design that reinvents a solved problem without saying why is not finished.
   ⭐ **Where sources disagree with each other, that IS the design question** — report the disagreement rather than silently picking a side. Brave-vs-Tor on randomisation-vs-uniformity is the worked example.
   ⛔ **Port patterns and semantics, never code.** A general rule, not a licence workaround — a pattern understood and re-implemented survives our next engine bump; a copied block does not.
   ⚠️ **Licence discipline:** Brave / Firefox / LibreWolf / Tor Browser are MPL-2.0 (per-file copyleft); **Vivaldi's UI layer is source-available, not open source**; Chromium is BSD-3; WebKit is LGPL/BSD. Confirm before anything beyond reading — the standing rule from the `go-private-backup-cache` review applies to all of these.
   ⭐ **Log what you looked at** in `development-docs/PRIOR_ART.md` — one row, one minute. It is how we learn which sources are actually worth the trip.

6. ⛔ **The instrument is not edited by the change it measures.**
   `scripts/preflight.ps1` gate patterns, gate baselines, and `REGRESSION_SET.md` are **not** touched in the same change that implements the code they measure. Loosening a pattern or raising a baseline is its own commit, with the reason written in `HARNESS.md` §4, and it re-runs `-NegativeControl`. This project has already shipped a preflight that reported PASS while running zero checks.

7. 🚨 **A POISONING defect outranks the sprint. Verify it cheaply, then stop and say so.**
   Adopted 2026-09-17. 👤 *"If we have something that's totally breaking a wallet, we can't continue — not everything in testing is going to be off, everything is broken and we can't trust anything after that."*

   **What makes a defect *poisoning* rather than merely bad:** it corrupts state that **later work reads**, so every measurement taken after it is suspect. A bug that produces a wrong answer is ordinary. A bug that makes *the next ten answers unverifiable* is not.

   ⭐ **Four trip-wires. Any one of them, treat as poisoning until disproved:**

   | # | Trip-wire |
   |---|---|
   | 1 | **A money-path row in a state no code expects** — `outputs` / `transactions` with a contradictory combination (`spendable=0` + `spent_by=NULL`, a `nosend` row with a spendable output, a balance that disagrees with the chain) |
   | 2 | **A check that returns a VERDICT where it should return an ERROR** — `.unwrap_or(false)`, a swallowed exception, a default that is indistinguishable from a real answer. The `new_status` reuse query answered *"not reusable"* for months because of exactly this |
   | 3 | **Durable state that outlives the process** — a DB row, cache entry or file that the next session, the next test and the next developer will read as fact |
   | 4 | **An instrument that cannot fail** — a gate, probe or assertion that would pass with the feature absent. Rule 6 is the special case; this is the general one |

   ⛔ **Do the cheap ground-truth check BEFORE escalating. Escalate on evidence, not suspicion.**
   For wallet state the authority is **the chain, and it is one HTTP call away**: a wallet row is a *claim about* the chain, never the chain itself. 🚨 On 2026-09-17 a "1.4M sats stuck, balance understated" alarm cost a full investigation and turned out to be a **spent** coin and our own test residue — one `curl` would have settled it before the owner was ever told. ⇒ **Ask the chain, then escalate.**

   **If it survives that check:**
   - ⛔ **Stop the sprint task.** Do not "finish this row first" — later rows read the poisoned state.
   - 👤 **Tell the owner plainly**, with the trip-wire, the evidence, and what is now untrustworthy.
   - ⛔ **Re-run anything already measured against the affected state.** A green from before the discovery proves nothing; say so rather than quietly keeping it.

   ⚠️ **A deliberate fault-injection test must DECLARE its residue.** A negative control that breaks bookkeeping on purpose leaves rows that look exactly like trip-wire 1. Say what it leaves behind, in the row that owns it, or the next person to find it will — correctly — treat it as a live defect. `PAYMENT_TEST_BATCH.md` M4 is the worked example.

## Scoping a sprint or release — `development-docs/SCOPING_PROCESS.md`

**Before a sprint exists, `SCOPING_PROCESS.md` is the procedure.** Four stages — **Scope → Telescope → Microscope → Telescope (close)** — with defined outputs, exit conditions, a loop limit, and explicit human decision points. The `sprint-scoper` agent (`.claude/agents/sprint-scoper.md`) owns it.

⛔ **It does not apply to small work.** A single ticket, a two-file change, or a sprint with no cross-sprint edges does not get a four-stage run — that is the over-planning it exists to prevent. Skip to the phase kickoff below.

Two rules from it that apply to **every** session, not only scoping ones:

- ⛔ **When two options are close, put both in one message with a recommendation and ask.** Do not iterate silently, and do not fan out agents to make a coin-flip decision.
- ⛔ **Stop at the exit condition.** Deliverables exist and open questions are answered ⇒ stop. Development sometimes just requires building, testing and adapting.

It sits **before** the phase kickoff below, and **beneath** `development-docs/<sprint>/HARNESS.md` — the scoping process never lowers a harness standard.

## Phase kickoff workflow (mandatory before any sprint-phase implementation)

Sprint phases live in `development-docs/<sprint>/phase-*/` folders. Before writing **any code** for a phase, run a brief kickoff review:

1. **Re-read the phase docs.** The phase's `README.md` plus every doc it links. Don't trust prior session memory or earlier summaries — code and line numbers may have moved.
2. **Verify cited code is current.** For every file:line reference in the phase doc, grep/Read the cited code and confirm it still exists at that location with the documented shape. Update the doc inline if anything moved.
3. **Reuse-first audit.** Map every change to existing functions/code/components. **Before writing anything new, prove the equivalent doesn't already exist** — grep for similar handlers, repos, components, IPC types. If something close exists, extend it rather than creating a parallel structure. Common reuse anchors:
   - Rust handlers in `rust-wallet/src/handlers.rs` — most BRC-100 + payment primitives are already there. Full roster: `rust-wallet/src/CLAUDE.md`
   - C++ overlays — the shared `notification_browser_` overlay multiplexes prompt types via `BRC100AuthOverlayRoot.tsx`'s type dispatch; add new types as new cases, don't add new HWNDs without strong reason
   - DB tables — extend via child tables joined by FK + CASCADE, mirroring the `cert_field_permissions` pattern (don't create parallel top-level tables)
   - Permission gates — the decision engine is Rust (`rust-wallet/crates/hodos_permission_engine`, driven by `rust-wallet/src/permission_service/`). `check_domain_approved` + the `domain_permissions` row already enforce per-tx / per-session / rate / max-tx-per-session limits, and per-session counters live in `PermissionService.session_counters` (`rust-wallet/src/permission_service/state.rs`), cleared per browser via `POST /wallet/session/close`. The C++ `PermissionEngine` and `SessionManager` were deleted in Phase 2.6-H — do not resurrect them
   - HTTP interception — `isWalletEndpoint` route table is the entry point for all new wallet endpoints; new endpoints go through the table, never around it
4. **Risk assessment.** What existing functionality could this change touch or break? Especially audit the **load-bearing UX safeguards**:
   - **Tab payment badge — the GOLD PILL indicator** (`payment_success_indicator` IPC chain: `HttpRequestInterceptor.cpp :: OnWalletCallSuccess` is the single emit site, reached from two logical paths — createAction silent-approve and `firePaymentSuccessIpc()` on the BRC-121 paid retry → relayed by `simple_render_process_handler.cpp` → consumed by `useTabManager.ts`) — fires on every auto-approved payment; the user's primary visual safeguard against silent payment abuse. It is a **gold pill**, never a "green dot"; it must survive every refactor
   - **Right-click "Manage Site Permissions"** (`MENU_ID_MANAGE_PERMISSIONS` in `simple_handler.cpp`) — quick revoke flow
   - **`DomainPermissionForm` "Always notify" toggle** — zeros the three spending limits (per-tx, per-session, max-tx-per-session), forcing a prompt on every payment; `rate_limit_per_min` is deliberately left alone (and floored to 1 by `parseInt(rateLimitPerMin) || 1`). The cautious-user opt-in path
   - **Privacy perimeter gates** — identity-key reveal, key-linkage reveal, sensitive cert fields, over-cap spends. Audit these four for regressions. Only **sensitive certificate fields** prompt unconditionally with no opt-out (`matrix_c.rs`); identity-key reveal goes silent when `domain_permissions.identity_key_disclosure_allowed=1` (user-facing toggle, global default ON) or on a session opt-in, key-linkage reveal goes silent on a session opt-in, and a spend is "large" only when it exceeds the user-configured `per_tx_limit_cents` (default 100 cents = $1.00)
   - **Per-session counter behavior** (resets on tab close — kept by design)
5. **Confirm the test plan** is actionable for this phase. Each phase needs unit + integration + smoke tests before merge, with explicit Windows/macOS parity verification per the Testing Standards table below.
6. **Hand back a tight summary** to the user listing remaining open questions / assumptions / decisions before any code is written. Wait for confirmation before writing the first commit.

The kickoff is meant to be ~15–30 minutes of work, not a re-plan. Its job is catching divergence between the plan and current reality before commits start landing.

## Testing Standards

**Every feature must be tested against real-world sites.** Standard verification sites are listed below.

| Level | When | Duration | Sites |
|-------|------|----------|-------|
| **Minimal** | After any browser-core change | 5 min | youtube.com, x.com, github.com |
| **Standard** | After sprint completion | 15 min | Auth category + 2-3 video/media + 1-2 news |
| **Thorough** | Before release/demo | 30-45 min | Full basket, all categories |

### ⛔ NEGATIVE CONTROL — mandatory for every acceptance test

**Every acceptance test must be demonstrated to FAIL when the feature it covers is disabled.** A test that has never been seen to fail has not been shown to test anything.

Do it explicitly: turn the feature off (config toggle, stub the call, revert the patch), re-run the test, and confirm it goes red *for the right reason*. Only then does a green run mean something. State the negative control alongside the green result when reporting — "passes, and fails when X is disabled".

**Why this is a hard rule and not advice.** The 0.4.0 farbling work produced **three** separate harnesses that would each have passed with the feature completely absent, and they cost days between them:

1. a CDP harness created tabs via `PUT /json/new`, which bypass `OnBeforeBrowse` — so the feature could never engage, and the test failed against **correct** code;
2. another asserted only that canvas methods report `[native code]`, which is true the instant the old JS is deleted, whether or not any replacement works;
3. a third drove the wrong **browser** — Hodos's header and ~14 overlays are separate CEF browsers that CDP all reports as `type:"page"` — which faked an "intermittent per-session bug" in code that was fine.

Each one looked rigorous. A negative control would have caught all three in minutes. Related: the shipped fingerprint bug survived in **every release since the feature was written**, because every check anyone ran was a same-session check that the bug passed — see `development-docs/TICKET_farbling_constant_seed_shipped.md`.

Corollary for privacy/security features: also assert the test is measuring the intended **subject** (right process, right browser, right document), not just the intended value.

**Farbling has a standing release gate** built on exactly this rule: `development-docs/DevOps-CICD/FARBLING_RELEASE_GATE.md`. `promote.yml` blocks without a seed-rotation result, and the harness (`development-docs/0.4.0/chromium-rebuild/farbling_seed_rotation_check.py`) ships its own `--negative-control` mode that inverts the exit code. It lives on the build host rather than in CI because the `cef-binaries` asset CI pulls has no farbling patches — a hosted job would go red against correct code, which is defect #4 of the family above.

**Cross-DPI (Windows):** Before any release/demo, and after any header/toolbar/overlay/layout change, run the DPI & resolution matrix — minimum cells #4/#6/#9 (125%/1366, 150%/1366, mixed-DPI). This catches "works on my machine" scaling bugs (e.g. clipped toolbar buttons on a 1366×768 / 150% laptop). See `development-docs/DevOps-CICD/DPI_RESOLUTION_TEST_MATRIX.md`.

**Categories:** Authentication (x.com, google.com, github.com), Video/Media (youtube.com, twitch.tv), News/Content (nytimes.com, reddit.com), E-commerce (amazon.com), Productivity (docs.google.com), BSV (whatsonchain.com)
## Overview

A Web3 browser built on CEF (Chromium Embedded Framework) with a native Rust wallet backend. Implements BRC-100 protocol suite for Bitcoin SV authentication and micropayments. This is production software handling real money; security and correctness take priority over development speed.

---

## Architecture

Three layers with strict separation:

```
React Frontend (Port 5137)
    │ window.hodosBrowser.*  (wallet calls ride the "wallet_call" CefProcessMessage
    │                         IPC bridge, NOT a direct fetch from the page)
    ▼
C++ CEF Shell
    │ HTTP interception & forwarding → 127.0.0.1:31301 for wallet functions
    ▼
Rust Wallet Backend (127.0.0.1:31301)
    │
    ▼
Bitcoin SV Blockchain (WhatsOnChain, GorillaPool)
```

> **Ports are environment-dependent — never hardcode them.** Wallet backend: **31301** release / **31401** under `HODOS_DEV=1`. Adblock engine: **31302** release / **31402** dev. Single source of truth is `cef-native/include/core/PortConfig.h` (`hodos::WalletPort()`, `hodos::WalletBaseUrl()`, `hodos::WalletUrl()`) mirrored by `wallet_port()` in `rust-wallet/src/main.rs`. Route every new call site through those helpers.

| Layer | Tech | Responsibility |
|-------|------|----------------|
| Frontend | React, Vite, TypeScript, MUI | UI, user interactions; never handles keys or signing |
| CEF Shell | C++17, CEF (exact pin: `CEF_VERSION` in `cef-binaries/include/cef_version.h` — read it, don't quote it from memory) | Browser engine, V8 injection, HTTP interception; browser data (history, bookmarks) |
| Wallet | Rust, Actix-web, SQLite | Crypto, signing, keys, BRC-100 protocol. Signing keys never leave this process — see Invariants #1 for the exact guarantee and its one deliberate exception |

**Overlay Model**: Settings, Wallet Panel, and BRC-100 Auth each run as separate CEF subprocesses with isolated V8 contexts. (The standalone Backup Modal overlay was deleted in beta.3 Phase 8c O8 — the recovery-phrase prompt is part of the wallet panel's create flow.)

> **⚠️ "CEF-based" ≠ "limited to prebuilt CEF."** We **build our own custom Chromium+CEF from source** (see `development-docs/DevOps-CICD/CEF_BUILD_RUNBOOK.md`). The CEF source-patch mechanism (`cef/patch/patch.cfg`, applied by `patcher.py` during `automate-git.py`) **is stood up and carrying real patches** (P3, 2026-08-05; `PLAN_patch_toolchain.md`). Farbling is now a **Blink patch set** in our CEF fork `Hodos-Browser/cef`, branch `hodos/7871` — C1 Supplement, C3 canvas, C4 WebGL `readPixels`, C5 WebAudio, C6 navigator, all gated on the single `HODOS_FARBLING` condition and registered in the fork's `HODOS_PATCHES.md`. The injected-JavaScript implementation it replaced (`FingerprintScript.h`) was **deleted 2026-08-09**. CEF is our *embedding API*, but the underlying Chromium is **ours to patch** — so capability is bounded by **patch scale + per-Chromium-bump maintenance, NOT by CEF's stock behavior.** When weighing a feature, don't reason "CEF won't let us"; reason "how large is the patch and how much does it churn each Chromium bump." (We remain a CEF *embedder*, not a full fork like Vivaldi — the more we patch the browser-UI layer, the closer we move to fork-level upkeep.)

---

## ⚠️ CRITICAL: UI Architecture Rules

**NEVER add new panels/menus/dropdowns directly to MainBrowserView.tsx (header_hwnd).**

All UI panels MUST be implemented as **overlays** in their own CEF subprocess. Every panel — wallet, settings, cookies, downloads, privacy shield, omnibox, menu, profile picker, bookmarks, site info, tab list — ships as a `<Name>OverlayRoot.tsx` page routed in `frontend/src/App.tsx`. **Roster: `frontend/src/pages/CLAUDE.md`.**

**Why overlays?**
- Each overlay is isolated V8 context (security)
- Doesn't block main browser thread
- Can be positioned relative to toolbar icons
- Consistent UX pattern across all panels

**Pattern for new panels:**
1. Create `<Name>OverlayRoot.tsx` in `frontend/src/pages/`
2. Add route in `frontend/src/App.tsx`
3. Add C++ handler to show/hide overlay in `simple_handler.cpp`
4. Trigger via `window.cefMessage.send('<name>_panel_show', [offset])`

**MainBrowserView.tsx should ONLY contain:**
- Tab bar
- Navigation buttons (back/forward/refresh)
- Address bar input
- Toolbar icon buttons (that TRIGGER overlays)
- Find bar (inline exception)

---

## ⚠️ Overlay Lifecycle & Close Prevention (IMPORTANT — Windows)

> **macOS note:** On macOS, overlays are **borderless `NSWindow`s** (`NSWindowStyleMaskBorderless`; `GenericOverlayWindow : NSWindow` in `cef-native/OverlayHelpers_mac.h`), not `NSPanel`s and not `WS_POPUP`. Click-outside dismissal is handled by paired NSEvent local+global monitors installed via `InstallClickOutsideMonitor()` (`cef-native/OverlayHelpers_mac.mm`), which consult `g_wallet_overlay_prevent_close` for the wallet overlay. `MainWindowDelegate::windowDidResignKey` only logs; there is no `resignMain` handler. The patterns below are Windows-specific.

Overlays are WS_POPUP windows (not children of `g_hwnd`). Each overlay has a different close/destroy pattern. Understanding these is critical for UX work.

### Overlay Close Mechanisms

| Mechanism | Where | Overlays Affected |
|-----------|-------|-------------------|
| **Click-outside (Mouse hook)** | `WH_MOUSE_LL` hook in C++ | Dropdown-style overlays (cookie, download, menu, profile, omnibox, …) — roster in `cef-native/src/handlers/CLAUDE.md` |
| **HWND activation loss (`WM_ACTIVATE`)** | overlay's own WndProc, e.g. `WalletOverlayWndProc` | Wallet — **the primary wallet close path**. Guarded by `g_wallet_overlay_prevent_close` |
| **IPC `overlay_close`** | React → `simple_handler.cpp` | Only the four full-panel roles: settings, wallet, brc100auth, notification (backup was deleted in 8c O8). Wallet and notification **hide** (keep-alive) rather than destroy. Dropdown panels are NOT handled here — each has its own hide IPC (`bookmarks_panel_hide`, `cookie_panel_hide`, `download_panel_hide`, `profile_panel_hide`, `siteinfo_panel_hide`, …). **A new overlay that sends `overlay_close` without a role arm silently no-ops.** |
| **Focus loss (`WM_ACTIVATEAPP`)** | `cef_browser_shell.cpp` main WndProc (primary window only) | Hides the **wallet**, the **omnibox** and the **site-info hub**. The wallet is *not* exempt — it is spared only while `g_wallet_overlay_prevent_close` is set; `g_file_dialog_active` spares all overlays by breaking out early |
| **Old overlay cleanup** | Only `CreateSettingsOverlayWithSeparateProcess()` (destroy + recreate) and `CreateSettingsMenuOverlay()` (destroy + return = toggle-close). `CreateNotificationOverlay()` is keep-alive, reusing the HWND and injecting `window.showNotification()`, destroying only a stale HWND. | **Every other overlay — wallet included — takes the keep-alive early return** (`if (hwnd && IsWindow(hwnd)) { Show*Overlay(...); return; }`) and reuses the existing browser |

### Close Prevention Patterns

**1. `g_file_dialog_active` (C++ synchronous guard)**
- Set to `true` in `OnFileDialog()` (C++ side, synchronous — before dialog opens)
- Cleared on `WM_ACTIVATEAPP(TRUE)` (app regains focus)
- Guards ALL overlays during native file dialog
- **Works because it's set synchronously in C++ before focus loss can fire**

**2. `g_wallet_overlay_prevent_close` (React → C++ IPC flag)**
- Set via `wallet_prevent_close` / `wallet_allow_close` IPC messages from React
- Consulted by the C++ close paths: `WalletOverlayWndProc`'s `WM_ACTIVATE(WA_INACTIVE)`, the main WndProc's `WM_ACTIVATEAPP`, and the `WH_MOUSE_LL` click-outside hook (and, on macOS, `InstallClickOutsideMonitor`)
- **⚠️ A React-set flag cannot reliably guard `WM_ACTIVATEAPP`** — IPC is async, and the flag may not have arrived before focus loss fires. That is why creation-time default (#3) exists
- Auto-cleared on `overlay_close` IPC

**3. Wallet creation-time default (flag set in C++, cleared by React)**
- `g_wallet_overlay_prevent_close` is set to `true` inside the Windows creator `CreateWalletOverlay(HINSTANCE, bool showImmediately, int iconRightOffset)` in `simple_app.cpp` (synchronous, no race). *(`CreateWalletOverlayWithSeparateProcess()` is the **macOS-only** entry point, in `cef_browser_shell_mac.mm`.)*
- React sends `wallet_allow_close` IPC once user reaches a safe state (live wallet, loading, locked)
- React sends `wallet_prevent_close` IPC when entering unsafe state (mnemonic display, PIN entry)
- Result: new overlay survives focus loss by default; React opts in to allow close once ready

### Key Rule: Synchronous vs Async Guards

> **If you need to prevent overlay close during a C++ event (like `WM_ACTIVATEAPP`), the guard flag MUST be set synchronously from C++.** React → IPC → C++ flags have a race condition because `WM_ACTIVATEAPP` fires immediately when the user clicks another window, before async IPC messages arrive.

**Safe pattern:** Set flag in `CreateXxxOverlay()` or in `OnFileDialog()` (C++ side)
**Unsafe pattern:** Set flag via React `useEffect` → `cefMessage.send()` → IPC handler (async, race condition)

### Destruction / hide paths for the wallet overlay

1. **`WM_ACTIVATE(WA_INACTIVE)` in `WalletOverlayWndProc`** — THE PRIMARY close path. Fires when the wallet HWND loses activation (click outside, Alt+Tab, click another app). Guarded by `g_wallet_overlay_prevent_close`. This is the WndProc for the overlay HWND itself, in `cef_browser_shell.cpp`.
2. **`WM_ACTIVATEAPP` in the main WndProc** — app-level focus loss, primary window only. Also guarded by `g_wallet_overlay_prevent_close` (and by `g_file_dialog_active`), in `cef_browser_shell.cpp`.
3. **IPC `overlay_close`** from React → `simple_handler.cpp` (wallet arm hides, keep-alive)
4. **Application shutdown** → `ShutdownApplication()` cleanup

> Note: the wallet creator does **not** destroy-and-recreate. `CreateWalletOverlay()` takes the keep-alive early return when a live HWND already exists.

> **Key lesson:** Overlays have BOTH app-level (`WM_ACTIVATEAPP`) AND HWND-level (`WM_ACTIVATE`) close paths. Both must be guarded. The HWND-level `WM_ACTIVATE` in the overlay's own WndProc is typically the one that actually fires first.

### Code Locations

> Symbols, not line numbers — line numbers rot, symbols survive edits.

| What | Where |
|------|-------|
| Overlay globals & flags | `cef_browser_shell.cpp` — top-of-file globals section |
| `WM_ACTIVATE` (**primary wallet close path**) | `cef_browser_shell.cpp :: WalletOverlayWndProc` |
| `WM_ACTIVATEAPP` handler (app-level focus loss) | `cef_browser_shell.cpp` — main `WndProc` |
| `overlay_close` IPC | `simple_handler.cpp` — IPC dispatch, five role arms |
| Wallet overlay creation + prevent-close flag init (Windows) | `simple_app.cpp :: CreateWalletOverlay` |
| Wallet overlay creation (macOS) | `cef_browser_shell_mac.mm :: CreateWalletOverlayWithSeparateProcess` |
| Prevent-close IPC handlers | `simple_handler.cpp` — `wallet_prevent_close` / `wallet_allow_close` |
| React preventClose logic | `WalletPanelPage.tsx` — `preventClose` derived state + `useEffect` |

---

## ⚠️ CEF Input Patterns (IMPORTANT)

CEF overlays have quirks with form inputs. Follow these patterns:

### Text Inputs
- **Use native `<input>` elements**, not MUI `TextField`
- MUI's extra layers break CEF focus handling
- Add delayed focus with `useEffect` + `setTimeout(50ms)`

```tsx
// ✅ Works in CEF
<input
  type="text"
  style={{ width: '100%', padding: '8px', border: '1px solid #ccc', borderRadius: '4px' }}
  onFocus={(e) => e.target.style.borderColor = '#1a73e8'}
  onBlur={(e) => e.target.style.borderColor = '#ccc'}
/>

// ❌ Broken in CEF overlays
<TextField variant="outlined" />
```

### File Inputs
- **Use VISIBLE file inputs**, not hidden ones triggered by click
- CEF handles visible `<input type="file">` correctly
- Hidden file inputs triggered via `.click()` often fail

```tsx
// ✅ Works in CEF (visible input)
<div style={{ background: '#f5f5f5', padding: '8px', borderRadius: '4px' }}>
  <input type="file" accept="image/*" onChange={handleFile} style={{ width: '100%' }} />
</div>

// ❌ Unreliable in CEF (hidden + click trigger)
<input type="file" style={{ display: 'none' }} ref={ref} />
<button onClick={() => ref.current?.click()}>Choose File</button>
```

### Reference Implementation
- `WalletPanelPage.tsx` — working file input for wallet recovery
- `WalletPanelPage.tsx` (recovery grid, PIN input) and `ProfilePickerOverlayRoot.tsx` — working native text inputs

### Focus & Keyboard Handling (C++ side)
CEF windowless overlays need explicit focus AND keyboard event forwarding:

**1. HWND Creation:**
```cpp
// Use WS_VISIBLE flag for proper focus
WS_POPUP | WS_VISIBLE,  // NOT just WS_POPUP
```

**2. Browser Settings:**
```cpp
settings.javascript_access_clipboard = STATE_ENABLED;
settings.javascript_dom_paste = STATE_ENABLED;
```

**3. WndProc (CRITICAL):**
```cpp
case WM_MOUSEACTIVATE:
    return MA_ACTIVATE;  // NOT MA_NOACTIVATE!

case WM_LBUTTONDOWN:
    SetFocus(hwnd);  // Windows focus
    browser->GetHost()->SetFocus(true);  // CEF focus
    browser->GetHost()->SendMouseClickEvent(...);
    return 0;

case WM_KEYDOWN:
case WM_KEYUP:
case WM_CHAR:
    // Forward ALL keyboard events to CEF browser
    browser->GetHost()->SendKeyEvent(key_event);
    return 0;
```

**4. OnAfterCreated:**
```cpp
browser->GetHost()->SetFocus(true);
```

**Reference:** `WalletOverlayWndProc` in `cef_browser_shell.cpp` — working keyboard input

---

## Dev Runbook

### ⚠️ CRITICAL: Dev/Production Data Isolation

Dev builds and the installed app use **separate data directories** to prevent database corruption:

| Context | Data Directory | How |
|---------|---------------|-----|
| **Dev builds** | `HodosBrowserDev/` | `HODOS_DEV=1` env var set by launcher scripts |
| **Installed app** (users) | `HodosBrowser/` | No env var — default path |

**Safeguard:** Dev binaries detect they are running from a build directory (`target/release/`, `build/bin/Release/`). If `HODOS_DEV=1` is not set, they **refuse to start** with a clear error. This prevents accidentally hitting the production database.

**NEVER run dev servers with bare `cargo run` or by launching the exe directly.** Always use the launcher scripts below.

### Run order (all three must be running):

1. **Rust wallet** (PowerShell): `.\dev-wallet.ps1` → **127.0.0.1:31401** (the launcher sets `HODOS_DEV=1`; 31301 is the release port)
   - Mac/Linux: `./dev-wallet.sh`
2. **Frontend dev server**: `cd frontend && npm run dev` → localhost:5137
3. **CEF browser**:
   - Windows: `cd cef-native && .\win_build_run.sh` (builds + launches with `HODOS_DEV=1`)
   - macOS: `cd cef-native && ./mac_build_run.sh`

### Build only (no launch)

Building does NOT require `HODOS_DEV`. The safeguard is at **runtime**, not build time.

- Rust: `cd rust-wallet && cargo build --release`
- Adblock: `cd adblock-engine && cargo build --release`
- Frontend: `cd frontend && npm run build`
- C++ (Developer Command Prompt): same cmake commands as before — see Build section below

### What Claude must do

When asked to run/test the wallet, adblock, or CEF browser during development:
- **Rust wallet:** Use `HODOS_DEV=1 cargo run --release` (or the launcher script)
- **Adblock engine:** Use `HODOS_DEV=1 cargo run --release` (or the launcher script)
- **CEF exe:** Ensure `HODOS_DEV=1` is in the environment before launching
- **NEVER run `cargo run` without `HODOS_DEV=1`** — the safeguard will block it anyway, but don't even try

#### ⛔ Stopping a dev process: match by EXE PATH, never by image name

**All three dev processes share their image name with the user's installed build** —
`HodosBrowser.exe`, `hodos-wallet.exe`, `hodos-adblock.exe`. A name-matched kill takes down the
**user's production browser and wallet** along with the dev ones.

⭐ **Use the script. Do not hand-write the kill:**

```powershell
.\scripts\stop-dev.ps1            # stops dev browser + wallet + adblock, path-matched
.\scripts\stop-dev.ps1 -WhatIf    # show what would be stopped, kill nothing
```

If you must write it inline, the `Where-Object` clause is **not optional**:

```powershell
# ⛔ WRONG — also kills the user's installed wallet
Get-CimInstance Win32_Process -Filter "Name='hodos-wallet.exe'" | Stop-Process -Force

# ✅ RIGHT — dev only
Get-CimInstance Win32_Process -Filter "Name='hodos-wallet.exe'" |
  Where-Object { $_.ExecutablePath -like '*rust-wallet	arget
elease*' } | Stop-Process -Force
```

> 🚨 **This happened, 2026-09-01.** A name-matched `Stop-Process` on `hodos-wallet.exe` — run to free
> a file lock before `cargo build` — killed the owner's **production** wallet backend. The installed
> browser stayed up and reported *"no wallet"* to every dApp for ~4 hours, and the owner reasonably
> concluded first that a site was broken, then that the browser was. ⚠️ The exe-path rule **already
> existed** in the beta.3 session prompt, but only for `HodosBrowser.exe`; it was honoured there and
> not carried across. That is why it now lives here, covering all three.
> ⭐ The *product* half — the browser never noticing its backend died — is
> `development-docs/0.4.0-beta.3/TICKET_wallet_backend_death_is_silent_and_unrecovered.md`.

⚠️ **You need to stop the dev processes before building:** the linker fails `LNK1104` while the dev
browser runs, and `cargo build` fails *"Access is denied"* while the dev wallet runs. That pressure is
exactly what produces a hasty kill — which is why the script exists.

**Storage (dev)**: Windows: `%APPDATA%/HodosBrowserDev/`, macOS: `~/Library/Application Support/HodosBrowserDev/`
**Storage (production)**: Windows: `%APPDATA%/HodosBrowser/`, macOS: `~/Library/Application Support/HodosBrowser/`

---

## Branch & Remote Workflow

> Canonical detail in `development-docs/DevOps-CICD/README.md`. The short version every session must know:

- **`origin` = development** (BSVArchie fork). **ALL code changes land here first.** Flow: feature branch → `origin/staging` → `origin/main`. `staging` = integration + where internal test builds are fetched from; `main` = blessed release-candidate.
- **`release` = the signed-build remote** (Hodos-Browser org; holds the GitHub signing keys). When ready for a **public** build, push `main` → `release` and run `BUILD_AND_RELEASE` there. `release` may be **ahead of** `origin` (e.g., release-specific auto-update commits) — that's tolerated, but **code originates in `origin` first**; `release` only consumes + adds release-specific bits.
- **Rule:** never author feature code directly on `release`. Internal/beta test builds are versioned `0.3.x-beta` and stay private (fetched locally, not the newest GitHub release); only the deliberate public release is tagged `0.4.0` and pushed to `release`.
- ⛔ **Rule (owner, 2026-09-12): nothing compiles C++ on push, by decision.** `origin` is a private repo on the free GitHub plan and the release builds run on the public org repo, so there is no CI build of the shell on `0.4.0`. Windows and Mac each compile their own platform locally. Therefore: **any commit that touches `cef-native/**` C++ gets a note in the current `development-docs/0.4.0-beta.3/MAC_RELAY_*.md` round naming the files, so the other side rebuilds after its next rebase.** A shared file with an `#ifdef _WIN32` / `#elif defined(__APPLE__)` split, or a `*_mac.*` file touched from Windows, is flagged explicitly. The release build (`release.yml`, both platforms) is the final backstop. No Mac-specific branch — it lengthens divergence without removing the blind spot.
- ⛔ **Rule (2026-09-19): fetch and rebase BEFORE you start, and REBUILD after the rebase. A clean rebase tells you nothing about whether it still compiles.** The sibling of the rule above, and the reason it is separate: that one is about warning the *other* side, this one is about not being caught by them. 📏 Both platforms now push to `0.4.0` several times a day — five macOS commits landed mid-session on 2026-09-19, and a later one **deleted shared shims from `simple_handler.h`**, the same header Windows had just added a member to. Git merged it without a conflict, because nothing in a textual rebase knows that a deleted declaration breaks a translation unit. ⇒ **`git fetch && git rebase origin/0.4.0`, then rebuild, then push.** ⚠️ And expect append-vs-append conflicts in the shared phase docs (`phase-*/README.md`, `HUMAN_TEST_QUEUE.md`, the relay rounds): both sides append to the end, and the resolution is almost always **keep both**, in history order — never take one side.
- *Open question:* whether `staging` stays a separate branch once `main` has CI-gated PRs — for now KEEP it as the integration / internal-beta branch.

## Build

**Prerequisites**: Rust, Node.js 18+, CEF binaries (download from https://cef-builds.spotifycdn.com/index.html → `./cef-binaries/`)

**Platform-specific build guides**: See `build-instructions/WINDOWS_BUILD_INSTRUCTIONS.md` or `build-instructions/MACOS_BUILD_INSTRUCTIONS.md` for first-time setup.

**Quick build (all platforms):**
```bash
# 1. CEF wrapper (first time only)
cd cef-binaries/libcef_dll/wrapper && mkdir build && cd build
cmake .. && cmake --build . --config Release

# 2. Rust wallet
cd rust-wallet && cargo build --release

# 3. Frontend
cd frontend && npm install && npm run build

# 4. CEF shell
cd cef-native
# Windows: cmake -S . -B build -G "Visual Studio 17 2022" -A x64 -DCMAKE_TOOLCHAIN_FILE=[vcpkg_root]/scripts/buildsystems/vcpkg.cmake
# macOS:   cmake -S . -B build -G "Unix Makefiles"
cmake --build build --config Release
```

---

## Invariants / Safety Rules

1. **Private keys never in JavaScript** — all signing happens in Rust.

   Signing keys never leave the Rust process. No EC private key is ever returned to JavaScript, and no signing happens there. The BIP39 recovery phrase is the one deliberate exception: it is shown once at wallet creation so the user can record it, and thereafter only through PIN re-verification in the wallet overlay. It is never reachable from web content -- the wallet's CORS allowlist admits only Hodos's own local UI origins.

2. **Do not change wallet DB schema** without asking first
3. **Do not change crypto/signing/derivation logic** without asking first
4. **Plan first** for cross-cutting refactors; implement in small steps
5. **Prefer minimal, reversible changes** - avoid "big bang" rewrites
6. **Read files before editing** - always use Read tool before Edit tool
7. **Build after changes**:
   - Rust: `cargo build`
   - TypeScript: `npm run build`
   - C++: `cmake --build . --config Release`
8. CEF lifecycle & threading rules are fragile — do not change message loop, browser creation timing, or render-process handlers without asking first.
9. **macOS cross-platform readiness**: All new C++ code must use `#ifdef _WIN32` / `#elif defined(__APPLE__)` platform conditionals. Never use raw WinHTTP for new singletons — use `SyncHttpClient` (or add macOS `#elif` with libcurl). New overlays need a macOS creation function in `cef_browser_shell_mac.mm`. New file paths must use cross-platform resolution, not hardcoded Windows paths.
11. **Update docs with features**: When completing a sprint or feature that changes architecture, APIs, endpoints, or user-facing behavior, update CLAUDE.md Key Files table and any affected top-level docs. Don't let docs drift.
12. **Document lessons learned → update Process & Procedures**: Whenever a build, dependency, codec, signing, release, or auto-update step surprises us, breaks, or teaches us something, write it down AND update the permanent Process & Procedures docs in `development-docs/DevOps-CICD/` (the CEF build runbook, build/release, auto-update, dependency-verification checklist). Goal: repeatable, automated, *testable* procedures a small team — or a small team of AI agents — can follow step-by-step with unit/integration tests at each stage. Treat P&P as code: keep it current or it rots. The DevOps-CICD folder is the canonical home; CLAUDE.md and sprint docs only point to it.
13. **Test-failure triage — verify which side is wrong; ask before changing production code**: When a test fails, first determine whether the **test** is at fault (stale assertion, incomplete fixture, drifted API call, over-specific string match) or the **production code/function** is. Decide by checking the *intended* behavior against an independent source — the spec, a co-located/sibling test, the function's other call sites, or git history — **never** by reflexively making the test pass. **Test-only fixes** (correcting a stale assertion, seeding a fixture, updating a drifted test call site, `#[ignore]`-ing a network test) may proceed. **If the evidence points at the production code being wrong, STOP and ASK before changing it** — present the evidence for which side is wrong and get approval first. Generalizes #2/#3 (never change schema/crypto silently) to all production code reached via a failing test.


---

## Key Files

| File | Purpose |
|------|---------|
| `rust-wallet/src/handlers.rs` | The wallet's HTTP endpoint surface: wallet CRUD, BRC-100, BRC-72 key linkage, domain permissions (incl. sub-permission CRUD at `/domain/permissions/{protocol,basket,counterparty}` POST/DELETE/GET), price, sync status, PeerPay, BRC-121 (`pay_402` mints a nosend BRC-29 BEEF + emits the 5 retry headers; `broadcast_nosend` broadcasts after the paid retry returns 200). **Full handler roster: `rust-wallet/src/CLAUDE.md`.** `/getPublicKey` gate: identity-key-style requests from external domains (`X-Requesting-Domain` present) route through `permission_service::dispatch_privacy_perimeter` with `CallKind::IdentityKeyReveal` — silent only when `domain_permissions.identity_key_disclosure_allowed=1` (V17) or a session opt-in is set; otherwise Rust returns 202 PENDING + `approvalId` and C++ re-issues with `X-User-Approved: <approvalId>`. The legacy `X-Identity-Key-Approved` header is ignored and no longer injected. |
| `rust-wallet/src/crypto/` | All crypto primitives: BRC-42/43 derivation, ECDSA signing (ForkID SIGHASH), BRC-2 encryption, BIE1, AES-GCM + GHASH, BRC-72 key linkage (counterparty + specific), PIN wrapping (PBKDF2+AES-GCM), and at-rest key protection (Windows DPAPI / macOS Keychain). Module roster: `rust-wallet/src/crypto/CLAUDE.md` |
| `rust-wallet/src/authfetch.rs` | BRC-103 AuthFetch HTTP client: 401 challenge-response with ECDSA signing, server/client nonce exchange, authenticated requests to external BRC-103 servers (MessageBox) |
| `rust-wallet/src/messagebox.rs` | MessageBox API client: BRC-2 encrypted message send/receive/acknowledge via `messagebox.babbage.systems`, deterministic HMAC message IDs, uses AuthFetch for authentication |
| `rust-wallet/src/database/` | Repository pattern — each table group has a `*Repository<'a>` borrowing `&'a Connection`; `WalletDatabase` (`database/connection.rs`) owns the connection, caches the unlocked mnemonic, and runs `migrate()` against `database/migrations.rs`. Full roster: `rust-wallet/src/database/CLAUDE.md`. Note: backup/restore is a **top-level** module (`rust-wallet/src/backup.rs`), not a database submodule |
| `rust-wallet/src/recovery.rs` | BIP32 legacy key derivation (`derive_private_key_bip32`), wallet recovery from mnemonic |
| `rust-wallet/src/price_cache.rs` | BSV/USD price cache: WhatsOnChain primary → CoinGecko (slug `bitcoin-cash-sv`) → MEXC fallback chain, 5-min in-memory TTL (`CACHE_TTL_SECONDS = 300`), plus SQLite `bsv_price_cache` restart persistence (V21) so a cold start falls back to the last known good price instead of `price_unavailable`. CryptoCompare was removed in the 2026-06-09 redesign after it began returning HTTP 401 |
| `rust-wallet/src/monitor/` | Background task scheduler (`Monitor` + named tasks on configurable intervals). Task roster: `rust-wallet/src/monitor/CLAUDE.md`. Notable: `TaskCheckPeerPay` — BRC-103 AuthFetch + BRC-2 encrypted MessageBox polling, then auto-accept via `store_derived_utxo`, guarded by an on-chain existence check (`check_tx_exists_on_chain`, a duplicated copy of `internalize_action`'s) with a `broadcast_transaction` fallback |
| `cef-native/cef_browser_shell.cpp` | Windows entry point; owns the overlay globals/flags, `WalletOverlayWndProc` and the main `WndProc` (incl. `WM_ACTIVATEAPP`), plus the `LOG_*` macro wrappers and the `Logger` lifecycle calls. (`Logger` itself is declared in `cef-native/include/core/Logger.h` / implemented in `src/core/Logger.cpp`.) Overlay create/show/hide functions are **not** here — they live in `cef-native/src/handlers/simple_app.cpp` (`CreateDownloadPanelOverlay`, `ShowDownloadPanelOverlay`, `HideDownloadPanelOverlay`, …); this file only holds `extern` decls and call sites |
| `cef-native/cef_browser_shell_mac.mm` | macOS entry point; NSWindow/NSView hierarchy, the macOS overlay creation functions (`Create*OverlayMacOS`), event forwarding, multi-window support. Overlay roster: `cef-native/include/handlers/CLAUDE.md` |
| `adblock-engine/src/engine.rs` | AdblockEngine wrapper: filter list downloading, engine compilation, serialization. Thread-safe checking through `RwLock<Option<Engine>>` — the `Option` lets the HTTP server come up before the compiled engine is available (`None` until load/compile completes). Auto-update every 6 hours. Filter lists + bundled scriptlets: `adblock-engine/src/CLAUDE.md` |
| `adblock-engine/src/handlers.rs` | Adblock HTTP endpoints on port 31302 (31402 under `HODOS_DEV=1`). Endpoint roster: `adblock-engine/src/CLAUDE.md` |
| `cef-native/include/core/AdblockCache.h` | `AdblockCache` singleton: sync WinHTTP to port 31302, URL result cache, per-browser blocked counts, cosmetic resource fetching. `AdblockBlockHandler` cancels blocked requests. `AdblockResponseFilter` (CefResponseFilter) buffers YouTube responses and renames ad-configuration JSON keys. `CookieFilterResourceHandler` returns cookie filter + response filter for YouTube. |
| `cef-native/src/handlers/simple_handler.cpp` | CEF client handler (implements many CEF interfaces incl. CefDownloadHandler, CefFindHandler, CefJSDialogHandler); IPC dispatch, keyboard shortcuts, context menus (all custom `MENU_ID_USER_FIRST`-based IDs — see working-notes.md #8), download tracking, find-in-page (JS `window.find()` — the CEF Find API is non-functional in our CEF build), beforeunload trap suppression, `OnBeforeBrowse` scriptlet pre-cache + fingerprint seed IPC, cosmetic CSS/scriptlet injection, menu IPC (print/devtools/zoom/exit), DNT/GPC header injection, settings_set dispatch. Helpers: `CreateNewTabWithUrl()`, `CopyTextToClipboard()`. Cross-platform wrapped. Interface list, shortcut table and menu-ID roster: `cef-native/src/handlers/CLAUDE.md` |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | V8 injection; class: `CefMessageSendHandler`; scriptlet pre-cache (`s_scriptCache` + `OnContextCreated` early injection); cosmetic CSS/script IPC handlers; fingerprint seed cache (`s_domainSeeds`) + fingerprint script injection in `OnContextCreated`. It **calls** `escapeJsonForJs` but does not define it — the canonical JS-string-literal encoder lives in `cef-native/include/core/JsStringEscape.h` (extracted by the F6 HelicOps audit fix so it is unit-testable without CEF and shared by every injection site) |
| `cef-native/include/core/FingerprintProtection.h` | `FingerprintProtection` singleton — now **policy only**, no seeds. Supplies the three inputs `OnBeforeBrowse` collapses into C2's single `enabled` bit: global toggle (`IsEnabled`/`SetEnabled`), the host-precise auth/OAuth allowlist (`IsAuthDomain`), and the user's per-site Privacy Shield opt-out (`IsSiteEnabled`/`SetSiteEnabled`, persisted to `fingerprint_settings.json`). ⚠️ The last two are **shipped user-facing control** — never delete while tidying. The JS-injection half (`GetDomainSeed`, session token, seed cache) was removed 2026-08-09 |
| `cef-native/include/core/FarblingPolicy.h` | The browser-process half of native farbling (C2): persistent per-profile seed (`EnsureProfileSeed`, CSPRNG, stored in `fingerprint_settings.json`), `DomainKeyForUrl` = HMAC(profile seed, registrable domain), and `RegistrableDomainFromUrl` — a deliberately hand-rolled reduction that libcef must **not** re-derive independently, or every registry lookup would miss and fail closed silently |
| ~~`cef-native/include/core/FingerprintScript.h`~~ | **DELETED 2026-08-09.** Farbling is native in Blink (fork patches C1/C3/C4/C5/C6), applied at API-call time, so the patched methods report `[native code]` again. Do not re-add an injected-JS farbling path: it cannot cover workers, it restores the `toString` tamper tell, and it would double-perturb values Blink already farbles |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | HTTP routing + modal orchestration + payment-context computation (satoshis→cents via `BSVPriceCache`). **NOT the decision engine.** Since Phase 2.6-H all permission/auto-approve *decisions* are Rust-authoritative; C++ forwards the call, intercepts Rust's 202 PENDING envelope, opens the modal via `OpenPromptModal`, and re-issues with `X-User-Approved: <approvalId>`. Also owns the BRC-121 402 paid-retry chain (`TryHandleBrc121_402`, `InstallAsync402HandlerIfPending`, `Async402ResourceHandler` + `Async402HTTPClient` and their paid-retry context registry) — the one subsystem here that spends money. After `firePaymentSuccessIpc()` it writes the paid response into `PaidContentCache` so a reload of the same URL serves bytes from disk instead of re-paying. Class roster: `cef-native/src/core/CLAUDE.md` |
| `rust-wallet/crates/hodos_permission_engine/` | **The** pure-logic decision engine for wallet permission gates. `decide(&PermissionContext) -> PermissionDecision` (`src/lib.rs`) delegates to `src/matrix_c.rs`, implementing `PERMISSION_UX_DESIGN.md` Matrix C: domain trust → privacy perimeter → scoped grants → payment caps → cert disclosure → generic. `PermissionDecision` (`src/decision.rs`) is a `#[serde(tag="kind")]` sum type — `Silent{reason}` / `Prompt{prompt_type, reason}` / `Deny{reason}`. No CEF/HTTP/globals; unit-tested in isolation. Wrapped by `rust-wallet/src/permission_service/` (`request_gate.rs`, `context_builder.rs`, `state.rs`, `audit.rs`; `dispatch_payment` / `dispatch_privacy_perimeter`) and wired as Actix middleware in `rust-wallet/src/main.rs`. Default limits: **$1.00 per transaction, $10.00 per session** (100 / 1000 USD cents). ⚠️ The C++ `PermissionEngine`, `PermissionGate` and `SessionManager` were **deleted** in Phase 2.6-H — stale comments still naming them are dead references, not code |
| `cef-native/src/core/ManifestFetcher.cpp` | Parses a dApp's wallet manifest. `ParseFromJson(json)` is pure and **still live** — the interceptor re-parses the manifest bytes Rust embeds in the `manifest_connect_bundle` 202 payload before opening the connect modal, falling back to `domain_approval` when the parse yields nothing. 🚨 Since beta.3 Phase 0.8, `valid == true` means **"declared at least one recognised permission"**, not merely "parsed OK" — it used to be set *"even if some sections are empty"*, which is why a permission-free document produced a bundle modal itemising nothing while the site held real access. ⛔ Do not relax it back. Understands three shapes in precedence order: **BRC-73** `metanet.groupPermissions`, deprecated `babbage.groupPermissions`, then our own top-level `permissions` (`PERMISSION_UX_DESIGN.md` §5). ⚠️ **Two-layer parser** — `rust-wallet/src/manifest.rs :: parse_manifest` is the other half and re-parses the same bytes; **change both or neither**, and note the `(N protocols…)` count in the connect log comes from the C++ parse. Arrays and strings are bounded (`kMaxManifest*`) because the 64 KB fetch cap is not a parse cap. `Fetch(origin)` is **not on the production path** — the network fetch moved to Rust in Phase 2.6-G.2; its only C++ caller, `handleIpcUnknownTrust`, is dead code slated for removal |
| `cef-native/src/core/PaidContentCache.cpp` | Phase 1 BRC-121 paid response cache. SQLite-backed singleton at `<profile>/paid_content_cache.db`. URL-keyed entries with TTL from server `Cache-Control: max-age` and a 500 MB LRU cap on `last_access`. Best-effort `Put` swallows exceptions. Read-side dispatch in `simple_handler.cpp::GetResourceRequestHandler` (single call site at the top, before adblock/wallet routing). Hard-reload (Ctrl+Shift+R) bypasses via `Cache-Control: no-cache` request header. Header-only playback handler: `cef-native/include/core/CachedContentResourceHandler.h`. |
| `cef-native/include/core/PendingAuthRequest.h` | `PendingRequestManager` singleton — thread-safe request tracking for auth/domain/payment/cert approvals |
| `cef-native/include/core/ProfileManager.h` | `ProfileManager` singleton: multi-profile support, profile creation/switching, profile directory management |
| `cef-native/include/core/TabManager.h` | `TabManager` singleton: per-window tab tracking, tab creation/close/switch, multi-window tab coordination, and the window→active-tab mapping (`GetActiveTabIdForWindow`, `MoveTabToWindow`) |
| `cef-native/include/core/WindowManager.h` | `WindowManager` singleton: multi-window lifecycle (`CreateWindowRecord` / `RemoveWindow` / `CreateFullWindow`), active + primary window tracking, and `GetWindowForBrowser` — which resolves the `BrowserWindow` owning a given **role** browser (header/webview/overlay), *not* tab browsers. It holds no tab state; window↔tab mapping belongs to `TabManager`. Its only tab call is one `TabManager::CreateTab()` to seed a new window's initial NTP tab |
| `cef-native/include/core/SettingsManager.h` | `SettingsManager` singleton: persistent settings storage, cross-platform settings resolution |
| `cef-native/include/core/ProfileImporter.h` | Chrome/Brave/Edge profile importer: **bookmarks and history only — no cookie import**. A Firefox stub exists (`GetFirefoxProfilePath()` returns `""`) but Firefox is never detected or imported |
| `cef-native/include/core/SyncHttpClient.h` | Cross-platform sync HTTP client (WinHTTP on Windows, libcurl on macOS). Use this for new singletons instead of raw WinHTTP |
| `frontend/src/hooks/useHodosBrowser.ts` | React hook wrapping the identity + navigation half of the `window.hodosBrowser` bridge. API surface: `frontend/src/hooks/CLAUDE.md` |
| `frontend/src/hooks/useDownloads.ts` | React hook for download state; listens for the `download_state_update` IPC and exposes the download control functions. API surface: `frontend/src/hooks/CLAUDE.md` |
| `frontend/src/pages/DownloadsOverlayRoot.tsx` | Download panel overlay page; lists active/completed downloads with progress bars, pause/resume/cancel, open/show-in-folder |
| `frontend/src/components/FindBar.tsx` | Find-in-page bar component; Ctrl+F triggered; sends `find_text`/`find_stop` IPC; displays "X of Y" match count |
| `frontend/src/components/MenuOverlay.tsx` | Three-dot menu dropdown; replaced the old standalone History+Settings toolbar buttons. Item roster: `frontend/src/components/CLAUDE.md` |
| `frontend/src/pages/SettingsPage.tsx` | Full-page settings with sidebar navigation. Route: `/settings-page/:section`; section roster: `frontend/src/pages/CLAUDE.md` |
| `frontend/src/hooks/usePrivacyShield.ts` | Composed privacy hook: adblock + cookie blocking + scriptlet toggle state. Used by `PrivacyShieldPanel` overlay |
| `frontend/src/bridge/initWindowBridge.ts` | Defines `window.hodosBrowser.navigation`, `window.hodosBrowser.overlay` via `cefMessage.send()` |

---

## Wallet Service Fee

Every outgoing transaction includes a **1000-satoshi service fee** output sent to the Hodos company treasury address (`1Q1A2rq6trBdptd3t6n53vB79mRN6JHEFT`). This applies to all four transaction builders:

| Builder | Location |
|---------|----------|
| `create_action_internal` | `handlers.rs` — standard sends, PeerPay, Paymail |
| `publish_certificate` | `certificate_handlers.rs` — identity certificate publish |
| `unpublish_certificate_core` | `certificate_handlers.rs` — identity certificate unpublish |
| `task_consolidate_dust` | `monitor/task_consolidate_dust.rs` — scheduled dust consolidation. ⚠️ This one is **not user-initiated**: a background task moves 1000 sats to the treasury on its own schedule |

**Constants**: `HODOS_FEE_ADDRESS`, `HODOS_SERVICE_FEE_SATS` in `handlers.rs` (both `pub`).

**Output order**: request outputs → service fee → change. The `CreateActionResponse.outputs` array excludes the service fee and change (only returns request outputs).

**Commission tracking**: Each service fee is recorded in the `commissions` table. Commission records are cleaned up on broadcast failure.

*(This section **is** the implementation doc — the standalone `WALLET_SERVICE_FEE_IMPLEMENTATION.md` was deliberately deleted in 3a7007d and its content folded in here.)*

---

## ⭐ Key vocabulary — short version

**Rewritten 2026-09-04.** ⛔ An earlier version of this section invented "three senses of public"
and made things worse, not better. Owner: *"Your charts still make no fucking sense."* Fair. The
convention was never the problem — this is.

### The convention is the normal one. Use it.

**"public key" = the shareable half of a keypair. "private key" = the secret half.** That is
standard and nothing here changes it.

### The one fact that resolves most of the confusion

⭐ **One private key derives many keypairs.** That is what BIP32/BRC-42 *are*. From a single master
private key you can compute unlimited **child keypairs**, each with its own private and public half.

⇒ So you do **not** have to hand the same public key to every site. You can derive a different one
per site, and those sites cannot link them. **The linkability is a choice, not a constraint.**

### The keys we actually have

| Name | What it is |
|---|---|
| **recovery phrase** | The BIP39 mnemonic. Everything below is computed from it |
| **master private key** | The one key at the top. `master_pubkey` / master private key in `handlers.rs` |
| **identity key** | The public half of the master key. **The same on every site you show it to**, which is what makes it linkable. BRC-103 calls it `identityKey`; `identity_key_disclosure_allowed` governs revealing it |
| **derived key** | A child keypair from (master private key + counterparty public key + invoice number). **Different inputs → different key** |
| **site-scoped key** | A *derived key* whose inputs include the site's origin, so each site gets a different one. ⭐ This is the unlinkable option |
| **payment output key** | A derived key used to lock one UTXO. The address is a hash of it |

⚠️ **"master" vs "root":** the code says **master**. Use *master*. (BRC-42 prose says "root" and
BRC-103 says "identity key" for the same thing — expect all three in the wild.)

### The one genuine oddity, and it is a real trap

**The well-known "anyone" key** — `PrivateKey(1)`, the secp256k1 generator
(`ANYONE_PUBKEY_HEX`, `certificate_handlers.rs`).

It is a public key like any other **except that its private half is the number 1, which everybody
knows.** So anything derived against it can be derived by anyone. That is sometimes exactly right —
publishing a certificate, or a public profile token — and catastrophic if used by mistake for
something meant to be secret.

⛔ It is **not** a wildcard meaning "any counterparty". It is one specific key.

### Protocol vocabulary

| Name | What it is |
|---|---|
| **protocol namespace** | The site-authored string, e.g. `server hmac`. **Untrusted text** |
| **security level** | BRC-43's `0`/`1`/`2`. ⚠️ **Higher = narrower.** Level 2 re-asks per counterparty; level 1 covers every counterparty at once. The name reads like a risk score and means the opposite |
| **protocol scope** | The pair `[security level, namespace]`. `is_protocol_granted` matches on **both**, so `[1,"foo"]` ≠ `[2,"foo"]` |
| **invoice number** | BRC-43's `{securityLevel}-{protocolID}-{keyID}`. With the counterparty, it determines which derived key comes out |

⛔ **Why this matters:** a site's `identity key retrieval` **protocol grant** and the wallet's
**Identity checkbox** are different mechanisms with different gates. They look identical because both
were being called "the identity key permission". Do not couple them —
`TICKET_connect_modal_two_views_drift.md` §"A third instance".

---

## Glossary

| Term | Meaning |
|------|---------|
| BRC-100 | BSV authentication/identity protocol suite |
| BRC-42 | ECDH-based child key derivation (root identity private key + counterparty key → **site-scoped derived key**). ⭐ Vocabulary: see the section above |
| BRC-43 | Invoice number format: `{securityLevel}-{protocolID}-{keyID}`. The **protocol scope** is the pair `[security level, namespace]` — both are matched |
| BRC-52 | Identity certificate format with selective disclosure |
| BRC-2 | Symmetric encryption using BRC-42-derived AES-256-GCM keys. Used for MessageBox message encryption |
| BRC-29 | PeerPay direct payment protocol: sender derives recipient key via BRC-42, creates P2PKH output, sends PaymentToken via encrypted MessageBox. Protocol ID: `3241645161d8` |
| BRC-103/104 | Mutual authentication protocol. Client side (`authfetch.rs`): 401 challenge → sign nonces+request → re-send with auth headers |
| MessageBox | Remote message relay service at `messagebox.babbage.systems`. BRC-103 authenticated, BRC-2 encrypted. Used for PeerPay payment delivery |
| BEEF | Background Evaluation Extended Format - atomic transaction format with SPV proofs |
| BUMP | BRC-74 Binary Merkle Proof format. Used inside BEEF for SPV verification |
| CEF | Chromium Embedded Framework |
| ForkID SIGHASH | BSV-specific transaction signing (differs from BTC since 2017 fork) |
| HD Wallet | Hierarchical Deterministic wallet using BIP39 (mnemonic→seed). New outputs use BRC-42 self-derivation; legacy BIP32 (`m/{index}`) preserved in recovery module |
| UTXO | Unspent Transaction Output |
| V8 Injection | Adding `window.hodosBrowser` API to JavaScript from C++ |
| `window.hodosBrowser` | JavaScript API exposed to React for wallet operations |
| Monitor Pattern | Background task scheduler (`src/monitor/`): named tasks registered in a `TaskSchedule` on configurable intervals. Replaced ad-hoc background services in Phase 6. Task roster: `rust-wallet/src/monitor/CLAUDE.md` |
| Browser Data | History, bookmarks, cookies — stored in C++ layer (`%APPDATA%/HodosBrowser/Default/`), separate from wallet |
| CefResponseFilter | CEF API for streaming modification of HTTP response bodies. Used by `AdblockResponseFilter` to strip YouTube ad keys at the network level before JavaScript sees the data |
| Cosmetic Filtering | CSS selector injection to hide ad-related DOM elements + scriptlet injection to override JavaScript ad behavior. Two-phase: hostname-specific selectors on page load, generic selectors after DOM class/ID collection |
| Scriptlet Injection | JavaScript injected into page context via V8 to override browser APIs (fetch, XHR, JSON.parse) and strip ad data. Pre-cached via `OnBeforeBrowse` IPC, injected in `OnContextCreated` |
| Fingerprint Farbling | Brave-style fingerprint randomization: per-session token hashed with domain → deterministic PRNG seed → Canvas/WebGL/Audio/Navigator values slightly perturbed. Same values within session+domain, different across sessions |
| `#@#+js()` | adblock-rust exception syntax: blanket disable all scriptlet injection for a domain. Used in `hodos-unbreak.txt` for auth sites |

---

## Context File Maintenance

**After each sprint, phase, or sub-phase:**
1. Review this CLAUDE.md — Is it still accurate? Update Key Files table if architecture changed.
2. Check sprint-specific CLAUDE.md in `development-docs/Final-MVP-Sprint/` or `development-docs/Sigma-BRC121-Sprint/`.
3. Add new patterns/gotchas to the relevant context file.

**Goal:** Context files should always reflect current reality. They're the institutional memory that lets any AI (or human) pick up where the last session left off.

### Sprint Documentation

| Folder | Purpose |
|--------|---------|
| `development-docs/SCOPING_PROCESS.md` | The four-stage scoping procedure (Scope → Telescope → Microscope → Telescope-close). Owned by the `sprint-scoper` agent. See the section near the top of this file |
| `development-docs/0.4.0-beta.4/` | **Current sprint — the wallet asset layer.** Four sprints in a settled order: UTXO safety guard → 1Sat Ordinals → OpNS naming → on-chain backup & sync. Start at its `README.md`, then `TELESCOPE.md`. ⛔ Its harness is **inherited by reference** from `0.4.0-beta.3/HARNESS.md`; beta.4's additions are in `HARNESS_DELTA.md` / `REGRESSION_ADDITIONS.md` only |
| `development-docs/architecture/` | Cross-layer architecture — the flows no single layer's `CLAUDE.md` owns. Rewritten against code 2026-08-03 (the prior contents described the deleted C++ permission engine). Three docs: `AUTO_APPROVE_ENGINE.md` (permission decision flow + Matrix C branch order), `IPC_BRIDGE.md` (the `wallet_call` process-message contract), `WALLET_API_MAP.md` (which endpoints are gated by which Rust dispatcher, and the shim surface). Inventory stays in the layer docs; these three carry only cross-layer flow. |
| `development-docs/FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md` | The original "engine in Rust" vision doc. Largely **realized** by Phase 2.6 — read it as history, not as a plan |
| `development-docs/Final-MVP-Sprint/` | Sprint: testing, optimization, security, macOS port |
| `development-docs/Final-MVP-Sprint/macos-port/` | macOS port tracking: progress, handover docs, archived milestones |
| `development-docs/Sigma-BRC121-Sprint/` | Sprint: BRC-100 surface completion. Phase folders are the authoritative status source — see the phase README in each |
| `development-docs/Sigma-BRC121-Sprint/phase-2.6-engine-to-rust/` | The permission-engine port to Rust (sub-phases A–H) — the change that deleted the C++ `PermissionEngine`, `PermissionGate` and `SessionManager` |
| `development-docs/Sigma-BRC121-Sprint/phase-4-demos/` | Phase 4 (demos); absorbed Phase 1.5 Step 7 |
| `archived-docs/UX_UI/` | Wallet UI phases (setup, notifications, wallet panel polish) — completed sprint, archived |
| `development-docs/Future-Features/` | Future feature docs (e.g. extensions) deferred beyond the current sprint |
| `build-instructions/` | Platform-specific build guides (Windows, macOS) |

### Active sprint status (as of 2026-05-30)

> ⚠️ **Stale — awaiting owner review (flagged 2026-08-03).** The as-of date has not been re-verified and several rows are known to have moved since (Phase 2.5 closed 2026-06-02 at `0baec25`; Phase 2.6 ran to completion at `f02cf91`). Until this table is refreshed, treat the phase folders under `development-docs/Sigma-BRC121-Sprint/` as authoritative for phase status.

| Phase | Status |
|-------|--------|
| Phase 1.5 (permission UX) | ✅ Landed |
| Phase 1.6 (indexer resilience) | ✅ Landed |
| Phase 2 (window.CWI shim) | ✅ Steps 1-4 + 3b + 3c landed; smoke surfaced CSP/CORS issue → Phase 2.5 |
| Phase 2.5 (wallet IPC bridge) | 🚧 Commits 1-4 landed; commits 5-7 multi-session work pending. See `development-docs/Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md` |
| Phase 3 (ordinals) | Queued — blocked on Phase 2.5 + Step 3d (getSignatures research) |
