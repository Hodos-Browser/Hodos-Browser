# PLAN — Chromium/CEF Version Bump (136 → target) for v0.4.0-beta.1

**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**Status:** DETAILED PLAN — research + design only. **NO code, NO builds.**
**Purpose:** Resolve *which* CEF/Chromium version we bump to (stable vs LTS, from primary sources) and give a followable, step-by-step bump-execution procedure — branch pin, GN diff, toolchain re-pin, macOS minos re-measure, dependency re-verify, breakage detection, rollback, and the signer-continuity gate. Feeds `Q5_full_edit_list.md` (the `Process` rows / P1–P2 baseline) and `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` (P1/P2 sequencing).

> **Scope boundary.** This doc owns the **version-target decision** and the **mechanics of moving the branch pin**. It does *not* own: codec flags (`PLAN_codecs.md`), farbling patches (`Q2_farbling_adblock.md` + `PLAN_farbling_blink.md`), Widevine/DRM (`Q4_widevine_amazon_drm.md`), or the full edit inventory (`Q5_full_edit_list.md`). Where those intersect the bump (patch rebase, codec re-verify, `enable_widevine` resolution) this doc points at them and states the *bump-time* obligation only.

> **Authoritative inputs (repo):** `DevOps-CICD/CEF_BUILD_RUNBOOK.md` (Step 1 branch choice, lines 71–108; §7.3 LTS rationale), `CEF_VERSION_UPDATE_TRACKER.md` (current version, macOS floor, must-investigate list), `DEPENDENCY_VERIFICATION.md` (per-bump dep checklist), `0.4.0/CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §2 Step 0 (version resolution) + open-question **C1** (LTS-vs-Extended-Stable conflation), `0.4.0/B1-farbling-design.md` (patch rebase cost), `scripts/build_hodos_cef.bat` / `scripts/build_hodos_cef_mac.sh` (the `--branch=` invocation).
> **Authoritative inputs (primary, web — verified 2026-07-10):** CEF `branches_and_building.html`; CEF issue #3947 (LTC/LTS automated builds); CEF issue #4114 (2-week-cadence adjustments); Chrome Releases blog + chromiumdash (M150 stable date + macOS floor).

---

## 0. TL;DR — the decision

**Bump `136` (branch `7103`) → `150` (branch `7871`): adopt branch `7871` and *ride it into the M150 LTS line*, gated on it reaching ≥ CEF-Stable channel by build day.**

> **⚠️ Channel-maturity caveat (read before anything else).** As of the verification date, branch `7871` is **CEF *Beta*** — it is **not LTS, and not yet even CEF-Stable.** The *current* newest CEF **LTS** is **M144 (branch `7559`)**; the *current* newest CEF **Stable** is **M149 (branch `7827`)**. M150 is a **future** LTS milestone (the LTS cadence is every 6th branch: **M138 → M144 → M150 → M156**), but a branch only becomes LTS after it exits Stable → LTC (~3 mo) → LTS — roughly a year out. **We are deliberately adopting `7871` and riding the same branch as it matures Beta → Stable → LTC → M150 LTS**, to avoid a near-term second jump off M144. This is only acceptable if the §3 pre-flight channel gate holds — see below.

- **Placeholder resolved.** The in-repo target was written as "CEF 149 / M150-LTS placeholder." Primary sources now pin the milestone concretely: **M150 = CEF branch `7871`**, whose Chromium promoted to stable ≈ late June 2026. M150 is the **next LTS milestone**, but that LTS designation is a *future* state of the branch, not its maturity today.
- **The old "no LTS exists" note is SUPERSEDED.** A real CEF LTC/LTS program exists, confirmed from three primary sources (below). Earlier session notes that said "CEF = stable M149, no LTS" were written before this was verified — treat them as stale. (Note: M149/`7827` *is* the current CEF-Stable — those notes had the *stable* milestone right; they were only wrong about LTS not existing.)
- **Default recommendation: ride branch `7871` into the M150 LTS, not chase newest-stable.** Reasoning in §2. In one line: with Chromium going to a **2-week stable cadence in Sept 2026**, chasing stable is untenable for a self-builder; the LTS line gives ~8–9 months of security coverage per milestone (**platform-agnostic fixes only** — see I2/§2) and only takes feature churn every 6 months — the exact profile a small team self-building Chromium needs. The trade is that we accept current Beta-channel maturity on `7871` up front, gated on it reaching ≥ CEF-Stable by build day (§3).

**Cost flagged up front:** the M136→M150 jump also **raises the macOS floor from Big Sur 11.0 → Monterey 12.0** (M150 is the last Chrome to support Monterey; M151 requires Ventura 13). This is a published-minimum change that touches auto-update — see §5.

---

## 1. Primary-source resolution — stable vs LTS (verified 2026-07-10)

### 1.1 The authoritative CEF branch table

From CEF `branches_and_building.html` ("Supported Release Branches"), quoted exactly:

| Branch | CEF Version | Chromium | Channel (as documented, verification date) |
|--------|-------------|----------|---------------------------------------------|
| **7871** | **150** | **150** | **CEF Beta** (Chromium-150 went upstream-stable ≈ late June 2026, but CEF's *binary* channel = Beta) |
| **7827** | **149** | **149** | **CEF Stable** ← *current newest CEF-Stable* |
| **7559** | **144** | **144** | **CEF LTS** ← *current newest CEF-LTS* |

> **Do not conflate two channel systems.** Chromium-150 promoting to *upstream* stable (≈ late June 2026) is independent of **CEF's own `Beta` label on branch `7871`**, which is CEF's assessment of *its binary's* maturity. As of the verification date, `7871` is **CEF Beta** — pinning it *today* yields a CEF-Beta-maturity binary. There is no evidence the Beta label is a stale table entry; treat it as authoritative. The branch number (`7871`) is what `automate-git --branch=` consumes, but **the maturity of that branch is gated by its CEF channel, which must reach ≥ Stable before we build for production** (§3 pre-flight, §7 acceptance). Sequence of maturity for `7871`: **Beta → Stable → LTC → M150 LTS.**

Our current pin: **branch `7103` = CEF 136 = Chromium 136** (`CEF_BUILD_RUNBOOK.md` line 82; `scripts/build_hodos_cef.bat` `--branch=7103`). This is **~14 months / 14 milestones behind stable** and **predates the M138 LTS program → zero current security-patch coverage.**

### 1.2 The LTC/LTS program is real (resolves outline open-question C1)

Three primary sources confirm a genuine long-term-support program — this settles the stale "no LTS exists" note:

1. **CEF `branches_and_building.html`:** *"Every sixth branch (starting with M138) proceeds through the long-term support candidate (LTC) and long-term support (LTS) channels after exiting stable. The LTC/LTS channels continue to receive **platform-agnostic** security fixes for ~8 additional months."* → LTS milestones are **M138, M144, M150, M156, …** **Two limits to hold in mind:** (a) the lifespan figure is **~8–9 months, source-dependent** (branches doc says ~8; issue #4114 says ~9) — use the range, not a point value, in any expiry math; (b) coverage is **platform-agnostic only** — a Windows-sandbox-specific or macOS-only CVE may *not* be backported to LTS and could force an off-cadence milestone jump (see I2 / §2).
2. **CEF issue #3947** (Add automated LTC/LTS builds, M138): LTS updates **every 6 months**; **LTC = LTS features 3 months early** (preview). Marked *enhancement / blocked* — i.e. the **automated *prebuilt* LTS builds on the Spotify CDN may still be lagging**, but this **does not affect us** (we build from source via `automate-git --branch=7871`; we do not consume the prebuilt LTS binary). See §6 open question OQ-3.
3. **CEF issue #4114** (Adjustments for Chromium's 2-week release cycle, starting Sept 2026): *"this will not impact LTC/LTS builds which will continue to run with an ~9 month lifespan."* Two candidate CEF strategies for stable — (a) **even-milestones-only** to hold a 4-week cadence, or (b) **move CEF stable to extended-stable (8-week milestones)**. Either way **LTS is the stable anchor for embedders.**

### 1.3 Cadence facts (chromiumdash / Chrome Releases)

- Chromium **stable cadence is currently 4-week**; **M150 upstream-stable ≈ late June 2026** (sources say "around 2026-06-30"; back-calculating 4-week from Chrome 153 on Sep 8 lands nearer mid-June — re-verify at execution per §3), **M151 stable ≈ 2026-07-28**. *Note: these are upstream-Chromium stable dates, distinct from CEF's own channel promotion of branch `7871` (see §1.1).*
- **Sept 2026: Chromium moves to a 2-week stable cadence** (developer.chrome.com "two-week release cycle"). Extended-stable = 8-week.
- **macOS floor:** **M150 is the last Chrome to support macOS 12 Monterey; M151+ requires macOS 13 Ventura.** (Big Sur 11 — our current M136 floor — is *believed* to have been dropped upstream before M150; the Monterey-focused sources don't state this directly, so don't treat the 11→12 framing as gospel. §4.4 is robust regardless because it takes `max(12.0, measured minos)` — confirm M150's actual floor by measurement, not the announcement.)

---

## 2. The recommendation, with reasoning

**RECOMMEND: adopt branch `7871` and ride it into the M150 LTS** — pinned now (gated on it reaching ≥ CEF-Stable by build day, §3), then held as our LTS anchor once `7871` reaches the LTS channel. Reject "chase newest CEF-stable forever." The realistic near-term alternatives an execution team actually weighs are the **current CEF-Stable (M149/`7827`)** and the **current CEF-LTS (M144/`7559`)** — both compared below.

> **Honest framing.** No branch is "the newest LTS that has entered stable" *today* — an LTS branch by definition has already *exited* stable, and the only branch satisfying "is CEF-LTS" right now is **M144**. We are choosing to get ahead of the LTS cadence by pinning the *future* M150 LTS branch (`7871`) early and riding it through Beta → Stable → LTC → LTS, rather than take M144 (near expiry) or chase M149-stable. The channel-maturity gate (§3) is what keeps this from shipping a Beta binary.

**Why ride M150 rather than pin current-stable or current-LTS — decision drivers:**

| Driver | Current CEF-Stable (M149 / `7827`, exists today) | **Ride branch `7871` → M150 LTS** ← pick |
|--------|--------------------------------------------------|------------------------------------------|
| **Rebase labor** = frequency × patch-depth on high-churn Blink files (Canvas2D `base_rendering_context_2d.cc` is the risky one — `B1-farbling-design.md` line 99) | Chasing CEF-stable means re-bumping on CEF's post-Sept-2026 stable cadence (even-milestones or extended-stable, issue #4114) → recurring churn; a cold build is ~10–12 h (`CEF_BUILD_RUNBOOK.md`) | Feature churn **every 6 months** once on LTS; in-between only cherry-pick security point-releases (patches re-apply trivially) |
| **Security coverage** | Full while it *is* stable, but stable rolls forward — you must chase to stay covered | **~8–9 months** of **platform-agnostic-only** security fixes per LTS (issue #3947, branches doc). ⚠️ Platform-*specific* CVEs (Windows-sandbox / macOS-only) may not be backported → may still force an off-cadence jump |
| **Runway before next forced jump** | Weeks (until CEF's next stable) | Maximal once M150 reaches LTS — its ~8–9-mo LTS window has **not yet begun** (entering stable ≠ starting the LTS clock), so pinning `7871` now positions us for the *full* window when it opens |
| **FedCM / media / API gates** (`CEF_VERSION_UPDATE_TRACKER.md` must-investigate) | Modern surface (M149) | M150 is modern (14 milestones newer than M136) → picks up FedCM handler surface, permission API, codec-flag currency in one jump |
| **Alignment with existing plan** | — | Matches runbook §7.3 + master-plan pin + `Q5` `Process` rows |

**Why not just pin M144 (the *current* shipped LTS):** M144 (branch `7559`, LTS since Dec 2025) is ~7 months into its ~8–9-month window → we'd inherit an LTS that expires within ~1–2 months and be forced to jump again immediately. **Riding `7871` positions us for the full M150 LTS runway** instead of a near-expiry LTS.

**Why not pin M149-stable (`7827`) and treat M150-LTS as a later jump:** valid fallback (option (b) — see §0 caveat), and it has real CEF-Stable maturity *today*. We prefer riding `7871` because M149-stable will itself roll forward off stable, forcing another bump, whereas `7871` converges onto our intended LTS anchor. **If the §3 channel gate shows `7871` is still Beta on build day and cannot be waited out, fall back to pinning M149/`7827` (current stable) rather than shipping a Beta binary.**

**Why not wait for M156:** M156 is the *next* LTS milestone (Q1 2027-ish) and is not on stable yet — waiting strands us on the dead M136 for another two quarters with zero security coverage. Adopt `7871` now; `7871`→M156 becomes the first "milestone jump" rebase (§5 of the runbook cadence).

**Net:** `automate-git --branch=7871`, pinned (gated on ≥ CEF-Stable at build day, §3); quarterly in-branch security point-release pulls; hold as the LTS anchor once `7871` reaches LTS; a milestone jump to M156 when M156 exits stable. Fallback if the channel gate blocks: M149/`7827` current stable.

---

## 3. Pre-flight — confirm the numbers at execution time

Version data drifts; re-verify these **the day the build starts** (outline §2 Step 0). Record answers in the build changelog.

1. **Confirm the M150 branch number is still `7871`** — cross-check `branches_and_building.html` and the CDN discovery file `https://cef-builds.spotifycdn.com/index.json` (parse for `chromium_version` starting `150.` on `windows64` + `macosarm64`/`macosx64`; note its `cef_version` string, e.g. `150.x.y+g<hash>+chromium-150.0.zzzz.w`).
2. **⛔ CHANNEL-MATURITY GATE (build-blocking).** Read the **CEF *channel*** of branch `7871` on `branches_and_building.html` **on build day**. Proceed **only if `7871` has reached ≥ CEF-Stable** (Stable, LTC, or LTS) — **NOT Beta.** On the verification date it was **Beta**; do not build a production money-handling browser off a CEF-Beta binary. If it is still Beta and cannot be waited out within the release window, **fall back to pinning the current CEF-Stable (M149/`7827`)** per §2, and record the fallback in the changelog. This gate is distinct from the milestone-label check below — a milestone can be "the future M150 LTS" while its *binary channel* is still Beta.
3. **Confirm M150 is still the newest LTS-designated milestone** (i.e. M156 has NOT yet exited stable and become the newest LTS). If M156 has shipped stable by execution time, re-evaluate M150 vs M156 with the §2 drivers (prefer the newer LTS only if it has entered stable and its toolchain is validated).
4. **Capture M150's exact latest security point-release** (e.g. `150.0.7871.NNN`) — pin to the newest patch of branch 7871, not the `.0`.
5. **Record the toolchain M150 was built with** — MSVC/Clang toolset + Windows SDK + min-macOS the CEF/Chromium M150 tree expects (drives §4.3 re-pin). Sources: CEF release notes for 7871, Chromium `//build` toolchain bump commits for M150.
6. **Look up M150's oldest supported macOS** (release notes): **macOS 12 Monterey** (verified 2026-07-10) — feeds §5 minos.

