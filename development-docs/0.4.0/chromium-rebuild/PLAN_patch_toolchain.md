# PLAN — CEF Patch Toolchain Standup (PIPE-A1)

**Created:** 2026-07-10 · **Revised:** 2026-08-05 (P3 kickoff — verified against the pinned tree)
**Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**Status:** IN EXECUTION (P3).

> **⚠️ "GREENFIELD" was dropped from the title, and five substantive corrections landed 2026-08-05.**
> Every mechanism claim below was re-verified against the actual build-host tree (`C:\cef\cef150`,
> CEF `94c1726`) rather than against upstream `master` or recollection. What changed:
>
> | # | Was | Now |
> |---|---|---|
> | 1 | Patches apply via **`run_patch_updater`** in the update phase | **`gclient_hook.py:37` → `patcher.py`**, in the *build* phase. `run_patch_updater` **never applies** on our pinned path (§1.3) |
> | 2 | "~150" / "105" / "`cef/patch/**` empty" | **114 registered entries, 115 files on disk** (§0, §1.1) |
> | 3 | Fork switch **requires a clean CEF dir** (R1, blocking) | One `git remote set-url`. R1 **downgraded**; the real hazard is the previously-undocumented **R9** distrib deletion |
> | 4 | Acceptance = "`applied +1`" | `skipped` is an **ambiguous bucket**; correct reading is `115 total (1 applied, 114 skipped, 0 failed)` (§1.1) |
> | 5 | CEF-2 authors a new manifest + GN-args audit | **Both already exist** — call, don't reimplement (§7.1) |
>
> OQ-1/2/3/4 **closed**, plus a new **OQ-8** (fork visibility) — §11.

**One-line purpose:** Wire the CEF source-patch pipeline to a fork we control (fork `chromiumembedded/cef` → `patch/patches/*.patch` → register in `patch/patch.cfg` → `automate-git.py --url=<fork>` → `patcher.py` applies pre-compile), so the Blink farbling patch set (FEAT-B1 / C1–C7) has a place to land. **This is the serial linchpin that blocks all source-level farbling.**

> **Authoritative inputs:** outline §3b / §3f / §4 P3 / §5 / §8; `DevOps-CICD/CEF_BUILD_RUNBOOK.md` (Step 2, Step 5.5, Step 3 build flow); `0.4.0/B1-farbling-design.md` ("Build integration (CEF patch.cfg)"); `chromium-rebuild/Q5_full_edit_list.md` (rows CEF-1..CEF-4, CEF↔farbling deps); `CEF_VERSION_UPDATE_TRACKER.md`; `DEPENDENCY_VERIFICATION.md`.
> **Primary sources (mechanism):** CEF `tools/patcher.py`, `tools/git_util.py` (the actual `git apply` invocation), `patch/patch.cfg`, `tools/automate/automate-git.py`, `tools/patch_updater.py` (all `github.com/chromiumembedded/cef`); the CEF wiki `branches_and_building.html`; CEF forum threads on persisting Chromium patches via `automate-git.py`. Cited inline.

> **TARGET — RESOLVED (was a placeholder).** **CEF 150 / Chromium 150.0.7871.187 / branch `7871`**, pinned at `150.0.17+g94c1726+chromium-150.0.7871.187` (CEF commit `94c1726`). The M149/`7827` fallback is **dead** — `7827` is already in CEF's *Unsupported* table. Every `<TARGET>` below means **`7871`**. The toolchain design remains branch-agnostic; the measured numbers in §0/§1.1 are specific to `94c1726`.

---

## 0. Verified starting state

> **⚠️ REWRITTEN 2026-08-05 (P3 kickoff).** The original table below said "greenfield" and was wrong in
> both directions: it conflated *"not in the Hodos app repo"* with *"does not exist"*, and it declared
> two scripts absent that are checked in under a different path. Measurements are from the pinned build
> host tree (`C:\cef\cef150`, CEF `94c1726`); full detail in `P3_BASELINE_94c1726.md`.

| Check | Result |
|---|---|
| CEF patch mechanism | **EXISTS AND RUNS ON EVERY BUILD** — `patch/patch.cfg` + `tools/patcher.py` in the CEF checkout, invoked pre-compile. We are adding an entry to a working pipeline. |
| `patch.cfg` entries / `.patch` files | **114 registered / 115 on disk** on `94c1726` (one upstream orphan — §7.1) |
| Entries carrying a `condition` | **0** — ours will be the first |
| `cef/patch/`, `patcher.py`, `automate-git.py` **in the Hodos app repo** | absent **by design** — they live in the CEF checkout, which is what the fork replaces. Nothing to check in. |
| `build_hodos_cef.bat` / `_mac.sh` | **PRESENT** at `development-docs/DevOps-CICD/scripts/` — not at a repo-root `scripts/`. **OQ-1 RESOLVED:** that path is canonical; the 35 references to a **repo-root** `scripts/build_hodos_cef*` across 12 docs were the bug — rewritten in P3 commit 2. |
| File-manifest drift audit | **PRESENT** — `DevOps-CICD/scripts/cef_dist_drift_audit.sh` |
| GN-args gate | **PRESENT** — `DevOps-CICD/scripts/cef_gn_args_gate.sh` |

**Consequence — the opposite of the original one:** most of this plan is *extension*, not authoring.
Genuinely net-new is only: our fork, our `.patch` files, the patch-apply half of the drift audit, and
the fork-watcher. The manifest and GN-args halves of CEF-2 already exist and must be **called**, not
reimplemented (§7.1).

---

## 1. How the CEF patch mechanism actually works (primary-source reference)

The pipeline we are adopting is **CEF's own built-in mechanism** — we are not inventing a patch system, we are populating the one `automate-git.py` already runs on every build. Three files matter.

### 1.1 `patch/patch.cfg` — the registry (Python, not JSON)
`patch.cfg` is **executed as Python** by the patcher (`exec(compile(open(config_file).read(), ...))`), exposing a top-level list named `patches`. Each entry is a dict:

