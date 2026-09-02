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

---

# 📋 ADDENDUM 2026-09-01 (Windows) — Phase 4 has **LANDED**. M3 was written before the code existed.

Commits `0b68727` (menu) + `420b722` (mute tab), both on `origin/0.4.0`.
Detail: `phase-4-tab-peripheral-parity/{PHASE_CONTRACT.md,MEASUREMENTS.md}` (M1–M17).

⭐ **Nothing here changes your ask** — it is still "one new overlay, when you get to it". This exists
because M3 above described a plan, and you should be working from what actually shipped.

## ✅ M6 — What the 15th overlay actually is, so you are not reverse-engineering it

| Piece | Windows file |
|---|---|
| React page | `frontend/src/pages/TabContextMenuOverlayRoot.tsx`, route `/tab-context-menu` — **cross-platform, already yours** |
| Create/Show/Hide trio | `simple_app.cpp :: Create/Show/HideTabContextMenuOverlay` |
| WndProc + click-outside hook | `cef_browser_shell.cpp :: TabMenuOverlayWndProc`, `TabMenuMouseHookProc` |
| Role | `tabmenu` (`BrowserWindow.cpp`, both accessor arms) |
| IPC | `tab_context_menu_show` / `_hide` / `_action` / `_request_context` — **all in cross-platform `simple_handler.cpp`**; the `_show` arm's macOS branch currently just logs "no macOS implementation yet" |

⭐ **The only genuinely macOS work is the window itself** — a borderless `NSWindow` + click-outside
monitor + `Create…OverlayMacOS`, then replace that `#elif defined(__APPLE__)` log line. Every action,
the target-tab bookkeeping, the menu contents and the mute state are already cross-platform and will
work the moment the window exists.

⚠️ **Two traps, both of which cost us time:**
1. It is anchored to the **cursor**, not to a toolbar icon — the only one of the 15. Your positioning
   helper takes an anchor point, not an icon offset.
2. On Windows a new overlay had to be added to `ReleaseOverlaysOwnedBy`'s **explicit list** or it is
   destroyed with its owner window. Whatever your equivalent lifetime list is, check it has the new
   window in it. This is Phase 3.5's K12 hazard, and the list names its members one by one.

## ✅ M7 — Mute tab shipped too, and it is **already cross-platform except for the trigger**

👤 Owner-requested after the menu landed. `Tab::muted` + `SetAudioMuted`/`IsAudioMuted`, all in
`Tab.h` and `simple_handler.cpp` — **no `_mac` arm needed**. It will start working on macOS as soon
as the overlay above can raise the menu.

🚨 **The finding worth carrying over:** 📏 **CEF's mute does NOT survive a navigation.** Mute a tab,
navigate it (same-origin *or* cross-origin — same `CefBrowser`, no `OnBeforeClose`), and
`IsAudioMuted()` reads back **false**. The mute is per-**document**; the user's intent is per-**tab**.
So `Tab::muted` holds the intent and `SimpleHandler::OnLoadingStateChange` re-applies it. That hook is
cross-platform, so you inherit the fix — ⚠️ but **verify the re-apply actually fires on your side**,
because it is the difference between the feature working and it silently dying on the first link
click. Look for `🔇 Re-applied mute to tab N after navigation`.

⛔ **Do not add a "tab is making noise" speaker icon.** 📏 This CEF build exposes **no
`OnAudioStateChanged`** (checked across `cef-binaries/include`). We show a muted-only glyph, which is
the honest subset. If macOS's CEF headers differ here, that is a real finding — tell us.

## ⭐ M8 — A Windows-only defect where **macOS was already right**, so there is nothing to port

📏 On Windows, `TabManager::OnTabBrowserClosed` — the point where a tab actually stops existing —
**never notified the frontend**. Single closes were masked by React's optimistic removal; the new
"close other tabs" has no such path, and the tab strip sat stale for a measured **17.5 s**.

📏 **Your `TabManager_mac.mm :: OnTabBrowserClosed` already ends with
`SimpleHandler::NotifyTabListChanged()`** — it has always done the right thing, at exactly the point
we just moved Windows to. ⇒ **no port, no action.** Recorded because "Windows found a bug, therefore
macOS has it" is the wrong default in both directions, and here the platforms had genuinely diverged
with macOS ahead.

⚠️ One difference left standing, deliberately: yours broadcasts to **all** windows
(`NotifyTabListChanged`), ours pushes to the **one** affected window
(`NotifyWindowTabListChanged(closed_window_id)`). Both correct; ours is narrower. Not worth churning.

