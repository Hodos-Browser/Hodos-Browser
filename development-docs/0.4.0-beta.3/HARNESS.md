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

### A RED must fail AT the subject — a control that goes red earlier tested nothing

A negative control is only evidence if it fails **inside the check under test**, for the reason the
row names. A control that goes red *upstream* — corrupted input, a crash, a missing file, a lookup
that never reaches the check — produces a RED that looks exactly like the one you wanted.

- **Record where it failed**, not just that it failed: the log line or error *from the subject* that
  names the reason. A top-level error string is not that — read the underlying error.
- **Perturb only the thing the check guards**, and keep everything else valid. Prefer the realistic
  attack (a valid substitute) over breaking the input.
- **Ask of every RED: would it still go red if the check under test were deleted?** If yes, it is
  not a control for that check.

📏 Worked example, C1 2026-09-23 (relay 23f §3): "flip one byte of the DMG" was the spec's control for
Sparkle's EdDSA check. Sparkle 2 verifies app-bundle archives **after** extraction, so the flip
crashed `Autoupdate` mid-copy — rejected **by corruption**, with the signature never read. The real
controls: flip one bit of the **signature** (bytes valid), and substitute a **valid different** DMG
(signature real); both failed with Sparkle's own `EdDSA signature does not match`. Same run, a second
trap: Sparkle shows *"The update is improperly signed"* as the top-level text for `3002 No suitable
install is found` — a rig artifact, not a signature verdict. Same family as the three farbling
harnesses and the `--enable-automation` cell in Phase 13.

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

### ⛔ `G11` did **not** move in Phase 3.5, and that is not a phase that failed — 2026-08-31

Phase 3.5 shipped its window-scoping fix and `G11` stayed at **60**. Recorded here because the rule
above only demands a written reason for *raising* a baseline, and the gap that leaves is worse: a
later reader sees a phase whose stated deliverable was `G11↓`, sees an unchanged baseline, and
concludes the work did not land. Two measured reasons (`phase-3.5-layout-window-scoping/MEASUREMENTS.md`
K4), neither of them a judgement call:

1. **Paths.** `G11.Paths = @('cef-native/src/handlers', 'cef-native/src/core')`. The file the phase is
   mostly about, `cef-native/cef_browser_shell.cpp`, sits at `cef-native/` and is in **neither**.
2. **Pattern.** `G11.Pattern` matches `GetPrimaryWindow()` / `GetActiveTab()`. It matches **no**
   `g_hwnd`, `g_header_hwnd` or `g_*_overlay_hwnd` — so even the 12 `ScalePx(x, g_hwnd)` sites the
   phase converted are invisible to it, in a file it *does* scan.

⛔ **Widening the paths or the pattern so the gate covers this code was deliberately NOT done**, per
working rule #6 — that is editing the instrument inside the change it measures, and this project has
already shipped a preflight that reported PASS while running zero checks. If it is wanted, it is its
own commit, after the fix, with its own `-NegativeControl` run and a re-measured baseline.

⚠️ **Known stale text, left alone on purpose:** `G11`'s description in `scripts/preflight.ps1` still
reads *"baseline lowered by Phase 3.5"*. It was written when the phase was planned and K4 measured it
false. Correcting it is an edit to the instrument and therefore belongs in that same separate commit,
not in the change it describes.

### ✅ `G11` **59 → 58**, 2026-09-19 — lowered by the macOS twin of the same deletion

📏 **58, MEASURED BY THE SCRIPT** at `33d6d4a`. Earned by the macOS side's `6775b63`
(*"delete the backup-overlay chain on macOS"*), which removed the remaining
`WindowManager::GetInstance().GetPrimaryWindow()` lookup while finishing, on its platform, the
deletion whose Windows half had already earned `60 → 59` at Phase 8c `O8`. ⭐ **The same pair of
commits opened and closed this ratchet step** — worth noting, because it is the shape a
cross-platform deletion is supposed to have.

⛔ **Instrument-only commit**, no product code, per working rule 6. `-NegativeControl` re-run.

