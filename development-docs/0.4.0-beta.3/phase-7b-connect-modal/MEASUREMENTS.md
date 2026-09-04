# Phase 7b — measurements

## Row 1 — the consent surface no longer asks Google who you are trusting

**SUBJECT.** Dev build, `HODOS_DEV=1`, CDP 9322. Target attached **by URL**:
`http://127.0.0.1:5137/brc100-auth?type=idle` — the **notification overlay browser**, not a tab.
Driven through `window.showNotification(<query>)`, the same injection path C++ uses.
Instrument: `netwatch.py` (CDP `Network.requestWillBeSent`), `console.py` (exceptions).

### N1 — 🔴 RED, observed on the wire (pre-fix code, via `git stash`)

Opening one connect modal for `github.com`:

```
https://www.google.com/s2/favicons?domain=github.com&sz=48
https://t2.gstatic.com/faviconV2?client=SOCIAL&type=FAVICON&fallback_opts=TYPE,SIZE,URL&url=http://github.com&...
GOOGLE REQUESTS: 2  <-- LEAK
```

🚨 **The leak was WIDER than the ticket recorded.** The ticket names one request. There are **two
Google-owned hosts**: `s2/favicons` **redirects** to `t2.gstatic.com/faviconV2`, and the second one
receives `url=http://github.com` — the full URL as a query parameter, with `client=SOCIAL`.

⭐ `faviconV2` is precisely the Google favicon service that **Brave and ungoogled-chromium strip**
from Chromium (`PRIOR_ART.md`, 2026-09-04). We were reaching it by redirect from our own code.

### N2 — 🟢 GREEN, both fallback paths

| Case | trigger | non-local requests | Google |
|---|---|---|---|
| C++ supplied **no** favicon (host has no live tab) | SHOWN | 0 | **0** |
| C++ supplied the page's favicon | SHOWN | 1 → `https://github.com/favicon.ico` | **0** |

The one remaining request is **first-party, to the site being consented to** — a site that already
knows the user is there, because it initiated the wallet call. No third party learns anything.

Render check: `<img src="https://github.com/favicon.ico">` — `complete`, `naturalWidth 32`. The icon
is not merely un-leaked, it is actually displayed; and the domain is on screen beside it.

### N3 — 🚨 The instrument was blind, and its first three greens were vacuous

⛔ `netwatch.py`'s first version fired `window.showNotification(...)` and never checked the result.
`window.showNotification` was **unbound** (the React component was throwing on mount), so the modal
never opened, no image was ever requested, and the script printed:

```
total requests observed : 0
GOOGLE REQUESTS         : 0  (none)
```

— which is **indistinguishable from a clean pass**. Three runs reported that. It was caught only
because a separate DOM probe showed `rootChildren: 0`.

⇒ `netwatch.py` now asserts its own trigger (`'SHOWN'` vs `'NO_HANDLER'` vs `'EXCEPTION'`) and
**exits 2** rather than reporting counts it cannot vouch for. A run that cannot prove the modal
opened proves nothing about what the modal requests.

### N4 — ⚠️ `tsc` passed on code that crashed the component at mount

The crash in N3 was mine: the per-prompt reset `setPageFaviconUrl(params.get('favicon') || '')`
landed in the `showNotification` callback instead of inside `applyParams`, where `params` is
declared. At runtime: `ReferenceError: params is not defined`, React unmounted the whole overlay,
**every consent modal in the browser was dead**.

`npx tsc --noEmit` reported **no error**, before and after. There is exactly one `params` binding in
the file (line 611, inside `applyParams`), so why the reference type-checked is **not explained** —
recording that as an open question rather than inventing a mechanism.

⚠️ Note also that the stack said `BRC100AuthOverlayRoot.tsx:648:23`, and source line 648 is
`const cents = params.get('cents')` — *inside* `applyParams`, entirely valid. The line number refers
to the **transformed** module. Do not chase source lines from a vite dev stack trace.

⇒ Two green instruments (`tsc`, and the network watcher) and one broken product. The console was the
only thing that saw it. **Check that the thing under test is alive before believing what it reports.**

---

## Row 2 — the local favicon store (omnibox / new tab / bookmarks)

**SUBJECT.** Dev build after a full rebuild, `HODOS_DEV=1`, CDP 9322. Each surface attached **by URL**
and reloaded with `Page.reload{ignoreCache:true}`; `netwatch_page.py` counts
`Network.requestWillBeSent` and warns when no load event is seen, so a run that did not actually
reload is not mistaken for a clean one (the N3 lesson, applied up front).

