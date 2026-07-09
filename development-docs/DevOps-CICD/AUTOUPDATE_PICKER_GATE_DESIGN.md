# Silent Auto-Update — Picker vs. Sole-Instance Gate (bug #3)

**Status:** design (found by the Stage-2 real-build test, 2026-07-03). Fixes the last of the
three real-build-test findings. See [`SILENT_UPDATE_TEST_PLAN.md`](./SILENT_UPDATE_TEST_PLAN.md)
and [`AUTOUPDATE_6B_SUPERVISOR_DESIGN.md`](./AUTOUPDATE_6B_SUPERVISOR_DESIGN.md).

## Problem (observed live)

On a launch that shows the profile **picker** (multi-profile + picker-on), the silent update
**defers and never applies**:

```
Silent apply: eligible staged build 40199 — Phase A
Silent apply: not sole instance (count=6) — defer
```

The bootstrap's sole-instance gate (`MaybeApplyStagedUpdate`, cef_browser_shell.cpp) calls
`SU_CountSelfBrowsers({app})` and requires the count to be exactly 1. But the picker is a live
`HodosBrowser.exe` **process tree** — 1 picker main + its CEF subprocesses (renderer/GPU/utility,
all the same exe at `{app}\HodosBrowser.exe`) = ~6. So the profile process that boots after the
user picks sees 6 and defers. For a picker-on user this can repeat every launch → the update
effectively never lands.

## Root cause

`SU_CountSelfBrowsers` counts **processes** (`{app}\HodosBrowser.exe` image), which is a poor
proxy for the gate's real intent. The gate exists to answer one question: **"is there another
live browser using a profile (with an open wallet / DBs) that swapping `{app}` would disrupt?"**
Two kinds of process inflate the count but are NOT that:
- **CEF subprocesses** — renderers/GPU/utility, same exe, no wallet/profile of their own.
- **The picker** — a transient chooser that owns **no profile and no lock** (simple_handler.cpp
  ~3241: "no DBs/lock were taken") and closes itself (`PostMessage(WM_CLOSE)`) the instant it
  spawns the chosen profile (it never waits on that child).

## ⚠️ REVISED AFTER ADVERSARIAL REVIEW (2026-07-03) — lock-probe REJECTED

Two independent skeptic reviews rejected the profile-lock-probe below as **unsafe**:

- **Cross-profile pre-lock race (HIGH).** The apply gate runs at the pre-CEF seam
  (~cef_browser_shell.cpp:4415); a real browser acquires its `profile.lock` only ~60 lines
  later (~:4475). So a **concurrently-booting** browser of a DIFFERENT profile holds no lock
  during its whole `[resolve → :4475]` startup → the lock-probe reads "not live" → the
  updater proceeds and its V3-2 step (`:4010`) shuts down the **shared machine-wide wallet**
  (31301/31401 — one backend for all profiles) out from under that booting browser. The
  existing process-count gate (counts every `{app}\HodosBrowser.exe`) DOES catch this; the
  lock-probe would REINTRODUCE the race. Trading a defer-annoyance for a wallet-disruption
  race is a bad trade. (The helper's image-unlock wait only backstops the binary SWAP, not
  the earlier bootstrap-side wallet shutdown + DB snapshot.)
- **Fail-open bucketing** (only `ERROR_SHARING_VIOLATION`→live; everything else→"not live")
  and **narrow-holder (`CreateFileA`/CP_ACP) vs wide-probe (`CreateFileW`/UTF-8) path
  divergence** for non-ASCII usernames — both push toward false "not live" → over-approve.

