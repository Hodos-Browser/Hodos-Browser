# Mac relay — beta.3 Phase 7a + 7b (the consent surface)

**Relayed 2026-09-04 from the Windows box.** ⛔ Nothing here is claimed as verified on macOS. Every
row is *"Windows measured this; Mac must measure it too."*

**Commits:** `c7ce5c6` (7a modal viewport) · `7dc00e4` (consent favicon) · `b3487a8` (favicon store)
· `0abc6d7` (tile fallback + sweep) · `3afe5d0` (one-view merge) · `ced5673` (reorder + fold)
· `b9e875f` (docs)

⚠️ **Mac is a long way behind.** Owner, 2026-09-04: *"I have not had Mac do any work recently so it
is falling way behind. I will get it started after we finish this phase."* Phases **6 (CUT), 7a, 7b**
have all landed since the last relay. ⇒ Read `MAC_RELAY_P35_P4_ROUND.md` and `MAC_RELAY_P5_ROUND.md`
first; their owed rows are still owed.

---

## M1 — Almost all of this is cross-platform, and that is the point

| Change | Files | Platform |
|---|---|---|
| Modal height cap + fallback tile | `BRC100AuthOverlayRoot.tsx`, `NewTabPage.tsx` | **shared** (React) |
| One-view connect merge, protocol ids, `domain_approval` copy parity | `BRC100AuthOverlayRoot.tsx` | **shared** |
| Favicon: page's own icon on consent modals | `TabManager.{h,cpp}`, `HttpRequestInterceptor.{h,cpp}`, `simple_handler.cpp` | **shared core**, ✅ both `#ifdef` arms already edited |
| Favicon store | `FaviconStore.{h,cpp}` (new), `cef_browser_shell.cpp`, `CMakeLists.txt` | ⚠️ **see M2** |
| Omnibox / new tab / bookmarks read from the store | `useFavicons.ts` + 3 pages | **shared** (React) |

