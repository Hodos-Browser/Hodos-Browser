# Phase 1 — measured results

Everything here is a **measurement**, taken 2026-08-25 on the owner's machine, dev build
`cef-native/build/bin/Release`, dev wallet port 31401, CDP 9322. Claims live in
`PHASE_CONTRACT.md`; this file only records what an instrument reported.

---

## M0 — The machine is natively DPI matrix cell #9

Measured with `GetDpiForMonitor(MDT_EFFECTIVE_DPI)` from a `PER_MONITOR_AWARE_V2` process.
⛔ A DPI-*unaware* query returns virtualised bounds and every per-monitor difference disappears —
that is how you measure the wrong thing here.

| Device | Origin | Physical | DPI | Scale |
|---|---|---|---|---|
| `\\.\DISPLAY1` | **(−1920, 0)** | 1920×1200 | **120** | **1.25** |
| `\\.\DISPLAY21` | (1920, 0) | 1920×1080 | 96 | 1.00 |
| `\\.\DISPLAY22` **PRIMARY** | (0, 0) | 1920×1080 | 96 | 1.00 |

⭐ **The primary is 100 % and the secondary is 125 %.** That is exactly the configuration reported
item 7 describes — "correct on the primary, offset on the smaller screen" — and it means the
mixed-DPI cell needs no special setup here: it is the machine's normal state.

⭐ `DISPLAY1` sits at **negative screen X**, which is the precondition for the two unconverted
`WM_MOUSEWHEEL` sites (contract §2.3) to feed a negative coordinate into CEF.

---

## M1 — 🎯 W1 SETTLED: libcef takes our OSR mouse coordinate **verbatim**

This was the question the whole phase turned on (contract §11.1 W1). If libcef had been scaling
the coordinate itself, converting on our side would have **shipped a regression** onto a working
path. It does not.

### Instrument

- **Native half** — `LogMouseProbe` in `cef_browser_shell.cpp`, logging what is passed to
  `SendMouseClickEvent` alongside the window's DPI, client rect and reported view rect. Changes no
  behaviour; the "would_be_if_converted" value is logged for comparison and **is not sent**.
- **DOM half** — a `mousedown` listener injected via `ExecuteJavaScript` on the **same
  `CefRefPtr<CefBrowser>` the WndProc is forwarding to**, reporting `clientX/Y`,
  `devicePixelRatio`, `innerWidth/Height` and `document.elementFromPoint(...)`. It reaches the log
  through a new `SimpleHandler::OnConsoleMessage` — a browser-process channel, because the overlays
  are windowless browsers with no devtools of their own and renderer-process logging is dead.
- **Injection proof** — the listener logs `HODOS_PROBE armed` when it attaches. ⛔ Without that
  line an absence of `dom` reports would be indistinguishable from an absence of logging.
- **Driving** — `WM_LBUTTONDOWN`/`UP` posted to the overlay HWND at an exact chosen client point
  (`scratchpad/click.ps1`). `SendInput` clicks are dropped in this environment, and this is in any
  case the better instrument: the coordinate under test is *chosen*, not inferred.

### 🎯 SUBJECT

The **wallet overlay** browser, addressed as `http://127.0.0.1:5137/wallet-panel?iro=50`, and the
matching HWND of class `CEFWalletOverlayWindow` in the **dev** process (`--profile=Default`,
CDP 9322). ⛔ The installed browser (CDP 9222, 64 procs) was never attached to; `cdp.py` refuses
port 9222 outright and errors on an ambiguous URL match rather than guessing a target.

⚠️ **Subject limit, stated with the result:** this measures the
`WndProc → SendMouseClickEvent → CEF → DOM` leg. It does **not** measure whether Windows hands the
WndProc correct client coordinates — that leg is sound by definition (`WM_LBUTTONDOWN`'s `lParam`
*is* client coordinates) but it is not what was instrumented.

### Results

| # | Monitor | scale | We sent (physical) | Correct (logical) | **DOM received** | `dpr` | view | `elementFromPoint` |
|---|---|---|---|---|---|---|---|---|
| a | primary | **1.00** | `200,400` | `200,400` | `200,400` | 1 | 400×931 | `DIV:Refresh$4.71USD…` |
| b | DISPLAY1 | **1.25** | `200,400` | `160,320` | **`200,400`** | 1.25 | 400×800 | `DIV:Refresh$4.71USD…` |
| c | DISPLAY1 | **1.25** | `200,900` | `160,720` | **`200,900`** | 1.25 | 400×800 | **`NONE`** |
| d | DISPLAY1 | **1.25** | `160,320` | — | `160,320` | 1.25 | 400×800 | **`BUTTON:Advanced`** |