**→ Revised Fix A: a bounded WAIT on the existing process count (below), not a lock probe.**
The picker is TRANSIENT (it `PostMessage(WM_CLOSE)`s itself the moment it spawns the profile
and its whole CEF tree dies within seconds); a real browser is NOT. So keep the coarse,
safe `SU_CountSelfBrowsers` and simply **poll it briefly**: if the count settles to 1
(everything else — i.e. the picker — exited) we are genuinely sole → apply; if it stays >1
(a real browser, booting or up, is live) → defer. This waits the picker out WITHOUT any
process-classification/lock/cmdline probe, so it cannot over-approve a concurrently-booting
browser (that browser is a live `{app}\HodosBrowser.exe` → keeps the count >1 → defer). The
only cost is a bounded startup delay (~8 s cap, and it exits early the instant the count
hits 1 — so the picker case typically costs ~2–4 s) on the rare boot where an update is
staged AND another browser is live. Fix D (below) covers the residual "picker too slow to
die" / "browser always open" long tail.

## ~~Fix A — gate on held profile-locks, not process count~~ (SUPERSEDED — see revision above)

Each real profile-browser holds an **exclusive** `<profileDataDir>\profile.lock`
(`ProfileLock.cpp`: `CreateFileA(GENERIC_WRITE, dwShareMode=0, CREATE_ALWAYS,
FILE_FLAG_DELETE_ON_CLOSE)`). The picker holds none; CEF subprocesses hold none. So "is any
OTHER browser live?" is answered precisely by **probing every profile's lock**:

```
SU_AnyProfileBrowserLive():
  root = %APPDATA%\<ns>                       // EnvUtf8_(APPDATA) + GetAppDirName()
  for each p in ProfileManager::GetAllProfiles():
      lock = root\<p.path>\profile.lock
      h = CreateFileW(lock, GENERIC_READ, SHARE_READ|WRITE|DELETE, OPEN_EXISTING)
      if h == INVALID:
          if GetLastError()==ERROR_SHARING_VIOLATION -> return true   // HELD => a browser is live
          // ERROR_FILE_NOT_FOUND (DELETE_ON_CLOSE removed it) / path-missing => not live
      else CloseHandle(h)   // opened => not exclusively held => not live
  return false
```

Gate becomes: `if (SU_AnyProfileBrowserLive()) defer;` (replacing `selfCount != 1`).

**Why the probe is correct + non-disruptive:**
- The holder opened with `dwShareMode=0`, so ANY concurrent open fails with
  `ERROR_SHARING_VIOLATION` → unambiguous "held". A *failed* open is a no-op for the holder
  (it does not truncate, delete, or block it). MUST use `OPEN_EXISTING` (never `CREATE_ALWAYS`)
  so the probe can't create/replace the lock.
- When not held, `FILE_FLAG_DELETE_ON_CLOSE` has removed the file → `ERROR_FILE_NOT_FOUND` →
  "not live". A never-launched profile has no lock file → "not live".
- The **applying** instance has NOT taken its own lock yet (the apply runs at the pre-CEF seam,
  BEFORE the SingleInstance/profile-lock step), so it never counts itself. Another instance of
  the SAME profile (which would hold the lock) correctly reads as live → defer.

**Why it's deadlock-free (the lifecycle):**
1. Picker spawns the profile instance (`LaunchWithProfile`) then immediately `PostMessage(g_hwnd,
   WM_CLOSE)` — it does NOT wait for the child. So there is no picker→profile wait to deadlock.
2. Profile instance boots → gate sees no OTHER held lock (picker holds none) → applies → spawns
   helper → `_exit(0)`.
3. The picker is meanwhile tearing down (CEF shutdown of its subprocess tree). Those processes
   still map `{app}\HodosBrowser.exe`, so the **helper's image-unlock wait** (transaction.cpp
   step 4 / §V3-12: "death != unlocked") naturally blocks until the picker tree is fully gone,
   THEN installs. No process is waiting on any other in a cycle.

