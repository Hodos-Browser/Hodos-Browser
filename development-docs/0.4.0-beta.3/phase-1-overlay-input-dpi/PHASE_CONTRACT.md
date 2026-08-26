# Phase 1 — Overlay input & DPI correctness · PHASE CONTRACT

**Workstream:** WS1 (reported items 1, 2, 7) · **Tickets:** `../TICKET_modal_buttons_unclickable_small_screen.md`
**Status:** 🚧 KICKOFF — ⛔ AWAITING OWNER SIGN-OFF, NO CODE WRITTEN
**Opened:** 2026-08-25 · **Owner:** Windows session · **Platforms:** Windows (primary), macOS (parity, relayed)
**Standard:** `../HARNESS.md`. **Inherited:** `P0.9-A5` (DPI cells #4/#6/#9 never run).

---

## 0. ⭐ RISK LIST — written before the code, per `feedback_risk_list_before_code`

Phase 0.9 was estimated on its happy path (an hour) and cost a day, because nobody wrote down what
it *touched*. This section is that list, first, not as a post-mortem.

### 0.1 Shared machinery this phase touches

| Machinery | Why it is a hazard here |
|---|---|
| **The notification overlay** | Ate ~60 % of Phase 0.9. Keep-alive, **full-main-window sized**, multiplexes every wallet modal / payment / permission prompt, and sits on the money path. It is *also* the overlay most exposed to the coordinate defect below (biggest view = biggest error) and the one where a wrong fix is least visible. Anything that changes its size, its view rect, or its input path is the most expensive change in this phase. |
| **48 hand-rolled mouse-forwarding sites across 15 WndProcs** | A per-site edit is 48 chances to be wrong, and the sites are **not uniform** — see §2.3. A blanket transformation would double-convert the wheel path. |
| **9 `WH_MOUSE_LL` low-level mouse hooks** | System-wide hooks. A mistake here degrades input for the whole desktop, not just Hodos. |
| **`g_file_dialog_active` / `g_wallet_overlay_prevent_close`** | `R-CLOSE`. Three close paths (`WM_ACTIVATE`, `WM_ACTIVATEAPP`, the hook). Item 2's fix adds a fourth consultation of one flag. |
| **`MyOverlayRenderHandler`** | One class serves all 15 overlays on both platforms. `GetViewRect` / `GetScreenInfo` / `GetScreenPoint` are the other three legs of the coordinate contract; changing one without the others reintroduces the same class of bug. |

### 0.2 Existing defects this lands on top of

- `TICKET_modal_buttons_unclickable_small_screen.md` — reported, never instrumented; its stated premise
  has now been **corrected twice** (see §1).
- P0.8 defects 4 & 5: *"the overlay is long-lived but its state was written as if freshly mounted."*
  Expect more of this pattern in any overlay we touch.
- P0.9: the invisible click-eating full-window overlay, and a modal painting over a live prompt.
- `R-CLOSE`'s own recorded gap: **no `*MouseHookProc` consults `g_file_dialog_active`.** This phase
  closes it; it was known before this phase opened.

### 0.3 State that must be controlled before any acceptance run

- ⛔ `python ../phase-0.9-chromium-prompt-branding/reset_test_state.py verify` **must exit 0** before
  every acceptance run. Stop the dev browser first.
- ⛔ **A fresh browser profile is NOT a fresh test.** One `wallet.db` serves every profile; wallet
  approvals are global.
- ⚠️ The notification overlay is keep-alive: after any frontend change, **restart the dev browser** or
  it keeps serving the old JS.
- ⛔ The owner's **installed** browser (`AppData\Local\HodosBrowser`, 63 procs measured today) and its
  wallet on **31301** must never be touched. Match by **exe path**, never process name.
- ⛔ `cargo build` fails "Access is denied" while the dev wallet runs — stop that PID **by path**.

### 0.4 Estimate, stated as coupling and not as a number

- **If the instrumentation confirms §2.4 on the first run:** the conversion is one helper + 38 call
  sites mechanically routed through it, plus 2 wheel sites, plus the item-2 guard — under a day.
- **If it does not:** the offset has an unknown cause, the second-monitor instrument is required, and
  this becomes a multi-day investigation.
- **If the notification overlay fights** (as it did in 0.9): add a day regardless of which of the
  above holds.

---

## 1. ⛔ The premise has now been wrong twice. Here is the measured version.

**Round 1 (the ticket, 2026-08-21):** *"overlays are OSR."*
**Round 2 (the correction, 2026-08-23):** *"MEASURED: every Windows overlay is `SetAsPopup` — a
windowed CEF browser. There is no `SetAsWindowless` at all. 'Overlays are OSR' is false on Windows."*

**Round 3 (this kickoff, 2026-08-25): round 2 is wrong. Round 1 was right.**
The grep was accurate; the inference from it was not. `SetAsWindowless()` is a convenience setter, not
the definition of windowless mode. MEASURED, five independent confirmations:

| Evidence | Location |
|---|---|
| `window_info.windowless_rendering_enabled = true;` on the line **before** `SetAsPopup(...)`, in **all 15** overlay creators | `src/handlers/simple_app.cpp` — 15 sites (646, 790, 1020, 1099, 1255, 1353, 1456, 1685, 1946, 2198, 2431, 2680, 2940, 3174) |
| `CefWindowInfo::SetAsPopup` sets `style`, `parent_window`, `bounds`, `window_name` — and **does not touch `windowless_rendering_enabled`**, so the flag survives | `cef-binaries/include/internal/cef_win.h :: SetAsPopup` |
| Every overlay attaches a `CefRenderHandler` (`MyOverlayRenderHandler`) via `SetRenderHandler`, returned by `SimpleHandler::GetRenderHandler()`. CEF enters OSR **only** when the client returns one | `simple_app.cpp` ×15; `simple_handler.cpp :: GetRenderHandler` |
| `settings.windowless_frame_rate = 30` + transparent `background_color` on every overlay | same 15 creators |
| `OnPaint` composites the CEF buffer into a `WS_EX_LAYERED` HWND with `UpdateLayeredWindow` — there is no CEF child window to paint into | `src/handlers/my_overlay_render_handler.cpp :: OnPaint` |
| Global `CefSettings.windowless_rendering_enabled = true` | `cef_browser_shell.cpp:4668` |

### ⭐ This answers the owner's sharp question directly

> *"Why does a windowed browser that already receives native input ALSO have a hand-rolled injection
> path? Double delivery and correct-native-plus-wrong-synthetic have different symptoms."*

**Neither. There is no double delivery.** The overlays are genuinely windowless, so there is no CEF
window to receive native input, and the hand-rolled path is the **only** input path. That is why it
exists. One path, one place to be wrong — which makes the coordinate hypothesis *stronger*, not
weaker, and narrows the fix from "reconcile two paths" to "fix one conversion".

### ⭐ And it explains why the header has never shown this bug

Header (`WindowManager.cpp:218`) and tab (`TabManager.cpp:120`) browsers use `SetAsChild` and **never**
set `windowless_rendering_enabled` — real windowed child HWNDs where CEF does its own DPI handling.
**Only the 15 overlays are exposed.** Every reported symptom is in an overlay. That is consistent.

⚠️ Two open items fall out of this and are recorded, not fixed here:
- We hand-roll `SetAsWindowless`'s effect but never set the `runtime_style = CEF_RUNTIME_STYLE_ALLOY`
  that `SetAsWindowless` also sets. Overlays therefore run OSR at `CEF_RUNTIME_STYLE_DEFAULT`. Unknown
  consequence → §6, out of scope, ticketed.
- `SetAsPopup` also stamps `WS_OVERLAPPEDWINDOW | WS_VISIBLE` into `window_info.style`, which is dead
  in OSR mode. Cosmetically misleading; it is what produced the round-2 error.

---

## 2. State per surface — claims and measurements separated

### 2.1 The coordinate contract (MEASURED — source read, four legs, three correct)

| Leg | Units | DPI handled? | Where |
|---|---|---|---|
| Overlay HWND size & position | **physical** | ✅ `ScalePx(cssPx, hwnd)` = `MulDiv(px, GetDpiForWindow, 96)` | `simple_app.cpp`, `LayoutHelpers.h` |
| `GetViewRect` → CEF's view size | **logical** | ✅ physical ÷ scale, explicitly commented | `my_overlay_render_handler.cpp :: GetViewRect` |
| `GetScreenInfo.device_scale_factor` | — | ✅ `GetDpiForWindow / 96` | `:: GetScreenInfo` |
| `GetScreenPoint` (view → screen) | logical → physical | ✅ `× scale` | `:: GetScreenPoint` |
| **`SendMouse*Event`** | **physical client px** (`GET_X_LPARAM`) | ❌ **no conversion anywhere** | 48 sites, `cef_browser_shell.cpp` |

CEF's contract is explicit: `CefMouseEvent.x/y` are *"relative to the upper-left corner of the
**view**"* (`cef_types.h`), and `SendMouseClickEvent` repeats it (`cef_browser.h:754`). Our view is
**logical** — we define it that way in `GetViewRect`. So the mouse event must be logical.

⭐ **This is not "48 sites lack a conversion".** It is: **three of the four legs of one coordinate
contract convert, and the fourth does not.** That is a much stronger claim, because the other three
prove the intended unit system.

### 2.2 What that predicts (CLAIM — falsifiable, not yet instrumented)

Delivered view coordinate = `p` (physical). Correct = `p / s`. Error = `p · (1 − 1/s)`.

1. Error is **multiplicative**, zero at the overlay's top-left, growing down and right.
2. At `s = 1.0` there is **no error at all** — which is why every 100 % check has passed.
3. At `s = 1.5`, the injected point is 50 % further down/right than the cursor ⇒ **the user must aim
   *above* a control to hit it.** That is the reported item-7 symptom verbatim.
4. ⭐ **It is not second-monitor-specific.** Any scaled monitor reproduces it; the owner's secondary
   screen is simply the scaled one. **If cells #4/#6 reproduce on a single monitor, the second-monitor
   instrument is not needed for the fix.**
5. ⭐ **It predicts the modal-unclickable ticket too, as the same root cause.** The notification
   overlay HWND is **full-main-window sized** (`simple_app.cpp` — `width/height` from `GetWindowRect(g_hwnd)`).
   On a 768-tall window at 150 %, the view is 512 logical tall; a click at physical `y = 700` is
   delivered as view `y = 700` — **off the bottom of the view, landing nowhere.** Controls in the
   lower part of a full-window modal become unreachable, exactly as reported, and worse on smaller /
   more-scaled screens.

⚠️ **If #1 or #2 fails under instrumentation, this hypothesis is dead** and the cause is elsewhere.
That is the point of stating it this precisely.

### 2.3 The 48 sites are NOT uniform (MEASURED — this is why a blanket edit is wrong)

| Message | Count | `lParam` units | Needs |
|---|---|---|---|
| `WM_LBUTTONDOWN` / `LBUTTONUP` / `RBUTTONDOWN` / `RBUTTONUP` / `LBUTTONDBLCLK` / `MOUSEMOVE` | **38** | client, physical | ÷ scale |
| `WM_MOUSEWHEEL` | **9** | **screen**, physical | `ScreenToClient` **then** ÷ scale |
| `WM_NCHITTEST` | 1 | screen | not forwarded — leave alone |

🚨 **New defect found by this audit, in nobody's report:** of the 9 `WM_MOUSEWHEEL` sites, **2 feed
screen coordinates straight into `SendMouseWheelEvent` with no `ScreenToClient`** — the **Settings
overlay** (`cef_browser_shell.cpp:1737`) and the **Download panel** (`:2996`). The other 7 convert
correctly. On a secondary monitor left of the primary, screen X is negative. Self-evident from the
7 correct siblings.

### 2.4 Item 7 — mouse offset · CLAIM, cause identified, not reproduced

Everything in §2.1–§2.3 is source measurement. **Nobody has reproduced the offset under
instrumentation, including me.** Treated as the leading hypothesis with a stated falsification test.

### 2.5 Item 1 — dead zone below the modal · structure MEASURED, delta NOT yet measured

- Overlay HWNDs are **fixed-size**: `ScalePx(380, 520)`, `ScalePx(450, 450)`, `ScalePx(360, 480)`, …
  React content is whatever it renders. Where content is shorter, a transparent strip inside the
  window is visually "the page below" but physically inside the overlay.
- **All 9 `*MouseHookProc`s test `PtInRect(GetWindowRect(overlay), pt)`** — the **window** frame.
  Structurally identical to the macOS §M1a finding (menu overlay: window 280×450 vs content 280×405).
- ⭐ **The hook's own coordinates are DPI-correct** — `MSLLHOOKSTRUCT.pt` is physical screen and
  `GetWindowRect` is physical screen. ⛔ **Do not "fix" the hook coordinates.** Item 1 is a *sizing*
  bug, not a coordinate bug; conflating the two would break a path that works.
- **MEASUREMENT OWED:** the Windows window-vs-content delta per overlay. macOS has its number; we do not.

### 2.6 Item 2 — profile picture · root cause MEASURED, second half unexplained

- `ProfilePanelMouseHookProc` (`cef_browser_shell.cpp:2943`) does **not** consult
  `g_file_dialog_active`. Per-hook grep: **0 of 9 hooks do.**
- `ProfilePanelOverlayWndProc`'s `WM_ACTIVATE` **does** check it (`:3275`). So the activation path is
  guarded and the hook path is not — the overlay survives losing activation and is then killed by the
  hook on the user's first click inside Explorer. **Exactly the reported symptom.**
- `OnFileDialog` sets the flag **synchronously** in C++ before returning `false`
  (`simple_handler.cpp:8907`), so the guard is set in time. The synchronous-guard rule is satisfied;
  only the consultation is missing.
- ⛔ **REFUTES the sprint plan's second lead.** The profile file inputs are **visible**, not hidden +
  `.click()` — `ProfilePickerOverlayRoot.tsx:287`, `:437`, `:683` all render an inline styled
  `<input type="file">`. The known-broken CEF pattern does **not** apply here. Do not "fix" it.
- ⚠️ **The second half is NOT explained**: *"selecting an image anyway does not populate."* Hiding the
  overlay does not cancel a `change` event. Leading hypothesis: `HideProfilePanelOverlay` hides the
  HWND that **owns** the modal file dialog. **Unmeasured.** ⛔ Do not assume one fix covers both halves.

### 2.7 Gaps found that are in none of the three reports

| ID | Finding | Status |
|---|---|---|
| **G1** 🚨 | `WM_DPICHANGED` (`cef_browser_shell.cpp:1630`) calls `NotifyScreenInfoChanged()` for **7** overlays (settings, walletPanel, omnibox, cookie, download, profile, menu). **Missing: notification, backup, brc100auth, siteinfo, tablist, bookmarks, settingsmenu.** `WM_SIZE` calls `WasResized()` for most, but **never `NotifyScreenInfoChanged()`** — so those overlays keep a **stale `device_scale_factor`** after a monitor change and render at the wrong scale. **The notification overlay is the money-path modal.** | MEASURED |
| **G2** | `SetProcessDpiAwarenessContext` still **discards its `BOOL`**, and is now at `cef_browser_shell.cpp:4573` — the matrix doc says **4412**. Doc is stale. (The dev-safeguard `MessageBoxA` above it creates a window pre-awareness, but only on the abort path.) | MEASURED |
| **G3** | **Windows "Make text bigger" is not honoured anywhere.** Zero hits across `cef-native/` for `TextScaleFactor`, `GetSystemMetricsForDpi`, `SystemParametersInfoForDpi`, `AdjustWindowRectExForDpi`. Answers the owner's open question: **we do not honour it at all.** For OSR overlays we supply the scale ourselves and never read the accessibility setting, so wallet and consent text does not grow for users who rely on it. | MEASURED |
| **G4** | Overlays run OSR with `runtime_style` left at `DEFAULT` (`SetAsWindowless` would set `ALLOY`). Consequence unknown. | MEASURED / unassessed |
| **G6** 🚨 | **The notification overlay has NO `WM_MOUSEWHEEL` handler — every consent, payment and connect modal is unscrollable by mouse wheel.** Same for `BRC100Auth`, `Backup`, `SettingsMenu` and `Omnibox` (0 handlers each); the 9 dropdown overlays all have one. ⭐ **This is a RIVAL hypothesis to §2.2 for the modal-unclickable ticket, not a complement** — at 150 % on a 1366×768 screen the modal content exceeds the viewport and the buttons are simply below the fold with no way to reach them. The two are distinguishable and the acceptance test **must** separate them: the coordinate bug leaves the button **visible but unclickable**; the no-scroll bug leaves it **not visible at all**. Item 7's report ("hovering does not highlight, slightly above does") is the coordinate class; the modal-unclickable report is ambiguous between the two. ⛔ Fixing one and declaring the ticket closed is the failure mode here. | MEASURED |
| **G5** | `DPI_RESOLUTION_TEST_MATRIX.md`'s pass criteria **and** its single programmatic assertion cover the **header / toolbar only**. Nothing about overlays or modals — where items 1/2/7 live and where P0.8 just changed layout. | MEASURED |

### 2.8 Ranking — likelihood × blast radius

| Rank | Item | Likelihood | Blast radius |
|---|---|---|---|
| 1 | §2.2 mouse-coordinate scale error (item 7 **+** modal-unclickable) | high | 🚨 money path — wrong control activated on a consent modal |
| 2 | **G1** stale `device_scale_factor` on the notification overlay | measured | 🚨 money path |
| 3 | §2.6 item 2 — hook ignores `g_file_dialog_active` (9 hooks) | certain | user-visible, no money path |
| 4 | **G6** notification/auth/backup overlays cannot wheel-scroll at all | certain | 🚨 money path — a consent control below the fold is unreachable |
| 5 | §2.3 2 wheel sites with unconverted screen coords | certain | scroll misdirection |
| 6 | §2.5 item 1 — window-vs-content dead zone | certain (macOS) / unmeasured (Windows) | UX friction |
| 7 | **G3** text-scale accessibility | certain | accessibility, all surfaces |
| 8 | **G2** discarded `BOOL` | certain | diagnostic only |
| 9 | **G4** `runtime_style` | unknown | unknown |

---

## 3. Goal

At every DPI matrix cell, a click in any Hodos overlay lands on the control the user aimed at, an
overlay stays open while a native file dialog is on screen, and a click just below a panel closes it.

## 4. Done means

- [ ] The pointer coordinate delivered to an overlay browser equals the user's cursor position, at
      100 / 125 / 150 %, **measured in one instrumented log line**, not eyeballed.
- [ ] Every control on the connect / payment consent modal is clickable at cells #4, #6, #9.
- [ ] Choosing a profile picture: the panel stays open through the Explorer dialog **and** the chosen
      image populates.
- [ ] A click in the strip immediately below any panel closes it.
- [ ] `DPI_RESOLUTION_TEST_MATRIX.md` has an **overlay/modal** section with pass criteria (G5).
- [ ] `P0.9-A5` discharged: the branded loopback prompt is clickable at #4 / #6 / #9.

## 5. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-CLOSE` | Overlay close guards | This phase **adds a fourth consultation** of `g_file_dialog_active` (the hook path). A guard that is too broad leaves overlays un-closable; too narrow and item 2 persists. All three existing paths must still behave. |
| `R-GOLD` | Gold pill payment indicator | Touching the notification overlay's input and view geometry. The pill fires from `HttpRequestInterceptor :: OnWalletCallSuccess`, not from overlay input — but the notification overlay is on the same money path and this phase resizes/re-notifies it. |
| `R-PERIM` | Four privacy-perimeter gates | A mis-scaled click on a consent modal can activate the *opposite* control. Until §2.2 is measured we **cannot rule out that a scaled click approves something the user tried to refuse.** |
| `R-COUNT` | Per-session counters | Only if an overlay lifecycle change alters tab-close ordering. Audit, do not touch. |

## 6. Evidence table

> Measured results are recorded in `MEASUREMENTS.md`. 🔴 **W1 is settled: libcef takes the
> coordinate verbatim, so the conversion is the right fix and will not regress a working path.**

⛔ No empty RED or SUBJECT cells. A green result is reported with its red half or not at all.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail, and how | 🎯 SUBJECT — proves the right thing was measured | Tier | Result |
|---|---|---|---|---|---|
| `P1-A1` | **Instrumented hit test.** One log line per click carrying: raw `GET_X_LPARAM` client pt, converted view pt, `GetWindowRect`, `GetViewRect` result, `GetDpiForWindow`. At 150 % the converted pt ≈ raw ÷ 1.5 and the click reaches the aimed element. | Set `HODOS_OVERLAY_RAW_MOUSE=1` (bypass the conversion) at 150 % → the aimed element is **not** the one that fires, and the miss distance matches `p·(1−1/s)` within a pixel. ⛔ Not "a click at 100 % also works" — that is the weak control and proves only that `s=1` is a no-op. | The **notification** overlay browser, identified by CEF role `"notification"` / its browser id — **not** "a page in CDP". The header and ~14 overlays all report `type:"page"`; driving the wrong one faked a bug here before. ⭐ The probe must **assert the instrumentation line was emitted with all six fields**, or "no offset observed" is indistinguishable from "no logging". | T3 | 🔴 **RED OBSERVED** (`MEASUREMENTS.md` M1 row b) |
| `P1-A2` | ⚠️ **Element identity, not click count.** The element that receives the event is the one under the cursor, asserted by id/text. | With the conversion off, a *different* named element receives it. | ⛔ **Guard-in-front-of-the-property trap:** on a small centred modal a 50 % error can still land *inside the modal* on a neighbouring button — which reads as "works" if we only assert that something was clicked. Assert **which** element. | T3 | 🔴 **RED OBSERVED** — aimed `BUTTON:Advanced`, got `DIV:Refresh…` (M1 rows b+d) |
| `P1-A3` | Pure unit test: `PhysicalToViewPoint(px, py, dpi)` returns `(px·96/dpi, py·96/dpi)` for dpi ∈ {96,120,144,168}. | Feed the identity function → the 120/144/168 cases fail. | `hodos_tests` (`build/bin/Release`, **not** `build/tests/Release`). Off-CEF, like `JsStringEscape.h`. | T1 | ⬜ |
| `P1-A4` | **T0 ratcheted gate:** no `SendMouse*Event` call site takes a raw `GET_X_LPARAM`. Baseline **48 → target 0**. | Re-introduce one raw site → gate fails. | `scripts/preflight.ps1`, both modes. Gate must name the file:line it rejects. | T0 | ⬜ |
| `P1-A5` | **Window-vs-content delta** measured per overlay; each ≤ 2 px, or the close test uses **content** bounds. | Pre-fix run records a **non-zero** delta on at least one Windows overlay (macOS measured 45 px on the menu). If Windows measures 0 everywhere, item 1 has a different cause and this row must say so. | `getBoundingClientRect()` **inside the overlay browser**, normalised by `s`, vs `GetWindowRect` in physical. Same overlay, same instant. | T3 | ⬜ |
| `P1-A6` | Profile panel survives the whole Explorer dialog **and** the chosen image populates. | Remove the guard again → the panel closes on the first Explorer click. **Second half needs its own red**: if it still fails to populate with the panel open, that is a *separate* defect and gets its own row. | `ProfilePanelMouseHookProc` specifically — the `WM_ACTIVATE` path already passes and would green this vacuously. | T3 | ⬜ |
| `P1-A7` | `NotifyScreenInfoChanged()` reaches **all 15** overlays on `WM_DPICHANGED` (G1). | Drop `notification` from the list → after a monitor drag it renders at the old scale. | The **notification** overlay after cell #9 drag, not at startup. | T3 | ⬜ |
| `P1-A8` | Cells **#4 / #6 / #9** pass for the header **and** the overlay section (G5), incl. the branded loopback prompt (`P0.9-A5`). | Cell #1 (100 % / 1920) passes in the same run — the negative control for "is this DPI-specific or broken everywhere". | ⛔ **M1** (Windows Settings + sign-out) for anything depending on the startup `WM_DPICHANGED`. M2 (`--force-device-scale-factor`) is the fast lever but is **not** sufficient evidence for that path. | T3 | ⬜ |
| `P1-A9` | The 2 unconverted `WM_MOUSEWHEEL` sites (`:1737`, `:2996`) convert like their 7 siblings. | Pre-fix: scroll the settings overlay with the window on a monitor at negative screen X → the wheel event carries an out-of-range point. | Settings + Download panel browsers specifically. | T2/T3 | ⬜ |

| `P1-A10` | Every control on the connect / payment modal is **reachable** at cells #4/#6/#9 — either the content fits, or the wheel scrolls it. | Pre-fix at cell #6: the modal's lower controls are **off-screen** and the wheel does nothing (0 handlers → nothing to disable; the RED is the shipped state, captured before the fix). | ⛔ **Must be reported separately from `P1-A1`/`P1-A2`.** A screenshot showing the button **visible** distinguishes the coordinate defect from this one. Subject = the **notification** overlay, at a window size where its content overflows. | T3 | 🔴 **RED OBSERVED** — `elementFromPoint` = `NONE` at physical y=900 (M1 row c) |

| `P1-A11` | The advanced wallet **Dashboard** fits a normal laptop viewport with no scrolling, and the Send card's border box matches the Recent Activity card's. | Re-inject the leaked `.form-group{margin-bottom:20px}` at runtime: content height goes 460 → **516** against a 460 client box and the Send button's bottom goes 771 → **831**, past the 808 viewport. Observed, and the probe asserts the injected rule actually computed to `20px` before trusting the result. | Measured inside the **`/wallet` tab browser** via CDP (`cdp.py`, exact-URL match) at `dpr=1.25` on `DISPLAY1` — not the `wallet-panel` overlay, which is a different browser with a different copy of the same form. | T3 | 🟢 **GREEN, red observed** (`MEASUREMENTS.md` M4) |

**Two-sided pairing:** `P1-A1`/`P1-A2` are each other's control — A1 proves the *number* is right, A2
proves the *target* is right. A fix satisfying one while breaking the other is the real risk here.

### 6.1 Scope amendment — 2026-08-25, owner request during the DPI session

The advanced wallet **Dashboard** required scrolling to reach the Send form, and the Send card's
border was visibly a different height from the Recent Activity card beside it. Added to this phase
rather than ticketed, because it is the same defect class the phase exists for — *overlay/panel
content does not fit the viewport it is given* — and it was reproducible in the same session on the
same 125 % display. Row `P1-A11`.

⚠️ **Deliberately NOT done:** no font size was changed (owner: the text has already shrunk and must
not shrink further), and the **unscoped** `.form-group { margin-bottom: 20px }` in `WalletPanel.css`
was **left alone** — it leaks into every page that imports that stylesheet, but the wallet overlay
panel's own form is laid out against it. The override is scoped to `.wd-send-card`. Widening that
cleanup is its own change with its own blast radius.

## 7. Blast radius

- `cef-native/cef_browser_shell.cpp` — 15 overlay WndProcs, 48 forwarding sites, 9 `WH_MOUSE_LL` hook
  procs, `WM_SIZE`, `WM_DPICHANGED`, `WM_ACTIVATEAPP`, `ShellWindowProc`.
- `cef-native/src/handlers/my_overlay_render_handler.cpp` — the other three legs of the contract;
  **shared with macOS**, so any change there needs a Mac relay item.
- `cef-native/src/handlers/simple_app.cpp` — 15 overlay creators (sizing only; creation is not being
  restructured).
- `frontend/src/pages/ProfilePickerOverlayRoot.tsx` — read-only unless item 2's second half is React.
- Not touched, but in the audit: `HttpRequestInterceptor.cpp` (gold pill emit site),
  `PermissionService` (R-PERIM), `TabManager` (R-COUNT).

## 8. Out of scope

- **G4 `runtime_style`** — recorded, ticketed, not changed. Changing the runtime style of 15 OSR
  browsers is its own phase.
- **G3 text-scale support** — the *finding* lands here (the owner asked); *implementing* accessibility
  text scaling does not. Ticket it.
- Restructuring overlay creation, or converting overlays to windowed browsers. Tempting — it would
  delete the whole hand-forwarding path — and firmly out of scope for beta.3.
- `TICKET_brand_remaining_permission_prompts.md` — deliberately blocked behind this phase.
- macOS symptom (b): no second monitor exists on that side and none is coming.
- Reported items 3, 4, 5, 6 (WS2/WS3/WS4).

## 9. What stays manual — said plainly

- **Every DPI cell is T3: a human at the machine.** There is no automated DPI harness in this repo,
  and ⛔ `SendInput` **clicks** are dropped in this agent environment (moves work) — physical click
  automation cannot be driven from here.
- Cell #9 requires the owner dragging the window across the monitor boundary.
- Item 7's second-monitor confirmation is owner-only. ⭐ But per §2.2 #4, if #4/#6 reproduce on a
  single monitor, the *fix* does not depend on that instrument — only the closing confirmation does.
- The rendered appearance of every overlay at #4/#6/#9 needs human eyes
  (`feedback_consent_surface_needs_human_eyes`) — no gate sees layout.
- **What can be automated:** `P1-A3` (pure unit test) and `P1-A4` (T0 ratcheted grep gate). Nothing else.

## 10. Rollback

Single commit revert. The coordinate change is one helper plus mechanical call-site routing; the
item-2 change is one condition added to 9 hook procs; `NotifyScreenInfoChanged` is 8 added calls. No
schema, no crypto, no persisted state.

## 11. Adversarial review — answered before the plan is accepted

### 11.1 What would make the coordinate fix wrong?

| # | Failure mode | How we would find out |
|---|---|---|
| **W1** | ⭐ **CEF may already scale OSR mouse input internally.** If libcef multiplies incoming `CefMouseEvent` by `device_scale_factor` before hit-testing, then dividing here would produce a *new* offset in the opposite direction and we would ship a regression on top of a working path. The API comment says "relative to the upper-left corner of the view", and our view is logical — but a comment is not the implementation. | ⛔ **This must be settled by `P1-A1`'s instrumented run BEFORE any conversion is written**, not after. If the raw (unconverted) coordinate already lands correctly at 150 %, the hypothesis is dead and the cause is elsewhere. That is the single most important measurement in this phase. |
| **W2** | **`GetViewRect`'s truncation.** `logW = (int)(physW / scale)` floors. At 125 % a 380 px CSS panel is 475 physical → 380.0 logical, exact; but a clamped panel height (`mainRect.bottom − overlayY`) is arbitrary and will not divide evenly. A conversion using a different rounding rule than `GetViewRect` puts the last row/column of pixels in the wrong place. | Use **one** helper deriving scale from `GetDpiForWindow(hwnd)` exactly as `GetViewRect` does, and unit-test the pair together (`P1-A3`). |
| **W3** | **Stale DPI on the HWND during a monitor drag.** `GetDpiForWindow` is queried at event time; if `WM_DPICHANGED` has fired but the HWND has not yet been repositioned, view rect and mouse conversion briefly disagree. | Cell #9 is the test. Transient, but on a consent modal a transient wrong click is still a wrong click. |
| **W4** | **Regressing a cell that passes today.** At 100 % the conversion is `×1` — a no-op. So cells #1 and #2 **cannot** regress from the arithmetic. They *can* regress from the refactor itself (a mis-routed call site, a wheel site double-converted). | Cell #1 is run in the same session as #4/#6/#9 (`P1-A8` RED column) precisely to catch this. |
| **W5** | **`WM_MOUSEWHEEL` double conversion.** 7 sites already do `ScreenToClient`; a blanket sed that adds a conversion would leave those 7 correct-then-wrong. | The helper takes an explicit `already_client` distinction, and `P1-A4`'s gate names file:line rather than counting. |

### 11.2 What would make the item-2 fix wrong?

- **Too broad:** if the hooks consult `g_file_dialog_active` and the flag is never cleared, every
  dropdown overlay becomes un-closable until the app regains focus. The flag clears on
  `WM_ACTIVATEAPP(TRUE)` only (`cef_browser_shell.cpp:1483–1485`) — if the user dismisses the dialog
  *without* the app regaining activation, the flag sticks. **`R-CLOSE` RED must be run per flag**, and
  a stuck-flag case must be tried deliberately.
- **Vacuous green:** `ProfilePanelOverlayWndProc`'s `WM_ACTIVATE` already honours the flag, so a test
  that only proves "the panel stayed open" may be passing on the WndProc path while the hook path is
  still broken. `P1-A6`'s SUBJECT column pins it to the hook.
- **The second half may not be the same bug.** *"Selecting an image anyway does not populate"* is not
  explained by a close-path defect. If it persists with the panel open, it gets its own row and its
  own cause — ⛔ do not let one fix claim both halves.

### 11.3 Where a check could go green for the wrong reason

- 🚨 **`P1-A2` is the guard-in-front-of-the-property case.** On a centred modal, a 50 % coordinate
  error can still land *inside* the modal and hit a neighbouring button. A harness asserting only
  "a click was received" would report GREEN with the bug fully present — and on a consent modal the
  neighbouring button is the opposite decision. **Assert element identity.**
- ⭐ **`P1-A1`'s injection proof.** If the instrumentation is not actually emitted (a build flag, a log
  level, a renderer-process logger that is dead — as farbling's was), "no offset observed" is
  indistinguishable from "nothing measured". The probe asserts the log line exists with all six
  fields before it reads any value.
- **`P1-A5` can go green vacuously** if the overlay under test happens to be one whose content fills
  its window. The row must name which overlay measured non-zero, or state plainly that Windows
  measured 0 everywhere and item 1 therefore has a different cause than macOS.
- **Subject risk, twice burned:** the header and ~14 overlays all report `type:"page"` over CDP.
  Every row that drives a browser names the role, not the CDP type.

### 11.4 Claim vs measurement — the standing ledger

| MEASURED (source read today, re-verified, not inherited) | CLAIM (not reproduced by anyone) |
|---|---|
| Overlays are windowless/OSR — 5 independent confirmations (§1) | The coordinate error is the cause of item 7 (§2.2) |
| `SendMouse*Event` receives physical client px, unconverted, at 48 sites | The same error causes the modal-unclickable ticket |
| The other 3 legs of the contract convert correctly | Item 2's second half is caused by hiding the dialog's owner HWND |
| 0 of 9 `*MouseHookProc` consult `g_file_dialog_active` | Item 1's Windows delta is non-zero (macOS's is) |
| Profile file inputs are **visible**, refuting the hidden-input lead | |
| 2 of 9 `WM_MOUSEWHEEL` sites lack `ScreenToClient` | |
| G1: 7 of 15 overlays get `NotifyScreenInfoChanged` on DPI change | |
| G2: the `BOOL` is discarded; the doc's line 4412 is now 4573 | |
| G3: nothing in `cef-native/` reads the Windows text-scale setting | |

