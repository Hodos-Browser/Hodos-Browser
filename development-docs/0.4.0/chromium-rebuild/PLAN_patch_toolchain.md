# PLAN — CEF Patch Toolchain Standup (PIPE-A1, GREENFIELD)

**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**Status:** DETAILED PLAN — Workflow-2 expansion of `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3b (PIPE-A1) + §4 P3. Research + design only — **NO code, NO builds.**
**One-line purpose:** Stand up, from zero, the CEF source-patch pipeline (fork `chromiumembedded/cef` → `patch/patches/*.patch` → register in `patch/patch.cfg` → `automate-git.py --url=<fork>` → `patcher.py` applies pre-compile), so the Blink farbling patch set (FEAT-B1 / C1–C7) has a place to land. **This is the serial linchpin that blocks all source-level farbling.**

> **Authoritative inputs:** outline §3b / §3f / §4 P3 / §5 / §8; `DevOps-CICD/CEF_BUILD_RUNBOOK.md` (Step 2, Step 5.5, Step 3 build flow); `0.4.0/B1-farbling-design.md` ("Build integration (CEF patch.cfg)"); `chromium-rebuild/Q5_full_edit_list.md` (rows CEF-1..CEF-4, CEF↔farbling deps); `CEF_VERSION_UPDATE_TRACKER.md`; `DEPENDENCY_VERIFICATION.md`.
> **Primary sources (mechanism):** CEF `tools/patcher.py`, `tools/git_util.py` (the actual `git apply` invocation), `patch/patch.cfg`, `tools/automate/automate-git.py`, `tools/patch_updater.py` (all `github.com/chromiumembedded/cef`); the CEF wiki `branches_and_building.html`; CEF forum threads on persisting Chromium patches via `automate-git.py`. Cited inline.

> **TARGET = placeholder.** Exact CEF stable version + branch (e.g. the runbook's candidate CEF 149 / Chromium 149 / branch `7827`, or an LTS pin) resolves from `cef-builds.spotifycdn.com/index.json` in the version-target plan (outline §2 Step 0). The toolchain design here is **branch-agnostic** — it works identically on 136 or TARGET.

---

## 0. Verified starting state (greenfield — confirmed 2026-07-10)

| Check | Result |
|---|---|
| `cef/patch/` directory in repo | **absent** |
| `patch.cfg` anywhere in repo | **absent** |
| `patcher.py` / `patch_updater.py` in repo | **absent** |
| `automate-git.py` in repo | **absent** (fetched ad-hoc into `C:\cef\automate\` per runbook §3) |
| `scripts/build_hodos_cef.bat` / `_mac.sh` in repo | **absent** — the runbook names them "canonical" but they are **not checked in** (only DevOps release scripts live in `scripts/`). **See OQ-1.** |

**Consequence:** there is nothing to extend — we are building the patch pipeline, its hosting, and its maintenance model from zero. Everything below is net-new (`Q5` rows **CEF-1..CEF-4**, all status GREENFIELD/NEW).

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
  # ... ~150 upstream CEF patches ...
]
```
There is **no `sha1`/checksum/version field** — integrity is by filename + the patch's own context lines. Note CEF applies with `git apply` (§1.2), which is **exact-context, not fuzzy** — there is effectively no fuzz tolerance; a context mismatch fails loud rather than shifting. *(Source: chromiumembedded/cef `patch/patch.cfg`.)*

### 1.2 `tools/patcher.py` / `tools/patch_updater.py` — the applier
- Reads `patch.cfg`, iterates `patches`, and for each entry applies `patch/patches/<name>.patch` to the tree at `path` (default `src`).
- **How the patch is applied (matters for drift):** `patcher.py` calls `tools/git_util.py::git_apply_patch_file`, which runs `git apply --check` then `git apply -p0 --ignore-whitespace`. This means: **(a)** patch paths are consumed at **`-p0`** — zero leading path components stripped (unlike a normal `git diff` which is authored for `-p1`); **(b)** whitespace differences are ignored; **(c)** there is **NO fuzz / `-C` context reduction / `--3way` / `--recount` / retry** — application is exact-context and **fail-loud**. (`git_util` only falls back to system `patch` if the target dir is not a git repo, which never applies to the Chromium `src` tree.)
- **Condition gate:** `if patch['condition'] not in os.environ: dopatch = False` → the patch is **skipped** (not failed) when its env var is unset.
- **Reporting:** prints `'%d patches total (%d applied, %d skipped, %d failed)'` — **no fuzz metric is emitted** (there is no fuzz to report). **On any failure it exits status 1 and prints revert instructions** — a failed patch **aborts the build before compile** (this is what makes drift loud, see §7).
- Single-file mode: `--patch-file FILE --patch-dir DIR` applies one patch outside the cfg loop (useful for testing a candidate patch in isolation).
- Authoring/regeneration: `patch_updater.py` both **re-applies** all registered patches to a fresh checkout and, in its resave mode, **regenerates the `.patch` files from a modified `src` tree** — this is the supported way to author/update a patch after hand-editing Chromium source (§8.2). **⚠️ `patch_updater.py` has no dry-run mode** and is **write-capable** — with `--restore` it *resaves* `.patch` files when the backed-up source changed, so it must never be pointed at the canonical fork as a "read-only check" (use `patcher.py` / `git apply --check` for that — I1, §7.1). Exact resave/add flag names, and the exact arg string `automate-git` passes on the normal build path, to confirm against the target-branch `patch_updater.py` — **OQ-2**.

*(Sources: chromiumembedded/cef `tools/patcher.py`, `tools/patch_updater.py`.)*

### 1.3 `tools/automate/automate-git.py` — where patches enter the build
- `--url=<git url>` points the checkout at **our CEF fork** instead of the upstream default, which is now **`https://github.com/chromiumembedded/cef.git`** (the current `automate-git.py` default `cef_url`; Bitbucket is the legacy remote — see §2.1 / OQ-4). The script **validates the URL against any existing checkout** (`Requested CEF checkout URL … does not match existing URL` → hard error), so a fork switch requires a clean CEF dir.
- `--branch=<n>` selects the CEF branch (and the per-branch sub-checkout name); the download root is set separately by `--download-dir`. `--checkout=<rev>` optionally pins an exact CEF revision (default = `origin/<branch>`).
- **Apply point (unchanged by us):** after Chromium `gclient sync`, before compile — `apply_deps_patch()` → `gclient runhooks` → `apply_runhooks_patch()` → **`run_patch_updater(...)`** (this is the step that applies **our** `patch/patches/*.patch` via `patch.cfg`) → build. So our patches are guaranteed to land on the Chromium source **pre-compile**, every build, with no extra wiring beyond populating `patch.cfg`. *(Exact `run_patch_updater` arg string on the build path — the primary source shows `run_patch_updater(output_file=...)` and a `--resave` branch, not clearly `--reapply --restore` — confirm against the target-branch tool: OQ-2.)*

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
scripts/
├─ build_hodos_cef.bat                 # CHECK IN (OQ-1) — add --url=<fork> --branch=TARGET --checkout=<pin>
├─ build_hodos_cef_mac.sh              # CHECK IN (OQ-1) — same, Mac
└─ cef_patch_drift_audit.py            # NEW — §7 drift-audit script (cross-platform Python)
development-docs/0.4.0/chromium-rebuild/
└─ PLAN_patch_toolchain.md             # this doc
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
2. **Clean-dir caveat:** `automate-git.py` refuses to switch URLs on an existing checkout. On a host that previously built stock CEF, the CEF sub-dir must be removed first (do **not** blow away `chromium_git/` — only the CEF checkout dir). Document the exact path in the runbook.
3. **Acceptance:** a build (or a `--no-build` dry checkout, if available) fetches CEF from **our fork** — verify `git -C <cef dir> remote -v` shows the fork URL and `run_patch_updater` reports **`N patches total (N applied, 0 skipped, 0 failed)`** with the stock upstream count (proves our fork's patch pipeline is intact and we've added nothing yet).

### Step 3 — Prove the pipeline with a no-op patch
1. Author a trivial, harmless patch (e.g. add a comment line to a stable, low-churn Chromium file, or a `.md`/`OWNERS` no-op) via the authoring workflow (§8.2).
2. Save as `patch/patches/hodos_noop_probe.patch`; append `{ 'name': 'hodos_noop_probe', 'note': 'PIPE-A1 pipeline smoke — remove after standup' }` to `patch.cfg`.
3. Verify apply on an **already-synced throwaway tree** via the **apply-only path** — `patcher.py` (or `git apply --check -p0 --ignore-whitespace` per patch). **Do not use `patch_updater.py --reapply`/`--restore`** (write-capable, resaves `.patch` files — I1). This is a **seconds-long** check, not a build.
4. **Acceptance:** patcher reports **applied count +1**, `0 failed`; the change is present in the Chromium `src` tree pre-compile. Confirming apply-health does **not** require a full build; a single end-to-end build is the final gate (§9), **not** one per probe/toggle iteration. **Then remove the probe** (patch file + cfg entry) and re-verify count returns to baseline.

### Step 4 — Wire the `condition` env gate (optional-but-recommended, Q5 row CEF-4)
1. Decide the gate variable name (recommend `HODOS_FARBLING` — §5).
2. Prove it via the **apply-only path** (`patcher.py` on a synced tree — no build needed): register the no-op probe with `'condition': 'HODOS_FARBLING'`; run once **without** the env var (expect **skipped +1**), once **with** it (expect **applied +1**).
3. **Acceptance:** condition toggles the patch between *applied* and *skipped* with no *failed*, exactly per `patcher.py`'s `condition not in os.environ` logic (no full build required to prove this).

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
In `build_hodos_cef.bat` / `_mac.sh`, the `automate-git.py` invocation gains three flags (runbook §Step 3 shows the current invocation without them):
```
python C:\cef\automate\automate-git.py ^
  --download-dir=C:\cef\chromium_git --depot-tools-dir=C:\cef\depot_tools ^
  --url=https://github.com/Hodos-Browser/cef.git ^      REM NEW — our fork
  --branch=<TARGET> ^                                    REM was 7103
  --checkout=<fork-rev-or-hodos/branch> ^                REM NEW — pin exact fork revision
  --x64-build --minimal-distrib --client-distrib --no-debug-build --force-build
```
And an env line for the gate:
```
set HODOS_FARBLING=1                                     REM condition gate (§5)
```
Plus a **pre-build audit gate** (§7): run `python scripts\cef_patch_drift_audit.py` and **abort the build on non-zero exit** before the expensive `automate-git.py` call.

### 6.2 Runbook edits (fold this plan into the canonical P&P — Invariant #12)
Update `CEF_BUILD_RUNBOOK.md`:
- **Step 2.2 "Farbling patches (B1)"** — replace the current forward-reference with: "patches live in the `Hodos-Browser/cef` fork under `patch/patches/hodos_farble_*.patch`, registered in `patch.cfg`, gated by `HODOS_FARBLING`; applied automatically by `run_patch_updater` in the `automate-git` flow. See `PLAN_patch_toolchain.md`."
- **Step 5.5 "Patches — re-apply `cef/patch/`"** — point its "report failures/offsets" line at `scripts/cef_patch_drift_audit.py` (this plan **is** the "A1 patch toolchain owns this" owner named there). Correct any "fuzz" wording in that step: CEF applies with `git apply` and does not fuzz (§1.2).
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
2. **Registry integrity** — every `'name'` in `patch.cfg`'s Hodos block has a matching `patch/patches/<name>.patch` file and vice-versa (no orphan files, no dangling registry entries). Cross-check against `HODOS_PATCHES.md`.
3. **Target-file existence** — for each Hodos patch, confirm the file(s) it targets still exist at the expected path in the new Chromium `src` (catches upstream renames/deletes *before* apply, with a clearer message than a raw hunk-fail).
4. **Runtime file-manifest drift (folds in runbook Step 5.5)** — diff the new CEF dist's DLL/`.bin`/`.pak`/`resources`/`locales` list against the hardcoded copy-lists in `cef-native/CMakeLists.txt` (Win) + the mac framework-embed list. A new/renamed/removed runtime file we don't copy = green build, runtime crash or missing feature — **and is exactly what breaks a silent auto-update** (feeds the outline §7 auto-update apply gate).
5. **GN-args drift** — diff our pinned `GN_DEFINES` against the target CEF's generated `args.gn` defaults; assert `ffmpeg_branding=Chrome` + `proprietary_codecs=true` still take effect (a flipped default ships a green build with no codecs — runbook Step 5.5).

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

- [ ] `Hodos-Browser/cef` fork exists; `hodos/<TARGET-branch>` created off upstream; URL reachable by the build host; upstream remote recorded.
- [ ] Both build scripts pass `--url`/`--branch`/`--checkout` at the fork; `git remote -v` in the CEF checkout shows the fork; `run_patch_updater` reports the **stock upstream patch count, 0 failed** (baseline, no Hodos patches yet).
- [ ] No-op probe patch demonstrably **applies pre-compile** (`applied +1, 0 failed`), build completes, probe removed and count returns to baseline.
- [ ] `condition: HODOS_FARBLING` demonstrably toggles a patch between **applied** (env set) and **skipped** (env unset), never **failed**.
- [ ] `scripts/cef_patch_drift_audit.py` runs, establishes baseline, emits a human-readable report, and is wired as a **pre-build gate** (exit 1 aborts) + a scheduled fork-watcher.
- [ ] `HODOS_PATCHES.md` (fork) + `CEF_VERSION_UPDATE_TRACKER.md` (app repo) record the standup; `CEF_BUILD_RUNBOOK.md` Step 2.2 / 5.5 / Open-TODOs updated (§6.2).
- [ ] OQ-1 resolved: `build_hodos_cef.bat` / `_mac.sh` checked into `scripts/`.
- [ ] **Ready-for-consumer gate:** a single real farbling patch (C1 alone) can be authored, registered, applied, and built end-to-end — proving the pipeline is ready for FEAT-B1 P4a.

---

## 10. Risks

| # | Risk | Mitigation |
|---|---|---|
| R1 | **URL-switch hard error** — `automate-git.py` refuses to change checkout URL on an existing CEF dir; a host that built stock CEF blocks the fork switch. | Document the exact CEF-checkout dir to remove (not `chromium_git/`); do it once at standup; capture in the runbook. |
| R2 | **Hunk offset (not fuzz)** — CEF's `git apply` is exact-context and fail-loud, so a context mismatch **hard-fails the build before compile** (not a silent misland). The residual risk is a hunk landing at a line **offset** — it still applies, but signals the patch is drifting toward a future break. | Rely on the fail-loud model for outright mismatches (build aborts, §1.2); drift audit (§7.1) scrapes `git apply` offset lines as the exit-2 early-warning; farbling acceptance tests (worker==window) catch any behavioral misland downstream. |
| R3 | **High-churn Blink files** — `base_rendering_context_2d.cc` etc. conflict on most milestone jumps → rebase labor. | Keep patches minimal + disjoint; budget ~2–8 h/bump; record actuals; the scheduled fork-watcher (§7.4) catches drift early, not at build time. |
| R4 | **BUILD.gn coupling** — C1's new-file patch also edits a Blink `BUILD.gn`; build files churn and rename. | Flag in `HODOS_PATCHES.md`; treat the `BUILD.gn` hunk as the canary in each rebase; verify the new source actually compiles into `libcef`. |
| R5 | **Security-coverage rot** — fork stops tracking upstream in-branch security commits between jumps (M6). | §7.4 scheduled fork-watcher auto-PRs upstream advances; standing duty recorded in `HODOS_PATCHES.md` + tracker. |
| R6 | **Clean-room contamination** — transcribing Brave's MPL-2.0 source while authoring a patch = derivative-work risk (M7). | Author from spec/behavior, not Brave source; the toolchain doesn't create the patches, but §8.2 enforces the boundary in the authoring step. |
| R7 | **`patch_updater.py` flag drift** — resave/add flags differ across CEF branches (OQ-2). | Confirm against the TARGET-branch tool before authoring; hand-crafted unified diffs are a fallback. |
| R8 | **Mac parity** — patches are shared text, but the Mac build is a full parallel effort; a patch that assumes a Win-only path breaks the framework build. | Patches target cross-platform Blink files; Mac Claude re-runs the drift audit + build on the framework (outline §5); GPU-string patch (C4) is the one intentional per-OS split. |

---

## 11. Open questions (with recommended defaults)

| # | Question | Recommended default |
|---|---|---|
| **OQ-1** | The runbook's "canonical" `build_hodos_cef.bat`/`_mac.sh` are **not in the repo** (§0). Where do they live? | **Check them into `scripts/`** as part of this standup so `--url` wiring is version-controlled and reviewable. Treat their current absence as a gap to close. |
| **OQ-2** | Exact `patch_updater.py` flags to **author/regenerate** a `.patch` on the TARGET branch (resave/add), and the exact `run_patch_updater` arg string on `automate-git`'s build path. | Confirm against the TARGET-branch tool at author time; **fallback = hand-crafted unified diff**, rooted at the tree `path` and **formatted for `git apply -p0`** (no `a/`…`b/` prefixes — §8.2 step 3), else it reports `failed`. Non-blocking for standup (the no-op probe can be hand-diffed). |
| **OQ-3** | Fork name/owner. | **`Hodos-Browser/cef`** (org that already holds signing trust for `cef-binaries`); default branch `hodos/<CEF-branch>`. |
| **OQ-4** | Upstream rebase remote — GitHub (authoritative) vs legacy Bitbucket. | **GitHub** (`github.com/chromiumembedded/cef`) — it is `automate-git.py`'s default clone source and the post-2023 canonical home; Bitbucket is legacy and likelier to lag. Record the choice in `HODOS_PATCHES.md`. |
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
