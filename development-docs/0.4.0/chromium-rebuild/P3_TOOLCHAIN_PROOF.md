# P3 — toolchain proof (CEF-1 / CEF-4 evidence)

**Started:** 2026-08-05 · **Tree:** `C:\cef\cef150`, CEF branch `7871`, Chromium `150.0.7871.187`
**Baseline:** `P3_BASELINE_94c1726.md` · **Plan:** `PLAN_patch_toolchain.md`

Measured evidence for the P3 acceptance gates. Every reading here was produced by running the tool,
not by reasoning about it.

---

## 1. Fork (CEF-1, first half) — ✅ PROVEN

| Item | Value |
|---|---|
| Fork | `Hodos-Browser/cef` — public, parent `chromiumembedded/cef` |
| Upstream branch in fork | `7871` @ `94c17267eb4595a1ad17fb67dee6cdb8ded41c6d` (identical to our pin) |
| Integration branch | `hodos/7871`, created off `7871` at that commit |
| First Hodos commit | `dbab19132` — `hodos_noop_probe` |
| Checkout switched by | `git remote set-url origin` on `C:\cef\cef150\cef` — **no re-clone** |

**URL-validation gate passed** (`automate-git.py --dry-run`, which is safe: `run()`,
`delete_directory`, `copy_directory` and `move_directory` are all dry-run-guarded):

```
--> CEF URL: https://github.com/Hodos-Browser/cef.git
--> CEF Current Checkout: 94c17267eb...   Desired: 94c17267eb...
--> Chromium Current/Desired: 30f6543ae9 (refs/tags/150.0.7871.187)
--> Not building. The source hashes have not changed
```

Post-switch state verified: HEAD unchanged, working tree clean, `origin/hodos/7871` fetchable, the
`94c1726` pin still resolves inside the fork (shared object graph).

> **R9 is dormant but not gone.** That last line confirms `cef_checkout_changed == False` while
> `--checkout=94c1726`, so `delete_directory(cef_src_dir)` does not fire yet. It **will** fire the
> moment we retarget `--checkout` at `hodos/7871` to pick up our own patch commits — which is exactly
> §4 below. Commit 1's tarball move is what makes that safe.

## 2. Probe patch + `HODOS_FARBLING` condition gate (CEF-4) — ✅ PROVEN

Probe: one comment line appended to `src/AUTHORS`. Chosen because it is **touched by none of the 114
upstream patches**, has **zero compile impact**, and cannot change behavior. Registered at the END of
`patch.cfg` in a commented Hodos block, with `'condition': 'HODOS_FARBLING'`.

Run from `chromium/src/cef` via `tools/patcher.py` (apply-only). Seconds, no build.

| Run | Per-patch stdout | Summary line | `AUTHORS` |
|---|---|---|---|
| `HODOS_FARBLING` **unset** | `Skipping patch file hodos_noop_probe` | `115 patches total (0 applied, 115 skipped, 0 failed)` | untouched |
| `HODOS_FARBLING=1` | `... successfully applied.` | **`115 patches total (1 applied, 114 skipped, 0 failed)`** | probe line present, ` M` |
| `HODOS_FARBLING=1` again | already applied | `115 patches total (0 applied, 115 skipped, 0 failed)` | **not duplicated** |

Three things this establishes:

1. **The gate works**, and the predicted middle reading — `115 total (1 applied, 114 skipped, 0 failed)`
   — came out exactly as `PLAN_patch_toolchain.md` §1.1 forecast for an already-patched tree. "Applied
   +1 over 114" would have been the wrong expectation.
2. **The summary line cannot prove the gate.** Rows 1 and 3 are **byte-identical**
   (`0 applied, 115 skipped`) despite meaning completely different things — gated-off vs already-applied.
   Only the per-patch stdout line disambiguates. This is why the acceptance criterion was rewritten.
3. **Re-running the patcher is idempotent and safe** (`git_util.py` reverse-checks first), which is what
   makes the seconds-long apply check viable against the real build tree instead of a throwaway sync.

Working tree restored afterwards: `AUTHORS` clean, dirty count back to the 442 baseline.

