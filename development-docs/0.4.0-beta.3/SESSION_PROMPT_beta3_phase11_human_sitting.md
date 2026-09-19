# Session prompt — beta.3 Phase 11, the `W10` sitting (DPI + text scale)

**Base:** `5a4819c` on `0.4.0`, **pushed**. ✅ **`W9` is DONE** — all six checks passed with the owner
at the keyboard on 2026-09-19, closing the native half of items 2, 3, 4 and 5. Its section is kept
below as context, not as work. **Platform:** Windows. **Standard:** `../HARNESS.md`.
**Owner:** Matthew. **Written:** 2026-09-19, at the end of the session that closed item 1.

---

## Your task

Run **`W10`** from `HUMAN_TEST_QUEUE.md` with the owner at the keyboard, then fix what it finds, then
do **item 7 route 2**. **10 of 11 items in Phase 11 are already closed**; `W10` and route 2 are all
that is left.

⛔ **The owner performs these. You cannot.** `SendInput` clicks are dropped in this environment, CDP
key and wheel events never reach the paths under test, and `Start-Process` does not foreground the
window — so the launch focus sequence differs from a real one. Your job is to prepare the build,
give **one instruction at a time**, read the instruments, and diagnose. Do not narrate a green you
did not observe.

## ⛔ `W10` is NOT expected to pass, and saying so up front is the point

It covers **item 7 route 2, which is unfixed** — `GetHeaderHeightPx()` sizes the header window from
raw `GetDpiForWindow()`, while Chromium sizes its **content** by `dpi × textScale`. So at a raised
Windows **text size** the header should clip, by construction. A green there would mean the test
missed it.

⭐ And `W10` settles **three** items in one sitting: item 6 (modal clicks), item 7 (header clipping),
item 10 (the matrix itself).

---

## Before you ask the owner to touch anything

1. **Read** `phase-11-ui-leftovers/README.md` items 1, 6, 7, 10 and `HUMAN_TEST_QUEUE.md` `W9`/`W10`.
2. **Rebase first.** 🍎 The macOS side pushes to this same branch — five commits landed mid-session
   last time. `git fetch && git rebase origin/0.4.0` before anything, and **rebuild** afterwards.
3. **Build and launch fresh**: `npm run build` in `frontend/`, `cmake --build build --config Release`
   in `cef-native/` (⛔ stop the dev browser first or the linker dies `LNK1104`), then launch with
   `HODOS_DEV=1` and `--profile=Default --remote-debugging-port=9322`.

## ✅ `W9` — DONE 2026-09-19, kept for context only

All six passed first time. ⭐ Two of them had never been exercised by anything: the `WH_MOUSE_LL`
click-outside hook (`SendInput` clicks are dropped in the agent environment) and the real tear-off
drag. Both matched Chrome. Do **not** re-run these unless something regresses.

### The six, for reference

Everything in items 2, 3, 4 and 5 was verified over **CDP**, which proves React → IPC → HWND and says
nothing about native delivery. Give these one at a time:

| | ask for | expect |
|---|---|---|
| a | type a URL and press **Enter fast**, within a beat of the last letter | the dropdown goes and **stays** gone |
| b | type, then **click a suggestion** the same way | same, and the URL appears in the address bar immediately |
| c | type, press **Tab** twice, then **Shift+Tab** | each Tab moves the highlight *and* the text in the bar; the caret **stays in the address bar**; the dropdown stays up |
| d | press **Escape** twice | 1st closes the dropdown and gives back **what you typed**, caret still in the bar; 2nd restores the page URL and leaves |
| e | with the dropdown up, **click elsewhere on the page** | the dropdown goes — ⛔ this is the `WH_MOUSE_LL` hook, which cannot be driven from here at all |
| f | tear a tab off with a **real drag**, then type in the new window and open its menu | the new window stays in front and does not vanish |

## `W10` — the DPI matrix, plus the text-scale dimension it lacks

