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
