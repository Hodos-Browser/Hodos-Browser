# Session prompt — beta.3 **Phase 3 (WS2): window / instance / focus identity**

Written 2026-08-26, after Phase 2 landed as `83ed311..362e21a` on `origin/0.4.0`.

`SPRINT_PLAN.md` §3 defines this as **WS2** and §4 places it at **Phase 3**, after WS1b(b).
It covers reported items **#3** (taskbar identity) and **#5** (virtual-desktop focus).

⛔ CLAUDE.md's **mandatory phase kickoff workflow applies**: re-read the docs, verify every cited
`file:line` still exists as described, reuse-first audit, risk assessment against the load-bearing
safeguards, confirm the test plan, **hand back a summary before writing any code.** Create
`phase-3-window-identity/` and write `PHASE_CONTRACT.md` from `PHASE_CONTRACT_TEMPLATE.md` as part
of the kickoff. ⭐ Per `feedback_risk_list_before_code`, the risk list goes in the contract
**first**, not in a post-mortem.

---

Paste the block below to open the session.

---

Start **beta.3 Phase 3 — WS2, window / instance / focus identity**. ⛔ **Kickoff first: deep
research and adversarial review, and no code until I sign off on the assessment.**

## 0. Read first

1. **Auto-loaded MEMORY.md**, plus Phase 2's results —
   `development-docs/0.4.0-beta.3/phase-2-logging-syncio/{PHASE_CONTRACT.md,MEASUREMENTS.md}`.
   Phase 2 changed how you can *see* anything: production now logs INFO and above, logs are
   per-process (`debug_output-<pid>.log`), URLs are redacted to origin, and there is a new
   `audit-<pid>.log`. **If you go looking for evidence in a log this phase, read that first or you
   will misread an absence as a fact.**
2. `development-docs/0.4.0-beta.3/SPRINT_PLAN.md` — **§1 items #3 and #5**, **§2's "#3 SOLVED at the
   desk"** finding, and **§3 WS2**.
3. `development-docs/0.4.0-beta.3/HARNESS.md` — tiers, and what can and cannot be a gate.
4. `development-docs/0.4.0-beta.3/MAC_RELAY_P2_ROUND.md` — what is owed from the Mac side. ⚠️ WS2 is
   **Windows-only** (§3: "macOS Spaces is a different mechanism and #3 has no macOS analogue"), so
   this phase adds nothing to the relay — but do not let the Mac asks go stale.
5. `development-docs/0.4.0-beta.3/REGRESSION_SET.md` — **the 2 → 3 boundary was run; read its run
   log.** R-INTEXT is 🟢 both halves and R-PERIM is 🟢 at T1 (73 engine tests). ⛔ **R-GOLD, R-CLOSE
   and R-COUNT were NOT run** — they need a real payment and a human, and they are the three that
   most directly guard the money path. They are **owed, not waived**; one real payment session
   closes R-GOLD, R-COUNT and the still-unobserved `payment.auto_approved` audit line together.
   ⭐ If the owner is at the machine for a Phase 3 check anyway, ask whether to fold that in.

## 1. What this phase is

Two reported defects that share a theme — Windows does not know which window is *us*.

### (a) #3 — the taskbar calls us "HodosBrowser.exe"

⭐ **Already root-caused at the desk. Verify it, do not re-derive it.** SPRINT_PLAN §2 says both
symptoms are one cause: an **AppUserModelID identity mismatch**. Two halves, both re-confirmed
current on 2026-08-26:

1. **Production single-profile never sets an AUMID.** `cef_browser_shell.cpp` (the call is at
   `SetCurrentProcessExplicitAppUserModelID`, ~L5159) is guarded by
   `hodos::IsDevEnv() || GetAllProfiles().size() > 1`. For an ordinary user — production, one
   profile — **neither is true and the branch never runs.** The comment says so on purpose; that
   deliberately-preserved legacy behaviour *is* the bug.
2. **The shortcuts declare no AUMID.** `installer/hodos-browser.iss` `[Icons]` (L81, L83) has no
   `AppUserModelID:` parameter, so Windows derives shortcut identity from the target path while the
   process has a different (or no) explicit identity.

