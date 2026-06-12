# Logging Review & Upgrade (0.4.0)

**Created:** 2026-06-12 · **Status:** 🟡 Scoping brief — partial infra landed, comprehensive review deferred
**Owner area:** Cross-layer (Rust wallet · CEF C++ · React frontend)
**Related audit items:** `AUDIT_FIX_TRACKER.md` F1, F2, F3, F8 · `deliverables/hodos-executive-summary.md` (observability 0.17/0.25)

> **Why this doc exists.** On 2026-06-12 we added persistent file logging to the Rust wallet (it had
> none — `env_logger` wrote to stderr only). While doing it we realized logging is a cross-cutting
> concern that deserves a *deliberate* pass, not a one-off: three layers log to three different places,
> the HelicOps audit found **secrets being written to logs**, and our own new persistent sink now makes
> some of those leaks worse. This doc captures (a) what landed, (b) the decisions we locked, and (c) an
> actionable brief for the comprehensive logging review/fix scheduled as part of the 0.4.0 build work.
>
> **This is a scoping brief, not a license to skip per-item planning.** The security-relevant items
> (F1/F2/F3/F8) follow the normal kickoff → plan → implement → test flow.

---

## Part 1 — What landed 2026-06-12 (Rust wallet persistent logging)

The Rust wallet now logs to a rotating file via [`flexi_logger`], in addition to the console.

| Change | File | Detail |
|---|---|---|
| Add file logger | `rust-wallet/Cargo.toml` | `flexi_logger = "0.29"` (implements the `log` facade — all 2,071 existing `log::` calls flow through unchanged) |
| `data_root()` helper | `rust-wallet/src/main.rs` | Cross-platform `<dirs::data_dir()>/<app_dir_name()>`; shared by `wallet/` (db) and `logs/` |
| `init_logging()` | `rust-wallet/src/main.rs` | Rotating file in `<data_root>/logs/` + duplicate-to-stderr; `WriteMode::Direct` (live-tailable); `detailed_format` (ts + level + module + `file:line`) |
| Safeguard reorder | `rust-wallet/src/main.rs` | `enforce_dev_safeguard()` now runs **before** logger init so a mis-launched dev build bails before touching any data dir |
| `println!`→`log::` | `rust-wallet/src/main.rs` | 104 startup `println!`/`eprintln!` migrated to `log::` macros so the startup banner is captured. The 9 pre-logger safeguard `eprintln!`s intentionally remain (they run before the logger exists) |
| Launcher hints | `dev-wallet.ps1` / `dev-wallet.sh` | Echo the log location on startup |

**Log location:**
- Dev: `%APPDATA%\HodosBrowserDev\logs\wallet_rCURRENT.log` (macOS: `~/Library/Application Support/HodosBrowserDev/logs/`)
- Prod: `%APPDATA%\HodosBrowser\logs\` (see Option A below)

**Decisions locked this session (do not re-litigate without reason):**

| # | Decision | Rationale |
|---|---|---|
| D1 | `flexi_logger` over `tracing`/`fern` | Drop-in for the `log` facade (zero migration of 2,071 calls), built-in rotation + dual sink |
| D2 | Rotation = **10 MB/file × keep 10** (~110 MB ceiling, oldest auto-pruned) | Bounded disk; size-based not per-launch |
| D3 | **Option A** prod log policy: dev = `info`, **prod = `warn`** (both `RUST_LOG`-overridable) | Privacy: installed users don't accumulate an on-disk `info`-level trail of every domain/payment. Implemented in `init_logging()` |
| D4 | `WriteMode::Direct` (flush per record) | File is tailable in real time during dev/testing |
| D5 | Logs live under `<data_root>/logs/`, reusing `data_root()` | One cross-platform resolver; prod gets logs too (for support/forensics) |

**Explicitly NOT done this session (→ comprehensive review):** CEF/C++ log unification · frontend log capture · the audit secret-scrub (F1/F2/F3/F8) · structured/JSON logging · error-message specificity · CI grep-gate.

---

## Part 2 — Current logging landscape (3 systems, 3+ locations)

```
EXISTING (fragmented)                                  GOAL (consolidated)
──────────────────────                                 ───────────────────
Rust wallet ─► <datadir>/logs/wallet_*.log  [NEW]  ┐
CEF C++ ─────► <datadir>/Default/chrome_debug.log  │
       └─────► build/bin/Release/debug_output.log  ├─►  one readable, rotated,
              (stdout redirect, per cef CLAUDE.md)  │    secret-free location
