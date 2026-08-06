# Chromium/CEF Build Relay — Windows → macOS

**Opened:** 2026-08-03, during the Windows `7871` execution session.
**Status:** ⏳ **Checkout phase complete-ish; the Windows build has NOT finished yet.** Everything
in §1–§4 is verified. §5 is what Mac owns and is unaffected by the pending build.

> **This is deliberately NOT a recipe.** The roadmap is explicit: *"Mac is a parallel build, not an
> inherit-and-verify afterthought (I8)."* The build is a **first-class, separate effort per OS** —
> Windows produces `libcef.dll`, Mac produces `Chromium Embedded Framework.framework` through its
> own clang/Xcode toolchain, signing, packaging and notarization. DLLs are not reusable on Mac.
>
> So: **§2 is what you inherit** (shared, cross-platform, take it as given). **§5 is what you OWN**
> — decisions a recipe would silently make for you, and which Windows explicitly did *not* make.
>
> Standing coordination stays in `MAC_WINDOWS_RELAY.md`. This doc is build-specific.

---

## 1. The pins — take these exactly

| Item | Value |
|---|---|
| CEF branch | **`7871`** (CEF 150 / Chromium 150, the **M150 LTS** line) |
| CEF checkout | **`94c1726`** = `150.0.17+g94c1726+chromium-150.0.7871.187` |
| Chromium (transitive) | **`refs/tags/150.0.7871.187`** = `30f6543ae91e6a860e73b76e3216b663b050f4e5` |
| `GN_DEFINES` | `is_official_build=true proprietary_codecs=true ffmpeg_branding=Chrome chrome_pgo_phase=0` |
| Build tool | **Siso** (default on a fresh out-dir; Ninja unsupported upstream since Sept 2025) |
| macOS floor | **12.0 Monterey** — but see §5, you must *measure*, not assume |

**Do not re-derive the version.** D1 is closed: `7871` is CEF-Stable, entered LTC 2026-07-21,
becomes LTS 2026-10-06, security refresh to 2027-04-13. M149/`7827` is **DEAD** (already
unsupported). M151 is **not** an LTS branch. Re-verified on build day 2026-08-03: `150.0.17` is
still the newest `7871` point-release.

⚠️ **`index.json` has no `lts` enum** — LTS builds are labelled `"stable"`. Key any automation off
the **branch number**, never the JSON `channel` field.

**Confirm a pin from `cef/CHROMIUM_BUILD_COMPATIBILITY.txt`**, not from the version string. That
file is what ties the CEF commit to its Chromium tag *and* to an exact `depot_tools` commit.

---

## 2. What Mac inherits (shared, cross-platform)

- **The pins in §1.** Same branch, same checkout, same `GN_DEFINES`. Codecs are always-on GN flags,
  not a separate build.
- **Codec GN args are unchanged M136 → M150** — `proprietary_codecs` and `ffmpeg_branding` were
  verified against `features.gni` / `ffmpeg_options.gni` @ 7871. No rename, no value change.
- **The dependency pins (DEP-1a..d), already landed on `0.4.0`:**
  - `cef-native/vcpkg.json` — Windows-only in effect, but it is now the declared C++ dep set.
  - **`/Brewfile` — this is yours** (DEP-1c). `release.yml` now runs `brew bundle --file=Brewfile`
    instead of a bare `brew install`. It pins `openssl@3`, `nlohmann-json`, `sqlite3`. Note honestly:
    Homebrew **cannot** pin a formula version in a Brewfile, so this buys a declared dependency set
    and a reviewable diff, **not** version-exactness. macOS dep pinning is genuinely weaker than the
    Windows vcpkg pin. Escalation if it ever bites: `brew extract` into a Hodos tap.
  - `rust-toolchain.toml` (1.97.1) in both Rust workspaces — applies to your builds too.
- **The M136 codec baseline** (below) is the pre-bump reference for the P6 comparison. It was
  measured on **Windows hardware**; re-measure your own — see §5 on HEVC.
- **The Layer-A probe harness approach** — a local HTTP page that runs `canPlayType` and POSTs the
  results back, so nothing depends on a human reading values off a screen. Reusable as-is.

### M136 codec baseline (Windows, i9-12950HX)

