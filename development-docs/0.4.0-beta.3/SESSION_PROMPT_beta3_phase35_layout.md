Start beta.3 Phase 3.5 — WS2 continued, layout is window-scoped. ⛔ Kickoff first: verify the inventory against the current tree, and no code until I sign off on the assessment.

# 0. Read first

1. Auto-loaded `MEMORY.md`, plus **Phase 3's results** — `development-docs/0.4.0-beta.3/phase-3-window-identity/{PHASE_CONTRACT.md,MEASUREMENTS.md}`. Phase 3 established the mechanism you are extending and made three claims that were **refuted by measurement**; read M10 and M11 before you form any theory about Windows shell behaviour. ⭐ Two of those refuted claims were things the assistant had already written into code.
2. **`phase-3.5-layout-window-scoping/PHASE_CONTRACT.md` — this phase's contract, already written and signed off in shape.** §1 and **§1.1 (the measured site inventory)** are the spine of the work. Do not re-derive them from scratch; **verify** them.
3. `development-docs/0.4.0-beta.3/HARNESS.md` — tiers, what can and cannot be a gate, and §4 on how a ratchet lowers.
4. `development-docs/0.4.0-beta.3/REGRESSION_SET.md` — run in full at the 3.5 → 4 boundary. ⛔ R-GOLD, R-CLOSE and R-COUNT are **still owed** from the 2 → 3 boundary; they need a real payment and a human. ⚠️ They are **not** install tests — they run on the dev build.
5. **`development-docs/0.4.0-beta.3/INSTALL_TEST_BATCH.md`** — ⛔ **read before deferring anything.** Owner decision 2026-08-31: every row that needs a real install is batched and run once before the RC. If this phase finds a new one, **add it there, naming the phase that owes it.**
6. `development-docs/SCOPING_PROCESS.md` — this session is a **Microscope** stage: it reads the contract, not the previous session's reasoning.

# 1. What this phase is

Phase 3 fixed the two window-identity defects the owner **reported**. This phase converts the **densest cluster of the same defect** — the layout code — because that is where one coherent change and one test story cover many sites at once.

**The defect, in one line:** a second launch of the same profile does not start a second process. It forwards over a named pipe and opens a second **window** in the running one, so any code that asks a process-global *"which window am I?"* can act on the wrong one.

⛔ **This is a half-finished migration, not a missing capability.** `BrowserWindow`, `WindowManager`, `GetOwnerWindow()` and `TabManager::GetActiveTabForWindow(id)` all already exist and are already used correctly in ~18 places. The edit is `g_x` → `bw->x`.

# 2. ⛔ Scope — already decided, do not widen

Per the contract §1/§1.1 and `TICKET_window_scoped_work_uses_process_globals.md`:

**IN:** the `WM_SIZE` picker arm (~L1188) and the **overlay-reposition block (~L1300–1470): 27 global references across 7 overlays** — settings, cookie panel, download panel, siteinfo panel, wallet, backup, notification. Plus the **12 `ScalePx(x, g_hwnd)`** sites in `simple_handler.cpp` (⚠️ those are a *different* set from the `ScalePx` calls inside the reposition block — see the trap below).

**ALSO IN, but fenced (contract §1.2):** two tickets — `chrome_ui_scales_but_its_window_does_not` and `modal_buttons_unclickable_small_screen`. ⚠️ They are here because they share this phase's **test rig** (a human at two monitors at different scale factors), **not** because they share its code — the second is React/CSS in an overlay. ⛔ **The conversion lands first and separately**; these are a distinct commit after it is green, so the conversion stays cleanly revertible. ⚠️ **If the conversion consumes the phase, these two go back to Phase 10 and the phase still closes.** They are the droppable half.

**OUT:** ⛔ `SaveSession` and `ShutdownApplication` — *all tabs in all windows* is **correct** there; converting them is a new defect, and `P3.5-A4` is the control that catches it. ⛔ `g_file_dialog_active` and `g_wallet_overlay_prevent_close` — genuinely process-wide. ⛔ The remaining ~60 sites the `G11` gate counts; they are beta.4's ticket.