⚠️ **Bisected, not assumed.** The drop was first noticed as *"58 violations, BELOW baseline 59"*
and three wrong explanations were checked and discarded before the real one: (a) a hand count of
**occurrences** said 59 — the gate counts matching **lines**, and one line carries two matches;
(b) the gate drops whole-line comments, and `simple_handler.cpp` has one commented mention of
`GetPrimaryWindow()` — but it dates to 2026-08-30 and so predates the 59 baseline; (c) the
comment-dropping filter itself was suspected of being a late instrument change — it has been in
the harness since `8ef5b93`, 2026-08-18. ⭐ Only walking every commit from the baseline-setting
commit forward, counting the way the gate counts, named the right one.

### ✅ `G11` **60 → 59**, 2026-09-13 — lowered by beta.3 Phase 8c `O8`, in its own commit

Phase 8c's `O8` deleted the dead backup-overlay chain (`267b079`). One of the lines that went with it
was `CreateBackupOverlayWithSeparateProcess()`'s `WindowManager::GetInstance().GetPrimaryWindow()`
lookup in `cef-native/src/handlers/simple_app.cpp`, and `preflight.ps1` reported
*"59 violations, BELOW baseline 60 — lower the baseline"* on every run after it. Per this section and
working rule #6 the baseline was **not** touched inside `267b079`; this commit lowers it, and while it
is in the instrument it also retires the stale *"lowered by Phase 3.5"* owner text above (measured
false by K4 — same commit, as that note asked). Re-measured by the script itself, and `-NegativeControl`
re-run: the probe line must take the count to `60 > 59` and go red.

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
| `G1` | bare-filename file sinks (relative path ⇒ CWD ⇒ `{app}`) | ~~52~~ → **0** | 0 | Phase 0 | ✅ Phase 0, 2026-08-18 |
| `G2` | substring origin checks on the internal frontend port | ~~5~~ → **2** | 2 | Phase 0.5 | ✅ Phase 0.5, 2026-08-19 |
| `G3` | F8 secret-log gate, Rust | **0** | 0 | ported from `test.yml` | — |
| `G4` | F8 secret-log gate, C++ | **0** | 0 | ported from `test.yml` | — |
| `G5` | full wallet HTTP response bodies reaching a sink | ~~15~~ → **0** | 0 | Phase 0 | ✅ Phase 0, 2026-08-18 |
| `G11` | window-scoped work resolved through a process-global (`GetPrimaryWindow()` / `GetActiveTab()`) | ~~60~~ ~~59~~ → **58** | 0 | Phase 3 (WS2) | ✅ beta.3 Phase 8c `O8`, 2026-09-13 (one site, with the backup-overlay deletion) · ✅ **2026-09-19, its macOS twin `6775b63`** (the remaining `GetPrimaryWindow()` lookup, same deletion finished on macOS); beta.4 drives to 0 |
| `G12` | unanchored host:port URL matchers on the wallet trust boundary | **4** | 0 | Phase 5 | — (driven to 0 by beta.4 W8) |

⭐ `G2` deliberately **does not** flag a prefix check — `rfind(X, 0) == 0` or `find(X) != 0`. Those
are the correct form; flagging them would teach the wrong lesson. Only unanchored substring searches
count.

⭐ `G12` was **added by Phase 5, 2026-09-02**, and is G2's sibling: G2 owns the frontend port
(`:5137`), G12 owns every other host:port matcher — our wallet port and the foreign bridge ports. It
is a **trust** gate, not a tidiness one: C++ is what stamps `X-Requesting-Domain`, and Rust reads a
missing header as internal and fully trusted, so a matcher that misses leaves traffic *trusted*.
Baseline **4**, measured by `preflight.ps1` itself per the rule above, and all four are deliberate —
`IsWalletHostPort` and `IsLoopbackHostPort` survive only to back `LegacyWalletGateMatch`, the W3
shadow predicate that decides nothing. Beta.4's W8 retires all three and drives this to 0.
⚠️ It catches the *literal* form, which is where the defect is written: it would have flagged the
pre-Phase-0.5 `url.find("localhost:3321")` gate. A call site routing through a helper is covered by
the helper's own violation — one owner per defect, not N.
⛔ Added in its **own commit**, after the fix it measures had already landed (`1743b20`), per
working rule #6, and re-run with `-NegativeControl` (detected `5 > 4`).

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

