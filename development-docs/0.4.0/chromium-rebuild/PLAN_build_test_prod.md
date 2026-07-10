# PLAN — Local-Build → Test → Prod-Build Pipeline (Chromium/CEF Rebuild → v0.4.0-beta.1)

**Status:** DETAILED PLAN (Workflow-2 expansion of `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §4 phase-order, §5 OS split, §7 readiness checklist). Research + design only — **NO code, NO builds.**
**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**Purpose (one line):** The followable end-to-end pipeline for the rebuild — provision the build host → baseline → prod-build the Tier-1 CEF binary → stage to the `cef-binaries` release → Tier-2 app consumes → the full test plan (codec/farbling/adblock/OAuth/minos + a **real N-1→N auto-update apply on both OS with signer-continuity verification**) → gate `v0.4.0-beta.1`.

> **Authoritative inputs:** `DevOps-CICD/CEF_BUILD_RUNBOOK.md` (Tier-1 full-build, Steps 0–8 + 5.5), `DevOps-CICD/BUILD_AND_RELEASE.md` (Tier-2 app release), `DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md` (toolchain + minos rules), `DevOps-CICD/DEPENDENCY_VERIFICATION.md`, `DevOps-CICD/SILENT_UPDATE_TEST_PLAN.md` (the living update-regression rig — Stage 1 rigs are the standing procedure), `DevOps-CICD/ORG_IDENTITY_SIGNING_MIGRATION.md` (the signer-continuity gate), `DevOps-CICD/WINDOWS_AUTOUPDATE_PLAN.md` + `AUTO_UPDATE.md`, and the sibling area docs `PLAN_codecs.md`, `Q2_farbling_adblock.md`, `Q4_widevine_amazon_drm.md`, `Q5_full_edit_list.md`. Build scripts: `scripts/build_hodos_cef.bat` / `scripts/build_hodos_cef_mac.sh`; CI: `.github/workflows/release.yml`.

> **TARGET = placeholder.** Exact CEF stable version + branch + milestone resolves from `cef-builds.spotifycdn.com/index.json` at plan-execution time (outline §2 Step 0 — the LTS-vs-stable / Extended-Stable-conflation question is settled in the *version-target* plan, **not here**). Runbook lines 80–85 anchor current stable at **CEF 149 / Chromium 149 (branch 7827)** with **M150-LTS** as the pin candidate. **⚠️ Caveat:** the runbook's surrounding LTS/cadence framing (lines 85–92: a "2-week stable cadence" + "every-6th-branch ~8-month LTS") is **contested** — the outline (C1/I13) debunks it as a garbled Extended-Stable conflation (Chromium is a 4-week cadence); treat those numbers as **not settled**, resolved only in the version-target plan. This doc is version-agnostic: the pipeline and the tests are identical whichever milestone Step 0 resolves.

---

## 1. What this plans (one screen)

- **The pipeline is TWO tiers** (existing model — `CEF_BUILD_RUNBOOK.md` + `BUILD_AND_RELEASE.md`), and the rebuild is a **Tier-1 event**:
  - **Tier-1 (this sprint's cold build):** produce a fresh custom Chromium+CEF binary distribution with our edits (codecs GN flags + the new Blink farbling patch set), on the self-hosted build host. Output → published to the **`cef-binaries` GitHub release**. Expensive (~10–12 hr/OS), infrequent.
  - **Tier-2 (the app release, unchanged mechanics):** `release.yml` pulls the `cef-binaries` artifact, rebuilds the CEF wrapper + `cef-native` shell + Rust + frontend, signs, packages (Inno/DMG), publishes the tagged app release + appcast. Fast, per-release.
- **The rebuild's new risk surface vs a routine app release:** (a) a *new* patch toolchain (greenfield), (b) *new* Blink patches that churn per bump, (c) a *changed CEF file manifest* that can silently break a **silent auto-update apply**, and (d) the **pending macOS signing-identity migration** which — if it lands mid-stream — is a **known reinstall-forcer**. Items (c) and (d) are why the test plan's centerpiece is a **real N-1→N update apply with signer-continuity verification on both OS**, not a proxy.
- **Headline recommendation:** run the pipeline in the outline's **P0→P7 phase order**, gate `v0.4.0-beta.1` on the §8 checklist, and treat the **`SILENT_UPDATE_TEST_PLAN.md` Stage-1 rigs + Stage-2/3 real-apply legs as the mandatory update-regression procedure for this build** (they already exist and are proven — reuse, don't reinvent). **Do NOT cut beta.1 until the macOS org-signing migration is either (i) complete so beta.1 is the first org-signed build, or (ii) explicitly deferred with beta.1 staying individual-signed AND a documented plan that the eventual org swap is itself gated by a real signer-continuity apply test.** See §7.

---

## 2. Build-host provisioning (P0) — the pipeline cannot start without this

The Chromium build **cannot run on GitHub-hosted runners** (6-hr job cap + far too little disk; `CEF_BUILD_RUNBOOK.md` A1 notes; confirmed — GitHub-hosted runners ship **~72 GB total, ~50 GB pre-consumed by tooling → ~14–29 GB free**, far short of Chromium's 100 GB+ tree). Tier-1 needs a **self-hosted VM or beefy machine**. This is a real phase, not a prerequisite footnote.

### 2.1 Host spec (per OS — Windows and macOS are separate hosts)

| Resource | Minimum | Recommended | Source |
|---|---|---|---|
| Disk (free) | 100 GB | **150+ GB NVMe/SSD** (Chromium tree is millions of small files; `gclient sync` is I/O-bound) | runbook line 57; Chromium build docs |
| RAM | 16 GB | **32+ GB** (linker peaks; OOM = failed link late in a 10-hr build) | runbook line 57 |
| CPU | 4 cores | **8+ cores** (compile is embarrassingly parallel) | runbook line 57 |
| Filesystem | — | **NTFS** (Win) / APFS (Mac) — **never exFAT** (Chromium needs symlinks + case sensitivity) | runbook Lessons |
| Path | — | Win: **short ASCII base `C:\cef\`** (260-char limit) + optionally enable `LongPathsEnabled` | runbook Step 3 |

> **⚠️ Two-tree footprint:** P1 keeps the **M136 baseline tree** and P2 builds the **TARGET tree** for isolation. If both coexist on one host that's **~2× the Chromium checkout** (200 GB+). Either **build sequentially and delete the M136 tree before P2**, or **size the host for both** — the 150 GB recommendation above assumes a single tree at a time.

> **⚠️ Cloud-spot acceptability is CONDITIONAL on the TARGET builder's resume semantics — verify before relying on it.** The "build is resumable" evidence is from the 2026-03-12 **M136** build, which used **Ninja** (`.ninja_log` skips completed objects on re-run — that build resumed after a Windows auto-restart with only ~17K of ~96K objects left). But Chromium **deprecated Ninja for external developers and switched to Siso as the default builder** (Siso is already default; see §2.4/M2). The **TARGET (M149/branch 7827) build almost certainly builds under Siso**, whose incremental/resume mechanism is **not** the `.ninja_log` path cited. **Step-0 lookup (add to P0):** determine the TARGET branch's **default build tool**, and *either* (a) verify Siso's cold-build resume + cache behavior on that branch before accepting a reclaimable spot VM, *or* (b) explicitly set `use_siso=false` in `args.gn` **and confirm Ninja is still supported on the TARGET branch** (Ninja is being removed upstream — this may not hold). **Until resumability is verified for the actual TARGET builder, default to a persistent/owned host** (see OQ-2). Keep the disk on a persistent volume regardless so a reclaim doesn't wipe progress.

### 2.2 Toolchain + tooling to install (Windows host)

Per `CEF_BUILD_RUNBOOK.md` Step 3 one-time setup:
1. **Visual Studio 2022 BuildTools** (MSVC **v143**) — workloads *Desktop dev with C++* + *Game dev with C++* (extra SDKs); components: latest Win 10/11 SDK, C++ CMake tools, C++ Clang.
2. **Windows SDK → Debugging Tools for Windows** (NOT installed by default — the build fails without it).
3. **Python matched to the TARGET branch's `.vpython3` / depot_tools bundled interpreter.** depot_tools ships its own interpreter (`$depot_tools/python-bin`) and requires Python **3.8+**; the usable *ceiling* is set by the TARGET branch's `.vpython3`, **not** a universal "3.12 breaks." The M136-era 3.11 ceiling is **not** a carry-forward constant — **re-confirm it for TARGET** as a Step-0 lookup (adjacent to #4) rather than provisioning the host to a stale ceiling.
4. **`depot_tools`** (Google's build tooling) + **`automate-git.py`** (CEF's build automation) — fetched per runbook Step 3 into `C:\cef\depot_tools` / `C:\cef\automate`.
5. **Windows Defender exclusions** for `C:\cef\` + `C:\cef\depot_tools\` (Defender on millions of small files = 2–5× slower).
6. **Pause Windows Update / disable auto-restart** for the build window (an overnight compile *will* be killed by a forced restart — the #1 cause of a lost build).

> **⚠️ Toolset is an ABI contract (`CEF_VERSION_UPDATE_TRACKER.md` "Toolchain").** Note the **exact MSVC/Clang toolset the TARGET CEF is built with** (Step 0 lookup #4) and provision the host to match. This is the *build-host* toolchain; it is **distinct** from the *CI app-build runner* toolchain (§6.3) — the ABI-critical match is between the **CEF binary's toolset and the toolset building the wrapper + `cef-native`**, not the Chromium-build runner's toolset (outline §3f I9).

### 2.3 macOS host

Xcode + Command Line Tools; same ~100 GB / 16 GB (32 rec.) spec; arch flag `--arm64-build` (Apple Silicon) / `--x64-build` (Intel). Output is `Chromium Embedded Framework.framework` (not `libcef.dll`) — **a fully separate first-class build, not an inherit** (outline §5 I8). The **arm64/x64/universal2 arch decision (outline §5 I8, default lean universal2)** must be settled with owner sign-off *before* P2 on Mac because it changes build time/disk **and** the C4 "common GPU strings" set (Apple Silicon vs Intel ANGLE).

### 2.4 sccache (build-caching — provision now, benefits incrementals only)

- Set `cc_wrapper="sccache"` in GN; with `chrome_pgo_phase=0` (already our flag) the toolchain **auto-drops** the MSVC `/Brepro` + `/showIncludes:user` flags that otherwise block caching (runbook A1). sccache supports MSVC + an **S3-backed shared cache** to share across machines/CI.
- **⚠️ Honest expectation (runbook A1 CAVEAT + primary source):** the oft-cited "~3× speedup" is a **WARM / incremental** figure. **A cold, from-scratch build gets NO benefit** — budget the full ~10–12 hr per OS regardless. Additionally, **sccache historically yields few cache-hits on MSVC/Windows** (mozilla/sccache + chromium issue 40188007) — treat Windows sccache as a best-effort incremental accelerator, not a cold-build lever. Provision it, warm it between the baseline (P1) and bump (P2) builds, but do not schedule around a speedup that won't materialize on the first build.
- **Distributed build (Siso + third-party REAPI) is OUT of this sprint** (reclient deprecation / the Ninja→Siso migration landed around **end of Sept 2025** — **Siso is already Chromium's default builder**, ties to I1; Google's hosted RBE is off-limits to non-Googlers — runbook A1). Accept self-hosted + local/S3 sccache for beta.1.

### 2.5 P0 exit criteria
- [ ] Both hosts provisioned to spec, toolset noted + matched to TARGET CEF (Step 0 #4).
- [ ] **TARGET default build tool determined (Ninja vs Siso)** and its cold-build resume/cache behavior verified — or `use_siso=false` set + Ninja confirmed still supported on TARGET (§2.1 ⚠️ / I1). Gates the OQ-2 spot-vs-persistent decision.
- [ ] `depot_tools` + `automate-git.py` present; `gclient --version` sane.
- [ ] sccache installed + a cache backend chosen (local disk for beta.1; S3 optional).
- [ ] **M136 still fetches/builds confirmation (OQ-7 / outline §4 P1 caveat):** a `gclient sync` of the current pinned M136 branch succeeds on the pinned toolchain. **If M136 is bit-rotted** (Google deprecates old sysroots/CIPD/toolchain packages over time), **downgrade P1 from a full cold baseline build to a smoke of the last-known-good environment** rather than treating an un-meetable build as a gate.

---

## 3. Baseline build (P1) — prove the unchanged pipeline before changing anything

**Goal:** a from-source build on the **current config (M136), codecs only, no new patches**, to isolate *most* subsequent breakage to the version jump (P2) rather than to our environment.

- Run `scripts/build_hodos_cef.bat` (Win) / `_mac.sh` (Mac) **verbatim on the current `--branch=7103`**, `GN_DEFINES=is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0`.
- Verify output per the runbook **Output file checklist** (`libcef.dll`, `chrome_elf.dll`, `icudtl.dat`, `v8_context_snapshot.bin`, `resources/`, `locales/`, …).
- Run the **codec Layer-A probe** (`PLAN_codecs.md` §6.1) on the baseline binary — this also lets us record HEVC's inherited-on behavior on M136 *before* the bump (`PLAN_codecs.md` §3.1).

**Honesty caveat (outline §4 I5):** P2 changes version **and** toolchain together, so P1→P2 isolation is only **partial** — state this plainly in the build changelog. P1's value is proving *our glue* (scripts, staging, wrapper rebuild) works before we also move the engine.

**P1 exit:** a green baseline build + codec Layer-A pass, OR (if M136 is bit-rotted per §2.5) a documented last-known-good smoke standing in for the full baseline.

---

## 4. Tier-1 cold build with our edits (P2–P5) — the rebuild proper

This section is the **execution wrapper** around the phase order; the *content* of each edit lives in the area docs (`Q5_full_edit_list.md` is the master inventory).

### 4.1 P2 — version bump + toolchain/minos/dep alignment
1. Update `--branch=` in **both** build scripts to TARGET (Step 0 #1).
2. **Deps pass** — full `DEPENDENCY_VERIFICATION.md` for **our** deps (vcpkg static: nlohmann-json, sqlite3, OpenSSL, quirc; CEF wrapper; frontend React/Vite/TS; Rust `rust-wallet`/`adblock-engine`). Record a per-bump "touched/deferred" table. (gclient resolves Chromium's *internal* deps automatically.)
3. **Toolchain align** — provision build-host to TARGET's toolset (§2.2); pin the CI app-build runner separately (§6.3).
4. **macOS minos** (Mac owns entirely — `CEF_VERSION_UPDATE_TRACKER.md`): look up TARGET Chromium's oldest macOS; **`vtool`-measure** the built framework's real `minos`; set published min = `max(Chromium floor, measured minos)` in **all three** places — `cef-native/CMakeLists.txt` `CMAKE_OSX_DEPLOYMENT_TARGET`, `cef-native/Info.plist` + `cef-native/mac/helper-Info.plist.in` `LSMinimumSystemVersion`; apply via `-DCMAKE_OSX_DEPLOYMENT_TARGET=` on the configure line + `MACOSX_DEPLOYMENT_TARGET` at job level.
5. **FedCM shell audit** (outline §3g — testable NOW): confirm `CefPermissionHandler` covers the FedCM permission type so "Sign in with Google" account chooser doesn't silently fail; re-verify on TARGET if the permission API changed.
6. **Step 5.5 drift audit** (see §5 below) + **codec re-verify** (`PLAN_codecs.md` §7 steps 1–3).

### 4.2 P3 — stand up the CEF patch toolchain (GREENFIELD)
Fork `chromiumembedded/cef`; create `patch/patches/*.patch`; register in `patch/patch.cfg`; point the build at the fork via `automate-git.py --url=<fork>`; **prove a no-op patch applies + builds** before any real Blink patch. This is the serial linchpin blocking all farbling (`Q5_full_edit_list.md` CEF-1). Standing duty: pull upstream in-branch security point-releases into the fork between milestone jumps (M6).

### 4.3 P4 — land the Blink farbling patch set (incremental)
Per outline §4: P4a Supplement + seed channel + **Canvas-first worker quick-win** → P4b Canvas+WebGL(incl. `readPixels`) → P4c Audio+Navigator(valid-set constrained) → P4d auth-domain exemption at source (`IsAuthDomain` only) → P4e OOP-worker seed plumbing. **Retire the old JS-farbling path** (M1 teardown checklist: delete injection at `simple_render_process_handler.cpp:586-632` — **⚠️ inherited line ref, re-confirm at P4-execution time per the project kickoff rule; code line refs drift** — retire `FingerprintProtection.h`/`FingerprintScript.h`, remove the seed IPC chain). Value tables (§B-VALUES in `Q5_full_edit_list.md`) stay OPEN pending `PLAN_farbling_blink.md`.

### 4.4 P5 — codecs + DRM re-verify (verification on the P4 output binary — NOT a second concurrent build)
> **Clarification:** codecs are always-on **GN flags compiled into the SAME tree/build** as the farbling patches — there is no separate codec build. P5 = **verify codecs + test DRM on the binary P4 produced**. "P4 ∥ P5" means the verification work can overlap the farbling landing; it does **not** mean scheduling a second concurrent 10–12 hr cold build on the one host.

Codecs: `PLAN_codecs.md` §7 procedure on TARGET. DRM: `Q4_widevine_amazon_drm.md` — component-updater CDM → Amazon test, incl. the **VMP-`.sig`-required-for-L3-on-Windows** check; default OUT of beta.1 unless free.

### 4.5 Build changelog (P2–P5, append to `CEF_VERSION_UPDATE_TRACKER.md`)
CEF branch, Chromium milestone, GN_DEFINES, patch-set version, deps touched/deferred, verification results, build duration, and **estimated per-bump patch-rebase hours** (outline I10 — the recurring cost + the primary stable-vs-LTS lever).

---

## 5. The drift audit (Step 5.5) — the bridge from Tier-1 to a safe auto-update

**This is the single most load-bearing step connecting the build to auto-update safety.** A green compile does NOT prove correctness after a bump: compile-time CEF API changes fail loudly, but **silent config/file-manifest drift survives a green build** — and a changed CEF file manifest **is exactly what breaks a silent update apply** (outline §3f; runbook Step 5.5).

On every bump, diff and human-review:
- **Runtime file manifest** — new CEF dist's DLL/`.bin`/`.pak`/`resources`/`locales` list vs the hardcoded copy-lists in `cef-native/CMakeLists.txt` (Win) + the macOS framework-embed list (Mac). A new/renamed/removed file we don't copy = green build → runtime crash or missing feature → **and a broken update apply** (the updater stages a tree that's missing/extra files vs what the shell expects).
- **`args.gn` (resolved, not script input)** — confirm `ffmpeg_branding=Chrome`, `proprietary_codecs=true`, `enable_widevine` still resolve (a flipped default ships green-with-no-codecs).
- **cmake/toolchain** — CEF version macro, wrapper rebuild ("Unsupported CEF version" ⇒ delete `CMakeCache`, rebuild), vcpkg ABI.
- **Patches** — re-apply `cef/patch/`, report fuzz (the P3 toolchain owns this).
- **macOS parity** — `Info.plist` framework version, helper embedding, entitlements, minos.

**Output:** a human-review diff report (manifest + args diffs are scriptable; cmake needs judgment — never auto-apply). **Feed the file-manifest diff directly into the §7 auto-update apply test** — it tells the tester exactly which new/changed files the N-1→N apply must correctly place.

---

## 6. Prod build + staging (P7) — Tier-1 output → Tier-2 consumption

### 6.1 Stage the Tier-1 binaries (runbook Step 4)
- Copy `cef_binary_<TARGET>.*` output → `cef-binaries/` (back up the current `cef-binaries/Release` first). Copy `Release/`, `Resources/`, `include/`, and the wrapper source into the matching layout (`BUILD_AND_RELEASE.md` §2.3).
- **Rebuild `libcef_dll_wrapper`** against the new headers (delete `build/CMakeCache.txt` + `build/`, regenerate, `cmake --build . --config Release`).
- **Rebuild `cef-native`** against the new wrapper/headers.
- **Publish** the binary distribution to the **`cef-binaries` GitHub release** — CI pulls `cef-binaries-windows.zip` / `cef-binaries-macos.tar.bz2` from that release.

### 6.2 Tier-2 app release consumes (unchanged — `BUILD_AND_RELEASE.md`)
`release.yml` on a `v*` tag: pulls the `cef-binaries` artifact → builds wrapper + `cef-native` + Rust + frontend → signs (Win Authenticode `CN=Marston Enterprises`; mac Developer ID + notarize) → packages (Inno/DMG) → publishes the **draft** release + appcast → **manual promote gate** → website deploy. Version is **tag-derived** (git tag = source of truth; `cargo-release` bumps+tags; CMake/Rust/Inno/TS all inject the tag).

### 6.3 CI runner pin (ABI-critical — `CEF_VERSION_UPDATE_TRACKER.md`)
Pin `runs-on:` in `release.yml` so the CI compiler **matches the CEF binary's toolset** — **never `windows-latest`/`macos-latest`** (the beta.16 `windows-2025` drift that killed the build at *configure*; the `macos-latest`→Tahoe drift that stamped `minos 26` and bricked mac auto-update). The build-host toolchain (§2.2) is documented **separately** from the CI runner pin — they are two distinct toolchains (I9).

---

## 7. TEST PLAN — the beta.1 acceptance gate

Every item maps to an outline §7 checklist row. Run **per-OS** and reconcile in `CHROMIUM_BUILD_RELAY.md` (or an extension of `MAC_WINDOWS_RELAY.md`).

### 7.1 Codec smoke (P5/P6 — `PLAN_codecs.md` §6)
- **Layer A (`canPlayType`):** H.264 baseline (`avc1.42E01E`), H.264 High (`avc1.640028`), AAC (`mp4a.40.2`), MP3 (`audio/mpeg`), VP9 (`vp09…`) → all **`probably` = GATE**; AV1 (`av01…`) → assert presence; HEVC (`hvc1…`) → record per-machine, **non-gating**; Dolby out.
- **Layer B (real playback, both OS):** youtube.com, x.com (video + animated-GIF-as-MP4 canary), reddit.com, twitch.tv, linkedin.com, an audio site (soundcloud/stable MP3 embed). Pass = real audio+video plays, seeks, no infinite spinner.
- A `""` on any GATE row = codec build regressed → **block the bump**, re-audit `args.gn` (Step 5.5).

### 7.2 Farbling acceptance (P6 — the B1 gate)
Test surfaces: **CreepJS**, **browserleaks.com** (canvas/webgl/audio/fonts), and a small custom worker-parity harness (CreepJS only exercises the dedicated-worker column — outline §3c I2).
- [ ] **worker column == window column** for canvas/WebGL/audio — **including service-worker, shared-worker, and OffscreenCanvas-in-worker** (P4e is not free; these are the OOP contexts the Supplement doesn't reach automatically).
- [ ] **Intra-session consistency:** same canvas/WebGL/audio read twice in one session+domain → **identical** perturbation (load-bearing for site correctness).
- [ ] **Cross-profile difference:** same site in two profiles → different farbled values.
- [ ] **Cross-site iframe:** a third-party origin embedded in two different first parties → **different** values (first-party/top-frame keying works — I4).
- [ ] **Cross-session login test (LOAD-BEARING):** create an account on a real site → fully restart the browser → revisit → **login does NOT break** (persistent per-profile seed working — this is the whole reason we chose persistent over Brave's per-session reset).
- [ ] Navigator values within the standard valid set (deviceMemory ∈ {2,4,8,16,32}; plausible concurrency); WebGL vendor/renderer decision (drop, or common-GPU-string map incl. **Mac ANGLE entries**) applied per the resolved §3c value table.
- [ ] **OAuth/auth-domain exemption (C7 = `IsAuthDomain` re-impl) verified:** pre-approved/auth sites are **un-farbled and log in** (Q3). Test the real OAuth basket below (§7.3).
- [ ] **No persistent seed on any renderer command line** — verify via ProcessExplorer (Win) / `ps` (Mac) that no stable per-profile secret is exposed on a child cmdline (C2 threat model).
- [ ] **Stability soak + renderer crash-rate gate** on the fresh Chromium bump + Blink patches (no elevated crashes vs baseline). **Baseline = the current shipping public M136 build** (which exists regardless of whether P1's from-source baseline build succeeds), **NOT** the P1 from-source binary — §2.5/§3 explicitly permit P1 to be downgraded to a last-known-good smoke, in which case there are no from-source baseline numbers to compare against.
- [ ] **Canvas/WebGL performance-regression gate:** `readPixels`/`getImageData` perturbation within an accepted budget **vs the current shipping public M136 build** (same baseline caveat as above — not the possibly-absent P1 from-source binary).
- [ ] **Escape-hatch works (ties to §8 fallback):** verify a build with the optional `condition` farbling gate toggled **off** actually ships **farbling disabled** (window == worker == un-farbled, values match a stock Chromium fingerprint). The §8 fallback relies on this toggle producing a farbling-off build; prove it before trusting it as the rollback lever.

### 7.3 Adblock still works + OAuth sites still log in (P6 — regression)
- **Adblock (Q2):** YouTube ad-strip via `CefResponseFilter` (`AdblockResponseFilter`) still works; cosmetic CSS + scriptlet injection still fires; the removed JS-farbling site (`:586-632`) was deleted cleanly **without disturbing adjacent scriptlet/cosmetic IPC handlers** (Q2 §ordering); blocked-count badges update. `hodos-unbreak.txt` untouched (adblock file, not farbling — I1).
- **OAuth / auth login basket (Q3 + FedCM §3g):** log in successfully on x.com, google.com (**"Sign in with Google" FedCM account chooser appears** — CefPermissionHandler FedCM coverage audited), github.com, and one federated-login relying party. These exercise both the auth-domain farbling exemption AND FedCM at once.

### 7.4 Standard site basket (P6 — CLAUDE.md Testing Standards, both OS)
Auth (x.com, google.com, github.com), Video/Media (youtube.com, twitch.tv), News (nytimes.com, reddit.com), E-commerce (amazon.com), Productivity (docs.google.com), BSV (whatsonchain.com). **Thorough** tier (30–45 min) since this is a pre-release engine bump.

### 7.5 DRM (P5/P6 — `Q4_widevine_amazon_drm.md`)
Component-updater Widevine CDM auto-download tested; **VMP-`.sig`-required-for-L3-on-Windows** question answered; Amazon result documented (plays free at L3 → in; SD-capped/refused/needs-VMP → deferred, with cost + broken-site list). Do **not** conflate a codec failure with a DRM failure (§7.1 smoke is deliberately DRM-free so a red result unambiguously implicates codecs).

### 7.6 macOS minos guard (P6 — the mac-auto-update safety net)
CI post-build guard **fails the build unless every exe/helper/Rust-bin `minos` ≥ the CEF framework `minos`** (an inequality, not `== floor`). CI runs on the newest macOS and **cannot** reproduce a sub-floor loader rejection → **also do a manual relaunch-after-update on a real machine at/near the floor** before promote (`CEF_VERSION_UPDATE_TRACKER.md`).

### 7.7 ⭐ REAL N-1→N auto-update apply — BOTH OS — WITH SIGNER-CONTINUITY (the highest reinstall-forcer class)

This is the centerpiece and a **hard gate**. **No proxies** — the actual updater, the actual new-CEF binary, the actual signed builds. Reuse the **existing, proven** rigs in `SILENT_UPDATE_TEST_PLAN.md` (silent update shipped live beta.25→26 Win / beta.21→22 mac; the Stage-1 rigs + Stage-2/3 real-apply legs are the standing regression procedure — do **not** rebuild them).

**Why this bump makes it non-optional:** the N-1 build (current public) carries the **old CEF file manifest**; N carries the **TARGET CEF manifest + new minos + new framework layout**. The Step 5.5 drift audit (§5) tells the tester exactly which files change; the apply must place them correctly and the browser must relaunch clean. This is *precisely* the failure mode a silent update hides.

**Procedure per OS (from `SILENT_UPDATE_TEST_PLAN.md`):**
1. **Stage 1 rigs (logic):** `scripts/test-update-feed.ps1`, `scripts/test-apply-rollback.ps1`, `scripts/test-apply-forward.ps1` — feed/verify + forward-apply + rollback correctness. Green = logic OK (not proof a real signed build works).
2. **Stage 2 real-build (dev wallet):** `scripts/setup-real-apply-test.ps1` builds N (current CEF) + N+1 (**TARGET CEF**), installs N, pre-stages signed N+1, applies at cold boot. **Both legs:** happy path commits + dev wallet intact; deliberately-broken N+1 (truncated exe / non-zero-exit stub) **rolls back** to N with `update-state.json paused=true` and wallet byte-intact. Use a throwaway dev wallet with its recovery phrase written down.
3. **Stage 3 production-signed (trivial-balance prod wallet):** CI-built **signed** N+1 with real Marston Authenticode + prod EdDSA, private appcast (never the public feed). Both legs on real signed builds; verify CI key self-checks green + website byte-stability (LF/un-minified — the CRLF trap silently breaks signatures).

**⭐ SIGNER-CONTINUITY VERIFICATION (fold into every leg — ties to `ORG_IDENTITY_SIGNING_MIGRATION.md`):**
A signing-identity change **forces a reinstall** ([[feedback_update_stability_principle]]). The apply test can pass on the *bytes* while prod forces a reinstall on the *signer* — so verify continuity explicitly:
- **Windows:** `CN=Marston Enterprises` is **unchanged** between N-1 and N (Authenticode subject CN — this is the field the shipped signer-continuity gate compares, beta.23; the Azure leaf thumbprint rotates ~3 days and must NOT be the comparison key). `signtool verify` / cert-subject check on both installers.
- **macOS:** `codesign -dv --verbose=4 <app>` on both N-1 and N shows **Team ID UNCHANGED** and Authority = the expected Developer ID. **Sparkle constraint (primary source):** Sparkle lets you rotate **either** the Developer ID cert **or** the EdDSA key in one update — **never both simultaneously** (or the chain of trust breaks → "improperly signed" → dead update). **⚠️ Team ID is NOT guaranteed preserved — confirm, don't assume.** `ORG_IDENTITY_SIGNING_MIGRATION.md` (lines 21–22) says the individual→org conversion *should* preserve Team ID but this is not contractually guaranteed by Apple and **MUST be confirmed pre-build** — a changed Team ID is itself a hard reinstall-forcer via Gatekeeper/keychain identity, **independent of Sparkle**. Separately, the Developer ID cert **identity/display string DOES change** on conversion (`ORG_IDENTITY_SIGNING_MIGRATION.md` line 76: the pipeline updates `release.yml` to the new cert name) — so this **is** the Sparkle cert-rotation case: **keep the EdDSA key unchanged.** IF Team ID is confirmed preserved AND EdDSA is not rotated, continuity holds. **If Team ID is NOT preserved, option (A) below is off the table and beta.1 stays individual-signed.**

**🚦 The org-migration gate on beta.1 (decision required — §9 OQ-1):**
Because the macOS org migration (`ORG_IDENTITY_SIGNING_MIGRATION.md`) is **pending**, resolve before cutting beta.1:
- **(A) Migration complete first (recommended, conditional on Team-ID confirmation):** beta.1 is the **first org-signed** build. **Precondition:** confirm Team ID is preserved by the conversion *before* investing the build (§7.7 ⚠️) — if it changes, (A) is off the table. If confirmed, the N-1→N apple apply test then runs N-1=individual-signed → N=org-signed and **MUST prove the update applies without a forced reinstall** (do NOT rotate EdDSA in the same step — the cert identity string already changes). This is the *only* way to validate the conversion is not a reinstall-forcer — a real apply, not an assumption.
- **(B) Migration deferred:** beta.1 stays individual-signed; the org swap is deferred to a later release **whose own N-1→N apply test is the gate**. Acceptable, but explicitly log that the reinstall-forcer risk is deferred, not eliminated.
- **Recommended default: (A)** — do the migration before beta.1 so the biggest reinstall-forcer is validated by the same real-apply test the CEF bump already requires (one test covers both risks).

**Pass criteria:** on **both OS**, N-1→N applies + relaunches clean with the new CEF manifest/minos; broken-N rolls back with wallet intact; **signer continuity verified** (CN / Team ID unchanged); no "no browser" state either way; no `update.lock`/RunOnce/stray `pending\` left behind.

---

## 8. v0.4.0-beta.1 readiness checklist (this doc's slice)

Build/pipeline + test items (farbling/codec/DRM value-decisions gated by the sibling docs' checklists):
- [ ] P0 hosts provisioned; toolset matched to TARGET; M136-still-builds confirmed (or P1 downgraded).
- [ ] P1 baseline green (or last-known-good smoke).
- [ ] P2 bump: deps pass; toolchain + minos aligned; FedCM audited; Step 5.5 drift audit clean (human-reviewed).
- [ ] P3 patch toolchain proven (no-op patch applies + builds).
- [ ] P4 farbling landed + old JS path retired (M1 teardown).
- [ ] P5 codecs re-verified; DRM tested + documented.
- [ ] P7 binaries staged to `cef-binaries` release; wrapper + `cef-native` rebuilt (no "Unsupported CEF version"); all Output-file-checklist files present; CI runner pinned (ABI-match, no `*-latest`).
- [ ] **§7 test plan all green on both OS**, incl. the **real N-1→N auto-update apply with signer continuity**.
- [ ] Build changelog appended to `CEF_VERSION_UPDATE_TRACKER.md` (branch, milestone, GN_DEFINES, patch-set version, deps, duration, **per-bump patch-rebase hours**).
- [ ] Org-signing gate resolved (OQ-1: (A) migrate-first recommended).
- [ ] **Fallback documented** (outline §8 #13): rollback to the M136 (or previous) branch if TARGET destabilizes at gate time — not just toggling farbling off via the optional `condition` build gate.

---

## 9. Open questions (with recommended defaults)

| # | Question | Recommended default |
|---|---|---|
| **OQ-1** | Migrate macOS individual→org **before** beta.1, or defer? | **Migrate first (A) — conditional on confirming Team ID is preserved by the conversion (§7.7 ⚠️).** If confirmed pre-build, beta.1 = first org-signed build and the required N-1→N apple apply test validates the conversion is not a reinstall-forcer (don't rotate EdDSA same step); one test then covers both the CEF-bump manifest risk and the signer risk. **If Team ID is NOT preserved, (A) is off the table → fall back to (B) individual-signed beta.1.** The "one test covers both" claim holds only after the Team-ID-unchanged confirmation. |
| **OQ-2** | Self-hosted persistent VM vs cloud spot for the cold build? | **Persistent/owned host by default; cloud spot ONLY after TARGET-builder resume is confirmed.** The `.ninja_log` resumability evidence is M136/Ninja-era, but the TARGET likely builds under **Siso** (now Chromium's default) whose resume semantics differ and are unverified here (I1). Do the P0 Step-0 build-tool lookup first: if Siso cold-resume is confirmed (or `use_siso=false` + Ninja still supported on TARGET), a cloud spot with a persistent disk volume is fine (reclaim costs only the delta); until then, prefer a persistent/owned beefy machine. Keep disk persistent either way. |
| **OQ-3** | S3 shared sccache now, or local disk? | **Local disk for beta.1.** Cold build gets no benefit either way; shared cache pays off only once there are multiple build hosts / warm CI incrementals. Revisit when a second build host exists. |
| **OQ-4** | Run the N-1→N apply test on a **funded** wallet? | **No — throwaway/trivial-balance only**, recovery phrase written down (`SILENT_UPDATE_TEST_PLAN.md` safety checklist). A funded user bricked by a bad update is the one unrecoverable case; the rigs + dev/trivial wallets prove the mechanism without that risk. |
| **OQ-5** | Automate the codec Layer-A + farbling acceptance probes in CI? | **Later, not a beta.1 blocker** — they need the built binary on the self-hosted host; fold into the Step-5.5 automation TODO (runbook line 323). Manual for beta.1. |
| **OQ-6** | macOS arch: arm64 / x64 / universal2? | **universal2** (distribution breadth) with owner sign-off — but it lengthens the Mac cold build and doubles the "common GPU strings" set (Apple Silicon + Intel ANGLE). Mac Claude owns; settle before P2-on-Mac (outline §5 I8). |
| **OQ-7** | Does M136-from-source still fetch/build in mid-2026? | **Confirm in P0.** If bit-rotted, downgrade P1 to a last-known-good smoke rather than a full baseline (outline §4 I5) — don't gate on an un-meetable build. |

**Standing risks carried into execution:** cold build ~10–12 hr/OS (no cache help); CEF-fork security-point-release upkeep is a new recurring duty; per-bump Blink patch-rebase on high-churn files is *the* recurring cost; a changed CEF file manifest is the silent-update failure mode (§5→§7.7); Amazon DRM outcome unknown until tested; the macOS signer migration is the biggest reinstall-forcer and must be validated by a real apply, never assumed.

---

*Feeds the outline §7 readiness checklist (build-integrity + auto-update-apply + regression/parity rows) and `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`. This doc stops at a followable plan; the implementing session runs §2–§7 against the real TARGET build.*

---

### Sources (primary)
- CEF — Branches & Building (build args, branch↔milestone, `automate-git.py`): https://chromiumembedded.github.io/cef/branches_and_building.html
- CEF builds CDN / version→branch (`index.json`, not the wiki): https://cef-builds.spotifycdn.com/index.json
- Chromium — Linux/Windows build instructions (disk/RAM; ~100 GB): https://github.com/chromium/chromium/blob/main/docs/linux/build_instructions.md
- GitHub-hosted runners reference (~72 GB total / ~50 GB pre-consumed → ~14–29 GB free; 6-hr job cap — why Chromium can't build on hosted runners): https://docs.github.com/en/actions/reference/runners/github-hosted-runners · https://github.com/actions/runner-images/discussions/9329
- mozilla/sccache (MSVC + shared/S3 cache; client-server model): https://github.com/mozilla/sccache
- Chromium `cc_wrapper` + sccache on Windows (few MSVC cache-hits caveat): https://issues.chromium.org/issues/40188007
- Sparkle — code-signing continuity (rotate cert OR EdDSA key, never both; Team ID/bundle-ID must match): https://sparkle-project.org/documentation/ · https://github.com/sparkle-project/Sparkle/discussions/2394
- In-repo authoritative anchors: `DevOps-CICD/CEF_BUILD_RUNBOOK.md` (Tier-1), `DevOps-CICD/SILENT_UPDATE_TEST_PLAN.md` (the update-apply rig), `DevOps-CICD/ORG_IDENTITY_SIGNING_MIGRATION.md` (signer-continuity gate), `DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md` (toolchain + minos).
