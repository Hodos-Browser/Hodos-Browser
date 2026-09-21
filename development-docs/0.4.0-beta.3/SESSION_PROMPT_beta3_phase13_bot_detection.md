# Session prompt — beta.3 **Phase 13: bot-detection compatibility**

Paste this to open the phase in a fresh context.

---

Read `development-docs/0.4.0-beta.3/phase-13-bot-detection/README.md` and its
`RESEARCH_steps_1_2.md`, then run Phase 13. Follow the phase-kickoff workflow in the root
`CLAUDE.md` and hand back the kickoff summary before writing any code.

## 🚨 STEP 0 FIRST. It may collapse this whole phase.

The complaint that opened Phase 13 came from a user on **0.3.x-beta**, which farbled by **injected
JavaScript**. That implementation carried the `toString` tamper tell — a patched-in-JS method does not
report `[native code]` — which is one of the cheapest and most widely deployed bot signals there is.
`FingerprintScript.h` was **deleted 2026-08-09**; 0.4.0 farbles natively in Blink, so the patched
methods report `[native code]` again.

⛔ **Do not design a fix before re-testing on the current build.** If nothing fails, this phase becomes
a regression guard plus a note, and the 210-cell matrix in the README is not the right shape.
👤 Owner's framing: *"if it works for us, then good. But it's still probably fingerprinting."* ⇒ a green
re-test closes the **reported defect**, not the **category**.

## ⛔ Settle the instrument BEFORE the first measurement — this phase is unusually easy to fake

**Two problems, and both produce a confident wrong answer.**

**1. 🚨 The CDP confound.** Phase 9 made the debug port **dev-only**: release binds **no** port, the dev
build binds **9322**. Driving a bot-detection measurement over CDP means attaching a debugger to a
browser with a debug port open and asking whether it looks automated. ⚠️ A red cell might be *your
instrument*. Decide, and write the decision down, before measuring:
- the **installed release build** is the honest subject (`0.4.0-beta.2` is installed, and it binds no
  port) — but then there is no CDP and the rows are **human-driven**;
- or measure the dev build over CDP and **state the confound in every row**;
- or build a release-config binary locally and drive it by hand.
⭐ Whatever you choose, the matrix must say **which browser, with which port state**, per row.

**2. ⚠️ You cannot solve a CAPTCHA, and you should not try.** The measurement is *"were we challenged,
looped, or blocked"* — **not** *"did we solve it"*. ⛔ Do not build or use CAPTCHA-solving automation.
Anything needing a real human judgement of an image grid is a **`HUMAN_TEST_QUEUE.md` row**, named as
such, with the instrument limit that makes it human-bound.

⛔ **The matrix needs its own negative control** (README step 3): one cell **made** to fail on purpose —
launch with `--enable-automation`, or flip `navigator.webdriver` through a dev seam — and **seen** to
fail. An all-green matrix without it proves the pages were reachable, not that the test can detect a bot
verdict. 📏 This sprint has shipped three harnesses that would have passed with the feature absent.

## What already exists — ⛔ do not redo it

- ⭐ **Steps 1 and 2 are DONE**: `RESEARCH_steps_1_2.md`, 241 lines — 13 vendors with demo URLs and
  pass/fail signatures, the signal inventory by family, and a prior-art source log. Confidence-tagged
  **VERIFIED / CITED / INFERRED**; ⚠️ re-check anything **CITED** before relying on it.
- ⭐ **The adblock engine already ships Cloudflare challenge exceptions** — `CONFIG_VERSION = 7`,
  *"v7 = +Cloudflare challenge exceptions"*, and `hodos-unbreak.txt` carries
  `challenges.cloudflare.com#@#+js()` plus `$generichide` and script exceptions. ⇒ One of step 4's
  expected fixes is **partly already done**; measure before adding more.
- **BOT-1** in `development-docs/0.4.0/chromium-rebuild/farbling_acceptance_battery.py` already asserts
  `navigator.webdriver === false`, the `window.chrome` stub, and `[native code]` on `getImageData` /
  `readPixels`. That is the 0.4.0 half of step 0's discriminator — **reuse it, don't rewrite it**.
- **Chrome 152.0.7977.78** is installed at `C:\Program Files\Google\Chrome\Application\chrome.exe` —
  the positive control the matrix needs. If Chrome fails a cell too, the vendor is broken, not us.

## ❓ Answer these before building the matrix

1. **Is there a 0.3.x build we can actually run?** The installed build is **`0.4.0-beta.2`**, so the
   cheap A/B the README asks for (`toString` on 0.3.x vs 0.4.0) **has no 0.3.x side on this machine**.
   Either fetch a 0.3.x-beta, or assert only the 0.4.0 half and say the mechanism is *demonstrated for
   the current build, inferred for the old one*. ⛔ Do not claim an A/B you did not run.
