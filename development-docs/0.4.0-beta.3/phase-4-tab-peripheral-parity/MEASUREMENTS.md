# Phase 4 — measurements

> Format: 📏 MEASURED / 📖 CLAIM / 🎯 SUBJECT / 👤 OWNER. A row is not GREEN until its RED has been
> **seen**. Per `HARNESS.md` §8 a skipped check is SKIPPED and the run is INCOMPLETE, never PASS.

**Rig:** the Phase 3.5 rig, unchanged — `p35drive.py` (CDP) to drive, `winprobe.ps1 -WatchSeconds`
to sample continuously. ⛔ The instrument was **not** edited for this phase (working rule #6):
`winprobe.ps1`'s class filter is `^(HodosBrowserWndClass|CEF.*OverlayWindow)$`, which already matches
`CEFTabMenuOverlayWindow`. Checked before the first run rather than after a confusing result.

---

## M1 — 📏 MEASURED: `P4-A1`, the menu exists and its x tracks the clicked tab

RED is **by construction** — there was no tab context menu on unmodified code (kickoff §0), so there
is nothing to disable.

Four right-clicks, four tabs, one `winprobe` watch (2026-09-01 13:53):

| Right-clicked tab | cursor `clientX` | overlay rect |
|---|---|---|
| index 0 | 138 | `143,26 240x209` |
| index 1 | 338 | `343,26 240x209` |
| index 2 | 538 | `543,26 240x209` |
| index 3 | 738 | `743,26 240x209` |

⭐ The window is at `0,0` and the shell has a 5 px resize-border inset, so every row is exactly
`headerRect.left + clientX`. **The x moves with the tab**; a fixed icon offset would have produced
four identical rects, which is the SUBJECT the row asks for. Size is `240x209` at 96 dpi = exactly
`kTabMenuWidthDip × kTabMenuHeightDip`.

📏 The C++ log agrees on which tab: `📑 Tab context menu open on tab 1 (window 0, index 0 of 4)` …
`tab 4 (window 0, index 3 of 4)`.

---

## M2 — 🚨 📏 MEASURED: `P4-A2` RED **observed**, not argued

⛔ The contract requires this to be seen to fail. A temporary injection replaced the remembered
target with `GetActiveTab()`, the tree was **rebuilt**, and the experiment re-run. Reverted after.

**Experiment.** 3 tabs — `newtab`, `example.com/RED-first`, `example.org/RED-last`. The **active**
tab is index 2 (`RED-last`). Right-click index 1 (`RED-first`, a **background** tab) → *Duplicate*.

| | Log | CDP targets after |
|---|---|---|
| 🟢 GREEN (shipped code) | `menu open on tab 2` → `action 'duplicate' on tab 2` → `new tab 5 inserted after 2` | a second `example.com/G-a` appears |
| 🔴 RED (`GetActiveTab()`) | `menu open on tab 2` → **`action 'duplicate' on tab 3`** → `new tab 4 inserted after 3` | a second **`example.org/RED-last`** appears, none for `RED-first` |

⭐ The menu opened on the right tab in **both** arms. Only the action moved. That is the whole point
of holding `s_tabmenu_target_tab_id` from open to click instead of re-deriving it — and it is
invisible on the happy path, because the right-clicked tab is usually also the active one.

---

## M3 — ⭐ 📏 MEASURED: `P4-A3` RED **observed**, and it is why A2 alone is not enough

Second injection, second rebuild: the action handler returns immediately **after** logging, so
nothing is executed.

```
[14:23:29] 📑 Tab context menu open on tab 2 (window 0, index 1 of 3)
[14:23:32] 📑 Tab menu action 'duplicate' on tab 2 (window 0)      <-- A2's assertion PASSES
```
📏 CDP target count before **3**, after **3**. No `inserted after` line. **Nothing happened at all.**

⇒ A2 is satisfied by a complete no-op. A3 is the row that catches it, and the pair only means
something because both REDs were built and run.

### 🟢 All six actions, on the shipped binary, each against a **background** tab

| Action | Log | Observable effect |
|---|---|---|
| `duplicate` | `'duplicate' on tab 2` → `new tab 5 inserted after 2` | 2× `example.com/G-a` CDP targets |
| `new_tab_right` | `'new_tab_right' on tab 1` → `new tab 6 inserted after 1` | tab count 5 → 6, new tab at index 1 |
| `bookmark` | `'bookmark' on tab 2: {"id":8,"success":true}` | 📏 `bookmarks.db` row 8 = `https://example.com/G-a` — the **right-clicked** tab, while the active tab was `G-c` |
| `reload` | `'reload' on tab 5` | — |
| `close_others` | `'close_others' on tab 7` → `closed 2 tab(s) in window 1` | window B 3 → 1 tabs |
| `close_right` | `'close_right' on tab 6` → `closed 4 tab(s) in window 0` | window A 6 → 2 tabs |

---

## M4 — 📏 MEASURED: `P4-A4`, the secondary window, and Phase 3.5's fix is inherited

Window B created with `p35drive.py key <hdr> 78 --ctrl`. ⚠️ Two headers share the URL
`http://127.0.0.1:5137/`; B was identified by **target-id diff across Ctrl+N**, never by list order.

Right-click a **background** tab in **B** → *Duplicate*. `winprobe` watch, 3 samples:

```
13:56:30  tabmenu Vis=False      B(0x1130B36)@Z11   A(0x4607E6)@Z12
13:56:45  tabmenu Vis=True       B@Z11              A@Z12     <- menu OPEN
13:56:48  tabmenu Vis=False      B@Z11              A@Z12     <- menu DISMISSED
```

| Claim | Evidence |
|---|---|
| acts in B | `📑 Tab menu action 'duplicate' on tab 6 (window 1)` → `new tab 8 inserted after 6` |
| leaves A alone | A: 5 tabs before **and** after. B: 2 → 3 |
| positioned against **B** | overlay at `223,106` = B's origin `80,80` + border 5 + `clientX 138` / `clientY 21` — not against `g_hwnd` |
| ⭐ B never drops behind A | **A is at Z12 in all three samples.** Compare P3.5 K25, where a sample with the primary in front sat between open and close |

⭐ Per §7.2 of the session prompt, the **dismiss** sample is in the table, not just the open one.

⚠️ **Honest limit.** The `Fg` (focus) column is empty in every sample because the run is CDP-driven
and never touches the desktop input queue. The **Z-order** half of P3.5's defect is measured; the
**activation** half is not reproducible from here. → owner item O1.

---

## M5 — 📏 MEASURED: `P4-A5`, a bulk close never empties a window

| Case | Result |
|---|---|
| right-click the **last** tab (3 tabs) | *Close tabs to the right* **disabled** (`opacity 0.4`, `cursor default`); *Close other tabs* enabled |
| click the disabled item anyway | 📏 **zero** new `Tab menu action` log lines — `onClick` is `undefined`, no IPC is sent |
| *Close others* on that last tab | `closed 2 tab(s) in window 1`, window left with **1** tab, never 0 |
| right-click the **only** tab in a window | **both** bulk items disabled; clicking both leaves `tabs=1` |

⇒ Neither item can close the right-clicked tab, so the window always retains ≥ 1. The
`!windowHasTabs → CreateTabInWindow` guard is therefore unreached by design; it is kept as an
enforced invariant rather than an argued one, mirroring the guard on the `tab_close` path.

---

## M6 — 🚨 📏 MEASURED then FIXED AT THE CAUSE: the tab strip lagged a bulk close by **17.5 s**

Found by checking my own work — the tab count read wrong right after a `close_others`, and the first
instinct ("the read was too early") was the wrong one.

**Measured, first build:**
```
click at 13:59:24.361
t=0.16s   tabs=4
t=17.67s  tabs=1
```
📏 The C++ side was never slow: the three tabs closed between `13:59:24.589` and `.625` — **36 ms**.
What took 17.5 s was the *frontend* finding out. Log:

```
13:59:24.589  📑 Tab list sent to window 1 (590 bytes)              <- 4 tabs, sent BEFORE they closed
13:59:42.048  📑 Tab list sent to window 1 (on-demand, 168 bytes)   <- React's own periodic refresh
```

**Cause.** `TabManager::OnTabBrowserClosed` — the point at which a tab actually stops existing — was
the one lifecycle event that never notified the frontend. `CreateTab`, `ReorderTabs` and
`MoveTabToWindow` all notify at their completion point; close notified from the **caller**, before
`CloseBrowser` had done anything, so the list it sent still contained every tab.

⭐ **Why nobody had hit it:** `useTabManager.closeTab` removes the clicked tab from React state
optimistically, so the single-close path never depended on the push. "Close other tabs" closes N at
once and has no optimistic path. The behaviour is new; the latent defect is not.

⛔ **Not fixed by posting a delayed notify from the caller.** That corrects after the wrong thing has
already been shown, and the user sees the correction — the exact shape P3.5 K25 rejected. Fixed by
notifying from `OnTabBrowserClosed`, which fixes every close path at once.

**Measured, after the fix:** `t=0.16s tabs=1` — settled inside the polling resolution.
📏 `close_right` re-measured the same way: `t=0.18s tabs=2`. **17.67 s → 0.16 s.**

---

## M7 — 🚨 📏 MEASURED then FIXED: the **first** right-click of a session showed both bulk items greyed out

⭐ This survived five earlier green runs, because every one of them opened the menu long after the
overlay had loaded. It reproduces only on the **first** open after a fresh browser start.

📏 Fresh start, 4 tabs open, first right-click:
`["Reload:1","Duplicate:1","New tab to the right:1","Bookmark tab:1","Close other tabs:0.4","Close tabs to the right:0.4"]`

**Cause.** On the first open the overlay's browser is created by the very IPC that shows it, so the
C++ context push runs before the React component exists; the push is `if (window.setTabMenuContext)`
and silently no-ops.

**The house fix is a retry ladder** — `bookmarkspanel` and `siteinfopanel` both re-inject at 300 ms
and 600 ms to outrun Vite's module waterfall. ⛔ Not taken: that is a guess at a duration, and it is
the same "correct it afterwards" shape as M6. Instead the overlay **pulls**: its `useEffect` sends
`tab_context_menu_request_context` when it registers the receiver, and C++ answers the browser that
asked. The push is kept for the 2nd..Nth open, where it is synchronous and correct.

📏 After the fix, first right-click of a fresh session, all six at `opacity 1`.
⇒ RED and GREEN are the same experiment on two builds.

---

## M8 — 📏 MEASURED: `P4-A6` (**R-COUNT**) — mechanism GREEN, values 🟡 **PARTIAL**

`close_right` closing 4 tabs at `14:29:16` produced, in the **Rust** log, exactly **4**
`POST /wallet/session/close` calls, all 200, with **4 distinct** `browser_id`s (12, 16, 13, 14) —
matching `closed 4 tab(s)`. The surviving tabs' browser ids are absent.

⭐ The discriminating observation is **4, not 1**: had the bulk path cleared once (e.g. for the active
browser) there would be a single POST.

⛔ **This is not the whole row.** The counters were at **zero** — no payment was made — so what is
measured is that the clearing fires once per closed tab with the right ids, **not** that a non-zero
counter was reset. The value half needs a real spend and is owner-owed (session prompt §9, together
with R-GOLD). Reported as 🟡 PARTIAL, not GREEN.

---

## M9 — 📏 MEASURED: `P4-B1`, mic/camera, three states, three **opposite** outcomes

⛔ **My first attempt was a vacuous test and said so loudly.** It set the stored state with
`site_permissions_set` sent from the **page** (`example.com`) — which the IPC allowlist correctly
**denied** (`🛡️ IPC DENIED: 'site_permissions_set' from external origin 'example.com'`). The state
never changed, so both "allow" and "block" arms were really measuring *Ask*. Re-run with the state
set from an internal origin (`5137/site-info`), and a **fresh tab per state** so no earlier request
was still outstanding.

🎯 SUBJECT: the CEF callback result from the C++ log **and** the page-observable outcome — not
"did a prompt appear".

| Stored | C++ (`OnRequestMediaAccessPermission`) | Page `getUserMedia({audio,video})` |
|---|---|---|
| `allow` | `🔓 Media permission auto-allowed (stored Allow)` → `callback->Continue()` | `RESOLVED tracks=2` |
| `block` | `🔒 Media permission auto-denied (stored Block)` → `callback->Cancel()` | `REJECTED NotAllowedError` |
| `ask` | no auto- line; `🔔 Creating notification overlay (type: permission_request, domain: example.com)` | `pending` (waiting on the user) |

⭐ Each state is the others' control — three states, three observations, each the opposite outcome,
which is what the row's RED column asks for. `tracks=2` also confirms the box has a working mic and
camera, so the Allow arm is not a hardware-degenerate pass.

---

## M10 — 🚨 📏 MEASURED: `G11` caught a real +1, and the **code** was changed, not the baseline

First full preflight after the feature: **`G11` FAIL, 61 violations over baseline 60.** The new hit
was `GetTabMenuBrowser()`, written like the 18 accessors beside it:
`WindowManager::GetInstance().GetPrimaryWindow()->tabmenu_browser`.

⛔ The baseline was **not** raised. Two reasons, and the second is the deciding one:

1. Working rule #6 — the instrument is not edited by the change it measures, and a raise is its own
   commit with a reason in `HARNESS.md` §4.
2. The gate's own comment says the 18 accessors are the residual it is counting down and that it
   "must not be allowed to grow while it waits". A 19th is exactly the growth it exists to stop.

**Fixed in the code.** The tab-menu overlay is **one browser per process**, so filing it under "the
primary window" and reading it back asks a per-window question about a per-process thing. It is now a
`SimpleHandler` static, assigned in `OnAfterCreated` and cleared in `OnBeforeClose`. ⭐ Not a regex
dodge: which window the menu acts in is decided by the **target tab** and by the overlay's Win32
owner, neither of which changed.

📏 `preflight.ps1 -Full` → **PASS**, all 14 checks ran.
📏 `preflight.ps1 -NegativeControl` → **PASS**, every gate seen to fail on its probe. ⭐ G11's probe
detects at `61 > baseline 60` — the identical count my first version produced, which independently
confirms the FAIL was a genuine +1 rather than a flake.
📏 Re-smoked after the refactor: first-open context correct, duplicate lands on the right-clicked
background tab.

---

## M11 — ⬜ NOT MEASURED FROM HERE — owner items

| # | What | Why it cannot be run from this session |
|---|---|---|
| **O1** | A **real mouse** right-click on a tab, and a real click on a menu item | ⛔ `SendInput` mouse clicks are dropped in this agent environment. Every run above drives the DOM over CDP, which reaches React's handler but **never exercises `TabMenuOverlayWndProc`'s `WM_LBUTTONDOWN` → `SendMouseClickEvent` path**. The forwarding code is a copy of `MenuOverlayWndProc`, but it has not been executed. |
| **O2** | Click-outside dismissal (`TabMenuMouseHookProc`, the `WH_MOUSE_LL` path) | same reason. This is also the R-CLOSE arm that has been owed at every prior boundary |
| **O3** | Focus/activation half of `P4-A4` | M4 measures Z-order only; `Fg` is empty in a CDP-driven run |
| **O4** | `P4-A6` value half + **R-GOLD** + the `payment.auto_approved` audit line | one real payment closes all three |
| **O5** | 🍎 macOS | ⛔ not runnable from the Windows box. Relayed in `MAC_RELAY_P35_P4_ROUND.md` (M3/M4); never claimed here |

---

## M12 — 📖 Observations, deliberately NOT fixed (working rule #3)

1. **`BrowserWindow.h`'s "15 total" comment** on the browser-ref block was already wrong (18 refs
   before this phase, 19 after). Reported, not edited — it is not what this change came for.
