# P13 — Step 0 (the collapse test) and the Block B signal sheet

**Measured 2026-09-21**, head `530194f`, Windows. Rows `P13-A1`, `P13-A2`, `P13-B1`…`P13-B4`.
Harnesses in this folder: `p13_signals.py` (Hodos), `p13_chrome.py` (Chrome control),
`farble_0.3.x.js` (the 0.3.x script, extracted verbatim).

---

## The instrument, written down BEFORE the first measurement — as the phase prompt required

| | |
|---|---|
| **Subject** | Hodos **dev build at head**, `cef-native/build/bin/Release/HodosBrowser.exe`, `HODOS_DEV=1`, `--profile=Default`, CDP on **9322** |
| **Control** | **Chrome 152.0.7977.78**, stock, same machine, same day, same network, fresh `--user-data-dir`, CDP on **9333** |
| **Why CDP is legitimate for these rows** | Every signal below is **instrument-insensitive**: a UA string, a `sec-ch-ua` header, a WebGL renderer string, a TLS ClientHello and a `toString()` result do not change because a debugger is attached. ⛔ This argument does **not** extend to challenge verdicts (Block C), which are behaviour-scored — those stay human-driven |
| **One client-side concession, stated** | `websocket-client` sends an `Origin` header and Chromium 403s Origin-bearing CDP upgrades. Hodos dev already passes `--remote-allow-origins=*`; for **Chrome** the fix was `suppress_origin=True` **in the client**, deliberately *not* a launch flag — a positive control may not be a non-stock Chrome |

### 🚨 A false premise in the session prompt, corrected

The prompt offered *"the installed release build is the honest subject (`0.4.0-beta.2` is installed,
and it binds no port)"*. **It binds 9222.** `v0.4.0-beta.2` = `0da5262`, **2026-08-17**; the D2
dev-only gate is `67a9ab6`, **2026-09-14**. The installed build predates the gate by four weeks.

📏 Confirmed empirically, not just by date arithmetic: `http://127.0.0.1:9222/json` on the owner's
running installed browser returns **81 targets**, including `https://www.linkedin.com/feed/` and
`https://x.com/home`. ⇒ **The release subject the prompt named does not exist on this machine**, and
`TICKET_cdp_port_open_in_release.md` is fixed at head but **live on the build the owner is using**.

---

## ⛔ The wrong-subject defect this phase produced, and the gate now standing in front of it

📏 The first full Block B sheet was collected from the **`tablistpanel` overlay**, not a tab. It
reported `screen: [340, 480, 340, 480, 32]` with `innerWidth == outerWidth == screen.width` — which
reads **exactly** like the degenerate-metrics headless signature the literature names, and which I was
one paragraph away from writing up as the phase's headline finding. On a real tab, `screen` is
`[1920, 1080, 1920, 1032, 24]`, **identical to Chrome's**.

⛔ **`resolve_tab`'s ambiguity check did not fire and is not sufficient on its own.** The `tab-list`
overlay was created *after* `snapshot_targets` froze the landscape, and the pinned tab's id went away
— leaving the overlay as the *sole* candidate. One candidate, no ambiguity, no error, confident wrong
answer. This is the fourth harness in this family to drive the wrong browser.

⭐ **The fix, now in `p13_signals.py :: assert_tab`:** the shell's own log is the only instrument that
names the role, so it is a **hard gate** — `check_role_in_log(<log dir>, host)` must return `tab_*` or
the run raises. Its negative control was run against the known-bad line already in the log:

```
--- NEGATIVE CONTROL: the gate on known-bad input (iana.org was served to tablistpanel) ---
GREEN: gate fired -> SUBJECT WRONG: iana.org was served to role=tablistpanel, not a tab.
```

⚠️ Second-order cause worth keeping: **all 12 overlay targets must already exist before the
snapshot.** A short settle window is not a style preference — it is what makes the exclusion set
complete.

---

