# 🧑 Human test queue — macOS and Windows

> ⭐ **Windows rows added 2026-09-15** (section W, at the end of the lettered sections). Owner: *"keep track of
> everything we still need to check … getting eyes on the yellow dot and those types of things that need humans."*
> ⛔ **Rule from now on:** any evidence row a phase marks *T3 / visual / human* gets a row here **in the same commit**
> that records it as owed. A human check that lives only inside a phase contract is how the orange dot shipped.

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
| D6 | **Consent prompt on a FIRST-visit site**, after the 2026-09-09 favicon change | The icon now comes from `FaviconStore`, which fills asynchronously, so a first visit can show the domain-initial tile where it previously showed the site's icon. 📏 Measured correct (`brokenImgs: 0`, tile renders); what a human owes is whether it *reads* as deliberate |

## E. Needs real money or real hardware

| # | Check | Note |
|---|---|---|
| E1 | **`R-GOLD` / `R-COUNT`** — the gold-pill payment indicator and its count | Needs a real payment. Owed at every boundary so far |
| E2 | **`WS1(b)`** — the two-display case | 👤 Owner: the Mac is normally a laptop on its own; a monitor can be plugged in when needed |
| E3 | **DPI matrix cells #4 / #6 / #9** | ⚠️ Not run on **either** platform |

---

## D (continued) — macOS visual rows added by Phase 10

| # | Check | Pass looks like | Source |
|---|---|---|---|
| D7 | **`P10d-A5` visual — "recipient not notified"** on the macOS wallet overlay and advanced wallet | Yellow header dot (not orange, not red); one-line yellow banner; Activity row shows the yellow cause line with **Retry** and **Copy details**; **Dismiss clears the dot and the Activity line stays**; readable at 7a's small-screen size. Seed an `undeliverable` outbox row in your **dev** DB for one of your own sent txids (shape: `10d-peerpay-delivery/PHASE_CONTRACT.md` T2 table) | `MAC_RELAY_BETA3.md` round 2026-09-15c |
| D8 | **`P10a` rejected-payment banner** now yellow | Reads as "needs you", not an error | same |

---

## W. Windows — needs a human at this machine

Same limits apply on Windows: **`SendInput` mouse clicks are dropped in the agent session** (moves work — memory
`reference_sendinput_clicks_blocked_in_agent_env`), CDP input enters below the native layer, and visual judgement is
not an agent's claim (L4).

