# Brave-Fork Feasibility Spike (A4) — Findings & Recommendation

**Created:** 2026-06-01
**Status:** ✅ Research complete — recommendation below; decisions pending user
**Method:** 4 parallel primary-source research passes (Widevine licensing, Brave build architecture,
farbling detectability/perf, CEF extensions + native UI). Every claim below traces to a cited source
in the per-topic appendices (see "Sources" at the end of each section).

> ⚠️ This is a research deliverable. It changes **no source code** and commits us to nothing. It
> exists to let us pick a direction with eyes open.

---

## TL;DR — the recommendation

1. **Do NOT fork or "build from Brave." Stay on CEF.** "CEF built from Brave" is architecturally
   incoherent — they live on different layers of Chromium. Adopting Brave means *abandoning CEF
   entirely*, porting our whole shell/overlay/interceptor stack, and signing up for a perpetual
   ~3-week Chromium-rebase treadmill that needs a dedicated team. (Confidence: HIGH)

2. **The elegant resolution for farbling (B1):** a self-built CEF *is* a Chromium build, so we can
   apply **Blink-level (C++) farbling patches to our OWN CEF build** — the same technique Brave uses
   — *without being Brave*. CEF's `patch.cfg` build mechanism supports custom patches; open-source
   reference implementations (helium-chromium, fingerprint-chromium) target the exact Blink files.
   This is the only path to *undetectable* farbling, and it **justifies keeping the self-build**.

3. **The unwelcome surprise (B4 extensions):** real Chrome Web Store extensions — including MetaMask
   — are **not feasible on CEF**. CEF's extension API is effectively dead (Alloy support removed
   ~M127; Chrome-runtime-only; ~4 of 70+ `chrome.*` APIs; no MV3 service workers). Vivaldi/Brave/Opera
   support extensions because they build the *full Chromium chrome layer*, which CEF deliberately
   does not. B4 needs to be re-scoped or rethought (see below). (Confidence: HIGH)

