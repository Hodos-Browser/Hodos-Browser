# Testing Strategy — Canonical (cross-stack)

**Created:** 2026-06-16 · **Owner:** DevOps/CI-CD · **Canonical home:** `development-docs/DevOps-CICD/`
**Per root CLAUDE.md Invariant #12** — keep current; append to §13 Lessons Learned.

> **This is the ONE testing strategy.** The HelicOps audit's "coverage gaps" and the pipeline's "CI test gate" are the **same problem** — solved once, here. Layer-specific test *details* live in each layer; this doc is the cross-stack strategy + the rules CI enforces.
> **Relationship to other docs:** `BUILD_AND_RELEASE.md` §5 holds the actual CI/release **workflow** (today PLANNED — no `ci.yml` exists); this doc defines **what** that workflow runs and **why**. **`TEST_PLAN.md`** is the detailed PLAN/CATALOG this strategy points to — ts-sdk vectors to port, the Vitest blueprint, the e2e/adblock/C++ inventory, manual QA checklists, and the reconciled per-file census. `DEPENDENCY_VERIFICATION.md` is the per-CEF-bump dep checklist. Master plan items: **PIPE-A7** (this strategy), **PIPE-CI** (build `ci.yml`), **PIPE-TESTGATE** (gate release), **TEST-HARNESS** (capped live e2e), **AUDIT-F8** (secret-log gate).

---

## 1. Core principle — co-locate files, centralize orchestration
- **Test FILES stay with the code** they test (move with the code, clear ownership):
  - Rust: `rust-wallet/tests/` + inline `#[cfg(test)]`; `adblock-engine/` inline.
  - C++: `cef-native/tests/` (GoogleTest, opt-in `-DHODOS_BUILD_TESTS=ON`).
  - Frontend: `frontend/` (Vitest — to add) + `frontend/e2e/` (Playwright).
- **Orchestration is centralized:** one runner (`scripts/test-all.ps1`), one reusable CI workflow (`.github/workflows/`), and this strategy doc. **Do NOT** physically gather test files into one folder.

## 2. Current census (verified 2026-06; replaces the inflated "780+" claim)
| Layer | Tests today | Gap |
|-------|-------------|-----|
| Rust (wallet) | ~480 (incl. crypto/signing vectors) | strong; keep |
| Adblock | 23 inline | ok |
| C++ (cef-native) | **139 in 8 GoogleTest files** (re-counted 2026-08-04), opt-in via `-DHODOS_BUILD_TESTS=ON` | expand to real targets (PaidContentCache, AdblockCache); see §14 on *why* the untested parts are untested |
| Frontend unit (Vitest) | **0** | **inverted pyramid — add a thin layer** |
| Frontend e2e (Playwright) | 54 | ok; Windows run unverified |
| **CI enforcement** | **none** — no `ci.yml`; `release.yml` runs zero tests | **the #1 gap** |

## 3. What to test at each layer (the pyramid, right-side-up)
- **Rust = the heavy base.** Unit + integration, with **real BSV/BRC vectors** for `crypto/` (derivation, signing, BEEF, BRC-121 wire format). This is where correctness lives — keep it deep.
- **C++ = targeted unit** on pure logic (parsers, caches, classifiers) — not UI. Expand opt-in GoogleTest onto real targets.
- **Frontend = thin Vitest** for the logic that *does* live in the client (formatters, validators, `DomainPermissionForm` validation, hooks) + **Playwright e2e** for flows. Don't chase coverage on presentational components (Testing Trophy: integration-leaning over isolated unit for UI). The inverted pyramid is acceptable *only* because logic lives in Rust — but the thin Vitest layer closes the gap cheaply.
- **Cross-stack = smoke + capped live e2e** (§9, §10).

## 4. CI gating model (how tests block merges & releases)
- **One reusable test workflow** (`on: workflow_call`) with jobs: `rust` (test + clippy `-D warnings`), `adblock`, `cpp` (ctest, `-DHODOS_BUILD_TESTS=ON`, matrix Win+Mac), `frontend` (vitest + playwright), `security` (cargo audit + npm audit + secret-log gate §8).
- **`ci.yml`** (`on: pull_request`) calls it → set as a **required status check** on the protected branch ⇒ **can't merge red.**
- **`release.yml`** calls the *same* workflow; build/sign jobs declare **`needs: [test]`** ⇒ **can't ship red.** No duplicated test logic.
- **Pre-commit hooks** (gitleaks, fast lint) for instant local feedback — bypassable (`SKIP=`), so **CI is the enforceable gate.**

