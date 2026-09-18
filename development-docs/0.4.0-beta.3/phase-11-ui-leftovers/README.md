# Phase 11 — UI / layout leftovers (was Phase 10; renumbered 2026-09-15)

**Opened:** 2026-09-15 as a folder; the bundle itself dates from 2026-08-31 (`../SPRINT_PLAN.md` §4.1).
**Status:** ⬜ PLANNED — kickoff not run. **Standard:** `../HARNESS.md`. Runs **after** Phase 10.

## What is in it

| # | Item | Source | Shape |
|---|---|---|---|
| 1 | 🟡 **DIAGNOSED, NOT FIXED (2026-09-18 — shipped attempt REVERTED)** — **Cursor is not in the address bar at launch** — a test user had to click elsewhere first | `../TICKET_omnibox_addressbar_interaction_defects.md` #1 | ⭐ First-run blast radius, external reporter. Establish *which* defect first (focus never lands / first click lost / caret invisible) with an instrumented probe on the header browser at startup; then the fix. 👤 Owner's target: on launch, focus is in the address bar with the caret visible, ready to type. T2 probe + T3 human check |
| 2 | ✅ **DONE 2026-09-18** — **Omnibox sometimes stays open after selecting a URL** | ticket #2 | 📏 **Reproduced deterministically.** Not the hide path — an uncancelled 150 ms **show** debounce in the header fires after the hide. See § Item 2 below |
| 3 | ✅ **DONE 2026-09-18** — **URL populates the address bar late after clicking a suggestion** | ticket #3 | 📏 **Not "late" — never.** Measured pre-fix: page navigated at 122 ms, clicked URL absent from the address bar after **35 s**. Fixed: **75 ms**, ahead of the navigation. See § Item 3 below |
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

## Item 1 — cursor does not appear in the address bar at launch · 🟡 DIAGNOSED, NOT FIXED

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


### ⚠️ What the green actually proves — and the one row still owed to a human

👤 Owner, next launch: *"I didn't see a cursor in either the address bar or the search box… I clicked
in the address bar, started typing, it worked fine for me, but I should have just started typing and
seen if it went into the address bar."*

⛔ **My green measured `document.activeElement` — that focus was ASSIGNED. A caret only RENDERS when
the window also has OS focus.** The rig launches the browser with `Start-Process`, which does not
necessarily bring it to the foreground, so `activeElement` can be the address bar with nothing
blinking on screen. ⇒ the measurement is sound for what it asked and **narrower than it reads**: it
does not establish that a real user sees a caret on a real launch. Same class of error as measuring
one browser, in a different dimension.

⬜ **The row that settles it, and it needs a human** (`HUMAN_TEST_QUEUE.md`):

> Launch the installed build yourself, click **nothing**, and **just start typing**. If the characters
> land in the address bar, item 1 is done for real.

That is also the test the original reporter's complaint was really about, and ⭐ it is the owner's own
formulation.

### 📖 Standard behaviour, checked because the owner asked

👤 *"now that I click off of it, the cursor's not in there… the browser itself is in focus and it
doesn't put the cursor back in the address bar. Is that what Brave and Chrome have?"*

✅ **Yes — that is correct and standard.** Chrome, Brave and Firefox all restore focus to whatever held
it when a window regains OS focus, and to the page when nothing did. ⛔ Re-focusing the address bar on
every window activation would hijack typing on every alt-tab. **No change needed; behaviour matches
the reference browsers.**

### ⛔ DECLINED 2026-09-18 — do NOT make the new-tab box forward focus (option "C")

👤 Owner: *"I don't see the point of C... The user starts typing, it looks for their history. If
they type something that's wrong or that they just type something without a dot domain, it just does
the search. That's fine. I don't understand C. I think A is fine."*

⭐ **The reasoning is right and settles it.** The address bar already does the whole job — history
lookup, search fallback, URL detection. C only changed *where the user types*, and there is no problem
with where the user types. ⛔ Do not re-propose it; the note below is kept only so the option is
understood rather than rediscovered.

**Why it was ever raised:**

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


---

