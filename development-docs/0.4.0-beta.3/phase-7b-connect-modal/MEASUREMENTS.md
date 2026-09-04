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
