# Phase 7a — the consent modal fits the screen · PHASE CONTRACT

**Workstream:** Phase 7 bundle, sub-phase a (see `../phase-7-consent-surface/PHASE_CONTRACT.md` §0)
**Tickets:** `TICKET_modal_buttons_unclickable_small_screen.md` (open since 2026-08-21, **unreproduced**)
**Status:** 🚧 IN PROGRESS — code landed, T0/T1 green, **owner human-eyes pass (`P7a-A4`) owed**
**Opened:** 2026-09-02 · **Owner:** Matthew Archbold · **Platforms:** Windows (macOS = relay)
**Standard:** `../HARNESS.md`. **Base commit:** `887c2cd`

---

## 0. Why this is first, and what changed about the ticket

🚨 **Owner-reported live, 2026-09-02:** *"I just approved a site that asked for a lot and the modal
went off the screen, I could click the top of the button but could not even read them."*

That is a user **granting consent while unable to read what they were granting** — the consent
surface failing in the most direct way it can. It goes ahead of every other row in the bundle.

### The mechanism — ✅ **REPRODUCED 2026-09-02**, see `MEASUREMENTS.md`

```
overlayBackdrop:  position:fixed; inset:0; display:flex;
                  align-items:center; justify-content:center; overflow:hidden
cardStyle:        maxWidth:440px; width:90%     ← no maxHeight, no overflowY
```

The card grows to its content. A flex item centred in a container shorter than itself overflows
**both** ends; `overflow:hidden` on the backdrop means there is no scrollbar, so the overflow is
**unreachable**, not merely below the fold. The buttons sit at the card's bottom.

The variable-height regions inside the connect-bundle card:

| Region | Cap today |
|---|---|
| permission list (protocols/baskets/certs/counterparties) | ✅ `maxHeight:240px; overflowY:auto` |
| **"Who these are with" counterparty footnote** | 🔴 **none** — one line per `securityLevel===2` protocol |
| payment-limits section | grows when expanded |
| `LocalAccessNotice` | conditional |
| the two checkbox labels | wrap to more lines as the card narrows |

### ⚠️ This likely re-diagnoses the ticket, and I will not record that until it reproduces

`TICKET_modal_buttons_unclickable_small_screen.md` carries a section headed *"Why this is probably a
hit-test bug, **not clipping**"* and points at DPI coordinate conversion. Its original report was
*"could not click the buttons on the small screen, could on the large monitor — same build, same
session, only the display changed"* — which a short viewport clipping a tall card explains with **no
DPI involved**. The owner's new detail (*could not read them*) is clipping in plain language; a
hit-test offset leaves buttons perfectly legible.

✅ **It reproduced** (`MEASUREMENTS.md` M1/M2) — on a **1920×1080 monitor at 100% scaling**, with no
small screen and no DPI involved. At **10 declared protocols** the Connect button is 2 px tall and
still clickable. ⇒ The ticket's DPI/hit-test rationale is to be **struck in place** (not deleted) and
the ticket closed here, rather than waiting for the two-monitor rig.

### Design decision — owner, 2026-09-02

> *"based on the content I think two scrollable boxes for permission list and the 'who these are
> with' would be better than making the whole modal scroll."* — **adopted as the design.**
> *"we don't want (b)"* — scroll-to-bottom-before-Connect is **rejected**; not built.

⭐ **Plus a card-level cap as a backstop.** Capping the two lists bounds the regions we thought of;
this modal has grown by the regions nobody thought of (the footnote itself was added in P0.8). The
cap is the invariant, the two boxes are the design. If the design works the cap never engages.

## 1. Goal

A user asked to trust a site can read the whole request and reach both buttons, at every display
size the browser supports — including a site that asks for a great deal.

## 2. Done means

- [ ] At **every** DPI-matrix viewport, the consent card's bottom edge is within the overlay window,
      for a manifest with enough permissions to overflow it today.