## 3. ⚠️ Finding — CRLF contamination hidden by git's stat cache

Hit while authoring the probe, and worth recording because it will bite the C1–C7 authoring work.

`src/AUTHORS` was **75400 bytes (CRLF) in the working tree** while the index held **73458 bytes (LF)** —
yet `git status` reported it **clean**. Cause: the file was checked out under `core.autocrlf=true`
before the build script's `chromium-build-gitconfig` guard was applied; the index's cached
`(size, mtime)` still matched the CRLF file, so `git status` short-circuited and **never compared
content**. `core.autocrlf` is now `false` and `.gitattributes` says nothing about `AUTHORS`.

**Consequence for patch authoring:** `git diff` against such a file emits a **whole-file rewrite**
(the first probe attempt produced a 149 KB diff of all 1942 lines instead of a 9-line one). A patch
like that would be junk.

**Rules adopted:**
- Before authoring a patch, confirm the target is byte-identical to its index blob — compare
  `git hash-object <file>` against `git ls-files -s <file>`. **Do not trust `git status`**, and do not
  use `git update-index --refresh` on this tree to find out (it mutates the index stat cache).
- Author with `git diff --no-prefix` (emits the `-p0` format CEF needs) and **inspect the line count**
  before saving. A one-line change producing a multi-thousand-line diff means contamination.

**Scope checked (read-only, byte-level):** all nine C1–C7 target files that exist on `7871` are
**clean LF and byte-identical to the index** — `execution_context.{cc,h}`,
`base_rendering_context_2d.cc`, `webgl_rendering_context_base.cc`, `audio_buffer.cc`,
`navigator_concurrent_hardware.cc`, `navigator_device_memory.cc`, `core/BUILD.gn`. So the farbling set
is not currently exposed. Contamination is real but sparse; no tree-wide census was run, because
answering it properly would mean mutating the index.

> **Tree change disclosed:** normalizing `AUTHORS` CRLF→LF is the one modification P3 made to the
> Chromium working tree. It is not compiled, it now matches the index, and the tree's dirty count is
> unchanged at 442 — but it is a deviation from the exact bytes that produced the green build, so it is
> recorded here rather than left implicit.
>
> This also **corrects a claim in P3 commit 1**, which described the 442 dirty files as "the applied
> patch state". Measured: **0** of them are whitespace-only, **438** carry real content changes, and the
> remainder are adds/mode changes — so the characterization holds for the 442, but it should not be read
> as implying the rest of the tree is byte-pristine. `AUTHORS` was *clean by git's account* and still
> CRLF-contaminated.

## 4. Drift audit (CEF-2) — ✅ PROVEN, both directions

`DevOps-CICD/scripts/cef_patch_drift_audit.sh`. Clean run against the live tree:

```
patch.cfg entries : 115          .patch files : 116 (+1 allowed upstream orphan)
Hodos entries     : 1  -> hodos_noop_probe  HODOS_FARBLING  src
self-check OK     : 115 entries parsed, path map complete, 4 sub-repo entries
upstream set matches baseline manifest
already applied (reverses cleanly) : 114
would apply cleanly                : 1
applies but at an offset (warn)    : 0
WILL NOT APPLY (hard fail)         : 0
AUDIT_RESULT: CLEAN (exit 0)
```

**Negative test — a gate never proven to fail is not proven.** Registered a patch whose context lines
do not exist in the target:

```
AUDIT_FAIL: hodos_negtest WILL NOT APPLY -- this aborts the build before compile
WILL NOT APPLY (hard fail) : 1
AUDIT_RESULT: HARD FAIL — DO NOT START THE BUILD (exit 1)
```

Removed afterwards; audit returned to `CLEAN (exit 0)`, and the in-tree `patch.cfg` verified
byte-identical to the fork's copy.

### Two design points that make the difference between a working gate and a decorative one

1. **Reverse-check before forward-check.** On an already-patched tree a naive `git apply --check` fails
   for *every* applied patch. A drift audit that reported 114 failures on a healthy tree would be
   switched off within a day. So the audit mirrors `git_util.py`'s order: reverse-check first (reverses
   cleanly ⇒ already applied **and** still matching the tree, itself a strong integrity signal), forward
   only otherwise.