| Codec | Gate | M136 result |
|---|---|---|
| H.264 baseline `avc1.42E01E` | GATE | `probably` |
| H.264 High `avc1.640028` | GATE | `probably` |
| AAC-LC `mp4a.40.2` | GATE | `probably` |
| MP3 `audio/mpeg` | GATE | `probably` |
| VP9 `vp09.00.10.08` | GATE | `probably` |
| AV1 `av01.0.05M.08` | assert present | `probably` |
| HEVC `hvc1…` / `hev1…` | **non-gating** | `probably` |

Rule: any **GATE** row returning `""` on `7871` blocks the bump. An **HEVC**-only change does not,
but must be recorded with the machine it was measured on.

---

## 3. What Windows did (exact, so you can compare — not so you can copy)

Three-phase split, deliberately **not** one `automate-git.py --force-build` invocation:

1. **checkout** — `automate-git.py … --no-build --no-distrib`
2. **⛔ gn-args codec gate** — `gn args --list` asserting `proprietary_codecs=true`,
   `ffmpeg_branding="Chrome"`, `chrome_pgo_phase=0`, plus Widevine/HEVC derivations recorded
3. **build** — `automate-git.py … --force-build`

**The gate is the point of the split.** A flipped or renamed codec default produces a **green build
with no codecs**; catching that after 10–12 h is the expensive failure. Do the same on Mac.

**Tree layout:** new tree with its **own `depot_tools`**, preserving the M136 tree. `automate-git.py`
hard-checkouts `depot_tools` to the commit its branch pins, so a shared `depot_tools` ends up pinned
to whichever branch ran last.

### The six failures Windows hit — check which apply to you

Full diagnosis + recovery for each is in `../DevOps-CICD/CEF_BUILD_RUNBOOK.md` (Lessons). Summary:

| # | Failure | Applies to Mac? |
|---|---|---|
| 1 | **`depot_tools` cloned shallow** → `fatal: reference is not a tree`. CEF pins an exact commit. | **YES** — clone full, never `--depth 1` |
| 2 | **`automate-git.py` fetched from `master`** — it is versioned *with* CEF and 7871's differs. | **YES** — the mac script had the same bug; fixed |
| 3 | **`rd exited with code 3221225794`** killing gclient on an *empty* temp dir after the clone succeeded. | **Windows-only** (`STATUS_DLL_INIT_FAILED`) |
| 4 | **googlesource HTTP 429**, also surfacing as `expected 'packfile'` / `expected flush after ref listing`. | **YES, likely** — a cold checkout is a lot of traffic. Resume with `gclient sync … -j2`; `automate-git.py` has no `--jobs` passthrough |
| 5 | **`core.autocrlf=true`** breaking third_party sub-repo checkouts. | **NO** — Git-for-Windows-specific installer default |
| 6 | **Flipping autocrlf on an existing tree** → equal-count diffstats, `git reset --hard` per repo. | **NO** — follow-on from 5 |
| 7 | **`update_depot_tools.bat` re-dirties depot_tools on EVERY `automate-git.py` run**, moving it off the pinned commit so the next pinned checkout fails — killed the build 3 s in. | **YES** — this is `automate-git.py` behaviour, not Windows. Pass **`--no-depot-tools-update`** after checking depot_tools out to the pin yourself |
| 8 | **The gate itself failing and looking exactly like a codec regression** (all four flags "MISSING"). Causes: `cef_create_projects.bat` uses a **relative** path so it must run from `src/cef`; and hooks had never run because every sync used `--nohooks`. | **YES, both** — run the gate's self-check (arg count + a control flag) before believing any MISSING verdict |

**#4 and #7 are the ones to plan around.** Budget for them; they are not breakage.
**#8 is the one that could waste a day** — it will tell you the codec flags were renamed. They were not.

---

## 4. Status of the Windows build

- ✅ P1 pins + DEP-1a..d landed
- ✅ Build-day pin re-check — no newer `7871` point-release
- ✅ M136 codec Layer-A baseline captured
- ✅ VER-5 drift audit script written, M136 baseline **CLEAN**
- ✅ `cef-binaries/Release` + `Resources` backed up before any staging
- ✅ Checkout complete — Chromium `30f6543a` = `refs/tags/150.0.7871.187`, CEF `94c17267e`
- ✅ **gn-args codec gate PASSED** (see table below)
- ⏳ Build running since `2026-08-04T01:49:56Z` (~10–12 h)