⚠️ **Nothing in the right-hand column may be treated as established until its evidence row is GREEN
with its RED observed.** The premise of this phase has already been corrected twice by exactly that
mistake.

---

## Sign-off

- [ ] Owner signs off on this assessment **before any code is written**
- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1` run — result + date recorded below
- [ ] `scripts/preflight.ps1 -NegativeControl` run — every T0 gate seen to fail
- [ ] `reset_test_state.py verify` exit 0 recorded before each acceptance run
- [ ] `../REGRESSION_SET.md` run in full at this boundary — result recorded
- [ ] `../HARNESS.md` §4 baseline recorded for `P1-A4` (48 → 0)
- [ ] `DPI_RESOLUTION_TEST_MATRIX.md` overlay section added; stale line ref 4412 → 4573 corrected
- [ ] Mac relay item raised for `my_overlay_render_handler` changes
- [ ] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight | | | |
| preflight -NegativeControl | | | |
| reset_test_state verify | | | |
| regression set | | | |
| adversarial review | | | |


---

## macOS evidence — round 2026-08-26b (Mac session)

Answers D6. Full narrative in `../MAC_RELAY_P1_ROUND.md`, ROUND 2026-08-26b.
Subject for every measured row: ad-hoc-signed **dev** bundle `cef-native/build/bin/HodosBrowser.app`,
`HODOS_DEV=1`, profile `Default`, wallet **31401**, CDP 9322, `NSScreen.backingScaleFactor = 2.00`.

| ID | 🟢 GREEN | 🔴 RED | 🎯 SUBJECT | Tier | Kind |
|---|---|---|---|---|---|
| `P1-M1` | macOS has **no** unit gap between window and view: wallet overlay measures NSWindow `visibleFrame(795) − 96 = 699 pt` vs CEF view `400 × 698` css px, `devicePixelRatio = 2` = `backingScaleFactor`. The Windows root cause (physical client vs logical view) does not exist here. | Not run as a click. The CDP-click harness I first wrote is **void** — `Input.dispatchMouseEvent` enters below the native NSView→`CefMouseEvent` layer and would pass with the defect present (HARNESS §6 Q1). A real-`CGEventPost` probe printed four misses **and then failed its own positive control** (a click on the known-good main window also registered nothing); root cause measured — `CGEventPost` is Accessibility-blocked for the session process. Result discarded. | dev bundle, wallet overlay target `127.0.0.1:5137/wallet-panel?iro=0` | T3 | MEASURED (structural) / **NOT RUN** (click) |
| `P1-M2` | macOS never re-queries screen info: **zero** occurrences of `windowDidChangeBackingProperties`, `NSWindowDidChangeScreen`, `NSApplicationDidChangeScreenParameters`; `NotifyScreenInfoChanged` has no macOS call site. `GetScreenInfo` itself is correct (`my_overlay_render_handler.mm:348,354` reads live `backingScaleFactor`), and the overlay picked up `dpr = 2` **at creation**. | The Windows victim class cannot occur here: header (`cef_browser_shell_mac.mm:5553`) and tabs (`TabManager_mac.mm:116`) are `SetAsChild` (windowed). Exposure is limited to the 14 `SetAsWindowless` overlays. ⚠️ Whether a long-lived overlay keeps a stale scale across a Retina↔non-Retina move is **NOT MEASURED** — needs the second display (WS1(b)). Not upgraded to a bug. | source tree at HEAD `64478a8` | T1 | CODE_READING |
| `P1-M3` | `InstallClickOutsideMonitor` (`OverlayHelpers_mac.mm:76`, both halves) and `InstallMenuClickOutsideMonitor` (`:5756`) do **not** consult `g_file_dialog_active`. The four references are four *other* monitors — profile `:4107`, bookmarks `:4274`, site-info `:4406`, tab-list `:4535`. The avatar picker lives in the profile panel (`:4207` → `/profile-picker`), which **is** guarded. | 🚨 **New macOS-only defect:** the flag is never cleared here. Four writes exist repo-wide; the only reset (`cef_browser_shell.cpp:1505`, `WM_ACTIVATEAPP`) is in a translation unit `CMakeLists.txt:349` compiles on Windows only, while the setter (`simple_handler.cpp:8927`, `OnFileDialog`) is **shared**. So the flag latches true at the first file dialog and those four panels lose click-outside dismissal for the rest of the session. Runtime confirmation deferred with the rest of the click batch. | source tree at HEAD `64478a8` | T1 | CODE_READING |
| `P1-M4` | Sizing contract confirmed cross-platform, and **already drifted**: macOS hard-codes every overlay height in C++ while React owns content height. 4 of 8 overlays disagree with the Windows table — downloads 400/**500**, bookmarks 480/**520**, privacy-shield 370/**500**, wallet-panel 740/**dynamic**. The two best-behaved rows on both platforms are the two that are not fixed constants. | Recommendation recorded in the relay: derive the window from reported content height rather than re-tuning constants — one mechanism covers the 45 px menu strip, the 157 px profile strip and the 107 px profile-edit overflow. Not implemented; owner/Windows decision. | `cef_browser_shell_mac.mm` `:2790 :4024 :4157 :4323 :4453 :4593 :5821`, `:2900` | T1 | CODE_READING |
| `P1-M5` | `std::cout`/`std::cerr` reach **no log** on macOS either — Windows' finding holds verbatim. Paired probe, same function, same startup: `Logger` (`LOG_INFO_PM`) → present in `HodosBrowserDev/debug_output.log`; `std::cout` (`ProfileManager.cpp:146`, no Logger twin) → absent from both `debug_output.log` and `debug.log`, present **only** in the launcher's inherited stdout. | Positive control is the Logger line from the same function in the same run: `[2026-08-26 12:01:19.009] [BROWSER] [INFO] 👤 Orphan sweep: 'Profile_3' …`. So the sink was live and the absence is real, not a dead instrument. ⇒ `cef-native/CLAUDE.md`'s "stdout is redirected" needs **striking**, not a platform qualifier (HARNESS §8). | dev bundle pid 72076 | T2 | MEASURED |
| `P1-M6` | The orphan sweep **works** on macOS: `Profile_3` (unlisted, carries `bookmarks.db`) → `Profile_3.orphaned-1787767279` on startup. It does **not** silently no-op. | ⚠️ But it works on **one marker out of six**. Measured against real profile dirs: `Preferences`/`History`/`Cookies`/`Local Storage` are all one level deeper (`Profile_N/Default/…`), `Network` exists at neither level, and only `bookmarks.db` resolves — because that one is *ours* (`BookmarkManager.cpp:47`), not Chromium's. Negative controls, same run: `Profile_1` (listed) not swept; `Profile_2` (listed, no markers) not swept; ⭐ `Default.backup.1783450862` — **unlisted AND carries `bookmarks.db`** — correctly excluded by `IsGeneratedProfileDirName`. Race guard survives (`CreateProfile` writes only the dir + `settings.json`). | `~/Library/Application Support/HodosBrowserDev/` | T2 | MEASURED |
| `P1-M7` | 🚨 **Build blocker, fixed:** `mac/entitlements.plist` carried a literal `--` inside an XML comment (from `33722d0`), which `codesign`'s AMFI parser rejects — every macOS bundle unsigned since 2026-08-18, and `release.yml` feeds that file to six codesign steps. | Two-sided control: non-ASCII-stripped variant **also fails** (first hypothesis refuted); comment collapsed to one line **also fails** (second refuted); comments removed **signs**; original with only `--options runtime` reworded **signs**; putting the `--` back **restores the failure**. Post-fix signature carries all six keys incl. `device.audio-input`. Swept every project-owned plist — sole instance. | `cef-native/mac/entitlements.plist` | T0/T4 | MEASURED |
| `P1-M8` | 🚨 **`hodos_tests` did not compile on macOS**, taking all ~263 cases down — `tests/overlay_mouse_test.cpp` uses `hodos::PhysicalToView` while `include/core/OverlayMouse.h` is entirely `#ifdef _WIN32`, and `tests/CMakeLists.txt:44` adds it unconditionally. Fixed with the `update_fs_test.cpp` precedent (test-only, HARNESS §6). | After the fix: **263 tests / 262 pass / 1 skip** (`UpdateStagerRig.StagesFromLocalFeed`). Red half was the observed compile error `use of undeclared identifier 'hodos'` → `make: *** [hodos_tests] Error 2`. | `cef-native/tests/` | T1 | MEASURED |
| `P1-M9` | `mac_build_run.sh` gained the Sparkle embed step it never had; the dev bundle now launches on a machine that has `external/Sparkle.framework`. | Red half observed before the fix: bundle built and signed, then `dyld: Library not loaded: @rpath/Sparkle.framework/Versions/B/Sparkle … Abort trap: 6`. Cause: `CMakeLists.txt:482-489` links it whenever present; only `release.yml:862-894` ever copied it in. | `cef-native/mac_build_run.sh` | T2 | MEASURED |
