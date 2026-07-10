# Chromium/CEF Rebuild Sprint — DESIGN OUTLINE (→ v0.4.0-beta.1)

**Created:** 2026-07-09 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**Status:** OUTLINE (Workflow-1 Phase C, post adversarial-review revision). Later sessions expand each area into detailed plans + `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`.
**Mode:** research + design only — **NO code, NO builds.** This document stops at the plan.

> **What this is.** The phase-ordered skeleton of the whole build sprint: provision the build host → prove the pipeline on the current config → bump to the newest sensible source *with our edits* → stand up the CEF patch toolchain → land the Blink farbling patch set → verify codecs/DRM → test (incl. a real auto-update apply) → prod build → `v0.4.0-beta.1`. Every area named here gets its own detailed plan doc in a later session (see §6, §9).

> **Authoritative inputs (read these before expanding any section):**
> `DevOps-CICD/CEF_BUILD_RUNBOOK.md`, `DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md`,
> `DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md`, `DevOps-CICD/DEPENDENCY_VERIFICATION.md`,
> `DevOps-CICD/WINDOWS_AUTOUPDATE_PLAN.md` + `AUTO_UPDATE_AND_SIGNING_0_4_0.md` (for the §7 update-apply gate),
> `0.4.0/B1-farbling-design.md`, `0.4.0/SPRINT_0_4_0_MASTER_PLAN.md` (§7.3, §7.6, §11), the kickoff
> `0.4.0/CHROMIUM_CEF_SPRINT_KICKOFF.md`, and the build scripts `scripts/build_hodos_cef.bat` /
> `scripts/build_hodos_cef_mac.sh` (paths referenced throughout the runbook; confirm at plan time).

---

## 1. Goal & non-goals

### Goal
Produce a fresh, reproducible custom Chromium+CEF build that carries **all of Hodos's own source edits**, on a
**current, security-supported** CEF/Chromium version, and drive it end-to-end:

0. **Provision** the build host (disk/RAM/depot_tools/local sccache — §4 P0).
1. **Pin** version + toolchain + runner images (they move together).
2. **Baseline** — prove the self-build pipeline still works on today's config (guarded — see §4 P1 / OQ-7).
3. **Version bump** 136 → target (see §2).
4. **Stand up the CEF patch toolchain** (greenfield — no patch infra exists in the repo today).
5. **Land the Blink farbling patch set** (owner-committed source/Blink migration).
6. **Verify codecs + basic DRM.**
7. **Test** (codec smoke, farbling acceptance, regression basket, minos guard, **real N-1→N auto-update apply**, Win+Mac parity).
8. **Prod build** + stage binaries to the `cef-binaries` GitHub release the Tier-2 app pipeline consumes.
9. **Gate `v0.4.0-beta.1`** on the readiness checklist (§7).

### Non-goals (explicitly OUT of this sprint / beta.1)
- **B4 — Chrome-extension hosting** (MetaMask etc.). Architecturally infeasible on CEF's content layer; moved
  to a future sprint (`Future-Features/B4-extensions.md`). The 0.4.0 slice is EIP-6963 deconfliction at the
  injection layer, which is *not* part of this build sprint.
- **B2 — header→native C++ (Views) port.** Decided keep-React (`SPRINT_0_4_0_MASTER_PLAN.md` Q17). No toolbar
  rewrite in this sprint.
- **Premium/HD DRM (Widevine L1 / VMP signing).** OUT of beta.1 unless a genuinely cheap/free path exists
  (owner stance). We test the free component-updater CDM path only — noting (I6) that VMP `.sig` may gate even
  L3 on Windows (§3d, §6-Q4).
- **Distributed/remote build (Siso + REAPI backend).** Deferred beyond 0.4.0 (`SPRINT_0_4_0_MASTER_PLAN.md`
  §7.3). We accept a self-hosted VM / beefy machine + local sccache for this sprint. Cold from-scratch builds
  get no sccache benefit either way.
- **Per-profile wallet** and other shelved architecture items — untouched.

---

## 2. Version-target decision

We are on **CEF 136 / Chromium 136 (branch 7103)** — **~15 months** behind stable (M136 ≈ April 2025 → target
build ≈ July 2026; corrected from the earlier "~12 months" per **M8**) and predating any enterprise long-term
channel we can rely on, so **effectively zero current security-patch coverage**
(`CEF_VERSION_UPDATE_TRACKER.md`; `SPRINT_0_4_0_MASTER_PLAN.md` §7.3). The bump is not optional.

### Default target: **current CEF stable** (revised per C1)
> **Revision (C1).** The prior draft made "newest CEF LTS branch" the default. That anchored the roadmap on a
> program whose existence is (a) unconfirmed, (b) contradicted by our own 2026-06-17 decision, and (c)
> described with numbers ("every 6th branch, ~8 months") that look like a **garbled restatement of Chromium
> *Extended Stable*** (every *2nd* milestone, +8 weeks — a Chromium/enterprise channel, **not** a CEF LTS).
> **We therefore invert the default: current CEF stable is the target.** LTS is an *upgrade path only if*
> primary sources confirm it (below).

### ⚠️ Step 0 of the version-target detailed plan: reconcile the doc conflict from PRIMARY sources
The in-repo docs (`SPRINT_0_4_0_MASTER_PLAN.md` Q16/§11.1) say "no LTS, target M149." The 2026-07-09 research
seed says a CEF LTS program exists from M138. **These contradict — resolve before any support-window planning
depends on it.** The plan MUST, as step 0:
- **Explicitly test the "LTS = misremembered Chromium Extended Stable" hypothesis (C1)** before trusting any
  LTS branch cadence or support window.
- Confirm the **exact current CEF stable** version + branch number from `cef-builds.spotifycdn.com/index.json`
  (never a wiki).