## `P13-A1` — the 0.4.0 half of the Step 0 discriminator ✅ PASS

⭐ **Reused, not rewritten** — `farbling_acceptance_battery.py` already carried it.

```
    BOT-1     webdriver=False (boolean)  window.chrome=True keys=loadTimes,csi,app
    toString  getImageData native=True  readPixels native=True
    ...
    BOT-1 (webdriver / window.chrome)  PASS
VERDICT: PASS      (engine Chrome/150.0.7871.187)
```

Subject asserted: `🌐 Resource request: https://example.com (role: tab_1)`.
Its validators' own negative control (`--self-test`) passes — including *"getImageData not
[native code] → rejected OK"*, which is the assertion that matters here.

## `P13-A2` — the 0.3.x half, and the Step 0 verdict ✅ the mechanism is GONE

The 0.3.x script (`v0.3.0-beta.29:cef-native/include/core/FingerprintScript.h`, extracted verbatim,
seed substituted) replayed in the current engine, **both reads in one page context** so no reload can
wipe the injection. Subject asserted `role=tab_1`.

| tell | 0.4.0 now | after the 0.3.x script |
|---|---|---|
| `toString` reports `[native code]`: `getImageData` | ✅ true | 🔴 **false** |
| …`toDataURL` | ✅ true | 🔴 **false** |
| …`toBlob` | ✅ true | 🔴 **false** |
| …`readPixels` | ✅ true | 🔴 **false** |
| …`getChannelData` | ✅ true | 🔴 **false** |
| …`getFloatFrequencyData` | ✅ true | 🔴 **false** |
| `navigator` **own**-property count | ✅ 0 | 🔴 **2** — `plugins`, `webdriver` |

**7 distinct tells**, and the last one is independent of `toString` entirely: the 0.3.x script put
`plugins` and `webdriver` as **own** properties of the `navigator` *instance*, where stock Chrome
carries them on `Navigator.prototype`. `Object.getOwnPropertyNames(navigator)` finds that in one line.

⭐ **This is also the negative control for the 0.4.0 column.** Same probe, same tab, same run, goes red
the moment the old implementation is present — so *"0.4.0 reports native"* is a measurement, not a
tautology.

⛔ **LIMIT, and it must not be dropped when this is quoted:** this is the 0.3.x **script** in a 0.4.0
**engine**. It demonstrates the mechanism the 0.3.x build carried. It is **not** an A/B against the
0.3.x binary — no 0.3.x build exists on this machine, and the installed `0.4.0-beta.2` (2026-08-17)
**post-dates** `FingerprintScript.h`'s deletion (`e446a86`, 2026-08-10), so it is not a 0.3.x
equivalent either.

### ⇒ Step 0's answer

👤 The owner's reading was right: **the defect the complaint described is the one 0.4.0 already
deleted**, and that is now demonstrated rather than assumed. The *reported* defect is closed.
The *category* is not — see Block B.

---

## Block B — the signal sheet. Hodos (tab_1) vs stock Chrome 152, same machine, same day

### Four residual differences