### ⛔ gn-args gate result — run this same gate before your build

1211 args resolved; self-check clean (a real `gn args --list` emits hundreds — a *broken* gate run
emitted 5, see the "gate must prove itself" lesson in the runbook).

| Flag | Resolved | Note |
|---|---|---|
| `proprietary_codecs` | `true` | **GATE** ✅ |
| `ffmpeg_branding` | `"Chrome"` | **GATE** ✅ |
| `chrome_pgo_phase` | `0` | **GATE** ✅ |
| `is_official_build` | `true` | **GATE** ✅ |
| `enable_widevine` | `true` | resolves |
| `enable_library_cdms` | `true` | resolves |
| `enable_cdm_host_verification` | `true` | already true on M136 — relevant to the Q4 VMP question |
| `enable_cdm_storage_id` | `true` | same |
| `enable_platform_hevc` | `true` | non-gating (build flag; runtime answer is machine-dependent) |
| `enable_hevc_parser_and_hw_decoder` | `true` | non-gating |
| `enable_av1_decoder` / `enable_dav1d_decoder` | `true` | AV1 present |
| `enable_mse_mpeg2ts_stream_parser` | `true` | recorded |
| `enable_platform_ac3_eac3_audio` | `false` | recorded |
| **`enable_platform_dolby_vision`** | **`true`** | ✅ **checked — not a bump regression.** `media/media_options.gni` is byte-identical on 7103 and 7871: `proprietary_codecs && (is_cast_media_device || is_win)`. It was already `true` on shipped M136. **`is_win`-gated, so it should resolve `false` for you** — expect a legitimate Win/Mac difference here, not a fault |
| `media/BUILD.gn` coupling guard | present | `assert(ffmpeg_branding != "Chromium", …)` survived 14 milestones |

**Headline: no codec flag was renamed or flipped M136 → M150.** The `GN_DEFINES` carried forward
verbatim, exactly as `PLAN_codecs.md` predicted. The generated `args.gn` matches the shipped M136
`args.gn` on every key flag.

**Nothing in §5 is blocked on the above.** Start your own provisioning and D3 now.

---

## 5. ⚠️ What MAC OWNS — decide these yourself; Windows deliberately did not

### D3 — architecture. **UNDECIDED. Yours.**
`universal2` vs `arm64` vs `x86_64`. Default in the plan is **universal2**, which means **two
per-arch builds plus `lipo`** — i.e. roughly double the already-10–12 h build. That is a real
build-time/coverage tradeoff and the owner has not signed off. **Surface it with a recommendation;
do not pick it unilaterally.**

### VER-4 — `minos`. **Yours entirely. And it is not a copy-paste.**
- The current **11.0** floor was **never `vtool`-measured** — the tracker marks it provisional. So
  `max(12.0, measured)` has **no prior measurement to compare against**. You are establishing the
  baseline, not verifying one.
- `vtool`-measure the built framework's real `minos`.

> #### 🚨 CORRECTION — the floor is written in **FIVE** places, not three
>
> The plan says "the three-place min-version edits". Verified against the working tree on
> 2026-08-03, `11.0` actually appears in **five**, and **the two the plan omits are in CI**:
>
> | # | Location | Note |
> |---|---|---|
> | 1 | `cef-native/CMakeLists.txt:115` `CMAKE_OSX_DEPLOYMENT_TARGET` | the plan's #1 |
> | 2 | `cef-native/Info.plist:24` `LSMinimumSystemVersion` | the plan's #2 |
> | 3 | `cef-native/mac/helper-Info.plist.in:22` `LSMinimumSystemVersion` | the plan's #3 |
> | 4 | `.github/workflows/release.yml:405` `MACOSX_DEPLOYMENT_TARGET: "11.0"` | **missed by the plan** |
> | 5 | `.github/workflows/release.yml:539` `-DCMAKE_OSX_DEPLOYMENT_TARGET=11.0` | **missed, and it OVERRIDES #1** |
>
> **#5 is the trap.** CI passes the deployment target on the `cmake` command line, which beats the
> `CACHE STRING` default in `CMakeLists.txt`. So editing #1 alone changes your **local** build and
> leaves **the shipped CI build still at 11.0** — a green edit with no shipped effect. Change all
> five, and prefer making #4/#5 read from a single source rather than re-hardcoding 12.0 twice more.

