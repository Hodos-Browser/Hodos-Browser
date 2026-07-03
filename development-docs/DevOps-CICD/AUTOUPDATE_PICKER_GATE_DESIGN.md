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
