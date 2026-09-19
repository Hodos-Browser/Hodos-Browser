# Phase 13 — bot-detection compatibility (proving we are not a bot)

**Opened:** 2026-09-15, from a user who could not pass a CAPTCHA in Hodos and stopped using it (site unknown —
the user does not remember). **Status:** ⬜ PLANNED. **Standard:** `../HARNESS.md`.
**Shape:** research → measured matrix → fixes. 👤 **The research and the matrix run in parallel with Phase 10**
(no code, no rig conflict; the result sizes the fix work). Fixes run after Phase 12.

## What the sprint already holds

| Where | What |
|---|---|
| `../../0.4.0/chromium-rebuild/farbling_acceptance_battery.py` | **BOT-1**: `navigator.webdriver === false` and the `window.chrome` stub survive; measured on whatsonchain (Cloudflare Turnstile) |
| `cef-native/cef_browser_shell.cpp` (port block comment) | measured 2026-08-11: binding the CDP port via `CefSettings` does **not** flip `navigator.webdriver`; the command-line switch path would |
| `../../0.4.0/chromium-rebuild/Q3_farbling_oauth.md` | the auth/OAuth allowlist design: exact-host, per-frame; challenge hosts (`challenges.cloudflare.com`, `hcaptcha.com`, `www.gstatic.com`, …) exempted; parent and challenge iframe farbled with the **same** values so cross-frame consistency checks pass |
| `cef-native/src/handlers/simple_app.cpp` | note that `--disable-web-security`, `--in-process-gpu` etc. are detectable ("Turnstile rejects them on whatsonchain"), so they are opt-in on macOS; ⚠️ we still pass `--disable-gpu-compositing` unconditionally — its effect on the WebGL renderer string is unmeasured |
| Phase 9 (`67a9ab6`) | the CDP port and `--remote-allow-origins=*` are now dev-only — one fewer automation tell in release |

What does **not** exist: any pass across vendors other than Cloudflare, and any measurement of *why* a challenge
fails when it does.

## Step 1 — enumerate the vendors (each has a public demo or test page; find and record the URL)

Cloudflare Turnstile (managed / non-interactive / invisible) and Cloudflare's managed challenge page ·
Google reCAPTCHA v2 checkbox, v2 invisible, v3, Enterprise · hCaptcha · Arkose Labs FunCaptcha · DataDome ·
HUMAN Security (PerimeterX) · Akamai Bot Manager · Kasada · Imperva (Incapsula) Advanced Bot Protection · AWS WAF
Challenge / CAPTCHA · GeeTest · Google's own "unusual traffic" interstitial (search) · Shape/F5. Note which
vendor each of our standard test sites uses (x.com, github.com, google.com, amazon.com, reddit.com, nytimes.com).

## Step 2 — enumerate the signals they score (read their public docs and the reverse-engineering literature)

| Family | Signals | Where Hodos stands (to be measured, not assumed) |
|---|---|---|
| Automation tells | `navigator.webdriver`, `--enable-automation`, headless flags, CDP artefacts (`cdc_` properties), `window.chrome` shape, `navigator.plugins`/`mimeTypes` emptiness | BOT-1 covers two; the rest unmeasured |
| Environment consistency | UA vs client hints vs `navigator.platform`; `Accept-Language`; screen vs window metrics; timezone vs locale | our UA reports `Chrome/150.0.0.0` on Windows — check every header the engine sends |
| Fingerprint consistency | canvas/WebGL/audio **same across frames and reads**; WebGL vendor/renderer strings plausible; `deviceMemory`/`hardwareConcurrency` plausible | farbling is deterministic per site and per-frame identical by design (C2); but **randomised values that differ from the machine's real ones can themselves score** — Brave's known tension. `--disable-gpu-compositing` may change the renderer string |
| Network / TLS | JA3/JA4, HTTP/2 fingerprint, ALPN | Chromium's stack — should match Chrome exactly; verify with a TLS fingerprint echo service |
| Behaviour | mouse/keyboard timing, focus events | human at the keyboard; only relevant if our input path drops or synthesises events |
| Storage / cookies | third-party cookie policy, `__cf_bm`, challenge cookies surviving navigation | our cookie filtering (`CookieFilterResourceHandler`) and the adblock lists could strip a challenge cookie — check the unbreak list |
| Extensions-shaped | our injected `window.hodosBrowser` / `window.CWI` globals, scriptlet-modified `fetch`/`XHR`/`JSON.parse` | detectable as "modified environment"; Phase 12's pre-cache timing matters here |

## Step 3 — the matrix (the deliverable of the research half)