**Row a** is the scale-1 control: sent == received, because ×1 is a no-op. It proves the instrument
reports agreement when there is agreement — and it is why every 100 % check anyone has ever run
passed.

**Row b** is the defect. The HWND client area is 500×1000 physical; the view CEF was told about is
400×800 logical (`GetViewRect` divides by the scale, and `devicePixelRatio` confirms CEF applied
1.25). We hand it a **physical** number into a **logical** space. Error = `p·(1 − 1/s)` = **20 %**,
growing with distance from the overlay's top-left. At 125 % that is "slight" — which is the word
the owner used.

**Row c is `TICKET_modal_buttons_unclickable_small_screen`, measured.** A click at physical
`y = 900` — well inside a 1000-px-tall window, on a control the user can see — is delivered at view
`y = 900`, past the bottom of an 800-tall view. `elementFromPoint` returns **`NONE`**: the click
lands nowhere. **The bottom 20 % of every overlay is dead at 125 %, and the bottom 33 % at 150 %.**

**Rows b + d together are the R-PERIM finding, and they are the reason this outranks the feature
work.** The user's cursor is on the pixel that is view `(160,320)` — `BUTTON:Advanced` (row d). The
element that actually receives the click is the one at view `(200,400)` — a different element
entirely (row b). **A user aims at one control and a different control is activated.** On a consent
modal that is "aims at Deny, activates Allow". It is no longer a hypothetical.

### What this rules out

- ⛔ **"libcef already scales OSR mouse input"** — refuted. Row b would have shown `160,320`.
- ⛔ **"There is double delivery from a windowed browser"** — refuted upstream by contract §1: the
  overlays are windowless, so the hand-rolled path is the only input path.
- ⛔ **"It is a second-monitor bug"** — refuted. Nothing in the mechanism involves a second monitor;
  it is a **scale ≠ 1** bug. The secondary is simply the scaled display on this machine.

---

## M2 — ⛔ Method note: `--force-device-scale-factor` (M2 in the matrix) is **invalid** for this path

Not yet measured directly, but it follows from M1's mechanism and must be checked before anyone
uses M2 as evidence here: overlay geometry is computed from **`GetDpiForWindow(hwnd)`**
(`ScalePx`, `GetViewRect`, `GetScreenInfo`), which is an OS query about the monitor. Chromium's
`--force-device-scale-factor` cannot change it. An M2 run on a 100 % monitor would therefore leave
`dpi = 96` throughout and **show no offset at all** — a green that means nothing.

⭐ That is the "harness that passes with the feature absent" trap in its purest form, and it is
about to be recommended by `DPI_RESOLUTION_TEST_MATRIX.md`, which offers M2 as the fast lever. The
matrix needs a note that M2 exercises the **Chromium/React** half only and is **not** evidence about
the native overlay DPI path. **M1 (real Windows scaling) or a genuinely scaled monitor is the only
valid method for overlays.**

⛔ Do not treat this as established until the `dpi=` field of the probe has been read under M2.

---

## M3 — Observations recorded in passing, not chased

- The **installed** browser (long-running session) has **two** `CEFWalletOverlayWindow`, **two**
  `CEFNotificationOverlayWindow` and **two** `CEFOmniboxOverlayWindow` HWNDs. All `vis=False`, all
  `WS_EX_LAYERED | WS_EX_TOPMOST`. The dev instance has one of each. Duplicated orphan overlay
  windows are the shape of the P0.9 invisible-click-eating-overlay defect; worth a look, out of
  scope here.
- `document.documentElement.getBoundingClientRect().height` is **0** in the wallet overlay
  (absolutely-positioned layout), so it is **not** a usable content-extent measure for `P1-A5`. The
  window-vs-content delta needs the union of laid-out children, not the root rect. Noted before the
  measurement is attempted, so the delta is not reported as "0 = no gap".
- The dev log `debug_output.log` is **272 MB** — `TICKET_production_debug_logging_unbounded.md`,
  Phase 2, still open and still growing.
- The dev browser starts in the **profile picker** (4 profiles exist), which sets `g_picker_mode`
  and **disables CDP entirely**. `--profile=Default` bypasses it. Worth knowing for every future
  harness on this machine.

---

## M4 — Dashboard fit (scope amendment `P1-A11`)

Measured in the `/wallet` tab browser via CDP at `dpr = 1.25`, viewport 808 logical px, on `DISPLAY1`.

| | Send card | Recent card | Send content | Send client box | Send button bottom |
|---|---|---|---|---|---|
| **Before** | **536** | 462 | — | — | off-screen |
| **After** | **462** | **462** | 460 | 460 | **771** (viewport 808) |
| **Negative control** (leaked margin re-injected) | 462 | 462 | **516** | 460 | **831** — past the viewport |

