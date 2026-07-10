# Q5 — Full Reconciled Source-Edit List (Chromium/CEF Rebuild → v0.4.0-beta.1)

**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**Status:** DETAILED PLAN — Workflow-2 **synthesis of the completed plan set.** Hardened against **all ten** area docs now authored: `PLAN_version_bump.md`, `PLAN_patch_toolchain.md`, `PLAN_farbling_blink.md`, `PLAN_codecs.md`, `PLAN_dependencies.md`, `PLAN_build_test_prod.md`, `Q1_mac_farbling.md`, `Q2_farbling_adblock.md`, `Q3_farbling_oauth.md`, `Q4_widevine_amazon_drm.md`. Research + design only — **NO code, NO builds.**
**What this is:** the single authoritative inventory of **every Chromium/CEF source edit + build-config change** the rebuild carries. It is the master row-set that `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` sequences and the §7 readiness checklist gates against. Each edit → {edit · why · layer · platform · status · depends-on · detailing doc}.

> **⭐ What changed in this revision (2026-07-10, all plan docs now exist).** The prior Q5 was written when `PLAN_farbling_blink.md` and `Q3_farbling_oauth.md` were *unwritten*, so the C1–C7 rows and the value table were carried OPEN. **Both are now authored, plus the four build-pipeline PLAN docs.** This revision:
> 1. **Resolves `TARGET`** from `PLAN_version_bump.md`: **CEF 150 / Chromium 150 / branch `7871`** (the future M150-LTS line, adopted at ≥ CEF-Stable per the §3 channel gate; **fallback = current CEF-Stable M149 / branch `7827`** if `7871` is still Beta on build day). macOS floor rises **11.0 Big Sur → 12.0 Monterey**.
> 2. **Fills C1–C7 + P4e** with the concrete Blink files, the `HodosSessionCache` Supplement design, and the **browser-side-HMAC / off-cmdline seed** model (supersedes the old "renderer derives the HMAC" line).
> 3. **Resolves the WebGL/navigator value table** (§B) from `PLAN_farbling_blink.md` §7 — no longer "OPEN pending unwritten docs." Only WebGL vendor/renderer stays **owner-sign-off-pending** (recommended default: **drop**).
> 4. **Closes TD-5** — Q3's `ShouldFarble()` re-homes the per-site toggle as input B (owner sign-off; C7b fallback). Destination now exists.
> 5. **Adds the signer-continuity gate** (§A.9) and the **bot-signal re-home** (BOT-1), and expands **DEP-1** into the four silent-drift re-pins.

> **Authoritative inputs:** the ten area docs above; outline §3/§4/§5/§6/§7; `CEF_BUILD_RUNBOOK.md` (Step 5.5/6/7); `CEF_VERSION_UPDATE_TRACKER.md`; `DEPENDENCY_VERIFICATION.md`; `WINDOWS_AUTOUPDATE_PLAN.md`; `ORG_IDENTITY_SIGNING_MIGRATION.md`; `B1-farbling-design.md`; `scripts/build_hodos_cef.bat` / `_mac.sh`.

---

## TARGET (resolved — was a placeholder) — `PLAN_version_bump.md`

| Item | Value | Source / caveat |
|---|---|---|
| **Bump** | CEF 136 / branch `7103` → **CEF 150 / Chromium 150 / branch `7871`** | `PLAN_version_bump.md` §0/§1 |
| **Channel intent** | Ride `7871` into the **M150 LTS** line (LTS milestones = M138, M144, M150, M156) | LTS program confirmed from CEF `branches_and_building.html` + issues #3947/#4114 |
| **⛔ Build-day gate** | Build **only if `7871` has reached ≥ CEF-Stable** (Stable/LTC/LTS) — **NOT Beta.** On the verification date `7871` was **CEF Beta** | `PLAN_version_bump.md` §3 step 2 (build-blocking) |
| **Fallback** | If `7871` is still Beta on build day → pin **current CEF-Stable M149 / branch `7827`** | §2 / OQ-7 |
| **macOS floor** | **11.0 → 12.0 Monterey** (M150 is the last Chrome to support Monterey; M151 needs Ventura) — a reinstall-adjacent published-min raise; announce in release notes | §4.4 / §5 |
| **Toolchain** | MSVC v143/VS2022 family expected; **confirm exact Windows SDK `7871` needs** (may exceed `windows-2022`) | §4.3 / OQ-6 |
| **Signer continuity** | Win Authenticode CN = `Marston Enterprises` unchanged; mac Team ID unchanged (org migration pending) | §8 (see §A.9 below) |

Every "TARGET" below now means **branch `7871` (fallback `7827`)**. Re-confirm the numbers at execution per `PLAN_version_bump.md` §3.

---

## Legend

**Layer** — where the edit lives:
- **GN** = build-config flag (`GN_DEFINES` / derived arg), no source patch.
- **CEF** = CEF fork / patch-toolchain artifact (`patch/patches/*.patch`, `patch.cfg`, `automate-git --url`).
- **Blink** = a `.patch` file against `third_party/blink/renderer/...` (or `platform/graphics`).
- **Shell** = our own `cef-native/` C++ (not a Chromium-tree edit, but a prerequisite/teardown listed where it gates a source edit).
- **Process** = build-host / CI / version / minos / packaging / signing mechanics (build-breaker or reinstall-forcer if wrong).

