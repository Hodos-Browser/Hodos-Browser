# IMPLEMENTATION ROADMAP — Chromium/CEF Rebuild → v0.4.0-beta.1

**Created:** 2026-07-10 (rebuilt from the completed plan set) · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude · **Mac:** coordinates via `CHROMIUM_BUILD_RELAY.md`
**Status:** MASTER ROADMAP — Workflow-2 synthesis over **all eleven** now-authored plan docs. Research + design only — **NO code, NO builds.** This doc sequences the plan; the implementing sessions execute it phase by phase.

> **What this is.** The single phase-ordered execution plan for the whole rebuild sprint:
> **P0 provision → P1 pin version/toolchain → P2 baseline build (136 guarded, then bump to TARGET) → P3 CEF patch toolchain → P4 Blink farbling (incremental) → P5 codecs/DRM verify → P6 test (incl. a real N-1→N auto-update apply with signer-continuity) → P7 prod build → gate `v0.4.0-beta.1`.**
> Every phase cites its detailed plan doc, its entry/exit criteria, and its Windows/Mac owner. **Patch toolchain (P3) MUST precede farbling (P4)** — it is the serial linchpin. The edit inventory this roadmap sequences (edit IDs GN-\*, CEF-\*, C1–C7, TD-\*, BOT-\*, DRM-\*, DEP-\*, VER-\*, UPD-\*, FEDCM-\*) lives in `chromium-rebuild/Q5_full_edit_list.md`.

> **⭐ What changed since the prior roadmap draft.** The prior draft flagged `PLAN_farbling_blink.md` and `Q3` as **unwritten** and carried `TARGET` as a placeholder. **All eleven plan docs now exist.** This rebuild:
> 1. **Resolves TARGET** (`PLAN_version_bump.md`): **CEF 150 / Chromium 150 / branch `7871`** (ride into the M150 LTS line), **fallback = current CEF-Stable M149 / branch `7827`** if `7871` is still CEF-Beta on build day. macOS floor rises **11.0 Big Sur → 12.0 Monterey**.
> 2. **Fills C1–C7 + P4e** from `PLAN_farbling_blink.md` (Supplement + browser-side-HMAC off-cmdline seed) and `Q3_farbling_oauth.md` (C7 `ShouldFarble`, browser-side membership test).
> 3. **Resolves the value table** — only WebGL vendor/renderer stays owner-sign-off-pending (recommended: **drop**).
> 4. **Elevates the SIGNING-IDENTITY / SIGNER-CONTINUITY gate** to a first-class, testable readiness item (it was missing from the earlier checklist).

> **Source docs (authoritative):**
> - Outline: `0.4.0/CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` (§2 version, §3 edits, §4 phases, §5 ownership, §7 checklist, §8 open Qs)
> - **Edit inventory (backbone):** `chromium-rebuild/Q5_full_edit_list.md`
> - Version target + bump mechanics + signer gate: `chromium-rebuild/PLAN_version_bump.md`
> - Patch toolchain: `chromium-rebuild/PLAN_patch_toolchain.md`
> - Blink farbling C1–C7: `chromium-rebuild/PLAN_farbling_blink.md`
> - Codecs: `chromium-rebuild/PLAN_codecs.md` · Dependencies: `chromium-rebuild/PLAN_dependencies.md`
> - Build→test→prod pipeline + auto-update/signer gate: `chromium-rebuild/PLAN_build_test_prod.md`
> - Q1 Mac farbling: `chromium-rebuild/Q1_mac_farbling.md` · Q2 farbling×adblock: `chromium-rebuild/Q2_farbling_adblock.md` · Q3 farbling×OAuth: `chromium-rebuild/Q3_farbling_oauth.md` · Q4 Widevine/DRM: `chromium-rebuild/Q4_widevine_amazon_drm.md`
> - DevOps P&P: `DevOps-CICD/CEF_BUILD_RUNBOOK.md`, `CEF_VERSION_UPDATE_TRACKER.md`, `DEPENDENCY_VERIFICATION.md`, `SILENT_UPDATE_TEST_PLAN.md`, `WINDOWS_AUTOUPDATE_PLAN.md`, `AUTO_UPDATE_AND_SIGNING_0_4_0.md`, `ORG_IDENTITY_SIGNING_MIGRATION.md`, `research/BRAVE_FORK_FEASIBILITY.md`
> - Build scripts: `scripts/build_hodos_cef.bat` / `scripts/build_hodos_cef_mac.sh` (**not yet checked in — OQ-1 in `PLAN_patch_toolchain.md`**)

---

## TARGET — resolved (was a placeholder) — `PLAN_version_bump.md`

| Item | Value | Caveat / source |
|---|---|---|
| **Bump** | CEF 136 / branch `7103` → **CEF 150 / Chromium 150 / branch `7871`** | version §0/§1 |
| **Channel intent** | Ride `7871` into the **M150 LTS** line (LTS milestones M138 → M144 → M150 → M156; ~8–9 mo **platform-agnostic-only** security coverage) | LTS program confirmed from CEF `branches_and_building.html` + issues #3947/#4114 |
| **⛔ Build-day channel gate (build-blocking)** | Build **only if `7871` has reached ≥ CEF-Stable** (Stable/LTC/LTS). On the verification date it was **CEF Beta** — do not ship a money-handling browser off a Beta binary | version §3 step 2 |
| **Fallback** | If `7871` is still Beta and cannot be waited out → pin **current CEF-Stable M149 / branch `7827`** | version §2 / OQ-7 |
| **macOS floor** | **11.0 → 12.0 Monterey** (M150 is the last Chrome to support Monterey; M151 needs Ventura). A published-min raise that **gates** (not crashes) sub-floor updates → announce in release notes | version §4.4 / §5 |
| **Toolchain** | MSVC v143/VS2022 family expected; **confirm the exact Windows SDK `7871` needs** — may exceed `windows-2022` (OQ-6) | version §4.3 |
| **Signer continuity** | Win Authenticode **CN = `Marston Enterprises`** unchanged; mac **Team ID** unchanged (org migration pending — sequence per P1 Step 6) | version §8 |

Every "TARGET" below means **branch `7871` (fallback `7827`)**. Re-confirm the numbers the day the build starts (version §3).

---

## 0. Phase map at a glance

