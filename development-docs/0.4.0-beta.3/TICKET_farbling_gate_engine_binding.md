# TICKET — the farbling release gate cannot tell which of our engines produced a token

**Filed:** 2026-08-17, during the beta.2 release run
**Severity:** gate weakness, not a shipped-product defect
**Status:** ✅ CLOSED 2026-09-14 (beta.3 Phase 9, `P9-C1` harness + `P9-C2` gate, two commits per rule 6). See `phase-9-release-readiness/PHASE_CONTRACT.md` §4.
**Sprint:** 📌 Phase 9 (release readiness) — bundled 2026-08-31.
**Owner decision needed:** no. This is a straight fix; only the sequencing is a choice.

---

## One-line statement

`promote.yml`'s farbling gate claims to *"bind the result to an engine that can actually
farble."* It actually binds to **"Chromium major ≥ 150"**, which every P4-era engine satisfies —
so a rotation token measured on **P4e** passes the gate for a **P4f** release, and nothing anywhere
notices.

## What is NOT wrong

Say this first, because the surrounding text is alarming and the distinction matters:

- The shipped browser is fine. C1–C6 + P4e + P4f are in the engine beta.2 was built against, and CI
  now asserts that by `CEF_VERSION` out of the artifact (`release.yml`, both arms, since `a861e53`).
- The rotation **measurements** are real. Unlinkability and determinism were measured on all four
  vectors, with both controls holding still and a negative control that goes RED. Nothing about the
  verdict is in doubt.

What is weak is the gate's **binding of a verdict to a build**. The gate cannot detect a token
pasted from the wrong engine.

## The defect, in three files

**1. The harness knows the right answer.** `farbling_seed_rotation_check.py:546` —

> ⛔ WHY THIS EXISTS, and why `engine_version()` is not a substitute. `engine_version()` reports the
> CHROMIUM version over CDP. Two different Hodos engines … Chromium 150.0.7871.187, differing only
> in our Blink patches.

**2. It built the right instrument and never called it.** `require_engine()` is defined at line 649
— the same function Mac wired into `farbling_psl_linkability_check.py` — and has **zero call sites**
in this file.

**3. It feeds the wrong value into the token.** Line 879:

```python
return {"exempt": ex, "farbled": fa, "engine": engine_version(args.port)}
```

Producing, measured on 2026-08-17 against a P4f build:

```
FARBLING-ROTATION-v1 engine=Chrome/150.0.7871.187 exempt=… large=… farbled=… verdict=PASS
```

**4. The gate's parse cannot recover.** `promote.yml`, farbling re-derive step:

```bash
MAJOR=$(printf '%s' "$ENGINE" | grep -oE '[0-9]+' | head -n1 || true)
if [ -z "$MAJOR" ] || [ "$MAJOR" -lt 150 ]; then  # refuse
```

`Chrome/150.0.7871.187` → first integer `150` → passes.

**5. And the harness's own usage note is false.** Line 73 tells the reader to *"check `CEF_VERSION`
(printed as `engine=`)"*. It does not print `CEF_VERSION`.

## Why this is exactly the trap we already wrote down

`reference_engine_identity_not_chromium_version` and relay §FF2 both record it: **P4e (`g7dd0357`)
and P4f (`g9ccef04`) are both `chromium-150.0.7871.187`.** The Chromium version proves two arms
match; it never proves *which* build. We identified the trap, wrote the guard, wired it into four
harnesses — and left it out of the one whose output a release gate consumes.

Concretely, P4e vs P4f is not a cosmetic difference. P4f is what closed:
- workers (the page's own realm — beta.1's headline bypass class),
- `OffscreenCanvas.convertToBlob`,
- three unhooked audio readers on the main frame.

A P4e token passing a P4f gate would green-light a release whose fingerprint claim is a ladder row
too high.

## The fix

**A. Harness — make the token carry the discriminating string.**
Call `require_engine()` and put `CEF_VERSION` in the `engine=` field:

```
engine=150.0.43-7871.3576+g9ccef04+chromium-150.0.7871.187
```

⭐ **Backward-compatible with the gate as it stands today** — `grep -oE '[0-9]+' | head -n1` on that
string still yields `150`, so the existing check keeps passing. Harness and gate can therefore ship
in either order, which removes the only real sequencing risk.

**B. Gate — assert the fork SHA, not just the major.**
Add a `farbling_expected_engine` input (or read the expected value out of `release.yml`'s
`env.CEF_ASSET`, which is now the single definition per arm) and require the token's `engine=` to
contain it. Keep the `≥150` check as the floor.

**C. Fix the false docstring** at line 73 so it describes what the code does.

**D. ⛔ Negative-control the fix, or it is not done.**
A gate change that has never been seen to refuse is exactly the defect class this ticket is about.
Required evidence before closing:
- a token with a **P4e** `engine=` string is **REFUSED** by the gate;
- a token with a garbage `engine=` string is **REFUSED**;
- the real P4f token still **PASSES**.

Run those against the gate's actual shell logic, not by reading it.

## Second, related gap — worth deciding in the same sitting

**The gate measures a build that is not the build being promoted.** The token comes from a **local
dev build** on the build host; the thing promoted is the **CI-built, signed installer**. Same tree
and same staged CEF, but not the same bytes, and the gate has no way to tell. That is inherent to
running the measurement on the build host — which is itself a deliberate, re-argued decision
(persistent profile, real restarts, GPU renderer; see `promote.yml`'s block comment, corrected
2026-08-15).

Options, in ascending cost:
1. Accept and **document the gap explicitly** in the gate comment, so no future reader mistakes
   "farbling gate green" for "the promoted installer was measured".
2. Have the harness also record the **subject binary's identity** (path + hash + `CEF_VERSION`) in
   the token, so at least the gap is visible per-release rather than implicit.
3. Run the rotation check against the **installed** artifact. The CDP port is open on release builds
   (`cef_browser_shell.cpp`: Default profile → 9222), so this is technically possible today — but it
   would install a beta over the owner's production profile, and it collides with the separate
   open question of whether that port should be open in release at all (see
   `TICKET_cdp_port_open_in_release.md`).

Recommendation: **1 + 2 now, 3 only if the CDP-port decision lands in a way that makes it clean.**

## Acceptance

- [x] `engine=` in the token carries `CEF_VERSION`, including the fork SHA — 📏 real run 2026-09-14: `engine=150.0.43-7871.3576+g9ccef04+chromium-150.0.7871.187`; `require_engine()` is called before launch and also ties the header to the loaded `libcef.dll` (md5) on Windows
- [x] gate refuses a P4e token and a garbage token; still passes the real one — **measured** 2026-09-14 by running the step's `run:` body under bash locally: real token exit 0; `+g7dd0357+` edit exit 1; legacy `Chrome/150.0.7871.187` exit 1; `banana` exit 1; sub-150 exit 1
- [x] line 73's usage note matches the code
- [x] the dev-build-vs-shipped-installer gap is stated in the gate's own comment ("WHAT THIS GATE STILL CANNOT SEE")
- [x] `FARBLING_RELEASE_GATE.md` updated to match

## Provenance

Found while producing the beta.2 rotation token (2026-08-17). The token, its negative control and
the full four-vector result are in the relay round of that date. The run that consumed this token —
`promote.yml` dry run `32050154040` — passed **with the weak binding in place**, which is precisely
why this is worth fixing before it is ever load-bearing.
