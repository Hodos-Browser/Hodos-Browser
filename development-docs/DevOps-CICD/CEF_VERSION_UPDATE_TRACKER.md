# CEF Version Update Tracker

Track features, fixes, and investigations to research when updating the CEF build.

**Current CEF version:** 136 (built from source with `proprietary_codecs=true ffmpeg_branding=Chrome`)
**Current macOS floor:** **11.0 (Big Sur)** — CEF 136 dropped 10.15 "Catalina"; published minimum must match (see *"macOS Minimum Deployment Version"* below). Re-check on every Chromium bump.

---

## ⭐ Version-lock — CEF 150 / branch `7871` (recorded 2026-08-03, pre-build)

The `PLAN_version_bump.md` §3 pre-flight, answered. **Re-verify the point-release on build day** — a
newer `7871` patch may land. Everything else here is settled.

| Item | Value | How verified |
|---|---|---|
| **Target branch** | **`7871`** = CEF 150 = Chromium 150 | `branches_and_building.html` + CDN `index.json` |
| **Pinned point-release** | **`150.0.17+g94c1726+chromium-150.0.7871.187`** (newest, NOT `.0`) | `index.json`, both `windows64` and `macosarm64` |
| **⛔ Channel gate** | **SATISFIED — `7871` is CEF-Stable.** It was Beta on 2026-07-10; it is not anymore | `branches_and_building.html` |
| **M149/`7827` fallback** | **DEAD — do not use.** `7827` is already in CEF's *Unsupported* table | same |
| **LTS window** | stable 2026-06-30 · **LTC 2026-07-21** · **LTS 2026-10-06** · security refresh ends **2027-04-13** (~9 mo) | chromiumdash; matches CEF's "Last Refresh" column exactly |
| **Why 150 not 151** | M151 is not a 6th branch → **no LTS at all**. Taking it would force an early re-bump | CEF issue #3947 cadence |
| **Coverage caveats** | **Platform-agnostic Chromium fixes only.** CEF's own fixes are **not** backported (maintainer, #3947) | #3947 |
| **⚠️ Automation trap** | `index.json` has **no `lts` enum** — LTS builds are labelled `"stable"` there. **Key off branch number `7871`, never the JSON `channel`** | reproduced locally: only `stable`/`beta` across 1,195 `windows64` builds |
| **Python** | `.vpython3` pins **3.11** on `7871` — **unchanged from `7103`** | `.vpython3` @ branch-heads/7871 |
| **Build tool** | **Siso** by default on a fresh out-dir (Ninja unsupported upstream since Sept 2025) | `build/toolchain/siso.gni` @ 7871 |
| **Codec GN args** | `proprietary_codecs` + `ffmpeg_branding` **unchanged** — no rename, no value change M136→M150 | `features.gni` / `ffmpeg_options.gni` @ 7871 |
| **Toolchain — OPEN** | CEF's table says **VS2022 17.13.4 + SDK `10.0.26100.4654`**; Chromium's own 7871 docs say **SDK `10.0.26100.7705`** and list **VS2026/VC145** as the packaged toolchain (VS2022/VC143 still supported). **Provision from `build/vs_toolchain.py` on the synced branch.** This box has MSVC 14.44.35207 (VS2022 17.14) + SDK 10.0.26100 + Debugging Tools | conflicting primary sources; unresolvable without building |
| **macOS floor** | **12.0 Monterey** (M150 is the last Chrome supporting it). Set `max(12.0, measured)` — the current 11.0 was **never `vtool`-measured** | Mac owns; VER-4 |

**Ledger:** the earlier "there is no CEF LTS channel" correction (2026-06-17, now in
`../0.4.0/archive/SPRINT_0_4_0_MASTER_PLAN.md`) is **retracted**. It was an artifact of reading the CDN
JSON rather than the website table. Full reasoning: `CEF_BUILD_RUNBOOK.md` Step 1.

**Build-day re-check (2026-08-03):** `index.json` re-queried — `150.0.17+g94c1726+chromium-150.0.7871.187`
is still the newest `7871` build (8 total on the branch; next-newest `150.0.14`). **No newer
point-release; the pin stands.** The CEF checkout's own
`cef/CHROMIUM_BUILD_COMPATIBILITY.txt` @ `94c1726` confirms the transitive Chromium pin:
`chromium_checkout: refs/tags/150.0.7871.187`.

---

## Codec baseline — M136, measured pre-bump (2026-08-03)