⚠️ The test is never *"is it global"* but **"does this have one value per process, or one per window?"**

# 3. 🚨 The trap that will bite you

**The overlay-reposition block MIXES correct and incorrect code.** `mainRect` and every `ScalePx(…, hwnd)` inside it already use the message's own `hwnd` and are **RIGHT**; only the header/overlay handles and icon offsets use globals. **A blanket find-and-replace damages the working half.**

That mixture is the signature of the half-finished migration and it is why this is a read-every-site job, not a `sed`.

# 4. Adversarial review — required before any plan is accepted

- Label every finding **claim vs measurement**. Phase 3 recorded a code reading as a mechanism twice and both were wrong.
- ⛔ Every proposed check needs its negative control, **shown to go RED**.
- ⚠️ **Assert the right SUBJECT: two windows of the SAME profile ⇒ ONE process.** Verify with `Win32_Process` that exactly one non-`--type=` browser process exists. Two *different* profiles are two processes and would pass while proving nothing — that is this phase's vacuous-test trap.
- ⭐ **Verify the inventory is still current.** Phase 3 edited `ShellWindowProc`; line numbers have moved. Confirm each arm by symbol and shape, and correct §1.1 in place if it drifted.
- For each conversion: what would make it wrong? Which arms are already correct and must be left alone?

# 5. Blast radius — audit explicitly

- ⚠️ **R-CLOSE** — overlay close guards. This phase touches overlay **positioning**; it must not touch overlay **lifetime**. The mouse hooks and prevent-close flags are correct as globals.
- ⚠️ **R-GOLD** — `Tab::id` ≠ `CefBrowser::GetIdentifier()`. Anything touching tab enumeration risks the payment pill landing on the wrong tab.
- ⚠️ **CLAUDE.md invariant #8** — CEF lifecycle/threading is fragile. `ShellWindowProc` is the main window procedure; change only the named arms, never the message routing.
- ⚠️ **DPI** — `P3.5-A3` covers the `ScalePx` half and has **never been reproduced**. Reproduce the defect *first*; if the pre-fix arm shows the offsets already differing, the claim is wrong and those 12 sites leave this phase.
- ⚠️ **macOS** — Windows-only. `cef_browser_shell_mac.mm` has a structurally different window model. Make no macOS claims from Windows; nothing is owed to the relay.

# 6. Deliverable

1. §1.1's inventory **re-verified against the current tree**, corrected in place if it drifted;
2. the conversion, arm by arm, leaving the already-correct arms alone;
3. `P3-G11`'s baseline **lowered** by exactly what you converted, re-measured **by `preflight.ps1`** (⛔ never by hand — three hand counts have been wrong this sprint), with residuals listed by `file:line` and a reason;
4. each evidence row GREEN **with its RED observed**;
5. what stays manual, said plainly. ⚠️ Almost every row here is **T3 — a human with two windows on two monitors**. Say so rather than inventing an automated proxy.

# 7. Machine state

