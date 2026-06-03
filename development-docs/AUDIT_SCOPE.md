# HodosBrowser External Audit — Scope & Priorities

## Project at a glance

HodosBrowser is a CEF-based Web3 browser with a native Bitcoin SV wallet handling real-money transactions. Three layers:

| Layer | Tech | Role |
|---|---|---|
| **Frontend** | React / Vite / TypeScript | UI only — never handles keys or signing |
| **Native shell** | C++17 / CEF 136 | Browser engine, V8 injection, HTTP interception, IPC |
| **Wallet service** | Rust / Actix-web / SQLite | Crypto, signing, BRC-100 protocol, key custody |

Most of the codebase was AI-assisted. The wallet service exposes a localhost HTTP API (`:31301`); the native shell intercepts wallet-bound requests and enforces a per-domain auto-approve permission engine before forwarding.

## What a good outcome looks like

Two measurable goals:

1. **Increased confidence the product is safe to ship.** Critical findings surfaced, classified, and accompanied by enough detail to fix.
2. **A clear, prioritized backlog so we can iterate without guessing.** Findings ordered by impact and shipped with fix sketches.

**Deliverable expectation: findings + fix sketches.** "Here's the bug" is half a deliverable; "here's the bug and a 5-line patch shape" is the full one. Every Tier-1 or Tier-2 finding should arrive with a recommended remediation outline, not just a problem description.

---

## Priorities — Tier 1 (Critical; must be in scope)

1. **Cryptographic implementation audit.** `rust-wallet/src/crypto/` (brc42, brc2, signing, aesgcm_custom, ghash, pin, dpapi, keys), plus `transaction/sighash.rs` (BSV ForkID) and `recovery.rs` (BIP32). Focus: spec conformance, AES-GCM nonce uniqueness, PBKDF2 iteration count, DPAPI/Keychain usage, secure memory zeroing, RNG sourcing, constant-time comparisons.

2. **Trust-boundary & IPC security review.** Three boundaries: frontend→native shell (V8 `window.hodosBrowser` surface, IPC dispatch), native→wallet (HTTP interception + auto-approve engine in `HttpRequestInterceptor.cpp`), wallet→external (ARC, WhatsOnChain, MessageBox). What is validated where; what is assumed; what an attacker controlling untrusted web content could reach.

3. **Native-shell C++ memory safety & correctness.** HWND lifecycle (multiple overlay WS_POPUP windows with documented close-race history), CEF threading rules (UI / IO / render-process boundaries), raw pointers in IPC dispatch, the singleton cache classes, `compare_exchange_strong` patterns. Run Semgrep + CodeQL on `cef-native/`.

4. **Auto-approve & permission engine logic.** `PermissionEngine.cpp`, `HttpRequestInterceptor.cpp`. This is where money decisions are made: per-tx caps, per-session caps, rate limits, "Always allow," BRC-121 cap cascade, identity-key reveal, BRC-72 linkage, certificate field disclosure. Verify the decision table against `PERMISSION_UX_DESIGN.md` and confirm engine-driven gates aren't bypassable via residual inline paths.

## Priorities — Tier 2 (High)

5. **Code duplication & parallel implementations.** Likely targets: multiple `*Cache` classes with copy-paste shape, parallel HTTP-client paths (SyncHttpClient vs raw WinHTTP vs CefURLRequest), redundant handler scaffolding in `handlers.rs`. Deliverable: a list of cases where a generic abstraction would consolidate three+ parallel implementations, each with effort estimate.

6. **Layer-placement architecture review.** Deliverable: a **placement matrix** — for each non-trivial piece of state or logic, where it lives today vs where it should live vs why. Particular interest in frontend code that should be in the native shell (tab/window state, navigation, history search, download state) and any wallet-domain arithmetic in C++ that should be in Rust.

7. **Concurrency & race conditions.** CEF render-vs-browser process message ordering, overlay close paths (`WM_ACTIVATEAPP` / `WM_ACTIVATE` documented races), `Arc<Mutex<WalletDatabase>>` write-serialization under load, Monitor task `try_lock()` patterns, `AtomicU64` block-event triggers.