`PLAN_codecs.md` §6.1 Layer-A probe, run against the **live shipping M136 build**
(`libcef.dll` `136.1.7+g15882fe+chromium-136.0.7103.114`, 249 MB, our own codec build) per **D9**
— the M136 re-build was skipped, so this is the pre-bump reference the P2b/P5 comparison uses.

Harness: local HTTP page calling `canPlayType`, results POSTed back (no human transcription).
Run under `cefclient.exe` from the M136 out-dir rather than the shipping shell, so the user's live
browser session was not disturbed. UA confirmed `Chrome/136.0.0.0`.

| Codec | Query | Gate | **M136 result** |
|---|---|---|---|
| H.264 baseline | `avc1.42E01E` | GATE | `probably` |
| H.264 High | `avc1.640028` | GATE | `probably` |
| AAC-LC | `mp4a.40.2` | GATE | `probably` |
| MP3 | `audio/mpeg` | GATE | `probably` |
| VP9 | `vp09.00.10.08` | GATE | `probably` |
| AV1 | `av01.0.05M.08` | assert present | `probably` |
| HEVC | `hvc1.1.6.L93.B0` | **non-gating** | `probably` |
| HEVC | `hev1.1.6.L93.B0` | **non-gating** | `probably` |

**All GATE rows pass.** HEVC answers `probably` **on this machine** (i9-12950HX) — it is
hardware-dependent and inherited-on, so it is recorded, not gated. A different test machine may
return `""` for the HEVC rows without that being a regression.

⚠️ **Post-bump comparison rule:** any GATE row returning `""` on `7871` = codec build regressed →
block the bump and re-audit `args.gn`. A change in the **HEVC** rows alone is *not* a blocker, but
must be recorded here alongside the machine it was measured on.

---

## GN-args pre-flight gate — `7871`, run 2026-08-04 (pre-build)

`PLAN_codecs.md` §7 steps 1–2, run **before** the 10–12 h build. **Result: PASS.** 1211 args
resolved. **No codec flag was renamed or flipped M136 → M150** — `GN_DEFINES` carried forward
verbatim, and the generated `args.gn` matches the shipped M136 `args.gn` on every key flag.

| Flag | `7871` | Status |
|---|---|---|
| `proprietary_codecs` | `true` | **GATE** ✅ |
| `ffmpeg_branding` | `"Chrome"` | **GATE** ✅ |
| `chrome_pgo_phase` | `0` | **GATE** ✅ |
| `is_official_build` | `true` | **GATE** ✅ |
| `enable_widevine` / `enable_library_cdms` | `true` | resolves |
| `enable_cdm_host_verification` / `enable_cdm_storage_id` | `true` | resolves — relevant to the Q4 VMP question |
| `enable_platform_hevc` / `enable_hevc_parser_and_hw_decoder` | `true` | non-gating |
| `enable_av1_decoder` / `enable_dav1d_decoder` | `true` | AV1 present |
| `enable_mse_mpeg2ts_stream_parser` | `true` | recorded |
| `enable_platform_ac3_eac3_audio` | `false` | recorded |
| **`enable_platform_dolby_vision`** | **`true`** | ⚠️ see below |
| `media/BUILD.gn` coupling guard | present | `assert(ffmpeg_branding != "Chromium", …)` **survived 14 milestones** |

### ✅ RESOLVED — `enable_platform_dolby_vision=true` is NOT a bump regression

Flagged as unexpected because every plan doc says **"Dolby out-of-scope / Dolby off"**
(`PLAN_codecs.md` §6.3, roadmap P5 step 1, readiness checklist), yet `7871` resolves it **`true`**
and we never set it.

**Checked against the M136 tree the same day — the declared default is byte-identical:**

```gn
# media/media_options.gni — IDENTICAL in 7103 and 7871
enable_platform_dolby_vision =
    proprietary_codecs && (is_cast_media_device || is_win)
```

Since we set `proprietary_codecs=true` and build on Windows (`is_win`), it resolved `true` on
**M136 as well**. **Nothing changed in the bump; the shipping M136 build already has it on.**

**What this actually corrects is the plan docs, not the build.** "Dolby off" describes an intent we
never implemented — the flag has been **inherited-on since M136**, exactly like HEVC, as a
consequence of `proprietary_codecs=true` on Windows rather than a choice. Treat it the same way:
**inherited, recorded, non-gating.** `enable_platform_ac3_eac3_audio=false` is unrelated (Dolby
*audio*, separately defaulted) — the pairing only looked odd.

