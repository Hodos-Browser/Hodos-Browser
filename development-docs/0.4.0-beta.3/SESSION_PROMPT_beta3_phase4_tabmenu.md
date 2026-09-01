Start beta.3 Phase 4 — tab context menu & peripheral parity. ⛔ **Kickoff is DONE and the scope is
agreed with the owner. No code has been written.** Read the contract, do not re-scope it.

# 0. Read first, in this order

1. Auto-loaded `MEMORY.md`, then `project_p35_layout_rescoped_2026_08_31.md` — Phase 3.5 just landed
   and **its fix is load-bearing for this phase** (see §4).
2. ⭐ **`phase-4-tab-peripheral-parity/PHASE_CONTRACT.md` — all of it.** §0 records two claims in
   `SPRINT_PLAN.md` §WS3 that were **measured false** at kickoff; reading the sprint plan first will
   send you at the wrong machinery.
3. `HARNESS.md` (tiers, ratchets, §4, §8, §9) and `REGRESSION_SET.md`.
4. `phase-3.5-layout-window-scoping/MEASUREMENTS.md` **K8, K11, K21, K25, K26** — not for the fix, but
   because the **rig** and the **four ways this sprint got fooled** are recorded there.

⛔ **Do not re-derive the kickoff inventory.** It was taken against the tree on 2026-09-01 and it
already refuted the plan's description of the work.

# 1. State

✅ Phase 3.5 **LANDED and closed**, fixed at the cause (`3237068`), regression boundary run and
recorded 🟡 INCOMPLETE (`7c3d4fe`). ⛔ **NO Phase 4 product code exists.** `HEAD` is pushed to
`origin/0.4.0` — ⛔ **`git fetch` and read `git log HEAD..origin/0.4.0` yourself; the Mac side pushes
to this branch.**

# 2. ⛔ Scope — agreed 2026-09-01, do not widen

**IN:** six tab-menu items, each reusing what exists — **reload** (`navigate_reload`), **bookmark**
(`bookmark_add`), **duplicate** (`tab_create` + same URL), **new tab to the right** (`tab_create` +
`TabManager::ReorderTabs`, needs an insert index), **close other tabs**, **close tabs to the right**
(both loop `TabManager::CloseTab`). Plus **mic/camera verification on Windows**.

**OUT — 👤 owner decision, already ticketed** (`0.4.0-beta.4/tickets/TICKET_tab_pin_and_mute_need_model_changes.md`):
⛔ pin · ⛔ mute tab · ⛔ mute site — 📏 `Tab` has neither a `pinned` nor a `muted` field, and pin
persistence would touch `session.json`, which carries an open defect.
⛔ **macOS** — cannot be run from the Windows box. Relay it; never report it as passed.

# 3. 🚨 The two things the kickoff already established, so you don't rebuild the wrong thing

1. ⛔ **There is no tab context menu today.** 📏 No `onContextMenu` in the header UI, no tab-strip
   command IDs. `P4-A1` is therefore **RED by construction** — do not go looking for its RED.
2. ⛔ **`MENU_ID_USER_FIRST` is the WRONG machinery.** That is CEF's **page** context menu, which
   fires on web content inside *tab* browsers. A right-click on a tab happens in the **header**
   browser. ⇒ build **overlay #15**: `TabContextMenuOverlayRoot.tsx` + a `Create/Show/Hide` trio in
   `simple_app.cpp`, following the menu/profile pattern exactly.

# 4. 🚨 The trap that is specific to this phase

⭐ **A new overlay inherits Phase 3.5's ownership fix — but only if it follows the pattern.**
3.5 made overlay ownership follow the requesting window (`OwnOverlayToRequestingWindow` on show,
`ReturnOverlayOwnershipToPrimary` on hide, `ReleaseOverlaysOwnedBy` on window close). An overlay that
positions itself against `g_hwnd`, or that skips the `Show*Overlay(offset, targetWin)` shape, **will
reintroduce the entire phase 3.5 defect** — the secondary window vanishing behind the primary.
⇒ `P4-A4` is the control for exactly this. Run it.

⚠️ Also add the new HWND to `ReleaseOverlaysOwnedBy`'s list in `simple_app.cpp`. It enumerates the 14
overlays by name; a 15th that is not in that list is not protected when its window closes.

# 5. 🚨 The sharpest correctness risk: which tab

⛔ **`Tab::id` ≠ `CefBrowser::GetIdentifier()`** — translate with
`TabManager::GetTabIdForBrowserIdentifier`. Every one of the six actions targets a *specific* tab,
and the whole point is that it is **the right-clicked tab, not the active one**. `P4-A2`'s RED is to
wire it to `GetActiveTab()` and watch the action land on the wrong tab; `P4-A3` is its partner and
exists because "do nothing at all" would otherwise pass `A2`.

⚠️ **R-COUNT:** *close others* / *close to the right* close **many** tabs at once, and per-session
counters reset per tab close. That path has never been run at N>1 (`P4-A6`).

# 6. The rig — reuse it, it works

`phase-3.5-layout-window-scoping/` carries a working T3 rig; nothing needs rebuilding:
- `p35drive.py` — CDP driver. `list` · `key <hdr> 78 --ctrl` (Ctrl+N makes window B) · `send <hdr> <ipc> '[args]'` · `eval`.
  ⚠️ Two headers share the URL `http://127.0.0.1:5137/` — disambiguate by **target-id diff across Ctrl+N**, never by list order.