**Acceptance for §3:** a one-paragraph "version-lock" note in the changelog: branch, **CEF channel of `7871` on build day (must be ≥ Stable — the step 2 gate)**, exact point-release, toolset, macOS floor, and confirmation M150 is still the newest LTS-designated milestone (or the recorded fallback to M149/`7827` if the channel gate blocked).

---

## 4. The bump procedure (followable)

> Runs inside `CEF_BUILD_RUNBOOK.md`'s Tier-1 flow. This section is the **version-specific overlay** — what changes *because* it's a 136→150 jump. Do NOT re-derive the environment setup; that's the runbook (Steps 3 env setup, depot_tools, `automate-git.py` fetch).

### 4.1 — Move the branch pin
- Edit the pin in **both** build scripts: `scripts/build_hodos_cef.bat` and `scripts/build_hodos_cef_mac.sh` — change `--branch=7103` → `--branch=7871`.
- If/when the farbling CEF fork exists (`Q5` CEF-1 / GREENFIELD), also rebase the fork onto upstream `7871` and keep `automate-git.py --url=<fork> --branch=7871` consistent. For the *pure version bump* baseline (P1, no farbling patches yet) this is stock upstream CEF at `7871`.
- Keep `GN_DEFINES` **byte-identical** to today for the baseline bump: `is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0` (`CEF_BUILD_RUNBOOK.md` line 164). Codec/DRM flag *changes* are `PLAN_codecs.md` / `Q4`, not this doc — bump first on unchanged flags to isolate version breakage from flag breakage.