- A **dynamic minos guard already exists** — at **`release.yml:645-672`** (the kickoff doc's
  `621-645` is wrong; that range is Sparkle XPC-service removal). It runs `vtool -show-build`,
  reads the CEF framework's real minos, and fails the build if any shipped Mach-O has a *lower*
  minos than the framework. So VER-4 is **fail-loud, not silent-drift** — but note what it does and
  does not catch: it compares binaries **against each other**, so it would happily pass a
  consistently-11.0 build. It will not tell you the published floor is wrong. Only #1–#5 above do.
- M150 is the **last** Chrome supporting Monterey. The 11.0 → 12.0 raise **strands Big Sur users** —
  it gates rather than crashes sub-floor updates, but it must be announced in release notes.

### The framework embed list + `CEF_HELPER_APP_SUFFIXES`
Windows' VER-5 drift audit targets the **installer's extension whitelist**
(`installer/hodos-browser.iss:68-72`) — a Windows-only mechanism. **Your equivalent is the framework
embed list and `CEF_HELPER_APP_SUFFIXES` (`cef-native/CMakeLists.txt:539-545`, 5 helper bundles).**
The failure mode is the same shape: a file that builds and smoke-tests fine from source, then is
missing from the packaged app — and therefore missing from a silent update too. Run the equivalent
audit; do not assume the Windows one covered you.

### Sparkle / notarization / EdDSA
Entirely yours. Note the chain-of-trust rule: rotate **either** the Developer ID cert **or** the
EdDSA key, **never both**.

### D7 — Apple individual→org signing sequencing
Still open, and it gates whether beta.1 is the first org-signed build. Hinges on **confirming Team
ID is preserved** across the conversion. If it is not confirmed, option (A) migrate-first is off the
table and it defaults to (B) defer. Windows' CN (`Marston Enterprises`) is already correct and
unchanged, so this is a Mac-only unknown.

### Mac GPU strings for C4 — **MOOT, do not build them**
**D4 is DECIDED: DROP** WebGL `UNMASKED_VENDOR`/`RENDERER` faking. The current build does not farble
these at all, so "drop" is exact status quo. This **removes the Mac GPU-string set from D3's scope**
(FB-6 is moot). If anyone hands you a task to assemble Apple Silicon / Intel ANGLE renderer strings,
it is stale — push back.

### Your own baseline + target builds
You own both. Per **D9**, Windows **skipped** the M136 from-source re-build and instead probed the
live shipping M136 build for the codec baseline. That was justified by an intact 175 GB M136 tree
plus the shipped binary. **Decide independently whether that reasoning holds for your host** — if
you have no equivalent last-known-good, you may genuinely need the baseline build.

---

## 6. App-layer findings from the Windows bootstrap migration (added 2026-08-04)

Windows is mid-migration to CEF's bootstrap model. Most of it is Windows-only mechanics, but three
findings either **cross over** or describe a symptom Mac shares through a *different* mechanism —
which is exactly the kind of thing that costs a session to rediscover.

### 6.1 The bootstrap model itself does NOT cross over

CEF **#3928** (sandbox linking → bootstrap executable) is **Windows-only**. macOS keeps the
helper-app model and `Chromium Embedded Framework.framework`. There is no `bootstrap.exe`
equivalent, `cef_sandbox.lib` was never part of the mac link, and `CEF_USE_BOOTSTRAP` is set only
under `if(OS_WINDOWS)` in `cmake/cef_variables.cmake`.

**But do not conclude the drift audit is done.** Windows' VER-5 result (`cef_sandbox.lib` removed,
`bootstrap.exe`/`bootstrapc.exe` added) tells you *nothing* about the mac dist. Run VER-5
independently against `cef_binary_150…_macos*`, and specifically diff: the framework version
directory layout, `CEF_HELPER_APP_SUFFIXES`, and any added/removed `.dylib`.

### 6.2 ⚠️ CROSS-PLATFORM SECURITY FINDING — the Chromium sandbox is OFF on **both** platforms

Found while scoping the Windows migration. Same outcome, two different causes:

