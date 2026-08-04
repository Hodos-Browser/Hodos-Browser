# CEF/Chromium Full-Build Runbook (Tier 1)

**Created:** 2026-06-01 · **Last updated:** 2026-08-03
**Status:** ✅ WORKING — grounded in `development-docs/DevOps-CICD/scripts/build_hodos_cef.bat` +
`build_hodos_cef_mac.sh` and the real build done 2026-03-12 (merged in from the former
`CEF_BUILD_FROM_SOURCE_GUIDE.md`).

> **⚠️ Script path corrected 2026-08-03.** Both build scripts live at
> **`development-docs/DevOps-CICD/scripts/`**, NOT at a repo-root `scripts/`. Every doc that cites
> `scripts/build_hodos_cef.bat` is citing a path that does not exist. (`Q5` CEF-5's "check them in —
> absent today" is likewise wrong: they exist, just not where cited. CEF-5 reduces to a *move*
> decision, not authoring.)
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
| Mac | Xcode + CLT; arch auto-detect (`--arm64-build` Apple Silicon / `--x64-build` Intel) | `.sh` |
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
2. **Farbling patches (B1)** — NEW step once B1 lands. Apply our Blink farbling patches via CEF's
   `cef/patch/patch.cfg` mechanism (add our `.patch` files + register them) so they're applied to the
   Chromium source before compile. The B1 Blink farbling work **rides on this build** — see
   `../0.4.0/B1-farbling-design.md`. Log "same as last build, or with these changes: ___" per build.
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
Canonical script: `scripts/build_hodos_cef.bat` (copy to `C:\cef\chromium_git\` and run from there in a
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
Use `scripts/build_hodos_cef_mac.sh`. Same `automate-git.py` + ninja flow; needs **Xcode + Command
Line Tools**; same ~100 GB / 16 GB (32 rec.) requirements. Arch: `--arm64-build` on Apple Silicon
(M1+), `--x64-build` on Intel. Output is `Chromium Embedded Framework.framework` instead of `libcef.dll`
— Windows DLLs cannot be used on macOS; this is a fully separate build.

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
- **Patches** — re-apply `cef/patch/` (farbling) and report any fuzz/failures (the A1 patch toolchain
  owns this).
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
  green exit with zero work done. Use `MSYS_NO_PATHCONV=1`, `cmd.exe //c`, or (preferred) drive
  `automate-git.py` from bash with exported env vars so exit codes are real.

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

---

## Open TODOs to make this fully turnkey
- [ ] Execute the **M150 / branch `7871`** move (Step 1) — **IN FLIGHT as of 2026-08-03.** Kickoff review
      done; target pinned to `150.0.17+g94c1726+chromium-150.0.7871.187` (newest security point-release,
      re-verify on build day). The channel gate is **satisfied** — `7871` reached CEF-Stable, so the
      M149/`7827` fallback is dead. See `../0.4.0/chromium-rebuild/KICKOFF_REVIEW_RESULTS_2026_08_03.md`.
- [ ] A1: stand up the **self-hosted runner / beefy VM + shared sccache** path; evaluate **Siso + a
      third-party REAPI backend** (EngFlow / BuildBuddy / NativeLink) for distributed builds.
- [ ] B1: farbling patch set + `patch.cfg` integration (own design session — `../0.4.0/B1-farbling-design.md`).
- [ ] Decide whether premium DRM (VMP) is a product goal (own mini-spike).
- [ ] Automate the **Step 5.5 build-config / file-manifest drift audit**: a CEF-version-pin–triggered
      `cef-bump-audit` script that diffs the new CEF dist manifest vs our cmake/mac copy-lists +
      `args.gn`, re-applies `cef/patch/` and reports fuzz, emitting a human-review diff. Build it
      alongside the A1 patch toolchain (`../0.4.0/B1-farbling-design.md` / CEF track).