This makes A strictly better than the alternative "let the picker itself apply" (Option B):
B would have to guess a profile for the health-probe (the user hasn't picked yet) and, after
commit, would leave the user in that guessed profile instead of their chosen one — a UX
regression. A keeps the apply in the profile instance, so the health-probe and the post-update
window are the user's actual profile.

## Fix D — chronic-deferral safety valve (never silently stall)

A is complete for the picker. But apply-on-cold-boot inherently defers whenever *any* real
browser is live, so a user who never fully closes the browser could defer indefinitely — that
violates the standing principle "updates must never silently stall". D adds a bounded counter:

- `update-state.json` gains `deferralStreak` (int).
- Each **transient** defer (sole-instance OR wallet-alive) increments it; a successful apply,
  a rollback, or staging a NEW build resets it to 0.
- When `deferralStreak >= DEFER_NOTIFY_THRESHOLD` (proposed 10), the build **degrades to notify**
  — the same escape hatch signer-continuity already uses: stop trying to silent-apply and let
  the normal notify/WinSparkle path offer "update ready — restart to install." (Wiring the
  notify path to surface a silently-staged build is the one piece that may land as a small
  follow-up if it's non-trivial; the counter + degrade decision land here.)

D never uses `lastFailureBuild` (that permanently rejects a build); a deferral is transient, so
the streak is a separate, resettable field.

## Invariants preserved
- **I2** (never overwrite a running browser): the helper's image-unlock wait is unchanged and
  remains the hard backstop; A only changes the *early-out heuristic*, not the actual swap gate.
- The wallet-dead gate (V3-2) is unchanged and still runs after this gate.

## Risks / review targets
- **R1** ProfileManager availability + profile-path shape at the pre-CEF seam (it's already used
  by the startup resolver just above the apply, so GetAllProfiles() is valid).
- **R2** Lock-probe encoding for non-ASCII usernames (use wide `CreateFileW` + `EnvUtf8_`).
- **R3** TOCTOU: a browser could launch between the probe and the helper install — covered by the
  helper image-unlock wait (backstop), same as today.
- **R4** A profile whose data dir doesn't exist yet (never launched) — probe returns "not live".
- **R5** Does any real browser fail to hold `profile.lock` (so it'd be missed)? Confirm every
  non-picker launch path acquires it before opening DBs.
- **R6** D's reset points must cover every "progress" transition so the streak can't wedge on.

## Test plan
- Rebuild both shells; re-run the Stage-2 real-build test with the **picker ON** and 3 profiles
  (the exact scenario that failed) → the happy path must now COMMIT (no "not sole instance").
- Negative: launch a second profile's browser, keep it open, stage an update, launch the target
  profile → must DEFER (a real other browser IS live), proving A doesn't over-approve.
- D: force `deferralStreak` past the threshold in the rig → assert degrade-to-notify.

---

# v2 (2026-07-09) — EXACT picker-exit wait via an inherited process handle

**Status:** design → **adversarial review BEFORE code** (money-path + process-lifecycle).
**Supersedes** the "~8 s fixed cap" of the Revised Fix A above — same safe *strategy*
(wait for the count to settle; never classify processes), but the wait is now **exact**.

## Why the shipped fix (Revised Fix A) still defers on a slow machine

Revised Fix A landed as `cef_browser_shell.cpp:4079-4089`: a fixed **`i < 16` (~8 s)**
poll of `SU_CountSelfBrowsers` that defers unless the count settles to exactly 1. Confirmed
live 2026-07-09 (beta.23 client applying build 30024) — the CN signer-gate fix reached
"Phase A", but:

```
06:20:51 pick "Default" → picker spawns --profile child + starts ShutdownApplication
06:20:51 [child] Silent apply: eligible staged build 30024 — Phase A
06:20:54 [picker] ShutdownApplication complete — waiting for browsers to close
06:21:00 Silent apply: not sole instance (count=8) after wait — defer   ← 8 s cap expired
```

The picker's ~8-process CEF tree takes **> 8 s** to fully tear down on the owner's Win10 box,
so the child's fixed cap expires while count is still 8 → **defer → never applies**. Proven by
launching the installed exe **directly** with `--profile="Default"` (no picker): count=1 →
beta.24 applied silently. So the picker path was *always* blocked on this machine; only
direct/non-picker launches ever silent-updated.

Bumping the cap is a guess (how slow is slow enough?). The exact fix removes the guess.

## The fix — the picker hands the child a wait target for its own death

**Insight:** the child can't safely *classify* which live `HodosBrowser.exe` is "the transient
picker" (the prior review REJECTED lock/cmdline probes — they misclassify a concurrently-BOOTING
real browser, and V3-2 would then kill its shared 31301/31401 wallet). But the **picker knows it
is the picker** — so let the picker *self-identify* by handing the child an inheritable **handle
to the picker's own process**. The child `WaitForSingleObject`s that handle (the picker WILL exit —
it `PostMessage(WM_CLOSE)`s itself the instant it spawns the child), THEN runs the **unchanged
count==1 settle loop** to mop up the picker's fast-dying subprocess tree.

This mirrors the existing **bootstrap→helper** `bootstrap-handle` pattern
(`cef_browser_shell.cpp:4285-4287` dup a SYNCHRONIZE+inheritable self-handle;
`SU_SpawnHelper:3899` passes it via `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`; the helper
`WaitForSingleObject`s it — comment at `:4284` "PID-reuse-immune wait target").

### Safety invariant preserved (the whole point)

The picker-handle wait is a **refinement of the wait, NOT a replacement of the gate**. After the
picker handle signals, we **still require `SU_CountSelfBrowsers == 1`**. So:
- A concurrently-booting/​running **real** browser keeps the count > 1 → **defer** (unchanged).
  We never over-approve, so V3-2 never kills a live browser's shared wallet.
- The handle only lets us wait out the *one* transient process we can prove is transient
  (the picker told us so) instead of blind-polling a fixed cap.
- A **forged** `--picker-handle` on a non-picker launch can only make us *wait* (or fail the
  wait and fall through) — it can never skip the count gate. No safety impact; worst case a
  self-inflicted local delay.

Non-picker `--profile=` launches (taskbar pin with a baked-in profile, `win_run.ps1`, the update
helper's health probe) pass **no** `--picker-handle` → **no wait** → today's behavior exactly.

### Only the *picker* passes the handle — not the in-browser profile switch

`profiles_switch` → `LaunchWithProfile` is called from **two** places:
1. the **pre-window picker** (`g_picker_mode == true`) — transient, dies right after spawn ✅ pass handle.
2. the **in-browser profile panel** (`g_picker_mode == false`) — the parent browser **stays alive**.
   If we passed *its* handle the child would wait the full cap for a process that never exits, then
   defer anyway (count > 1, parent alive). ❌ do **not** pass the handle → child defers immediately
   via the count gate (correct, no wasted delay).

So handle-passing is gated on `g_picker_mode` at the call site (`simple_handler.cpp:3302`).

## Code changes (Windows only)

### 1. `ProfileManager::LaunchWithProfile` — add `bool linkParentExitHandle = false`

Windows branch (`ProfileManager.cpp:533-563`): when `linkParentExitHandle`, duplicate an
**inheritable, SYNCHRONIZE-only** handle to our own process, append `--picker-handle <value>`
to the child command line, and restrict inheritance to **exactly that one handle** via
`STARTUPINFOEXW` + `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` (identical guard to `SU_SpawnHelper`, so
we never leak *other* inheritable handles into the child) + `bInheritHandles=TRUE` +
`EXTENDED_STARTUPINFO_PRESENT`. **Best-effort:** any failure in dup / attribute-list setup →
fall back to the plain `CreateProcessW` (no handle, no `--picker-handle` arg) so a profile
launch can never fail because of this. Close the parent's handle copy after spawn (the child
inherited its own reference). mac/`__APPLE__` branch: **no change** — Launch Services (`open -n
-a`) can't inherit Win32 handles, and the picker-defer bug is Windows-only (mac silent update is
Sparkle install-on-quit, not this count gate).

