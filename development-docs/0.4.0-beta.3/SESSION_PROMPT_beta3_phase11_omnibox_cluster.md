# Session prompt — beta.3 Phase 11, omnibox cluster (items 2, 3, 4)

**Base:** `1acf20b` on `0.4.0`, pushed. **Platform:** Windows. **Standard:** `../HARNESS.md`.
**Owner:** Matthew. **Written:** 2026-09-18, at the end of the session that closed item 11.

---

## Your task

Complete **items 2, 3 and 4** of the omnibox cluster. They are **three separate defects that share a
surface** — ⛔ not one "omnibox fixes" branch, and not one commit.

Source of truth: `TICKET_omnibox_addressbar_interaction_defects.md` (read it in full — it is short and
it already tells you what not to do) and `phase-11-ui-leftovers/README.md`.

⛔ **Run the phase kickoff first** (root `CLAUDE.md`): re-read the docs, **verify every file:line
citation still resolves** — the ticket is from 2026-08-31 and things have moved (`MainBrowserView.tsx`
is now in `pages/`, not `components/`) — do the reuse-first audit, then hand back a summary **before**
writing code.

---

## The three items, and the precondition each one has

### Item 2 — the omnibox dropdown sometimes stays open after selecting a URL

> 👤 *"it gets stuck open after selection and navigation… it's not often and I haven't noticed a
> pattern… I have to click off to get it to go away."*

⛔ **Reproduce it before touching anything.** Per `HARNESS.md` §8 it stays "not reproduced" until
someone reproduces it. ⭐ An intermittent overlay-lifetime bug where a plausible cause is adopted early
is exactly what cost days in the farbling work.

📖 The hide is IPC-driven from **two different browsers** (the omnibox overlay sends `navigate` +
`omnibox_hide` after a suggestion click; the header sends it on Enter/Escape/empty), plus **five**
unconditional `HideOmniboxOverlay()` calls from window messages in `cef_browser_shell.cpp`
(~lines 873/882, 1163, 1474, 1522, 2537 — verify). 🧠 The ticket's untested hypothesis: a hide that
depends on an IPC from the *overlay's own* browser can be lost or arrive after a re-show. Worth testing
first **because it is cheap, not because it is likely.**

⭐ Also settle deliberately: `HideOmniboxOverlay()` takes **no** `BrowserWindow*`, unlike all ten
`Show*Overlay` functions. Probably right for a process-singleton, but it is the same show/hide
asymmetry Phase 3.5 found — decide it, do not inherit it.

### Item 3 — the URL appears in the address bar noticeably late after clicking a suggestion

> 👤 *"it takes a long time for the url to load in the address box after the user clicks it."*

⛔ **Time the gap before fixing anything.** "A long time" is not a number. Phase 2's lesson applies
directly: **the control there read 2.04 s**, so measure the baseline before calling anything slow.

📖 The click sends only `navigate` + `omnibox_hide` (`OmniboxOverlayRoot.tsx` ~202-203) — it does **not**
carry the URL to the header, so the address bar cannot update until navigation reports back. There is
**no optimistic update**. That is *consistent with* the report, ⚠️ not confirmation of it.

⇒ This is the one item with an obvious cheap fix (send the URL with the click, set it optimistically),
**which is exactly why it must be measured first**. If the measured gap is small, the right answer may
be to do nothing and say so.

### Item 4 — Tab / Enter autocomplete conformance

> 👤 *"I want to make sure our tab and enter behavior in the addressbar is standard for auto-complete
> suggested etc. so we have the same feel as what people are accustomed to."*

**Not a defect — a conformance question**, the only one of the four.

📖 Today (`MainBrowserView.tsx` ~737/751/758): **Tab**, **ArrowRight** and **End** all accept the inline
autocomplete; **Enter** navigates; **Escape** dismisses and restores the typed text.

⛔ **Working rule 5 job before it is a coding job.** Read what Chrome, Firefox and Vivaldi actually do,
**record the row in `development-docs/PRIOR_ART.md`**, then decide. They **disagree with each other** —
notably on whether **Tab** accepts the inline completion or moves focus into the suggestion list, and
on what **Enter** does while a list item is highlighted. ⭐ A change here alters muscle memory on every
navigation, so bring the owner the *differences* and a recommendation rather than a finished change.

