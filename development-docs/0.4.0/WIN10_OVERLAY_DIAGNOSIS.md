# Win10 Dead New-Toolbar-Buttons + Bookmark Write Failure — Diagnosis & Fix Design

> ## ⛔ CORRECTION (2026-07-15) — the §2/§4/§5 root cause below is REFUTED. Do NOT implement F-A/F-B as written.
> A 4-lens adversarial review (`wf_e371861c-6ee`, all verified against real code) overturned the core theory:
> - **F-A (unflushed `WS_EX_TRANSPARENT` clear) is a RED HERRING** — that flag is *never set* on site-info/bookmarks/tab-list (only on settings/wallet/backup via `simple_handler.cpp:5274`). The Show-path clear is a no-op; F-A fixes nothing.
> - **The paint code, WndProc, and window styles are IDENTICAL to the working overlays** — ruled out.
> - **Real cause (buttons):** the three dead panels are the **last three pre-warmed** (`cef_browser_shell.cpp:5218-5231`, 3.5/4/4.5 s). On a slow Win10 box under the startup subprocess burst they likely **never reach first OnPaint** → an all-transparent layered DIB → invisible AND click-through by DWM alpha hit-testing (no `WS_EX_TRANSPARENT` needed). Works on the picker launch because it spreads the spawns out.
> - **⚠️ F-B (destroy+recreate self-heal) is DANGEROUS** — `CreateBrowser` is async; repeated clicks destroy the in-flight browser before it paints → a destroy/recreate LOOP that leaves the button *permanently* dead, and it clobbers the mouse-hook / `last_hide_tick` / static-ref invariants. Do NOT build as designed.
> - **Real cause (bookmarks):** the "detached-thread init race" is REFUTED (`bookmarkThread.join()` at `cef_browser_shell.cpp:5128` precedes the message loop). Likely cause = **`BookmarkManager::OpenDatabase()` fails for the whole session** (WAL/AV-lock/profile-path) → `db_` null → every write fails, every read empty. **This failure is ALREADY LOGGED** (`BookmarkManager.cpp:52/95`), so mom's existing `%APPDATA%\HodosBrowser\logs\debug_output.log` likely already contains the smoking gun.
> - **Plan:** (1) get mom's existing log to CONFIRM (free); (2) SAFE to land now: bookmark "gold-star-only-on-success + surface the swallowed error", plus diagnostic logging (per-overlay first-OnPaint + CreateBrowser result). (3) Likely-safe real button fix = **create-on-click instead of pre-warming those three** (the proven first-launch path), NOT the fragile F-B. All overlay changes stay gated by CLAUDE.md inv #8 (adversarial review + Win11 parity) AND owner sign-off, and only after the log confirms.
>
> ## 📋 UPDATE (2026-07-15) — mom's beta.26 log obtained; buttons STILL dead; the log can't see it
> Owner supplied mom's actual `debug_output.log` (Win10, user "Mary"), spanning beta.20 → beta.22 → **beta.26** (2026-07-11 session).
> - **The log LOOKS like the buttons work:** `bookmarks_panel_show → "shown"`, `siteinfo_panel_show → "shown"`, `tablist_panel_show → "shown"`, and `bookmark_add → "Added bookmark … id 14/15/16"` persisting to the DB; `BookmarkManager … initialized successfully`.
> - **Owner confirms in person: the three buttons 100% DO NOT work on her machine.** Resolution: for these layered/OSR overlays, `"overlay shown"` only means the **C++ show-path ran** (SetWindowPos + clear `WS_EX_TRANSPARENT`) — it does **NOT** mean the subprocess painted a pixel or that a click lands. A working button and a blank/never-painted dead button emit the **identical** log line. **Our current logs cannot tell shown-from-rendered.** That is the #1 diagnostic gap.
> - **"Slow hardware" theory WEAKENED (owner's point):** other apps run on her machine, and **5 of 8 overlays work** (menu/wallet/shield/profile/settings). A machine that paints five overlays isn't "too slow for overlays." The failure is **deterministic and selective** (always the same three) → points at a code/environment difference, not a timing race. Only concrete differentiator: the three dead ones are the newest, added together in the header-UX pass (left-anchored, last-pre-warmed) — but anchor side almost certainly does NOT itself break paint; it's just a marker for "same code path, spawned last." **No proven mechanism yet.**
> - **Update integrity:** the beta.26 log shows the NEWEST native behavior (pre-warms all 8 overlays at startup) AND the newest frontend bundles load → she is genuinely on beta.26 code (not a stale-native/fresh-frontend split) that session. But a PRIOR botched update could have left corrupt cached state (known Windows silent-update regression on file).
> - **Blast-radius question (owner, unresolved):** slow-hardware CLASS bug (broad, invisible first-run churn) vs. this-one-garbage-machine (narrow). Decisive cheap tests: (1) **clean / over-the-top reinstall of beta.28 on her machine** — works after ⇒ update-corruption ⇒ narrow (remedy = reinstall + harden updater); still dead ⇒ real code/Win10 issue ⇒ broad; (2) a **throttled Windows 10 VM** for a repro we control; (3) a **per-overlay first-paint log** (the missing signal) so any machine's log becomes decisive — safe, pure diagnostics, queued for beta.29 IF beta.28 clean-install still fails.
> - **Decision:** NO overlay surgery in beta.28. beta.28 ships the current `0.4.0` tip (deconfliction + WS1) as a DRAFT; owner installs it fresh on mom's machine. Buttons return ⇒ it was corruption. Still dead ⇒ add paint-logging for beta.29 + repro on a throttled Win10 VM. **F-B destroy/recreate stays OFF the table.**
>
> Everything below is the ORIGINAL 2026-07-13 diagnosis, retained for the record — treat its root cause as SUPERSEDED.

**Status (2026-07-13):** Diagnosis confidence — failure CLASS is HIGH; exact runtime sub-mechanism is
NOT statically resolvable and the target machine (mom's Win10) is **unavailable** for runtime
confirmation. Therefore the fix must be **robust across all plausible sub-mechanisms** and
**safe-by-construction** (no regression to Win11 or to the already-working overlays). Gated by
CLAUDE.md invariant #8 (CEF overlay/window lifecycle is fragile) → owner sign-off + adversarial review
+ Win11 parity check before any code lands.

Precedes / gates: the single clean 0.4.0 build (owner decision 2026-07-13: do NOT promote beta.27;
fold mom's fixes in so users update once).

---

## 1. Symptom (owner-reported, Windows 10, NOT reproduced on owner's Win11)

- After install + a 2nd launch (profile picker skipped, Default loads directly), the **three new
  toolbar buttons** — bookmarks, site-info ("site controls"), tab-list ("search tab") — are **DEAD**:
  clicking does nothing, no overlay ever appears. Confirmed PERSISTENT: **~10 clicks over a full
  minute, never opens** (kills any "recovers after pre-warm" timing theory).
- The **four old buttons** (wallet, shield/privacy, profile, settings) work normally on the same launch.
- On the **first** launch (picker shown, ~8s human delay before clicking) the new three DID work.
- **Bookmark star:** clicking turns the star gold, but the bookmark does NOT persist — confirmed gone
  after reopening the panel (owner test) → the write fails, not a redraw drop.
- Owner's Win11 reproduces NONE of this, even with "Clear data on exit" ON.

---

## 2. What the code actually says (grounded this session, current line numbers)

1. **All 8 dropdown overlays pre-warm in one `!g_picker_mode` block**, staggered 1000–4500ms
   (`cef_browser_shell.cpp:5218-5231`). Order: menu 1000, wallet 1500, download 2000, cookie 2500,
   profile 3000, **siteinfo 3500, bookmarks 4000, tablist 4500**. The new three are the **last three**.
   This is the ONLY code-level difference between the failing new overlays and the working old ones.
2. **The new-three click handlers already self-heal a destroyed HWND** and already removed the fragile
   "visible → Hide" branch (the F1 fix): `simple_handler.cpp:6807-6816` (bookmarks), `:6908-6915`
   (tablist), `:7009-7019` (siteinfo). If `!IsWindow(hwnd)` they `Create(showImmediately=true)`.
3. **The Show functions for new vs old are essentially identical.** Compare `ShowSiteInfoPanelOverlay`
   (`simple_app.cpp:2253-2328`) with `ShowDownloadPanelOverlay` (`:2007-2089`): both do
   `SetWindowPos(HWND_TOPMOST, …, SWP_NOACTIVATE | SWP_SHOWWINDOW)` (NO `SWP_FRAMECHANGED`), then
   `SetWindowLong(… exStyle & ~WS_EX_TRANSPARENT)`, then `NotifyScreenInfoChanged + WasResized +
   Invalidate(PET_VIEW)`. **No meaningful difference.**
4. **`WS_EX_TRANSPARENT` (the click-through flag) is cleared AND flushed only inside `OnPaint`**
   (`my_overlay_render_handler.cpp:229-242`), and only when `OnPaint` still sees the flag set
   (`if (exStyle & WS_EX_TRANSPARENT)`) AND `UpdateLayeredWindow` succeeded. The in-code comment
   (`:233-239`) states plainly: a `GWL_EXSTYLE` change is cached until a `SWP_FRAMECHANGED`
   `SetWindowPos` flushes it; **Win11's compositor tolerates the omission, Win10's does not — "which
   is why the newer left-anchored panels broke only on the owner's Win10 machine."**

---

## 3. Honest read on "why the new 3 and not the old 4"

From the code, they **should not** differ — the show/paint logic is the same. The only code-level
difference is pre-warm ORDER (new three last). So the trigger is either (a) pre-warm timing/contention
or (b) runtime paint state, **neither statically provable and unconfirmable without mom's machine.**
We deliberately do NOT invent a mechanism to rationalize it. What we DO fix is the code-proven
fragility below, which makes ANY of these overlays able to go permanently click-through on Win10.

---

## 4. The code-proven fragility (the fixable core)

The Show path clears `WS_EX_TRANSPARENT` via `SetWindowLong` **without flushing it** (its `SetWindowPos`
lacks `SWP_FRAMECHANGED`), relying on `OnPaint` to flush. But `OnPaint` only flushes if it still sees
the flag set — and the Show path just cleared it — so the flush can be **skipped**. On Win10 an
unflushed clear leaves the window click-through → the button is dead. This is latent for every dropdown
overlay; it bites whichever one's paint/flush timing loses the race (empirically, the last-pre-warmed
three, on a slow Win10 box). A never-painted overlay is worse still: invisible (blank DIB) AND
click-through (flag never cleared), and the current Show path cannot repair either condition on Win10.

---

## 5. Fix design (robust across sub-mechanisms; reuses already-proven patterns)

**F-A — flush `WS_EX_TRANSPARENT` in the Show path (core; LOW novelty). Apply to ALL dropdown Show
functions (old + new) for consistency.**
When the Show path clears `WS_EX_TRANSPARENT`, immediately flush it with the SAME pattern `OnPaint`
already uses (`my_overlay_render_handler.cpp:240-241`):
`SetWindowPos(hwnd, nullptr, 0,0,0,0, SWP_NOMOVE|SWP_NOSIZE|SWP_NOZORDER|SWP_NOACTIVATE|SWP_FRAMECHANGED);`
Sites: `ShowSiteInfoPanelOverlay:2312`, `ShowTabListPanelOverlay` (analogous), `ShowBookmarksPanelOverlay`
(analogous), and the old `ShowDownloadPanelOverlay:2069` / `ShowCookiePanelOverlay` / `ShowProfilePanelOverlay`
for parity. Makes the window clickable on Win10 without depending on `OnPaint`'s conditional flush.

**F-B — self-heal if the pre-warmed browser is dead / never painted (MED). Covers the invisible/blank
sub-mode.**
Track a per-overlay "has painted ≥ once" flag, set in `OnPaint` (`my_overlay_render_handler.cpp` after a
successful `UpdateLayeredWindow`). In each new-three click handler's `else → Show` branch
(`simple_handler.cpp:6816/6914/7018`), change the guard from "HWND exists" to "HWND exists AND browser
alive AND has-painted": if the browser (`GetXxxPanelBrowser()`) is null OR has-painted==false →
`DestroyWindow(hwnd)` + `CreateXxxPanelOverlay(…, showImmediately=true, …)` synchronously — the proven
pre-F2 self-healing path. This is the exact gap all diagnosis passes circled.

**F-C — keep the existing `WasResized + Invalidate(PET_VIEW)` on Show (belt; already present).**

**Bookmark (independent; LOW risk, no CEF lifecycle surface).**
Root: `AddBookmark` returns `{success:false}` without throwing when `db_` is null
(`BookmarkManager.cpp:278-282`); the star goes gold unconditionally ignoring `add()`
(`BookmarksOverlayRoot.tsx`), and `BookmarkManager::Initialize` runs on a detached thread post-
`CefInitialize` (`cef_browser_shell.cpp:5086-5091`) → a click before init hits null `db_` → INSERT lost.
Fix: (1) only turn the star gold on `add()` success; (2) surface the currently-swallowed error to the
log; (3) block/queue the first `bookmark_add` until `Initialize` completes.

---

## 6. Risk & verification (CRITICAL: no target machine)

- All overlay fixes touch overlay window styling/paint → **invariant #8**. F-A/F-B **reuse the exact
  pattern `OnPaint` already runs** (low novelty); F-B's destroy+recreate must be reviewed against the
  mouse-hook install/remove and `last_hide_tick` toggle-suppression guards for ordering/races.
- **We cannot test on mom's Win10.** Mitigations: (1) F-A is a strict superset of the existing proven
  flush — doing it earlier and unconditionally cannot make Win11 or the working overlays worse; (2) F-B
  only fires when the pre-warm is provably bad (null/never-painted), degrading to the proven
  `showImmediately=true` create path; (3) verify no regression on Win11 (owner) and, if possible, on a
  throttled/VM Win10; (4) all changes additive + reversible.
- Confidence statement to preserve honesty: the FIX addresses the code-proven click-through fragility
  and the dead-browser case with HIGH confidence; whether that is precisely what bites mom's machine is
  ~80% (the runtime remainder is unobservable without her box). The fix is designed so that being wrong
  about the exact sub-mechanism still leaves us correct, because it covers all of them.
