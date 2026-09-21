# Testing CAPTCHAs by hand — where to go, what to look for

👤 Written 2026-09-21 for the owner's question: *"there has to be a site I can just go to that we know
will trigger a CAPTCHA and try it."*

> ⭐ **Start at `CHALLENGE_TYPE_COVERAGE.md` instead of here if you want the full picture.**
> 👤 Owner, 2026-09-21: *"I need to test all of these in a comprehensive assessment of the whole
> CAPTCHA prove you're a human thing."* Fair — this file lists *sites*, which is only half the job.
> There are **eleven** variants of human-verification and we had exercised **one** (checkbox). The
> three worth doing first are **audio fallback, rotate/orient and slider**, because those are the ones
> where something Hodos specifically modifies — WebAudio farbling, canvas/WebGL farbling, and our mouse
> coordinate path — could plausibly be the cause. None of them has ever been tested.

## ⚠️ First, the honest caveat — there is no site that challenges everybody

That is the whole design of these systems. A clean residential IP on a real browser is *supposed* to
sail through. So **"I wasn't challenged" is the normal result and proves very little on its own.**

That is also why the automated matrix came back inconclusive (`STEP0_AND_SIGNAL_SHEET.md`): every
vendor **demo** page is configured to succeed, so even a browser launched with `--enable-automation`
scored 0.9 out of 1.0.

⇒ **Everything below is "run Hodos and Chrome side by side and compare."** Chrome is the control. If
Chrome gets challenged too, the site is just like that and we learned nothing about us.

---

## ⭐ The failure mode to watch for is a LOOP, not a block

This matters more than the site list. The documented Brave-vs-Cloudflare failure
(`brave/brave-browser#45608`) is **not** "Access Denied". It is:

- the checkbox ticks, spins, and resets — forever; or
- the "Verifying you are human" page reloads itself in a cycle; or
- the widget never appears at all.

🚨 **That is almost certainly what the original complainant hit** — *"could not pass a CAPTCHA"*, not
*"was blocked"*. So when you test, do not just ask "did I get in". Ask:

| Watch for | Means |
|---|---|
| Widget never renders | script blocked (adblock?) — try the toggle below |
| Ticks then resets, repeatedly | **the loop.** This is the one that matters |
| Reload cycle on a "just a moment" page | the loop, Cloudflare's full-page variant |
| Image grid appears | escalated — annoying but **working**; note it and move on |
| Straight through | pass |

---

## ⭐ The 3-step protocol — 2 minutes, and it localises the cause

Whenever any site challenges or loops on you:

1. **Open the same URL in Chrome.** Same machine, same network, same moment.
   Challenged there too? ⇒ the site, not us. Stop.
2. Still only Hodos? **Turn farbling off for that one site** — Privacy Shield panel, per-site toggle —
   and reload. Works now? ⇒ it is our fingerprint randomisation, and we have localised it to a single
   cause in one click.
3. Still stuck? **Turn adblock off for that site too** and reload. Works now? ⇒ a filter list is
   eating a challenge script, which is a `hodos-unbreak.txt` line, not an engine problem.

📏 **Write down which step fixed it.** That one fact is worth more than the whole automated matrix,
because it is a real site making a real decision with a known-bad starting state.

---

## A page that WILL show you a challenge, every time

`captcha-bench.html`, next to this file. Three real widgets — Cloudflare Turnstile forced into its
interactive mode, reCAPTCHA v2, hCaptcha — using each vendor's public test keys.

```
cd development-docs/0.4.0-beta.3/phase-13-bot-detection/manual-test
python -m http.server 8777
```

then open `http://localhost:8777/captcha-bench.html` in **Hodos and Chrome**.

⚠️ **What it proves:** the widgets load, run and complete in our engine — the loop failure mode.
⛔ **What it does not prove:** that a real site trusts us. Test keys always pass; no risk decision is
being made. It is a smoke test, not a verdict.

⭐ It also prints, at the bottom, the two things Phase 13 actually found — our `sec-ch-ua` line and the
extra `window` globals — so you can see them in both browsers without opening devtools.

---

## 👤 "What about the nine-squares, pick-the-bicycles kind?"

⛔ **The bench page cannot give you those, and no local page can.** Two reasons, and the second is the
hard one:

1. **Test keys always pass.** Every vendor's public test sitekey is configured to succeed. No risk
   decision is made, so nothing ever escalates to a grid.
2. ⛔ **Sitekeys are domain-locked.** A real, risk-scoring sitekey only works on the domain it was
   registered to. I cannot embed someone else's real key in a page on `localhost` — it simply refuses.
   And a key I registered myself would be brand new with no traffic, i.e. low risk, i.e. no grid.

