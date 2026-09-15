# P13 Research — Steps 1 & 2: Bot-Detection Vendor Enumeration + Signal Inventory

Research-only. No repo changes. Confidence tags used throughout:
**VERIFIED** = fetched the URL myself this session and confirmed content described.
**CITED** = found via search / a named source, URL not independently fetched (or fetch failed) — treat as "found cited," re-check before relying on it.
**INFERRED** = my own synthesis/reasoning connecting two cited facts, not itself sourced.
Access date for all web sources: **2026-09-15**.

---

## Step 1 — Vendors, public demos, and known site usage

### 1. Cloudflare Turnstile (managed / non-interactive / invisible) + Cloudflare managed challenge

- **Demo/test**: No single Cloudflare-hosted interactive demo page exists; instead Cloudflare documents **test sitekeys** you drop into your own page. **VERIFIED** (fetched `developers.cloudflare.com/turnstile/troubleshooting/testing/`):

| Sitekey | Behavior | Widget |
|---|---|---|
| `1x00000000000000000000AA` | Always passes | Visible |
| `2x00000000000000000000AB` | Always fails | Visible |
| `1x00000000000000000000BB` | Always passes | Invisible |
| `2x00000000000000000000BB` | Always fails | Invisible |
| `3x00000000000000000000FF` | Forces interactive challenge | Visible |

  Third-party hosted form using these keys: `https://2captcha.com/demo/cloudflare-turnstile` — **CITED**, not fetched.
- **Pass vs fail**: Pass → token minted client-side (`XXXX.DUMMY.TOKEN.XXXX` for test keys), page proceeds silently or after a checkbox click depending on mode. Fail / full block on a real deployment → Cloudflare's own interstitial ("Checking your browser…", or an outright **Error 1020 Access Denied** page) — **CITED** via multiple bypass-guide writeups (zenrows, hasdata, capsolver).
- **Error/ray id**: Yes — every Cloudflare block page shows a **Ray ID**, used by the site's admin to look up the specific blocked request in their firewall log. **CITED**.
- **Managed challenge page**: no standalone vendor demo; only observed by hitting a live Cloudflare-protected site configured to challenge. Say so explicitly — **no public demo exists** for the managed-challenge full page in isolation.

### 2. Google reCAPTCHA v2 checkbox / v2 invisible / v3 / Enterprise

- **Demo**: `https://recaptcha-demo.appspot.com/` — **VERIFIED** reachable. Shows v2 checkbox (standard + explicit-render variant), v2 invisible, and v3 ("Request scores"). It does **not** include an Enterprise demo.
- **Enterprise**: **no public demo exists** — Enterprise requires a GCP project/API key; the only way to see it is on a live customer site. Say so explicitly.
- **Pass vs fail**: v2 checkbox — pass shows a green check; fail (or randomly, by risk) escalates to an image-grid challenge ("select all squares with…"). v2 invisible — same escalation, but no checkbox is shown up front. v3 — no visible UI at all; it returns a float score 0.0 (bot) – 1.0 (human) to the site's own backend, and the *site* decides what "fail" looks like (it is not a captcha vendor UI decision). **VERIFIED**/**CITED** (docs: `developers.google.com/recaptcha/docs/v3`).
- **Error/ray id**: v3 exposes no client-visible id. v2's server-side `error-codes` (`missing-input-response`, `invalid-input-response`, etc.) come back on `siteverify`, not shown to the end user. **CITED** (Google docs).

### 3. hCaptcha

- **Demo**: `https://accounts.hcaptcha.com/demo` — **VERIFIED** reachable. Shows "Sample Form with hCAPTCHA"; accepts `?hl=`, `?sitekey=`, `?secret=` query params to reconfigure the widget live.
- **Pass vs fail**: pass → checkmark, form unblocks. Fail → repeated image-classification challenges; hCaptcha also offers a documented "accessibility cookie" bypass path for assistive-tech users (sign up at `hcaptcha.com/accessibility`) — **CITED**.
- **Error/ray id**: hCaptcha returns a `siteverify` JSON response and, per-challenge, internal challenge/session identifiers, but there is no public-facing "ray id"-style page. **CITED**.

### 4. Arkose Labs FunCaptcha