2. **The path map must cover upstream entries, not just ours.** This audit's own first run produced
   **4 confident false failures** — `tarball_gclient`, `v8_build`, `angle_commit_config`,
   `dawn_dxil_redist`, the four upstream patches with a non-default `path`. It had built the target-dir
   map from Hodos entries only, so those four were checked against the Chromium root. The output was
   indistinguishable from real patch rot. Fixed, and a **second self-check** now asserts the map is
   complete and that ≥1 sub-repo entry exists, because that specific malfunction mimics exactly the
   failure the audit exists to detect.

**Honest limitation, stated in the script:** hunk offsets can only be measured for patches *not yet
applied*. Against a fully-patched tree this check proves "present and matching", not "would apply
cleanly to pristine source". Getting the latter requires a fresh sync — which is what the scheduled
fork-watcher (CEF-3) is for.

**Deliberately not reimplemented:** the runtime file-manifest diff and the GN-args gate already exist
(`cef_dist_drift_audit.sh`, `cef_gn_args_gate.sh`). The audit chains the first via `--with-dist` and
tells you to run the second separately (it generates GN projects, which is slow and writes out-dirs).
It prints that skipping is **not** the same as clean, so a skipped section can't be misread as a pass.

## 5. Fork delivery + apply-pre-compile — ✅ PROVEN (and it exposed a footgun)

To make delivery a real proof rather than an assumption, the probe was first **deleted** from the
in-tree copy (`chromium/src/cef` back to pristine 114 entries, no `hodos_noop_probe`). Then
`automate-git.py` was pointed at the fork commit and asked to sync.

### ⚠️ First attempt delivered NOTHING — and reported success

```
--> CEF URL: https://github.com/Hodos-Browser/cef.git
--> CEF Current Checkout: 0a709e5845...   Desired: 0a709e5845... (0a709e584)
--> Chromium Current/Desired: 30f6543ae9 (refs/tags/150.0.7871.187)
```
…and afterwards: in-tree still **114** entries, **no** `hodos_noop_probe.patch`, **no**
`HODOS_PATCHES.md`.

**Mechanism** (`automate-git.py:1358-1360`): `cef_checkout_changed = cef_checkout_new or force_change
or --force-cef-update or cef_current_hash != cef_desired_hash`. The standalone checkout had been
manually put on `0a709e584` already, so current == desired ⇒ **False** ⇒ neither the
`delete_directory(cef_src_dir)` at `:1535-1539` nor the `copy_directory` at `:1597-1599` fires.

> **This is the sharpest trap found in P3.** You can have the correct fork, the correct pin, a clean
> green `automate-git` run — and build with **zero Hodos patches compiled in**, because the tree that
> actually builds is a stale *copy*. Nothing in the output says so.
>
> **Detection:** the patcher's own count in the build log (114 vs 115), and the drift audit's
> `Hodos entries` line. Both would have caught it. This is precisely why the patch-count acceptance
> gate is worth keeping rather than waving through.
>
> **Fix:** `--force-cef-update`, or delete `chromium/src/cef` and let `:1597` re-copy.
>
> ### ⛔ CORRECTION 2026-08-05 (landing C1) — "the normal workflow self-corrects" was WRONG
>
> This section originally read: *"The normal workflow self-corrects — land a patch, bump `--checkout`
> to the new SHA, hashes differ, refresh happens. The trap needs manual intervention in the standalone
> checkout."* **That is not what the code does, and believing it costs you a build.**
>
> `cef_current_hash = get_git_hash(cef_dir, 'HEAD')` (`automate-git.py:1351`) reads the **standalone
> checkout**. Landing a patch *requires* committing there — that is the only place the patch can be
> authored — which moves its `HEAD` to **exactly the SHA you then pin**. So `current == desired`,
> `cef_checkout_changed` is `False`, and the copy is **never** refreshed. The trap does not need
> "manual intervention"; it is the **default outcome of the normal patch-landing workflow**.
>
> Measured while landing C1: pin bumped `0a709e584` → `4ed200cf9`, fork pushed, build launched —
> patcher printed **`114 patches total`**, not 115. A fully green run that would have compiled **zero
> Hodos patches**. Re-run with `--force-cef-update`: **`115 patches total (1 applied, 114 skipped,
> 0 failed)`**.
>
> **Both build scripts now pass `--force-cef-update` unconditionally** — the refresh is a directory
> copy costing seconds, which is not worth trading against a silent multi-hour miscompile. Treat the
> flag as part of the build, not as a recovery step.
>
> ⚠️ **The drift audit will not catch this for you.** `cef_patch_drift_audit.sh` sets
> `CEF_SRC=/c/cef/cef150/chromium/src/cef` — the in-tree copy. Run it straight after committing to the
> fork and it reports `Hodos entries : 0` / `AUDIT_RESULT: CLEAN`, which reads like success and is
> really the audit telling you the copy is stale. Correct order: commit + push → bump pin → sync (with
> `--force-cef-update`) → **then** audit → build, reading the `N patches total` line.