## 5. Coverage policy
- Tool: **`cargo-llvm-cov`** (cross-platform incl. Windows MSVC; `cargo-tarpaulin` is Linux-only/unreliable on Windows). Run coverage on a **Linux runner** for stability; the Win/Mac matrix runs the tests themselves.
- Thresholds (gate via `--fail-under-*`): **crypto/signing/key-derivation ≥ 90%** (line+branch, aim near-100%); **general ~70–80%.**
- Coverage is a **signal, not a target** (Goodhart) — back it with **mutation testing** on `crypto/` to prove the tests actually catch breakage.

## 6. Accuracy & anti-gaming (keep tests trustworthy as volume grows)
- Assert **real behavior**, not tautologies; use real vectors so a failure means the logic is actually wrong.
- **Ban `continue-on-error` / silent retries** on security-critical jobs.
- **Fail CI on skipped/`#[ignore]` tests in crypto paths** (a grep gate).
- **Quarantine flaky tests visibly** (tracked issue + dashboard), never auto-retry-to-green.
- **Review test diffs** in PR. Automated checks **flag**; a human / adjudicating agent **confirms** (HelicOps meta-lesson: "SAST = lead-gen, not ledger" — don't blindly trust green).

## 7. Regression discipline
- **Every audit/bug finding gets a regression test.** F1/F2/F3 (secrets→disk) → a test asserting no key/seed/mnemonic reaches logs (and the §8 gate). A bug the audit found = a test we were missing.

## 8. Secret-in-logs gate (AUDIT-F8 — durable mitigation)
- **gitleaks** (custom `.gitleaks.toml` rules for key/seed/mnemonic/privkey near `log::`/`println!`/`std::cout`) **+ a custom ripgrep gate** targeting the exact sink shapes.
- Run as **both** a pre-commit hook (convenience) **and** a CI job (the real gate — can't be skipped).
- **Compile-time:** gate crypto-debug logging behind a cargo **feature flag** (off by default, never in release) or `cfg(debug_assertions)`. Feature flag preferred (explicit, greppable).

## 9. Capped test-wallet harness (agent-run live e2e) — TO BUILD
**Goal:** let agents (or humans) run **real** browser/wallet tests against **real sites** with a **hard-bounded** worst case — safe enough to automate.
- **`HODOS_DEV`** → separate dev DB; structurally cannot touch production data.
- **Dedicated TEST WALLET** with a tiny real balance (or testnet if supported — **confirm**), so blast radius is cents.
- **Low caps via the existing domain-permission/spending system** (per-tx, per-session, max-tx-per-session) — **enforced in Rust**, so the harness cannot exceed them.
- **Domain allowlist** for tests (e.g. `now.bsvblockchain.tech` BRC-121 site + the §10 verification basket).
- **Defense in depth:** cap at the wallet (Rust) **+** cap in the harness **+** allowlist domains **+** every spend fires the **gold pill** (audit trail).
- Agents are granted permission to run the **e2e harness** (Playwright) against allowlisted sites — **not raw wallet access.**
- *Status: design only — this is a feature to build (master plan `TEST-HARNESS`).*

## 10. Smoke & real-world verification (from CLAUDE.md)
A **smoke test** is a quick, shallow "did we fundamentally break it" check across critical paths after every build — a tripwire, not exhaustive.
| Tier | When | Sites |
|------|------|-------|
| **Minimal** | after any browser-core change | youtube.com, x.com, github.com |
| **Standard** | after a sprint | auth (x/google/github) + 2–3 video/media + 1–2 news |
| **Thorough** | before release/demo | full basket, all categories incl. BSV (whatsonchain.com) |

## 11. Hermetic CI notes
- **GoogleTest** is pulled via CMake FetchContent (needs network) → make hermetic: vcpkg (already our dep manager) **or** pinned `GIT_TAG` + `actions/cache`. 
- Pin all action versions; cache cargo/npm; vendor or cache test deps so CI is reproducible.

## 12. Decisions to lock (when we build PIPE-CI)
- Build the thin **Vitest** layer (recommended) vs formally retire `TEST_PLAN.md` §3 (the Vitest blueprint) — **recommend build.**
- Where C++/coverage runs (Linux for coverage; matrix for tests).
- Crypto coverage threshold + whether mutation testing is a gate or a periodic report.

## 13. Lessons Learned (append per Invariant #12)
- *(2026-06-16)* Test census was inflated across docs ("780+") — real ~480 Rust / 39 C++ / 23 adblock / 54 e2e / 0 Vitest. Trust source counts, not doc claims.
- *(2026-06-16)* The "CI gate exists" claims in BUILD_AND_RELEASE/`UNIT_TESTING` (now `TEST_PLAN.md`) were fiction — there is no `ci.yml`. Build it; don't trust the doc.
- *(2026-08-04)* **The census drifted the *other* way and nobody noticed for two months.** The C++ row said "39 in 2 files"; the tree actually had **139 in 8**. Undercounting is not harmless — it makes the layer look more neglected than it is and mis-aims effort. The 2026-06-16 lesson ("trust source counts") applies to *our own* doc, so the census needs a re-count on every touch, not a re-read.
- *(2026-08-04)* **Testability is decided when you choose where a function lives, not when you sit down to test it.** `escapeJsonForJs` is unit-tested (15 cases) only because the F6 audit *extracted* it to `JsStringEscape.h`, away from CEF. `jsonToV8` is the same kind of pure logic but lives in `IdentityHandler.cpp`, which drags in `cef_v8.h` + `WalletService` — so it is **not** reachable from `hodos_tests` without linking CEF into the test target. That is why an int64 truncation bug (`CreateInt` is int32; Chromium-epoch µs timestamps overflow it) sat there undetected. **Extract-then-test is the pattern that works for our C++.** See §14.
- *(add new lessons as the workflow lands…)*

---

## 14. What the CEF-150 / bootstrap work is teaching us (added 2026-08-04)

Written while migrating to CEF's bootstrap model, because most of that work is **not unit-testable**
and it is worth being explicit about why — otherwise "add tests" becomes a reflex that produces
nothing useful.

### 14.1 Three different kinds of change, three different gates

| Kind of change | Example from this work | Right gate | Wrong gate |
|---|---|---|---|
| **Pure logic** | `jsonToV8`, `escapeJsonForJs`, `ParseCacheControl` | GoogleTest unit | e2e |
| **Process-boundary / capability** | renderer no longer opens the history DB; enabling the Chromium sandbox | **runtime smoke** — only observable in a live multi-process run | unit tests cannot see it |
| **Build & packaging invariants** | bootstrap ↔ libcef version pinning, cert-thumbprint equality, file manifest | **CI assert** on the built artifact | unit tests, which never see the artifact |

The instinct that "we'll need to test every single thing" is right about *coverage of behaviour* and
wrong about *mechanism*. Most of the CEF-bump risk lands in rows 2 and 3, where the leverage is a
smoke matrix and a handful of CI asserts — not more unit tests.

### 14.2 The C++ testability seam

`hodos_tests` links **no CEF**. That is deliberate and worth keeping — it is what makes the suite
fast and hermetic. The consequence is a hard rule:

> **Anything that takes or returns a `CefV8Value` / `CefRefPtr<…>` is unreachable from unit tests.**
> If logic inside such a function is worth testing, the testable seam is the **data layer beneath the
> CEF types** (JSON in / JSON out, string in / string out), and that layer has to be *extracted to its
> own header* to be reachable.

Concrete debt this surfaces:
- **`jsonToV8` (`IdentityHandler.cpp`)** — now carries the int64-widening rule and the
  "nested values stay `.dump()` strings" contract that `identity.get` / wallet-info depend on. Both
  are exactly the kind of thing a future refactor breaks silently. Extract the JSON-shape decisions
  into a testable helper, or accept it stays untested and say so.
- **`HistoryV8Handler` argument marshalling** — the optional-parameter defaults (`limit` 50,
  `offset` 0, frecency `limit` 6) moved from the old synchronous body into the new IPC path by hand.
  Nothing enforces they stayed the same.

### 14.3 The gap this work actually exposed: **Vitest is still 0, and that is now load-bearing**

Making `history.*` async introduced a **stale-response race** (a fast typist lands a newer omnibox
query while an older one is in flight; without a guard, stale results overwrite fresh ones). That is:
- invisible to C++ unit tests (it is hook logic),
- invisible to Rust tests,
- and only *intermittently* visible to Playwright.

It is a textbook thin-Vitest test — mock the bridge, fire two overlapping queries, assert the older
result is discarded. **We have no layer to put it in.** §5's "inverted pyramid is acceptable because
logic lives in Rust" holds right up until the client starts doing async coordination, which it now
does (wallet bridge promises, chunked responses, and now history). Treat this as the first concrete
repayment case for the thin Vitest layer, not a hypothetical.

### 14.4 Smoke matrix owed by this workstream — write the test before the change

- **history-over-IPC (landed `83fe472`)** — ✅ **DONE on Windows 2026-08-04**, against the CEF 150
  build. Results and method in §14.6 below. **Still owed on macOS**, where this path was previously
  dead and is expected to start working.
- **Sandbox ON (2b)** — the whole point is that renderers lose capabilities, so smoke exactly what a
  sandboxed renderer touches: overlays render and take keyboard input; file dialogs open; downloads
  write; adblock + farbling still inject; the wallet bridge still round-trips. A renderer crash-loop
  on startup is the expected failure shape.
- **Bootstrap migration (commit 1)** — dev app launches unsigned (exe + DLL + `chrome_elf` all
  unsigned ⇒ passes CEF's check); subprocesses spawn; `--profile=` still reaches renderers.

### 14.5 Two CI asserts this workstream should leave behind

Neither is a unit test; both are cheap and both fail *loudly* at build time instead of on a user's
machine:
1. **Cert-thumbprint equality** — after the Windows signing step, assert `HodosBrowser.exe`,
   `HodosBrowser.dll` and `chrome_elf.dll` share one primary thumbprint. CEF's bootstrap `LOG(FATAL)`s
   at launch if they don't, so without this the failure mode is "shipped build won't start."
2. **Staging manifest completeness** — add `HodosBrowser.dll` to `build-release.ps1`'s
   `$requiredFiles`. The bootstrap model makes a missing DLL fatal rather than degraded.

And the standing one, unchanged: a **real N-1 → N update apply test** before promote
(`feedback_update_stability_principle`) — the `{app}` file manifest changes shape in this migration,
and a partial apply that replaces the exe but not the DLL now hard-FATALs where it used to survive.

### 14.6 The 2a history-over-IPC smoke — method and results (2026-08-04, Windows, CEF 150)

**Result: the 2a transport is sound.** Every `history.*` method works over the new browser-process
IPC path, the history page and omnibox both drive it correctly, and all three mutations persist
across a restart. Two side findings below are *not* 2a regressions.

#### Method — worth reusing, this is our first real UI smoke harness

Driven over the **Chrome DevTools Protocol**, which turns "click through the UI" into something
scriptable and repeatable:

1. The dev build already opens a CDP port — `9222 + profileOffset`, **+100 under `HODOS_DEV=1`**
   (`cef_browser_shell.cpp`, near `settings.remote_debugging_port`). So dev Default = **9322**.
2. `http://127.0.0.1:9322/json` lists targets; each page target has a `webSocketDebuggerUrl`.
3. Node 22+ has a global `WebSocket`, so a ~40-line script is enough to drive
   `Runtime.evaluate` with `awaitPromise: true` — which is exactly what a promise-returning
   `window.hodosBrowser.history.*` needs. No Puppeteer, no dependencies.

> ⛔ **The near-miss that matters more than the harness.** Production Default is **9222**, dev
> Default is **9322** — one digit apart. The first connection in this smoke landed on **9222**, i.e.
> the *installed production browser* with the user's real session, one step before running
> `clearAll()` against real browsing history. **Always confirm the owning process before driving a
> CDP port**, e.g. `Get-NetTCPConnection -LocalPort <p> | ... Get-CimInstance Win32_Process` and
> check `ExecutablePath`. A second cheap confirmation: `/json/version` reports the engine —
> `Chrome/136.x` is production, `Chrome/150.0.7871.187` is the new dev build.

Destructive steps were made safe by copying `HodosHistory`, `-wal` and `-shm` out of the profile
first, running the full matrix including `clearAll()`, then restoring. Verified restored: 634 → 634.

#### API contract (confirmed by reading `HistoryV8Handler`, after a positional-args test misled)

All parameterised methods take an **options object**, not positional args. A positional call is not
an error — it silently falls through to the defaults, which looks exactly like "the limit argument
is ignored". That cost a false-positive bug report during this smoke.

| Method | Signature | Default |
|---|---|---|
| `get` | `get({limit, offset})` | limit 50, offset 0 |
| `search` | `search({search, limit, offset, startTime, endTime})` | limit 50, offset 0 |
| `searchWithFrecency` | `searchWithFrecency({query, limit})` | limit 6 |
| `delete` | `delete(urlString)` | — |
| `clearRange` | `clearRange({startTime, endTime})` | — |
| `clearAll` | `clearAll()` | — |

#### Results

| Check | Result |
|---|---|
| `get` returns a Promise resolving to an array | ✅ |
| `get` default / `limit` honoured / `offset` paginates with no overlap | ✅ 50 / 5,3 / no overlap |
| `search` limit honoured, all results relevant | ✅ |
| `searchWithFrecency` default 6, limit honoured, `frecencyScore` present | ✅ |
| History page renders through IPC | ✅ "625 entries • Showing 1-20", count matched the API exactly |
| History page pagination | ✅ pages 1→2→3 changed both the range label and the rows |
| Omnibox history suggestions appear **and rank above** Google Suggest | ✅ |
| `delete` removes the URL | ✅ 634 → 633 |
| `clearRange` scoped, leaves out-of-range data | ✅ 633 → 625 |
| `clearAll` empties | ✅ 625 → 0 |
| delete + clearRange persist across restart | ✅ 625 after restart, victim still gone |

#### Two side findings — neither is a 2a regression

1. **Omnibox cold-start race (new observation).** The *first* thing typed after launch shows
   "No suggestions". `omnibox_update_query` was delivered at `12:39:57.604`, but the overlay's React
   app did not finish loading until `12:39:58.01` — the query arrives ~400 ms before a listener
   exists and is dropped. Typing again, with the overlay warm, works. Worth a ticket: either replay
   the last query on overlay ready, or hold the query until the overlay signals `allSystemsReady`.
2. **`clearRange` leaves stale `last_visit_time` (pre-existing).** After clearing a range, three
   URLs still reported a `visitTime` inside the cleared window — all with high visit counts (18, 33,
   15), i.e. URLs with visits both inside and outside the range. It deletes visit rows without
   recomputing the URL's last-visit time. **`git show --stat 83fe472` confirms 2a never touched
   `HistoryManager.cpp`**, so this predates the IPC move and is a separate data-consistency ticket.

> Note on timestamps: Chromium-epoch microseconds are ~1.34e16, **larger than `2^53`**, and they
> cross this IPC as **doubles**. They are therefore not exactly representable. It did not cause the
> finding above (the survivors were far from the boundary, not off-by-a-few-microseconds), but any
> future exact-boundary time logic on this path should not assume integer precision.

### 14.7 ⚠️ Security finding surfaced by this smoke — CDP is open in production

Not a testing issue; recorded here because the smoke is what exposed it.

- `cef_browser_shell.cpp` sets `settings.remote_debugging_port = 9222` for the Default profile
  (`9222 + N` for others) **unconditionally in production builds**. It is hardcoded, not a setting,
  and there is no off switch short of picker mode.
- `simple_app.cpp :: OnBeforeCommandLineProcessing` additionally appends
  `--remote-allow-origins=*` **unconditionally**, which removes the Origin check on the DevTools
  WebSocket.

CDP has **no authentication**. Anything that can reach that port can enumerate tabs, read page
content, execute arbitrary JS in any origin — *including the internal `127.0.0.1:5137` wallet and
overlay pages* — and read cookies. For a browser that custodies a BSV wallet and auto-approves
payments up to a configured cap, that is a material exposure, and `--remote-allow-origins=*`
widens who can attempt the WebSocket upgrade beyond local tooling.

**Not changed here** — turning it off would break the DevTools workflow and is an owner decision.
Suggested shape: keep the port under `HODOS_DEV=1` only, or gate it behind an explicit setting that
defaults to off in release, and drop `--remote-allow-origins=*` from production builds.
