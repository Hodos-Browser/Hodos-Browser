# Silent-State Writer + Global Update Mode — Design (Windows silent, commit #1)

**Status:** DESIGN — no code yet. Adversarial review pending. Part of the Windows silent
auto-update flip (see `SILENT_UPDATE_TEST_PLAN.md`, `AUTOUPDATE_6B_SUPERVISOR_DESIGN.md`).

## Problem (verified in code)

The silent apply bootstrap gates on the GLOBAL `update-state.json`:

```cpp
// cef_browser_shell.cpp:3964  (inside MaybeApplyStagedUpdate)
if (!state.silent || state.paused) return false;
```

`UpdateState::silent` is documented as "mirror of autoUpdateMode==\"silent\"" (`UpdateApply.h:76`)
and defaults **false** (fail-safe-off). **Nothing in production ever sets it true.** The only
writers of `update-state.json` are:
- `rejectPersistent` (`cef_browser_shell.cpp:4004`) — reachable only *after* the gate passed;
- the helper's rollback (`transaction.cpp:380`, sets `paused=true`) and success (`:402`) paths.

So on a real installed client, `update-state.json` is absent (→ parse fails → `return false`,
`:3962`) or has `silent=false` (→ `return false`, `:3964`). **A staged update downloads but
never applies.** The Stage-2 "green" real-build test masked this: its setup script
hand-writes the file (`scripts/setup-real-apply-test.ps1` emits `silent=$true; paused=$false`).
The production glue that mirrors the user's setting into that gate was never implemented.

### Second problem this exposes: `autoUpdateMode` is per-profile, the update is global

`autoUpdateMode` lives in per-profile `settings.json` (`SettingsManager` is per-profile via
`Initialize(profile_path)`; active path = `<profile>/settings.json`). But an update is one
install for the whole app. If two profiles disagree ("silent" vs "notify"), the global apply
gate has no single answer. **Owner decision: make `autoUpdateMode` GLOBAL** (Chrome's model —
the updater is machine/user-global; profiles have no per-profile update switch). One value,
shared by every profile, shown identically. This dissolves the tie entirely.

## Timing fact that shapes the design (verified)

`MaybeApplyStagedUpdate` runs at `cef_browser_shell.cpp:4456`; `SettingsManager::Initialize`
runs at `:4519` — **63 lines later.** At the apply seam the user's setting is NOT loaded yet;
that's precisely *why* the gate reads a persisted global file instead of live settings. So the
mirror must be written on a **prior** launch. That lines up perfectly with reality: staging
happens during run N (background thread), apply happens at the cold boot of run N+1. The mirror
written during run N is exactly what run N+1's gate reads. The first silent-capable build writes
the mirror on its own first run (the writer runs every launch, independent of whether anything
is staged).

## Design

### Part A — make `autoUpdateMode` a global setting

Keep the 3-state (`off`/`notify`/`silent`) as a **user** setting, but source and persist the
`autoUpdateMode` field to the **global** settings file (`GetGlobalSettingsFilePath()` —
`%APPDATA%\<app>\settings.json`, roaming), not the per-profile file. All other settings stay
per-profile. Concretely:

- **New private helpers** in `SettingsManager`:
  - `std::string LoadGlobalAutoUpdateMode() const` — read `browser.autoUpdateMode` from the
    global settings.json; return validated value, or `""` if absent/unparseable.
  - `void PersistGlobalAutoUpdateMode(const std::string& mode)` — read-modify-write ONLY the
    `browser.autoUpdateMode` key of the global settings.json (never clobber other global keys).
- **`LoadInternal()` (`SettingsManager.cpp:106`)** — after loading the per-profile struct,
  override `browser_.autoUpdateMode` from `LoadGlobalAutoUpdateMode()`. If the global value is
  absent, seed it from the current `browser_.autoUpdateMode` (which is either the per-profile
  legacy value or the "silent" default) via `PersistGlobalAutoUpdateMode`, so the global file
  becomes authoritative from first run.
