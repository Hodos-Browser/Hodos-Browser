# DESIGN REVIEW — the farbling exception list: UX vs privacy, and what to build

**Created:** 2026-08-10 · **Status:** RESEARCH + RECOMMENDATION. **No code changes made. Owner-gated.**
**Question asked (owner, 2026-08-10):** *"Can we have both — smooth browsing without constant logins or
rejections, and real anti-fingerprinting — or do we need the exempt list? Enumerating site by site does
not seem good; there are too many and they change at any time. Is Brave's remote list publicly
accessible, should we use it, and should we build a Webcompat Exceptions Manager? Per-vector sounds
good but what are the risks?"*

Companion to `Q3_farbling_oauth.md` §2.5 (which carries the summary) and `Q2_farbling_adblock.md`.

> ## 📖 READ ORDER
> §0 the four answers → §5 the first-draft plan → **§5b the adversarial review, which overturns two
> of its steps** → **§5c the revised plan, which is the one to execute.**
> ⚠️ Do not implement §5. It is kept only so the reasoning is auditable.

---

## 0. The four answers, up front

1. **Yes, Brave's list is public** — `brave/adblock-lists` → `brave-lists/webcompat-exceptions.json`,
   plain JSON on GitHub, every entry justified by a linked bug.
2. **It contains ~20 entries.** That is the headline. Brave farbles more aggressively than we do,
   ships to tens of millions of users, and has a public bug tracker feeding it — and the whole list
   is about twenty sites. **The "too many to enumerate" fear is empirically wrong**, and the reason it
   stays small is exactly the per-vector design.
3. **Our list is 37 entries and each one is 4× blunter than theirs.** `IsAuthDomain` is
   all-or-nothing: one entry disables canvas + WebGL + audio + navigator together. So we are *more*
   exposed on *more* sites than Brave, from a list nobody can update without a release.
4. **You can have both** — but by making the list smaller and sharper, not by deleting it. Nobody who
   has tried has managed without one, including Brave.

---

## 1. What Brave actually ships

`https://raw.githubusercontent.com/brave/adblock-lists/master/brave-lists/webcompat-exceptions.json`

```json
{
  "include": ["*://maps.gsi.go.jp/*"],
  "exceptions": ["canvas"],
  "issue": "https://github.com/brave/brave-browser/issues/35755"
}
```

Vectors that can be individually disabled: `canvas`, `language`, `keyboard`, `audio`, `plugins`,
`screen`, `webgl2`, `referrer`, `hardware-concurrency`.

Three properties worth copying, independent of whether we ever consume their data:

| Property | Why it matters |
|---|---|
| **One vector per entry** | Google Docs needs `plugins` unfarbled. It does **not** need canvas, audio or WebGL unfarbled — and under our scheme it would get all four. Precision is the entire reason their list stays at ~20 entries and ours is at 37. |
| **Every entry cites a bug** | The list is auditable. An entry with no linked evidence is a candidate for removal, which is what stops a webcompat list rotting into a permanent privacy hole. |
| **Shipped as data, updated remotely** | A site that changes its bot detection is a data push, not a release. This is the direct answer to *"they can change at any time."* |

## 2. Can we use Brave's list? Three different questions

**(a) Consume their remote *service*? No.** The exceptions reach Brave users through their component
updater (`webcompat_exceptions_service`), keyed to Brave's component IDs and update servers, and
resolved into Brave's own content-settings types. There is no public endpoint for a third party, and
building against their infrastructure would make our privacy posture depend on their release
schedule. Not viable and not desirable.

**(b) Consume their *data* as an input? Yes, and it is genuinely useful.** The JSON is a public file
in a public repo. We could mirror it, translate the vector names onto ours, and use it to seed and
augment our own list — with attribution, and with each entry still gated by our own testing.
⚠️ **Action before any of this: confirm the repo's licence and comply with it.** Do not vendor the
file on the assumption that "public on GitHub" means "free to redistribute."

