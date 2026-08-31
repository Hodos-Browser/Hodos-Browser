Continue beta.3 Phase 3.5 — kickoff is DONE and the phase was RE-SCOPED at `4b323c6`. ⛔ One 60-second measurement comes before any code.

# 0. Read first, in this order

1. Auto-loaded `MEMORY.md`, then `project_p35_layout_rescoped_2026_08_31.md`.
2. ⭐ **`phase-3.5-layout-window-scoping/PHASE_CONTRACT.md` — read `§0.1` and `§5.0` FIRST.** They
   **supersede** `§1.1`'s verdicts and the `§5` table, which are kept only for provenance. Reading the
   old table first will send you at work that has already been measured away.
3. **`phase-3.5-layout-window-scoping/MEASUREMENTS.md` K1–K9** — every number in the contract is cited
   from here, each labelled 📏 MEASURED or 📖 CLAIM.
4. `HARNESS.md` (tiers, ratchets, §4, §9), `REGRESSION_SET.md`, `INSTALL_TEST_BATCH.md`.
5. `SCOPING_PROCESS.md` — this is a **Microscope** session: it reads the contract, not the previous
   session's reasoning.

⛔ **Do not re-derive the inventory.** It was verified against the tree on 2026-08-31 and three of the
original contract's claims were refuted in the process. Verify only what you are about to change.

# 1. State

✅ Kickoff, adversarial review and re-scope complete. ⛔ **NO product code has been written.**
`4b323c6` is docs + one probe script. `HEAD == origin/0.4.0` as of that commit — ⛔ **`git fetch` and
read `git log HEAD..origin/0.4.0` yourself; the Mac side pushes to this branch and has done so
mid-phase.** ⚠️ `4b323c6` may still be **local only** — check whether it needs pushing.

# 2. ⛔ Scope — re-scoped, do not widen

**IN:**
1. 🚨 **The Z-order fix (`P3.5-Z1`)** — opening a dropdown in window B must not send B behind A.
2. **12 × `ScalePx(x, g_hwnd)`** in `simple_handler.cpp` → the owning window (L2936, 3099, 3168, 3384,
   5906, 7446, 7504, 7611, 7710, 7810, 7813, 7814).
3. **Omnibox create-path positioning (`P3.5-A7`)** — the one live F4 instance.

**OUT — each for a *measured* reason, not a judgement call:**
- ⛔ The **overlay-reposition block**. A primary-only guard three lines above it makes it unreachable
  for secondary windows ⇒ `g_x → bw->x` there is a **no-op** and `P3.5-A1` is **vacuous** (K3).
