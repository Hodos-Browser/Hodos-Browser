# Phase 4 — tab context menu & peripheral parity · PHASE CONTRACT

**Workstream:** WS3 · **Opened:** 2026-09-01 · **Status:** 🟢 **LANDED 2026-09-01** — 7 rows GREEN, `P4-A6` 🟡 PARTIAL, `P4-B2` relayed
**Owner:** Matthew · **Platform:** Windows (macOS relayed, not claimed)
**Standard:** `../HARNESS.md`. Inherits the beta.3 harness in full.

---

## 0. What the kickoff changed, before anything was built

⛔ **Two claims in `SPRINT_PLAN.md` §WS3 did not survive contact with the tree.** Recorded here in the
same commit as the contract, per `HARNESS.md` §1.

| Plan said | Measured 2026-09-01 |
|---|---|
| *"Tab context-menu **parity**"* | ⛔ **There is no tab context menu at all.** 📏 No `onContextMenu` handler anywhere in the header UI, and no tab-strip command IDs in C++. This is *build one*, not *extend one* |
| *"uses the existing custom `MENU_ID_USER_FIRST` context-menu machinery"* | ⛔ **Wrong machinery.** That is CEF's **page** context menu (`OnBeforeContextMenu`), which fires on web content inside *tab* browsers. A right-click on a tab happens in the **header** browser, where 📏 the existing code deliberately offers only edit items for non-tab roles |

⇒ the tab menu is a **dropdown**, and `CLAUDE.md`'s UI rule is explicit: dropdowns are overlays, not
additions to `MainBrowserView.tsx`. It becomes **overlay #15** — `TabContextMenuOverlayRoot.tsx` plus
a `Create/Show/Hide` trio in `simple_app.cpp`, following the menu/profile pattern exactly.

⭐ **A new overlay built on that pattern inherits Phase 3.5's ownership fix for free** — ownership
follows the requesting window on show and returns to the primary on hide. ⚠️ It only inherits it if
it uses `Show*Overlay(offset, targetWin)`; an overlay that positions itself against `g_hwnd` would
reintroduce the whole of 3.5.

## 1. Scope — six items, agreed with the owner 2026-09-01

**IN — the six that reuse what already exists:**

| Item | Reuse |
|---|---|
| Reload | `navigate_reload` IPC |
| Bookmark | `bookmark_add` IPC |
| Duplicate | `tab_create` with the same URL |
| New tab to the right | `tab_create` + existing `TabManager::ReorderTabs`, needs an insert index |
| Close other tabs | loop `TabManager::CloseTab` |
| Close tabs to the right | loop `TabManager::CloseTab` |

**IN — peripheral:** mic/camera **verification** on Windows (`OnRequestMediaAccessPermission` already
exists and honours `SitePermissionStore`). 📖 *"Reported working"* is a claim, not a measurement.

**OUT — 👤 owner decision, deferred to a follow-up ticket:**
- ⛔ **Pin** — 📏 `Tab` (`include/core/Tab.h`) has **no `pinned` field**. Needs a model change,
  pinned-first ordering, and `session.json` persistence — which drags in the session-restore code
  that already carries a known defect (`P3.5-A4` / K17).
- ⛔ **Mute tab / mute site** — 📏 no `muted` field either. CEF's `SetAudioMuted` is available (already
  used on the shutdown path), but per-domain mute needs storage; extend `SitePermissionStore`, ⛔ do
  **not** create a parallel store.
- ⛔ **macOS** — unverifiable from the Windows box. Relayed, never claimed.

## 2. Goal

Right-clicking a tab offers the six actions above, each acting on **the tab that was right-clicked**,
in **the window it belongs to**.

## 3. Done means

- [ ] Right-click on a tab opens a menu anchored to that tab.
- [ ] Each of the six actions acts on the **right-clicked** tab, including when it is not the active one.
- [ ] The actions behave correctly in a **secondary** window and disturb nothing in the primary.
- [ ] Closing the last tab still auto-creates an NTP rather than leaving an empty window.
- [ ] Mic/camera allow / block / ask are **measured** on Windows against a live `getUserMedia` page.

## 4. Invariants preserved

| ID | Why this phase could break it |
|---|---|
| **R-GOLD** | 🚨 **The sharpest risk here.** Every action resolves a *specific* tab, and `Tab::id` ≠ `CefBrowser::GetIdentifier()` — translate via `TabManager::GetTabIdForBrowserIdentifier`. An off-by-one identity here puts the gold pill, or a close, on the wrong tab |
| **R-COUNT** | ⚠️ *Close others* / *close to the right* close **many** tabs at once. Per-session counters reset on tab close, so a bulk close fires that path N times. Untested at N>1 |
| **R-CLOSE** | A 15th overlay adds another close path. It must use the existing `Show*/Hide*` pattern or it will not inherit Phase 3.5's ownership handling |
| **R-INTEXT / R-PERIM** | Untouched. Listed because `simple_handler.cpp` is edited |
| **R-UPDATE** | Untouched |