2. **Does `0.4.0-beta.2` (the installed build) already have native farbling?** It is the subject you are
   most likely to measure. Check it rather than assuming — if it predates 2026-08-09 it still has the
   injected-JS path, which makes it a *usable* 0.3.x-equivalent for the A/B.
3. **Scope.** 13 vendors × 5 columns × 3 runs is ~200 observations, most needing human judgement. 📏 The
   sprint is weeks behind. ⭐ Recommend to the owner a **subset first**: the vendors our own basket
   actually hits (Cloudflare, reCAPTCHA, hCaptcha, Google's interstitial), and expand only if step 0
   shows a real failure.
4. ⚠️ **`--disable-gpu-compositing` is passed unconditionally** and its effect on the WebGL renderer
   string is **unmeasured** — the README calls it the first thing to check in step 4. It is cheap.

## Prior art is mandatory (root `CLAUDE.md` rule 5)

**Brave** has this exact tension — randomised values that differ from the machine's real ones can
themselves score as suspicious — and ships per-site Shields toggles as the escape hatch. **Tor Browser**
chooses uniformity for the same threat. ⭐ Where they disagree **is** the design question; report it
rather than picking a side. Log rows in `development-docs/PRIOR_ART.md` — `RESEARCH_steps_1_2.md`
already has a source log ready to transcribe.

## State at handover

- Head **`05f40e9`** on `0.4.0`. `preflight -Full` **PASS**, `-NegativeControl` last run PASS at
  `b47faf3`. Rebase before starting and **rebuild after** — macOS pushed 6 commits during the last
  session, three touching shared C++.
- **Phase 12 is 🟨 OPEN, deliberately.** Root cause found (the cosmetic scriptlet push lands in the
  **wrong render process**, not a redirect problem) and a mitigation shipped (`d79a869`): cross-process
  arrivals now inject at 25–41 ms instead of ~1200 ms. ⛔ Residual is **not** zero — an inline
  `<script>` still wins that window. The real fix needs a CEF patch and is parked in
  `development-docs/DevOps-CICD/NEXT_CHROMIUM_BUILD.md` §PART 2. 👤 **No Chromium build is being run.**
- ⭐ **Phase 12 is directly relevant here.** The early scriptlet injection exists so anti-bot scripts see
  the *mutated* surface, and it was measured **never to run on a cross-site arrival** — which is how a
  user reaches any site from a link. The mitigation shrank that window but did not close it. ⚠️ Raises
  Phase 13's prior; does **not** replace step 0.
- Dev rig: `.\dev-wallet.ps1`, `.\dev-adblock.ps1`, `cd frontend && npm run dev`, then
  `cd cef-native && .\win_build_run.sh`. ⚠️ The dev browser opens the **profile picker** with 4 profiles;
  pass `--profile=Default` to skip it (agent sessions cannot click it).
- ⛔ **Never kill a Hodos process by image name** — `.\scripts\stop-dev.ps1`. 44 of the owner's installed
  processes were running alongside the dev ones all last session.

## Standing rules that bit last session

- ⛔ **Vary the input every trial.** Reusing one test URL let a one-shot cache serve trial *n*'s failure
  as trial *n+1*'s success — **2 of 3 GREEN for a fix that did not work**. Distinct input per trial, and
  ask *"could this GREEN have been produced by the previous trial?"*
- ⛔ **Name the file your instrument reads.** `[BROWSER]` lines are in `debug_output-<pid>.log`;
  `[RENDER]` lines are in **`cef_debug.log`** (truncated per launch) with the renderer **PID** prefix.
  `hodos::LogSafeUrl` is **origin-only** — path and query are stripped, so it cannot distinguish two
  URLs on the same host.
- ⛔ A green negative control must also show it **reached the subject** — last session's control proved
  itself by logging 0 injections *while 2 page contexts were still created*.

### Carried forward — do not lose

| | |
|---|---|
| `TICKET_createAction_accepts_an_empty_lockingScript_and_spends_into_it.md` | money path; **asked, not changed** (invariant 13) — owner's call still owed |
| `TICKET_transaction_row_can_sit_at_created_while_its_coin_is_on_chain.md` | LOW; its non-coverage is protective |
| `development-docs/DevOps-CICD/PLANNED_devops_review.md` | ⬜ not scheduled — folder restructure + lessons-learned practice. ⛔ Not this phase |
| Phase 12 `P12-B*` rows | belong to the CEF patch, not here |
