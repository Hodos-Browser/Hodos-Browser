# Q4 — Amazon Movie DRM: Widevine CDM Root-Cause + "Fix-It Button" Plan

**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**Status:** DETAILED PLAN (Workflow-2 expansion of `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3d / §6-Q4). Research + design only — **NO code, NO builds.** A later session executes the test spike in §7 and folds the result into `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`.

> **What this answers/plans.** Why the Amazon movie failed while YouTube/X/LinkedIn work; whether the cause is a missing/insufficient Widevine CDM (vs an EME/codec gap); exactly how Brave offers the "install Widevine" prompt and whether a CEF embedder can replicate it; the licensing/cost reality of getting Amazon movies to play (VMP signing, L1 vs L3, Google MLA vs castLabs); and a **followable test-first spike** for the version bump with a clear DEFER-if-not-cheap gate per owner stance.

> **Authoritative inputs (read before executing):** `development-docs/0.4.0/CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` (§3d, §6-Q4, §7 DRM checklist), `development-docs/DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md` (TL;DR #4, Widevine appendix — CEF issues #1631/#3149/#3404, castLabs), `development-docs/DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md` ("Must Investigate: EME/Widevine handler changes"), `scripts/build_hodos_cef.bat` (GN_DEFINES), `CLAUDE.md` Testing Standards (site basket).

---

## 1. TL;DR — the answer + the recommendation

1. **Root cause (leading hypothesis, to be confirmed/falsified by Spike-1): the Amazon failure is most likely a Widevine *robustness/attestation* gap, not a missing CDM and not an EME or codec gap.** YouTube/X/LinkedIn play because they serve **clear (non-DRM) H.264/VP9/AV1** — which our `proprietary_codecs=true ffmpeg_branding=Chrome` build already handles — OR accept a plain software (L3) CDM with no VMP attestation. Amazon Prime Video **movies** (especially purchased/rental catalogue titles) are among the strictest EME services: their license server **refuses or SD-caps** a CDM that cannot present a **VMP (Verified Media Path) signature**. The hypothesis is that our current CEF build ships **no `.sig` files**, so even after the CDM auto-downloads, the license request carries an **unverified L3 header → Amazon denies the license → playback error**. **Two things keep this a hypothesis, not a conclusion, until Spike-1 runs:** (a) the failure was observed on the **current M136 build (~15 months old)**, whose component-updater behaviour may differ from the target bump — so the bump alone could change the outcome; and (b) we have **not yet audited whether our own build suppresses the component updater** (privacy-browser builds commonly pass `--disable-component-update` or blocklist `*.googleapis.com`), which would produce a plain **missing-CDM** failure with a *free* fix, not a VMP problem (see §3 pre-condition + Spike-1 step 0).

2. **Brave's "fix-it button" is a two-part thing, and only the *first* part is free to copy.** Part A = an **on-demand component-updater prompt** ("this site wants to use Widevine — install it?") that downloads the CDM into the user-data dir. This is replicable on CEF (the CDM already auto-downloads; we'd only be adding the *prompt* UX). Part B = Brave ships a **VMP-signed browser** (Brave holds a Widevine license + production VMP certificate). **Part B is what actually makes Amazon movies play, and it is NOT free to replicate.**

3. **The cheap/free path (component-updater CDM) probably will NOT fix Amazon movies.** It may fix *some* DRM (Bitmovin demos, possibly Spotify-web, possibly Amazon SD) but Amazon movies almost certainly need VMP. **We should still TEST it on the bump** (§7 Spike-1) because it's a ~1-hour test and it definitively answers which sites break.

4. **VMP signing is a *necessary-but-possibly-insufficient* condition for Amazon movies, and it costs real money/time — so it gates OUT of beta.1** per owner stance. **Necessary:** without VMP the license is refused outright. **Possibly insufficient:** even VMP-signed *software* L3 may only yield an SD-capped stream, and some titles may demand hardware L1 (which desktop browsers don't have) — see R6. Therefore Spike-2's pricing/commit decision must be **gated on first confirming the target title actually plays at acceptable quality in a VMP browser at all** — verify on **Brave** (which is VMP-signed) that the specific Amazon title plays acceptably before spending on castLabs/MLA. Two routes: **(a) Google Widevine MLA** — free-ish but a **~4+ month** opaque approval wait + you self-run VMP signing; **(b) castLabs commercial VMP browser/app signing** — "instant" and supports Chromium embedders, but **paid + quote-based (not published; expect $ meaningful for an indie)**. **castLabs' *free* EVS is Electron-only** and cannot sign our CEF binaries. **Recommendation: DEFER Amazon-movie DRM out of beta.1; ship the free component-updater path + an honest "some premium video (Amazon movies) unsupported" note; open a VMP mini-spike as a post-beta.1 line item.**

---

## 2. Why YouTube/X/LinkedIn work but Amazon movies don't — the layer map

There are **three independent layers** a video needs, and each site uses a different subset:

| Layer | What it is | Our current state | Who needs it |
|---|---|---|---|
| **Codec** | H.264/AAC/MP3/VP9/AV1 decode | ✅ We have it (`proprietary_codecs=true ffmpeg_branding=Chrome`) — the reason we self-build (`BRAVE_FORK_FEASIBILITY.md` TL;DR #4) | Everyone |
| **EME + CDM present** | `navigator.requestMediaKeySystemAccess('com.widevine.alpha')` resolves; `widevinecdm` binary loaded | ⚠️ CDM **auto-downloads via component updater** to `<user_data>/WidevineCdm/` shortly after startup, but see below | Every DRM site |
| **CDM robustness / VMP attestation** | License server trusts the CDM's security level + VMP signature | ❌ **No `.sig` files → unverified L3 → strict services refuse** | **Amazon movies, Netflix, Disney+, etc.** |

- **YouTube / X / LinkedIn:** X/LinkedIn serve **clear** adaptive streams (no EME) for the vast majority of content. **YouTube is more precise and more useful to our argument:** a large share of YouTube playback uses **Widevine L3 EME** (not clear) — yet it plays on our build today. That means **unattested L3 already works** for us, and the *only* thing missing for stricter services is **VMP attestation**, not EME/CDM plumbing. None of these three exercise the VMP layer → they work today, which **confirms our codec + EME + L3 base is fine** and isolates the gap to VMP/robustness.
- **Amazon Prime Video movies:** exercise **all three** layers and enforce the VMP/robustness layer hard. Desktop Amazon plays at up to 1080p **only through a VMP-signed software CDM** ("Chrome CDM up to 1080p; L1 up to 4K" — [movilforum](https://en.movilforum.com/Widevine-CDM:-What-it-is-and-how-it-affects-your-streaming./)). Without VMP, the license is denied → the movie fails. This is exactly the class of failure we observed.

**Diagnostic implication:** we must **confirm which sub-layer** fails on the target build (§7 Spike-1 step 4). If EME resolves and the CDM is loaded but the license request is refused, the cause is **VMP/robustness** (expected). If EME itself fails to resolve, the cause is a **CDM-load/config** problem (cheaper to fix).

---

## 3. Root-cause detail: is our error a missing CDM, or insufficient (unattested) CDM?

**Leading hypothesis: *insufficient*, not *missing* — but only after ruling out that our own build disables the CDM download (pre-condition below).** Primary-source chain:

> **⚠️ PRE-CONDITION (must clear before attributing anything to VMP): does *our* build disable the component updater?** We are a **privacy browser**, and CEF's component updater phones `update.googleapis.com` to fetch the CDM ([CEF forum t=18741](https://magpcss.org/ceforum/viewtopic.php?t=18741)). If we ever added `--disable-component-update`, otherwise suppressed component-updater traffic, or blocklisted `*.googleapis.com` for privacy, then **the CDM never downloads** and Amazon fails with a plain **missing-CDM** — a *free* fix (a flag/allowlist), not a multi-month MLA. **Audit the actual launch flags + GN config on the build where the failure was observed** (`scripts/build_hodos_cef.bat` GN_DEFINES, launcher argv, any privacy blocklist) for `--disable-component-update`, component-updater suppression, and `*.googleapis.com` filtering — **before** any VMP conclusion. This is Spike-1 **step 0**.

- On modern CEF, the **component updater auto-downloads** the Widevine CDM into `<CefSettings.user_data_path>/WidevineCdm/` "shortly after application startup" — component-updater support was **restored for the Alloy runtime by CEF issue [#3149](https://github.com/chromiumembedded/cef/issues/3149)** ("alloy: Add component updater support for Widevine"), which is the authoritative source for the download location and the flags below (confirm the exact enabling version from #3149 rather than assuming a specific milestone). Flags: `--disable-component-update` disables it; `--component-updater=fast-update` forces the immediate download and is the **primary** way to trigger it in the spike (per Google, Widevine becomes available "within a few seconds" of a successful install — not minutes) ([CEF #3149](https://github.com/chromiumembedded/cef/issues/3149), [CEF forum t=14459 / ZennoLab notes](https://www.magpcss.org/ceforum/viewtopic.php?t=14459)). So the **binary is present** on a normal run *unless our build suppressed the updater* (see pre-condition above).
- But CEF issue **#3820** ("widevine on recent versions seems to not work out of the box", CEF 127–130): the CDM binary downloads (`libwidevinecdm` v4.10.2830.0) yet **DRM demo playback still fails**, while stock Chromium on the same box works. The reporter notes Chromium ships a **symlink** to the CDM that CEF lacks. Issue **closed as not planned** — i.e. CEF does not guarantee turnkey DRM. ([#3820](https://github.com/chromiumembedded/cef/issues/3820))
- CEF issue **#3404** (VMP / persistent licenses): the maintainer states **CDM host verification depends on `.sig` files that "require a signing certificate from Google,"** and that **"Chromium will selectively enable CDM host verification at runtime if valid sig files exist"** — it doesn't crash without them, it just **disables the attested path**. ([#3404](https://github.com/chromiumembedded/cef/issues/3404))
- The `.sig` requirement on Windows is concrete: **`<AppName>.exe.sig` and `libcef.dll.sig` must exist beside the binaries**; **without VMP signing, "Widevine will work, but since it is not VMP signed, it will only send an L3 header,"** and **"many premium content streaming services will deny DRM license requests if a VMP isn't present."** ([Thorium FAQ](https://github.com/Alex313031/thorium/blob/main/docs/FAQ.md); [castLabs VMP wiki](https://github.com/castlabs/electron-releases/wiki/VMP))

**Conclusion (provisional — confirm in Spike-1):** the *most likely* explanation is the **unattested-L3** case — the CDM is (or will be) present, and Amazon's license server rejects it for lack of VMP, a **robustness/attestation** problem solvable only by VMP-signing our binaries (§5), not by any GN flag or component tweak. This is **not yet established**: it presumes the pre-condition above clears (our build does *not* suppress the CDM download) and that the M136-era observation reproduces on the target bump. Confirm/falsify via Spike-1 before treating VMP as the cause.

> **Confirm, don't assume, that we ship no `.sig` files.** The "no `.sig`" premise is stated throughout this doc but has not been verified against the build. Add a one-line check: grep `scripts/build_hodos_cef.bat` and the release scripts (`release.yml`, packaging steps) for any VMP / `.sig` signing step. If none exists, the premise is confirmed as evidence rather than assumption.

> ⚠️ **Caveat to verify on the bump (do not assume):** whether the CDM auto-downloads *cleanly on our specific CEF config* (issue #3820 shows it can download-but-not-wire on Linux). Windows is the priority target and historically more turnkey, but §7 Spike-1 must confirm the CDM actually **loads** (not just downloads) before attributing the Amazon failure to VMP.

---

## 4. How Brave's "install Widevine" prompt works, and can a CEF embedder replicate it?

### What Brave actually does
- **Part A — the on-demand prompt (the visible "button"):** Brave gates the CDM behind a user prompt. When a page requests Widevine, Brave shows a content-settings bubble; on accept, `BraveWidevineBundleManager` **downloads + unzips the CDM component into the user-data dir**, then a bubble prompts a **restart** because the CDM only loads at zygote/process startup. On **Windows and macOS the CDM is installed/updated by the standard Chromium component updater** (Linux needed Brave's own bundle manager pre-M79). ([Brave wiki: Support widevine on Brave linux](https://github.com/brave/brave-browser/wiki/Support-widevine-on-Brave-linux); [brave-core PR #3959](https://github.com/brave/brave-core/pull/3959)). This is the "Ask to install Widevine" toggle.
- **Part B — the signed browser (why it actually plays premium content):** Brave holds a **Widevine license and a production VMP certificate**, so Brave's shipped binaries are **VMP-signed** and the CDM presents an **attested** identity. This — not the prompt — is what makes Amazon/Netflix play.

### What a CEF embedder can replicate
| Brave capability | CEF-embedder replicable? | Notes |
|---|---|---|
| CDM download/update via component updater | ✅ **Yes — already happens automatically** | No prompt needed; auto-downloads to `<user_data>/WidevineCdm/`. We could *add* a prompt for UX/consent parity, but it isn't required for function. |
| "Ask to install Widevine" prompt UX | ✅ Yes, buildable | Our overlay system could render a consent bubble; low value unless we want the consent story. **Not required to fix Amazon.** |
| VMP-signed binaries (attested CDM) | ❌ **No — requires a Widevine license + VMP cert we don't hold** | This is the gating piece. See §5. |

**Bottom line:** we can trivially match Brave's *prompt*, but the prompt is **cosmetic**; the part that fixes Amazon (VMP signing) is a **licensing/credential** barrier a CEF embedder cannot shortcut. Brave's "fix-it button" *feels* like it fixes DRM because behind it sits Brave's Widevine license.

---

## 5. Licensing reality — what Amazon movies actually require, and what it costs

### The requirement
Amazon Prime Video movies on desktop require a **CDM that presents a valid VMP signature** (attested software path, "Chrome CDM" up to 1080p; L1 hardware path up to 4K, which desktop browsers generally don't have anyway). Practically, for us that means: **our shipped `HodosBrowser.exe` + `libcef.dll` must be VMP-signed** via the Windows `<exe>.sig` / `libcef.dll.sig` pair. **macOS VMP path is TBD (not parity):** mac VMP attestation ties into framework code-signing/notarization differently from the Windows `.sig` pair, so do not assume a 1:1 mac equivalent — scope it separately when VMP is un-deferred.

### Two routes to VMP

| Route | What you get | Cost | Time | CEF-compatible? |
|---|---|---|---|---|
| **(A) Google Widevine MLA** (Master License Agreement) direct | Your own dev + production VMP certificates; you self-run the signing (hash exe → Google signs → `.sig` back) | License itself is not a published $ fee, but **opaque, slow** | **~4+ months** approval wait; automated-email black hole ([samuelmaddock](https://blog.samuelmaddock.com/posts/the-end-of-indie-web-browsers/)) | ✅ Yes (you own the cert; sign any binary) |
| **(B) castLabs commercial VMP browser/app signing (3PL certification)** | VMP signing for custom Windows/macOS browsers/apps; supports custom **Chromium/Electron** adaptations. **Mechanism:** commercial **3PL certification** unlocks EVS to sign our own CEF/Chromium build — i.e. *get audited → then use EVS on our CEF*, not "EVS can never touch CEF." | **Paid, quote-based** ("low-cost" per their page, no public price) | **Faster than Google MLA, but NOT instant** — commercial certification "involves an audit of the application and the relevant code" before 3PL EVS access is granted | ✅ Yes ([castLabs Widevine certification](https://castlabs.com/security/widevine-certification/); [electron-releases wiki](https://github.com/castlabs/electron-releases/wiki/)) |
| **(B-free) castLabs EVS free tier** (Electron Video Signing) | **Free** production VMP signing — **free tier only** | Free | Fast | ❌ **NO for the free tier — Electron-only.** The *free* EVS tier signs "application packages **derived from official releases of Electron for Content Security**," not arbitrary CEF binaries. (EVS *itself* can sign our CEF build, but only once route (B)'s commercial 3PL certification unlocks that access.) ([EVS wiki](https://github.com/castlabs/electron-releases/wiki/EVS); [electron-releases README](https://github.com/castlabs/electron-releases/blob/master/README.md)) |

**Key trap to record:** the *free* castLabs path (EVS **free tier**) that indie devs cite is **Electron-only** and does **not** apply to our CEF stack. Our only free route is Google's MLA (slow/opaque); our faster route is castLabs' **paid** commercial 3PL certification — faster than the MLA but still **audit-gated (not instant)**, after which EVS can sign our own CEF binaries.

### Cost estimate (for the roadmap)
- **Google MLA path:** ~$0 direct fee, but **months of latency + engineering time** to integrate VMP signing into our release pipeline (hash → sign → embed `.sig` beside every binary, per-build, cross-platform). Realistic engineering: **1–2 weeks of pipeline work** once the cert exists, plus the multi-month wait to *get* the cert.
- **castLabs commercial path:** **paid (quote required; budget a low-four-figure setup + possible per-period/per-title terms — unconfirmed, get a quote).** Faster to production.
- **PlayReady (Amazon's *other* DRM):** **$10,000 advance + $0.35/unit royalty** — a non-starter for us ([samuelmaddock](https://blog.samuelmaddock.com/posts/the-end-of-indie-web-browsers/)). **Not on our code path:** on a Chromium-based browser Amazon uses **Widevine**, not PlayReady (PlayReady is the Edge / Windows-Store path). Mentioned only to close a door we would never actually open.

### Which sites break **without** VMP (expected)
- **Break/SD-cap:** Amazon Prime Video movies, Netflix, Disney+, Max, Hulu, most purchased/premium catalogues.
- **Work anyway (no/loose EME):** YouTube (incl. most content), X, LinkedIn, Reddit, Twitch, Vimeo, generic HTML5 `<video>`, and DRM *demo* pages that accept L3 (Bitmovin may or may not, depending on their config).

---

## 6. Recommendation (owner stance = "OUT of beta.1 unless cheap")

**DEFER Amazon-movie / premium DRM out of v0.4.0-beta.1.** Concretely:

1. **On the bump, run the free component-updater spike (§7 Spike-1)** — ~1 hr, zero cost. It (a) confirms the CDM loads on the target CEF, (b) fixes whatever non-VMP DRM it can, and (c) definitively enumerates which sites break.
2. **If Spike-1 unexpectedly makes Amazon movies play** (low probability), great — document and keep.
3. **If Amazon needs VMP (expected):** DEFER. Ship beta.1 with the CDM auto-download enabled and a one-line honest limitation in release notes ("Some premium streaming — e.g. Amazon Prime Video movies, Netflix — is not yet supported; hardware/verified DRM is on the roadmap"). Open a **post-beta.1 VMP mini-spike** (§8) to price castLabs vs start the Google MLA clock.
4. **Do NOT build the Brave-style prompt for beta.1** — the CDM already auto-downloads; a prompt adds consent UX but fixes nothing. Log it as optional polish.

This matches the outline's §3d default ("OUT of beta.1 unless the free component-updater path fixes Amazon cheaply") and the §7 DRM checklist item.

---

## 7. The executable spike (later session runs this on the target bump)

**Precondition:** P5 of the outline (codecs/DRM verify) — a working target-CEF build exists on the build host.

### Spike-1 — Free component-updater CDM test (~1 hr, $0) — DO THIS
**Goal:** confirm the CDM loads on the target build and enumerate exactly which DRM sites work vs break, and *classify* each failure (EME-resolve vs license-refused).

**Steps (Windows first; repeat on macOS):**
0. **Step 0 — audit our own build for CDM suppression (do this FIRST; it can make the whole VMP thesis moot).** On the build where the Amazon failure was observed, check for anything that stops the CDM from downloading: grep `scripts/build_hodos_cef.bat` GN_DEFINES and the launcher argv for `--disable-component-update` and any component-updater suppression; check our privacy blocklist / adblock filters for `*.googleapis.com` (specifically `update.googleapis.com`). If the updater is suppressed, the "Amazon failure" may be a **free-to-fix missing-CDM**, not VMP — resolve this before proceeding.
1. **Build/confirm** the target CEF has `enable_widevine` set. It is auto-set by CEF's build system; confirm in the generated GN args — no new flag needed in `scripts/build_hodos_cef.bat`. (Record the value in `CEF_VERSION_UPDATE_TRACKER.md`.)
2. **Launch** the target build and force the CDM download with **`--component-updater=fast-update`** (primary — CDM should appear within seconds). A normal launch without the flag downloads it "shortly after startup" as a fallback data point.
3. **Verify the CDM downloaded:** check `<user_data>/WidevineCdm/` exists with `_platform_specific/win_x64/widevinecdm.dll` (and the manifest). On mac: the framework-relative WidevineCdm dir. If **absent**, the failure is *download/config* (see Risk R1) — investigate before blaming VMP.
4. **Classify EME on a probe page** (e.g. `https://bitmovin.com/demos/drm`, or a minimal local page calling `navigator.requestMediaKeySystemAccess('com.widevine.alpha', [...])`):
   - **EME rejects / CDM not loaded** → CDM-load problem (cheaper; check symlink/dir layout vs issue #3820).
   - **EME resolves + demo plays** → non-VMP DRM works; note it.
   - **EME resolves but license request is refused (HTTP 4xx from license server)** → **VMP/robustness gap** (expected for Amazon).
5. **Run the site matrix** and record result + failure class for each:
   | Site | Content | Expected | Record |
   |---|---|---|---|
   | Amazon Prime Video | **PRIMARY: a purchased/rental or premium movie — the same tier that originally failed** (free-with-ads titles carry *lower* DRM robustness and can give a false "works," so they are only a secondary data point). Record the exact title + tier. | refused/SD-cap (VMP) | actual + exact title/tier + license HTTP code |
   | Amazon Prime Video | secondary: a free-with-ads catalogue title | may work (lower robustness) | actual (additional data point only) |
   | Netflix | any title | refused (VMP) | actual |
   | Bitmovin DRM demo | Widevine demo | maybe works (L3) | actual |
   | YouTube | normal video | works (regression baseline) | actual |
   | Spotify web player | any track | test (EME, may accept L3) | actual |
6. **Capture the Amazon error** exactly (screenshot + devtools console + any license-server HTTP status) and compare to Brave's behavior on the same title (Brave should play → confirms VMP is the differentiator).

**Acceptance:** we can state, with evidence, (i) the CDM loads on the target build, (ii) the precise Amazon failure class, (iii) the full works/breaks site list. This satisfies the outline §7 DRM checklist item regardless of outcome.

### Spike-2 — VMP feasibility pricing (post-beta.1, no build) — OPTIONAL, DEFERRED
0. **GATE (do first): confirm the payoff exists.** On **Brave** (already VMP-signed) verify the exact purchased/rental/premium Amazon title from Spike-1 actually plays at an **acceptable quality** (e.g. HD, not a stub/SD-only). If even a VMP-signed browser can't play it acceptably (e.g. it demands hardware L1), then VMP is *insufficient* and spending on castLabs/MLA buys nothing — stop here. Only proceed to pricing if Brave plays it well.
1. **Email castLabs** for a commercial VMP browser-signing quote for a CEF-based Windows+macOS browser (state: CEF, `libcef.dll` + `HodosBrowser.exe`, mac framework+helpers; ~N users). Ask: one-time vs recurring, per-title terms, turnaround, whether they sign **our** CEF binaries (not just their Electron).
2. **In parallel, start the Google Widevine MLA clock** (submit the license request) so the ~4-month wait runs in the background even if we later choose castLabs. Zero commitment to sign.
3. **Scope the pipeline work** (independent of which cert): where in `release.yml` the `.sig` generation slots in (after code-sign, before packaging), which binaries need `.sig` (`HodosBrowser.exe.sig`, `libcef.dll.sig`; **mac VMP path TBD — not a direct `.sig` parity**, ties into framework code-signing/notarization, scope separately), and how the auto-update apply must carry any new signing artifacts (ties to the §7 auto-update drift audit — a new file class in the manifest).

---

## 8. Roadmap feed

- **beta.1:** Spike-1 only. CDM auto-download enabled (default), documented limitation, no VMP. **DRM checklist item = "tested, Amazon deferred, sites-list recorded."**
- **Post-beta.1 (new line item — `VMP_SIGNING_SPIKE.md`):** Spike-2 pricing + Google MLA clock-start + pipeline scoping. Decision gate: castLabs paid vs MLA-free-but-slow vs stay-deferred, driven by product priority of premium streaming.
- **Cross-refs to update after Spike-1:** `CEF_VERSION_UPDATE_TRACKER.md` (record `enable_widevine` value + CDM version + Amazon result under "EME/Widevine handler changes"); `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §7 DRM checkbox; `BRAVE_FORK_FEASIBILITY.md` open-question #4 (mark DRM cost answered).

---

## 9. Risks & open questions (with recommended defaults)

| # | Risk / question | Recommended default |
|---|---|---|
| R1 | **CDM downloads but doesn't load** on our CEF config (issue #3820 pattern — missing dir/symlink wiring). Would masquerade as a VMP problem. | Spike-1 step 3–4 explicitly distinguishes *not-downloaded* vs *not-loaded* vs *license-refused* before concluding "VMP." If not-loaded, file a CEF-layout fix task (cheap) — do **not** jump to VMP. |
| R2 | **VMP `.sig` may be required even for L3 on Windows** (outline I6) — could block Spike-1's "free L3" sites too, not just Amazon. | Treat any L3 site that works as a bonus; if *everything* DRM refuses, that's the "VMP-gates-even-L3" world → strengthens the DEFER + documents that basically all premium video is out until VMP. |
| R3 | **castLabs commercial cost unknown**; could be indie-affordable or not. | Get the quote in Spike-2 before committing; keep Google MLA clock as the free fallback. Budget nothing in beta.1. |
| R4 | **Auto-update must carry new `.sig` files** if we ever VMP-sign → new manifest file class → silent-update drift risk (the outline's known reinstall-forcer). | Fold `.sig` into the Step-5.5 file-manifest drift audit *when* VMP lands; not a beta.1 concern. |
| R5 | **Bundling widevinecdm.dll directly** (vs component updater) to guarantee presence. | Prefer component updater (auto-updates, no redistribution-license headache). Only bundle if Spike-1 shows the updater is unreliable on our config, and check redistribution terms first. |
| R6 | **Amazon may still refuse even with VMP** if it demands hardware L1 for a given title. | Desktop generally gets 1080p via VMP-signed software CDM; accept SD/HD ceiling, no 4K. Document the ceiling, don't chase L1. |
| Q1 | Do we want the Brave-style consent *prompt* at all (privacy-browser positioning: "we don't silently pull Google's CDM")? | **Optional, post-beta.1.** Functionally unnecessary; may fit our privacy story. Log as UX polish, decide with owner. |
| Q2 | Is premium streaming (Amazon/Netflix) an actual product goal, or nice-to-have? | Confirm with owner before spending on VMP. Current read: nice-to-have → DEFER is correct. |
| Q3 | **Filename cross-ref mismatch:** this file is `Q4_widevine_amazon_drm.md`, but the outline §6 stub and §8 roadmap point to `Q4-amazon-drm.md`. | This file (`Q4_widevine_amazon_drm.md`) is canonical; update the outline's §6/§8 cross-refs to match on the next outline edit (out of scope for this file). |

---

## Sources
- CEF Widevine component-updater support (canonical): [CEF issue #3149 – "alloy: Add component updater support for Widevine"](https://github.com/chromiumembedded/cef/issues/3149) — authoritative for download-to-`user_data_path/WidevineCdm/`, `--disable-component-update`, `--component-updater=fast-update`, and the enabling version
- Component updater phones `update.googleapis.com` (privacy relevance): [CEF forum t=18741](https://magpcss.org/ceforum/viewtopic.php?t=18741)
- CEF Widevine component-updater behavior + flags (secondary): [CEF forum t=14459](https://www.magpcss.org/ceforum/viewtopic.php?t=14459), [ZennoLab Widevine notes](https://zennolab.atlassian.net/wiki/spaces/EN/pages/2111864833/Instructions+for+using+Widevine+component)
- CEF Widevine not turnkey on recent versions: [CEF issue #3820](https://github.com/chromiumembedded/cef/issues/3820)
- VMP / `.sig` / host-verification requirement: [CEF issue #3404](https://github.com/chromiumembedded/cef/issues/3404), [Thorium FAQ](https://github.com/Alex313031/thorium/blob/main/docs/FAQ.md), [castLabs VMP wiki](https://github.com/castlabs/electron-releases/wiki/VMP)
- Brave's prompt + component-updater flow: [Brave wiki – Support widevine on linux](https://github.com/brave/brave-browser/wiki/Support-widevine-on-Brave-linux), [brave-core PR #3959](https://github.com/brave/brave-core/pull/3959)
- castLabs EVS *free tier* is Electron-only vs commercial 3PL certification (paid, audit-gated, then unlocks EVS to sign custom Chromium/CEF): [EVS wiki](https://github.com/castlabs/electron-releases/wiki/EVS), [electron-releases README](https://github.com/castlabs/electron-releases/blob/master/README.md), [electron-releases wiki](https://github.com/castlabs/electron-releases/wiki/), [castLabs Widevine certification](https://castlabs.com/security/widevine-certification/)
- Amazon L1/L3 SD-cap + "Chrome CDM up to 1080p": [movilforum – Widevine CDM](https://en.movilforum.com/Widevine-CDM:-What-it-is-and-how-it-affects-your-streaming./)
- Indie-browser DRM licensing reality (4-month wait, VMP CSRs, PlayReady $10k+$0.35/unit): [Samuel Maddock – The End of Indie Web Browsers](https://blog.samuelmaddock.com/posts/the-end-of-indie-web-browsers/)
- Prior internal finding (self-build = codecs; Widevine premium = VMP): `development-docs/DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md`
</content>
</invoke>