**No action, and specifically do NOT "fix" it during the bump.** Adding
`enable_platform_dolby_vision=false` would be a *behaviour change* introduced under cover of a
version bump — the opposite of the pin-don't-bump discipline used for DEP-1a..d. If Dolby is
genuinely unwanted, that is its own owner-decided change with its own smoke test, on the M136
baseline as much as on `7871`.

---

## ✅ P3 PATCH TOOLCHAIN STOOD UP — our CEF fork (2026-08-05)

From this point the build no longer consumes upstream CEF directly. Source patches are ours to add.

| Item | Value |
|---|---|
| **Fork** | **`github.com/Hodos-Browser/cef`** — public fork of `chromiumembedded/cef` |
| **Upstream remote (rebase from)** | `https://github.com/chromiumembedded/cef.git` — GitHub, **not** legacy Bitbucket |
| **Integration branch** | `hodos/7871`, created off upstream `7871` @ `94c17267eb` |
| **Build pin** | `--url=<fork>` + `--checkout=0a709e584` in both `scripts/build_hodos_cef.{bat,sh}` |
| **Condition gate** | `HODOS_FARBLING=1` set in both build scripts — one gate for the whole C1–C7 set |
| **Registered Hodos patches** | **0** — the standup probe was proven then removed (OQ-7). Count is back to upstream **114** |
| **Ledger** | `HODOS_PATCHES.md` **in the fork** (not this repo) |
| **Drift audit** | `scripts/cef_patch_drift_audit.sh` — exit 0 clean / 2 offsets / 1 do-not-build |
| **Security watcher** | `.github/workflows/cef-fork-watch.yml` — weekly; ⚠️ cron only fires from the **default branch**, so it is dormant until 0.4.0 reaches `main` |
| **Verification build** | `AUTOMATE_EXIT=0`, all four distributions produced |

**Where patches apply:** `cef/tools/gclient_hook.py:37` → `tools/patcher.py`, invoked from
`automate-git.py:1671` in the **build** step — **not** `run_patch_updater`, which never applies on a
pinned checkout. So **`--force-build` alone re-applies patches.**

### ⚠️ Two traps to re-check on every future bump

1. **`chromium/src/cef` is a COPY**, refreshed only when the CEF checkout **hash** changes
   (`automate-git.py:1358-1360`). Manually checking out the standalone dir to the target commit before
   building means the copy never refreshes and **the build silently compiles zero Hodos patches** — with
   a green run and correct-looking checkouts. **Always confirm the patcher's `N patches total` line.**
   Fix: `--force-cef-update`.
2. **Fork builds report `CEF_VERSION_PATCH 0`** — `150.0.0-HEAD.<n>+g<sha>+chromium-150.0.7871.187`
   instead of upstream's `150.0.17` / `PATCH 17` (`cef_version.py:189-225`: our commits are *descendants*
   of `7871`, and the SHA checkout detaches HEAD). **✅ INVESTIGATED + CLOSED 2026-08-05 — accepted as-is,
   no change.**
   - **This is cosmetic, not a security gap.** `chromium-150.0.7871.187` — the field that carries the CVE
     fixes — is present in the version string **in every variant**. CEF's `150.0.x` counter tracks CEF's
     own commits, not Chromium security content. An earlier write-up called this a security-tracking
     regression; that was **overstated**.
   - The candidate fix (`--checkout=hodos/7871`, so the checkout lands on a named branch) was **tested**
     via `cef_version.py`: it yields `150.0.19-7871.<n>+g<sha>+…`. `PATCH` there is a **count of branch
     commits** (`:72-105`), i.e. upstream's 17 **plus our 2** — so it still does not state the upstream
     level, it **drifts ahead of upstream**, and it will **collide** with a real upstream `150.0.19`.
     A number that looks like an upstream release but isn't is worse than an obviously-synthetic `0`.
   - It would also trade an exact reproducible pin for a moving branch tip, on a signed money-handling
     build. Recovering that needs a SHA assertion — re-adding the pin to buy a wrong number.
   - **Authoritative build identifier is `CEF_COMMIT_HASH`.** Fork-commit → upstream-base mapping lives in
     `HODOS_PATCHES.md` §2, where it cannot collide.

Full evidence: `../0.4.0/chromium-rebuild/P3_TOOLCHAIN_PROOF.md`. Restore point:
`../0.4.0/chromium-rebuild/P3_BASELINE_94c1726.md`.

---

## ✅ BUILD CHANGELOG — CEF 150 / `7871` (Windows, 2026-08-04)

