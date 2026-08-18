# beta.3 sprint harness — the standard

**Opened:** 2026-08-18 · **Applies to:** every phase in `SPRINT_PLAN.md` §4, retrofitted to Phase 0 and 0.5.

> **The problem this exists to solve.** This project's failure mode is not too few tests — it is tests
> that were never *capable* of failing. Four farbling harnesses would each have passed with the
> feature completely absent, and the shipped constant-seed bug survived every release because every
> check anyone ran was one the bug passed. So the harness is built around **provable falsifiability
> and provable subject**, not around coverage.

---

## 1. Unit of work: the phase contract

One `PHASE_CONTRACT.md` per `phase-*/` folder, written **before** the first line of code, from
`PHASE_CONTRACT_TEMPLATE.md`. Seven sections, none optional:

| § | Section | Rule |
|---|---|---|
| 1 | **Goal** | One sentence, user-observable. "Refactor X" is not a goal. |
| 2 | **Done means** | Results, not activities. Measurable. |
| 3 | **Invariants preserved** | Named, drawn from `REGRESSION_SET.md`. "Don't break anything" is banned. |
| 4 | **Evidence table** | See §2. This is the contract's spine. |
| 5 | **Blast radius** | What this touches that it is not about. |
| 6 | **Out of scope** | So scope creep shows up in review. |
| 7 | **Rollback** | How to undo in one commit. If you cannot say, the phase is too big. |

⚠️ **Scope changes amend the contract in the same commit that changes the scope.** Silent drift is
how a phase "passes" without having done the thing.

## 2. The evidence table — four columns, no empty cells

Every acceptance row:

| ID | 🟢 GREEN | 🔴 RED | 🎯 SUBJECT | Tier |
|---|---|---|---|---|

- **GREEN** — what must be true.
- **RED** — what must be *observed* to fail, and the exact action that forces it. Not "this would
  fail if broken" — the run you actually did, and its result.
- **SUBJECT** — what proves you measured the intended thing: which process, which browser, which
  binary, which build type. This column is where three farbling harnesses died.
- **Tier** — see §3.

⛔ **A row with an empty RED or SUBJECT cell is not done, regardless of how green it looks.**
⛔ **A green result is reported with its red half or not at all**: *"passes, and fails when X is off."*

### Two-sided invariants get two rows that are each other's control

Where the requirement is "A must happen and B must not", write both, and make each the other's
negative control. `P0.5-R1`/`R2` are the worked example: the user's own send must **never** prompt,
an external send over cap must **always** prompt. A fix that satisfies one by breaking the other is
the actual risk, and one row cannot see it.

## 3. Test tiers

| Tier | What | Where it runs | Status this sprint |
|---|---|---|---|
| **T0** | Static gates — grep-shaped, ratcheted (§4) | `scripts/preflight.ps1`, and `test.yml` when it wakes | ⚠️ **CI dark until ~2026-09-01** — local only |
| **T1** | Unit — `cargo test` ×2, `hodos_tests` | `scripts/preflight.ps1` | local |
| **T2** | Integration — real processes, dev ports, real wallet | by hand / scripted | local |
| **T3** | Observational — DPI matrix, multi-launch, overlay input, modal behaviour | a human at the machine | local |
| **T4** | Release gates — AV seeding, farbling rotation | build host | unaffected by the CI outage |

⚠️ **A phase that changes UI and declares no T3 row is a smell.** Most of this sprint's defects are
only visible to a person looking at a screen.

## 4. Static gates ratchet, they do not flip

A T0 gate carries a **baseline** count and fails when violations **exceed** it. That lets a gate land
*before* its fix — the gate is written in the phase that discovers the problem, and the phase that
fixes it drives the baseline to its target.

- A new violation always fails, even at a non-zero baseline. That is what preserves the negative control.
- Lowering a baseline is a deliberate commit. **Raising one requires a written reason in this file.**
- Residual violations at a non-zero target are **listed by file:line in the owning contract**, with why
  each is allowed. An unexplained residual is a defect, not a baseline.

## 5. Execution loop

```
Contract → Implement → Self-check → Adversarial review → Fix loop → Boundary regression → Sign-off
   ▲                                                          │
   └──────────  contract amended if scope actually changed  ◄──┘
```

- **Self-check** — the author runs the full evidence table, both halves of every row.
- **Fix loop** — a fix re-runs the **whole** table, not the failing row. Fixes are where regressions enter.
- **Boundary regression** — `REGRESSION_SET.md` in full, at every phase boundary, not only in the
  phase that owns the code.
- **Sign-off** — evidence table complete, preflight recorded, regression set recorded.

## 6. Adversarial review — a posture, not a checklist

A separate pass, by someone or something that did not write the code, whose job is to **refute**.
Four questions, answered in writing:

1. **Can I make this test pass with the feature removed?** If yes, the test is void — say so and stop.
2. **What is the subject?** Which process, browser, binary, build type. Prove it, do not assert it.
3. **What would I expect to see if this were broken — and did anyone look for that?**
4. **Is the claim a measurement or a code reading?** Both are legitimate. Mislabelling one as the
   other is not.

On disagreement, CLAUDE.md invariant #13: decide which side is wrong from **independent** evidence.
Test-only fixes may proceed. If the evidence points at production code, **stop and ask.**

### Where a workflow earns its cost

Fan-out is for breadth, not for ceremony. Use it at three points only:

| Phase | Workflow | Why |
|---|---|---|
| 0 — stray logs | ❌ | A grep and a build. |
| **0.5 — money path / trust boundary** | ✅ adversarial panel | Security boundary; distinct lenses beat one reviewer. |
| 1 — overlay/DPI | ❌ | Bounded, and the evidence is a human at two monitors. |
| **5 — routing predicate** | ✅ call-site sweep | Rewrites a predicate every request passes through. |
| **every boundary** | ✅ regression sweep | Independent, parallel, cheap. |

## 7. Traceability

Every evidence row has an ID: `P<phase>-<letter><n>` (`P0-A1`, `P0.5-R3`) or `R-<NAME>` for standing
checks. **The ID is cited in the commit message that satisfies it.** Then "did we meet our standards"
is a `grep`, and a phase that closed with an unreferenced row is visible.

## 8. Reporting honestly

- A skipped check is **SKIPPED**, and the run is **INCOMPLETE** — never PASS. `preflight.ps1` exits `2`.
- "Not reproduced" is a valid, complete state. Say it, name the experiment, do not upgrade it.
- Do not record a cause you have not reproduced. Two items in this sprint (the beta.1 stall, the
  `:5137` reachability) are explicitly hypotheses with named experiments.
- Correct stale rationale **in place**, struck rather than deleted — a plausible-but-wrong reason is
  how this project has previously talked itself into the wrong conclusion.

---

## 9. Gate baseline registry

Measured **by `scripts/preflight.ps1` itself** on 2026-08-18 — not by a hand-rolled grep. Two of
these differ from the hand counts that preceded them (`G2` 6→5, `G5` 11→15), which is the reason for
the rule: **baseline with the tool that will do the measuring, or the gate fails on day one for the
wrong reason.**

| Gate | What it catches | Baseline | Target | Owner | Lowered by |
|---|---|---|---|---|---|
| `G1` | bare-filename file sinks (relative path ⇒ CWD ⇒ `{app}`) | **52** | 0 | Phase 0 | Phase 0 |
| `G2` | substring origin checks on the internal frontend port | **5** | 2 | Phase 0.5 | Phase 0.5 |
| `G3` | F8 secret-log gate, Rust | **0** | 0 | ported from `test.yml` | — |
| `G4` | F8 secret-log gate, C++ | **0** | 0 | ported from `test.yml` | — |
| `G5` | full wallet HTTP response bodies reaching a sink | **15** | 0 | Phase 0 | Phase 0 |

⭐ `G2` deliberately **does not** flag a prefix check — `rfind(X, 0) == 0` or `find(X) != 0`. Those
are the correct form; flagging them would teach the wrong lesson. Only unanchored substring searches
count.

⭐ `G5` exists because `G1` alone is not enough. `G1` going to 0 removes today's sinks; `G5` keeps
catching the *shape* — a full wallet response body reaching any sink — if it later reappears through
`Logger` instead of `ofstream`. That is the specific path by which the recovery phrase reaches disk.

### Negative control — run and recorded

`pwsh scripts/preflight.ps1 -NegativeControl`, 2026-08-18: **all 5 gates PASS**, each detecting its
injected probe (`53>52`, `6>5`, `1>0`, `1>0`, `16>15`), probe files cleaned up. So every gate in this
table has been *observed* to fail. Re-run it whenever a pattern or baseline changes.

### Defects found by running the harness on itself

Recorded because they are the exact class this project keeps shipping, and three of the four would
have produced a **false green**:

1. **`-Only G1,G2` bound as one string** under `-File`, so every `-contains` missed and **zero checks
   ran** — reported as `PASS`. Fixed by splitting on commas.
2. **Zero executed checks reported `PASS`.** This is the shape of all four farbling harnesses. Now an
   explicit `INCOMPLETE` / exit 2.
3. **`,$hits` re-wrapped in `@()`** at the call sites nested the array, so every gate counted **1**.
   G1 read `1 violation` against a real 52 — and *passed*, as "below baseline".
4. `cargo`'s stderr warnings became terminating errors under `EAP=Stop`, killing the run on a warning.

### Baseline run — 2026-08-18, before any Phase 0 code

```
T0  G1  PASS  52 violations, at baseline   [Phase 0,    target 0]
T0  G2  PASS   5 violations, at baseline   [Phase 0.5,  target 2]
T0  G3  PASS   0 violations, at baseline   [ported,     target 0]
T0  G4  PASS   0 violations, at baseline   [ported,     target 0]
T0  G5  PASS  15 violations, at baseline   [Phase 0,    target 0]
T1a     PASS  cargo test - rust-wallet
T1b     PASS  cargo test - adblock-engine
T1c  SKIPPED  hodos_tests - not built
T1d  SKIPPED  frontend build - not requested

PREFLIGHT: INCOMPLETE - 0 failed, 2 skipped.        exit 2
```

⭐ **This is the harness working, not failing.** Nothing is broken — two checks did not run, so the
run is `INCOMPLETE` and exits `2`. A tool that had printed PASS here would be the same instrument
that let a constant seed ship in every release.

To close the two skips: build `hodos_tests` once
(`cmake -S cef-native -B cef-native/build -DHODOS_BUILD_TESTS=ON`, then
`cmake --build cef-native/build --config Release --target hodos_tests`) and pass `-Full` for the
frontend leg. Until then, **record INCOMPLETE in the sign-off table — do not round it up.**