Caller `simple_handler.cpp:3302`: `LaunchWithProfile(id, /*linkParentExitHandle=*/g_picker_mode)`.

### 2. `MaybeApplyStagedUpdate` — wait on the handle before the settle loop

New static helper `SU_ParsePickerHandle()` parses `--picker-handle N` from `GetCommandLineW()`
(via `CommandLineToArgvW`, mirroring the `--post-update-health-probe` parse at `:4457`), returns
`nullptr` when absent/zero. Just before the count loop (`:4079`):

```cpp
// bug#3 v2: the transient picker (if it spawned us) handed us a handle to its own process.
// Wait for it to fully EXIT first (self-identified — no classification), so the only residual
// the count loop below must settle is its fast-dying CEF subprocess tree. A forged/bogus handle
// just fails/expires the wait and falls through; the count==1 gate below still governs, so this
// can never over-approve a concurrently-booting real browser.
if (HANDLE pickerH = SU_ParsePickerHandle()) {
    LOG_INFO("Silent apply: spawned by picker — waiting for it to exit before sole-instance check");
    bool waitFailed = false;
    for (int i = 0; i < 40 && !g_update_abort.load(); ++i) {          // ~20s cap; picker self-closes
        DWORD wr = WaitForSingleObject(pickerH, 500);
        if (wr == WAIT_OBJECT_0) { LOG_INFO("Silent apply: picker exited"); break; }
        if (wr == WAIT_FAILED)   { LOG_WARNING("Silent apply: picker-handle wait failed — proceeding");
                                   waitFailed = true; break; }
    }
    if (!waitFailed) CloseHandle(pickerH);   // skip on WAIT_FAILED (forged/garbage value)
}
int selfCount = SU_CountSelfBrowsers(appDirW);
for (int i = 0; selfCount > 1 && i < 30 && !g_update_abort.load(); ++i) {  // ~15s residual (was ~8s)
    Sleep(500);
    selfCount = SU_CountSelfBrowsers(appDirW);
}
if (selfCount != 1) { /* defer, unchanged */ }
```