| # | Signal | Hodos | Chrome 152 | Family | Read |
|---|---|---|---|---|---|
| `B1` | **`sec-ch-ua` carries no Chrome brand while the UA string claims Chrome** | `"Not;A=Brand";v="8", "Chromium";v="150"` | `"Chromium";v="152", "Not?A_Brand";v="24", "Google Chrome";v="152"` | B — environment consistency | 🔴 **Highest.** This is an *internal* inconsistency, not merely a difference: `navigator.userAgent` says `Chrome/150.0.0.0`, the Client Hints say no Chrome. Exactly the shape CreepJS-style "lie detection" scores. ⚠️ Every shipping Chromium derivative adds its own third brand (Brave, Edge, Vivaldi); two brands is what a **bare Chromium / headless** build reports |
| `B2` | **6 non-standard `window` globals on every page** | `hodosBrowser`, `cefMessage`, `__hodos_walletResponse`, `__hodos_walletResponseChunk`, `__hodos_walletCall`, `CWI` | none | G — modified environment | 🟠 High. Mechanically identical to how automation frameworks are caught by their own markers (`cdc_…`, `window._selenium`). A detector does not need to know what `hodosBrowser` is — only that a browser claiming to be Chrome has globals Chrome does not. ⚠️ Present on `example.com`, which is not a dApp |
| `B3` | **`outerWidth/Height == innerWidth/Height`** — no browser chrome accounted for | `[1910, 926]` and `[1910, 926]` | outer `[945, 1012]`, inner `[929, 917]` | B — environment consistency | 🟡 Medium. Named in the literature as an automation signature ("a 1:1 outer/inner window size"). Structural: a Hodos tab is its own CEF browser filling the webview, with no knowledge of the surrounding chrome |
| `B4` | Engine is **2 majors behind**; JA4 differs by exactly one TLS extension | UA `Chrome/150.0.0.0`, JA4 `t13d1516h2_8daaf6152771_806a8c22fdea` | UA `Chrome/152.0.0.0`, JA4 `t13d1517h2_8daaf6152771_cb7bf5808d99` | B / D | 🟢 Low–medium. ⭐ The **cipher** component is identical (`8daaf6152771`); the whole delta is Chrome sending extension **`0xca34`**, which we do not (`1516` vs `1517` extensions). That is a Chrome-151/152 addition, i.e. the version gap — **not** something Hodos does. Our UA (150) and our TLS (150-shaped) are **mutually consistent**; the exposure is being old, not being incoherent |

### Everything that came back clean — and it is most of the sheet

| Signal | Result |
|---|---|
| `navigator.webdriver` | `false`, and **not** an own property · no `cdc_`/`_selenium`/`__webdriver` keys |
| `window.chrome` | present, same keys as Chrome (`loadTimes`, `csi`, `app`) |
| `navigator.plugins` / `mimeTypes` | **identical lists**, 5 and 2 — not the empty arrays headless reports |
| `screen`, `devicePixelRatio` | **identical** — `[1920, 1080, 1920, 1032, 24]`, dpr 1 |
| **WebGL vendor / renderer / version / extension count / `MAX_TEXTURE_SIZE`** | **byte-identical** to Chrome, WebGL1 *and* WebGL2, including `ANGLE (Intel, Intel(R) UHD Graphics (0x00004688) Direct3D11 vs_5_0 ps_5_0, D3D11)` |
| HTTP/2 fingerprint (Akamai) | **identical hash** `52d84b11737d980aef856699f885ca86` |
| Request header **set and order** | **identical**, `:method` → `priority`. `accept`, `accept-encoding`, `accept-language` all byte-identical. No DNT/GPC injected at defaults |
| timezone / locale / languages / platform / `navigator.vendor` | identical |
| `deviceMemory` | `32` — **and Chrome 152 also reports 32 on this machine.** ⚠️ I expected the spec's clamp at 8 and was wrong; checked before reporting. No finding, and the battery's `{4,8,16,32}` set is not too loose |
| `hardwareConcurrency` | Hodos **10**, real **24**. This is C6 farbling working as designed and inside the plausible band — it is the **Brave tension**, not a defect. See "the design question" below |

### `P13-B4` — `--disable-gpu-compositing`, the README's "first thing to check" ✅ NOT the problem

Answered by **direct manipulation in both browsers**, not by inference:

```
  chrome, no flag      : ANGLE (Intel, Intel(R) UHD Graphics (0x00004688) Direct3D11 vs_5_0 ps_5_0, D3D11)
  chrome, WITH flag    : ANGLE (Intel, Intel(R) UHD Graphics (0x00004688) Direct3D11 vs_5_0 ps_5_0, D3D11)
  hodos  (ships flag)  : ANGLE (Intel, Intel(R) UHD Graphics (0x00004688) Direct3D11 vs_5_0 ps_5_0, D3D11)
  chrome webgl1/webgl2 identical with and without the flag : True / True
  hodos == chrome(no flag)                                 : True / True
```

