# Phase 12 — adblock on redirected arrivals (YouTube ads when the video is reached from X)

**Opened:** 2026-09-15, from the owner's report. **Status:** ⬜ PLANNED — reproduce first; no fix design until the
mechanism is *measured*. **Standard:** `../HARNESS.md`. Runs after Phase 11.

## The report

👤 *"The ad blocker works when the user navigates to YouTube and watches a video. If the user clicks a YouTube
video from inside X (and possibly other sites), it launches in a new tab or navigates there, but the ad is not
blocked."*

## The mechanism — a hypothesis from a code read, not a measurement

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

## Evidence rows (names reserved)

| ID | GREEN | RED | SUBJECT |
|---|---|---|---|
| `P12-A1` | YouTube reached via a `t.co`/`youtu.be` redirect: the pre-roll ad is blocked (the `AdblockResponseFilter` key-rename fires **and** the scriptlet inject line names the committed URL) | today's binary, same click: ad plays, inject line absent or names the wrong URL | the tab's `role: tab_<n>` in the log; the video page URL; the adblock engine on 31402 (dev) |
| `P12-A2` | Direct navigation to youtube.com still blocked (non-regression) | adblock disabled for the site ⇒ ad plays | same |
| `P12-A3` | New-tab arrival (`OnBeforePopup` path) and same-tab arrival both blocked | one of them left on the old key ⇒ red | which path was taken, from the log |
| `P12-A4` | Minimal site basket unchanged (youtube, x, github) | — | Testing Standards table |

Real-site testing is mandatory (CLAUDE.md Testing Standards); both platforms; 🍎 relay row.
