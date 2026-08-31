# TICKET — 44 raw `ofstream("debug_output.log")` writes land a log INSIDE `{app}`, the one place the silent updater forbids

**Filed:** 2026-08-17, investigating the "everything bogged down" incident
**Severity:** ~~🚨 **can SILENTLY abort silent auto-update** (confirmed mechanism)~~ → see §0 —
**wallet financial data written into the install root, outside every logging control**
**Status:** 🟡 CODE COMPLETE (Phase 0, 2026-08-18) — T2/T3 rows owed
**Sprint:** 📌 **Phase 9 (release readiness)** — but ⭐ **fold the owed T2/T3 rows into the Phase 3 install session instead.** Both need a REAL install, and Phase 3 changed the installer (`[Icons]` now declare an AUMID), so `P3-A7` already re-owes the "nothing new inside `{app}`" assertion. Testing separately means installing twice.
**Present in:** beta.1 **and beta.2** (`WalletService.cpp` is byte-identical between the two tags)

---

## §0 — What this ticket is actually about (added 2026-08-18)

This ticket has now produced **two** headline justifications, and **both were refuted by
verification**. They are kept, struck, because each is the reading a careful person naturally
arrives at, and the next person will arrive at it too.

| # | Claimed | Verdict |
|---|---|---|
| 1 | The signed `expected-new-manifest.json` check rejects unknown files in `{app}` | ❌ **Refuted** — `VerifyTreeAgainstManifest` iterates only `m.entries`; it never enumerates the directory |
| 2 | The stray log **silently aborts silent auto-update** via the backup walk | ❌ **Refuted** — `MaybeApplyStagedUpdate` (`cef_browser_shell.cpp:4787`) runs **before** `CefInitialize` (`:5277`) and only past a `selfCount == 1` gate (`:4155-4165`) plus a wallet-dead gate (`:4172`). No Hodos process can hold the log open during `BuildManifestForTree` |
| 3 | The **BIP39 recovery phrase** reaches `{app}` in plaintext | ❌ **Refuted by measurement** — `P0-S1`, see the phase contract §4a |

### ⭐ What is actually true, and it is enough

The 52 writes are **live and shipping**, and what they put in the install root is **wallet financial
and privacy data**:

- `{app}\debug_output.log` on the owner's beta.2 install grew **1,896,960 → 2,118,968 bytes between
  14:15 and 15:26 on 2026-08-18** — measured twice, while the browser was simply in use.
- Its contents are **full wallet HTTP response bodies**: 2,551 `/wallet/balance` bodies in that one
  session (exact satoshi balance + BSV/USD price), and via `WalletService.cpp:531/558/585` the
  **entire `/transaction/send` request body** — destination addresses and amounts.
- Written **unconditionally in production**, bypassing level gating, the resolved log directory, and
  any future rotation; **copied into the rollback backup**; and never cleaned up.
- `getBalance` alone did **four** open/write/close cycles per call, synchronously, on the browser UI
  thread — 13,643 calls in one dev log.

On a browser whose product claim is privacy, that is the defect. It does not need the auto-update
story to be worth fixing, and §2's *class* problem (A1) is real independently of our log.

### ⚠️ Corollary for §6.5 of `SPRINT_PLAN.md`

The open **disclosure-posture** decision was scoped as depending on Phase 0's reproduction result.
There is **no key-material disclosure** here, so no disclosure decision is owed for this defect.
Phase 0.5's (a fund-moving endpoint with no approval gate) is untouched by this and remains open.

## The finding

Across five shipped files there are **44** raw writes of the form:

```cpp
std::ofstream debugLog("debug_output.log", std::ios::app);
debugLog << "…" << std::endl;
debugLog.close();
```

| File | count |
|---|---|
| `src/core/WalletService.cpp` | **19** |
| `src/core/AddressHandler.cpp` | ✓ |
| `src/handlers/simple_app.cpp` | ✓ |
| `src/handlers/my_overlay_render_handler.cpp` / `.mm` | ✓ |
| **total** | **44** |

The path is **relative**, so it resolves against the process **current working directory** — which for
an app launched from its installed shortcut is `{app}`.

**Measured on the live install:**