- ⛔ The **`WM_SIZE` picker arm** — one picker window per process by construction (K2).
- ⛔ **Lowering `P3-G11`** — the gate scans neither that file nor those patterns (K4). Baseline stays
  **60**. Widening it = editing the instrument in the change it measures (working rule #6).
- ⛔ `SaveSession` / `ShutdownApplication` — correct as global; `P3.5-A4` is the control.
- ⛔ The **14-creator per-window migration**. The owner asked whether we can "just run overlay creation
  again for window B": right end state, **bigger not smaller** — every creator is hard-bound to the
  primary and re-calling them orphans A's overlays *and* their CEF subprocesses (contract §0.1.1).
- ⛔ `TICKET_omnibox_addressbar_interaction_defects.md` — four owner/test-user observations, **none
  reproduced**. ⚠️ Its **#2 (omnibox stuck open)** becomes genuinely adjacent once you touch overlay
  lifetime; still do not fix it here, but **re-check it after `P3.5-Z3`** in case the fix moves it.

# 3. ⛔ FIRST ACTION — `P3.5-Z4`, before any code

**Is the Z-order defect all 14 overlays or only the omnibox?** It is marked **UNMEASURED** in the
contract precisely so nobody generalises it in writing. It changes how big the fix is.

Walk the owner through it — they do the clicking:
1. Dev browser running (`HODOS_DEV=1`, `--profile=Default`). **Ctrl+N** for window B; drag it to
   overlap window A on the same monitor. ⛔ **Ctrl+N, never a second launch** — see §5.
2. Owner pastes, from any directory:
   `pwsh -NoProfile -ExecutionPolicy Bypass -File "C:\Users\archb\Hodos-Browser\development-docs\0.4.0-beta.3\phase-3.5-layout-window-scoping\winprobe.ps1" -WatchSeconds 30`
3. Owner switches to window B and opens the **menu or shield dropdown** (not the omnibox).

**Both outcomes are informative:** B falls in `Z` ⇒ all 14 overlays, one shared fix. B does not ⇒
omnibox-specific and much smaller. ⛔ Record which, and correct K9.3 in place.

# 4. The fix, and its one real hazard

📏 All 14 overlays are `CreateWindowEx(..., g_hwnd, ...)` — owned by the primary, ownership fixed at
creation. `Show*Overlay(targetWin)` can move an overlay but **cannot re-own it**. Candidate fix:
`SetWindowLongPtr(hwnd, GWLP_HWNDPARENT, targetWin->hwnd)` in the show path.

🧠 **The cause is a CANDIDATE, not established** — nothing in `ShowOmniboxOverlay` explicitly raises A.
**The fix is itself the decisive experiment**, with revert as its negative control. ⛔ Do not write it
up as the mechanism until the fix is seen to stop the drop.

⚠️ 🚨 **This crosses into overlay LIFETIME (R-CLOSE).** An owned window is destroyed with its owner, so
an overlay re-owned to B dies when **B** closes while `g_*_overlay_hwnd` still points at it. The phase
can no longer claim "positioning, never lifetime". **`P3.5-Z3` owes that test and it is the row most
likely to fail.** `IsWindow()` guards exist on the show/hide paths — confirm they are enough rather
than assuming.

⭐ Rows `Z1` and `A7` are **already RED, pre-fix, on unmodified code** (K9). Do not re-run the RED half
by breaking the fix; it has been observed and is recorded. Every *other* row still owes its RED.

# 5. 🚨 Traps that have already bitten this phase

- ⛔ **`winprobe.ps1 -WatchSeconds N` is mandatory for any row with a visible overlay.** Overlays hide
  on focus loss (`HideAllOverlays` via `WM_ACTIVATEAPP`), and **clicking the console to run the probe
  IS focus loss** — one-shot mode can never see one and reports a clean, empty, wrong result.
- ⛔ **Read the `Z` column, not "did it disappear".** *Behind* and *minimized* look identical on screen.
  A `-32000,-32000` rect is minimized. This session offered a `-32000` sighting as a lead **in error**;
  the real defect is z-order.
- ⛔ **Ctrl+N makes window B.** A second launch from Explorer/taskbar hits the **dev safeguard** (a
  `.lnk` cannot carry `HODOS_DEV=1`). Signature: a live process with a `#32770` *"HodosBrowser - Dev
  Safeguard"* dialog and **no `debug_output-<pid>.log`**. Working as designed; fine in production.
- ⚠️ **SUBJECT:** two windows of the **same profile ⇒ ONE process**. `winprobe.ps1` asserts this. Two
  *profiles* are two processes and would pass every row while proving nothing.
- ⛔ Launch dev with PowerShell `Start-Process` after `$env:HODOS_DEV='1'` — a detached bash `&` launch
  comes up **minimized**.
- ⚠️ Any new PowerShell probe: `CharSet=CharSet.Unicode` on `GetWindowTextW`/`GetClassNameW`, and
  **filter by class, never by size** — both mistakes produced clean-looking wrong output here (K8).

# 6. Machine state

- ⛔ The owner's installed browser (`%LOCALAPPDATA%\HodosBrowser`, wallet on **31301**) must never be
  touched. **Match by exe path, never process name.** It ran throughout the last session, untouched.
- ⚠️ **A dev browser was left RUNNING at the end of the last session.** ⛔ The linker fails `LNK1104`
  while it runs and `cargo build` fails "Access is denied" while the dev wallet runs — **stop them by
  path before building.**
- Dev: wallet **31401**, Vite **5137**, browser from `cef-native/build/bin/Release`. Dev CDP is **9322**
  and is off entirely in picker mode; `--profile=Default` bypasses the picker.
- 📏 **No mixed-DPI rig exists** — all three monitors read 96 dpi. `P3.5-A3` needs a Windows scale
  change (150 % on one monitor) first, or it is **SKIPPED ⇒ INCOMPLETE**, never PASS, and the 12
  `ScalePx` sites defer to beta.4.
- Probes: `phase-3.5-layout-window-scoping/winprobe.ps1`, `phase-3-window-identity/{winaumid,lnkaumid}.ps1`,
  `phase-2-logging-syncio/{stub_wallet,uisample,e1}.py`, `phase-1-overlay-input-dpi/cdp.py`.

# 7. Deliverable

1. `P3.5-Z4` measured and K9.3 corrected in place;
2. the fix, smallest form that closes `Z1` — ⛔ leave the already-correct arms alone;
3. every evidence row in **`§5.0`** GREEN with its RED observed (`Z1`/`A7` already RED);
4. `P3.5-Z3` (R-CLOSE / overlay lifetime) run and reported honestly — it is the likeliest failure;
5. `HARNESS.md` §4 records **why `G11` was not lowered**, so the next reader does not read it as a
   phase that failed;
6. what stays manual, said plainly. Almost every row is **T3 — a human with two windows.**

# 8. Carried, not this phase

- ⛔ **Anything needing a real install goes to `INSTALL_TEST_BATCH.md`**, naming the phase that owes it.
  Owed, never waived, never reported as passed. Phase 3.5 has added **nothing** so far.
- 🔴 **R-GOLD, R-COUNT and the unobserved `payment.auto_approved` audit line** — one real payment
  (teragun) closes all three. Needs the owner. ⚠️ **Not** install tests — they run on the dev build.
- 🔴 `P3-A5d` OPEN by decision (taskbar name without a shortcut; `PKEY_AppUserModel_RelaunchDisplayNameResource`
  is **UNVERIFIED** — ⛔ measure before writing).
- P2-A7 **NOT MET** — 8,208 torn lines, cause unestablished. ⛔ Do not re-derive the refuted explanation.
- `DPI_RESOLUTION_TEST_MATRIX.md` still has no overlay section — ⭐ a natural by-product if `A3` runs.
- ⛔ Ticket triage is **DONE**; phases 7–10 are a holding pattern, **not a queue anyone pulls from**.

# 9. Where this phase sits

`0 · 0.5 · 0.6 · 1 · 2 · 3` ✅ · **`3.5` ← YOU ARE HERE (re-scoped, no code)** · `4 · 5 · 6` planned ·
`7 · 8 · 9 · 10` ticket consolidation.

# 10. Working style

⭐ Walk the owner through anything they need to click, step by step. Tell them what to do, what they
should see, and **what a wrong result would mean.**

⭐⭐ **Check before sending them.** This sprint's pattern is that the owner's two-minute test refutes
something the assistant already believed — and in this phase it happened to the **contract itself**,
twice. Prefer *"here is the experiment and both outcomes are informative"* over *"this should work"*.
When a claim is a code reading, label it one.