## 🔴 M9 — `P4-B2` (mic/camera) is unchanged and still yours

Windows is now **measured** across all three stored states — `allow` → `Continue()` + `RESOLVED
tracks=2`; `block` → `Cancel()` + `NotAllowedError`; `ask` → prompt + pending. So the *"reported
working"* claim in M4 is now a real result on our side.

⛔ **macOS is still ⬜ and will not be marked from our run** — TCC has no Windows analogue.
⚠️ **Method warning that cost us a run:** setting the stored state via `site_permissions_set` sent
**from the page** is correctly **DENIED** by the IPC allowlist, so both arms silently measure *Ask*.
Set it from an internal origin, and use a **fresh tab per state** — an outstanding permission request
blocks the next one.

## 🗒️ M10 — Roster + ticket updates since M3/M5

- `cef-native/CLAUDE.md` now says **Windows 15 / macOS 14** explicitly, and names this file as where
  the macOS half is tracked. ⇒ the parity line is no longer quietly wrong; it is loudly asymmetric.
- 🎫 One new beta.4 ticket: **split view** (`TICKET_split_view_needs_multi_visible_tab_model.md`).
  👤 Owner raised it, chose ticket-not-build. ⚠️ Relevant to you only as a heads-up that it would be a
  **window-layout** change on both platforms, not a menu item — our tab model shows exactly one tab
  per window, and yours does too.
- 🎫 `TICKET_tab_pin_and_mute_need_model_changes.md` **retitled and rescoped** — mute-tab is done, pin
  and mute-**site** remain. If you were holding it as one item, it is now two.

---

## 🔴 M11 — NEW, and it is **yours too**: a dead wallet backend is silent on both platforms

Filed 2026-09-01 as `0.4.0-beta.3/TICKET_wallet_backend_death_is_silent_and_unrecovered.md`.
👤 Owner put it in **beta.3**, not beta.4.

**What happens:** the browser spawns `hodos-wallet` **once at startup and never looks again**. If it
dies, nothing notices, nothing restarts it, and every dApp is told **"no wallet"** — which is
indistinguishable from *"your wallet is gone"* on an app that holds real money. 🚨 The hazard is a
user deciding to **restore from their recovery phrase** to fix what is actually "restart the app".

⛔ **This is not a Windows-only ticket, so please do not skip it.** 📏 I read your arm:
`cef_browser_shell_mac.mm` has the same shape — `SpawnWalletServer()`, a startup health-check loop,
then nothing. `g_walletServerRunning` is only ever set `true` (2 sites) and read (3 sites); it is
**never** set back to `false`.

⭐ **One place macOS is genuinely better than us, and it should survive the fix:** on health-check
timeout you log *"health check timed out"* and leave the flag **false**. Windows forces it `true`
with the comment *"Process was launched, just slow to start"* — a latch that lies. When supervision
is built, keep your honest-flag behaviour and make Windows match it, not the other way round.

⚠️ Same pattern on `g_adblockServerRunning`, both platforms. Lower stakes (ads stop being blocked),
but worth covering once rather than twice.

⛔ **Do not fix this by waking `WalletService::monitorDaemon()`.** 📏 It has **zero call sites**
outside `WalletService.cpp` on either platform, and on exit it only logs and `break`s — **no
restart**. `WalletService_mac.cpp` has its own `startDaemon`/`isDaemonRunning`, equally uncalled.
Adopting or deleting that API is a separate decision; §8 of the ticket keeps it out of scope.

### 🗒️ And a dev-hygiene rule that now applies to your box too

🚨 The ticket was found because I killed the owner's **production** wallet with a name-matched
`Stop-Process`. **All three processes share their image name between the dev and installed builds**
(`HodosBrowser` / `hodos-wallet` / `hodos-adblock`), so a name-matched kill takes down the user's
real browser and wallet.

⇒ `CLAUDE.md`'s Dev Runbook now carries the exe-path rule for **all three**, and
`scripts/stop-dev.ps1` implements it (Windows). ⭐ **If you kill dev processes by name on macOS
(`pkill hodos-wallet`, `killall`), you have the identical hazard** — the owner runs an installed
Hodos on his Mac too. A `scripts/stop-dev.sh` doing the same path-matched job would be a genuinely
useful thing for you to add; ⛔ I am not writing it blind for a platform I cannot test on.