4. **Why we self-build (ANSWERED — it's codecs, not Widevine):** confirmed in our own build scripts
   (`build_hodos_cef.bat` / `build_hodos_cef_mac.sh`): `GN_DEFINES="... proprietary_codecs=true
   ffmpeg_branding=Chrome ..."`. Stock CEF binaries omit H.264/AAC/MP3, which breaks video/audio
   across the open web (x.com, reddit, LinkedIn). **The self-build is mandatory for codecs and is not
   going away.** Widevine is a *separate* layer: the CDM auto-downloads on any CEF build (basic DRM),
   but premium services (Amazon/Netflix) additionally need **VMP signing of our binaries** (Google
   MLA or Castlabs) — independent of CEF-vs-Brave and independent of the codec build. The "Brave has
   a fix-it button" on Amazon = Brave's Widevine CDM enablement prompt; that's the VMP/premium piece.

5. **Header (B2):** the slowness is **architectural** (a separate browser subprocess + IPC round-trip
   for the React header), confirming the instinct. CEF Views is a viable native path but **cannot
   reproduce arbitrary CSS branding** — it's Skia-via-C++-subclassing, no CSS. Measure first; this
   stays its own dedicated session.

---

## The architecture picture (why "build from Brave" is a category error)

```
        ┌─────────────────────────────────────────────┐
        │            Chromium source tree              │
        │                                              │
        │   //chrome  (chrome layer)  ← Brave, Vivaldi,│
        │      │        full browser app, extensions,  │  Opera build HERE
        │      │        toolbar (Views), CWS, sync      │
        │      ▼                                        │
        │   //content (content layer) ← CEF wraps HERE  │  Hodos is HERE (libcef)
        │      │        WebContents, no browser UI,     │
        │      │        no extension system by default  │
        │      ▼                                        │
        │   //third_party/blink (renderer)             │  ← farbling patches live HERE
        │              Canvas/WebGL/Audio/Navigator     │     (reachable from a CEF build)
        └─────────────────────────────────────────────┘
```

- **CEF** = an *embedding library* (`libcef`) exposing the **content layer**. We write our own shell
  (`cef_browser_shell.cpp`, `simple_handler.cpp`, overlays). No browser chrome, no extensions.
- **Brave** = a *full browser application* built on the **chrome layer** as a patch/overlay set
  (`brave-core` at `src/brave`, MPL-2.0). It is NOT a fork and NOT CEF.
- **Blink** (renderer) is *shared by both* and is where fingerprinting farbling is patched. **A CEF
  build pulls the same Blink source**, so we can patch it too — this is the key that unlocks B1
  without touching Brave.

"Build CEF from Brave" would mean porting CEF's content-layer embedding API onto Brave's modified
tree — a multi-month project nobody has done. Practically: not on the table.

---

## A4 — Should we build from Brave? **NO.** (Confidence: HIGH)

| Cost of going Brave | Detail |
|---------------------|--------|
| Abandon CEF | Re-port `simple_handler`, `simple_render_process_handler`, `HttpRequestInterceptor`, overlay system, wallet injection to Chrome-layer C++ APIs (different classes entirely) |
| Build scale | 60–150 GB disk, 16–32 GB RAM, 4–8+ hr cold builds; **cannot run on GitHub-hosted runners** (disk + 6h limit) |
| Maintenance | Rebase private patches every ~3 weeks on new Chromium; Brave runs a dedicated team for this |
| Widevine | No benefit — VMP creds are private (see Widevine section) |
| Farbling | Brave's patches depend on Brave-only session infrastructure; don't drop into CEF cleanly anyway |

**Verdict:** the things we wanted from Brave (source-level farbling, extensions) are either achievable
on CEF directly (farbling) or not solved by Brave either (extensions need the chrome layer, which a
Brave-as-our-shell migration *could* give but at ruinous cost). Stay on CEF.

---

## B1 — Farbling: the real fix (Confidence: HIGH feasibility, MEDIUM maintenance cost)

**Why our current JS-injection farbling breaks logins:** JS injection is detectable through ≥6
independent mechanisms — `Function.prototype.toString` regression, property-descriptor anomalies,
`delete`-to-reveal-prototype, **cross-context escape (the big one: `OnContextCreated` does NOT fire
for Web/Service Workers, so workers see raw unfarbled values)**, injection timing races, and stack-trace
footprints. CreepJS-class detectors exploit all of these. Login sites read the mismatch as
"automated/suspicious browser." (Confidence: HIGH)

**The options, ranked:**

| Option | Undetectable? | Worker coverage | Cost | Verdict |
|--------|--------------|-----------------|------|---------|
| A. Better JS injection (toString spoof, Proxy, descriptor cloning) | No (fundamental JS limits) | No | Low | Stopgap only |
| B. CEF public native hooks (V8 interceptors, snapshots) | No | No | Low | Insufficient — API doesn't exist |
| C. **Blink-level patches in our own CEF build** | **Yes** | **Yes** (ExecutionContext base covers workers) | High (rebase cadence) | **The fix** |
| D. Worker-only CEF source hook (`DidInitializeWorkerContextOnWorkerThread`) | Partial | Yes | Medium | Good intermediate step |

**Path C details:** CEF applies its own patches to Chromium via `cef/patch/patch.cfg`; third parties
add custom `.patch` files there (documented; OBS ships a patched CEF fork in production). Adapt the
*approach* (not a wholesale copy) from MPL-2.0 references **helium-chromium** and **fingerprint-chromium**,
which patch the exact Blink files (Canvas readback, WebGL `getParameter`, AudioBuffer, navigator) with
an origin-bound session-seed model. Start with Canvas + WebGL (highest value), tie to our existing
session token, commit to the ~4–6 week rebase cadence.

**This is why we keep self-building CEF** (ties directly to A1). Worker coverage (D) is the single
highest-impact fix and can land before the full patch set.

> License note: Brave/helium/fingerprint-chromium patches are MPL-2.0 / open. Adapt the technique;
> get a license check before reusing patch text verbatim.

---

## B4 — Extensions: re-scope required (Confidence: HIGH)

**Layer clarity (important — self-building does NOT unlock extensions):** farbling works in our
self-build because it lives in **Blink (renderer layer)**, *below* CEF. Extensions live in the
**chrome layer**, *above* where CEF operates — and the barrier is architectural, not a build flag.
Downloading + patching Chromium ourselves (which we already do for codecs) gets us farbling but NOT
extensions, because CEF deliberately wraps the content layer and the extension system is welded to
the chrome-layer `Browser` object CEF doesn't use. So "extensions = just more patches like farbling"
is **false**. Real extensions = adopt CEF's constrained Chrome runtime, OR eventually move off CEF
to a full-Chromium chrome-layer shell (Brave/Vivaldi model) — a major future stack decision, not 0.4.0.

**Hard finding:** CEF cannot run real Chrome Web Store extensions.
- Alloy-runtime extension API is unsupported/removed (~M127); only the **Chrome runtime** has any
  extension support, and that **requires showing Chrome's own toolbar UI** (conflicts with our custom
  header) and still implements only ~4 of 70+ `chrome.*` APIs.
- **MetaMask is MV3** (service-worker background). CEF does not support MV3 extension service workers
  or `chrome.scripting`. No documented case of MetaMask running on CEF exists. Even Electron (deeper
  Chrome integration than CEF) has these gaps.
- Vivaldi/Brave/Opera get extensions *only* because they compile the full `//chrome` layer.

**Implication:** "Allow MetaMask, block conflicting BSV wallets" is **not achievable on our current
CEF stack** as a Chrome-extension feature. B4 must be re-scoped. Candidate directions (need their own
research):
1. **Drop CWS extensions; keep a curated, first-party integration model** (e.g. we integrate specific
   wallet providers directly rather than hosting their extensions).
2. **Adopt the CEF Chrome runtime** — gains partial extension support but forces Chrome's toolbar UI
   and breaks our custom-header/overlay model (collides with B2).
3. **Reconsider the stack** for extensions specifically (full-Chromium shell) — very expensive, same
   class of cost as the Brave migration we just rejected.

