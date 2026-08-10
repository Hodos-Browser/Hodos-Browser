# VMP SIGNING — status, steps and progress tracker

**Created:** 2026-08-10 · **Owner:** Matthew (Marston Enterprises) · **Status:** 🟡 APPLICATION NOT YET SUBMITTED
**Why this doc exists:** the owner's decision on 2026-08-10 was *"if it takes months, we have to defer it —
but we should start the process now."* Deferring the **feature** and deferring the **clock** are two
different things. This doc starts the clock and tracks it.

Referenced by: `0.4.0/IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` (D2, DRM-2),
`0.4.0/chromium-rebuild/Q4_widevine_amazon_drm.md` (Spike-2).

---

## 1. What VMP is, in one paragraph

**VMP = Verified Media Path.** It is a set of signature files (`.sig`) that sit beside the browser
binaries and attest that the application framework around the Widevine CDM has not been tampered
with. Chromium enables CDM host verification at runtime **only when valid `.sig` files are present**;
without them it refuses to hand out the attested capabilities, no matter what flags you set. It is a
**signing/attestation** problem, not a build-flag problem — there is nothing to turn on.

## 2. What we measured, so the decision rests on evidence

Measured 2026-08-10 on the CEF 150 build `c63654654` with
`0.4.0/chromium-rebuild/drm_check.py` (re-runnable — do not re-derive this by hand):

| Fact | Evidence |
|---|---|
| The Widevine CDM downloads and loads | `4.10.3050.0`, 21.6 MB, per profile, via the component updater |
| We ship **no** VMP `.sig` files | none beside `HodosBrowser.exe` / `libcef.dll`; no signing step in `release.yml` |
| Basic Widevine **works** | Bitmovin's Widevine demo fetched a real licence and decrypted: **+2,893,374 B decoded**, `mediaKeys` attached |
| `SW_SECURE_CRYPTO` + `SW_SECURE_DECODE` | **granted** (`getConfiguration()` negotiates `SW_SECURE_DECODE` back) |
| **`distinctiveIdentifier: required`** | **REFUSED** ← this is the wall |
| every `HW_SECURE_*` tier | refused |

⚠️ **This corrects the 2026-08-05 record**, which said we were capped below `SW_SECURE_DECODE`. That
does not reproduce and looks like a probe artifact (audio has no `SW_SECURE_DECODE` tier, so a probe
that sets the same robustness on video *and* audio gets an unrelated `NotSupportedError`). **Cite the
distinctive identifier, not a robustness cap.** Full write-up: `Q4_widevine_amazon_drm.md` §7 re-run.

**So the honest statement of what VMP buys us:** not "DRM works at all" — DRM already works. It buys
the **distinctive identifier and the hardware robustness tiers**, which is precisely what Netflix,
Amazon Prime and other subscription services require. Nothing else in the product is affected.

## 3. Two routes

| | **(A) Google direct** | **(B) Certified Third Party Lab (3PL)** |
|---|---|---|
| What | Execute a Master License Agreement with Google, then obtain VMP signing | A Google-certified partner audits and signs on Google's behalf |
| Who | Google Widevine team | castLabs (the one we have looked at); other 3PLs listed at widevine.com/solutions/widevine-providers |
| Timeline | historically **months** (this is the reason for deferring) | castLabs advertise "instant VMP signing", explicitly positioned against Google's "lengthy delays" |
| Cost | no fee for the licence itself | paid; described only as "low-cost", no public pricing |
| Signs our CEF? | yes | **yes — ✅ CORRECTION** |

> ✅ **Correction to a claim carried in our own docs and memory.** We had recorded that "castLabs free
> EVS is Electron-only, cannot sign our CEF." That is true of their **free pre-approved Electron
> build** only. Their paid **Widevine certification / VMP signing** service states support for
> third-party frameworks **including Chromium**, not just their own Electron. So route (B) is a real
> option for a CEF embedder, not a dead end. Verify in writing before spending anything.

**Owner decision 2026-08-10: start (A) now** — the clock is the expensive part, and starting it costs
nothing and commits us to nothing. (B) stays open as the fast path if premium streaming becomes a
product goal before (A) lands.

## 4. Steps — the actual checklist

