# macOS CEF 150 build notes — Xcode 26 / Tahoe toolchain

**Status:** Draft for review. Written on the Mac side 2026-08-05/06.
**Intended destination:** consolidate into `CEF_BUILD_RUNBOOK.md` (its "Lessons learned"
section already uses dated subsections — this is written to slot in there).
**Not pushed.** Left untracked deliberately so the Windows side keeps lead on consolidation.

---

## Bottom line

A full CEF 150 macOS ARM64 build **completed green** on 2026-08-05: 57,901 ninja targets,
**0 failures**, ~4.5 hours wall clock. Four blockers were hit and fixed; all four were
**environment/toolchain gaps introduced by the Xcode 26 transition**, none were defects in
CEF or Chromium source.

### ⚠️ Read this before treating the output as shippable

**The binary that was produced is UPSTREAM CEF, not the Hodos fork.** Verified:

| Check | Result |
| --- | --- |
| `cef` checkout remote | `https://github.com/chromiumembedded/cef.git` (upstream) |
| `cef` HEAD | `94c17267e` — upstream 7871 head |
| `patch/patches/hodos_*.patch` | **0 present** |
| Total patches in tree | 115 (all upstream) |
| Resulting version string | `150.0.17+g94c1726+chromium-150.0.7871.187` |

So this build has **zero Hodos patches compiled in** — no farbling, none of the P3 patch set.
It was built from a hand-rolled tree at `~/cef/cef150/`, *not* via
`development-docs/DevOps-CICD/scripts/build_hodos_cef_mac.sh`, and therefore does not honour the
`CEF_CHECKOUT` fork pin (currently `4ed200cf9`).

**What this build IS good for:** it proves the macOS toolchain works end to end on
Tahoe + Xcode 26, and it pins down exactly which toolchain pieces are required. That is the
transferable result, and it is what the rest of this document is about.

**What it is NOT:** a distributable Hodos CEF. Producing that means re-running through
`build_hodos_cef_mac.sh` against the fork pin, with `--force-cef-update`. The toolchain fixes
below are prerequisites for that run and should apply unchanged.

---

## Toolchain requirements (the part worth keeping)

The runbook's Mac row currently reads only *"Xcode + CLT; arch auto-detect"*. That is no longer
sufficient on Chromium 150. Concretely required:

| Component | Required version | Notes |
| --- | --- | --- |
| macOS | 26.x (Tahoe) | 26.6 used. Needed to run Xcode 26. |
| Xcode | **26.5** (build `17F42`), SDK 26.5 | see version rationale below |
| Metal toolchain | separate download, 688 MB | **not bundled with Xcode 26** |
| clang-format | from the Chromium tree | must be on `PATH` for packaging |

### Why Xcode 26.5 specifically, and not 26.6+

`build/config/mac/mac_sdk.gni:51` pins `mac_sdk_official_version = "26.5"`.

That exact pin only binds for `is_official_build` **with hermetic Xcode**. With system Xcode the
real constraint is just `mac_sdk_min = "15"` (`build/config/mac/mac_sdk_overrides.gni:10`), which
is a floor, not an exact match — so a newer SDK will configure fine. 26.5 was still chosen
deliberately: Chromium builds `-Werror`, and a newer SDK can introduce fresh deprecation warnings
that break the build for no benefit. **Recommend pinning the whole team to 26.5.**

Install without the App Store (App Store only offers latest):

```bash
brew install aria2          # optional; the .xip is ~12 GB and aria2 parallelises it
xcodes install 26.5         # prompts for Apple ID + 2FA
sudo xcode-select -s /Applications/Xcode-26.5.0.app/Contents/Developer
sudo xcodebuild -license accept
sudo xcodebuild -runFirstLaunch
xcrun --show-sdk-version    # must print 26.5
```

---

## The four blockers, with exact symptoms

### 1. SDK 15.x is too old — hard compile failure

```
../../skia/ext/skia_utils_mac.mm:84:11: error: use of undeclared identifier
  'kCGImageByteOrder32Host'; did you mean 'kCGImageByteOrder32Big'?
```

Fails at roughly object 4,825/58,002 (in `skia`), so you burn ~10 minutes before seeing it.
`kCGImageByteOrder32Host` exists only in the macOS 26 SDK. **Fix:** Xcode 26.5 as above.

