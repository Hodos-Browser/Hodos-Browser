# DESIGN REVIEW — the farbling exception list: UX vs privacy, and what to build

**Created:** 2026-08-10 · **Status:** RESEARCH + RECOMMENDATION. **No code changes made. Owner-gated.**
**Question asked (owner, 2026-08-10):** *"Can we have both — smooth browsing without constant logins or
rejections, and real anti-fingerprinting — or do we need the exempt list? Enumerating site by site does
not seem good; there are too many and they change at any time. Is Brave's remote list publicly
accessible, should we use it, and should we build a Webcompat Exceptions Manager? Per-vector sounds
good but what are the risks?"*

Companion to `Q3_farbling_oauth.md` §2.5 (which carries the summary) and `Q2_farbling_adblock.md`.

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
