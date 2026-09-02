# Phase 7a — measurements

**Run 2026-09-02, before any code change.** Base `887c2cd`.

**SUBJECT.** Dev build `cef-native/build/bin/Release/HodosBrowser.exe`, `HODOS_DEV=1`,
`--remote-debugging-port=9322`. CDP target attached by URL:
`http://127.0.0.1:5137/brc100-auth?type=idle` — the **notification overlay browser**, not a tab and
not the header. Driven through `window.showNotification(<query>)`, the same injection path
`CreateNotificationOverlayTask` uses. Overlay window measured at **1920×1032 CSS px, dpr 1** — i.e.
a full 1920×1080 monitor at **100% scaling**, taskbar excluded. ⛔ Not a small screen, not scaled.

Fixture: `mkfixture.py N` — N level-2 protocols, N/2 baskets, N/4 certificates, N/4 counterparties.
Rig: `measure.py`, `whatclipped.py`, both alongside this file.

## M1 — card geometry vs viewport (`P7a-A1`, `P7a-A2` RED)

| N protocols | card height | card top | overflows by | top clipped | Connect bottom | Connect fully visible |
|---|---|---|---|---|---|---|
| 2 | 776 | 128 | — | no | 876 | ✅ |
| 4 | 917 | 58 | — | no | 946 | ✅ |
| 6 | 998 | 17 | — | no | 987 | ✅ |
| **8** | 1079 | **−23** | 23 | 🔴 **yes** | 1027 | ✅ |
| **10** | 1160 | −64 | 64 | 🔴 yes | **1068** | 🔴 **no** |
| 14 | 1322 | −145 | 145 | 🔴 yes | 1149 | 🔴 no |
| 20 | 1565 | −266 | 266 | 🔴 yes | 1270 | 🔴 no |

Confirmed computed styles at every N: `cardMaxHeight: "none"`, `cardOverflowY: "visible"`,
`backdropOverflow: "hidden"`. **The mechanism is as predicted**: an uncapped card centred by flex in
a container with no scrollbar, overflowing both ends.

Growth is **~81 px per declared protocol** — each one adds a permission-list row *and* an uncapped
"Who these are with" footnote row.

## M2 — 🚨 the owner's exact symptom, measured (`P7a-A2`)

> *"I could click the top of the button but could not even read them."* — owner, 2026-09-02

| N | Decline | Customize | Connect | |
|---|---|---|---|---|
| 9 | 22 px visible (59%) | 22 px (59%) | 22 px (59%) | clickable, partly legible |
| **10** | **2 px (5%)** | **2 px (5%)** | **2 px (5%)** | 🔴 **clickable, unreadable** |
| 11 | 0 px (0%) | 0 px (0%) | 0 px (0%) | not clickable |

⛔ **N=10 is a fail-OPEN band, and it is the worst state of the three.** All three buttons —
*Decline*, *Customize*, *Connect* — are side by side in a **2-pixel strip**. They are clickable and
their labels are unreadable, so a user aiming at that strip is choosing between refuse, review and
grant with nothing to distinguish them. N=11 fails *closed* (nothing clickable). N=10 does not.

This is the hazard `TICKET_modal_buttons_unclickable_small_screen.md` describes — *"a user who aims
at Deny … could land on Allow — a security bug wearing a layout bug's clothes"* — reached by
illegibility rather than by coordinate translation, with the same outcome.

## M3 — what is lost off the top

At **N=20** (`cardTop −266`), entirely above the viewport and unreachable:

```
ABOVE  top=-171 bot=-149   Greedy Test dApp          ← the site's declared name
ABOVE  top=-147 bot=-131   greedy.example            ← the DOMAIN being authorised
ABOVE  top=-127 bot=-109   fixture                   ← its description
ABOVE  top=-93  bot=-74    This site is asking permission to:
```

⭐ **Checked, and the alarming version is NOT true.** I expected a band where the site's identity is
clipped while Connect is still clickable — i.e. approving a site you cannot see the name of. Measured
at N=10: the only off-screen content is the button row; the domain is still visible (it sits ~119 px
into the card, and by the time `cardTop < −119` the buttons are long gone). **The identity goes after
the buttons, not before.** Recorded because it was nearly asserted from the N=20 reading alone.

## M4 — ⚠️ trap found before writing the fix: `overflow` clips tooltips

`InfoIcon`'s tooltip is `position:absolute; bottom:calc(100% + 6px)` on a `position:relative`
wrapper — it renders **outside** the icon's box. Any ancestor with `overflow:auto|hidden` clips it.

Consequences for the two candidate fixes:

| Change | Tooltip risk |
|---|---|
| `maxHeight` + `overflowY:auto` on `cardStyle` | Low but **unverified** — the tooltip-bearing icons sit mid-card and their tooltips extend upward, so they should stay inside the padding box. ⛔ Must be measured, not reasoned |
| `maxHeight` + `overflowY:auto` on the **"Who these are with" footnote** | 🔴 **Direct** — that block carries an `InfoIcon` on its heading *and* one per entry (`cp.tooltip`), which is where the explanation of what a counterparty *is* lives |