## 5. Evidence table

⛔ No empty RED or SUBJECT cells. A green result is reported with its red half or not at all.
🎯 **Standing SUBJECT:** the tab acted on is read from **`TabManager`**, never from the UI's own idea
of which tab is selected.

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P4-A1` | Right-click on a tab opens the menu, anchored to that tab | ✅ **Already RED on unmodified code** — 📏 there is no tab context menu at all today | The overlay HWND exists **and** its x-position tracks the clicked tab, not a fixed offset | T3 | 🟢 GREEN (M1) — x = 143/343/543/743 for four tabs; RED by construction |
| `P4-A2` | Each action affects the **right-clicked** tab | 🔴 Right-click a **background** tab and act. Wire it to `GetActiveTab()` instead → the action lands on the wrong tab. ⛔ Must be observed, not argued from the diff | The `Tab::id` acted on, from the C++ log — **not** which tab looks selected | T3 | 🟢 GREEN (M2) — RED built + run: `GetActiveTab()` duplicated `RED-last` while the menu was opened on `RED-first` |
| `P4-A3` | ⭐ `A2`'s partner: the action actually *works* on that tab | Skip the action entirely → `A2` passes (nothing lands on the wrong tab) while nothing happens at all | The observable effect: tab count, tab URL, bookmark row | T3 | 🟢 GREEN (M3) — RED built + run: A2 passed on a total no-op |
| `P4-A4` | Right-clicking a tab in a **secondary** window acts there and leaves the primary untouched | Resolve the window via `g_hwnd` → the action lands in the primary. ⭐ This is Phase 3.5's defect class; the row exists because a new overlay is the easiest place to reintroduce it | `winprobe.ps1`: the acting window's tab set **and** its Z-order | T3 | 🟢 GREEN (M4) — acts in window 1, A untouched at 5 tabs, B stayed at Z11 in all 3 samples. ⚠️ Z-order only; focus half → O3 |
| `P4-A5` | *Close others* / *close to the right* never leave an empty window | Close-others on a window's only tab, and on a window where the right-clicked tab **is** the last one → an NTP must appear | Tab count after, per window | T3 | 🟢 GREEN (M5) — both bulk items disabled + inert on a 1-tab window; close-others on the last tab left 1, never 0 |
| `P4-A6` | ⚠️ Bulk close does not corrupt per-session counters (**R-COUNT**) | Spend, then *close others* → counters for the closed tabs reset, and the surviving tab's do not | `PermissionService.session_counters`, not a UI total | T2 | 🟡 PARTIAL (M8) — 4 tabs closed ⇒ 4 `session/close` POSTs, 4 distinct browser_ids. Counters were ZERO; the value half needs a real spend → O4 |
| `P4-B1` | Mic/camera honour the stored Allow / Block / Ask | Flip the stored state per site and observe the **opposite** outcome each way. ⛔ Three states, three observations | The **CEF callback result** and the stored row, not just whether a prompt appeared | T3 | 🟢 GREEN (M9) — allow→`Continue()`/`RESOLVED tracks=2`; block→`Cancel()`/`NotAllowedError`; ask→prompt/pending |
| `P4-B2` | 🍎 macOS mic/camera | ⛔ **NOT RUN — cannot be run from this box.** ✅ Relayed 2026-09-01 (`MAC_RELAY_P35_P4_ROUND.md` M4). ⚠️ macOS has an OS layer we do not — **TCC**: a site allowed in `SitePermissionStore` still fails without bundle entitlements, a failure mode with no Windows analogue. ⛔ Never marked passed on the strength of the Windows run | — | — | ⬜ relayed, not claimed |

## 6. Blast radius

| Risk | Mitigation |
|---|---|
| 🚨 **Wrong-tab actions** | `A2` + `A3` are two-sided. `Tab::id` ≠ `CefBrowser::GetIdentifier()` is called out in §4 |
| **Bulk close** | `A5` + `A6`. Closing N tabs is the only genuinely new *behaviour* in this phase |
| **A 15th overlay** | Follow the `Show*/Hide*` pattern exactly, or Phase 3.5's fix does not apply. `A4` is the control |
| **Testing is again mostly T3** | Accepted. The CDP + `winprobe` rig from Phase 3.5 is reusable as-is |

## 6.1 🍎 What this phase owes macOS — relayed, not built

⚠️ **A 15th overlay breaks a parity line that is currently true.** `cef-native/CLAUDE.md` records
Windows and macOS at **14 overlays each**, and invariant #9 requires a macOS creation function in
`cef_browser_shell_mac.mm` for any new overlay. Windows will have 15.

✅ **Relayed 2026-09-01** (`MAC_RELAY_P35_P4_ROUND.md` M3). ⛔ **Not written from here** — a
cross-platform overlay authored on a Windows box and never executed is the failure mode this project
keeps paying for (same call as the Phase 3 round).

⭐ Nothing is broken meanwhile: Windows simply has a menu macOS does not. ⚠️ But the parity line in
`cef-native/CLAUDE.md` goes **stale the moment this phase lands** — update it in the landing commit
rather than leaving a doc asserting 14/14 when it is 15/14.

## 7. Out of scope

⛔ Pin · mute tab · mute site · macOS · anything touching `session.json` (it carries an open defect) ·
the CEF page context menu, which is a different menu and is not being changed.

## 8. Rollback

One commit. A new overlay plus new IPC handlers; no schema change, no installer change, no
persistence. `git revert` restores today's behaviour.

---

## 9. Found while building this phase — three defects, all fixed at the cause

Full detail in `MEASUREMENTS.md`. Recorded here because two of them were **in code this phase
touched but did not create**, and the third was caught by a gate rather than by a test.

| # | Defect | Fixed |
|---|---|---|
| **D1** | 📏 The tab strip lagged a bulk close by **17.5 s**. `TabManager::OnTabBrowserClosed` — where a tab actually stops existing — was the one lifecycle event that never notified the frontend; close notified from the *caller*, before `CloseBrowser` had done anything. Hidden until now by React's optimistic single-tab removal, which a bulk close does not have | Notify from `OnTabBrowserClosed`. ⛔ **Not** a delayed re-send from the caller — that corrects after the user has already seen the wrong thing (P3.5 K25). **17.67 s → 0.16 s** (M6) |
| **D2** | 📏 The **first** right-click of a session rendered both bulk-close items greyed out. The overlay's browser is created by the same IPC that shows it, so C++'s context push ran before React existed | The overlay **pulls** its context on mount (`tab_context_menu_request_context`). ⛔ Not the house 300/600 ms retry ladder — that is a guess at a duration (M7) |
| **D3** | 🚨 `G11` went **61 > baseline 60**: `GetTabMenuBrowser()` was written like the 18 accessors beside it, through `GetPrimaryWindow()` | ⛔ Baseline **not** raised. The overlay is one browser per process, so it is now a `SimpleHandler` static — the getter stopped asking a per-window question about a per-process thing (M10) |

⭐ D1 and D2 were both found by **checking my own work**, not by a failing row: D1 because a tab
count read wrong and the first explanation ("read too early") was wrong, D2 because five earlier
green runs had all opened the menu long after the overlay had loaded.

## 10. ⬜ What stays manual — said plainly

⛔ **Every result above was driven over CDP.** That reaches React's handlers exactly as a click does,
but it **never** exercises `TabMenuOverlayWndProc`'s `WM_LBUTTONDOWN → SendMouseClickEvent` path or
`TabMenuMouseHookProc`, because `SendInput` mouse clicks are dropped in the agent environment. The
forwarding code is a line-for-line copy of `MenuOverlayWndProc`, but **it has not been executed.**

Owner items, in the order they are worth doing — see `MEASUREMENTS.md` M11:

| # | Ask | A wrong result means |
|---|---|---|
| **O1** | Right-click a tab with the real mouse; click each of the six items | the OSR mouse forwarding in the new WndProc is wrong — nothing would happen on click |
| **O2** | With the menu open, click somewhere outside it | the `WH_MOUSE_LL` click-outside hook is not dismissing (R-CLOSE) |
| **O3** | Open the menu in a **second** window and watch whether the first window jumps forward | Phase 3.5's defect reintroduced on the activation axis (the Z-order axis is measured GREEN) |
| **O4** | One real payment, then *close other tabs* | closes `P4-A6`'s value half, **R-GOLD**, and the unobserved `payment.auto_approved` audit line together |
| **O5** | 🍎 macOS | relayed only — Windows now has **15** overlays, macOS has 14 |

---

## Sign-off

- [x] Every evidence row GREEN **and** its RED observed — ⚠️ except `P4-A6`, reported 🟡 **PARTIAL**
      (mechanism measured, counter values need a real spend), and `P4-B2`, relayed not claimed
- [x] `scripts/preflight.ps1` + `-NegativeControl` run, recorded below
- [x] `../REGRESSION_SET.md` run at the 4 → 5 boundary — recorded 🟡 **INCOMPLETE**
- [x] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight | **PASS** — all 14 checks ran; `G11` FAILed at 61/60 first and the **code** was fixed, not the baseline | 2026-09-01 | assistant |
| preflight -NegativeControl | **PASS** — every gate seen to fail on its probe (`G11` at `61 > 60`) | 2026-09-01 | assistant |
| regression set | 🟡 **INCOMPLETE** — R-INTEXT external half GREEN; R-COUNT partial; R-GOLD / R-CLOSE / R-UPDATE owner-owed, all pre-existing debt | 2026-09-01 | assistant |