React frontend ─► DevTools console only (ephemeral) ┘    (Rust + CEF + frontend)
```

| Layer | Sink today | Persistent? | Notes |
|---|---|---|---|
| **Rust wallet** | `flexi_logger` → `<datadir>/logs/wallet_*.log` + stderr | ✅ (new) | Structured, rotated, `RUST_LOG`-aware |
| **CEF C++** | Chromium `CefSettings.log_file` → `chrome_debug.log` | ✅ | Chromium-internal; not app-structured |
| **CEF C++ (stdout)** | redirected to `build/bin/Release/debug_output.log` | ✅ | Per `cef-native/CLAUDE.md`; **this is where F1's mnemonic lands** |
| **React frontend** | `console.log` → DevTools console | ❌ | Nothing on disk. Capturable via `OnConsoleMessage` (CefDisplayHandler) or a `log` IPC to the Rust sink |

---

## Part 3 — ⚠️ Audit cross-reference: secrets in logs (and how our change interacts)

The HelicOps audit's highest-stakes cluster is **secret material written to log/output sinks** (20 findings; exec summary calls leaked/predictable key material "the highest-stakes failure mode in the entire report"). From `AUDIT_FIX_TRACKER.md`:

| Audit ID | Sev | Where | Secret leaked | Sink |
|---|---|---|---|---|
| **F1** | 🔴 Critical | `cef-native/src/core/WalletService.cpp:440` | Full BIP39 **mnemonic** on every `createWallet` | `std::cout` → `debug_output.log` (disk) |
| **F2** | 🔴 Critical | `certificate_handlers.rs:1710-1711, 1744` | Full 32-byte **cert-field symmetric key** (hex + base64) | `log::info!` |
| **F3** | 🟠 High | `crypto/brc2.rs:75,111,112,150,277,281` · `certificate/verifier.rs:310,324` · `handlers.rs:7346` · `certificate_handlers.rs:1445,1747,2261,2265` | privkey fragments, HMAC scalar, ECDH shared secrets, master-key halves | `log::info!` / `std::cout` |
| **F8** | 🟡 Med | systemic + `rust-wallet/src/bin/extract_master_key.rs` | debug binary dumps master key; sweep for siblings; **CI grep-gate** recommended | mixed |

### 🔺 Our 2026-06-12 change RAISES the priority of F2/F3

Before today, the Rust wallet's `log::info!` secret dumps (F2/F3) went to **stderr only** — ephemeral, scrolled past in a dev console. **As of today they persist to `wallet_rCURRENT.log` on disk** (dev, `info` level). A developer's machine now accumulates cert symmetric keys / key fragments in a rotated file that lives ~110 MB deep.

- **Option A (D3) does NOT fix this.** Prod = `warn` suppresses these `info`-level lines *in production*, but **dev still writes them**, and the audit's own pitfall note applies: *"Lowering the log level — the secret is still written."* The real fix is **deleting the lines** (F2/F3), not gating by level.
- **F1 is unaffected by our change** (it's C++ `std::cout` → `debug_output.log`, already on disk) but belongs in the same scrub.
- **Action implication:** F2/F3 should be treated as *near-term* — arguably ahead of the rest of the comprehensive review — because we just widened their blast radius. Consider folding the F2/F3 line deletions into the next wallet commit rather than waiting for the full logging sprint.

---

## Part 4 — Audit observability signal (our work advances it)

`deliverables/hodos-executive-summary.md`: *"runtime observability (0.25) … very low"* and recommends *"add structured runtime logging"* before any agentic workflow. The flexi infra (Part 1) is the first concrete step toward that score. The comprehensive review should aim the rest of the way: structured fields, correlation across layers, and a single readable location.

---

## Part 5 — Comprehensive review: instructions for the future sprint

When this is picked up (planned for the 0.4.0 build/distribution work — the *new build folder* is also where production logging best-practices research belongs), run a proper kickoff and produce an improvement plan covering:

1. **Secret-scrub (security — do first, possibly ahead of the rest).**
   - Fix **F1** (delete mnemonic `std::cout`), **F2** (delete cert-key logs), **F3** (delete/compile-gate ~13 crypto-secret sites), **F8** (remove `extract_master_key.rs`, sweep tree).
   - Add a **CI grep-gate** (F8c) rejecting `log::*` / `std::cout` / `println!` of crypto material (mnemonic, privkey, symmetric key, shared secret, HMAC scalar) so deletions can't silently regress.
   - Re-verify against current `file:line` (audit citations may have drifted).

2. **Consolidate logs into one readable location.** Unify Rust (`<datadir>/logs/`), CEF C++ (`chrome_debug.log` + `debug_output.log`), and frontend into a single place/scheme so a developer (or an agent) reads one location:
   - Point `CefSettings.log_file` at `<datadir>/logs/`.
   - Replace the C++ `std::cout`→`debug_output.log` redirect with a real logger writing to the same dir (also kills the F1 sink shape).
   - Capture frontend `console.*` via `OnConsoleMessage` and/or a `log` IPC into the Rust sink.

3. **Error-logging specificity (not generic).** The audit's largest raw cluster is "unhandled error / panic (DoS)" (≈245 → root-caused, see **F4** mutex-poison). Beyond the poison fix, audit our error logging for *generic* messages (bare `{}` errors, swallowed `Err(_)`, context-free `unwrap`/`expect`). Every error log should carry: operation, identifying context (txid/domain/endpoint — never secrets), and the underlying error. Define an error-logging convention.

4. **Production logging policy & best practices.** Revisit D3 (Option A) holistically against the 0.4.0 distribution model:
   - Should prod be `warn`, opt-in (`Settings → Diagnostic logging`), or off? (Current default: `warn`.)
   - Retention/rotation tuning for installed users; log-location discoverability for support.
   - Crash-report path (separate from routine logging).
   - Confirm **no secrets / minimal PII** at whatever prod level ships (depends on #1).

5. **Structured logging (optional, observability score).** Evaluate JSON/structured output + cross-layer correlation IDs so a single dApp interaction can be traced Rust↔C++↔frontend.

### Acceptance for the comprehensive pass
- Zero secret-shaped values reach any log sink (CI-enforced).
- One documented location to read all three layers' logs in dev.
- Error logs are specific and contextual (no generic/empty messages).
- A deliberate, documented production logging policy (level, retention, secret-safety).
- Audit items F1/F2/F3/F8 closed in `AUDIT_FIX_TRACKER.md`.

---

## Open questions / decisions needed
- **Fold F2/F3 line deletions into the next wallet commit** (recommended, given the amplification in Part 3), or hold for the full sprint? *(lean: do the deletions soon)*
- Prod policy final form — keep Option A `warn`, or move to opt-in? *(deferred to #4)*
- Consolidation target: extend the Rust `flexi` sink to be the single sink (C++/frontend feed into it via IPC), or a shared directory with per-layer files? *(deferred to #2)*

## References
- `AUDIT_FIX_TRACKER.md` — F1/F2/F3/F4/F8 (this folder)
- `deliverables/hodos-technical-findings.md` — "Secret written to log/output" (20), observability bands
- `deliverables/hodos-agent-brief.md:360-389` — per-site secret-in-log table
- `rust-wallet/src/main.rs` — `init_logging()`, `data_root()` (the landed infra)
- `cef-native/CLAUDE.md` — `debug_output.log` stdout-redirect note (F1's sink)

[`flexi_logger`]: https://docs.rs/flexi_logger