⭐ Today **no** tooltip sits inside a scrolling container: the permission list (240 px) and the
Customize list (260 px) contain no `InfoIcon`. So capping the footnote would be the **first** place
the product clips a consent tooltip. That is a disclosure regression traded for a layout fix, which
§3 of this contract forbids.

---

# AFTER the fix — same rig, same subject, 2026-09-02

Change: the Level-2 counterparty footnote is **collapsed by default** (count always shown), and
`cardStyle` gains `maxHeight: calc(100vh - 88px)` + `overflowY: auto`.

## M5 — card height no longer tracks permission count (`P7a-A1`, `P7a-A3` GREEN)

| N protocols | card height | card top | Connect fully visible |
|---|---|---|---|
| 8 | 749 | 142 | ✅ |
| 10 | 749 | 142 | ✅ |
| 14 | 749 | 142 | ✅ |
| 20 | 749 | 142 | ✅ |
| **40** | **749** | 142 | ✅ |

Constant from 8 to 40. Before: 1079 → 1565 over the same range, with the buttons gone from N=10.

## M6 — 🚨 the first fix was WRONG, and only the expanded state showed it

`maxHeight: calc(100vh - 32px)` — the obvious form — **measured as a 1056 px border box in a 1032 px
viewport**. The cap engaged and the buttons still went off-screen.

Cause: this card is **content-box** (no border-box reset), so `max-height` bounds the *content* box
and `padding: '28px 32px'` sits outside it. 1000 + 28 + 28 = 1056.

⛔ Recorded because the collapsed state was **green with the wrong value in place** — card 749, fits,
buttons visible, every default-path check passing. Only expanding the footnote pushed content past
the cap and exposed it. A fix verified solely against the default state would have shipped.

Corrected to `calc(100vh - 88px)` (32 breathing room + 56 own padding). Verified: `maxHeight`
computes to **944 px**, and at N=20 and N=40 expanded the card is exactly 1000 px, top 16, bottom
1016, inside a 1032 viewport, scrolling internally (`scrollHeight` 1565).

⭐ Scrolling the card to its end brings Connect to **100% visible**. Reachable, which it was not before.

## M7 — tooltips are NOT clipped (`P7a-A7` GREEN)

The risk: `InfoIcon` renders `position:absolute` outside its icon's box, and `overflow:auto`
establishes a clip box whether or not it is currently scrolling.

Hovering **one icon at a time**, counting only icons whose own rect is inside the card's visible box:

| state | visible icons hovered | clipped |
|---|---|---|
| collapsed (default) | 3 | **0** |
| expanded, scrollTop 0 | 13 | **0** |
| expanded, scrollTop 300 | 20 | **0** |
| expanded, scrollTop 565 (end) | 20 | **0** |

⚠️ **A cruder version of this test reported 5–8 "clipped" tooltips and was wrong.** It hovered all 23
icons at once and counted tooltips belonging to icons scrolled out of view — a state no pointer can
produce. The subject had to be *"tooltips of icons a user can actually point at"*, not *"all
tooltips"*. Recorded because the wrong version would have condemned a correct fix.

⚠️ Instrument note: React derives `onMouseEnter` from delegated `mouseover`. Dispatching a raw
`mouseenter` opens **nothing** and reports a false all-clear — the first run returned
`openTooltips: 0`, which reads identical to "no clipping".

## M8 — the cap reaches every branch (`P7a-A5` GREEN), and found one more uncapped list

`maxHeight` computes to **944 px** on all five branches sampled — the cap is on the shared
`cardStyle`, so one change covers the class.

| branch | card height | fits viewport | scrolls | least-visible button |
|---|---|---|---|---|
| `domain_approval` | 584 | ✅ | no | Block 38/38 px |
| `payment_confirmation` | 379 | ✅ | no | Deny 38/38 px |
| **`certificate_disclosure`** (40 fields) | **1000** | ✅ | **yes** | **Deny 0/38 px** |
| `rate_limit_exceeded` | 304 | ✅ | no | Deny 38/38 px |
| `identity_key_reveal` | 359 | ✅ | no | Deny 38/38 px |

🆕 **`certificate_disclosure` has an uncapped field list** — the same defect class as the counterparty
footnote, in a different branch, and on the one gate that prompts **unconditionally with no opt-out**
(`matrix_c.rs`, `R-PERIM`). At 40 fields it fills the card and its buttons sit below the internal
fold. **It is now reachable by scrolling** (pre-fix it would have been off-screen entirely), so the
backstop is holding — but the branch wants the same collapse treatment the connect modal just got.
⛔ Not done here: that is a design change to a different modal and belongs with 7b, not smuggled in.