- **Demo**: `https://demo.arkoselabs.com/?key=DF9C4D87-CB7B-4062-9FEB-BADB6ADA61E6` — **CITED**, found via search, not independently fetched this session. Third-party live-widget playgrounds: `https://capzy.ai/solvers/funcaptcha/demo`, `https://test.cap.guru/demo/funcap` — **CITED**.
- **Pass vs fail**: In low-risk sessions the client script runs invisibly and returns a session token via callback with no UI shown at all. In higher-risk sessions it shows an interactive 3D-rotation / image-matching puzzle. Fail → puzzle repeats or the integrating site blocks the action (Arkose itself doesn't render a "you are blocked" page — that's the integrator's call). **CITED**.
- **Error/ray id**: no public error-code page; Arkose issues a session/public-key token consumed server-side by the integrator.

### 5. DataDome

- **No public pass/fail demo page exists.** DataDome offers a sales-gated "Watch a demo" video (`datadome.co/watch-a-demo/`) and a signup-gated "free vulnerability scan" (`datadome.co/signup/`) — neither is an open, anonymous test-and-see-the-verdict page. Say so explicitly. **CITED**.
- **What its challenge looks like in the wild**: a DataDome-branded interstitial (slider puzzle or "select the images" grid) served directly by DataDome's edge in front of the protected site — **CITED**, general knowledge from scraping-community writeups, not a vendor-provided demo.
- Sets a `datadome` cookie once cleared — **CITED**.

### 6. HUMAN Security (PerimeterX)

- **No public pass/fail demo page exists** — product demos are sales-contact-gated. Say so explicitly.
- **Cookie fingerprint** (useful as detection evidence on live sites): any combination of `_pxvid`, `_px3`, `_pxhd`, `_pxcts` — consistent across every PerimeterX/HUMAN deployment regardless of site. **CITED** (scrappey.com, decodo.com bypass guides).
- Detection signals described publicly: TLS fingerprint, IP reputation, HTTP header order, JS/device fingerprint, and behavioral biometrics (mouse/typing patterns), with "Code Defender" watching for live JS-patch/tamper attempts. **CITED** (scraperapi.com blog).

### 7. Akamai Bot Manager

- **No public interactive demo/test page.** Akamai's own "Bot Manager Demo" (`akamai.com/resources/video/bot-manager-demo`) is a **marketing video**, not a live pass/fail page — flag this distinction explicitly, it's easy to conflate.
- **What the challenge looks like**: the "Adaptive Challenge" (**sec-cpt**) is a proof-of-work gate served *after* the always-on Bot Manager (`_abck` cookie) sensor has already run. It announces itself as **HTTP 428 Precondition Required** with a JSON body, or a 200 HTML page titled "Challenge Validation," and sets a `sec_cpt` cookie. Three challenge providers are documented in the wild: `crypto` (proof-of-work + mandatory wait), `behavioral` (sensor-data resubmission), `adaptive` (both combined). **CITED** (blog.crawlex.net, Hyper Solutions API docs — third-party reverse-engineering, not Akamai's own docs).

### 8. Kasada

- **Demo**: `https://becorbot.kasada.io/` — advertised as a page to "test your vendor's ability to detect a headless Chrome bot." **CITED**, found via search. My own `WebFetch` attempt against it returned **HTTP 401** — consistent with (but not proof of) it actively rejecting a non-browser fetch; note this as an interesting data point, not a confirmed page description. Verify by loading it in an actual browser (real Chrome and Hodos) rather than trusting an automated fetch.
- **Pass/fail mechanics**: client-side proof-of-work challenge (`x-kpsdk-ct` session token) combined with TLS/JA3, HTTP, and JS-environment fingerprinting; on failure the request is typically blocked outright rather than shown a traditional captcha. **CITED** (scrapfly.io "Bypass Kasada," evomi.com blog).

### 9. Imperva (Incapsula) Advanced Bot Protection

- **No public pass/fail demo page** — Imperva's "demo tours" (`imperva.com/demo-tours/`) are a curated product walkthrough, not an anonymous test. Say so explicitly.
- **Cookie/interstitial signature**: commonly reported as `incap_ses_*` / `visid_incap_*` cookies and an "Additional security check" redirect on protected sites. **Flagging this as weaker than the rest of this document** — I did not get an independent citation for it in this research pass (general industry recollection only); verify against a live Imperva-protected site or a fresh search before using it in a report.

### 10. AWS WAF Challenge / CAPTCHA

- **Demo**: AWS provides sample **code**, not a hosted live page: `https://github.com/aws-samples/aws-waf-captcha-react-demo` — **CITED**, a React reference app you'd have to deploy yourself; there is no `aws.amazon.com`-hosted anonymous pass/fail page.
- **Two distinct actions**, per AWS's own docs (`docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge-actions.html` — **CITED**): **CAPTCHA** = visible puzzle the visitor solves; **Challenge** = silent background browser-verification with no visible UI. AWS WAF Bot Control additionally exposes JA3/JA4 TLS-fingerprint matching and IP-reputation labels attached to requests (`x-waf-fingerprint`, `x-waf-token-id` headers). **CITED**.

### 11. GeeTest

- **Demo**: `https://www.geetest.com/en/adaptive-captcha-demo` — **VERIFIED** reachable. Shows five modes: No CAPTCHA (passwordless), Slide, Icon, Gobang, IconCrush.
- **Pass vs fail**: pass → a "successtip" confirmation banner, action proceeds. Fail → refresh/retry controls inside the widget; the exact failure copy wasn't captured in this pass.

### 12. Google's "unusual traffic" interstitial

- This is **not a third-party vendor** — it's Google Search's own internal risk system, distinct from reCAPTCHA-the-product even though it uses a reCAPTCHA widget to clear itself. It lives at `google.com/sorry/index` (a.k.a. `g.co/sorry`) and **cannot be demoed on demand** — it only appears organically when Google's own systems flag a connection (shared IP, VPN exit node, automated-looking query pattern, etc.). Say explicitly: **no demo page exists by design**; you can only encounter it live. **CITED** (`support.google.com/websearch/answer/86640`).

### 13. Shape / F5 (F5 Distributed Cloud Bot Defense)

- **No public demo/test page found.** Signal collection is described only in secondary technical writeups: a JS agent collects environment + behavior telemetry and attaches it to requests against sensitive paths (login, account-creation, scraping targets); telemetry is scored by "Shape AI Cloud" ML models. **CITED** (F5 community DevCentral article, blog.crawlex.net).

---

### Which vendor each named site is known to use

| Site | Vendor | Confidence | Evidence |
|---|---|---|---|
| **x.com** | Arkose Labs FunCaptcha, layered with X's own bot-scoring | CITED (moderate-strong) | Multiple 2026 scraping-guide writeups name Arkose + AWS WAF + "X's own bot-scoring" together (`browseract.com/blog/twitter-scraping-2026`); Arkose's own historical partnership with Twitter (as FunCaptcha) for account-creation/login is long-public. Not independently screenshot-verified this session. |
| **github.com** | GitHub's own "OctoCaptcha" front door, which itself depends on **DataDome** and **Arkose Labs** domains | CITED (**first-party**) | GitHub's own connectivity-troubleshooting docs (`docs.github.com/en/get-started/using-github/troubleshooting-connectivity-problems`) instruct users behind firewalls to allow `octocaptcha.com` and `arkoselabs.com`; `octocaptcha.com/test` is GitHub's own connectivity-check endpoint. This is the strongest-sourced row in this table. |
| **google.com** | reCAPTCHA (product surfaces generally) **and**, separately, Google Search's own "unusual traffic" interstitial at `google.com/sorry` (not the same system) | CITED | `support.google.com/websearch/answer/86640` describes the Search-specific interstitial; reCAPTCHA is Google's own product used elsewhere on google.com properties. |
| **amazon.com** | AWS WAF CAPTCHA (Amazon's own product, self-hosted) | CITED | Multiple captcha-solving-service docs (2captcha, CapMonster, CapSolver) describe "Amazon AWS WAF Captcha" appearing on amazon.com's own login/search flows — i.e., Amazon dogfoods its own AWS product on its retail site. |
| **reddit.com** | hCaptcha (commonly reported) | **CITED — weak.** Multiple scraping-community sources bundle Reddit into "hCaptcha-protected sites" lists, but none surfaced in this session's searches is a primary/first-party confirmation. **Verify live before relying on this.** |
| **nytimes.com** | HUMAN Security / PerimeterX (commonly reported) | **INFERRED / not confirmed this session.** PerimeterX's cookie signature (`_px3`/`_pxvid`/`_pxhd`) is well-documented in general, and NYT↔PerimeterX is a claim I recall from general industry knowledge, but no source in this session's search results named nytimes.com explicitly. **Do not put this in a report without a live check** (open devtools on nytimes.com, look for `_px3`/`_pxvid`/`_pxhd` cookies and a `px-cloud.net` or similar script origin). |

---

## Step 2 — Signals scored, grouped by family

### A. Automation tells
- `navigator.webdriver === true` — the WebDriver-spec property; Chrome sets it whenever any automation flag is present. Pre-Chrome-88 it could just be deleted; from 88 onward stock Chrome instead needs `--disable-blink-features=AutomationControlled` to suppress it at the source. **CITED** (puppeteer-extra-plugin-stealth readme via npm/GitHub).
- `cdc_...` properties injected onto `document`/`window` by ChromeDriver (e.g. `document.$cdc_asdjflasutopfhvcZLmcfl_`) — anti-bot code pattern-matches on `cdc_|\$chrome_|\$cdc_`. **CITED**.
- Other framework markers: `window._selenium`, `window._Selenium_IDE_Recorder`. **CITED**.
- `window.chrome` shape — missing `window.chrome.runtime` in older headless Chrome was a classic tell (patched by stealth plugins since ~2019). **CITED**.
- Empty `navigator.plugins` / `navigator.mimeTypes` arrays — headless defaults to zero plugins where a real desktop Chrome install reports a small fixed set (PDF viewer, etc.). **CITED** (crawlex.net headless-detection post).
- **CDP-protocol-level tell (2026, DataDome)**: DataDome's Feb-2026 post describes detecting the CDP/WebSocket wire protocol itself — the way the automated browser and driver serialize messages to talk to each other — rather than any specific per-framework side effect. This is notable because it targets the *underlying remote-debugging channel*, which is relevant to Hodos given CEF's own devtools/CDP surface (see root `CLAUDE.md`'s P9 sprint notes on CDP port gating). **CITED** (`datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/`).

### B. Environment consistency
- User-Agent string vs. Client Hints (`Sec-CH-UA*`) vs. `navigator.platform` — these three should describe the same OS/browser; a mismatch (e.g. UA claims Windows, Client Hints claim something else) is a documented tell. **INFERRED**, standard consistency-check pattern, consistent with what BotD/CreepJS detector categories check (see below) but not pinned to one specific vendor blog this session.
- `Accept-Language` header vs. `navigator.language`/`navigator.languages` — mismatch is a scriptable header the site can compare against the JS-visible value.
- Screen metrics vs. window metrics — headless/automation contexts frequently report degenerate or suspiciously "round" `screen.width/height` vs `window.innerWidth/innerHeight`, or a 1:1 outer/inner window size (no chrome/toolbar space).
- Timezone (`Intl.DateTimeFormat().resolvedOptions().timeZone`) vs. IP-geolocated locale — VPN/proxy use commonly produces a timezone that doesn't match the exit IP's geography.

### C. Fingerprint consistency
- **Stability across repeated reads / frames**: castle.io's fraud-detection writeup describes explicitly drawing canvas output **multiple times within one session** to tell "stable" (real GPU/driver noise, or deliberate anti-fingerprint randomization) from "manipulated" (unstable per-call in a way that looks like tampering rather than a fixed per-session/per-domain farble). **VERIFIED** (fetched `blog.castle.io/when-canvas-lies-handling-randomized-fingerprints-in-fraud-detection/`).
- **WebGL vendor/renderer plausibility** and the **"Picasso" approach**: DataDome's Feb-2024 writeup describes using specific canvas-API graphical primitives to bucket a device into a *browser/OS family* even when individual values are randomized — resilience against randomization comes from grouping into device classes, not needing an exact-match fingerprint. It explicitly lists CPU core count, device memory, and GPU type as collected signals. **VERIFIED**/**CITED** (`datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/`; academic root: Google's "Picasso" paper, `github.com/antoinevastel/picasso-like-canvas-fingerprinting`).
- **CreepJS "lie detection"**: rather than just collecting a fingerprint, CreepJS cross-checks whether the *signals are internally consistent* — e.g., does the claimed WebGL renderer actually behave the way that renderer should. This is the general pattern industrial bot-detectors use to catch naive spoofing (changing `navigator.platform` without changing anything that platform's rendering pipeline would actually affect). **CITED** (`github.com/abrahamjuliot/creepjs`).
- **Academic prior art on randomization-as-defense**: PriVaricator (randomization breaks cross-visit *linkability*, not uniqueness) and FPRandom (randomizes canvas, AudioContext, and JS-property enumeration order in a modified Firefox) are the direct academic ancestors of Brave's farbling approach. **CITED** (Semantic Scholar / Springer listings for both papers).

**⭐ Documented case: randomized fingerprints scoring as bots (the Brave-vs-Turnstile tension)**
- `brave/brave-browser#45608` — "Cloudflare Turnstile Verification always fails in Brave Browser," reported even with all Shields disabled. **CITED**.
- `brave/brave-browser#58915` (filed 2026-09-11, open, unresolved) — Cloudflare's own diagnostics explicitly logged **"WebGL renderer info is spoofed/blocked (unmasked renderer: Brave)"** as an interference signal; in this particular report the Turnstile challenge still ultimately passed, but the *interference flag fired*, meaning the farbled WebGL value was legible to Cloudflare as "this browser is doing something to its fingerprint." **VERIFIED** (fetched the issue directly).
- A third-party writeup (`hacktivis.me/articles/cloudflare-turnstile-webgl-fingerprinting`) argues Turnstile's non-interactive mode depends on being able to read a **plausible, fingerprintable** WebGL surface to clear low-risk visitors silently — my own fetch of that page failed (HTTP 402 from the fetch tool, not from the site), so treat its specific claims as **CITED, not independently confirmed** this session; worth reading directly in a browser.
- **INFERRED, tying it together**: castle.io's four-step approach (detect instability → categorize into "known randomizing browsers" [it names Firefox, Brave, Samsung Browser] vs. "abnormal" → don't penalize known-good randomizers, but do raise risk score for anything *not* on that allowlist) means the safe path for a randomizing browser is **being recognized by name** by the detector. A CEF-embedder browser with its own UA/brand string (Hodos) that farbles the same way Brave does, but isn't in anyone's allowlist, is structurally more likely to land in the "abnormal, raise risk score" bucket than Brave itself is — this is a plausible mechanism for exactly the failure the user hit, not a proven one.

### D. Network / TLS
- **JA3 / JA4**: JA3 hashes the TLS ClientHello (cipher list, extensions, elliptic curves, point formats); JA4 (FoxIO/John Althouse) was created because Chrome 110+'s extension-order shuffling and GREASE broke JA3 stability for real browsers, and JA4 sorts extensions before hashing to fix that, plus explicitly covers TLS 1.3 fields (ALPN, signature algorithms, supported versions) that JA3 never captured. **CITED**.
- **Public TLS-fingerprint echo services to compare Hodos vs. stock Chrome**: `https://tls.peet.ws` (raw JA3/JA4 echo, shows full ClientHello byte breakdown) and `https://browserleaks.com/tls` — **CITED**, both are exactly the "paste your fingerprint back at you" tool the user asked for. Since Hodos builds its own Chromium/CEF from source (per root `CLAUDE.md`), its BoringSSL build/version pin and therefore its ClientHello shape could plausibly diverge from stock Chrome's even with identical JS-visible fingerprinting — this is a distinct signal family from anything JS can see or farble.
- **AWS WAF** now supports JA3 and JA4 fingerprint *match* rules and rate-limiting keyed on them, and CloudFront can expose the JA3 fingerprint as a request header (`x-waf-fingerprint`) to the origin. **CITED** (AWS WAF docs + AWS blog).
- **HTTP/2 fingerprint**: frame/SETTINGS ordering is mentioned by name as a DataDome signal (`blog.crawlex.net/blog/datadome-http2-fingerprinting/` title) — **CITED**, not independently read in depth this session.
- Real-world example of TLS fingerprint mattering in practice: Kasada is reported to instantly flag Python `requests`' well-known JA3 hash, and to reject a session even if its proof-of-work math is otherwise solved correctly, because the TLS + browser fingerprint pairing doesn't match. **CITED** (scrapfly.io "Bypass Kasada").

### E. Behaviour
- Mouse-movement timing, focus events, and "timing jitter" (natural micro-variance in event arrival) are explicitly named as part of what Turnstile's non-interactive JS challenges watch. **CITED** (crawlex.net Turnstile-internals writeup).
- PerimeterX/HUMAN is described as monitoring mouse movement, click, and typing-cadence patterns as a "behavioral biometrics" layer, on top of the TLS/IP/header/JS-fingerprint checks. **CITED** (scraperapi.com blog).

### F. Storage / cookies
- `__cf_bm` — Cloudflare Bot Management's short-lived (~30 min) cookie.
- `cf_clearance` — long-lived proof a browser passed a Cloudflare JS/Turnstile challenge; documented as **bound to the exact TLS signature, IP, and User-Agent that earned it** — reusing the cookie value alone from a different TLS/IP/UA combination invalidates it. **CITED** (zenrows.com cf_clearance guide).
- `_pxvid` / `_px3` / `_pxhd` / `_pxcts` — PerimeterX/HUMAN's cookie family, consistent across every deployment. **CITED**.
- `datadome` — DataDome's clearance cookie. **CITED** (name only, mechanism not independently sourced this session).
- `sec_cpt` — Akamai's Adaptive Challenge cookie. **CITED**.
- `incap_ses_*` / `visid_incap_*` — commonly attributed to Imperva/Incapsula. **Flagged weak/unverified this session**, see vendor section above.
- **What strips them**: clearing cookies or private/incognito browsing forces a fresh challenge on next visit (trivially, no cookie = no proof-of-pass). More subtly, `cf_clearance` specifically is invalidated by an IP/TLS/UA change even if the cookie itself is intact and present. For Hodos specifically: this project's adblock engine does cosmetic filtering, scriptlet injection, and (per root `CLAUDE.md`) has a `CookieFilterResourceHandler` currently scoped to YouTube — whether any *general* cookie-blocking rule in Hodos's privacy shield could incidentally strip challenge-clearance cookies (treating them as "tracking cookies") is an **open question for the measured matrix**, not something confirmed in this research pass.

### G. Modified-environment tells
- **Overridden `fetch`/`XHR`/`JSON.parse`**: Hodos's own adblock scriptlet injection does exactly this by design (per root `CLAUDE.md`: "JavaScript injected into page context via V8 to override browser APIs (fetch, XHR, JSON.parse) and strip ad data"). This is architecturally the same *shape* of thing bot-detection vendors watch for — a page-context script whose `fetch`/`XHR` doesn't behave like the stock browser's.
- **Non-native `toString` on built-ins**: the canonical tell is `Function.prototype.toString.call(fn)` no longer returning `"function fn() { [native code] }"` once a function has been monkey-patched without preserving the native-code string. Root `CLAUDE.md` names this exact concern as the reason the old injected-JS farbling implementation (`FingerprintScript.h`) was deleted in favor of native Blink patches: the injected-JS approach "restores the `toString` tamper tell." **This is a first-party, Hodos-specific data point**, not third-party research — but directly on-topic: it confirms the project's own engineers already identified `toString` tampering as a real, scored signal family, which is exactly Step 2's "modified built-ins" category.
- **Extra `window` globals**: `window.hodosBrowser` and `window.CWI` are non-standard globals injected for the dApp/wallet bridge. A fingerprinting/bot-detection script that enumerates `window` properties (a cheap, common technique — CreepJS and BotD both do some form of this) would see property names that don't exist on stock Chrome. This is **directly analogous**, in mechanism, to how automation frameworks get caught via their own injected markers (`cdc_...`, `window._selenium` — see Family A) — the detector doesn't need to know *what* `window.hodosBrowser` is, only that "this browser has globals a stock browser of its claimed identity doesn't." **INFERRED**, but the analogy to a well-documented existing detection pattern is direct and low-risk to state.

---

## Prior-art source log (for `development-docs/PRIOR_ART.md`)

| Source | URL | What it was good for | Worth the trip? |
|---|---|---|---|
| Cloudflare Turnstile testing docs | https://developers.cloudflare.com/turnstile/troubleshooting/testing/ | Official test sitekeys/behaviors table | Yes |
| Cloudflare Turnstile widget concepts | https://developers.cloudflare.com/turnstile/concepts/widget/ | Managed/non-interactive/invisible mode definitions | Yes |
| reCAPTCHA public demo | https://recaptcha-demo.appspot.com/ | Confirmed v2/v2-invisible/v3 demo, no Enterprise | Yes |
| reCAPTCHA v3 docs | https://developers.google.com/recaptcha/docs/v3 | Score semantics, action tags | Yes |
| hCaptcha demo | https://accounts.hcaptcha.com/demo | Confirmed reachable, param-configurable demo | Yes |
| GeeTest adaptive demo | https://www.geetest.com/en/adaptive-captcha-demo | Confirmed reachable, 5 challenge modes, pass/fail copy | Yes |
| Arkose Labs demo | https://demo.arkoselabs.com/?key=DF9C4D87-CB7B-4062-9FEB-BADB6ADA61E6 | Located official demo URL | Yes (not yet fetched) |
| Kasada headless-bot demo | https://becorbot.kasada.io/ | Located demo; fetch returned 401 (itself informative) | Yes |
| GitHub connectivity troubleshooting | https://docs.github.com/en/get-started/using-github/troubleshooting-connectivity-problems | First-party confirmation GitHub depends on octocaptcha.com + arkoselabs.com | Yes — strongest single citation found |
| AWS WAF CAPTCHA/Challenge docs | https://docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge-actions.html | CAPTCHA vs Challenge action distinction | Yes |
| AWS WAF CAPTCHA React demo (sample code) | https://github.com/aws-samples/aws-waf-captcha-react-demo | Confirms no hosted live demo, only sample code | Yes |
| Google "unusual traffic" help page | https://support.google.com/websearch/answer/86640 | Confirms it's Search-internal, not on-demand | Yes |
| DataDome — Picasso device-class fingerprinting | https://datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/ | Signal list (CPU/memory/GPU), randomization-resilience via device-class bucketing | Yes |
| DataDome — headless Chrome & CDP signal | https://datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/ | 2026 CDP-protocol-level detection angle | Yes |
| Cloudflare Turnstile internals (3rd-party) | https://blog.crawlex.net/blog/cloudflare-turnstile-internals/ | Behavioral + proof-of-work + network signal breakdown | Yes |
| castle.io — "When canvas lies" | https://blog.castle.io/when-canvas-lies-handling-randomized-fingerprints-in-fraud-detection/ | Direct evidence of vendors allowlisting known-randomizing browsers (Firefox/Brave/Samsung) | Yes — directly on-topic for the Brave tension |
| brave/brave-browser#45608 | https://github.com/brave/brave-browser/issues/45608 | Turnstile fails in Brave even w/ Shields off | Yes |
| brave/brave-browser#58915 | https://github.com/brave/brave-browser/issues/58915 | Cloudflare explicitly logs "WebGL renderer spoofed/blocked" as interference | Yes — fetched directly, best single piece of evidence |
| hacktivis.me — Turnstile WebGL fingerprinting | https://hacktivis.me/articles/cloudflare-turnstile-webgl-fingerprinting | Argues Turnstile needs plausible WebGL to pass silently | Cited only — fetch failed (HTTP 402 from fetch tool), re-fetch manually |
| FingerprintJS BotD | https://github.com/fingerprintjs/botd | Open-source bot-detector signal/detector categories | Yes |
| CreepJS | https://github.com/abrahamjuliot/creepjs | "Lie detection" — internal-consistency checking concept | Yes |
| puppeteer-extra-plugin-stealth readme | https://github.com/berstend/puppeteer-extra/blob/master/packages/puppeteer-extra-plugin-stealth/readme.md | Canonical evasion-module list = signal list | Yes |
| tls.peet.ws | https://tls.peet.ws | Public JA3/JA4 TLS echo service | Yes — directly what the task asked for |
| browserleaks.com/tls | https://browserleaks.com/tls | Alternative TLS fingerprint echo/analysis | Yes |
| AWS WAF JA3 fingerprint match announcement | https://aws.amazon.com/about-aws/whats-new/2023/09/aws-waf-ja3-fingerprint-match | Confirms JA3/JA4 used operationally at AWS edge | Yes |
| scrapfly.io — Bypass Kasada | https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf | TLS+PoW+JS combined scoring, Python-requests JA3 example | Yes |
| zenrows — cf_clearance guide | https://www.zenrows.com/blog/cf-clearance | Confirms cf_clearance bound to TLS+IP+UA triple | Yes |
| scrappey.com — PerimeterX cookie names | https://scrappey.com/qa/anti-bot/what-is-perimeterx | `_pxvid`/`_px3`/`_pxhd`/`_pxcts` cookie family | Yes |
| Semantic Scholar — FPRandom | https://www.semanticscholar.org/paper/FPRandom:-Randomizing-Core-Browser-Objects-to-Break-Laperdrix-Baudry/c4cb2071f011c18cee299393c2a92cec3af2a675 | Academic root of randomization-as-defense | Yes |
| PriVaricator (ResearchGate listing) | https://www.researchgate.net/publication/318155017_FPRandom_Randomizing_Core_Browser_Objects_to_Break_Advanced_Device_Fingerprinting_Techniques | Linkability-vs-uniqueness framing | Yes |
| Akamai sec-cpt handling (3rd-party) | https://docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt | HTTP 428 / sec_cpt cookie mechanics | Yes |
| F5 DevCentral — What is Shape Security | https://community.f5.com/t5/technical-articles/what-is-shape-security/ta-p/284359 | Shape/F5 signal-collection description | Yes |

---

## Suggested matrix layout

**Rows** — one per vendor demo page confirmed above (skip the "no public demo" vendors, or replace their row with the best available live-site proxy and mark it as such):

1. Cloudflare Turnstile test-key page (visible-always-passes key first; repeat with the forced-interactive key `3x00000000000000000000FF`)
2. reCAPTCHA demo — v2 checkbox
3. reCAPTCHA demo — v2 invisible
4. reCAPTCHA demo — v3 (record the numeric score, not just pass/fail)
5. hCaptcha demo
6. GeeTest adaptive demo (run each of the 5 modes as its own sub-row if time allows, otherwise just "Slide")
7. Arkose Labs demo.arkoselabs.com
8. Kasada becorbot.kasada.io
9. (proxy row) A known Akamai-protected live site, since no vendor demo exists — pick one from public "sites known to use Akamai Bot Manager" lists and note it's a live site, not a vendor demo
10. (proxy row) A known DataDome/HUMAN/Imperva-protected live site, same caveat
11. AWS WAF CAPTCHA — deploy the `aws-waf-captcha-react-demo` sample app once, or use a known AWS-WAF-protected public endpoint
12. `tls.peet.ws` and `browserleaks.com/tls` — not a captcha at all, but run every column against these too so the raw JA3/JA4 delta is in the matrix independent of any captcha verdict

**Columns:**

| Column | What it is |
|---|---|
| Chrome (positive control) | Stock, unmodified Google Chrome, same OS, same network |
| Hodos defaults | Normal dev/release build, farbling on, adblock on |
| Hodos farbling-off | `FingerprintProtection::SetEnabled(false)` / the Privacy Shield toggle off — everything else default |
| Hodos adblock-off | Adblock engine disabled/stopped, farbling default |
| Hodos both-off | Farbling off **and** adblock off — closest approximation to "plain CEF+Chromium, nothing Hodos-specific active" |

**3 runs per cell** (fresh profile or at least fresh challenge-cookie state each run — see Family F above on why stale `cf_clearance`/`_px3`/etc. would fake a pass) — record: verdict (pass / interactive-challenge / hard-block), any error/ray id shown, and the raw signal delta where visible (e.g. reCAPTCHA v3's numeric score, `tls.peet.ws`'s JA3/JA4 hash).

**How to make one cell fail on purpose (negative control)** — pick whichever is easiest to wire up given CEF's launch surface:
- Launch the browser with `--enable-automation` (this is the flag stock Chrome/Chromedriver sets, and it's exactly what `navigator.webdriver` reflects) — this should flip at least the reCAPTCHA v3 score and likely trip Turnstile's non-interactive check, on **both** the Chrome control column and a Hodos column, proving the harness actually distinguishes "detected as automation" from "not detected."
- Alternative/additional: drive the browser via CDP with a fresh, un-real-user session (per root `CLAUDE.md`'s P9 sprint notes, Hodos's own CDP port already exists dev-only) — hitting the demo pages via `Page.navigate`/`Runtime.evaluate` instead of real input should itself read as "not human" on the behavior family (Step 2-E) even with `navigator.webdriver` suppressed, giving a second independent way to force a red result.
- Whichever method is used, state the negative control result **alongside** the green result per CLAUDE.md's standing rule ("passes, and fails when X is disabled") — and, per CLAUDE.md's negative-control-at-the-*right*-*subject* corollary, confirm the failing run actually hit the **captcha vendor's real production check** and not a stale demo-page cache or a test-sitekey that was left in "always passes" mode by accident.

**macOS parity** — for a second person to run the identical matrix on macOS:
- Build/launch per `cef-native/mac_build_run.sh` with `HODOS_DEV=1` (per root `CLAUDE.md`'s Dev Runbook) — same three-process order (Rust wallet → frontend dev server → CEF browser).
- Farbling toggle and adblock toggle are the same cross-platform settings surface (`SettingsManager`, Privacy Shield panel) — no macOS-specific UI path to note here per current `CLAUDE.md`.
- `--enable-automation` and CDP-driven negative controls work identically on macOS launch args; no platform-specific flag substitution needed.
- Record OS + Chromium/CEF build SHA (per root `CLAUDE.md`'s note that "Chromium version does NOT identify our engine" — cite the fork SHA, not just a version string) alongside every row, since TLS/JA3 and some JS-engine-version-dependent signals can differ across the two platforms' builds even with identical source.