- Determine whether a **CEF LTS/LTC channel** genuinely exists from **primary sources** (CEF branch policy /
  cef-project / official announcements — not a wiki, not the seed). If confirmed, record its branch cadence
  and a **concrete security-support-end date** for the candidate branch.
- Cross-check the **Chromium Dash** stable-exit + any Extended-Stable windows for each candidate milestone.
- **Verify the actual current Chromium stable cadence (I13).** The draft asserted a "2-week cadence arriving
  Sept 2026"; Chromium has been on a **4-week** stable cadence since M94 (2021) with an 8-week Extended-Stable
  option, and no 2-week cadence is known to us. Confirm against primary release-schedule sources and, if it is
  4-week, correct the rebase-frequency math below and soften the LTS-urgency framing accordingly.

### Upgrade to LTS only if ALL hold
Adopt an LTS branch instead of current stable only if primary sources confirm: (1) the program exists as a
genuine CEF (not Chromium Extended-Stable) channel; (2) it covers the platforms/arches we ship (Win x64, mac
arm64/x64 — §I8); (3) the newest LTS branch is **not already near its support-window end**; and (4) it isn't
missing a milestone-gated feature/fix we need (FedCM maturity, an EME/Widevine handler change, a media fix).
If any fail, **ship current stable** and pull only security point-releases between deliberate milestone jumps.

### Why milestone frequency matters even without LTS (I10, I13)
The recurring cost is **rebase labor = bump-frequency × patch-depth**. Our chosen Blink patch targets
(`base_rendering_context_2d.cc`, `webgl_rendering_context_base.cc`, `static_bitmap_image.cc`,
`navigator_base.cc`) are **high-churn files that will conflict on most milestone jumps**. The detailed plan
must produce an **estimated hours-per-bump for patch-conflict resolution** (not just build hours) and use
*that* number, alongside the corrected cadence (I13), to weigh stable vs LTS. LTS still reduces frequency; the
magnitude of the benefit depends on the real cadence.

### Concrete lookups the detailed plan must perform (do not guess)
1. Current CEF **stable** version → branch → Chromium milestone (`index.json`).
2. Whether a CEF **LTS/LTC** program exists (primary sources), and if so branch → milestone → support-end date.
3. Chromium Dash schedule + real stable cadence (I13) + Extended-Stable windows.
4. **MSVC/Clang toolset** the candidate CEF is built with (drives §3f + §I9 ABI alignment).
5. **macOS floor** for the candidate Chromium (which macOS versions dropped) — for §3f minos.
6. Feature deltas vs 136: FedCM UI/permission API (see new §3g), Permissions API, CefResponseFilter
   stability, EME/Widevine handler changes (`CEF_VERSION_UPDATE_TRACKER.md` "Must Investigate").

> **Assumption carried in this outline:** target = **current CEF stable**, LTS only on confirmation. Where a
> section needs a concrete number it uses "TARGET" as a placeholder. The phase order is identical either way.

---

## 3. The complete source-edit list (reconstructed)

Every Chromium/CEF-tree edit we plan to carry. "Edit" = a change applied to the Chromium/CEF source *before
compile* (GN flag, `patch.cfg` `.patch` file, or automate-git config) — distinct from our C++ shell code in
`cef-native/`, which is not a Chromium-tree edit but *is* listed where shell changes are prerequisites.

### (a) Proprietary codecs — GN flags `[EXISTS — carry forward]`
- **What:** `GN_DEFINES = is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome
  chrome_pgo_phase=0` in `scripts/build_hodos_cef.bat` / `_mac.sh`.
- **Why:** Stock CEF omits H.264/AAC/MP3 → breaks video/audio across the open web. This is the reason we
  self-build (`BRAVE_FORK_FEASIBILITY.md` §4; `reference_cef_self_build_reason`).
- **Platform:** both. **Size:** tiny (flags; ~15 MB codec code in `libcef.dll`). **Dependency:** none —
  baseline (P1). Re-verify the flag still takes effect after the bump (a flipped Chromium default can ship a
  green build with no codecs — `CEF_BUILD_RUNBOOK.md` Step 5.5).