⚠️ Especially fragile under the **bootstrap model**: `HodosBrowser.exe` is CEF's `bootstrap.exe`
loading `HodosBrowser.dll` and spawning children.

⛔ **It is NOT the version resource.** `kFileDescription` is already `"Hodos Browser"` and is
present in the shipped beta.2 binary — verified by extracting the exe from the released portable
zip. Do not "fix" that again.

### (b) #5 — Ctrl+F on virtual desktop 2 switched desktops and opened find in the *other* instance

Two Hodos instances on two Windows virtual desktops. `Ctrl+F` in the second one **switched
desktops** and drove the first. Same-desktop multi-instance results were "mixed and inconsistent".

§3's read: *"the reported inconsistency suggests focus resolution keyed on a global rather than the
active window."* ⚠️ **That is a hypothesis, not a finding.** Treat it as the first thing to falsify.

⭐ There is a concrete place to start: `SimpleHandler::OnPreKeyEvent`'s Ctrl+F arm sends `find_show`
to *a* header browser and moves CEF focus to it. Which header? Answer that and you have either
confirmed or killed the hypothesis in an hour.

## 2. ⛔ Scope — the sprint plan has already made this call

§4: *"#3 is solved at the desk (~1 day). #5 is an unbounded deep dive — **take #3, defer #5**."*

⇒ **#3 is the deliverable. #5 is a time-boxed investigation, not a commitment.** Box it explicitly
in the contract, and if the box expires, write down what was learned and file a ticket rather than
letting it eat the phase. ⭐ Phase 2 lost most of a session to a 0.08 % cosmetic side-quest that
turned out to be unexplainable; the owner had to pull it back on track. Don't repeat that.

⚠️ If the kickoff finds #5 is *also* small and shares a root cause with #3, say so and re-propose —
but with evidence, not optimism.

## 3. Adversarial review — required before any plan is accepted

- Label every finding **claim** vs **measurement**. Phase 2 recorded an inference as a measurement
  and it cost a session; the correction is in `MEASUREMENTS.md` M2.1.
- ⛔ **Every proposed check needs its negative control, shown to go RED.**
- ⚠️ **This phase is unusually easy to fake green.** "The taskbar looks right" on a dev machine with
  4 profiles proves nothing — the broken path is *production, single profile*, which is precisely
  the configuration you cannot launch from `build/bin/Release` (the dev safeguard refuses without
  `HODOS_DEV=1`, and `HODOS_DEV=1` takes the branch that already works). **Work out how you will
  test the production single-profile path before you write the fix**, and if the honest answer is
  "only from a real install", say so in the contract.
- ⚠️ **Assert the right SUBJECT:** which window, which process, which AUMID, which shortcut. An
  AUMID is per-process and inherited by children; a shortcut's is a file property. They are
  different objects and Windows compares them.
- For each proposed fix: what would make it wrong? Would it regress something that works today?

## 4. Blast radius — audit explicitly

- ⚠️ **The multi-profile taskbar behaviour works today and users rely on it.** `TaskbarProfile.cpp`
  (`SetupTaskbarProfile`) generates a per-profile overlay icon and AUMID so profiles get separate
  buttons. **A fix that gives every window the same identity would collapse that** — the two
  requirements pull in opposite directions and the contract must say how they coexist.
- ⚠️ **`AUMID set: …` is a startup log line**, and Phase 2 changed the log. It is `LOG_INFO`, so it
  survives the production gate — confirm that rather than assuming it.
- ⛔ **Changing the installer touches the update path.** `R-UPDATE` — a staged update must still
  apply. Adding `AppUserModelID:` to `[Icons]` rewrites shortcuts on upgrade; make sure that is
  what happens, and that it does not orphan a pinned icon the user already has.
- ⚠️ **Pinned shortcuts are user data.** A user who has pinned Hodos has a shortcut file with a
  path-derived identity. Ask what happens to *their* pin when the identity changes — the honest
  answer may be "it needs re-pinning once", which is a **release-note item, not a silent change**.
- **`WindowManager` owns primary-window tracking and overlay handles.** Anything touching focus or
  window identity risks the overlay lifecycle — `R-CLOSE`.
- #5 touches `OnPreKeyEvent`, which is every keyboard shortcut in the browser.

## 5. Deliverable

