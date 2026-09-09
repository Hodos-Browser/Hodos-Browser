# Consent surfaces fetch favicons from google.com

**Status:** 🔴 **OPEN — still live, verified 2026-08-31.** `BRC100AuthOverlayRoot.tsx` still fetches `https://www.google.com/s2/favicons?domain=<site>`, so every consent prompt tells Google which site the user is connecting a wallet to, at the moment of the privacy decision. Labelled 2026-08-31 (had no status line).
**Sprint:** 📌 **Phase 7 (consent surface)** — bundled 2026-08-31; see `SPRINT_PLAN.md` §4.1.
**Approach: ✅ DECIDED by the owner, 2026-08-31 — use the page's own favicon.**

> ⭐ Why this is the right call and not just the cheap one. We **already have** the page's favicon:
> `Tab::favicon_url` is populated from `OnFaviconURLChange` and the tab strip renders it. So the
> page's own icon is (a) **more private** — no third-party request at all, (b) **more accurate** —
> it is the icon of the site actually being consented to, not whatever Google has cached for that
> domain, and (c) **already fetched**, so it costs nothing new.
>
> ⚠️ Do not "solve" this by proxying the Google request through our own backend — that keeps the
> third party and adds a hop. ⚠️ A bundled generic icon is the acceptable **fallback** when the page
> has no favicon, and is what the existing `faviconError` state should fall back to instead of the
> current behaviour.
>
> 🎯 **Assert the right subject when testing:** the check is that **no request leaves the machine for
> `google.com`** when a consent prompt opens — not merely that an icon renders. An icon renders today.

**Opened 2026-08-24** (noticed while building the Phase 0.9 prompt). **Status:** 🔵 OPEN. Pre-existing,
not introduced by 0.9.

## The defect

`frontend/src/pages/BRC100AuthOverlayRoot.tsx` renders, on multiple modal branches:

```tsx
<img src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=32`} … />
```

So every time a Hodos consent modal appears — permission prompt, domain approval, payment
confirmation, rate limit, certificate disclosure — the browser makes a request to **Google**,
disclosing the domain the user is being asked to approve.

## Why it matters

1. It is a third-party network call from a **privacy** browser, on its most sensitive surface.
2. The leaked value is high-signal. Not merely "visited a site", but "was asked to grant this site
   wallet or device access, at this moment".
3. It happens whether or not the user approves, and whether or not they even read the modal.

## Options

- Use the favicon Chromium already holds for the tab — no new network request.
- Fall back to the existing `avatarStyle` domain-initial, which is already the `onError` path, so the
  UI degrades gracefully today.
- Proxy through our own layer if a remote fetch is genuinely wanted.

## Negative control

Watch outbound requests while a consent modal opens. Before: a request to `google.com/s2/favicons`
carrying the domain. After: none.

⛔ Assert on the **network**, not on whether an icon renders. An icon can render from cache while the
request still fires, and it can fail to render for reasons unrelated to the fix.

---

## 🆕 2026-09-09 (macOS) — the fix landed, and a **residual third-party request** remains. Owner decision needed.

**The original defect is gone.** 📏 Measured on macOS: a consent modal opens and makes **0 requests**
to `google.com` / `gstatic.com` / `duckduckgo.com`, with the modal proven mounted (its fixture text
read out of the DOM, not just a `SHOWN` return). New tab likewise: **125 requests, 0 non-local**.
`phase-7b-connect-modal/PHASE_CONTRACT.md` §4c.

**But the shipped path does not meet this ticket's stated goal of "no third-party request at all",**
for any site whose declared icon lives on a different host. Each link evidenced separately:

| # | Claim | Type |
|---|---|---|
| 1 | `FaviconParamForDomain()` (`cef-native/src/core/HttpRequestInterceptor.cpp:721-726`) builds `&favicon=` from `TabManager::GetFaviconUrlForHost(host)` — the site's own **remote** icon URL, verbatim | CODE_READING |
| 2 | The overlay renders it directly: `<img src={pageFaviconUrl}>` (`frontend/src/pages/BRC100AuthOverlayRoot.tsx:566`, `:1525`). No `favicon_get`, no store | CODE_READING |
| 3 | 📏 A `domain_approval` modal fed `favicon=https://favicon-probe.invalid/icon.png` issued **1 non-local request to that host**. Modal mounted; `onError` then drew the Hodos fallback — the graceful path works | **MEASURED** |
| 4 | 📏 `favicons.db`: `www.google.com` declares its icon at `https://www.gstatic.com/images/branding/searchlogo/ico/favicon.ico` — **a different host** | **MEASURED** |

⇒ A real consent prompt for `google.com` fetches from **gstatic.com** at the moment of the decision;
likewise any CDN-hosted icon.

⚠️ **This is much smaller than the original leak and should not be described as the same defect.**
`s2/favicons?domain=X` told Google about *every* site the user was asked to trust. Here the icon host
learns only about its own site, which it already serves. The ticket's rationale (a) — *"no
third-party request at all"* — is nonetheless not met.

⭐ **A fix now exists that did not when this ticket was written.** `FaviconStore` (Phase 7b, and
initialised on macOS since 2026-09-08) holds the PNG bytes locally, and `favicon_get` already serves
them to the new tab as `data:` URIs — 📏 proved by `google.com`'s new-tab tile decoding to exactly the
1391 bytes stored for that host. Routing the consent modal through the same call would make the
request genuinely zero, and would fall back to the domain-initial avatar for hosts with no stored
icon, exactly as today.

⚠️ **What is NOT proven, stated so it is not over-read:** whether Chromium's HTTP cache would satisfy
the real request without touching the network. The probe used an unresolvable host, so *"a request is
issued"* is measured; *"packets leave the machine"* is **not**. The rationale in this ticket assumed
the icon is "already fetched, so it costs nothing new" — that assumption has never been tested either.

⛔ **No production code was changed** — this is the wallet's consent surface and the call is the
owner's (HARNESS §6: evidence pointing at production code stops and asks).