## Item 1 — OUTCOME 2026-09-18: attempt REVERTED, defect diagnosed but not fixed

🟡 **Everything below the diagnosis was reverted. The tree is back to pre-2026-09-18 behaviour:
one caret, in the new-tab search box, and "launch and type" works.**

### What the three attempts established

👤 Owner, testing each build by launching and typing without clicking:

| Attempt | Change | Result |
|---|---|---|
| 1 | Focus the address bar from React (`MainBrowserView.tsx`) | Address bar got DOM focus **and** the new-tab box kept its own ⇒ **two carets** |
| 2 | Also stop the new-tab box auto-focusing (`NewTabPage.tsx`) | One caret — but typing did **nothing at all**. ⛔ **Worse than the original**, which at least accepted keystrokes |
| 3 | Also give the header **native** focus at tab registration (`TabManager.cpp`) | The new code **fired** (log line confirmed) and behaviour was **unchanged** |

### ⭐ The real finding — two layers of focus, and only one was ever in question

| Layer | What it decides | Who sets it at startup |
|---|---|---|
| **DOM focus** | which element receives keys **once they arrive at a browser** | the page's own React |
| **NATIVE focus** | whether keys **arrive at that browser at all** | `TabManager::RegisterTabBrowser` → **the TAB** |

⇒ The new-tab search box holding DOM focus is the **only** reason "launch and type" works today.
Removing it left the header holding DOM focus it could not use, so keystrokes reached **nobody**.

⛔ **And the part that is NOT understood:** `header->GetHost()->SetFocus(true)` at registration ran —
confirmed by its own log line — and changed nothing. CEF's documented advice for windowed browsers is
exactly that call (root `CLAUDE.md`, "CEF Input Patterns"), so the cause sits **below** it. No
hypothesis is recorded here on purpose; this ticket already burned a day on plausible-sounding ones.

### ⛔ Before anyone writes more code here

1. **Establish where native keyboard focus actually lands at startup** — which HWND, and which CEF
   browser believes it has focus — *before* changing anything. That measurement does not exist yet.
2. ⚠️ **The agent environment cannot settle it.** CDP key dispatch targets a browser directly and
   bypasses native focus entirely; `PostMessage(WM_CHAR)` to a top-level window does not route to the
   focused child; `SendInput` clicks are dropped here. Every green in this session that *looked* like
   progress was measuring a layer the user does not experience. ⇒ this needs a human at the keyboard,
   and `HUMAN_TEST_QUEUE.md` **W8** is that row.
3. ⭐ **Three times in one session a measurement was scoped to the wrong layer** — one browser instead
   of two, `activeElement` instead of a visible caret, DOM focus instead of native focus — and each
   time it produced a confident green that did not survive the owner typing. **On a multi-process UI,
   name the layer your instrument reads before you report what it means.**

### What survives

- The diagnosis above, and the two in-code warnings (`NewTabPage.tsx`, `MainBrowserView.tsx`) telling
  the next person why the obvious change is wrong on its own.
- `W8` in the human queue.
- 👤 The owner's report that clicking the address bar works immediately, which makes the test user's
  *"had to click elsewhere first"* look like **"the caret was not where I expected"** rather than a
  second, separate defect. ⭐ Worth **asking the reporter** before treating it as one.

---

## Item 3 — the URL never reaches the address bar after a suggestion click · ✅ DONE 2026-09-18

👤 *"clicking on a url in the omnibox works functionally (navigates to the page) but it takes a long
time for the url to load in the address box after the user clicks it."*

### 📏 Measured first, as the ticket demanded — and "a long time" is not the finding

Instrument: `omniboxprobe.py item3` — **DOM layer**, the header browser's `<input>.value`, with the
tab parked on the new-tab page so the clicked URL cannot already be there. Subject printed each run
(dev pid, header target, omnibox target; dev CDP port 9322 only).

