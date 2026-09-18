# Phase 11 — UI / layout leftovers (was Phase 10; renumbered 2026-09-15)

**Opened:** 2026-09-15 as a folder; the bundle itself dates from 2026-08-31 (`../SPRINT_PLAN.md` §4.1).
**Status:** ⬜ PLANNED — kickoff not run. **Standard:** `../HARNESS.md`. Runs **after** Phase 10.

## What is in it

| # | Item | Source | Shape |
|---|---|---|---|
| 1 | ✅ **DONE 2026-09-18** — **Cursor is not in the address bar at launch** — a test user had to click elsewhere first | `../TICKET_omnibox_addressbar_interaction_defects.md` #1 | ⭐ First-run blast radius, external reporter. Establish *which* defect first (focus never lands / first click lost / caret invisible) with an instrumented probe on the header browser at startup; then the fix. 👤 Owner's target: on launch, focus is in the address bar with the caret visible, ready to type. T2 probe + T3 human check |
| 2 | **Omnibox sometimes stays open after selecting a URL** | ticket #2 | reproduce before anything; the hide path (`omnibox_hide`) is the suspect, not Phase 3.5's create/show arm |
| 3 | **URL populates the address bar late after clicking a suggestion** | ticket #3 | measure the gap first (Phase 2's lesson: the control read 2.04 s); then send the URL with the click and set it optimistically. 👤 Owner: the page navigating while the bar still shows the old URL reads as "the click did nothing" |
| 4 | **Tab / Enter autocomplete behaviour** | ticket #4 | prior-art read (Chrome, Firefox, Brave omnibox key handling) before code |
| 5 | **Tear-off window overlay sweep** | 👤 owner 2026-09-15: typing in a torn-off tab's address bar makes that window disappear | Phase 3.5 measured and fixed this for Ctrl+N windows (K9: z-order occlusion; overlays now owned by the requesting window, owner-confirmed `Z5`). It was **never measured on a torn-off window** (`tab_tearoff` → `CreateFullWindow`, same creator). Re-run the Phase 3.5 `winprobe.ps1` z-order read on a torn-off window, **every overlay** (the 3.5 doc predicted "generalises beyond the omnibox" and said "not yet tested on a second overlay"), with `OwnOverlayToRequestingWindow` reverted as the negative control. If it is green, the owner's observation predates the fix and the row closes; if red, tear-off differs and gets fixed here |
| 6 | `modal_buttons_unclickable_small_screen` | old bundle | may already be covered by 7a's viewport work — verify, do not re-do |
| 7 | ❔ `chrome_ui_scales_but_its_window_does_not` | old bundle | may collapse into 3.5 — verify |
| 8 | ❔ `longlived_surfaces_snapshot_state_at_startup` | old bundle | |
| 9 | ❔ `disable_features_autofill_is_a_noop` | old bundle | |
| 10 | Phase 1's overlay dead strip · the DPI matrix overlay section | old bundle | `DPI_RESOLUTION_TEST_MATRIX.md` cells #4/#6/#9 |
| **11** | 🔴 **A BRC-121 payment shows the user NOTHING while it spends** | `../TICKET_brc121_paid_retry_aborts_and_mints_a_payment_each_time.md` | ⛔ **RUNS FIRST — money path, and the only item here that costs real BSV when it goes wrong.** Four fixes in causal order, plan + negative controls in the ticket. 👤 Owner 2026-09-17: *"it's not okay."* See below |

## Item 11 — why it is here and why it goes first

Folded in 👤 by owner decision 2026-09-17, out of the payment sitting. It is a UI defect, which is why
it lands in this phase — but it is a UI defect **on the shipped money path**, so it is not queued
behind the omnibox cluster.

**Measured, 2026-09-16** (`debug_output-40428.log` 21703–21893): one paywalled article took ≈41 s and
minted **three** payments. Of that, **34.5 s was the site's own origin** (`cfOrigin;dur=34481` — not
ours, not fixable by us). The rest was ours: we showed the user nothing, so they clicked again, and
**each click cancelled the paid request in flight** and caused a fresh 402 to be paid. Proof it was
clicks and not our reload: the third attempt requests a **different article**.

⭐ **The root defect is one sentence.** `PaymentPendingPage` is only ever rendered *behind the
domain-approval modal*, so on the **silent auto-approved path — the normal path — nothing ever
reaches the screen.** We built the "we are paying" screen only for the case where the user was
already being told.