### N5 — 🔴 RED, observed: **32 third-party favicon requests on one new-tab open**

`git stash` of `NewTabPage.tsx` only, everything else fixed, one reload:

```
https://icons.duckduckgo.com/ip3/now.bsvblockchain.tech.ico
https://icons.duckduckgo.com/ip3/x.com.ico
https://icons.duckduckgo.com/ip3/www.cnn.com.ico
https://icons.duckduckgo.com/ip3/www.youtube.com.ico
https://www.google.com/s2/favicons?domain=github.com&sz=32
…
THIRD-PARTY FAVICON HOSTS: 32  <-- LEAK
```

🚨 **Opening a new tab sent the owner's actual most-visited sites to two third parties.** Note both
`icons.duckduckgo.com` **and** `google.com`: `buildFaviconUrl()` picked the service from the user's
**search-engine** setting, so choosing DuckDuckGo for privacy only changed *which* company received
the browsing profile. That was not in the ticket — the ticket names Google only.

### N6 — 🟢 GREEN, after

| surface | total requests | non-local | third-party favicon hosts |
|---|---|---|---|
| new tab | 127 | **0** | **0** |
| bookmarks | 132 | **0** | **0** |

⚠️ **Omnibox is NOT yet measured** — the overlay is created lazily and had no CDP target while these
runs were made, so there is no observed RED or GREEN for it. Its code path is the same `<img src>`
swap, but that is a **code reading**, not a measurement. Recorded as owed, not as passed.

### N7 — the store, end to end

Visited `https://github.com` in a real tab, then read the SQLite file directly:

| host | icon_url | bytes | width |
|---|---|---|---|
| `github.com` | `https://github.githubassets.com/favicons/favicon.svg` | 2364 | 64 |
| `127.0.0.1` | `http://127.0.0.1:5137/Hodos_Gold_Icon.svg` | 4552 | 64 |

`OnFaviconURLChange` → `DownloadImage(is_favicon=true)` → `CefImage::GetAsPNG` → `FaviconStore::Put`.
Note it stored a **PNG rendered from an SVG** favicon — CEF decodes and re-encodes, so the store is
format-independent.

Read path, one batched IPC from the bookmarks overlay:

```
favicon_get ['github.com', 'never-visited-example.test']
  -> { "github.com": "data:image/png;base64,iVBORw0KGg…" }   (3174 chars)
```

⭐ `never-visited-example.test` is **omitted from the map**, not returned empty and not substituted —
so the React fallback is its own initial-letter tile, and there is no code path back to a remote
lookup for a site the user has not visited. That is the case that matters: an unvisited site is
exactly the one whose name would have been most revealing to leak.

⚠️ **Known cosmetic wart:** the store also keeps a row for `127.0.0.1` (our own internal pages). It is
harmless and unused, but it is noise in a privacy-relevant table; skipping loopback hosts on write is
a one-line follow-up, deliberately not smuggled into this build.

### N8 — 🚨 `npx tsc --noEmit -p tsconfig.json` is a BLIND instrument in this repo

It reported **clean** on code the real build rejects. `npm run build` (`tsc -b`, what `T1d` runs)
found four errors the `--noEmit` invocation did not:

```
BookmarksOverlayRoot.tsx(27,69): TS6133 'url' is declared but its value is never read.
NewTabPage.tsx(34,10):          TS6133 'saveCachedTiles' is declared but its value is never read.
OmniboxOverlayRoot.tsx(15,9):   TS6133 'favicons' is declared but its value is never read.
OmniboxOverlayRoot.tsx(244,52): TS2304 Cannot find name 'favicons'.
```

The last one is a genuine scope bug — the hook was in `OmniboxOverlayRoot` while the icon was
rendered inside `SuggestionItem`, a sibling component. **The omnibox would not have worked at all.**

⇒ Use `npm run build` (or `preflight -Full`) to check TypeScript. ⛔ Do not trust
`npx tsc --noEmit -p tsconfig.json` — it is the fourth instrument this phase that returned a green
it had no basis for (see N3, and the vacuous `showNotification` runs).

⭐ The `saveCachedTiles` error was load-bearing, not lint noise: removing the favicon pre-fetch
orphaned the only writer of the new-tab tile cache, while `getCachedTiles()` kept reading it. Left
alone, the NTP would have painted permanently stale tiles. The unused-symbol error was the only
thing that pointed at it.
