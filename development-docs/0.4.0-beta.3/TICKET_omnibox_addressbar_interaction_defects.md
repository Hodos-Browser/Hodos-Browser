# Omnibox / address-bar interaction — four defects, one of them from a test user

**Status:** 🔴 **OPEN — recorded 2026-08-31, not investigated.** Four owner/test-user observations, captured verbatim during the Phase 3.5 kickoff. ⛔ **None reproduced, none instrumented, no cause established for any of them.**
**Sprint:** 📌 **Not Phase 3.5.** Recorded here because Phase 3.5 is editing the omnibox create/show path and the owner asked whether these were already in scope. 📏 **They are not** — see "Overlap with Phase 3.5" below. Needs owner assignment.

**Reported:** 2026-08-31 by the owner (#2, #3, #4) and by a **test user** (#1).

> Owner's framing: *"please do not get distracted from our current phase and tasks, we can add these
> a ticket for later … this would be a good time to zoom out just a little and add here if we are
> already in these code blocks."*

⚠️ Every line below is 👤 **an observation** or 📖 **a code reading**. Nothing here is 📏 measured, and
no hypothesis in this file has been tested. Recording an untested cause as a cause is the specific
failure this sprint keeps hitting (`HARNESS.md` §8) — so the pointers are offered as *where to start
looking*, explicitly not as findings.

---

## 1. 👤 Cursor does not appear in the address bar after initial launch — **test user**

> *"a test user said they were having trouble getting the cursor to show up in the address bar after
> initial launch. They said they had to click elsewhere (in the search text box is what they said)
> first and then they could click in the address bar and it would then allow them to type."*

⭐ **The most serious of the four**, for three reasons: it is on the **first-run** path, it was found
by **someone who is not us**, and the workaround (click something else first) is not one a new user
would discover. A browser whose address bar appears dead on launch reads as broken.

⚠️ Note the reporter distinguishes *cursor does not show up* from *typing does not work* — they had to
click elsewhere **and then** the address bar accepted input. Whether the first click is lost, or focus
never lands, or the caret is invisible while focus is correct, is **unknown and not the same bug**.
Establish which before designing anything.

**Where to start looking** 📖 — CEF focus handling is the documented fragile area here
(`CLAUDE.md` "CEF Input Patterns": native `<input>` over MUI `TextField`, delayed focus with
`setTimeout(50ms)`, and `browser->GetHost()->SetFocus(true)` alongside Win32 `SetFocus`). The header is
a windowed CEF browser; first-paint/first-focus ordering at startup is the suspect region.
⛔ Do not assume it is the same defect as #2/#3 just because all four are "the address bar".

## 2. 👤 Omnibox overlay sometimes stays open after selecting a URL and navigating

> *"I noticed the omnibox gets stuck open after selection and navigation, I am not sure how or why
> because it is not often and I haven't noticed a pattern but sometimes it says open after I select a
> URL and navigate and then I have to click off to get it to go away."*

⚠️ **Intermittent and pattern-unknown.** Per `HARNESS.md` §8 this stays "not reproduced" until someone
reproduces it. ⭐ An intermittent overlay-lifetime bug is exactly the shape that wasted days in the
farbling work when a plausible cause was adopted early — resist that here.

**Where to start looking** 📖 — the hide is IPC-driven from **two different browsers**, and either
could be the one that is lost:

| Path | Sender | Code |
|---|---|---|
| Click a suggestion | the **omnibox overlay's** browser | `OmniboxOverlayRoot.tsx:187-190` — sends `navigate`, then `omnibox_hide` |
| Enter / Escape in the address bar | the **header** browser | `MainBrowserView.tsx:731`, `:734` |
| Empty input | header | `MainBrowserView.tsx:699` |

📖 Both land on `simple_handler.cpp:2912 "omnibox_hide"` → `HideOmniboxOverlay()`
(`simple_app.cpp:1546`). There are also **five** unconditional `HideOmniboxOverlay()` call sites in
`cef_browser_shell.cpp` (`:865`, `:1157`, `:1482`, `:1530`, `:2611`) driven by window messages
(focus loss, resize, …).

⇒ 🧠 **Hypothesis, untested:** a hide that depends on an IPC from the *overlay's own* browser has a
failure mode the window-message paths do not — if that message is dropped or arrives after a
re-show, the overlay stays visible. **Worth testing first because it is cheap to test, not because it
is likely.**

## 3. 👤 URL appears in the address bar noticeably late after clicking an omnibox suggestion

> *"clicking on a url in the omnibox works functionally (navigates to the page) but it takes a long
> time for the url to load in the address box after the user clicks it. seems like maybe the url
> loading in the address bar after being clicked is being called later than it should."*

📖 The click sends only `navigate` + `omnibox_hide` (`OmniboxOverlayRoot.tsx:187-190`) — it does **not**
carry the URL to the header. So the address bar can only update once navigation reports back through
whatever address-change path the header listens on. ⇒ the owner's read ("called later than it should")
is consistent with the code shape: **there is no optimistic update**.

⚠️ **Consistent-with is not confirmed.** The gap has not been timed, and "a long time" is not a number.
⭐ Phase 2's lesson applies directly: *the control read 2.04 s* — measure the baseline before calling
anything slow. This is also the one of the four with an obvious cheap fix (send the URL with the click
and set it optimistically), which is exactly why it should be **measured before it is fixed**.

## 4. 👤 Tab / Enter autocomplete behaviour should match what users expect

> *"I also kind of want to make sure our tab and enter behavior in the addressbar is standard for
> auto-complete suggested etc. so we have the same feel as what people are accustomed to."*

Not a defect report — a **conformance question**, and the only one of the four that is.

📖 Today (`MainBrowserView.tsx:739`): **Tab**, **ArrowRight** and **End** all accept the inline
autocomplete; **Enter** navigates; **Escape** dismisses and restores the typed text.

⇒ ⭐ This is a `CLAUDE.md` **working rule #5** job before it is a coding job: Chrome, Firefox and
Vivaldi have all settled this, and their answers differ from each other (notably on whether **Tab**
moves focus to the suggestion list vs. accepts inline autocomplete, and on what **Enter** does while a
list item is highlighted). ⛔ Do not "fix" this from intuition — read what they do, record it in
`PRIOR_ART.md`, then decide. A change here alters muscle memory for every user on every navigation.

---

## Overlap with Phase 3.5 — 📏 checked, and the answer is "almost none"

The owner's question was whether these are already in the code Phase 3.5 is editing. Checked rather
than assumed:

| # | In Phase 3.5's edit set? |
|---|---|
| 1 | ❌ No. Focus/first-paint on the header browser; Phase 3.5 touches neither |
| 2 | 🟡 **Adjacent, not overlapping.** Phase 3.5's live site is the omnibox **create-or-show** arm (`simple_handler.cpp:2886`); this is the **hide** path (`:2912`). Same message-handler region, different arm, different mechanism |
| 3 | ❌ No. Navigation → address-bar update; not `ShellWindowProc` and not `ScalePx` |
| 4 | ❌ No. React key handling in `MainBrowserView.tsx` |

⛔ **Nothing here is being fixed in Phase 3.5.** Fixing #2 while in the neighbourhood would blend an
intermittent, unreproduced lifetime bug into a diff whose whole value is that it reverts cleanly
(contract §8), and would put **overlay lifetime** into a phase that is deliberately scoped to overlay
**positioning** (R-CLOSE, contract §4).

⭐ One thing genuinely worth carrying forward: 📖 `HideOmniboxOverlay()` takes **no** `BrowserWindow*`,
unlike all ten `Show*Overlay` functions which take a `targetWin`. For a process-singleton overlay a
global hide is *probably* right — but it is the same show/hide asymmetry Phase 3.5 found on the
create side, so whoever takes #2 should settle it deliberately rather than inherit it.

## Suggested next step

⚠️ These are **four separate defects that share a surface**, not one bug. Do not open a single
"omnibox fixes" branch. #1 is the one with an external reporter and a first-run blast radius; #3 is
the one with a cheap fix that must be measured first; #2 needs a reproduction before anything else;
#4 needs prior-art reading before code.
