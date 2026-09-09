# 🧑 Human test queue — macOS

**What this is.** Every check that is owed on macOS and **cannot be run by an agent session**, with
the reason it cannot, gathered in one place so it can be worked as a single sitting at a machine
rather than rediscovered one relay round at a time.

**Created 2026-09-09** from the accumulated Mac relay rounds. ⚠️ Not a list of everything owed — see
each round's own "owed" section for the automatable remainder. This file is specifically the
**human-bound** subset.

⛔ **Do not mark a row here from a code reading.** Every row exists because reading it is exactly
what does *not* settle it.

---

## Why each row needs a human — the four instrument limits

These are measured facts about this machine, not preferences. They are why the rows below could not
simply be automated:

| # | Limit | Consequence |
|---|---|---|
| **L1** | 📏 **`CGEventPost` is Accessibility-blocked** for the agent session process | Synthetic OS clicks silently do nothing. A probe using it printed a clean "FAIL" and only its positive control revealed the instrument was dead |
| **L2** | 📏 **CDP `Input.dispatchMouseEvent` enters BELOW the native `NSView`→`CefMouseEvent` layer** | It **passes with the overlay coordinate-bug class fully present**, so it cannot be substituted for a real click. This nearly faked a bug in this project once already |
| **L3** | ⚠️ **Ad-hoc dev builds do not engage the hardened-runtime policy**, and TCC has no Windows analogue | Anything touching camera/mic/entitlements needs a **CI-signed** build before a result means anything |
| **L4** | ⚠️ **Visual judgement** — contrast, clipping, "does this look deliberate" | Not a measurement an agent should claim. Phase 0.8 shipped six such defects with every gate green |

⭐ **What CAN be automated, and was.** An overlay's whole C++ path — create, position, render, hide,
action — can be exercised without the opening gesture by sending its IPC from an **internal-origin**
browser over CDP (proved on the tab context menu, 2026-09-09, rows `P4-M18`–`P4-M24`). Only the
**gesture itself** stays human-bound. Try that before adding a row here.

---

## A. Gesture-bound (L1 + L2)

| # | Check | Expect | Source |
|---|---|---|---|
| A1 | **Tab context menu — the right-click gesture.** Right-click a tab, including a **background** tab | Menu opens at the cursor, on the tab actually clicked | `P4-M18` note, `MAC_RELAY_P35_P4_ROUND.md` |
| A2 | **Tab menu click-outside dismissal**, by a real mouse-down | Menu closes; the click still reaches whatever was under it | same |
| A3 | ⚠️ **Tab menu right-click on a SECOND tab while it is open** | Menu moves to the new tab **and acts on the new tab** — this is the arm the extra `NSEventMaskRightMouseDown` monitor exists for; `P4-A2` from the other side | same |
| A4 | **Tab menu Cmd+Tab dismissal** (app focus loss) | Menu disappears; it attaches to no parent window, so this is the only thing hiding it | same |
| A5 | **Phase 5 `W7` / ticket §8.6** — open **wallet**, **wallet_panel**, **settings**, **backup** overlays from the toolbar | Each still reaches the Rust wallet after the Phase 5 predicate swap to `IsOurWalletOrigin`. ⚠️ Partial only so far: internal-origin traffic is confirmed alive in the log but not attributable to these four | `MAC_RELAY_P5_ROUND.md` M6 |
| A6 | **Phase 4 `O2`** — click-outside dismiss (owner item) | — | `MAC_RELAY_P7_ROUND.md` M6 |
| A7 | **Phase 4 `O3`** — the focus half of `P4-A4` | — | same |
| A8 | **Phase 4 `O6`** — *hear* a muted tab | Audio actually stops. ⚠️ Also confirm the mute **survives a navigation** — CEF's mute is per-document, `OnLoadingStateChange` re-applies it; look for `🔇 Re-applied mute to tab N after navigation` | `MAC_RELAY_P35_P4_ROUND.md` M7 |
| A9 | **`g_file_dialog_active` latch** — open a file dialog once, then try click-outside on any of the 4 affected panels | 🐞 Suspected macOS-only defect: the setter is in shared `simple_handler.cpp` but its only reset lives in a **Windows-only TU**, so it should latch `true` and kill click-outside dismissal for the rest of the session | 2026-08-26 round |
| A10 | **Profile panel does not dismiss on app focus loss** while its siblings do | Confirm, then decide whether it belongs in `InstallAppFocusLossHandler` | 2026-08-26 round |
| A11 | **Phase 0.9 `A3.3` / `A3.4`** | — | 2026-08-26 round |

## B. Multi-window (L1 + L2, and it needs two real windows)

