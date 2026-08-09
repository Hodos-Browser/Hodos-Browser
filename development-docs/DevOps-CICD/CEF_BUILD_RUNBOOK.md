# CEF/Chromium Full-Build Runbook (Tier 1)

**Created:** 2026-06-01 · **Last updated:** 2026-08-03
**Status:** ✅ WORKING — grounded in `development-docs/DevOps-CICD/scripts/build_hodos_cef.bat` +
`build_hodos_cef_mac.sh` and the real build done 2026-03-12 (merged in from the former
`CEF_BUILD_FROM_SOURCE_GUIDE.md`).

> **⚠️ Script path corrected 2026-08-03 · OQ-1 CLOSED 2026-08-05.** Both build scripts live at
> **`development-docs/DevOps-CICD/scripts/`**, NOT at a repo-root `scripts/`. `Q5` CEF-5's "check them
> in — absent today" was wrong: they exist, just not where most docs cited them.
>
> **Owner decision (2026-08-05): this path is canonical — the scripts stay put.** CEF-5 therefore
> reduces to a *citation* fix, not a move and not authoring. The 35 stale `scripts/build_hodos_cef*`
> references across 12 docs were rewritten to the full path in P3 commit 2. New CEF build/patch tooling
> lands **here**, beside `cef_gn_args_gate.sh` / `cef_dist_drift_audit.sh` / `chromium-build-gitconfig`.

**Owner:** DevOps/CI-CD · **Covers:** A1 (self-build), A2 (latest stable), A3 (dependency bump), A5 (Tier 1)

> **Read this first — terminology.** We are a **CEF-based browser that does custom Chromium builds.**
> CEF is not an alternative to Chromium; CEF's `automate-git.py` downloads the full Chromium source,
> applies the CEF layer, and compiles `libcef`. Our shell (`cef-native/`) is built against that.
> "Full build" = this Tier-1 process: produce fresh CEF binaries. It is **expensive and infrequent**.
> The fast Tier-2 path (bug-fix app release that *reuses* these binaries) is in `BUILD_AND_RELEASE.md`.

> **History note (2026-06-16):** the detailed step-by-step guide that used to live at
> `CEF_BUILD_FROM_SOURCE_GUIDE.md` (created 2026-03-01, updated 2026-03-12 with real build results)
> was **merged into this runbook** so there is a single Tier-1 build doc. Everything concrete from
> that guide — env setup, depot_tools, `automate-git.py` invocation, GN flags, output paths, the
> Windows `.bat` / macOS `.sh` specifics, and the hard-won lessons — is now below.

## Why we self-build (settled — do not relitigate)

Stock CEF binaries are built `ffmpeg_branding=Chromium` → **no H.264/AAC/MP3** → video/audio broken
across the open web. We build with `proprietary_codecs=true ffmpeg_branding=Chrome` to fix that.
Self-build is **mandatory for codecs**, and is *also* the only way to do renderer-layer farbling (B1).
Widevine premium DRM (Amazon/Netflix) is a **separate** VMP-signing concern — see §6.

**Sites that break without proprietary codecs** (prebuilt Spotify CEF returns `""` from
`canPlayType`): x.com (videos + animated GIFs, which are really MP4), Reddit (video spinner forever),
Twitch (many streams), Instagram, TikTok, most news-site embeds. After the codec build these all play.

```javascript
// Prebuilt CEF: returns "" (empty). Our build: returns "probably".
video.canPlayType('video/mp4; codecs="avc1.42E01E"')  // H.264
audio.canPlayType('audio/mp4; codecs="mp4a.40.2"')    // AAC
```

## What "CEF-based" means for us (capability, not a black box)

We **build custom Chromium+CEF from source** and apply source patches via `cef/patch/`. Our capability is therefore **not** limited to stock CEF API behavior — it is bounded by **patch scale + per-Chromium-bump maintenance**:
- **Small / localized patches** (farbling: a handful of Blink functions) — cheap, low churn.
- **Browser-UI-layer patches** (e.g. surfacing Chrome extensions in our *custom* header — the "Vivaldi model") — a large patch set against `chrome/browser/ui`, heavy per-bump rebase, approaching fork-level maintenance.

Both use the **same patch toolchain** (`cef/patch/patch.cfg`). The decision for any such feature is **effort / maintenance / risk — not "can CEF do it."** We remain a CEF *embedder* (not a full Chromium fork like Vivaldi), so the more we patch the UI layer, the closer we move to fork-level upkeep.

## Current known-good configuration (from our scripts + the 2026-03-12 build)

| Setting | Value | Source |
|---------|-------|--------|
| CEF branch | `7103` (CEF 136 / Chromium 136.0.7103.x) — **~14 milestones behind; branch `7103` is now in CEF's *Unsupported* list and its own HEAD commit reads "Pin depot_tools version for out-of-support branch." Expect bit-rot on a re-sync; see §1** | both scripts |
| GN_DEFINES | `is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0` | both scripts |
| Build tool | CEF `automate-git.py` (`--minimal-distrib --client-distrib --no-debug-build --force-build`) | both scripts |
| Win toolchain | VS 2022 BuildTools; `DEPOT_TOOLS_WIN_TOOLCHAIN=0`; `GYP_MSVS_VERSION=2022` | `.bat` |
| Win SDK | 10.0.22621.0+, **with Debugging Tools for Windows** (not installed by default) | guide §4.2 |
| Python | **Set by the branch's `.vpython3`, not a universal ceiling.** Measured 2026-08-03: **both** `7103` and `7871` pin `python_version: "3.11"`. depot_tools also ships its own interpreter. Re-read `src/.vpython3` on each bump instead of trusting a fixed "3.12+ breaks" rule | `src/.vpython3` @ branch |
| Mac | **Xcode 26.5 (build `17F42`) + SDK 26.5 on macOS 26.x Tahoe**, plus the **separately-downloaded Metal toolchain** and `clang-format` on `PATH`; arch auto-detect (`--arm64-build` Apple Silicon / `--x64-build` Intel). "Xcode + CLT" is **no longer sufficient on Chromium 150** — see §macOS below | `.sh`, measured 2026-08-05 |
| Archive format | `tar.bz2` (`CEF_ARCHIVE_FORMAT`) | `.bat` |
| Resources | ~100 GB disk (150 GB SSD rec.), 16 GB RAM min (32 rec.), 4 cores min (8+ rec.) | `.sh` header / guide §3 |
| Build duration | ~10–12 hr first build (download+compile+package); 30–60 min incremental | guide §1 (real build) |
| Output (Win) | `chromium_git/chromium/src/cef/binary_distrib/cef_binary_136.*_windows64_minimal/` | both scripts |
| Output (Mac) | same path; produces `Chromium Embedded Framework.framework` (not `libcef.dll`) | `.sh` |
| libcef.dll size | ~239 MB with codecs (vs ~224 MB prebuilt — the 15 MB delta is the codec code) | 2026-03-12 build |

---

## The full-build checklist

### Step 0 — Decide WHY this full build is happening
Trigger is one of: (a) Chromium/CEF version bump (A2), (b) new/changed farbling patches (B1),
(c) codec/flag change, (d) Widevine/VMP change. Record the trigger in the build's changelog entry.

### Step 1 — Choose the CEF branch (A2: latest stable / LTS)

CEF branches map 1:1 to Chromium milestones (branch `7103` = M136). The CEF version/branch mapping
and "what's current stable" come from the CEF release surface:
- **CEF builds CDN / version list:** https://cef-builds.spotifycdn.com/index.html (gives version → branch).
- **Chromium release schedule (for milestone exit dates / LTS windows):** the Chromium Dash schedule.

| CEF Version | Chromium | Branch | CEF channel (verified 2026-08-03) |
|-------------|----------|--------|-----------------------------------|
| CEF 151 | Chromium 151 | 7922 | Beta |
| **CEF 150** | **Chromium 150** | **7871** | **Stable → LTC since 2026-07-21 → LTS 2026-10-06. ⭐ OUR TARGET** |
| CEF 149 | Chromium 149 | 7827 | **Unsupported** (already dropped off the supported table) |
| CEF 144 | Chromium 144 | 7559 | LTS (expires 2026-10-06, hands off to M150) |
| CEF 136 | Chromium 136 | 7103 (**what we ship**) | **Unsupported** |
| CEF 127 | Chromium 127 | 6533 | Unsupported |

