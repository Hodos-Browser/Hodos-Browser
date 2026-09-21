# Phase 12 — adblock on redirected arrivals (YouTube ads when the video is reached from X)

**Opened:** 2026-09-15, from the owner's report. **Status:** 🟨 REPRODUCED 2026-09-19 — mechanism measured, and it is **not**
the one this document hypothesised. See §"What the measurement actually found" below; the hypothesis section is kept
only as the record of what we believed going in. **Standard:** `../HARNESS.md`. Runs after Phase 11.

## The report

👤 *"The ad blocker works when the user navigates to YouTube and watches a video. If the user clicks a YouTube
video from inside X (and possibly other sites), it launches in a new tab or navigates there, but the ad is not
blocked."*

## ⛔ DISPROVED — the hypothesis from a code read (kept as the record of what we believed going in)

- `simple_handler.cpp :: OnBeforeBrowse` pre-fetches the cosmetic resources for the **request URL** (`navUrl`)
  and sends them to the renderer as `preload_cosmetic_script` keyed by that exact string.
- `simple_render_process_handler.cpp :: OnContextCreated` looks the cache up by the **frame's URL** and injects
  only on an exact match (`s_scriptCache`, "URL → scriptlet JS", one-shot).
- A link from X is a redirect chain: `t.co/…` → often `youtu.be/…` → `youtube.com/watch?v=…`. The key is the first
  URL, the document is the last. ⇒ no match, no injection, the ad plays.
- The comment in that very block says the pre-cache exists so *"anti-bot scripts like Cloudflare Turnstile
  observe the mutated surface"* — so a miss here also feeds Phase 13.

Open questions the reproduction answers: does CEF call `OnBeforeBrowse` again for the redirected navigation
(`is_redirect == true`) with the final URL, and if so why is the key still stale (query-string or host
canonicalisation? `www.` vs bare? a client-side redirect the browser process never sees?). Does the new-tab
path (`OnBeforePopup` → `CreateNewTabWithUrl`) behave differently from same-tab navigation?

## How to research it

1. **Reproduce with the logs.** Dev rig, x.com, click a YouTube card. Read `debug_output-<pid>.log` for the
   `OnBeforeBrowse: pre-caching scriptlets for <url>` and `OnContextCreated: injecting scriptlets for <url>`
   lines and compare the URLs. Repeat for a `youtu.be` link pasted into the address bar (no X involved) — if
   that alone reproduces it, X is irrelevant and the phase is about redirects.
2. **Prior art — Brave** (rule 5; log a `PRIOR_ART.md` row): Brave injects cosmetic filters and scriptlets from
   the renderer at script-context creation, keyed by the frame's **committed** URL, fetched synchronously from
   the browser-process adblock service (`brave/components/cosmetic_filters/renderer/cosmetic_filters_js_handler.cc`,
   `CosmeticFiltersTabHelper`). A redirect cannot desynchronise a lookup that happens at commit time. Read the
   actual source before citing the shape; MPL-2.0 — port the pattern, never the code.
