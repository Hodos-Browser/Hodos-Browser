# Mac ⇄ Windows relay (0.4.0) — cross-device coordination hub

Both the Windows Claude session and the Mac Claude session coordinate through THIS doc (committed to
`origin/0.4.0`). Pull before reading; push after writing.

---

## PLAN (2026-07-08): profile picker SHELVED; Mac does the buttons now; cutting beta.23
- **Profile-picker architecture (Option 1/2) is SHELVED** — revisit way down the road with real
  market feedback. Today's behavior is accepted: the picker works on a plain launch; if a taskbar
  entry carries `--profile=` it goes straight to that profile, and users can open another profile
  from inside the browser. No picker work now. (Wallet stays SHARED across profiles too.)
- **Mac buttons are UN-BATCHED and ready to do now** (they were only batched with the picker, which
  is shelved).

## → FOR THE MAC CLAUDE SESSION (do this now)
1. `git pull origin 0.4.0` (Windows is at `f8edb20`+ — all the Windows 0.4.0 fixes + these docs).
2. **Implement:** `development-docs/0.4.0/MACOS_DROPDOWN_BUTTON_CONSISTENCY_BRIEF.md` — make all macOS
   toolbar dropdown buttons consistent with each other: bring `menu`, `profile`, and optionally
   `download` up to the keep-alive + toggle + 0.3s-debounce pattern that cookie/bookmark/site-info/
   tab-list already use. **You IMPLEMENT + compile + smoke on macOS** (`.mm`/`#elif __APPLE__` code that
   doesn't build on the Windows machine). Do NOT touch the Windows blocks or the already-correct
   bookmark/site-info/tab-list mac branches.
3. Also do any still-open independent mac items if not already done (verify state first):
   `MACOS_0_4_0_EXECUTION_BRIEF_2026_07_07.md`, `MACOS_PORT_0_4_0.md`, `MACOS_UPDATE_STABILITY_EXECUTION.md`.
4. Verify the earlier Windows fixes are macOS-safe by build (they're mostly `#ifdef _WIN32`; the
   signer-gate + SettingsManager-global + bookmark-favicon changes are cross-platform — just confirm
   the mac build is clean). Do NOT re-implement the Win10 overlay F1/F2/F3/F5 on mac (Windows-only bug).
5. When done: commit + `git push origin 0.4.0`, and **fill in "MAC → WINDOWS REPORT-BACK" below**
   (commits, files, compile + smoke results, any blockers). This is how the Windows/release side learns
   your status — needed before beta.23 is cut with the mac changes in it.

## → FOR THE WINDOWS / RELEASE SIDE (heads-up)
- Mac Claude is doing the dropdown-button consistency now (standalone; picker is shelved).
- **Before cutting beta.23 with the mac changes, `git pull origin 0.4.0` and read "MAC → WINDOWS
  REPORT-BACK" to confirm Mac's work landed + compiles.** Don't re-implement mac `.mm` from Windows.

---

## MAC → WINDOWS REPORT-BACK (Mac Claude fills this in + pushes)
_(empty — Mac session to populate: date, commits, files changed, compile result, smoke result, blockers)_