- `winprobe.ps1 -WatchSeconds N` — Z-order, rect, DPI, and a `Fg` (focus) column. ⛔ **Watch mode is
  mandatory for anything with a visible overlay**: overlays hide on focus loss, and clicking a console
  to run a probe **is** focus loss.
- `gapprobe.ps1` · `dpiprobe.ps1` · `movewin.ps1` · `z3probe.ps1` · `hideprobe.ps1`.
- 📏 **A mixed-DPI rig exists**: the laptop (`\\.\DISPLAY24`) is 1920×1200 at **125 %**. ⛔ Any probe
  must call `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` first or Windows lies to it.

# 6.1 Machine state — as left 2026-09-01

✅ **The dev environment is STOPPED.** Dev browser, dev adblock, dev wallet and Vite were all shut
down deliberately at the end of the previous session so this one starts clean. Nothing to kill first.

**To bring it up (all three, in this order):**
1. `.\dev-wallet.ps1` — sets `HODOS_DEV=1`, wallet on **31401**
2. `cd frontend && npm run dev` — Vite on **5137**
3. Launch the browser with PowerShell `Start-Process` after `$env:HODOS_DEV='1'`, args
   `--profile=Default` (bypasses the picker; dev CDP is then on **9322**).
   ⛔ A detached bash `&` launch comes up **minimized**. ⛔ Never launch the exe directly without
   `HODOS_DEV=1` — the dev safeguard blocks it, by design.

⛔ **The owner's INSTALLED browser is running** (~70 processes under `%LOCALAPPDATA%\HodosBrowser`,
wallet on **31301**). **Never touch it. Match by exe path, never by process name** — both builds ship
the same image name `HodosBrowser.exe`. To stop only the dev build:
`Get-CimInstance Win32_Process -Filter "Name='HodosBrowser.exe'" | Where-Object { $_.ExecutablePath -like '*cef-native\build\bin\Release*' } | Stop-Process -Force`

⚠️ The linker fails **`LNK1104`** while the dev browser runs, and `cargo build` fails *"Access is
denied"* while the dev wallet runs — stop them before building.
⚠️ `browser.restoreSessionOnStart` was flipped on for a Phase 3.5 test and **reverted to `false`**,
its pre-session value. ⛔ The shipped default was never checked — read `SettingsManager`, not
`useSettings.ts`'s frontend fallback.

# 7. 🚨 The four ways this sprint got fooled — all four are live for this phase

1. ⛔ **A green row on a sample of two is not a green row.** Phase 3.5 shipped a fix verified on 2 of
   9 overlays; it was a **no-op for 4 of them, including the wallet**. Here: six menu items and at
   least two window contexts — test the set, not a representative.
2. ⛔ **Test the *closing* path, not only the *opening* one.** Every row in 3.5's table tested opening
   an overlay; the owner found the dismiss defect in ten seconds because no row covered it.
3. ⛔ **A fix that corrects after the wrong thing happens is a patch** — and the user sees the
   correction. 3.5 shipped three of those before the root fix deleted all of them.
4. ⛔ **An instrument that gives different answers on repeat runs is not measuring the subject.**
   `hideprobe.ps1` gave three different results and one of them was *right symptom, wrong reason*.

# 8. Deliverable

1. The six items, each acting on the **right-clicked** tab in the **right window**;
2. every evidence row in `PHASE_CONTRACT.md` §5 GREEN with its RED **observed**;
3. `P4-B1` mic/camera **measured** on Windows across all three stored states — ⛔ *"reported working"*
   is what this row exists to replace;
4. `P4-B2` macOS — ✅ **already relayed** (`MAC_RELAY_P35_P4_ROUND.md`); never claimed from here;
4b. ⚠️ **update `cef-native/CLAUDE.md`'s overlay parity line in the landing commit** — it says
    Windows and macOS have **14 each**, and this phase makes Windows **15**;
5. preflight + `-NegativeControl`, and the 4 → 5 regression boundary;
6. what stays manual, said plainly.

# 9. Carried, not this phase

- 📌 `TICKET_token_outputs_destroyed_by_dust_paths` — 👤 **owner decision 2026-09-01: stays in Phase 8
  (money-path correctness). Do NOT interrupt Phase 4 for it.** ⚠️ The previous session flagged this
  twice as near-urgent on the strength of its *mechanism* (an automatic daily task that destroys
  1-sat outputs with no user action) — that framing **overstated it**, and the ticket's own severity
  section is the correction: *"the population today is probably near zero, because we ship no ordinal
  support"*. ⭐ **The real deadline is an event, not a date: it must close before beta.4's 1Sat
  Ordinals sprint begins**, because that is when the population stops being zero. Phase 8 clears that
  comfortably. ⚠️ One question in the ticket is still **unverified** and decides whether it should
  move up — *does an ordinary incoming 1-sat payment become a tracked default-basket row without a
  recovery scan?* ~20 minutes of code reading; answer it at the **start of Phase 8**, not now.
- 🔴 R-GOLD, R-COUNT and the unobserved `payment.auto_approved` audit line — one real payment
  (teragun) closes all three. Needs the owner.
- 🎫 New beta.4 tickets from 3.5: multi-window session restore loses all but the last window · menu
  Exit closes the primary · pin/mute model changes.
- ⛔ `G11` stays at **60** and cannot be lowered — reason in `HARNESS.md` §4.

# 10. Working style

⭐ Walk the owner through anything they must click: what to do, what they should see, **and what a
wrong result would mean.**
⭐⭐ **Check your own work before sending them.** Every defect this sprint found late was found by a
two-minute owner test of something already reported as done — and 👤 the owner has said the pace feels
slow. The way to be fast here is to not ship the same fix three times.