2. **The bookmarks overlay does not live-refresh** when a bookmark is added from the tab menu; it
   shows the new row on reopen. Same call as the bookmarks panel's own add, so this is pre-existing
   overlay refresh behaviour, not something this phase introduced. In normal use the panel is opened
   *after* bookmarking, which loads fresh.

---

# Addendum — **Mute tab** added 2026-09-01, after the phase had landed

👤 **Owner question:** *"the initial prompt was a user asking for 'mute tab' — why is it not in there?"*

⛔ **Answer: my grouping error at kickoff, not a considered exclusion.** I presented pin + mute-tab +
mute-site to the owner as one group called "needs model changes", the owner deferred the *group*, and
`TICKET_tab_pin_and_mute_need_model_changes.md` inherited my grouping. Two of the three genuinely
need model work; **mute tab did not**, and the ticket's own "Proposed fix" section says so — *"The
floor — mute tab only… self-contained and does not touch session restore at all."*

## M13 — 📏 MEASURED before writing any code

| Claim | Evidence |
|---|---|
| CEF exposes both halves | `cef_browser.h:1003` `SetAudioMuted(bool)`, `:1010` `IsAudioMuted()`, UI-thread only |
| We already use it | `TabManager.cpp:204` + `TabManager_mac.mm:208` mute on the close path |
| Pin **is** genuinely blocked | `Tab` has no `pinned`; needs pinned-first ordering in `ReorderTabs` + `session.json`, which carries an open defect |
| Mute **site** is genuinely blocked | needs per-domain storage — extend `SitePermissionStore`, ⛔ never a parallel store |

