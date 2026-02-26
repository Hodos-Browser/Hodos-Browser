# Test Site Basket — Standard Verification Sites

**Purpose**: Use this basket for comprehensive testing of browser features.
**Rule**: When building or fixing a feature, test against the relevant category (minimum) or full basket (thorough).

---

## Category: Authentication & Login (SSL/Cookies Critical)

| Site | Why | Tests |
|------|-----|-------|
| x.com (Twitter) | Complex auth flow, frequent login issues | SSL certs, session cookies, OAuth redirects |
| google.com → accounts.google.com | OAuth hub for many services | SSL, cookie handling, FedCM (future) |
| github.com | Developer-focused, good cert practices | Standard login, session persistence |
| reddit.com | Heavy cookie usage, OAuth options | Login, session, cookie consent |
| discord.com (web) | WebSocket-heavy, complex auth | SSL, cookies, WebSocket connections |
| amazon.com | E-commerce, strict security | Login, payment flows (visually verify) |

## Category: Video/Media (Ad Blocking, Playback)

| Site | Why | Tests |
|------|-----|-------|
| youtube.com | Primary video target, complex ads | Ad blocking (pre-roll, mid-roll), playback, recommendations |
| twitch.tv | Live streaming, different ad delivery | Ad blocking, live playback, chat |
| vimeo.com | Cleaner video player, fewer ads | Basic playback, embed handling |
| netflix.com | DRM (Widevine), premium content | DRM support (if implemented), playback |
| spotify.com (web player) | Audio streaming, DRM | Audio playback, ad blocking |

## Category: News/Content (Ad Blocking, Tracking)

| Site | Why | Tests |
|------|-----|-------|
| nytimes.com | Paywalled, heavy tracking | Ad blocking, tracker blocking, paywall detection |
| bbc.co.uk | No paywall, heavy ads | Ad blocking, EU cookie notices |
| theverge.com | Tech news, moderate ads | Ad blocking, content layout |
| medium.com | Metered paywall, clean design | Session tracking, paywall behavior |
| reddit.com | Infinite scroll, dynamic content | Ad blocking in feed, lazy loading |

## Category: E-commerce (Cookies, Sessions, Cart)

| Site | Why | Tests |
|------|-----|-------|
| amazon.com | Dominant, complex sessions | Cart persistence, login session |
| ebay.com | Auction timers, session-critical | Session persistence, timer accuracy |
| etsy.com | Smaller scale, good test case | Standard e-commerce flow |

## Category: Productivity/Web Apps (Permissions, Complex JS)

| Site | Why | Tests |
|------|-----|-------|
| docs.google.com | Heavy JS, complex auth | Performance, auth, copy/paste |
| notion.so | Modern web app | Performance, sessions |
| figma.com | Canvas rendering, heavy app | Performance, WebGL (if used) |
| zoom.us (web) | Camera/mic permissions | Permission prompts, WebRTC |
| meet.google.com | Video conferencing | Camera/mic, WebRTC |

## Category: Developer/Technical (Edge Cases)

| Site | Why | Tests |
|------|-----|-------|
| github.com | Code rendering, file downloads | Syntax highlighting, download handling |
| stackoverflow.com | Heavy community content | Search, navigation |
| localhost:* | Local development | Dev server access, HMR |
| codepen.io | Iframes, sandboxed execution | iframe handling, JS execution |

## Category: BSV/Crypto (Our Domain)

| Site | Why | Tests |
|------|-----|-------|
| whatsonchain.com | BSV block explorer | Basic browsing, JSON responses |
| handcash.io | BSV wallet/paymail | OAuth flows, API calls |
| relayx.com | BSV marketplace | Wallet integration testing |

---

## Quick Test Workflow

### Minimal (5 min) — After any browser-core change:
1. youtube.com — video plays, ads blocked
2. x.com — can log in
3. github.com — pages load correctly

### Standard (15 min) — After sprint completion:
- All of "Authentication" category
- 2-3 from "Video/Media"
- 1-2 from "News/Content"

### Thorough (30-45 min) — Before release/demo:
- Full basket, all categories
- Note any issues in sprint working notes

---

## Updating This Document

When adding a new feature, ask:
- What sites exercise this feature?
- Add them to the appropriate category

When a site changes or becomes irrelevant:
- Remove or replace it
- Note why in commit message

---

*Last updated: 2026-02-25 (Sprint 9 planning)*
