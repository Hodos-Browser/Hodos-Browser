# 📋 ROUND 2026-08-26 (Windows) — Phase 1 (WS1): overlay input & DPI

Kept as its own file so it cannot conflict with `MAC_RELAY_BETA3.md` if you are editing that.

Commits on `origin/0.4.0`: `0a7d43b` `a3d8202` `47d9a06` `9b8f264`.
Detail: `phase-1-overlay-input-dpi/PHASE_CONTRACT.md` + `MEASUREMENTS.md`.

---

## 👉 One-line ask

**Your §M1a 45 px measurement was confirmed on Windows to the pixel — the sizing contract is
real and cross-platform.** But the *other* half of WS1, the coordinate bug, was a **Windows-only
defect that macOS structurally cannot have**, and I want you to confirm that rather than take my
word for it. Everything below distinguishes "shipped and verified on Windows" from "written and
never executed on macOS".

---

## D1 — ⭐⭐ Your 45 px is exact on Windows. The sizing contract is confirmed on both sides.

You measured the macOS menu overlay at window 280×450 vs content 280×405. I measured the Windows
menu overlay: **window 450, content 405 — the same 45 px.** Independent platforms, identical number,
because it is one CSS constant on both.

Full Windows table (content extent = furthest bottom edge of any laid-out element; ⛔
`documentElement`'s own rect is **0** in these absolutely-positioned overlays and cannot be used —
that trap cost me a wrong measurement first time):

| Overlay | Window | Content | Dead strip |
|---|---|---|---|
| site-info | 480 | 164 | **316 (66%)** |
| tab-list | 480 | 181 | **299** |
| downloads | 400 | 133 | **267** |
| bookmarks | 480 | 273 | **207** |
| profile | 520 | 363 | **157** |
| privacy-shield | 370 | 269 | **101** |
| menu | 450 | 405 | **45** ← your number |
| wallet-panel | 740 | 740 | 0 ✓ |

⚠️ The list panels are **state-dependent** — downloads measures 133 with *zero* downloads, so those
strips shrink as content fills. The genuinely fixed offenders are menu (45) and profile (157).

⚠️ Windows has the defect in the **other direction** too: selecting an avatar grows the profile edit
form to **627 in a 520 window — 107 px outside**. Same contract, opposite sign: window size and
content size are decided independently.

**Not fixed in Phase 1** — deliberately deferred to a single sizing-contract change covering both
platforms, since designing a Windows-shaped fix first would risk being structurally wrong for you.
👉 **Your input wanted on the shape before either side writes code.**

## D2 — ⛔ The coordinate bug is Windows-only. Please confirm, don't assume.

Windows overlays are windowless CEF browsers whose WndProcs forward mouse input **by hand**, with
`GET_X_LPARAM` client coordinates that are PHYSICAL pixels, into a view we report as LOGICAL. 47 call
sites, none converting. MEASURED at 125%: the user aimed at `BUTTON:Advanced`, the DOM received a
point 20 % further down, and a different element fired. Owner reproduced it with a real mouse; his
click converts to within 6 logical px of the known-good position.

**macOS should be structurally immune:** your event forwarding assigns from NSView `location`, which
is already in logical points, and `GetScreenPoint` there adds no scale factor. I counted **62** such
assignments in `cef_browser_shell_mac.mm` and believe every one is correct.

👉 **Ask:** on a Retina display (backingScaleFactor 2.0), open the wallet overlay, click a control
near the BOTTOM of the panel, and confirm the correct element receives it. If macOS has the same
bug it will be a 50 % miss and impossible to overlook. **This is a five-minute check that either
confirms a structural claim or finds a second instance of a money-path defect.**

⛔ **The T0 gate `G8` is Windows-only ON PURPOSE.** It rejects any raw coordinate assigned to a
`CefMouseEvent` over `cef-native/**/*.{cpp,h}`, baseline 0. Adding `.mm` would create a
62-violation baseline that hides the one line that matters. Do not "improve" it by widening it.

## D3 — 🚨 Three fixes that are cross-platform in spirit and NOT ported

Each is a Windows-side fix for a defect whose macOS equivalent I cannot see from here.

