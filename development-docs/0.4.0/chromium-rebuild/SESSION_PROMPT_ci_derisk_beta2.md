# Session kickoff prompt — add `workflow_dispatch`, then de-risk CI before beta.2

> Paste everything between the rules into a fresh session.

---

Two jobs, in order:

1. **Add a manual trigger (`workflow_dispatch`) to `release.yml`**, safely, and document it in
   `development-docs/DevOps-CICD/BUILD_AND_RELEASE.md`.
2. **Use it to prove CI builds cleanly against the new P4f CEF assets** before
   `v0.4.0-beta.2` is tagged. CI has never pulled these assets, and the failure we are
   insuring against is discovering a problem *during the release tag*.

## Read first

1. `development-docs/0.4.0/MAC_WINDOWS_RELAY.md` — newest round only (`2026-08-15c`).
2. `development-docs/DevOps-CICD/BUILD_AND_RELEASE.md` — the release flow you are modifying.
3. `CLAUDE.md` → Guidelines, Testing Standards, and Branch & Remote Workflow.
4. `.github/workflows/promote.yml` — **it already uses `workflow_dispatch` with typed inputs.
   Follow that pattern rather than inventing one.**

## Already verified — do not redo

- Both assets are uploaded, versioned and round-trip verified:
  `cef-binaries-windows-150.0.43-g9ccef04.zip` (239,428,480 B, md5 `bf9f1e5f1acc…`) and
  `cef-binaries-macos-150.0.43-g9ccef04.tar.bz2` (127,585,737 B, md5 `82d300fcd650…`).
  P4e assets untouched.
- All six `release.yml` references swapped in one commit (`b018258`);
  `grep -rn "g7dd0357\|150\.0\.42" .github/workflows/` is empty.
- The Windows zip is spec-clean: single `cef-binaries/` root, no backslashes, 1689 files,
  CRCs pass, `CEF_VERSION` read out of the archive **and** out of a fresh download.

## ⛔ Three hazards found while scoping this. Handle each explicitly.

**H-A — the build number collides, and it feeds an anti-rollback gate.**
`release.yml`'s `Get version from tag` does `$version = "${{ github.ref_name }}" -replace '^v',''`.
On a `workflow_dispatch` `ref_name` is the **branch**, not a tag. From branch `0.4.0` that
yields version `0.4.0`, no `-beta.N`, so `beta = 99` and
`BUILD_NUMBER = 0*1000000 + 4*10000 + 0*100 + 99 = 40099` — **exactly what the eventual final
0.4.0 release will produce.** The Windows updater compares this integer as an anti-rollback
gate, so a stray 40099 artifact is not merely untidy. Also, a non-version branch name (`main`)
crashes the parse at `[int]$p[1]`.
⇒ The dispatch path needs its own version handling. Decide and justify: a required `version`
input, and/or a distinct build number that cannot collide with any real release.

**H-B — `publish` would mint a release.** The `publish` job runs
`softprops/action-gh-release@v2` with `draft: true`. On a dispatch run that must not happen.
⇒ Gate it (e.g. `if: github.event_name == 'push'`) and **prove the gate works** rather than
assuming — a de-risk run that quietly creates a draft release is worse than no run.

**H-C — it can only run on the `release` (org) repo.** `release.yml` downloads with
`--repo ${{ github.repository }}`, and the `cef-binaries` release **does not exist on the dev
fork** (`gh release view cef-binaries --repo BSVArchie/Hodos-Browser` → *release not found*).
The Azure Trusted Signing secrets live there too.
⇒ Per CLAUDE.md the code originates on `origin`, so the change lands there first and must then
reach `release` before it can be dispatched. **Put that sequencing to the owner before pushing
to `release`.**

## Decisions to put to the owner — do not settle these yourself

1. **Does the dispatch run exercise signing?** Letting it run proves more of the pipeline but
   consumes signing quota on a throwaway build. Skipping it leaves the signing path untested by
   this exercise.
2. **The version/build-number scheme for dispatch runs** (from H-A).
3. **Pushing the workflow change to `release`** (from H-C).

## Hard rules

- ⛔ **Read `CEF_VERSION`, never the Chromium version.** P4e (`g7dd0357`) and P4f (`g9ccef04`)
  are **both** Chromium `150.0.7871.187`. `Chrome/150.0.7871.187` in a log proves nothing about
  which engine CI pulled. Require `150.0.43-7871.3576+g9ccef04` **out of the artifact**. A stale
  asset is a silent failure: CI builds green and ships a browser missing the fix the release
  exists for.
- **Verify by artifact, not exit code.** This project has had a clean
  `121 patches total (0 applied, 0 failed)` report on a build with **every** farbling patch
  silently skipped.
- ⛔ **Never push a `v*` tag to `origin`** — the build dies at the CEF download and burns ~19 min
  of the dev fork's 2,000-min monthly allowance. Org Actions are free; the dev fork's are not.
- **Never `--clobber` a `cef-binaries` asset.** New name per engine — that is what keeps a CI
  failure bisectable across engines.
- If something fails, decide whether the **workflow** or the **asset** is wrong from independent
  evidence before changing either (CLAUDE.md invariant #13).

## The DevOps-CICD note (explicitly asked for)

Add to `development-docs/DevOps-CICD/BUILD_AND_RELEASE.md`, near the existing trigger
description:

- that `release.yml` now has a manual trigger, **what it is for** (validating an engine/asset
  swap without minting a release), and how to run it;
- ⛔ **what it deliberately does NOT do** — publish — and the guard that enforces that;
- the build-number hazard from H-A and how the dispatch path avoids it;
- that it only works on the **release** repo, and why.

Keep it in `BUILD_AND_RELEASE.md` rather than a new file unless it genuinely does not fit —
this project's docs rule is one home per fact.

## Definition of done

- `workflow_dispatch` added, publish provably gated off, version/build-number hazard handled.
- A dispatch run has built **both** platforms against `g9ccef04`, with the engine confirmed by
  `CEF_VERSION` from each artifact and quoted in the report.
- No draft release was created by that run — verified, not assumed.
- `BUILD_AND_RELEASE.md` updated as above.
- A new round written into `MAC_WINDOWS_RELAY.md` and pushed.

## Then state plainly

Whether `v0.4.0-beta.2` is safe to tag, and what remains. As of 2026-08-15 that is: macOS app
rebuild + H11 re-measure (Mac, unblocked), and owner approval of
`RELEASE_NOTE_farbling_draft.md`.