⇒ Fix order is causal, not severity-ordered: **feedback → honest timeout → don't re-mint on
navigation → release the abandoned transaction.** Fixing the feedback removes the clicks, which
removes the aborts and the extra mints.

⛔ Negative controls are in the ticket and each names its **subject**: the tab's rendered document
(not a log line), the count of `nosend` rows (not an HTTP status), the phantom outputs and the
reservation. ⚠️ No money is required for the GREEN halves — a stubbed slow retry reproduces it.

🍎 macOS: the pending screen is React and relays; the navigation/cancel arm is shared C++
(`HttpRequestInterceptor.cpp`) ⇒ relay note naming the files, per the root `CLAUDE.md` build rule.

## Harness notes

- Items 1–5 are **five separate defects sharing a surface** (the ticket says so); one contract, one row each, no
  single "omnibox fixes" branch.
- **Item 11 runs before items 1–10** and gets its own contract; it shares no surface with the omnibox cluster.
- Item 1 is the one with a T3 human row: a person launching the installed build and typing without clicking.
  Its instrumented half: a startup probe that reads which HWND has focus and whether the header's `<input>` is
  `document.activeElement` at first paint, RED = the current binary.
- Item 5's instrument exists (`../phase-3.5-layout-window-scoping/` winprobe); reuse it, do not write a new one.
- Hard-reload before every React measurement (Vite HMR fakes negative controls — memory).
- 🍎 macOS: items 1–4 are React + header focus (CEF focus handling differs; relay for their eyes); item 5 has
  its own macOS half already in `HUMAN_TEST_QUEUE.md` B4.

---

## Item 1 — cursor does not appear in the address bar at launch · ✅ DONE 2026-09-18

👤 Reported by a **test user**: *"had to click elsewhere first and then they could click in the
address bar and it would then allow them to type."* ⭐ The only one of the four with an external
reporter and a first-run blast radius.

### Which of the three defects it is — measured, not argued

The ticket insists on establishing *which* before designing, because three different bugs produce that
one sentence. 📏 Measured on a fresh launch **before** any change:

| Question | Answer |
|---|---|
| `document.activeElement` at startup | **`BODY`** |
| Does a real input event focus it? | ✅ yes |
| Does programmatic `.focus()` work? | ✅ yes |

⇒ **(a) focus never lands** — nothing *tries*. Confirmed in code: there is no `autoFocus` and no
mount-time focus anywhere in `MainBrowserView.tsx`; the only two focus calls are **reactive** (the
`focus_address_bar` IPC and the Ctrl+L shortcut).

⛔ **Boundary of what this proves.** The click was a CDP input event delivered to the header browser.
A user's click travels OS → HWND → CEF's input pipeline — a different path, and the one CEF is
documented to be fragile about. So the **renderer** side is cleared; the **native** side is not.
⬜ Whether a real first click is lost remains **unproven in both directions** and needs a human
(SendInput clicks are dropped in this agent environment). That is the test user's *"click elsewhere
first"* half, and it is **not** claimed fixed.

### The rule shipped — deliberately about CONTENT, not about how the window was made

👤 Owner asked for "safest, fresh launch only" and asked why tear-off was risky. It is not; the right
axis dissolves the question:

> **Focus the address bar when the window opens on an empty new-tab page. Never when it opens on a
> real page.**

| Case | Opens on | Focus |
|---|---|---|
| Fresh launch → NTP | nothing | ✅ |
| Fresh launch → restored session | a page | ❌ |
| Ctrl+N new window | NTP | ✅ |
| **Tear-off** | the dragged tab's page | ❌ |

⛔ Keying off *creation type* would get tear-off wrong — tear-off and Ctrl+N both come from
`CreateFullWindow`, but one arrives showing content and the other empty. Content is the axis that
answers every case without a list of exceptions.

**Guards:** once per window (`hasAutoFocusedRef`, never reset) · stands down **permanently** if the
window opened on content, so a later navigation to the NTP cannot grab focus · does not steal focus if
anything else already holds it when the timer fires · 60 ms delay, the documented CEF pattern (root
`CLAUDE.md` "CEF Input Patterns") — a bare focus on mount races the header browser's own first focus
and is silently dropped.

### Evidence

