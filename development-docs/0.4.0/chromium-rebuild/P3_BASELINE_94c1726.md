# P3 BASELINE — build-host state before the fork switch

**Captured:** 2026-08-05 · **Host:** Windows build host · **Purpose:** the restore point for P3
(CEF patch toolchain standup). Everything below was measured, not recalled.

This is the snapshot half of P3 commit 1. It exists because P3 changes the `origin` URL of the CEF
checkout that produced the green 150 build, and `automate-git.py` **deletes `chromium/src/cef`**
whenever the CEF checkout target changes (`tools/automate/automate-git.py:1536-1539`). The green
build's distribution tarballs lived inside that directory.

---

## 1. Tree identity

| Item | Value |
|---|---|
| Standalone CEF checkout | `C:\cef\cef150\cef` — **65 MB** |
| Standalone CEF HEAD | `94c17267eb4595a1ad17fb67dee6cdb8ded41c6d` (detached) |
| Standalone CEF `origin` (pre-switch) | `https://github.com/chromiumembedded/cef.git` |
| Standalone CEF working tree | **clean** (0 modified files) |
| In-tree CEF copy | `C:\cef\cef150\chromium\src\cef` — same HEAD, same `origin` |
| Chromium `src` HEAD | `30f6543ae91e6a860e73b76e3216b663b050f4e5` = `refs/tags/150.0.7871.187` |
| Chromium `src` working tree | **442 modified files** — this *is* the applied-patch state |
| `depot_tools` pin | `f4fadaf6a5ba1bced9d3d9021060667b563bf583` (per `CHROMIUM_BUILD_COMPATIBILITY.txt`) |
| Tree total | `C:\cef\cef150` — **195 GB** (986 GB free on `C:`) |
| Staged dist in repo | `cef-binaries/include/cef_version.h` → `150.0.17+g94c1726+chromium-150.0.7871.187` |

The 65 MB / 195 GB split is the whole argument for this snapshot: the part P3 modifies is the
cheap, re-clonable part. The expensive part only has to be left alone.

## 2. Upstream patch baseline (the numbers every P3 acceptance gate compares against)

Measured by **executing** `patch.cfg` the way `tools/patcher.py` does (`exec(compile(...))`), not by
grepping it — grep over this file miscounts, because comments contain the word `name`.

| Metric | Value |
|---|---|
| `patch.cfg` entries | **114** |
| Unique names | 114 (no duplicates) |
| `.patch` files on disk | **115** |
| Registered but missing from disk | none |
| **On disk but NOT registered** | **`chrome_browser_privacy_1119417.patch`** ← pre-existing upstream orphan |
| Entries carrying a `condition` | **0** |
| Entries carrying a non-default `path` | 4 — `v8_build`→`v8`, `tarball_gclient`→`third_party/depot_tools`, `angle_commit_config`→`third_party/angle`, `dawn_dxil_redist`→`third_party/dawn` |

**Three consequences for P3:**

1. **Prior doc counts were all wrong.** `Q5` CEF-1 said `cef/patch/**` is empty; the roadmap said 105
   patches; `PLAN_patch_toolchain.md` §1.1 said ~150. The correct figure is **114 registered / 115 on
   disk**. Corrected in those docs by P3 commit 2.
2. **The orphan is upstream's, not ours, and it must be baselined.** The drift audit's
   registry-integrity check (`PLAN_patch_toolchain.md` §7.1.2) would otherwise exit 1 on every single
   run, and a gate that always fails is a gate that gets ignored. The audit scopes its orphan check to
   the Hodos block and carries this one known upstream orphan as an explicit allowance.
3. **Our `HODOS_FARBLING` gate will be the first `condition` in the file.** That makes the CEF-4
   toggle test unambiguous: the `skipped` count moves 0 → 1 with the env var unset. It also means
   the four non-default `path` values confirm the `path` mechanism is live upstream, so the farbling
   set's reliance on the default (`src`) is sound.

> **Report-line reading.** `patcher.py` reverse-checks every patch and reports **`skip`** when it is
> *already applied* — the same bucket used for condition-gated-off patches. So `skipped` is
> **ambiguous**, and the plan's "expect `applied +1`" only holds against a *fresh* tree. Against this
> already-patched tree the correct expectation after adding one patch is
> **`115 total (1 applied, 114 skipped, 0 failed)`**.

## 3. What was moved, and why

The four distribution tarballs were inside `chromium/src/cef/binary_distrib/` — i.e. inside the
directory `automate-git.py` deletes on a checkout change. Moved **out** of the delete path:

```
C:\cef\cef150\chromium\src\cef\binary_distrib\*.tar.bz2
  →  C:\cef\cef150\binary_distrib_94c1726\
```

| Tarball | Size |
|---|---|
| `..._windows64.tar.bz2` | 166 MB |
| `..._windows64_client.tar.bz2` | 163 MB |
| `..._windows64_minimal.tar.bz2` | 165 MB |
| `..._windows64_release_symbols.tar.bz2` | **898 MB** |

Total **1.39 GB**. The `release_symbols` tarball is the one that genuinely cannot be regenerated
without a full rebuild, and it is the reason this step exists. The *extracted* sibling directories
were deliberately left in place — they are re-extractable from these tarballs, and `cef-binaries/`
in the repo was already staged from them.

## 4. Restore procedure

| To undo | Do |
|---|---|
| The fork switch | `git -C C:/cef/cef150/cef remote set-url origin https://github.com/chromiumembedded/cef.git` |
| A lost standalone CEF checkout | Re-clone (65 MB) and `git checkout 94c1726`. No backup needed. |
| A deleted `chromium/src/cef` | `automate-git.py` re-copies it from `C:\cef\cef150\cef` automatically (line 1597-1599) |
| Lost distrib tarballs | Restore from `C:\cef\cef150\binary_distrib_94c1726\` |
| A lost `chromium/src` patch state | **No cheap restore — do not revert this tree.** Re-syncing + re-patching costs hours. Never run `--force-clean`, `patch_updater.py --revert`, or `--restore` against it. |

## 5. Hands-off list for the rest of P3

- `chromium/src` working tree (the 442 modified files)
- `chromium/src/out/` (the object tree — what keeps a rebuild incremental rather than ~5 h cold)
- `C:\cef\cef150\binary_distrib_94c1726\`
- `C:\cef\chromium_git\` (the preserved 175 GB M136 tree — a separate tree entirely, per D11)