| Item | Value |
|---|---|
| Branch / checkout | `7871` / `94c1726` = `150.0.17+g94c1726+chromium-150.0.7871.187` |
| Chromium | `refs/tags/150.0.7871.187` = `30f6543ae91e6a860e73b76e3216b663b050f4e5` |
| `GN_DEFINES` | `is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0` (unchanged from M136) |
| Build tool | **Siso** (default on fresh out-dir) |
| **Wall-clock** | **289 min (4 h 49 m)** — well under the 10–12 h estimate |
| Host | i9-12950HX, 16C/24T, 31.7 GB RAM; 24 parallel `clang-cl` |
| `libcef.dll` | **292 MB** (M136 was 249 MB) |
| Wrapper | `libcef_dll_wrapper.lib` **104 MB**, builds clean on the new headers |
| Patch set | none at build time (P3 stood up **after** this build — see the P3 entry above) |
| Deps touched | none in the CEF tree; DEP-1a..d pinned separately, no version moved |
| Codec Layer-A | **PASS — all GATE rows `probably`**, AV1 present, HEVC unchanged vs M136. Re-confirmed 2026-08-05 **with the Chromium sandbox ON** (HEVC also `probably` on this host) |
| Codec Layer-B | **PASS — real decode proven** on 4 of 7 targets; every GATE codec covered. See below |
| Est. per-bump patch-rebase hours (I10) | **N/A this bump** (zero Hodos patches). Baseline for the next one: ~5 h build + ~4 h of checkout/tooling firefighting, all now documented |

#### Codec Layer-B — real-playback smoke (Windows, 2026-08-05, sandbox ON)

`PLAN_codecs.md` §6.2. **Layer-A only proves a codec is *registered*.** Pass here is
`webkitVideoDecodedByteCount` / `webkitAudioDecodedByteCount` actually **climbing** between two
samples ~6 s apart, with `currentTime` advancing — i.e. bytes really decoded. Driven over CDP on the
dev build (port 9322).

| Target | Exercises | Result | Evidence (Δ over ~6 s) |
|---|---|---|---|
| **x.com** | H.264 + AAC | ✅ **PASS** | video **+3,074,656 B**, audio **+98,191 B**, `dt +6.0s` |
| **twitch.tv** | live H.264/AAC (HLS) | ✅ **PASS** | video **+5,589,264 B**, audio **+121,839 B**, `dt +6.0s` |
| **youtube.com** | VP9/AV1 + audio | ✅ **PASS** | video **+108,594 B**, audio **+80,321 B**, `dt +5.7s` |
| **MP3** (direct decode) | MP3 | ✅ **PASS** | `decodeAudioData`: 39,868 B → **2.074 s PCM**, 48 kHz, 2 ch, 99,562 samples |
| reddit.com | H.264 (v.redd.it) | ⚠️ **blocked** | enterprise reCAPTCHA interstitial killed the tab target. Bot detection, **not** a codec result |
| linkedin.com | H.264 feed video | ⚠️ **blocked** | redirects to `/login`; no signed-in session on this host |
| soundcloud.com | AAC/MP3 | ⚠️ **substituted** | `/discover` instantiates no media element. §6.2 explicitly permits substituting a stable MP3/AAC source — the direct MP3 decode above is that substitute |

**Every GATE codec is covered by a passing row:** H.264 (x, twitch), AAC (x, twitch), MP3 (direct
decode), VP9/AV1 (youtube). The three non-passing rows are all **access/bot-detection** problems on
our side of the network, not decode failures, and each is redundant with a passing row for codec
purposes (reddit + linkedin = H.264, already proven twice; soundcloud = MP3/AAC, both proven).

> Repro harnesses: `layerb.py` (site smoke) and `mp3-decode.py` (direct decode) in the session
> scratchpad. Two gotchas if rebuilding them: sites spawn **out-of-process iframes** that appear in
> `/json/list`, so pin the tab's `targetId` once rather than re-picking by URL (a reCAPTCHA iframe
> gets picked otherwise and you silently probe the wrong document); and reddit/twitch/soundcloud put
> players in **shadow DOM**, so a plain `document.querySelectorAll('video,audio')` finds nothing.

**Still owed:** the same Layer-B run on **macOS** (§6.3 requires both OSes) — tracked in
`0.4.0/MAC_WINDOWS_RELAY.md`.

#### ✅ P5 CODEC RE-VERIFY on the farbling build — Windows, 2026-08-10

The run above was against **`94c1726`**, the pre-patch 150 baseline. P5 asks for the codec gate on
the binary that ships, so both layers were re-run against the current staged build:

| | |
|---|---|
| Fork pin | **`c63654654`** (C1+C2+C3+C4+C5+C6) |
| `CEF_VERSION` | `150.0.40-7871.3573+gc636546+chromium-150.0.7871.187` |
| Engine (CDP) | `Chrome/150.0.7871.187` |
| Harness | `0.4.0/chromium-rebuild/codec_check.py` (both layers, one script) |
| Subject | shell log confirms `example.com` served to **`role=tab_1`** — a tab, not one of the ~14 overlays |

**Step 1–3 (resolved args, not script input).** `gn args --list` needs the VS env, so the evidence
taken was the **generated artifact the compiler actually consumed**, which is strictly better:
`out/Release_GN_x64/gen/media/media_buildflags.h` has `USE_PROPRIETARY_CODECS() (1)`,
`ENABLE_PLATFORM_HEVC() (1)`, `ENABLE_HEVC_PARSER_AND_HW_DECODER() (1)`, `ENABLE_DAV1D_DECODER() (1)`,
`ENABLE_AV1_DECODER() (1)`, `ENABLE_PLATFORM_AC3_EAC3_AUDIO() (0)`, `ENABLE_PLATFORM_AC4_AUDIO() (0)`.
`ffmpeg_branding` is not a buildflag, so its receipt is the compiled config path in the generated
ninja: **`third_party/ffmpeg/chromium/config/Chrome/win/x64`**. The coupling guard still exists at
`media/BUILD.gn:85-89`.

**Layer A — 6/6, all controls red-capable:**

| Row | Result | |
|---|---|---|
| H.264 baseline `avc1.42E01E` | `probably` | GATE ✅ |
| H.264 High `avc1.640028` | `probably` | GATE ✅ |
| AAC-LC `mp4a.40.2` | `probably` | GATE ✅ |
| MP3 `audio/mpeg` | `probably` | GATE ✅ |
| VP9 `vp09.00.10.08` | `probably` | GATE ✅ |
| AV1 `av01.0.05M.08` | `probably` | presence ✅ |
| HEVC `hvc1.1.6.L93.B0` | `probably` | recorded, non-gating (this host has a decoder) |
| **Dolby Vision `dvh1.05.07`** | **`""`** | recorded — buildflag is **1** (inherited via `is_win`), runtime feature keeps it invisible to sites. Closes the loop on the "not a bump regression" entry above: inherited-on **and** user-invisible. |
| AC-3 / E-AC-3 / bogus | `""` / `""` / `""` | **CONTROL** — proves the probe can report absence |

**Layer B — decode receipts:** MP3 `+3,135 B`, AAC `+3,005 B`, H.264 `+394 B` (local `data:` assets,
`currentTime +1.000s` each); youtube `video +165,161 B / audio +46,584 B`, x.com `+255,975 / +49,207`,
twitch `+1,408,702 / +62,945`, all with `currentTime` advancing ~3 s.

**⛔ Negative control (Layer B):** an **AC-3-in-MP4** asset, built by the same ffmpeg from the same
tone as the passing AAC asset, played through the same element and read from the same counters →
`NotSupportedError`, counters flat. Same probe, same page, one absent decoder. So Layer B has been
demonstrated to go red.

Token: `CODEC-GATE-v1 engine=Chrome/150.0.7871.187 H.264_baseline=probably H.264_High=probably
AAC-LC=probably MP3=probably VP9=probably AV1=probably HEVC/H.265=probably Dolby_Vision=empty`

**Failures survived (all documented in `CEF_BUILD_RUNBOOK.md` Lessons):** shallow `depot_tools`;
stale `automate-git.py`; `rd`/`STATUS_DLL_INIT_FAILED` aborting gclient on an empty temp dir; HTTP
429; `core.autocrlf`; the autocrlf follow-on; `update_depot_tools.bat` re-dirtying the pin; and a
**gate malfunction that impersonated a codec regression**.

### 🚨 VER-5 DRIFT FOUND — `cef_sandbox.lib` is GONE, `bootstrap.exe` is NEW

**This is the drift audit paying for itself.** Comparing the shipped M136 `Release/` against `7871`:

| File | M136 | 7871 |
|---|---|---|
| `cef_sandbox.lib` | **present** (we link it) | **ABSENT from the distribution** |
| `bootstrap.exe` | — | **NEW** |
| `bootstrapc.exe` | — | **NEW** |

`cef_sandbox.lib` still *builds* (`out/Release_GN_x64/obj/cef/cef_sandbox.lib`) but CEF no longer
copies it into the distribution — only `include/cef_sandbox_win.h` ships.