### Baselines lowered — 2026-08-18, Phase 0 (`P0-A3`, `P0-A8`)

`G1` **52 → 0** and `G5` **15 → 0`**. No residuals in either: every one of the 52 relative-path
writes is gone, and no log statement in `WalletService.cpp`/`WalletService_mac.cpp` names a response
at all.

⚠️ **Two things worth keeping from driving `G5` to zero**, because both are the harness catching the
author rather than the code:

1. **Two replacement lines passed only because `grep` is line-based.** `LOG_INFO_BROWSER("… " +
   response["txid"]…)` wrapped across two lines, so the gate never saw `LOG_…(` and `response` on one
   line. That is a **false green** produced by code formatting, and it is the same family as the four
   defects in §9 below. Fixed properly by extracting `txid`/`err` into named locals **before** the log
   call — which is also better code.
2. **`G5` at target 0 is a naming rule, not only a leak detector.** Reaching zero required rewording
   four log messages that merely contained the English word "response". That is accepted deliberately:
   for these two files — the wallet HTTP transport — "no log statement may name the response" is a
   cheap, enforceable bright line, and the alternative (loosening the pattern so prose passes) would
   weaken the gate to make the code look clean.

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

### Phase 0 run — 2026-08-18, after the fix

```
T0  G1  PASS   0 violations, at baseline   [Phase 0,    target 0]
T0  G2  PASS   5 violations, at baseline   [Phase 0.5,  target 2]
T0  G3  PASS   0 violations, at baseline   [ported,     target 0]
T0  G4  PASS   0 violations, at baseline   [ported,     target 0]
T0  G5  PASS   0 violations, at baseline   [Phase 0,    target 0]
T1a     PASS  cargo test - rust-wallet
T1b     PASS  cargo test - adblock-engine
T1c     PASS  hodos_tests            181 tests, 180 passed, 1 skipped
T1d     PASS  frontend build (-Full)

PREFLIGHT: PASS - all checks ran and passed.                exit 0
```

Negative control re-run at the **new** baselines: all 5 gates seen to fail
(`1>0`, `6>5`, `1>0`, `1>0`, `1>0`), probes cleaned up. `G1` and `G5` detecting at `1 > 0` is
strictly stronger than the old `53 > 52` / `16 > 15` — at a baseline of 52 a single new violation was
a rounding error in the count.

### T1c closed — 2026-08-18

`hodos_tests` now builds and runs: **176 tests, 175 passed, 1 skipped**
(`UpdateStagerRig.StagesFromLocalFeed`). Build:

```
cmake --build cef-native/build --config Release --target hodos_tests
```

⚠️ Two traps found doing it, both worth keeping:

1. **The binary lands in `cef-native/build/bin/Release/`, not `build/tests/Release/`** as
   `cef-native/tests/CMakeLists.txt`'s own header comment states. Preflight now probes four candidate
   paths — a hard-coded wrong path reads as "not built" and **SKIPS**, which is silent loss of
   coverage wearing the costume of a clean run.
2. **`cmake --build … | tail` discards the exit code** and a failed configure reported success. Any
   build or test invocation whose result is piped must capture `$?` separately. This is the same
   false-green family as the four in §9.

Configuring a *fresh* build dir hits `nlohmann-json 3.12.0#2` missing from the local vcpkg registry
(`VCPKG_MANIFEST_MODE=ON`). The existing `cef-native/build` has `VCPKG_MANIFEST_MODE=OFF` and already
carries `HODOS_BUILD_TESTS=ON`, so it builds. ⇒ **Use the existing build dir**; a clean-machine
bootstrap needs a vcpkg registry new enough for the pinned port-version — a real instance of the
freeze-with-no-thaw problem in `TICKET_dependency_freshness_review.md`.