- [ ] The counterparty footnote is capped and independently scrollable, like the permission list.
- [ ] A capped list that has more content **looks** like it has more content.
- [ ] No consent modal branch — not just the connect bundle — can place a button off-screen.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-CLOSE` | overlay close guards | This edits the shared notification overlay's root layout. Phase 0.9 left an invisible click-eating full-window overlay on this exact surface. |
| — | **consent legibility** (the thing being fixed) | ⛔ A fix that makes the card fit by *shrinking or hiding* disclosure trades one consent defect for another. Nothing may become unreachable that is reachable today. |

## 4. Evidence table

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail, and how | 🎯 SUBJECT — proves the right thing was measured | Tier | Result |
|---|---|---|---|---|---|
| `P7a-A1` | With a greedy manifest at a short viewport, `card.getBoundingClientRect().bottom <= window.innerHeight` | **Run this BEFORE the fix and observe `bottom > innerHeight`.** That capture is the reproduction of the open ticket — if it does not reproduce, the ticket's DPI hypothesis stands and this contract is wrong | The **notification overlay browser** in the dev build, target `…/brc100-auth?type=idle`, driven via `window.showNotification` — the same path C++ uses. Overlay measured at 1920×1032 CSS px, dpr 1 | T2 | 🔴 **RED OBSERVED** — card 1079 px vs 1032 viewport at N=8; top clipped from N=8, `maxHeight:none`, `overflowY:visible`, backdrop `overflow:hidden` |
| `P7a-A2` | Both buttons are fully within the viewport at matrix cells #4 / #6 / #9 | Same greedy manifest pre-fix → button row bottom exceeds `innerHeight` | The **buttons'** rects specifically | T2 | 🔴 **RED OBSERVED** — N=10: all three buttons **2 px visible (5%) and clickable**; N=11: 0 px. The owner's report, measured |
| `P7a-A3` | The counterparty footnote is capped and scrolls | Feed 20 level-2 protocols → pre-fix the footnote grows unbounded and pushes the buttons out; post-fix it scrolls and they stay put | Count is rendered in the always-visible heading, entries one click away | T2 | 🟢 **GREEN** — collapsed by default; card height **constant at 749 px from N=8 to N=40** (was 1079→1565). Expanded at N=40 the card caps at 1000 px in a 1032 viewport and scrolls; scrolling to the end puts Connect at **100% visible** |
| `P7a-A4` | A list with hidden content is visibly scrollable | 👤 Owner looks at a 20-permission list and says whether it reads as "there is more below". ⛔ **This row cannot be passed by a measurement** — `overflowY:auto` already produces a technically-present scrollbar today, which is exactly the state under suspicion | Owner's eyes on the rendered dark card. ⚠️ **Possible pre-existing defect:** the list has been capped at 240px all along; if its scrollbar is invisible against the dark card, the browser has been silently hiding permissions | T3 👤 | ⬜ |
| `P7a-A5` | No modal branch can place a button off-screen | Remove the card cap, re-run the greedy fixture against each branch → the branches with long content go red | **All** branches — the cap is on the shared `cardStyle` | T2 | 🟢 **GREEN** — `maxHeight` computes to 944 px on all five branches sampled; all fit the viewport. 🆕 `certificate_disclosure` (40 fields) has its **own uncapped list** — now reachable by scrolling where it was previously off-screen. Logged for 7b, not fixed here (M8) |
| `P7a-A6` | Nothing reachable before is unreachable now | Diff the rendered text content of each branch pre/post at a tall viewport → identical | Rendered text, not the JSX | T2 | 🟡 **PARTIAL — by design.** Every branch renders identical content. The connect modal's counterparty identifiers are now **one click away instead of inline** — the owner's deliberate change, with the count and heading always on screen per BRC-116 §4.1. ⛔ Stated as a change, not claimed as a no-op |

**Fixture.** `applyParams` reads the query string on initial load as well as from JS injection, so the
modal is drivable directly:
`/brc100-auth?type=manifest_connect_bundle&domain=<d>&manifest=<urlencoded JSON>`.
No wallet, no site, no C++ round-trip needed to vary permission count — which is what makes `A1`'s
RED cheap enough to run before writing any fix.

### ⚠️ Amendment 2026-09-02 — a trap found before writing the fix

`InfoIcon` renders its tooltip `position:absolute` **outside** the icon's box, so any ancestor with
`overflow` clips it. Today no tooltip sits inside a scrolling container. **Capping the footnote would
be the first place the product clips a consent tooltip** — trading disclosure for layout, which §3
forbids. Options are in the report to the owner; `A7` below covers whichever is chosen.

| ID | 🟢 GREEN | 🔴 RED | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P7a-A7` | No `InfoIcon` tooltip is clipped by any container this phase adds | Hover each tooltip-bearing icon at the largest fixture; pre-fix all render fully — a post-fix clip is the regression | Tooltip rect vs the card's box, hovering **one icon at a time** and counting only icons a pointer can reach | T2 | 🟢 **GREEN** — 0 clipped across 56 hovers (collapsed, and expanded at scrollTop 0/300/565). ⚠️ A cruder all-at-once version reported 5–8 false clips; see M7 |

## 5. Blast radius

- `cardStyle` and `overlayBackdrop` are **shared by every branch** in `BRC100AuthOverlayRoot.tsx`.
  A height cap changes all ~12 consent modals at once. That is why `A5` and `A6` sweep all branches
  rather than the one that was reported.
- The overlay **window** height comes from C++ (`CreateNotificationOverlay`). If the window itself is
  short, a CSS cap makes the card scroll rather than fixing the geometry — `A1`'s subject note
  exists so that this is measured rather than assumed.
- ⛔ **Not touched:** the permission list's existing 240px cap (unless `A4` says it hides content),
  the merge to one view (7b), and anything about what the modal *says*.

## 6. Out of scope

- Scroll-to-bottom-before-Connect — owner-rejected above.
- The one-view merge and any re-ordering of the modal's content — **7b**. ⚠️ Recorded because it is
  tempting: while capping the footnote it is obvious it should also be collapsed by default. That is
  a design change and belongs with the merge, where the owner sees pixels.
- The DPI/hit-test investigation. If `A1` reproduces, that hypothesis is *struck*, not pursued.

## 7. Rollback

One commit touching two style objects and one wrapper element. `git revert` restores the current
(overflowing) layout exactly. No state, no schema, no C++.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1` + `-NegativeControl`
- [ ] `../REGRESSION_SET.md` at the 7 boundary
- [ ] 👤 Owner has looked at the rendered modal (`P7a-A4`)
- [ ] `TICKET_modal_buttons_unclickable_small_screen.md` updated in place with the verdict
- [ ] Commit messages cite the row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight | 🟢 **PASS** (`-Full`, all T0+T1 ran) | 2026-09-02 | assistant |
| preflight -NegativeControl | 🟢 **PASS** — every gate seen to fail | 2026-09-02 | assistant |
| regression set | | | |
| owner human-eyes pass | | | |