**Caps:** picker-exit ~20 s (`40 × 500 ms`) — generous margin over the observed >8 s teardown, but
exits early the instant the picker dies (typical ~3-5 s). Residual settle bumped ~8 s→~15 s
(`16→30`) — the cheap interim improvement, and it also helps the *no-handle* fallback path.
Worst-case added boot delay ~35 s, and **only** on a cold boot that (a) has an eligible staged
update AND (b) a picker/real-browser is live — otherwise the gate returns before this code. All
loops honor `g_update_abort`.

## Adversarial-review targets (v2)

- **AV1** Handle inheritance correctness: does the child receive the **same numeric handle value**
  the picker printed? (Yes — `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` inheritance preserves the value.)
  Confirm the dup is of `GetCurrentProcess()` (a pseudo-handle) into a real inheritable handle
  first (can't inherit the pseudo-handle directly) — matches bootstrap `:4286`.
- **AV2** Handle-list restriction actually prevents leaking *other* inheritable handles when
  `bInheritHandles=TRUE` (that's the reason `SU_SpawnHelper` uses the attribute list). Verify the
  `STARTUPINFOEXW`/`EXTENDED_STARTUPINFO_PRESENT`/`cb` wiring is exact.
- **AV3** Over-approval: prove that after the handle signals, a live **real** browser still forces
  a defer (count > 1). This is the safety linchpin (V3-2 wallet kill). Walk the concurrently-booting
  case explicitly.
- **AV4** Forged `--picker-handle`: (a) `WAIT_FAILED` path; (b) `CloseHandle` on a forged non-zero
  value — is skipping-on-`WAIT_FAILED` sufficient, or can a *valid-looking* forged handle be closed
  (closing one of our own handles early)? Local-only, non-security, but weigh a stricter guard.
- **AV5** Does the picker reliably reach process exit after `PostMessage(g_hwnd, WM_CLOSE)` so the
  handle is guaranteed to signal (not hang forever)? (If it hangs, we hit the 20 s cap → count
  loop → defer — safe, but confirm no *worse* outcome.)
- **AV6** Best-effort fallback: every failure branch in change #1 must still launch the profile
  (plain `CreateProcessW`), never leave the user with nothing (the G-1 principle at
  `simple_handler.cpp:3316`).
- **AV7** Boot-delay UX: is ~35 s worst-case acceptable? Is there a window where the user sees
  nothing (picker gone, new window not yet up)? (Same as today; the apply splash is up only later
  at `:4216`.) Consider whether the picker-exit cap should be tighter.
- **AV8** Interaction with Fix D (chronic-deferral valve) and `g_update_abort` semantics.

## Adversarial-review OUTCOME (2026-07-09) — two independent reviewers, both SHIP-WITH-CHANGES

Both converged; the design's safety linchpin (the handle wait is *additive* — the unchanged
`count==1` gate at `:4084` and the V3-2 wallet-dead gate at `:4092` still govern, so it can never
over-approve a concurrently-booting real browser) was independently confirmed sound. Adopted changes:

- **[HIGH — bug in the new code] Close the picker handle ONLY on `WAIT_OBJECT_0`.** A forged/stray
  `--picker-handle N` whose value collides with a *valid, non-signaled* handle the child already
  owns (the `update.lock` owner handle from `:4048`, or `g_instance_mutex` from `:4497`) returns
  `WAIT_TIMEOUT` every iteration → loop exhausts with `waitFailed==false` → the original
  `if(!waitFailed) CloseHandle` would close **our own** live handle. Fix: only `CloseHandle` on the
  proven-real death path (`WAIT_OBJECT_0`); leak on timeout/failure (one `SYNCHRONIZE` handle for
  process lifetime is harmless — `_exit(0)` on apply, OS reclaims on defer). **Additionally** dup
  with `SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION` and gate the whole wait on
  `GetProcessId(pickerH) != 0` so a non-process forged value is ignored outright (defeats the
  signaled-event collision too).
- **[HIGH — UX + self-inflicted race] Neutral pre-commit splash across the wait.** The picker window
  is hidden (`cef_browser_shell.cpp:1492`, before `ShutdownApplication`) and the child is at the
  pre-CEF seam with no window, and the committed apply splash only comes up at `:4216` (AFTER this
  gate) — so the user stares at **nothing** for the whole wait (~5-7 s typical, up to ~35 s
  pathological). A user who thinks it hung re-double-clicks the taskbar pin (which bakes `--profile`)
  → the 2nd launch fails `lock.AcquireWithRetry` (first child holds `update.lock`) but *succeeds*
  `TryAcquireInstance` (first child hasn't taken the profile pipe yet — that's at `:4554`, after the
  gate) → boots a real browser + wallet backend → first child's post-wait count is now >1 → **defer,
  apply lost for this boot**. Safe (never over-approves) but v2 *widens* the window. Fix: raise a
  **neutral** `UpdateSplash` ("Hodos is starting…", NOT "updating" — we may still defer and fall
  through to normal startup) right before the picker wait, and close it before the committed
  `applySplash` at `:4216`. Managed via `std::unique_ptr` (auto-closes on every `return false`;
  explicit `.reset()` before `applySplash` so the two never overlap — avoids scoping it around the
  `instH` guard that must live to the helper spawn at `:4298`).
- **[VALIDATION — why the fix actually works, not just is safe] CEF shutdown ordering.** The
  `--picker-handle` targets the picker's **browser (main) process** (`profiles_switch` IPC runs in
  the browser process; `GetCurrentProcess()` there is that process). In CEF/Chromium graceful
  shutdown the browser process runs `CefShutdown`, which signals **and waits for** the
  renderer/GPU/utility subprocess hosts, THEN the browser process exits **last**. So
  `WaitForSingleObject(pickerH)` blocks through the multi-second *bulk* of the teardown and by the
  time it signals the subprocesses are already reaped — the residual `count==1` loop only mops a
  ~0-2 s tail. The observed log maps cleanly: `06:20:54 "ShutdownApplication complete"` is that
  function *returning* (`:795`), not process exit; the browser process then runs `CefShutdown` +
  cleanup and exits *after* `06:21:00` — exactly the window the old 8 s cap expired inside.
