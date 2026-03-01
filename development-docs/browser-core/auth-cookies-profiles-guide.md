# Authentication, Cookies & Profiles — Comprehensive Reference

**Created**: 2026-02-26
**Purpose**: Deep reference for how web authentication works, how Brave handles it, how we handle it, and the gaps that cause sign-in failures.

---

## Table of Contents

1. [How Web Authentication Actually Works](#1-how-web-authentication-actually-works)
2. [How Brave Browser Handles It](#2-how-brave-browser-handles-it)
3. [How Hodos Browser Handles It (Current State)](#3-how-hodos-browser-handles-it-current-state)
4. [Gap Analysis: Hodos vs Brave](#4-gap-analysis-hodos-vs-brave)
5. [Known Sign-In Failures & Root Causes](#5-known-sign-in-failures--root-causes)
6. [Architecture Deep Dive](#6-architecture-deep-dive)
7. [Action Items](#7-action-items)

---

## 1. How Web Authentication Actually Works

### 1.1 Three Authentication Systems (Google)

Google maintains three parallel sign-in systems. As of August 2025, **FedCM is mandatory** for new Google Sign-In integrations.

| System | Era | How It Works | Cookie Requirements |
|--------|-----|-------------|-------------------|
| **OAuth 2.0 (Traditional)** | 2012+ | Full-page redirects through accounts.google.com | First-party cookies on accounts.google.com; third-party cookies during redirect chain |
| **Google Identity Services (GSI)** | 2022+ | JavaScript SDK loads iframe from accounts.google.com, shows One Tap popup | Third-party cookies from accounts.google.com embedded in the site's page |
| **FedCM** | 2024+ (mandatory Aug 2025) | Browser-native account picker dialog — no iframes, no redirects | No third-party cookies needed — browser mediates directly |

**Key insight**: Traditional OAuth (what x.com uses) requires a redirect chain:
```
x.com → accounts.google.com/o/oauth2/auth → (user signs in) → accounts.google.com/o/oauth2/approve → x.com/callback
```
During this chain, cookies must persist across:
- `accounts.google.com` (first-party during the auth page)
- `x.com` (needs to read the OAuth callback)
- Various Google subdomains (`ssl.gstatic.com`, `apis.google.com`, `accounts.gstatic.com`)

### 1.2 Google's Cookie Architecture

Google maintains parallel cookie sets:

| Cookie | Scope | Purpose |
|--------|-------|---------|
| `__Secure-1PSID` | First-party, `SameSite=None` | Primary session ID |
| `__Secure-3PSID` | Third-party variant | Cross-site session (used by GSI iframe approach) |
| `__Secure-1PAPISID` | First-party | API authentication |
| `__Secure-3PAPISID` | Third-party | Cross-site API auth |
| `SAPISID` | `.google.com`, `SameSite=None` | General API auth (SAPISIDHASH) |
| `SID`, `HSID`, `SSID` | `.google.com` | Legacy session cookies |
| `NID` | `.google.com` | Preferences + anti-abuse |

**Critical**: The `__Secure-1P*` cookies are what make Google Sign-In work in first-party context. If these are blocked, users see a **blank white screen** at accounts.google.com.

### 1.3 Google Required Subdomains

These must NOT be blocked by ad blocking or cookie filtering for sign-in to work:

| Domain | Purpose | Type |
|--------|---------|------|
| `accounts.google.com` | OAuth UI, sign-in form | Page |
| `ssl.gstatic.com` | Static assets (CSS, JS, images) for login page | Resource |
| `accounts.gstatic.com` | Profile photos, account picker assets | Resource |
| `apis.google.com` | GSI JavaScript SDK | Script |
| `www.gstatic.com` | reCAPTCHA assets | Script |
| `fonts.googleapis.com` | Web fonts (login page) | Resource |
| `www.googleapis.com` | Token exchange API | XHR |
| `oauth2.googleapis.com` | OAuth 2.0 token endpoint | XHR |
| `content-autofill.googleapis.com` | Autofill service | XHR |
| `play.google.com` | Sometimes loaded in auth flow | Resource |

### 1.4 FedCM (Federated Credential Management)

FedCM is a **browser-native** API that replaces iframe/cookie-based federated auth:

```
1. Site calls navigator.credentials.get({ identity: { providers: [...] } })
2. Browser shows native account picker dialog (NOT an iframe)
3. Browser contacts identity provider directly (e.g., accounts.google.com)
4. Browser returns credential token to site JavaScript
```

**Why this matters**: FedCM doesn't need third-party cookies at all. The browser mediates directly. CEF 136 (Chromium 136) should inherit FedCM support, but:
- FedCM shows a **native browser UI dialog** — CEF may or may not render this
- If CEF doesn't support FedCM UI, sites using GSI will show **nothing** (blank/broken)
- Traditional OAuth redirect flow still works regardless of FedCM

### 1.5 X.com (Twitter) Authentication

X.com uses **traditional OAuth 2.0** with Google as one option:

```
1. User clicks "Sign in with Google" on x.com
2. Redirect to accounts.google.com/o/oauth2/auth?client_id=...&redirect_uri=https://api.twitter.com/...
3. User authenticates at accounts.google.com (blank screen if broken)
4. Google redirects to api.twitter.com/account/login_challenge (or similar callback)
5. X.com sets its own cookies: auth_token, ct0, twid
```

**X.com minimum cookies for authenticated session:**
- `auth_token` — Session authentication (`.x.com` or `.twitter.com`)
- `ct0` — CSRF protection token
- `twid` — Twitter user ID

### 1.6 Blank Screen Causes (accounts.google.com)

When a user clicks "Sign in with Google" and gets a blank white page, the causes are:

| Cause | Likelihood | Detection |
|-------|-----------|-----------|
| **JS resources blocked** (gstatic.com, googleapis.com) | HIGH | Network tab shows blocked requests to ssl.gstatic.com |
| **Third-party cookies blocked** during redirect | MEDIUM | OAuth callback fails silently |
| **CSS resources blocked** (cosmetic filtering or network) | MEDIUM | Page loads but is invisible (white text on white bg) |
| **COOP headers** (`Cross-Origin-Opener-Policy: same-origin`) | LOW | Popup window loses reference to opener |
| **Fingerprint protection breaking CAPTCHA** | MEDIUM | reCAPTCHA fails to load or triggers endless challenges |
| **CSP violations from injected scripts** | MEDIUM | Console errors about Content-Security-Policy |
| **User-Agent rejected** | LOW | Google's `disallowed_useragent` blocks WebView UAs (NOT CEF with Chrome UA) |

---

## 2. How Brave Browser Handles It

### 2.1 Brave Shields Architecture

Brave uses a layered approach:

| Layer | What It Does | User Control |
|-------|-------------|-------------|
| **Network blocking** | Blocks ad/tracker HTTP requests | Per-site: Standard / Aggressive / Off |
| **Cookie blocking** | Ephemeral partitioned storage for third-party | Per-site toggle |
| **Cosmetic filtering** | CSS hide rules | Part of Shields toggle |
| **Fingerprint farbling** | Deterministic noise injection | Per-site: Standard / Off |
| **De-AMP** | Redirects AMP pages to original | Global |
| **Bounce tracking** | Strips tracking redirects | Global |

### 2.2 Brave's Cookie Strategy (Key Difference from Us)

Brave does NOT simply "block third-party cookies." Instead:

**Ephemeral Partitioned Storage:**
1. Third-party cookies are blocked at the **HTTP level** (network stack)
2. But JavaScript `document.cookie` and Storage APIs get **partitioned ephemeral storage**
3. This ephemeral storage is scoped to the `(top-level site, third-party site)` pair
4. When the **last tab** of a top-level site closes, ephemeral storage is **destroyed** after a **30-second grace period**

**Why the 30-second grace period matters:**
- OAuth redirects temporarily navigate away from the original site
- Without the grace period, the redirect back would find the storage gone
- The 30-second window allows: `site.com → accounts.google.com → site.com` to work

**How this differs from our approach:**
- We use a binary **allow/block** model with AUTH_COOKIE_DOMAINS allowlist
- Brave gives every domain partitioned storage — no allowlist needed for basic functionality
- Brave's approach is more robust for edge cases we haven't thought of

### 2.3 Brave's Google Sign-In Handling

Brave has a **specific Google Sign-In permission**:

1. When a site uses Google Sign-In (GSI) and Shields blocks the required cookies
2. Brave detects the GSI SDK and shows a prompt: "Allow Google Sign-In on this site?"
3. If user allows, Brave adds an exception for `accounts.google.com` cookies on that specific site
4. This is NOT a global setting — it's per-site

**FedCM in Brave:**
- Brave inherited FedCM from Chromium
- When FedCM is available, Brave uses the browser-native dialog (no cookie issues)
- For sites using traditional OAuth, Brave relies on its ephemeral storage + grace period

### 2.4 Brave's Fingerprint Farbling

Identical in concept to ours (Sprint 12), but with some differences:

| Aspect | Brave | Hodos |
|--------|-------|-------|
| Session token | Random at startup, memory-only | Same (FingerprintProtection.h) |
| Per-domain seed | HMAC-SHA256(token, eTLD+1) | Custom hash (token XOR domain) |
| Canvas | Noise injection on readback | Same |
| WebGL | Generic vendor/renderer | Same |
| Navigator | Override hardwareConcurrency, deviceMemory | Same |
| Audio | Noise on getFloatFrequencyData | Same |
| Screen | **Removed** (too much breakage) | Never added (same reasoning) |
| Auth bypass | Brave detects auth domains | `IsAuthDomain()` + `hodos-unbreak.txt` |

### 2.5 Brave's Profile Isolation

- Each profile is a complete Chromium profile directory
- Separate cookie stores, history, bookmarks, extensions
- Password manager is per-profile
- Brave Rewards (BAT) is per-profile
- **Wallet (crypto)** is per-profile in Brave (we share wallet across profiles)

---

## 3. How Hodos Browser Handles It (Current State)

### 3.1 Cookie Blocking Architecture

```
HTTP Request arrives
    │
    ▼
CookieAccessFilterWrapper (IO thread)
    │
    ├── CanSendCookie(request, cookie)
    │     1. Skip localhost → ALLOW
    │     2. Check blocked_domains_ set → BLOCK if match
    │     3. Check AUTH_COOKIE_DOMAINS → ALLOW if match (bypass everything)
    │     4. Check IsThirdParty() → BLOCK if third-party
    │     5. Otherwise → ALLOW
    │
    └── CanSaveCookie(request, cookie)
          (Same logic as CanSendCookie)
```

### 3.2 AUTH_COOKIE_DOMAINS (Current)

```cpp
static const std::unordered_set<std::string> AUTH_COOKIE_DOMAINS = {
    // Google
    "google.com", "accounts.google.com", "myaccount.google.com",
    "googleapis.com", "gstatic.com", "youtube.com",
    // Microsoft
    "microsoft.com", "login.microsoftonline.com",
    "login.live.com", "login.microsoft.com",
    // Apple
    "apple.com", "appleid.apple.com", "icloud.com",
    // Social
    "facebook.com", "github.com", "githubusercontent.com",
    // Financial
    "amazon.com", "paypal.com", "stripe.com",
    // Twitter / X
    "x.com", "twitter.com", "twimg.com",
};
```

**Matching logic**: `IsAuthCookieDomain(cookie_domain)` does:
1. Strip leading dot from cookie domain (`.google.com` → `google.com`)
2. Exact match against AUTH_COOKIE_DOMAINS
3. Suffix match: check if cookie domain ends with `.` + any auth domain
4. If match → **always allow** (bypass blocked list AND third-party check)

### 3.3 Fingerprint Protection Auth Bypass

`FingerprintProtection::IsAuthDomain()` exempts these from farbling:

```cpp
"accounts.google.com", "myaccount.google.com",
"login.microsoftonline.com", "login.live.com", "login.microsoft.com",
"appleid.apple.com", "github.com", "www.facebook.com",
"discord.com", "x.com", "twitter.com"
```

### 3.4 Adblock Exception List (hodos-unbreak.txt)

Disables **scriptlet injection** (`#@#+js()`) and **cosmetic CSS** (`$elemhide`) on:

- **Twitter/X**: `x.com`, `twitter.com`, `api.twitter.com`, `abs.twimg.com` + `$elemhide`
- **Google Auth**: `accounts.google.com`, `accounts.youtube.com`, `myaccount.google.com`
- **Microsoft Auth**: `login.microsoftonline.com`, `login.live.com`, `login.microsoft.com`
- **GitHub**: `github.com`
- **Apple**: `appleid.apple.com`
- **Facebook**: `www.facebook.com`
- **Discord**: `discord.com`
- **Reddit**: `reddit.com`, `www.reddit.com`
- **Banking**: `chase.com`, `bankofamerica.com`, `wellsfargo.com`, `paypal.com`, `stripe.com`
- **E-commerce**: `amazon.com`
- **OAuth SDK**: `@@||connect.facebook.net^*/sdk/$script`, `@@||apis.google.com/js/api.js$script`

### 3.5 Profile Architecture

```
%APPDATA%/HodosBrowser/
    ├── profiles.json                 # Profile metadata (id, name, color, avatar)
    ├── Default/                      # Default profile directory
    │   ├── settings.json             # SettingsManager
    │   ├── cookie_blocks.db          # CookieBlockManager
    │   ├── history.db                # HistoryManager
    │   ├── bookmarks.json            # BookmarkManager
    │   ├── adblock_site_settings.json # AdblockCache per-site toggle
    │   ├── cache/                    # CEF cache (HTTP, DNS, etc.)
    │   └── ...                       # CEF-managed profile data
    ├── Profile_1/                    # Additional profile
    │   └── (same structure)
    └── adblock/                      # Shared across profiles
        ├── *.txt                     # Filter lists
        └── engine.bin                # Compiled engine
```

### 3.6 Initialization Sequence (Critical Order)

```
1. ProfileManager::Initialize()
2. Parse --profile argument
3. Set current profile ID
4. Get profile cache path
5. Acquire ProfileLock (prevents multi-instance on same profile)
6. SettingsManager::Initialize(profile_path)
7. AdblockCache::Initialize(profile_path)
8. FingerprintProtection::Initialize()
9. Set CEF cache paths (root_cache_path, cache_path)
10. CefInitialize()
11. HistoryManager::Initialize(profile_path)
12. CookieBlockManager::Initialize(profile_path)
13. BookmarkManager::Initialize(profile_path)
```

### 3.7 Third-Party Detection (IsThirdParty)

Current logic (simple substring matching):

```cpp
bool IsThirdParty(cookie_domain, page_domain) {
    // Normalize: strip leading dots
    // Check: cookie == page → first-party
    // Check: cookie is subdomain of page → first-party
    // Check: page is subdomain of cookie → first-party
    // Otherwise → third-party
}
```

**Known limitation**: No public suffix list (PSL). Cannot correctly handle:
- `site.co.uk` vs `tracker.co.uk` (both end in `.co.uk`)
- `user.github.io` vs `other.github.io` (github.io is a public suffix)
- `user.blogspot.com` vs `other.blogspot.com`

---

## 4. Gap Analysis: Hodos vs Brave

### 4.1 Critical Gaps (Likely Causing Sign-In Failures)

| Gap | Impact | Severity |
|-----|--------|----------|
| **No adblock exception for Google resource domains** | `ssl.gstatic.com`, `accounts.gstatic.com`, `fonts.googleapis.com` may be blocked at the network level by EasyList/EasyPrivacy, causing blank page at accounts.google.com | **CRITICAL** |
| **No `$elemhide` for accounts.google.com** | Cosmetic CSS selectors may hide elements on the sign-in page | **HIGH** |
| **No FedCM verification** | If CEF 136 doesn't render FedCM's native dialog, GSI-based sign-in breaks silently | **HIGH** |
| **Aggressive third-party cookie blocking during OAuth redirects** | During `x.com → google → x.com`, cookies set at google.com are "third-party" from x.com's perspective even though google is the top-level page at that moment | **MEDIUM** — AUTH_COOKIE_DOMAINS should cover this, but redirect timing may be an issue |
| **No ephemeral/partitioned storage** | Unlike Brave, we have no fallback — if a cookie is blocked, it's gone | **MEDIUM** |

### 4.2 Important Gaps (Not Immediately Breaking)

| Gap | Impact |
|-----|--------|
| **No public suffix list** | IsThirdParty() gives wrong results for `.co.uk`, `.github.io`, etc. |
| **No Storage Access API handling** | Sites can't request cookie access via W3C standard (CEF 136 may handle natively) |
| **No Google Sign-In permission prompt** | Brave detects GSI and asks user; we have no such UI |
| **No grace period for ephemeral storage** | OAuth redirects have zero tolerance — no 30-second window |
| **Cookie lifetime not capped** | Brave caps JS cookies to 7 days, HTTP to 6 months; we don't |
| **No bounce tracking protection** | Tracking redirects like `tracker.com/bounce?url=target.com` work freely |

### 4.3 Things We Do Well

| Feature | Notes |
|---------|-------|
| Auth domain allowlist | Comprehensive and auto-bypasses both cookie blocking and third-party checks |
| Per-profile isolation | Separate CEF cache, cookie DB, settings, history |
| Fingerprint protection with auth bypass | Auth domains get no farbling |
| Scriptlet exception list | Prevents fetch/XHR proxying from breaking OAuth token exchange |
| Network-level ad blocking | CefResponseFilter for YouTube, adblock engine for general |

---

## 5. Known Sign-In Failures & Root Causes

### 5.1 X.com Google Sign-In → Blank Screen

**Symptom**: Click "Sign in with Google" on x.com, redirected to accounts.google.com, see blank white page.

**Root causes (most to least likely)**:

1. **Google static resources blocked by adblock engine**: EasyList/EasyPrivacy may block requests to `ssl.gstatic.com` or `www.gstatic.com` with rules targeting tracking scripts. Google's login page loads critical JS from these domains.

2. **No `$elemhide` exception for accounts.google.com**: Cosmetic CSS rules may be hiding the entire login form. We have `#@#+js()` (scriptlet exception) but NOT `$elemhide` (CSS exception) for accounts.google.com.

3. **Fingerprint protection breaking reCAPTCHA**: Google shows reCAPTCHA during sign-in. Canvas farbling or WebGL spoofing may cause reCAPTCHA to fail or loop. `accounts.google.com` is in `IsAuthDomain()` but `www.gstatic.com` (where reCAPTCHA loads from) is NOT.

4. **Scriptlet injection on a Google subdomain not in the exception list**: Some Google auth subdomains (e.g., `consent.google.com`, `myaccount.google.com`) may not be in `hodos-unbreak.txt` yet.

### 5.2 X.com Can't Load Images (Non-Ad Content Hidden)

**Symptom**: Scrolling x.com, normal user posts have no images; promoted tweets DO show images.

**Root cause**: EasyList CSS cosmetic selectors match x.com's organic tweet containers (which use the same DOM structure as ads). Promoted tweets use different DOM structures that don't match the selectors.

**Fix applied**: `@@||x.com^$elemhide` and `@@||twitter.com^$elemhide` in `hodos-unbreak.txt`.

### 5.3 Profile-Related Login Issues

**Symptom**: Login works in one profile but not another, or login state "leaks" between profiles.

**Root causes**:
- ProfileLock prevents multiple instances on the same profile (correct behavior)
- If user switches profiles without restarting, CEF cache may still reference old profile (this shouldn't happen with per-profile root_cache_path)
- New profiles inherit settings from creating profile — if creating profile had weird cookie allowances, new profile gets them too

---

## 6. Architecture Deep Dive

### 6.1 CookieBlockManager Database Schema

```sql
-- Per-profile: %APPDATA%/HodosBrowser/{Profile}/cookie_blocks.db

CREATE TABLE blocked_domains (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain TEXT NOT NULL UNIQUE,
    is_wildcard INTEGER DEFAULT 0,    -- 1 = matches *.domain
    source TEXT DEFAULT 'user',       -- 'user' or 'default' (DefaultTrackerList.h)
    created_at INTEGER NOT NULL
);

CREATE TABLE allowed_third_party (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);

CREATE TABLE block_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cookie_domain TEXT NOT NULL,
    page_url TEXT NOT NULL,
    reason TEXT NOT NULL,              -- 'blocked_domain' or 'third_party'
    blocked_at INTEGER NOT NULL
);

-- Purged: logs older than 30 days auto-deleted on startup
```

### 6.2 Cookie Blocking Flow (IO Thread)

```
CanSendCookie(browser, frame, request, cookie_list)
    │
    for each cookie:
    │
    ├── extract page_url from request->GetURL()
    │   extract cookie_domain from cookie.GetDomain()
    │
    ├── Skip if localhost/127.0.0.1
    │
    ├── shared_lock(mutex_) → check blocked_domains_ set
    │   match = exact or wildcard suffix
    │   if blocked → log async → return false
    │
    ├── IsAuthCookieDomain(cookie_domain)?
    │   Strip leading dot, check AUTH_COOKIE_DOMAINS
    │   Suffix match for subdomains
    │   if auth → return true (ALWAYS allow)
    │
    ├── IsThirdParty(cookie_domain, page_domain)?
    │   if third-party AND not in allowed_third_party_
    │   → log async → return false
    │
    └── return true (allow)
```

### 6.3 Frontend IPC API

```typescript
// Cookie management
window.hodosBrowser.cookies.getAllCookies(): Promise<Cookie[]>
window.hodosBrowser.cookies.deleteCookie(url, name): Promise
window.hodosBrowser.cookies.deleteDomainCookies(domain): Promise
window.hodosBrowser.cookies.deleteAllCookies(): Promise

// Cookie blocking configuration
window.hodosBrowser.cookieBlocking.blockDomain(domain, isWildcard): Promise
window.hodosBrowser.cookieBlocking.unblockDomain(domain): Promise
window.hodosBrowser.cookieBlocking.getBlockList(): Promise<BlockedDomainEntry[]>
window.hodosBrowser.cookieBlocking.allowThirdParty(domain): Promise
window.hodosBrowser.cookieBlocking.removeThirdPartyAllow(domain): Promise
window.hodosBrowser.cookieBlocking.getBlockLog(limit, offset): Promise<BlockLogEntry[]>
window.hodosBrowser.cookieBlocking.clearBlockLog(): Promise
window.hodosBrowser.cookieBlocking.getBlockedCount(): Promise<{ count: number }>
window.hodosBrowser.cookieBlocking.resetBlockedCount(): Promise
```

### 6.4 Default Tracker List (Populated on First Run)

24 domains from `DefaultTrackerList.h`:
- **Google Ads/Analytics**: `google-analytics.com`, `googletagmanager.com`, `googlesyndication.com`, `doubleclick.net`
- **Facebook/Meta**: `facebook.net`, `fbcdn.net`
- **Ad Networks**: Criteo, Taboola, Rubicon, PubMatic, OpenX, AppNexus
- **Analytics**: Hotjar, Mouseflow, FullStory, Mixpanel, Amplitude, Segment, New Relic
- **Other**: Quantcast, Amazon-adsystem

**Note**: `facebook.net` is in the default block list but `@@||connect.facebook.net^*/sdk/$script` is in `hodos-unbreak.txt` to allow the Facebook Login SDK.

### 6.5 Per-Profile Settings (SettingsManager)

```json
{
  "browser": {
    "homepage": "https://www.google.com",
    "searchEngine": "google",
    "zoomLevel": 0.0,
    "showBookmarkBar": true,
    "downloadsPath": "",
    "restoreSessionOnStart": false
  },
  "privacy": {
    "adBlockEnabled": true,
    "thirdPartyCookieBlocking": true,
    "doNotTrack": false,
    "clearDataOnExit": false,
    "fingerprintProtection": true
  },
  "wallet": {
    "autoApproveEnabled": false,
    "defaultPerTxLimitCents": 100,
    "defaultPerSessionLimitCents": 500,
    "defaultRateLimitPerMin": 10
  }
}
```

---

## 7. Action Items

### 7.1 Immediate Fixes (Sign-In Breakage)

**P0 — Add `$elemhide` exception for Google auth domains:**
```
@@||accounts.google.com^$elemhide
@@||myaccount.google.com^$elemhide
```
Add to `hodos-unbreak.txt`. Without this, EasyList cosmetic CSS may hide Google's login form.

**P0 — Add network-level exceptions for Google resource domains:**
```
@@||ssl.gstatic.com^$script,domain=accounts.google.com
@@||accounts.gstatic.com^$image,domain=accounts.google.com
@@||www.gstatic.com^$script,domain=accounts.google.com
@@||fonts.googleapis.com^$stylesheet,domain=accounts.google.com
@@||apis.google.com^$script,domain=accounts.google.com
```
These ensure Google's login page can load its JavaScript, CSS, images, and fonts.

**P0 — Add reCAPTCHA domains to fingerprint auth bypass:**
Add `www.gstatic.com` and `www.google.com` to `FingerprintProtection::IsAuthDomain()` so reCAPTCHA canvas operations aren't farbled.

**P1 — Verify adblock engine isn't blocking Google auth subdomains:**
Test with adblock disabled to confirm sign-in works. If it does, progressively re-enable layers to find the exact blocker.

### 7.2 Medium-Term Improvements

**Add `$elemhide` for all auth domains in hodos-unbreak.txt:**
Any domain with `#@#+js()` should probably also have `$elemhide` to prevent cosmetic rules from breaking functionality.

**Test FedCM in CEF 136:**
1. Load a page using `navigator.credentials.get({ identity: { providers: [{ configURL: 'https://accounts.google.com/gsi/...' }] } })`
2. Does CEF show the native account picker dialog?
3. If not, sites using Google Identity Services (GSI) will fail silently
4. May need to enable CEF/Chromium flags or handle via CefClient interface

**Implement public suffix list for IsThirdParty():**
Current substring matching fails for `.co.uk`, `.github.io`, etc. Consider embedding Mozilla's public suffix list or a minimal version.

**Add logging to diagnose auth failures:**
When `CanSendCookie`/`CanSaveCookie` blocks a cookie on a domain containing "google", "microsoft", "apple", or "github", log a warning. This makes debugging sign-in failures much easier.

### 7.3 Long-Term (Post-MVP)

**Ephemeral partitioned storage:**
Brave's approach is fundamentally more robust. Instead of maintaining an allowlist, partition third-party storage by (top-level, third-party) pair and clean up after tab close + 30-second grace period.

**Google Sign-In permission prompt:**
Detect GSI SDK usage and show a permission prompt like Brave does: "Allow Google Sign-In on this site?"

**Cookie lifetime caps:**
Brave caps JS-set cookies to 7 days, HTTP cookies to 6 months. Prevents long-lived tracking.

**Bounce tracking protection:**
Detect and strip tracking redirects (e.g., `tracker.com/bounce?url=target.com`).

**CDN-hosted hodos-unbreak.txt:**
Move exception list to a CDN URL and update via the existing 6-hour filter list auto-update cycle. Allows hotfixing auth breakage without requiring a browser update.

---

## Appendix A: Cookie Blocking Decision Matrix

| Cookie Type | Domain Match | Our Action | Brave's Action |
|-------------|-------------|------------|----------------|
| First-party | page == cookie | ALLOW | ALLOW |
| Third-party, auth domain | AUTH_COOKIE_DOMAINS | ALLOW (bypass) | Ephemeral partitioned |
| Third-party, user-allowed | allowed_third_party table | ALLOW | Ephemeral partitioned |
| Third-party, blocked list | blocked_domains table | BLOCK | BLOCK |
| Third-party, unknown tracker | not in any list | BLOCK (if thirdPartyCookieBlocking on) | Ephemeral partitioned |

## Appendix B: All Auth Protection Layers

For a domain like `accounts.google.com`:

| Layer | Protection | Bypass Mechanism |
|-------|-----------|-----------------|
| Network ad blocking | EasyList may block subresources | `hodos-unbreak.txt`: network exceptions (`@@||domain^$script`) |
| Cosmetic CSS filtering | EasyList CSS rules may hide elements | `hodos-unbreak.txt`: `@@||domain^$elemhide` |
| Scriptlet injection | Fetch/XHR proxy scripts may break OAuth | `hodos-unbreak.txt`: `domain#@#+js()` |
| Cookie blocking | Third-party cookies may be blocked | `AUTH_COOKIE_DOMAINS` in CookieBlockManager.cpp |
| Fingerprint protection | Canvas/WebGL farbling may break CAPTCHA | `IsAuthDomain()` in FingerprintProtection.h |

All five layers must have exceptions for auth to work reliably.