- **Codec coverage note (M3):** the smoke set below (§7) asserts H.264/AAC/MP3/VP9/**AV1 decode presence**.
  **HEVC/H.265 (`enable_platform_hevc`) and Dolby are OUT of scope for beta.1** — record explicitly rather
  than leave ambiguous.

### (b) CEF patch toolchain standup — PIPE-A1 `[GREENFIELD — verified: no cef/patch/ in repo]`
- **What:** Fork `chromiumembedded/cef`; create `patch/patches/*.patch` + register in `patch/patch.cfg`;
  point the build at the fork via `automate-git.py --url=<our cef fork>`. `patcher.py` applies our patches to
  the Chromium source before compile. Optional `condition` env gate for build-time on/off.
- **Why:** No patch infrastructure exists today (`cef/patch/**` glob returns nothing — **verified**). Every
  Blink farbling patch (c) needs this first. **The serial linchpin that blocks all source-level farbling.**
- **Platform:** both (one fork/toolchain; patches are cross-platform text — §5). **Size:** L. **Dependency:**
  rides on the self-build (P1); must exist before P4.
- **Sub-work:** decide fork-hosting + maintenance model; wire `automate-git --url`; add the Step 5.5
  drift-audit hook (re-apply patches, report fuzz) as part of standing it up.
- **Standing obligation (M6):** the fork **must track CEF's own in-branch security point-releases** — pull
  upstream security commits into the fork between deliberate milestone jumps, or the "security coverage"
  benefit erodes. This is a recurring maintenance duty, not a one-time setup.

### (c) Blink farbling patch set — FEAT-B1 `[NEW — owner-committed source/Blink migration]`
Replaces today's JS-injection farbling (`cef-native/include/core/FingerprintScript.h`,
`FingerprintProtection.h`, injected in `simple_render_process_handler.cpp:586-632`). Sub-edits, highest value
first (land incrementally — §4 P4):

| Sub-edit | What | Blink files (`third_party/blink/renderer/...`) | Size | Dep |
|---|---|---|---|---|
| **C1 — HodosSessionCache Supplement** | `Supplement<ExecutionContext>` holding farbling state; patched APIs read `HodosSessionCache::From(*ctx)`. Attaches to `LocalDOMWindow` **and in-process `WorkerGlobalScope`/worklets**; OOP workers need explicit plumbing (see below + I2). | new supplement file + hooks | L | PIPE-A1 |
| **C2 — Per-profile seed wiring (NO persistent secret on cmdline — C2 fix)** | Persistent per-profile `profile_seed` lives in **C++ profile data** (NOT the wallet). **It is never placed on a renderer command line.** Delivery to the renderer is either (a) a **per-launch ephemeral nonce** on the cmdline that the browser process maps to the real seed, or (b) a **non-inspectable channel (mojo / pref-on-navigation)** — chosen in the C2 detailed plan. Renderer computes `domain_seed = HMAC-SHA256(profile_seed, first_party_eTLD+1)` (**first-party keying — I4**). | switch/mojo wiring + renderer seed derivation; C++ in `ProfileManager`/`SettingsManager` (shell) | M | C1 |
| **C3 — Canvas 2D** | Readback farbling: `getImageData`/`toDataURL`/`toBlob`; `measureText` gate. Prefer the shared bitmap-readback path (`platform/graphics/static_bitmap_image.cc`) for canvas-2D readback. **Does NOT cover WebGL `readPixels` (I3).** | `modules/canvas/canvas2d/base_rendering_context_2d.cc`, `canvas_rendering_context_2d.cc`, `platform/graphics/static_bitmap_image.cc` | L | C1 |
| **C4 — WebGL** | `getParameter` (incl. `UNMASKED_VENDOR/RENDERER`), `getSupportedExtensions`, **and `readPixels` with its OWN patch point** in the WebGL contexts (framebuffer readback does not funnel through C3's `StaticBitmapImage` path — **verify code paths before sizing C3/C4**). | `modules/webgl/webgl_rendering_context_base.cc`, `webgl2_rendering_context_base.cc` | L | C1 |
| **C5 — WebAudio** | AnalyserNode / AudioBuffer readback perturbation. | `modules/webaudio/analyser_handler.cc`, `audio_buffer.cc`, `realtime_analyser.cc` | M | C1 |
| **C6 — Navigator** | `deviceMemory`, `hardwareConcurrency` (compile-time getter replacement, constrained to valid sets), plugins array. | `core/frame/navigator_device_memory.cc`, `core/execution_context/navigator_base.cc`, `modules/plugins/dom_plugin_array.cc` | M | C1 |
| **C7 — Auth-domain exemption at source (re-implement `IsAuthDomain` ONLY — I1)** | Re-implement today's **`FingerprintProtection::IsAuthDomain`** hardcoded C++ allowlist at the Blink/browser layer so pre-approved/auth sites are NOT farbled. **`hodos-unbreak.txt` and adblock scriptlet exemptions are untouched by this sprint** (they are an adblock-engine concern, not farbling — verified). No `#@#+js()` involvement. | eTLD+1 auth allowlist passed to renderer; `HodosSessionCache` returns pass-through when top-frame origin ∈ allowlist | M | C2, Q3 |

**OOP-worker seed plumbing (I2 — was overstated as "workers for free").**
`Supplement<ExecutionContext>` covers `LocalDOMWindow` and **in-process** worker/worklet contexts, but **shared
workers and service workers run in separate processes** with their own command lines and origin semantics
(service-worker origin = registration scope, not top-frame). The cmdline/mojo seed delivery and first-party
keying do **not** reach them automatically. The C1/C2 detailed plan must **enumerate worker/worklet types**
(dedicated, shared, service, audio worklet, paint worklet, OffscreenCanvas-in-worker) and specify seed
plumbing to the OOP ones. Acceptance (§7) adds explicit **service-worker, shared-worker, and
OffscreenCanvas-in-worker** cases — not just the dedicated-worker column CreepJS exercises.

- **Why:** JS injection is detectable ≥6 ways and **never fires for workers** (`FingerprintScript.h` confirmed
  worker-blind) → raw values leak → login sites read us as suspicious → login breakage. Blink patches are
  undetectable and cover in-process workers (`BRAVE_FORK_FEASIBILITY.md` B1; `B1-farbling-design.md`).
- **Platform:** both (single cross-platform patch set, built per-OS — §5, Q1).
- **License (M7):** **re-implement the technique in a genuine clean room.** Brave = MPL-2.0 file-copyleft:
  don't copy text — and note that *transcribing Brave's logic while reading its MPL-2.0 source is still
  derivative-work risk*, so maintain a real clean-room boundary (read the spec/behavior, not the source, when
  writing our patch). fingerprint-chromium = BSD-3 permissive but its WebGL-metadata path is Linux-only +
  Chrome 144 removed the flags → Win/Mac must re-implement. **Bromite = GPL-3 FORBIDDEN.**

- **JS-farbling teardown checklist (M1).** Migration must retire, not just orphan, the old path:
  delete injection at `simple_render_process_handler.cpp:586-632`; retire the `FingerprintProtection.h` /
  `FingerprintScript.h` singletons; remove the fingerprint-seed IPC chain (`s_domainSeeds`, the `OnBeforeBrowse`
  seed IPC); **migrate `IsAuthDomain`'s allowlist into C7** (do not leave two sources of truth). Guard against
  double-seeding / dead code.

- **🚩 OPEN DESIGN CONFLICT (settle before writing C4/C6 — feeds Q5):** the `B1-farbling-design.md` list
  **re-adds** WebGL `UNMASKED_VENDOR/RENDERER` and navigator `hardwareConcurrency`/`deviceMemory` that the
  current JS impl **deliberately removed** as detectable. §7.6 resolves:
  - **deviceMemory / hardwareConcurrency: SAFE to re-add at C++**, but **MUST constrain to the standard valid
    set** (deviceMemory ∈ {2,4,8,16,32}; concurrency to plausible counts) — an out-of-spec value is itself a
    fingerprint.
  - **WebGL vendor/renderer: DANGEROUS even natively** — random strings are *more* unique than the truth. If
    re-added, map to a **small set of common real GPU strings** (must include **Apple Silicon *and* Intel Mac
    ANGLE strings** — I8), never noise; the original instinct to **drop** it was defensible.
  - **Owner decision 2026-06-17 (Q18):** default to **Brave parity** unless concrete site-breakage argues
    otherwise. **Conflict stays OPEN for the C4/C6 detailed plan to settle with a final value table (Q5).**

- **Additional fingerprint vectors to decide or log as accepted gaps (M2).** Not in the current JS impl and
  not yet scoped: **UA-CH high-entropy client hints** (`navigator.userAgentData.getHighEntropyValues`),
  **screen / `devicePixelRatio`**, **`getClientRects`**, **font enumeration** beyond `measureText`, and
  **`enumerateDevices`**. The Q5 value table must either add these under Brave-parity or **explicitly log each
  as an accepted gap** with rationale.

### (d) Widevine / DRM — component-updater test + VMP `[TEST + LIKELY DEFER]`
- **What:** (1) On the target build, `enable_widevine=true` is set by CEF's build system; the CDM is **not** in
  the output — Chromium's component updater auto-downloads it at runtime (~5 min after first launch). **Test
  whether basic L3 DRM (Amazon movie) then plays.** (2) If Amazon demands higher robustness (L1), that needs
  **VMP signing** of our binaries — its own mini-spike (Google MLA vs commercial 3PL; castLabs' free EVS path
  is Electron-only, not CEF).
- **Reality-check (I6 — "free L3" is over-optimistic on two counts):**
  - (a) **On Windows the component-updater L3 CDM commonly still requires a VMP `.sig`** alongside the binary
    for playback — so "test L3 for free" may itself be blocked by the very VMP signing we defer. **Verify
    whether a VMP `.sig` is required for L3 on Windows *before* concluding "free."**
  - (b) **Amazon is among the strictest services**; software-only L3 is frequently **refused or SD-capped**.
    Expect L3 may yield SD-only or be refused entirely. "Amazon likely plays at L3" is the weakest link —
    treat it as a hypothesis to falsify, not an assumption.
- **Why:** Amazon movie failed while YouTube/X/LinkedIn work → prime suspect missing/insufficient CDM (Q4).
- **Platform:** both. **Size:** S to test; VMP = L + $ (deferred; note it may gate L3, not only L1).
  **Dependency:** rides on the codec build (P5). **Default: OUT of beta.1** unless the free component-updater
  path fixes Amazon cheaply. Document the exact error, whether it matches Brave's ("fix-it button" = on-demand
  CDM enablement), cost, and which sites break either way.

### (e) Dependency bumps — A3 `[RIDES WITH THE VERSION JUMP]`
- **What:** After the bump, run the full `DEPENDENCY_VERIFICATION.md` pass for **Hodos's own** deps (gclient
  resolves Chromium's internal deps automatically; the hard part is *our* deps staying ABI/toolchain-compatible):
  vcpkg static deps (`nlohmann-json`, `sqlite3`, OpenSSL, quirc); the CEF wrapper; frontend React/Vite/TS +
  browser-API-dependent JS; Rust crates sensitive to platform/toolchain (`rust-wallet`, `adblock-engine`).
  Record a per-bump "touched / deferred" table.
- **Platform:** both (vcpkg = Win-heavy; libcurl/Keychain = Mac). **Size:** M. **Dependency:** after the bump
  fetches (P2), before test (P6).

### (f) Version-bump mechanics + toolchain/minos alignment `[PROCESS — build-breaker if wrong]`
- **What (all move together — `CEF_VERSION_UPDATE_TRACKER.md` "Toolchain" + "macOS Minimum"):**
  - **Branch:** update `--branch=` in both build scripts to TARGET.
  - **Toolchain ABI match (revised per I9 — two DISTINCT toolchains, do not conflate):**
    1. **Chromium/CEF build toolchain** — lives on the **self-hosted build host** (the Chromium build cannot
       run on GitHub-hosted runners: 6-hr job cap, ~14 GB disk). Note the MSVC/Clang toolset the *target CEF*
       is built with and provision the build host to match.
    2. **CI app-build runner** — builds `cef-native` + the CEF wrapper. **The ABI-critical match is between
       the CEF binary's toolset and the toolset building `cef-native`/the wrapper**, NOT the Chromium build's
       runner. Pin the CI `runs-on:` (`release.yml`) so its MSVC/Clang matches the CEF binary's toolset —
       **never `windows-latest`/`macos-latest`** (the beta.16 windows-2025 drift failure). Make the ABI-match
       requirement explicit in the plan.
  - **macOS minos:** look up the new Chromium's oldest supported macOS; **`vtool`-measure** the actual CEF
    framework `minos`; set published min = `max(Chromium floor, measured minos)` in **all three** places
    (`cef-native/CMakeLists.txt` `CMAKE_OSX_DEPLOYMENT_TARGET`, `cef-native/Info.plist` +
    `cef-native/mac/helper-Info.plist.in` `LSMinimumSystemVersion`); apply via
    `-DCMAKE_OSX_DEPLOYMENT_TARGET=` + `MACOSX_DEPLOYMENT_TARGET` at job level; **CI minos guard** fails the
    build unless every exe/helper/Rust-bin `minos ≥` framework minos.
  - **Runtime file-manifest drift audit (Step 5.5):** diff the new CEF dist's DLL/`.bin`/`.pak`/`resources`/
    `locales` list against the hardcoded copy-lists in `cef-native/CMakeLists.txt` + the mac framework-embed
    list; diff pinned `GN_DEFINES` vs new defaults. **This audit feeds the auto-update apply gate (C3/§7): a
    changed manifest is exactly what breaks a silent update.**
  - **Version single-sourcing (PIPE-VERSION):** git tag = source of truth; `cargo-release` bumps+tags; CMake
    (`cmake-git-version-tracking`), Rust (`shadow-rs`), and a CI step inject the tag into the Inno `.iss` +
    the TS constant.
- **Platform:** both. **Size:** M–L. **Dependency:** P0 (pin) + gates P2 (bump) and P7 (prod).

### (g) FedCM permission-handler coverage `[SHELL AUDIT — testable NOW, per I7]`
- **What:** FedCM has been on-by-default since ~M108, so it is **already live on our M136 build.** "Sign in
  with Google" via FedCM routes through a `CefPermissionHandler` path; if our handler doesn't cover the FedCM
  permission type, the account chooser **silently fails.** Audit `CefPermissionHandler` FedCM coverage **now**
  (not as a bare post-bump regression checkbox). If the target version changed the permission API, scope the
  shell edit here.
- **Platform:** both. **Size:** S–M. **Dependency:** independent of farbling; verify pre-bump and re-verify on
  target.

---

## 4. Phase order

Dependencies flow top-to-bottom; a phase may not start until its blockers are green.

```
P0  PROVISION+PIN  build host (≥100+ GB disk, 32+ GB RAM, depot_tools,       (blocks all)
     │             cc_wrapper/local sccache) + version/toolset/runner pin +
     │             minos plan + confirm M136 still fetches/builds (OQ-7)
     ▼
P1  BASELINE       from-source build on CURRENT config (136) — codecs (a)    (proves pipeline;
     │             only, no new patches. GUARDED: if M136 no longer           partial isolation —
     │             gclient-syncs/builds on the pinned toolchain, DOWNGRADE     see note)
     │             to a smoke of the last-known-good environment instead
     ▼
P2  BUMP           jump to TARGET (§2); deps pass (e); toolchain+minos align  (blocks P3+)
     │             (f); FedCM audit (g); Step 5.5 drift audit; codec re-verify
     ▼
P3  PATCH TOOLCHAIN  stand up PIPE-A1 (b): fork cef, patch/patches,           (blocks P4)
     │               patch.cfg, automate-git --url; prove a no-op patch
     │               applies + builds
     ▼
P4  FARBLING       land C1..C7 INCREMENTALLY:                                 (independent of P5)
     │   P4a  C1 Supplement + C2 seed/channel (ephemeral-nonce or mojo) →
     │        WORKER-COVERAGE QUICK WIN (ship Supplement w/ just Canvas first)
     │   P4b  C3 Canvas + C4 WebGL incl. readPixels (resolve §3c conflict here)
     │   P4c  C5 Audio + C6 Navigator (valid-set constrained)
     │   P4d  C7 auth-domain exemption at source (IsAuthDomain only, Q3)
     │   P4e  OOP-worker seed plumbing (service/shared/worklet — I2)
     ▼
P5  CODECS/DRM     re-verify codecs (a) on target; test component-updater     (parallel-ok w/ P4)
     │             Widevine CDM → Amazon (d/Q4), incl. VMP-.sig-for-L3 check
     ▼
P6  TEST           codec smoke; farbling acceptance (worker==window + intra-   (blocks P7)
     │             session consistency + cross-profile difference + cross-
     │             session login + cross-site iframe); soak/crash-rate;
     │             canvas/WebGL perf gate; minos guard; REAL N-1→N auto-update
     │             apply on BOTH OS; Win+Mac parity; adblock + OAuth still work
     ▼
P7  PROD BUILD     official Release build both OS; stage → cef-binaries        (gated by P6)
     │             GitHub release; rebuild wrapper + cef-native; Tier-2
     │             app pipeline (release.yml) consumes
     ▼
[GATE] v0.4.0-beta.1 readiness checklist (§7)
```

**Why this order / what blocks what:**
- **P0 provisioning is a real phase (M4).** ~100+ GB disk, 32+ GB RAM, depot_tools, and local sccache/cc_wrapper
  are prerequisites the build literally cannot start without. P0 also **confirms M136 still fetches** (I5/OQ-7).
- **P1 before P2 — with an honesty caveat (I5).** Proving the *unchanged* pipeline builds first isolates *most*
  P2 breakage to the version jump. **But P2 changes version AND toolchain together, so isolation is only
  partial** — state this plainly. Additionally, **building M136 from source in mid-2026 may no longer fetch**
  (Google deprecates old sysroots/CIPD/toolchain packages). P0 must confirm M136 still `gclient sync`s and
  builds on the pinned toolchain; **if it is bit-rotted, downgrade P1 from a full cold build to a smoke of the
  last-known-good environment** rather than a gate that can't be met.
- **P3 before P4** — no Blink patch can build without the patch toolchain (PIPE-A1 blocks FEAT-B1-PATCH,
  `SPRINT_0_4_0_MASTER_PLAN.md` §11.4). The serial linchpin.
- **P4a first inside P4** — the Supplement + in-process worker coverage is the single highest-signal detection
  fix and can ship before the full patch set (`B1-farbling-design.md` "quick win"). **P4e (OOP workers)** is
  called out as its own step because it is *not* free.
- **P4 ∥ P5** — codec/DRM verification is independent of farbling; run in parallel to shorten the critical path.
- **P6 gates P7** — no prod build until farbling acceptance + intra-session consistency + soak + perf + minos
  guard + **the real auto-update apply** pass on both OS.

**Serialization warnings:** F4 (parking_lot) and FEAT-B1-PATCH are both L/XL cross-cutting — do not run
concurrently (`SPRINT_0_4_0_MASTER_PLAN.md` line 150). Cold from-scratch builds get **no** sccache benefit;
budget ~10–12 hr per cold build **per OS** (§I8: the Mac build is its own ~10–12 hr from-source build, not a
light inherit), plan incrementals on warm objects.

---

## 5. Windows vs macOS ownership split

**Core principle:** the source edits are **one cross-platform patch set + one shared GN config**, but **the
build is a full, first-class, separate effort per OS** (Windows produces `libcef.dll`; macOS produces
`Chromium Embedded Framework.framework` with its own Xcode/clang build, signing, framework packaging, and
notarization; DLLs cannot be reused on Mac — `CEF_BUILD_RUNBOOK.md` Step 3). **Mac is a parallel build, not an
inherit-and-verify afterthought (I8).**

**macOS architecture decision (I8 — must be settled in §5's detailed plan, before P2 on Mac):**
choose **arm64 vs x86_64 vs universal2**. This materially changes Mac build time/cost/disk and the "common
real GPU strings" set for C4 (Apple Silicon ANGLE strings vs Intel Mac). Default lean to confirm with owner:
**universal2** for distribution breadth, accepting the longer build — but the plan must state the cost and get
sign-off.

| Work | Shared / per-OS | Owner |
|---|---|---|
| GN flags / codecs (a) | Shared config, built per-OS | Windows (lead) authors; Mac verifies flag takes effect on framework |
| CEF fork + patch.cfg + `.patch` files (b, c) | **Shared** — one patch set, cross-platform Blink text | Windows (lead) authors the toolchain + patches |
| Blink farbling C1–C7 | **Shared source**, compiled into each OS's binary | Windows authors; Mac inherits patches + owns its build/behavior + Mac GPU-string entries (Q1) |
| Per-profile seed C++ wiring (C2) | Shell code — `#ifdef _WIN32` / `#elif __APPLE__`; Mac creation paths in `cef_browser_shell_mac.mm` | Windows authors, Mac ports platform conditionals (Invariant #9) |
| OOP-worker seed plumbing (P4e) | Shared design, per-OS verify | Windows authors; Mac verifies OOP contexts on framework |
| **macOS arch decision (arm64/x64/universal2)** | **Mac-specific** | **Mac Claude owns** (with owner sign-off) |
| Chromium build toolchain (self-hosted) | Per-OS (MSVC vs Xcode/clang) | Windows owns win build host; **Mac owns Xcode/clang build host** |
| CI app-build runner pin (ABI-match to CEF binary — I9) | Per-OS | Windows owns win runner pin; **Mac owns `macos-NN` runner pin** |
| **minos / deployment-target / `vtool` / plist edits / minos guard** | **Mac-specific** | **Mac Claude owns entirely** |
| Runtime file-manifest copy-list (Step 5.5) | Per-OS: `cef-native/CMakeLists.txt` (Win) vs framework-embed list (Mac) | Each OS owns its list |
| **Real N-1→N auto-update apply test (C3 gate)** | Per-OS (WinSparkle/custom updater vs Sparkle) | Each OS runs its own; Windows leads write-up |
| Codec smoke + farbling acceptance + regression basket | Per-OS run | Each OS runs its own; reconciled in the coordination doc |
| Widevine CDM component-updater test (d) incl. VMP-.sig check | Per-OS behavior differs | Each OS tests; Windows leads the write-up |

**Coordinate via a new `CHROMIUM_BUILD_RELAY.md`** (or an extension of `MAC_WINDOWS_RELAY.md`). **Q1** is
answered by this table: **one shared patch set, built per-OS as a first-class parallel effort; Mac inherits the
patches, owns the OS-specific build/arch/minos/plist wiring + Mac GPU strings** — its dedicated doc expands it.

---

## 6. The 5 research questions (stubs → dedicated docs)

- **Q1 — Mac farbling.** *Stub:* one **cross-platform Blink patch set**, compiled per-OS — Mac does not author
  separate farbling logic, but the Mac build is a **full parallel effort** (I8), not an inherit. Mac inherits
  C1–C7; owns the OS-specific build (framework not DLL), the **arm64/x64/universal2 arch decision**,
  minos/plist wiring, per-profile-seed platform conditionals in `cef_browser_shell_mac.mm`, and macOS farbling
  acceptance. **The "common real GPU strings" set (§3c C4) needs Mac entries — both Apple Silicon and Intel
  ANGLE strings.** → `chromium-rebuild/Q1_mac_farbling.md`.
- **Q2 — Farbling × adblock.** *Stub:* adblock is a **separate Rust process (31302) + C++ `AdblockCache` +
  cosmetic CSS/scriptlet injection**; farbling moving into the CEF binary (Blink, below JS) is a *different
  layer* and should not collide. **`hodos-unbreak.txt` is an adblock file and is untouched by this sprint
  (I1).** Verify: (1) **ordering** — cosmetic scriptlet injection in `simple_render_process_handler.cpp` and
  Blink farbling both touch the renderer; confirm no shared V8-timing assumption breaks; (2) the removed JS
  farbling site (`:586-632`) is deleted cleanly without disturbing adjacent scriptlet/cosmetic IPC handlers
  (per the M1 teardown checklist). → `chromium-rebuild/Q2_farbling_adblock.md`.
- **Q3 — Farbling × OAuth pre-approved sites.** *Stub:* today JS-farbling **skips auth domains via the
  hardcoded C++ `FingerprintProtection::IsAuthDomain` list** (NOT `hodos-unbreak.txt`). C7 re-implements *that
  list* at source: pass an eTLD+1 auth allowlist to the renderer alongside the seed channel; when the current
  **top-frame** origin ∈ allowlist, `HodosSessionCache` returns pass-through (un-farbled) values. Persistent
  per-profile seed already reduces login breakage; the exemption is belt-and-suspenders for the most sensitive
  OAuth flows. → `chromium-rebuild/Q3_farbling_oauth.md`.
- **Q4 — Amazon DRM.** *Stub:* prime suspect = missing/insufficient **Widevine CDM**. On the target build the
  CDM likely auto-downloads via component updater → **test whether Amazon plays at L3 first**, but expect it
  may be **SD-capped or refused** (I6) and **verify whether a VMP `.sig` is required for L3 on Windows before
  concluding "free."** If Amazon demands L1/higher robustness → **VMP signing** (Google MLA or commercial 3PL;
  castLabs' free path is Electron-only) — **default OUT of beta.1.** Document the exact error, Brave-parity,
  cost, and which sites break. → `chromium-rebuild/Q4_widevine_amazon_drm.md`.
- **Q5 — Full reconciled edit list.** *Stub:* = §3 of this outline, hardened into a final table once the §3c
  WebGL/navigator conflict is settled (value tables: deviceMemory/concurrency valid sets; common-GPU-string
  map incl. Mac entries, or the decision to drop vendor/renderer) **and the M2 extra vectors (UA-CH, screen/DPR,
  getClientRects, fonts, enumerateDevices) are each added or logged as accepted gaps.** Every edit →
  what/why/platform/size/dep + final value decisions. → `chromium-rebuild/Q5_full_edit_list.md`.

---

## 7. v0.4.0-beta.1 readiness checklist

Gate items — all green before cutting `v0.4.0-beta.1` (each maps to a phase):

**Build integrity**
- [ ] Target version + branch confirmed from `index.json`; **LTS-vs-stable decision recorded with the
      Extended-Stable-conflation hypothesis explicitly tested (C1)**; cadence corrected per I13.
- [ ] **Target branch confirmed on ACTIVE security support; support-end date recorded in
      `CEF_VERSION_UPDATE_TRACKER.md`; in-flight security point-release cadence documented (I12).**
- [ ] Build is **reproducible** from `scripts/build_hodos_cef.{bat,sh}` on the pinned toolchain; changelog
      appended to `CEF_VERSION_UPDATE_TRACKER.md` (branch, milestone, GN_DEFINES, patch-set version, deps,
      duration, **estimated per-bump patch-rebase hours — I10**).
- [ ] **CI app-build runner** images pinned (`runs-on:` — no `*-latest`) so their MSVC/Clang **match the CEF
      binary's toolset** (ABI, I9); build-host toolchain documented separately.
- [ ] Step 5.5 **file-manifest + GN-args drift audit** produced a clean human-reviewed diff; `cef-native`
      copy-lists updated; all Output-file-checklist files present (`libcef.dll`, `icudtl.dat`,
      `v8_context_snapshot.bin`, `resources/`, `locales/`, …).
- [ ] Wrapper + `cef-native` rebuilt against new headers (no "Unsupported CEF version").
- [ ] Binaries staged to the **`cef-binaries` GitHub release**; Tier-2 `release.yml` consumes them.

**Auto-update apply (C3 — the highest reinstall-forcer class)**
- [ ] **Real installed N-1 → N silent update applies + relaunches cleanly on BOTH OS**, with the new CEF file
      manifest/framework layout and new minos, tied to the Step 5.5 drift audit. (No proxies — the actual
      updater, the actual new-CEF binary.)

**Codecs / media**
- [ ] `canPlayType` → `'probably'` for H.264 (`avc1.42E01E`), H.264 High, AAC (`mp4a.40.2`), MP3, VP9;
      **AV1 decode presence asserted.** HEVC/Dolby explicitly out-of-scope (M3).
- [ ] Real playback smoke: x.com (video + animated GIF), Reddit, Twitch, YouTube, an audio site.

**Farbling (B1 acceptance — concrete criteria, per I11)**
- [ ] **worker column == window column** for canvas/WebGL/audio, **including service-worker, shared-worker,
      and OffscreenCanvas-in-worker** cases (not just CreepJS's dedicated-worker column — I2).
- [ ] **Intra-session consistency:** same canvas/WebGL/audio read twice in one session+domain → **identical**
      perturbation (load-bearing for site correctness).
- [ ] **Cross-profile difference:** same site in two profiles → different farbled values.
- [ ] **Cross-site iframe:** a third-party origin embedded in two different first parties → **different**
      farbled values (first-party/top-frame keying works — I4).
- [ ] **Cross-session login test (load-bearing):** create account → restart → revisit → logins do NOT break
      (persistent per-profile seed working).
- [ ] Navigator values within the standard valid set; WebGL vendor/renderer decision (drop or common-string
      map incl. Mac GPU entries) applied per the resolved §3c conflict.
- [ ] OAuth/auth-domain exemption (C7 = `IsAuthDomain` re-impl) verified: pre-approved sites un-farbled and
      logging in (Q3).
- [ ] **No persistent seed on any renderer command line (C2 threat model):** verified via
      ProcessExplorer/`ps` that no stable per-profile secret is exposed on a child cmdline.
- [ ] **Stability soak + crash-rate gate** on the fresh Chromium bump + Blink patches (no elevated renderer
      crashes vs the 136 baseline).
- [ ] **Canvas/WebGL performance-regression gate:** readback perturbation on `readPixels`/`getImageData`
      within an accepted budget vs baseline.

**DRM**
- [ ] Component-updater Widevine CDM auto-download tested; **VMP-`.sig`-required-for-L3 question answered
      (I6)**; Amazon result documented (plays free at L3 → in; SD-capped/refused/needs-VMP → deferred, with
      cost + broken-site list recorded).

**Regression / parity**
- [ ] Standard site basket (CLAUDE.md Testing Standards): Auth, Video/Media, News, E-commerce, Productivity,
      BSV — on **both Windows and macOS**.
- [ ] Adblock still works incl. YouTube CefResponseFilter ad-strip + cosmetic/scriptlet (Q2).
- [ ] **FedCM ("Sign in with Google" account chooser) works** — `CefPermissionHandler` FedCM coverage audited
      (§3g); any new permission-API methods on the target version handled.
- [ ] **macOS minos guard GREEN**: every exe/helper/Rust-bin `minos ≥` framework minos; manual
      relaunch-after-update on a machine at/near the floor.
- [ ] Wallet send/receive + CWI shim intact (BRC-121 test site) — the build didn't disturb the money path.

---

## 8. Open questions / assumptions

| # | Assumption / default chosen | Alternative | Where it bites |
|---|---|---|---|
| 1 | **Target = current CEF stable** (revised — C1). LTS only if primary sources confirm the program (not Chromium Extended-Stable), coverage, freshness, and features. | Newest CEF LTS branch, IF confirmed. In-repo docs (`SPRINT_0_4_0_MASTER_PLAN.md` Q16) still say "no LTS, target M149" — **reconcile at plan time via Step 0.** | §2 — changes the branch number, not the phase order. |
| 2 | **WebGL vendor/renderer + navigator values follow Brave-parity** (owner Q18) within valid sets / common-GPU-string map (incl. Mac entries). | Keep the JS impl's original drop of WebGL vendor/renderer (more conservative). | §3c — load-bearing farbling value decision; still OPEN. |
| 3 | **Persistent per-profile seed delivered off-cmdline** (ephemeral nonce or mojo/pref — C2). | Cmdline switch — **rejected**: leaks a stable machine-local identifier to every local process. | §3c C2; threat model documented. |
| 4 | **First-party/top-frame seed keying** (anti-cross-site-tracking — I4). | Frame-own-origin keying — rejected (stable value to cross-site trackers). | §3c C2; cross-site iframe test. |
| 5 | **Basic Widevine CDM path tested; may need VMP `.sig` even for L3 on Windows; Amazon may be SD-capped/refused** (I6). VMP deferred. | Amazon needs L1/VMP → real $ → stays OUT of beta.1. | §3d / Q4. |
| 6 | **Persistent per-profile seed** (login fix) — accepts loss of cross-*session* unlinkability. | Brave's per-session reset (better anti-tracking, breaks logins) — rejected for a wallet browser. | §3c C2; owner signed off Q8. |
| 7 | **One shared cross-platform patch set, built per-OS as a first-class parallel effort; Mac owns arch (universal2 default) + minos/plist** (I8). | Per-OS divergent patches only if a Blink file is platform-gated (e.g. GPU strings). | §5 / Q1. |
| 8 | **Self-hosted VM / beefy machine + local sccache**; distributed build deferred; cold builds get no sccache benefit; **~10–12 hr per OS.** | Siso + third-party REAPI — deferred beyond 0.4.0. | §1 non-goals; §4. |
| 9 | **P1 baseline before bumping — GUARDED** (downgrade to last-known-good smoke if M136 is bit-rotted — I5). | Bump directly (faster, conflates version+env breakage). | §4 P0/P1. |
| 10 | **P2 conflates version + toolchain** — isolation from P1 is only **partial** (I5). | (No clean alternative — stated honestly.) | §4 P1/P2. |
| 11 | **Real N-1→N auto-update apply is a hard gate on both OS** (C3). | Ship on proxies — rejected (known reinstall-forcer). | §7. |
| 12 | **Farbling ships behind an optional `condition` build gate** so it can be toggled if it destabilizes beta.1. | Always-on (simpler; less escape hatch). | §3b/§3c. |
| 13 | **Fallback if TARGET destabilizes at gate time** (M5): documented rollback to the 136 (or previous) branch, not just toggling farbling off. | No fallback — rejected. | §4 [GATE]. |
| 14 | **Coordination via a new `CHROMIUM_BUILD_RELAY.md`.** | Extend existing `MAC_WINDOWS_RELAY.md`. | §5. |

**Standing risks to carry into the detailed plans:**
- Cold-build time **~10–12 hr per OS**, no caching help on cold builds.
- **CEF fork maintenance is a new recurring obligation**, including **pulling upstream in-branch security
  point-releases (M6)** — otherwise the security-coverage benefit erodes between jumps.
- **Per-bump patch-rebase labor on high-churn Blink files (I10)** is *the* recurring cost and the primary
  stable-vs-LTS lever — must be estimated in hours, not hand-waved.
- The **§3c value-table conflict** (+ M2 extra vectors) is the highest-risk unresolved *design* point.
- **Amazon DRM outcome is unknown** until the component-updater + VMP-`.sig` test runs (I6).
- **M136-from-source may be bit-rotted (I5)** — P0 must confirm before P1 is treated as a gate.
- **MPL-2.0 clean-room boundary (M7)** must be genuine — behavior/spec, not source-transcription.
- Corrected Chromium cadence (**4-week, not 2-week — I13**) makes the LTS-vs-stable call real but less urgent
  than the draft implied.

---

*Next: this revised outline → Workflow-2 expands each §3/§6 area into its own detailed plan (resolving every
still-OPEN item: §3c value table + M2 vectors, C2 delivery-channel choice, OOP-worker plumbing, Mac arch,
Amazon/VMP result) and synthesizes `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`.*