## Priorities — Tier 3 (Medium)

8. **Dependency & supply-chain audit.** Run `cargo audit`, `npm audit --production`, check CEF 136 version pinning, vcpkg packages, `external/winsparkle`, `adblock-rust` crate. Flag any unmaintained or CVE-tagged transitive dependencies.

9. **Performance & resource efficiency.** Process count and memory footprint (9+ processes at runtime), cache hit-rate analysis, database query patterns (N+1, missing indexes), background-task scheduling efficiency, V8 injection overhead.

10. **Testing & CI/CD posture — gap analysis.** Deliverable is forensic + prescriptive: inventory of current tests, gap analysis against critical paths (signing, broadcast, BEEF parsing, auto-approve decisions), recommended minimum CI gates (build, test, lint, semgrep, cargo-audit, npm-audit), effort estimate for reaching that baseline.

## Tier 4 — Caveated scope (best-effort; specialist follow-up planned)

**BRC structural conformance & documentation drift.** Verify Hodos's implementations match the published BRC wire formats and shapes; flag drift between `CLAUDE.md` docs and the actual code. **Defer protocol-semantic and ecosystem-interop review to a follow-up BSV-specialist audit.** Reference set provided:
- Spec repo: `github.com/bitcoin-sv/BRCs`
- Reference implementations: `@bsv/sdk`, `@bsv/wallet-toolbox`, `@bsv/402-pay`
- Specific BRCs in use: 2, 29, 42, 43, 52, 72, 74, 77, 100, 103, 104, 121

---

## 12 AI-specific failure modes to keep in scope

This codebase was largely AI-assisted. Common failure modes to scan for explicitly:

1. **Parallel structures** added instead of extending existing infrastructure.
2. **Plausible-but-wrong code** — compiles and runs, but semantically subtly off (wrong sign, off-by-one, wrong byte order, wrong opcode).
3. **Hallucinated / outdated APIs** — confidently-used library calls that don't exist or have changed.
4. **Defensive over-coding** — null checks on impossible nulls, error handlers for unreachable branches.
5. **Half-finished features & orphan TODOs** — stubs that became permanent.
6. **Comment / code drift** — comments describe behavior from two refactors ago.
7. **Test bugs that mirror implementation bugs** — AI wrote both; both share the same misconception. Verify tests against published spec, not against implementation.
8. **Style inconsistency across sessions** — different naming, different idioms, different error-handling patterns. Sometimes masks real divergence.
9. **Missing edge cases on adversarial inputs** — oversized arrays, malformed encodings, 200-with-garbage-body, concurrent-with-shutdown.
10. **Premature or unused abstractions** — generic infrastructure with one caller.
11. **Naive concurrency** — lock-everywhere serialization or lock-nothing optimism.
12. **Magic numbers without context** — embedded constants for timeouts, fees, limits, retry counts.

---

## Engagement structure (recommended)

- **Time-box** the engagement. If Tier 1 finishes ahead of schedule, advance into Tier 2 in order.
- **Stream findings** as a running document rather than waiting for a final report. Lets us start fixing in parallel.
- **Tag every finding with a blast-radius note** — what else may be affected by the same root cause.
- **Reserve budget for one re-review** after critical fixes land. Second-pass is where most audit value crystallizes.

## Provided context

- `CLAUDE.md` at repo root + per-layer (`rust-wallet/`, `cef-native/`, `frontend/`, `adblock-engine/src/`)
- `PROJECT_OVERVIEW.md` — consolidated architecture reference
- `development-docs/Sigma-BRC121-Sprint/` — current active sprint, with phase docs and design notes
- `development-docs/Final-MVP-Sprint/SECURITY_MINDSET.md` — internal security posture
- Curated bug-history bundle (separate `AUDIT_CONTEXT.md`) — known race conditions, deferred items, cache-poisoning lessons, architectural decisions
- BRC reference set (links above)
