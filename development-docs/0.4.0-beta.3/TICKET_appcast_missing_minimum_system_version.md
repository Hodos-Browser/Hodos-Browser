# TICKET — the macOS appcast advertises no `minimumSystemVersion`, and the floor just moved 11.0 → 12.0

**Filed:** 2026-08-17, during the DevOps-CICD doc review
**Severity:** 🚨 **auto-update brick risk on macOS 11** — blocks promoting the 0.4.0 line to Sparkle
**Status:** OPEN — **candidate blocker for promoting 0.4.0**
**⛔ Production code untouched.** Per CLAUDE.md invariant #13 the evidence points at
`scripts/generate-appcast.py` being wrong, so this is filed for approval rather than fixed.

---

## The defect

`scripts/generate-appcast.py` **never emits `<sparkle:minimumSystemVersion>`.** There is no such
argument, no such code path, and no OS gating of any kind — verified by reading its whole argument
surface and grepping the file.

Consequently **no feed we have ever published carries it**:

| Feed | `minimumSystemVersion` occurrences |
|---|---|
| `v0.4.0-beta.2` draft appcast (built 2026-08-17) | **0** |
| **live** `hodosbrowser.com/appcast.xml` (beta.29, what users' Sparkle reads today) | **0** |

Meanwhile the macOS floor moved with the CEF 150 bump. The beta.2 build's own CI log:

```
minos guard PASSED (all >= framework 12.0)      minos=12.0
```

`release.yml` sets `MACOSX_DEPLOYMENT_TARGET: "12.0"` and `-DCMAKE_OSX_DEPLOYMENT_TARGET=12.0`.

## The failure

A user on **macOS 11 (Big Sur)** running 0.3.x:

1. Sparkle fetches the feed. The macOS item has **no OS constraint**, so 0.4.0 is offered.
2. Sparkle downloads the DMG, verifies the EdDSA signature (it is genuine), and installs.
3. Every shipped Mach-O has `minos = 12.0`. **dyld refuses to load on 11.x.**
4. The app does not launch. Sparkle has already replaced the installed app, so the user is left with
   a browser that will not start and no in-product path back.

That population is not hypothetical: **11.0 was our published floor for the entire CEF 136 era**, so
anyone who installed 0.3.x on Big Sur is exactly who this hits.

⚠️ **This directly violates the standing principle that auto-update must never brick an install**
(`feedback_update_stability_principle`). It is the beta.16 "requires macOS 26.0" failure from the
opposite direction: then the binary's floor was accidentally too high for the runner, now the feed
fails to declare a floor that legitimately rose.

## Why it hasn't bitten yet

**Because we have not promoted.** beta.1 and beta.2 are both drafts, so no 0.4.0 feed has ever
reached a Sparkle client. The live feed still advertises beta.29, which is an M136 build whose
`minos` genuinely is 11.0 — correct today, purely by accident of not having shipped.

⭐ Holding beta.2 unpromoted is what kept this out of users' hands. Fix before the 0.4.0 line goes
live, not after.

## The fix

Emit the element on the macOS item:

```xml
<sparkle:minimumSystemVersion>12.0</sparkle:minimumSystemVersion>
```

- Add a `--macos-minimum-system-version` argument to `generate-appcast.py`, **required** for the
  macOS item — defaulting it invites the value to rot silently, which is how we got here.
- Pass it from `release.yml`'s appcast step. ⭐ **Derive it from the same value the build uses**
  (`MACOSX_DEPLOYMENT_TARGET`) rather than hardcoding a second copy, so the feed cannot disagree
  with the binary. A second literal is a second thing to forget on the next bump.
- Assert it in `release.yml`'s existing offline appcast check **and** in `promote.yml`'s pre-flip
  superset, alongside the version/enclosure/signature assertions already there.

⚠️ **Windows needs no equivalent** — WinSparkle has no `minimumSystemVersion`, and our Windows floor
has not moved. Scope this to the macOS item.

## ⛔ Negative control

An assertion for a field that has never been absent in a test has not been shown to test anything.
Required before closing:

- generate a feed with the value **omitted** → the new assertion must **FAIL**;
- generate with a **wrong** value (e.g. `11.0` while the build targets `12.0`) → must **FAIL**, since
  that is the exact drift being guarded;
- generate with the correct value → passes.

Ideally the check compares the feed's value against the **measured** `minos` of the DMG's Mach-O
rather than against a literal, which makes drift impossible rather than merely detectable.

## Also fix the doc that got this right

`BUILD_AND_RELEASE.md` §4.5's appcast example **does** show the element, with the comment
*"Must match our published floor"* — and carried the stale `11.0`. So the documentation specified
the correct behaviour and the implementation never had it. Worth noting when auditing other specs
in that file: an example in a doc is not evidence the code does it.

## Acceptance

- [ ] macOS appcast item carries `minimumSystemVersion`, sourced from the build's deployment target
- [ ] `release.yml` and `promote.yml` both assert it
- [ ] omitted / wrong values both **measured** to fail the assertion
- [ ] `BUILD_AND_RELEASE.md` §4.5 example updated to `12.0`
- [ ] decide whether a Big Sur user on 0.3.x should be offered *anything* (a pinned final 0.3.x, or
      nothing) rather than silently never updating again
