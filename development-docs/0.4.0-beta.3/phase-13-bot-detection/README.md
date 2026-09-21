# Phase 13 — human-verification compatibility ("prove you're not a bot")

**Opened** 2026-09-15. **Rewritten 2026-09-21** after the first pass drifted. **Standard:** `../HARNESS.md`.
**Status:** ⏸️ **PARKED 2026-09-21 by the owner** — *"let's just leave it for now … get beta.3 done."* Not a beta.3 blocker. The evidence says the current build does not have a fingerprinting problem (test benches: Hodos scored the same as stock Chrome on every verdict; Brave scored worse on 3 of 4). The one open thread is re-running **GitHub signup on a build with no debug port**. `P13-I` instrumentation is designed but not built.

> ⛔ **Rewritten because the first version of this phase was scoped wrong.** It was a 210-cell vendor
> matrix, it ranked work by what was technically interesting rather than by how many users hit it, and
> it produced a session that chased an **audio-CAPTCHA** lead — a path essentially nobody uses —
> while the mainline path went untested. 👤 Owner: *"We don't chase things that we find now."*
> This version is ranked by user impact and says what is **not** being done.

---

## 1. The problem, in one paragraph

A user hit a human-verification check in Hodos, could not get past it, and stopped using the browser.
👤 The owner's account of the symptom is the single most useful fact we have: *"they kept doing it over
and over again, and it kept coming back over and over again."* **Solve it → asked again → forever.**
The site is unknown. The user does not remember it. 👤 It was **not** an audio check.

⚠️ **This is a retention bug, not a feature gap.** A browser that cannot get through a human check is
one a person stops using — which is exactly what happened. That is why it outranks its apparent size.

---

## 2. What "solve it and it comes back" actually is

A human check has two halves, and only the first is visible.

1. **The check.** You click the box, or pick the bicycles, or the page silently scores you. The vendor
   decides you are human and hands the page a **token** — a receipt.
2. **The receipt being cashed.** The page sends the token to the site's server, the server confirms it
   with the vendor, and the server then gives your browser a **pass** — a cookie (`cf_clearance`,
   `datadome`, `_px3`, `sec_cpt`). Every later request carries the pass, and the edge stops asking.

⇒ **The loop is half 1 working every time and half 2 failing every time.** The puzzle is not broken.
The receipt is being lost after you hand it over.

**Three ways half 2 fails:**

| # | Failure | Would we cause it? |
|---|---|---|
| C1 | The pass is never **stored** | We ship a cookie blocker, on by default |
| C2 | The pass is stored but never **sent back** | Same |
| C3 | The pass is sent and the **edge rejects it** — `cf_clearance` is bound to the exact IP + User-Agent + TLS signature that earned it, and is void if any of them differ | Only if something about us changes between requests |

---

## 3. What was measured on the current build — 2026-09-21

⭐ All of this is **done**. Detail and method: `STEP0_AND_SIGNAL_SHEET.md`.

| Question | Result |
|---|---|
| **Step 0** — is the reported defect the one 0.4.0 already deleted? | ✅ **The mechanism is gone, demonstrated.** The 0.3.x script replayed verbatim creates **7 tells**; 0.4.0 shows none. ⛔ Limit: the 0.3.x *script* in a 0.4.0 *engine*, not the 0.3.x binary |
| **C1 — is the pass stored?** | ✅ `cf_clearance` (HttpOnly, Secure) **was stored** on a live Cloudflare site |
| **C2 — does it survive and get sent back?** | ✅ **Survived a reload** intact |
| Are our scriptlets rewriting the functions a token is submitted with? | ✅ `fetch`, `XHR.open`, `XHR.send`, `JSON.parse`, `sendBeacon` all **stock native** on three live captcha pages |
| Does the challenge iframe see a different fingerprint than its parent? | ✅ **No** — cross-frame farbling is consistent (`farbling_iframe_check.py`, strong assertion) |
| Do we look spoofed the way Brave does? | ✅ **No** — WebGL vendor/renderer/version/extensions **byte-identical** to Chrome; HTTP/2 fingerprint identical; header set **and order** identical |
| `--disable-gpu-compositing` — the old lead | ✅ **Not the cause.** Proven by direct manipulation in both browsers |

⇒ ⛔ **C1 and C2 are ruled out on evidence. C3 is untested and needs a site that actually challenges.**

⚠️ **Two near-misses, recorded so they are not repeated.** A signal sheet was collected from the
**`tablistpanel` overlay** instead of a tab and reported a headless-looking `screen` of `[340,480]`
(real answer: identical to Chrome). And a `cf_clearance` present in Hodos but not Chrome looked like
differential treatment until clearing cookies showed it was **stale from our own earlier testing**.
Both were caught before being reported; both are why the subject gate now hard-fails.

---

## 4. Every variant, ranked by how many users hit it

⭐ **This is the comprehensive part.** The ranking is by user impact, not by interest. Everything below
the line is **deliberately not run**, and says why.