```
P0 PROVISION ─▶ P1 PIN VERSION/TOOLCHAIN ─▶ P2 BASELINE BUILD ─▶ P3 PATCH TOOLCHAIN ─▶ P4 FARBLING ─┐
 (build host:     (Step-0 version resolve      (P2a 136 baseline,     (CEF-1..5: fork,     (C1..C7,     │
  150GB+/32GB+/     CEF150/7871 + fallback       GUARDED; then          patch.cfg, no-op     TD-1..5,     │
  depot_tools/      7827; VER-2/3 toolchain+      P2b BUMP to 7871:      patch proves out)    BOT-1, P4e   │
  sccache;          runner pin; VER-4 minos       VER-1..6, DEP-1a..d + │                     incremental)│
  M136-builds?)     plan; signer sequencing)      DEP-1, FEDCM-1,       └─▶ P5 CODECS/DRM ───────────────┤ (∥ P4; forks off P2b — NOT gated by P3)
                                                  GN-5..8, VER-5 drift)     (GN-5..8 re-verify + DRM-1)  │
                                                                                                          ▼
                                                             P6 TEST (farbling acceptance + Q2 T1–T8 + Q3 T1–T10
                                                             + codec smoke + DRM + minos guard + FedCM + parity
                                                             + REAL N-1→N auto-update apply w/ SIGNER CONTINUITY — BOTH OS)
                                                                          │
                                                             P7 PROD BUILD ─▶ stage to cef-binaries release
                                                                          │
                                                             [GATE] v0.4.0-beta.1 readiness checklist (§ below)
```

> **Phase-label mapping.** This roadmap uses the sprint's P0–P7 labels. They map onto the plan docs' phases as: **P1 (pin) = outline/PLAN §"P0 Step 0" version resolution + toolchain/runner/minos pinning**; **P2 (baseline build) = docs' P1 baseline (=P2a) + P2 bump (=P2b)**. Everything downstream (P3–P7) matches the docs 1:1.

