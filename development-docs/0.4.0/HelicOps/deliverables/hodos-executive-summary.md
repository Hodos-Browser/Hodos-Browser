# Code Audit — Hodos Browser

## Executive Summary

**Verdict: `needs_review`.** The audit covered 108 files across four languages (Rust, TypeScript, C/C++, Python) against 42 policy checks. The headline is encouraging in one dimension and demanding in two: Hodos is a **fast, well-architected runtime** that is also **under-tested** and **not yet safe to hand to autonomous tooling**. None of the issues below are structural dead-ends — they are concentrated, nameable, and fixable.

The single most important framing for this report: **the risk is concentrated in two modules** — the Rust wallet (`rust-wallet/`) and the native browser layer (`cef-native/`). That is good news. It means remediation is a focused effort, not a rewrite.

---

## Readiness at a glance

| Dimension | Band | Score | Why it landed here |
|---|---|---|---|
| ⚡ Performance | **Efficient** | 0.98 | Even the weakest signals — loop efficiency (0.97) and hot-path efficiency (0.98) — are near-perfect. Genuinely fast code. |
| 🧪 Test | **Weak** | 0.53 | Dragged down by coverage of the highest-risk code (0.12): the dangerous code is the code with the least testing. |
| 🤖 Agentic | **Experimental** | 0.17 | The ability for an agent to verify its own changes (0.12) and runtime observability (0.25) are both very low. |

**The story in one line:** speed is not the problem; *verifiable trust* in the code is. Performance is already excellent, so engineering effort is best spent on coverage of the risky surfaces and on the few real security defects below.

---

## The 5 critical findings

| # | Issue | Location |
|---|---|---|
| 1 | Command execution via shell from a browser process (injection surface; no shell on an embedded target) | `/tmp/hodos/cef-native/src/core/ProfileManager.cpp:437` |
| 2 | Command execution via shell | `/tmp/hodos/cef-native/src/handlers/simple_handler.cpp:6851` |
| 3 | Command execution via shell | `/tmp/hodos/cef-native/src/handlers/simple_handler.cpp:7105` |
| 4 | SQL built by string interpolation instead of bound parameters — injection shape | `/tmp/hodos/rust-wallet/src/handlers.rs:2048` |
| 5 | Hardcoded `api_key` literal in source | `/tmp/hodos/rust-wallet/src/handlers.rs:8332` |

Three of the five sit in the native layer; two sit in the wallet. All five are days-of-work fixes, not weeks.

---

## What's driving the risk (beyond the criticals)

Four high-severity themes account for the bulk of the *real* signal:

- **Wallet credential hygiene.** Secret material (keys/seeds/mnemonics) is written to log/output sinks — 20 findings, e.g. `/tmp/hodos/rust-wallet/src/handlers.rs:6483` — and cryptographic values are drawn from a non-cryptographic randomness source (18 findings). In a wallet, leaked or predictable key material is the highest-stakes failure mode in the entire report.
- **Remote denial-of-service via panic.** A large family of unchecked unwraps sit on request paths — 284 findings, e.g. `/tmp/hodos/rust-wallet/src/handlers.rs:267`. A single malformed request can crash the worker. This is the largest high-severity group.
- **Renderer-input injection.** Untrusted web/renderer input reaches a code-execution sink without encoding — 12 findings, e.g. `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:927` — a DOM-injection surface across the process boundary.
- **Inconsistent neutralization.** 16 native-layer call sites reach a sink *without* the neutralization step the comparable safe sites apply, e.g. `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1185`. The codebase already knows the safe pattern; these sites simply skip it.

---

## Recommended next steps (in priority order)

1. **Close the 5 criticals first.** Parameter-bind the SQL at `handlers.rs:2048`; move the `api_key` at `handlers.rs:8332` into runtime config / a secrets manager; replace the shell-outs in the native layer with a direct process API or remove them.
2. **Harden the wallet immediately.** Strip secret values from every log/output sink and move cryptographic draws onto a secure (OS) randomness source. Highest real-world stakes; do this in parallel with step 1.
3. **Stop the remote-DoS bleeding.** Convert the unchecked-unwrap sites on request paths to proper error returns. Use the inconsistent-neutralization findings as a worklist — the safe pattern already exists in the codebase.
4. **Lift the score that matters.** Ignore the global test number (0.53) and target coverage of the highest-risk code (0.12): write tests for the wallet handlers and the criticals *first*. Coverage on risky code moves the band far faster than coverage on safe code.
5. **Before any agentic workflow:** stand up a way for changes to be automatically verified, and add structured runtime logging. Until then, the 0.17 agentic band means human-driven changes only.

---

## How to read the numbers

The audit surfaced 3,152 findings total. The overwhelming majority — 2,673 — are **style and maintainability** items (debug output, missing docs, complexity, formatting). These are deliberately set aside so they do not bury the real defects. Filtering to **security, correctness, and performance** leaves **479** findings worth acting on: 5 critical, 405 high, 69 medium. *That* 479-finding view is the one to work from; the full 3,152 is the raw, unfiltered count.

---

*Verdict `needs_review`. Every figure and file location in this document is drawn directly from the audit results and machine-checked against them — no number or location was authored by hand. The companion Technical Findings report carries the full breakdown.*