**Root cause: CEF issue #3928 — CEF removed `cef_sandbox` linking in favour of a bootstrap
executable.** On Windows, `USE_SANDBOX` now defines **`CEF_USE_BOOTSTRAP`** instead of
`CEF_USE_SANDBOX` (`cmake/cef_variables.cmake:609-613`). CEF's own reference client shows the new
model (`tests/cefclient/CMakeLists.txt:592-596`):

```cmake
add_library(${CEF_TARGET} SHARED ${CEFCLIENT_SRCS})   # the app becomes a DLL
COPY_SINGLE_FILE(... bootstrap.exe → ${CEF_TARGET}.exe)  # CEF's exe, renamed
```

**Impact on Hodos — this is an architecture change, not a packaging tweak.**
`cef-native/CMakeLists.txt:473` links `cef_sandbox`, so `cef-native` **cannot link as-is**. Adopting
the upstream model means `HodosBrowser.exe` becomes `HodosBrowser.dll` + a renamed copy of CEF's
`bootstrap.exe`, which touches:

- **Code signing** — a new binary to Authenticode-sign (CN continuity gate).
- **Silent auto-update** — the `{app}` file manifest changes shape. This is precisely the
  VER-5 → P6 linkage the roadmap flagged: *a changed manifest is what breaks a silent update.*
- **The installer** — `hodos-browser.iss` `[Files]` and the `.exe`/`.dll` split.
- **Dev/prod safeguard + launcher scripts**, which key off the executable path/name.

**⛔ NOT actioned — owner decision required.** Options: **(A)** adopt the bootstrap model (upstream's
path; real work + a mandatory N-1→N update test); **(B)** hand-copy `cef_sandbox.lib` out of the
build tree and keep linking it (non-standard, diverges from upstream, may not survive the sandbox
init API change); **(C)** `USE_SANDBOX=OFF` — **rejected**, that disables the Chromium sandbox in a
money-handling browser.

---

## Must Investigate on Next CEF Update

### Toolchain (MSVC) & Dependency Alignment
- **Priority:** HIGH — build-breaker. Nothing compiles or links if this is wrong, and the errors *look* like our code but aren't.
- **Why:** The compiler toolset (currently **MSVC v143**, shipped by Visual Studio 2022) is a cross-cutting **ABI contract**. **Four things must all sit on the same toolset** or you get linker/ABI failures:
  1. The **CEF binaries** — whether prebuilt download *or* our own full Chromium+CEF source build
  2. The **vcpkg static deps** (`nlohmann-json`, `sqlite3`, …) — compiled per-toolset
  3. Our **C++ shell** code
  4. The **CI runner image** — the `windows-XXXX` / `macos-XX` GitHub label that *provides* the compiler
- **The rule for FULL builds (Chromium/CEF bump), not just shell builds:** when moving to a new stable Chromium/CEF, treat the toolset as a deliberate choice:
  1. Pull the **latest stable Chromium**; note which **MSVC/Clang toolset** its CEF is built with.
  2. Re-validate **every dependency version** against that toolset — vcpkg baseline, CEF wrapper, Inno Setup, Sparkle/WinSparkle, etc. (see `DEPENDENCY_VERIFICATION.md`).
  3. Rebuild the **vcpkg static deps** and the **CEF wrapper** on the chosen toolset.
  4. **Pin the CI runner image** (`runs-on:` in `release.yml` / `ci.yml`) to one that ships that exact toolset — **never `windows-latest` / `macos-latest`**, which float and silently roll the compiler forward under you.
  5. Bump `APP_VERSION` + installer + appcast versions; run the full smoke matrix (CLAUDE.md Testing Standards).
- **Cautionary tale (2026-06-25):** GitHub rolled the `windows-latest` label from the windows-2022 image to **windows-2025**. The `"Visual Studio 17 2022"` CMake generator stopped resolving ("could not find any instance of Visual Studio") and the beta.16 Windows build died at *configure* — before compiling a single file. Pure infra drift, zero code changes. Fix: pin `runs-on: windows-2022`. **Generalize: pin runner images; don't let them float.**
- **References:**
  - `DEPENDENCY_VERIFICATION.md` — per-bump dependency checklist
  - `CEF_BUILD_RUNBOOK.md` — full Chromium+CEF source build
  - `.github/workflows/release.yml` — `runs-on:` pins + the explanatory comment on the windows job
- **Added:** 2026-06-25

