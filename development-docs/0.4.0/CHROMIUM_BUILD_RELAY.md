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

**#4 is the one to plan around.** Budget for it; don't treat it as breakage.

---

## 4. Status of the Windows build

- ✅ P1 pins + DEP-1a..d landed
- ✅ Build-day pin re-check — no newer `7871` point-release
- ✅ M136 codec Layer-A baseline captured
- ✅ VER-5 drift audit script written, M136 baseline **CLEAN**
- ✅ `cef-binaries/Release` + `Resources` backed up before any staging
- ⏳ Checkout finalize running
- ⏳ gn-args gate — **not yet run**
- ⏳ 10–12 h build — **not yet started**

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

## 6. Open questions Mac must answer

1. **D3 arch** — universal2 vs arm64. Recommendation + owner sign-off.
2. **Measured `minos`** — what does `vtool` actually report on the 7871 framework?
3. **Does the M136-skip (D9) reasoning transfer**, or do you need a from-source baseline?
4. **Which of the six Windows failures reproduce** on macOS — especially #4 (429).
5. **Team ID preservation** for D7.
6. **Does `CefResponseFilter` still exist and still stream on 7871?** It is flagged LOW-stability in
   the tracker and it is what strips YouTube ads. Windows will check too; compare notes.
7. **Framework embed / helper-suffix drift** across 14 milestones.

---

## 7. Protocol

`git pull origin 0.4.0` before reading, `git push origin 0.4.0` after writing. Append findings under
a `## MAC → WINDOWS` section here (build specifics) or in `MAC_WINDOWS_RELAY.md` (status). Windows
will append the build result, the codec Layer-A/B comparison and the VER-5 drift outcome to §4 when
the build finishes.
