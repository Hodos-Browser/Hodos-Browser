# Testing Strategy — Canonical (cross-stack)

**Created:** 2026-06-16 · **Owner:** DevOps/CI-CD · **Canonical home:** `development-docs/DevOps-CICD/`
**Per root CLAUDE.md Invariant #12** — keep current; append to §13 Lessons Learned.

> **This is the ONE testing strategy.** The HelicOps audit's "coverage gaps" and the pipeline's "CI test gate" are the **same problem** — solved once, here. Layer-specific test *details* live in each layer; this doc is the cross-stack strategy + the rules CI enforces.
> **Relationship to other docs:** `BUILD_AND_RELEASE.md` §5 holds the actual CI/release **workflow** (today PLANNED — no `ci.yml` exists); this doc defines **what** that workflow runs and **why**. `DEPENDENCY_VERIFICATION.md` is the per-CEF-bump dep checklist. Master plan items: **PIPE-A7** (this strategy), **PIPE-CI** (build `ci.yml`), **PIPE-TESTGATE** (gate release), **TEST-HARNESS** (capped live e2e), **AUDIT-F8** (secret-log gate).

---

## 1. Core principle — co-locate files, centralize orchestration
- **Test FILES stay with the code** they test (move with the code, clear ownership):
  - Rust: `rust-wallet/tests/` + inline `#[cfg(test)]`; `adblock-engine/` inline.
  - C++: `cef-native/tests/` (GoogleTest, opt-in `-DHODOS_BUILD_TESTS=ON`).
  - Frontend: `frontend/` (Vitest — to add) + `frontend/e2e/` (Playwright).
- **Orchestration is centralized:** one runner (`scripts/test-all.ps1`), one reusable CI workflow (`.github/workflows/`), and this strategy doc. **Do NOT** physically gather test files into one folder.

## 2. Current census (verified 2026-06; replaces the inflated "780+" claim)
| Layer | Tests today | Gap |
|-------|-------------|-----|
| Rust (wallet) | ~480 (incl. crypto/signing vectors) | strong; keep |
| Adblock | 23 inline | ok |
| C++ (cef-native) | 39 in 2 GoogleTest files (ManifestFetcher, SensitiveCertFields), opt-in | expand to real targets (PaidContentCache, AdblockCache) |
| Frontend unit (Vitest) | **0** | **inverted pyramid — add a thin layer** |
| Frontend e2e (Playwright) | 54 | ok; Windows run unverified |
| **CI enforcement** | **none** — no `ci.yml`; `release.yml` runs zero tests | **the #1 gap** |

## 3. What to test at each layer (the pyramid, right-side-up)
- **Rust = the heavy base.** Unit + integration, with **real BSV/BRC vectors** for `crypto/` (derivation, signing, BEEF, BRC-121 wire format). This is where correctness lives — keep it deep.
- **C++ = targeted unit** on pure logic (parsers, caches, classifiers) — not UI. Expand opt-in GoogleTest onto real targets.
- **Frontend = thin Vitest** for the logic that *does* live in the client (formatters, validators, `DomainPermissionForm` validation, hooks) + **Playwright e2e** for flows. Don't chase coverage on presentational components (Testing Trophy: integration-leaning over isolated unit for UI). The inverted pyramid is acceptable *only* because logic lives in Rust — but the thin Vitest layer closes the gap cheaply.
- **Cross-stack = smoke + capped live e2e** (§9, §10).

## 4. CI gating model (how tests block merges & releases)
- **One reusable test workflow** (`on: workflow_call`) with jobs: `rust` (test + clippy `-D warnings`), `adblock`, `cpp` (ctest, `-DHODOS_BUILD_TESTS=ON`, matrix Win+Mac), `frontend` (vitest + playwright), `security` (cargo audit + npm audit + secret-log gate §8).
- **`ci.yml`** (`on: pull_request`) calls it → set as a **required status check** on the protected branch ⇒ **can't merge red.**
- **`release.yml`** calls the *same* workflow; build/sign jobs declare **`needs: [test]`** ⇒ **can't ship red.** No duplicated test logic.
- **Pre-commit hooks** (gitleaks, fast lint) for instant local feedback — bypassable (`SKIP=`), so **CI is the enforceable gate.**