- ⛔ The owner's installed browser (`%APPDATA%\HodosBrowser`) and its wallet on **31301** must never be touched. **Match by exe path, never process name.**
- Dev stack: wallet **31401**, Vite **5137**, dev browser from `cef-native/build/bin/Release`.
- ⛔ `cargo build` fails "Access is denied" while the dev wallet runs; the **linker fails `LNK1104`** while the dev browser runs. Stop them **by path** before building.
- ⚠️ The dev browser starts in the profile picker when >1 profile exists, and picker mode sets `remote_debugging_port = 0` — CDP is off entirely. `--profile=Default` bypasses it; dev CDP is **9322**. ⛔ `cdp.py` refuses 9222 — that is the installed browser.
- ⛔ **A `.lnk` cannot carry an environment variable**, so a shortcut into `build/bin/Release` always hits the dev safeguard. Launch dev with `HODOS_DEV=1` directly.
- ⭐ **No log file at all ⇒ the process died before `Logger::Initialize`** — only `EnforceDevSafeguard` runs there. Fast triage signal.
- Reusable probes: `phase-3-window-identity/{winaumid.ps1,lnkaumid.ps1}`, `phase-2-logging-syncio/{stub_wallet.py,uisample.py,e1.py}`, `phase-1-overlay-input-dpi/cdp.py`.
- ✅ Everything through Phase 3 + the ticket triage is pushed to `origin/0.4.0`. ⛔ **Do not trust a commit hash written in a doc — always `git fetch` and read `git log HEAD..origin/0.4.0` yourself.** ⚠️ The Mac side pushes to this same branch and has done so mid-phase before, including a retraction and a cross-platform wallet fix. Rebase before pushing.

# 8. Carried, not part of this phase

- ⛔ **ANY row needing a real install goes to `INSTALL_TEST_BATCH.md` — do not run it here.** Owner decision 2026-08-31: building, installing, testing and uninstalling is the sprint's most expensive loop, so every phase's install-dependent rows are **batched and run once before the RC**. ⭐ If this phase discovers a new one, **add it to that file with the phase that owes it** — a deferral with no home is a row that never runs. ⛔ It is OWED, never waived, and never reported as passed.
- 🔴 **`P3-A5d` is OPEN by decision**: the owner wants the profile name on a window's taskbar button, the Start Menu must carry **one** entry only, and per-profile shortcuts were rejected. Naming without a shortcut needs `PKEY_AppUserModel_RelaunchDisplayNameResource` on the window's property store — **UNVERIFIED**, and the docs say it takes an *indirect resource reference*, not a plain string. ⛔ **Measure before writing.** Not this phase.
- 🔴 **R-GOLD, R-COUNT and the still-unobserved `payment.auto_approved` audit line** — one real payment (teragun) closes all three. Needs the owner at the machine; they have said they will try to fold it in. ⚠️ Not an install test — it can run on the dev build.
- P2-A7 is **NOT MET** — 8,208 torn lines, cause unestablished after three failed reproductions. ⛔ Do not re-derive the refuted explanation.
- `DPI_RESOLUTION_TEST_MATRIX.md` still has no overlay section — last outstanding Phase 1 kickoff deliverable. ⭐ **Directly relevant here:** `P3.5-A3` needs matrix cell #9 (mixed-DPI) **with two windows**, which no test has ever run. Writing that section is a natural by-product of this phase.
- **Ticket triage is DONE** (2026-08-31, `TICKET_TRIAGE_2026-08-31.md`): 6 closed, 3 partial, 11 open, 6 needing owner review, 1 accepted. Every ticket now carries a **Status** and a **Sprint** line. ⛔ Do not re-triage.
- **Phases 7–10 exist** as a holding pattern for the consolidated tickets (`SPRINT_PLAN.md` §4.1). ⛔ They are **not a queue anyone pulls from** — a ticket is not work until the owner assigns it.

# 9. Where this phase sits

`0 · 0.5 · 0.6 · 1 · 2 · 3` ✅ complete · **`3.5` ← YOU ARE HERE** · `4 · 5 · 6` planned · `7 · 8 · 9 · 10` ticket consolidation.

⭐ Phase 3 closed with two of its own claims **retracted** after the owner's two-minute tests refuted things the assistant had already written into code. Read `phase-3-window-identity/MEASUREMENTS.md` M10 and M11 before forming any theory about Windows shell behaviour.

# 10. Working style

⭐ Walk me through anything I need to click, step by step. I am doing the clicking. Tell me what to do, what I should see, and **what a wrong result would mean**.

⭐⭐ And check before you send me. This sprint's pattern: the owner's two-minute test has repeatedly refuted something the assistant had already implemented and described as correct. Prefer *"here is the experiment and both outcomes are informative"* over *"this should work"*.
