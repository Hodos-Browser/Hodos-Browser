# 📋 ROUND 2026-09-01 (Windows) — Phase 3.5 (WS2 cont.) landed · Phase 4 (WS3) kicked off

Its own file so it cannot conflict with `MAC_RELAY_BETA3.md`, `MAC_RELAY_P1_ROUND.md`,
`MAC_RELAY_P2_ROUND.md` or `MAC_RELAY_P3_ROUND.md` if you are editing those.

Detail: `phase-3.5-layout-window-scoping/{PHASE_CONTRACT.md,MEASUREMENTS.md}` (K1–K27) and
`phase-4-tab-peripheral-parity/PHASE_CONTRACT.md`.

---

## 👉 One-line ask

**Phase 3.5 is Windows-only and does not touch your build** — ✅ verified below. But it found a defect
class that **macOS very likely has too, by a more explicit mechanism than ours**, and **Phase 4 will
put one real item on your queue**: a new overlay needs a macOS creation function or the 14/14 roster
parity breaks. ⚠️ Everything below is written, compiled and measured **on Windows only**.

---

## ✅ M1 — No shared signature changed. Your build is unaffected.

📏 Verified by inspection (⛔ **not** compiled on macOS — I cannot build there):

| New symbol | Guard |
|---|---|
| `SimpleHandler::OwnerHwndForScaling()` | `#ifdef _WIN32` in `simple_handler.h` (L304–310) |
| `CreateOmniboxOverlay(..., BrowserWindow* targetWin)` | inside the `_WIN32` block of `simple_app.h` |
| `OwnOverlayToRequestingWindow` · `ReturnOverlayOwnershipToPrimary` · `ReleaseOverlaysOwnedBy` | all inside `simple_app.cpp`'s `_WIN32` block |

📏 `grep` across every `.mm` and header finds **no** reference to any of them. ⇒ nothing to do on your
side to keep building. Unlike the Phase 3 round, I did **not** change a function you share.

## 🚨 M2 — The defect, and why I think you have it — with your own line numbers

**What it was on Windows.** All 14 overlays were created `CreateWindowEx(..., g_hwnd, ...)` — *owned
by the primary window*, decided once at creation. Win32 keeps an owner and its owned windows together
as a z-order group, so with a **secondary** window in front:

- opening any dropdown pulled the **primary** up over the secondary — 👤 the owner saw their second
  window "disappear, just the omnibox floating over window A";
- closing one that held focus handed the primary back **both** the top of the z-order **and the
  keyboard**.

📏 Both measured (K9, K23). **One cause, both halves.** Fix: ownership now follows the window that
asked, for as long as the overlay is on screen — handed over on show, handed back on hide, plus a
safety net when a window closes (K26).

**Why macOS is likely the same, and in one place worse.** 📖 Code reading of
`cef_browser_shell_mac.mm` — ⛔ I have run **none** of this:

```objc
:2681   [g_main_window addChildWindow:g_settings_overlay_window ordered:NSWindowAbove];
:2842   [g_main_window addChildWindow:g_cookie_panel_overlay_window ordered:NSWindowAbove];
:2451   [g_main_window makeKeyAndOrderFront:nil];        // on notification-overlay hide
```

⭐ `g_main_window` is a **process global**, exactly analogous to our `g_hwnd`. AppKit child windows
move and order **with their parent**, which is the same coupling that caused the Windows bug — so
`addChildWindow:` on a process-global parent is the macOS shape of the same mistake.

🚨 And `:2451` is **more explicit than anything Windows had**: on hide it *literally* calls
`makeKeyAndOrderFront:` on the process-global main window. On Windows the primary being handed focus
was an emergent Win32 behaviour we had to measure; on macOS there is a line that does it on purpose.

### 🎯 The 60-second test, and both outcomes are informative

⛔ Do **not** port our fix blind. Measure first:

1. **Cmd+N** for a second window (⛔ not a second launch — same-profile launches forward to the
   running process, so a second *launch* may not give you two windows in one process).
2. Drag it so it overlaps the first.
3. In the **second** window, open a dropdown (menu / shield / wallet).

| Observation | Meaning |
|---|---|
| Second window drops **behind** the first | Same defect class. Then also close the dropdown and watch focus — that is the second half |
| Nothing moves | AppKit's child-window ordering does not couple the way Win32's owner group does ⇒ **you do not have it**, and that is worth recording |

⚠️ **Read z-order, not "did it disappear."** *Behind* and *minimised* look identical on screen and
have different causes — that distinction cost us a day (K9.1). On macOS, check window ordering and
`isMiniaturized`, not just whether you can see it.

## 📌 M3 — Phase 4 puts ONE item on your queue: a 15th overlay

Phase 4 builds a **tab context menu**, and 📏 the kickoff established it must be an **overlay**, not a
CEF page menu — a tab right-click lands in the header browser, not in web content.

⇒ `cef-native/CLAUDE.md` records Windows and macOS at **parity: 14 overlays each**. Windows will have
**15**. Per invariant #9, a new overlay needs a macOS creation function in `cef_browser_shell_mac.mm`.

⭐ **No rush and nothing is broken meanwhile** — the Windows build simply has a menu you do not. But
the parity line in `cef-native/CLAUDE.md` will be **wrong** the moment Phase 4 lands, and I would
rather you heard it from this file than found it in a doc that had quietly gone stale.

⛔ **I am not writing the macOS half blind.** Same reason as the Phase 3 round: a cross-platform
"fix" written from a Windows box and never executed is the failure this project keeps paying for.

## 🔴 M4 — `P4-B2`: mic/camera on macOS is yours, and it is unverified on both platforms

`OnRequestMediaAccessPermission` exists and honours `SitePermissionStore` (cross-platform code). But
📖 *"Windows is reported working"* is a **claim, not a measurement** — Phase 4 turns that into a real
T3 run on Windows across all three stored states (Allow / Block / Ask).

⛔ **macOS has never been verified at all**, and I cannot run it. It also has an OS-level layer we do
not: TCC. A site allowed in `SitePermissionStore` still fails if the **app bundle** lacks camera /
microphone entitlements or the user denied them in System Settings → Privacy & Security. That is a
macOS-only failure mode with no Windows analogue, so it cannot be inferred from our result.

⇒ run the same three states on your side and report them separately. `P4-B2` stays ⬜ until you do —
⛔ it will not be marked passed on the strength of the Windows run.

## 🗒️ M5 — Two smaller things worth knowing

- 👤 **Owner, on your rig (K20):** *"No DPI exist on the mac, I can plug it into one of these monitors
  when needed but normally I just have that mac laptop by itself."* ⇒ the **mixed-DPI** case Phase 3.5
  tested does not arise for you in normal use, and no Mac DPI rig needs standing up. ⚠️ The
  **multi-window** case still does — Cmd+N needs no second monitor.
- 🎫 Three new beta.4 tickets came out of this round, all filed under `0.4.0-beta.4/tickets/`. Two are
  ⚠️ **cross-platform by nature and unassessed on macOS**: multi-window session restore keeps only the
  last window's tabs, and menu → Exit closes the *primary* window rather than the one clicked. The
  macOS Exit path takes a different arm (`ShowQuitConfirmationAndShutdown()`) and **may not have the
  defect at all** — worth a two-minute check when you are next in that code.