| | pre-fix | post-fix |
|---|---|---|
| `tB` page navigated | 122 ms | 99 ms |
| `tA` **clicked URL** in the address bar | 🔴 **NOT WITHIN 35 s** | 🟢 **75 ms** |
| address bar final value | `''` (the parked NTP's) | `https://example.com/` |

⭐ The bar now leads the navigation (75 ms < 99 ms), which is what Chrome does.
⭐ Contrast row, measured on the same build: the **Enter** path was never broken — 235 ms.

### The cause, and ⛔ a correction to my own first statement of it

📖 The click sends only `navigate` + `omnibox_hide`; neither carries the URL to the header, and the
header's own update path is the *tab list*, which C++ pushes on `OnTitleChange` — `OnAddressChange`
pushes nothing, and `useTabManager`'s safety poll is **30 s**.

But that is not what made it *never*. The header's "sync address bar with the active tab's URL"
effect is guarded by `if (!isEditingAddress)`, and:

> ⛔ **I first wrote that clicking a suggestion "never blurs the header input" (because the omnibox
> WndProc returns `MA_NOACTIVATE` so the dropdown cannot steal the caret). That is wrong, and the
> negative control is what caught it.** 📏 Measured focus events on the header `<input>`:
>
> ```
>    49 ms  blur   value='exam'
>    67 ms  focus  value='exam'
> ```
>
> It blurs for **~18 ms and re-focuses**. So the sync effect does not run zero times — it runs
> **exactly once, with the pre-navigation URL**, and is blocked from then on. That is why the bar
> showed the *previous page's* URL in one control run and the *typed fragment* in another: which one
> you get depends only on whether that 18 ms window lands before or after the tab URL updates. Both
> are the same defect and both are wrong.

### The fix

`OmniboxOverlayRoot.tsx` sends a new `omnibox_navigated` IPC carrying the URL **before** `navigate`.
C++ forwards it to the **owning window's** header; the header mirrors its own Enter branch — sets the
address, leaves edit mode, snapshots the pre-nav URL so tab sync suppresses the stale push.

⛔ **Reuse considered and rejected:** the existing `omnibox_autocomplete` channel already runs
overlay → C++ → header and would have shown the URL with no new message. It was rejected because it
is a *preview* of an arrow-key selection and deliberately **leaves the header in edit mode** — reusing
it would have fixed the visible symptom and left the address bar frozen for every redirect after it.

⛔ **Routed to `GetOwnerWindow()->header_browser`, not `SimpleHandler::GetHeaderBrowser()`** — the
latter resolves the **primary** window's header (it is one of the `G11` sites), so in a second window
the URL would have landed in the wrong address bar. `ShowOmniboxOverlay()` already retargets this
handler's `window_id` to the requesting window, which is what makes the owner lookup correct.

### 🔴 Negative control — run, and it failed for the right reason

The `omnibox_navigated` send was deleted from `OmniboxOverlayRoot.tsx`, the served module verified to
**lack** the token *after the dev server had settled* (⛔ the Vite trap that nearly ate item 1's
control), both browsers hard-reloaded, and the log confirmed C++ received no `omnibox_navigated` on
the control run. Result: page navigated at 122 ms, clicked URL **never** appeared in 35 s.

⚠️ **The control also condemned the first version of the probe.** Its `tA` asserted only that the
address bar value *changed* — and with the feature off it changes at ~99 ms, to the wrong URL. That
probe scored the broken build green. `tA` now requires the **clicked host** to appear. Recorded here
because it is exactly the family this harness exists for.

### 🍎 macOS

Shared C++ (`simple_handler.cpp`, `simple_render_process_handler.cpp`), no platform split added —
relay note in `../MAC_RELAY_P11_ROUND.md`. The *cause* cited above is Windows-specific in one detail
(`MA_NOACTIVATE`); whether macOS shows the same focus flicker is ⬜ unmeasured, but the fix is
correct on both and macOS gets it from the shared files.

---

## Item 2 — the omnibox stays open after selecting a URL · ✅ DONE 2026-09-18

👤 *"it gets stuck open after selection and navigation… it's not often and I haven't noticed a
pattern… I have to click off to get it to go away."*

### 📏 Reproduced first, deterministically — and the ticket's suspect was the wrong half

⛔ The ticket and the session prompt both pointed at the **hide** path: *"a hide that depends on an
IPC from the overlay's own browser can be lost or arrive after a re-show."* Tested because it was
cheap. **No hide is ever lost.** The defect is on the **show** side, and it is not intermittent at
all once you know the variable.

`MainBrowserView.tsx`'s `onChange` schedules a **150 ms debounce** that sends `omnibox_update_query`
+ `omnibox_show`. Nothing cancelled it — not Enter, not Escape, not blur, not unmount, and the header
never learned that the overlay had hidden itself after a suggestion click. So if the user commits
within 150 ms of their last keystroke, the timer fires *after* the hide and re-shows the overlay,
and nothing hides it again.

Instrument: `omniboxprobe.py item2 <gap_ms>` / `item2click <prefix> <gap_ms>` — **HWND layer**,
`IsWindowVisible()` on the real `CEFOmniboxOverlayWindow`, sampled every 20 ms on its own thread
while the action is driven over CDP. The single variable is the gap between the last keystroke and
the commit.

| Path | gap | transitions (ms from the last keystroke) | stuck open |
|---|---|---|---|
| Enter | 40 | `0:VIS 54:hid 233:VIS` | 🔴 **yes** |
| Enter | 60 | `0:VIS 87:hid 168:VIS` | 🔴 **yes** |
| Enter | **200** | `0:VIS 225:hid` | 🟢 no |
| Enter | 800 | `0:VIS 826:hid` | 🟢 no |
| Click a suggestion | 40 | `0:VIS 81:hid 173:VIS` | 🔴 **yes** |
| Click a suggestion | 800 | `0:VIS 844:hid` | 🟢 no |

⭐ The boundary is exactly 150 ms, on both paths. That **is** the "no pattern": it depends on nothing
but how fast you hit Enter after the last letter, which is why it looked random.

### The fix

One helper, `cancelPendingOmniboxShow()`, called by every path that dismisses the dropdown: Enter,
Escape, blur, unmount, and the `omnibox_navigated` message from item 3 — which is the **only** way
the header can learn a suggestion was clicked, and the reason item 3 was committed first.

⭐ **The two-sided check is built into the same run:** every row prints
`after typing … visible=True` before the commit. A fix that cancelled too eagerly would stop the
dropdown appearing at all and would fail that line, in the same run, without a second test.

### 🔴 Negative control — run and observed, both paths

The two lines inside `cancelPendingOmniboxShow` were replaced with a comment (the eight call sites
left in place, so the control disables the *mechanism*, not the wiring), the served module verified
to no longer contain `omniboxDebounceRef.current = null` after the dev server had settled, and both
browsers hard-reloaded:

| | control | result |
|---|---|---|
| Enter, 40 ms | cancellation off | 🔴 `0:VIS 86:hid 194:VIS` — stuck open |
| Click, 40 ms | cancellation off | 🔴 `0:VIS 83:hid 180:VIS` — stuck open |
| Enter, 40 ms | restored | 🟢 `0:VIS 84:hid` |
| Click, 40 ms | restored | 🟢 `0:VIS 89:hid` |

### ⭐ `HideOmniboxOverlay()` takes no `BrowserWindow*` — settled deliberately, **keep it**

The prompt asked for this to be decided rather than inherited. Decision: **no change.** There is
exactly one omnibox HWND per process (`g_omnibox_overlay_hwnd` is a single global), so a hide has
nothing to choose. `Show*Overlay(targetWin)` takes a window because it must choose a **position** and
an **owner**; adding the same parameter to the hide would advertise per-window overlays that do not
exist. ⚠️ Related correction to the ticket: its five `cef_browser_shell.cpp` hide sites are described
as *"unconditional"* — they are **not**. All five are guarded by `IsWindowVisible`. (Their line
numbers have also moved: 873/882, 1163, 1474, 1522, 2537.)

### 🍎 macOS

React-only — no rebuild needed. ⬜ The HWND half of the evidence has no macOS analogue yet
(`omniboxprobe.py` reads `user32!IsWindowVisible`); noted in `../MAC_RELAY_P11_ROUND.md`.