**Status:** **EXISTS** (carry-forward, re-verify on TARGET) · **GREENFIELD** (no equivalent in-repo) · **NEW** (net-new this sprint, incl. retiring deletions) · **TEST** (verification/spike, may DEFER) · **DEFER** (explicitly OUT of beta.1, recorded so it isn't silently dropped).

**Platform:** Win / Mac / both. Patch *text* is shared/cross-platform; the *build* is a first-class parallel per-OS effort (outline §5, I8; `Q1_mac_farbling.md`).

---

## A. Master edit table

### A.1 — Proprietary codecs (GN) — `PLAN_codecs.md`

| ID | Edit | Why | Layer | Platform | Status | Depends-on | Doc |
|---|---|---|---|---|---|---|---|
| **GN-1** | `proprietary_codecs=true` in `GN_DEFINES` (both build scripts) | Master switch for H.264/AAC/MP3 + gates HEVC derivations; the reason we self-build | GN | both | EXISTS | — (baseline P1) | codecs §2 |
| **GN-2** | `ffmpeg_branding=Chrome` | Selects the FFmpeg config that ships the decoders; `Chromium` ships none (coupled to GN-1 — mismatch = loud build-time assert) | GN | both | EXISTS | GN-1 | codecs §2 |
| **GN-3** | `is_official_build=true` | Optimized/branded build; does NOT itself enable codecs or AV1 — keep flags explicit | GN | both | EXISTS | — | codecs §2/§4 |
| **GN-4** | `chrome_pgo_phase=0` | Disable PGO for sccache determinism (drops `/Brepro`); codec-neutral | GN | both | EXISTS | — | codecs §2 |
| **GN-5** | **HEVC: leave `enable_platform_hevc` / `enable_hevc_parser_and_hw_decoder` at derived-from-`proprietary_codecs` default (ON, hardware-only). Do NOT force off.** Already inherited on M136; carries forward | Removing it needs an *extra* override + patch for no benefit; hardware-decoder-only = no size/licensing surface | GN (derived) | both | EXISTS (smoke-only, non-gating) | GN-1 | codecs §3.1 / CQ-1 |
| **GN-6** | **AV1** via `enable_dav1d_decoder` (default true, all builds) — **assert decode presence**, add no flag | Free codec already present on M136; guards a surprise regression | GN (default) | both | EXISTS → TEST | — | codecs §3 / CQ-2 |
| **GN-7** | **Dolby (AC-3/EAC-3/AC-4, Dolby Vision) — explicitly NOT enabled** | Licensing-gated, separate flags, no demand | GN | both | DEFER (OUT) | — | codecs §3 / CQ-4 |
| **GN-8** | `enable_widevine=true` — auto-set by CEF build system; **confirm it resolves in generated args** (no manual flag) | DRM plumbing present; boundary to Q4 | GN (auto) | both | EXISTS → TEST | — | codecs §5 / Q4 §7 |

> **Doc-reconcile obligation (GN-5):** update outline **§3a M3** + the outline **§7 codec-checklist row** (currently "HEVC/Dolby explicitly out-of-scope") to read "HEVC = inherited hardware-only, non-gating" (codecs CQ-1). Verify all four flags via `gn args --list` **before** the 10–12 hr build (a flipped default ships a green build with no codecs — codecs §7 step 1).

### A.2 — CEF patch toolchain standup (GREENFIELD) — `PLAN_patch_toolchain.md`

| ID | Edit | Why | Layer | Platform | Status | Depends-on | Doc |
|---|---|---|---|---|---|---|---|
| **CEF-1** | Fork `chromiumembedded/cef` → **`Hodos-Browser/cef`**, branch `hodos/<7871>`; add `patch/patches/hodos_*.patch` + register in `patch/patch.cfg`; point build at fork via `automate-git.py --url=https://github.com/Hodos-Browser/cef.git --branch=7871 --checkout=<pin>`; **prove a no-op probe patch applies pre-compile + builds**, then remove the probe. Applied via `git apply -p0 --ignore-whitespace` (exact-context, **fail-loud, no fuzz**) | **No patch infra exists today** (`cef/patch/**` empty — verified). The serial linchpin blocking ALL source-level farbling | CEF | both | GREENFIELD | P1 baseline | patch_toolchain §1/§4/§8 |
| **CEF-2** | Land `scripts/cef_patch_drift_audit.py` (Step-5.5 hook): per-patch `git apply --check` (read-only; **never** `patch_updater.py --reapply/--restore` — write-capable), scrape hunk-**offset** lines (soft warning), registry/orphan check, target-file-existence, **runtime file-manifest diff** (folds VER-5), GN-args diff. Exit 1 = build must not start; wired as pre-build gate | Detect patch rot / manifest drift **before** a 10–12 hr build; the manifest diff feeds the auto-update apply gate | CEF / Process | both | GREENFIELD | CEF-1 | patch_toolchain §7 |
| **CEF-3** | **Standing duty: pull upstream in-branch security point-releases into the fork** between milestone jumps — automate as a scheduled `gh`/Actions fork-watcher that opens a rebase PR when upstream `7871` advances | Otherwise the bump's "security coverage" benefit erodes (M6) | CEF | both | NEW (recurring) | CEF-1 | patch_toolchain §2.3/§7.4 |
| **CEF-4** | **Single `condition: HODOS_FARBLING` env gate** on the whole farbling patch set (all-or-nothing, never half-applied) — a rebuild with the var unset ships a farbling-free binary without touching `patch.cfg` | Escape hatch if farbling destabilizes beta.1 (outline §8 #12; farbling FB-5) | CEF | both | NEW | CEF-1 | patch_toolchain §5 |
| **CEF-5** | **Check `build_hodos_cef.bat` / `_mac.sh` into `scripts/`** (referenced as canonical but **absent** from the repo — OQ-1) + add `HODOS_PATCHES.md` fork ledger | The `--url`/`--checkout` wiring must be version-controlled; the ledger is the rebase engineer's institutional memory | Process | both | GREENFIELD | CEF-1 | patch_toolchain §0/§3.2/§6.2 |

**C1–C7 attachment map** (patch_toolchain §8.1) — each farbling row → one patch file → one `patch.cfg` entry, all `condition: HODOS_FARBLING`, all `path: src`, ordered C1 first:
`hodos_farble_session_cache.patch` (C1) · `hodos_farble_seed_wiring.patch` (C2) · `hodos_farble_canvas2d.patch` (C3) · `hodos_farble_webgl.patch` (C4) · `hodos_farble_webaudio.patch` (C5) · `hodos_farble_navigator.patch` (C6) · `hodos_farble_auth_exempt.patch` (C7). **C1 creates a new Blink source file → its patch also edits a Blink `BUILD.gn` (the one higher-churn rebase target).**

### A.3 — Blink farbling patch set C1–C7 + P4e (NEW — owner-committed Blink migration) — `PLAN_farbling_blink.md`, `Q1_mac_farbling.md`, `Q3_farbling_oauth.md`

One **shared cross-platform patch set**, compiled into each OS's binary; replaces today's detectable, worker-blind JS injection (`FingerprintScript.h` / `FingerprintProtection.h`). Land incrementally **P4a→P4e** (§C). Clean-room re-implementation of Brave's *technique* — read behavior/spec, not Brave's MPL-2.0 source (farbling §9).

| ID | Edit | Why | Layer | Platform | Status | Depends-on | Doc |
|---|---|---|---|---|---|---|---|
| **C1** | **`HodosSessionCache : Supplement<ExecutionContext>`** — new `hodos_session_cache.{h,cc}` under `core/execution_context/` + hook in `execution_context.{h,cc}` + Blink `BUILD.gn`. Holds `domain_key_` + `enabled_`; exposes `From(ctx)`, `MakePrng()`, `PerturbPixels()`, `FarbleAudioSample()`, `FarblingEnabledForThisContext()`. Farbles at **API-call time**, not context-creation. Attaches to `LocalDOMWindow` + **in-process** worker/worklet contexts | Native = undetectable (`.toString()`→`[native code]`), covers in-process workers (today's #1 leak: `OnContextCreated` never fires for workers) | Blink | both | NEW | CEF-1 | farbling §3/§6 |
| **C2** | **Persistent per-profile seed, OFF the renderer command line.** `profile_seed` (32B CSPRNG via **`BCryptGenRandom`** / `SecRandomCopyBytes`, NOT deprecated `CryptGenRandom`) stored in **C++ profile data** — a new `profileSeed` field in the existing `%APPDATA%/HodosBrowser/<profile>/fingerprint_settings.json` (NOT the wallet). **Browser process computes `domain_key = HMAC-SHA256(profile_seed, first-party eTLD+1)` and delivers ONLY `{domain_key, farble_enabled}` to the renderer** — the master seed **never leaves the browser** (supersedes B1-design's "renderer derives the HMAC"). **Channel = OPEN (FB-1):** default **(A) mojo / commit-params per-navigation** (farbling FB-1); alt **(B) ephemeral per-launch nonce on the child cmdline** (a throwaway, not the seed — Q1 leans this on first-paint timing-safety). First-party/top-frame keying (I4) | Login-stability across restarts (persistent) without leaking a stable machine-local secret to any local process (C2 threat model); first-party keying blocks cross-site tracking | Blink + Shell (`ProfileManager`/`SettingsManager`) | both | NEW (channel OPEN) | C1 | farbling §4; Q1 §2.2 |
| **C3** | **Canvas 2D** readback farbling via the shared bitmap path `platform/graphics/static_bitmap_image.cc` (so `getImageData` + `toDataURL`/`toBlob` funnel through one perturbation site) + hooks in `modules/canvas/canvas2d/base_rendering_context_2d.cc`, `canvas_rendering_context_2d.cc`; `measureText` = gate only. Keep the small-canvas (<65536px) LSB gate. **Does NOT cover WebGL `readPixels`** | JS canvas farbling detectable + worker-blind → login/anti-bot friction | Blink | both | NEW | C1 | farbling §6 C3 |
| **C4** | **WebGL** — **`readPixels` with its OWN patch point** (framebuffer readback does not route through C3's `StaticBitmapImage` — verify paths before sizing); `getParameter`/`getSupportedExtensions` only if §B chooses to farble `UNMASKED_VENDOR/RENDERER` (recommended **drop**). Files: `modules/webgl/webgl_rendering_context_base.cc`, `webgl2_rendering_context_base.cc`. **Mac verifies readPixels perturbs on ANGLE→Metal (T-M2)** | readPixels is a distinct leak path from canvas-2D; already shipped in JS, keep | Blink | both | NEW | C1 (verify vs C3 paths) | farbling §6 C4; Q1 §3.2 |
| **C5** | **WebAudio** — per-sample fudge (`*= 1.0 + (rng()-0.5)*4e-7`, BALANCED-equivalent) in `modules/webaudio/audio_buffer.cc` (`getChannelData`), `analyser_handler.cc`, `realtime_analyser.cc` (`getFloatFrequencyData`) | Audio fingerprint vector; already shipped, keep | Blink | both | NEW | C1 | farbling §6 C5 |
| **C6** | **Navigator** — `deviceMemory` + `hardwareConcurrency` (compile-time getter replacement, **constrained** — see §B), plugins array (keep realistic 5-PDF set). Files: `core/frame/navigator_device_memory.cc`, `core/execution_context/navigator_base.cc`, `modules/plugins/dom_plugin_array.cc` | Navigator entropy; an out-of-spec/absent value is itself a fingerprint → must constrain | Blink | both | NEW (values RESOLVED — §B) | C1 | farbling §6 C6/§7 |
| **C7** | **Auth-domain exemption re-implemented at the BROWSER process** (Q3 OQ1 — **supersedes outline C7 line 158's "list passed to renderer"**). One `ShouldFarble(top_frame) = GlobalEnabled && !IsAuthDomain(top_frame_HOST) && IsSiteEnabled(top_frame_eTLD+1)`; allowlist match on the **full committed top-frame host** (OQ3 — do NOT collapse to eTLD+1), registrable domain used only for the seed key. Delivers the single `farble_enabled` **bit alongside C2's `{domain_key}` payload** (no new IPC **iff** C2 = per-navigation channel — R2 fork). Renderer `HodosSessionCache` hard-bypasses to native when false (non-one-shot). Structurally fixes the Turnstile parent/iframe inconsistency | Pre-approved/OAuth sites must not be farbled → belt-and-suspenders on login-sensitive flows | Blink + Shell | both | NEW | C2, Q3 | Q3 §2 |
| **C7b** | *(fallback of C7)* **Re-home the user per-site toggle** (`IsSiteEnabled` input B) as a sibling edit landing in the same P4d step, **if** the owner wants C7 kept minimal to `IsAuthDomain` only. Default = fold into C7's `ShouldFarble` (owner sign-off) | The toggle is a shipped feature; folding it into C7 is a deliberate scope extension flagged for sign-off | Blink + Shell | both | NEW (owner-gated) | C7 | Q3 §3 / OQ5 |
| **P4e** | **OOP seed/exemption plumbing** — deliver the **top-frame-derived** `{domain_key, farble_enabled}` to out-of-process contexts: **shared workers, service workers** (key = registration-scope eTLD+1, FB-3), **and cross-site (OOP) iframes** at subframe navigation commit; audio/paint worklet + OffscreenCanvas-in-worker are in-process (free once C2 lands, but tested). Needs a **purpose-built worker-parity harness** (CreepJS only exercises the dedicated-worker column) | "Workers for free" was overstated (I2); OOP contexts run in separate processes with their own command lines/origins | Blink + Shell | both | NEW | C1, C2 | farbling §5/§11 |

**Cross-platform split (Q1):** the patch *set* is shared; **Mac owns** the build (framework not DLL), the **arm64/x64/universal2 arch decision** (default lean universal2 = **two per-arch builds + `lipo`**, owner sign-off), minos/plist wiring, the seed-delivery *verification* (argv-parse if nonce; ordering proof if mojo), and the **Mac GPU-string entries** for C4 *if* §B chooses the common-string map. The one legitimate per-OS split is the C4 GPU-string **data** (runtime map keyed off the real `UNMASKED_RENDERER`, **not** a `#if BUILDFLAG(IS_MAC)` constant — can't separate Apple-Silicon vs Intel in a universal2 binary).

### A.4 — JS-farbling teardown (M1 — retire, don't orphan) — `Q2_farbling_adblock.md` §3, `PLAN_farbling_blink.md` §6

Deletions in files the adblock engine also touches — "don't nick the neighbour" hygiene (Q2 TP-1/TP-2). **Atomic per-value rule (I-4):** decompose the monolithic `FINGERPRINT_PROTECTION_SCRIPT` per-API and **delete each JS fragment in the exact commit its native patch lands** (canvas in P4a, WebGL in P4b, audio in P4c) → no double-farbling window, **no runtime guard flag needed.**

| ID | Edit | Why | Layer | Platform | Status | Depends-on | Doc |
|---|---|---|---|---|---|---|---|
| **TD-1** | Delete the JS FP injection block **`simple_render_process_handler.cpp:581–627`** (working tree 2026-07-10; outline's `:586-632` is stale — reconcile at edit time): auth-domain skip (`:585`), `s_domainSeeds` lookup, `FINGERPRINT_PROTECTION_SCRIPT` patch+inject (`:617`). **Keep** the adjacent scriptlet block `:567–579` byte-identical; **keep** the `window.chrome` stub `:629–653` | JS injection detectable ≥6 ways + worker-blind; native path (C3–C6) supersedes it | Shell | both | NEW (deletion) | native path live (C3+) | Q2 TP-1; farbling §6 |
| **TD-2** | Retire `FingerprintProtection.h` / `FingerprintScript.h` singletons (JS-injection parts) | Dead once C1–C7 own farbling | Shell | both | NEW (deletion) | TD-1 | Q2 §3 / farbling §6 |
| **TD-3** | Remove FP-only caches/IPC: `s_domainSeeds`+`s_seedMutex` (:37–38), `s_fingerprintDisabledUrls`+`s_fpDisabledMutex` (:42–43); `fingerprint_seed`/`fingerprint_site_disabled` sends `simple_handler.cpp:7484–7521` + renderer handlers `:1198/:1213`. **Keep** `s_scriptCache`/`preload_cosmetic_script`/`inject_cosmetic_css`/`inject_cosmetic_script` | Retire the seed IPC chain the native off-cmdline C2 channel replaces | Shell | both | NEW (deletion) | **C2 channel implemented + verified delivering per-domain seeds (P4a smoke green)** — deleting on a mere design *choice* would strand the renderer with a constant/absent seed (detectable + login breakage) | Q2 TP-2; farbling §6 |
| **TD-4** | **Migrate `IsAuthDomain` allowlist into C7's `ShouldFarble`** (single source of truth); delete the C++/renderer auth gate ONLY once C7 owns it | Two sources of truth = drift | Shell → Blink | both | NEW | C7 | Q3 §2 / Q2 TP-2 |
| **TD-5** | **Re-home the per-site fingerprint on/off toggle** (`IsSiteEnabled`/`SetSiteEnabled` + `fingerprint_get/set_site_enabled` IPC `simple_handler.cpp:6191/6210`, `FingerprintProtection.h:123/135`) into **C7's `ShouldFarble` input B (or C7b)** — a **shipped Privacy-Shield user control**, NOT the auth allowlist. **Do NOT delete until `ShouldFarble` consumes it** (Q2 T8 gate) | Deleting drops a shipped feature with no replacement | Shell + Blink | both | NEW (destination NOW EXISTS — was blocked on the unwritten plan) | C7 / C7b | Q3 §3 (owns the re-home) |

### A.5 — Bot-signal shims re-home (NEW) — `PLAN_farbling_blink.md` FB-4

| ID | Edit | Why | Layer | Platform | Status | Depends-on | Doc |
|---|---|---|---|---|---|---|---|
| **BOT-1** | **Preserve `navigator.webdriver=false`** (currently living **inside** the deleted `FINGERPRINT_PROTECTION_SCRIPT`) as a tiny standalone native/JS shim, independent of farbling per-site enable; **keep** the `window.chrome` stub `:629–653` byte-identical | These are **bot signals, not farbling** — absence/`true` `webdriver` = bot tell. Must survive the TD-1 JS-block deletion | Shell | both | NEW | TD-1 | farbling §6 C6 / FB-4 |

### A.6 — Widevine / DRM (TEST + DEFER) — `Q4_widevine_amazon_drm.md`

| ID | Edit | Why | Layer | Platform | Status | Depends-on | Doc |
|---|---|---|---|---|---|---|---|
| **DRM-1** | **Spike-1: free component-updater CDM test** on TARGET. **Step 0** = audit our own build for CDM suppression (`--disable-component-update`, `*.googleapis.com`/`update.googleapis.com` blocklist) — could make the whole VMP thesis moot; confirm CDM downloads **and loads** (not just downloads, cf. #3820); classify EME-resolve vs license-refused; run **Amazon purchased/rental tier** + Netflix/Bitmovin/YouTube/Spotify matrix; **answer whether a VMP `.sig` is required even for L3 on Windows (I6)**; compare Brave (VMP-signed) on the same title | Amazon movie failed while YouTube/X/LinkedIn work → isolate missing-CDM vs unattested-L3 vs codec | GN (auto) / Test | both | TEST | P5 target build | Q4 §7 Spike-1 |
| **DRM-2** | **VMP signing of binaries** — `HodosBrowser.exe.sig` + `libcef.dll.sig` (mac VMP path TBD — ties into framework code-sign/notarize, not 1:1). Route A = Google MLA (free-ish, ~4+ mo opaque wait); Route B = castLabs **paid audit-gated** 3PL (castLabs *free* EVS is **Electron-only** → cannot sign our CEF). **Gated on Spike-2 step 0: confirm the target title plays acceptably on VMP-signed Brave first** | The gating piece for Amazon/Netflix premium; a CEF embedder cannot shortcut the license/cert | Process / Signing | both | **DEFER (OUT of beta.1)** | DRM-1 result + owner $ decision | Q4 §5/§7 Spike-2/§8 |
| **DRM-3** | *(optional)* Brave-style "install Widevine" consent prompt | CDM already auto-downloads → the prompt is cosmetic/consent-only, fixes nothing functional | Shell (overlay) | both | DEFER / optional | — | Q4 §4 / Q1 |

### A.7 — Dependency bumps (RIDES WITH THE VERSION JUMP) — `PLAN_dependencies.md`

| ID | Edit | Why | Layer | Platform | Status | Depends-on | Doc |
|---|---|---|---|---|---|---|---|
| **DEP-1** | Full `DEPENDENCY_VERIFICATION.md` pass for **Hodos's own** deps + **rebuild (not just re-declare)** the CEF wrapper and every vcpkg static dep (`nlohmann-json`, `sqlite3`, OpenSSL `/MT` static, quirc) on the **exact toolset the target `libcef` was built with**; audit Rust (`rust-wallet`/`adblock-engine` — hold the `adblock=0.10.3`/`rmp=0.8.14`/`actix-web=4.11.0` MSRV-ceiling pins) + frontend (browser-API-dependent JS vs the new Chromium). Record a per-bump "touched/deferred" table. **NEVER silently bump wallet crypto crates (Invariant #3)** | gclient resolves Chromium's internal deps; the risk is *our* deps staying ABI/toolchain-compatible with the new `libcef` | Process | both (vcpkg Win-heavy; libcurl/Keychain Mac) | RIDES WITH BUMP | after P2 fetch, before P6 | deps §3/§5 |
| **DEP-1a** | **Pin the vcpkg baseline** — add `cef-native/vcpkg.json` manifest with `builtin-baseline` + `overrides` pinning exact versions **incl. `port-version`** (`openssl 3.6.0#3`, `sqlite3 3.51.1`, `nlohmann-json 3.12.0#1`); switch CI to manifest mode | Today `vcpkg install` runs against the **runner image's** baseline → a runner refresh silently rolls deps forward (beta.16 drift class) | Process / CI | Win (+brew mirror on Mac) | NEW (close silent-drift hole) | before DEP-1 | deps §4-A |
| **DEP-1b** | **Pin Inno Setup** — `choco install innosetup --version=<6.7.x>` (7.0 beta already published, breaking `.iss`) | A `choco`-served Inno 7 major silently breaks the installer compile | Process / CI | Win | NEW | before DEP-1 | deps §4-B |
| **DEP-1c** | **Pin macOS Homebrew deps** — `Brewfile` / recorded formula versions for openssl/nlohmann-json/sqlite3 | `brew install` floats; a newer OpenSSL min-macOS could break launch on the floor OS | Process / CI | Mac | NEW | before DEP-1 | deps §4-C |
| **DEP-1d** | **Add `rust-toolchain.toml`** pinning the `rustc` channel at the workspace root(s); reconcile/remove the `dtolnay/rust-toolchain@stable` CI step | No toolchain file today → `rustc` floats with `@stable` and can drift past the held-back adblock/actix pins' MSRV window | Process / CI | both | NEW | before DEP-1 | deps §4-D |

### A.8 — Version-bump mechanics + toolchain / minos (PROCESS — build-breaker if wrong) — `PLAN_version_bump.md`

| ID | Edit | Why | Layer | Platform | Status | Depends-on | Doc |
|---|---|---|---|---|---|---|---|
| **VER-1** | Update `--branch=7103` → **`--branch=7871`** (fallback `7827`) in **both** build scripts | The bump itself | Process / GN | both | PROCESS | P0 pin + §3 channel gate | version §4.1 |
| **VER-2** | **Chromium/CEF build-host toolchain match** — provision the self-hosted host's MSVC/Clang + Windows SDK to the toolset `7871` is built with (the Chromium build can't run on GitHub runners: 6-hr cap, ~14 GB disk) | ABI/build correctness | Process | both | PROCESS | **P0 Step-0 version resolution** (predecessor is the version-target decision, not the VER-1 string) | version §4.3; build_test_prod §2.2 |
| **VER-3** | **CI app-build runner pin** — `runs-on:` in `release.yml`, **no `*-latest`** (`windows-2022` / `macos-15` or a deliberately-validated newer pin), so its MSVC/Clang **matches the CEF binary's toolset** (ABI-critical match = CEF-binary ↔ `cef-native`/wrapper). **Re-validate the pin ships the SDK `7871` needs (OQ-6)** | beta.16 windows-2025 drift + macos-latest→Tahoe minos disaster | Process / CI | both | PROCESS | VER-2 | version §4.3 |
| **VER-4** | **macOS minos alignment** — Chromium floor `7871` = **12.0 Monterey** (raise from 11.0); `vtool`-measure the framework `minos`; set published min = `max(12.0, measured)` in **all three** (`CMakeLists.txt` `CMAKE_OSX_DEPLOYMENT_TARGET`, `Info.plist` + `mac/helper-Info.plist.in` `LSMinimumSystemVersion`); apply via `-DCMAKE_OSX_DEPLOYMENT_TARGET=12.0` + `MACOSX_DEPLOYMENT_TARGET`; **CI minos guard** fails unless every exe/helper/Rust-bin `minos ≥` framework minos + manual relaunch-after-update on a real Mac at floor | A too-low published min = crash-on-old-macOS; a floor raise strands Big Sur users (honest gate, announce) | Process / Shell | Mac | PROCESS | VER-1 | version §4.4 |
| **VER-5** | **Runtime file-manifest drift audit (Step 5.5)** — diff `7871`'s CEF dist DLL/`.bin`/`.pak`/`resources`/`locales` vs hardcoded copy-lists in `cef-native/CMakeLists.txt` (Win) + the mac framework-embed list; diff pinned `GN_DEFINES` vs new defaults. **14 milestones of drift → expect ≥1 changed resource** | **A changed manifest is exactly what breaks a silent auto-update** → feeds the §A.9 apply gate | Process / Shell | both (per-OS lists) | PROCESS | P2 | version §4.6 (folded into CEF-2 tool) |
| **VER-6** | **Version single-sourcing (PIPE-VERSION)** — git tag = source of truth; `cargo-release` bumps+tags; CMake (`cmake-git-version-tracking`), Rust (`shadow-rs`), + a CI step inject the tag into the Inno `.iss` + the TS constant | One version, no hand-edits | Process | both | EXISTS + extend | VER-1 | outline §3f |

### A.9 — Release gates: auto-update apply + signer continuity (PROCESS — highest reinstall-forcer class) — `PLAN_build_test_prod.md` §7.7, `PLAN_version_bump.md` §8

| ID | Edit | Why | Layer | Platform | Status | Depends-on | Doc |
|---|---|---|---|---|---|---|---|
| **UPD-1** | **Real N-1→N auto-update apply** on **both OS** — no proxies; reuse the proven `SILENT_UPDATE_TEST_PLAN.md` Stage-1 rigs + Stage-2 (dev wallet) + Stage-3 (prod-signed, trivial-balance wallet) legs. N carries the new CEF manifest + new minos + new framework layout; apply must place them right and relaunch clean; broken-N rolls back wallet-intact | The changed CEF file manifest (VER-5) is precisely the failure a silent update hides | Process | both | PROCESS (hard gate) | P6, VER-5 | build_test_prod §7.7 |
| **UPD-2** | **Signer-continuity assertion folded into every apply leg.** **Windows:** Authenticode **Subject CN = `Marston Enterprises` unchanged** (compare CN, NOT the ~3-day-rotating Azure leaf thumbprint — beta.23 root cause). **macOS:** `codesign` **Team ID UNCHANGED** (confirm pre-build — org conversion *should* preserve it but Apple doesn't contractually guarantee) + rotate **either** Developer-ID cert **or** EdDSA key, never both. Org-migration gate on beta.1: **(A) migrate-first (recommended, conditional on Team-ID confirmation)** vs (B) defer individual-signed | A signing-identity change **forces a reinstall** — the apply test passes on bytes while prod reinstalls on the signer | Process / Signing | both | PROCESS (hard gate) | UPD-1 | version §8; build_test_prod §7.7 / OQ-1 |

### A.10 — FedCM permission-handler coverage (SHELL AUDIT — testable NOW) — outline §3g, `PLAN_build_test_prod.md` §4.1

| ID | Edit | Why | Layer | Platform | Status | Depends-on | Doc |
|---|---|---|---|---|---|---|---|
| **FEDCM-1** | Audit `CefPermissionHandler` FedCM coverage **now on M136** (FedCM on-by-default since ~M108); if the handler doesn't cover the FedCM permission type, "Sign in with Google" silently fails. If TARGET changed the permission API, scope the shell edit here. **Note (Q3 §2.6):** FedCM renders in browser-native UI → **no farblable JS surface**; do NOT add IdP origins to the C7 allowlist expecting an effect | Live account-chooser breakage class, independent of farbling | Shell | both | TEST → NEW if handler gap | verify pre-bump + re-verify on TARGET | outline §3g; build_test_prod §4.1 |

---

## B. Farbling value table — RESOLVED (was OPEN) — `PLAN_farbling_blink.md` §7 / FB-7/FB-8, `Q3`

These are the **value fills for rows C4 and C6** plus the M2 vector-scope decision. Now settled with recommended defaults (owner default 2026-06-17 Q18 = **Brave-*technique* parity** — adopt the valid-set-constraint / reduce-only-cores / per-eTLD+1-seed *approach*, **NOT** Brave's literal mobile-tuned value sets). Only WebGL vendor/renderer remains **owner-sign-off-pending.**

| Vector | Row | Decision (recommended default) | Constraint / value | Status |
|---|---|---|---|---|
| Canvas `getImageData`/`toDataURL`/`toBlob` | C3 | **Farble** | LSB, small-canvas gate kept | RESOLVED |
| WebGL `readPixels` | C4 | **Farble** (own patch point) | LSB, already shipped | RESOLVED |
| WebAudio | C5 | **Farble** | per-sample fudge `4e-7` | RESOLVED |
| `navigator.plugins` | C6 | **Keep native** realistic 5-PDF set | empty array = bot tell | RESOLVED |
| `navigator.webdriver` | BOT-1 | **Keep `false`** (bot signal, re-home) | must survive JS teardown | RESOLVED |
| `navigator.deviceMemory` | C6 | **Farble, desktop-plausible set** (or drop — defensible owner alt) | **∈ {4,8,16,32}** — NOT Brave's mobile `{0.25..8}`; never emit an implausible value. *(Note: `PLAN_build_test_prod.md` §7.2 still says `{2,4,8,16,32}` — reconcile to the value-owning farbling plan's `{4,8,16,32}`.)* | RESOLVED (FB-8) |
| `navigator.hardwareConcurrency` | C6 | **Reduce-only** — random plausible value **≤ real core count** (Brave's `[2, real]`), **NOT** a fixed set | a fixed set can *inflate* a 4-core box to 16 = cross-referenceable tell | RESOLVED (FB-7) |
| WebGL `UNMASKED_VENDOR/RENDERER` | C4 | **DROP (recommended)** unless a common-GPU-string map is built | random strings are *more* unique than truth. If mapped: small set of **real** GPU strings incl. **Apple-Silicon + Intel-Mac ANGLE** (I8/Q1), runtime-selected off the real reported renderer, never noise | **owner sign-off pending (FB-2) — highest-risk value decision** |
| UA-CH high-entropy (`getHighEntropyValues`) | — | **Omit, log accepted gap** | out of beta.1 scope (M2) | RESOLVED (gap) |
| `screen` / `devicePixelRatio` | — | **Omit, log accepted gap** | ~3–4 bits, high breakage | RESOLVED (gap) |
| `getClientRects` / font metrics beyond `measureText` | — | **Omit, log accepted gap** | not scoped for beta.1 | RESOLVED (gap) |
| `enumerateDevices` | — | **Omit, log accepted gap** | not scoped (M2) | RESOLVED (gap) |

> **The single remaining OPEN value decision is WebGL vendor/renderer (FB-2): drop vs common-string-map.** If **drop** → C4's `getParameter` hook is minimal and the Mac GPU-string entries are **not required** (must not block the farbling gate). If **map** → Mac Claude authors the Apple-Silicon + Intel ANGLE rows (Q1 §5). Owner sign-off required either way.

---

## C. Dependency / phase ordering (condensed from outline §4 + the PLAN docs)

```
P0 provision+pin (build host: 150GB+/32GB+/depot_tools/sccache; Step-0 version+toolset+minos+build-tool[Ninja/Siso] lookup; confirm M136 still fetches — else P1→last-known-good smoke)
   │
P1 baseline (GN-1..4 only, guarded)
   │
P2 bump (VER-1..6 + WRAPPER REBUILD; DEP-1a..d re-pins FIRST, then DEP-1; FEDCM-1 re-verify; GN-5..8 re-verify; CEF-2/VER-5 drift audit)
   │
P3 patch toolchain (CEF-1..5) ── serial linchpin, blocks all farbling
   │
   ├───────────────────────────────────────────────┐
P4 farbling (incremental):                          P5 codecs/DRM (GN-5..8 re-verify + DRM-1)  ∥ P4
   P4a  C1 Supplement + C2 seed → Canvas(C3)-first worker quick-win  (delete JS canvas frag)
   P4b  C4 WebGL incl. readPixels + resolve FB-2      (delete JS WebGL frag)
   P4c  C5 Audio + C6 Navigator(§B values) + BOT-1    (delete JS audio frag; re-home webdriver)
   P4d  C7 auth exemption (ShouldFarble) + TD-4 + TD-5/C7b toggle re-home  (JS block fully torn down; M1 complete)
   P4e  OOP workers (shared/service) + cross-site-iframe top-frame delivery
   │        TD-1..3 land as each native value goes live (atomic per-value, I-4)
   └───────────────────────────────────────────────┘
   │
P6 TEST (farbling acceptance + Q2 T1–T8 + Q3 T1–T10 + codec smoke + DRM + minos guard + FedCM + parity + UPD-1/UPD-2 real N-1→N apply w/ signer continuity — BOTH OS)
   │
P7 prod build → stage to cef-binaries release → Tier-2 release.yml consumes ─▶ [GATE] v0.4.0-beta.1 (§7 checklist)
```

**Serial linchpins:** CEF-1 blocks all of C1–C7/P4e; C1 blocks C2–C7 + P4e; **C2 gates C7** (the `farble_enabled` bit rides C2's payload — R2 fork if C2 picks a non-per-navigation channel) **and gates the TD-3 seed-IPC deletion**; **TD-5's destination is now C7/C7b** (was blocked on the unwritten plan). **DRM-2 deferred**, gated on DRM-1 + owner $. **P4 ∥ P5.** DEP-1a..d land as their own small commits **before** DEP-1. Cold build **~10–12 hr per OS** (universal2 Mac = two per-arch builds + `lipo`); do not run F4/parking_lot concurrently with the farbling set.

---

## D. "Nothing else" completeness check

Cross-checked against outline **§3 (a–g)** + **§6-Q5** + all ten now-authored area docs. Every planned Chromium/CEF edit + build-config change is accounted for:

| Outline / doc source | Captured as | Detailing doc (now exists) | ✓ |
|---|---|---|---|
| **§3a** codecs GN flags | GN-1..GN-4 | PLAN_codecs | ✅ |
| **§3a** M3 HEVC / AV1 / Dolby scope | GN-5 (HEVC inherited-on), GN-6 (AV1), GN-7 (Dolby) | PLAN_codecs §3 | ✅ |
| **§3b** CEF patch toolchain (PIPE-A1) + M6 security-pull + `condition` gate | CEF-1..CEF-5 | PLAN_patch_toolchain | ✅ |
| **§3c** C1–C7 Blink farbling (patch set) | C1–C7 (+C7b) | PLAN_farbling_blink; Q3 (C7) | ✅ |
| **§3c** OOP-worker plumbing (I2) | P4e | PLAN_farbling_blink §5 | ✅ |
| **§3c** C2 per-profile seed + **off-cmdline channel** + first-party keying | C2 (browser-side HMAC; channel = FB-1 OPEN) | PLAN_farbling_blink §4; Q1 §2.2 | ✅ |
| **§3c** C7 / **§6-Q3** auth-domain exemption (`IsAuthDomain` only; browser-side test) | C7 + TD-4 | Q3_farbling_oauth | ✅ |
| **§3c** M1 JS-farbling teardown (atomic per-value) | TD-1, TD-2, TD-3, TD-4 | Q2 §3; PLAN_farbling_blink §6 | ✅ |
| **§3c** bot-signal shims (`webdriver`/`window.chrome`) survive teardown | BOT-1 | PLAN_farbling_blink FB-4 | ✅ |
| **Q2 TP-2** shipped per-site toggle re-home | TD-5 (destination = C7/C7b — **now resolved**) | Q3 §3 | ✅ |
| **§3c §7.6 / M2** value tables (deviceMemory/concurrency/WebGL vendor + extra vectors) | §B (RESOLVED except FB-2 sign-off) | PLAN_farbling_blink §7 | ✅ |
| **§3d / Q4** Widevine component-updater test + VMP | DRM-1, DRM-2, DRM-3 | Q4_widevine_amazon_drm | ✅ |
| **§3e** (A3) dependency bumps + silent-drift re-pins | DEP-1, DEP-1a..DEP-1d | PLAN_dependencies | ✅ |
| **§3f** version-bump mechanics (branch, toolchain, minos, drift audit, version single-source) | VER-1..VER-6 | PLAN_version_bump; build_test_prod | ✅ |
| **§3f** auto-update apply + signer continuity (reinstall-forcer gate) | UPD-1, UPD-2 | build_test_prod §7.7; version §8 | ✅ |
| **§3g** FedCM permission-handler coverage | FEDCM-1 | outline §3g; build_test_prod §4.1 | ✅ |
| **§5 / Q1** Mac arch (arm64/x64/universal2) + Mac GPU-string data | C4 GPU-string split (Mac-owned) + §B note + A.3 cross-platform note | Q1_mac_farbling | ✅ |
| **§2 / TARGET resolution** (stable vs LTS, Extended-Stable conflation C1) | TARGET block (CEF 150/`7871`, fallback `7827`; ≥Stable channel gate) | PLAN_version_bump §1/§2/§3 | ✅ |
| Codecs↔DRM boundary (PLAN_codecs §5) | GN-8 ↔ DRM-1 separation asserted | PLAN_codecs §5 | ✅ |

**Nothing else outstanding as a *new edit category*.** All C1–C7 rows and the value table are now **filled** (the prior revision's OPEN farbling rows are closed). The only items not resolved to a final concrete decision are, by design, single value/spike choices — **not** missing edits:
1. **C2 seed delivery channel** — (A) mojo/commit-params per-navigation (farbling FB-1 default) vs (B) ephemeral-nonce cmdline (Q1 timing-lean). Both reach Mac; decision on delivery-timing merits.
2. **§B FB-2** — WebGL vendor/renderer **drop** (recommended) vs common-GPU-string map (Mac ANGLE entries). Owner sign-off.
3. **DRM-1 outcome** → whether **DRM-2 VMP** un-defers (post-beta.1 `VMP_SIGNING_SPIKE.md`).
4. **Build-day channel gate** — `7871` ≥ CEF-Stable, else fallback to `7827` (PLAN_version_bump §3).
5. **Org-signing gate on beta.1** — (A) migrate-first (Team-ID-confirmed) vs (B) defer (UPD-2 / build_test_prod OQ-1).

**Housekeeping (non-edit):** filename-convention drift persists — outline §6 registers `Q1/2/3/4-...` (hyphen-`x`); files are underscore (`Q1_mac_farbling.md`, `Q2_farbling_adblock.md`, `Q3_farbling_oauth.md`, `Q4_widevine_amazon_drm.md`). Resolve in a single rename pass across all Q-docs; **not** an edit to the Chromium/CEF tree.

---

*Feeds `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` (this is its edit-inventory backbone) and the outline §7 readiness checklist. All ten detailing docs now exist; this table is the reconciled join across them. Reconcile only on live execution (line-number drift, the FB-1/FB-2 decisions, the build-day channel gate).*
