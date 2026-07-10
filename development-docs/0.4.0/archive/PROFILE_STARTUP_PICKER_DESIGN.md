# Profile Startup + Pre-Window Picker — Design (0.4.0)

**Created:** 2026-06-19 · **Status:** Approved shape; implementation in progress · **Branch:** `0.4.0`
**Folds:** CHUNK 1 (new-window-loses-profile) + R5 (registry-write hardening) + R7 (validate `--profile` / coherent fallback) + new feature: Chrome-style pre-window profile picker.

> Produced via the per-chunk harness: kickoff (cited-code re-verified against current source) → bounded external research (Chromium/Firefox profile-launch UX, sourced) → adversarial design review (architecture-fit + regression-risk + CEF-lifecycle skeptic, grounded in source). All design decisions below were ratified by the user on 2026-06-19.

---

## 1. The original bug, correctly diagnosed

**Symptom:** opening a new window from a non-default profile comes up in **Default** (its NTP tiles show Default's history).

**Mis-diagnosis in the boot memory:** "Ctrl+N / new-window relaunches the exe and falls back to Default — thread the profile through." **False against current source.** Verified:
- Ctrl+N (`simple_handler.cpp:7110`), open-link-in-new-window (`:7549`), tab tear-off (`:1862`), session restore (`simple_app.cpp:318`), and the single-instance forward (`cef_browser_shell.cpp:1348`, `WM_SINGLE_INSTANCE_NEW_WINDOW`) all create windows **in-process** via `CreateFullWindow()` — they inherit the active profile and cannot emit a "Profile parsed: …" startup line.
- The **only** in-app exe relaunch is `ProfileManager::LaunchWithProfile` (profile switch), and it **already** passes `--profile=<id>` correctly.

**Real root cause:** a **no-argument launch** (taskbar / desktop / Start shortcut — natural given the per-profile AUMID grouping at `:3227`) hits `cef_browser_shell.cpp:3221-3223`, which falls back to `GetDefaultProfileId()` (= `defaultProfileId_`, normally `"Default"`), **ignoring** the `lastUsedProfile` that `Load()` already read into `currentProfileId_` (`ProfileManager.cpp:117`). So a no-arg launch always opens Default.

We do **not** have per-profile shortcuts/jumplists carrying `--profile=` (grep: zero `IShellLink`/`ICustomDestinationList`/`--profile-directory`). The AUMID grouping (`:3235`) is therefore a half-feature.

---

## 2. Approved behavior

Startup resolves into **three modes**:

| Condition | Behavior |
|-----------|----------|
| `--profile=X` present | Validate X (**R7**). Acquire X's pipe + lock, init X's 4 DBs, open shell. |
| no-arg, **exactly 1** profile | Become that profile directly. No picker. |
| no-arg, **>1** profile, picker enabled (**default: ON when >1**) | **Picker mode** (see §4). |
| no-arg, **>1** profile, picker disabled | Open the **default (starred) profile**. |

**No "last-used" concept (decided 2026-06-19).** Earlier drafts fell back to the last-used profile when the picker was off; the user removed it as cruft. With the picker on (the default), the user picks every cold start, so last-used is never consulted; when the picker is off, a deterministic **default** beats a surprising "whatever I poked at yesterday." `SetCurrentProfileId` is set in-memory only at startup (no registry rewrite — R5). The persisted `lastUsedProfile` field is now vestigial/informational.

**R7 coherent fallback:** an unknown/garbage `--profile` id falls back to the **default** profile **coherently** — `currentProfileId_` *and* the data-dir agree — **before** SQLite init. The default fallback is itself guarded: if `defaultProfileId` names a deleted profile, drop to the first real profile. Never let the UI think it's profile X while the data dir is something else.

---

## 3. R5 — registry-write hardening (decided)

- **Delete the every-boot `Save()`**: startup used to call `SetCurrentProfileId()` which unconditionally `Save()`d on every launch. `SetCurrentProfileId(id, persist)` now takes a flag and startup passes `persist=false` (in-memory only) — no boot rewrite. (Since last-used was dropped, nothing persists at startup at all.)
- **Cross-process lock** (named mutex on Windows / `flock` on macOS) around **both `Load()` and `Save()`** — not just Save. (H-2: a torn read during another process's write makes a process fall into the empty-profiles `catch` and then `Save()`, **destroying** the registry.)
- **Atomic `Save()`**: write `profiles.json.tmp` → `rename()`. The current `std::ofstream` truncate-rewrite (`:165`) corrupts on a mid-write crash regardless of locking. Atomic rename fixes that independently of the lock.
- Lock ordering: acquire the cross-process lock as the first statement inside `Load()`/`Save()` (both already run under `mutex_`), so order is always `mutex_` → cross-process lock. No deadlock.

---

## 4. Picker mode (Windows-first) — feature

A short-lived **chooser process that owns no profile**:

1. **Own single-instance gate** — a dedicated `\\.\pipe\hodos-browser-.picker` (reuse `SingleInstance` with profile id `".picker"` — note `.` is not in `IsValidProfileId`, so it can never collide with a real profile; the pipe-name builder takes a raw string so it still works). A 2nd no-arg launch focuses the existing picker instead of dying. **(C-1: this is the guard that replaces CEF's own SingletonLock failure on a shared cache dir.)**
2. **`CefInitialize`** with `root_cache_path = <AppPaths-root>/.picker-cache` (derived from the already-resolved dev/prod root — **never hardcode** `HodosBrowser`; M-5), and `remote_debugging_port = 0` (M-4, avoids colliding with a running profile's DevTools port).
3. **Skip** `AcquireProfileLock`, the profile single-instance pipe, and the 4 SQLite manager `Initialize()` calls. (Shutdown cascade is null-guarded end-to-end — L-1 — so skipping init is free at teardown.)
4. **Render the picker as a full window** reusing the React `/profile-picker` route (`ProfilePickerOverlayRoot.tsx`) — render as a window, not a dropdown overlay. `useProfiles` IPC (`profiles_get_all`/`profiles_switch`) works because `ProfileManager` is initialized from the **app-data root** before profile resolution (M-3). Picker-created profiles must inherit settings from a sane source, not a half-initialized "current" (M-3 open item — define `currentProfileId_` in picker mode; use `Default` as the settings-copy source).
5. **On choice** → `LaunchWithProfile(chosen)` (spawns `--profile=chosen`); on success, close the picker window → drain the message loop → **clean `CefShutdown`** (never `ExitProcess` — G-1, else orphaned render subprocesses). On `CreateProcessW` **failure**, the picker **stays open and shows an error** (do not leave the user with nothing).
6. **Picking an already-running profile** is fine: the spawned child forwards via the profile's pipe **before** its own `CefInitialize`, so no orphaned child CEF; the running instance opens an in-process window (H-1).

**Retire the legacy `:1240` startup-overlay picker** (open Default then show a dropdown over it) — mutually exclusive with picker mode (M-1). The profile-icon dropdown stays for **in-session** switching (a different moment).

**macOS:** picker mode is **Windows-first**. macOS keeps its native `applicationShouldHandleReopen` behavior; `SingleInstance` is already a stub there. A `cef_browser_shell_mac.mm` picker path is a tracked TODO in `MACOS_PORT_0_4_0.md` (L-2).

---

## 5. R7 — validation hardening

- Validate the resolved id against the registry by **exact string match** (`IsValidProfileId` + exists-in-`profiles_`). Unknown → coherent `Default` fallback before SQLite init. The fallback itself is guarded: if `defaultProfileId` names a deleted profile, fall through to the first real profile (M2) so the resolved id always has a real data dir.
- **C-2 resolved (no space rejection needed).** Exact-registry-match *is* the coherence guarantee: a quote-stripped mangle (`"Profile 2"`→`Profile`) misses the registry and falls to Default coherently — it can never silently land a *different* profile. And our own relaunch (`LaunchWithProfile`→`CreateProcessW`) passes the id **quoted with no shell**, so a legacy `"Profile N"` round-trips intact and still resolves correctly. Rejecting spaces in `IsValidProfileId` was therefore dropped — it would only lock legacy space-id users out of their own profiles for no coherence gain. (`GenerateProfileId` already emits the space-free `Profile_N` for all new profiles.)

---

## 6. Implementation order (small steps; build between)

1. **Step 1 — Foundation (R5 + R7 + resolver, picker OFF path).** ProfileManager: atomic `Save()`, cross-process lock around Load+Save, split persist from in-memory set. main(): replace `:3221-3224` with a resolver — explicit `--profile` (validated, R7) → else 1 profile → else last-used; delete the every-boot Save. *Interim:* no-arg + >1 falls to last-used (still an improvement over Default). Unit-testable bits get tests in `hodos_tests`.
2. **Step 2 — Picker mode.** `.picker` instance gate; neutral-cache `CefInitialize` branch; picker window creation (Windows); React `/profile-picker` full-window mode; choose → spawn → clean shutdown; error fallback. Flip default: picker ON when >1. Retire `:1240`.
3. **Step 3 — (optional, separate chunk) per-profile shortcuts/jumplists** carrying `--profile=` — the real "taskbar new window → right profile" fix. Deferred by decision.

## 7. Test plan

- **Unit (`hodos_tests`, GoogleTest):** resolver decision table (explicit/1-profile/last-used/invalid→Default); `IsValidProfileId` space-rejection at launch boundary; atomic-save round-trip; persist-only-on-explicit-choice.
- **Integration / live smoke (Windows):** (a) switch to Profile_1, quit, relaunch no-arg → picker appears (>1) or last-used (picker off); (b) double no-arg launch → single picker (C-1); (c) pick already-running profile → new window in it, picker exits cleanly; (d) pick not-running profile → it cold-starts; (e) 1-profile install → no picker; (f) garbage `--profile=../../x` → coherent Default; (g) quick restart still clean (R2/R3 regression).
- **macOS:** picker mode out of scope; verify no-arg + native reopen unaffected; `MACOS_PORT_0_4_0.md` TODO.

## 8. Adversarial-review ledger (resolved)

**Design review:** C-1 `.picker` pipe gate · C-2 subsumed by exact-registry-match (space ids retained; see §5) · H-2 lock Load+Save + atomic save · H-3 delete every-boot Save · G-1 clean CefShutdown + spawn-fail fallback · M-1 retire `:1240` · M-4 port=0 · M-5 cache from AppPaths · L-1 shutdown null-guarded (safe) · L-2 Windows-first.

**Code review (post-implementation):** no Critical/High; normal startup verified byte-identical. Applied: **M2** (R7 fallback guards `exists(defaultProfileId)`, +1 test) · **L1** (RegistryLock tracks mutex ownership, logs degraded/timeout case) · **picker-window React fix** (`/profile-picker?mode=window` → every selection launches, fixing the same-id hang). Acknowledged-not-blocking: L2 (atomic-save fallback degrades to non-atomic only when `rename` fails), L3 (mac flock no timeout — Mac-sprint TODO).

## 9. Status (2026-06-19) — implemented, built, NOT yet committed/smoked

- **Step 1 (R5+R7+resolver)** ✅ done. **Step 2 (pre-window picker, Windows)** ✅ done. **Post-smoke revisions** ✅ done: dropped last-used (picker-off → default profile); one-time **v1→v2 migration** flips `showPickerOnStartup` to `true` for existing users (so nobody edits a file); `win_run.ps1` run-only launcher added (no rebuild, no process-kill — supports several profiles running at once).
- Builds: Windows shell clean; `hodos_tests` **10 ResolveStartup + 9 id = 19** green; frontend TS green. macOS `.mm` resolver edit + picker port = compile-deferred / TODO (`MACOS_PORT_0_4_0.md`).
- **No in-picker "Show on startup" toggle yet.** Picker is on by default (migrated). Turning it *off* currently requires a file edit (picker-off → opens default profile). Recommended UX follow-up: add the checkbox so users can opt out from the UI. Deferred by decision (migration-only chosen).
- **Confirmed live (this session):** in-session profile switch opens a new window with **isolated cookies/logins** (x.com did not carry Default's session). **Separate pre-existing item to investigate:** a freshly-switched profile's new-tab tiles still *looked like* Default's history — likely a history / most-visited isolation gap, NOT a picker issue; cookies are correctly isolated. Track + investigate independently.
- **One-process-per-profile confirmed:** N profiles run simultaneously as N processes; launching the same profile twice opens a new window in the existing process (single-instance pipe forward).
- **Live smoke is the real gate** (startup/window/shutdown has no unit coverage) — §7 checklist, next dev run; key cases now **A** (picker shows), **D** (pick already-open profile → new window in it), **E** (double-launch → one picker), **H** (quick-restart clean).