> ### ⚠️ Pin to a CEF LTS branch — target **M150 / branch `7871`** — NOT newest stable
>
> **The LTS program is REAL — re-verified from primary sources 2026-08-03.** This block was written
> 2026-06-16, then contradicted by a 2026-06-17 note (now in `../0.4.0/archive/SPRINT_0_4_0_MASTER_PLAN.md`)
> claiming *"CEF publishes only stable + beta — there is NO CEF LTS channel, so the 'pin to LTS (M150)'
> premise was false."* **That correction was itself wrong.** The runbook's original guidance stands.
>
> **How the confusion happened — and the trap to avoid:** `cef-builds.spotifycdn.com/index.json`
> exposes only `"channel": "stable"` and `"channel": "beta"`. There is **no `lts` enum in the JSON**, so
> LTS builds are labelled `stable` there. The 2026-06-17 note read the JSON and concluded no LTS exists.
> The authority is the **website table** (`branches_and_building.html`), not the JSON.
> **⇒ Any automation must key off the BRANCH NUMBER (`7871`), never the JSON `channel` field.**
>
> **Proof the train is running:** M144 (branch `7559`) branched Dec 2025, exited stable months ago, and
> still shipped a fresh build on 2026-07-31 — newer than any M150 or M151 build. Meanwhile M149 (`7827`)
> is *already* unsupported. An LTS branch outliving a newer stable one only makes sense if LTS is real.
>
> **M150's window (chromiumdash, cross-checked against CEF's "Last Refresh" column — exact match):**
> stable 2026-06-30 · **LTC 2026-07-21** · **LTS 2026-10-06** · security refresh ends **2027-04-13**.
> Roughly 9 months of coverage. M151 is not a 6th branch and gets **no LTS at all** — which is why we
> take 150 over the newer 151.
>
> **Two limits to hold in mind:**
> 1. LTS carries **platform-agnostic Chromium security fixes only**. A Windows-sandbox- or macOS-specific
>    CVE may not be backported and could force an off-cadence jump.
> 2. **CEF's own fixes are not backported.** Maintainer, CEF issue #3947: *"There is no plan to actively
>    backport CEF fixes to LTC/LTS branches. It may still occur in certain rare cases."*
>
> Our current `7103`/M136 predates the M138 LTS program entirely → **ZERO current security coverage.**
>
> **Primary sources:** https://chromiumembedded.github.io/cef/branches_and_building.html ·
> CEF issue [#3947](https://github.com/chromiumembedded/cef/issues/3947) (closed 2025-08-07, LTC builds
> live; cadence: ~1 mo stable → ~2.5 mo LTC → ~6 mo LTS ≈ 9.5 mo total) ·
> CEF issue [#4114](https://github.com/chromiumembedded/cef/issues/4114) (open — Chromium's 2-week
> cadence from Sept 2026 *"will not impact LTC/LTS builds which will continue to run with an ~9 month
> lifespan"*; note CEF's **stable**-channel policy after Sept 2026 is still undecided, which strengthens
> the case for anchoring on LTS).

> ### Cadence — two distinct rebases
> - **Quarterly (cheap):** pull the latest **security point-release** of the pinned LTS branch. Patches
>   (codec flags, B1 farbling) usually **re-apply trivially**. Light dependency pass (see
>   `DEPENDENCY_VERIFICATION.md`).
> - **~6-monthly (expensive):** **milestone jump** to the *next* LTS (e.g. M150 → M156). **Budget
>   patch-rework** + a **full** dependency-verification pass + full regression + codec re-verify.

> ### Drift red-line
> A self-build is **dangerously stale** once its branch is **past its Chromium stable-exit date AND
> outside any LTS window** — track both via the **Chromium Dash schedule**. (M136 is already past this
> line today, which is why the M150 move is the priority.)

**Compatibility gate (A2):** before committing to a new milestone, list what a Chromium jump may
break — CEF API changes (handler signatures), removed flags, V8/Blink behavior, our patch rebase.
Diff CEF's release notes between branches and run the **dependency-verification** pass below.

### Step 2 — Apply OUR source modifications (before build)
1. **Codec flags** — confirm `GN_DEFINES` includes `proprietary_codecs=true ffmpeg_branding=Chrome`.
   (We set codecs via `GN_DEFINES`, **not** the `--proprietary-codecs` automate-git flag — more reliable.)
2. **Farbling patches (B1)** — **toolchain LIVE as of 2026-08-05 (P3).** Our patches live in the
   **`Hodos-Browser/cef` fork** on branch **`hodos/7871`**, under `patch/patches/hodos_*.patch` and
   registered in `patch/patch.cfg`, gated by the single **`HODOS_FARBLING`** env var. Nothing to do
   per build: they are applied automatically by **`cef/tools/gclient_hook.py` → `tools/patcher.py`**
   during the build step of the `automate-git` flow. The build scripts already pass
   `--url=https://github.com/Hodos-Browser/cef.git` and pin `--checkout` at a fork commit.
   - **Do not write "`run_patch_updater` applies them"** — it does not; on a pinned checkout that
     function never applies anything. See `../0.4.0/chromium-rebuild/PLAN_patch_toolchain.md` §1.3.
   - **Adding a patch:** land it on `hodos/7871`, then **bump `CEF_CHECKOUT`** in both build scripts and
     record the new SHA in the fork's `HODOS_PATCHES.md`. A moving branch tip is not a reproducible build.
   - **Turning farbling off** for a build: unset `HODOS_FARBLING`. The patches are *skipped*, no
     `patch.cfg` edit and no revert needed. All-or-nothing — never gate a subset.
   - Log the patch count from the build log per build ("`N patches total`"): it is the cheapest detector
     of a stale in-tree `src/cef` copy silently dropping every Hodos patch.
   Plan: `PLAN_patch_toolchain.md` · evidence: `P3_TOOLCHAIN_PROOF.md` · content design (C1–C7, still to
   author): `PLAN_farbling_blink.md`.
3. **Extensions** — **N/A on CEF.** Extensions are chrome-layer; self-build does NOT unlock them. Do
   not add extension patches here. (Strategic future item; see `../Future-Features/B4-extensions.md`.)
4. **Any other custom patches** — list and version them.

### Step 3 — Build

#### One-time environment setup (Windows)
1. **Visual Studio 2022** (Community works). Workloads: *Desktop development with C++*,
   *Game development with C++* (extra SDKs). Individual components: latest Win 10/11 SDK,
   C++ CMake tools, C++ Clang compiler.
2. **Windows SDK → Debugging Tools for Windows** — *not* installed by default. Settings → Apps →
   "Windows Software Development Kit" → Modify → check **Debugging Tools for Windows**.
3. **Python 3.9–3.11** on PATH (verify `python --version`; **3.12+ breaks the build**).
4. **Disable Windows Defender real-time scan for the build dir** — add folder exclusions for `C:\cef\`
   and `C:\cef\depot_tools\`. Defender on millions of small files = 2–5× slower build. Re-enable after.
5. **Pause Windows Update / disable auto-restart** for the build window (an overnight compile **will**
   be killed by a forced restart — see Lessons). `gpedit.msc` → Windows Update → "No auto-restart with
   logged on users", plus Pause Updates + Active Hours.
6. **Short, ASCII-only base path** — use `C:\cef\` (Windows 260-char path limit; Chromium's tree is
   deep). Optionally enable long paths:
   `New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" -Name LongPathsEnabled -Value 1 -PropertyType DWORD -Force` (admin).

#### depot_tools + automate-git.py (Windows)
```powershell
mkdir C:\cef ; mkdir C:\cef\automate ; mkdir C:\cef\depot_tools ; mkdir C:\cef\chromium_git

# depot_tools (Google's Chromium build tooling).
# ⚠️ Clone FULL, never `--depth 1`. CEF pins an EXACT depot_tools commit in
# cef/CHROMIUM_BUILD_COMPATIBILITY.txt and automate-git.py checks it out; a
# shallow clone fails with "fatal: reference is not a tree: <sha>".
# (Hit for real on the 2026-08-03 7871 checkout — see Lessons.)
cd C:\cef
git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git C:\cef\depot_tools
cd C:\cef\depot_tools ; .\update_depot_tools.bat
$env:PATH = "C:\cef\depot_tools;$env:PATH"     # (or add permanently via System env vars)
gclient --version                               # sanity check
# Already cloned shallow? Recover without re-cloning:  git fetch --unshallow
```

**`automate-git.py` — use the BRANCH-MATCHED copy, not `master`.**
`automate-git.py` is versioned with CEF, so the right copy is the one inside the CEF checkout the
build is actually using: `<tree>\cef\tools\automate\automate-git.py`. Fetching it from `master` (as
this runbook previously instructed) drifts it away from the branch and was found to differ on `7871`.
Chicken-and-egg on a brand-new tree: run *once* with any recent copy to clone `cef/`, then re-run
using the checkout's own copy — or clone `cef/` by hand first.

