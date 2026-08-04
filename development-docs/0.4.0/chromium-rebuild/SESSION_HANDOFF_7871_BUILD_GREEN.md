# Session Handoff — CEF 150 / `7871` BUILD IS GREEN

**Session:** 2026-08-03 → 2026-08-04 (Windows execution). **19 commits on `0.4.0`, none pushed.**
**Read after:** `SESSION_BRIEF_WINDOWS_EXECUTION.md` → `KICKOFF_REVIEW_RESULTS_2026_08_03.md` → this.

---

## 1. What was achieved

**The milestone in the brief — "a clean `7871` build with codecs verified" — is MET.**

| | |
|---|---|
| Build | `BUILD_EXIT=0`, **289 min (4 h 49 m)**, Siso, 24× `clang-cl` |
| Pin | `150.0.17+g94c1726+chromium-150.0.7871.187`, Chromium `refs/tags/150.0.7871.187` |
| Tree | `C:\cef\cef150\` with its **own** `depot_tools`. `C:\cef\chromium_git` (M136) preserved |
| `libcef.dll` | 292 MB (M136: 249 MB) |
| Wrapper | `libcef_dll_wrapper.lib` 104 MB, clean against new headers |
| gn-args gate | **PASS** — 1211 args, no codec flag renamed or flipped M136→M150 |
| Codec Layer-A | **PASS** — all 5 GATE rows `probably`, AV1 present, HEVC unchanged |

**Artifacts:** `C:\cef\cef150\chromium\src\cef\binary_distrib\cef_binary_150.0.17+...windows64{,_minimal,_client,_release_symbols}\`
**M136 rollback backup:** `C:\cef\cef150\m136_cef_binaries_backup\` (438 MB).
**`cef-binaries/` in the repo is UNTOUCHED** — staging deliberately not done (see §2).

---

## 2. ⛔ THE ONE BLOCKING DECISION — sandbox → bootstrap (CEF #3928)

**Owner is leaning (A); confirm and execute.**

VER-5 drift audit found:

| File | M136 | `7871` |
|---|---|---|
| `cef_sandbox.lib` | present (we link it) | **ABSENT from the dist** (still *builds*, just not copied) |
| `bootstrap.exe` / `bootstrapc.exe` | — | **NEW** |

CEF replaced sandbox *linking* with a bootstrap *executable*. On Windows `USE_SANDBOX` now defines
**`CEF_USE_BOOTSTRAP`** (`cmake/cef_variables.cmake:609-613`). Upstream's model
(`tests/cefclient/CMakeLists.txt:592-596`):

```cmake
add_library(${CEF_TARGET} SHARED ${SRCS})              # app becomes a DLL
COPY_SINGLE_FILE(... bootstrap.exe → ${CEF_TARGET}.exe)  # CEF's exe, renamed
```

`cef-native/CMakeLists.txt:473` links `cef_sandbox` → **`cef-native` cannot link as-is.**

### Work implied by (A) — do NOT under-scope this
1. `cef-native/CMakeLists.txt` — `HodosBrowserShell` becomes a **SHARED** target + copy/rename
   `bootstrap.exe` → `HodosBrowser.exe`.
2. **Entry point** — `WinMain` in `cef_browser_shell.cpp` moves to the DLL's exported entry.
3. **⚠️ The bootstrap VERIFIES the signature of the client DLL.** CEF commits reference
   *"Log SHA-1 of client DLL when LoadLibraryEx fails"*, *"Log SHA-1 for DLLs when signature
   verification fails"* (#3935), and `cef_certificate_util_win.cc` /
   `cef_scoped_library_loader_win.cc` are new in the wrapper. **`HodosBrowser.dll` must therefore be
   signed**, not just the exe. Confirm the exact requirement before designing the signing step.
4. **`bootstrap: Add sandbox compatibility hash (#4092)`** — there is a compatibility hash between
   bootstrap and libcef. Do not mix versions.
5. **Installer** — `hodos-browser.iss` `[Files]`. Note the whitelist already covers `*.dll`, and
   `HodosBrowser.exe` is listed explicitly, so this may need *less* change than feared — verify.
6. **Dev/prod safeguard + launcher scripts** key off executable path/name (`AppPaths.h`,
   `win_build_run.*`). Re-verify both gates still fire.
7. **⭐ Silent auto-update — the real risk.** The `{app}` manifest changes shape. Per
   `feedback_update_stability_principle`, a REAL N-1→N apply test is mandatory, not optional.

**(B)** hand-copy `cef_sandbox.lib` from `out/Release_GN_x64/obj/cef/` — non-standard, unsupported by
upstream, re-litigated every bump. **(C)** `USE_SANDBOX=OFF` — **rejected**, disables the Chromium
sandbox in a money-handling browser.

---

## 3. Farbling — correctly NOT in this build. Nothing was missed.

A natural worry: *"farbling is a Blink patch, so shouldn't it be compiled into `libcef.dll` before we
build?"* Answer: **yes eventually, no not yet — and this build loses nothing.**

**Measured facts, this tree:**
- `cef/patch/patches/` holds **115 upstream CEF patches, 0 `hodos_*`**. Our fork and patch set do not
  exist yet — P3 has not been stood up.
- **Farbling today is NOT in Blink at all.** It is injected JavaScript from the embedder:
  `cef-native/include/core/FingerprintScript.h` (161 lines,
  `FINGERPRINT_PROTECTION_SCRIPT`), injected in `OnContextCreated`, with per-domain seeds from
  `FingerprintProtection.h`. That code lives in **`cef-native`, not `libcef.dll`.**

**So this `7871` build has exactly the same farbling capability as the shipped M136 build.** Moving
farbling into Blink (P3 → P4) is a *future upgrade*, not a regression this build introduced.

**Why the ordering is deliberate** (`SESSION_BRIEF` §4 "Ordering correction"): P3 (patch toolchain)
is the serial linchpin and blocks all of C1–C7. Codecs are the opposite — already-on GN flags, so
verification is nearly free. Building the unpatched baseline first de-risks everything after it: we
now know the toolchain, pins and codecs are sound, so any future breakage is attributable to *our
patches* rather than to the version bump.

**Consequence to plan for: this binary is NOT the shipping binary.** Every farbling sub-step
(C1…C7) requires its own rebuild — the roadmap budgets **~5 builds, not one**.

> **⚠️ Cost correction.** Those are **INCREMENTAL** rebuilds, ~**30–60 min** each
> (`CEF_BUILD_RUNBOOK.md` line 65), **not** repeats of the 4 h 49 m cold build. A farbling patch
> touches a handful of Blink files; the 65 GiB checkout and the 32k-target full compile happen
> **once**. Budget **~3–5 h total for all of P4**, not ~25 h.
>
> This is also *why* build-first-patch-second is the efficient order, not a detour:
> 1. the expensive part is paid once and reused by every later build;
> 2. the patches and the mechanism to apply them (our fork + `patch.cfg`) do not exist yet, so
>    patching first would have blocked on design work with the toolchain unproven;
> 3. **isolation** — with a proven-green baseline, any future breakage is attributable to *our
>    Blink patches* rather than to the 14-milestone version jump.

**Scope note:** moving farbling into Blink is a **co-equal half of 0.4.0**, not a follow-up. The
version bump is the *enabler* — you cannot patch Blink without a source tree, a fork and a working
build. This session built the foundation; the farbling feature work is still ahead.

---

## 4. Eight failures survived — all in `CEF_BUILD_RUNBOOK.md` Lessons

1. `depot_tools` cloned shallow → `fatal: reference is not a tree` (CEF pins an exact commit)
2. `automate-git.py` fetched from `master` — it is versioned *with* CEF
3. `rd exited with code 3221225794` (`STATUS_DLL_INIT_FAILED`) aborting gclient on an **empty** temp
   dir *after* the 65 GiB clone succeeded — recovery is `rmdir`, **not** a re-clone
4. googlesource **HTTP 429**, also masquerading as `expected 'packfile'` / `expected flush after ref
   listing`
5. `core.autocrlf=true` (Git-for-Windows system default) breaking third_party sub-repo checkouts
6. Flipping autocrlf on an existing tree → equal-count diffstats; `git reset --hard` per repo
7. `update_depot_tools.bat` re-dirtying depot_tools on **every** `automate-git.py` run → use
   `--no-depot-tools-update` and **repo-local** `core.autocrlf=false`
8. **The gate malfunctioning in a way that impersonated a codec regression** — all four flags
   "MISSING". The gate now self-checks (arg count + control flag) before accusing anything

---

## 4b. Macro flow — and where the app first becomes runnable

```
[DONE] Version bump ──── 7871 engine built, codecs verified   (COLD build, once: 4h49m)
   │
   ▼
(A) Bootstrap fix ────── cef-native links again          ◀── ★ FIRST RUNNABLE DEV APP
   │                     app-layer only: cef-native + wrapper CMake.
   │                     NO Chromium rebuild. 7871 engine + today's JS farbling.
   │                     This is the real integration test of the bump.
   ▼
P3  Patch toolchain ──── fork chromiumembedded/cef → Hodos-Browser/cef,
   │                     patch.cfg, prove a NO-OP patch applies + builds
   ▼
P4  Farbling → Blink ─── C1..C7, ~30–60 min INCREMENTAL rebuild each   ◀── ★ RUN AFTER EACH
   │                     each sub-step atomically deletes its JS counterpart
   ▼
P5/P6/P7 ─────────────── DRM spike · full test suite · prod build → v0.4.0-beta.1
```

**Two runnable moments.** The first is close: after **(A)**, the dev app launches on the new engine
with farbling behaving exactly as today. The second is progressive — P4 swaps farbling into Blink
one vector at a time, with a run + smoke after each. The "have to build again" is deliberate
small-step testing, not rework.

## 5. Immediate next steps

1. **Confirm (A)** and execute the bootstrap migration (§2). Nothing else should be staged first.
2. `cef-native` rebuild — **currently blocked** by §2.
3. Stage to `cef-binaries/` **only after** (A) lands and `cef-native` links. Backup already exists.
4. **P5 DRM Spike-1** (~1 h, $0) — unblocked now, independent of §2.
5. Push `0.4.0` (19 commits sitting local) and the macOS relay so the Mac session can start.

## 6. Unverified / owed

- **DEP-1a/b/c (vcpkg manifest, Inno 6.7.1, Brewfile) cannot be tested locally** — they only run in
  `release.yml`. First real exercise is the next release build. A failure there is *that* change,
  not the CEF bump.
- **Layer-B codec smoke** (6 real sites) not run — needs a working `cef-native`, so blocked by §2.
- **beta.29 AV seeding still owed** (unchanged from the brief).
- `enable_platform_dolby_vision=true` — **checked, not a regression**; identical default on M136.
  Do not "fix" it during a bump.