| Platform | Mechanism | Where |
|---|---|---|
| Windows | `CefExecuteProcess` / `CefInitialize` are both passed `nullptr`, and `cef_sandbox_info_create()` is **never called anywhere** in `cef-native/`. CEF's `CefMainRunner::ContentMainInitialize` sets `*no_sandbox = true` whenever `windows_sandbox_info == nullptr` — verified byte-identical in the M136 and 7871 trees. `cef_sandbox.lib` was linked but unused. | `cef_browser_shell.cpp :: WinMain` |
| **macOS** | `settings.no_sandbox = true`, **unconditional**. The comment says *"Disable sandbox on macOS for development (requires code signing otherwise)"* — but it is **not gated on dev vs prod**, so shipped mac builds are unsandboxed too. | `cef_browser_shell_mac.mm` (~:5278) |

**Why it matters here specifically:** an unsandboxed renderer that gets popped by a web page can
read the profile databases off disk *and* open a socket straight to the wallet port, going around
the C++ interception layer — and therefore around every permission gate, the spend caps, and the
gold-pill payment indicator. Signing keys stay in Rust either way; the **authorization** boundary is
what's lost.

**Windows fixes this as a side effect of the migration** (bootstrap supplies real sandbox info, so
it becomes a matter of forwarding it instead of passing `nullptr`).
**macOS does NOT get it for free, and it is not the same change.** On mac the sandbox requires a
properly signed app to initialize — which entangles it with **D7 (individual → org signing)**.
Sequence it *after* D7 lands, and treat it as its own change with its own smoke matrix. Do not
bundle it into the version bump.

Note `--allow-loopback-in-sandbox` is already appended on mac in
`simple_app.cpp :: OnBeforeCommandLineProcessing` — currently a no-op, but it becomes load-bearing
the moment the sandbox goes on (the frontend dev server and both Rust backends are loopback).

### 6.3 The renderer holds file handles it shouldn't — and fixing it helps Mac *more* than Windows

`SimpleRenderProcessHandler`'s constructor opens the **history SQLite database directly inside every
renderer process** on Windows (`HistoryManager::GetInstance().Initialize(%APPDATA%\…\<profile>)`,
gated on `--type=renderer`). macOS takes the `#else` branch and is stubbed out — **which is why
`window.hodosBrowser.history.*` already returns empty on mac today.**

The fix is the same on both: route the 7 `history.*` V8 methods through `cefMessage.send` IPC to the
browser process, where `HistoryManager` is already correctly initialized (precedent already exists —
`get_most_visited` works exactly this way). That makes it:

- a **sandbox prerequisite** on Windows (a sandboxed renderer cannot open the DB), and
- a **straight bug fix** on macOS (history page + omnibox history suggestions currently blank).

**Coordinate so this is done once, not twice.** Windows owns the first cut; Mac should verify the
mac side of `HistoryV8Handler` afterward rather than writing a parallel implementation.

Same file also carries **23 `std::ofstream` writes to relative-path `debug_output.log` /
`test_debug.log`**, one of them firing on *every* `cefMessage.send()` call. Relative paths resolve
against the CWD (the install root in a shipped build). Sandbox-incompatible, and a file
open/write/flush/close per IPC message. **Delete rather than port** — `LOG_*_RENDER` already covers
it and correctly no-ops in the renderer.

### 6.4 Signing gains a hard *runtime* coupling on Windows — check for a mac analogue

Windows bootstrap `LOG(FATAL)`s at launch unless `HodosBrowser.exe`, `HodosBrowser.dll` and
`chrome_elf.dll` are **either all unsigned, or all validly signed with the same primary certificate
thumbprint** (`bootstrap_win.cc :: CheckDllCodeSigning` → `ThumbprintsInfo::IsSame`). Dev builds
pass trivially (all unsigned); release depends on one Azure Trusted Signing batch issuing one leaf.

Mac question to answer: does the 7871 framework/helper load path impose any *new* signature-
consistency requirement beyond what notarization already enforces? Check the non-Windows branch of
`include/wrapper/cef_library_loader.h`, and whether the **#4092 sandbox-compatibility hash** check
applies to the framework load path or only to `bootstrap ↔ libcef.dll`.

### 6.5 Confirmed non-issues on Windows — spot-check the mac analogue, don't re-derive

