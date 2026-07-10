# F5 — robust single-instance handoff (reopen vs a dying instance) — DESIGN (2026-07-08)

**Status: IMPLEMENTED (v2) + FINAL ADVERSARIAL REVIEW = SHIP (2026-07-08). Built exit 0, LOCAL/unpushed.**
Final review (code-auditor, on the written code) verdict **SHIP** — all 7 attack vectors clean; the
DB-corruption invariant is preserved (two live instances still impossible; pipe frees strictly AFTER
the profile lock on every path); decouple/reorder/retry-bump/picker-no-op/idempotency all correct.
- **S1 (pre-existing bug, landed in this change):** the "Profile Locked" `return 1` (`cef_browser_shell.cpp`
  ~4600) ran after `StartListenerThread` but skipped `StopListenerThread` → joinable `std::thread` global
  → `std::terminate()` at static destruction (crash right after the dialog). Fixed by calling
  `StopListenerThread()` before `return 1`. No DB risk (lock never acquired).
- **H1 (accepted, out of scope):** a sub-millisecond TOCTOU where the listener frees the pipe name
  between serving one client and re-creating the next instance — same class as the accepted gap-(a)
  µs race; only a persistent dedicated listening instance would fully close it.
- **H2 (optional, deferred):** add a `WaitNamedPipeA` before the StopListenerThread self-connect for
  defense-in-depth (practically unreachable with a single listening instance).

Original design (below) kept for the reasoning trail.

---

**Status: design + adversarial review BEFORE any code (owner instruction — this touches the
single-instance gate that prevents profile-DB corruption).**

## The mechanism today (already fairly robust)
Single-instance is a **named-pipe** gate per profile (`SingleInstance.cpp`), NOT a mutex:
- `TryAcquireInstance(profileId)` = `CreateNamedPipe(... FILE_FLAG_FIRST_PIPE_INSTANCE)`. First
  process wins; others get ERROR_ACCESS_DENIED/PIPE_BUSY → "not first".
- First instance runs `StartListenerThread` — a loop that accepts client connections and, per
  message, either PostMessages `WM_SINGLE_INSTANCE_NEW_WINDOW` to `g_hwnd` and replies `"ok"`,
  or replies `"not_ready"` (window not up) / `"shutting_down"` (g_shutting_down set).
- A second launch runs `SendToRunningInstance` — connects, sends `new_window:<url>`, reads the
  reply, and **retries up to 10×** (1s apart). Crucially, **each retry first calls
  `TryAcquireInstance` again** — so once the old process releases the pipe (fully exits), the new
  process becomes the first instance and `SendToRunningInstance` returns `false` → caller
  continues normal startup. `"shutting_down"`/`"not_ready"` → keep retrying.
- Pipe name is released by the OS when the last handle closes (i.e., when the old process exits or
  `StopListenerThread` closes its handles) — so a crashed old process frees the gate automatically.

**This already handles the common dying-instance cases:** old fully exited (fresh start), or old
mid-shutdown with the flag set (`"shutting_down"` → retry → take-over). `SetShuttingDown()` is
called at the top of `ShutdownApplication` (`cef_browser_shell.cpp:537`).

## The residual bug (mechanism 5) — a narrow "ok-but-dropped" window
Shutdown funnels through `WM_CLOSE` (last-window branch, `cef_browser_shell.cpp:1472-1484`):
`g_app_shutting_down = true` (1482) → `ShowWindow(SW_HIDE)` (1483) → `ShutdownApplication()` (1484)
→ … → `SingleInstance::SetShuttingDown()` (537).

Between `WM_CLOSE` arriving (1472) and line 537, `g_shutting_down` is **still false** and `g_hwnd`
is still a valid window. If a reopen's `SendToRunningInstance` connects in that span, the listener
thread (separate thread) takes the normal path: PostMessage `WM_SINGLE_INSTANCE_NEW_WINDOW` +
reply `"ok"`. But the UI thread is now synchronously executing the WM_CLOSE→ShutdownApplication
chain and will **never process that posted message** → the forwarded command is dropped and the new
process already exited on `"ok"`. Result: reopen appears to "do nothing" / the old window vanishes.
On slow Win10 this span is wider (context switches between 1472 and 537), which is why it surfaces
there and not on fast Win11.

