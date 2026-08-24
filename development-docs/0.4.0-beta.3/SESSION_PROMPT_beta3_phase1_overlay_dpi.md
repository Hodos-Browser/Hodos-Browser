# Session prompt — beta.3 **Phase 1 (WS1): Overlay input & DPI correctness**

Written 2026-08-23 after P0.8 round 3 (`6fc35d2` / `7aa5fbb`, pushed to `origin/0.4.0`).

⭐ **This is a SPRINT PHASE, not a one-off gate run.** `SPRINT_PLAN.md` §3 defines it as
**WS1 — Overlay input & DPI correctness**, covering reported items **1, 2 and 7**, and §4
places it at **Phase 1**, immediately after 0 / 0.5 / 0.6. Phases 0.7 and 0.8 were inserted
ahead of it and are now closed, so this is next.

⛔ CLAUDE.md's **mandatory phase kickoff workflow applies** — re-read the docs, verify every
cited file:line still exists as described, do a reuse-first audit, assess risk to the
load-bearing safeguards, confirm the test plan, and **hand back a summary before writing any
code.** There is **no `phase-1-*` folder yet**; create it and write `PHASE_CONTRACT.md` from
`PHASE_CONTRACT_TEMPLATE.md` as part of the kickoff.

---

Paste the block below to open the session.

---

Start **beta.3 Phase 1 — WS1, Overlay input & DPI correctness**. ⛔ **Kickoff first: deep
research and adversarial review, and no code until I sign off on the assessment.**

We have fixed DPI-ish layout bugs three times (`7277980`, the header-UX regression, `ff3e2ee`)
and each fix was right in isolation while the class kept recurring. I want it understood
properly once. I also want a comprehensive pass over DPI / scaling best practice, not just
these three symptoms.

## 0. Read first

1. **Auto-loaded MEMORY.md** — especially `project_p08_consent_surface_round3_2026_08_23`
   (what just changed and what was explicitly NOT verified) and
   `feedback_consent_surface_needs_human_eyes`.
2. `development-docs/0.4.0-beta.3/SPRINT_PLAN.md` — §1 items **1, 2, 7**; §3 **WS1** (the
   leads already in hand); §4 for why this is Phase 1.
3. `development-docs/0.4.0-beta.3/TICKET_modal_buttons_unclickable_small_screen.md` — ⛔ read
   its **2026-08-23 correction** first; the original premise was wrong.
4. `development-docs/DevOps-CICD/DPI_RESOLUTION_TEST_MATRIX.md` — the 9 cells, methods
   M1/M2/M3, the code-review checklist, and History (the record of this class recurring).
5. `development-docs/0.4.0-beta.3/MAC_RELAY_BETA3.md` — **§D1** (the WS1 (a)/(b) split and the
   Mac 45 px measurement) and **§B4** (what Windows has not verified).
6. `development-docs/0.4.0-beta.3/HARNESS.md` — T3 is "a human at the machine"; know what can
   and cannot be a gate before proposing one.

## 1. The three items, and what is already known

| # | Symptom |
|---|---|
| **1** | Click-outside closes overlays **except directly below the modal** — just under it does nothing; further below, or left/right, works. |
| **2** | **Profile picture cannot be selected** — the overlay closes as soon as the user navigates in the native file dialog. Worked before. |
| **7** | **Mouse offset on the secondary monitor.** In the wallet overlay, hovering a button does not highlight but slightly above it does. All modals on the secondary screen; correct on the primary. |

⛔ **#7 is money-path correctness, not cosmetics** — an offset inside the wallet overlay during
a send means the user hovers one control and activates another. That is why this phase outranks
the feature work behind it.

**WS1 splits** (per `MAC_RELAY` §D1, after the Mac side measured a 45 px window-vs-content
strip: window 280×450, content 280×405):

- **(a) sizing contract** — cross-platform, confirmed on both sides. Overlay window height must
  equal rendered content height, or the close test must use *content* bounds, not window bounds.
- **(b) DPI / offset** — Windows-only until proven otherwise.

## 2. ⭐ Pre-kickoff finding — verify it before building on it

**MEASURED 2026-08-23 by grep. NOT reproduced under instrumentation.**

- Every Windows overlay is created `SetAsPopup` (`simple_app.cpp`) — a **windowed** CEF
  browser, which receives mouse input from Windows directly. **No `SetAsWindowless` at all.**
  ⛔ The ticket's original *"overlays are OSR"* premise was **false on Windows**. It is true on
  macOS, which is exactly why WS1 split into (a) and (b).
- **AND** `cef_browser_shell.cpp` has **48 `GET_X_LPARAM` sites** hand-forwarding
  `SendMouseClickEvent` / `SendMouseMoveEvent` with **zero DPI conversion**, while the process
  is `PER_MONITOR_AWARE_V2`.

⭐ So: windowed browsers that already receive native input **also** carry a parallel hand-rolled
injection path. **Ask why that path exists before assuming its coordinates are the bug.** Double
delivery, or correct-native-plus-wrong-synthetic, produce different symptoms from a single wrong
path — and different fixes.

⚠️ Everything above is a measurement of **code shape** plus an **assumption about cause**. Nobody
has reproduced the offset under instrumentation. Do not let the assumption promote itself.