A written assessment plus `phase-3-window-identity/PHASE_CONTRACT.md` that I sign off **before** any
code:

1. #3's two halves re-verified against the current tree, with the file:line for each;
2. how a **production, single-profile** build will actually be tested — the thing the fix is for;
3. how the fix coexists with per-profile taskbar buttons, which work today;
4. what happens to an existing **pinned** shortcut on upgrade, stated plainly;
5. the #5 time-box, and what gets filed if it expires;
6. each check with its negative control;
7. what stays manual, said plainly.

## 6. Machine state

- ⛔ The owner's **installed** browser (`%APPDATA%\HodosBrowser`, ~70 procs) and its wallet on
  **31301** must never be touched. Match by **exe path**, never process name.
- Dev stack: wallet **31401**, Vite **5137**, dev browser from `cef-native/build/bin/Release`.
  ⛔ `cargo build` fails "Access is denied" while the dev wallet runs — stop that PID by path.
  ⛔ The **linker** fails the same way while the dev browser runs — stop it before `cmake --build`.
- ⚠️ The dev browser starts in the **profile picker** when >1 profile exists, and picker mode sets
  `remote_debugging_port = 0`, so **CDP is off entirely**. `--profile=Default` bypasses it; dev CDP
  is **9322** (9222 + 100). ⛔ `cdp.py` refuses 9222 — that is the installed browser.
- Reusable harness from Phase 2 lives in `phase-2-logging-syncio/`: `stub_wallet.py` (occupies the
  wallet port so the browser skips launching a real one), `uisample.py`, `e1.py`, `cdp.py` is in
  `phase-1-overlay-input-dpi/`.
- ⛔ Never commit `development-docs/X402_INTEGRATION.md` or
  `development-docs/Onchain-Backup-and-Sync/*` — the owner's parallel work. Stash/pop around a rebase.
- ✅ **Phase 2 is pushed** — `origin/0.4.0` at `2e18b7a` as of 2026-08-26. ⚠️ The **Mac side pushes
  to the same branch and did so three times during Phase 2**, including a retraction and a
  cross-platform wallet fix. `git fetch` and check `git log HEAD..origin/0.4.0` before you assume
  anything about the remote, and **rebase before pushing** (Windows↔Mac deconfliction protocol).
  ⛔ Stash the owner's parallel work around every rebase.

## 7. Carried, not part of this phase

- **`P2-A7` is NOT MET** — 8,208 torn lines in the production log, cause unestablished after three
  failed reproductions. Recorded, not fixed. Do not re-derive the refuted explanation.
- **The audit log's `consent.prompt_shown` half is now confirmed live** (2026-08-26, during the
  2 → 3 boundary run):
  `[14:23:14.503] consent.prompt_shown | example.com | type=domain_approval`.
  ⚠️ **`payment.auto_approved` is still unobserved.** If you make a payment for any reason this
  phase, check `audit-<pid>.log` and report it.
- **`DPI_RESOLUTION_TEST_MATRIX.md` still has no overlay section** — last outstanding Phase 1
  kickoff deliverable. ⭐ Known to be short: fixed-size overlays are DPI-invariant, so only the
  header, the notification overlay and the wallet panel reflow.
- **The overlay dead strip** (Phase 1 item 1) — measured on both platforms, 45 px on the menu
  overlay to the pixel. Deferred to one cross-platform sizing contract; needs the Mac side's input
  (`MAC_RELAY_P1_ROUND.md` §D1).
- Tickets filed, unscheduled: `TICKET_bridge_single_slot_callbacks_race.md` (⛔ read its "NOT fixed
  and why" — deduping `sendTransaction` would collapse two payments into one),
  `TICKET_chrome_ui_scales_but_its_window_does_not.md`,
  `TICKET_longlived_surfaces_snapshot_state_at_startup.md`,
  `TICKET_deleted_profile_id_reused_over_orphaned_data.md`.

## 8. Working style

⭐ Walk me through anything I need to click **step by step**. I am doing the clicking. Tell me what
to do, what I should see, and what a wrong result would mean.

⭐⭐ And check before you send me: Phase 2's owner-facing test would have passed while hiding three
real defects, because the assistant read the code first instead of assuming the screen would tell
the truth. Two minutes of my time is worth ten of yours.