- **`SetAutoUpdateMode()` (`SettingsManager.cpp:258`)** — after setting `browser_`, call
  `PersistGlobalAutoUpdateMode(validated)` (in addition to / instead of the per-profile `Save()`
  for this field). Then call the Part-B mirror (below).
- **`UpdateFromJson()` (`SettingsManager.cpp:372`)** — the bulk path replaces `browser_`
  wholesale, so it can change `autoUpdateMode` without `SetAutoUpdateMode`. If the incoming
  `browser.autoUpdateMode` differs, route it through the same global-persist + mirror.

The settings UI needs **no change** — it renders `browser_.autoUpdateMode` (via `ToJson`), which
now reflects the shared global value in every profile. The per-profile `settings.json` may retain
a stale `autoUpdateMode` from struct serialization; it is always overridden from global on load,
so it is inert (documented, not cleaned, to avoid touching the struct schema).

### Part B — the silent-state mirror writer

A single function that mirrors the (now global) mode into the global `update-state.json` gate:

```
void SyncSilentEligibilityState():
    state = ReadUpdateStateOrDefault(GetUpdateStatePath())   // read-modify-write; preserve ALL fields
    state.silent = (SettingsManager global autoUpdateMode == "silent")
    WriteFileAtomic(GetUpdateStatePath(), SerializeUpdateState(state))
```

- **Preserves every other field** (`paused`, `highWaterBuild`, `lastFailureBuild`,
  `rescanAfterRollback`) — only `silent` is touched. Uses existing `ParseUpdateState` /
  `SerializeUpdateState` (UpdateApply) + `ReadFileAll` / `WriteFileAtomic` (updatefs) +
  `AppPaths::GetUpdateStatePath`. No new infrastructure.
- **Call sites:** (1) once at startup **after** `SettingsManager::Initialize` (~`:4519`);
  (2) at the end of `SetAutoUpdateMode`; (3) in `UpdateFromJson` when the mode changed.