Rows: every vendor demo page from step 1. Columns: **Chrome on the same machine** (positive control — if Chrome
fails too, the vendor is broken, not us) · **Hodos dev, defaults** · **Hodos, farbling off for the site**
(Privacy Shield opt-out) · **Hodos, adblock off for the site** · **Hodos, both off**. Cell = pass / interactive
challenge / loop / block, with the vendor's own error id where it shows one. Three runs per cell (challenge
outcomes are noisy). Same profile, same network, same day.

⛔ **Negative control for the matrix itself:** one cell must be *made* to fail on purpose (launch with
`--enable-automation`, or set `navigator.webdriver` true via a dev seam) and be seen to fail — otherwise a
matrix of all-green proves the pages were reachable, not that the test can detect a bot verdict.

## Step 4 — fixes, decided from the matrix

Expected shapes, in order of likelihood: a flag we pass that reads as automation (measure `--disable-gpu-compositing`
first); a challenge cookie or script the adblock lists strip (add to `hodos-unbreak.txt`, the `#@#+js()` mechanism
already exists); a farbled value outside the vendor's plausibility band (tighten the perturbation range, or exempt
the challenge iframe host — the allowlist mechanism exists); a UA/client-hint inconsistency. Prior art (rule 5):
**Brave** has this exact tension and ships per-site Shields toggles as the escape hatch; **Tor Browser** chooses
uniformity for the same reason. Log the rows in `PRIOR_ART.md`.

## Evidence rows (names reserved)

`P13-M1` the matrix, complete, with its negative-control cell red · `P13-F<n>` one row per fix, each with the
vendor page that was red before and green after, and Chrome as the control · `P13-R1` the farbling acceptance
battery still passes after any fix (a bot-compat fix must not silently weaken farbling — the constant-seed bug
was exactly a "fix" nobody measured). 🍎 macOS runs the same matrix (vendors score platform signals).

---

## 🚨 STEP 0, ADDED 2026-09-19 BY THE OWNER — **the complaint predates the farbling rewrite. Re-test before assuming a defect exists.**

👤 *"The user who made the complaint was still running a version before we moved the farbling into
the actual Chromium... still running an old 0.3.x-beta. Not with the farbling in the actual Chromium
build like we now have it in 0.4.0."*

⛔ **This may invalidate the premise of this whole phase, and it must be checked FIRST.**

| | |
|---|---|
| What they ran | `0.3.x-beta` — farbling by **injected JavaScript** (`FingerprintScript.h`) |
| What we ship now | `0.4.0` — farbling as **Blink patches** in the fork (C1/C3/C4/C5/C6), applied at API-call time |
| When it changed | `FingerprintScript.h` **deleted 2026-08-09** |

⭐ **And the old implementation had a tell that bot detection specifically looks for.** The root
`CLAUDE.md` gives it as a reason never to go back:

> *"Do not re-add an injected-JS farbling path: it cannot cover workers, it restores the `toString`
> tamper tell, and it would double-perturb values Blink already farbles."*

A patched-in-JS method does not report `[native code]` from `toString()`. That is one of the cheapest,
most widely deployed bot signals there is — and we were emitting it on every farbled method, on every
page, in exactly the build this user was running. The native Blink patches report `[native code]`
again, because the method genuinely *is* native.

⚠️ **So the most likely reading is that the reported failure was caused by the thing 0.4.0 already
removed.** Not proven — the site is unknown and the user does not remember it — but it reorders the
work.

### What this changes

1. ⛔ **Do not design a fix first.** Step 0 is: run the CAPTCHA basket on **0.4.0** and see whether
   anything fails at all. If nothing does, this phase collapses to a regression guard plus a note.
2. ⭐ **A cheap discriminator exists and should be the first measurement:** compare
   `Function.prototype.toString` output for the farbled methods on 0.3.x vs 0.4.0. If 0.3.x shows
   JS source where 0.4.0 shows `[native code]`, the mechanism is demonstrated rather than assumed.
   `BOT-1` in `farbling_acceptance_battery.py` already asserts the `[native code]` half on 0.4.0.
3. 👤 **Owner's own framing:** *"if it works for us, then good. But it's still probably
   fingerprinting."* 📏 So a green re-test closes the *reported* defect, not the *category* — keep the
   matrix, drop the assumption that something is currently broken.

⭐ **The general lesson, which is the same one this sprint keeps paying for:** a bug report names a
build. Check that the build is the one you are about to fix before you fix it. This is the
`feedback_test_subject_must_be_the_production_call` shape, applied to a *report* rather than a test.
