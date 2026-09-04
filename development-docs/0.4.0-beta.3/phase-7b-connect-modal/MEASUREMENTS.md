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

### N9 — 🚨 OWNER-REPORTED REGRESSION: every new-tab favicon was broken

> *"All of the favicons on the new tab are broken/not displaying"* — owner, 2026-09-04

**My bug, and my own test could not see it.** N6 proved no request LEAKED. It proved nothing about
whether an icon RENDERED — the two are independent, and a page that renders nothing at all passes
the leak test perfectly. This is the same shape as N3: an instrument that only checks the absence of
a bad thing will happily bless the absence of the good thing too.

Two causes, both mine:

1. **`<img src="">`.** The tile fell back to `''` when the store had no icon. An empty `src` does not
   reliably fire `error` in Chromium, so the existing `onError` hide never ran — every tile drew a
   broken/blank image. Bookmarks and the omnibox already rendered a globe/history glyph instead of an
   `<img>` in that case; the NTP had never needed a fallback because it always had a URL.
2. **Cold start is normal, not a bug.** The store learns an icon only when the user VISITS a site, so
   a freshly created store legitimately holds none for historical top sites. At the time of the
   report it held **2 rows**; the NTP was showing 8 tiles.

Fix: a `TileIcon` component — real favicon when we hold one, otherwise a **letter square**. The letter
square is the permanent fallback, not a stopgap. ⛔ It must never be "fixed" by fetching from a remote
favicon service; that is the leak this phase removed.

Measured after, same page: `imgs: 1, imgsRendering: 1, imgsBrokenOrEmpty: 0, letterTiles: 7` — one
real icon (github.com, the only site visited since the store existed) and seven letter tiles. Icons
fill in as the user browses.

## Row 3 — full favicon sweep across the app (owner request)

> *"Can we double check all the favicons throughout the app because we use them in quite a few
> spaces."* — owner, 2026-09-04

Every `<img>` in `frontend/src` was enumerated, not just the four surfaces the ticket named.

| Surface | Icon source | Third party? | Fallback when absent |
|---|---|---|---|
| Consent modals (`BRC100AuthOverlayRoot`) | `Tab::favicon_url` via C++ | **No** | domain-initial avatar ✅ |
| New tab (`NewTabPage`) | our store (data URI) | **No** | letter square ✅ *(fixed in N9)* |
| Bookmarks (`BookmarksOverlayRoot`) | our store | **No** | globe glyph ✅ |
| Omnibox (`OmniboxOverlayRoot`) | our store | **No** | history glyph ✅ |
| Tab strip (`TabComponent`) | `tab.favicon` — the **site's own** icon URL | No third party | globe glyph ✅ |
| Tab drag ghost (`TabBar`) | same | No third party | grey circle ✅ |
| Tab list (`TabListOverlayRoot`) | same | No third party | globe glyph ✅ |
| Recently closed (`TabListOverlayRoot`) | — carries only `{url, title}` | none rendered | n/a ✅ |
| WhatsOnChain marks (Activity/Dashboard/Tokens tabs) | local `/whatsonchain.png` | **No** | n/a ✅ |

**No third-party favicon service remains anywhere in the app.** The only surviving hits for
`s2/favicons` / `duckduckgo` / `gstatic` are the comments recording what was removed.

### 🆕 Two remote-image loads found by this sweep that are NOT favicons

⛔ Neither is fixed here — both are outside this phase's fence, and one is on the money path
(Phase 8). Reported rather than silently changed.

| Where | What | Why it matters |
|---|---|---|
| `TransactionForm.tsx` — `<img src={s.avatar_url}>` and `<img src={paymailInfo.avatar_url}>` | **Paymail avatars**, loaded from a URL the paymail provider supplies | 🚨 Fires **as the user types a recipient** and again when a paymail resolves. The avatar host learns *"this user is preparing a payment to this person, now."* Same defect class as the consent-modal favicon, on the **payment** surface, and the URL is chosen by a third party rather than being a fixed service we control — so it doubles as a per-user tracking pixel a provider could mint deliberately |
| `CertificatesTab.tsx` — `<img src={value}>` for an avatar-shaped certificate field | Remote image from a **certificate field value** | Viewing your own certificates discloses that visit to whatever host the issuer named |

⭐ The pattern worth naming: **any place we render a URL someone else chose, we hand that someone a
timestamped signal about the user.** Favicons were the instance we knew about; these are the same
instance wearing different clothes. A standing check — "does this surface render a remote URL we did
not author?" — belongs in the review criteria, not in one ticket.

### ⚠️ Tab strip: first-party, but worth a decision

The tab strip, drag ghost and tab list render `Tab::favicon_url` directly — e.g.
`https://github.githubassets.com/favicons/favicon.svg`. That is **not** a third-party favicon service;
it is the site's own asset host, for a site already open in that tab.

But it is still a request per tab, and **session restore fires one for every restored tab at
once** — telling N sites' CDNs "this browser just started". Pointing these three at the store instead
would remove that entirely and make them work offline. Not done here: it is a behaviour change to the
tab strip, which is not what this phase was scoped to touch.

---

## Row 4 — the one-view merge (`P7b-A3` / `A5` / `A6`)

**SUBJECT.** Notification overlay browser, attached by URL, driven through
`window.showNotification` — the path C++ uses. Rig: `merged_view_probe.py`.