Related: on SDK 15 a `use_clang_modules=false` workaround was needed in `args.gn` because the
older SDK's modulemaps were incompatible. **Remove that arg once on SDK 26** — modules are fine
there, and leaving it off costs Objective-C compile time.

### 2. Xcode 26 unbundles the Metal compiler

```
error: cannot execute tool 'metal' due to missing Metal Toolchain;
use: xcodebuild -downloadComponent MetalToolchain
```

Apple removed the Metal compiler from the Xcode installer in 26 to shrink the download. The
`metal` binary is present as a stub, so `xcrun -f metal` **succeeds and is not a valid check** —
`xcrun metal --version` is. ANGLE needs it to compile `.metal` shaders to `.air`.

```bash
xcodebuild -downloadComponent MetalToolchain     # 688 MB; sudo NOT required
xcrun metal --version                            # verify: "Apple metal version 32023.883"
```

Fails at ~object 5,766/57,901 — about 6 minutes in.

### 3. `clang-format` must be on PATH for packaging

```
FileNotFoundError: [Errno 2] No such file or directory: 'clang-format'
  at clang_util.py:44 → make_distrib.py:319 transfer_gypi_files()
```

`make_distrib.py` reformats the headers it copies and invokes `clang-format` **by bare name**, so
it must resolve via `PATH`. It ships in the Chromium tree:

```bash
export PATH="<tree>/chromium/src/buildtools/mac_arm64-format:$PATH"
```

This one is nasty because it fires *after* the multi-hour compile.

### 4. Missing dSYM at packaging time

```
FileNotFoundError: .../out/Release_GN_arm64/Chromium Embedded Framework.dSYM
```

Not a defect. `make_distrib.py:1392` says it outright:

> `# dSYMs are only generated when is_official_build=true or enable_dsyms=true.`

This build used `is_official_build=false` (see hardware section), so no dSYM existed.
Either pass `--no-symbols` to `make_distrib.py`, or set `enable_dsyms=true` in `args.gn` and
rebuild. **The real Hodos build path uses `is_official_build=true`, so this blocker should not
appear there** — noted only so it isn't misdiagnosed if someone reproduces this config.

---

## ⚠️ Flag trap: `automate-git.py` flags are NOT `make_distrib.py` flags

This cost real time and is worth calling out, because the two sets look interchangeable and are not.

`build_hodos_cef_mac.sh` correctly passes to **`automate-git.py`**:

```
--minimal-distrib --client-distrib --no-debug-build
```

Those are all valid *there*. `automate-git.py` invokes `make_distrib.py` once per distrib type
internally. But passed to **`make_distrib.py` directly**, the equivalents behave differently:

| Flag | On `automate-git.py` | On `make_distrib.py` |
| --- | --- | --- |
| `--no-debug-build` | valid | **does not exist** — `--minimal` already means release-only |
| `--minimal-distrib` / `--client-distrib` | valid together | `--minimal` + `--client` **hard-error as mutually exclusive** (`make_distrib.py:765`) |
| output location | derived | `--output-dir` is **required** |
| `--arm64-build` | — | **required on macOS**, despite help text saying *"(Linux only)"* |

That last one is the dangerous one. Its help string is wrong. Without it, `platform_arch` silently
falls back to `'32'`/x86 (`make_distrib.py:842-853`) and you get a **mislabeled distribution rather
than an error**.

Correct direct invocation:

```bash
python3 make_distrib.py --ninja-build --arm64-build --minimal \
        --output-dir "<tree>/chromium/src/cef/binary_distrib"
```

Also: **missing Doxygen is non-fatal.** It prints `ERROR: Please install Doxygen` /
`ERROR: No docs generated.` and continues. Ignore it, or `brew install doxygen`.

---

## Hardware notes — 16 GB Apple Silicon

Build host was a MacBookPro17,1 (original M1, 8 cores, **16 GB RAM**). Two settings were tuned for
that and are **specific to low-memory machines**, not general recommendations:

- **`ninja -j 8`** instead of the default `ncpu+2 = 10`. Some Chromium TUs peak near 1 GB;
  10 concurrent jobs pushes a 16 GB machine into swap, which is net slower. With `-j 8`, swap
  stayed at **0.00 MB** and memory free at ~76% for the entire build. No thermal throttling
  (`pmset -g therm` clean throughout).
