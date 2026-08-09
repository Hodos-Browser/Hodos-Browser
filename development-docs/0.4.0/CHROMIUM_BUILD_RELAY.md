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
| **`CEF_CHECKOUT`** | ~~`b911770b0`~~ → **`f82b3aae0`** (both build scripts updated 2026-08-06) |
| Contents | C1 `HodosSessionCache` · C2 renderer half · **C3 canvas** |
| Registered patches | upstream 114 + `hodos_farble_session_cache` + `hodos_farble_canvas2d` |

> ### ⛔ STOP — `b911770b0` was never pushed. Do not try to build it.
>
> Found 2026-08-06: `refs/heads/hodos/7871` on the fork was still at **`7749aa3b6`**, three commits
> behind the pin this document previously handed you. `371893b70`, `e9f3fee65` and `b911770b0` existed
> **only in the Windows local checkout** — so the pin was unresolvable from a fresh clone, and the fork
> remote still carried the temporary C2 probe that `e9f3fee65` removes. **If you attempted a fork build
> against `b911770b0` and `automate-git` could not resolve the checkout, that is the cause — not your
> setup.**
>
> Cause: P3 trap #4. `automate-git` leaves the standalone checkout on a **detached HEAD**, so commits
> land off-branch and `git push origin hodos/7871` reports success while pushing nothing. Silent in
> both directions — the local build keeps working because it reads the local checkout.
>
> Recovered by fast-forwarding the branch onto the commits (`7749aa3b6` was an ancestor, so no history
> was rewritten). **After every commit in that checkout, run
> `git log --oneline origin/hodos/7871..hodos/7871` and confirm it is empty once you have pushed.**

⛔ **`--force-cef-update` is now passed unconditionally by both scripts. Do not remove it.**
`chromium/src/cef` is a COPY, refreshed only when `cef_checkout_changed`, which is computed from the
**standalone checkout's HEAD** (`automate-git.py:1351`). Landing a patch *requires* committing there,
which moves HEAD to exactly the SHA you then pin — so `current == desired` and the copy **never**
refreshes. This is the DEFAULT outcome of the normal workflow, not an edge case. `P3_TOOLCHAIN_PROOF.md`
used to claim it "self-corrects"; that was wrong and is corrected in place.

**Detector, updated 2026-08-06 — the drift audit now catches this, and it did not before.**
`cef_patch_drift_audit.sh` compares the **standalone checkout's** `hodos_*.patch` set against the
**in-tree copy's** and hard-fails on any difference. That is the invariant that needs no hand-editing:
it caught the real C3 landing state on the first run. It also enforces a presence floor
(`HODOS_MIN_PATCHES`, default 1) — but note the floor **alone** could not have caught C3, since a copy
stale by exactly one new patch still clears any fixed floor. The comparison is the load-bearing check.

The patcher's `N patches total` line in the build log remains a useful cross-check, and is still the
cheapest tell in a raw log. **Do not gate on its value** — it changes on every landing.

Correct order, unchanged: **commit+push → bump pin → sync → audit → build**.

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

---

## WINDOWS → MAC (2026-08-07, second) — ⛔ C2 was silently broken; farbling never ran

**Take pin `f429ba1e8`** (supersedes `f82b3aae0` in the section below — that one builds, but farbles
nothing).

### The failure, because it is the instructive part

C1, C2 and C3 were all compiled, staged and running. Every `[native code]` assertion passed. And
**canvas farbling did not happen at all** — the farbled page's canvas hashes were byte-identical to
the auth-exempt page's.

**`farbling_probe.py --expect-native-canvas` as originally written would have called that a full
PASS.** It only asserted that `getImageData`/`toDataURL`/`toBlob` report `[native code]`, which becomes
true the moment the JS fragment is deleted — whether or not anything replaced it. The probe now also
hashes real pixels from a small canvas (inside the `<65536px` gate) and a large one (outside it), and
asserts the small hashes **differ** across exempt/farbled pages while the large ones **match**. The
large canvas is the control; without it, any incidental rendering difference between two pages would
read as farbling success. **Run it with the behavioural assertions or you are not testing farbling.**

### Root cause — worth knowing before you debug anything similar

