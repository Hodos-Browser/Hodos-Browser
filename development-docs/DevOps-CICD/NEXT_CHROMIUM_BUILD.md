# The next Chromium build — everything that has to go into it

**This is the human-readable front page for Chromium builds.** Opened 2026-09-21.
If you only read one Chromium-build doc, read this one. The step-by-step is in
`CEF_BUILD_RUNBOOK.md`; this page is **what goes in and why**, in plain language.

> **Who this is for:** the person deciding *whether* to do a Chromium build and *what must be in it*.
> Not the person typing the commands — that's the runbook.

---

## Plain English: what a "Chromium build" even is

Hodos is built on CEF, which is a wrapper around Google's Chromium browser engine. Most projects just
download a prebuilt CEF and move on. **We don't** — we compile Chromium and CEF ourselves, from source,
because two things we need are not in anyone's prebuilt binaries:

1. **Video and audio codecs** (H.264, AAC). Without these, YouTube and most video sites break. Prebuilt
   CEF ships without them for licensing reasons.
2. **Our privacy patches** (farbling). These change how the browser answers fingerprinting questions.
   They are *inside* the engine, not JavaScript we inject — which is the whole point, because injected
   JavaScript is detectable and native code is not.

So "a Chromium build" means: check out ~30 GB of Google's source, apply our patches on top, and compile
it. **It takes hours, it happens on the build host, and it cannot be tested until it finishes.** That is
why we batch things up instead of doing one per fix.

**Two tiers, don't confuse them:**

| | What it is | How long | How often |
|---|---|---|---|
| **Tier 1 — the Chromium build** | Compiling the engine itself. *This page.* | **Hours** | Rarely — a few times a year |
| **Tier 2 — the app build** | Compiling Hodos against the already-built engine | ~35 min | Every release |

When someone says "just rebuild", they almost always mean Tier 2. Tier 1 is the expensive one.

---

## When do we do a Tier-1 build?

Any one of these is a trigger:

- **(a)** Chromium/CEF version bump — security updates, new web features
- **(b)** A new or changed privacy (farbling) patch
- **(c)** A codec or build-flag change
- **(d)** Widevine / premium DRM changes
- **(e)** ⭐ **Anything on the PENDING list below has piled up enough to be worth the hours**

⛔ **Do not start a Tier-1 build for a single item unless it is urgent.** Check the PENDING list first
and take everything that's ready.

---

## PART 1 — The standing list: in EVERY build, forever

⛔ **These are not optional and they are not "done once".** Every Tier-1 build must have all of them, and
if one is silently missing the build looks fine and the product is broken in a way that is hard to trace.

| # | What | Why it's there | How it gets in | How you know it worked |
|---|---|---|---|---|
| 1 | **Proprietary codecs** (H.264/AAC) | Video sites work at all | `GN_DEFINES` must include `proprietary_codecs=true ffmpeg_branding=Chrome`. ⛔ Set via `GN_DEFINES`, **not** the `--proprietary-codecs` automate-git flag | Play a YouTube video in the built binary |
| 2 | **Farbling patches** (C1, C3–C6) | Fingerprinting defence, native instead of injected JS | Live in the `Hodos-Browser/cef` fork, branch `hodos/7871`, as `patch/patches/hodos_*.patch`, registered in `patch/patch.cfg`, gated on the `HODOS_FARBLING` env var. Applied **automatically** during the build | ⭐ **Log the patch count from the build log** ("`N patches total`") — it is the cheapest detector of a stale source copy silently dropping every Hodos patch |

⚠️ **The farbling patches also have a release gate**, separate from the build: `FARBLING_RELEASE_GATE.md`.
A build that contains the patches can still ship broken if the seed doesn't rotate — the gate catches that.

⛔ **Extensions are NOT on this list and never will be via self-build.** Extensions are a chrome-layer
feature; compiling Chromium ourselves does not unlock them. Don't add patches trying.

---

## PART 2 — PENDING: waiting for the next build

⭐ **This is the queue.** When a fix needs an engine patch, it goes here **the day we find it**, not when
we get around to building. Once a build ships it, move the row into Part 1 if it's permanent, or delete it
if it was a one-off.

