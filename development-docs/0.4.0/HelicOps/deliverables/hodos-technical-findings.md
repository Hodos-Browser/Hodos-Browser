# Code Audit — Hodos Browser

## Technical Findings Report

This document is the engineering companion to the Executive Summary. It carries the full severity accounting, the per-theme finding breakdown with evidence, and the priority roadmap.

---

## Scope

- **Files scanned:** 108
- **Policy checks evaluated:** 42
- **Languages:** Rust, TypeScript, C/C++, Python
- **Verdict:** `needs_review`

All four languages were fully analyzed, so this reflects complete coverage — not a partial picture.

---

## Severity accounting (the number that matters)

The audit reports two views of the same results. Both are correct; they answer different questions.

| Severity | Raw (all checks) | Actionable (security / correctness / performance) |
|---|---|---|
| 🔴 Critical | 5 | 5 |
| 🟠 High | 1040 | 405 |
| 🟡 Medium | 1124 | 69 |
| 🔵 Low | 982 | 0 |
| ⚪ Info | 1 | 0 |
| **Total** | **3152** | **479** |

The gap is **2,673** findings that are **style / maintainability** — debug output, missing docs, complexity, formatting. The raw "high" count of 1,040 is inflated by lint/style noise (stray debug output and console statements account for most of it); once that noise is set aside, the genuine high-severity count is **405**.

**Work from the 479-finding column.** The split between actionable and style is a fixed, documented categorization applied uniformly, so it is reproducible — not a per-finding judgement call.

---

## Critical findings (all 5)

| Category | Severity | Location | Fix |
|---|---|---|---|
| Command execution via shell | critical | `/tmp/hodos/cef-native/src/core/ProfileManager.cpp:437` | Replace the shell-out with a direct process API, or remove it. |
| Command execution via shell | critical | `/tmp/hodos/cef-native/src/handlers/simple_handler.cpp:6851` | As above. |
| Command execution via shell | critical | `/tmp/hodos/cef-native/src/handlers/simple_handler.cpp:7105` | As above. |
| SQL injection (string-built query) | critical | `/tmp/hodos/rust-wallet/src/handlers.rs:2048` | Use bound parameters; never interpolate input into SQL text. |
| Hardcoded secret in source | critical | `/tmp/hodos/rust-wallet/src/handlers.rs:8332` | Move the `api_key` to runtime config / a secrets manager; rotate the exposed key. |

---

## High-severity themes

These groups make up most of the 405 actionable highs. Counts are exact; the location shown is one representative site per group.

| Category | Count | Severity | Example site | What it means |
|---|---|---|---|---|
| Unhandled error / panic risk (DoS) | 284 | high | `/tmp/hodos/rust-wallet/src/handlers.rs:267` | An unchecked unwrap on a request path — malformed input crashes the worker. |
| Path traversal (unvalidated file path) | 22 | high | `/tmp/hodos/scripts/generate-appcast.py:97` | File open on a non-literal path with no containment guard. |
| Secret written to log/output | 20 | high | `/tmp/hodos/rust-wallet/src/handlers.rs:6483` | Key/seed/mnemonic material written to a log/output sink. |
| Weak randomness for cryptography | 18 | medium | `/tmp/hodos/rust-wallet/archive/old-tests/interoperability_test.rs:74` | Crypto material drawn from a non-cryptographic randomness source. |
| Blocking call in async context | 16 | high | `/tmp/hodos/adblock-engine/src/engine.rs:402` | A blocking file call inside an async function stalls the executor. |
| Missing input neutralization | 16 | high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1185` | Reaches a sink without the neutralization its comparable safe sites apply. |
| Untrusted input reaches code-execution sink | 12 | high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:927` | Untrusted input reaches a script-execution sink unescaped — injection. |
| Inefficient lookup in loop | 9 | high | `/tmp/hodos/rust-wallet/src/handlers.rs:3604` | A linear membership scan inside a loop — quadratic at scale. |
| Insecure deserialization | 7 | high | `/tmp/hodos/frontend/src/hooks/useBitcoinBrowser.ts:42` | Untrusted data deserialized without a schema/allowlist. |
| Prototype pollution | 7 | high | `/tmp/hodos/frontend/src/hooks/useHodosBrowser.ts:42` | Unvalidated keys merged into objects can corrupt prototypes. |
| Unsafe memory copy / deserialization | 6 | high | `/tmp/hodos/cef-native/src/core/HttpRequestInterceptor.cpp:714` | A copy whose size is not a literal/`sizeof` — validate length first. |

---

## Cross-cutting findings

Two of the categories above come from analysis that operates *across* files rather than line-by-line — this is where the audit adds signal a conventional linter cannot:

- **Untrusted input reaches code-execution sink** is a data-flow result: not "this line looks dangerous," but "untrusted input *reaches* this sink without passing through an encoder," established by following the flow across the process boundary.
- **Missing input neutralization** compares structurally similar call sites and flags the outliers: most comparable sites apply a neutralization step before the sink; these specific sites skip it. The safe pattern already exists in the codebase, which makes these high-confidence, low-effort fixes.

---

## C/C++ coverage

The native layer was fully analyzed, including coding-standard conformance. Notably, that pass added **no new criticals or highs** — its contribution is low-severity maintainability (unscoped casts, partial special-member sets, coding-standard deviations). The reassuring result: the genuinely dangerous native-code shapes (shell execution, unchecked memory copies, path traversal) were already caught, and the conformance pass confirms the C/C++ layer carries no *additional* security surprises beyond style-tier debt.

---

## Readiness methodology

Each readiness dimension is scored 0–1 from underlying signals; the two weakest of each are reported.

- **Test — Weak (0.53).** Weakest: coverage of the highest-risk code (0.12), and how closely tests sit to the code they exercise (0.20). The 0.53 overall is not "half the lines are covered" — it is "some scaffolding exists, but almost none of it sits on the high-risk code." Coverage targeted at the wallet handlers and the criticals moves the band fastest.
- **Agentic — Experimental (0.17).** Weakest: the ability for an agent to verify its own changes (0.12), and runtime observability (0.25). An agent editing this repo today is flying blind.
- **Performance — Efficient (0.98).** Weakest: loop efficiency (0.97), hot-path efficiency (0.98). Both near-perfect — no meaningful performance debt.

---

## Remediation roadmap

1. **Criticals (days):** the 5 listed above.
2. **Wallet hardening (days):** secret-in-log (20) and weak-randomness (18) — strip secrets from output, move crypto onto a secure randomness source.
3. **DoS sweep (1–2 sprints):** the 284 unchecked-unwrap-on-request-path sites → proper error returns. Pair with the 12 injection and 16 missing-neutralization sites.
4. **Test coverage (ongoing):** target coverage of the highest-risk code — risky code first.
5. **Agentic enablement (later):** automated change-verification + structured logging before pointing any agent at the repo.
6. **Maintainability (background):** the 2,673 style findings — burn down opportunistically; not a security priority.

---

*Verdict `needs_review`. Every count and `file:line` in this report is drawn from the audit results and machine-checked against them; the narrative is written to those results under a contract that forbids any number or location not present in the data.*