- **OS-version detection / app manifest.** Windows' Win10 `supportedOS` GUID survives the move
  because `bootstrap.exe` embeds Chromium's `default_exe_manifest` — verified *in the binary*, not
  just in GN. Mac analogue is `LSMinimumSystemVersion` + `minos`, already owned by VER-4.
- **Auto-update version source.** WinSparkle takes its version from an explicit
  `win_sparkle_set_app_details()` call, not from the binary's VERSIONINFO, so replacing the exe
  didn't disturb it. Sparkle on mac reads `CFBundleVersion` — **re-verify it still resolves if the
  framework version directory layout changed** (see 6.1).
- **Installer file manifest.** Windows needed no `[Files]` change (wildcards already covered it).
  Mac's analogue is the framework embed list — which is *not* wildcarded, so it is not free.

---

## 7. Open questions Mac must answer

1. **D3 arch** — universal2 vs arm64. Recommendation + owner sign-off.
2. **Measured `minos`** — what does `vtool` actually report on the 7871 framework?
3. **Does the M136-skip (D9) reasoning transfer**, or do you need a from-source baseline?
4. **Which of the six Windows failures reproduce** on macOS — especially #4 (429).
5. **Team ID preservation** for D7.
6. **Does `CefResponseFilter` still exist and still stream on 7871?** It is flagged LOW-stability in
   the tracker and it is what strips YouTube ads. Windows will check too; compare notes.
7. **Framework embed / helper-suffix drift** across 14 milestones.
8. **Does the mac dist have its own VER-5 drift?** (framework version dir, helper suffixes,
   added/removed dylibs) — §6.1. Windows' result does not transfer.
9. **What is the mac path to enabling the Chromium sandbox**, and does it require D7 to land
   first? — §6.2. Currently `no_sandbox = true` unconditionally, including in shipped builds.
10. **Does `#4092`'s sandbox-compat hash apply to the mac framework load path**, or only to
    Windows `bootstrap ↔ libcef.dll`? — §6.4.

---

## 8. Protocol

`git pull origin 0.4.0` before reading, `git push origin 0.4.0` after writing. Append findings under
a `## MAC → WINDOWS` section here (build specifics) or in `MAC_WINDOWS_RELAY.md` (status). Windows
will append the build result, the codec Layer-A/B comparison and the VER-5 drift outcome to §4 when
the build finishes.

---

## WINDOWS → MAC (2026-08-06) — P3 is closed, P4 farbling is landing

Windows has moved past the build-standup work this doc was written for. Read §1–§8 for the 150
bring-up; this section supersedes anything above it that concerns **patches or farbling**.

### 1. Take this pin, and note the flag that is now mandatory

| | |
|---|---|
| Fork | `github.com/Hodos-Browser/cef`, branch `hodos/7871` |
| **`CEF_CHECKOUT`** | **`e9f3fee65`** (already set in both build scripts) |
| Contents | C1 `HodosSessionCache` · C2 renderer half |
| Registered patches | **115** = upstream 114 + `hodos_farble_session_cache` |

⛔ **`--force-cef-update` is now passed unconditionally by both scripts. Do not remove it.**
`chromium/src/cef` is a COPY, refreshed only when `cef_checkout_changed`, which is computed from the
**standalone checkout's HEAD** (`automate-git.py:1351`). Landing a patch *requires* committing there,
which moves HEAD to exactly the SHA you then pin — so `current == desired` and the copy **never**
refreshes. This is the DEFAULT outcome of the normal workflow, not an edge case. `P3_TOOLCHAIN_PROOF.md`
used to claim it "self-corrects"; that was wrong and is corrected in place.

**Your one cheap detector:** the patcher's `N patches total` line in the build log. It must read
**115**. If it reads 114, you built with zero Hodos patches and a fully green run.

⚠️ **The drift audit will NOT catch this** — it reads the in-tree copy (`CEF_SRC=…/chromium/src/cef`),
so running it right after committing to the fork reports `Hodos entries : 0 / CLEAN`, which looks like
success and means "your copy is stale". Correct order: **commit+push → bump pin → sync → audit → build**.

### 2. Before your first fork build

1. `git -C <tree>/cef remote set-url origin https://github.com/Hodos-Browser/cef.git` — `automate-git`
   hard-errors on a `--url` mismatch. A clean dir is not required; the fork shares upstream's object graph.