| # | Variant | How common | What it looks like when it fails | Status |
|---|---|---|---|---|
| 1 | **Invisible / silent check** (Cloudflare non-interactive, reCAPTCHA v3, AWS WAF Challenge) | ⭐ **Highest.** Runs on a large share of the web; most users never see it | The site is just slow, empty or broken. **No puzzle is ever shown** | 🎯 **IN SCOPE** |
| 2 | **"Just a moment / Checking your browser" interstitial** | ⭐ **Highest visible.** The most common thing a user actually sees | **Reload cycle.** ⇒ the most likely match for the reported symptom | 🎯 **IN SCOPE** |
| 3 | **Checkbox** ("I'm not a robot", Turnstile managed) | High — logins, signups, contact forms | Ticks, spins, resets, repeat | 🎯 **IN SCOPE** |
| 4 | **Image grid** (escalation from 3) | Moderate — the visible escalation | Endless new grids; **or blank tiles**, which is adblock eating the vendor's images | 🎯 **IN SCOPE** |
| — | — | — | — | — |
| 5 | **Silent XHR-level challenge** (403/428 on a background request) | Uncommon but nasty | The page looks broken and **no check is ever shown** — anyone testing "did I get a CAPTCHA" scores it as a pass | ⬜ NOT RUN — covered instead by the §5 instrumentation, which catches it without a sitting |
| 6 | **Slider / drag** (GeeTest, DataDome) | Low in our market | Piece will not drop | ⬜ NOT RUN — 🚩 would stress our mouse-coordinate path, but too rare to spend the sitting on |
| 7 | **Rotate / orient** (Arkose) | Low — mainly x.com / GitHub **sign-up**, not everyday browsing | Puzzle renders wrong or drag fails | ⬜ NOT RUN — 🚩 canvas/WebGL are farbled, so worth revisiting *if* a report points here |
| 8 | **Proof-of-work gate** (Akamai `sec-cpt`, Kasada) | Low, and no public demo exists | Hangs or cycles | ⬜ NOT RUN — no reachable test surface |
| 9 | **Queue / waiting room** (Ticketmaster) | Low, event-driven | Never advances | ⬜ NOT RUN |
| 10 | **Full-page block** (1020, "unusual traffic") | Low | Instant refusal, no way through | ⬜ NOT RUN — it is a verdict, not a mechanism; nothing for us to break |
| 11 | **Audio fallback** | ⛔ **Effectively never.** The accessibility path | Distorted or silent | ⛔ **OUT OF SCOPE.** 🚩 It is the one place a thing we farble (WebAudio) is itself content a human must understand, so it is a real *accessibility* question — but it is **not this phase's problem** and chasing it is what derailed the first pass. ⇒ its own ticket if anyone ever wants it |

---

## 5. Deliverables

### `P13-I` — instrumentation, so the next report is diagnosable 👤 *owner-chosen, 2026-09-21*

⭐ **The reasoning:** three causes tested clean and the bug does not reproduce here. We cannot fix what
we cannot see, and guessing a fix for a cause measured clean is how this phase went wrong the first
time. So the deliverable is to make the **next** occurrence produce evidence instead of a description.

| Row | What |
|---|---|
| `P13-I1` | When `CookieBlockManager` blocks a cookie whose name matches a known clearance cookie (`cf_clearance`, `__cf_bm`, `datadome`, `_px3`, `_pxvid`, `sec_cpt`, `_GRECAPTCHA`), log it **distinctly** — not buried in the ordinary blocked-cookie stream |
| `P13-I2` | Log `403` / `429` / `503` responses from known challenge hosts, with the **Cloudflare Ray ID** when present — that is the one error id a site's admin can actually look up |
| `P13-I3` | ⛔ **Negative control:** both must be shown firing. `I1` by blocking a synthetic clearance-named cookie; `I2` by pointing at a URL that returns 403. A log line nobody has seen fire is not instrumentation |

⚠️ **Constraints.** Release logging is `warn` and `TICKET_production_debug_logging_unbounded.md` is open
— these must be **bounded** and must not reintroduce that. ⛔ And they must not log cookie **values**,
only names: a clearance cookie value is a credential.

### `P13-M` — the mainline human sitting

Variants **1–4** only, in `HUMAN_TEST_QUEUE.md` `W11`. Hodos and Chrome side by side, same network,
same minute. Four outcomes, because pass/fail cannot express the one that matters:

✅ **pass** · 🟠 **harder** (challenged where Chrome was not) · 🔴 **loop** (solved, asked again) · ⬛ **broken**

⛔ **Being escalated to a puzzle is NOT a failure. Being asked again after solving it is.**

When anything is 🔴 🟠 or ⬛, run the three steps — Chrome → farbling off for that site → adblock off for
that site. **Which step fixes it is the diagnosis.** (`manual-test/HOW_TO_TEST_BY_HAND.md`.)

### `P13-R1` — the farbling acceptance battery still passes after any fix

A bot-compat fix must not silently weaken farbling. The constant-seed bug was exactly a "fix" nobody
measured.

---

## 6. Explicitly NOT in this phase

| | Why |
|---|---|
| The 210-cell vendor matrix | The first pass proved vendor **demos** cannot decide anything: Chrome 0.9, Hodos 0.9, and Hodos launched with `--enable-automation` also **0.9**, because the reCAPTCHA demo's score is a *sample*. A demo is configured to succeed and has no money behind the decision |
| Adblock exceptions for reCAPTCHA / hCaptcha / Arkose / GeeTest | ⚠️ The gap is **real** — `hodos-unbreak.txt` has **Cloudflare only**. But scriptlets measured **not** to be patching anything on live captcha pages, so adding them now is a fix for a cause measured clean. ⇒ add only if `P13-M` step 3 points here |
| The `Sec-CH-UA` brand (`Hodos;v=150`) | Engine fix, queued in `DevOps-CICD/NEXT_CHROMIUM_BUILD.md` PART 2. ⚠️ **Nothing has been observed to fail because of it** |
| The wallet-bridge globals | `../TICKET_wallet_bridge_plumbing_is_advertised_to_every_site.md` — a **privacy** ticket, not a bot-detection one |
| Audio, slider, rotate, queue, PoW | §4, below the line |

---

## 7. What would settle it fastest

⭐ 👤 **Get the original complainant to name the site — and which build they were on.** One site that
really failed is worth more than everything above, because it is the one thing none of this can
manufacture: a real deployment, under real risk, with a known-bad outcome. And if they were on
`0.3.x`, §3 already says the cause is gone — ask them to retry before anyone spends another day.