### 4.2 — GN args / defaults diff (silent-drift guard)
A green compile does **not** prove flag intent survived (runbook Step 5.5). On the M150 tree:
- Regenerate `args.gn` and **diff M150's derived args against M136's** for our four pinned flags. Confirm each still resolves:
  - `proprietary_codecs=true` and `ffmpeg_branding=Chrome` still select the decoder-shipping FFmpeg config (a flipped default = green build, no codecs).
  - `chrome_pgo_phase=0` still drops `/Brepro` (sccache determinism).
  - `enable_widevine=true` still auto-resolves in generated args (boundary to `Q4` — assert only, don't add a manual flag).
- Diff `enable_platform_hevc` / `enable_dav1d_decoder` derived defaults vs M136 (`PLAN_codecs.md` GN-5/GN-6) — assert unchanged; don't re-decide here.
- Emit the diff into the changelog as a human-review artifact.

### 4.3 — Toolchain re-pin (build-breaker — highest priority)
Per `CEF_VERSION_UPDATE_TRACKER.md` "Toolchain & Dependency Alignment," four things must sit on the **same toolset** or you get linker/ABI failures that *look* like our bug but aren't: CEF binaries, vcpkg static deps, our C++ shell, the CI runner image.
1. From §3 step 5, take the toolset M150 was built with (MSVC v143/VS2022 family expected — confirm the exact Windows SDK M150 wants; it may require a newer 10.0.2xxxx SDK than M136).
2. **Rebuild `libcef_dll_wrapper`** against the new headers: delete `cef-binaries/.../wrapper/build/CMakeCache.txt` + `build/`, then reconfigure/rebuild (`"Unsupported CEF version"` ⇒ this step wasn't done — runbook Lessons).
3. **Rebuild vcpkg static deps** (nlohmann-json, sqlite3, OpenSSL, quirc) on the confirmed toolset; re-check the vcpkg baseline.
4. **Re-pin the CI runner images** in `.github/workflows/release.yml` (`runs-on:`) to the pinned `windows-2022` (or the validated newer pin) and `macos-15` (or newer validated) — **never `windows-latest` / `macos-latest`** (the beta.16 windows-2025 drift + the beta.16 minos disaster). ⚠️ These are the *current* pins; **re-validate them against the toolset M150/Chromium-150 actually requires** — Chromium-150's clang / Windows-SDK bump may exceed what `windows-2022` ships (see **OQ-6**). Confirm the pinned image actually ships the toolset M150 needs; bump the pin deliberately (never to `-latest`) if not.
5. Run the full **`DEPENDENCY_VERIFICATION.md`** pass (this is a *milestone jump* → full pass, not the light quarterly pass). Answer the 7-point checklist in writing for each of the 5 layers; append the old→new table to `CEF_VERSION_UPDATE_TRACKER.md`.

### 4.4 — macOS minos re-measure (ships broken auto-update if wrong)
Per `CEF_VERSION_UPDATE_TRACKER.md` "macOS Minimum Deployment Version":
1. Chromium floor for M150 = **macOS 12 Monterey** (verified §1.3). This is a **raise from our current 11.0 Big Sur.**
2. **Measure the actual framework minos** on a Mac (don't trust the announcement):
   `vtool -show-build "<...>/Chromium Embedded Framework.framework/Chromium Embedded Framework" | awk '/minos/{print $2}'`
3. Set published minimum = **`max(12.0, measured minos)`** in all three places, kept identical:
   - `cef-native/CMakeLists.txt` → `CMAKE_OSX_DEPLOYMENT_TARGET`
   - `cef-native/Info.plist` → `LSMinimumSystemVersion`
   - `cef-native/mac/helper-Info.plist.in` → `LSMinimumSystemVersion`
4. **Apply it for real** — pass `-DCMAKE_OSX_DEPLOYMENT_TARGET=12.0` on the configure command line (a bare `set(... CACHE ...)` after `project()` is a silent no-op) and export `MACOSX_DEPLOYMENT_TARGET=12.0` at job level so the wrapper, cargo, and sub-cmakes inherit one floor.
5. **CI minos guard** must pass: after build, read `minos` of the main exe, every helper app, and both Rust binaries; **FAIL unless each `minos` ≥ the CEF framework's minos** (inequality, not `==`). Plus a **manual relaunch-after-update on a real Mac at/near 12.0** before `promote --latest`.

> **Fleet note:** raising the floor 11.0→12.0 strands any Big Sur users on the last 11.0-compatible build. Because a floor raise *gates* the update (Sparkle refuses on sub-floor OS) rather than crashing, this is the "published min too HIGH but honest" mode — acceptable, but call it out in release notes. This is a natural consequence of tracking modern Chromium; it is not avoidable while on M150.

### 4.5 — Farbling / codec / DRM re-verify (bump-time obligations, owned elsewhere)
- **Patches:** if the farbling patch set exists by build time, re-apply via `patch.cfg` and report fuzz/failures (runbook Step 5.5). Budget the "milestone jump" rebase (~2–8 h for ~5–8 patches — `B1-farbling-design.md` line 99). For the **pure P1 version bump**, there are no patches yet → skip; this is why P1 (bump) precedes the farbling patch work in the roadmap.
- **Codecs:** re-verify for real (flags persist, behavior must be smoke-tested) — `video.canPlayType('video/mp4; codecs="avc1.42E01E"')` → `'probably'`; smoke x.com/Reddit/Twitch/YouTube (`PLAN_codecs.md` acceptance).
- **Widevine:** confirm `enable_widevine=true` resolved + CDM auto-downloads (`Q4`).

### 4.6 — Runtime file-manifest diff (silent-drift guard)
Diff M150's CEF dist file list (DLLs, `.bin`, `.pak`, `resources/`, `locales/`) against the hardcoded copy-lists in `cef-native/CMakeLists.txt` ("Copying CEF binaries") **and** the macOS framework-embed list in `build_hodos_cef_mac.sh`. A new/renamed/removed file we don't copy = green build, runtime crash. Cross-check the runbook "Output file checklist" (line 309). 14 milestones of drift makes this **high-yield** — expect at least one changed resource.

---

## 5. Breakage detection

Two failure classes; hit both:

**A. Compile-time (fails loud — good).** CEF API surface changed across 14 milestones. Before the 10–12 h build, **diff CEF's release notes / `cef_version.h` + handler signatures M136→M150** and pre-audit our overrides for:
- `CefResponseFilter` (our `AdblockResponseFilter` for YouTube ad-key stripping — flagged LOW-stability in the tracker; **re-verify it still exists + streams**).
- `CefPermissionHandler` surface (FedCM handler methods likely *added* — tracker HIGH item; opportunity, not breakage).
- Download / find / JS-dialog handler signatures in `simple_handler.cpp` (12 interfaces).
- Render-process V8 injection timing (`OnContextCreated` / worker context hooks) — do not change threading; just confirm signatures.

**B. Silent drift (green build, wrong behavior — dangerous).** Covered by §4.2 (GN diff), §4.4 (minos guard), §4.6 (manifest diff). Plus the acceptance smoke matrix (§7) — this is the only net that catches a flipped codec default or a dropped `.pak`.

**Regression matrix:** CLAUDE.md Testing Standards **Thorough** basket (release gate), Windows **and** macOS. Specifically exercise: Google Sign-In / OAuth (FedCM), media playback (codecs), ad blocking (response filter), fingerprint protection (current JS impl until B1 lands).

---

## 6. Rollback

The bump is **reversible by construction** — nothing about M150 is committed to users until the `cef-binaries` GitHub release is published and `release.yml` consumes it.

| Stage | Rollback action |
|-------|-----------------|
| Before publishing new `cef-binaries` release | Revert `--branch` in both scripts to `7103`; the old `cef-binaries/Release` backup (runbook Step 4 says back it up first) is untouched. Zero user impact. |
| New CEF binaries staged, shell won't build/link | Toolchain mismatch (§4.3) — restore prior wrapper/vcpkg; do **not** ship. |
| Built + smoke-failed pre-promote | Do not publish the `cef-binaries` release; app pipeline keeps consuming the M136 binaries. |
| Published `cef-binaries` but app release not promoted | `release.yml` still points at the prior `cef-binaries` tag until deliberately bumped — pin it back. |
| App beta.1 promoted, field regression | Standard app-level rollback via the auto-update pipeline (promote the prior good build). **Precondition: signer continuity held (§8)** — else a rollback *also* forces a reinstall. |

**Keep the M136 `cef-binaries` release tag live** until M150 beta.1 has soaked — it is the rollback artifact.

---

## 7. Acceptance criteria (gate before `promote --latest`)

- [ ] **CEF channel of branch `7871` is ≥ Stable (Stable/LTC/LTS, NOT Beta) on build day** — the §3 step-2 gate; or the recorded fallback to M149/`7827` is in effect.
- [ ] Branch pin = `7871` (or fallback `7827`) in both build scripts; changelog "version-lock" note recorded (§3).
- [ ] `GN_DEFINES` unchanged from M136 baseline; GN-args diff reviewed, all four flags resolve (§4.2).
- [ ] `libcef_dll_wrapper` + vcpkg deps rebuilt on M150's toolset; `DEPENDENCY_VERIFICATION.md` full pass appended to the tracker (§4.3).
- [ ] CI runner images pinned (not `-latest`); pinned image ships M150's toolset (§4.3).
- [ ] macOS published minimum = `max(12.0, measured minos)` in all three files, applied via `-DCMAKE_OSX_DEPLOYMENT_TARGET`; CI minos guard green; real-Mac relaunch-after-update at floor passes (§4.4).
- [ ] Runtime file-manifest diff reconciled with both copy-lists (§4.6).
- [ ] Codec smoke passes (`canPlayType` → `probably`; x.com/Reddit/Twitch/YouTube play).
- [ ] FedCM: "Sign in with Google" account chooser appears (tracker HIGH item — verify M150 delivered it).
- [ ] Ad blocking (response filter) + fingerprint protection still function.
- [ ] Thorough regression basket green on Windows **and** macOS.
- [ ] **Signer-continuity gate green (§8).**
- [ ] Tracker + runbook updated with M150 findings (Invariant #12).

---

## 8. Signer-continuity gate (fold-in — must not be skipped)

A **signing-identity change forces a full reinstall**, which silently defeats the auto-update test (the update "passes" locally while prod re-installs). beta.1 may be the first *signed* 0.4.0 build, so this bump and the signing migration can collide.

- **macOS:** the **individual→org signing migration is PENDING** (`ORG_IDENTITY_SIGNING_MIGRATION.md`). The real N−1→N apply test must verify the **Team ID is UNCHANGED** across the update. If the org migration lands in the same release as the M150 bump, sequence it so the **Team ID stays constant** (org migration keeps the Team ID per that doc) — otherwise split them so only one reinstall-forcing change ships at a time.
- **Windows:** already `CN=Marston Enterprises` — the apply gate must assert the **signer Subject CN is unchanged** (the beta.22/23 regression root cause was comparing rotating Azure Trusted Signing *leaf thumbprints*; compare **CN**, not thumbprint).
- **Gate wording for the roadmap:** "the N−1→N auto-update apply test passes **only if** signer continuity holds (macOS Team ID / Windows CN unchanged); a signer change = expected reinstall, must be a deliberate, announced release — never coincident with a routine version bump."

---

## 9. Open questions (with recommended defaults)

| # | Question | Recommended default |
|---|----------|---------------------|
| **OQ-1** | Post-Sept-2026, CEF picks even-milestones-only vs stable→extended-stable (issue #4114). Does this change our LTS pin? | **No.** We anchor on **LTS regardless of the stable strategy** — issue #4114 explicitly says LTC/LTS is unaffected (~9-mo lifespan). Re-read #4114 at the M150→M156 jump to confirm M156 is still the 6th-branch LTS. |
| **OQ-2** | M150 vs M156 at execution time — if M156 has entered stable by build day. | Take the **newest LTS that has entered stable AND whose toolchain we've validated.** If M156 is stable + validated on build day, prefer it (more runway); if not, ship M150 now — do not stay on dead M136 waiting. |
| **OQ-3** | Automated *prebuilt* LTS binaries on the Spotify CDN may lag (issue #3947 "blocked"). Does that block us? | **No.** We **build from source** (`automate-git --branch=7871`); prebuilt LTS availability is irrelevant to a self-builder. Note it only so nobody waits on a CDN artifact. |
| **OQ-4** | macOS floor raise 11.0→12.0 strands Big Sur users. Accept? | **Accept** (honest gate, not a crash). Unavoidable on M150. Announce in release notes; consider a one-time "you're on the last supported build" notice for sub-12.0 users (auto-update track, not this doc). |
| **OQ-5** | Bump + farbling patches together, or bump-first? | **Bump-first (P1), patches-second (P2).** Isolates version breakage from patch breakage; matches the roadmap. The pure bump ships on stock upstream `7871` with unchanged GN flags. |
| **OQ-6** | Windows SDK version M150 requires may exceed our pinned runner's SDK. | Verify in §3 step 5 **before** the 10–12 h build; if M150 needs a newer SDK than `windows-2022` ships, either add the SDK component to the runner or bump the runner pin deliberately (never to `-latest`). Cross-ref §4.3 step 4. |
| **OQ-7** | Branch `7871` is **CEF Beta** as of the verification date, not LTS or Stable. Which C1-resolution do we take? | **Fix option (a): adopt `7871` and ride it into the M150 LTS, gated on it reaching ≥ CEF-Stable by build day (§3 step 2).** We did **not** reject options (b) pin current-stable M149/`7827` or (c) pin current-LTS M144/`7559` — (b) is the explicit **fallback if the channel gate still shows Beta on build day**; (c) is rejected only because M144 is ~1–2 months from LTS expiry. No adversarial point from the review was rejected; all were applied. Re-confirm `7871`'s channel at execution — if it has already reached LTS by build day, this OQ is moot. |

---

## 10. What this feeds

- **`Q5_full_edit_list.md`** — resolves the "TARGET = placeholder" note (Q5 line 11): **TARGET = CEF 150 / Chromium 150 / branch 7871 (the future M150 LTS line; adopted at ≥ CEF-Stable per the §3 gate, ridden into LTS).** The `Process`-layer rows (toolchain re-pin, minos, dep-verify, manifest diff) are §4.3/4.4/4.6 here.
- **`IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`** — **P1 = this bump** (stock `7871`, unchanged flags, toolchain re-pin, minos raise, dep-verify, regression). **P2 = codec/farbling/DRM edits** ride on the P1 binaries. Signer-continuity gate (§8) is a release-gate row.
- **`CEF_BUILD_RUNBOOK.md` / `CEF_VERSION_UPDATE_TRACKER.md`** — after execution, update the branch table (7103→7871), the macOS floor (11.0→12.0), and append the milestone-jump findings (Invariant #12).

---

## Sources (primary, verified 2026-07-10)

- CEF branch/LTS policy + branch table: https://chromiumembedded.github.io/cef/branches_and_building.html
- CEF automated LTC/LTS builds (M138 start, 6-month cadence, ~8-mo security): https://github.com/chromiumembedded/cef/issues/3947
- CEF 2-week-cadence adjustments (Sept 2026; LTC/LTS ~9-mo lifespan unaffected): https://github.com/chromiumembedded/cef/issues/4114
- Chromium 150 upstream-stable (≈ late June 2026): https://chromereleases.googleblog.com/2026/
- Chrome two-week release cycle (Sept 2026): https://developer.chrome.com/blog/chrome-two-week-release
- M150 = last macOS Monterey; M151 requires Ventura: https://www.superchargebrowser.com/library/chrome-150-macos-monterey-end-support-2026/
- CEF build discovery file: https://cef-builds.spotifycdn.com/index.json