---

## ⛔ Hard-won rules from the session that produced this prompt

These cost real time on 2026-09-17/18. Ignoring them will cost it again.

1. ⛔⛔ **Name the LAYER your instrument reads.** Three confident greens died in one session because
   the measurement was scoped to a layer the user does not live in: one browser instead of two ·
   `activeElement` (focus *assigned*) instead of a *visible caret* · **DOM** focus instead of **NATIVE**
   focus. Before reporting green, write *"this proves X at layer Y"* and check Y is the layer the
   complaint came from.
2. ⛔ **The header, every tab and ~14 overlays are separate CEF browsers**, and CDP reports every one as
   `type:"page"`. Pick the target deliberately; for the omnibox you will usually want **two** sessions
   (the overlay *and* the header) because item 2's whole question is which one lost a message.
3. ⛔ **Vite can serve stale modules.** Verify the served file contains a **behavioural token** (never a
   comment — vite strips them) **after the dev server has settled**, not 4 s after the edit. And ⛔
   never narrate a result your own instrument check just contradicted — I did, and got lucky.
4. ⛔ **`npm run build`, not `npx tsc --noEmit`** — tsc passes on code the build rejects.
5. ⛔ **Stop the dev browser before building the shell** or the linker dies `LNK1104`, and the build you
   then test is the *old* binary. ⛔ Use `scripts/stop-dev.ps1` — **never** kill by image name.
6. ⛔ **The agent environment cannot reach the native input layer.** CDP key dispatch bypasses native
   focus; `PostMessage(WM_CHAR)` to a top-level window does not route to the focused child; `SendInput`
   clicks are dropped. Anything about real mouse/keyboard delivery is a **human** row
   (`HUMAN_TEST_QUEUE.md`), not an agent claim. ⚠️ Item 2 may land here — if it does, say so early
   rather than producing three greens that do not mean what they say.
7. ⛔ **Commit by pathspec, never by directory.** The owner keeps uncommitted work in
   `development-docs/` (e.g. `X402_INTEGRATION.md`, `0.4.0-beta.4/tickets/`). `git add -- <dir>` has
   already swept his edits into one of my commits once.
8. ⛔ **PowerShell here-strings (`@'…'@`) do not work in this shell**, and backticks inside a
   double-quoted `python -c` are eaten by bash. Write a `.ps1` or use a heredoc into `python -`.

## Negative controls

Every acceptance test must be **seen to fail** with the feature off, *for the right reason*
(root `CLAUDE.md`). For this cluster specifically:

- **Item 2:** the control is a reproduction. Until you have one, there is nothing to invert.
- **Item 3:** the RED is the **measured pre-fix gap**, in milliseconds. ⛔ Not "it feels faster".
- **Item 4:** a keyboard conformance test whose subject is **what the address bar contains and where
  focus is** after each key — not that "no error occurred".

## Definition of done

- Each item its **own** commit, citing its row.
- Item 2: reproduced → cause **established by measurement** → fixed → RED observed. If it cannot be
  reproduced, say so plainly and leave it open — ⛔ do **not** fix a bug you never saw.
- Item 3: the gap **in numbers**, before and after. A decision not to change anything is a valid
  outcome if the measurement says so.
- Item 4: a `PRIOR_ART.md` row, the differences between the three browsers written down, a
  recommendation, and 👤 **the owner's decision** before the behaviour changes.
- `npm run build` clean · `scripts/preflight.ps1 -Full` PASS · shell rebuilt explicitly if C++ changed
  (⛔ `-Full` does **not** build `HodosBrowserShell`).
- 🍎 If any shared C++ is touched, add a relay note naming the files (root `CLAUDE.md` build rule).
- Update `phase-11-ui-leftovers/README.md` per item.

## After this cluster

Items 5–10 of Phase 11, then 👤 the owner's requested beta.3 item: give
`TaskSweepReservations` its second verdict — *"observed **spent** ⇒ stop asking and close the row out"*
(`TICKET_reservation_can_be_held_indefinitely.md`). ⭐ That change can only move a row from "reserved
forever" to "terminally spent" and can **never** set `spendable = 1`, which is what makes it safe on
the money path — assert exactly that in its negative control.