⛔ **The fixture protocol is security level 1**, deliberately: `[1, "3241645161d8"]` (BRC-29,
payment-key derivation) described as *"Show your profile picture."* A **level-2** fixture would have
had its identifier printed by the old counterparty footnote, so it passes whether or not this change
works. That is the vacuous test this row exists to avoid.

### N10 — 🔴 RED, observed (pre-merge code via `git stash`)

```
protocolIdShown          False   <- "3241645161d8" appears NOWHERE on screen
deceptivePurposeShown    True    <- "Show your profile picture." does
ticked                   2       <- only identity + quiet mode; no per-item ticks
hasCustomizeButton       True
protectedBasketMark      False   <- the protected-basket warning was behind the click too
RESULT: FAIL
```

🚨 The ticket's attack, reproduced exactly: the site's sentence was the **only** thing on screen
describing its own grant, and the real scope was invisible.

### N11 — 🟢 GREEN, after

```
checkboxes 7   ticked 6   disabled 4      <- per-item ticks on the FIRST screen, all on by default
hasCustomizeButton        False           <- one view; no second wording to drift from
protocolIdShown           True            <- [1] 3241645161d8 beside the site's sentence
level0IdShown             True            <- level 0 too, not just the level the footnote covered
identityAcrossMetanet     True
quietModeFullLabel        True            <- "including ones it did not list above"
quietModeCallout          True            <- the ticks-are-inert warning, beside the ticks
protectedBasketMark       True
counterpartyFootnoteGone  True
allowWithoutLimits        True            <- capability preserved, see the flag below
infoIcons                 2               <- identity + quiet-mode tooltips, which Customize NEVER had
RESULT: PASS
```

`disabled 4` is correct: quiet mode is on by default, which disables the protocol and basket ticks
(they would be inert), and the protected `default` basket is disabled independently.

### N12 — `domain_approval`, the third view (`P7b-A6`)

```
identityAcrossMetanet True   quietFullLabel True
oldIdentityLabel      False  oldQuietLabel  False   infoIcons 2
```

All three views — the merged connect screen and `domain_approval` — now carry the same label **and**
the same tooltip. There is no fourth.

### N13 — geometry still holds (`P7b-A8`)

Merged view at 6 declared permissions: card **897 px** in a 1032 px viewport, top 68, Connect fully
visible, `maxHeight 944px`, `overflowY auto`. 7a's cap is the backstop and is not being leaned on.

### Owed, and not claimed

- ⬜ **`P7b-A4` — that an unticked item actually fails to persist.** The probe proves the ticks
  *render*; it does not prove that unticking one leaves no row in `domain_protocol_permissions`.
  That needs a real connect against a real dApp. The handlers and state were carried over from
  Customize **unchanged**, so the risk is low — but "unchanged code" is a reading, not a measurement,
  and this is the permission path.
- ⬜ **`P7b-A2` omnibox** — still unmeasured from Row 2.
- ⬜ **👤 the owner reading the merged screen.** Every consent defect this sprint was found that way
  and none by a gate.

### 🙋 Owner decision surfaced by the merge

**"Allow without limits"** (raises caps to $1000/tx, $10000/session) lived behind the Customize
click. With one view it is in front of every user. Kept — silently deleting a spending control is
not mine to do — but promoting one is not either. Options in `PHASE_CONTRACT.md` §6.

### N14 — layout order + folding "Allow without limits" (owner, 2026-09-04)

**The reason for the order, stated so it survives a future refactor:** quiet mode **governs** the
list beneath it. While it is on — the default — every tick in that list is inert, because
`decide_scoped_grant` returns `Silent` on `bundled_scope_grant` before it ever consults the V18 rows
those boxes write. The previous order put the greyed-out list **above** the control that greyed it,
so the user met the effect before the cause.

New order: **identity + quiet-mode toggles → the quiet-mode callout → "This site is asking permission
to:" → the itemised list → limits → Decline / Connect.**

Measured on the rendered screen (character offsets in `body.innerText`):

```
identity  42   callout 204   "asking permission to" 363   list item 398   limits 538   Connect 595
```

⚠️ **The cost, recorded rather than glossed:** two pre-ticked broad grants now sit above the
itemisation, so a fast user can reach Connect having read only those. That was already true of a
user who skimmed the list; the callout sitting directly under quiet mode is the mitigation. It is a
trade, not a free win.

**"Allow without limits" is folded into the limits disclosure**, resolving the decision `P7b` flagged.
Collapsed: absent from the DOM. Expanded: present at offset 852, inside the limits section (538) and
above Connect (1040). The capability is exactly as reachable as before the merge — one click — without
putting the widest spending control on the screen in front of a user who never asked about limits.

`merged_view_probe.py` now asserts both: `orderToggleFirst`, `orderCalloutBeforeList`, and that
`Allow without limits` is **absent** while limits are collapsed. All PASS.

### N15 — ⚠️ a preflight FAIL that was not a failure

`preflight -Full` reported `T1d ... npm run build exited 143`. **143 is SIGTERM** — the build was
killed, not broken. The dev browser, dev wallet and vite were all running and competing for CPU; the
same build took 2m49s standalone. Stopping the dev stack and re-running gave a clean PASS.

⛔ Worth knowing before someone "fixes" a build that is not broken: read the exit code. 143/137 mean
something killed it. This is the mirror image of the session's other lesson — a green that means
nothing, and here a red that means nothing.