| Run | activeElement | is the address bar |
|---|---|---|
| 🟢 fresh launch on the NTP | `INPUT` | ✅ **with caret** |
| 🟢 window opened on content (header re-mounted while the tab showed Wikipedia) | `BODY` | ❌ correctly stood down |
| 🔴 feature disabled, fresh launch on the NTP | `BODY` | ❌ |

⛔ **The Vite trap nearly ate the negative control.** My verification curl ran 4 s after the edit and
reported the control **absent** — and I printed "RED as expected" anyway. Re-checked: Vite simply had
not picked the edit up at that instant. The RED was then re-run with the control **verified present in
the served module before the browser launched**. ⇒ verify the served code *after* the dev server has
settled, and ⛔ never narrate a result your own instrument check just contradicted.

`npm run build` clean (⛔ not `npx tsc --noEmit`, which passes on code the build rejects).


### ⛔ Follow-up, same day — my fix produced TWO carets, and my measurement had been scoped too narrowly

👤 Owner on a fresh launch: *"I see the cursor in the search bar in the middle of the new tab and not
in the address bar… when I clicked in the address bar, the cursor showed up in there. But the cursor
in the search bar in the web page view still stayed in there and was still blinking."*

**Both halves of that are findings.**

**1. The premise of item 1 was narrower than I wrote.** `NewTabPage.tsx` has auto-focused its own
search box since long before today (a 100 ms timer on mount). So on launch there *was* a caret — in
the new-tab page's search box, not in the header. My probe read the **header browser only**,
concluded "focus never lands", and that was true *of the header* while being misleading about what
the user sees.

⛔ This is a cousin of the farbling defect the root `CLAUDE.md` records — *"a third drove the wrong
browser … which faked an intermittent per-session bug in code that was fine."* I drove the **right**
browser for the question I asked; the question was scoped too narrowly to describe the product.
⇒ **On a multi-browser surface, measure every browser that can hold the thing you are asking about.**

**2. My fix then made it two carets.** The header and the new-tab page are **separate CEF browser
processes with separate documents**, so each renders its own caret independently; only one receives
keystrokes. Typing went to the address bar and worked — it just looked wrong.

**Fixed:** `NewTabPage.tsx` no longer auto-focuses. The address bar wins on the new-tab page, which is
the owner's stated target and what Chrome does.

| Run | HEADER `activeElement` | NEW TAB `activeElement` | carets |
|---|---|---|---|
| 🔴 before | `INPUT` (address bar) | `INPUT` (search box) | **2** |
| 🟢 after | `INPUT` (address bar) | `BODY` | **1** |

⭐ The green is asserted **across both browsers in one run**, which is the correction the mistake
above demands.

### ⚠️ Two claims from the original ticket now look weaker

- 👤 The owner clicked the address bar and typing worked **immediately** — evidence *against* the test
  user's *"had to click elsewhere first"*, at least on this machine. ⇒ either it is
  environment-specific, or the report really meant *"the cursor was not where I expected"*. ⛔ Worth
  **asking the reporter** before chasing a native-focus bug that may not exist.
- ⇒ item 1's native half stays **unproven in both directions**, and is now also **less likely**.

### ⬜ Recorded, not done — make the new-tab box forward focus (option "C")

The two fields are functionally the same: the new-tab box uses the **same** `isUrl` / `normalizeUrl` /
`toSearchUrl` utilities and the same search-engine setting as the address bar.

**The one real difference:** the address bar has the omnibox dropdown — history suggestions and inline
autocomplete. `NewTabPage.tsx` contains **zero** omnibox references. So the same keystrokes in the
middle box give a worse result.

⛔ **I first called this "real work across two browsers" and that was wrong.** The route already
exists: `focus_address_bar` is wired end to end (C++ `simple_handler.cpp` → render handler →
`MainBrowserView.tsx`) and is what Ctrl+L uses. The missing piece is only that the new-tab page does
not *send* it. 👤 Owner's instinct — *"don't we already have C?"* — was closer to right than my
answer.

### ⬜ Still open in this cluster

- **Item 1's native half** — whether a real first click is lost. Needs a human; not claimed.
- **Item 2** (omnibox stuck open) — needs a *reproduction* before anything else.
- **Item 3** (late URL in the address bar) — the gap must be **timed** before the obvious fix; Phase 2's
  lesson (the control read 2.04 s) applies directly.
- **Item 4** (Tab/Enter autocomplete) — working rule 5: read Chrome/Firefox/Vivaldi and record in
  `PRIOR_ART.md` **before** code. A change here alters muscle memory on every navigation.