## Proposed fix — minimal, no gate rewrite
**Primary (closes the window):** call `SingleInstance::SetShuttingDown()` at the *very first line*
of the last-window teardown branch — right next to `g_app_shutting_down = true` (line 1482),
BEFORE `ShowWindow`/`ShutdownApplication`. From that instant the listener replies `"shutting_down"`
to any reopen, which the existing retry+`TryAcquireInstance` loop turns into a clean take-over once
the old process exits. The existing `SetShuttingDown()` at 537 stays (belt-and-suspenders /
covers any other ShutdownApplication caller). Scope: ONLY the `windowCount <= 1` (full-shutdown)
branch — the multi-window branches (1485+, 1507+) must NOT set it (the app isn't exiting).

**Secondary (optional, slow-teardown tail):** bump `SendToRunningInstance` retries from 10 to ~20.
Each `"shutting_down"` retry is ~1s (WaitNamedPipe returns fast while the pipe exists), so 10 ≈ 10s
of takeover tolerance; 20 ≈ 20s. This avoids the tail case where a >10s teardown exhausts retries
→ falls through to `AcquireProfileLock` → "profile in use" error dialog. Pure count change; no
logic change. (Skip if we want zero risk this pass.)

## Why this is low-risk
- No change to `TryAcquireInstance` / the pipe-creation atomicity — the actual DB-corruption
  safeguard (only one process can hold the profile pipe + `AcquireProfileLock`) is untouched.
- `SetShuttingDown()` only flips `g_shutting_down` earlier within a teardown that is already
  irreversibly committed (WM_CLOSE last-window). It cannot leave a *live* process falsely marked
  shutting-down (the flag is only ever set on the committed-exit path).
- Worst case if the primary fix mis-fires: identical to today (reopen retries, or falls through to
  the profile-lock gate) — never two live instances on one profile (that's still gated by
  `FILE_FLAG_FIRST_PIPE_INSTANCE` + `AcquireProfileLock`).

## Explicitly NOT doing
- No "block the reopen until the old process fully exits" (the risky version I first floated) — the
  existing retry+takeover already achieves that outcome without new blocking logic.
- No touching the multi-window WM_CLOSE branches.
- macOS unaffected (single-instance is NSApplication-delegate based; these are Windows `#ifdef`).

## REVISED DESIGN v2 (after adversarial review — 2026-07-08)
The review (verdict SHIP-WITH-CHANGES) found a **second, worse residual the v1 design missed**, and
my follow-up read found the review's proposed fix needs one more correction. Net revised plan:

### Gap (a) — the "ok-but-dropped" window: primary fix is correct but NARROWS, not closes
`SetShuttingDown()` at line 1482 is verified **correct/complete/safe** (it's the sole `g_app_shutting_down`
site; every real shutdown funnels through the `windowCount<=1` WM_CLOSE branch; both message-loop
exits are guarded by `g_app_shutting_down`; `IsShuttingDown()` has zero UI-thread readers; idempotent
under re-entry; picker-mode no-op). It shrinks the drop to a ~microsecond TOCTOU (a client already
past `SingleInstance.cpp:167` before 1482 runs). Truly closing gap (a) needs the listener to reply
`"ok"` only after the UI thread confirms creation (`SendMessage` not `PostMessage`) — a bigger rewrite
we are NOT doing. Keep the primary fix; correct the wording (narrows, not closes).

### Gap (b) — pipe gate frees ~seconds before the profile lock (the DANGEROUS one, was missed)
`StopListenerThread()` runs at `cef_browser_shell.cpp:780` (early in `ShutdownApplication`, releasing
the pipe name) but `ReleaseProfileLock()` runs at `:5281` (end of main() cleanup, AFTER the whole
browser-close drain + SQLite checkpoint/close). In that multi-second gap a reopen's `TryAcquireInstance`
**succeeds** (pipe gone) → it becomes "new instance" → `AcquireProfileLock` (only `6×500ms=3s` retry,
`ProfileLock.cpp:17-40`) can FAIL if the old teardown tail > 3s → hard **"Profile Locked" dialog**
(`:4585-4591`). On slow Win10 with a big history/bookmark DB this is reachable — the exact target class.
**The lock CANNOT be released earlier** — R2/R3 (`:5265-5269`) deliberately holds it until the DBs are
checkpointed+closed to avoid a relaunch winning the lock while old still holds live-WAL handles. So the
**pipe gate must stay held until the lock releases**, i.e., the listener must keep serving until ~5281.

**Correction to the review's fix:** just moving `StopListenerThread()` later does NOT keep the pipe
alive, because the listener loop **self-terminates on `g_shutting_down`** — guard `while(!g_shutting_down)`
(`:97`) + breaks at `:133`/`:151`. With the flag set at 1482, the next client connect unblocks
`ConnectNamedPipe`, the `:133` check sees the flag → `break` → all instances closed → pipe name freed
early anyway (and the client gets NO reply → empty → retries → TryAcquire succeeds → gap (b) again).

**Correct fix — decouple "advertise shutting-down" from "terminate the loop":**
1. Add a separate `g_listener_stop` atomic. Listener loop guard becomes `while(!g_listener_stop)`; the
   `:133`/`:151` `break`s check `g_listener_stop` (not `g_shutting_down`). The `:167` `"shutting_down"`
   reply stays and is NOW actually reached while `g_shutting_down` is set → reopens get `"shutting_down"`
   → retry (the existing loop), instead of the listener breaking.
2. `StopListenerThread()` sets `g_listener_stop` (its self-connect + join + pipe-close unchanged).
3. **Move `StopListenerThread()` from `:780` to `main()` cleanup, immediately AFTER `ReleaseProfileLock()`
   (`:5281`).** Order matters: release the lock FIRST, then the pipe — so when the reopen's next-retry
   `TryAcquireInstance` finally succeeds, the profile lock is already free → `AcquireProfileLock`
   succeeds → clean take-over with no "Profile Locked" dialog and no dropped reopen.
4. Secondary: bump `SendToRunningInstance` retries 10→20 — now meaningful (covers the full teardown incl.
   lock release; each `"shutting_down"` retry is ~1s). Downside: a genuinely-hung old process = a silent
   windowless reopen for up to ~20s before takeover/lock-error. Acceptable; pipes die with the process.

### Risk delta of v2 vs v1
v2 touches the listener loop's stop semantics (moderate, well-contained in `SingleInstance.cpp`) — more
than v1's one-liner, but it's the only correct way to hold the gate until the lock frees given R2/R3.
The DB-corruption safeguard is UNCHANGED: two live instances are still impossible
(`FILE_FLAG_FIRST_PIPE_INSTANCE` + `AcquireProfileLock`), and the pipe now frees strictly AFTER the lock,
never before. Worst-case mis-fire = today's behavior (retry / fall through to the lock gate).

### Doc corrections applied
Primary fix NARROWS gap (a) (µs TOCTOU), not "closes." The retry-exhaustion fallback is `AcquireProfileLock`'s
3s retry, not an immediate hard fail.

## Test plan
- **Unit-ish / manual:** rapid close→reopen on the same profile (fast + artificially-slowed
  teardown) → new instance takes over and shows a window; never a dropped reopen; never a spurious
  "profile locked" dialog.
- **Adversarial focus for review:** (a) is there ANY teardown path that reaches the message-loop
  stop WITHOUT going through the WM_CLOSE last-window branch (so the early flag is missed)? (b) can
  the early flag ever be set in a process that then KEEPS running (false shutting-down)? (c) does
  setting the flag before `ShowWindow`/browser-close change any teardown ordering assumption? (d)
  double-shutdown re-entrancy interaction with the flag.
