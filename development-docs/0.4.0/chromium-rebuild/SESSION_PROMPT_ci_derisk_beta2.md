# Session kickoff prompt — de-risk the CI pipeline before tagging beta.2

> Paste everything between the rules into a fresh session.

---

Prove that CI builds cleanly against the **new P4f CEF assets** before `v0.4.0-beta.2` is
tagged. CI has never pulled these assets, and the failure mode we are buying insurance
against is discovering a problem *during the release tag* rather than on a throwaway run.

## Read first

1. `development-docs/0.4.0/MAC_WINDOWS_RELAY.md` — newest round only (`2026-08-15c`, §II1 and
   §II6). It carries what was swapped and what is still owed.
2. `development-docs/0.4.0/chromium-rebuild/FARBLING_DEFINITION_OF_DONE.md` §H — the standing
   backlog, so you can tell blocking work from nice-to-have.
3. `CLAUDE.md` → Guidelines + Testing Standards.

## What is already done — do not redo it

- Both `cef-binaries` assets are uploaded, versioned, and round-trip verified:
  `cef-binaries-windows-150.0.43-g9ccef04.zip` (239,428,480 B) and
  `cef-binaries-macos-150.0.43-g9ccef04.tar.bz2` (127,585,737 B). P4e assets untouched.
- All six `release.yml` references were swapped to them in one commit (`b018258`).
  `grep -rn "g7dd0357\|150\.0\.42" .github/workflows/` is empty.
- The Windows zip is spec-clean: single `cef-binaries/` root, **no backslashes** in entry
  names, 1689 files, `testzip()` clean, `CEF_VERSION` read out of the archive **and** out of a
  fresh `gh release download`.

## ⛔ The thing that makes this non-trivial

**`release.yml` has NO `workflow_dispatch`. Its only trigger is `on: push: tags: v*`.** So
there is no button to press, and the first decision is how to trigger a build at all. Work out
the cheapest safe option and **put it to the owner before doing it** — each path has a real
cost:

| Path | Cost / risk |
|---|---|
| Add `workflow_dispatch` to `release.yml` | A permanent, arguably-good improvement — but the job derives its version from the tag (`Get version from tag`), so a dispatch run needs a `version` input and a fallback, and you must check whether the **publish** job would mint a real release on a dispatch run. Do not let it publish. |
| Push a throwaway `v*` tag to `release` | Runs the real pipeline, but mints a draft release and consumes the signing path. Org Actions are free, so minutes are not the issue — the artifact clutter is. |
| ⛔ Push a `v*` tag to `origin` | **Never.** The `cef-binaries` release exists only on the org repo, so the build dies at the download step and burns ~19 min of the dev fork's 2,000-min allowance for nothing. |
| Verify locally instead | Cheapest. Extract the published zip **with 7z** (what the runner uses, not Python) into a clean dir and confirm CMake's expected tree — `cef-binaries/Release/libcef.dll`, `cef-binaries/include/`, `cef-binaries/build_wrapper/libcef_dll_wrapper/Release/libcef_dll_wrapper.lib`. Does not exercise the six changed lines or the end-to-end build. |

**Recommend one, with the trade-off stated, and wait for the owner's answer.**

## Hard rules

- ⛔ **Read `CEF_VERSION`, never the Chromium version.** P4e (`g7dd0357`) and P4f (`g9ccef04`)
  are **both** Chromium `150.0.7871.187`, so `Chrome/150.0.7871.187` in a log or over CDP
  proves nothing about which engine was used. Require `150.0.43-7871.3576+g9ccef04` out of the
  **artifact**. A stale asset is a silent failure: CI builds green and ships a browser missing
  the fix the release exists for.
- **Verify by artifact, not by exit code.** A green run whose inputs you have not checked is
  not evidence. This project has had a clean `121 patches total (0 applied, 0 failed)` report
  on a build with **every** farbling patch silently skipped.
- **Never `--clobber` a `cef-binaries` asset.** New name per engine, always — that is what
  keeps a CI failure bisectable across engines.
- If something fails, decide whether the **workflow** or the **asset** is wrong from
  independent evidence before changing either (CLAUDE.md invariant #13).

## Definition of done

- CI has built **both** platforms against `g9ccef04`, or the owner has accepted a documented
  local-only verification instead.
- The engine in each artifact is confirmed by `CEF_VERSION`, quoted in the report.
- Any workflow change you made is explained, and its blast radius on the real release path is
  stated explicitly.
- The result is written into `MAC_WINDOWS_RELAY.md` as a new round, and pushed.

## Then state plainly

Whether `v0.4.0-beta.2` is safe to tag, and what remains (as of 2026-08-15: macOS app rebuild
+ H11 re-measure, and owner approval of `RELEASE_NOTE_farbling_draft.md`).