| Field | Required | Meaning |
|---|---|---|
| `'name'` | **yes** | Patch filename **without** the `.patch` extension; the file lives in `patch/patches/<name>.patch`. |
| `'path'` | no | Repo root the patch applies against. **Defaults to the Chromium `src` tree.** Set it for patches that target a sub-repo (e.g. `'third_party/depot_tools'`, as CEF's own `tarball_gclient` entry does). |
| `'condition'` | no | **Name of an environment variable.** The patch is applied **only if that env var is set** in the build environment. This is our build-time on/off gate (§5). |
| `'note'` | no | Message printed after the patch applies (human breadcrumb). |

Representative upstream entries (verbatim shape):
```python
patches = [
  { 'name': 'gritsettings' },                                  # applies to Chromium src (default path)
  { 'name': 'gn_config' },
  { 'name': 'tarball_gclient', 'path': 'third_party/depot_tools' },
  # ... 114 upstream CEF entries total on branch 7871 @ 94c1726 ...
]
```
**Count discipline:** `patch.cfg` is Python — **parse it by `exec`ing it, never by grepping.** The
header comment block contains the words `name`, `path` and `condition`, so `grep -c` over this file
overcounts. Verified figures on `94c1726`: **114 entries, 115 files on disk**, 4 entries using a
non-default `path` (`v8`, `third_party/depot_tools`, `third_party/angle`, `third_party/dawn`),
0 using `condition`. (Earlier doc figures of "empty", "105" and "~150" were all wrong —
`P3_BASELINE_94c1726.md` §2.)

There is **no `sha1`/checksum/version field** — integrity is by filename + the patch's own context lines. Note CEF applies with `git apply` (§1.2), which is **exact-context, not fuzzy** — there is effectively no fuzz tolerance; a context mismatch fails loud rather than shifting. *(Source: chromiumembedded/cef `patch/patch.cfg`.)*

### 1.2 `tools/patcher.py` / `tools/patch_updater.py` — the applier
- Reads `patch.cfg`, iterates `patches`, and for each entry applies `patch/patches/<name>.patch` to the tree at `path` (default `src`).
- **How the patch is applied (matters for drift):** `patcher.py` calls `tools/git_util.py::git_apply_patch_file`, which runs `git apply --check` then `git apply -p0 --ignore-whitespace`. This means: **(a)** patch paths are consumed at **`-p0`** — zero leading path components stripped (unlike a normal `git diff` which is authored for `-p1`); **(b)** whitespace differences are ignored; **(c)** there is **NO fuzz / `-C` context reduction / `--3way` / `--recount` / retry** — application is exact-context and **fail-loud**. (`git_util` only falls back to system `patch` if the target dir is not a git repo, which never applies to the Chromium `src` tree.)
- **Condition gate:** `if patch['condition'] not in os.environ: dopatch = False` → the patch is **skipped** (not failed) when its env var is unset.
- **Reporting:** prints `'%d patches total (%d applied, %d skipped, %d failed)'` — **no fuzz metric is emitted** (there is no fuzz to report). **On any failure it exits status 1 and prints revert instructions** — a failed patch **aborts the build before compile** (this is what makes drift loud, see §7).
- **⚠️ `skipped` is AMBIGUOUS — two unrelated causes share the bucket.** `git_util.py :: git_apply_patch_file` runs a **reverse-check** (`git apply --reverse --check`) before applying, and returns `'skip'` when the patch is **already applied**. `patcher.py` also returns `'skip'` for **condition-gated-off**, and for a **missing target directory**. So a `skipped` count alone cannot distinguish "gate is off" from "already applied" from "target dir vanished". Consequences: **(a)** the CEF-4 toggle proof must read the *per-patch* stdout lines (`Skipping patch file X` = gate off, vs `... already applied (skipping).` = reverse-check), not the summary count; **(b)** this plan's repeated acceptance criterion "expect `applied +1`" **only holds against a freshly-synced tree.** Against an already-patched tree — which is what the build host has — adding one patch correctly reads **`115 total (1 applied, 114 skipped, 0 failed)`**. Every "+1 applied" gate below is annotated accordingly.
- **Idempotent by construction:** because of that reverse-check, re-running `patcher.py` against an already-patched tree is a safe no-op. This is what makes the seconds-long apply check in Step 3 possible without a build or a clean tree.
- Single-file mode: `--patch-file FILE --patch-dir DIR` applies one patch outside the cfg loop (useful for testing a candidate patch in isolation).
- Authoring/regeneration: `patch_updater.py` both **re-applies** all registered patches to a fresh checkout and, in its resave mode, **regenerates the `.patch` files from a modified `src` tree** — this is the supported way to author/update a patch after hand-editing Chromium source (§8.2). **⚠️ `patch_updater.py` has no dry-run mode** and is **write-capable** — with `--restore` it *resaves* `.patch` files when the backed-up source changed, so it must never be pointed at the canonical fork as a "read-only check" (use `patcher.py` / `git apply --check` for that — I1, §7.1). Exact resave/add flag names, and the exact arg string `automate-git` passes on the normal build path, to confirm against the target-branch `patch_updater.py` — **OQ-2**.

*(Sources: chromiumembedded/cef `tools/patcher.py`, `tools/patch_updater.py`.)*

### 1.3 `tools/automate/automate-git.py` — where patches enter the build
- `--url=<git url>` points the checkout at **our CEF fork** instead of the upstream default, which is now **`https://github.com/chromiumembedded/cef.git`** (the current `automate-git.py` default `cef_url`; Bitbucket is the legacy remote — see §2.1 / OQ-4). The script **validates the URL against any existing checkout** (`:1320-1326`: `Requested CEF checkout URL … does not match existing URL` → hard error).
- **⚠️ "a fork switch requires a clean CEF dir" is FALSE — corrected 2026-08-05.** The check reads the URL via `get_git_url(cef_dir)` (`:274-278`), which is just `git config --get remote.origin.url`. We own that checkout, so the cheap fix is to change what it reports:
  ```
  git -C C:/cef/cef150/cef remote set-url origin https://github.com/Hodos-Browser/cef.git
  ```
  Our fork is a **GitHub fork of upstream — same object graph** — so the pinned `94c1726` resolves inside it and `--checkout` keeps working. **Zero re-clone, zero re-sync.** Two further outs: the whole check is bypassed by `--no-cef-update` (`:1321` is `if not options.nocefupdate and ...`), and the standalone checkout is only **65 MB** anyway, so even a real re-clone is a sub-minute operation. **R1 is downgraded from a blocking risk to a one-line command** (§10).
- **⚠️ The actual destructive hazard is elsewhere and was undocumented — see R9.** At `:1535-1539`, when `cef_checkout_changed` is true, `automate-git.py` calls **`delete_directory(cef_src_dir)`** — and `chromium/src/cef/binary_distrib/` is *inside* that directory. Pointing `--checkout` at a new fork revision therefore deletes the previous build's distribution tarballs, including the 898 MB `release_symbols` one that cannot be regenerated without a full rebuild. Moved aside in P3 commit 1 (`P3_BASELINE_94c1726.md` §3).
- `--branch=<n>` selects the CEF branch (and the per-branch sub-checkout name); the download root is set separately by `--download-dir`. `--checkout=<rev>` optionally pins an exact CEF revision (default = `origin/<branch>`).
- **Apply point — ⚠️ CORRECTED 2026-08-05.** This plan previously named **`run_patch_updater`** as the step that applies our patches. **That is wrong, and on our pinned build path `run_patch_updater` never applies anything at all.** Verified against `94c1726`:

  | `run_patch_updater` call site | Args | Fires on our path? |
  |---|---|---|
  | `:1369`, `:1543` | `--backup --revert` | only when the checkout is *changing* — and it **reverts**, never applies |
  | `:1596` | `--reapply --restore` | **no** — guarded by `cef_dir == cef_src_dir`, i.e. `--fast-update` mode, which we don't pass |
  | `:1625` | *(no args)* | **no** — guarded by `chromium_checkout != chromium_compat_version`. We pin `--checkout=94c1726`, so Chromium **is** the compat version and this is False |
  | `:1628` | `--resave` | **no** — requires `--resave` |

  **The real apply point is `cef/tools/gclient_hook.py:37` → `tools/patcher.py`** (the full `patch.cfg` loop), invoked from `automate-git.py:1671-1672` inside the **build** step, immediately before `autoninja`.

  The plan's *conclusion* survives — our patches do land on the Chromium source pre-compile on every build, with no wiring beyond `patch.cfg`. Three consequences change:

  1. **`--force-build` alone re-applies patches.** No re-sync, no re-checkout, no `--force-clean` needed to iterate on a patch. The P3 loop is far cheaper than this plan assumed.
  2. **`patcher.py` runs from `chromium/src/cef`, and `src/cef` is only re-copied from the standalone checkout when `cef_checkout_changed`** (`:1597-1599`). So **editing the standalone `C:\cef\cef150\cef\patch\` does not propagate to the tree that actually builds.** For iteration, edit `chromium/src/cef/patch/` (also a full git checkout at the same HEAD); for reproducibility, let the fork-checkout copy carry it.
  3. **The `condition` env var is read by `patcher.py`**, which inherits the environment from the build script through `automate-git` → `gclient_hook`. A `set HODOS_FARBLING=1` at the top of `build_hodos_cef.bat` reaches it correctly (§5).

  **OQ-2 is RESOLVED, not deferred.** `patch_updater.py` on 7871 accepts `--resave --reapply --revert --backup --restore --patch --add`; the authoring path is `--resave --patch <name> --add <path>` (`cef/docs/chromium_update.md:136`). There is no "exact `run_patch_updater` arg string on the build path" left to confirm, because there is no such call.

*(Sources: chromiumembedded/cef `tools/automate/automate-git.py`; CEF forum "Persist Chromium Patch Using automate-git.py".)*

> **Key implication:** once the fork is wired via `--url`, adding a farbling patch is **two edits inside the fork** (drop `patch/patches/hodos_farble_canvas2d.patch` + append one dict to `patch.cfg`) — no change to Hodos's app repo, no change to the build invocation. The toolchain's whole job is to make that true and keep it true across rebases.

---

## 2. Fork hosting + maintenance model (the recurring cost)

### 2.1 Where the fork lives
Fork `chromiumembedded/cef` into the **Hodos-Browser GitHub org** (the signed-build remote already trusted by the release side): `github.com/Hodos-Browser/cef` (name TBD — **OQ-3**). Rationale:
- `--url` needs a stable git URL we control and can pin/tag.
- Org-owned keeps it inside the same trust boundary as `cef-binaries` releases.
- CEF upstream is now canonically on **GitHub** (`github.com/chromiumembedded/cef`) — it is `automate-git.py`'s default clone source and where the CEF issue tracker migrated (2023). **Bitbucket (`bitbucket.org/chromiumembedded/cef`) is the legacy remote** and the one more likely to lag. Rebase our fork from the **GitHub** upstream (authoritative + `gh`/Actions ergonomics); treat Bitbucket as legacy/possibly-stale. Record the chosen remote in `HODOS_PATCHES.md` (**OQ-4**).

### 2.2 Branch strategy inside the fork
- Track upstream branches **by branch number** (CEF branches map 1:1 to Chromium milestones — e.g. branch `7103`=M136; the TARGET branch, illustrated here as `7827`≈M149, is an **unverified placeholder** — confirm the exact branch number from `cef-builds.spotifycdn.com/index.json` in the version-target plan; do not let `7827` harden by repetition). Our fork carries a **long-lived integration branch per pinned CEF branch**, e.g. `hodos/<branch>` (`hodos/<TARGET>`), created off upstream `<branch>` with our patch commits on top.
- Our actual source edits live **only** as files under `patch/patches/*.patch` + entries in `patch/patch.cfg` — i.e. our delta from upstream is **a handful of added files + a few appended cfg lines**, nothing touching Chromium source directly in the fork. This keeps the rebase surface tiny (the patches themselves absorb Chromium churn at apply-time, not at fork-merge-time).
- Tag each build's exact fork revision (`hodos-cef-<branch>-<date>`) so a build is reproducible (feeds `CEF_VERSION_UPDATE_TRACKER.md` changelog).

### 2.3 The rebase-on-upstream maintenance model (two cadences, mirrors the runbook)
The runbook (§Step 1 "Cadence") defines two rebases; the fork participates in both:

| Cadence | Trigger | Fork action | Patch impact |
|---|---|---|---|
| **Quarterly (cheap)** | Security point-release of the pinned CEF branch | Pull upstream `<branch>` HEAD into `hodos/<branch>`; **re-run the drift audit (§7)**; patches usually re-apply clean (no hunk offsets) | Trivial re-apply expected |
| **~6-monthly (expensive)** | Milestone jump to next CEF branch (e.g. 7827→next LTS/stable) | Create `hodos/<newbranch>` off upstream; **re-generate every `.patch`** against the new Chromium source (high-churn Blink files **will** conflict); full dependency + drift pass | **Budget patch-rework hours** — the primary recurring cost (outline I10) |

> **Standing security duty (Q5 row CEF-3 / outline M6):** between milestone jumps, the fork **must pull upstream in-branch security commits**, or the "we bumped for security coverage" benefit erodes. Automate as a scheduled `gh`/Actions job that opens a PR when upstream `<branch>` advances (design in §7.4). This is a **recurring obligation, not one-time setup.**

> **Per-bump patch-rebase estimate (feeds the version-target plan / `CEF_VERSION_UPDATE_TRACKER.md`):** on a milestone jump, budget **~2–8 h** to rebase the ~5–8 farbling patches (B1-farbling-design.md "Maintenance"), driven by churn in `base_rendering_context_2d.cc` (riskiest) and `webgl_rendering_context_base.cc`. Record actual hours each bump to sharpen the estimate and inform stable-vs-LTS.

---

## 3. Directory layout

Two distinct trees — **do not conflate them.**

### 3.1 Inside the CEF fork (what `--url` checks out; where patches actually live)
```
cef/                                   (= our fork, e.g. Hodos-Browser/cef, branch hodos/<branch>)
├─ patch/
│  ├─ patch.cfg                        # append Hodos entries at END of the `patches` list (§4.3)
│  └─ patches/
│     ├─ ...~150 upstream CEF patches... (DO NOT edit — upstream-owned)
│     ├─ hodos_farble_session_cache.patch     # C1 (Supplement)   — added by FEAT-B1
│     ├─ hodos_farble_seed_wiring.patch        # C2
│     ├─ hodos_farble_canvas2d.patch           # C3
│     ├─ hodos_farble_webgl.patch              # C4
│     ├─ hodos_farble_webaudio.patch           # C5
│     ├─ hodos_farble_navigator.patch          # C6
│     └─ hodos_farble_auth_exempt.patch        # C7
├─ tools/
│  ├─ patcher.py                       # upstream (unchanged)
│  ├─ patch_updater.py                 # upstream (unchanged) — used to author/reapply
│  └─ automate/automate-git.py         # upstream (unchanged)
└─ HODOS_PATCHES.md                    # NEW — our patch manifest / provenance ledger (§4.4)
```
**Naming convention for our patches:** `hodos_<feature>_<area>.patch` (all-lowercase, `hodos_` prefix so they sort together and never collide with upstream names). Registry `'note'` records the owning feature + the outline/Q5 row ID.

### 3.2 Inside the Hodos-Browser app repo (glue that points the build at the fork)
```
development-docs/DevOps-CICD/scripts/        # <-- canonical (OQ-1 (c)); NOT a repo-root scripts/
├─ build_hodos_cef.bat                 # EXISTS — edit: add --url=<fork> [--checkout=<pin>]
├─ build_hodos_cef_mac.sh              # EXISTS — same, Mac
├─ cef_dist_drift_audit.sh             # EXISTS — file-manifest half of CEF-2; CALL, don't reimplement
├─ cef_gn_args_gate.sh                 # EXISTS — GN-args half of CEF-2; CALL, don't reimplement
├─ chromium-build-gitconfig            # EXISTS — CRLF guard (load-bearing for -p0 exact-context)
├─ cef_patch_baseline_7871.txt         # NEW (P3 commit 1) — 115-name baseline manifest
└─ cef_patch_drift_audit.sh            # NEW — §7.1 checks 1-3 only, + invokes the two above
development-docs/0.4.0/chromium-rebuild/
├─ PLAN_patch_toolchain.md             # this doc
└─ P3_BASELINE_94c1726.md              # NEW (P3 commit 1) — restore point + measured baseline
```
> The **only** app-repo change to *use* the toolchain is adding `--url`/`--checkout` to the two build scripts (§6). All patch content lives in the fork.

---

## 4. Standup steps (followable)

Ordered; each has an acceptance gate. **Phase P3 in the outline; runs after P2 (bump) proves the unchanged pipeline builds on TARGET.** Steps 1–5 stand up an **empty but wired** toolchain proven with a **no-op patch** — the farbling set (§8) is the first real consumer and does **not** block standup.

### Step 1 — Fork + branch
1. Fork `chromiumembedded/cef` → `Hodos-Browser/cef` (OQ-3).
2. Record the chosen **upstream remote** (GitHub authoritative vs legacy Bitbucket — OQ-4) in `HODOS_PATCHES.md`.
3. Create `hodos/<branch>` off upstream `<branch>` (TARGET branch number from the version-target plan).
4. **Acceptance:** `hodos/<branch>` exists, is byte-identical to upstream `<branch>` (zero Hodos commits yet), and its git URL is reachable by the build host.

### Step 2 — Point the build at the fork
1. Add to both build scripts (§6): `--url=https://github.com/Hodos-Browser/cef.git --branch=<TARGET> --checkout=<fork-rev-or-branch>`.
2. **URL switch (corrected — no clean dir needed):** `git -C C:/cef/cef150/cef remote set-url origin <fork>`. Same object graph, so the pin still resolves. Do **not** delete the CEF checkout, and never touch `chromium_git/` or `chromium/src`. (§1.3)
3. **Acceptance:** `git -C C:/cef/cef150/cef remote -v` shows the fork URL, and `automate-git.py --dry-run` gets **past** the URL validation at `:1320-1326` printing `CEF URL: <fork>`. Note the baseline patch-report line comes from **`patcher.py` via `gclient_hook.py`** during the *build* step, not from `run_patch_updater` (§1.3) — so the full baseline reading (`114 patches total`, 0 failed) is observed at the Step-3/§9 build, not here.

### Step 3 — Prove the pipeline with a no-op patch
1. Author a trivial, harmless patch (e.g. add a comment line to a stable, low-churn Chromium file, or a `.md`/`OWNERS` no-op) via the authoring workflow (§8.2).
2. Save as `patch/patches/hodos_noop_probe.patch`; append `{ 'name': 'hodos_noop_probe', 'note': 'PIPE-A1 pipeline smoke — remove after standup' }` to `patch.cfg`.
3. Verify apply via the **apply-only path**, run from `chromium/src/cef` (so `patcher.py` resolves `src_dir` to the Chromium tree — it derives both paths from its own location): single-file mode `python tools/patcher.py --patch-file hodos_noop_probe --patch-dir <dir>`, or a bare `git apply --check -p0 --ignore-whitespace`. **Do not use `patch_updater.py --reapply`/`--restore`** (write-capable, resaves `.patch` files — I1). This is a **seconds-long** check, not a build.
   > **⚠️ Never run `patcher.py` from the standalone `C:\cef\cef150\cef`.** Its `src_dir` is the *parent* of the CEF dir, which there is `C:\cef\cef150` — not a git repo. `git_util.py` would fall back to the **GNU `patch` tool with `--force`** (`_patch_apply_patch_string`), which *does* fuzz and *can* misland. The exact-context / fail-loud guarantee this plan relies on holds **only** inside `chromium/src`.
4. **Acceptance:** `0 failed`, and the change is present in the Chromium `src` tree pre-compile. On the build host's **already-patched** tree the summary line reads **`115 total (1 applied, 114 skipped, 0 failed)`** — *not* "applied +1 over 114" (§1.1 `skipped` ambiguity). Confirming apply-health does **not** require a full build; a single end-to-end build is the final gate (§9), **not** one per probe/toggle iteration. **Then remove the probe** (patch file + cfg entry) and re-verify the count returns to baseline.

### Step 4 — Wire the `condition` env gate (optional-but-recommended, Q5 row CEF-4)
1. Decide the gate variable name (recommend `HODOS_FARBLING` — §5).
2. Prove it via the **apply-only path** (`patcher.py` from `chromium/src/cef` — no build needed): register the no-op probe with `'condition': 'HODOS_FARBLING'`; run once **without** the env var, once **with** it.
3. **Acceptance — read the per-patch stdout line, not the summary count.** Because `skipped` is an ambiguous bucket (§1.1), the summary alone cannot prove the gate worked. The unambiguous signals are:
   - env var **unset** → `Skipping patch file hodos_noop_probe` (emitted by `patcher.py` *before* it ever calls the applier)
   - env var **set**, patch not yet applied → `... successfully applied.`
   - env var **set**, patch already applied → `... already applied (skipping).` — also lands in `skipped`, which is exactly why the summary count is not the proof
   In all three cases `failed` must be **0**. Upstream carries **zero** `condition` entries on 7871, so ours is the first and no other patch can perturb the reading.

### Step 5 — Stand up the drift-audit hook (Q5 row CEF-2)
1. Land `scripts/cef_patch_drift_audit.py` (§7).
2. Run it against the current fork/branch to establish the **baseline manifest + expected patch-apply report**.
3. **Acceptance:** the script runs clean (0 failed, no hunk offsets, manifest matches) and emits a human-readable report; wire it as a **pre-build gate** in the build scripts and as a scheduled job (§7.4).

### Step 6 — Document + register the recurring duties
1. Fill `HODOS_PATCHES.md` (§4.4) with the initial (empty) manifest + the maintenance model (§2.3).
2. Append a `CEF_VERSION_UPDATE_TRACKER.md` entry: fork URL, branch, upstream remote, standup date.
3. Add the **security-pull automation** design task (§7.4) to the DevOps backlog.
4. **Acceptance:** a new engineer can, from `HODOS_PATCHES.md` alone, add a patch and know the rebase/security-pull cadence.

### 4.3 `patch.cfg` edit discipline
- **Append Hodos entries at the END** of the `patches` list, in a clearly-commented `# --- Hodos patches (see HODOS_PATCHES.md) ---` block. Never interleave with upstream entries (keeps rebase diffs clean).
- One dict per patch; always set `'note'` to `"<feature> — <Q5 row id>"`.
- Patch **order matters** if two patches touch the same file — list them in dependency order (C1 Supplement before C3–C7 that read it). Farbling patches touch **disjoint** Blink files (canvas vs webgl vs audio vs navigator), so cross-patch conflicts are unlikely *except* C1↔C3/C4 if the Supplement adds includes to a file a later patch also edits — **verify at author time.**

### 4.4 `HODOS_PATCHES.md` (new, in the fork) — the patch ledger
Per-patch row: `name` · owning feature (C1..C7) · Q5 row id · Blink/Chromium files touched · upstream remote+branch it was generated against · last-rebase date · last-apply reading (clean / hunk-offset lines) · `condition` (if any). This is the institutional memory the drift audit checks against and the rebase engineer works from.

---

## 5. The `condition` build-time on/off gate

Adopt **one** env-var gate for the whole farbling patch set: **`HODOS_FARBLING`**.
- Every farbling patch entry carries `'condition': 'HODOS_FARBLING'`.
- Set it in the build scripts' env block by default (farbling ON in shipped builds).
- **Escape hatch (outline §8 #12 / #13):** if the Blink patches destabilize beta.1 at gate time, a rebuild with `HODOS_FARBLING` **unset** produces a farbling-free binary **without touching `patch.cfg` or reverting commits** — the patches are simply *skipped*. This is cleaner than the documented full-branch rollback (#13) and complementary to it.
- **Caveat — mixed conditions:** if C2's seed-wiring patch touches a file also needed by non-farbling behavior, gating it off must not break the build. Farbling patches are self-contained (they only *add* perturbation to readback paths), so skipping them yields stock Chromium behavior — verify no farbling patch is a *prerequisite* for a non-farbling patch (it must not be).
- **Do NOT** gate C1/C2 separately from C3–C7 — a half-applied farbling set (Supplement present, readback patches absent, or vice-versa) is worse than all-or-nothing. Single gate.

---

## 6. Integration with build scripts + `CEF_BUILD_RUNBOOK.md`

### 6.1 Build-script edits (the only app-repo glue)
**⚠️ The snippet this section used to show was against the stale M136 invocation** (`--download-dir=C:\cef\chromium_git`, `--depot-tools-dir=C:\cef\depot_tools`, `--branch=7103`, and `automate-git.py` taken from `C:\cef\automate\`). The live script is already on the 7871 tree layout (D11) and takes `automate-git.py` **from the CEF checkout**, not from master. Only **one** flag is genuinely new; `--branch`/`--checkout` are already there:

```
set HODOS_FARBLING=1                                       REM NEW — condition gate (§5)

python C:\cef\cef150\cef\tools\automate\automate-git.py ^
  --download-dir=C:\cef\cef150 ^
  --depot-tools-dir=C:\cef\cef150\depot_tools ^
  --url=https://github.com/Hodos-Browser/cef.git ^         REM NEW — our fork
  --branch=7871 ^                                          REM already present
  --checkout=94c1726 ^                                     REM already present; retarget to the fork rev
  --x64-build --minimal-distrib --client-distrib ^
  --no-debug-build --no-depot-tools-update --force-build
```

Do **not** drop `--no-depot-tools-update` when adding `--url` — the live script documents why (it re-pulls depot_tools off its pinned commit and kills the build seconds in).

Plus a **pre-build audit gate** (§7): run `bash cef_patch_drift_audit.sh` and **abort on non-zero exit** before the expensive `automate-git.py` call. Note it must run *before* the call but its patch-apply check reads a **synced** tree, so on a cold host the first run has nothing to check — it is a re-build gate, not a first-build gate.

### 6.2 Runbook edits (fold this plan into the canonical P&P — Invariant #12)
Update `CEF_BUILD_RUNBOOK.md`:
- **Step 2.2 "Farbling patches (B1)"** — replace the current forward-reference with: "patches live in the `Hodos-Browser/cef` fork under `patch/patches/hodos_farble_*.patch`, registered in `patch.cfg`, gated by `HODOS_FARBLING`; applied automatically by **`gclient_hook.py` → `patcher.py`** during the build step of the `automate-git` flow. See `PLAN_patch_toolchain.md`." *(Do **not** write "`run_patch_updater`" here — that was this plan's own error, corrected in §1.3.)*
- **Step 5.5 "Patches — re-apply `cef/patch/`"** — point its "report failures/offsets" line at `DevOps-CICD/scripts/cef_patch_drift_audit.sh` (this plan **is** the "A1 patch toolchain owns this" owner named there). Correct the "fuzz" wording in that step: CEF applies with `git apply -p0 --ignore-whitespace` and **does not fuzz** (§1.2) — the only sub-failure signal is a hunk **offset**.
- **Open TODOs** — check off "B1: farbling patch set + `patch.cfg` integration" (toolchain half) and "Automate the Step 5.5 drift audit" (§7).
- **Resolve OQ-1:** check the two `build_hodos_cef*` scripts into `scripts/` (they are referenced as canonical but absent from the repo — §0).

### 6.3 CI reality (carry forward from runbook §Step 3 A1)
The full Chromium+CEF build **cannot** run on GitHub-hosted runners (6-hr cap, ~14 GB disk). The fork + patch toolchain runs on the **self-hosted build host / beefy VM**; only the *app* build (`cef-native` + wrapper, consuming the published `cef-binaries` release) runs in CI. The drift audit (§7) is cheap and **can** run in CI as a scheduled fork-watcher even though the build cannot.

---

## 7. Drift-audit script (`scripts/cef_patch_drift_audit.py`)

**Purpose:** the drift audit is a **fast-fail pre-flight** that surfaces patch trouble *before* committing to a 10–12 hr build. Two things to be precise about, because they shape what this script can and can't detect:

- **CEF's `git apply` is fail-loud, not fuzzy (see §1.2).** The classic GNU-`patch` failure mode — a hunk applies via fuzzy match and *silently lands in the wrong place* — **essentially cannot occur here**: if context doesn't match, `git apply --check` fails, the patcher's `failed` count is non-zero, and the build **aborts before compile**. This is a real upside of CEF's toolchain: **milestone-jump drift hard-fails; it does not silently misland.** The plan leans on this rather than defending against a fuzz mode CEF doesn't use.
- The one residual signal short of an outright failure is a hunk applied at a **line offset** (`git apply` reports offsets on stderr; the hunk still lands, just shifted). That is *not* "fuzz" and is a much weaker risk than a fuzzy mismatch, but it's the useful early-warning that the next milestone jump will likely break the patch.

So this script exists to (a) catch the fail-loud cases **cheaply and early** rather than 10 h into a build, and (b) scrape the offset lines as a soft warning — plus the file-manifest / GN-args checks below that a green compile genuinely does not prove. It is a **superset** of the runbook's manifest audit, focused on patches.

### 7.1 What it checks
1. **Patch apply health** — run **read-only** against a **throwaway synced tree**: either `patcher.py` (apply-only) or, per patch, `git apply --check -p0 --ignore-whitespace`. **⚠️ Never use `patch_updater.py --reapply`/`--restore` for this — it has no dry-run mode and is write-capable (it *resaves* the `.patch` files it's supposed to be validating; I1).** Signals to collect: **(a)** per-patch `git apply --check` pass/fail; **(b)** the patcher's `N patches total (A applied, S skipped, F failed)` line — **any `failed` → hard fail** (CEF's `git apply` is exact-context, so a `failed` means the patch will abort the real build too); **(c)** scrape `git apply`'s **stderr offset lines** (`Hunk #n succeeded at NNN (offset ±M lines)`) as the **soft early-warning** that the target moved under the patch and the next milestone jump will likely break it. There is **no fuzz metric** to parse — CEF does not fuzz (§1.2); do not key on one.
2. **Registry integrity** — every `'name'` in `patch.cfg`'s Hodos block has a matching `patch/patches/<name>.patch` file and vice-versa (no orphan files, no dangling registry entries). Cross-check against `HODOS_PATCHES.md`. Parse `patch.cfg` by **`exec`ing it** (as `patcher.py` does), never by grepping — the header comment overcounts (§1.1).
   > **⚠️ Upstream already ships one orphan.** `chrome_browser_privacy_1119417.patch` is on disk but **not registered** in `patch.cfg` on `94c1726`. It is upstream's, not ours. **The orphan check must scope to the Hodos block and carry this as an explicit baselined allowance** — otherwise the audit exits 1 on every run from day one, and a gate that always fails is a gate that gets ignored. Baseline manifest: `DevOps-CICD/scripts/cef_patch_baseline_7871.txt`.
3. **Target-file existence** — for each Hodos patch, confirm the file(s) it targets still exist at the expected path in the new Chromium `src` (catches upstream renames/deletes *before* apply, with a clearer message than a raw hunk-fail).
4. **Runtime file-manifest drift** — **⚠️ ALREADY BUILT. Do not reimplement; call it.** `DevOps-CICD/scripts/cef_dist_drift_audit.sh` covers this, and its header documents that *this plan's stated target was wrong*: there are no "hardcoded copy-lists in `cef-native/CMakeLists.txt`" — CMake does a wholesale `copy_directory` and can never drop a file. The real gate is the **installer's extension whitelist** (`installer/hodos-browser.iss`: `*.dll`/`*.bin`/`*.dat`/`*.pak`/`*.json` + `locales\*`); a CEF file with any other extension survives a from-source smoke test and is then **silently dropped at packaging**, which is precisely the class of change that breaks a silent auto-update.
5. **GN-args drift** — **⚠️ ALREADY BUILT. Do not reimplement; call it.** `DevOps-CICD/scripts/cef_gn_args_gate.sh` is already the mandatory pre-build codec gate (asserts `ffmpeg_branding=Chrome` + `proprietary_codecs=true` take effect; a flipped default ships a green build with no codecs).

> **Scope correction (reuse-first, 2026-08-05):** of the five checks above, only **1–3 (the patch-apply half) are net-new.** CEF-2 is therefore a *patch* audit that **invokes** the two existing scripts, not a new tool that duplicates them. It also lands as **bash alongside them** in `DevOps-CICD/scripts/`, not as a lone `.py` at a repo-root `scripts/` — matching its neighbours (`cef_dist_drift_audit.sh`, `cef_gn_args_gate.sh`, `chromium-build-gitconfig`) and the OQ-1 (c) resolution.

### 7.2 Output
A single human-review report (stdout + a file artifact): per-patch apply status + any hunk **offset** lines, registry/orphan findings, target-file-missing list, manifest add/remove/rename diff, GN-args diff. **Manifest + args + apply diffs are scriptable; cmake/copy-list *edits* need human judgment — the script REPORTS, never auto-edits** (runbook Step 5.5).

### 7.3 Exit codes
`0` clean; `2` warnings (hunk **offset** lines present, GN-args or manifest diff present) — build may proceed with sign-off; `1` hard fail (any patch `failed` / `git apply --check` fails, any target file missing, orphan registry entry) — **build must not start.** The build scripts gate on this (§6.1).

### 7.4 Scheduled fork-watcher (security-pull automation, Q5 row CEF-3)
A cheap CI job (runs where the app build runs, **not** the Chromium build) on a cron: fetch upstream `<branch>`, and if it has advanced beyond our fork's `hodos/<branch>` base, **open a PR** that rebases our patch commits onto the new upstream HEAD and runs §7.1 apply-health read-only, posting the offset/fail report as the PR body. This operationalizes the "pull in-branch security point-releases" duty so it doesn't rot between milestone jumps. *(This is the drift-audit script + `gh` + a schedule — no new machinery.)*

---

## 8. How the farbling patch set plugs in (first consumer — BLOCKS on this toolchain)

FEAT-B1 (C1–C7, Q5 §A.3) is the **first and, for beta.1, only** consumer. This plan **blocks** it; the farbling *values/design* are settled in `PLAN_farbling_blink.md` (unwritten) — here we only define **how the patches attach.**

### 8.1 Attachment map (each C-row → one patch file → one cfg entry)
| Farbling row | Patch file (`patch/patches/`) | Targets `path` | `condition` |
|---|---|---|---|
| C1 HodosSessionCache Supplement | `hodos_farble_session_cache.patch` | `src` (default) | `HODOS_FARBLING` |
| C2 seed wiring (off-cmdline) | `hodos_farble_seed_wiring.patch` | `src` | `HODOS_FARBLING` |
| C3 Canvas 2D | `hodos_farble_canvas2d.patch` | `src` | `HODOS_FARBLING` |
| C4 WebGL (incl. readPixels) | `hodos_farble_webgl.patch` | `src` | `HODOS_FARBLING` |
| C5 WebAudio | `hodos_farble_webaudio.patch` | `src` | `HODOS_FARBLING` |
| C6 Navigator | `hodos_farble_navigator.patch` | `src` | `HODOS_FARBLING` |
| C7 auth-domain exemption | `hodos_farble_auth_exempt.patch` | `src` | `HODOS_FARBLING` |

- **Order in `patch.cfg`:** C1 first (Supplement is read by all others), then C2–C7. All target `src` (the Blink renderer lives in the Chromium tree, not a sub-repo), so no `'path'` override.
- **Flagged design conflict (out of scope here, do not lose it):** the seed docs note C4 (WebGL `UNMASKED_VENDOR`/`UNMASKED_RENDERER`) and C6 (navigator `hardwareConcurrency`/`deviceMemory`) would **re-add values the current JS farbling deliberately removed as detectable**. This toolchain only defines *how* those patches attach; whether/how those specific values are perturbed is a **farbling-design decision owned by `PLAN_farbling_blink.md`** — resolve the conflict there, not here.
- **Incremental landing (outline P4a→P4e):** land C1+C2 as the first two entries (the "worker-coverage quick win"), verify apply-health, then add C3.. one patch at a time. Because each is a separate file + cfg entry, **partial land is trivial** — add the next patch, re-run the drift audit, rebuild.
- **New Blink files** (C1 adds a *new* supplement source file, not just edits): the patch **creates** the file (unified-diff against `/dev/null`) and must also patch the Blink `BUILD.gn` to compile it — flag this in `HODOS_PATCHES.md` (it's the one place a farbling patch touches a build file, and thus a higher-churn rebase target).

### 8.2 Authoring workflow for each farbling patch (clean-room — outline M7)
1. On the build host, `gclient sync` a clean TARGET checkout **via our fork** (patches from earlier rows already applied by `run_patch_updater`).
2. Hand-edit the Chromium/Blink source per the (clean-room, spec-derived) design in `PLAN_farbling_blink.md` — **read behavior/spec, not Brave's MPL-2.0 source** (M7 clean-room boundary).
3. Regenerate the `.patch` from the dirty tree using CEF's `patch_updater.py` resave/add path (exact flag names to confirm — **OQ-2**), or hand-produce the unified diff. **Path-format requirement (load-bearing):** CEF applies with `git apply -p0` (§1.2), so patch paths must carry **zero strippable prefix** — i.e. paths are rooted at the tree `path` (e.g. `third_party/blink/renderer/...` relative to `src`) with **no `a/`…`b/` prefixes**. A normal `git diff` (authored for `-p1`, carries `a/`/`b/`) will report `failed` under `-p0`; match upstream CEF `.patch` path formatting exactly. `patch_updater.py` emits this format automatically; a **hand-authored** diff (the OQ-2 fallback) must be formatted for `-p0` by hand. Note `--ignore-whitespace` is also in effect, so pure-whitespace diffs are tolerated.
4. Register in `patch.cfg` (§4.3) with `'condition': 'HODOS_FARBLING'` + `'note'`.
5. Run the drift audit (§7) — expect `applied +1, 0 failed`, no hunk offsets.
6. Update `HODOS_PATCHES.md`.

### 8.3 What the toolchain does NOT own
- The **shell-side** seed generation/storage (`ProfileManager`/`SettingsManager`, off-cmdline delivery channel — Q5 C2) is **app-repo C++**, not a CEF patch. The toolchain delivers the *renderer-side* Blink code; the browser-process seed plumbing is ordinary `cef-native` work.
- The **JS-farbling teardown** (Q5 TD-1..TD-5) is app-repo `cef-native` deletions, sequenced *after* the native patches are live — not part of standing up the toolchain.

---

## 9. Acceptance criteria (toolchain standup complete)

- [ ] `Hodos-Browser/cef` fork exists; `hodos/7871` created off upstream `7871`; URL reachable by the build host; upstream remote recorded.
- [ ] Both build scripts pass `--url` at the fork; `git remote -v` in the CEF checkout shows the fork; **`patcher.py` (via `gclient_hook.py`)** reports the **114-entry upstream baseline, 0 failed** — no Hodos patches yet.
- [ ] No-op probe patch demonstrably **applies pre-compile** with **`0 failed`** (on the already-patched host tree that reads `115 total (1 applied, 114 skipped, 0 failed)` — *not* "applied +1"); build completes; probe removed and count returns to the 114 baseline.
- [ ] `condition: HODOS_FARBLING` demonstrably toggles the probe, **proven from the per-patch stdout line** (`Skipping patch file …` vs `… successfully applied.`) rather than the ambiguous `skipped` count (§1.1), never **failed**.
- [ ] `DevOps-CICD/scripts/cef_patch_drift_audit.sh` runs, establishes baseline, emits a human-readable report, **invokes rather than duplicates** `cef_dist_drift_audit.sh` + `cef_gn_args_gate.sh`, baselines the known upstream orphan, and is wired as a **pre-build gate** (exit 1 aborts) + a scheduled fork-watcher.
- [ ] `HODOS_PATCHES.md` (fork) + `CEF_VERSION_UPDATE_TRACKER.md` (app repo) record the standup; `CEF_BUILD_RUNBOOK.md` Step 2.2 / 5.5 / Open-TODOs updated (§6.2).
- [ ] OQ-1 resolved **as (c)**: `DevOps-CICD/scripts/` confirmed canonical; the 35 repo-root `scripts/build_hodos_cef*` citations across 12 docs corrected.
- [ ] R9 discharged: distrib tarballs moved outside the `delete_directory(cef_src_dir)` path before any `--checkout` retarget.
- [ ] **Ready-for-consumer gate:** a single real farbling patch (C1 alone) can be authored, registered, applied, and built end-to-end — proving the pipeline is ready for FEAT-B1 P4a.

---

## 10. Risks

| # | Risk | Mitigation |
|---|---|---|
| ~~R1~~ | ~~**URL-switch hard error** blocks the fork switch on a host that built stock CEF.~~ **DOWNGRADED 2026-08-05 — not a real risk.** The check just reads `git config --get remote.origin.url`, so `git remote set-url origin <fork>` satisfies it with no re-clone (same object graph, pin still resolves). Also bypassable via `--no-cef-update`, and the checkout is only 65 MB regardless. | One command, §1.3 / §4 Step 2. |
| **R9** | **`delete_directory(cef_src_dir)` destroys the previous build's distrib output** — `automate-git.py:1535-1539` deletes `chromium/src/cef` on any CEF checkout change, and `binary_distrib/` lives inside it. That includes the **898 MB `release_symbols` tarball**, which cannot be regenerated without a full ~5 h rebuild. **This was the real hazard R1 was mistaken for**, and it was undocumented. | Move `binary_distrib/*.tar.bz2` outside the delete path **before** changing `--checkout` (done, P3 commit 1 → `C:\cef\cef150\binary_distrib_94c1726\`). Standing rule for every future fork-revision bump. |
| **R10** | **`patcher.py` run from the wrong directory silently loses the fail-loud guarantee.** From the standalone `C:\cef\cef150\cef`, `src_dir` resolves to a non-git dir, and `git_util.py` falls back to GNU `patch --force`, which **fuzzes and can misland** — the exact failure mode §7 says "essentially cannot occur here". | Always invoke from `chromium/src/cef`. Called out at §4 Step 3; the drift audit asserts its own CWD. |
| R2 | **Hunk offset (not fuzz)** — CEF's `git apply` is exact-context and fail-loud, so a context mismatch **hard-fails the build before compile** (not a silent misland). The residual risk is a hunk landing at a line **offset** — it still applies, but signals the patch is drifting toward a future break. | Rely on the fail-loud model for outright mismatches (build aborts, §1.2); drift audit (§7.1) scrapes `git apply` offset lines as the exit-2 early-warning; farbling acceptance tests (worker==window) catch any behavioral misland downstream. |
| R3 | **High-churn Blink files** — `base_rendering_context_2d.cc` etc. conflict on most milestone jumps → rebase labor. | Keep patches minimal + disjoint; budget ~2–8 h/bump; record actuals; the scheduled fork-watcher (§7.4) catches drift early, not at build time. |
| R4 | **BUILD.gn coupling** — C1's new-file patch also edits a Blink `BUILD.gn`; build files churn and rename. | Flag in `HODOS_PATCHES.md`; treat the `BUILD.gn` hunk as the canary in each rebase; verify the new source actually compiles into `libcef`. |
| R5 | **Security-coverage rot** — fork stops tracking upstream in-branch security commits between jumps (M6). | §7.4 scheduled fork-watcher auto-PRs upstream advances; standing duty recorded in `HODOS_PATCHES.md` + tracker. |
| R6 | **Clean-room contamination** — transcribing Brave's MPL-2.0 source while authoring a patch = derivative-work risk (M7). | Author from spec/behavior, not Brave source; the toolchain doesn't create the patches, but §8.2 enforces the boundary in the authoring step. |
| R7 | **`patch_updater.py` flag drift** — resave/add flags differ across CEF branches (OQ-2). | Confirm against the TARGET-branch tool before authoring; hand-crafted unified diffs are a fallback. |
| R8 | **Mac parity** — patches are shared text, but the Mac build is a full parallel effort; a patch that assumes a Win-only path breaks the framework build. | Patches target cross-platform Blink files; Mac Claude re-runs the drift audit + build on the framework (outline §5); GPU-string patch (C4) is the one intentional per-OS split. |

---

## 11. Open questions (with recommended defaults)

> **OQ-1 through OQ-4 and OQ-8 are all CLOSED as of 2026-08-05** (owner decisions + P3 kickoff verification). Retained with their resolutions for the record.

| # | Question | **RESOLUTION** |
|---|---|---|
| **OQ-1** | The runbook's "canonical" `build_hodos_cef.bat`/`_mac.sh` are not in the repo (§0). Where do they live? | ✅ **CLOSED — they were never missing.** They are at `development-docs/DevOps-CICD/scripts/`, and the runbook already declares that path canonical (line 9). **Owner chose (c): leave them there.** CEF-5 reduces from "check in two scripts" to "fix the 35 repo-root citations across 12 docs" (done, P3 commit 2). New patch tooling lands beside them, not at a repo root. |
| **OQ-2** | Exact `patch_updater.py` flags to author/regenerate a `.patch`, and the `run_patch_updater` arg string on the build path. | ✅ **CLOSED by direct inspection of `94c1726`.** Flags: `--resave --reapply --revert --backup --restore --patch --add`; authoring path `--resave --patch <name> --add <path>` (`cef/docs/chromium_update.md:136`). The second half of the question **dissolves**: `run_patch_updater` is not on the apply path at all (§1.3). |
| **OQ-3** | Fork name/owner. | ✅ **CLOSED — `Hodos-Browser/cef`** (owner-approved). Name is free in the org; matches GitHub's default fork name and upstream tooling expectations. Integration branch `hodos/7871`. Operator is **org admin**, so `gh repo fork --org` works directly. |
| **OQ-4** | Upstream rebase remote — GitHub vs legacy Bitbucket. | ✅ **CLOSED — GitHub**, and now settled empirically rather than by inference: the checkout that produced the green 150 build already has `origin = https://github.com/chromiumembedded/cef.git`. Bitbucket was never involved. |
| **OQ-8** | **NEW.** Fork visibility. A GitHub fork **cannot be private** — it inherits the upstream network — so the C1–C7 farbling patches become public as authored. | ✅ **CLOSED — public fork accepted** (owner). Correct rationale: farbling is **per-domain seeded**, so its effectiveness does not depend on the patch being secret (Brave ships theirs openly). CEF is BSD, so licensing is not a constraint either way. **⚠️ The rationale offered at decision time — "we're just taking Brave's public code" — is NOT the basis, and must not become one:** Brave is **MPL-2.0**, and transcribing it makes our patches a derivative work carrying MPL obligations. The **M7 clean-room boundary stands unchanged** (§8.2 step 2, R6): author from spec/behavior/CreepJS expectations, never from Brave source. Recorded per-patch in `HODOS_PATCHES.md`. |
| **OQ-5** | One `HODOS_FARBLING` gate vs per-patch conditions. | **Single gate** for the whole set (§5) — a half-applied farbling set is worse than all-or-nothing; no per-patch conditions. |
| **OQ-6** | Should the drift audit run in CI (not just the build host)? | **Yes** — it's cheap and needs no Chromium build; run it as the scheduled fork-watcher (§7.4) even though the full build can't run in CI. |
| **OQ-7** | Do we `condition`-gate the no-op probe permanently or remove it? | **Remove after Step 3/4** — the probe is standup scaffolding; leaving it in ships a pointless hunk and inflates the patch count the drift audit baselines against. |

---

## 12. What this feeds

- **`Q5_full_edit_list.md`** — hardens rows **CEF-1** (toolchain standup), **CEF-2** (drift-audit hook), **CEF-3** (security-pull duty), **CEF-4** (`condition` gate) from "GREENFIELD stub" to concrete, followable steps; confirms the C1–C7 attachment map (§8.1) and the CEF↔farbling serial-linchpin dependency.
- **`PLAN_farbling_blink.md`** (unwritten) — this doc defines the **slots** (patch files + cfg entries + authoring workflow) that plan fills with actual Blink patch content; §8 is the contract between them.
- **`CEF_BUILD_RUNBOOK.md`** — §6.2 edits fold this into the canonical build P&P (Step 2.2 / 5.5 / Open-TODOs).
- **`IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`** — P3 (patch toolchain) is fully specified here; P4 (farbling) is unblocked once §9 acceptance is green.

---

*Sources (mechanism, primary): chromiumembedded/cef `patch/patch.cfg`, `tools/patcher.py`, `tools/git_util.py`, `tools/patch_updater.py`, `tools/automate/automate-git.py` (github.com/chromiumembedded/cef, master); CEF wiki `branches_and_building.html`; CEF forum "Persist Chromium Patch Using automate-git.py" (magpcss.org/ceforum). In-repo: `CEF_BUILD_RUNBOOK.md`, `B1-farbling-design.md`, `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md`, `Q5_full_edit_list.md`.*