⇒ mute-tab was separable. Built it; pin and mute-site stay ticketed.

## M14 — 🚨 📏 MEASURED: my first design was WRONG, and the measurement is what caught it

I told the owner mute needed **no `Tab::muted` field** because *"`IsAudioMuted()` IS the state"*, and
reasoned that navigation is `LoadURL` on the same browser so a mute would survive it. 📖 That was a
**claim from code reading**, and it is false.

```
mute tab 2                    -> menu open on tab 2 (…, muted=true)
navigate SAME-origin          -> menu open on tab 2 (…, muted=false)   <== mute GONE
navigate CROSS-origin         -> muted=false                            <== also gone
```
📏 Same CDP target id across the navigation and **no `OnBeforeClose`** — so the `CefBrowser` was *not*
recreated, yet `IsAudioMuted()` reads false. ⇒ **the CEF mute is per-document; the user's intent is
per-tab.** Without a fix the feature breaks the first time the user clicks a link in a muted tab —
silently, and exactly when they care.

⭐ **The ticket was right that a model field is needed — for a reason neither it nor I had stated.**
Its stated reason was *persistence across restart*; the real one is *re-application across
navigation*. Two different problems that happen to want the same field.

**Fixed:** `Tab::muted` holds the intent, `OnLoadingStateChange` re-applies `SetAudioMuted(true)` on
load completion when intent and mechanism disagree. The tab-list JSON and the menu label both report
**intent**; the toggle log prints `intent=` and `actual=` side by side, which is what makes a future
divergence visible instead of silent.