```
%LOCALAPPDATA%\HodosBrowser\debug_output.log     9,865 bytes     Aug 17 15:50
```

Written minutes after a fresh **beta.2** install.

## ⛔ Why this is not merely untidy

`cef-native/CLAUDE.md` states the rule and the reason it exists:

> **Log file location:** `%APPDATA%\HodosBrowser\logs\debug_output.log` … It is deliberately
> **outside** the install root: the browser holds the log open for writing, and **a log inside
> `{app}` broke the silent-update backup hash of the `{app}` tree.**

⚠️ **2026-08-18:** this rule is still right, but note that the *historical* reason it records is the
one §0 row 2 refutes for **today's** code. The rule stands on §0's grounds instead.

We already hit this once and moved the *Logger's* file out of `{app}`. **These 44 raw writes were
never moved with it**, so the exact condition the rule forbids is back — and it is live in a shipped
build.

~~The silent updater verifies the installed tree against the signed `expected-new-manifest.json` …
precisely the kind of drift that check exists to catch.~~

⚠️ **Struck 2026-08-18 — that reasoning was wrong, and the real mechanism is in the next section.**
The signed-manifest check does **not** look for unknown files, so it is not what catches this. Kept
struck rather than deleted because it is the obvious-but-incorrect read, and the next person to
inspect this will arrive at it too.

## ✅ RESOLVED 2026-08-18 — and the answer moves the risk, it does not remove it

**Q: does the signed `expected-new-manifest.json` check reject unknown files in `{app}`?**
**A: No.** `updatefs::VerifyTreeAgainstManifest` (`src/core/UpdateFs.cpp:145`) iterates **only over
`m.entries`**, checking each listed file exists and its sha256 matches. It **never enumerates the
directory**, so a file present in `{app}` but absent from the manifest is never examined. The
post-install integrity gate is therefore **not** the exposure.

⛔ ~~**But the BACKUP side is, and it is worse — it is a silent abort of the whole update.**~~

⚠️ **STRUCK 2026-08-18 — refuted. Everything from here to the end of this section describes a real
mechanism that cannot actually be reached.** The chain below is correct about `Sha256FileW` and
`BuildManifestForTree`; what it misses is *when* the walk runs. See §0 row 2. The code reasoning is
kept because A1 below is a genuine defect of the same shape that **is** reachable by other writers.

`cef_browser_shell.cpp:4302`, immediately before the `{app}` backup:

```cpp
if (!updatefs::BuildManifestForTree(appDirW, oldManifest, {L"update"})) {
    LOG_WARNING("Silent apply: cannot manifest {app} — abort"); return false;
}
```