### macOS Minimum Deployment Version (published min must match Chromium's floor)
- **Priority:** HIGH — ships a **broken auto-update** if wrong. Sibling of the runner-pin lesson above; same root cause (a floating runner image silently overrode our intent).
- **Why:** Two numbers must agree or mac auto-update breaks:
  1. **The oldest macOS the Chromium/CEF build actually supports** (Chromium raises this every few majors as Apple drops old OSes — e.g. **CEF 136 dropped macOS 10.15 "Catalina"**, so the true floor is **macOS 11.0 "Big Sur"**, *not* the `10.15` our config historically claimed).
  2. **Our published minimum** — `CMAKE_OSX_DEPLOYMENT_TARGET` (`cef-native/CMakeLists.txt`), `LSMinimumSystemVersion` (`cef-native/Info.plist`, `cef-native/mac/helper-Info.plist.in`), and the binary's actual Mach-O `LC_BUILD_VERSION minos`.
- **The two failure modes (both real):**
  - **Published min too HIGH** (the beta.16 bug): the build floated on `macos-latest` = macOS 26 "Tahoe"; the deployment-target intent was a silent CMake no-op, so the **linker stamped the binary's `minos` at the runner's SDK (26)**. Sparkle/the loader then refuses to relaunch on every user below that OS → "requires macOS 26.0 or later" → **dead auto-update**. (This is *why* pinning the runner — see the Toolchain item — and forcing the deployment target both matter.)
  - **Published min too LOW** (claim 11.0 when the framework needs 12.0): the OS *accepts* the update, then dyld fails to load the higher-`minos` CEF framework → **launch crash after update**. Worse than gating honestly.
- **The rule, every Chromium/CEF bump:**
  1. **Look up the new Chromium's oldest supported macOS** (Chromium release notes / "Chrome to drop support for macOS X" announcements).
  2. **Measure the prebuilt CEF framework's real floor** on a Mac: `vtool -show-build "<...>/Chromium Embedded Framework.framework/Chromium Embedded Framework" | awk '/minos/{print $2}'` (or `otool -l | grep -A4 LC_BUILD_VERSION`). Do **not** trust the announcement alone.
  3. **Set our published minimum = `max(Chromium floor, measured framework minos)`** in **all three** places: `CMakeLists.txt` `CMAKE_OSX_DEPLOYMENT_TARGET`, `Info.plist` `LSMinimumSystemVersion`, `helper-Info.plist.in` `LSMinimumSystemVersion`. Keep them identical.
  4. **Apply it for real** — pass `-DCMAKE_OSX_DEPLOYMENT_TARGET=<floor>` on the configure command line (a bare `set(... CACHE ...)` after `project()` is a silent no-op) and export `MACOSX_DEPLOYMENT_TARGET=<floor>` at job level so the CEF wrapper, cargo, and sub-cmakes all inherit one floor.
  5. **Guard it in CI** (the standing per-build check — see `BUILD_AND_RELEASE.md` release checklist): after build, read `minos` of the main exe, all helper apps, and the Rust binaries and **FAIL the build unless each `minos` ≥ the CEF framework's `minos`** (an inequality, not `== <floor>`). CI runs on the newest macOS and *cannot* reproduce a sub-floor loader rejection, so also do a **manual relaunch-after-update on a real machine at/near the floor** before `promote --latest`.
