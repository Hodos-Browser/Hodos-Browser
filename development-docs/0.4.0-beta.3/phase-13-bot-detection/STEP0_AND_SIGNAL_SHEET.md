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