`BuildManifestForTree` (`UpdateFs.cpp:118`) walks `{app}` **recursively**, excluding only `update\`,
and hashes **every regular file**:

```cpp
const std::string sha = Sha256FileW(p.wstring());
if (sha.empty()) return false;   // unreadable file => fail (don't ship a partial manifest)
```

And `Sha256FileW` (`UpdateFs.cpp:72`) opens with:

```cpp
CreateFileW(path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_DELETE, …)
```

⭐ **`FILE_SHARE_WRITE` is absent.** So hashing **fails on any file another process currently holds
open for writing** — which is exactly what our 44 `ofstream(…, std::ios::app)` calls do, repeatedly:
`getBalance` alone opens/writes/closes it **four times per call**, and the incident window recorded
**417 balance calls in 52 minutes**.

**The resulting chain:** stray log open for write → `Sha256FileW` returns `""` →
`BuildManifestForTree` returns false → **`"Silent apply: cannot manifest {app} — abort"`** → the
silent update **does not happen**.

⚠️ **Failure mode: silent.** It is a `LOG_WARNING` and a `return false`. Nothing surfaces to the
user. They simply stop receiving updates, with no error and no prompt — on the mechanism by which
every future security fix reaches them. That is strictly worse than a loud verification failure, and
it directly contradicts the standing principle that auto-update must never be silently broken.

⭐ **This is precisely the history `cef-native/CLAUDE.md` records** — *"a log inside `{app}` broke the
silent-update backup hash of the `{app}` tree"*. The Logger's file was moved out in response; **the
44 raw writes were never moved with it**, so the same defect is live again.

⚠️ **Timing-dependent, therefore intermittent — do not expect a clean repro.** The writes are
short-lived, so whether the apply aborts depends on whether a write is in flight during the walk. An
intermittent, silent failure to update is the hardest possible thing to notice in the field, which
argues for fixing it rather than measuring how often it bites.

**Second-order effects**, lower severity but real: the growing log is **copied into the rollback
backup** (`CopyTreeRecursive(appDirW, rollbackW, {L"update"})`) and, on rollback, swapped back into
`{app}` — restoring a stale log — and it inflates every backup.

## They also bypass every logging control

These writes never touch `Logger`, so they ignore:

- level gating (the subject of `TICKET_production_debug_logging_unbounded.md`) — they log
  **unconditionally in production**, including full request bodies;
- the resolved log directory (`AppPaths::GetLogDir()`);
- any future rotation or size cap.

And they are **pathologically expensive**: `WalletService::getBalance` alone opens, writes and closes
the file **four times per call** — twice on entry, twice on the result. The incident window recorded
**417 `get_balance` calls in 52 minutes**, i.e. ~1,700 open/write/close cycles on a file in the
install directory, synchronously, on top of the 1.58 GB `%APPDATA%` log.

## What this does and does not explain about the incident

**Established from the logs:**

- Balances were healthy until ~14:31 (`{"balance":85506903,"bsvPrice":15.145}`).
- From **15:18:02** every call returned `{"error":"Failed to fetch total balance"}`, continuously to
  the 15:21 shutdown.
- ⭐ **The BSV price was never the problem** — `bsvPrice: 15.145` was populating fine, and the
  fallback chain (WhatsOnChain → CoinGecko → MEXC, plus SQLite persistence across restarts) is
  intact. The owner's exchange-rate hypothesis is **ruled out by evidence**.
- The failing branch is `makeHttpRequest("GET", "/wallet/balance")` returning no `balance` key —
  a **wallet/indexer** failure, not a price failure.
- `[MAIN] [WARN] freopen failed - stdout: 13, stderr: 13` (errno 13 = `EACCES`) appears repeatedly —
  multiple instances contending for the same log file. Worth pursuing separately: it means
  `std::cout`/`std::cerr` output from those processes went **nowhere**.
- Session scale: **45 tabs across 4+ windows**, ~30,000 log lines in 52 minutes.

**NOT established:** why *web pages* also stopped loading. A wallet-endpoint failure should not stall
`x.com`. The credible mechanism — worth testing, not asserting — is that these synchronous file
writes plus synchronous wallet HTTP run on the browser UI thread, so a slow/failing wallet backend
stalls **everything**, including local DB reads. The uniform **2.0 s** gaps in the log during the
incident look like a timeout, not contention, which supports it.

⛔ **Do not record a cause.** Reproduce it: with a stub that makes `/wallet/balance` hang, confirm
whether page loads stall too. That single experiment settles it.

## Fix

1. **Delete all 44 raw writes.** They are debug scaffolding. Anything worth keeping becomes a
   `LOG_DEBUG_*` call through `Logger`, which resolves the correct directory and will honour the
   level gate.
2. ⛔ **Never write a relative-path file from the browser process.** CWD is not ours to assume —
   it is `{app}` from a shortcut and something else entirely from a jump-list or protocol launch.
3. **Ship a cleanup** removing any `{app}\debug_output.log` left by prior installs.
4. **Add a build-time guard**: fail the build if `ofstream(` + a bare filename appears outside tests.
   A rule this specific, already violated once and re-violated 44 times, needs enforcement rather
   than documentation.

## Two adjacent defects this exposed — fold into the same phase

Both are in the code path above, both are independent of our stray log, and both are cheap while
someone is already in `UpdateFs.cpp`.

### A1 — `Sha256FileW` omits `FILE_SHARE_WRITE`, so ANY writer in `{app}` aborts the update

```cpp
CreateFileW(path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_DELETE, …)
```

Removing our 44 writes fixes *our* instance of this. It does **not** fix the class: any file being
written anywhere under `{app}` at walk time — an AV scanner's temp file, a crash dump, a
partially-written CEF cache artifact, a future feature — aborts the entire silent apply, silently.

⚠️ **This is a genuine judgement call, not an obvious bug.** Strictness here is defensible: hashing
a file while it changes yields a hash that describes nothing, and a backup manifest that silently
records a torn hash is arguably worse than refusing. **Decide deliberately** between:

1. **Keep strict, but be specific** — distinguish "sharing violation" (`ERROR_SHARING_VIOLATION`)
   from "genuinely unreadable" and report which. Today both collapse into `""`.
2. **Add `FILE_SHARE_WRITE`** and accept possibly-torn hashes for files nobody promised are stable.
3. **Exclude by policy** — skip known-volatile paths (`*.log`, `crashpad`, `.tmp`) from the backup
   manifest the way `update\` is already excluded.

⭐ Option 3 is closest to the existing design — the exclusion mechanism already exists and is already
used for exactly this reason.

### A2 — the abort is a `LOG_WARNING`, which is why nobody would ever know

```cpp
LOG_WARNING("Silent apply: cannot manifest {app} — abort"); return false;
```

An update that decides not to happen is not a warning-level event. It is the failure of the
mechanism that delivers every future security fix, and it currently produces one line in a log
nobody reads — in a build where, per
`TICKET_production_debug_logging_unbounded.md`, that log is 1.58 GB of noise.

⚠️ **Fixing only the log level is not enough** — nothing surfaces to the user or to us either way.
Minimum bar: raise to `LOG_ERROR`, record the abort **and its reason** in the update state so a
subsequent launch can see that the last apply refused and why, and make repeated aborts visible.
⛔ Do not silently retry forever with no record; that is the current behaviour and it is why this
defect could have run indefinitely.

## A3 — the SECOND stray file, `{app}\debug.log`, is CEF's — investigated 2026-08-18

⚠️ **Deleting the 44 writes does not remove this one.** `{app}` holds two volatile files:

```
debug_output.log   869,638 bytes   (ours — the 44 writes; growing on beta.2)
debug.log              351 bytes   (CEF's default log name)
```

### What it is

`debug.log` is Chromium's **default** log destination — used when `LOG()` is called before logging
has been configured, written to the **current working directory**, which for a shortcut launch is
`{app}`. We never call `SetCurrentDirectory` in the browser, so CWD is whatever the launcher gave us.

**Its entire contents are one message:**

```
3 × "TabManager initialized"      ← src/core/TabManager.cpp:33, a raw LOG(INFO)
```

⭐ **It is ordering-dependent, not reliably early**: the same message appears **3×** in
`{app}\debug.log` and **2×** in the correctly-configured `logs\cef_debug.log`. So `TabManager`'s
singleton is sometimes constructed before `CefInitialize` and sometimes after. That intermittency is
why it was never noticed.

### ⛔ "Set the log path earlier" is NOT feasible

`CefSettings.log_file` is applied **inside `CefInitialize`** (`cef_browser_shell.cpp:4631` sets it;
`:5277` initialises). Chromium's `LOG()` lazily initialises logging on first use and defaults to
`debug.log` in the CWD. **No public CEF API configures browser-process logging before
`CefInitialize`** — `CefExecuteProcess` runs earlier but does not set it. So the window cannot be
closed from the logging side.

### ✅ The fix is to remove the thing that logs in the window

1. ⭐ **Remove or re-route the `LOG(INFO)` in `TabManager::TabManager()`.** It is a singleton
   breadcrumb of near-zero diagnostic value, and it is empirically the **only** message that ever
   reaches `debug.log`. One line.
2. **Keep `debug.log` in the backup exclusion** regardless — for installs that already carry the
   file, and as belt-and-braces if another early logger ever appears.

### ⛔ Considered and REJECTED: setting the process CWD

One `SetCurrentDirectoryW(logDir)` at startup would neutralise `debug.log` **and** all 44 relative-path
writes at once, and it is tempting for exactly that reason.

**Rejected because it hides the defect instead of fixing it.** A future relative-path write would
silently land in the log directory and nobody would ever learn it was wrong — whereas the **build
guard** already in this ticket fails it loudly at compile time. It would also change relative-path
resolution and child-process inheritance process-wide, which is a far bigger behavioural change than
it appears. *(The update-helper does set CWD — `update-helper/main.cpp:99` — but that is a
short-lived single-purpose process, not the browser.)*

### ⚠️ CORRECTED 2026-08-18 by running it — "the only message" was true of PRODUCTION only

Measured on the dev build after A3 landed. The pre-change dev `debug.log` (381 KB) had **three**
sources, not one:

| Source | Lines |
|---|---|
| `ChildProcessLogSink.cpp:57` | 1,452 |
| `TabManager.cpp:33` | 404 |
| `TabManager.cpp:34` | 210 |

After A3: **TabManager 614 → 0** (the fix works), `ChildProcessLogSink` remains. Same defect class,
one process over — `SimpleRenderProcessHandler`'s constructor logs before the *render* process has
logging configured, so Chromium's `LOG()` falls back to `debug.log` in CWD.

⭐ **It does not ship.** That arm is gated on `g_verbose`, set only by `--hodos-render-verbose`, which
`SimpleApp::OnBeforeChildProcessLaunch` (`simple_app.cpp:91`) appends **only when `IsDevEnv()`** —
which is exactly why the production file held 351 bytes of TabManager and nothing else. So the claim
was right about production and too strong as written. Left unswept, per the note below; the
consequence is that **A3's acceptance can only be judged on a release-shaped build.**

### Residual risk to note, not to sweep

There are **41** raw `LOG()` calls across 4 files (`TabManager.cpp`, `simple_app.cpp`,
`simple_handler.cpp`, `ChildProcessLogSink.cpp`). Only TabManager's fires pre-init **today** — but
that is a property of current call ordering, not a guarantee. ⚠️ Do not sweep them all as part of
this ticket; note the hazard, and if a new message ever appears in `{app}\debug.log`, this is why.

## ⛔ Negative control

- With the guard in place, deliberately add one `ofstream("foo.log")` and confirm the build **fails**.
- After the fix, run a full wallet session and confirm **no** file is created in `{app}` — and that
  the same events **do** appear in `%APPDATA%\…\logs\` (proving logging was moved, not deleted; a
  silent no-op looks identical to a fix).

## Acceptance

- [x] all **52** raw writes removed (44 `debug_output.log` + 8 `startup_log.txt` — the original count
      of 44 missed `startup_log.txt` entirely); equivalents routed through `Logger`
- [x] build guard rejects bare-filename `ofstream` in shipped code, **verified by making it fail**
      (`preflight.ps1` gate `G1`, negative control observed `1 > 0`)
- [ ] `{app}` stays clean across a full session — measured *(T3, owed)*
- [x] cleanup ships for existing installs *(verified already present: `hodos-browser.iss:93-105`,
      four files + `{app}\*.log` on uninstall; the upgrade-install **run** is still owed)*
- [x] ~~answered: does the signed-manifest check reject unknown files in `{app}`?~~ **No** — resolved
      2026-08-18; the exposure is the backup walk, not the integrity gate (see above)
- [x] **A1** decided and implemented: **option 3 (exclude by policy)**. `IsVolatileArtifact` skips
      `*.log`, `*.tmp`, `crashpad\`, and `startup_log.txt`, in **both** `BuildManifestForTree` and
      `CopyTreeRecursive`. ⚠️ Note the copy side is *not* an abort fix — see the phase contract §5a.3
- [x] **A2**: raised to `LOG_ERROR`, reason + consecutive count recorded in `update-state.json`,
      cleared once a backup succeeds. ⚠️ Recorded in **new** `lastAbortReason`/`lastAbortCount`
      fields, **not** `lastFailureBuild` — that one permanently blocks the build (contract §5a.2)
- [ ] **A3**: `TabManager`'s raw `LOG(INFO)` removed (**done** — measured 614 → 0 lines on a live dev
      run); `debug.log` excluded from the backup (**done**, via `IsVolatileArtifact`);
      verified `{app}\debug.log` stops being recreated across several launches (it is
      ordering-dependent, so **one clean launch is not evidence** — check repeatedly)
