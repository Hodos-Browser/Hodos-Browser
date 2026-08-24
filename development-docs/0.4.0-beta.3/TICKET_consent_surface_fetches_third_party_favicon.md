# Consent surfaces fetch favicons from google.com

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
