# DRAFT — beta.2 release-note wording for fingerprint protection

> **Status: DRAFT, owner approval required.** This is user-facing product communication about
> a privacy limitation, which is an owner line, not an engineering one (relay §DD4).
>
> ⚠️ **Contingent on Phase 3.** The wording below assumes P4f goes green. If any P4f
> acceptance row fails, we stay on §D **row 1** and *no fingerprinting claim of any kind may
> be made* — §D.2 below is the wording for that case. **Pick one; do not blend them.**

---

## The rule this document exists to enforce

`FARBLING_DEFINITION_OF_DONE.md` §D: **the claim follows the matrix, never the other way
round.** Every sentence below is traceable to a measured cell. If a cell moves, the sentence
moves with it.

---

## D.1 — wording IF P4f is green (§D ladder row 2)

**Permitted claim:** *"Fingerprint protection on pages and their frames. Background service
workers are not yet covered."*

### Suggested user-facing text

> **Fingerprint protection.** Hodos randomises the values sites use to fingerprint your
> browser — canvas, WebGL, audio, and device characteristics — using a value unique to your
> profile and to each site you visit. The same site sees stable values, so logins keep
> working; different sites see different values, so you cannot be tracked between them.
>
> Protection covers the page, everything embedded in it, and the background scripts pages
> start. It does **not** yet cover shared or service workers, which run outside any single
> page.
>
> Some things are deliberately not randomised, because changing them would break sites or
> would itself make you more identifiable: your screen size, your installed fonts, your
> timezone, your language, and your graphics card's name.

### ⛔ The four residuals — all four must be stated

| # | Residual | Evidence | Why it must be stated |
|---|---|---|---|
| 1 | **Widgets on non-exempt sites are now farbled where they were native** — payment and captcha iframes (Stripe, reCAPTCHA, Turnstile, 3-D Secure) | P4e, MEASURED | It is a behaviour change users may notice as a site breaking. Not established as harmful — the incoherent farbled-parent/native-child combination that shipped *before* P4e is the configuration these scorers actually reject. |
| 2 | **Exemption inheritance (D5)** — on all 37 `IsAuthDomain` hostnames, **every** embedded third party reads true native values | `farbling_d5_residual_check.py`, MEASURED (macOS) | The list includes x.com, facebook.com, amazon.com, github.com, paypal.com, chase.com, bankofamerica.com, wellsfargo.com — pages users are most likely to be logged into and which carry many third-party frames. |
| 3 | **Shared and service workers are unfarbled** | CODE-READ; owner-signed deferral, §F, 2026-08-14 | Reachable **same-origin only**, so narrower than the dedicated-worker gap P4f closes — but it is the one remaining unfarbled realm and it must be named, not omitted. |
| 4 | **Fenced frames are unmeasured** | §A.6, UNKNOWN | The container exists in this build. Not embedder-scriptable, so it is a tracker-visibility question rather than a bypass — but ❓ is not ✅ and it may not be quietly omitted. |

### ⚠️ Residual 2 is NOT new, and saying it is would be the costly error

Pre-P4e, third-party frames on those hostnames were native **too** — they were simply
unkeyed, like every other subframe. P4e did not create this; it made it *coherent* and
therefore visible. Describing it as a new limitation would be inaccurate **in the direction
that costs the most trust**: it would read as "the privacy feature got worse", when what
actually happened is that a bypass affecting *every* site was closed and a narrow, deliberate
exemption survived.

### ⚠️ What changed since beta.1, stated plainly

beta.1 shipped with farbling on **the main frame only**. Any page could read its true
values through a same-origin `about:blank` iframe or a `window.open()` popup — one line of
JavaScript. P4e closed both. P4f closes the third container of the same class: the page's
own workers, plus an unfarbled canvas encoder (`OffscreenCanvas.convertToBlob`) and three
unhooked audio readers reachable on the main frame.

⇒ **beta.1's protection was defeatable by the page it protected.** If the notes describe
beta.1's fingerprint protection at all, they must not imply it was complete.

---

## D.2 — wording IF P4f is NOT green (§D ladder row 1)

**Permitted claim: none.** Not "partial protection", not "protection on most surfaces",
not a feature bullet with a caveat. Row 1 exists because a protection the page can switch
off for itself is not a protection, and describing it as one is the defect the ladder was
built to prevent.

The feature may still ship — it raises cost for passive trackers. It may not be *claimed*.
Suggested handling: no fingerprinting entry in the user-facing notes, and a plain line in
the technical changelog:

> Continued work on fingerprint randomisation. Not yet complete; see the project notes.

---

## Open owner decisions that would change this text

1. **WebGL `UNMASKED_RENDERER` / `VENDOR`** (E7) — these are the most valuable of the
   deliberately-unfarbled vectors and are trivially readable. Farbling `readPixels` while
   leaving the GPU model in a plain string is a coherence question. The draft above names
   the graphics card in the "deliberately not randomised" list; if E7 goes the other way,
   remove it from that list.
2. **R12 fenced frames** (E10) — if measured and green, residual 4 disappears. If signed as
   a documented gap, it stays and should name the signature date.
3. **The `IsAuthDomain` allowlist itself** — the lever that would narrow residual 2 is
   *shrinking the list*, not changing the code (relay §DD3: exempting only the main frame
   recreates the incoherence the exemption exists to avoid). Whether `amazon.com` needs a
   fingerprinting exemption is an owner call.