- **[MUST — impl] Best-effort + exact handle-list wiring.** Every failure branch in
  `LaunchWithProfile` (dup / `InitializeProcThreadAttributeList` / `UpdateProcThreadAttribute` /
  extended `CreateProcessW`) must fall through to the plain, unmodified `CreateProcessW` (no handle),
  never leaving the user with no browser (the G-1 principle at `simple_handler.cpp:3316`). Mirror
  `SU_SpawnHelper` exactly: `STARTUPINFOEXW.cb = sizeof(STARTUPINFOEXW)`, one
  `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` entry = `{pickerSelf}`, `bInheritHandles=TRUE`,
  `EXTENDED_STARTUPINFO_PRESENT` (the handle-list is what confines inheritance to the ONE handle
  instead of leaking every inheritable handle the ~8-proc picker holds).
- **[CAPS] picker-exit ~30 s (`60 × 500 ms`, early-exits the instant the picker dies), residual
  8 s→15 s (`16→30`).** Splash-backed, so a generous picker cap is nearly free on the common path and
  covers a slow spinning-disk/AV Win10 box; residual 15 s comfortably covers the subprocess tail.
- **[ROLLOUT — document] One-build lag.** The gate runs on the *installed* build to apply the *next*
  one, so a client on a pre-fix build still defers on the picker path — every picker user needs ONE
  notify/manual hop onto a fix-containing build, THEN silent resumes (same shape as the
  beta.22→23 signer-gate bridge). Confirm the standard WinSparkle notify path delivers that hop
  before broad rollout (Fix D degrade-to-notify surfacing is a separate, pre-existing item).