## 5. Coverage policy
- Tool: **`cargo-llvm-cov`** (cross-platform incl. Windows MSVC; `cargo-tarpaulin` is Linux-only/unreliable on Windows). Run coverage on a **Linux runner** for stability; the Win/Mac matrix runs the tests themselves.
- Thresholds (gate via `--fail-under-*`): **crypto/signing/key-derivation ≥ 90%** (line+branch, aim near-100%); **general ~70–80%.**
- Coverage is a **signal, not a target** (Goodhart) — back it with **mutation testing** on `crypto/` to prove the tests actually catch breakage.

## 6. Accuracy & anti-gaming (keep tests trustworthy as volume grows)
- Assert **real behavior**, not tautologies; use real vectors so a failure means the logic is actually wrong.
- **Ban `continue-on-error` / silent retries** on security-critical jobs.
- **Fail CI on skipped/`#[ignore]` tests in crypto paths** (a grep gate).
- **Quarantine flaky tests visibly** (tracked issue + dashboard), never auto-retry-to-green.
- **Review test diffs** in PR. Automated checks **flag**; a human / adjudicating agent **confirms** (HelicOps meta-lesson: "SAST = lead-gen, not ledger" — don't blindly trust green).

## 7. Regression discipline
- **Every audit/bug finding gets a regression test.** F1/F2/F3 (secrets→disk) → a test asserting no key/seed/mnemonic reaches logs (and the §8 gate). A bug the audit found = a test we were missing.

## 8. Secret-in-logs gate (AUDIT-F8 — durable mitigation)
- **gitleaks** (custom `.gitleaks.toml` rules for key/seed/mnemonic/privkey near `log::`/`println!`/`std::cout`) **+ a custom ripgrep gate** targeting the exact sink shapes.
- Run as **both** a pre-commit hook (convenience) **and** a CI job (the real gate — can't be skipped).
- **Compile-time:** gate crypto-debug logging behind a cargo **feature flag** (off by default, never in release) or `cfg(debug_assertions)`. Feature flag preferred (explicit, greppable).

## 9. Capped test-wallet harness (agent-run live e2e) — TO BUILD
**Goal:** let agents (or humans) run **real** browser/wallet tests against **real sites** with a **hard-bounded** worst case — safe enough to automate.
- **`HODOS_DEV`** → separate dev DB; structurally cannot touch production data.
- **Dedicated TEST WALLET** with a tiny real balance (or testnet if supported — **confirm**), so blast radius is cents.
- **Low caps via the existing domain-permission/spending system** (per-tx, per-session, max-tx-per-session) — **enforced in Rust**, so the harness cannot exceed them.
- **Domain allowlist** for tests (e.g. `now.bsvblockchain.tech` BRC-121 site + the §10 verification basket).
- **Defense in depth:** cap at the wallet (Rust) **+** cap in the harness **+** allowlist domains **+** every spend fires the **gold pill** (audit trail).
- Agents are granted permission to run the **e2e harness** (Playwright) against allowlisted sites — **not raw wallet access.**
- *Status: design only — this is a feature to build (master plan `TEST-HARNESS`).*

## 10. Smoke & real-world verification (from CLAUDE.md)
A **smoke test** is a quick, shallow "did we fundamentally break it" check across critical paths after every build — a tripwire, not exhaustive.
| Tier | When | Sites |
|------|------|-------|
| **Minimal** | after any browser-core change | youtube.com, x.com, github.com |
| **Standard** | after a sprint | auth (x/google/github) + 2–3 video/media + 1–2 news |
| **Thorough** | before release/demo | full basket, all categories incl. BSV (whatsonchain.com) |

## 11. Hermetic CI notes
- **GoogleTest** is pulled via CMake FetchContent (needs network) → make hermetic: vcpkg (already our dep manager) **or** pinned `GIT_TAG` + `actions/cache`. 
- Pin all action versions; cache cargo/npm; vendor or cache test deps so CI is reproducible.

## 12. Decisions to lock (when we build PIPE-CI)
- Build the thin **Vitest** layer (recommended) vs formally retire UNIT_TESTING.md §3 — **recommend build.**
- Where C++/coverage runs (Linux for coverage; matrix for tests).
- Crypto coverage threshold + whether mutation testing is a gate or a periodic report.

## 13. Lessons Learned (append per Invariant #12)
- *(2026-06-16)* Test census was inflated across docs ("780+") — real ~480 Rust / 39 C++ / 23 adblock / 54 e2e / 0 Vitest. Trust source counts, not doc claims.
- *(2026-06-16)* The "CI gate exists" claims in BUILD_AND_RELEASE/UNIT_TESTING were fiction — there is no `ci.yml`. Build it; don't trust the doc.
- *(add new lessons as the workflow lands…)*
