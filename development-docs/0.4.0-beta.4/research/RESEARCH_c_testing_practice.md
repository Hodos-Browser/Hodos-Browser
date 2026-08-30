# Research (c) — test harnesses, standing regression sets, and testing practice

**Researched:** 2026-08-29 · **For:** v0.4.0-beta.4 scoping · **Status:** research input, no code or plan changed

**Bottom line.** The beta.3 harness is unusually good at the thing it was built for — it demands an
*observed* failure and a *named subject* per assertion, and it verifies its own static gates can fail
— and it is unusually bad at the thing that actually determines whether a regression set works:
none of it runs unless a person chooses to run it. The project has hand-built, one file at a time,
the exact mechanism that mutation testing automates (T1e/T1f/T1g each rewrite a real source file to
reintroduce a known bug and assert the check notices). For Rust that mechanism exists off the shelf,
diff-scoped, and costs a CI job: `cargo-mutants --in-diff`. For C++ the equivalent is not worth
building here. The larger finding is that the standing regression set is not a standing set — three
of its six rows have been "owed, not waived" across every boundary, and four of six boundary rows are
blank — because four of six rows require a live app and a human and nobody budgeted the minutes.
The fix is not more discipline; it is to split the set into a small automatable tier that always runs
and a scheduled, timeboxed human session with a session sheet, and to wire the money-path checks to
the real browser process over CDP so they stop needing a human at all. Separately: the six Playwright
specs in `frontend/e2e/` run stock Chromium against a mocked bridge and are wired into neither
preflight nor CI — by the harness's own SUBJECT rule they prove nothing about the shipping browser,
and should be either re-pointed at the real binary or relabelled.

---

## 0. What is actually in the repo (measured, 2026-08-29)

Stated because several recommendations below depend on it, and two of these are findings in their own
right.

| Thing | State |
|---|---|
| `scripts/preflight.ps1` | 6 T0 gates (G1,G2,G3,G4,G5,G8) + T1a–T1g. Local only. `-NegativeControl` injects a probe per gate. Exit 0/1/2. |
| `cargo test` | ~601 `#[test]`/`#[tokio::test]` across `rust-wallet` + `adblock-engine`. No proptest, no quickcheck, no `arbitrary` in any `Cargo.toml`. |
| `hodos_tests` (C++) | 21 test files, GoogleTest via `FetchContent` (tag `v1.14.0`), ~181 tests. Pure-logic only by design; nothing that needs CEF. |
| C++ under test | 107 `.cpp/.h/.mm` files, ~35k lines in `cef-native/src` alone. So `hodos_tests` covers the pure-logic slice, not the shell. |
| Frontend unit tests | **None.** `package.json` has no unit test script — `"test": "playwright test"`. |
| `frontend/e2e/*.spec.ts` | 6 specs. `playwright.config.ts` uses `browserName: 'chromium'` against `http://localhost:5137`, and `smoke.spec.ts` injects `BRIDGE_MOCK_SCRIPT`. **Not referenced by `preflight.ps1`. Not referenced by `test.yml`** (the `frontend:` job is a commented stub). |
| `retries: 1` | Already set in `playwright.config.ts`. Nothing records what retried. |
| `test.yml` | Blocking: `cargo test` ×2 legs + adblock + the F8 secret-log grep. Advisory (`continue-on-error: true`): clippy, `cargo-audit`, `npm audit`. Staged/commented: C++ ctest matrix, frontend, coverage. Push trigger suspended since 2026-08-17 on Actions quota; re-enable ~2026-09-01. |
| `ci.yml` | `pull_request` + push to `main`. Comment says "once green, set as a REQUIRED status check" — no evidence it is one yet. |
| Coverage | Two contradictory statements in the repo: `scripts/test-all.ps1:60` runs `cargo tarpaulin`; `test.yml:209` says "cargo-llvm-cov (**NOT** tarpaulin)". Neither runs today. |
| Sanitizers | No `/fsanitize`, no `-fsanitize`, nothing in any CMakeLists. |

Two of these deserve to be findings, not inventory:

⛔ **The Playwright suite fails the harness's own SUBJECT test.** `HARNESS.md` §2 asks "which process,
which browser, which binary". Answer for `frontend/e2e/`: a stock Playwright Chromium, not the Hodos
CEF shell; a mocked `window.__hodos_*` bridge, not the real IPC; a dev server, not the packaged app.
That is a legitimate way to test React components — it is not browser coverage, and it is currently
filed as if it were. It is also dead code: no runner in the repo invokes it.