⚠️ **Scope:** one machine, one GPU (Intel UHD 630-class, D3D11). A discrete-GPU machine could differ,
and this says nothing about the flag's effect on *other* GPU-adjacent signals. But the specific worry
the README recorded — that the flag mangles the renderer string — is **measured false here**.

---

## An incidental live observation, labelled as such

⚠️ **Not a controlled test. Do not cite it as a matrix cell.** While enumerating targets on the
owner's **installed** `0.4.0-beta.2`, the target list contained:

- `https://www.google.com/recaptcha/enterprise/anchor?ar=2&k=6LcIy_MqAAAA…` — a live **reCAPTCHA
  Enterprise** iframe on `linkedin.com`, which the research doc recorded as having *no public demo*;
- `https://www.linkedin.com/feed/` and `https://x.com/home` — both logged-in and in use.

⇒ Weak but real evidence that a Hodos build passes reCAPTCHA Enterprise and X in ordinary use. Weak
because it is beta.2 (not head), uncontrolled, and says nothing about *how close* to a challenge it ran.

---

## The design question, reported rather than decided (rule 5)

⭐ **Brave and Tor disagree, and the disagreement IS the question.** `B1` and `hardwareConcurrency`
are the same tension from two directions:

- **Brave** randomises, and then has to ship per-site Shields toggles because randomised values that
  differ from the machine's real ones score as suspicious. `brave/brave-browser#58915` has Cloudflare
  logging *"WebGL renderer info is spoofed/blocked (unmasked renderer: Brave)"* — the interference flag
  fired even though the challenge passed.
- **Tor Browser** chooses **uniformity** — everyone identical — for the same threat.
- ⭐ **castle.io's four-step method is the sharpest thing in the literature for us:** detect
  instability → bucket into *known randomising browsers* (it names Firefox, Brave, Samsung) vs
  *abnormal* → don't penalise the known-good randomisers. ⇒ **the safe path for a randomising browser
  is being recognised by name.**

⛔ That is precisely what `B1` denies us. We farble like Brave, and we present as a **bare Chromium
with no brand of its own**. We are structurally more likely to land in the "abnormal, raise score"
bucket than Brave is — and unlike Brave we are not on anyone's allowlist.

👤 **Not deciding this here.** Adding a `"Hodos"` brand to `sec-ch-ua` is a two-edged change: it makes
us internally consistent and honest, and it also makes us *nameable* and trivially targetable. That is
an owner call, not an implementation detail.

---

## What is still owed

| Row | State |
|---|---|
| `P13-C*` — the verdict matrix (Turnstile forced-interactive, reCAPTCHA v3 score, v2 checkbox, hCaptcha) | ⬜ **Not run.** 👤 Owner's scope decision: run only if Step 0 showed a failure. It did not. The rows are specified in `HUMAN_TEST_QUEUE.md` so the sitting can happen at any time |
| A release-shaped subject at head | ⬜ Needs a copied-out binary launched **without** `HODOS_DEV=1`. ⛔ `AppPaths` rule 2 scrubs a stray `HODOS_DEV=1` off a non-build-path binary and there is **no data-root override** ⇒ it will use `%APPDATA%\HodosBrowser`, the owner's **production profile**. Owner approved a back-up-and-restore, but it also **collides with the running installed browser's SingletonLock**, so it needs the owner to close their browser first |
| Adblock vendor coverage | ⚠️ `hodos-unbreak.txt` has **Cloudflare only**. No reCAPTCHA, hCaptcha, Arkose or GeeTest exception exists. Measure before adding — that is what the matrix is for |
| 🍎 macOS parity | ⬜ The same two harnesses run unchanged; `--disable-gpu-compositing` is passed on both platforms and the GPU stack differs |

---

# Addendum, same day — 👤 the owner asked the right question: **look at Brave, not Chrome**

