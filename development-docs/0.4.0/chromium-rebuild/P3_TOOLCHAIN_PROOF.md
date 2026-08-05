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

## 4. Remaining P3 gates — ⬜ NOT YET PROVEN

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