### Phase 0 — before applying (do first, costs nothing)
- [x] **Confirm the CDM loads and classify the exact failure.** Done 2026-08-10 — §2 above.
- [x] **Confirm we ship no `.sig` files and no signing step.** Done — §2.
- [ ] **GATE: confirm the payoff exists (Q4 Spike-2 step 0).** On **Brave** (already VMP-signed), play
      the exact Amazon/Netflix title we care about and confirm it plays at acceptable quality. If even
      a VMP-signed browser can't play it acceptably (e.g. it demands hardware L1, which VMP alone does
      not give), **VMP buys nothing and this whole doc stops here.** ⚠️ Do this before any paid route.
- [ ] Record the exact Amazon/Netflix failure on Hodos (title, tier, licence-server HTTP status).

### Phase 1 — start the Google clock (route A)
- [ ] Submit the Widevine licence/partner request to Google. Entry points:
      `https://www.widevine.com/contact` and the Widevine Help contact form
      (`support.google.com/widevine/contact/wv_cwipcf`). State plainly: **CEF-based desktop browser,
      Windows (`HodosBrowser.exe` + `libcef.dll`) and macOS (framework + 5 helper bundles), seeking
      VMP signing for a browser implementation.**
- [ ] Record the submission date + any case/ticket number **in the tracker table below** the same day.
- [ ] Execute the Master License Agreement if/when Google offers it.
- [ ] Ask Google explicitly, in writing, the two questions that decide the pipeline work:
      (i) does VMP alone unlock `distinctiveIdentifier` at `SW_SECURE_DECODE`, or is a hardware tier
      also required for the services we care about? (ii) what is the macOS signing artifact —
      is it `.sig` files as on Windows, or does it ride the framework code-signature?

### Phase 2 — pipeline work (only after a cert exists; est. small, but it touches the release)
- [ ] Generate `.sig` for `HodosBrowser.exe` and `libcef.dll` (Windows) — slots into `release.yml`
      **after** Authenticode signing, **before** packaging.
- [ ] macOS path **TBD, not 1:1 with Windows** — scope separately once Google answers (ii).
- [ ] ⚠️ **Auto-update impact:** `.sig` files are a **new file class in the update manifest**. This
      ties directly to the VER-5 drift audit and the N−1 → N apply test — a new file class that the
      updater does not carry is exactly the shape of a forced-reinstall bug, which is the one thing
      `feedback_update_stability_principle` says must never happen. Do not treat this as cosmetic.
- [ ] Re-run `drm_check.py`. **Acceptance: the `distinctiveIdentifier: required` rung flips from
      REFUSE to GRANT.** That single row is the whole test — it is what fails today and what VMP is
      being bought to fix. Then re-run the Amazon/Netflix matrix.

## 5. Progress tracker — update this table, don't rewrite the doc

| Date | Step | Who | Result / reference |
|---|---|---|---|
| 2026-08-05 | Spike-1 steps 0/3/4 on `94c1726` | Windows Claude | CDM loads; no `.sig`; ladder result later found non-reproducible |
| 2026-08-10 | Spike-1 re-run on `c63654654` + Bitmovin | Windows Claude | §2. Wall = `distinctiveIdentifier`; basic Widevine works |
| 2026-08-10 | This doc created; owner elects to start route (A) now, ship beta.1 without VMP | Owner | DRM-2 stays deferred |
| | **Google application submitted** | **Owner — NOT DONE** | *(record date + case number here)* |
| | Brave payoff gate (Phase 0) | | |

## 6. What ships in 0.4.0-beta.1 regardless

CDM auto-download stays on; **no VMP**; subscription video (Netflix, Amazon Prime movies) does not
play, and that is a documented limitation, not a bug. Everything users actually do — YouTube, X,
Twitch, Reddit, LinkedIn, audio — is **codecs**, not DRM, and is verified green (`PLAN_codecs.md`
§6.3). ⛔ Do **not** build the Brave-style "install Widevine" consent prompt (DRM-3): the CDM already
auto-downloads and the prompt is cosmetic.

---

### Sources
- Widevine — Third Party License Service Providers: https://www.widevine.com/solutions/widevine-providers
- Widevine — CWIP / Master License Agreement with Google: https://www.widevine.com/training
- Widevine Help contact: https://support.google.com/widevine/contact/wv_cwipcf
- castLabs — Widevine certification & VMP signing (states third-party Chromium support): https://castlabs.com/security/widevine-certification/
- CEF issue #3404 — Chromium enables CDM host verification only when valid sig files exist: https://github.com/chromiumembedded/cef/issues/3404
- In-repo: `0.4.0/chromium-rebuild/Q4_widevine_amazon_drm.md`, `0.4.0/chromium-rebuild/drm_check.py`