## 3. Also: comprehensive DPI / scaling best practice

Beyond the three items:

- **Windows** per-monitor DPI v2, `WM_DPICHANGED`, CSS logical vs device pixels, flex/grid
  shrink behaviour, minimum click-target size. ⚠️ **"Make text bigger" is a SEPARATE setting
  from display scaling** — establish whether we honour it at all; I do not know.
- **Every surface** — header/toolbar, all ~14 overlays, the connect and payment consent modals,
  wallet panel, settings, dialogs.
- **macOS parity** — backing scale, Retina / non-Retina, Display "Larger Text", and borderless
  `NSWindow` vs `WS_POPUP`.
- **C++ invariants** from the matrix checklist: `...ForDpi` metrics, `MulDiv` via
  `LayoutHelpers.h`, and the single `SetProcessDpiAwarenessContext` call. ⚠️ A recorded
  follow-up says that call **discards its `BOOL` return**, making a silent fallback to UNAWARE
  invisible — confirm whether that is still true.

⭐ **A gap I already found in the gate itself:** `DPI_RESOLUTION_TEST_MATRIX.md`'s pass criteria
and its single programmatic assertion cover the **header / toolbar only**. They say nothing about
**overlays or modals** — which is where items 1/2/7 live *and* where P0.8 just changed layout
(three checkboxes now share a wrapping row; two consent labels now wrap). The matrix needs an
overlay section; propose one.

## 4. Adversarial review — required before any plan is accepted

- Label every finding **claim** vs **measurement**.
- For each proposed fix: what would make it wrong? Would it regress a cell that passes today?
- ⛔ **Every proposed check needs its negative control**, shown to go RED. This project has
  shipped **four** harnesses that would have passed with the feature absent (three farbling,
  plus a contrast probe whose injection silently no-op'd after a restyle). ⭐ **A control that
  cannot prove it injected anything is worth nothing** — assert the injection changed the source.
- ⚠️ **Guard-in-front-of-the-property trap:** a check can go red for the wrong reason and look
  like it works (T1f's first control — a magnitude cap rejected the fixture before the unit rule
  was ever reached).
- ⚠️ **Assert the right SUBJECT.** The header and ~14 overlays are separate CEF browsers that
  CDP all reports as `type:"page"`. Driving the wrong one faked a bug here before.
- ⚠️ **Physical mouse automation does not work in this environment** — `SendInput` **clicks** are
  dropped (moves work). Plan for CDP + unit tests + me at the keyboard.

## 5. Blast radius — audit explicitly

`SPRINT_PLAN.md` names these and they are load-bearing: the **gold pill** payment indicator,
`g_wallet_overlay_prevent_close`, `g_file_dialog_active`, and the four privacy-perimeter gates.

⭐ `g_file_dialog_active` is item 2's prime suspect: CLAUDE.md documents it on the
`WM_ACTIVATEAPP` path, while the profile panel is a **mouse-hook** overlay
(`ProfilePanelMouseHookProc`). If the hook does not consult the flag, that is precisely the
reported symptom.

⚠️ Item 2's second known trap: a hidden `<input type="file">` triggered by `.click()` is the
broken CEF pattern; **visible** file inputs work. `WalletPanelPage.tsx` is the working reference.

## 6. Deliverable

A written assessment plus `phase-1-*/PHASE_CONTRACT.md` that I sign off on **before** any code:

1. state per surface, claims and measurements separated;
2. gaps ranked by likelihood × blast radius;
3. proposed changes — code, the matrix doc, and what is gated in CI;
4. each check with its negative control;
5. what stays manual, said plainly rather than implied.

Then run the cells (**#4 / #6 / #9 minimum**, full sweep if the assessment argues for it) and
report **measured** results.

## 7. Machine state

- Dev stack was left running: wallet **31401 (schema V25)**, Vite **5137**, dev browser from
  `cef-native/build/bin/Release`. Re-check; it may be closed.
- ⛔ The owner's **installed** browser (`AppData\Local\HodosBrowser`, ~61 procs) and its wallet on
  **31301** must never be touched. Match by **exe path**, never by process name.
  ⛔ `cargo build` fails "Access is denied" while the dev wallet runs — stop that PID by path.
- ⚠️ The notification overlay is **keep-alive**: after a frontend change, restart the dev browser
  or it keeps serving the old JS.
- ⛔ Never commit `development-docs/X402_INTEGRATION.md` or
  `development-docs/Onchain-Backup-and-Sync/*` — my parallel work. Stash / pop around a rebase.
- **M2 is the fast lever**: `--force-device-scale-factor=1.5 --window-size=1366,728`, and do not
  maximize. ⛔ Never ship that flag. ⚠️ **M1** (Windows Settings + sign-out) is higher fidelity and
  the only method that exercises the real startup `WM_DPICHANGED` — if a finding depends on that
  path, M2 is not sufficient evidence for it.
- ⚠️ Item 7 needs a **second monitor**. The Mac side has none and none is coming, so that half is
  Windows-only by instrument availability.

## 8. Working style

⭐ Walk me through anything I need to click **step by step**. I am doing the clicking. Tell me what
to do, what I should see, and what a wrong result would mean.