- **⚠️ Runner SDK vs. deployment target — do not conflate (this is what caused beta.16):** these are TWO independent things.
  - **Build runner** (`runs-on:`) = the machine that *compiles* the app. It does **not** decide which users can run it. Best practice: **build on the *current stable, pinned* image** (newest GitHub-supported `macos-NN` you've validated), **never the floating `macos-latest`**. Re-validate and bump the pin on each Chromium bump (and whenever GitHub retires the image — old images *are* eventually removed, so "pin once forever" isn't an option).
  - **Deployment target** (`CMAKE_OSX_DEPLOYMENT_TARGET` / `minos`) = the **minimum-requirements label** that decides backward compatibility. This — not the runner — is what makes one binary run on the floor OS *and everything newer*. Standard Apple practice (and Chrome/Firefox's): **build with the latest SDK, set the deployment target to the oldest OS you support.**
  - **The trap:** if you forget to *explicitly* set + enforce the deployment target, the linker stamps `minos` from the **runner's SDK** — so a newer runner makes the app run on *fewer* machines, not more (the beta.16 "requires macOS 26" failure). Building on a newer runner never *widens* user compatibility; only lowering the deployment target does. So: newest pinned runner is fine and recommended, **provided the explicit target + the `minos` guard are in place.**
- **Decision log:** for **CEF 136**, published floor = **macOS 11.0 (Big Sur)**, pending the §2 `vtool` measurement confirming the framework isn't higher. 10.15 is retired (Chromium dropped Catalina). Runner = **current stable pinned image (`macos-15`)**, not `macos-latest`. Owner approved "Big Sur or newer" + "current stable, pinned runner" 2026-06-26.
- **References:**
  - `cef-native/CMakeLists.txt` (`CMAKE_OSX_DEPLOYMENT_TARGET`), `cef-native/Info.plist` + `cef-native/mac/helper-Info.plist.in` (`LSMinimumSystemVersion`)
  - `.github/workflows/release.yml` — mac build job runner pin + deployment-target flag + the post-build `minos` guard
  - `development-docs/0.4.0/archive/POST_BETA16_PLAN.md` — Thread 5 (full root-cause + fix)
- **Added:** 2026-06-26

### FedCM (Federated Credential Management) Support
- **Priority:** HIGH
- **Why:** Google made FedCM mandatory for "Sign in with Google" as of August 2025. CEF 136 does not implement the browser-level UI (account chooser dialog) that FedCM requires. This breaks "Sign in with Google" on any site that migrated to FedCM-only (no popup/redirect fallback).
- **What to check:**
  - Does the new CEF version include `CefPermissionHandler` methods for FedCM?
  - Is there a `navigator.credentials.get({identity: ...})` handler we can implement?
  - Check Chromium commit history for FedCM-related CEF changes
  - Test: Go to any site with "Sign in with Google" — does the account chooser appear?
- **Workaround (current):** Sites that still support OAuth popup/redirect fallbacks work. Sites that went FedCM-only do not show the Google sign-in button at all.
- **References:**
  - https://developer.chrome.com/docs/identity/fedcm/overview
  - https://developers.google.com/identity/gsi/web/guides/fedcm-migration
  - CEF issue tracker: search "FedCM" or "Federated Credential Management"
- **Added:** 2026-05-01

### Permissions API Updates
- **Priority:** MEDIUM
- **Why:** CEF 136 handles some permissions natively via Chrome bootstrap. Newer CEF versions may add `CefPermissionHandler` methods for notifications, geolocation, camera/mic that we should implement.
- **What to check:**
  - New `CefPermissionHandler` methods
  - Permission persistence APIs
  - Test: Check if notification permissions, camera access work
- **Added:** 2026-05-01

### CefResponseFilter Stability
- **Priority:** LOW
- **Why:** We use `CefResponseFilter` for YouTube ad-key stripping (`AdblockResponseFilter`). This API has had stability issues in some CEF versions.
- **What to check:**
  - Verify YouTube ad blocking still works (response filter streaming)
  - Check if API changed or was deprecated
- **Added:** 2026-05-01

---

## Nice to Have / Research

### Web Bluetooth / Web USB
- CEF may add support for these APIs in newer versions
- Currently not available in CEF 136
- Low priority for a browser focused on BSV/Web3

### COOP/COEP Header Handling
- Cross-Origin-Opener-Policy affects OAuth popup `window.opener` preservation
- Newer Chromium versions have `restrict-properties` mode
- Verify our popup handling still works with stricter COOP defaults

### Codec Updates
- We build CEF from source with proprietary codecs
- Check if build flags changed for H.264/AAC/H.265 support
- Verify media playback on YouTube, Twitch after update

---

## Process for CEF Version Updates

1. Check this document for investigation items
2. Build from source with `proprietary_codecs=true ffmpeg_branding=Chrome`
3. **Align the toolchain** — note the toolset the new CEF is built with; rebuild vcpkg static deps + the CEF wrapper on it; re-run `DEPENDENCY_VERIFICATION.md`; and **re-pin the CI runner images** (`runs-on:`) to one shipping that toolset (never `windows-latest`/`macos-latest`). See *"Toolchain (MSVC) & Dependency Alignment"* above.
4. **Re-check the macOS minimum version** — look up the new Chromium's oldest supported macOS, `vtool`-measure the prebuilt CEF framework's real `minos`, set our published minimum = `max(those)` in `CMakeLists.txt` + both plists, apply it via `-DCMAKE_OSX_DEPLOYMENT_TARGET=` on the configure line, and confirm the CI `minos` guard passes. See *"macOS Minimum Deployment Version"* above.
5. Run full test suite (Minimal + Standard site verification from CLAUDE.md)
6. Specifically test: Google Sign-In, OAuth flows, media playback, ad blocking, fingerprint protection
7. Update this document with findings
8. Update `CLAUDE.md` x.com media section if codec situation changes