`DevOps-CICD/DPI_RESOLUTION_TEST_MATRIX.md` cells **#4** (125% / 1366×768), **#6** (150% / 1366×768),
**#9** (mixed-DPI). Then a pass the matrix does **not** cover: Windows Settings → Accessibility →
**Text size** at 125% and 150%.

In each cell:

- **(a) open an approval modal and click its buttons.** The button you aim at must be the one that
  responds, and the bottom of the modal must not be dead. ⭐ A control comes free: relaunch with
  `HODOS_OVERLAY_RAW_MOUSE=1` and the **same binary** must go bad.
- **(b) look at the bottom edge of the header.** 📏 At 125% content needs ~150 px and the window
  gives 96 — expect clipping under **text size**, which is route 2.
- **(c) Ctrl+scroll over the toolbar, then over the page.** ✅ Already passed 2026-09-18; re-confirm
  cheaply.

## Then, in order

1. **Item 7 route 2** — the fix is one factor: multiply `GetHeaderHeightPx()` by
   `HKCU\Software\Microsoft\Accessibility\TextScaleFactor`, plus a change notification so it
   re-lays-out live. ⛔ No rebuild of Chromium, no CEF patch. ⚠️ This is the header-layout path the
   DPI matrix records as having regressed **three times** — `W10` is its evidence, which is why it
   comes after.
2. 👤 **The owner's requested beta.3 item**: give `TaskSweepReservations` its second verdict —
   *"observed **spent** ⇒ stop asking and close the row out"*
   (`TICKET_reservation_can_be_held_indefinitely.md`). ⭐ That change can only move a row from
   "reserved forever" to "terminally spent" and can **never** set `spendable = 1` — assert exactly
   that in its negative control.

## ⛔ Hard-won rules from the session that produced this prompt

1. ⛔⛔ **`document.activeElement` is not keyboard delivery.** It answers *"which element gets keys
   once they arrive at this browser"*. It produced **four consecutive false greens** on item 1 while
   the owner watched a caret blink and type nothing. The instrument that settles delivery is
   `OnPreKeyEvent`'s log line, which names **the role that received each key**.
2. 🚨 **That log is `%APPDATA%/HodosBrowserDev/logs/debug_output-<pid>.log`, NOT `cef_debug.log`.**
   A whole round was lost reporting "zero key events received" from the wrong file.
3. ⛔ **Assert the VALUE, not that something changed.** An item-3 probe read GREEN with the feature
   deleted, because the address bar *did* change — to the previous page's URL.
4. ⛔ **Name the subject of the assertion.** A tear-off probe demanded window B stay in front
   *whichever* window was typed in, so correct behaviour scored as a reproduction.
5. ⛔ **Don't attribute by geometry when things overlap.** The same probe assigned four right-edge
   overlays to the wrong window and printed four confident REDs that were entirely its own.
6. ⛔ **A probe's driver must not navigate the thing it measures.** A settings probe sent its edit
   from the tab it then navigated, so the edit was never sent — it read RED before *and after* the
   fix.
7. ⛔ **Vite strips comments AND esbuild constant-folds.** A served-module check for
   `false && e.ctrlKey` found nothing because esbuild had folded it to `if (false)`. Pick a token
   that survives the transform.
8. ⛔ **Commit by pathspec, never by directory** — the owner keeps uncommitted work under
   `development-docs/`.
9. ⛔ **Stop the dev browser before building** (`scripts/stop-dev.ps1`, never by image name), and
   remember `preflight -Full` does **not** build `HodosBrowserShell`.

## Definition of done

- Every `W9` and `W10` row answered with what the **owner observed**, not what a probe printed.
- Anything found gets its own commit, citing its row, with a negative control that was **seen** to
  fail.
- `npm run build` clean · `scripts/preflight.ps1 -Full` PASS · shell rebuilt explicitly if C++ changed.
- 🍎 Any shared C++ gets a note in `MAC_RELAY_P11_ROUND.md`. ⚠️ And when a fix writes a **persisted
  preference**, the control must revert the **state** as well as the code — the macOS side found that
  trap on item 9.
- `phase-11-ui-leftovers/README.md` and `HUMAN_TEST_QUEUE.md` updated per row.