⛔ **CI has been dark since 2026-08-14 and the harness knows it, but the sprint plan does not price
it.** `HARNESS.md` §3 marks T0 "CI dark until ~2026-09-01 — local only". That date is three days from
now. beta.4 scoping is the moment to decide what becomes a *required* check the day the quota
returns, otherwise the local-only arrangement calcifies.

---

## 1. Ranked adoptions

Ranked by (value here) ÷ (cost here). Each states what it catches that the current harness structurally
cannot.

### 1. `cargo-mutants`, diff-scoped, starting with `hodos_permission_engine` — **adopt**

**What it is.** Applies small mutations to Rust source (swap operators, replace a function body with
`Default::default()`, invert conditions), rebuilds, and runs the test suite per mutation. A mutation
that survives is a line whose behaviour no test observes. This is precisely the project's stated
question — "can I make this test pass with the feature removed?" — asked mechanically for every
function. ([cargo-mutants book](https://mutants.rs/), [Thoughtworks Radar
entry](https://www.thoughtworks.com/radar/tools/cargo-mutants))

**Cost here.** `taiki-e/install-action` fetches a prebuilt binary; one CI job. Do **not** run the full
suite: use `--in-diff` against the PR base plus `--in-place` (skips the source-tree copy), which the
tool's own CI guidance recommends over full runs
([mutants.rs/ci.html](https://mutants.rs/ci.html)). Start with
`rust-wallet/crates/hodos_permission_engine` — 73 tests over pure decision logic, hermetic, no I/O,
which is the ideal shape. Expect minutes per PR, not hours.

**What it catches that the harness cannot.** T1e/T1f/T1g are hand-written negative controls: each
rewrites one real source file to reintroduce one known bug. Excellent, and O(1 human day) per
invariant — which is why there are three of them and not thirty. `cargo-mutants` generates that class
of probe for every function automatically, including the ones nobody thought to doubt. It also finds
the reverse case: tests that exist but assert nothing load-bearing.

**Caveats to write into the contract, not discover later.**
- It does **not understand `#[cfg]`**, and reports functions for other platforms as untested
  ([limitations](https://mutants.rs/limitations.html)). `rust-wallet` has `#[cfg(windows)]` DPAPI
  auto-unlock and a non-Windows stub — expect noise there and exclude or annotate it.
- It requires a hermetic suite. Any test touching the real filesystem or a port will produce garbage
  conclusions.
- It cannot see C++ or TypeScript. It is a Rust answer only.

**Worth it: yes, highest value in this list.** The evidence that this is the right shape rather than a
fashion is Google's, not a vendor's: full-codebase mutation testing was unusable at their scale, and
what worked was *diff-scoped, filtered* mutants surfaced during code review — deployed across 1,000+
projects and 24,000+ developers ([Practical Mutation Testing at Scale: A view from
Google](https://arxiv.org/abs/2102.11378)). Adopt the diff-scoped form; do not attempt a full run.

### 2. `necessist` as a one-off audit of the Rust test suite — **adopt, not as a gate**

**What it is.** Trail of Bits' tool that iteratively removes statements and method calls **from the
tests** and reruns them. A test that still passes with a statement deleted is a test whose statement
did nothing — often the assertion, or the setup that made the assertion meaningful. Supports Rust and
Vitest ([trailofbits/necessist](https://github.com/trailofbits/necessist); paper at Mutation 2024).

**Cost here.** One run, offline, over ~601 tests. Slow (it is O(statements) × suite time), so run it
once per sprint on a spare machine, not in CI.

**What it catches that the harness cannot.** `cargo-mutants` mutates production code and asks "does a
test notice?". `necessist` mutates the *test* and asks "does this test do anything?". These find
different defects. The four void farbling harnesses are the second kind — the tests existed and ran
and asserted something that was true regardless. This tool is aimed exactly there.

**Worth it: yes, cheaply.** Treat the output as a to-read list, not a gate.

### 3. Point the e2e suite at the real binary over CDP — or relabel it — **do the relabel now, scope the CDP work as a phase**

**What it is.** CEF speaks the Chrome DevTools Protocol. Launch the Hodos shell with
`--remote-debugging-port=0`, read the assigned port, and attach with
`chromium.connectOverCDP()`. Playwright then drives the *real* process, the real renderer, the real
IPC bridge — no mock ([Playwright issue #10927 on CEF +
connectOverCDP](https://github.com/microsoft/playwright/issues/10927); the practical write-up is
[Automating Custom Browsers with
Playwright](https://changjoon-baek.medium.com/automating-custom-browsers-with-playwright-a4e0158d0530)).

**Cost here.** Real. It needs: a debug-build flag that exposes the port (and must be *impossible* in
release — this is a remote code execution surface on a wallet), per-test profile isolation via a
scratch `--user-data-dir`, and a launcher that waits for the port. Call it one beta.4 phase, not a
side task. Known limitation: Playwright's browser-context management is unsupported over a CEF CDP
connection, so tests must be written page-oriented, one profile per run.

**What it catches that the harness cannot.** This is the only path by which `R-GOLD`, `R-COUNT` and
the UI half of `R-INTEXT` stop needing a human. Those three are the checks recorded NOT RUN at every
boundary, and they are the three that guard the money path. Automating them converts "owed" into
"ran".

**Worth it: the CDP work, yes — as a scoped phase with an explicit security review of the debug port.
The relabel, immediately and for free:** either delete `frontend/e2e/` or add a header to
`playwright.config.ts` and the spec files stating plainly that these run stock Chromium against a
mocked bridge and are not evidence about the shipping browser. Under §8's honesty rule an unlabelled
suite of this kind is a standing false green waiting for someone to cite it.

### 4. ASan on `hodos_tests` only — **adopt**

**What it is.** Build the C++ unit target a second time with MSVC's `/fsanitize=address` and run it.
Supported in MSVC since VS2019 16.9 ([Microsoft
Learn](https://learn.microsoft.com/en-us/cpp/sanitizers/asan)); ARM64 support arrived in VS2026.

**Cost here.** One extra CMake configuration of an existing, already-building target. ~2× runtime on
181 tests, which is nothing. No CEF involvement — `hodos_tests` is deliberately pure-logic, which is
exactly the code ASan can instrument cleanly.

**What it catches that the harness cannot.** Every T0 gate in this project is a regex over source
text, and every T1 C++ test asserts on return values. Neither can see a heap overflow or a
use-after-free. The code under test includes a hand-rolled JS string escaper, URL/origin anchoring,
QR payload classification, and physical→view coordinate maths — buffer-arithmetic code, in a process
that handles keys. Sanitizers "miss paths you do not execute, but they catch real dynamic failures
with concrete values, stacks, allocations".

**Worth it: yes for `hodos_tests`. No for the CEF shell** — mixing instrumented code with the prebuilt
CEF binaries is where ASan becomes a support burden, and the ABI pin (VER-3) makes toolchain
experiments expensive. Add UBSan later if it is free; do not block on it.

### 5. Give the advisory CI legs an expiry date and a ratchet — **adopt**

**What it is.** Three checks in `test.yml` are `continue-on-error: true` with a comment promising they
will become blocking "after a cleanup chunk": clippy (~198 pre-existing warnings), `cargo-audit`, and
`npm audit`. Advisory checks are legitimate practice; advisory checks with no date are noise that
trains people to skim the log.

**What to do, concretely.**
- **clippy:** do not flip to `-D warnings`. Apply the mechanism this project already invented and got
  right — a ratcheted baseline. 198 today, fails at 199, lowered by deliberate commit. That gets the
  negative control (a *new* warning always fails) without a cleanup sprint first.
- **`cargo audit`:** the current form is `cargo audit || true` inside a `continue-on-error` step —
  two independent suppressions of the same signal. Replace with `cargo deny check advisories`, which
  is the CI-gating tool of the pair (cargo-audit is advisories-only; cargo-deny adds bans, sources,
  licences and, critically, a **written allowlist**). An allowlist entry with a reason is a decision
  with an owner; `continue-on-error` is not. Note the trap: plain `cargo audit` **exits 0 while
  reporting findings** unless `--deny warnings` is passed — a false green of exactly the family §9
  catalogues. ([cargo-deny advisories
  config](https://embarkstudios.github.io/cargo-deny/checks/advisories/cfg.html),
  [RustSec](https://rustsec.org/))
- **`npm audit`:** a poor gate — it reports transitive dev-dependency advisories with no reachability
  analysis, which is why it is currently `|| true`. If a frontend dependency gate is wanted, use
  [OSV-Scanner](https://github.com/google/osv-scanner), which matches against ecosystem-native version
  ranges rather than fuzzy ranges. Otherwise drop the step rather than keep a check nobody reads.

**Worth it: yes.** This is a day's work and it converts three decorative steps into two real gates and
one deliberate deletion.

### 6. `cargo-nextest` as the runner — **adopt**

**What it is.** A drop-in replacement for `cargo test` with per-test process isolation, per-test
timeouts, `--partition` sharding, JUnit output, and `--retries` that marks a test **FLAKY** rather
than silently passing it ([nexte.st retries](https://nexte.st/docs/features/retries/),
[partitioning](https://nexte.st/docs/ci-features/partitioning/)).

**Cost here.** Near zero. One line in `preflight.ps1`'s `Invoke-CargoTest` and one in `test.yml`.

**What it catches that the harness cannot.** Process isolation surfaces tests that accidentally share
process-global state — and `rust-wallet` has plenty (DPAPI handles, `flexi_logger` global init, data
directory paths, env). Under `cargo test` those tests pass because they run in a fixed order in one
process. It also gives the project its first actual flake signal.

**Important:** use `--retries` in **CI only, never in `preflight.ps1`**, and treat a FLAKY result as a
finding to triage, not as a pass. A retry that is not recorded is the same instrument as a harness
that prints PASS on zero executed checks.

### 7. Property-based testing (`proptest`) for three or four named invariants — **adopt narrowly**

**What it is.** Generate inputs from a strategy, assert a property, shrink on failure. `proptest` is
the mainstream Rust choice; its strategy-per-value model (vs quickcheck's one-generator-per-type)
matters as soon as you need constrained inputs, which you will here
([proptest-rs/proptest](https://github.com/proptest-rs/proptest),
[Proptest vs QuickCheck](https://altsysrq.github.io/proptest-book/proptest/vs-quickcheck.html)).

**Where it earns its cost in this codebase — name these, do not adopt a policy:**
- **The permission decision matrix.** Matrix C is a pure function from (origin, operation, caps,
  session state) → decision. A model-based/state-machine property ("a sequence of spends can never
  exceed the session cap"; "sensitive cert fields prompt under every input") is dramatically stronger
  than 73 example tests, and it is the check `R-PERIM` actually wants. Note the standard caution: for
  stateful systems, generate *sequences of operations with state in the generator*, or you produce
  mostly meaningless operations.
- **BIP21 / QR payload classification.** Parsers are the canonical property-test target — round-trip
  (`format(parse(x)) == x`) plus the security invariant that no input outside the scheme allowlist
  ever yields an address. This is where the `slice(8)` bug lived.
- **Cap arithmetic.** Satoshi↔USD conversion and cap comparison: no overflow, monotonic, never
  negative.
- **Farbling seed derivation.** Same eTLD+1 ⇒ same seed; different eTLD+1 ⇒ different seed. The
  constant-seed bug that shipped in every release is a property, and it is trivially expressible as
  one.

**Where it does not earn its cost:** anything involving a real process, IPC, the UI, or the update
path. Do not property-test plumbing.

**Worth it: yes for those four, no as a general policy.** One non-obvious payoff: property tests kill
far more mutants per line than example tests, so adopting #1 and #7 together is worth more than
either alone.

### 8. Restructure the standing set into R-EVERY and R-RELEASE — **adopt; this is the structural fix**

See §2 for the critique. The concrete change:

| Bucket | Contents | When |
|---|---|---|
| **R-EVERY** | Only checks that are automatable and cheap: `R-PERIM` T1 (engine), `R-UPDATE` T1, the T0 gates, and — once #3 lands — `R-INTEXT`, `R-GOLD`, `R-COUNT` over CDP. | Every boundary, in `preflight.ps1`/CI, unattended. |
| **R-RELEASE** | The irreducibly human ones: `R-CLOSE` (three close paths, native file dialog, per flag), the DPI matrix, multi-monitor, overlay input. | One scheduled, **timeboxed** session per sprint, with a written session sheet and a named owner. |

The evidence for the split is in the project's own record: `R-GOLD`, `R-CLOSE` and `R-COUNT` are
marked "**owed, not waived**" and have been owed across every boundary attempted. A check whose modal
state is "owed" is not a standing check; it is an intention. Moving it to a scheduled session with a
timebox is not a lowering of standards — it is the difference between a check that runs four times a
release and one that runs zero.

Attach two rules that current practice supports and this project does not yet have:
- **A row does not enter the regression set until it has been run green *and* red once, with a date.**
  `R-INTEXT` was **unrunnable as written for its entire life** (its SUBJECT demanded a Rust log line
  that did not exist until 2026-08-26). That is the same class as `HARNESS.md` §9's own rule:
  "baseline with the tool that will do the measuring". Extend the rule from gates to regression rows.
- **A runtime budget per bucket, in minutes, written down.** There is no runtime figure anywhere in
  `HARNESS.md` or `REGRESSION_SET.md`. Without a budget the only lever anyone has when time is short
  is skipping, and skipping is what the record shows.

### 9. Coverage as a diff signal, never a threshold — **adopt the signal, drop the thresholds**

`test.yml`'s staged coverage job carries `crypto>=90, certificate>=80, general>=60`. Drop those
numbers. Coverage thresholds are the textbook Goodhart case: when the number becomes the target,
teams add trivial tests that assert nothing to move it, which is *the exact defect this project keeps
shipping*. Coverage tells you what you definitely have not tested; it says nothing about whether what
ran was checked.

Publish instead, to `$GITHUB_STEP_SUMMARY`: **new/changed lines in this diff with no test execution**.
That is actionable and ungameable in the same way. Use `cargo-llvm-cov`, not tarpaulin — and fix the
contradiction: `scripts/test-all.ps1:60` runs `cargo tarpaulin` while `test.yml:209` says "NOT
tarpaulin". Two scripts in one repo giving opposite instructions is a small instance of the drift
problem HARNESS_DELTA.md §preamble is about.

### 10. Add a "what is this test allowed to do" column to the tier table — **adopt, cheap**

T0–T4 is a good taxonomy and better than most teams have. Its one flaw is that it mixes *what the test
is* with *who runs it*: T2 is "integration", T3 is "a human at the machine". The dimension that
actually predicts flakiness, runtime, and CI-eligibility is what the test is permitted to touch —
Google's small/medium/large is defined by exactly that: "not by its number of lines of code, but by
how it runs, what it is allowed to do, and how many resources it consumes"
([SWE at Google, ch. 11](https://abseil.io/resources/swe-book/html/ch11.html)).

Keep the T0–T4 names; add one column:

| Tier | Allowed to |
|---|---|
| T0 | read source text only |
| T1 | one process, no network, no filesystem outside a temp dir |
| T2 | one machine, loopback only, scratch profile |
| T3 | a real display, real input, a human |
| T4 | a build host, signing material, real installers |

Then "can this run in CI?" is a property of the row, decided when it is written, rather than a
negotiation at the boundary. Chromium runs the same discipline for the same reason: prefer
`unit tests > browser_tests > interactive_ui_tests`, and use interactive UI tests "only if they're
really necessary" — focus, blocking UI, drag-and-drop ([Chromium browser
tests](https://www.chromium.org/developers/testing/browser-tests/)). That maps almost exactly onto
T1/T2/T3 here, which is reassuring: the tiering is right, only the constraint is unstated.

### 11. A written flake policy — **adopt, one paragraph**

`HARNESS.md` and `REGRESSION_SET.md` do not mention flaky tests at all, while `playwright.config.ts`
already sets `retries: 1`. That means the project is silently retrying and keeping no record of what
retried — structurally the same as the "zero checks ran, reported PASS" defect in §9.

The industry has converged on detect → quarantine → dashboard → SLA. At this project's size you do
not need Trunk or Datadog; you need three sentences:
1. A test that fails and then passes on retry is recorded **FLAKY**, never PASS.
2. A test flaky twice is disabled with a `TODO(#issue)` and listed in a quarantine block in
   `HARNESS.md`, with a date.
3. A test quarantined more than one sprint is deleted or fixed — not left.

Chromium's own guidance is worth borrowing on the second point: do not disable on a single failure
(you cannot distinguish flake from regression), but once flaky, the test "contributes almost no signal
and might make it impossible to tell if a failure is something new" ([Chromium
sheriffing](https://chromium.googlesource.com/chromium/src/+/80.0.3987.87/docs/sheriff.md),
[Chromium Chronicle #2](https://developer.chrome.com/blog/chromium-chronicle-2/)).

### 12. Make `ci.yml` a required check the day the quota returns — **adopt, zero engineering cost**

`ci.yml` already exists and its own header says it should become a required status check. Until it is
one, every gate above is advisory by construction: a green local `preflight.ps1` is a claim by the
author about a run nobody else saw. This is a branch-protection setting, not a project.

---

## Explicitly not worth it

**Mull (mutation testing for C++).** Mull works on LLVM IR via a clang plugin
([mull-project/mull](https://github.com/mull-project/mull)). This project is pinned to a specific MSVC
toolset for CEF ABI compatibility (VER-3, called out in `test.yml`'s own comments). Standing up a
parallel clang toolchain to mutate 35k lines of C++ is a sprint of work for a signal you can get more
cheaply: `hodos_tests` covers ~21 pure-logic units, and the project already does the manual equivalent
well — `logger_concurrency_test.cpp`'s comment records that its negative control disables the lock
**on the same binary**, which is the right instinct executed by hand. Keep doing that. Do not build a
second toolchain.

**Stryker for the frontend.** [Stryker's incremental mode](https://stryker-mutator.io/docs/stryker-js/incremental/)
is genuinely good and would be the right tool *later*. Today there are zero frontend unit tests to
mutate; mutation testing against an empty suite reports "everything survives" and tells you what you
already know. Revisit after frontend unit tests exist.

**Coverage gates.** Covered above. No.

**FlaUI / Appium-style desktop UI automation.** WinAppDriver is effectively dead — last stable release
November 2020, v1.3 stuck as a July 2020 RC. FlaUI is the live option and is fine for Win32/WPF, but
it drives the UI Automation tree, and the ~15 Hodos overlays are **windowless CEF browsers** — they
have no meaningful UIA tree to drive. So FlaUI would buy you the shell chrome and none of the surfaces
where the defects actually are (overlay input, DPI conversion, close guards). CDP (#3) reaches the web
content; a human reaches the rest. There is no third option worth paying for here.

**A commercial flaky-test platform.** At ~800 tests, a text file and three rules (#11) is enough.

**Mutation testing on every commit.** The tool's own CI guidance and Google's deployment both point
the same way: diff-scoped only. A full run is a research activity, not a gate.

---

## 2. Honest critique of `HARNESS.md` / `REGRESSION_SET.md`

### What it does better than typical practice

These are not compliments for form; each is genuinely rare.

- **The RED column.** Requiring an *observed* failure per assertion — "the run you actually did, and
  its result", not "this would fail if broken" — is manual mutation testing applied at assertion
  granularity. Almost no team does this. It is the single best idea in the document.
- **The SUBJECT column.** Naming process, browser, binary and build type is the defence against the
  most common way a green test lies. It is also, ironically, the rule the project has not applied to
  its own Playwright config (§0).
- **Two-sided invariants as mutual controls.** `R-INTEXT`'s "internal never prompts / external always
  gates", each half being the other's negative control, is a better formulation than a coverage
  target could ever produce. It is essentially a hand-written property.
- **Ratcheted gates with a `-NegativeControl` mode that injects a probe and asserts the count rises.**
  Verifying that your own lint gates can fail is vanishingly rare. It is also what caught G1 reading
  "1 violation" against a real 52.
- **INCOMPLETE ≠ PASS, exit 2.** Correct, unusual, and the right hill.
- **§9's record of the harness's own four false-green defects.** Most teams fix these quietly and
  delete the evidence. Keeping them is why the same class keeps getting caught.
- **The `⭐ deny not approve` note in the 2→3 run log** — recognising that approving `example.com`
  would make every future run of `R-INTEXT` vacuous. That is a real understanding of test decay, and
  it is more sophisticated than most published regression-suite guidance.

### Where it is weak

- **It is entirely local and entirely human-triggered.** Every gate, every negative control, every
  boundary sweep runs when a person types a command. There is no unattended execution of any of it,
  and CI has been dark since 2026-08-14. The harness's rigour and its fragility are the same property:
  it depends on someone choosing to be rigorous, every time, under deadline. This is the single
  largest gap between this document and current practice, and it is fixable in a day (#12).
- **The negative controls are bespoke and therefore rationed.** T1e, T1f and T1g each hand-rewrite one
  real source file to reintroduce one specific known bug. That is exactly the right idea. It is also
  why there are three, covering three defects that had already been found. The generative version —
  which mutates functions nobody suspected — exists and is one CI job (#1). The harness identified the
  right mechanism and then built it the expensive way.
- **Grep gates are line-based, and the document already knows this is a ceiling.** §9 records a false
  green caused purely by a wrapped log line. That is not a bug to fix; it is a structural limit of
  regex-over-source. Two gates are *type* problems wearing text costumes: G8 (raw physical coordinate
  assigned to a `CefMouseEvent`) and G2 (unanchored origin substring). The durable answer to G8 is a
  newtype — distinct `ClientPoint` and `ViewPoint` types so the assignment does not compile — after
  which the gate is the compiler and cannot be defeated by formatting. Recommend that explicitly; a
  gate whose baseline is 0 and whose target is 0 and whose only failure mode is a wrapped line is a
  gate ready to be retired into the type system.
- **The standing regression set is aspirational, not standing.** Six rows; four require a live app and
  most a human. The boundary record shows: four of six boundary rows entirely blank; at the one
  boundary worked in earnest, three of six checks NOT RUN, and the doc says plainly they are "owed,
  not waived" — across every boundary. That is a set which trains its readers to discount it, which is
  the failure mode the harness exists to prevent. See #8.
- **`⬜ deferred to P0.5 (owns this boundary)` inverts the point of a standing set.** A regression check
  is precisely the thing you run when *another* phase touches the area. Deferring it to the phase that
  owns the code makes it a phase acceptance test, not a regression check.
- **`R-INTEXT` was unrunnable as written for its entire existence.** Its SUBJECT demanded a Rust log
  line that no code emitted until 2026-08-26. The doc records this honourably, but the lesson is
  structural: writing a check is not the same as having one. A row must be executed once — green and
  red — at the moment it is written.
- **No runtime budget anywhere.** Not for `preflight.ps1`, not for `hodos_tests`, not for a boundary
  sweep. Budgets are the mechanism that keeps a suite from rotting; without one, the only available
  response to time pressure is to skip, and the record shows skipping.
- **No flake policy at all**, while `retries: 1` is already configured. See #11.
- **The frontend is a hole with load-bearing logic in it.** `manifestConsent.ts` (BRC-73 caps) and
  `bip21.ts` (payment URIs) are money-path code. Their only tests are bespoke `.mjs` probe harnesses
  living under `development-docs/0.4.0-beta.3/phase-*/probes/` — outside the source tree, outside any
  test runner, discoverable only by reading `preflight.ps1`. They are good tests in the wrong place.
  Move them into a real frontend unit runner (Vitest) next to the code, and keep the
  `--negative-control` mode as a flag.
- **The tier table conflates what a test is with who runs it.** See #10.

---

## 3. Making "how will this be tested" a checkable scoping output

The current process gets the *order* right — `PHASE_CONTRACT.md` is written before the first line of
code — and the *content* wrong: at scoping time the evidence table's RED and SUBJECT cells are
aspirational prose, and its Tier cell is often the last thing filled in. The result is predictable and
visible in the record: rows whose mechanism turns out to be "a human, a real payment, and an hour" get
written down at scoping and discovered at sign-off, at which point the only options are slip or skip.

Five changes, in order of value:

**1. At scoping, every acceptance row must name its Tier and its *mechanism*, and the mechanism must
be a thing that exists.** Not "verify the pill appears" but "T2, CDP script `pill.spec.ts`" or "T3,
human, DPI matrix, 20 min". A row whose mechanism is "by hand, TBD" is a row that will read NOT RUN at
the boundary. Making this a scoping-gate field means the cost is visible while the phase can still be
resized, which is the whole point of scoping.

**2. Sum the T3 minutes for the sprint and put the number in `SPRINT_PLAN.md`.** beta.3's evidence is
that unbudgeted human testing does not happen. If the sum exceeds roughly an hour per boundary, the
standing set is too large — cut it, automate it, or move it to R-RELEASE (#8). Do not write down an
hour you have not scheduled.

**3. Add one question to the phase contract: "which existing check would have caught this, and why
didn't it?"** For a bug-fix phase the answer *is* the new regression row, and it is usually more
precise than anything invented from scratch. For a greenfield phase the honest answer is "none", which
is itself the argument for adding a row. This costs a sentence and prevents the most common failure —
fixing a defect and adding a test that would have passed before the fix.

**4. Make unbacked rows a command, not a reading exercise.** §7 already gives every row an ID that is
cited in the commit that satisfies it — a genuinely good design that nothing currently exploits. Add a
`-Contracts` mode to `preflight.ps1` that walks every `PHASE_CONTRACT.md` and reports: rows with an
empty Tier, rows with an empty RED or SUBJECT, rows whose Result is still `⬜`, and row IDs that appear
in no commit message. Then "did we meet our standards" is `pwsh scripts/preflight.ps1 -Contracts`, and
it can be a CI job. This is a small script and it converts the document's best structural idea into an
enforced one.

**5. Make a blank boundary row an INCOMPLETE.** `REGRESSION_SET.md`'s boundary table has four of six
rows entirely empty, and nothing in the process notices. Apply the harness's own §8 rule to itself:
an empty cell is SKIPPED, and a boundary with any SKIPPED cell is INCOMPLETE, not passed. If that is
too strict to be honoured, the standing set is the wrong size — which is information worth having.

---

## Sources

Established practice / primary:
- [Practical Mutation Testing at Scale: A view from Google](https://arxiv.org/abs/2102.11378) — Petrović & Ivanković et al., TSE 2021. Diff-scoped, filtered mutation testing deployed to 1,000+ projects; the evidence that full-codebase mutation testing does not scale and diff-scoped does.
- [Software Engineering at Google, ch. 11 — Testing Overview](https://abseil.io/resources/swe-book/html/ch11.html) — test *size* (what a test is allowed to do) rather than test *type*.
- [Chromium: Browser Tests](https://www.chromium.org/developers/testing/browser-tests/) and [Testing in Chromium](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/testing/testing_in_chromium.md) — prefer unit > browser > interactive UI tests; interactive UI tests only for focus, blocking UI, drag-and-drop.
- [Chromium Chronicle #2 — Fighting Test Flakiness](https://developer.chrome.com/blog/chromium-chronicle-2/) and [Chromium Sheriffing](https://chromium.googlesource.com/chromium/src/+/80.0.3987.87/docs/sheriff.md) — flake policy, why a flaky test contributes almost no signal.
- [Chromium Chronicle #10 — Pixel Tests](https://developer.chrome.com/blog/chromium-chronicle-10/) — disable animation, mock data, minimum surface area.
- [MSVC AddressSanitizer](https://learn.microsoft.com/en-us/cpp/sanitizers/asan) — ASan on Windows/MSVC, supported since VS2019 16.9.

Tools (primary documentation):
- [cargo-mutants book](https://mutants.rs/) · [CI guidance](https://mutants.rs/ci.html) · [limitations](https://mutants.rs/limitations.html) · [Thoughtworks Technology Radar](https://www.thoughtworks.com/radar/tools/cargo-mutants)
- [trailofbits/necessist](https://github.com/trailofbits/necessist) — statement-removal mutation of tests; Rust + Vitest support.
- [cargo-nextest: retries and flaky tests](https://nexte.st/docs/features/retries/) · [partitioning/sharding](https://nexte.st/docs/ci-features/partitioning/)
- [proptest](https://github.com/proptest-rs/proptest) · [Proptest vs QuickCheck](https://altsysrq.github.io/proptest-book/proptest/vs-quickcheck.html)
- [cargo-deny advisories config](https://embarkstudios.github.io/cargo-deny/checks/advisories/cfg.html) · [RustSec advisory DB](https://rustsec.org/) · [OSV-Scanner](https://github.com/google/osv-scanner)
- [StrykerJS incremental mode](https://stryker-mutator.io/docs/stryker-js/incremental/)
- [mull-project/mull](https://github.com/mull-project/mull) — C/C++ mutation testing via LLVM IR.
- [Playwright #10927 — connectOverCDP against CEF](https://github.com/microsoft/playwright/issues/10927)

Secondary / one team's write-up (treated as such, not as established practice):
- [Automating Custom Browsers with Playwright](https://changjoon-baek.medium.com/automating-custom-browsers-with-playwright-a4e0158d0530) — practical CEF + `connectOverCDP` walkthrough.
- [An Introduction to Property-Based Testing in Rust](https://www.lpalmieri.com/posts/an-introduction-to-property-based-testing-in-rust/) — Palmieri.
- [Rust Project Primer — Mutation Testing](https://rustprojectprimer.com/testing/mutations.html) and [Property Testing](https://rustprojectprimer.com/testing/property.html)
- [The test pyramid and its discontents](https://www.qase.io/blog/the-test-pyramid-and-its-discontents/) — useful summary of why teams disagree about the shape.
- WinAppDriver status (last stable v1.2.1, Nov 2020; v1.3 stuck as a 2020 RC) is reported consistently across vendor comparison posts rather than by Microsoft; treat the *conclusion* (do not start new work on it) as solid and the details as secondary. [FlaUI](https://github.com/FlaUI/FlaUI.WebDriver) is the live open-source alternative.