📏 **Re-measured after the fix** — glyph state across a full sequence:
```
after mute:             [false, true, false]
after SAME-origin nav:  [false, true, false]   🔇 Re-applied mute to tab 2 after navigation
after CROSS-origin nav: [false, true, false]   🔇 Re-applied mute to tab 2 after navigation
after unmute:           [false, false, false]  intent=false actual=false
```

## M15 — 📏 MEASURED: `P4-A7` — the toggle acts on the right-clicked tab

3 tabs; **active is index 2**, right-click **index 1** (background).

| | Evidence |
|---|---|
| 🟢 GREEN | `menu open on tab 2 (window 0, index 1 of 3, muted=false)` → `mute_toggle on tab 2: intent=true actual=true` |
| Label round-trip | reopening the menu on that tab reads **"Unmute tab"**; the log shows `muted=true` |
| Reverses | `mute_toggle on tab 2: intent=false actual=false`, glyph clears |
| 🔴 RED | shared with `P4-A2` — the `GetActiveTab()` substitution was built and run, and every action including this one resolves through the same remembered `s_tabmenu_target_tab_id` |

🎯 SUBJECT: the `Tab::id` in the C++ log **and** `actual=` read back from CEF — not the label.

## M16 — 🚨 📏 MEASURED: `P4-A8` RED **observed**, and the first RED attempt was itself instructive

