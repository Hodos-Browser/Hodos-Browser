# P0-b — Win10 overlay cluster: root-cause research (2026-07-08)

## IMPLEMENTATION STATUS (2026-07-08) — F1/F2/F3 landed, built exit 0, LOCAL/unpushed
- **F1 (dead buttons) — DONE.** Removed the fragile `IsOverlayEffectivelyVisible → Hide`
  branch from the bookmark/tab-list/site-info `*_panel_show` handlers (`simple_handler.cpp`).
  Button now only OPENS; closes via the click-outside mouse hook (+ the existing `<250ms`
  guard for click-while-open). A desynced panel re-Shows (recovers) instead of dead-hiding.
  **Windows only** — macOS blocks left as-is (NSPanel, unaffected); mac parity is an open Q.
- **F2 (blank first-open) — DONE.** Pre-create bookmark + tab-list overlays hidden at startup
  (`cef_browser_shell.cpp` deferred list, +4000/4500ms), matching the other dropdowns.
- **F3 (empty list) — DONE.** (a) `refreshBookmarkList` re-fetches on panel SHOW (needed now
  the panel is pre-created), wired C++→React; (b) `useBookmarks.refresh()` retries 3× on
  timeout; (c) `getAll` bridge timeout 5s→15s.
- **F4 (offset-0) — covered by F2 + existing diagnostics.** The offset is measured at CLICK
  time (button laid out), so 0 is a narrow edge case; F2's non-zero pre-create seed persists
  via the `if (offset>0)` guard, so the dangerous "off-screen dead panel" symptom can't occur.
  Raw offset is already logged (6805/6883/7001) to confirm on mom's next log. No code needed.