#### Run the build (Windows)
Canonical script: `development-docs/DevOps-CICD/scripts/build_hodos_cef.bat` (copy to `C:\cef\chromium_git\` and run from there in a
**normal** cmd/PowerShell, *not* a Developer Command Prompt). What it does:

```batch
set GYP_MSVS_VERSION=2022
set DEPOT_TOOLS_WIN_TOOLCHAIN=0
set GYP_MSVS_OVERRIDE_PATH=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools
set vs2022_install=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools
set GN_DEFINES=is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0
set CEF_ARCHIVE_FORMAT=tar.bz2
set PATH=C:\cef\depot_tools;%PATH%

REM Pre-fetch deps (ninja, node, etc.), then run the build
gclient sync --nohooks --no-history
gclient runhooks
python C:\cef\automate\automate-git.py ^
  --download-dir=C:\cef\chromium_git ^
  --depot-tools-dir=C:\cef\depot_tools ^
  --branch=7103 ^
  --x64-build --minimal-distrib --client-distrib --no-debug-build --force-build
```

`automate-git.py` flags we rely on:

| Flag | Purpose |
|------|---------|
| `--branch=7103` | CEF/Chromium milestone (update for a bump — see Step 1) |
| `--x64-build` / `--arm64-build` | target arch |
| `--minimal-distrib` | smaller output (no debug symbols) |
| `--client-distrib` | include `cefclient` for testing |
| `--no-debug-build` | Release only (faster) |
| `--force-build` | force rebuild **but keep existing objects → resumable** (we do NOT use `--force-clean` on re-runs) |

#### macOS
Use `development-docs/DevOps-CICD/scripts/build_hodos_cef_mac.sh`. Same `automate-git.py` + ninja flow.
Arch: `--arm64-build` on Apple Silicon (M1+), `--x64-build` on Intel. Output is
`Chromium Embedded Framework.framework` instead of `libcef.dll` — Windows DLLs cannot be used on macOS;
this is a fully separate build.

##### Disk — budget 150 GB+, and NEVER delete `src/.git` to reclaim it

Measured on a 7871 mac tree: **~53 GB before the build, ~123 GB after.** The finished
`out/Release_GN_arm64` alone was **56 GB for a *non-official* build**; `is_official_build=true` adds
ThinLTO objects and multi-GB dSYMs on top. Budget **150 GB+**, and treat a 256 GB machine as
genuinely marginal for this work.

> ⛔ **Do not delete `chromium/src/.git` to reclaim space.** Done on the mac host in an earlier
> session (~97 GB reclaimed) and it is a trap with a delayed bill: the dependency repos under
> `third_party/` keep their own `.git`, so the tree *looks* intact and **still builds happily under
> ninja** — but `src` is no longer a git repository, so `gclient sync` cannot run against it and
> `automate-git.py` cannot update or re-pin the checkout. **You keep the ability to rebuild what you
> already have and lose the ability to change what you are building**, which is the more valuable
> half — and the loss is invisible until the next pin bump.
>
> **Recovery if it is already gone:** `automate-git.py --no-chromium-history` is the cheap path — it
> pins the gclient URL to `@<version>` and syncs without history. Its validation
> (`automate-git.py:1423-1436`) compares `chrome/VERSION` against the target, so a tree already at the
> right version is **reused rather than re-fetched**, provided you also pass `--force-update` or
> `--force-clean`. A full-history re-clone is the expensive alternative.

##### Toolchain — Chromium 150 needs more than "Xcode + CLT"

| Component | Required | Notes |
|---|---|---|
| macOS | 26.x (Tahoe) | needed to run Xcode 26 |
| Xcode | **26.5** (build `17F42`), SDK 26.5 | see rationale below |
| Metal toolchain | separate 688 MB download | **not bundled with Xcode 26** |
| `clang-format` | from the Chromium tree | must be on `PATH` at packaging time |

**Why 26.5 specifically, not 26.6+.** `build/config/mac/mac_sdk.gni:51` pins
`mac_sdk_official_version = "26.5"`, but that exact pin only binds for `is_official_build` *with
hermetic Xcode*. With system Xcode the real constraint is `mac_sdk_min = "15"`
(`build/config/mac/mac_sdk_overrides.gni:10`) — a floor, not an exact match, so a newer SDK configures
fine. 26.5 is chosen deliberately anyway: **Chromium builds `-Werror`, and a newer SDK can introduce
fresh deprecation warnings that break the build for no benefit.** Pin the whole team to 26.5.

```bash
brew install aria2          # optional; the .xip is ~12 GB and aria2 parallelises it
xcodes install 26.5         # prompts for Apple ID + 2FA (App Store only offers latest)
sudo xcode-select -s /Applications/Xcode-26.5.0.app/Contents/Developer
sudo xcodebuild -license accept
sudo xcodebuild -runFirstLaunch
xcodebuild -downloadComponent MetalToolchain     # 688 MB; sudo NOT required
export PATH="<tree>/chromium/src/buildtools/mac_arm64-format:$PATH"   # clang-format
```

##### ⛔ Preflight — run this BEFORE the multi-hour phases

Both toolchain blockers surface only *after* long phases (one ~10 min in, one after the entire
compile). Assert them up front:

```bash
xcrun --show-sdk-version | grep -q '^26\.' || { echo "Need macOS SDK 26.x (Xcode 26.5)"; exit 1; }
xcrun metal --version >/dev/null 2>&1 || { echo "Metal toolchain missing: xcodebuild -downloadComponent MetalToolchain"; exit 1; }
command -v clang-format >/dev/null || { echo "clang-format not on PATH (buildtools/mac_arm64-format)"; exit 1; }
```

⚠️ **Check `xcrun metal --version`, never `xcrun -f metal`.** When the Metal toolchain is missing the
`metal` binary is still present as a **stub**, so `xcrun -f metal` **succeeds and is not a valid
check**.

##### ⚠️ `make_distrib.py` flags are NOT `automate-git.py` flags

The two sets look interchangeable and are not. `build_hodos_cef_mac.sh` passes `--minimal-distrib
--client-distrib --no-debug-build` to **`automate-git.py`**, which is correct there —
`automate-git.py` invokes `make_distrib.py` once per distrib type internally. Passed to
`make_distrib.py` **directly**, the equivalents differ:

| Flag | On `automate-git.py` | On `make_distrib.py` |
|---|---|---|
| `--no-debug-build` | valid | **does not exist** — `--minimal` already means release-only |
| `--minimal-distrib` / `--client-distrib` | valid together | `--minimal` + `--client` **hard-error as mutually exclusive** (`make_distrib.py:765`) |
| output location | derived | `--output-dir` is **required** |
| `--arm64-build` | — | **required on macOS**, despite help text saying *"(Linux only)"* |

**`--arm64-build` is the dangerous one — its help string is wrong.** Without it `platform_arch`
silently falls back to `'32'`/x86 (`make_distrib.py:842-853`), producing a **mislabeled distribution
rather than an error**. Correct direct invocation:

```bash
python3 make_distrib.py --ninja-build --arm64-build --minimal \
        --output-dir "<tree>/chromium/src/cef/binary_distrib"
```

Also: **missing Doxygen is non-fatal** — it prints `ERROR: Please install Doxygen` / `ERROR: No docs
generated.` and continues. Ignore it, or `brew install doxygen`.

#### A1 pain-reduction — build caching & remote/distributed build (verified 2026-06, master-plan §7.3)
The point of A1 is to make this not take ~2 weeks. Levers, in priority order:

- **Build caching — `sccache` + `chrome_pgo_phase=0`.** Setting `cc_wrapper="sccache"` (GN) routes
  compiles through sccache; with `chrome_pgo_phase=0` the toolchain **auto-drops** the MSVC `/Brepro`
  and `/showIncludes:user` flags that otherwise **block caching**. sccache supports **MSVC** and an
  **S3-backed shared cache** (share the cache across machines/CI).
  > **CAVEAT:** the oft-cited "~3× speedup" is a **WARM-cache / incremental** figure. A **cold,
  > from-scratch** build gets **no benefit from caching alone** — the first build still pays full cost.
- **Local dev-iteration levers (DEV ONLY):** `is_component_build=true symbol_level=0 is_debug=false`
  for fast iteration. **Component build is a DEV-ONLY layout, not a shippable single-binary release** —
  never ship a component build.
- **Remote / distributed build:** **reclient is being REMOVED from Chromium (~Sept 2026)** and replaced
  by **Siso**. Any remote-build investment must target **Siso + a third-party REAPI backend**
  (EngFlow / BuildBuddy free tier / NativeLink). **Google's hosted RBE is off-limits to non-Googlers.**
- **CI reality:** **GitHub-hosted runners CANNOT do a full Chromium build** (disk + 6 h job cap). The
  lowest-cost realistic path = **a self-hosted runner / beefy VM for the cold build + a shared sccache
  for incrementals.** (Spot VM for the cold build is fine; keep the sccache warm between runs.)
- **Linux:** placeholder only — not a current target.

### Step 4 — Stage & publish binaries
- Copy `cef_binary_136.*` output → `cef-binaries/`. Back up the current `cef-binaries/Release` first.
  Copy `Release/`, `Resources/`, `include/`, and the wrapper source into the matching
  `cef-binaries/...` locations (see `BUILD_AND_RELEASE.md` §2.3 for the directory layout).
- **Rebuild `libcef_dll_wrapper`** (it must match the new headers): delete `build/CMakeCache.txt` +
  `build/`, then `cmake -G "Visual Studio 17 2022" -A x64 ..` and `cmake --build . --config Release`.
- **Rebuild `cef-native`** against the new wrapper/headers.
- **Publish** the binary distribution to the **`cef-binaries` GitHub release** so the Tier-2 app
  pipeline (`release.yml`) consumes it — CI pulls `cef-binaries-windows.zip` /
  `cef-binaries-macos.tar.bz2` from that release.

### Step 5 — Dependency reconciliation (A3)
After a Chromium/CEF bump, run the full **`DEPENDENCY_VERIFICATION.md`** checklist for **Hodos's own
deps** (the hard part of a bump is *our* deps staying ABI/toolchain-compatible with the new CEF, not
Chromium's internal deps which gclient resolves automatically). Re-check everything pinned to the old
engine and **annotate** what needs updating:
- Frontend: React/Vite/TypeScript + any browser-API-dependent JS/TS.
- Rust (`rust-wallet`, `adblock-engine`): crates sensitive to platform/toolchain.
- C++: vcpkg deps (nlohmann-json, sqlite3, OpenSSL), quirc.
- Record a per-bump "dependencies touched / deferred" table (the verification doc captures this).

### Step 5.5 — Build-config & file-manifest drift audit (CEF-bump only)
A successful compile does **NOT** prove the build is correct after a bump. Compile-time CEF API changes
fail loudly; **silent config drift survives a green build.** On every CEF/Chromium bump, audit OUR build
glue (not just deps) for drift:
- **Runtime file manifest** — diff the new CEF dist's file list (DLLs, `.bin`, `.pak`, `resources/`,
  `locales/`) against the hardcoded copy-lists in `cef-native/CMakeLists.txt` ("Copying CEF binaries"
  step) **and** the macOS framework-embed list in the mac build script. A new/renamed/removed file we
  don't copy = green build, runtime crash or missing feature. Cross-check the **Output file checklist**
  below.
- **GN args / `args.gn`** — diff our pinned `GN_DEFINES` against the new CEF's defaults; confirm the
  proprietary-codec flag (`ffmpeg_branding=Chrome`) and other required overrides still take effect (a
  flipped default ships a green build with no codecs).
- **cmake / toolchain** — CEF version macro, sandbox/linking changes, vcpkg ABI, wrapper rebuild
  (`Unsupported CEF version` ⇒ delete `CMakeCache`, rebuild — see Lessons).
- **Patches** — run **`scripts/cef_patch_drift_audit.sh`** (landed 2026-08-05, P3/CEF-2). Exit `0` clean ·
  `2` hunk **offsets** present (may proceed with sign-off; the patch is drifting and will likely break at
  the next milestone jump) · `1` a patch will not apply → **do not start the build**. It also chains the
  file-manifest audit via `--with-dist`.
  > **There is no "fuzz" to report here** — CEF applies with `git apply -p0 --ignore-whitespace`, which is
  > exact-context and **fail-loud**: a context mismatch aborts before compile rather than fuzzily landing a
  > hunk in the wrong place. The only sub-failure signal is a hunk applied at a line **offset**. Do not key
  > any check on a fuzz metric; there isn't one.
  >
  > **⚠️ Also confirm the patch COUNT in the build log** (`N patches total`). `chromium/src/cef` is a
  > *copy* refreshed only when the CEF checkout hash changes, so a stale copy means the build silently
  > compiles **none** of our patches — with a green run and correct-looking checkouts. See
  > `PLAN_patch_toolchain.md` §1.3 and the warning block in the build scripts. Fix: `--force-cef-update`.
- **macOS parity** — `Info.plist` CEF framework version, helper-app embedding, entitlements.
Emit a **human-review diff report** (manifest + args diffs are scriptable; cmake changes need judgment —
never auto-apply). Until scripted (see Open TODOs), run this as a checklist on every bump.

### Step 6 — Widevine / premium DRM (separate track)
- Basic DRM (CDM auto-download) works on the codec build already: `enable_widevine=true` is set
  automatically by CEF's build system (no manual flag). The actual `widevinecdm.dll` is **NOT** in the
  output — Chromium's component updater auto-downloads it at runtime (~5 min after first launch). No
  license needed for the auto-download path. Once it lands, basic DRM content works.
- Premium (Amazon/Netflix HD) needs **VMP signing** of our binaries — its own mini-spike (Castlabs
  commercial path vs Google MLA), **not** part of the routine build. **Widevine/DRM is a SEPARATE
  concern and is not covered by self-building** (self-build is for codecs + farbling).

### Step 7 — Verify (acceptance gate)
- **Codecs (re-verify EVERY bump — flags persist but smoke-test for real):**
  `video.canPlayType('video/mp4; codecs="avc1.42E01E"')` → `'probably'`. Also check H.264 High, AAC
  (`mp4a.40.2`), MP3 (`audio/mpeg`), VP9, AV1. Then smoke **real** video/audio/image playback on
  x.com (video + animated GIF), Reddit, Twitch, YouTube, plus an audio site.
- **Farbling (once B1 lands):** CreepJS / fingerprintjs show no "lie"; logins that broke before now
  work; **workers** report farbled values (the current gap).
- **Regression:** the standard site basket (CLAUDE.md Testing Standards) on **both Windows and macOS**.

### Step 8 — Record the build
- Changelog entry: CEF branch, Chromium milestone, GN_DEFINES, patch set version, deps touched,
  verification results, build duration. Append to `CEF_VERSION_UPDATE_TRACKER.md` — that's the
  institutional memory for the next full build.

---

## Lessons learned

### Counting `patch.cfg` entries — anchor the grep, then stop counting totals

Hit **twice**, independently, on both platforms — Windows during P3 and macOS on 2026-08-06 — so it is
a pattern, not an accident:

```bash
grep -c "'name'"      patch/patch.cfg    # 116 -- WRONG, counts the doc comment
grep -c "^\s*'name'"  patch/patch.cfg    # 115 -- real entries (114 upstream + our C1)
```

`patch.cfg` is Python whose **header comment documents the format** and contains the literal `'name'`
on line 7 (`# - 'name'  Required. …`), so an unanchored grep counts the documentation as an entry.
On upstream `94c17267e` the true figures are **114 registered entries** and **115 `.patch` files** —
the extra file being the known unregistered orphan `chrome_browser_privacy_1119417`.

**The durable fix is not a better grep, it is not gating on a total at all.** The expected total
changes on every patch landed, so the gate needs hand-editing each time, and a gate that must be
hand-updated eventually gets updated wrongly. `cef_patch_drift_audit.sh` now (a) `exec`s `patch.cfg`
the way `patcher.py` does rather than grepping it, (b) gates on `hodos_*.patch` **presence**
(`HODOS_MIN_PATCHES`, default 1), and (c) — the load-bearing check — compares the **standalone
checkout's** Hodos patch set against the **in-tree copy's**, since a copy stale by exactly one new
patch still clears any fixed floor. The patcher's `N patches total` line stays useful as a cross-check
and as the cheapest stale-copy tell in a raw build log; it is not the gate.

### From the 2026-08-05 macOS CEF 150 build (Xcode 26 / Tahoe)

A full CEF 150 macOS ARM64 build completed green: **57,901 ninja targets, 0 failures, ~4 h 30 m**.
Four blockers were hit; **all four were environment/toolchain gaps introduced by the Xcode 26
transition** — none were defects in CEF or Chromium source. The requirements they establish are in
§macOS above; this is the diagnosis record.

> ⚠️ **That binary was UPSTREAM CEF, not the Hodos fork** — `cef` remote was
> `chromiumembedded/cef`, HEAD `94c17267e`, `patch/patches/hodos_*.patch` = **0 present**, version
> string `150.0.17+g94c1726+chromium-150.0.7871.187`. It was built from a hand-rolled tree, not via
> `build_hodos_cef_mac.sh`, so it never honoured the `CEF_CHECKOUT` fork pin. **What it proves is the
> toolchain, which is the transferable result. It is not a distributable Hodos CEF.**

| # | Symptom | Fires at | Cause / fix |
|---|---|---|---|
| 1 | `skia_utils_mac.mm:84:11: error: use of undeclared identifier 'kCGImageByteOrder32Host'; did you mean 'kCGImageByteOrder32Big'?` | ~object 4,825/58,002, ~10 min in | SDK 15.x too old — that identifier exists only in the macOS 26 SDK. Fix: Xcode 26.5. **Also remove the `use_clang_modules=false` workaround** that SDK 15 needed for incompatible modulemaps; modules are fine on SDK 26 and leaving it off costs Objective-C compile time |
| 2 | `error: cannot execute tool 'metal' due to missing Metal Toolchain; use: xcodebuild -downloadComponent MetalToolchain` | ~object 5,766/57,901, ~6 min in | Apple unbundled the Metal compiler in Xcode 26 to shrink the download. ANGLE needs it to compile `.metal` → `.air`. **The stub binary makes `xcrun -f metal` succeed** — check `xcrun metal --version` |
| 3 | `FileNotFoundError: [Errno 2] No such file or directory: 'clang-format'` at `clang_util.py:44` → `make_distrib.py:319 transfer_gypi_files()` | **after the multi-hour compile** | `make_distrib.py` reformats the headers it copies and invokes `clang-format` **by bare name**, so it must resolve via `PATH`. Ships in-tree at `buildtools/mac_arm64-format` |
| 4 | `FileNotFoundError: .../out/Release_GN_arm64/Chromium Embedded Framework.dSYM` | packaging | **Not a defect.** `make_distrib.py:1392`: *"dSYMs are only generated when `is_official_build=true` or `enable_dsyms=true`."* That build used `is_official_build=false`. **The real Hodos path uses `is_official_build=true`, so this should not appear there** — recorded only so it isn't misdiagnosed |

**Low-memory host tuning (16 GB M1 — specific to that machine, not a general recommendation).**
`ninja -j 8` instead of the default `ncpu+2 = 10`: some Chromium TUs peak near 1 GB and 10 concurrent
jobs push a 16 GB machine into swap, which is net slower. At `-j 8` swap stayed at **0.00 MB**, memory
free ~76%, no thermal throttling throughout. `is_official_build=false` was also used because official
enables ThinLTO + whole-program devirtualization, and the Chromium Framework **link** is the single
most memory-hungry step — a genuine OOM risk on 16 GB *after* hours of compiling. **The shippable
Hodos build wants `is_official_build=true`, so use a larger host.** Flipping that flag invalidates
**every** object file; decide before starting.

**Timing reference (8-core M1, `-j 8`):** GN gen ~10 s · full compile+link of 57,901 targets ~4 h 30 m
· `make_distrib.py --minimal` ~2 m. **Early progress badly overstates throughput** — the first ~5,000
objects (sqlite, brotli, boringssl) fly by in minutes, then Blink/V8/WebRTC dominate. Do not
extrapolate from the first 10 minutes.

**Output verification that was performed** (`cef_binary_150.0.17+g94c1726+chromium-150.0.7871.187_macosarm64_minimal`,
712 MB unpacked / 229 MB zip): framework is Mach-O 64-bit dylib **arm64**, `lipo -archs` = `arm64`,
install name `@executable_path/../Frameworks/...`, **`LC_BUILD_VERSION minos` = 12.0** (matches VER-4's
floor exactly — closes that open question), `otool -L` resolves clean. `cefclient.app` launched and
stayed up: 1 browser, 1 `--type=gpu-process`, 2 `--type=renderer`, 2 `--type=utility`, one on-screen
window 800×632.

Two macOS verification tricks worth keeping:

- When `osascript`/`screencapture` are blocked by missing Accessibility / Screen Recording permission
  (both are, on a fresh Tahoe install), enumerate windows through Quartz instead:
  ```bash
  /usr/bin/python3 -c "
  import Quartz
  wl = Quartz.CGWindowListCopyWindowInfo(
      Quartz.kCGWindowListOptionOnScreenOnly | Quartz.kCGWindowListExcludeDesktopElements,
      Quartz.kCGNullWindowID)
  print([(w.get('kCGWindowOwnerName'), w.get('kCGWindowBounds')) for w in wl])"
  ```
- **`pgrep -f 'Helper (GPU)'` silently matches nothing** — `pgrep -f` takes an ERE, so the parentheses
  are group syntax. Escape them, or match on `--type=` instead.

### From the 2026-08-03 M150 / `7871` checkout

- **Clone `depot_tools` FULL. `--depth 1` breaks the checkout.** CEF pins an exact depot_tools
  commit in `cef/CHROMIUM_BUILD_COMPATIBILITY.txt` (alongside the Chromium tag), and
  `automate-git.py` does a hard `git checkout <sha>` on it. A shallow clone dies with
  `fatal: reference is not a tree: <sha>` **after** cloning `cef/` — far enough in to look like
  progress. Recovery is cheap and does not need a re-clone: `git fetch --unshallow`.

- **`automate-git.py` is versioned with CEF — use the checkout's own copy.** The copy at
  `<tree>\cef\tools\automate\automate-git.py` differs from `master` on `7871`, and differed from
  our M136-era copy at `C:\cef\automate\`. This runbook previously told you to fetch it from
  `master`; that instruction was wrong and has been corrected above.

- **`CHROMIUM_BUILD_COMPATIBILITY.txt` is the ground truth for what a CEF pin means.** For
  `94c1726` (= `150.0.17`) it reads `chromium_checkout: refs/tags/150.0.7871.187` — i.e. pinning
  the CEF commit pins the Chromium tag transitively. Read this file to confirm a pin rather than
  inferring the Chromium version from the CEF version string.

- **Running a `.bat` from git-bash via `cmd.exe /c` silently no-ops.** MSYS rewrites the `/c` flag
  into a path, so `cmd` starts *interactively*, prints its banner, hits EOF and exits **0** — a
  green exit with zero work done.

  > ⛔ **CORRECTED 2026-08-07: `MSYS_NO_PATHCONV=1` and `cmd //c` are NOT reliable fixes.** This
  > runbook previously listed both as workarounds. Launching the C3 build as
  > `MSYS_NO_PATHCONV=1 cmd //c "…build_hodos_cef.bat > log 2>&1"` reproduced the failure exactly:
  > exit 0 in under a minute, **no log file created at all**, and the captured output was the cmd
  > banner followed by an interactive prompt. Do not trust either form.
  >
  > **What works:** put the invocation in a **`.ps1` wrapper file** and run
  > `powershell -NoProfile -ExecutionPolicy Bypass -File <wrapper>.ps1`. A file has no
  > slash-arguments for MSYS to rewrite, so the command reaches `cmd` intact. See
  > `C:\cef\cef150\run_build_c3.ps1` for the working shape. Driving `automate-git.py` directly from
  > bash with exported env vars also works and keeps exit codes real.
  >
  > **Two consequences of the PowerShell route worth knowing before you read the log:** the log is
  > **UTF-16**, so `grep` finds nothing until you strip NULs (`tr -d '\0' < build.log | grep …`);
  > and PowerShell reports the child's *stderr* as `NativeCommandError` records, which look like
  > failures and are not — `*>&1` merges them into the stream. Neither affects the build.
  >
  > ⛔ **The `.ps1` wrapper has TWO MORE traps of its own. Both cost a "green" no-op run on
  > 2026-08-07, and both are silent in exactly the same way as the original bug.**
  >
  > 1. **QUOTE THE WRAPPER PATH, or use forward slashes.** From the Bash tool,
  >    `-File C:\cef\cef150\run_build_pull.ps1` has its backslashes eaten by the shell before
  >    PowerShell ever sees them; PowerShell then reports
  >    `The argument 'C:cefcef150run_build_pull.ps1' ... does not exist`, and the surrounding
  >    pipeline still exits **0**. Use `-File 'C:/cef/cef150/run_build_pull.ps1'` (quoted, forward
  >    slashes). The bare-backslash form appears in older session notes — it is wrong from bash.
  > 2. **`Start-Process -RedirectStandardInput 'NUL'` throws `FileNotFoundException`.** `NUL` is a
  >    device, not a file, and `Start-Process` insists on a real path. Redirecting stdin at all is
  >    worth doing — `build_hodos_cef.bat` ends in `pause`, which can block forever with the build
  >    already finished — so create a zero-byte file (`C:\cef\cef150\empty_stdin.txt`) and point at
  >    that instead.
  >
  > **The common failure signature for all of these: exit 0, and a log file that is empty or a few
  > dozen bytes.** Before believing a fast build, check `ls -l` on the log. A real build log is
  > megabytes; a 72-byte log means nothing ran.

- ⛔ **`siso` HIDES COMPILE ERRORS when it detects an agent environment.** Discovered 2026-08-07.
  The build log ends with:

  ```
  ........Detected AI agent env. Prepending --quiet --batch=false --heartbeat_period=30s
          to improve latency and reduce context pollution.
  The build has finished with an error.
  ```

  — and **that is all you get**. `grep -i error` over the whole build log returns the summary line
  and nothing else: no file, no line, no diagnostic. The build genuinely failed; the reason is simply
  not in the log you were tailing.

  > ⭐ **CONFIRMED ENV-DEPENDENT 2026-08-09 (macOS round).** The Mac build did **not** hit this:
  > `.siso_failed_targets` absent, `siso_output` 799 bytes with one SUCCESS record. The trigger is
  > siso's own agent-environment detection — the banner above is siso announcing that it is adding
  > `--quiet` itself — so it fires from an agent session and not from a plain terminal.
  > **Do not try to predict whether it fired: read `siso_output` unconditionally.** Checking a file
  > that is usually boring is far cheaper than re-running a 5-hour build blind. The same two files
  > are also what prove a *green* build actually compiled something.

  > ⛔ **`NINJA_CORE_ADDITION` / `NINJA_CORE_LIMIT` DO NOTHING UNDER SISO** (found 2026-08-09, macOS).
  > `autoninja`'s `-j` computation (`autoninja.py:558-592`) is on the **ninja** path only. Capping
  > parallelism on a RAM-tight box with those vars produces no effect and no error. On a 16 GB M1
  > siso self-selected 8 concurrent compiles — correct there, but by luck, not by control. Neither
  > var is set in the Windows script (checked 2026-08-09).

  > ⛔ **Do NOT adopt `--no-chromium-history` in the Windows script** (Mac raised it 2026-08-09;
  > Windows agrees, and it is already absent). `automate-git.py:1423-1437` **deletes `chromium/src`
  > and re-fetches** when `chrome/VERSION` does not match the target. macOS needs it only because its
  > `chromium/src` is shallow; a checkout with real history has no reason to skip that fetch, and on
  > a 175 GB tree the downside is not "slow", it is catastrophic.

  **Where the error actually is** (all under `chromium/src/out/Release_GN_x64/`):

  | File | Contents |
  |------|----------|
  | `siso_output` | **the real compiler diagnostics** — `grep -E "error" siso_output` |
  | `.siso_failed_targets` | JSON naming the failed `.obj`, e.g. `{"failed":["obj/cef/libcef_static/frame_impl.obj"]}` — the fastest "which file" answer |
  | `siso_failed_commands.bat` | the exact failing command lines, re-runnable by hand |

  Do **not** conclude "green build, mysterious failure" from the main log alone, and do not go looking
  for a patch/checkout problem — check these three first. They pinpointed a one-line type error in
  seconds after the top-level log offered nothing.

- ⛔⛔ **CDP REPORTS HODOS'S HEADER AND ALL ~14 OVERLAYS AS `type: "page"`. A harness that picks
  "the first page target" will sometimes drive an OVERLAY instead of the tab.** This is not a
  farbling-specific problem — it applies to **any** CDP-driven test of page behaviour in this browser,
  so it belongs here rather than only in the farbling plan.

  On 2026-08-08 it manufactured a convincing fake bug: a feature appeared to work on some launches and
  fail on *every* navigation of others (6/6 vs 0/5), which read as a race. The harness was actually
  driving the **tab-list overlay** (`role: tablistpanel`), which legitimately does not get the
  page-content behaviour under test — and which target CDP returned first varied per launch, hence the
  "intermittent" appearance. The tell is in `debug_output.log`:

  ```
  🌐 Resource request: https://example.com/ (role: tablistpanel)     <-- WRONG BROWSER
  🌐 Resource request: https://example.com/ (role: tab_1)            <-- what you wanted
  ```

  ⛔ **Asserting `location.href` does NOT catch it.** Once the overlay has been navigated, its
  `location.href` genuinely is the URL you asked for.

  **The rule:** identify browser chrome **once, at startup, by CDP target id** — every
  `127.0.0.1:5137` target *except* the `/newtab` one (that is the tab; the rest are the header and
  overlays: `/tab-list`, `/menu`, `/wallet-panel`, `/downloads`, `/privacy-shield`, `/brc100-auth`,
  `/profile-picker`, `/site-info`, `/bookmarks`, …) — then exclude those ids for the rest of the run.
  **Never** identify the tab as "the target that is not `127.0.0.1:5137`": after the first navigation
  an overlay does not match that either. Cross-check the `role:` in the shell log when a CDP result
  surprises you. Also never create targets with `PUT /json/new` — those bypass `OnBeforeBrowse`
  entirely, so they fail against correct code.

- ⛔⛔ **A BUILD DETACHES THE FORK'S HEAD, so every commit you make AFTER a build silently leaves
  the branch behind.** This is the known "automate-git leaves DETACHED HEAD" trap, but the sharp edge
  is worse than "push pushes nothing" — **`git checkout hodos/7871` later REVERTS your working tree**,
  because the branch never moved. Observed 2026-08-07:

  ```
  dd4da3989 HEAD@{5}: commit: C2 ...              <- committed ON the branch
  dd4da3989 HEAD@{4}: checkout: moving from hodos/7871 to dd4da3989   <- automate-git DETACHED here
  3b1acaf97 HEAD@{3}: commit (amend): C2 ...      <- these three amends
  2fff6384d HEAD@{2}: commit (amend): C2 ...         all advanced a DETACHED HEAD
  116b7fd8b HEAD@{1}: commit (amend): C2 ...         while hodos/7871 stayed at dd4da3989
  ```

  Three rounds of compile fixes existed only as detached commits. `git log origin/hodos/7871..hodos/7871`
  still reported "1 commit ahead" and looked healthy, because it compares the *branch*, which was stale.

  **Check after every build, not just before every commit:**
  ```bash
  git -C C:/cef/cef150/cef rev-parse --abbrev-ref HEAD   # "HEAD" means DETACHED
  git -C C:/cef/cef150/cef log --oneline -1 hodos/7871   # must equal the SHA you built
  ```
  **Recovery is lossless** if the tree is clean — the commits are in the reflog, so do NOT reset first:
  ```bash
  git checkout --detach <built-sha>     # brings the working tree back to the good content
  git branch -f hodos/7871 HEAD         # move the branch to it
  git checkout hodos/7871               # reattach
  ```
  Then assert `branch tip == CEF_COMMIT_HASH` of the distrib you just built. If they differ, the pin
  in the build scripts points at content nobody can reproduce.

- ⛔ **A killed build looks EXACTLY like a compile error. Launch CEF builds DETACHED.**
  Any harness that caps how long a command may run (an agent's background-task timeout, a CI step
  timeout, an SSH session dropping) will kill `automate-git.py` mid-compile. What it leaves behind
  reads as a genuine failure:

  ```
  FAILED: … "./obj/cef/libcef_static/browser_info_manager.obj" CXX …
  err: exit=1
  ```

  **The tell is that there is no `error:` line anywhere** — not in the build log, not in
  `siso_output`, and `.siso_failed_targets` is empty or absent. `err: exit=1` with zero diagnostics
  means the compiler was *terminated*, not that it rejected the code. Do not go debugging your patch.
  Re-run; siso resumes from the existing objects.

  Launch so nothing upstream can kill it:

  ```bash
  powershell -NoProfile -Command "Start-Process -FilePath 'powershell' \
    -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','C:/cef/cef150/run_build_x.ps1' \
    -WindowStyle Hidden"
  ```

  That returns immediately and the build outlives the caller; watch
  `build_x.err.log` for the `BUILD_EXIT=` line the wrapper appends. A full build is hours and even an
  incremental libcef relink is well over 10 minutes, so this is the normal case, not the exception.

- ⚠️ **Adding ONE method to `cef.mojom`'s `BrowserFrame` obligates TWO classes, not one.**
  `CefBrowserFrame` is the obvious implementor (`libcef/browser/browser_frame.h`), but
  **`CefFrameHostImpl` also derives from `cef::mojom::BrowserFrame`**
  (`class CefFrameHostImpl : public CefFrame, public cef::mojom::BrowserFrame`) so it can receive
  calls forwarded from `CefBrowserFrame`. Miss it and the error is reported in a *third*,
  unrelated-looking file:

  ```
  cef/libcef/browser/browser_info.cc(184,30): error: allocating an object of
      abstract class type 'CefFrameHostImpl'
  ```

  — i.e. it points at the allocation site, not the missing override. Before adding a mojom method,
  enumerate implementors with
  `grep -rn "public cef::mojom::BrowserFrame\|CefFrameServiceBase<cef::mojom::BrowserFrame>" libcef/ tests/`.
  The same applies to `RenderFrame` and any other interface in that file.

- **`GURL::host()` returns `std::string_view` on Chromium 150, not `const std::string&`.**
  `const std::string h = url.host();` therefore does **not** compile (`std::string`'s `string_view`
  constructor is `explicit`, so copy-initialisation is not viable). Use `const std::string h(url.host());`
  — or keep the `string_view` if you never need an owning copy. A silent porting trap for any code
  moved from an older Chromium.

- **Toolchain measured on this host (2026-08-03):** MSVC **14.44.35207** (VS2022 BuildTools 17.14),
  Windows SDK **10.0.26100** + 10.0.22621, Python 3.10.11 on PATH (depot_tools supplies its own;
  `7871`'s `.vpython3` wants 3.11).

- **`gclient sync` dying on "Failed to remove path `_gclient_src_<random>`" does NOT mean re-clone.**
  Seen on the 7871 checkout after the full 65 GiB clone completed. gclient clones into a temp
  `chromium\_gclient_src_<random>` directory and then moves it to `chromium\src`. **The move
  succeeded**; what failed was deleting the now-empty temp directory, and gclient treats that as
  fatal. The give-away in the log is the line just above the traceback:

  ```
  rd exited with code 3221225794
  ```

  `3221225794` = `0xC0000142` = `STATUS_DLL_INIT_FAILED` — the `rd` **process could not start at
  all** (transient Windows resource/desktop-heap exhaustion after a very long, very
  process-heavy operation). It is not a file lock, not corruption, and not antivirus.

  **Recovery (seconds, not hours):**
  1. Confirm the clone really did land: `chromium\src` is a git repo with a multi-GB
     `size-pack` (`git -C chromium/src count-objects -vH`), and
     `chromium\_gclient_src_<random>` is **empty**. An empty working tree is expected — the clone
     is `--no-checkout`.
  2. `rmdir` the empty temp directory.
  3. Re-run the checkout. gclient sees `src` and continues into the DEPS sync.

  **Do not delete `src` and start over.** The failure happens *after* the expensive part, so a
  reflexive clean re-clone throws away hours of transfer to fix an empty folder.

- **A big cold checkout can get you HTTP 429'd by `chromium.googlesource.com` — and the throttle
  does not always say 429.** After the 65 GiB main-repo clone succeeded, the small DEPS sub-repo
  clones began failing with a *mix* of messages:

  ```
  fatal: unable to access '...': The requested URL returned error: 429
  fatal: expected 'packfile'
  fatal: expected flush after ref listing
  ```

  Only the first names the cause. The other two are the **same throttling**, seen as a response
  truncated mid-protocol, and read convincingly as repo corruption or a bad network. If a cold
  checkout starts failing on *small* third-party repos right after the *large* one succeeded,
  suspect rate limiting before you suspect breakage.

  **Diagnose cheaply — never re-run the whole checkout just to test.** One request settles it:
  `git ls-remote <the-repo-that-failed> HEAD`. Success means the window has already reopened
  (ours cleared within minutes).

  **Then resume at REDUCED parallelism.** `automate-git.py` hardcodes its sync arguments
  (`sync_args = '--nohooks --with_branch_heads'`) and has **no `--jobs` passthrough**, so re-running
  it retries at full parallelism and can re-trip the limit. Do the expensive part by hand, then hand
  back:

  ```bash
  # .gclient has managed:False, so gclient will NOT move src's revision for you.
  git -C chromium/src checkout --force <chromium-tag-sha>
  cd chromium && gclient sync --nohooks --with_branch_heads -j2   # the low -j IS the fix
  # then re-run automate-git.py — its own revert/sync is now cheap
  ```

  Wrap that sync in retry-with-backoff rather than a bare retry: a 429 is transient, and hammering
  it extends the block.

- **⛔ The codec gate must distinguish "codecs are broken" from "my gate is broken." They look
  identical.** The pre-build `gn args` gate reported:

  ```
  MISSING  proprietary_codecs  (flag renamed or removed?)  <-- GATE FAIL
  MISSING  ffmpeg_branding     (flag renamed or removed?)  <-- GATE FAIL
  MISSING  chrome_pgo_phase    (flag renamed or removed?)  <-- GATE FAIL
  MISSING  is_official_build   (flag renamed or removed?)  <-- GATE FAIL
  ```

  Read literally that says "the codec flags were renamed on M150" — a serious finding that would
  send you re-auditing `features.gni`. **It was wrong.** Nothing was renamed; the gate had failed to
  generate the GN projects at all.

  **The tell: implausible unanimity.** *All four* independent flags missing — including
  `is_official_build`, which has nothing to do with codecs — plus **every** recorded section empty
  (Widevine, HEVC, AV1). Four unrelated flags do not vanish together. When a check reports total
  failure across independent dimensions, suspect the check.

  Two real causes, both worth knowing:

  1. **`cef_create_projects.bat` must be run from `src/cef`, not `src`.** Its entire contents are
     `python3.bat tools\gclient_hook.py` — a **relative** path. From `src/` it resolves to
     `src\tools\gclient_hook.py`, which does not exist, and it exits without generating anything.
  2. **Hooks had never run.** Every `gclient sync` in the recovery path used `--nohooks`, and
     `automate-git.py` only runs hooks as part of its **build** step — which the `--no-build`
     checkout never reached. No `src/build/util/LASTCHANGE`, no toolchain, so `gn gen` cannot work.
     Gate on that file's existence and run `gclient runhooks` if it is absent.

  **Make the gate prove itself.** Before trusting a MISSING verdict, assert that
  `gn args --list` produced a plausible line count (hundreds, not 5) and that a known-stable
  control flag resolves. A gate that cannot tell its own failure from the failure it is looking for
  is worse than no gate — it burns the credibility you built it to provide.

  ✅ **Genuinely verified in the same run** (`PLAN_codecs.md` §7 step 2): the `media/BUILD.gn`
  coupling guard **still exists on `7871`** —
  `assert(... ffmpeg_branding != "Chromium", "proprietary codecs and ffmpeg_branding set to Chromium are incompatible")`
  — so the fail-loud safety net survived the 14-milestone jump.

- **`core.autocrlf=true` breaks the DEPS sync — and Git for Windows sets it by default.** The
  installer ships `core.autocrlf=true` in its **system** config
  (`C:/Program Files/Git/etc/gitconfig`), so it applies to every repo on the box. depot_tools prints
  a warning about it on every invocation; that warning is easy to scroll past and is **not**
  cosmetic.

  Symptom: `gclient sync` fails on a *freshly cloned* third-party repo with

  ```
  <repo> (ERROR) ... Rebase produced error output:
  error: Your local changes to the following files would be overwritten by checkout:
  ```

  listing essentially **every text file in the repo**. "Local changes" in a repo that was just
  cloned is the tell.

  **Why the main tree looks fine:** Chromium's own `src/.gitattributes` forces `text eol=lf` for
  every source extension, which overrides `autocrlf`. Verified — `src/DEPS` and `src/BUILD.gn` are
  LF-clean even with `autocrlf=true`. **`third_party` sub-repos carry no such protection**, so the
  failure appears only there, which makes it look like a problem with one unlucky dependency rather
  than a global config fault.

  **Fix it build-scoped, not machine-wide.** Editing the system config would change how *every*
  repo on the machine checks out, including this one. Instead point the build at its own config:

  ```bash
  export GIT_CONFIG_GLOBAL=C:/cef/cef150/gitconfig   # autocrlf=false, filemode=false,
                                                     # fscache=true, preloadindex=true
  ```

  Precedence is **system < global < local**, so a global `autocrlf=false` overrides the installer
  default without touching it.

  **Then re-sync with `--force --reset -D`.** `--reset` alone is not enough: it discards local
  modifications to **tracked** files, and the sync then fails a second time with a *different*
  message — "The following **untracked** working tree files would be overwritten by checkout".
  `-D` (`--delete_unversioned_trees`) is what clears those. Prefer letting gclient clean its own
  tree over hand-deleting sub-repo directories:

  ```bash
  gclient sync --nohooks --with_branch_heads --force --reset -D -j2
  ```

  **Set `autocrlf=false` BEFORE the first clone if you can.** Flipping it on an existing tree makes
  every repo that was already cloned under `autocrlf=true` look massively dirty, because the
  worktree holds CRLF while git now expects LF. Next symptom, in a repo you never edited:

  ```
  error: Your local changes to the following files would be overwritten by checkout
  ```

  **Identify it by the diffstat — the two numbers are equal:**

  ```
  579 files changed, 193116 insertions(+), 193116 deletions(-)   # depot_tools
  137 files changed,  50972 insertions(+),  50972 deletions(-)   # cef
  ```

  Identical insertions and deletions across every file is a pure line-ending rewrite, never real
  content. **Repair is one command per affected repo** (safe — these are pristine upstream clones
  with no local work):

  ```bash
  git -C <repo> reset --hard      # rewrites the worktree with LF
  ```

  Both `depot_tools` and `cef` needed it. `chromium/src` did **not** — its `.gitattributes` had
  protected it all along (3 untracked DEPS dirs, zero tracked modifications), which is a useful
  confirmation that the protection theory is right rather than a coincidence.

  **`GIT_CONFIG_GLOBAL` alone is NOT enough — set `core.autocrlf=false` REPO-LOCALLY.** The damage
  came back and killed the build **3 seconds in**, at
  `git checkout <pinned depot_tools sha>` → *"your local changes would be overwritten"*, with the
  same equal-count diffstat (403 files, 127154/127154). Two compounding reasons:

  1. **`automate-git.py` runs `update_depot_tools.bat` on EVERY invocation.** That re-pulls
     depot_tools, moves it **off** the pinned commit, and re-writes its files — so any line-ending
     repair you did by hand is undone on the next run, and the pinned checkout that immediately
     follows then fails.
  2. **depot_tools ships its own bundled git**, which does not necessarily honour the
     `GIT_CONFIG_GLOBAL` you exported for your shell.

  Repo-local config is the only scope that survives both (precedence: system < global < **local**):

  ```bash
  for r in depot_tools cef chromium/src; do
    git -C "$r" config core.autocrlf false
    git -C "$r" config core.filemode false
    git -C "$r" reset --hard
  done
  ```

  **Then pass `--no-depot-tools-update`** to `automate-git.py` for the build, having first checked
  depot_tools out to the pinned sha yourself. Once it is *at* the pin, the update step is pure churn
  risk with no upside.

  ⚠️ **This matters beyond the sync:** P3 applies CEF patches with `git apply -p0` **exact-context,
  no fuzz**. CRLF-contaminated sources would fail to patch, and the error would point at the patch
  rather than at git config.

- **The build outlives any supervising process — DETACH IT.** A Chromium checkout is hours and the
  build is 10–12 h, longer than an agent tool call, a CI step, or an SSH session. Start it with
  `nohup … > build.log 2>&1` so it is reparented and survives its launcher, then **watch the log
  file**, never the process handle. Verified the hard way: the supervising invocation was killed
  mid-checkout and the detached `bash`/`python`/`git` chain carried on regardless — the clone
  finished and delta resolution continued, with output still landing in the log. Had it *not* been
  detached, ~66 GB of transfer would have been lost.

  Corollary for whoever automates this: **"my watcher died" and "the build died" are different
  events and must be distinguished.** Confirm with
  `Get-CimInstance Win32_Process -Filter "Name='bash.exe'"` (or `ps`) before concluding anything
  failed, and make the script's last line an explicit `echo EXIT=$?` marker so log-watchers have an
  unambiguous terminal signal rather than inferring completion from silence.

  **Do NOT use log freshness as your liveness check.** `git` suppresses progress output when
  stdout/stderr is redirected to a file, so a **quiet log during `fetch`, delta resolution or
  repack is completely normal** and can last 30+ minutes. A naive "log idle → stalled" watchdog
  cries wolf exactly when the tool is working hardest. Ours did, twice, while a `git.exe` sat at
  **528 s CPU and 10.4 GB resident.**

  Use **CPU accumulation** instead — poll the total CPU time of the `git`/`python` processes and
  treat "no increase across several consecutive polls" as the stall signal:

  ```powershell
  $p = Get-CimInstance Win32_Process -Filter "Name='git.exe' OR Name='python.exe'"
  (($p | Measure-Object UserModeTime -Sum).Sum + ($p | Measure-Object KernelModeTime -Sum).Sum)/1e7
  ```

  Two more watchdog traps worth knowing, both of which bit here:
  - **Match processes on the command line, not a substring that your own watcher also contains.**
    A filter like `*sync_deps*` matches the watcher itself (its command line names the log file),
    so it reports phantom survivors — or kills itself.
  - **`comm` requires sorted input.** Diffing successive log snapshots with `comm` on unsorted
    lines emits scrambled output that misrepresents the order events happened in. Use `diff`.

### From the real 2026-03-12 build

- **The build IS resumable — but the mechanism CHANGED. Read this before relying on it.**
  The 2026-03-12 M136 build used **Ninja**, which tracks completed work in `.ninja_log`. Interrupted by
  a Windows auto-restart at **78,821** objects, the resume only had to compile **~17,336** more (of ~96K
  total). `make_distrib.py` packaging took ~404 s (~7 min).

  **From M150 (branch `7871`) onward this evidence does NOT carry forward.** Chromium switched its
  default build tool from Ninja to **Siso**; Ninja has been *officially unsupported for external
  developers since end of September 2025*. On branch `7871`, `use_siso_default = true` whenever
  `build_with_chromium` is set (it is, in a CEF checkout) **and the output dir has no `.ninja_deps`** —
  so **a fresh out-dir gets Siso**, while our existing M136 out-dir keeps Ninja. CEF's `automate-git.py`
  calls bare `autoninja`, which makes this choice for you.

  | | Ninja (M136 build) | Siso (M150 onward) |
  |---|---|---|
  | Incremental state | `.ninja_log` | `.siso_fs_state` + `.siso_deps` |
  | Crash recovery | implicit | append-only **`.siso_fs_state.journal`**, replayed on next start |
  | Interrupt | any kill | **one Ctrl-C is graceful** (state is flushed); a **second aborts** |
  | Remote backend | n/a | **not required** — runs fully local. External contributors can't use Chromium's RBE on Windows anyway |
  | Concurrent builds in one out-dir | allowed (racy) | **locked** |

  **Escape hatch:** `use_siso=false` in `args.gn` still works on `7871`, but is unsupported upstream and
  untested on Chromium CI — treat as a fallback, not the plan. **Never mix**: switching requires
  `gn clean` (depot_tools refuses otherwise).

  ⚠️ **Not yet proven for us:** whether a *hard* kill (power loss / forced restart) resumes cleanly under
  Siso. The journal is designed for exactly that, but only a real interrupted build proves it. Until
  then, keep Windows Update paused and assume a hard kill may cost the build.
- **Disable Windows auto-restart before starting** — see Step 3 setup. This is the #1 cause of a lost
  overnight build.
- **`chrome_pgo_phase=0`** disables PGO (which needs pre-existing profile data we don't have) → avoids
  build failures from missing profiles. Perf difference is minimal for CEF usage; also it's what lets
  sccache caching work (see Step 3 A1 notes).
- **External-drive builds** work on USB 3.0+ **SSD** (~1.5–2× slower, mostly in `gclient sync`'s
  millions of small files). USB HDD = 2–3× slower (avoid). USB 2.0 = unworkable. Use **NTFS** (not
  exFAT — Chromium needs symlinks + case sensitivity). Add the Defender exclusion for the actual drive.
- **Common build errors:** "Failed to download VS toolchain / hash check failed" → set
  `DEPOT_TOOLS_WIN_TOOLCHAIN=0` (forces use of local VS 2022 instead of Google's internal toolchain).
  "Debugging Tools not found" → reinstall Win SDK with that component. Path-too-long → use `C:\cef\`.
  Out of disk → need ~100 GB. Hangs/crashes → check RAM (16 GB+), close apps, optionally
  `--build-args="--jobs=4"`.
- **Integration errors:** "Unsupported CEF version" → wrapper not rebuilt against new headers (delete
  CMakeCache, rebuild). Browser crashes on startup → a CEF DLL or resource is missing (`libcef.dll`,
  `chrome_elf.dll`, `icudtl.dat`, `v8_context_snapshot.bin`, `locales/`) or wrapper/version mismatch.

## Licensing note (codecs)
Distributing proprietary codecs uses patented tech. Under ~100k installs: typically free under
MPEG-LA/Via terms. Over 100k: royalties may apply (~$0.10–0.20/unit with caps). Add MPEG-LA
attribution to the About page; consult legal if Hodos grows significantly.

## Output file checklist (must be present after staging)
`libcef.dll`, `chrome_elf.dll`, `d3dcompiler_47.dll`, `icudtl.dat`, `libEGL.dll`, `libGLESv2.dll`,
`snapshot_blob.bin`, `v8_context_snapshot.bin`, `vk_swiftshader.dll`, `vk_swiftshader_icd.json`,
`vulkan-1.dll`; `resources/` (`cef.pak`, `cef_100_percent.pak`, `cef_200_percent.pak`,
`cef_extensions.pak`, `devtools_resources.pak`); `locales/` (`en-US.pak`, …).

### From integrating the `7871` build into `cef-native` (2026-08-04)

The engine building green is **not** the same as the app running on it. Linking + launching the
existing embedder against CEF 150 took four further changes, none optional, none discoverable from
the build log. Expect the macOS bump to hit all four.

- **CEF 150 requires C++20 — the headers do not parse under C++17.** `include/base/cef_scoped_refptr.h`
  uses a `requires(std::convertible_to<U*, T*>)` constraint. The distribution's own
  `cmake/cef_variables.cmake` moved `/std:c++17` -> `/std:c++20`, so the shipped
  `libcef_dll_wrapper.lib` is a C++20 build and the embedder must match it or risk an ABI mismatch.
  The first symptom is a wall of `syntax error: identifier 'convertible_to'` **inside CEF headers**,
  which reads like a corrupt checkout — it is not.

- **`NOMINMAX` became mandatory.** 150's `cef_ref_counted.h` uses `std::numeric_limits<int>::max()`
  and `cef_types_wrappers.h` uses `std::min()`; the `windows.h` `min`/`max` macros shred both into
  `'(': illegal token on right side of '::'`. M136 avoided both spellings, so only the few `.cpp`
  files that `#define NOMINMAX` themselves used to matter. Define it for the whole directory.

- **⛔ Chromium 150 ships the AI "Actor" UI enabled by default, and it null-derefs on every
  CEF-hosted browser.** `ActorUiContentsContainerController::OnWebContentsAttached` calls
  `tabs::TabInterface::GetFromContents()`, which dereferences null for a `WebContents` that is not a
  real Chrome tab — which is every browser CEF creates. Symptom: access violation deep inside
  `libcef` moments after `CefRunMessageLoop`, **with no log line at all**. Fix:
  `--disable-features=GlicActorUi`.

  Two traps around it. First, this only bites **Chrome-style** browsers, and
  `runtime_style = CEF_RUNTIME_STYLE_DEFAULT` **means Chrome style**
  (`libcef/browser/browser_host_create.cc :: IsChromeStyle`) — so a client that never mentions
  runtime style is fully exposed, while windowless/OSR browsers are immune (windowless is always
  Alloy). Second, `CefCommandLine::AppendSwitchWithValue` **replaces** the value, so an app that
  already appends `--disable-features=…` in `OnBeforeCommandLineProcessing` will silently discard a
  `--disable-features` passed on the command line. Verifying the fix from the command line first
  will appear to fail. Add to the existing list, don't pass a second one.

- **A failed `freopen_s` on `stdout` becomes fatal inside the bootstrap client DLL.** `freopen_s`
  closes the stream *before* it tries to reopen, so on failure `stdout` is left with an invalid fd;
  the next `std::cout` write goes `fwrite` -> `_isatty(bad fd)` -> `_invalid_parameter` ->
  `__fastfail(FAST_FAIL_INVALID_ARG)`. The same failure was survivable when the app was an EXE, so
  a redirect that had been failing benignly for months turned into a startup crash. Reopen the
  stream on `NUL` when the redirect fails.

### Debugging technique that actually resolved both runtime crashes

Guessing was useless; two cheap mechanical steps settled each in minutes.

1. **Get the untruncated exit code.** Bash reports Windows status codes mod 256, which turns
   `0xC0000409` into a meaningless `9` and `0xC06D007F` into `127`. Use
   `Start-Process -PassThru -Wait` and print `$p.ExitCode` in hex. `0xC0000409` = `__fastfail`
   (subcode 5 = `FAST_FAIL_INVALID_ARG`, i.e. a CRT invalid parameter — *not* a stack overrun,
   despite the name `STATUS_STACK_BUFFER_OVERRUN`). `0xC06D007F` = delay-load, proc not found.
2. **Symbolize.** Release builds carry no `/Zi`, so reconfigure with
   `-DCMAKE_CXX_FLAGS_RELEASE="/O2 /Ob2 /DNDEBUG /Zi"` +
   `-DCMAKE_SHARED_LINKER_FLAGS_RELEASE="/INCREMENTAL:NO /DEBUG"`, then run under
   `cdb -g -G -y "<builddir>;<release_symbols dir>" -c "g;.lastevent;kp 25;q"`. The
   `..._release_symbols` distribution has `libcef.dll.pdb` (5.2 GB) and gives fully named Chromium
   frames — that is what identified the Actor UI in one shot. **Run cmake from git-bash with
   `MSYS_NO_PATHCONV=1`**, or MSYS rewrites `/O2` into `C:/Program Files/Git/O2` and the flags land
   as bogus source files (same class of bug as the `cmd /c` lesson above).
3. **Rule out the engine before blaming it.** The `..._client` distribution ships a prebuilt
   `cefclient.exe`; if it runs, `libcef` is healthy on that machine and the fault is in the embedder.

---

## Open TODOs to make this fully turnkey
- [ ] Execute the **M150 / branch `7871`** move (Step 1) — **IN FLIGHT as of 2026-08-03.** Kickoff review
      done; target pinned to `150.0.17+g94c1726+chromium-150.0.7871.187` (newest security point-release,
      re-verify on build day). The channel gate is **satisfied** — `7871` reached CEF-Stable, so the
      M149/`7827` fallback is dead. See `../0.4.0/chromium-rebuild/KICKOFF_REVIEW_RESULTS_2026_08_03.md`.
- [ ] A1: stand up the **self-hosted runner / beefy VM + shared sccache** path; evaluate **Siso + a
      third-party REAPI backend** (EngFlow / BuildBuddy / NativeLink) for distributed builds.
- [x] **B1 toolchain half: `patch.cfg` integration — DONE 2026-08-05 (P3).** Fork `Hodos-Browser/cef`,
      branch `hodos/7871`, `--url` wired into both build scripts, `HODOS_FARBLING` condition gate proven,
      no-op probe proven to apply pre-compile through the real build path. The farbling patch *content*
      (C1–C7) is still to author — `../0.4.0/chromium-rebuild/PLAN_farbling_blink.md`. Evidence:
      `../0.4.0/chromium-rebuild/P3_TOOLCHAIN_PROOF.md`; ledger: `HODOS_PATCHES.md` in the fork.
- [ ] Decide whether premium DRM (VMP) is a product goal (own mini-spike).
- [x] **Automate the Step 5.5 drift audit — DONE 2026-08-05 (P3/CEF-2).** Three scripts, not one:
      `scripts/cef_patch_drift_audit.sh` (patch apply health, registry integrity, target-file existence;
      chains the next via `--with-dist`), `scripts/cef_dist_drift_audit.sh` (file manifest), and
      `scripts/cef_gn_args_gate.sh` (`args.gn` / codecs). Two corrections to the wording above: the
      manifest target is the **installer's extension whitelist**, not `cef-native/CMakeLists.txt`
      copy-lists (CMake does a wholesale copy and can never drop a file); and there is **no fuzz to
      report** — CEF's `git apply -p0` is exact-context and fail-loud, so the only sub-failure signal is
      a hunk **offset**.