Rule 5 names Brave as ⭐ *closest to us in intent*. It is installed on this machine, so this is
**measured**, not recalled. Same probe, same machine, same day.

## What each browser tells a site it is

| | UA string (tail) | `sec-ch-ua` brand list |
|---|---|---|
| **Hodos (head)** | `Chrome/150.0.0.0 Safari/537.36` | `Not;A=Brand;v=8, Chromium;v=150` |
| **Chrome 152** | `Chrome/152.0.0.0 Safari/537.36` | `Chromium;v=152, Not?A_Brand;v=24, Google Chrome;v=152` |
| **Brave 153** | `Chrome/153.0.0.0 Safari/537.36` | `Brave;v=153, Not_A Brand;v=8, Chromium;v=153` |

*(Edge 153 is installed but its target set defeated the one-page-target assertion; not measured rather
than guessed.)*

⭐ **Brave's answer is a deliberate SPLIT, and it is not what either half of the room assumes.**
Its **UA string carries no "Brave" token at all** — it is byte-shaped like Chrome's. Its **Client
Hints do name Brave.** So 👤 the owner's recollection (*"Brave just sends Brave"*) is half right: it
names itself in the new, structured, opt-in header and **lies in the old one that every server parses**.

⛔ **And Brave is internally CONSISTENT.** `Chrome/153` + `Chromium;v=153` + `Brave;v=153` compose into
one coherent claim: *"I am Brave, which is Chromium 153, presenting as Chrome 153."* Nothing contradicts.

⇒ **`B1` is not "we differ from Brave". It is that we are the only one of the three that contradicts
itself.** Our UA says Chrome; our brands say a raw, unbranded Chromium with no Chrome in it. Those two
cannot both be true. That pairing is the CEF/Chromium **default**, so the crowd we are standing in on
the open web is embedded webviews, CEF-based scrapers and automation — not browsers.

## Cost of fixing it — measured, and it is not where I expected

📏 Launched the dev build with `--user-agent-product=Chrome/150.0.0.0` (subject asserted `role=tab_1`):

```
  userAgent : ... Chrome/150.0.0.0 Safari/537.36      <- changed
  sec-ch-ua : Not;A=Brand;v=8, Chromium;v=150          <- UNCHANGED
```

⛔ **CEF's UA controls (`CefSettings.user_agent`, `user_agent_product`, and the matching switches) do
not reach the Client Hints brand list.** The brand list is built inside Chromium from the branding
build flag and product name. ⇒ **`B1` is an ENGINE fix, not a shell fix** — it belongs in
`DevOps-CICD/NEXT_CHROMIUM_BUILD.md` §PART 2, per the standing rule that engine-only fixes are queued
the day they are found.

⚠️ **The tempting shortcut is a trap.** We already rewrite request headers in C++ (DNT/GPC), so
rewriting the `sec-ch-ua` **header** looks cheap. It would fix what the server sees and leave
`navigator.userAgentData.brands` in JS saying the old thing — so the header and the JS would then
disagree. That is a **new** inconsistency, in the exact family the signal belongs to, and strictly
worse than doing nothing. ⛔ Do not do this.

## The two signals pull in opposite directions, and saying which is "worse" depends on the question

| | `B1` — the brand list | `B2` — `window.hodosBrowser` / `CWI` / `__hodos_*` |
|---|---|---|
| **"Do I look automated?"** | 🔴 **Worse.** Raw-Chromium-claiming-Chrome is an automation correlate | 🟡 Milder — real users run extensions that add page globals |
| **"Who exactly are you?"** | 🟡 Says a *category* (some Chromium build), not us | 🔴 **Worse.** Says **Hodos**, by name, uniquely, and that the user runs a BSV wallet — to every site, including ones that never asked |
| **Where the fix lives** | ⛔ Engine patch (measured above) | ✅ **Our own code** |

⇒ ⭐ **The one we can act on now is `B2`, and it is also the one that actually deanonymises the user.**

