# Phase 0 — stray writes in the install root · PHASE CONTRACT

**Workstream:** WS1b(a) · **Ticket:** `../TICKET_stray_log_in_install_root.md` · **Status:** 🟡 VERIFIED ON DEV — release-shaped rows owed
**Opened:** 2026-08-18 (retrofitted to the harness) · **Platforms:** Windows (3 of 52 writes are also macOS)
**Standard:** `../HARNESS.md`.
**Amended:** 2026-08-18 — `P0-S1` NOT REPRODUCED; §1/§2 corrected; three scope changes recorded in §5a.

---

## 1. Goal

The installed application directory holds no file the browser writes at runtime, and no wallet HTTP
response body is ever written to disk outside the level-gated log directory.

> ⚠️ **Amended 2026-08-18, after `P0-S1` ran.** This goal originally read *"…no wallet HTTP response
> body — ~~**including the BIP39 recovery phrase**~~ — is ever written to disk"*. The struck clause is
> **refuted by measurement** (§4a): the phrase never reaches these sinks, because the live
> wallet-creation path does not traverse `WalletService`. Struck rather than deleted — this is the
> **second** obvious-but-wrong justification this ticket has produced, and the next reader will
> arrive at it too.
>
> **What survives is still real, still shipping, and still worth the phase**: wallet *financial* data
> — 2,551 balance response bodies in a single production session, plus full `/transaction/send`
> request bodies — written into the install root unconditionally, outside every logging control,
> never rotated, and copied into the rollback backup.

## 2. Done means