### Second attempt, with `--force-cef-update` — delivered

```
--> Removing directory C:\cef\cef150\chromium\src\cef
--> Copying directory C:\cef\cef150\cef to C:\cef\cef150\chromium\src\cef
```

| Check | Result |
|---|---|
| `hodos_noop_probe.patch` in-tree | ✅ 348 bytes, from the fork |
| `HODOS_PATCHES.md` in-tree | ✅ 8279 bytes |
| in-tree `patch.cfg` entries | ✅ **115**, ours last, `condition: HODOS_FARBLING` |
| `chromium/src` patch state | ✅ intact, 442 |
| `out/Release_GN_x64` | ✅ survived (rebuild stays incremental) |

### Applies pre-compile, in the real build flow — ✅

From the actual `--force-build` run (`gclient_hook.py` → `patcher.py`, not a hand-invoked patcher):

```
Apply hodos_noop_probe.patch in C:\cef\cef150\chromium\src
        1       0       AUTHORS
... successfully applied.
-------------------------------------------------------------------------------
!!!! NOTE: PIPE-A1 pipeline smoke -- remove after standup (CEF-1)
-------------------------------------------------------------------------------

115 patches total (1 applied, 114 skipped, 0 failed)

Generating CEF project files...
```

A patch authored in our fork reached the Chromium source **before compile**, on the real build path,
with the predicted count and the `note` breadcrumb printed. R9's `delete_directory` fired for real and
the distrib tarballs were already out of its way (commit 1).

## 6. Build completes — ✅ PROVEN (`AUTOMATE_EXIT=0`)

Full `--force-build` through the fork, ending in all four distributions:

```
115 patches total (0 applied, 115 skipped, 0 failed)     ← probe already applied from the prior run
[Process 1] Creating tar.bz2 archive for ..._windows64_release_symbols...
[Process 1] Exited with code 0     Execution time: 589.9 seconds
AUTOMATE_EXIT=0
```

The `0 applied / 115 skipped` reading is correct, not a regression: `AUTHORS` still carried the probe
from the earlier run, so the reverse-check reported `already applied (skipping)` — the third state
proven in §2.

### First attempt failed, and it was NOT the patch

Worth recording so nobody re-litigates it. The first run died at **`726 done, 1 failed, 1 remaining`**:

```
FAILED: LINK cefclient.exe
err: exit status 0xc0000142
```

`0xC0000142` is `STATUS_DLL_INIT_FAILED` — `lld-link.exe` could not *initialize*, i.e. the process
failed to launch. It is not a compile or link error. Corroborating evidence: **zero** `error C####` or
`fatal error` lines in 1836 log lines; the same linker had produced a 292 MB `libcef.dll` + 5.2 GB PDB
minutes earlier; and the failure landed at the exact moment the harness force-stopped the task tree.
Re-running the same inputs succeeded. **Collateral from process termination, not a defect.**