**Root cause, two independent halves:**

1. **The border mismatch.** `.wd-quad-br .wd-recent-card` carried `flex:1; overflow-y:auto`, while
   `.wd-quad-bl .wd-send-card` carried only `flex:1`. A flex item's `min-height` defaults to `auto`,
   so without `min-height:0` the send card **cannot shrink below its content** and grew past its
   quadrant to 536 px while Recent sat inside one at 462. The two cards differed by exactly that
   pair of declarations. ⭐ After the fix the card measures 462 in **all three** states above,
   including with the defect re-injected — the border now matches regardless of content.

2. **The scrolling.** `.transaction-form-content` spaces its children with `gap: 14px`, *and*
   `WalletPanel.css`'s **unscoped** `.form-group { margin-bottom: 20px }` leaks in — so every field
   was separated by **34 px**, double-spaced by two rules doing the same job. Zeroing the leaked
   margin in the `.wd-send-card` context recovers 60 px; header padding/margin and card padding
   recover 18 more.

⭐ The negative control proved its own injection took effect (`getComputedStyle` read back `20px`)
before any geometry was trusted — per the P0.8 `T1g` lesson that a control which cannot prove it
injected anything is worth nothing.

⚠️ No font size changed. The unscoped `WalletPanel.css` rule was left in place deliberately.

---

## M5 — The fix, verified, with its negative control on the SAME binary

Wallet overlay on `DISPLAY1` (125%), client 500×1000 physical, view 400×800 logical.
Both runs use the identical build; the only difference is `HODOS_OVERLAY_RAW_MOUSE=1`.

| Clicked (physical) | 🟢 fixed → DOM | element | 🔴 `RAW_MOUSE=1` → DOM | element |
|---|---|---|---|---|
| `200,400` | **`160,320`** | **`BUTTON:Advanced`** | `200,400` | `DIV:Refresh…` |
| `228,411` ← *the owner's real failing click* | **`182,329`** | **`BUTTON:Advanced`** | `228,411` | `DIV:Refresh…` |
| `200,900` | **`160,720`** | a real element | `200,900` | **`NONE`** |

⭐ Row 2 is the strongest evidence in the phase: it is the owner's **actual** physical click,
captured from the log at 09:58:40 on the unfixed build, replayed against the fixed one. It now
lands on the button he was aiming at.

⛔ The negative control runs on the **same binary**. A red produced by a *different* build leaves
"did the binary change, or did the behaviour?" unanswered — which is why the lever exists rather
than relying on the pre-fix measurement alone.

### Gates

| Check | Result |
|---|---|
| `hodos_tests` — `OverlayMouse.*` | 7/7 pass |
| `hodos_tests` — full suite | **303 tests, 302 pass, 1 pre-existing skip**, no regressions |
| `preflight.ps1 -Full` | **PASS** — T0 (incl. new `G8`) + T1a–T1g |
| `preflight.ps1 -NegativeControl -Only G8` | **PASS** — gate detected its injected probe (1 > baseline 0) |

### `G8` — the durable part

`Pattern = '\w*[Ee]vent\.(x|y) *= *[^;]'` over `cef-native/**/*.{cpp,h}`, baseline **0**, target **0**.
All 47 call sites now route through `hodos::ClientToViewPoint`, which does not match. Any new direct
assignment fails the gate.

⚠️ **Windows only, deliberately.** macOS assigns from NSView `location`, already in logical points —
62 such lines in `cef_browser_shell_mac.mm` are **correct**. Including `.mm` would set a 62-violation
baseline that hides the one line that matters. Recorded so nobody "improves" the gate by widening it.

---

## M6 — Owner test session, 2026-08-25 (reported items 1, 2, 7 + two new findings)

### Item 2 — profile picture · ✅ CLOSED, non-vacuously

Owner: panel stayed open through the whole Explorer dialog, and the avatar populated.
⭐ The session produced its own negative control, organically:

```
13:34:12.574  📂 File dialog requested - setting guard flag
13:34:12.635  📱 App losing focus but file dialog is active - keeping overlays open
   … 27s of clicking inside Explorer, no dismissal …
13:34:39.285  📱 App regaining focus - clearing file dialog guard
13:35:02.145  🖱️ Click detected outside profile panel — dismissing     ← guard off, hook works
```

Same hook, same class of click: **guard on → no dismiss; guard off → dismiss.** That also refutes
the "guard too broad" risk from contract §11.2 — panels are not left un-closable.

### Item 7 / modal reachability — ✅ owner-confirmed on the notification overlay