2. `git pull --rebase origin 0.4.0` — Windows has ~10 commits since your last sync.
3. Bumping the pin makes automate-git **delete `chromium/src/cef`, which contains `binary_distrib/`**
   (R9). Move it out first.

### 3. ⚠️ The version string changed — this affects your packaging

Fork builds now report a real patch level instead of `150.0.0-HEAD`:

```
CEF_VERSION "150.0.22-7871.3555+g4ed200c+chromium-150.0.7871.187"   CEF_VERSION_PATCH 22
```

`PATCH` = upstream's branch-commit count **plus ours**, so it drifts ahead and will eventually collide
with a real upstream `150.0.22`. **Owner accepted this 2026-08-05**; provenance comes from
`CEF_COMMIT_HASH`, not the version number. Not caused by `--force-cef-update`, and not reversible —
the old `150.0.0-HEAD` was an artifact of pinning a commit that carried no branch decoration.

**What this means for you:** distribution directory and tarball names now embed that string
(`cef_binary_150.0.22-7871.3555+g4ed200c+..._macos*`). Anything matching them by name — your framework
embed step, the `cef-binaries/` staging, the CI asset — must not assume a fixed string. Windows owes
the same check.

### 4. Two of your §7 open questions now have Windows answers

- **Q6 — does `CefResponseFilter` still exist and stream on 7871?** Yes. Adblock verified live on the
  150 build: 2357 engine events and 13 scriptlet injections in one session, YouTube path intact.
- **Q9 — the mac path to enabling the Chromium sandbox.** Windows solved its equivalent: the blocker
  was **the dev safeguard running inside child processes**, which do **not inherit the environment**,
  so `HODOS_DEV` was invisible, the guard `return 1`'d, and every renderer died with no dump. Gate on
  `--type=` from the **command line**, never an env var. Also: setting `browser_subprocess_path`
  silently disables the sandbox, and `no_sandbox=0` is NOT proof — read child **token integrity**.

### 5. What Mac owns in P4, and what just got cheaper

✅ **FB-2 closed = DROP WebGL `UNMASKED_VENDOR/RENDERER`.** No GPU-string map. **So the Mac
Apple-Silicon/Intel ANGLE string work (FB-6, `Q1_mac_farbling.md` §5) is CANCELLED** and must not block
the farbling gate.

Already done for you, needs only a compile check on your side: `FarblingPolicy::InitializeForProfile()`
is wired into `cef_browser_shell_mac.mm` beside `FingerprintProtection::LoadSiteSettings`, and
`FarblingPolicy` uses `SecRandomCopyBytes` on macOS.

Still yours: the arm64/x64/universal2 decision, `minos` (floor is **12.0 Monterey**), framework
packaging, and OOP-context verification (P4e).

### 6. Instruments you should use rather than rebuild

- **`development-docs/0.4.0/chromium-rebuild/farbling_probe.py`** — drives CDP and asserts
  `navigator.webdriver`, the plugin list, and canvas `[native code]` on **both** an auth-exempt and a
  farbled page. Run it against your build: `python farbling_probe.py --port 9322`. After C3 lands, add
  `--expect-native-canvas`.
- **`cef_patch_drift_audit.sh`** — run it *after* the sync, per §1.

### 7. ⚠️ A Blink gotcha that will bite you too: `LOG()` inside Blink is NOT Chromium's

A temporary probe using `LOG(WARNING)` in `blink_glue.cc` failed to compile:
`use of undeclared identifier 'WARNING'`. `base/logging.h` was already included and is irrelevant —
inside `third_party/blink`, `LOG(channel)` is **WTF's macro taking a `WTFLogChannel` object**, so
`WARNING` is looked up as an identifier. The probe was dropped rather than fought; C3 makes
verification behavioural instead.

**If you need diagnostics in CEF code compiled inside Blink, do not reach for `LOG()`.**

### 8. C2's honest status

C2 is **compiled and wired, not behaviourally verified.** Nothing reads the key until C3, so there is
no observable signal to assert against yet. Verification arrives with C3, when
`farbling_probe.py --expect-native-canvas` flips and per-origin/per-profile stability becomes testable.
Recorded as owed, not claimed.