Process fix: the build now launches via `Start-Process` and redirects its own output with `exec`, so it
is fully detached and a killed wrapper cannot reach into it.

## 7. ⚠️ FINDING — building from the fork degrades the CEF version string

The fork build produced:

```
CEF_VERSION       "150.0.0-HEAD.3552+g0a709e5+chromium-150.0.7871.187"
CEF_VERSION_PATCH 0
CEF_COMMIT_HASH   "0a709e5845602e5e5dfce35f04552f867c831dc9"
```

versus the staged upstream dist's `"150.0.17+g94c1726+chromium-150.0.7871.187"`, `PATCH 17`.

**Mechanism** (`cef/tools/cef_version.py:189-225`):
1. `on_release_branch = is_ancestor(HEAD, '7871') or is_ancestor(HEAD, 'origin/7871')`. Our patch commits
   are **descendants** of `7871`, not ancestors ⇒ **False**. Any fork that adds commits on top of a
   release branch fails this test by construction.
2. It then reads the branch name — but `automate-git` does `git checkout <rev>`, which **detaches HEAD**,
   so the name is literally `"HEAD"` ⇒ the `master`/`HEAD` arm at `:202` sets `MINOR = PATCH = 0`.

**Why it matters:** the binary no longer reports which upstream security point-release it is based on.
That directly undercuts the CEF-3 tracking duty — you cannot tell from a `150.0.0-HEAD` binary whether
it contains the `150.0.17` fixes. It also changes every distribution directory and tarball name.

**Blast radius is otherwise small:** nothing in `cef-native` compares `CEF_VERSION` (the only repo hits
are the staged header, the stale `cef-binaries-backup/` M136 copy, and a comment in `release.yml`), and
our CI asset name is set by hand. Wrapper/`libcef` consistency is enforced by `CEF_API_HASH`, not this
string, and both come from the same build.

### ✅ TESTED 2026-08-05 — and the test REVERSED the recommendation. **Do not apply the "fix".**

Measured with `cef/tools/cef_version.py current <chromium_src>`, which is the exact computation
`version_manager.py` uses to write `cef_version.h` — seconds, no build required.

| In-tree `src/cef` state | Computed `CEF_VERSION` |
|---|---|
| detached at `0a709e584` (**what we ship today**) | `150.0.0-HEAD.3552+g0a709e5+chromium-150.0.7871.187` |
| on branch `hodos/7871`, **same commit** | `150.0.19-7871.3552+g0a709e5+chromium-150.0.7871.187` |

> **Method note:** a first attempt appeared to disprove the hypothesis, but the test was invalid.
> `VersionFormatter` sets `cef_path = <chromium_src>/cef` (`cef_version.py:25`), so it *always* reads the
> **in-tree copy** — the standalone checkout's branch state is irrelevant. Both runs had been measuring
> the same detached copy.

**The branch fix yields `PATCH 19`, not the 17 I predicted — and that is the reason to reject it.**
`get_cef_branch_version_components()` (`cef_version.py:72-105`) computes `PATCH` as a **count of commits
on the branch that did not modify the API-versions file**. Upstream at `94c1726` had 17; our two patch
commits made it **19**. So the number is *upstream's counter plus our own commits*:

- It **does not equal** the upstream patch level, so it does not actually answer "which upstream release
  is this?" — the thing the fix was supposed to deliver.
- It **drifts ahead of upstream** by our commit count, and will **collide**: upstream will eventually
  publish a real `150.0.19`, after which two materially different binaries both report `150.0.19`,
  distinguishable only by the `-7871.3552+g<sha>` suffix.
- A number that *looks* like an upstream CEF release but isn't is worse than one that obviously isn't.
  `150.0.0-HEAD` cannot be mistaken for an upstream release; `150.0.19-7871` invites exactly that.

### ⚠️ Correcting the severity: the security level was never lost

The original write-up above (and the P3 commit-7 message) said this "undercuts the CEF-3 security-pull
duty." **That was overstated.** `chromium-150.0.7871.187` is present in the version string **in every
variant**, including the current one — and the Chromium point release is what carries the CVE fixes. CEF's
`150.0.x` counter tracks **CEF's own commits**, not Chromium security content.