## v2 runtime test steps (OWNER, on hardware — cannot run from a headless shell)

The apply gate is inert in dev (`IsDevEnv() && !HODOS_UPDATE_TEST`) and the automated apply rigs
exercise the *helper transaction*, not the picker→child sole-instance path — so these are manual,
real-build, multi-profile tests (the beta.24→beta.25 silent-resume proof). Grab
`%APPDATA%\HodosBrowser\logs\debug_output.log` after each.

1. **Picker happy-path (the fix).** Multi-profile + picker ON, a build staged. Cold-launch (no
   args) → pick a profile. **Expect:** child logs `spawned by picker — waiting for it to exit`,
   then `picker exited`, then `count=1` → apply commits → splash → relaunch on the new build. The
   neutral "Hodos is starting…" splash should be visible during the wait (no blank screen).
2. **`--profile=` bypass still works.** Launch the installed exe directly `--profile="Default"`
   (no picker). **Expect:** no `--picker-handle`, no picker wait, count=1 → apply (unchanged path).
3. **Negative — real other browser stays live (no over-approve).** Open profile A, keep it up;
   stage a build; launch profile B (via picker or `--profile=B`). **Expect:** after the picker
   wait, `SU_CountSelfBrowsers` is still >1 (A alive) → `not sole instance … defer`. Prove V3-2
   wallet shutdown never runs while A is live.
4. **Non-update launch unaffected.** No staged build: picker + pick → `MaybeApplyStagedUpdate`
   returns before the wait (no eligible build) → instant profile open, no splash, no delay.
5. **Mojibake.** On the committed apply splash, confirm "Hodos is updating…" / "…please don't
   power off." render correctly (no `â€¦` / `â€"`).

## Mojibake fix (folded in, cosmetic)

`update-helper/splash.h` is `#include`d by **both** targets. `hodos-update-helper` compiles with
`/utf-8` (`CMakeLists.txt:714`) → its splash renders correctly; `HodosBrowserShell` does **not**,
so it misreads the UTF-8 source bytes of the non-ASCII glyphs when building even the *wide*
literals at `splash.h:69` (`…`) and `:77` (`—`, `'`) → mojibake on the shell-side apply splash.
**Fix (targeted, zero blast radius):** replace those three glyphs in the wide literals with
charset-independent universal-character-name escapes — `…` (…), `—` (—), `’` (') —
which produce the correct UTF-16 code unit regardless of the compiling target's source-charset,
so both the shell and the helper render correctly with no CMake flag change. (Rejected alt: add
`/utf-8` to the whole `HodosBrowserShell` target — correct but a broad flag change to the largest
target for a cosmetic bug.)