- **`is_official_build=false`.** Official enables ThinLTO + whole-program devirtualization, and the
  Chromium Framework link is the single most memory-hungry step in the build — a genuine OOM risk
  on 16 GB *after* hours of compiling. **The real Hodos build wants `is_official_build=true`**, so
  a machine with more RAM (or a lot of patience and swap headroom) is the better host for the
  shippable build.

Note that flipping `is_official_build` invalidates **every** object file — it is not a cheap thing
to change your mind about. Decide before starting.

Disk: the tree consumed ~53 GB before this build and ~123 GB after; budget **150 GB+** free.

---

## Timing reference (8-core M1, `-j 8`)

| Phase | Duration |
| --- | --- |
| GN gen | ~10 s |
| Full compile+link, 57,901 targets | ~4 h 30 m |
| `make_distrib.py --minimal` | ~2 m |

Early progress badly overstates throughput — the first ~5,000 objects (sqlite, brotli, boringssl)
fly by in minutes, then Blink/V8/WebRTC dominate. Do not extrapolate from the first 10 minutes.

---

## Verification performed on the output

Distribution: `cef_binary_150.0.17+g94c1726+chromium-150.0.7871.187_macosarm64_minimal`
(712 MB unpacked, 229 MB zip).

| Check | Result |
| --- | --- |
| Framework binary type | Mach-O 64-bit dylib, **arm64** |
| `lipo -archs` | `arm64` |
| Install name | `@executable_path/../Frameworks/...` |
| `LC_BUILD_VERSION minos` | **12.0** — runs on macOS 12+ |
| System framework linkage (`otool -L`) | resolves clean |
| Distrib contents | `include/`, `libcef_dll/`, `Release/`, cmake + bazel |

Runtime smoke test — `cefclient.app` launched and stayed up:

| Process | Count |
| --- | --- |
| browser (main) | 1 |
| `--type=gpu-process` | 1 |
| `--type=renderer` | 2 |
| `--type=utility` | 2 |

Window server confirmed one on-screen window, 800×632, layer 0, alpha 1.0 — i.e. it really created
and presented a window, GPU process included.

Useful trick when `osascript`/`screencapture` are blocked by missing Accessibility / Screen
Recording permission (both were, on a fresh Tahoe install):

```bash
/usr/bin/python3 -c "
import Quartz
wl = Quartz.CGWindowListCopyWindowInfo(
    Quartz.kCGWindowListOptionOnScreenOnly | Quartz.kCGWindowListExcludeDesktopElements,
    Quartz.kCGNullWindowID)
print([(w.get('kCGWindowOwnerName'), w.get('kCGWindowBounds')) for w in wl])"
```

Also note `pgrep -f 'Helper (GPU)'` **silently matches nothing** — `pgrep -f` takes an ERE, so the
parentheses are group syntax. Escape them, or match on `--type=` instead.

---

## Open questions for the Windows side

1. **Patch count.** This upstream tree carries **115** patches in `patch/patches/`. The note in
   `build_hodos_cef_mac.sh` says the patcher "must equal 114 upstream + our patches". Is 114 stale,
   or is one of the 115 here not counted the same way? Worth reconciling before the count is used
   as a gate.
2. **Does the `--force-cef-update` finding change anything for Mac?** It was measured on Windows
   2026-08-05 and described as platform-independent; nothing here contradicts that, but the Mac
   fork build hasn't been run yet to confirm.
3. **Where should the Xcode 26.5 pin be recorded** so it is enforced rather than documented —
   the runbook's config table, or a check inside `build_hodos_cef_mac.sh`? A preflight assert on
   `xcrun --show-sdk-version` and `xcrun metal --version` would have saved most of the time lost
   above, since both blockers surface only *after* long build phases.

---

## Suggested preflight (not yet added to any script)

```bash
xcrun --show-sdk-version | grep -q '^26\.' || { echo "Need macOS SDK 26.x (Xcode 26.5)"; exit 1; }
xcrun metal --version >/dev/null 2>&1 || { echo "Metal toolchain missing: xcodebuild -downloadComponent MetalToolchain"; exit 1; }
command -v clang-format >/dev/null || { echo "clang-format not on PATH (buildtools/mac_arm64-format)"; exit 1; }
```

Deliberately checks `xcrun metal --version` and not `xcrun -f metal`, per blocker 2.