| Added | What | Why it needs an ENGINE patch (not an app fix) | Full spec | Priority |
|---|---|---|---|---|
| 2026-09-21 | **Cosmetic-filter / ad-blocker payload delivery** — file the scriptlets browser-side and have the renderer **pull** them when the page's JavaScript context is created, instead of pushing them | The ad-blocker's scriptlets are pushed to the render process *before* the browser knows which process will actually show the page. On any cross-site click (X → YouTube, or any link to another domain) they land in the **wrong process** and are thrown away, so ads aren't blocked early. Fixing it properly means the renderer has to **ask** for them at exactly the right moment, and that moment is inside libcef — we cannot reach it from app code. ⭐ The same fix already exists in our engine for the farbling keys (`hodos::FarblingRegistry`); this extends that pattern to the ad-blocker | `../0.4.0-beta.3/phase-12-adblock-redirect-arrivals/PHASE_CONTRACT_registry_pull.md` | **Medium** — a partial fix shipped in `d79a869`, so ads are blocked ~30 ms in instead of ~2.6 s. Not urgent, but the gap is real and only an engine patch closes it |
| 2026-09-21 | **Say who we are in `Sec-CH-UA`** — the browser's "name badge" becomes `"Hodos";v="150", "<GREASE>";v="8", "Chromium";v="150"`, matching Brave's shape. The User-Agent **string** is unchanged | 📏 **Measured 2026-09-21, not assumed.** Launching with `--user-agent-product=Chrome/150.0.0.0` changes the UA string and leaves `sec-ch-ua` **completely unchanged** — CEF's UA controls (`CefSettings.user_agent`, `user_agent_product`, and the matching switches) do not reach the brand list, which Chromium builds from its branding flag. ⛔ **The app-level shortcut is a trap:** we already rewrite request headers in C++ for DNT/GPC, so rewriting the `sec-ch-ua` *header* looks cheap — but `navigator.userAgentData.brands` in JS would still say the old thing, so header and JS would then disagree. That is a **new** inconsistency in the very family the signal belongs to, and strictly worse than doing nothing | `../0.4.0-beta.3/phase-13-bot-detection/STEP0_AND_SIGNAL_SHEET.md` §Addendum | **Low — do it when you are building anyway.** 👤 Owner decided 2026-09-21 to use **our own brand, not Chrome's**: claiming `Google Chrome` would blend better but is a fresh lie in the one field designed to be honest, and it buys nothing because the wallet bridge identifies us regardless. ⚠️ **Nothing has been observed to fail because of this** — it was found by lab comparison against Chrome, not by anything breaking. Do not let it justify a build on its own |

### ⚠️ Decision still owed on the pending item above

The contract offers two shapes and the owner picks **before** work starts:
- **b1 (recommended)** — the engine exposes a small "give me the scriptlets" call that our own code uses.
  Keeps the actual ad-blocking logic in code we can test without a Chromium build.
- **b2** — the engine does the injecting itself, exactly like the farbling patch does. No new engine API,
  but the logic moves somewhere our normal tests can't reach.

---

## PART 3 — How to actually run the build

→ **`CEF_BUILD_RUNBOOK.md`** — the step-by-step: branch choice, environment setup, the build itself,
staging the binaries, dependency reconciliation, and the acceptance gate.

Supporting docs:
- `CEF_VERSION_UPDATE_TRACKER.md` — what changed in each past CEF bump, and what bit us
- `BASELINE_CEF150.md` — the reference the next build gets diffed against
- `DEPENDENCY_VERIFICATION.md` — the per-bump dependency check
- `FARBLING_RELEASE_GATE.md` — the gate that proves farbling actually works in the shipped build

---

## How to add something to this page

When you find a problem that **can only be fixed inside the engine**, add a row to Part 2
**immediately** — while you still remember why. A row needs:

1. **What** it is, in a sentence a non-specialist can follow.
2. **Why it needs an engine patch** rather than an app fix. ⛔ Be honest here. Most things don't. If it
   can be fixed in `cef-native/` or `rust-wallet/`, it does not belong on this page.
3. **A link to the full spec** — a phase contract or ticket carrying the acceptance criteria.
4. **A priority**, and whether something already partially covers it.

⭐ **Why this matters:** an engine patch discovered between builds is easy to lose. By the time the next
build happens the person who found it has moved on, and it silently doesn't get done — then the next
build is hours of work that still doesn't fix the thing. **Writing the row takes two minutes.**

> Per `CLAUDE.md` invariant #12, this page is Process & Procedure: keep it current or it rots.