⇒ **Image grids only come from real sites under real risk.** Here is where to get one.

### ⭐ Best single page for an image grid

**`https://2captcha.com/demo/recaptcha-v2`**

📏 Verified 2026-09-21: it serves reCAPTCHA v2 with the **real sitekey**
`6LfD3PIbAAAAAJs_eEHvoOl75_83eXSqpPSRFJ_u` — *not* a test key. It is a live widget doing live risk
scoring, on a page that captcha-solving customers hammer constantly, so it escalates to the grid far
more readily than a clean site would. Click the checkbox and you will usually get pictures.

⚠️ Two neighbours checked at the same time, so you do not waste the trip:
- `2captcha.com/demo/cloudflare-turnstile` — uses the **same `3x00000000000000000000FF` test key** our
  own bench page already uses. No advantage over running locally.
- `2captcha.com/demo/hcaptcha` — **does not exist**; it redirects to their homepage.

### Then the sign-up flows, which are where grids really live

`x.com` sign-up gives Arkose's rotating-image puzzle very reliably. `github.com/signup` gives the
Arkose grid. Steam account creation gives reCAPTCHA's. You do not need to complete any of them —
reaching the puzzle *is* the measurement.

### ⭐ What a grid actually tells you — and it is not what it looks like

A grid is **not** a failure. It means the vendor was unsure and asked a question, which is the system
working. ⛔ **Do not report "we got an image grid" as a defect.**

📏 The three outcomes that matter, in order of badness:

| What you see | Verdict |
|---|---|
| Grid appears, you solve it, **it accepts and you proceed** | ✅ **Working.** Escalated, but fine |
| Grid appears, you solve it, **it gives you another one, forever** | 🔴 **THE DEFECT.** This is the Brave loop |
| Grid appears in Hodos but **Chrome gets straight through** on the same network | 🟠 Worth reporting — we are being scored harder |

⇒ So when you get a grid: **solve it**, and watch what happens next. The interesting result is on the
far side of the puzzle, not the puzzle itself.

---

## Real sites that challenge readily

Ranked by how reliably they escalate. ⛔ **Visit normally. Do not hammer, script, or retry in a loop
to force a challenge** — that is abuse, and it also poisons the result by making you genuinely look
like a bot.

### Tier 1 — highest yield: **sign-up and login flows**

Vendors are tuned hardest at account creation. You do not have to finish; reaching the challenge is
the measurement.

| Site | Vendor | Note |
|---|---|---|
| `x.com` — sign-up, or login from a fresh session | Arkose FunCaptcha | The rotating-image puzzle. Reliable |
| `github.com/signup` | Arkose via GitHub's own `octocaptcha.com` | 📏 First-party confirmed: GitHub's docs tell firewalled users to allow `octocaptcha.com` + `arkoselabs.com` |
| `steamcommunity.com` / Steam account creation | reCAPTCHA | Long-standing, challenges readily |
| `reddit.com` register | hCaptcha (commonly reported) | ⚠️ unconfirmed first-party — verify while you are there |

### Tier 2 — live Cloudflare, the vendor most likely to be our problem

| Site | Note |
|---|---|
| `whatsonchain.com` | ⭐ **Our own basket already uses it**, and it is Turnstile-protected. Loaded clean in both browsers on 2026-09-21 |
| `bsvalias.org` | 📏 Recorded in `PRIOR_ART.md` as sitting behind a Cloudflare JS challenge — a fetch got *"Just a moment"* 403 |
| `indeed.com` | Cloudflare, challenges readily |
| Any site showing a **Ray ID** on an error page | That is Cloudflare. Note the Ray ID — it is the one error id you can actually quote |

### Tier 3 — retail / ticketing, aggressive but noisy

| Site | Vendor |
|---|---|
| `ticketmaster.com`, `stubhub.com` | Imperva / queue systems |
| `amazon.com` — rapid search or login | AWS WAF CAPTCHA (Amazon dogfoods its own) |
| `nytimes.com` | HUMAN / PerimeterX (⚠️ commonly reported, not first-party confirmed) |

### Tier 4 — Google's own interstitial

`google.com/sorry` cannot be summoned on demand. It appears organically on a shared IP, a VPN exit, or
an unusual query pattern. ⛔ Do not try to provoke it. If you happen to hit it, that is a data point.

---

## ⭐ Still the highest-value thing you can do

Get the **original complainant to name the site** — and which build they were on. One site that really
failed beats this entire list, because it comes with the thing none of these have: a known-bad
outcome to reproduce against.

And if they are still on `0.3.x`, Step 0 already says the cause is gone — in which case ask them to
retry on the current build before anyone spends another day on this.