| # | Check | Pass looks like | Source | State |
|---|---|---|---|---|
| W1 | **`P10d-A5` visual — "recipient not notified"** in the dev browser (wallet overlay + advanced wallet) | Yellow header dot; yellow one-line banner; Activity row: yellow cause line, **Retry**, **Copy details** (pastes the claim block JSON); **Dismiss clears the dot**, the Activity line stays; nothing clipped | `10d-peerpay-delivery/PHASE_CONTRACT.md` | ✅ **PASS 2026-09-16, owner at the machine.** Dot reads as heads-up not alarm; banner one line, warning not error; Activity carries the yellow cause line, Retry and Copy details; **Dismiss clears the dot instantly** (the `F6-10d` fix's only human verification) and the Activity record survives it. ⭐ Retry was exercised live: it re-contacted the **real** MessageBox, got a genuine `413 ERR_MESSAGE_BODY_TOO_LARGE`, marked the row undeliverable and stopped — and **spent nothing**. Copy details produced a claim block whose `senderIdentityKey` is genuinely this wallet's, which is the first real emitted block any row has seen (`F3-10d`). 👤 Owner: *"this is more than the casual user should have to do — we really just need to make the message box work"* |
| W2 | **`P10a` rejected-payment banner** (now yellow) | One quiet banner per sender; no modal | `10a-peerpay-atomic-subject/PHASE_CONTRACT.md` | ✅ **PASS 2026-09-16, owner at the machine.** One quiet banner for two senders, no modal, wording clear. The once-per-sender rule was proven at the data layer in the same run: a second fabricated payment from the same key added no row. 🚨 This sitting is what exposed the **green-banner defect** (the header passed `unread_count` as the received count, so a wallet holding two REJECTED payments announced *"Received 2 payments"*) — fixed in `ddf8c04` and re-verified clean by the owner. 👤 No Details button: correct, a rejected attempt has no txid and no amount, so there is nothing to link to. Giving refusals somewhere to live is a beta.4 item |
| W3 | **`P9-A3` T3 — right-click the wallet overlay** | No *Inspect Element* item; F12 on the overlay does nothing | `phase-9-release-readiness/PHASE_CONTRACT.md` | ✅ **PASS 2026-09-16, with its control.** Right-click on an ordinary page offers Inspect Element (the control — without it, "absent on the overlay" would prove nothing); right-click inside the wallet panel offers nothing; F12 on the panel does nothing |
| W4 | **DPI matrix cells #4 / #6 / #9** | See `E3` — not run on either platform | `DevOps-CICD/DPI_RESOLUTION_TEST_MATRIX.md` | ⬜ owed |
| W5 | **`R-GOLD` / `R-COUNT`** with a real payment — the gold pill on the originating tab; counters reset on tab close | See `E1`; the Windows half is `PAYMENT_TEST_BATCH.md` M1/M2 | `REGRESSION_SET.md` | ⬜ owed at every boundary |
| W6 | ⭐ **10b — the burst prompt** (added when 10b's modal changes): the queued payment modal shows *its own* amount and a **"1 of N"** line; the next one appears after the click; readable at the small-screen size | — | `10b-one-click-one-spend/PHASE_CONTRACT.md` | ✅ **PASS 2026-09-16 — the CU-1 assertion proven with a real human click.** Two over-cap payments (130,000 / 150,000 sats) from `https://example.com`, per-tx cap set to 1 cent, both paying the dev wallet's own address. One Approve ⇒ **exactly one** transaction, `b30a65dc…`, for **130,000 sats — the amount that modal displayed**. The second modal appeared on its own about a second later showing **its own** 150,000; Deny produced no transaction and the page received `User rejected authentication`. Readable, nothing clipped. ⚠️ **Finding:** the *"1 of N"* line never appeared. `queuedFromSite` is computed once at enqueue/take time and frozen into the modal, so the **first** prompt of a burst always shows 0 (nothing else had arrived yet) and a **two**-request burst can never show it at all. Informational only; CU-1 itself is unaffected. ⇒ 10e |
| W7 | 🔴 **Panel `F1-10b` live RED — an expired prompt must not come back and spend.** Raise an over-cap payment from an external https page so a prompt queues **behind** another, leave it past the 10-minute prompt timeout, then answer it | The queued prompt is **never posted** once it is older than the timeout; if one is on screen when it expires, clicking Approve resolves **nothing** and **no txid appears** on WhatsOnChain. ⛔ Subject is the **transaction**, not the HTTP status: the page was already told "Approval timeout", so a broadcast here is money into a response nobody reads | `ADVERSARIAL_PANEL.md` `F1-10b`; fix `12c76bd` | ✅ **PASS 2026-09-16, owner at the machine.** Prompt raised 09:20:29, `⏰ IPC auth timeout fired` at 09:30:29 exactly on schedule, and the modal stayed on screen as a ghost. Owner clicked **Approve** on it at 09:35:32 ⇒ **no transaction, and no 222,000-sat transaction anywhere in the wallet**. The overlay closed and nothing resolved, because the id had been popped five minutes earlier. ⚠️ **What it does and does not prove:** the burst went over the IPC path, which already popped correctly. The `F1-10b` fix was for the **HTTP** path, which did not. So this proves the shared defence (a popped id cannot be answered), not the new code — that half still wants an HTTP-path run. 🚨 **This row found a second defect:** the timeout popped the request and sent its rejection to the **C++ prompt id** while the page's shim waits on the **page-supplied** id, so the dApp's call never settled — it hung forever. `resumeIpcResponse` had been fixed this exact way in Phase 2.6-C.5 and the timeout path never was. No money at risk; fixed same day |
| W8 | ⭐ **`P11-1` — launch and TYPE WITHOUT CLICKING.** Start the browser yourself, click **nothing**, and just start typing | The characters land in the **address bar**. ⛔ Subject is where the TEXT goes, not whether a caret is visible — a caret only renders when the window has OS focus, so a rig launch can assign focus with nothing blinking, and that gap is exactly why this row exists | `phase-11-ui-leftovers/README.md` item 1; `TICKET_omnibox_addressbar_interaction_defects.md` #1 | ⬜ **owed.** 👤 The owner's own formulation: *"I should have just started typing and see if it went into the address bar."* ⚠️ Also the row that would settle the test user's *"had to click elsewhere first"* half — which now looks **less likely**, since the owner clicked the address bar and typing worked immediately |
| W9 | ⭐ **`P11-2` / `P11-4` with a REAL mouse and a REAL keyboard.** (a) Type a URL and press **Enter fast** — within a beat of the last letter; (b) type, then **click a suggestion** the same way; (c) type, press **Tab** twice then **Shift+Tab**; (d) press **Escape** twice; (e) with the dropdown up, **click somewhere else on the page** | (a)+(b) the dropdown goes and **stays** gone. (c) each Tab moves the highlight and the text in the bar, the caret **stays in the address bar**, the dropdown stays up. (d) first Escape closes the dropdown and gives back **what you typed**, caret still in the bar; second restores the page URL and leaves. (e) the dropdown goes | `phase-11-ui-leftovers/README.md` items 2 and 4 | ⬜ **owed.** ⛔ Every green on these rows was driven over **CDP**, which reaches the renderer directly. It proves the React/IPC/HWND chain and says nothing about native delivery — `SendInput` clicks are dropped in the agent session and CDP keys bypass native focus. (e) in particular exercises the `WH_MOUSE_LL` click-outside hook, which **cannot be driven from here at all**. ➕ **`P11-5`** — tear a tab off with a **real drag**, then type in the new window's address bar and open its menu: the new window must stay in front and must not vanish. 📏 16/16 green over the `tab_tearoff` **IPC**, which creates the window without the mouse capture and activation changes a drag also causes |
| W10 | ⭐ **`P11-6` / `P11-7` / `P11-10` — ONE SITTING SETTLES THREE ITEMS.** Run `DPI_RESOLUTION_TEST_MATRIX.md` cells **#4** (125%/1366×768), **#6** (150%/1366×768) and **#9** (mixed-DPI), and add a **text-scale** pass the matrix does not have: Windows Settings → Accessibility → Text size at 125% and 150%. In each cell: (a) open an approval modal and **click its buttons**; (b) look at the bottom edge of the header; (c) ⭐ **hold Ctrl and scroll the wheel over the toolbar**, then over the page | (a) the button you aim at is the one that responds, and the bottom of the modal is not dead — 📏 the conversion is proven in `overlay_mouse_test.cpp` and gate `G8`, but nothing has confirmed it on a screen. ⭐ A control comes free: relaunch with `HODOS_OVERLAY_RAW_MOUSE=1` and the SAME binary must go bad. (b) no part of the header is hidden behind the webview — 📏 at 125% the content needs ~150 px and the window gives 96. (c) the **page** zooms and the **toolbar/tabs do not move at all** — ⛔ this is the row that actually settles `P11-I7a`: the guard is proved to fire at the DOM layer, but a **synthetic** wheel never reaches Chromium's zoom path (the tab did not zoom either), so only a real wheel can confirm it | `phase-11-ui-leftovers/README.md` items 6, 7, 10; `DevOps-CICD/DPI_RESOLUTION_TEST_MATRIX.md` | ⬜ **owed.** ⛔ This environment cannot do any of it: CDP delivers events straight to the renderer and never enters the WndProc where the DPI conversion lives, and neither zoom route can be driven from here |

---

## F. Environment — needs a human at the keyboard

| # | Check | Note |
|---|---|---|
| F1 | ⚠️ **Does *Always Allow* on the wallet Keychain dialog actually stop it recurring?** Next dev start after clicking **Always Allow** (not *Allow*) | 📏 2026-09-09 the dialog fired again **with no wallet rebuild** (`SecurityAgent` at 10:36:39 = the wallet spawn second; wallet alive-but-not-listening at `main.rs:568`; binary mtime still Sep 8 16:11). ⇒ "after a rebuild" is not the whole trigger. ⛔ Never diagnose with `security find-generic-password -w` — it hangs on its own prompt. ⛔ The dev stack **cannot come up unattended** while this is unresolved |

---

## Not human-bound — try these with a harness first

Listed so nobody parks them here by mistake:

- ✅ **Phase 7 `M4` #1/#2 — DONE 2026-09-09.** Both green with a store-hit control (new tab:
  `google.com`'s tile decodes to 1391 bytes = its `favicons.db` row). `phase-7b-connect-modal/PHASE_CONTRACT.md` §4c.
- ✅ **Phase 7c `M4` — DONE 2026-09-09, all six rows.** ⭐ Including the three "no checkbox is
  visible" rows, which were **not** eyes-on after all: a `outerHTML` search for `quiet` with a
  positive control on the same regex settles them, and the per-item ticks are settled by reading
  `disabled` on each checkbox. `phase-7c-quiet-mode/PHASE_CONTRACT.md` §5.4. The *visual* half
  (does the layout look deliberate with the callout gone) stays human-bound — that is `D4`.
  ⚠️ `input[type=checkbox]` is **blind** on `DomainPermissionForm` / `ApprovedSitesTab`; their
  toggles are custom elements. Use the text search there.
- ⭐ **A React `element.click()` over CDP is a legitimate instrument** and closed `key_id='*'`
  end-to-end: it runs the real `onClick`. It is **not** `Input.dispatchMouseEvent`, which is still
  barred by `L2`. Try it before parking a button-driven row here.
- **Stubbed `R-INTEXT` REDs** — forced-flip stubs, owed before the release boundary.