---

# `P13-C` first pass — 🔴 **INCONCLUSIVE BY CONSTRUCTION. The negative control did not go red.**

Run 2026-09-21 at the owner's request (*"I guess let's run the matrix"*), harness
`p13_challenge.py`. ⛔ **Read the verdict before the table.**

## The result

| Page | Chrome 152 | Hodos defaults | 🔴 Hodos **+ `--enable-automation`** |
|---|---|---|---|
| reCAPTCHA **v3** (score) | `0.9` | `0.9` | **`0.9`** |
| reCAPTCHA v2 checkbox | no challenge | no challenge | no challenge |
| hCaptcha demo | no challenge | no challenge | no challenge |
| Cloudflare live (`whatsonchain.com`) | loaded, no challenge | loaded, no challenge | *(gate caught a wrong subject — not measured)* |

📏 The control **did** take effect: `navigator.webdriver === True` was read back from a real tab
(`role=tab_1`) under `--enable-automation`, and is `False` without it. So the flag worked and the
**page did not care**.

## ⛔ Therefore this proves the pages were reachable, and nothing else

Per the Testing Standards hard rule — *a test that has never been seen to fail has not been shown to
test anything* — these four rows are **not** evidence that Hodos passes bot detection. A browser openly
declaring itself automated scored **identically** to stock Chrome. Any "all green" reading of this
table would be the fourth harness in this sprint that passes with the feature absent.

## Why, and the finding that comes out of it

⭐ **The reCAPTCHA v3 demo's score is a sample, not a live verdict.** The page says so itself, in text
the harness captured:

> *"NOTE: This is a sample implementation, the score returned here is not a reflection on your …"*

`0.9` in all three columns is that sample. ⛔ **`recaptcha-demo.appspot.com` cannot serve as the
matrix's instrument**, which is exactly what `W11` had been written around — my error, corrected there.

⚠️ The other three rows are *demo* pages too. A vendor demo is configured to succeed; it is not a site
with money behind it deciding whether to trust you. ⇒ **The matrix needs real deployments under real
risk, which is what makes it human-bound** — not merely the image grids.

## The subject gate earned its keep a second time

📏 `cloudflare-live` in the control run: `SUBJECT WRONG: whatsonchain.com was served to role=header,
not a tab.` Without the gate that would have been silently recorded as a Cloudflare pass measured on
the **header browser**.

## What this changes

- ⛔ **`P13-M1` is NOT satisfied.** The matrix stays open.
- ⭐ **It does not reopen Step 0.** Step 0's discriminator is a different, *instrument-insensitive*
  measurement with its own working negative control (the 0.3.x replay goes red). Nothing here weakens it.
- 👤 **The owner's own proposal is now the highest-value next step, by some distance:** *"I'll try to
  specifically get the user to let me know what site it was on."* One real site that actually failed is
  worth more than any number of vendor demos — it is a subject under real risk, with a known-bad
  outcome, i.e. the negative control this matrix could not manufacture.

---

# 🚨 `P13-M1` HUMAN SITTING, 2026-09-21 — one reproduction out of twelve sites, and a rig confound that may explain it

👤 Owner at the keyboard, dev build, farbling + adblock + third-party-cookie-blocking all ON.

## Results

| Site | Vendor | Result |
|---|---|---|
| whatsonchain, indeed | Cloudflare | ✅ no check at all |
| 2captcha reCAPTCHA v2 | reCAPTCHA | ✅ **passed** — checkbox + submit returned true |
| hCaptcha demo | hCaptcha | ✅ **passed, including the escalation** — 👤 *"it escalated to some tests and that passed as well"* |
| nytimes, zillow, bestbuy, walmart, ticketmaster | mixed | ✅ never challenged |
| amazon (full login) | AWS WAF | ✅ never challenged |
| **github.com signup** | **DataDome** | 🔴 **LOOP, then hard block** |

