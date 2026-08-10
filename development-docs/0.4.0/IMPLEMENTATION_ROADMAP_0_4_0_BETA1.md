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
> - Build scripts: `development-docs/DevOps-CICD/scripts/build_hodos_cef.bat` / `development-docs/DevOps-CICD/scripts/build_hodos_cef_mac.sh` (**not yet checked in — OQ-1 in `PLAN_patch_toolchain.md`**)

---

## TARGET — resolved (was a placeholder) — `PLAN_version_bump.md`

> **✅ UPDATED 2026-08-03 (kickoff review).** The build-day channel gate is **already satisfied** —
> `7871` reached **CEF-Stable** and entered **LTC 2026-07-21**; it becomes **LTS 2026-10-06** with
> security refresh through **2027-04-13**. Pin **`150.0.17+g94c1726+chromium-150.0.7871.187`**.
> **The M149/`7827` fallback is DEAD** — `7827` is already in CEF's *Unsupported* table, so falling back
> to it would land us on an unsupported branch. The only build-day re-check left is whether a newer
> `7871` **point-release** exists. Full record + the retracted "no LTS exists" note:
> `../DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md` §Version-lock.

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
4. **TARGET default build-tool lookup (Ninja vs Siso).** ✅ **ANSWERED 2026-08-03.** **Siso is the default** on `7871` for a *fresh* out-dir (`use_siso_default = true` when `build_with_chromium` and no `.ninja_deps`); our M136 out-dir keeps Ninja. Siso **runs fully local** — no RBE required (and Chromium's RBE is unavailable to external contributors on Windows regardless). Resume state = `.siso_fs_state` + `.siso_deps` + a replayed `.siso_fs_state.journal`; one Ctrl-C is graceful, a second aborts. `use_siso=false` still works but is **unsupported upstream since Sept 2025** — fallback only, and switching requires `gn clean`. **Hard-kill resume is unproven for us → keep the persistent/owned host and keep Windows Update paused.** Detail: `CEF_BUILD_RUNBOOK.md` Lessons.
5. **OQ-7 / M136-still-builds confirmation.** ⚠️ **Evidence points at bit-rot — decide before spending 10–12 h.** Branch `7103` is in CEF's **Unsupported** table and the local CEF checkout's HEAD commit is literally *"Pin depot_tools version for out-of-support branch."* **However, a complete 175 GB M136 tree from 2026-03-12 still exists at `C:\cef\chromium_git\`** with `binary_distrib` intact, and its output (238 MB `libcef.dll`) is what we ship today. **Recommended: invoke the I5 guarded fallback now** — treat the existing tree + shipped binary as the last-known-good baseline, run the `PLAN_codecs.md` §6.1 Layer-A probe against the **live M136 build** to capture the pre-bump codec/HEVC baseline, and skip the re-build. *(Owner decision — tracked as D9 in the kickoff results doc.)*

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
8. **VER-4 minos exec (Mac):** `vtool`-measure framework `minos`; set published min = `max(12.0, measured)` in **all FIVE places** (⚠️ corrected 2026-08-03 — "three" was wrong): (1) `cef-native/CMakeLists.txt:115` `CMAKE_OSX_DEPLOYMENT_TARGET`, (2) `cef-native/Info.plist:24` + (3) `cef-native/mac/helper-Info.plist.in:22` `LSMinimumSystemVersion`, (4) `release.yml:405` `MACOSX_DEPLOYMENT_TARGET`, (5) `release.yml:539` `-DCMAKE_OSX_DEPLOYMENT_TARGET`. **#5 is the trap: the CI command line OVERRIDES #1**, so editing `CMakeLists.txt` alone changes the local build and leaves the *shipped* build at 11.0 — a green edit with no shipped effect. Then wire the CI minos guard (`release.yml:645-672`).
9. **VER-6:** confirm version single-sourcing (git tag → `cargo-release` → CMake/shadow-rs/`.iss`/TS constant) injects cleanly on the new tree.
10. **FEDCM-1:** audit `CefPermissionHandler` FedCM coverage (on-by-default since ~M108 → already live on M136; re-verify on `7871`); scope a shell edit if the permission API changed. *(Note Q3 §2.6: FedCM is browser-native UI, no farblable JS surface — do NOT add IdP origins to the C7 allowlist.)*
11. Rebuild the CEF wrapper + `cef-native` against new headers (no "Unsupported CEF version").

**Entry:** P1 exit. **Exit:** (P2a) clean 136 baseline + codec baseline, or documented last-known-good smoke; (P2b) clean `7871` build both OS; channel gate satisfied; GN args resolve; DEP-1a..d pinned + DEP-1 pass with touched/deferred table; VER-5 drift audit clean + human-reviewed + copy-lists updated; minos aligned + guard green; FedCM audited; wrapper + `cef-native` compile.
**Ownership:** **Windows = LEAD** (VER-1/2/3/5/6, DEP-1 vcpkg-heavy, FEDCM-1, Win builds). **Mac owns entirely:** VER-4 minos/`vtool`/plist/guard, DEP-1c Brewfile, mac framework-embed list, its own baseline + target builds.

---

## P3 — CEF PATCH TOOLCHAIN *(serial linchpin; blocks P4)* — 🟢 SUBSTANTIALLY COMPLETE 2026-08-05

> **Status.** Fork `Hodos-Browser/cef` + `hodos/7871` live; `--url` wired into both build scripts and
> pinned at a fork commit; `HODOS_FARBLING` condition gate proven in all three states; drift audit landed
> and negative-tested; fork-watcher landed. A patch authored in our fork was proven to reach the Chromium
> source **pre-compile through the real build path** (`115 patches total (1 applied, 114 skipped,
> 0 failed)`). **P4 is unblocked for authoring.**
>
> Still open: the probe's *build-completes* half, probe removal, the `CEF_VERSION_UPDATE_TRACKER.md`
> entry, Mac-side verification, and the ready-for-consumer gate (C1 end-to-end — deliberately deferred,
> since `AUTHORS` is not compiled and so the probe cannot prove a patch reaching the *compiler*).
>
> **Three findings worth carrying into P4** — evidence in `chromium-rebuild/P3_TOOLCHAIN_PROOF.md`:
> 1. Patches apply via `gclient_hook.py:37` → `patcher.py` in the **build** step, **not**
>    `run_patch_updater`. So `--force-build` alone re-applies; no re-sync needed to iterate.
> 2. `chromium/src/cef` is a **copy** refreshed only when the CEF checkout **hash** changes. Manually
>    checking out the standalone dir first makes the build silently compile **zero** Hodos patches, with
>    a green run. Watch the patch count; fix with `--force-cef-update`.
> 3. The patcher's **`skipped` count is ambiguous** (gated-off vs already-applied vs missing-target-dir).
>    Prove a `condition` gate from the per-patch stdout line, never the summary.

> **✅ DE-RISKED 2026-08-03 · numbers + mechanism CORRECTED 2026-08-05 (P3 kickoff).** "GREENFIELD"
> overstates it. The **patch mechanism already exists and runs on every build**. Measured on the pinned
> tree (CEF `94c1726`): **114 registered `patch.cfg` entries / 115 `.patch` files on disk** — the "105"
> figure was wrong, as were the plan's "~150" and Q5's "empty". Parse `patch.cfg` by `exec`ing it, not
> by grepping; the header comment overcounts.
>
> Applied by **`cef/tools/gclient_hook.py:37` → `tools/patcher.py`**, invoked from
> `automate-git.py:1671` in the **build** step — **not** by `run_patch_updater`, which on our pinned
> path (`--checkout=94c1726`, Chromium == compat version) never applies anything. Practical upside:
> **`--force-build` alone re-applies patches**, so P3 iteration needs no re-sync.
>
> What is greenfield is **our fork and our patches** (`hodos_*` count today: 0). We are adding a 115th
> entry to a working pipeline, not standing a pipeline up. The no-op probe (CEF-1) validates *our fork
> wiring*, not the mechanism — keep it, but expect it to pass. Full measured baseline + restore point:
> `chromium-rebuild/P3_BASELINE_94c1726.md`.

**Plan docs:** `PLAN_patch_toolchain.md` (full); outline §3b/§4 P3.
**Edit IDs:** CEF-1 (fork + patch.cfg + `automate-git --url` + no-op probe), CEF-2 (`cef_patch_drift_audit.py` Step-5.5 hook), CEF-3 (upstream security-pull duty / fork-watcher), CEF-4 (single `HODOS_FARBLING` `condition` gate), CEF-5 (check `build_hodos_cef.{bat,sh}` into `scripts/` + `HODOS_PATCHES.md` ledger — resolves OQ-1).

**Steps**
1. **CEF-1:** fork `chromiumembedded/cef` → **`Hodos-Browser/cef`**, branch `hodos/7871`; add `patch/patches/hodos_*.patch` + register in `patch/patch.cfg`; point the build at the fork via `automate-git.py --url=https://github.com/Hodos-Browser/cef.git --branch=7871 --checkout=<pin>`. Patches apply via `git apply -p0 --ignore-whitespace` (**exact-context, fail-loud, no fuzz** — a context mismatch aborts before compile). **Prove a no-op probe patch applies pre-compile + builds**, then remove it and re-verify the count returns to the stock upstream baseline. Clean-dir caveat: `automate-git` refuses a URL switch on an existing CEF checkout — remove the CEF sub-dir first (not `chromium_git/`).
2. **CEF-4:** wire the single `HODOS_FARBLING` `condition` gate (all-or-nothing; never half-apply the set); prove toggle applied↔skipped, never failed. Escape hatch for beta.1 stability.
3. **CEF-2:** land `DevOps-CICD/scripts/cef_patch_drift_audit.sh` — read-only per-patch `git apply --check` (**never** write-capable `patch_updater.py --reapply/--restore`), scrape hunk-**offset** lines (soft warning), registry/orphan + target-file-existence checks. **Reuse-first: the VER-5 file-manifest diff and the GN-args diff ALREADY EXIST** as `cef_dist_drift_audit.sh` + `cef_gn_args_gate.sh` — invoke them, do not reimplement. Must baseline the pre-existing upstream orphan `chrome_browser_privacy_1119417.patch`, or the gate exits 1 on every run. Exit 1 = build must not start; wire as a pre-build gate.
4. **CEF-3:** document + automate the recurring duty to pull upstream in-branch security point-releases into the fork (scheduled `gh`/Actions fork-watcher that opens a rebase PR when upstream `7871` advances). Record in `CEF_VERSION_UPDATE_TRACKER.md`.
5. **CEF-5:** **OQ-1 closed as (c)** — the two `build_hodos_cef*` scripts already exist at `development-docs/DevOps-CICD/scripts/`, which the runbook already declares canonical; fix the 35 repo-root `scripts/` citations across 12 docs instead of moving them (done, P3 commit 2). Create `HODOS_PATCHES.md` fork ledger.
6. **Attachment map ready** (patch_toolchain §8.1): C1 first, then C2–C7 → `hodos_farble_{session_cache,seed_wiring,canvas2d,webgl,webaudio,navigator,auth_exempt}.patch`, all `condition: HODOS_FARBLING`, all `path: src`. C1 also patches a Blink `BUILD.gn` (its higher-churn rebase target).

> **Prerequisite authoring (no build dependency — start as early as P0).** `PLAN_farbling_blink.md` and `Q3_farbling_oauth.md` are **now written** — they no longer block P4 entry. ✅ **Both owner value-fills are now CLOSED (2026-08-05): FB-1 = (A′) CEF per-frame mojo push + lazy sync pull; FB-2 = DROP WebGL vendor/renderer.** Remaining pre-P4 design work is only the FEDCM/OQ housekeeping; none require a build.

**Entry:** P2 exit. **Exit:** fork stands up; a no-op patch demonstrably applies + builds on both OS; `HODOS_FARBLING` toggles applied↔skipped; drift-audit script wired as a pre-build gate + scheduled fork-watcher; `HODOS_PATCHES.md` + tracker updated; `build_hodos_cef*` checked in; C1-alone can be authored→applied→built end-to-end (ready for P4a).
**Ownership:** **Windows = LEAD** authors the fork + toolchain + patch infra (shared cross-platform text). **Mac** verifies the no-op patch applies + builds through its own automate-git/framework path.

---

## P4 — FARBLING *(incremental; independent of P5 — patch toolchain MUST precede)*

**Plan docs:** `PLAN_farbling_blink.md` (full: §3 Supplement, §4 seed, §5 worker matrix, §6 files, §7 value table, §8 land order); `Q3_farbling_oauth.md` (C7); `Q1_mac_farbling.md` (Mac build/arch/GPU strings); `Q2_farbling_adblock.md` §3 (teardown hygiene TP-1/TP-2 + T1–T8); outline §3c.
**Edit IDs:** C1..C7 (+C7b fallback), P4e, TD-1..TD-5, ~~BOT-1~~ (✅ **DONE 2026-08-05**, ahead of P4a).

**Land order (each sub-step builds + smokes; each atomically deletes its own JS counterpart in the same commit — I-4, no double-farbling window, no guard flag):**
- **P4a — C1 `HodosSessionCache : Supplement<ExecutionContext>` (✅ **LANDED 2026-08-05**, fork `4ed200cf9`) + C2 seed/channel → Canvas-first worker quick-win.** *(C1 needs **no** `execution_context.{h,cc}` hook — `ExecutionContext` is already `Supplementable`; and the source list is the per-directory `core/execution_context/build.gni`, not `core/BUILD.gn`. So C1 modifies no existing source file. Keep it that way on rebases.)* Persistent per-profile `profile_seed` (32B CSPRNG via **`BCryptGenRandom`**/`SecRandomCopyBytes`, NOT deprecated `CryptGenRandom`) stored in `%APPDATA%/HodosBrowser/<profile>/fingerprint_settings.json` (NOT the wallet). **Browser process computes `domain_key = HMAC-SHA256(profile_seed, first-party eTLD+1)` and delivers ONLY `{domain_key, farble_enabled}` to the renderer — master seed never leaves the browser** (supersedes B1-design's renderer-HMAC). Ship the Supplement wired to **Canvas (C3) only** first (highest-signal detection fix; closes the window-vs-worker mismatch for canvas). Delete the JS **canvas** fragment this same step. **C2 delivery channel ✅ CLOSED 2026-08-05 (FB-1) = (A′):** push over CEF's **existing** per-frame mojo channel (`cef/libcef/common/mojom/cef.mojom` + `CefBrowserFrame::RegisterBrowserInterfaceBindersForFrame` + `frame_impl.cc :: ConnectBrowserFrame(DID_COMMIT)`) at navigation commit, **plus a lazy `[Sync]` pull on first farbled API call** to close the first-inline-script race. All of that is **fork-local — zero Chromium patches browser-side**; only the renderer→Blink setter is a Chromium patch. (B) ephemeral-nonce-cmdline **rejected**: same mojo round-trip *plus* a cmdline token and a nonce→profile map. ⚠️ In-process dedicated workers are **not literally free** — the key must be threaded into `GlobalScopeCreationParams` at worker start (small Blink patch, still P4a).
- **P4b — C4 WebGL incl. `readPixels` (its OWN patch point).** ✅ **FB-2 CLOSED 2026-08-05 = DROP** vendor/renderer — no GPU-string map, so C4's `getParameter` hook is unnecessary and **FB-6 never opens** (Mac ANGLE strings not required; must not block the gate). Delete JS WebGL fragment.
- **P4c — C5 WebAudio + C6 Navigator (valid-set constrained — §B).** *(BOT-1 already landed ahead of P4a — see the block above; `navigator.plugins` needs no C6 work, native is the spec'd list.)* deviceMemory **∈ {4,8,16,32}** (FB-8), hardwareConcurrency **reduce-only ≤ real cores** (FB-7), plugins realistic 5-PDF set. Delete JS audio fragment.

> ### 🚨 BOT-1 — CORRECTED 2026-08-03. The citation was half wrong, and the wrong half bites.
> **`navigator.webdriver = false` is NOT at `:629-653`.** Verified against the working tree, it lives at
> **`FingerprintScript.h:128-133` — *inside* the block TD-1 deletes.** So does the 5-entry
> **`navigator.plugins` spoof (`:99-126`)**. Both are **anti-bot** surfaces, not farbling surfaces.
> Only the **`window.chrome` stub** is genuinely separate; it has its own `isExternalPage` gate and
> survives the teardown by construction.
>
> ✅ **Re-verified 2026-08-05 at the P4a kickoff: the two `FingerprintScript.h` cites above are CORRECT
> and current.** The `window.chrome` stub, however, is **no longer at `:629-653`** — it is at
> **`simple_render_process_handler.cpp:549–573`** (guard recomputed `:551`, object `:559`).
>
> **Consequence:** performing TD-1 as written **silently drops `navigator.webdriver` and
> `navigator.plugins` spoofing.** No compile error, no test failure — just bot-detection regressions in
> the field.
>
> **This is not hypothetical.** `simple_app.cpp:92-94` already records that Cloudflare Turnstile rejects
> detectable browsers on **whatsonchain.com** — a site in our own BSV regression basket — which is why
> the detectable Chromium dev flags were moved behind `HODOS_MAC_DEV_FLAGS=1`. Git history shows the
> same lesson: `4fad37b` "Fix Cloudflare bot detection blocking (B-5)" landed **the same day** as
> `b514c30`, the refactor that removed the WebGL GPU-string spoofing.
>
> **Required sequencing — do this FIRST, as its own commit, BEFORE any teardown step:** re-home
> `navigator.webdriver = false` and the `navigator.plugins` set into the independent `window.chrome`
> stub block (or another carrier that does not depend on farbling being enabled). They must survive
> **both** the JS-block deletion **and** a per-site farbling opt-out — a user disabling farbling on a
> site must not thereby announce they are automated. Only then proceed to TD-1.

> ### ✅ BOT-1 DONE 2026-08-05 — and the measurement REVERSED the plan: both overrides were **deleted, not re-homed**.
>
> Measured against our own M150 build (source + live CDP probe on an auth-exempt page **and** a farbled page). Native Chromium was already correct on both, and **our overrides were the thing making us detectable**:
>
> - **`navigator.plugins`** — we shipped a list naming **`"Chrome PDF Plugin"`**. Chromium returns the spec'd hard-coded list from whatwg/html#6738 (Blink `DOMPluginArray` ctor, gated on `IsPdfViewerAvailable()`), which names **`"Chromium PDF Viewer"`** in that slot — `"Chrome PDF Plugin"` is the pre-2021 name. We build `enable_pdf = true`, so the native list is present. Our spoof therefore produced a plugin list **no real Chrome has**, checkable against a published constant.
> - **`navigator.webdriver`** — natively `false` already, by **two independent margins**: Blink's `AutomationControlled` is off unless `--enable-automation` / `--headless` / `--remote-debugging-pipe` / `--remote-debugging-port=`**`0`** (an *explicit* port is deliberately left unset), we pass none of the first three, **and** our `remote_debugging_port = 0` disable path never reaches Chromium because CEF only appends the switch for values in `[1024, 65535]`. Re-homing it would have put an **own-property** accessor on `navigator` where real Chrome has a **prototype** accessor — exactly the prototype-tamper signal Turnstile/DataDome look for.
>
> **Landed instead:** both JS overrides deleted; a **TRIPWIRE** comment at the `remote_debugging_port` block in `cef_browser_shell.cpp` (the only path that can flip `webdriver`); and a standing harness, **`chromium-rebuild/farbling_probe.py`**, that asserts all of it over CDP every release. Deleting also fixes a real hole: the old overrides vanished with the script, so a **per-site farbling opt-out silently changed the bot signature**. The guarantee is now structural.
>
> Live result (16/16 PASS): `webdriver=false`, own-property `False`, prototype accessor `True`, plugins == spec'd 5 — **identical on the exempt and the farbled page**.
- **P4d — C7 auth-domain exemption at the BROWSER process** (`Q3` — supersedes outline C7's "list passed to renderer"). One `ShouldFarble(top_frame) = GlobalEnabled && !IsAuthDomain(top_frame_HOST) && IsSiteEnabled(top_frame_eTLD+1)`; allowlist match on the **full committed top-frame host** (OQ3 — do NOT collapse to eTLD+1); registrable domain used only for the seed key. Delivers the single `farble_enabled` bit **alongside C2's `{domain_key}` payload** (no new IPC **iff** C2 = per-navigation channel — R2 fork). Structurally fixes the Turnstile parent/iframe inconsistency. **TD-4** migrate `IsAuthDomain` here; **TD-5/C7b** re-home the per-site user toggle (`IsSiteEnabled`) into `ShouldFarble` (owner sign-off; C7b sibling if C7 kept minimal). JS FP block now fully torn down → **M1 complete**.
- **P4e — OOP seed/exemption plumbing:** deliver top-frame-derived `{domain_key, farble_enabled}` to **shared workers, service workers** (key = registration-scope eTLD+1, FB-3), **and cross-site (OOP) iframes** at subframe navigation commit. Audio/paint worklet + OffscreenCanvas-in-worker are in-process (free once C2 lands, but tested). Needs a purpose-built worker-parity harness (CreepJS only exercises the dedicated-worker column).

**Teardown (M1 — retire, don't orphan; atomic per-value):** TD-1 delete JS FP block `simple_render_process_handler.cpp:501–547` (**re-verified 2026-08-05**; both the 2026-07-10 `:581–627` and the outline's `:586-632` are stale), keep adjacent scriptlet `:487–499` byte-identical; TD-2 retire `FingerprintProtection.h`/`FingerprintScript.h` JS-injection parts; TD-3 remove FP seed caches/IPC (`:34–35`, `:39–40`, `simple_handler.cpp:7578–7615`, renderer handlers `:1131/:1146`; **NOT** `fingerprint_get_site_enabled_response :2033`, which is TD-5-gated) **only after C2 channel verified delivering per-domain seeds (P4a smoke green)** — a design-choice-only deletion would strand the renderer with a constant/absent seed. **TD-5 stays gated** until `ShouldFarble` consumes `IsSiteEnabled` (Q2 T8) — do NOT delete the toggle first.

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
- [ ] Build is **buildable/repeatable** from `development-docs/DevOps-CICD/scripts/build_hodos_cef.{bat,sh}` (now checked in — CEF-5) on the pinned toolchain; changelog appended (branch, milestone, GN_DEFINES, patch-set version, deps, duration, **per-bump patch-rebase hours — I10**).
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
- [x] **Windows** — `canPlayType` → `'probably'` for H.264 (`avc1.42E01E`), H.264 High (`avc1.640028`), AAC (`mp4a.40.2`), MP3, VP9; **AV1 decode presence asserted.** Re-run 2026-08-10 on the farbling build `c63654654` (the 08-05 run was the pre-patch baseline `94c1726`). Harness `chromium-rebuild/codec_check.py`. **[ ] macOS owed.**
- [x] HEVC = **inherited hardware-only, per-machine, non-gating** (CQ-1): `probably` on this host. **Dolby corrected, not out-of-scope**: Dolby *audio* is off (`ENABLE_PLATFORM_AC3_EAC3_AUDIO=0`), but **Dolby Vision's buildflag is inherited-ON** via `proprietary_codecs && is_win` — and has been since M136 — while `canPlayType('dvh1…')` still returns `""` behind its runtime feature. Inherited, recorded, non-gating; do **not** add an override during the bump.
- [x] **Windows** real playback smoke, proven by decoded-byte counters climbing: YouTube (VP9/AV1+audio), x.com (H.264+AAC), Twitch (live H.264/AAC), plus local `data:` MP3 / AAC / H.264 decode receipts. Reddit + LinkedIn remain access-blocked and are redundant with passing rows. **[ ] macOS owed.**
- [x] **The codec gate has been demonstrated to go red**: an AC-3-in-MP4 asset built from the same tone as the passing AAC asset fails to decode through the same element and counters (`ENABLE_PLATFORM_AC3_EAC3_AUDIO=0`), and `ac-3`/`ec-3`/bogus return `""` from the same `canPlayType` probe.

**Farbling — B1 acceptance (P4/P6)**
- [ ] CreepJS zero "lies" (`.toString()` → `[native code]`); **worker column == window column** incl. **service-worker, shared-worker, OffscreenCanvas-in-worker** (purpose-built harness — I2).
- [x] **Intra-session consistency** (same read twice → identical perturbation). **Windows 2026-08-10**, `farbling_acceptance_battery.py`: two reads of the same origin in one session are identical across canvas/WebGL/audio/navigator. Carries its own **sensitivity control** — a second origin measured in the same session must differ, and does (canvas/WebGL/audio/cores all move); without it, "identical" is equally consistent with a broken measurement, an unloaded page, or the wrong browser. **[ ] macOS owed.**
- [x] **Cross-profile difference** (same site, two profiles → different values). **Windows 2026-08-10**, `farbling_cross_profile_check.py`: canvas/WebGL/audio all differ across `Default` vs `Profile_1` with all five controls holding still; negative control (same seed in both) collapses every value to identical. **[ ] macOS owed.**
- [⛔] **Cross-site iframe difference** (third-party origin under two first-parties → different values; first-party keying + P4e). **MEASURED RED on Windows 2026-08-10** — `farbling_iframe_check.py`. `example.org` embedded under `example.com` and under `example.net` returns values **identical to each other and identical to the true-native baseline** (canvas `53225ec8`, audio `07ff541f`, `(32, 24)`), while the same origin loaded top-level *is* farbled (`39e8b0d9`) — so farbling was active during the run and the iframe genuinely is not being farbled.
  - **This is a coverage gap, not a keying bug.** `OnBeforeBrowse` sends `hodos_farble_key` only `if (frame->IsMain() …)`; a cross-site iframe is an **OOPIF in a different renderer process**, never receives a key, and fails closed to native. Identical shape to the measured worker gap.
  - Controls all held: size-gate controls identical across all five measurements; **same-parent repeat identical** (so the cross-parent comparison is stable, not noise); top-level farbled ≠ top-level hard-bypassed.
  - ⚠️ **This widens P4e again, and the honest description of farbling's scope is now "the main frame and same-site frames only — ALL workers and ALL cross-site iframes are unfarbled."** The third-party-iframe case is the *common tracking vector*, so this is the gap with the most product-claim consequence. **Owner decision needed on whether the release notes / privacy claims are scoped accordingly.** P4e remains deferred; only its measured size changed.
- [x] **Cross-session login test** (create account → restart → revisit → logins do NOT break — persistent per-profile seed). **Windows 2026-08-10**: YouTube session survived a real restart on a farbled origin with a byte-identical fingerprint; negative control (seed rotated between phases) moved all three values. ⛔ Note the trap it dodged: the dev profile's other logins (x.com, github.com) are **auth-exempt and therefore unfarbled**, so the obvious run of this test would have passed vacuously — `farbling_cross_session_login_check.py` parses `IsAuthDomain` at runtime and refuses such a target. **[ ] macOS owed.**
- [~] Navigator values in valid set (deviceMemory ∈ {4,8,16,32}; hardwareConcurrency ≤ real cores) — **Windows 2026-08-10 PASS** via `farbling_acceptance_battery.py` (measured `(32, 10)` against 24 real logical cores). The validator is a **set-membership** check, not a range check, because an out-of-ladder deviceMemory is itself a tell. Negative control is `--self-test` (no browser): deviceMemory=7, cores above the real count, cores=0 and a missing value must all be rejected, plus a positive control that legitimate readings are accepted. **[ ] macOS owed.** **[ ] Still open: WebGL vendor/renderer decision (drop — recommended — or common-GPU-string map incl. Mac ANGLE) applied per FB-2.**
- [~] **C7 OAuth/auth-domain exemption verified** — **T2 GREEN on Windows 2026-08-10**, `farbling_exemption_check.py`: five allowlist hosts (github.com, x.com, whatsonchain.com, www.google.com, paypal.com) measured **byte-identical to a true-native baseline of the same origin** obtained via the per-site hard bypass ⇒ exemptions are LIVE, not merely seed-independent. ⚠️ Note why constancy was not enough: the rotation gate's `exempt=53225ec8/53225ec8/53225ec8` would look identical if the exempt path farbled with a *fixed* key, so T2 is the only assertion that discriminates. Independent cross-check: the hard bypass reproduces canvas `53225ec8` / audio `07ff541f`, the same natives the rotation gate reports for the exempt origin. Negative control **red** — a non-exempt host (example.com) tested as if exempt reports NOT-LIVE, differing on canvas/WebGL/audio/cores, and the two size-gate controls held on every host. **T7 (per-site toggle) exercised in passing** — the bypass demonstrably forced example.com to native. **T8 (global toggle) GREEN 2026-08-10** via `farbling_acceptance_battery.py`: toggling `privacy.fingerprintProtection` off changes the values, the setting persists across restart, and — the stronger assertion — global-off lands on **exactly the same native values as the per-site hard bypass**, so two independent code paths agree on "native". Incidentally proves the renderer **fails closed**: with the global toggle off, `OnBeforeBrowse` sends no `hodos_farble_key` at all, so landing on native means a key-less renderer degrades to native rather than to a partially-initialised key. **T1 partial (2026-08-10):** four exempt origins load with no bot-block or login wall, and **x.com is in a logged-IN state** on an exempt origin (title `Home / X`) — real evidence the exemption does not break an authenticated session. ⚠️ **Not full T1**, and deliberately not claimed as such: `github.com` came up on its logged-OUT marketing page (that session has since expired), and T1's actual claim — that a user can *complete a sign-in* — needs credentials and, per `EXCEPTIONS_DESIGN_REVIEW.md` §5b A2, a **fresh cookie-less profile** doing a real sign-in/sign-up, N ≥ 3 trials. Owner-gated. **Still owed:** full T1, **32 of 37 allowlist entries unproven** (asset-only origins are not top-level navigable; `accounts.google.com` attempted and would not load in 90 s). **[ ] macOS owed.**
- [x] **No persistent seed on any renderer command line** — ProcessExplorer/`ps` (C2 threat model). **Windows 2026-08-10**, `farbling_cmdline_seed_check.py`: live `Win32_Process.CommandLine` for all 16 build-dir processes, searched for the seed (hex + base64), both derived domain keys and any whole-value 32+ char hex — zero hits, with a positive control proving the scan can read renderer command lines (it aborts as BLIND otherwise) and a self-test proving the detector still catches a planted leak. **[ ] macOS owed (`ps -ww -o args`).**
- [x] `navigator.webdriver=false` + `window.chrome` stub survived JS-block deletion (BOT-1). **Windows 2026-08-10**, `farbling_acceptance_battery.py`: `navigator.webdriver === false` (boolean, not undefined) and `window.chrome` present with `loadTimes,csi,app`, measured **while driving the page over CDP** — i.e. under the condition most likely to expose automation. Same probe re-asserts `getImageData`/`readPixels` report `[native code]` (the cheap echo of the Q2 T6 gate). Negative control via `--self-test`: webdriver=true, an absent `window.chrome`, and a non-native `toString` must each be rejected. **[ ] macOS owed.**
- [ ] **Stability soak + crash-rate gate** — no elevated renderer crashes vs the 136 baseline (or current-public-M136 telemetry if P2a smoked).
- [x] **Canvas/WebGL performance-regression gate** — readback within budget vs baseline. **Windows 2026-08-10 PASS**, `farbling_perf_check.py`. Both arms timed on the same machine back to back (native = per-site hard bypass), minimum-of-N rather than mean because desktop timing noise is one-sided.

  | operation | native | farbled | ratio |
  |---|---|---|---|
  | `getImageData` 200×50 *(farbled)* | 0.0500 ms | 0.0775 ms | **1.55×** |
  | `readPixels` 32×32 *(farbled)* | 0.8640 ms | 0.9820 ms | **1.14×** |
  | `getImageData` 400×200 *(control, above gate)* | 0.3200 ms | 0.3233 ms | 1.01× |
  | `readPixels` 256×256 *(control, above gate)* | 0.9233 ms | 1.0300 ms | 1.12× |

  ⭐ **The null-effect control is what makes these numbers trustworthy**: operations above the farbling size gates are not perturbed in either arm, so their ~1.0× ratio is a direct measurement of this rig's timing noise, taken on the same APIs in the same run. The harness refuses to report a verdict if those controls drift from 1.0 by more than `--control-tolerance`. Worst case is **+0.028 ms per `getImageData` call** on a 10k-pixel canvas — `readPixels` is dominated by GPU sync, which is why its ratio is lower. Budget is `--max-ratio` (default 3.0), deliberately a CLI argument because acceptable overhead is a product judgement, not something the measurement decides. **[ ] macOS owed.**
- [~] **Farbling × adblock (Q2 T1–T8)** — **T1/T5/T6/T7/T8 GREEN on Windows 2026-08-10**, `q2_farbling_adblock_check.py`. **[ ] macOS owed. [ ] T2/T3 need a human watching a video. ⛔ T4 is KNOWN RED**
  - **T1**: a blocked request is genuinely **cancelled in the browser**, not merely classified by the engine — probed with a `no-cors` fetch (a normal cross-origin fetch fails on CORS whether or not it was blocked, so it would read "blocked" for the wrong reason; in `no-cors` a request that goes through resolves opaque and only a *cancelled* one rejects). **Negative control**: with the engine disabled via `POST /toggle` the same URL goes through.
  - **T7**: the same holds on an **auth-exempt origin** (github.com) — the farbling exemption does not disable adblock. Two independent systems, measured independently.
  - ⚠️ **Two test defects this row had to fix first, both of which faked a product bug.** (1) `AdblockCache` memoises the verdict per URL and clears only on the **browser's** toggle / filter update / site toggle — *not* on the engine's HTTP `/toggle` — so the first negative control re-read a cached "blocked" and reported that disabling adblock changed nothing. Fixed with a per-probe nonce that changes the cache key while still matching the same filter rule. (2) The benign control was cross-origin, and github.com's **CSP `connect-src`** cancelled it for reasons unrelated to adblock; it is now same-origin (`location.origin + '/favicon.ico'`), which is CSP-safe — and a 404 still *resolves*, so a missing path does not fake a cancellation. (CreepJS worker column ≠ window column — *all* workers are unfarbled, P4e deferred; record as an accepted gap, do not chase).
  - **T6 `[native code]` GATE**: `toDataURL`, `getImageData`, `readPixels`, `getParameter`, `getChannelData` all native **on a farbled origin** (an exempt origin would not engage the patched paths at all), and `Function.prototype.toString` is itself unpatched — without which every other answer would be a lie in the safe-looking direction. **In-page negative control**: a deliberately JS-wrapped function reads NON-native through the same detector. ⚠️ **T6 is necessary, not sufficient — deleting farbling entirely would also pass it.** Cite only alongside the seed-rotation gate.
  - **T8**: zero live references to all five retired symbols; **the guard set is the positive control** — `IsAuthDomain`/`IsSiteEnabled`/`SetSiteEnabled`/`fingerprint_*_site_enabled` must still be PRESENT (Q2 T8 forbids that group going to zero before TD-5). ⚠️ **A naive grep fails here against correct code**: all five retired symbols survive as *tombstone comments* explaining the 2026-08-09 deletion, so the audit strips comments — and a stripper aggressive enough to hide a retired symbol would collapse the guard set first.
  - **T5**: zero canvas/WebGL/audio API references in either injectable set (downloaded `resources/scriptlets.js` + the six bundled scriptlets), each with a positive control proving the file was read ⇒ **no double-wrap risk in the shipped configuration**, answering Q2-1 empirically. ⚠️ **Method note:** the filter lists reference scriptlets by *alias* (`aopr`, `acs`, `set`, …), so grepping rule text for "canvas" returns a confident 0 whether or not one is in use — 2,771 `+js()` rules across the four lists, 49 distinct scriptlet names, none of which contain the word. Only the implementations can be injected, so they are the correct subject.
  - `hodos-unbreak.txt` untouched (adblock file, not farbling — I1).
- [ ] **Escape hatch proven:** a `HODOS_FARBLING`-unset build ships farbling-disabled (window == worker == stock fingerprint).

**DRM (P5)**
- [x] **Windows, on `c63654654` (2026-08-10)** — Component-updater Widevine CDM auto-download tested + **loads** (4.10.3050.0, per profile); no VMP `.sig` beside the executable. **I6 answered:** a `.sig` is **not** required for L3 — the unattested CDM gets a real licence from a real licence server and decrypts (Bitmovin demo: +2,893,374 B decoded). ⚠️ **The wall is `distinctiveIdentifier: required`, not a `SW_SECURE_DECODE` cap** — software `SW_SECURE_DECODE` *is* granted here, contradicting the 08-05 ladder row, which does not reproduce and looks like a probe artifact (audio has no such tier). See `Q4_widevine_amazon_drm.md` §7 re-run. Harness: `chromium-rebuild/drm_check.py`. **[ ] macOS owed.**
- [ ] Amazon result documented (plays free at L3 → in; SD-capped/refused/needs-VMP → DRM-2 deferred, with cost + broken-site list). Brave-parity error compared. **Owner-gated: needs a real Prime/Netflix/Spotify account.** D2's default (defer DRM-2) is unaffected by the correction above — the identifier gap is still unreachable without VMP.

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