**Wallet deconfliction (if/when extensions exist):** detect via EIP-6963 `rdns` (`io.metamask`),
the `isMetaMask` flag, and enforce via a **CRX extension-ID allow/block list**. Today, since no
extension can install, we can defensively lock `window.yours` (BSV provider) against override in our
V8 injection. (Confidence: HIGH detection / MEDIUM enforcement.)

---

## B2 — Header → C++: measure first (Confidence: HIGH cause, MEDIUM solution)

- **Cause confirmed:** the React header runs as a **separate CEF browser subprocess** (localhost:5137);
  every interaction crosses Win32 → CEF IPC → Blink/V8 → React → layout → composite. The latency is
  architectural, not a React micro-optimization problem.
- **Chrome/Brave** render the toolbar in native **C++ Views** (Skia), not web, not Win32 widgets.
- **CEF embedder options:** (A) OS-native Win32/Cocoa, (B) **CEF Views** (`CefWindow`/`CefPanel`/
  `CefTextfield`/`CefLabelButton`), (C) keep web (status quo). 
- **CEF Views trade-off:** viable for a native toolbar shell (eliminates the subprocess + IPC) but
  **no CSS** — custom branding is Skia-via-C++-subclassing. Pixel-perfect arbitrary CSS branding is
  NOT reproducible; a hybrid (Views shell + `CefBrowserView` for complex panels) is the realistic
  shape. Keeping the **exact** current look may be in tension with the native move — a key thing the
  dedicated B2 session must resolve.
- **Coupling alert:** the Chrome runtime (only extension path, B4 option 2) wants to own the toolbar —
  directly conflicts with a custom native header. **B2 and B4 must be decided together.**

> Next step for B2 is **measurement** (is it subprocess spawn, IPC warmup, or React render?) before
> any architecture is chosen. Stays a dedicated multi-agent session.

---

## Cross-item implications (the through-line)

- **Stay on CEF** (A4 = no Brave).
- **Keep self-building CEF — the justification is PROPRIETARY CODECS** (confirmed in build scripts:
  `proprietary_codecs=true ffmpeg_branding=Chrome`). Farbling is a *second* reason to self-build, and
  it rides on the build we already do — not a new cost center.
- **B1 (farbling) → Blink patches in our self-build** + worker coverage. Couples to A1's build pipeline.
- **B4 (extensions) is the strategic fork:** real extensions need the chrome layer CEF lacks. Re-scope
  B4, and decide it **jointly with B2** because the Chrome-runtime option fights a custom native header.
- **Widevine premium (Netflix) = VMP signing** (Google MLA or Castlabs), a separate workstream from
  the build question. Worth its own mini-spike if premium DRM is a product goal.

---

## Open questions for the user / next research

1. **A1:** ✅ ANSWERED — self-build is for proprietary codecs (`proprietary_codecs=true
   ffmpeg_branding=Chrome`), mandatory, not going away. The *real* A1 work is making the build **not
   take ~2 weeks**: caching (sccache), remote/cloud build execution, a reproducible documented runbook
   (see `CEF_BUILD_RUNBOOK.md`), and choosing the latest-stable CEF branch deliberately.
2. **B1:** Commit to the Blink-patch path (and its ~4–6 wk rebase cadence)? Start with worker-coverage
   hook as a quick win?
3. **B4:** Which re-scope direction — curated first-party wallet integration, Chrome-runtime adoption
   (with B2 cost), or defer extensions? This likely needs its own dedicated session like B2.
4. **DRM:** Is premium streaming (Netflix) a 0.4.0 product goal? If yes, spin a VMP-signing spike
   (Castlabs commercial path looks most accessible for a small team).
5. **B2 + B4 joint session:** sequence a combined native-UI + extensions architecture session, since
   the Chrome-runtime decision couples them.

---

## Source appendices

Full cited findings (every claim → primary source URL, with confidence ratings and explicit
"unknowns") are preserved from the four research passes. Key primary sources:
- Widevine: CEF issues #1631/#3149/#3404, CEF forum t=19440 (Greenblatt), Brave PR #2023 / issue #10865,
  Chromium `widevine.gni`, Castlabs VMP docs.
- Brave build: brave-browser wiki (Build Deconstructed, Chromium Rebases), brave-core `patching_and_chromium_src.md`,
  CEF General Usage (content vs chrome layer), GitHub Actions limits docs, brave.com/blog/building-brave.
- Farbling: brave.com fingerprinting-defenses, brave-core `execution_context.cc`, palant.info, CreepJS,
  helium-chromium, fingerprint-chromium, OBS CEF fork, CEF `patch.cfg`.
- Extensions/UI: CEF forum (Greenblatt on Chrome-style-window requirement), CEF issue #3450/#1947,
  cef-announce Alloy deprecation, EIP-1193 / EIP-6963, Chromium Views design docs, CefWindow/CefBrowserView refs.
```
(If we want the full per-source tables inlined here later, they're in the session transcript and can
be appended — kept lean for now per the scan-fast convention.)
```
