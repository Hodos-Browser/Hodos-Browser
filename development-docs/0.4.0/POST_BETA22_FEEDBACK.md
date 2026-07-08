# Post-beta.22 Owner Feedback (2026-07-08)

beta.21 → beta.22 auto-update test results + owner-observed bugs. **macOS silent
auto-update WORKED** (beta.21→beta.22, first confirmed live). **Windows silent did NOT
(regression).** Plus Win10 cluster still broken on the owner's mom's machine + bookmark
bugs/features. This is the backlog for a **fresh session** (context ran long shipping
beta.22). Owner's machine = fine; mom's slow old Win10 = where most of this reproduces.

---

## 🔴 P0 — Windows silent auto-update REGRESSED (money-path)
- **beta.21 → beta.22 did NOT silently update on Windows** (mac did). Windows silent was
  PROVEN beta.19→beta.20, so something between beta.20 and beta.21/22 broke it.
- Owner asked: did Mac Claude's Windows AutoUpdater change cause it? **Likely NOT** — that
  change was to WinSparkle (the NOTIFY path), and it's already gated to Notify-only
  (`a4f0ae3`). Windows SILENT is driven by the custom **stager** (`StagePendingUpdate` at
  startup) + **`MaybeApplyStagedUpdate`**, which are separate from WinSparkle.
- **⭐ PRIME SUSPECT = the A1/A2 update-presentation changes (`6112d78`), which are the ONLY
  thing that touched the Windows apply path between beta.20 (worked) and beta.21/22
  (broke):**
  - A1: added a shell-side `UpdateSplash applySplash;` in `MaybeApplyStagedUpdate` right
    before the {app} backup, and hoisted the helper splash to the top of
    `RunApplyTransaction`.
  - A2: made `hodos-update-helper.exe` **GUI subsystem** (`/SUBSYSTEM:WINDOWS
    /ENTRY:wmainCRTStartup`) + dropped `DETACHED_PROCESS` from the helper's `Spawn()`.
  - **GAP: the apply RIGS (`scripts/test-apply-forward.ps1` / `test-apply-rollback.ps1`)
    were NOT re-run after A1/A2** — only built + adversarially reviewed. A GUI-subsystem
    helper or the Spawn-flag change could break the real apply in a way a review won't catch.
- **Fresh-session step 1:** re-run `test-apply-forward.ps1` + `test-apply-rollback.ps1` on
  current code (RIG build `-DHODOS_UPDATE_TEST_SEAM=ON`) to reproduce/isolate. Suspect order:
  (a) GUI-subsystem helper doesn't run/spawn right when launched by the shell's
  STARTUPINFOEX handle-inheritance; (b) shell splash interferes with the backup/apply; (c)
  Spawn `DETACHED_PROCESS` drop. Also get the owner's Windows `update\pending\helper\
  helper.log` + `%APPDATA%\HodosBrowser\logs\debug_output.log` from a real apply-boot attempt.
- Owner has beta.22 on the feed now; a Windows beta.21 machine that fails to update is the
  live repro.

## 🔴 P0 — Win10 cluster STILL broken on mom's machine (C1/C2/C3) — likely ONE shared cause
The hardening helped a little but didn't fix it. Owner's sharpened observations + a strong
new clue:
- **C3 picker:** worked the FIRST launch, then every subsequent launch **bypasses the picker
  and loads her default profile.** NEW CLUE: **mom has "Restore last session" turned on** —
  could that be short-circuiting the picker? (Does session-restore pass a profile / skip the
  no-arg picker path?)
- **Buttons dead after close+reopen:** after closing and reopening, the toolbar buttons
  (bookmark, Site Info) **stop working again** — same as before.
- **⭐ Owner's hypothesis (very plausible):** a **global variable not being set correctly on
  launch because the picker path is taken/bypassed** — i.e., when the profile is launched via
  the picker (or the picker is bypassed on later launches), some global/init that the overlays
  depend on isn't set, so the buttons + picker both fail. This would UNIFY C1/C2/C3 into one
  root cause tied to the picker/profile-launch init path on Win10. **Investigate the
  profile-launched process init vs a normal launch — what globals/overlay state differ.**
- Reminder: the **C3 diagnostic log** (`pickerDecision: profileCount/pickerSettingOn/
  defaultId/showPicker`) is now emitted — get mom's `debug_output.log` and read that line
  across launches to see exactly which input flips (and whether session-restore is involved).
- Context: **Brave works flawlessly on her machine; Hodos is buggy.** Other browsers handle
  her slow old Win10 fine → this is a Hodos robustness gap, not just "her machine is too old."

## 🟠 P1 — Bookmark bugs + features
- **Add fails on mom's machine:** clicked the star → **star turned gold (UI) but the URL was
  NOT actually added** (didn't appear in the list, gone after reopen). So the star-toggle UI
  fired but the backend `bookmark_add` (BookmarkManager SQLite write) didn't land / didn't
  persist. Works on owner's machine. Suspect a slow-machine IPC/timing or DB-write failure on
  Win10. (Likely same environmental class as the Win10 cluster.)
- **Delete button:** bookmarks currently have **no way to delete** — add a delete affordance
  in the bookmark overlay/list. (`bookmark_remove` IPC already exists in the handler.)
- **Favicons:** the bookmark list/modal **doesn't show site favicons** — add them (the NTP
  already caches favicons; reuse that). Make favicons show in the bookmark window list.

## 🟡 P2 — Design questions (owner is weighing these)
- **Cross-profile settings:** "Should all selected settings apply across ALL profiles for
  now?" — a simplification worth considering: the per-profile settings/state machinery may be
  contributing to the Win10 bugs. Applying settings globally (at least temporarily) could
  reduce surface area. DESIGN DECISION — discuss before implementing.
- **"Lowest common denominator" target:** owner wants Hodos to work on a low-end baseline,
  but acknowledges mom's *super* slow old Win10 might be too extreme to be "common." BUT: Brave
  et al. work fine on it, so the bar is achievable — the takeaway is Hodos has real robustness
  gaps on slow/old Windows (timing races, overlay lifecycle, per-profile init) that need
  systematic hardening, not just her specific machine.

---

## Suggested fresh-session plan
1. **Windows silent regression (P0):** re-run the apply rigs on current code → isolate to
   A1/A2 (helper subsystem / splash / Spawn) vs elsewhere → fix → re-prove via rig + a real
   beta bump. This is money-path — top priority.
2. **Win10 cluster (P0):** get mom's `debug_output.log`; chase the owner's global-var/
   picker-init hypothesis + the "restore last session" interaction as the likely shared root
   cause of C1/C2/C3 + the bookmark-add failure. Consider a diagnostic build for her.
3. **Bookmark P1:** delete button + favicons (both concrete, self-contained — good to land
   regardless).
4. **Design P2:** decide cross-profile-settings + the robustness-baseline stance with owner.

Everything else from this session shipped and is live (beta.22 public; website deploy
permanently fixed; macOS silent auto-update PROVEN).
