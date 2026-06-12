# WSL Hybrid Workspace — Strategy + Runbook

**Created:** 2026-06-08
**Status:** 📋 Planned — execute when not mid-sprint
**Trigger:** Edwin's recall pipeline can't read Windows-side content efficiently — measured 200× slowdown via WSL2's 9P bridge (1m43s for a search against the 500-file Hodos-Browser repo vs 0.53s for the same search against an ext4-native dir).
**Owner:** DevOps/CI-CD · **Covers:** dev environment architecture, repo location strategy, sync automation
**Companion docs:** `../Dolphin Milk + Edwin Integration/INTEGRATION_PLAN_v1.md` (Edwin angle), `../Dolphin Milk + Edwin Integration/EDWIN_SETUP_FEEDBACK_FOR_JAKE.md` (the measurement the strategy is responding to)

---

## TL;DR

**Hybrid workspace:** most repos live canonically in WSL ext4 (`~/repos/`). Hodos-Browser stays canonical on Windows because its build chain requires native Windows tooling. A WSL-side read-only mirror of Hodos-Browser keeps Edwin queries fast.

Sync is **GitHub-mediated**, not rsync. Hard rule: uncommitted state never crosses the boundary, eliminating the lost-work risk.

```
Canonical (where work happens)        Mirror (read-only for Edwin / cross-platform tests)
─────────────────────────────         ──────────────────────────────────────────────────
C:\Users\archb\Hodos-Browser\         ~/repos/Hodos-Browser/    (clone, never edited, periodic git pull)

~/repos/Marston-Enterprises/         ─ canonical in WSL ─        \\wsl.localhost\Ubuntu\... (UNC access from Windows)
~/repos/hodos-brand/                 ─ canonical in WSL ─
~/repos/Edwin-Integration-Notes/     ─ canonical in WSL ─
                                                                  Edwin reads all of these natively from ext4

C:\Users\archb\HodosBrowser*.exe     ─ production install ─       (untouched by the workspace move)
%APPDATA%\HodosBrowser\              ─ production user data ─
%APPDATA%\HodosBrowserDev\           ─ dev user data ─            (already separated per CLAUDE.md invariant 9)
```

---

## Why this shape

### 1. Hodos-Browser must stay canonical on Windows

The build chain isn't portable:
- **C++ CEF shell** uses Win32 APIs (HWND, DPAPI, native overlays) on Windows side, NSPanel/Cocoa on Mac side via `cef_browser_shell_mac.mm`. Neither compiles cleanly on Linux.
- **Rust wallet** has DPAPI bindings under `#[cfg(target_os = "windows")]` that only work on Windows.
- **MSI installer + Windows code signing (Azure Trusted Signing)** can only happen on Windows hosts.
- **CEF Tier 1 self-build** uses VS 2022 BuildTools + Windows toolchain.

Moving Hodos-Browser to WSL would force every build through UNC paths (`\\wsl.localhost\...`) — same 9P bridge in reverse, equally slow. The dev cycle (`win_build_run.sh`) would crawl.

**Decision:** Hodos-Browser repo lives on Windows. Period.

### 2. Everything else benefits from being in WSL ext4

For non-Hodos-Browser content (business docs, brand assets, BSV protocol specs, Edwin integration notes, marketing material):
- Edwin reads at ~500 ms instead of ~100 sec
- Git operations are 5-10× faster on Linux ext4 vs Windows NTFS or 9P
- File watching (inotify) works correctly
- No Windows Defender randomly scanning node_modules
- VS Code's WSL Remote extension gives a native-feeling edit experience from Windows

**Decision:** Default everything-else to WSL canonical.

### 3. Hodos-Browser mirror in WSL — read-only, for Edwin

We still need Edwin to read Hodos's docs (BRC-100 plans, audit briefs, integration plans, etc.) at chat speed. A periodic `git pull` into a WSL-side read-only clone solves this without compromising the Windows-canonical build.

**Hard rule:** never `git commit` or `git push` from the WSL Hodos-Browser mirror. Treat it as a cache. If you find yourself wanting to edit there, you're doing it wrong — go back to the Windows side.

---

## Why GitHub-mediated, not rsync

Three options were considered:

| Option | Shape | Why rejected (or chosen) |
|---|---|---|
| **rsync (one-way or two-way)** | Periodic file-level sync via rsync over the 9P bridge | Bidirectional rsync's conflict resolution is "last write wins" — silent overwrites. Detecting "which side has the latest" requires complex mtime+hash tracking. **Lost-work risk is real.** |
| **git worktree** | One `.git` directory, two checkouts on different filesystems | `.git` directory accessed across 9P is slow for any git op. Doesn't solve the perf problem; introduces complexity. |
| **GitHub-mediated** ✅ | Both sides are full git clones of the same remote. Sync = `git push` / `git pull --ff-only` | Git's content-addressed hash trees ARE the "make cloning fast" answer Matt asked about. Incremental fetch only transfers new objects (a 5-line markdown edit = a few KB). Conflict detection is robust. Lost-work risk: only if commits aren't pushed before a machine dies — same as any normal git workflow. |