⚠️ **Two different profiles is NOT the test** — those are separate processes and would pass while
proving nothing. It must be two windows of the **same** profile, i.e. one process.

| # | Check | Expect / why |
|---|---|---|
| B1 | **Does a second same-profile launch even forward on macOS?** | ⭐ If it starts a second *process*, the whole defect class does not arise here — which is itself the answer. Use **Cmd+N** for the real test |
| B2 | **Cmd+F in the second window** | Find bar opens in the **second** window, not the first |
| B3 | **HTML5 video fullscreen in one window** | The *other* window's header does not vanish and its content does not resize |
| B4 | **Overlap the windows, open a dropdown in the second** | The second window does **not** drop behind the first |
| B5 | ⚠️ **Read z-order and `isMiniaturized`, not "did it disappear"** | *Behind* and *minimised* look identical on screen and have different causes. That distinction cost Windows a day (K9.1) |

⛔ **Do not port the Windows fix blind — measure first.** Both outcomes are informative.
Source: `MAC_RELAY_P3_ROUND.md` M2.1, `MAC_RELAY_P35_P4_ROUND.md` M2.

## C. Needs a CI-signed / hardened-runtime build (L3)

| # | Check | Note |
|---|---|---|
| C1 | 🚦 **Sparkle 2.9.6 verification + its negative control** | **BLOCKS PROMOTION.** beta.2 shipped 2.9.3; beta.3's tag build is CI's first 2.9.6 run |
| C2 | **Big Sur / `minimumSystemVersion`** call | Pinned final 0.3.x as a permanent second feed item — documented, not measured. `TICKET_appcast_missing_minimum_system_version.md` |
| C3 | 🔴 **`P4-B2` — mic/camera in all three stored states** (allow / block / ask) | ⛔ macOS has a layer with no Windows analogue: **TCC**. `helper-Info.plist.in` still carries **neither** `NSMicrophoneUsageDescription` **nor** `NSCameraUsageDescription`, and capture runs in the helper. ⚠️ **Method trap:** setting the stored state via `site_permissions_set` **from the page** is correctly DENIED by the IPC allowlist, so both arms silently measure "Ask" — set it from an **internal origin** and use a **fresh tab per state** |
| C4 | **`WS6` — CIDetector on a real `bsv:` QR** | Once it lands in a signed build |
| C5 | **Mic entitlement `device.audio-input`** actually shipping | Root-caused 2026-08-18; the `entitlements.plist` `--`-in-comment bug meant it likely never shipped. Verify on the next CI build: prompt appears, level meter nonzero |

## D. Visual judgement (L4)

| # | Check | Note |
|---|---|---|
| D1 | **`T1g`** — limit-field contrast on macOS | |
| D2 | **`P7d-A13`** — a human reads **every changed screen**: labels legible, not clipped, not shattered across lines | ⛔ "Not a screenshot I took and did not read" |
| D3 | **Phase 0.8 connect modal** — card is **840 px in a 794 px window** (23 px off top and bottom, nothing scrolls) | 👤 Owner's call: **make it fit, no scrollbar** |
| D4 | **Phase 7c connect modal** now that the Quiet-mode checkbox and callout are gone | Confirm the layout still looks deliberate rather than leaving a gap |
| D5 | **`D1` overlay sizing contract** | 4 of 8 overlay heights disagree across platforms. Recommendation on record: derive window height from content rather than re-tuning constants |

## E. Needs real money or real hardware

| # | Check | Note |
|---|---|---|
| E1 | **`R-GOLD` / `R-COUNT`** — the gold-pill payment indicator and its count | Needs a real payment. Owed at every boundary so far |
| E2 | **`WS1(b)`** — the two-display case | 👤 Owner: the Mac is normally a laptop on its own; a monitor can be plugged in when needed |
| E3 | **DPI matrix cells #4 / #6 / #9** | ⚠️ Not run on **either** platform |

---

## Not human-bound — try these with a harness first

Listed so nobody parks them here by mistake:

- **Phase 7 `M4` #1/#2** — zero third-party favicon requests from the consent modal and new tab.
  `phase-7b-connect-modal/netwatch.py` / `netwatch_page.py` exist and assert their own trigger.
  ⚠️ **Re-run now that `FaviconStore` is initialised on macOS** (fixed 2026-09-08) — before that fix
  macOS could produce neither a store hit *nor* a Google request, so a green would have been vacuous.
- **Phase 7c `M4`** — the quiet-mode probe pair is `curl`; the `key_id = '*'` check is `sqlite3`.
  Only the three "no checkbox is visible" rows are eyes-on.
- **Stubbed `R-INTEXT` REDs** — forced-flip stubs, owed before the release boundary.