- **F5 (single-instance forward to dying process) — PENDING a design + adversarial review**
  before any code (owner's instruction; touches the DB-corruption single-instance safeguard).
- **Mechanism 6 (picker "shows once") — deferred** to the same-process picker refactor
  (`PROFILE_PICKER_SAME_PROCESS_PLAN.md`).

---



Three parallel read-only investigations (OSR render pipeline / global-init divergence /
bookmark-display path). **Conclusion: the Win10 cluster is NOT one root cause — it is a family
of slow-machine TIMING races, each in a specific overlay mechanism.** The backend is healthy
(saves, profile resolve, DB init all succeed on mom's machine). Everything below is the
overlay/interaction layer failing under a saturated UI thread on a slow old Win10 box.

Owner's hypothesis ("a global is set wrong because the picker path is taken/bypassed") **does not
literally hold** — see §B — but an adjacent cross-launch path (single-instance forward to a dying
old process) does explain the "dead after reopen" symptom.

---

## The six mechanisms (ranked by how much of the cluster they explain)

### 1. Dead buttons = "toggle-off on the already-visible branch" + a visibility-perception desync — HIGH
The broken panels gate their toolbar button on visibility and **hide** when they think they're
already open: bookmark `simple_handler.cpp:6775`, tab-list `6875`, site-info `6935`
(`IsOverlayEffectivelyVisible(...) → HideXxx`). The **working** panels (download `6717-6721`, menu)
**never** toggle-off on the button — they only Create/Show and close via the click-outside hook.
So whenever the window is `WS_VISIBLE` but not *perceived* as open, the next button press takes the
hide branch → nothing appears → the button looks dead; the press after re-shows. Classic
"works once, then dead." Desync sources on slow Win10: DWM-cloaking of an owned layered popup
(`IsWindowVisible`==TRUE while off-screen — the C1 `DWMWA_CLOAKED` check targets this), off-screen
placement from offset-0 (§4), and `SetForegroundWindow` failure (§below).

**Why C1/C2 "helped but didn't fix it":** the C2 `SWP_FRAMECHANGED`-after-`WS_EX_TRANSPARENT`
flush is **dead code for these panels** — bookmark/site-info/tab-list are never created with
`WS_EX_TRANSPARENT` (only settings/wallet/backup get it via `overlay_input`
`simple_handler.cpp:5229-5249`), so the `if (exStyle & WS_EX_TRANSPARENT)` guard in
`my_overlay_render_handler.cpp:231` is never true for them. C2 was inert here; the real cause is
the toggle-desync above.

### 2. Blank/empty panel on first open = not pre-created before first OSR paint — HIGH
download/menu/cookie/profile/**site-info** are pre-created hidden at startup (staggered 1000–3500 ms,
`cef_browser_shell.cpp:5138-5147`, `showImmediately=false`) so React + the first frame are ready
before the user opens them. **Bookmark and tab-list are NOT pre-created** — first click runs
`Create(..., showImmediately=true)` which shows the `WS_POPUP|WS_VISIBLE` window **synchronously**
while `CreateBrowser` is async. Until the first `OnPaint`/`UpdateLayeredWindow` arrives the layered
window shows the constructor's zero/transparent DIB (`my_overlay_render_handler.cpp:65`). No
wait/retry for the first frame. On slow Win10 the React first-paint is hundreds of ms→seconds later
→ blank panel.

### 3. Bookmark list stays empty = the 5 s one-shot `getAll` timeout drops the slow response, no retry — HIGH
`initWindowBridge.ts:973-988`: `bookmarks.getAll` registers a **one-shot**
`window.onBookmarkGetAllResponse`, arms a **5000 ms** timeout that `delete`s the callback, then
sends `bookmark_get_all`. On slow Win10 the browser UI thread is saturated by the overlay-creation
storm while it also runs `BookmarkManager::GetAllBookmarks()` **synchronously** (cold SQLite open +
COUNT + N prepared-statement + `json::parse`, `BookmarkManager.cpp:593-655`). If the round trip
exceeds 5 s the callback is deleted; the late response hits
`if (window.onBookmarkGetAllResponse)` (`simple_render_process_handler.cpp:2262`) and is **silently
dropped — no retry anywhere**. `useBookmarks.refresh()` runs once on mount
(`BookmarksOverlayRoot.tsx:63-67`) and the `catch` only sets an (unshown) error → list stays `[]` →
"No bookmarks yet". Same path explains "star turns gold but no row" (toggleStar → add → refresh
times out identically). The B1 300/600 ms deferred inject (`aa5188a`) only re-runs
`setBookmarkContext` (the star context) — it NEVER re-fetches the list.

### 4. Off-screen placement = icon offset stuck at 0 — MEDIUM (matches owner's "global" instinct)
`g_bookmarks_icon_left_offset` / `g_siteinfo_icon_left_offset` / `g_tablist_icon_left_offset`
(`cef_browser_shell.cpp:155-159`) init to **0** and are written from the React `*_panel_show` IPC
**only when > 0**: `if (iconLeftOffset > 0) g_..._offset = iconLeftOffset;`
(`simple_app.cpp:2629`, `2747`, etc.). If React sends 0 (`getBoundingClientRect().left` measured
before the header has laid out — likely on slow Win10 right after reopen), the offset stays 0 →
panel positioned at the window's far-left edge → "button does nothing visible." A runtime-timing
failure, NOT a launch-path branch — which is why the owner perceived it as a "global not set."

### 5. Reopen forwards to a DYING old instance — MEDIUM (the real cross-launch effect)
`cef_browser_shell.cpp:4545-4577`: if the prior `--profile=Default` process hasn't fully exited when
the user reopens (slow Win10 teardown), `TryAcquireInstance` fails, the new launch **forwards and
exits (return 0)**, and the surviving old (mid-teardown) window is what the user interacts with →
degraded/dead. Unifies "picker only shows once" (old instance reused) + "dead buttons." The
profile-resolution log prints BEFORE this check, so mom's log alone can't tell a forwarded launch
from a fresh one.

### 6. Picker "shows once" = the two-process `--profile` relaunch — (separate, deferred)
Confirmed §B: the picker is a throwaway process that spawns `--profile=X` then exits; taskbar/relaunch
carries `--profile=`, which correctly bypasses the picker. The **same-process picker refactor**
(`PROFILE_PICKER_SAME_PROCESS_PLAN.md`) is the real fix; out of scope here.

---

## §B — Why the literal "picker sets a global wrong" hypothesis fails
Overlay code only ever runs in `!g_picker_mode`. The picker process creates NO overlays and exits
immediately. The **real** browser — whether spawned by the picker OR relaunched from the taskbar —
runs the identical `--profile=X`, `g_picker_mode=false` path. Each launch is a **fresh process**, so
all overlay globals reset to their initializers; a cross-launch "stale global" is impossible unless
the old process survives — which is exactly §5. The only `argProfile`-dependent startup differences
are the AUMID (taskbar grouping) and the DevTools port — neither touches overlays.

---

## Proposed fixes (design only — not yet implemented)

| # | Fix | Addresses | Risk | Notes |
|---|-----|-----------|------|-------|
| F1 | **Stop toggling-off on the button** for bookmark/site-info/tab-list — button only Create/Show; close via the click-outside hook (match download/menu, the proven-working panels) | §1 dead buttons | LOW | Structural "make broken match working." Minor UX change (button opens; click-away closes). Highest-impact single fix. |
| F2 | **Pre-create bookmark + tab-list overlays at startup** (add to the staggered `5138-5147` list, `showImmediately=false`) | §2 blank first-open, and warms §3 | LOW | Requires a **refresh-on-show** trigger so a pre-created list isn't stale (see F3). |
| F3 | **Make the bookmark list fetch robust**: (a) refresh the list on panel SHOW (not just mount), (b) retry `getAll` 2–3× on timeout/empty instead of dropping, (c) raise the 5 s timeout for this call | §3 empty list | LOW-MED | (a)+(b) are the real fix; (c) is a cheap backstop. Consider moving `GetAllBookmarks` off the UI thread later (bigger). |
| F4 | **Don't accept offset 0 silently** — React measures the icon offset after layout (rAF / header-ready) and re-sends; or C++ falls back to a computed position when 0 | §4 off-screen | LOW | Small targeted change; add a log of the raw offset first (diagnostic). |
| F5 | **Robust single-instance handoff** — on reopen, wait briefly for the old `--profile` process to exit before forwarding, or make teardown faster | §5 dying-instance | MED | Riskier (touches instance gating). Consider deferring; the same-process picker refactor (§6) partly subsumes it. |
| DIAG | Log raw `iconLeftOffset` in each `*_panel_show` handler + whether `TryAcquireInstance` forwarded, and log `UpdateLayeredWindow` failures | confirm §4/§5 on mom's box | LOW | Ship a diagnostic build to mom to confirm before/after. |

**Recommended sequencing:** F1 + F2 + F3 first (low-risk, cover the bulk of the cluster:
dead buttons + blank panel + empty list), with DIAG folded in. F4 next. F5 and §6 (picker refactor)
as a separate, more careful pass. All are Windows-only overlay changes; **macOS is unaffected**
(NSPanel, no OSR layered window, no Win10 bug) — but F1/F3 touch the shared IPC-handler / React, so
verify mac parity there.

## Open questions for the owner
1. **F1 UX:** OK that the toolbar button only OPENS the panel (close by clicking away), matching the
   download/menu panels? (Removes the click-button-again-to-close nicety, but kills the dead-button
   class outright.)
2. **F5 scope:** tackle the single-instance-forward-to-dying-process now, or defer it into the
   same-process picker refactor?
3. Ship a **diagnostic build to mom first** to confirm §4/§5, or go straight to the F1–F3 fixes
   (which are low-risk regardless)?