**Critical safety property:** uncommitted state is invisible to git, so it's invisible to sync. You can't accidentally overwrite uncommitted work because the sync doesn't see it. If you're mid-edit on Windows and don't want to lose it, just don't commit — WSL's mirror won't touch your working tree.

For the "I'm mid-edit and want Edwin to see it RIGHT NOW" case: manual `git add . && git commit -m "wip" && git push` from Windows, then `git pull` in WSL. Or set up a tighter sync cadence. Or accept that Edwin sees committed state only (recommended — committed state is more meaningful anyway).

---

## Repos to migrate (initial list, refine before execution)

| Repo | Current location | Target | Migration steps |
|---|---|---|---|
| **Hodos-Browser** | `C:\Users\archb\Hodos-Browser` ✓ canonical | Stays Windows. + WSL mirror at `~/repos/Hodos-Browser/` | `git clone <remote> ~/repos/Hodos-Browser` |
| **hodos-brand** (or marketing repo — naming TBD) | `C:\Users\archb\Marston Enterprises\hodos-brand\` ? | WSL canonical at `~/repos/hodos-brand/` | Pilot first — smallest repo, best to learn the workflow on |
| **Marston Enterprises** (business docs, contracts, planning) | `C:\Users\archb\Marston Enterprises\` | WSL canonical at `~/repos/marston-enterprises/` | Needs a private GitHub repo first if not already; high sensitivity content — confirm key-rotation policy first |
| **Dolphin Milk + Edwin Integration notes** | `C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\` | Subset of Hodos-Browser; stays where it is. WSL Edwin reads via the Hodos-Browser mirror. | No separate repo |
| **API_KEYS.md / admin notes** | `C:\Users\archb\Marston Enterprises\admin\` | WSL canonical; private repo or encrypted (TBD) | Sensitive — wait for explicit decision on key storage strategy before migrating |
| **BRCs, ts-sdk, arc, runar, BSV repos** | Already `~/repos/` in WSL ✓ | No-op | Already correct |
| **edwinpai, ~/.shad, ~/qmd** | Already WSL ✓ | No-op | Already correct |

**Pilot recommendation:** start with `hodos-brand` (smallest, lowest-risk). Validate the sync mechanism for ~1 week. Then migrate the larger business repos. Hodos-Browser mirror is the final step.

---

## Sync rules (the hard-coded part)

### Rule 1 — Per-repo classification

Every repo has exactly one of these classifications, written in a `.workspace-role` file at the repo root:

```
WINDOWS_CANONICAL    # Edit on Windows. WSL is read-only mirror. Sync = WSL pulls only.
WSL_CANONICAL        # Edit in WSL. Windows reads via UNC if needed. Sync = Windows pulls only.
BIDIRECTIONAL        # Either side can edit. Sync = both sides push + pull. Discipline required.
```

For Hodos-Browser the file contains `WINDOWS_CANONICAL`. For most others it's `WSL_CANONICAL`. Avoid `BIDIRECTIONAL` unless absolutely necessary — it requires the most discipline to avoid conflicts.

### Rule 2 — Auto-sync cadence

Cron / systemd timer in WSL fires every **10 minutes** (configurable). For each repo:

```bash
for repo in ~/repos/*/; do
  role=$(cat "$repo/.workspace-role" 2>/dev/null || echo "WSL_CANONICAL")

  case "$role" in
    WINDOWS_CANONICAL)
      # WSL-side read-only mirror. Pull only.
      git -C "$repo" fetch --quiet
      git -C "$repo" pull --ff-only --quiet 2>&1 || \
        echo "[WSL_MIRROR_FAIL] $repo — fast-forward failed, may have stale uncommitted state. Investigate."
      ;;

    WSL_CANONICAL)
      # WSL-side canonical. Push committed changes.
      if [ -n "$(git -C "$repo" status --porcelain)" ]; then
        echo "[WSL_CANONICAL_DIRTY] $repo — uncommitted changes, skipping push"
        continue
      fi
      git -C "$repo" push --quiet 2>&1 || echo "[PUSH_FAIL] $repo"
      ;;

    BIDIRECTIONAL)
      # Both: pull first (refuse if dirty), then push.
      if [ -n "$(git -C "$repo" status --porcelain)" ]; then
        echo "[BIDIR_DIRTY] $repo — uncommitted, skipping sync"
        continue
      fi
      git -C "$repo" fetch --quiet
      git -C "$repo" pull --ff-only --quiet 2>&1 || \
        echo "[BIDIR_DIVERGED] $repo — manual merge required"
      git -C "$repo" push --quiet 2>&1
      ;;
  esac
done
```

Scheduled task on Windows side (PowerShell, runs every 10 min) does the same for `WINDOWS_CANONICAL` and `BIDIRECTIONAL` repos: `git push` if committed work pending, `git fetch && git pull --ff-only` otherwise.

### Rule 3 — Loud alerts on anomalies

Any `[*_FAIL]` or `[*_DIRTY]` lines from the sync script go to a daily summary email / desktop notification. Silent failures = lost-work risk.

### Rule 4 — Never sync the working tree

The sync layer only operates on committed state. Uncommitted, untracked, or unstaged work stays on the side where it was made. **This is the load-bearing safety property.**

### Rule 5 — Conflict resolution is always manual

`--ff-only` refuses non-trivial merges. When sync alerts you that a repo diverged, drop into it interactively and resolve. No automated merge ever happens.

---

## Setup checklist (execute when ready)

This is a **future runbook**, not for today. Capturing the steps so future-me (or future-Claude) can execute cleanly when the moment comes:

1. **Pre-flight: settle the private GitHub story.**
   - Confirm which repos already have private GitHub remotes
   - Create private repos for any that don't (Marston Enterprises business docs, hodos-brand if missing)
   - Confirm the "Edwin Token" PAT or whichever auth the WSL/Windows clones will use

2. **Pilot: hodos-brand repo (or smallest business repo) end-to-end.**
   - Pick canonical location (probably WSL)
   - Clone in both Windows and WSL
   - Add `.workspace-role` file
   - Set up sync script in one place
   - Run for a week, verify no surprises

3. **Sync infrastructure.**
   - Write the sync script (skeleton above)
   - Install as systemd user timer in WSL: `~/.config/systemd/user/workspace-sync.{service,timer}`
   - Install as Windows Scheduled Task on the Windows side
   - Both fire every 10 minutes
   - Set up alert delivery (email or systemd-notify desktop)

4. **Migrate larger business repos.**
   - Marston Enterprises → `~/repos/marston-enterprises/`
   - Validate auth + sync still works on the larger payload

5. **Hodos-Browser mirror.**
   - `git clone <remote> ~/repos/Hodos-Browser/`
   - Mark `WINDOWS_CANONICAL`
   - Confirm sync only does fetch/pull, never push
   - Update Edwin's collection paths to point at the WSL mirror

6. **Edwin reconfiguration (final).**
   - Edit `~/.config/qmd/index.yml`: `hodos` collection path → `/home/archboldmatt/repos/Hodos-Browser` (was `/mnt/c/Users/archb/Hodos-Browser`)
   - Edit shad-context's `collectionPaths` to include all WSL canonical paths + the Hodos mirror
   - Restart gateway, verify recall lane returns sub-second
   - Re-run the `time shad search` measurement to confirm 200× → 1× speedup

7. **Document the new architecture in CLAUDE.md.**
   - Add note to root CLAUDE.md describing the hybrid + sync invariants
   - Link this doc

---

## Open questions to resolve before execution

1. **Private GitHub vs other host?** The Marston Enterprises content is sensitive. GitHub private is fine for most code. For HIGH-sensitivity docs (financial, legal, customer data), consider whether a self-hosted git server or end-to-end encryption is warranted.
2. **Key/credential storage for sensitive repos.** API_KEYS.md, contracts, etc. — do we git-commit these at all, or use git-crypt, or keep them out of any repo? Decide before migrating Marston Enterprises.
3. **Sync cadence.** 10 min is a starting point. Faster (1 min) = closer to real-time but more git noise. Slower (60 min) = less noise but Edwin sees stale content. Tune after pilot.
4. **Mac angle.** When Hodos goes back to Mac development, does the same hybrid apply on Mac? Probably no — macOS file system access is native, no 9P. Mac developers wouldn't have a WSL layer. So this doc is Windows-specific.
5. **Backup strategy.** Cloud backup of the WSL Linux filesystem is harder than Windows Documents. Decide on a backup story for the WSL canonical repos before moving anything critical.

---

## Cross-platform validation (modest secondary benefit)

While Edwin access drove this design, there's a small secondary benefit for the **DevOps/CI-CD sprint's A7 question** (where tests run, on which platforms):

- WSL gives a Linux-target build environment locally — useful for "did we introduce a Windows-only or Mac-only assumption?" detection
- ~10-20% of cross-platform bugs in shared code (Rust wallet, React frontend) are caught earlier
- Does NOT substitute for Mac hardware in CI matrix; does NOT help with Mac CEF or Cocoa code
- See `../README.md` A7 open question for full context

This is a **secondary** consideration. Don't design the hybrid around it. The primary driver is Edwin's chat-speed access to your content.

---

## Decision log

| Date | Decision | Reasoning |
|---|---|---|
| 2026-06-08 | GitHub-mediated sync, not rsync | Git's content-addressed model already solves "fast incremental sync." Conflict detection is robust. Uncommitted state never crosses the boundary (lost-work prevention). |
| 2026-06-08 | Hodos-Browser stays Windows-canonical | Build chain requires Windows-native tooling. UNC access from Windows back to WSL recreates the 9P perf problem. |
| 2026-06-08 | `.workspace-role` file per repo | Explicit classification beats convention. Sync script can be generic. |
| 2026-06-08 | `--ff-only` always | Refuses surprising merges. Forces manual resolution of divergence. |
| 2026-06-08 | Pilot with smallest repo first | Validate the workflow before staking critical content on it. |
| 2026-06-08 | Execute post-sprint, not now | Sigma-BRC121 sprint is active; workspace migration is a workflow change unsuitable for mid-sprint. |