**(c) Depend on it as our only source? No.** Their list solves *webcompat* (a site's features break).
Ours mostly solves *anti-bot* (Turnstile/reCAPTCHA scoring a login attempt). Those overlap but are not
the same problem, and their list will never contain the BSV-specific sites in our regression basket.
**Use it as an augment and a sanity check, never as the backbone.**

## 3. What "per-vector" means for us, concretely

We farble four things. Today one allowlist entry switches off all four:

| Vector | What it protects | What a blunt exemption costs |
|---|---|---|
| canvas `getImageData` | the highest-signal fingerprint | the big one — usually the vector actually needed |
| WebGL `readPixels` | GPU-derived rendering signature | given away for free on all 37 sites |
| WebAudio `getChannelData` | audio-stack signature | given away for free on all 37 sites |
| navigator `deviceMemory` / `hardwareConcurrency` | coarse hardware class | given away for free on all 37 sites |

Concretely: `github.com` is exempt so logins and OAuth work. GitHub does not need our *audio* stack
unfarbled to log you in — but it gets it. Multiply by 37.

**Per-vector would mean** an entry naming the host *and* the vectors, e.g.
`whatsonchain.com: [canvas, webgl, audio]` (Turnstile reads all three, so this one stays broad) versus
`docs.google.com: [navigator]`. The saving is not uniform — for anti-bot sites you often do need the
visual trio — but it is large across the long tail, and it makes each entry state a *reason*.

## 4. Risks — the honest list

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| R1 | **A remote list is a new remote input that can weaken privacy.** Today the exemptions are compiled in and can only change with a signed release. A fetched list is a channel that, if compromised or spoofed, could switch farbling off broadly. **This is a strictly worse failure mode than a bad ad-filter rule** and is the main argument *against* the change. | **HIGH** | Serve over HTTPS from our own origin; **sign the list** and verify before use; hard structural cap — an entry may only name `(host, vectors)`, there must be **no expressible global-off**; cap entry count and reject an oversized list; keep a compiled-in fallback so a fetch failure fails **closed** (farble everything) rather than open. |
| R2 | **More states are more identifying.** A site seeing "canvas farbled, audio native" learns something about the browser. | LOW | It identifies *Hodos*, not *the user* — the list is public either way, and it is the same for every user, so it adds no per-user entropy. Accept and document. |
| R3 | **Silent fail-open.** More granular state means more paths where a vector ends up unfarbled by mistake, with no visible symptom. | MED | The seed-rotation gate already asserts **all four vectors independently** and already has a working negative control. Extend it to assert the *expected* per-vector state for a listed host. This is the existing machinery, not new work. |
| R4 | **List rot.** Entries added for a 2026 breakage stay forever and quietly become permanent holes. | MED | Copy Brave: **every entry must cite evidence** (a bug/test result + date). Add a periodic re-test — the exemption harness makes it cheap. |
| R5 | **Effort spent for a smaller win than it sounds.** For the anti-bot sites that dominate our list, you often need the visual trio anyway. | MED | Phase it (§5): do the cheap high-value half first and measure before building the rest. |

## 5. Recommendation — phased, cheapest first

**Phase 1 — measure before building anything (hours, no new architecture).**
Temporarily empty `IsAuthDomain`, rebuild **`cef-native` only** (minutes — this is a shell rebuild, not
the ~5 h CEF build), and try the auth basket: x.com, github.com, accounts.google.com,
whatsonchain.com, a bank, plus a Turnstile page. Q3 **T2** (farbled values must equal the true-native
baseline) is the only proof an exemption is actually live. **This is the decision-quality data we do
not currently have** — right now, nobody knows how many of those 37 entries are still load-bearing
*with native farbling*, because the list predates it and was written against the JS implementation
whose tamper tells are gone. It is entirely possible the answer is "most of them can go."

**Phase 2 — make each surviving entry per-vector.** Change the entry shape to `(host, vectors)` and
narrow every entry to what Phase 1 proved it needs. Still compiled in. This is the biggest privacy win
per hour of work and carries **none** of R1.

**Phase 3 — only if Phase 1 shows the list genuinely churns:** move it onto the adblock filter-list
channel (which already downloads and hot-reloads every 6 h), signed, with a compiled-in fail-closed
fallback. **Do not do this first.** It is the only phase that adds attack surface, and it should be
justified by observed churn rather than by the fear of it.

**Phase 4 (UX, independent):** a per-site control in the Privacy Shield panel — "this site is
exempted from X because Y" — plus a report-breakage path. Users already have the per-site toggle;
this makes the automatic exemptions *visible*, which is a privacy feature in itself.

**What NOT to do**
- ⛔ Don't delete the list. Brave sunset their Strict mode precisely because protection users cannot
  browse with is not protection — and the people who enabled it became *easier* to fingerprint by
  standing out.
- ⛔ Don't build the remote channel before Phase 1. R1 is real and unnecessary today.
- ⛔ Don't randomize *more* to compensate. Our FB-2 decision to drop WebGL vendor/renderer
  randomization was right: a random GPU string is more unique than the truth. Brave's own rule is to
  return values with "the *shape* of expected values."
- ⛔ Don't confuse this with logins. See §6.

## 5b. ⚔️ ADVERSARIAL REVIEW of §5 — six attacks, four sustained

Run 2026-08-10 against the plan immediately above, before any of it was implemented. **Four attacks
landed, and two of them reorder the plan.** The revised plan is §5c; §5 is left standing so the
reasoning is auditable.

---

### A1 — "Phase 2 is the biggest win per hour of work" is **WRONG**. It is the most expensive phase. ✅ SUSTAINED

Per-vector exceptions are not a shell change. The on/off decision is delivered as a **single boolean**
(`simple_handler.cpp:7611/7640`, `fmsg->GetArgumentList()->SetBool(1, …)`), stored as
`bool enabled` in `libcef/browser/hodos_farbling_registry.h:87`, and read by each patched Blink call
site. Turning that bit into a per-vector bitmask touches, verified by grep in the fork at
`c63654654`:

| Layer | Files |
|---|---|
| Shell (cheap) | `simple_handler.cpp` — the two `SetBool` sites |
| **libcef (fork)** | `hodos_farbling_registry.{h,cc}`, `browser_frame.{h,cc}`, `frame_host_impl.cc`, `blink_glue.{h,cc}`, `frame_impl.cc` |
| **Blink patches (fork)** | `hodos_farble_session_cache.patch` (the Supplement must carry the mask) + **all four** of `hodos_farble_canvas2d/webgl/webaudio/navigator.patch`, each to test its own bit |

⇒ **a full CEF rebuild (~5 h Windows), a full Mac rebuild, and a permanently larger patch set to
rebase on every Chromium bump.** Calling that "the biggest privacy win per hour" was flatly wrong.

**Consequence — and this is the useful part:** if Phase 1 shows most of the 37 entries can be
*deleted*, then a **short all-or-nothing list is fine** and per-vector may never be worth building.
Trimming 37 → 6 with the existing single bit is a **larger** privacy gain than making 37 entries
surgical, and it costs a shell rebuild instead of two engine rebuilds. **Per-vector is contingent on
Phase 1's result, not a foregone conclusion.**

---

### A2 — Phase 1 cannot prove what the plan needs it to prove. The evidence is **asymmetric**. ✅ SUSTAINED — most important finding

Bot-detection verdicts are **not a function of the fingerprint alone.** Cloudflare, DataDome and
friends score IP reputation, ASN, account age, cookie history and behaviour, then apply a rolling
risk model. So a Phase-1 pass on **this machine, this IP, and accounts that have been logged in for
months** is close to the easiest possible case, and generalises poorly to the case that actually
matters: a brand-new user, on a residential IP, creating an account for the first time.

    A FAILURE is conclusive  — the entry is load-bearing, keep it.
    A PASS is weak evidence  — it does not license removal.

The plan as written treats a pass as a licence to delete. It is not. Left uncorrected, Phase 1 would
have produced a confident trim that breaks for exactly the users we never tested: new ones.

**Required corrections** (folded into §5c):
- test in a **fresh profile with no cookies**, not the logged-in one — the sign-in flow is the subject,
  not the signed-in session;
- test **sign-in**, and where possible **sign-up**, not "does the page load";
- **N ≥ 3 trials per site**, on different days, because these verdicts are stochastic;
- ⚠️ **record it as evidence-for-keeping, never as proof-of-safe-removal**, and trim only entries with
  a positive rationale beyond "it passed once here."

---

### A3 — A privacy browser has **no feedback channel** for privacy-caused breakage. Phase 4 is a prerequisite, not a finish. ✅ SUSTAINED

We deliberately ship no telemetry. So if trimming the list breaks a site for a real user, **we find out
never.** The failure is silent, it is attributed to "Hodos is broken", and the user leaves. Every
other browser doing this has a signal we do not: Brave's ~20 entries each cite a **public bug report**
— that list exists because users could tell them.

Phases 1–3 are all "change the exemptions and hope." That is not a testable procedure, which is the
standard this project holds everything else to.

**Correction:** the in-product **"this site is broken" report** moves from Phase 4 (polish) to
**Phase 0 (prerequisite)**. Minimum viable version: one click, sends **hostname + which vectors were
active + browser version**, nothing else, with explicit consent and a visible preview of the payload.
Without it, we should not trim at all.

---

### A4 — Hosting the list in our GitHub repo makes a **repo compromise equal to a privacy kill-switch**. ✅ SUSTAINED (does not block, but changes the design)

The owner's proposal — maintain the list in our GitHub repo, fetched by installed browsers — is the
right host, and it is how the adblock lists already work. But the threat model is not the same:

- A bad **ad-filter** rule shows an ad. A bad **exception** rule turns off fingerprinting protection —
  silently, with no user-visible symptom, for every installed browser, within one fetch interval.
- Anyone with push access, a leaked PAT, or a compromised Action can do it. **No code review gate
  applies to a data file**, which is exactly what makes data files convenient.

**Required, if we do this:**
1. **Detached signature over the list, verified in the client, with a key that is NOT a GitHub
   credential** — so repo write access alone is insufficient. (We already sign releases; reuse the
   discipline, not the same key.)
2. **Monotonic version counter**, rejected if it goes backwards — otherwise an attacker replays an
   older, broader list.
3. **Structural cap:** the schema can express `(host, vectors)` **and nothing else**. No wildcard-all,
   no global disable, no regex. If "off everywhere" cannot be spelled, it cannot be pushed.
4. **Fail CLOSED**: fetch failure, signature failure or version regression ⇒ fall back to the
   compiled-in list, i.e. farble everything. Never fail open.
5. **Size/entry cap**, rejecting an implausibly large list.
6. ⚠️ **Privacy note to state honestly:** a fetch from a *first-party* Hodos URL is a check-in that
   reveals installed-base size and per-user IP/timing to us and to the host. The adblock lists already
   fetch from third-party origins, so the *marginal* exposure is small — but "we don't phone home" is
   no longer strictly true, and the privacy policy must say so.

---

### A5 — "Phase 1 is cheap" ⇒ verify it is really shell-only. ✅ CONFIRMED, attack fails

`IsAuthDomain` is a static list in `cef-native/include/core/FingerprintProtection.h`, consumed by
`simple_handler.cpp :: OnBeforeBrowse` to compute the single `enabled` bit. Emptying it is a **shell
rebuild only** (minutes). No CEF rebuild, no patch change. Attack does not land — but note the
experimental build **must never be staged or shipped**; do it on a branch and keep it off
`cef-binaries/`.

---

### A6 — Priority: does any of this belong before 0.4.0-beta.1? ⚠️ PARTIALLY SUSTAINED

P6 (test) and P7 (prod build) are the release path; this is not on it. Phases 2–4 are **post-beta.1**
and should be stated as such so they cannot silently absorb release time.

**But Phase 1 has a release-relevant output:** it tells us whether the shipped exemption list is
larger than it needs to be, which is a **privacy claim we are about to make in a release**. It is a
few hours, it is shell-only, and it can run in parallel with the macOS work that currently gates P6.
Keep Phase 1 now; defer the rest.

---

## 5c. REVISED PLAN — after the adversarial review

| # | Step | Cost | Gate to proceed |
|---|---|---|---|
| **0** | **Breakage-report path** — one click, sends hostname + active vectors + version, explicit consent, visible payload | frontend + a small endpoint | ⛔ **Nothing may be trimmed before this exists** (A3) |
| **1** | **Measure which entries are still load-bearing.** Empty `IsAuthDomain`, rebuild **shell only**, on a branch never staged. **Fresh profile, no cookies. Test sign-IN and sign-UP. N ≥ 3 trials.** Q3 T2 (native-value equality) proves an exemption is live. | hours | Failures are conclusive; passes are **not** a licence to delete (A2) |
| **2** | **Trim conservatively** using the single existing bit. Delete only entries with a positive rationale; keep anything that failed once. Each surviving entry cites its evidence + date, Brave-style. | shell rebuild | If 37 → single digits, **stop here** — the win is banked and per-vector may be unnecessary (A1) |
| **3** | **Per-vector — ONLY if step 2 leaves a long list.** Bitmask through 8 fork files + 5 patches, full CEF rebuild on both platforms, permanent patch-set growth. | ~5 h build ×2 platforms + rebase cost forever | Explicit owner sign-off on the build cost |
| **4** | **Remote list — ONLY if churn is observed.** Our GitHub repo is the right host. Detached signature (non-GitHub key), monotonic version, `(host, vectors)`-only schema, fail-closed, size cap, privacy-policy update. | moderate | Observed churn, not anticipated churn (A4) |
| **5** | **Surface it in Privacy Shield** — "this site is exempted from X because Y", with the report button from step 0. | frontend | — |

**The two changes that matter:** the report path moved to the front and became mandatory, and
per-vector moved to the back and became conditional. Both because the cheap thing (delete entries)
turns out to be worth more than the expensive thing (make entries surgical), and because trimming
without a feedback channel is not a testable procedure.

---

## 6. Logins are cookies, not fingerprints — and we measured it

The worry "users will have to log in every time" is not what this list prevents. Sessions ride
**cookies and JWTs**; farbling never touches them. On 2026-08-10 the cross-session test rotated the
profile seed — the maximum possible fingerprint change — and **YouTube stayed logged in**.

What the exemptions actually protect is narrower and more specific:
1. the **login moment**, where Turnstile / reCAPTCHA / DataDome score the browser before letting you
   authenticate, and
2. **risk engines** that treat a changed device signature as a reason to re-verify.

Both are about *anti-fraud scoring*, not session storage. That is why the persistent per-profile seed
matters more than the exemption list for day-to-day smoothness: it keeps the signature *stable*, which
is what risk engines care about. And that guarantee is already proven green.

---

## 7. Appendix — the other owner question: does farbling do the ad-blocking?

**No. They are separate systems that share no code.** This is worth stating plainly because the two
get conflated:

| | Ad-blocking | Farbling |
|---|---|---|
| What | stops ad/tracker requests, hides ad elements, strips YouTube ad data | perturbs canvas / WebGL / audio / navigator readings |
| Where | Rust `adblock-engine` on port 31302 (31402 dev) + browser-process IO thread + injected CSS/scriptlets | Blink C++ patches inside the renderer, below JavaScript |
| Talks to the other? | no | no |

**Did moving farbling into Blink affect ad-blocking? No — and it is a net improvement.** Measured on
the current build (`c63654654`) today:

- engine healthy: **4 lists, 86,188 + 56,443 rules**; ad URLs blocked (`googletagmanager`, `pubmatic`),
  benign URLs not blocked — i.e. the check discriminates rather than saying "blocked" to everything
- **cnn.com**: cosmetic CSS injected, `#hodos-cosmetic-css` present, 2,153 chars
- **youtube.com**: no cosmetic CSS **and that is correct** — the engine returns `generichide: true` with
  an empty selector list for YouTube and blocks its ads via a **scriptlet**
  (`ytInitialPlayerResponse.adPlacements`) plus the `AdblockResponseFilter` that rewrites the response
  body before the renderer ever sees it. Judging YouTube by the CSS path would be measuring the wrong
  mechanism.
- `getImageData`, `readPixels` and `getChannelData` all report **`[native code]`**

That last line is the net win `Q2_farbling_adblock.md` predicted: the old JavaScript implementation
replaced those methods with JS functions, so they no longer *looked* native — exactly the prototype
tampering that anti-bot and anti-adblock stacks flag. Native farbling restores it. Ad-blocking is
slightly *harder* to detect than before, not easier.

> Method note: an early run of this check reported "no cosmetic CSS" on cnn.com. That was the probe
> reading the page before injection landed, not a regression — polling to `readyState: complete`
> showed the style element present and stable. Recorded because the file the teardown edited is the
> same file this injection lives in (`Q2` TP-1), so a future reader will have the same scare.

---

### Sources
- Brave webcompat exceptions data (public): https://github.com/brave/adblock-lists/blob/master/brave-lists/webcompat-exceptions.json
- Brave remote list webcompat exceptions service: https://github.com/brave/brave-browser/issues/37074
- Per-feature exception examples: #55271 (language/Albertsons), #43555 (screen), #39924 (manager scope)
- Brave — Fingerprinting defenses 2.0: https://brave.com/privacy-updates/4-fingerprinting-defenses-2.0/
- Brave — sunsetting strict fingerprinting mode: https://brave.com/privacy-updates/28-sunsetting-strict-fingerprinting-mode/
- In-repo: `Q2_farbling_adblock.md`, `Q3_farbling_oauth.md`, `PLAN_farbling_blink.md` §7/§11,
  `cef-native/include/core/FingerprintProtection.h :: IsAuthDomain`