So what is actually missing is a secondary provenance nicety, and it is **exactly recoverable** from
`CEF_COMMIT_HASH` (`0a709e5845…`), which is written into the same header and identifies the fork commit —
and therefore its upstream ancestor — unambiguously. This is a **cosmetic/provenance** matter, not a
security defect, and not urgent.

### ⭐ SUPERSEDED 2026-08-05 (landing C1): the version string changed on its own, and the owner ACCEPTED it

The decision below assumed `150.0.0-HEAD` was ours to keep. It was not — it was an **artifact of
pinning an intermediate commit**, and it disappeared the moment we pinned a real patch commit.

`git_util.get_branch_name()` falls back on a detached HEAD to `git log -1 --pretty=%d` and takes the
**last** decoration. `0a709e584` was mid-history and carried none → `"HEAD"` → MINOR/PATCH zeroed.
`4ed200cf9` is the branch tip, so its decoration reads `(HEAD, origin/hodos/7871, hodos/7871)` →
`"hodos/7871"` → `.split('/')[-1]` → `"7871"` → real MINOR/PATCH. **Every future landing pins the
commit just pushed, i.e. the branch tip**, so `150.0.0-HEAD` is not reproducible going forward.

Shipped at C1: `CEF_VERSION "150.0.22-7871.3555+g4ed200c+chromium-150.0.7871.187"`, `PATCH 22`
(= upstream 17 + our 5 commits). **Owner accepted 2026-08-05**, collision risk recorded in the fork's
`HODOS_PATCHES.md` §2. Note this was NOT caused by `--force-cef-update`; the SHA pin is unchanged and
still exact. ⚠️ Distribution directory/tarball names now embed this version — the `cef-binaries/`
staging step and the CI asset must not assume a fixed string.

<details><summary>Historical: the superseded decision</summary>

### Decision: keep the SHA pin (status quo). No change to the build scripts.

1. The security-relevant field (`chromium-150.0.7871.187`) is present either way.
2. `150.0.0-HEAD` is *honest* — it is not an upstream release, and it says so. No collision is possible.
3. It preserves an **exact, reproducible pin**, which is the property that matters for a signed
   money-handling build. A branch tip is a moving target: `--checkout=hodos/7871` means two builds a day
   apart can differ in source with no visible change in the build script. Recovering reproducibility
   would need a SHA assertion bolted on — i.e. re-adding the pin we just gave up, to buy a version number
   that is wrong anyway.
4. `CEF_COMMIT_HASH` already gives exact provenance.

**Close the real (small) gap in our records, not in the version string:** the fork-commit → upstream-base
mapping belongs in `HODOS_PATCHES.md` §2 and the tracker, where it is unambiguous and cannot collide.
That is already in place.

## 8. Remaining P3 gates — ⬜ NOT YET PROVEN

| Gate | What it needs | Cost |
|---|---|---|
| **Fork actually delivers the patch** | Retarget `--checkout` at `hodos/7871`, `--no-build --no-distrib`; automate-git deletes + re-copies `src/cef`; confirm `hodos_noop_probe.patch` arrives in the in-tree copy from the fork | sync-only, minutes — **not** a build |
| **Applies pre-compile + builds** | One `--force-build`. Chromium pin unchanged and `out/` intact, so expect incremental, not the 4h49m cold build — **unverified, see below** | one build |
| **Probe removed, count returns to 114** | Delete `.patch` + cfg entry on `hodos/7871`, re-run patcher | seconds |
| **CEF-2 drift audit** | Patch-apply checks + invoke the two existing scripts | — |
| **CEF-3 fork-watcher** | Scheduled upstream-advance PR | — |

> **Open assumption, flagged not hidden:** the build gate is expected to be incremental because the
> Chromium pin is unchanged and `chromium/src/out/` survives. A comment-only `AUTHORS` change should
> recompile nothing at all. That has **not** been measured, and a future patch touching a
> widely-included Blink header could still cascade broadly. Measure at the gate; do not promise it.

</details>