- [x] `{app}` contains **zero** runtime-written files after a full session — measured, not inspected once *(measured on a dev build, CWD-equivalent; see §4b)*
- [x] All 52 relative-path writes gone: `debug_output.log` (44) + `startup_log.txt` (8)
- [x] No wallet HTTP **response body** reaches any sink; anything kept is routed through `Logger`
- [ ] `{app}\debug.log` stops being recreated across **≥10** launches (ordering-dependent) — A3's own source confirmed gone (614 → 0); a **release-shaped** run still owed, see §4c
- [x] T0 gate `G1` baseline driven **52 → 0**, and `G5` (response bodies reaching a sink) **15 → 0**
- [x] A1 decided and implemented; A2's abort recorded in update state and raised above `LOG_WARNING`

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-UPDATE` | Update still applies and still rolls back | Touches `UpdateFs.cpp` (A1) and the apply abort path (A2) |
| `R-INTEXT` | Internal never prompts, external always gates | Not touched — but 19 of the writes are inside `WalletService`'s money-path methods, so the diff sits on that code |
| `R-GOLD` | Gold pill fires | Not touched. Recorded so the boundary run is not skipped |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P0-S1` | ⛔ **Reproduction first.** Create a wallet; the phrase does **not** appear on disk outside the log dir | *Before* the fix the same run **must find it**. If it does not, this row's premise is wrong and §1 changes | Release-shaped build + 4 months of dev builds, browser process. `WalletService_mac.cpp` has zero `ofstream` — Windows only | T2 | 🔴 **NOT REPRODUCED** — §4a. Closed by owner decision 2026-08-18 |
| `P0-A1` | Full session → **no** new file in the browser's CWD | 🔴 **observed**: shipped beta.2, live, `{app}\debug_output.log` grew **1,896,960 → 2,118,968 B** between 14:15 and 15:26 on 2026-08-18 while the owner browsed | ⚠️ **Dev build, CWD = `cef-native/build/bin/Release`** — see §4b. The mechanism is CWD-relative, so this exercises the same code; the literal `%LOCALAPPDATA%\HodosBrowser\` path still wants an installed build | T3 | ✅ **GREEN** — both files **absent** after a live session carrying 21 wallet HTTP calls (≈84 writes under the old code) |
| `P0-A2` | The same events **do** appear under `%APPDATA%\…\logs\` | Suppress the `Logger` call → they vanish there too | Both halves. **A silent no-op looks identical to a fix** | T3 | ✅ **GREEN** — all 7 re-routed message families present in `HodosBrowserDev\logs\debug_output.log`, §4b |
| `P0-A3` | `preflight.ps1` gate `G1` passes at baseline **0** (from 52) | Add one `ofstream("foo.log")` → gate **exits non-zero** | `preflight.ps1 -NegativeControl` proves it, not a code read | T0 | ✅ **GREEN** — `0 violations, at baseline`. 🔴 re-observed at the **new** baseline: `1 > 0` |
| `P0-A8` | `preflight.ps1` gate `G5` passes at baseline **0** (from 15) — no wallet response body reaches **any** sink | Route one body through `Logger` instead of `ofstream` → `G5` still catches it | `G1` alone is insufficient: it only sees `ofstream`. `G5` guards the *shape* | T0 | ✅ **GREEN** — `0 violations`. 🔴 observed `1 > 0` |
| `P0-A4` | `{app}\debug.log` absent across ≥10 launches | 🔴 **observed**: present on the shipped build — 351 B, contents are only 3× `TabManager initialized` | ⚠️ Ordering-dependent. **One clean launch is not evidence.** ⛔ SUBJECT **corrected** — must be a **release-shaped** run; a dev build has a second, dev-only writer (§4c) | T3 | 🟡 **A3 VERIFIED** — TabManager lines **614 → 0** across the pre/post files. File still created **in dev only**, by a different source. Release-shaped ≥10-launch run ⬜ OWED |
| `P0-A5` | `[InstallDelete]` clears the files on upgrade | Plant them, run an upgrade install → all gone | Already implemented. **Verify, do not rebuild** | T2 | 🟡 **CODE VERIFIED** — `hodos-browser.iss:93-105` lists **four** files (not three) plus `{app}\*.log` on uninstall. Upgrade-install run ⬜ OWED |
| `P0-A6` | A1: a file held open for write no longer aborts the backup | Hold a `.log` open without `FILE_SHARE_WRITE`, run the walk → **pre-fix** it must abort | `BuildManifestForTree` return value, not the log line | T1 | ✅ **GREEN** — `Manifest.OpenLogAbortsWholeWalkUnlessVolatileExcluded`. 🔴 The RED is **inside the test**: the same run asserts the pre-fix call returns false before the green half |
| `P0-A7` | A2: an abort records its reason in update state and is visible | Force an abort with the recording removed → nothing observable | `update-state.json` — ⚠️ **not** `lastFailureBuild`/`lastFailureReason`; see §5a.2 | T1 | ✅ **GREEN** — `UpdateState.AbortFieldsDefaultEmptyAndParseFromAnOlderFile` + `UpdateState.AbortRecordDoesNotBlockTheStagedBuild`, whose RED half asserts the wrong-field variant **does** block the build |

**Pairing:** `P0-A1` and `P0-A2` are two-sided — "gone from `{app}`" and "still present in `logs\`".
Either alone is satisfied by deleting logging outright, which is the failure mode to avoid.

### 4a. `P0-S1` — the reproduction, and its result

**Result: NOT REPRODUCED.** The premise fails, and it fails on the second of its two parts.

**(a) The sink is real.** Positive control, so the instrument is not what failed: `WalletService.cpp:224`
produced **13,484** verbatim `Response: {…}` lines in a single dev log (e.g.
`Response: {"balance":22701367,"bsvPrice":11.87}`). `/wallet/create`'s body is ~200 chars, well inside
the old 500-char cap — **so if it reached this sink it would be written in full.**

**(b) `/wallet/create` never reaches it.** Census of the unambiguous entry marker
(`makeHttpRequest:`, `WalletService.cpp:130`) across **115 MB of dev stray logs spanning 2026-04-20 →
2026-08-17**, plus the live production beta.2 install:

| Endpoint | dev (5 files, 115 MB) | production (beta.2) |
|---|---|---|
| `GET /wallet/balance` | 13,643 | 2,551 |
| `POST /wallet/address/generate` | 4 | — |
| `POST /transaction/send` | 3 | — |
| **`POST /wallet/create`** | **0** | **0** |

Corroborating: `mnemonic` across every stray log, the 1.63 GB production `Logger` log and the 168 MB
dev `Logger` log → **41 hits, zero phrase values** (all are the literal word in a UI log line, a
DuckDuckGo dictionary URL, or an x.com page title). During that same window the wallet's mnemonic/PIN
overlay step fired **≥22 times**, so the UI path was exercised repeatedly.

**Why, from source.** The live path is `WalletPanelPage.tsx:359 → walletFetch('/wallet/create') →
wallet_call IPC → HandleIpcWalletCall → runIpcCallDirect → SyncHttpClient`, which has **no body sink**.
`WalletService::createWallet()` is reachable only via the `create_wallet` IPC
(`simple_handler.cpp:3726`), and both of its frontend callers are dead: `useWallet.ts` is imported by
nothing, and `App.tsx:108` sits inside a commented-out block.

⛔ **Not run:** a fresh live wallet creation in a release-shaped build. The dev wallet DB is *not*
per-profile, so forcing `/wallet/create` means destroying the owner's dev wallet. Owner accepted the
four-month census as sufficient and closed the row (2026-08-18).

⭐ **Consequence for the sprint:** `SPRINT_PLAN.md` §6.5 (disclosure posture) says it *"depends on
Phase 0's reproduction result"*. For Phase 0 there is **no evidence of key-material disclosure**, so
no disclosure decision is owed on this defect. Phase 0.5's remains open.

### 4b. The live run — 2026-08-18, dev build at `ceca3ad`

Dev browser launched from `cef-native/build/bin/Release` (`HODOS_DEV=1`), against the dev wallet on
**31401** and the Vite server on **5137**. The three stray files were moved aside to `*.prePhase0`
first, so anything appearing is unambiguously new. Session ran until the wallet hot path had logged
**21** `makeHttpRequest:` calls — under the old code that is ~84 open/write/close cycles into CWD.

| File | Before | After a live session |
|---|---|---|
| `debug_output.log` | 13,248,873 B | **absent** ✅ |
| `startup_log.txt` | 712,613 B | **absent** ✅ |

And the other half of the pair — the same events, now in `HodosBrowserDev\logs\debug_output.log`:

```
OnContextInitialized entered   2      makeHttpRequest:        5
Header setup: visible=         2      Getting total balance   5
After SetAsChild               2
Tab setup: g_hwnd=             1      Initial tab creation result   1
```

```
[BROWSER] [INFO]  🚀 OnContextInitialized entered - g_header_hwnd=00000000001C05E8 (IsWindow=1) g_hwnd=0000000000340258 (IsWindow=1)
[BROWSER] [DEBUG] 📊 Header setup: visible=1 rect=1910x96
[BROWSER] [DEBUG] 📊 Tab setup: g_hwnd=0000000000340258 IsWindow=1 tabHeight=936
```

⭐ **Both halves matter.** Deleting the logging outright would satisfy the first table and fail the
second; that is the failure mode `P0-A1`/`P0-A2` were paired to catch.

⚠️ **What this run does and does not prove.** The write is `ofstream("debug_output.log")` — a
*relative* path resolved against the process CWD — so a dev run exercises the identical code with
CWD = the build dir instead of `{app}`. It proves **the browser no longer creates these files**. It
does **not** prove anything installer-specific: `P0-A5`'s cleanup of files left by *prior* installs
still needs an upgrade install.

### 4c. ⚠️ Finding — `{app}\debug.log` has a SECOND source, and the ticket's claim was too strong

The ticket states TabManager's line was *"empirically the **only** message that ever reaches
`{app}\debug.log`"*. That is true of the **production** file (351 B = 3 lines) but **false in a dev
build**. Sources in the pre-change 381 KB dev `debug.log`:

| Source | Lines |
|---|---|
| `ChildProcessLogSink.cpp:57` | 1,452 |
| `TabManager.cpp:33` | 404 |
| `TabManager.cpp:34` | 210 |

After Phase 0: **TabManager 614 → 0** (A3 confirmed), `ChildProcessLogSink` remains. It is the same
defect class — `SimpleRenderProcessHandler`'s constructor logs `[RENDER] [DEBUG]` before the *render*
process has logging configured, so Chromium's `LOG()` defaults to `debug.log` in CWD, exactly the
pre-`CefInitialize` window A3 is about.

⭐ **It does not ship.** That arm is gated on `g_verbose`, set only by the `--hodos-render-verbose`
switch, which `SimpleApp::OnBeforeChildProcessLaunch` (`simple_app.cpp:91`) appends **only when
`hodos::IsDevEnv()`**. That is why production's `debug.log` contains no renderer lines.

⇒ Left alone: §6 already scopes the other raw `LOG()` calls out, and `ChildProcessLogSink.cpp` is one
of the four files the ticket names in its residual-risk list. But `P0-A4`'s SUBJECT is corrected — it
can only be judged on a **release-shaped** build. Residual risk, noted not swept: anyone passing
`--hodos-render-verbose` to a *production* build would recreate `{app}\debug.log`.

## 5. Blast radius

- `WalletService.cpp` (19 writes) — inside `makeHttpRequest`, `readResponse`, `createTransaction`,
  `signTransaction`, `broadcastTransaction`, `getBalance`, `getTransactionHistory`, `sendTransaction`.
  These are the **only** diagnostics in some of those functions: re-routed, not simply deleted.
- `simple_app.cpp` (17 + 8) — includes `CreateBRC100AuthOverlay*` and `InjectHodosBrowserAPI`.
- `AddressHandler.cpp` (2), `my_overlay_render_handler.cpp/.mm` (3 + 3 — **macOS ships these**).
- `UpdateFs.cpp` / `UpdateFs.h` — A1 changes what the backup walk tolerates.
- `UpdateApply.cpp` / `UpdateApply.h` — A2's two new `UpdateState` fields.
- `TabManager.cpp:33` — A3.
- ⚠️ `cef_browser_shell.cpp:4632` falls back to the **relative** literal `"debug.log"` when
  `AppPaths::GetLogDir()` is empty — a latent second source of the same file. **Left in place**; it is
  a fallback that only fires when the log dir cannot be resolved at all, and `G1`'s pattern does not
  reach it (it is a `CefString` assignment, not a file sink). Noted for Phase 2.

### 5a. Scope changes — amended in the same commit that changed the scope

1. **`std::cerr` response-body writes removed too.** §2 claims *no* wallet response body reaches *any*
   sink. Two `std::cerr << "Response body: " << responseBody` lines sat one line from the `ofstream`
   ones. They currently write to `NUL` — `freopen_s` to the log path **always** fails with `EACCES`
   and falls back to `NUL` (`cef_browser_shell.cpp:4577-4600`, documented and observed) — so they are
   inert *today*, but they are a latent disk sink the moment that redirect ever succeeds. Removed, so
   §2 is literally true rather than true-by-accident.
2. **A2 uses two NEW `UpdateState` fields, not the two the contract named.** The contract said to reuse
   `lastFailureBuild`/`lastFailureReason`. That would have been a **defect**: the skip gate
   (`cef_browser_shell.cpp:4063`) permanently skips any staged build whose number equals
   `lastFailureBuild`, so recording a *transient* I/O abort there would mean one AV scanner holding one
   file open permanently blocks a good update — the opposite of this phase's intent, and the existing
   `rejectPersistent` comment already warns *"NOT for transient defers"*. Added `lastAbortReason` +
   `lastAbortCount` instead, deliberately **not** consulted by the skip gate. `UpdateState` is a JSON
   sidecar, not the wallet DB, so invariant #2 does not apply; older readers ignore the new keys and
   `ParseUpdateState` defaults them, which `P0-A7`'s first test pins.
3. **A claim corrected: `CopyTreeRecursive` does *not* abort on an open log.** The first draft of
   `P0-A6` asserted it failed "for the same reason" as the manifest walk. It does not:
   `std::filesystem::copy_file` opens the source permissively, while only `Sha256FileW` omits
   `FILE_SHARE_WRITE`. The copy-side exclusion is therefore **not** an abort fix — it exists solely so
   a growing/stale log is not carried into the rollback backup and restored over `{app}`. The test now
   asserts that, and is renamed `StaleLogIsNotCarriedIntoTheBackup`.

## 6. Out of scope

- The other **41** raw `LOG()` calls across 4 files. Only `TabManager`'s fires pre-init *today*; that
  is a property of call ordering, not a guarantee. Noted as a hazard, not swept.
- `Logger`'s level gate and rotation — **Phase 2**. ⚠️ Note this phase *moves* traffic into `Logger`,
  which is still ungated, so Phase 2 matters more after this change, not less.
- Extending the F8 gate to cover `ofstream` sinks of secret-shaped data — deliberately left: `G1`
  removes the sinks entirely, so the F8 widening belongs with Phase 2's logging work.

## 7. Rollback

`git revert` the phase's commits. The changes are deletions plus one exclusion policy and two
`UpdateState` fields; nothing is schema-breaking and nothing else depends on them.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed — **7 of 9 complete**; `P0-A4` needs a
      release-shaped ≥10-launch run and `P0-A5` a T2 upgrade install; `P0-S1` closed as NOT REPRODUCED
- [x] `scripts/preflight.ps1` run — recorded below
- [x] `scripts/preflight.ps1 -NegativeControl` — every T0 gate seen to fail
- [ ] `../REGRESSION_SET.md` run in full at the 0 → 0.5 boundary
- [ ] Adversarial review — four questions answered in writing
- [x] `G1` 52 → 0 and `G5` 15 → 0 in `../HARNESS.md` §9
- [x] Commits cite row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight | **PASS** — exit 0, all of G1–G5 + T1a–T1d ran (`-Full`) | 2026-08-18 | assistant |
| preflight -NegativeControl | **PASS** — all 5 gates seen to fail (`1>0`, `6>5`, `1>0`, `1>0`, `1>0`); probes cleaned up | 2026-08-18 | assistant |
| `hodos_tests` | **181 tests, 180 passed, 1 skipped** (`UpdateStagerRig.StagesFromLocalFeed`, pre-existing). 5 new: 3 × A1, 2 × A2 | 2026-08-18 | assistant |
| live dev session (`P0-A1`/`P0-A2`) | **GREEN both halves** — stray files absent, events present in `logs\`; §4b | 2026-08-18 | assistant |
| regression set (0 → 0.5) | ⬜ owed | | |
| adversarial review | ⬜ owed | | |