**Serial linchpins:** P0→P1→P2 strictly serial. `CEF-1` (patch toolchain) blocks **all** of C1–C7. `C1` blocks C2–C7 + P4e. `C2` gates C7 (the `farble_enabled` bit rides C2's payload — R2 fork) and gates the TD-3 seed-IPC deletion. **P4 ∥ P5** (farbling is independent of codec/DRM; P5 forks off **P2b**, NOT P3). P6 gates P7. Do **not** run F4/parking_lot concurrently with the farbling patch set (both L/XL cross-cutting). Cold build ≈ **10–12 hr per OS**, **no** sccache benefit on cold builds; universal2 Mac = two per-arch builds + `lipo`.

---

## P0 — PROVISION *(blocks everything)*

**Plan docs:** `PLAN_build_test_prod.md` §2; `CEF_BUILD_RUNBOOK.md` Step 3.
**Edit IDs staged here:** none (infrastructure).

**Steps**
1. Provision the **self-hosted build host(s), per OS** (Win → `libcef.dll`; Mac → framework — separate hosts): **≥150 GB NVMe/SSD** (100 GB min; two-tree footprint if 136 + TARGET coexist = 200 GB+), **32+ GB RAM**, **8+ cores**, NTFS/APFS (never exFAT), short ASCII base `C:\cef\`. The Chromium build **cannot** run on GitHub-hosted runners (6-hr cap, ~14–29 GB free).
2. Install toolchain/tooling: VS2022 BuildTools (MSVC v143) + Win SDK + **Debugging Tools for Windows**; `depot_tools` + `automate-git.py`; branch-matched Python (re-confirm the `.vpython3` ceiling for TARGET — the M136-era 3.11 ceiling is not a carry-forward); Defender exclusions; pause Windows Update. Mac: Xcode + CLT.
3. Provision **sccache** (`cc_wrapper="sccache"`; `chrome_pgo_phase=0` auto-drops `/Brepro`). Honest expectation: **cold builds get NO benefit**; MSVC/Windows historically yields few cache hits. Local disk for beta.1; S3 later.
4. **TARGET default build-tool lookup (Ninja vs Siso).** Siso is now Chromium's default and its cold-resume differs from the `.ninja_log` path the M136 build relied on. Verify Siso resume/cache **or** set `use_siso=false` + confirm Ninja still supported on `7871` (gates the spot-vs-persistent-host decision — **default to a persistent/owned host** until resumability is proven).
5. **OQ-7 / M136-still-builds confirmation.** Verify M136 still `gclient sync`s and builds on the pinned toolchain. If **bit-rotted** (deprecated sysroots/CIPD), **downgrade P2a** from a full cold build to a smoke of the last-known-good environment.

**Entry:** none. **Exit:** both hosts to spec; toolset noted; `gclient --version` sane; sccache backend chosen; TARGET build-tool resumability answered; M136-still-builds answered (full baseline vs last-known-good smoke decided).
**Ownership:** **Windows = LEAD** (Win host). **Mac** owns Xcode/clang host provisioning.

---

## P1 — PIN VERSION / TOOLCHAIN *(blocks P2)*

**Plan docs:** `PLAN_version_bump.md` §1/§2/§3/§8; outline §2 Step 0; `CEF_VERSION_UPDATE_TRACKER.md`.
**Edit IDs:** version-target decision (→ VER-1), VER-2 (build-host toolchain), VER-3 (CI runner pin), VER-4 (minos **scoping**).

**Steps**
1. **Step-0 version resolution from PRIMARY sources** (not wikis/seed): confirm **M150 = branch `7871`** on `branches_and_building.html` + `cef-builds.spotifycdn.com/index.json`; confirm the **LTC/LTS program is real** (resolves outline C1 — it is; M138/144/150/156, ~8–9 mo platform-agnostic-only); pin to the newest security point-release of `7871` (not `.0`); record the toolset (MSVC/Clang + Windows SDK) and macOS floor (12.0). **Record the LTS-vs-stable decision** (default: ride `7871` into M150 LTS; fallback M149/`7827`) with support-end date in `CEF_VERSION_UPDATE_TRACKER.md`.
2. **VER-2 build-host toolchain pin:** provision the self-hosted host's MSVC/Clang + Windows SDK to the toolset `7871` was built with (ABI contract).
3. **VER-3 CI app-build runner pin:** `runs-on:` in `release.yml` — **never `*-latest`** (`windows-2022`/`macos-15` or a deliberately-validated newer pin) so the CI compiler **matches the CEF binary's toolset** (ABI-critical match = CEF-binary ↔ `cef-native`/wrapper, NOT the Chromium-build runner). **Re-validate the pin ships the SDK `7871` needs (OQ-6).**
4. **VER-4 minos plan (scoping only; executed in P2b):** record the target macOS floor (12.0); plan the three-place min-version edits + CI minos guard.
5. **Signer-identity sequencing (decide NOW, not at gate time).** beta.1 may be the first *signed* 0.4.0 build. Decide whether the Apple individual→org migration (`ORG_IDENTITY_SIGNING_MIGRATION.md` — itself a reinstall-forcer) lands **before** the P7 prod build (recommended **(A) migrate-first, conditional on confirming Team ID is preserved**) or is **deferred past beta.1 (B)**. Windows CN is already `Marston Enterprises`; only the mac Team ID needs the pre-/post-check. Record the decision so the P6 auto-update gate tests the right identities and isn't masked by a shared dev cert.

**Entry:** P0 exit. **Exit:** TARGET version + branch + milestone recorded; LTS-vs-stable + build-day channel-gate rule logged; build-host toolchain + CI runner pinned (no `*-latest`); minos plan written; signer-migration sequencing decided + recorded.
**Ownership:** **Windows = LEAD** (version research + decision, Win toolchain/runner). **Mac** owns `macos-NN` runner pin + macOS-floor lookup for VER-4.

---

## P2 — BASELINE BUILD *(P2a 136 guarded, then P2b bump to TARGET; blocks P3+P5)*

**Plan docs:** `PLAN_build_test_prod.md` §3/§4.1; `PLAN_version_bump.md` §4; `PLAN_codecs.md`; `PLAN_dependencies.md`; `CEF_BUILD_RUNBOOK.md` Step 5.5.
**Edit IDs:** **P2a** GN-1..GN-4 (codec carry-forward, no new patches). **P2b** VER-1 (branch), VER-2/3 (applied), VER-4 (minos exec), VER-5 (drift audit), VER-6 (version single-source); DEP-1a..d (silent-drift re-pins) then DEP-1; FEDCM-1; GN-5..GN-8 re-verify.

**P2a — 136 baseline (guarded, partial isolation only)**
1. From-source Release build on **current `--branch=7103`**, `GN_DEFINES` **byte-identical** (`is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0`).
2. Verify Output-file checklist; run `PLAN_codecs.md` Layer-A `canPlayType` probe + Layer-B smoke → record the **codec baseline** (and HEVC inherited-on, non-gating) for the P6 regression comparison.
3. **Guarded fallback (I5):** if P0 Step 5 found M136 bit-rotted, run the last-known-good-environment smoke instead — do not treat an unbuildable M136 as an unmeetable gate. *(Consequence: no numeric 136 baseline → P6 crash-rate + perf gates fall back to **the current shipping public M136 build** telemetry or absolute thresholds; state which in the P2b write-up — I2.)*
> **Honesty caveat:** P2b changes version **and** toolchain together, so P2a→P2b isolation is only **partial**. State plainly in the changelog. P2a's value is proving *our glue* works before moving the engine.

**P2b — bump to TARGET `7871`**
1. **Build-day channel gate:** confirm `7871` ≥ CEF-Stable, else fall back to `7827` (record).
2. **VER-1:** `--branch=7103` → `--branch=7871` in **both** build scripts; `gclient sync`.
3. **Pre-build GN-args check** (`PLAN_codecs.md` §7; version §4.2): `gn args --list` confirms `proprietary_codecs=true`, `ffmpeg_branding="Chrome"`, `chrome_pgo_phase=0`, `enable_widevine` resolve, HEVC/AV1 derivations unchanged — **before** the 10–12 hr build (a flipped default ships a green build with no codecs).
4. **Full target build** (per OS).
5. **DEP-1a..d silent-drift re-pins FIRST (own small commits), then DEP-1:** pin vcpkg baseline (`cef-native/vcpkg.json` manifest + overrides incl. `port-version`), pin Inno Setup (`<6.7.x>`), pin macOS Homebrew (`Brewfile`), add `rust-toolchain.toml`; then the full `DEPENDENCY_VERIFICATION.md` pass (rebuild — not re-declare — the CEF wrapper + vcpkg static deps on `7871`'s toolset; hold the `adblock=0.10.3`/`rmp`/`actix-web` MSRV pins; **never silently bump wallet crypto crates — Invariant #3**). Record a "touched/deferred" table.
6. **VER-5 Step 5.5 drift audit:** diff `7871`'s CEF dist file-manifest (DLL/`.bin`/`.pak`/`resources`/`locales`) vs the hardcoded copy-lists (`cef-native/CMakeLists.txt` Win + mac framework-embed list); diff pinned `GN_DEFINES` vs new defaults. **14 milestones of drift → expect ≥1 changed resource. This audit feeds the P6 auto-update apply gate — a changed manifest is exactly what breaks a silent update.**
7. **GN-5..GN-8 re-verify:** HEVC inherited-on (non-gating), AV1 present, Dolby off, `enable_widevine` resolves.
8. **VER-4 minos exec (Mac):** `vtool`-measure framework `minos`; set published min = `max(12.0, measured)` in all three (`CMakeLists.txt` `CMAKE_OSX_DEPLOYMENT_TARGET`, `Info.plist` + `mac/helper-Info.plist.in` `LSMinimumSystemVersion`); apply via `-DCMAKE_OSX_DEPLOYMENT_TARGET=12.0` + `MACOSX_DEPLOYMENT_TARGET`; wire CI minos guard.
9. **VER-6:** confirm version single-sourcing (git tag → `cargo-release` → CMake/shadow-rs/`.iss`/TS constant) injects cleanly on the new tree.
10. **FEDCM-1:** audit `CefPermissionHandler` FedCM coverage (on-by-default since ~M108 → already live on M136; re-verify on `7871`); scope a shell edit if the permission API changed. *(Note Q3 §2.6: FedCM is browser-native UI, no farblable JS surface — do NOT add IdP origins to the C7 allowlist.)*
11. Rebuild the CEF wrapper + `cef-native` against new headers (no "Unsupported CEF version").

**Entry:** P1 exit. **Exit:** (P2a) clean 136 baseline + codec baseline, or documented last-known-good smoke; (P2b) clean `7871` build both OS; channel gate satisfied; GN args resolve; DEP-1a..d pinned + DEP-1 pass with touched/deferred table; VER-5 drift audit clean + human-reviewed + copy-lists updated; minos aligned + guard green; FedCM audited; wrapper + `cef-native` compile.
**Ownership:** **Windows = LEAD** (VER-1/2/3/5/6, DEP-1 vcpkg-heavy, FEDCM-1, Win builds). **Mac owns entirely:** VER-4 minos/`vtool`/plist/guard, DEP-1c Brewfile, mac framework-embed list, its own baseline + target builds.

---

## P3 — CEF PATCH TOOLCHAIN *(GREENFIELD — serial linchpin; blocks P4)*

**Plan docs:** `PLAN_patch_toolchain.md` (full); outline §3b/§4 P3.
**Edit IDs:** CEF-1 (fork + patch.cfg + `automate-git --url` + no-op probe), CEF-2 (`cef_patch_drift_audit.py` Step-5.5 hook), CEF-3 (upstream security-pull duty / fork-watcher), CEF-4 (single `HODOS_FARBLING` `condition` gate), CEF-5 (check `build_hodos_cef.{bat,sh}` into `scripts/` + `HODOS_PATCHES.md` ledger — resolves OQ-1).

**Steps**
1. **CEF-1:** fork `chromiumembedded/cef` → **`Hodos-Browser/cef`**, branch `hodos/7871`; add `patch/patches/hodos_*.patch` + register in `patch/patch.cfg`; point the build at the fork via `automate-git.py --url=https://github.com/Hodos-Browser/cef.git --branch=7871 --checkout=<pin>`. Patches apply via `git apply -p0 --ignore-whitespace` (**exact-context, fail-loud, no fuzz** — a context mismatch aborts before compile). **Prove a no-op probe patch applies pre-compile + builds**, then remove it and re-verify the count returns to the stock upstream baseline. Clean-dir caveat: `automate-git` refuses a URL switch on an existing CEF checkout — remove the CEF sub-dir first (not `chromium_git/`).
2. **CEF-4:** wire the single `HODOS_FARBLING` `condition` gate (all-or-nothing; never half-apply the set); prove toggle applied↔skipped, never failed. Escape hatch for beta.1 stability.
3. **CEF-2:** land `scripts/cef_patch_drift_audit.py` — read-only per-patch `git apply --check` (**never** write-capable `patch_updater.py --reapply/--restore`), scrape hunk-**offset** lines (soft warning), registry/orphan + target-file-existence checks, **folds VER-5 file-manifest diff + GN-args diff**. Exit 1 = build must not start; wire as a pre-build gate.
4. **CEF-3:** document + automate the recurring duty to pull upstream in-branch security point-releases into the fork (scheduled `gh`/Actions fork-watcher that opens a rebase PR when upstream `7871` advances). Record in `CEF_VERSION_UPDATE_TRACKER.md`.
5. **CEF-5:** check the two `build_hodos_cef*` scripts into `scripts/` (referenced as canonical but **absent** today); create `HODOS_PATCHES.md` fork ledger.
6. **Attachment map ready** (patch_toolchain §8.1): C1 first, then C2–C7 → `hodos_farble_{session_cache,seed_wiring,canvas2d,webgl,webaudio,navigator,auth_exempt}.patch`, all `condition: HODOS_FARBLING`, all `path: src`. C1 also patches a Blink `BUILD.gn` (its higher-churn rebase target).

> **Prerequisite authoring (no build dependency — start as early as P0).** `PLAN_farbling_blink.md` and `Q3_farbling_oauth.md` are **now written** — they no longer block P4 entry. Remaining pre-P4 design work is only the two owner value-fills (FB-1 seed channel, FB-2 WebGL vendor/renderer) and the FEDCM/OQ housekeeping; none require a build.

**Entry:** P2 exit. **Exit:** fork stands up; a no-op patch demonstrably applies + builds on both OS; `HODOS_FARBLING` toggles applied↔skipped; drift-audit script wired as a pre-build gate + scheduled fork-watcher; `HODOS_PATCHES.md` + tracker updated; `build_hodos_cef*` checked in; C1-alone can be authored→applied→built end-to-end (ready for P4a).
**Ownership:** **Windows = LEAD** authors the fork + toolchain + patch infra (shared cross-platform text). **Mac** verifies the no-op patch applies + builds through its own automate-git/framework path.

---

## P4 — FARBLING *(incremental; independent of P5 — patch toolchain MUST precede)*

**Plan docs:** `PLAN_farbling_blink.md` (full: §3 Supplement, §4 seed, §5 worker matrix, §6 files, §7 value table, §8 land order); `Q3_farbling_oauth.md` (C7); `Q1_mac_farbling.md` (Mac build/arch/GPU strings); `Q2_farbling_adblock.md` §3 (teardown hygiene TP-1/TP-2 + T1–T8); outline §3c.
**Edit IDs:** C1..C7 (+C7b fallback), P4e, TD-1..TD-5, BOT-1.

**Land order (each sub-step builds + smokes; each atomically deletes its own JS counterpart in the same commit — I-4, no double-farbling window, no guard flag):**
- **P4a — C1 `HodosSessionCache : Supplement<ExecutionContext>` + C2 seed/channel → Canvas-first worker quick-win.** Persistent per-profile `profile_seed` (32B CSPRNG via **`BCryptGenRandom`**/`SecRandomCopyBytes`, NOT deprecated `CryptGenRandom`) stored in `%APPDATA%/HodosBrowser/<profile>/fingerprint_settings.json` (NOT the wallet). **Browser process computes `domain_key = HMAC-SHA256(profile_seed, first-party eTLD+1)` and delivers ONLY `{domain_key, farble_enabled}` to the renderer — master seed never leaves the browser** (supersedes B1-design's renderer-HMAC). Ship the Supplement wired to **Canvas (C3) only** first (highest-signal detection fix; closes the window-vs-worker mismatch for canvas). Delete the JS **canvas** fragment this same step. **C2 delivery channel = OPEN (FB-1):** default **(A) mojo / commit-params per-navigation**; fallback **(B) ephemeral per-launch nonce on cmdline** (a throwaway, not the seed).
- **P4b — C4 WebGL incl. `readPixels` (its OWN patch point) + resolve FB-2** (WebGL vendor/renderer drop-vs-map). Delete JS WebGL fragment.
- **P4c — C5 WebAudio + C6 Navigator (valid-set constrained — §B) + BOT-1.** deviceMemory **∈ {4,8,16,32}** (FB-8), hardwareConcurrency **reduce-only ≤ real cores** (FB-7), plugins realistic 5-PDF set. **BOT-1:** re-home `navigator.webdriver=false` + keep `window.chrome` stub (`:629-653`) byte-identical — bot signals, not farbling; must survive teardown. Delete JS audio fragment.
- **P4d — C7 auth-domain exemption at the BROWSER process** (`Q3` — supersedes outline C7's "list passed to renderer"). One `ShouldFarble(top_frame) = GlobalEnabled && !IsAuthDomain(top_frame_HOST) && IsSiteEnabled(top_frame_eTLD+1)`; allowlist match on the **full committed top-frame host** (OQ3 — do NOT collapse to eTLD+1); registrable domain used only for the seed key. Delivers the single `farble_enabled` bit **alongside C2's `{domain_key}` payload** (no new IPC **iff** C2 = per-navigation channel — R2 fork). Structurally fixes the Turnstile parent/iframe inconsistency. **TD-4** migrate `IsAuthDomain` here; **TD-5/C7b** re-home the per-site user toggle (`IsSiteEnabled`) into `ShouldFarble` (owner sign-off; C7b sibling if C7 kept minimal). JS FP block now fully torn down → **M1 complete**.
- **P4e — OOP seed/exemption plumbing:** deliver top-frame-derived `{domain_key, farble_enabled}` to **shared workers, service workers** (key = registration-scope eTLD+1, FB-3), **and cross-site (OOP) iframes** at subframe navigation commit. Audio/paint worklet + OffscreenCanvas-in-worker are in-process (free once C2 lands, but tested). Needs a purpose-built worker-parity harness (CreepJS only exercises the dedicated-worker column).

**Teardown (M1 — retire, don't orphan; atomic per-value):** TD-1 delete JS FP block `simple_render_process_handler.cpp:581–627` (working-tree 2026-07-10; outline's `:586-632` is stale — reconcile at edit time), keep adjacent scriptlet `:567–579` byte-identical; TD-2 retire `FingerprintProtection.h`/`FingerprintScript.h` JS-injection parts; TD-3 remove FP seed caches/IPC **only after C2 channel verified delivering per-domain seeds (P4a smoke green)** — a design-choice-only deletion would strand the renderer with a constant/absent seed. **TD-5 stays gated** until `ShouldFarble` consumes `IsSiteEnabled` (Q2 T8) — do NOT delete the toggle first.

> **Per-teardown adblock smoke (M5):** the full Q2 T1–T8 suite runs in P6, but TD-1 edits the file the adblock scriptlet block lives in — add a **one-line smoke at each teardown sub-step** ("adblock still cancels a blocked request + a scriptlet still fires + YouTube `AdblockResponseFilter` intact") so a regression surfaces at land time.

> **Clean-room (M7):** re-implement Brave's *technique* only — read behavior/spec (fingerprinting-defenses blog, CreepJS expectations, this plan's value tables), **not** Brave's MPL-2.0 source. Bromite (GPL-3) FORBIDDEN. Record the clean-room boundary in each PR.

**Entry:** P3 exit (fork + no-op patch builds; farbling design docs authored). **Exit:** C1–C7 + P4e land + build on both OS; TD-1..TD-4 + BOT-1 complete with no orphaned symbols (Q2 T8, scoped to fully-retired symbols until TD-5 re-homes the toggle); farbling co-exists with adblock (Q2 T1–T8, run in P6).
**Ownership:** **Windows = LEAD** authors the shared C1–C7 patch set + teardown + P4e design + C2 shell wiring (`ProfileManager`/`SettingsManager`). **Mac** inherits the patches; owns its build/behavior, the **arm64/x64/universal2** arch decision (default universal2 = two per-arch builds + `lipo`), the **Mac GPU-string entries** for C4 if FB-2 = map (Apple Silicon + Intel ANGLE), the C2 platform conditionals in `cef_browser_shell_mac.mm`, and OOP-context verification on the framework.

---

## P5 — CODECS / DRM *(parallel-ok with P4; forks off P2b, NOT P3; gates P6)*

**Plan docs:** `PLAN_codecs.md` (§6 smoke matrix, §7 procedure); `Q4_widevine_amazon_drm.md` (§7 Spike-1).
**Edit IDs:** GN-5..GN-8 re-verify on target; DRM-1 (Spike-1 free component-updater CDM test); DRM-2 **DEFER**; DRM-3 optional/defer.

> **Clarification:** codecs are always-on GN flags compiled into the **same tree as farbling** — there is no separate codec build. P5 = **verify codecs + test DRM on the binary P2b/P4 produced**. "P4 ∥ P5" = the verification overlaps the farbling landing; it does **not** mean a second concurrent 10–12 hr cold build.

**Steps**
1. **Codec re-verify (`PLAN_codecs.md` §7):** Layer-A `canPlayType` (H.264 baseline `avc1.42E01E` + High `avc1.640028`, AAC `mp4a.40.2`, MP3, VP9 = **`'probably'` GATE**; AV1 assert present; HEVC record-only non-gating; Dolby out) + Layer-B real-playback smoke (YouTube, x.com incl. animated-GIF-as-MP4, Reddit, Twitch, LinkedIn, an audio site). A `""` on any GATE row = codec regressed → **block the bump**, re-audit `args.gn`.
2. **DRM-1 Spike-1 (~1 hr, $0):** Step 0 = audit our own build for CDM suppression (`--disable-component-update`, `*.googleapis.com` blocklist — could moot the VMP thesis); force CDM download (`--component-updater=fast-update`); confirm it **loads** (not just downloads, cf. #3820); classify EME-resolve vs license-refused; run the Amazon(primary)/Netflix/Bitmovin/YouTube/Spotify matrix; **answer whether a VMP `.sig` is required even for L3 on Windows (I6)**; compare Brave (VMP-signed) on the same title.
3. **DRM decision:** free path plays Amazon acceptably → keep + document. Amazon needs VMP (expected) → **DEFER DRM-2** (VMP signing: Google MLA free-ish/slow, or castLabs paid 3PL — castLabs free EVS is Electron-only, cannot sign our CEF) out of beta.1; ship CDM auto-download + honest limitation note; open post-beta.1 `VMP_SIGNING_SPIKE.md`. **Do NOT build the Brave-style consent prompt (DRM-3, cosmetic).**

**Entry:** P2b exit (target build exists). Runs in parallel with P4. **Exit:** codec Layer-A/B gates pass both OS; Spike-1 evidence for (i) CDM loads, (ii) precise Amazon failure class incl. the VMP-`.sig`-for-L3 answer, (iii) works/breaks site list; DRM defer-vs-keep recorded in `CEF_VERSION_UPDATE_TRACKER.md`.
**Ownership:** **Windows = LEAD** (codec + DRM write-up, Win spike). **Mac** runs its own codec smoke + DRM Spike-1 (mac VMP path TBD, not 1:1).

---

## P6 — TEST *(blocks P7)*

**Plan docs:** `PLAN_build_test_prod.md` §7; `Q2_farbling_adblock.md` §4 (T1–T8); `Q3_farbling_oauth.md` §6 (T1–T10); `PLAN_codecs.md` §6; `PLAN_farbling_blink.md` §11; `SILENT_UPDATE_TEST_PLAN.md`; `ORG_IDENTITY_SIGNING_MIGRATION.md`.

**Test suites (all on BOTH Windows and macOS):**
1. **Codec smoke** — Layer-A gate rows `'probably'` + AV1 present; Layer-B six sites; HEVC recorded non-gating.
2. **Farbling acceptance:** CreepJS zero "lies" (`.toString()` → `[native code]`); **worker column == window column** incl. **service-worker, shared-worker, OffscreenCanvas-in-worker** (purpose-built harness — CreepJS only does the dedicated-worker column); intra-session consistency (same read twice → identical); cross-profile difference; cross-site iframe difference (first-party keying, P4e); **cross-session login test** (create account → restart → revisit → logins don't break — the whole reason for persistent-over-per-session); navigator values in valid set; WebGL vendor/renderer decision applied; C7 OAuth exemption verified; **no persistent seed on any renderer cmdline** (ProcessExplorer/`ps`); **escape-hatch works** (a `HODOS_FARBLING`-off build ships farbling-disabled).
3. **Farbling × adblock (Q2 T1–T8):** adblock block still cancels; scriptlet + cosmetic fires after FP teardown; YouTube `AdblockResponseFilter` intact; farbling+adblock same session; canvas-touching scriptlet double-wrap tolerated; **T6 `[native code]` toString GATE**; auth-domain exemption clean (no double source); **T8 no orphaned FP symbols** (scoped until TD-5 re-homes the toggle).
4. **Farbling × OAuth (Q3 T1–T10):** exempt auth sites log in (T1); **T2 hard-bypass native-value equality = SOLE proof of a live exemption**; CAPTCHA on non-exempt (T3) + exempt (T4) parents; **T5 cross-site-iframe consistency (R2 gate)**; user per-site toggle survives (T7); global toggle survives (T8); no orphaned exempt symbols (T9); login persistence across restart (T10).
5. **Stability soak + crash-rate gate** vs the 136 baseline (**or the current shipping public M136 build** if P2a fell back to smoke — I2).
6. **Canvas/WebGL performance-regression gate** (`readPixels`/`getImageData` within budget; same baseline caveat).
7. **macOS minos guard GREEN** + manual relaunch-after-update on a machine at/near the 12.0 floor.
8. **⭐ REAL N-1 → N silent auto-update apply + relaunch on BOTH OS** — reuse the proven `SILENT_UPDATE_TEST_PLAN.md` Stage-1 rigs + Stage-2 (dev wallet) + Stage-3 (prod-signed, **trivial-balance** wallet — OQ-4: no funded wallet). N carries the new CEF manifest + new minos + new framework layout, tied to the VER-5 drift audit. **No proxies.** Broken-N rolls back wallet-intact. **Signer-continuity verified in every leg (see readiness checklist below).**
9. **FedCM** ("Sign in with Google" account chooser) works.
10. **Regression basket** (CLAUDE.md Testing Standards, **Thorough** tier): Auth, Video/Media, News, E-commerce, Productivity, BSV — both OS.
11. **Wallet send/receive + CWI shim intact** (BRC-121 test site) — the build didn't disturb the money path.

**Entry:** P4 exit (farbling lands) + P5 exit (codec/DRM verified). **Exit:** every readiness item green on both OS; results reconciled in `CHROMIUM_BUILD_RELAY.md`.
**Ownership:** **Windows = LEAD** (write-up + reconciliation). **Mac** runs its own full suite; owns the minos-guard relaunch + Sparkle update-apply leg.

---

## P7 — PROD BUILD *(gated by P6)*

**Plan docs:** `PLAN_build_test_prod.md` §6; `CEF_BUILD_RUNBOOK.md` Step 4/6/7; `BUILD_AND_RELEASE.md`; `release.yml`.

**Steps**
1. Official Release build both OS on the pinned toolchain, signed with the identity resolved in P1 Step 5 — confirm the signer **matches what the P6 signer-continuity gate was tested against** (no last-minute cert swap). Back up the current `cef-binaries/Release` first.
2. Stage binaries to the **`cef-binaries` GitHub release** the Tier-2 pipeline consumes; rebuild `libcef_dll_wrapper` + `cef-native` against the staged binaries (no "Unsupported CEF version").
3. Confirm `release.yml` consumes cleanly (draft-first → manual promote gate → website deploy). Keep the M136 `cef-binaries` tag live as the rollback artifact until beta.1 soaks.
4. Append the changelog to `CEF_VERSION_UPDATE_TRACKER.md` (branch, milestone, `GN_DEFINES`, patch-set version, deps touched/deferred, duration, **estimated per-bump patch-rebase hours — I10**).

**Entry:** P6 all-green. **Exit:** reproducible prod build both OS staged to `cef-binaries`; Tier-2 pipeline green; tracker changelog appended.
**Ownership:** **Windows = LEAD** (Win prod build + `cef-binaries` staging + tracker). **Mac** produces + stages its own framework build (first-class parallel).

---

## [GATE] v0.4.0-beta.1

All readiness-checklist items green on both OS → cut `v0.4.0-beta.1`. **Fallback if TARGET destabilizes at gate time (M5/M13):** documented rollback to the M136 (or previous) branch — not just toggling farbling off via the `HODOS_FARBLING` `condition` gate.

---

## Dependency ledger — what blocks what

| Phase / edit | Blocked by | Blocks |
|---|---|---|
| **P0** provision | — | all |
| **P1** pin (VER-2/3, VER-4 scope, signer sequencing) | P0 (host + M136-builds answer) | P2 |
| **P2a** 136 baseline (GN-1..4) | P1 | P2b |
| **P2b** bump (VER-1..6, DEP-1a..d + DEP-1, FEDCM-1, GN-5..8, VER-5 drift) | P2a | **P3, P5** |
| **P3** CEF-1 patch toolchain | P2 | **all of C1–C7** |
| CEF-2 drift hook / CEF-4 `condition` gate / CEF-5 scripts-checkin | CEF-1 | P4 hygiene |
| **C1** Supplement | CEF-1 | C2–C7, P4e |
| **C2** seed channel (FB-1) | C1 | **C7** (bit rides C2 payload — R2 fork), **TD-3** (seed-IPC deletion) |
| **C7** (`ShouldFarble` / `IsAuthDomain`) | C2 + Q3 | TD-4 |
| **TD-5 / C7b** per-site toggle re-home | `ShouldFarble` consuming `IsSiteEnabled` | (must NOT delete toggle until this lands — Q2 T8) |
| **P4e** OOP workers + cross-site-iframe | C1, C2 | full worker/iframe acceptance in P6 |
| **P5** codecs/DRM | **P2b** (NOT P3) | P6 (∥ P4) |
| **DRM-2** VMP | DRM-1 result + owner $ | **DEFERRED — post-beta.1** |
| **DEP-1a..d** re-pins | — | land before DEP-1 |
| **P6** test | P4 + P5 | P7 |
| **UPD-1/UPD-2** update-apply + signer continuity | P6, VER-5 | [GATE] |
| **P7** prod build | P6 all-green | [GATE] beta.1 |

**Cross-cutting serialization warning:** do not run F4 (parking_lot) and the FEAT-B1 farbling patch set concurrently — both L/XL cross-cutting.

---

## Windows-LEAD / Mac-owns ownership matrix

**Core principle:** the source edits are **one cross-platform patch set + one shared GN config**, but the **build is a full, first-class, separate effort per OS** (Win → `libcef.dll`; Mac → `Chromium Embedded Framework.framework` with its own Xcode/clang build, signing, packaging, notarization — DLLs are not reusable on Mac). **Mac is a parallel build, not an inherit-and-verify afterthought (I8).**

| Phase | Windows (LEAD) | Mac (owns) |
|---|---|---|
| **P0** provision | Win build host + tooling; TARGET build-tool lookup; M136-builds check | Xcode/clang host + tooling |
| **P1** pin | version-target research + decision; VER-2 Win toolchain; VER-3 Win runner pin; signer sequencing | `macos-NN` runner pin; macOS-floor lookup for VER-4 |
| **P2** baseline+bump | VER-1/2/3/5/6; DEP-1 (vcpkg-heavy) + DEP-1a/b/d; FEDCM-1; Win baseline + target builds | **VER-4 minos/`vtool`/plist/guard entirely**; DEP-1c Brewfile; mac framework-embed list; own baseline + target builds |
| **P3** patch toolchain | fork + patch.cfg + automate-git + no-op patch + drift audit + `condition` gate + scripts-checkin (shared) | verify no-op patch applies + builds through framework path |
| **P4** farbling | authors C1–C7 + teardown + BOT-1 + P4e design + C2 shell wiring | **arm64/x64/universal2 arch decision; Mac GPU strings for C4 (if FB-2=map)**; C2 platform conditionals in `cef_browser_shell_mac.mm`; OOP verify on framework |
| **P5** codecs/DRM | codec + DRM write-up; Win spike | own codec smoke + DRM Spike-1 (mac VMP TBD) |
| **P6** test | leads write-up + reconciliation; Win full suite + Win update-apply leg | own full suite + minos-guard relaunch + **Sparkle update-apply leg** |
| **P7** prod | Win prod build + `cef-binaries` staging + tracker | own framework build + staging |

**Coordinate via a new `CHROMIUM_BUILD_RELAY.md`** (or an extension of `MAC_WINDOWS_RELAY.md`).

---

## §7 — v0.4.0-beta.1 READINESS CHECKLIST

Concrete, testable gate items — **all green on both Windows and macOS** before cutting the tag. Each ties to a phase.

**Build integrity (P1/P2/P7)**
- [ ] Target = **CEF 150 / branch `7871`** confirmed from `index.json`; **`7871` is ≥ CEF-Stable on build day** (NOT Beta) OR the recorded fallback to M149/`7827` is in effect; **LTS-vs-stable decision recorded** with the Extended-Stable-conflation hypothesis tested (C1); cadence corrected (4-week, I13).
- [ ] Target branch confirmed on **ACTIVE security support**; support-end date + in-flight point-release cadence recorded in `CEF_VERSION_UPDATE_TRACKER.md` (I12); **CEF fork tracks upstream in-branch security point-releases (CEF-3 fork-watcher live)**.
- [ ] Build is **buildable/repeatable** from `scripts/build_hodos_cef.{bat,sh}` (now checked in — CEF-5) on the pinned toolchain; changelog appended (branch, milestone, GN_DEFINES, patch-set version, deps, duration, **per-bump patch-rebase hours — I10**).
- [ ] **CI app-build runner images pinned** (`runs-on:` — no `*-latest`) so MSVC/Clang **match the CEF binary's toolset** (ABI, I9); build-host toolchain documented separately; **Windows SDK the runner ships covers what `7871` needs (OQ-6)**.
- [ ] **DEP-1a..d silent-drift re-pins landed** (vcpkg baseline manifest, Inno `<6.7.x>`, Brewfile, `rust-toolchain.toml`); DEP-1 pass with touched/deferred table; **no silent wallet-crypto-crate bump (Invariant #3)**.
- [ ] **VER-5 Step 5.5 file-manifest + GN-args drift audit** produced a clean human-reviewed diff; `cef-native` copy-lists updated; all output-checklist files present (`libcef.dll`, `icudtl.dat`, `v8_context_snapshot.bin`, `resources/`, `locales/`, …).
- [ ] Wrapper + `cef-native` rebuilt against new headers (no "Unsupported CEF version").
- [ ] Binaries staged to the **`cef-binaries` GitHub release**; Tier-2 `release.yml` consumes them; M136 `cef-binaries` tag kept live as rollback artifact.
- [ ] **Fresh-install smoke (M6):** clean first-install on a machine with no prior Hodos data — installer runs, app launches, wallet setup reachable — both OS.

**Auto-update apply — the highest reinstall-forcer class (P6 / UPD-1)**
- [ ] **Real installed N-1 → N silent update applies + relaunches cleanly on BOTH OS**, with the new CEF file manifest / framework layout + new **12.0** minos, tied to the VER-5 drift audit. **No proxies — the actual updater, the actual new-CEF binary.** Broken-N rolls back wallet-intact.
- [ ] **Funded-wallet safety:** the apply test uses a **throwaway/trivial-balance** wallet with recovery phrase written down (OQ-4) — NOT a funded production wallet; verify money-DB intact + balance/outputs preserved + **graceful money-DB shutdown** across relaunch.

**⭐ SIGNING-IDENTITY / SIGNER-CONTINUITY GATE (P1 decision + P6 verification / UPD-2 — the #1 reinstall-forcer, and one the apply test can silently mask)**
- [ ] **N-1 is signed with the currently-shipped production identity; N with the identity beta.1 will ship under** — verified in **every** apply leg (not a shared dev cert, which would pass on bytes while production forces a reinstall).
- [ ] **Windows:** Authenticode **Subject CN = `Marston Enterprises` UNCHANGED** N-1↔N — compare **CN**, NOT the ~3-day-rotating Azure Trusted Signing leaf thumbprint (the beta.23 regression root cause). `signtool verify` / cert-subject check on both installers.
- [ ] **macOS:** `codesign -dv` shows **Team ID UNCHANGED** N-1↔N (confirm pre-build — org conversion *should* preserve Team ID but Apple does not contractually guarantee it) + Authority = expected Developer ID; rotate **either** the Developer ID cert **or** the EdDSA key, **never both** (Sparkle chain-of-trust).
- [ ] **Org-migration sequencing recorded (tie to `ORG_IDENTITY_SIGNING_MIGRATION.md`, sequenced BEFORE the P7 prod build):** EITHER **(A)** the Apple individual→org migration landed first (**conditional on confirmed Team-ID preservation**) so beta.1 is the first org-signed build and the N-1(individual)→N(org) apply test proves no forced reinstall (do NOT rotate EdDSA the same step) — one test covers both the CEF-bump manifest risk and the signer risk; **OR (B) explicitly record that beta.1 stays on the pre-migration (individual) identity and the migration is deferred past beta.1**, whose own N-1→N apply test is that later release's gate. **If Team-ID preservation is NOT confirmed, (A) is off the table → (B).**

**Codecs / media (P5/P6)**
- [ ] `canPlayType` → `'probably'` for H.264 (`avc1.42E01E`), H.264 High (`avc1.640028`), AAC (`mp4a.40.2`), MP3, VP9; **AV1 decode presence asserted.**
- [ ] HEVC = **inherited hardware-only, per-machine, non-gating** (CQ-1); Dolby out-of-scope.
- [ ] Real playback smoke: YouTube, x.com (video + animated-GIF-as-MP4), Reddit, Twitch, LinkedIn, an audio site.

**Farbling — B1 acceptance (P4/P6)**
- [ ] CreepJS zero "lies" (`.toString()` → `[native code]`); **worker column == window column** incl. **service-worker, shared-worker, OffscreenCanvas-in-worker** (purpose-built harness — I2).
- [ ] **Intra-session consistency** (same read twice → identical perturbation).
- [ ] **Cross-profile difference** (same site, two profiles → different values).
- [ ] **Cross-site iframe difference** (third-party origin under two first-parties → different values; first-party keying + P4e).
- [ ] **Cross-session login test** (create account → restart → revisit → logins do NOT break — persistent per-profile seed).
- [ ] Navigator values in valid set (deviceMemory ∈ {4,8,16,32}; hardwareConcurrency ≤ real cores); **WebGL vendor/renderer decision (drop — recommended — or common-GPU-string map incl. Mac ANGLE) applied per FB-2**.
- [ ] **C7 OAuth/auth-domain exemption verified** — Q3 T2 native-value equality (SOLE proof of a live exemption) + pre-approved sites logging in; per-site toggle (T7) + global toggle (T8) survive.
- [ ] **No persistent seed on any renderer command line** — ProcessExplorer/`ps` (C2 threat model).
- [ ] `navigator.webdriver=false` + `window.chrome` stub survived JS-block deletion (BOT-1).
- [ ] **Stability soak + crash-rate gate** — no elevated renderer crashes vs the 136 baseline (or current-public-M136 telemetry if P2a smoked).
- [ ] **Canvas/WebGL performance-regression gate** — readback within budget vs baseline.
- [ ] **Farbling × adblock (Q2 T1–T8)** all pass incl. the **T6 `[native code]` toString GATE**; T8 no-orphaned-symbols (scoped until TD-5 re-homes the toggle). `hodos-unbreak.txt` untouched (adblock file, not farbling — I1).
- [ ] **Escape hatch proven:** a `HODOS_FARBLING`-unset build ships farbling-disabled (window == worker == stock fingerprint).

**DRM (P5)**
- [ ] Component-updater Widevine CDM auto-download tested + **loads**; **VMP-`.sig`-required-for-L3 question answered (I6)**; Amazon result documented (plays free at L3 → in; SD-capped/refused/needs-VMP → DRM-2 deferred, with cost + broken-site list). Brave-parity error compared.

**Regression / parity (P6)**
- [ ] Standard site basket (Thorough): Auth, Video/Media, News, E-commerce, Productivity, BSV — both OS.
- [ ] Adblock still works incl. YouTube `CefResponseFilter` ad-strip + cosmetic/scriptlet (Q2).
- [ ] **FedCM** ("Sign in with Google" account chooser) works — `CefPermissionHandler` FedCM coverage audited (§3g).
- [ ] **macOS minos guard GREEN** (every exe/helper/Rust-bin `minos ≥` framework minos) + manual relaunch-after-update on a machine at/near the 12.0 floor; **Big-Sur-strand announced in release notes**.
- [ ] Wallet send/receive + CWI shim intact (BRC-121 test site).

---

## Decisions still open for the owner

| # | Decision | Default in the plan | Where it bites | Needs owner input? |
|---|---|---|---|---|
| **D1** | **Stable vs LTS version target.** | **Ride branch `7871` into the M150 LTS** (LTS program confirmed real, resolves outline C1); **fallback M149/`7827`** if `7871` is still CEF-Beta on build day. | version §2; sets the branch number, not the phase order. | **YES — confirm after P1 Step-0 research + the build-day channel check.** |
| **D2** | **Widevine/Amazon DRM defer y/n.** | **DEFER DRM-2 (VMP) out of beta.1.** Run free Spike-1; ship CDM auto-download + honest note; VMP → post-beta.1 `VMP_SIGNING_SPIKE.md` only if premium streaming is a real product goal. | Q4 / §3d. | **YES — nice-to-have (→ defer) vs product goal (→ fund MLA/castLabs).** |
| **D3** | **macOS arch: arm64 vs x86_64 vs universal2.** | **universal2** (distribution breadth; longer Mac build; two per-arch builds + `lipo`); also sets the C4 Mac GPU-string set. | §5 / Q1. | **YES — sign off on the universal2 build-time/cost tradeoff.** |
| **D4** | **WebGL `UNMASKED_VENDOR/RENDERER` — drop vs Brave-parity common-GPU-string map (FB-2).** Plus M2 extra vectors (UA-CH, screen/DPR, getClientRects, fonts, enumerateDevices) as add vs accepted-gap. | **Drop** (recommended — random strings are *more* unique than truth); M2 vectors = **accepted gaps** for beta.1. Owner Q18 default leans Brave-parity-with-map. | §3c / `PLAN_farbling_blink.md` §7. | **YES (or delegate to the farbling plan) — the highest-risk value decision.** |
| **D5** | **C2 seed delivery channel (FB-1): (A) mojo/commit-params per-navigation vs (B) ephemeral-nonce cmdline.** | **(A)** — browser-side HMAC, master seed browser-only; off-cmdline either way. **Load-bearing for C7:** C7's "no new IPC" property holds ONLY under a per-navigation channel + top-frame keying (Q3 R2). | §3c C2 / `PLAN_farbling_blink.md` §4. | Design-level — owner sign-off optional; flagged for awareness (affects C7 scope). |
| **D6** | **`HODOS_FARBLING` `condition` build gate (CEF-4).** | **Yes** — ship the escape hatch. Rollback fallback (M5) = revert to the 136 branch, not just the toggle. | §8 #12/#13. | Optional — confirm the escape-hatch appetite. |
| **D7** | **Signing-migration sequencing on beta.1 (A migrate-first vs B defer).** | **(A) migrate-first, conditional on confirmed Team-ID preservation** so one real apply test validates both the CEF-bump and the signer change; **(B) defer** if Team-ID preservation is not confirmed. **Sequenced BEFORE the P7 prod build either way.** | UPD-2 / `PLAN_build_test_prod.md` §7.7 OQ-1 / version §8. | **YES — confirm (A) vs (B) after the Team-ID-preservation check.** |
| **D8** | **Brave-style "install Widevine" consent prompt (DRM-3).** | **Defer/optional** — CDM already auto-downloads; prompt is cosmetic. May fit the privacy story. | Q4. | Optional. |

---

*This roadmap sequences the edit inventory in `chromium-rebuild/Q5_full_edit_list.md` and gates against the readiness checklist above. All eleven plan docs now exist; reconcile only on live execution (line-number drift, the FB-1/FB-2 owner value-fills, the build-day channel gate, the Team-ID-preservation check). Filename-convention drift across the Q-docs (underscore vs the outline's hyphen-`x` stubs) to be resolved in a single rename pass — not a Chromium/CEF-tree edit.*