⭐ `FaviconParamForDomain()` was written **once** and called from both the Windows and macOS arms of
`FireHodosPermissionPrompt` (invariant #9). No macOS-specific favicon code should be needed.

## M2 — 🎯 The one genuinely Mac-shaped risk: `FaviconStore` init and teardown

`hodos::FaviconStore::GetInstance().Initialize(profile_cache)` and `.Shutdown()` are wired in
**`cef_browser_shell.cpp`** — the **Windows** entry point — beside `SitePermissionStore` and
`PaidContentCache`.

⛔ **Check `cef_browser_shell_mac.mm` initialises it too.** If macOS does not, `IsInitialized()`
returns false everywhere and the failure is **silent and plausible-looking**: every surface falls
back to its letter tile / globe glyph and simply shows no icons. Nothing errors. ⇒ A Mac reviewer
would see "no favicons anywhere" and reasonably assume it is a rendering bug rather than an
uninitialised singleton.

`src/core/FaviconStore.cpp` is in the **shared** `SOURCES` list in `CMakeLists.txt`, so it compiles
on both; only the init/shutdown call sites are platform-specific.

⚠️ Also confirm `CefBrowserHost::DownloadImage(url, is_favicon=true, …)` behaves on macOS — it is a
CEF API we had **never used before this phase**, on either platform.

## M3 — How to reproduce, no special rig needed

**Modal geometry (7a).** `phase-7a-modal-viewport/` has `mkfixture.py` (builds a greedy manifest),
`measure.py` (card vs viewport), `verify.py`, `branches.py`. They drive the real notification overlay
over CDP via `phase-3.5-layout-window-scoping/p35drive.py`.

> Windows RED, for comparison: at **10 declared protocols** the card was 1160 px in a 1032 px
> viewport and Decline/Customize/Connect were a **2-pixel strip, clickable and unreadable**. After:
> constant 749 px from 8 to 40 permissions.

⚠️ **The cap is `calc(100vh - 88px)` and the 88 is arithmetic, not taste** — 32 breathing room + 56
of the card's own padding, because the card is **content-box**. `calc(100vh - 32px)` was tried,
measured at a 1056 px border box in a 1032 px viewport, and **the buttons still went off-screen**.
⛔ If macOS uses a different card padding, this number changes.

**Favicon leak.** `phase-7b-connect-modal/netwatch.py` (consent modal), `netwatch_page.py` (any
surface), `omnibox_watch.py` (human-driven). ⭐ **All three assert their own trigger and print
`⛔ VACUOUS` rather than a green if the subject rendered nothing** — see M5.

**Merged view.** `phase-7b-connect-modal/merged_view_probe.py` — one command, PASS/FAIL, asserts
order, the protocol id at level 1, no Customize button, and that "Allow without limits" is hidden
while limits are collapsed.

## M4 — 🚨 What to re-measure on Mac, specifically

| # | Check | Windows result |
|---|---|---|
| 1 | Consent modal opens → **zero** requests to `google.com` / `gstatic.com` | 0 (was **2**: `s2/favicons` → `t2.gstatic.com/faviconV2?…&url=…`) |
| 2 | New tab / bookmarks / omnibox → zero third-party favicon requests | 0 (new tab was **32**, to Google **and** DuckDuckGo) |
| 3 | `FaviconStore` actually stores after visiting a site | `favicons.db`, 1 row per host, PNG bytes |
| 4 | Missing icon renders a **letter tile**, never a broken image | ✅ after `0abc6d7` |
| 5 | Merged connect view: order, protocol id, no Customize | `merged_view_probe.py` PASS |
| 6 | Modal fits at the mac DPI/resolution set | 897 px in 1032 px |

⚠️ **#2's macOS numbers will differ** — they depend on the profile's own history. What must match is
**zero third-party**, not the total.

## M5 — ⚠️ What a Mac reviewer should be most suspicious of

⛔ **The instruments in this phase produced FOUR false greens on Windows before they were trusted.**
Do not treat a clean run as a pass until the run proves the subject was alive:

1. `netwatch.py` fired `window.showNotification` without checking it existed. It did not — the
   component was throwing at mount — so no modal opened, no image was requested, and it printed
   `GOOGLE REQUESTS: 0 (none)`, which **reads exactly like success**. Three runs.
2. The omnibox run reported 0 requests while the overlay rendered **nothing at all**
   (`rows: 0, imgs: 0, bodyLen: 14`).
3. `npx tsc --noEmit -p tsconfig.json` passed on code that `npm run build` rejects with 4 errors —
   including a scope bug that would have left the omnibox broken. ⛔ **Use `npm run build` /
   `preflight -Full`, never `--noEmit`, in this repo.**
4. A tooltip-clipping test reported 5–8 clipped tooltips by hovering all 23 icons at once, counting
   ones belonging to icons scrolled out of view. Hovering one at a time: **0 clipped**. It nearly
   condemned a correct fix.

⭐ Also: `preflight -Full` reported `T1d ... exited 143` once. **143 is SIGTERM** — the dev stack was
competing for CPU. Stop dev, re-run, and read the exit code before "fixing" a build that is not broken.

## M6 — Carried, still Mac-owed from earlier rounds

- Sparkle 2.9.6 verification + its negative control; the Big Sur / `minimumSystemVersion` call
- `T1g` (limit-field contrast) on macOS
- **P4 M3** — `CreateTabContextMenuOverlay` has **no macOS implementation**. Windows has 15 overlays,
  macOS 14. Nothing is broken; macOS simply has no tab context menu
- Phase 4 owner items **O2** (click-outside dismiss), **O3** (focus half of P4-A4), **O5** (macOS),
  **O6** (hear a muted tab)
- WS5(b) W7 overlay coverage; the two Phase-5 macOS unknowns in `MAC_RELAY_P5_ROUND.md` M2

## M7 — Not in this relay

**Phase 6 (Chrome import) was CUT** → beta.5. Do not start the macOS Keychain half.
**Phase 7c–7e are not written yet**; 7c is an engine change in Rust (`matrix_c.rs`), so it will be
one binary and platform-neutral when it lands.

---

## ✅ ANSWERED BY MAC 2026-09-08 — 🚨 **M2 was REAL. `FaviconStore` was never initialised on macOS.**

Your prediction was exactly right, including the failure mode. Evidence:
`MAC_RELAY_BETA3.md` (round 2026-09-08 Mac, §B) and `phase-7b-connect-modal/PHASE_CONTRACT.md` §4b.

- `cef_browser_shell_mac.mm` called neither `Initialize` nor `Shutdown`, and did not include the
  header. Dead since `b3487a8` (2026-09-04). **Fixed and runtime-verified this round.**
- Silent exactly as you said: `OnFaviconURLChange` gates on `IsInitialized()` (never downloads) and
  `favicon_get` returns `""` (host omitted → letter tile). Nothing errors.
- 📏 The artifact, not a log: `favicons.db` **birth 2026-09-08 16:32:30**, while the profile and its
  sibling stores date from 2026-07-07. ⛔ My first control was worthless — `build/bin/debug.log` has
  0 `FaviconStore` lines but also 0 `SitePermissionStore` lines, i.e. the wrong sink.
- ⭐ **`DownloadImage(is_favicon=true)` works on macOS** — 4,552 real PNG bytes at width 64 stored.

⚠️ **Not as bad as it sounds:** the *privacy* subject held anyway. The React surfaces stopped
emitting `google.com/s2/favicons` regardless, and the store path was skipped, so **no third-party
request was ever made**. De-Googling intact; only the local replacement was dead.

⬜ **M4 #1/#2 still owed on macOS**, and this fix *changes what they measure* — before today macOS
could produce neither a store hit nor a Google request, so a green would have been vacuous.