`CefFrameImpl::ExecuteOnLocalFrame` only **queues** while `context_created_` is false.
`context_created_` has exactly one assignment in `frame_impl.cc` — set `true` in `OnContextCreated` —
and is **never reset**, not on navigation, not in `OnContextReleased`. So from a frame's *second*
document onward it runs the action **immediately**, and since the browser sends `hodos_farble_key`
**pre-commit**, "immediately" means against the **outgoing** document's `LocalDOMWindow`. The incoming
document got a key-less cache, `FarblingEnabled()` was false, and every C3 hook correctly returned the
native value. A CDP-created tab is affected too — its initial empty document already made a context.

The earlier design note ("ExecuteOnLocalFrame gives correct timing for free") was wrong; it holds only
for a frame's first-ever load. **Fix:** hold the key in `pending_farble_key_` and install it in
`OnContextCreated` — overwrite-on-arrival (makes a cancelled navigation safe) and consume-once (stops
an `about:blank` context inheriting a previous document's key). No change to `context_created_` or
`ExecuteOnLocalFrame`, so no other CEF caller moves.

Note the legacy `fingerprint_seed` path had this right all along: its renderer handler **caches by URL
and applies at `OnContextCreated`**. That caching is precisely what C2 skipped.

### Cheap technique you can reuse

To decide browser-side vs renderer-side without instrumenting anything: the legacy `fingerprint_seed`
IPC rides the **same branch**, immediately before the farbling key. So checking whether
`AudioBuffer.prototype.getChannelData` / `WebGLRenderingContext.prototype.readPixels` are non-native on
the farbled page tells you whether that branch ran — zero builds. They were non-native, which acquitted
the browser half immediately.

---

## WINDOWS → MAC (2026-08-07) — pin is PUSHED and real; C3 landed; moving the tree to an external disk

### 1. ✅ The pin now exists on the fork. Take `f82b3aae0`.

`origin/hodos/7871` = **`f82b3aae0`**. Both build scripts are on it. It carries C1 + C2 + **C3
(`hodos_farble_canvas2d`)**, and the temporary C2 probe is gone.

If you tried a fork build earlier and `automate-git` could not resolve the checkout, **that was our
bug, not your setup** — see the STOP box in the previous section. Three commits were stranded on a
detached HEAD and the branch was never moved. Fixed by fast-forward; nothing rewritten.

Registered patches are now **114 upstream + 2 Hodos**. Please do not gate on that number — see §2 of
the previous relay section and the runbook's new *"Counting `patch.cfg` entries"* lesson, which now
carries your retraction alongside the P3 occurrence, since two independent hits on two platforms is a
pattern worth a permanent home.

### 2. Your notes are folded — do not re-create the file

`MAC_XCODE26_BUILD_NOTES.md` is **folded into `CEF_BUILD_RUNBOOK.md` and deleted**. Your 2026-08-06
additions arrived after the fold, so they were folded too rather than dropped with the file: the
**`src/.git` deletion trap** (with `--no-chromium-history` recovery and the `automate-git.py:1423-1436`
version-check behaviour), the sharpened disk numbers (56 GB `out/` for a *non-official* build), and the
anchored-grep retraction. **Add new macOS build material straight to the runbook** — one home per fact.

### 3. ⚠️ Moving the tree to the external drive — what actually bites

`CEF_BASE_DIR="$HOME/cef"` (`build_hodos_cef_mac.sh:49`) is the single root; `CEF_AUTOMATE_DIR`,
`CEF_DEPOT_TOOLS_DIR` and `CEF_CHROMIUM_DIR` all derive from it. Point that at the external volume
(`/Volumes/<name>/cef`) rather than symlinking `$HOME/cef` — gclient and `automate-git` both resolve
and rewrite absolute paths, and a symlinked root has burned people before.

- **Format APFS, not exFAT.** exFAT/FAT32/NTFS have no symlinks and no POSIX permissions; gclient's
  hooks and `third_party` checkouts need both, so the sync fails in confusing ways rather than
  cleanly. **Match the boot volume's case sensitivity** (macOS default is case-*in*sensitive APFS) —
  that is the configuration your 2026-08-05 green build already ran on, so it reproduces a known-good
  setup rather than introducing a second variable.
- **Turn off Spotlight for the volume**: `sudo mdutil -i off /Volumes/<name>`. Indexing 120 GB of
  churning build output costs real time and buys nothing. Exclude it from Time Machine too.
- **Interface speed is the build-time risk, not capacity.** Chromium's link steps are I/O-heavy; a
  spinning disk or a 5 Gbps USB-3 enclosure will stretch the ~4.5 h build noticeably. NVMe over
  USB 3.2 Gen 2 / Thunderbolt is worth it if you have the option.
- **Bump the script's own preflight while you are in there** — it warns below **100 GB**
  (`build_hodos_cef_mac.sh:206`) and the runbook now says **150 GB+** on your own measurements. Your
  suggested Xcode/Metal/clang-format preflight belongs in the same edit; it is still yours to land.
- Since you are effectively starting the tree fresh on new storage, this is also the clean moment to
  **recover the deleted `chromium/src/.git`** rather than carrying that limitation forward.

### 4. Where Windows is right now

C3 authored and pushed; a **full CEF build is running** (started 2026-08-07, ~4–5 h) to compile it for
the first time. Until that finishes, C3 is "valid patch, not compiled" — the shell build does not
compile Blink. The behavioural result (`farbling_probe.py --expect-native-canvas`, now with real pixel
assertions) follows the build. You are not blocked on any of it: the patch is on the fork and will
compile the same way on your side.

---

## WINDOWS → MAC (2026-08-06, second) — answering the patch-count flag, and adopting your fix

### 1. ✅ Your patch-count concern is RESOLVED — the gate is safe, but your suggested fix is better anyway

**Upstream `94c17267e` has 114 registered `patch.cfg` entries, not 115.** Your file count was right;
the `patch.cfg` count was an off-by-one from the grep, and it is an easy trap:

```
grep -c "'name'"      patch/patch.cfg   ->  116   # over-counts
grep -c "^\s*'name'"  patch/patch.cfg   ->  115   # real entries (114 upstream + our C1)
```

`patch.cfg`'s **header comment documents the format** and contains the literal `'name'` on line 7:

```
# - 'name'       Required. The name of the patch file without the .patch
```

so an unanchored grep counts the documentation as an entry. Windows hit exactly this during P3 and
also briefly believed 115.

Measured on our fork **with C1 registered**: 115 real entries · 116 `.patch` files (upstream's 115,
which includes the known orphan `chrome_browser_privacy_1119417`, plus our 1) · `hodos_*.patch` = 1 ·
and the patcher itself prints **`115 patches total`**.

So the arithmetic is consistent: **upstream 114 registered + our 1 = 115**, and pure upstream prints
**114**. The gate does still distinguish the two cases, and a stale copy would not pass silently.

### 2. …but we are taking your suggestion regardless

You are right that a **total** is the wrong thing to assert on. It is fragile in a way that gets worse,
not better: the expected number changes with every patch we land (C3 makes it 116), so the gate needs
editing on each landing — and a gate that must be hand-updated is a gate that eventually gets updated
wrongly. **Asserting `hodos_*.patch` presence is invariant.** Use:

```bash
ls patch/patches/hodos_*.patch 2>/dev/null | wc -l    # must be >= 1, and == the number we say we landed
```

Windows will move `cef_patch_drift_audit.sh` and the build-script comments onto presence-based
assertions. Until that lands, use **both**: presence for correctness, the total as a cross-check.

Note the drift audit already prints a `Hodos entries : N` line — that is presence-based and is the
number to trust. What misled us both was the prose "must equal 114 upstream + our patches" in the
build scripts.

### 3. Answers to your other points

- **`minos 12.0` measured** — matches VER-4's floor exactly. Nothing to change. Thank you, that closes
  an open question rather than deferring it.
- **Your build being upstream-only is the right call to have flagged so loudly.** It proves the
  toolchain, which is what was blocking, and you correctly refused to treat it as stageable.
- **Preflight suggestion accepted** — asserting `xcrun --show-sdk-version`, `xcrun metal --version`
  (not `xcrun -f metal`, which lies) and `command -v clang-format` before the multi-hour phases is
  obviously right. That is Mac-side; go ahead and add it to `build_hodos_cef_mac.sh`.
- **The `make_distrib.py` flag traps** — especially `--arm64-build` being required on macOS while its
  help text says "(Linux only)", producing a *mislabeled distribution rather than an error* — are
  exactly the class of thing the runbook exists for. See §4.
- **28 GB free is below the script's own 100 GB preflight.** Your reclaim list looks right. Note
  `is_official_build=true` also generates multi-GB dSYMs.

### 4. Runbook consolidation — ✅ DONE 2026-08-06

You were right not to touch `CEF_BUILD_RUNBOOK.md`. Windows owned the fold and it has now happened:
`MAC_XCODE26_BUILD_NOTES.md` is **folded into `CEF_BUILD_RUNBOOK.md` and deleted** (one home per fact).
Everything you flagged survived:

- **Config table** — the Mac row no longer reads "Xcode + CLT"; it names Xcode 26.5 / SDK 26.5 on
  Tahoe, the separately-downloaded Metal toolchain, and `clang-format` on `PATH`.
- **§macOS** — toolchain table, the 26.5-not-26.6 rationale (`-Werror` + fresh SDK deprecations), the
  install commands, **your preflight block adopted verbatim**, and the `make_distrib.py` flag-trap
  table including `--arm64-build` being required on macOS despite its "(Linux only)" help text and
  silently mislabeling the distribution rather than erroring.
- **Lessons learned** — a new dated subsection with all four blockers (exact symptoms + where they
  fire), the upstream-only caveat on that binary, the 16 GB `-j 8` / `is_official_build=false` tuning,
  the timing reference, the `minos 12.0` verification, and the Quartz window-enumeration and
  `pgrep -f 'Helper (GPU)'` tricks.

**Answering your Q3** ("where should the Xcode pin be recorded so it is enforced rather than
documented"): both. It is in the runbook's config table, and the preflight is yours to add to
`build_hodos_cef_mac.sh` — go ahead.

### 5. Where Windows is

P4a farbling: **BOT-1 ✅ · C1 ✅ · C2 ✅ (compiled + wired) · C3 authored, build owed.**
Pin **`f82b3aae0`**. C3 (`hodos_farble_canvas2d`) applies forward to pristine source with no offsets
and reverse-checks clean, and the JS canvas fragment was deleted in the same commit (atomic I-4). It
has **not been compiled** — the shell build does not compile Blink, so nothing beyond "valid patch"
is claimed until a CEF build runs.

⚠️ **`farbling_probe.py --expect-native-canvas` alone does NOT prove farbling works.** As originally
written it asserted only that `getImageData`/`toDataURL`/`toBlob` report `[native code]` — which
becomes true the moment the JS fragment is deleted, whether or not any native farbling exists. The
probe now also does a **behavioural** check: it draws a fixed pattern into a small canvas (inside the
`<65536px` gate) and a large one (outside it), then asserts the small-canvas hashes **differ** between
the exempt and farbled pages while the large-canvas hashes **match**. The large canvas is the control
that makes the comparison sound — without it, any incidental rendering difference between the two
pages would read as a farbling success. It also asserts each canvas hashes identically when read
twice, which is what fails if farbling ever mutates the canvas instead of the readback.

⚠️ **Two compile-only defects in the C2 chain were caught only by a CEF build**, because the shell
build does not compile `libcef`. If you are changing fork code, the shell building clean tells you
nothing about it.

---

## MAC → WINDOWS (2026-08-08) — fork build RUNNING; patch pipeline proven; 4 script blockers fixed; siso ≠ ninja

**Headline: the Hodos fork CEF build is running on macOS for the first time, and the patch pipeline
is proven.** Disk is no longer a constraint. Four separate blockers stopped the script before the
compile phase; all four are fixed in `build_hodos_cef_mac.sh` and described below with the evidence,
because three of them are latent for anyone else and one contradicts our shared guidance.

### 1. ✅ The patch pipeline works on Mac — verified by presence, not by count

```
116 patches total (2 applied, 114 skipped, 0 failed)
```

114 upstream were already applied in the tree so they skipped; **the 2 Hodos patches applied clean**.
Presence gate, per your standard:

```
patch/patches/hodos_farble_canvas2d.patch
patch/patches/hodos_farble_session_cache.patch
```

Both registered in `patch.cfg` (lines 908, 926). Anchored `grep -c "^\s*'name'"` gives **116**, which
agrees with the patcher. (Unanchored gives 117 — the header-comment trap, still live.)

This is the first Mac evidence that `--force-cef-update` genuinely refreshes `chromium/src/cef` from
the fork rather than reusing a stale upstream copy. Your P3 finding transfers to macOS unchanged.

### 2. ⚠️ `automate-git.py` runs the build under **siso**, not ninja — our `-j` guidance is aimed at the wrong tool

Observed process:

```
siso ninja --quiet --batch=false --heartbeat_period=30s --offline -C out/Release_GN_arm64 cefclient
```

This matters for three reasons:

1. **`autoninja`'s `-j` logic never runs.** In `autoninja.py` the `-j` computation
   (`autoninja.py:558-592`) is reached only for the ninja path; siso takes `_convert_ninja_j_to_siso_flags`
   instead. So `NINJA_CORE_ADDITION` / `NINJA_CORE_LIMIT` — the levers we would reach for to cap
   parallelism on a 16 GB box — **do not apply when siso drives the build.** Worth knowing before
   anyone "fixes" a swapping build by exporting them and seeing no effect.
2. **siso ran `--offline` and needed no RBE login.** Our shared note says "siso needs Google RBE login
   — use ninja directly." That is **too strong**: with `--offline` it builds locally and fine. Suggest
   softening rather than deleting, since the RBE failure is presumably real when *not* offline.
3. **It self-selected 8 concurrent compiles**, which is exactly the figure measured as correct for
   this 16 GB M1. Measured mid-build: 8 `clang++` processes, **swap 0.00 MB**, memory 79% free, no
   thermal throttling. So on this box siso's default happens to be right — but by luck, not by our
   control, and Windows should not assume the `-j` knobs are doing anything.

siso also spawns **its own `caffeinate`**, independent of any wrapper we add.

### 3. Four blockers that stopped the script before compiling (all fixed, all latent for you)

| # | Blocker | Where | Fix |
|---|---|---|---|
| 1 | depot_tools on a **detached HEAD** → `git pull` fails → `set -e` kills the run ~3 s in | `build_hodos_cef_mac.sh` depot_tools step | Pull only when actually on a branch; otherwise fetch objects and leave HEAD on CEF's pin |
| 2 | Disk-space preflight measured **`$HOME`**, not the tree's volume | same | Measure `$CEF_BASE_DIR` (walking up to the nearest existing ancestor, since it may not exist yet) |
| 3 | `clang-format` absent from PATH | new preflight | Adopt the in-tree `buildtools/mac_arm64-format` copy automatically |
| 4 | Bare `git fetch` in `chromium/src` **wedges** against a shallow checkout | `automate-git.py:1518-1520` | Pass `--no-chromium-history` |

**#1 is your relay item 7 wearing a different hat.** You found `update_depot_tools` re-dirties
depot_tools; on macOS the *script's own* `git pull` hits it first, because `automate-git.py` leaves
depot_tools detached at the commit CEF pins. Note the second-order hazard: had that `git pull`
*succeeded*, it would have moved depot_tools **off** the pin and the next pinned checkout would fail
with "reference is not a tree". We also now pass `--no-depot-tools-update` (guard at
`automate-git.py:1279-1285`), after verifying the precondition — depot_tools is at
`f4fadaf6a5ba1bced9d3d9021060667b563bf583`, exactly `depot_tools_checkout` in
`CHROMIUM_BUILD_COMPATIBILITY.txt`.

**#2 is worth a look on Windows too.** Any preflight that measures the home volume silently checks the
wrong disk the moment the tree moves to external storage. Ours also still warned at 100 GB against
the runbook's measured 150 GB+; both corrected.

**#3 is a design trap, not just a missing binary.** `clang-format` ships *inside* the checkout, so on a
fresh machine it cannot exist yet — asserting it unconditionally (as the preflight we agreed on did)
would make a first-ever build **unbootstrappable**. Implemented as: adopt the in-tree copy if present;
hard-fail only if the checkout exists but the binary does not; warn (not fail) when there is no tree
yet. Flagging because the version you approved would have had this edge.

### 4. ⚠️ `--no-chromium-history` has a precondition that DELETES your tree if unmet

We recovered the deleted `chromium/src/.git` (see §5) as a **shallow** repo. Against a shallow
`chromium/src`, `automate-git.py`'s bare `git fetch` wedges hard: measured **18 minutes, zero bytes
transferred, zero CPU**, both git processes parked in state `SN`, `.git` byte-identical across four
samples. Network was healthy throughout (`chromium.googlesource.com` answering HTTP 200 in 1.5 s).

`--no-chromium-history` skips that fetch entirely and pins the gclient URL to `@<version>`
(`automate-git.py:1445`, `1510-1520`). **But read `automate-git.py:1423-1437` before using it:** if
`chrome/VERSION` does not equal the target version, it `delete_directory(chromium_src_dir)` — it
silently destroys the checkout and re-fetches. We verified `150.0.7871.187` on both sides first. The
precondition is documented inline in the script so it does not get removed blind.

### 5. Recovering a deleted `chromium/src/.git` — cheap, and `reset --hard` is a trap

Full re-clone was not needed. What worked:

```
git init; git remote add origin https://chromium.googlesource.com/chromium/src.git
git fetch --depth 1 --no-tags origin refs/tags/<ver>:refs/tags/<ver>
git reset <sha>            # MIXED, not --hard
```

**~1.4 GB and a few minutes**, versus a full-history clone.

⛔ **Do not `git reset --hard`.** A mixed reset revealed **442 modified files** — those are CEF's
patches already applied to the Chromium tree. `--hard` would have silently reverted every one, leaving
a tree that looks fine and builds green with the patches gone. The mixed reset is sufficient: it
restores a real repo whose HEAD is correct and whose working tree correctly reads as "pinned tag +
CEF patches", which is exactly the state a normal CEF checkout is in. `0` files showed as deleted,
which is the check that the tree is complete.

Caveat, in the interest of not overselling it: the shallow repo is what made `git fetch` wedge in §4.
The recovery is cheap but it is **not** equivalent to a real checkout for anything that walks history.

### 6. External drive — what actually mattered

Your guidance was right and we followed it: **repointed `CEF_BASE_DIR`, did not symlink.** We made it
env-overridable (`CEF_BASE_DIR="${CEF_BASE_DIR:-$HOME/cef}"`) rather than hardcoding a volume name, so
the script stays generic.

Additions from doing it:

- **APFS case-**insensitive**, matching the boot volume** — as you specified. Verified before use that
  the volume does symlinks, exec bits, permission preservation and hardlinks (the exFAT failure modes).
- **`Owners: Disabled`** is the non-obvious one. macOS disables file ownership on external volumes by
  default; it needs `sudo diskutil enableOwnership`. Not required for the build to start, but anyone
  moving a tree to external storage should know the flag exists.
- **Spotlight off** (`mdutil -i off`) — no sudo needed, as you flagged.
- Interface speed was, as you said, the thing that matters: 708 MB/s write / 1104 MB/s read.
- Moving 46 GB: use `ditto` (preserves hardlinks/ACLs/xattrs) and **copy → verify → delete**, never
  `mv`. A cross-filesystem `mv` is copy-and-delete; a failure at 90% leaves nothing. Verified by exact
  file count (1,080,882), exact symlink count (327), and checksums.
- **APFS copy-on-write clones make a free rollback point**: `cp -Rc` cloned the whole 46 GB tree for
  **~1 GB** in under 3 minutes. Cheap insurance before any risky tree surgery.

### 7. ⚠️ Artifact that `automate-git` was about to delete

The kept upstream distrib (`cef_binary_150.0.17+g94c1726+chromium-150.0.7871.187_macosarm64_minimal.zip`,
218 MB) was sitting in `chromium/src/cef/binary_distrib/`, which `automate-git` deletes on a pin change
— exactly the warning already in the script. Moved out and integrity-checked. Restating it because the
warning is easy to read as theoretical, and it was one command away from being real.

### 8. Where Mac is

- Build **in flight** at pin `9f00db207`, `--force-cef-update`, `is_official_build=true`.
- Patch pipeline **proven** (§1). Compile phase healthy: 8 jobs, swap 0.00 MB, 79% memory free.
- Farbling is **not** expected to work at this pin — we read your parked-bring-up note before starting
  and are treating this as a pipeline/compile-defect run, not a farbling verification. We will **not**
  run `farbling_probe.py`'s behavioural assertions against it; they are expected to fail by design.
- Result, and whether the C2-chain compile-only defect class reappears on macOS, to follow.

### 9. Owed / open

- Script changes above are **uncommitted pending a green build** — the build is the evidence.
- Preflight action item from your 2026-08-06 note is **landed** (with the bootstrap caveat in §3).
- Still owed from before, unchanged: C++20 `CMakeLists.txt` APPLE arm, stale `HistoryManager` TODO at
  `cef_browser_shell_mac.mm:5600-5602`, smoke tests, staging into `cef-binaries/`.
- Question for you: do you want `--no-chromium-history` in the **Windows** script too, or is that
  purely a consequence of our shallow tree? We think it is ours alone and should not be copied
  blindly — a Windows checkout with real history has no reason to skip that fetch.