⚠️ **Eleven of twelve never triggered a check at all**, which the owner correctly called out as the
weakness of the whole exercise: *"nothing ever even triggered it."* A green from a site that never
challenged you is not a pass, it is a no-op.

## The one reproduction, in our own logs

⭐ **It was DataDome, not Arkose** — `geo.captcha-delivery.com` / `ct.captcha-delivery.com` is
DataDome's challenge delivery. (GitHub's octocaptcha front door layers DataDome *and* Arkose.)

📏 From `debug_output-25440.log`:
- **105 requests** to `captcha-delivery.com` between **10:57:46 and 11:00:42** — 55 GET, 50 POST,
  alternating roughly once a second. A completed challenge is a handful of requests.
- **Nine top-level reloads of `github.com` in 4.3 seconds** (10:58:38.5 → 10:58:42.8), each one
  re-running `Cosmetic P1` and re-injecting the bridge.
- Then **72 seconds of total silence** (10:58:42 → 10:59:54) — 👤 *"it was just black for like a full
  minute"*.
- Ending in DataDome's block page, with its own id `becc5957-01a5-1263-773b-673d780e41a9`.

## What the loop is NOT

| Ruled out | Evidence |
|---|---|
| Our cookie blocker eating the pass | 📏 The `datadome` cookie **is in the store**, first-party on `.github.com`. It was never blocked |
| Our adblock blocking the vendor | 📏 Zero blocked requests or cookies logged for `captcha-delivery.com`; `Cosmetic P1: css=0 script=0` on github.com |
| "DataDome rejects Hodos" | 📏 `datadome` cookies are **also in the store for `.www.nytimes.com`**, which the owner browsed **with no challenge at all.** Same vendor, two sites, one fine |

## 🚨 The confound — and it is mine, not the product's

DataDome's block page lists its reasons, and two of them are about the **test rig**, not the browser:

> Rapid taps or clicks · JavaScript disabled or not working ·
> **Automated (bot) activity on your network (IP 64.93.120.110)** ·
> **Use of developer or inspection tools**

1. ⛔ **The dev build binds CDP on 9322 and passes `--remote-allow-origins=*`. Release binds nothing**
   (Phase 9, D2). And `RESEARCH_steps_1_2.md` §A already records that **DataDome published research in
   Feb 2026 specifically on detecting the CDP wire protocol** — `datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/`.
   ⇒ **We ran the one vendor publicly known to detect debug ports against a build with a debug port open.**
   🚨 The session prompt warned about exactly this in capital letters — *"a red cell might be your
   instrument"* — and the sitting was still set up on the dev build. That is on me.
2. The owner fumbled the slider and retried, which is itself a listed reason.
3. The IP is named in the block, so it may be flagged independently of the browser.

## ⇒ The one test that settles it

Re-run **github.com signup** on a build with **no debug port**, same machine, same IP. Blocks again ⇒
real defect. Passes ⇒ it was the rig, and this never affects a shipped user.

⛔ **Until that runs, this row is NOT evidence of a product defect.** It is a reproduction of a loop
under conditions no released build has.

⚠️ Also worth a Chrome control on the same IP: if Chrome is blocked too, the IP is flagged and the
browser is irrelevant.

## ⭐ What it changes regardless

- **DataDome is the vendor to care about**, not Cloudflare. Cloudflare is the one we already tuned, and
  every Cloudflare site in the sitting sailed through. ⇒ the §4 ranking should put DataDome first.
- **The slider is variant #6**, marked `NOT RUN` in §4 as too rare. It is the one that broke. ⚠️ Note
  the mouse-path worry in that row is **weaker than written**: tab browsers are *windowed* CEF browsers
  and get native mouse handling, whereas the Phase 11 DPI conversion bug was in *windowless overlays*.
- ⛔ **Eleven no-ops out of twelve** means site lists are a poor instrument. What produced the one
  result was a **sign-up flow**, which is where vendors are tuned hardest.
