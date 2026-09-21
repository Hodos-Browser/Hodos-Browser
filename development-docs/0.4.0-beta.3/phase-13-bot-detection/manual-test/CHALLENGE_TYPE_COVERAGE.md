# The whole "prove you're a human" category — every variant, and which ones we have actually tested

👤 Opened 2026-09-21 on the owner's point: *"I need to test all of these in a comprehensive assessment
of the whole CAPTCHA prove you're a human thing."* He is right — the coverage so far was **one and a
half variants out of eleven**, and the half was worthless.

## ⛔ Honest coverage as of today

| | |
|---|---|
| Actually exercised | **Checkbox** (on test keys, which always pass) |
| Exercised and worthless | **Invisible/score** — the reCAPTCHA demo's number is a *sample*, identical for Chrome, Hodos, and a browser openly flagged as automated |
| Never exercised | **The other nine** |

---

## ⭐ The part that matters most: different variants stress different parts of OUR stack

This is why "we tested a checkbox" tells us almost nothing. A checkbox exercises signal-scoring and
token delivery. It does not touch the two things Hodos specifically modifies that could plausibly
break a challenge.

| Variant | What it leans on | 🚩 Our specific exposure |
|---|---|---|
| Invisible / score | Fingerprint plausibility, TLS, header order | 🟡 Measured clean vs Chrome (Block B) — but scores are invisible, so failure is silent |
| Checkbox | Same + token round-trip | 🟢 Lowest risk |
| **Image grid** | Loading dozens of **images from the vendor's CDN** | 🚩 **Adblock.** If a filter list eats the image requests the grid renders blank or never completes |
| **Slider / drag puzzle** | **Precise mouse coordinates through a drag** | 🚩🚩 **Our mouse path.** Phase 11 `W10` found a real DPI coordinate-conversion bug in overlays; a slider is the web-content equivalent and we have never tested a drag against a challenge |
| **Rotate / orient (Arkose)** | Canvas + WebGL rendering of the puzzle, plus drag | 🚩🚩 **Canvas and WebGL are exactly what we farble.** If farbling perturbs the rendered puzzle, *we* may not be able to see it correctly either |
| **Audio fallback** | **WebAudio** decoding of the spoken digits | 🚩🚩🚩 **We farble WebAudio** (`getChannelData`, `getFloatFrequencyData`). An audio CAPTCHA is the accessibility path — if our audio farbling distorts it, we have broken the route a blind user depends on, and nobody would ever have noticed |
| **Proof-of-work interstitial** ("Just a moment") | JS speed, timers, **workers** | 🚩 Farbling in **workers** was an explicit reason for going native; unverified against a real PoW gate |
| Full-page block (1020 / "unusual traffic") | Nothing — decision already made | 🟢 Nothing to break; it is the verdict |
| Queue / waiting room | Long-lived connection, timers | 🟡 Untested |
| Device/SMS verification | Out of scope — not a bot check | ⬜ |
| **Silent XHR-level challenge** | A background request gets 403/428 and **no puzzle is ever shown** | 🚩 **The nastiest.** The user sees a broken page, not a CAPTCHA. Anyone testing "did I get a CAPTCHA" scores this as a pass |

⭐ **The three 🚩🚩+ rows — audio, rotate, slider — are the ones worth a human sitting first.** They are
the variants where a Hodos-specific modification could plausibly be the cause, and all three are
completely untested. The checkbox we *did* test is the row with the least to tell us.

⚠️ **The audio row is the one I would look at first.** It is the only place where a thing we
deliberately perturb (WebAudio) is *itself the content the user must understand*. Everywhere else
farbling changes a value someone measures; here it changes a sound someone has to hear.

---

## The sitting checklist

For every row: **open it in Hodos and in Chrome, same network, same minute.** Chrome is the control.

### How to record a result — four outcomes, not two

| Mark | Meaning |
|---|---|
| ✅ **pass** | got through, with or without solving something |
| 🟠 **harder** | Hodos was challenged or escalated where Chrome was not |
| 🔴 **loop** | solved it correctly and was asked again — the defect |
| ⬛ **broken** | the widget never rendered, images blank, audio silent, slider won't drop |

⛔ Escalating to a puzzle is **not** a failure. Being asked *again after solving it* is.

### Rows

- [ ] **Invisible / score** — any Cloudflare-protected site loading without a visible check.
      ⚠️ Silent by nature: the only signal is the page working or subtly not.
- [ ] **Checkbox** — `captcha-bench.html` (local), or `2captcha.com/demo/recaptcha-v2`.
- [ ] **Image grid** — `2captcha.com/demo/recaptcha-v2`, click through until pictures appear.
      🚩 Watch: do the tiles actually **load**, or are some blank? Blank tiles = adblock.
- [ ] **Audio fallback** — on that same grid, click the **headphones icon**.
      🚩🚩🚩 Can you hear the digits clearly? Compare with Chrome. Distorted/silent in Hodos only ⇒ our
      WebAudio farbling, and that is a real accessibility defect regardless of bot detection.
- [ ] **Rotate / orient** — `x.com` sign-up (Arkose). Does the puzzle render right, and does dragging work?
- [ ] **Slider / drag** — `geetest.com/en/adaptive-captcha-demo`, "Slide" mode.
      🚩🚩 Does the piece follow the mouse 1:1, and does it accept the drop?
- [ ] **Proof-of-work interstitial** — any "Checking your browser… / Just a moment" page.
      🔴 Does it cycle? That is the classic loop location.
- [ ] **Full-page block** — only if it happens. Record the **Ray ID** if shown.
- [ ] **Silent XHR challenge** — ⭐ do this one differently: on a site that *feels* broken (a search that
      returns nothing, a button that does nothing), open DevTools → Network and look for a **403 or 428**.
      That is a challenge you were never shown.
- [ ] **Queue / waiting room** — `ticketmaster.com` during any on-sale.

### When anything is 🔴 or 🟠 or ⬛, run the 3 steps

1. Same URL in **Chrome** → also bad? ⇒ the site, not us.
2. **Farbling off** for that site (Privacy Shield) → fixed? ⇒ our fingerprint randomisation.
3. **Adblock off** for that site → fixed? ⇒ a filter list, and the fix is a `hodos-unbreak.txt` line.

📏 **Write down which step fixed it.** That one fact is the whole diagnosis.

---

## What already exists, so nobody re-does it

- ⭐ `hodos-unbreak.txt` carries **Cloudflare exceptions only** — `challenges.cloudflare.com#@#+js()`,
  `cf-turnstile.com`, `$generichide`. **No reCAPTCHA, hCaptcha, Arkose or GeeTest exception exists.**
  ⇒ if step 3 above fixes anything on those vendors, the fix is a one-line addition in a proven pattern.
- 📏 Already measured clean on the current build (`STEP0_AND_SIGNAL_SHEET.md`): WebGL identical to
  Chrome, HTTP/2 fingerprint identical, header set and order identical, cross-frame farbling
  consistent, and `fetch`/`XHR`/`JSON.parse` unmodified on three live captcha pages.
- ⛔ Vendors with **no public demo at all** — DataDome, HUMAN/PerimeterX, Akamai, Imperva, Kasada,
  Shape/F5. Reachable only by finding a live site that uses them. Do not burn time hunting for a demo.