- **Fail-safe direction:** if the mode can't be determined, write `silent=false` (never silently
  apply on uncertainty). Never *clear* `paused` here (that is commit #2's concern).

Where the writer lives: a free function in the update-apply translation unit (near
`MaybeApplyStagedUpdate`) or a small `updatefs`/`AppPaths` helper — TBD by review; it must be
callable from both startup and `SettingsManager` without a layering cycle (SettingsManager should
not depend on the shell TU). Likely a standalone helper that takes the mode string, so
`SettingsManager` passes its value in and the shell passes the loaded value in.

## Concurrency / correctness risks (for the review to probe)

1. **Cross-process write race on `update-state.json`.** The startup mirror writer (browser
   process) and the helper (separate process, during an apply) both read-modify-write this file.
   `WriteFileAtomic` makes each write atomic (no torn read), but the read→modify→write critical
   section is not locked across processes. Risk: the mirror reads state, the helper writes
   `paused=true`, the mirror writes back and clobbers `paused`. Need to confirm the mirror never
   runs concurrently with the helper (ordering: the mirror is at `:4519`, AFTER
   `MaybeApplyStagedUpdate` at `:4456` — a boot that applies does not reach `:4519` the same way;
   a post-apply fresh boot runs the mirror while the old helper may still be finishing rollback).
   Options: (a) prove ordering makes it safe; (b) take the existing `update.lock` around the
   mirror's RMW; (c) accept last-write-wins because the mirror re-reads current state each time
   and only ever *sets* `silent` (so it preserves whatever `paused` it read). Review to decide.
2. **Thread-safety of the mirror vs `SettingsManager::mutex_`.** `SetAutoUpdateMode` holds
   `mutex_` while mutating `browser_`; the mirror must run OUTSIDE that lock (it does I/O and must
   not deadlock with `Save()`), reading the validated value passed in.
3. **First-run global seed** must not race two profiles launching simultaneously (both seeding
   the global file). `PersistGlobalAutoUpdateMode` RMW + atomic write; last-writer-wins on
   identical default is harmless.

## Fail-safe review checklist (does any path wrongly ENABLE or wrongly BLOCK silent?)

- Fresh install, no `update-state.json`, default mode "silent" → first run writes `silent=true`;
  correct (default is silent per owner). A user who never touches settings gets silent — intended.
- User sets "notify" in any profile → global persist "notify" → mirror writes `silent=false` →
  next boot the gate blocks silent apply; WinSparkle still notifies. Correct opt-out.
- Corrupt/absent global settings → mode falls back to per-profile/default; mirror still fail-safe
  (writes false only on uncertainty). Confirm no path writes `silent=true` without a real
  "silent" mode.

## Migration

Existing installs have `autoUpdateMode` only in per-profile files. On first run after this lands,
`LoadInternal` finds no global `browser.autoUpdateMode` (the global settings.json predates the
field being authoritative there) → seeds global from the current per-profile value. A user who
previously set "notify" in their (single) profile keeps "notify". Multi-profile installs with
divergent values: the first profile to launch seeds global; document that this is the one-time
collapse to a single global value (acceptable; near-universal case is one profile).

## Test plan

- **Unit (extend `update_apply_test.cpp`):** `SyncSilentEligibilityState` mirrors mode→silent;
  preserves `paused`/`highWaterBuild`/`lastFailureBuild`/`rescanAfterRollback`; fail-safe writes
  `false` on unknown mode; RMW never drops other fields.
- **Unit (SettingsManager):** global autoUpdateMode is read on load, persisted on
  `SetAutoUpdateMode` and `UpdateFromJson`, and per-profile value is overridden by global.
- **The real proof (rig):** rewrite `scripts/setup-real-apply-test.ps1` to **stop hand-seeding**
  `update-state.json`. Let the real writer produce it on the installed N build's first run. If
  the N→N+1 apply still fires, that is end-to-end proof the writer works — closing the exact gap
  the prior "green" test skipped. Keep both legs (commit + rollback).

## Revision 1 — post adversarial review (2026-07-06): MUST-FIX before code

Two independent reviews (correctness + fail-safe direction) found the v1 design unsafe. The
timing model and gate diagnosis are correct, but "make `autoUpdateMode` global via the existing
settings.json, global-wins-unless-absent" opens multiple **enable-silent-when-not-chosen** paths.
Revised design:

- **R1 (CRITICAL) — stale global overrides a live "notify".** Pre-split builds wrote
  `autoUpdateMode` into the GLOBAL settings.json (`GetActiveSettingsFilePath` returns the global
  path while `!initialized_`), then froze it when the per-profile split landed. So "global wins
  unless absent" can let a stale global `"silent"` override a user's live per-profile `"notify"`.
  **Fix:** do the global switch as a one-time, `version_`-gated (1→2) reconciliation that takes
  the **most-conservative** value (`off` < `notify` < `silent`) across the existing global + all
  per-profile settings.json files — never a blind "global wins." Fresh install (no prior files)
  → default `silent`.
- **R2 (CRITICAL) — legacy bool `autoUpdateEnabled=true` maps to `"silent"`** at
  `SettingsManager.h:63` and `simple_handler.cpp:2997`. That auto-grants silent-apply to users who
  only ever consented to notify-era updates. (NB: `WINDOWS_AUTOUPDATE_PLAN.md:23` CLAIMS this was
  corrected to `true→notify`; the code still says `true→silent` — doc/code drift.) **Fix:** remap
  `true → "notify"` in both places. *This is a production behavior change to the installed base —
  owner sign-off required (see decision D1).*
- **R3 (CRITICAL) — multi-profile collapse must prefer the most-conservative value,** not
  first-to-launch. A user's explicit `"notify"` in one profile must never be overridden to silent
  by another profile's launch order. (Same reconciliation as R1.)
- **R4 (CRITICAL) — global-persist failure must fail-safe-off.** If `PersistGlobalAutoUpdateMode`
  can't confirm the write, write `silent=false` to the mirror for this run + log; never leave a
  stale global to silently become authoritative. (The existing `Save()` swallows write failures —
  the new global writer must NOT.)
- **R5 (HIGH) — "uncertainty → silent=false" is currently unreachable.** Corrupt/absent settings
  resolve to the `"silent"` struct default (`SettingsManager.h:16`; `LoadInternal` catch resets to
  defaults, `:141`), so corruption yields `silent=true`. **Fix:** distinguish genuinely-empty
  (fresh install → silent, per owner) from a detected load/parse failure (→ carry an explicit
  "indeterminate" signal so the mirror writes `silent=false`).
- **R6 (MUST, correctness) — close the cross-process RMW race by MUTUAL EXCLUSION, not by taking
  the owner lock.** (Revised after tracing the honor path.) The naive fear: a post-apply
  health-probe boot runs the mirror while the helper holds `update.lock` and is about to write
  `paused=true`, and the mirror clobbers it → re-arms a bad build. **But the honor-at-launch defer
  at `cef_browser_shell.cpp:4391` (`if (UpdateLockIsHeld(...) && !healthProbe)` → defer the whole
  launch) already fires BEFORE settings init (`:4519`), so a NORMAL launch never reaches the mirror
  while the lock is held.** The only launch that proceeds past 4391 with the lock held is the
  health-probe (`g_post_update_probe`). **Fix:** run the mirror only when
  `!g_picker_mode && !g_post_update_probe` → it is then mutually exclusive with the helper by
  construction; **no lock needed.** Do NOT take `UpdateLockOwner` in the mirror — creating/holding
  the exclusive `update.lock` would itself trip the 4391 honor-probe and spuriously defer other
  concurrent launches. Keep a cheap permissive `UpdateLockIsHeld` PROBE-and-skip as defense-in-depth
  (covers any cross-process apply), which never mutates the lock file.
- **R9 (NEW, fail-safe) — `UpdateFromJson` must not let an OMITTED `autoUpdateMode` reset to the
  "silent" struct default.** The bulk path replaces `browser_` wholesale via `from_json`; if the
  incoming `browser` JSON lacks `autoUpdateMode`, `from_json` defaults it to `"silent"`
  (`SettingsManager.h:16`) — so an unrelated bulk settings save (e.g. changing the homepage) could
  silently flip a "notify" user to silent. **Fix:** in `UpdateFromJson`, if `j["browser"]` does not
  contain `autoUpdateMode`, preserve the current value instead of taking the default.
- **R7 (MUST, platform/build) — guard the writer** under `#ifdef HODOS_SILENT_AUTOUPDATE` **and**
  `#ifdef _WIN32` at every call site (no-op on macOS/Linux). Otherwise a flag-OFF 0.4.0 build would
  stamp `silent=true`, pre-seeding a later flag-ON build; and `UpdateFs.h`/`UpdateLock.h` are
  Windows-only so the mac build breaks.
- **R8 (home + reuse correction).** Do NOT host the free function in `UpdateApply.cpp` (pure-TU
  contract, links into the helper). Use a **shell-only** new TU `SilentStateWriter.cpp` (added to
  `${SOURCES}` only). `AppPaths::GetUpdateStatePath()` returns `std::string` but `ReadFileAll` /
  `WriteFileAtomic` take `std::wstring` — widen internally (`SU_Widen` equivalent).
  `ReadUpdateStateOrDefault` is **new** (tiny), not existing — so "no new infrastructure" becomes
  "one tiny read-or-default wrapper + widen + the shell-only TU."
- **Tests to add:** the R1 regression (existing install: global `"silent"` + per-profile
  `"notify"` → mirror writes `silent=false`, no apply); legacy-bool `true` → `notify`; the
  lock-contention skip (mirror must not write while `update.lock` is held); corrupt-settings →
  `silent=false` (R5).

## Out of scope (tracked elsewhere)

- The `paused` sticky-latch recovery (commit #2).
- Turning `HODOS_SILENT_AUTOUPDATE` ON in CI, website `.ed` sidecar, sidecar fail-closed (#3–#6).
- macOS (Sparkle install-on-quit is config-only; no `update-state.json` gate).