The connect modal at 125%: hover tracks the cursor, every control clickable, whole modal visible and
readable. This is the **full-window** overlay, so it carried the largest possible coordinate error and
sits on the money path.

### Window-vs-content delta, all overlays (`P1-A5`)

Content extent = furthest bottom edge of any laid-out element. ⛔ `documentElement`'s own rect is 0 in
these absolutely-positioned overlays and cannot be used — noted in M3 before the measurement.

| Overlay | Window | Content | Dead strip |
|---|---|---|---|
| site-info | 480 | 164 | **316 (66%)** |
| tab-list | 480 | 181 | **299** |
| downloads | 400 | 133 | **267** |
| bookmarks | 480 | 273 | **207** |
| profile | 520 | 363 | **157** |
| privacy-shield | 370 | 269 | **101** |
| menu | 450 | 405 | **45** |
| wallet-panel | 740 | 740 | 0 ✓ |

⭐⭐ The Windows menu overlay is **45 px** — the exact number macOS measured independently
(`MAC_RELAY_BETA3.md` §M1a: window 280×450, content 280×405). Identical to the pixel on both
platforms, so the sizing contract is genuinely cross-platform and the macOS finding transfers.

**This is reported item 1.** The click-outside hooks test the WINDOW rect, so a click in the dead
strip is "inside" and does nothing, while a click below the window edge closes — exactly as reported.

⚠️ The list panels are **state-dependent**: downloads measures 133 with *zero* downloads. Treat the
fixed offenders (menu 45, profile 157) as the real constants. The owner also observed the inverse —
selecting an avatar grows the profile edit form to **627 in a 520 window, 107 px outside**. Both
directions are one defect: window size and content size are decided independently.

### 🚨 New finding — the profile delete button is invisible, not missing

Owner reported it absent. Measured on the default profile:

```
disabled=true   rect=[282,306]  insideWindow=true
icon #4b5563 on #111827  →  contrast 2.35:1   title=null
```

Not clipped, not missing — **rendered at 2.35:1 and silently disabled** (deleting the default profile
is forbidden), with no tooltip saying why. "The delete button was not there" is an accurate
description of what that looks like.

⭐ **Third control shipped at a contrast a human reads as absent**, after P0.8's invisible spending
cap (1.07:1) and the 1.5:1 marked label. ⛔ We already have a gate for this class — **T1g** — and it
is scoped to `BRC100AuthOverlayRoot.tsx` alone, so it has never looked at this file. A gate guarding
one screen against a defect that recurs across screens will keep being re-learned. Generalising T1g
is worth more than fixing this one colour.

### 🚨 New finding — monitor-drag relayout · ROOT-CAUSED AND FIXED

Owner: slow drag fine; **fast drag reproduces** (~1 in 3); recovers only on maximize; intermittent.

MEASURED over 18 transitions: **`WM_SIZE` arrives ~6 ms BEFORE `WM_DPICHANGED`, already carrying the
new DPI.** So the header re-lays-out at the new pixel size while CEF still holds the OLD
`device_scale_factor`; `WM_DPICHANGED` then updates the scale but never asks for a re-layout, and its
`SetWindowPos` requests a size 1 px off what the window already is, which Windows clamps — so no
further `WM_SIZE` follows. A slow drag emits extra `WM_SIZE` traffic afterwards and self-corrects; a
fast drag does not. **That is the intermittency.**

**Cause:** header and tab browsers received `NotifyScreenInfoChanged()` **without** `WasResized()`.
Overlays got both. `NotifyScreenInfoChanged` updates the *scale*; only `WasResized` triggers a
re-layout. ⭐ Same omission class as G1 (the overlay notify list missing 8 of 15), found twice in one
session in adjacent code — the pairing wants to be a helper, not a convention to remember.

| | DPI transitions | Maximize recoveries |
|---|---|---|
| Pre-fix | 18 | **5** |
| Post-fix | 12 | **0** |

Owner-confirmed clean on every post-fix run, including deliberately fast drags.

⚠️ **Unexplained, recorded not rounded off:** one post-fix transition shows `delta=0x-93` — Windows
granted 93 px less height than it suggested. Caused no failure (no maximize followed). Most likely a
work-area limit. Seen once.

### ⛔ My own probe lied, and it is worth remembering why

The first version of the `dpichanged` line tested exact equality against the suggested rect and
printed **`applied=NO` on every 96→120 transition**. That reads as the smoking gun. The real
difference was **one pixel** — Windows clamping a suggested 1033 to the monitor's 1032 work area.
Trusting it would have meant hunting a `SetWindowPos` that was working perfectly. It now reports the
**delta** instead of a verdict. ⭐ A diagnostic that renders a judgement will eventually render the
wrong one; a diagnostic that reports a number cannot.