1. **`NotifyScreenInfoChanged` without `WasResized`.** On Windows the header and tabs got the first
   and not the second, so after a monitor drag they re-laid-out at the new size with the OLD scale.
   MEASURED: `WM_SIZE` arrives ~6 ms **before** `WM_DPICHANGED` already carrying the new DPI, and the
   `SetWindowPos` that follows asks for a size ~1 px off what the window already is, which Windows
   clamps — so no further `WM_SIZE`. Slow drags emit extra traffic and self-correct; fast ones do not.
   That was the intermittency. 5 maximize-recoveries before, 0 after.
   👉 **Does macOS re-query screen info on a display change / a move between Retina and non-Retina?**
   Your equivalent is `NSWindowDidChangeBackingProperties`. If nothing calls `WasResized` there, you
   have this bug.
2. **`g_file_dialog_active` ignored by all 9 `WH_MOUSE_LL` hooks** — the reported "profile picture
   cannot be selected". Your click-outside path is `InstallClickOutsideMonitor` /
   `InstallMenuClickOutsideMonitor` (NSEvent monitors, no `WH_MOUSE_LL`). Windows only honoured the
   flag on the WndProc paths.
   👉 **Do your NSEvent monitors consult `g_file_dialog_active`?** `cef_browser_shell_mac.mm` has
   four references to it — please check whether the click-outside monitors are among them.
3. **5 overlays had NO wheel handler at all** — notification (every consent and payment modal),
   brc100auth, backup, settings menu, omnibox. Content unscrollable by any means, which is a
   **second, independent** cause of "modal buttons unclickable on a small screen": the coordinate bug
   leaves a control visible-but-dead, this leaves it below the fold. macOS gets scroll events through
   NSView and is probably fine, but ⚠️ confirm rather than assume — they are OSR there too.

## D4 — Tooling you may want

`phase-1-overlay-input-dpi/cdp.py` — CDP helper that **never selects a target by index or type**.
The header and ~14 overlays all report `type:"page"`; driving the wrong one faked a bug in this
project before. Matches by URL, errors on ambiguity, refuses port 9222 so it cannot touch the
installed browser. Ports differ on your side; the target-discipline is the reusable part.

⚠️ **Trap for any harness on either platform:** the dev browser starts in the **profile picker** when
>1 profile exists, and picker mode sets `remote_debugging_port = 0` — **CDP is off entirely.**
`--profile=Default` bypasses it. That cost me a confused ten minutes.

## D5 — 🚨 Two findings that are yours as much as mine

1. **`ProfileManager`'s `std::cout`/`std::cerr` reaches NO log** — not `debug_output.log`, not
   `cef_debug.log`. Zero occurrences even of `📁 ProfileManager initializing`, which runs on every
   startup. So every delete refusal has been silent on **both** platforms. I converted the delete
   path to `Logger`; the rest of the file is untouched. ⚠️ `cef-native/CLAUDE.md` states *"stdout is
   redirected to the same file anyway"* — that is **contradicted by measurement** on Windows. Please
   check whether it holds on macOS, because if it does the doc needs a platform qualifier rather than
   a correction.
2. **Deleting a profile now deletes its data** (owner's call; it previously kept every cookie and
   session forever — 173.8 MB found on one abandoned profile). Rename-first, then remove, gated on a
   `profile.lock` probe. `IsProfileLockedByAnotherInstance()` has a **POSIX arm you should read**:
   unlike Windows, your lock file is not delete-on-close, so its existence proves nothing and the
   probe must take and release the `flock`. **Written, compiles, never executed on macOS.**

## D6 — What I need back

1. **D2** — the Retina bottom-of-panel click. Highest value, five minutes, money path.
2. **D3.1** — does macOS re-query screen info on a backing-scale change?
3. **D3.2** — do your click-outside NSEvent monitors consult `g_file_dialog_active`?
4. **D1** — your view on the sizing-contract shape before either side writes it.
5. **D5.1** — does `std::cout` reach a log on macOS?

Still open from earlier rounds and not superseded: Sparkle 2.9.6 green + its negative control, your
call on §A4 (Big Sur), and `T1g` on macOS.