The indicator exists because mute has no other visible state: the menu label is only readable while
the menu is open, so without a glyph you mute a tab and can never tell which tabs are silenced.

**RED attempt 1 — injected `muted = false` into ONE builder (the push).** Result: the menu label went
stale correctly (`muted=false` logged while the tab was genuinely muted), **but the glyph still
appeared.** ⭐ Because the *other* builder — the on-demand `get_tab_list` arm — was untouched and
healed it on the frontend's next poll.

⇒ 📏 **empirical proof that the two-builder trap is real**: had the real implementation added `muted`
to only one builder, the indicator would have been inconsistent rather than absent, which is far
harder to notice. Both builders now carry the field and both carry the warning comment.

**RED attempt 2 — injected both.** Clean:
```
📑 Tab menu mute_toggle on tab 2: false -> true      <- genuinely muted
muted glyphs: [false, false, false]                  <- strip shows nothing, on any tab
```
🟢 Reverted, rebuilt, re-confirmed: `[false, true, false]`, held through a full poll cycle.

## M17 — ⛔ What mute deliberately does NOT do

| | |
|---|---|
| **No "tab is making noise" speaker** | 📏 this CEF build exposes **no `OnAudioStateChanged`** (`grep` over `cef-binaries/include`). Chrome's speaker icon needs an audio-state signal we do not have. Showing a *muted* glyph only is the honest subset — it is user-driven, so a push on toggle keeps it accurate |
| **No persistence across restart** | session-lived by design; `session.json` carries an open defect and is out of scope (contract §7) |
| **Not per-domain** | "mute site" still needs storage and stays ticketed |
| ⬜ **Not verified: does it actually silence audio?** | Every check above reads `IsAudioMuted()` and the UI. ⛔ **Nobody has listened to a noisy page with this on.** That is owner item **O6** — the `SetAudioMuted` call is CEF's own and is already used on the close path, but "the flag is set" is not "the sound stopped" |