3. **Candidate fixes**, decided after step 1: (a) re-key on every `OnBeforeBrowse` including redirects and
   canonicalise the key (cheapest, keeps the pre-cache); (b) move the lookup to commit time — the renderer asks
   the browser process synchronously in `OnContextCreated` with the committed URL (Brave's shape; a sync IPC on
   the render thread's critical path, so measure first-paint cost); (c) both — pre-cache as the fast path, commit-time
   lookup as the miss handler.

## ⭐ What the measurement actually found (2026-09-19, head `92bc491`, fresh rig)

⛔ **The redirect is not a factor. The render-process boundary is the entire mechanism.**

A 2×2 factorial, driven by CDP `Page.navigate` on the **existing** tab target (never `/json/new`, which
bypasses `OnBeforeBrowse` and is defect #1 of the farbling-harness family):

| | **same render process** | **cross render process** |
|---|---|---|
| **no redirect** | **B** `youtube.com/watch?v=A` → `?v=B` — ✅ early injection fires | **A** `127.0.0.1:5137/newtab` → `youtube.com/watch` — ❌ **no early injection** |
| **redirect** | **C** `youtube.com` → `youtu.be/ID` → `youtube.com/watch` — ✅ early injection fires | **D** `github.com` → `youtu.be/ID` → `youtube.com/watch` — ❌ **no early injection** |

The URL key was **correct in all four cells**. `OnBeforeBrowse` *does* re-fire on the redirect with the
**final** URL (cell C proves it), so the key is already the committed URL and the exact-match lookup is fine.

**The actual defect.** `OnBeforeBrowse` ends in
`frame->SendProcessMessage(PID_RENDERER, "preload_cosmetic_script")`. That delivers to the render process
that hosts the frame **at that moment** — the **source** document's process. On any cross-site navigation
Chromium commits the new document in a **different** render process, and `s_scriptCache` in
`simple_render_process_handler.cpp` is a **process-local `static`**. The destination process's cache is empty,
so the lookup misses and the early injection is skipped in silence.

Measured, cell D — the two halves land in different processes:

```
18:37:40.894 [BROWSER] OnBeforeBrowse: pre-caching scriptlets for https://www.youtube.com (23130 chars)
18:37:40.9xx [RENDER] PID 43176  Pre-cached scriptlets   <- github.com's process; never hosts the document
18:37:4x.xxx [RENDER] PID 8344   OnContextCreated, Frame URL: https://www.youtube.com
                                  (no "injecting scriptlets" line — PID 8344's cache is empty)
18:37:43.544 [BROWSER] Injecting scriptlets for https://www.youtube.com   <- the LATE path, post-load
18:37:43.5xx [RENDER] PID 8344   Injecting cosmetic scriptlets (23130 chars)
```

⭐ **Why the feature still "works" on a direct visit, and why that misled the report.** There are *two*
injection paths and only the early one is broken. The late path in `OnLoadingStateChange` sends
`inject_cosmetic_script` on `browser->GetMainFrame()` **after load completes**, by which time the frame is in
the destination process — so it is **process-correct** and always lands. The regression is therefore not
"no scriptlets" but **when**:

| | first mutation of the JS surface |
|---|---|
| same-process arrival (cell B) | **+147 ms**, inside `OnContextCreated` — **before any page JS runs** |
| cross-process arrival (cell D) | **+2650 ms**, at load-complete — **after** the page's own JS has run |

⇒ ~**2.6 s of un-mutated `fetch`/XHR/canvas surface** on every arrival-from-a-link. That is the whole
window in which the YouTube player reads its ad configuration — which is why the owner sees the pre-roll
when the video is reached from X, and not when they are already browsing YouTube.

⭐ **Phase 13 link — now evidence, not speculation.** The early path's stated purpose is that anti-bot
scripts *"observe the mutated surface"*. It has been measured **never to run on a cross-site arrival**, and
arriving from a link is how a user reaches any site. ⚠️ This raises Phase 13's prior but does not settle it:
Phase 13 **step 0** (re-test the CAPTCHA report on 0.4.0, since it was filed against 0.3.x injected-JS
farbling) still stands.

### ⚠️ Two instrument corrections — the phase doc sent the last reader to the wrong files

1. ⛔ **The renderer lines are NOT in `debug_output-<pid>.log`.** That file carries `[BROWSER]` lines only.
   `💉 Pre-cached scriptlets` and `💉 OnContextCreated: injecting scriptlets` are `LOG_*_RENDER` and land in
   **`%APPDATA%/HodosBrowserDev/logs/cef_debug.log`**, via `ChildProcessLogSink.cpp`, prefixed with the
   renderer **PID** — which is what makes the process split visible at all. The session prompt's warning
   ("not `cef_debug.log`") is right about the *browser* half and wrong about the *renderer* half; you need
   **both files**. ⚠️ `cef_debug.log` is **truncated on every launch** — it covers the current session only.
2. ⛔ **`hodos::LogSafeUrl` is origin-only** (`LogSafeUrl.h`), so every one of these lines prints
   `https://www.youtube.com` with **the path and query stripped**. ⇒ The measurement this README proposed —
   *"compare the two URLs in the log and see them disagree"* — **cannot distinguish a requested URL from a
   committed URL that shares its origin**, which is the exact comparison it was asked to make. The process
   **PID** in `cef_debug.log` is the discriminator that works.

### Prior art — and it corrected this document

Logged in `development-docs/PRIOR_ART.md` (2026-09-19). Read from Brave source, MPL-2.0, pattern only.
⛔ **This README's citation was wrong**: Brave does not key the lookup off the committed URL *at
script-context creation*. `CosmeticFiltersJsRenderFrameObserver` stores the URL in `DidStartNavigation` and
calls `ProcessURL` in **`ReadyToCommitNavigation`** — a callback that runs **in the frame that is about to
commit**, i.e. **in the destination render process**. That single placement is what makes both a redirect and
a cross-process swap harmless. Two further guards our code lacks: a **fallback key** (empty / invalid /
`about:blank` ⇒ the frame's security origin) so a miss cannot be silent, and `RunScriptsAtDocumentStart`
**waiting** on a `OneShotEvent` rather than skipping when the data has not arrived. ⚠️ Brave's *synchronous*
load is behind the `kCosmeticFilteringSyncLoad` feature flag with async mojo as the alternative — so
"sync IPC on the critical path" is a **tunable Brave itself hedges**, which is relevant to candidate (b).

## ⛔ Candidate (a′) RULED OUT by measurement — a push from `OnLoadStart` loses the race

Owner's call was to spend 30 minutes on the cheap no-patch option before considering a fork patch.
Done, 2026-09-19, with a throwaway probe (`SimpleHandler::OnLoadStart` override sending the same
`preload_cosmetic_script` payload; **probe reverted afterwards, no code left in the tree**).

**Result: the payload reaches the right process, but always too late.** Three trials, three
**distinct** video URLs, `github.com` → `youtube.com/watch?v=…` each time:

| trial | destination-process `OnContextCreated` | probe receipt in that same process | verdict |
|---|---|---|---|
| 1 | 18:54:49.788 | 18:54:49.804 | **16 ms late** ❌ |
| 2 | 18:55:16.075 | 18:55:16.110 | **35 ms late** ❌ |
| 3 | 18:55:42.339 | 18:55:42.357 | **18 ms late** ❌ |

**Early injections: 0 of 3.** And the ordering is *structural*, not a tight race — the browser process's
`OnLoadStart` **fires after** the destination renderer has already created the V8 context (measured
8 ms after, in the first run). There is no margin to tune: by the time CEF tells the browser process
the load started, the only injection point has passed.

⭐ **What the probe did prove:** a push from `OnLoadStart` lands in the **correct** render process every
time (the `OnBeforeBrowse` push never does). So the *process-targeting* half is solvable without a patch;
it is the *ordering* half that is not.

### 🚨 The near-miss — read this before trusting any re-run of this experiment

The **first** three trials reused **one** video URL and scored **2 of 3 GREEN**. That was an artifact and
it would have shipped the wrong conclusion. `s_scriptCache` is keyed by URL and one-shot, so trial *n*'s
too-late payload **sat in the cache and was consumed by trial _n+1_'s context** — the same URL, one
navigation later. Re-running with distinct URLs scored **0 of 3**.

⛔ **Any re-run of this measurement must use a fresh URL per trial**, and must assert the *value*
(an injection attributable to **this** navigation) rather than merely that an injection line appeared.
A first arrival at a URL is the only case the user ever experiences.

### ⇒ Recommendation: candidate (b), the registry + renderer-pull patch

The remaining option is the one this repo has **already built once, for the identical defect**, in the
same function: `OnBeforeBrowse`'s farbling block files `hodos_farble_key` into `hodos::FarblingRegistry`
browser-side (intercepted in libcef's `CefFrameHostImpl::SendProcessMessage`) and the renderer **pulls**
it at `OnContextCreated`. Its own comment states the reason verbatim — *"a push from here is pre-commit,
so it lands on the outgoing document"* — and the cosmetic-scriptlet path was simply never moved across.
Brave reaches the same shape from the other direction (`ProcessURL` in `ReadyToCommitNavigation`, i.e. in
the committing frame). ⚠️ **Cost: a CEF fork patch on `hodos/7871` + a full engine rebuild + a
`CEF_CHECKOUT` bump** — there is no local Chromium checkout, so this is build-host work, and it carries
per-Chromium-bump maintenance. ⚠️ Candidate (c), a renderer-side direct fetch to the adblock engine,
remains **unverified** — the Windows render sandbox may block the socket outright, and that must be
proven, not assumed, before it is costed.

**Interim mitigation available without a patch (owner's call, not applied):** inject on *receipt* of
`preload_cosmetic_script` when the frame's URL already matches, instead of only caching for a context
that has already come and gone. Measured effect: first mutation moves from **~1200 ms** after context
creation (today's late path) to **~16–35 ms**. A 97% cut, but **not** a guarantee of beating an inline
script, so it is a mitigation and not the fix.

## 🟨 MITIGATION LANDED 2026-09-21 — Phase 12 stays OPEN

Owner's call: land the no-patch mitigation now, scope the registry patch for the build host.
⛔ **This does not close Phase 12.** It shrinks the exposure window; it does not remove it.

**What changed** (two files, no CEF patch):

1. `simple_handler.cpp :: OnLoadStart` (new override) — re-pushes `preload_cosmetic_script`. By this
   point the frame is in the **destination** render process, so unlike the `OnBeforeBrowse` push it
   always lands in the renderer that will host the document.
2. `simple_render_process_handler.cpp` — the receipt handler now has three branches: **inject now** if
   this frame's `OnContextCreated` already ran for this URL and did not inject; **drop** if it already
   injected (⛔ *not* cache — a leftover entry is the stale-entry artifact below); otherwise pre-cache as
   before. Two per-frame maps (`s_contextRanUrl`, `s_injectedUrl`, keyed by `CefFrame::GetIdentifier()`,
   which is a **`CefString`** in this CEF, not an int64) keep a single navigation from injecting twice —
   double-injected scriptlets would double-wrap the very APIs they override.

### Results — same rig, same harness, distinct URL per trial

| row | test | before | after |
|---|---|---|---|
| `P12-A1` | cross-process arrival (`github.com` → `youtube.com/watch`), 3 trials | **0/3** injected | **3/3** injected, **25 / 41 / 37 ms** after context creation (was ~1200 ms via the load-complete path) |
| `P12-A2` | same-process arrival, 2 trials — non-regression | injects at `OnContextCreated`, pre-JS | **unchanged**, still pre-JS, and the re-push is logged `duplicate payload … dropped` — **no double injection** |
| `P12-A4` | minimal basket: youtube, x, github | — | all three navigate clean, **0 errors, 0 crashes** in the browser log |

### ⛔ NEGATIVE CONTROL — and it went red for the right reason

`www.youtube.com` set to `scriptletsEnabled: false` in `adblock_settings.json`, browser restarted, two
cross-process trials re-run: **0 late-arrival injects, 0 injections of any kind** — while **2 YouTube V8
contexts were still created**, which is what proves the harness actually reached the subject rather than
failing to navigate. (Owner's settings file was backed up and restored byte-for-byte afterwards.)

⚠️ **Residual exposure is NOT zero.** 25–41 ms still elapses between context creation and injection, and an
inline `<script>` in the document head runs inside that window. The pre-JS guarantee only exists on
same-process navigations. ⇒ **Phase 12 remains open pending candidate (b).**

⚠️ `P12-A3` (the `OnBeforePopup` → `CreateNewTabWithUrl` new-tab arrival) is **not yet measured** — it is
the same defect class, but a brand-new browser's process assignment was not exercised here. It belongs to
the candidate-(b) work.

⚠️ Scriptlet payload size changed 23130 → 34283 chars between the 09-19 and 09-21 runs — the adblock
engine's 6-hourly filter-list update, not a code effect. The before/after counts are still comparable
because both arms of each comparison ran on the same payload.

## Evidence rows (names reserved)

| ID | GREEN | RED | SUBJECT |
|---|---|---|---|
| `P12-A1` | YouTube reached via a `t.co`/`youtu.be` redirect: the pre-roll ad is blocked (the `AdblockResponseFilter` key-rename fires **and** the scriptlet inject line names the committed URL) | today's binary, same click: ad plays, inject line absent or names the wrong URL | the tab's `role: tab_<n>` in the log; the video page URL; the adblock engine on 31402 (dev) |
| `P12-A2` | Direct navigation to youtube.com still blocked (non-regression) | adblock disabled for the site ⇒ ad plays | same |
| `P12-A3` | New-tab arrival (`OnBeforePopup` path) and same-tab arrival both blocked | one of them left on the old key ⇒ red | which path was taken, from the log |
| `P12-A4` | Minimal site basket unchanged (youtube, x, github) | — | Testing Standards table |

Real-site testing is mandatory (CLAUDE.md Testing Standards); both platforms; 🍎 relay row.
