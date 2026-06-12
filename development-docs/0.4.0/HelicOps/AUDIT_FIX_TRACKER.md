# Audit Fix Tracker (0.4.0)

**Created:** 2026-06-09 · **Status:** ✅ Adjudicated — backlog populated (implementation pending)
**Scope:** HelicOps findings we **AGREE** with and intend to fix. Part of the 0.4.0 backlog.

> Only findings VERIFIED against current source land here (see `README.md` methodology). Each is a
> real issue we accept. Disputed/false ones go to `HELICOPS_FEEDBACK.md`. Adjudication was run as a
> 17-agent zero-trust verification workflow (3 research + 13 cluster verifiers + 1 referee) on
> 2026-06-09; the headline criticals + the one real command-injection were additionally
> hand-verified against current source by the reviewer.

## How 479 findings became ~9 fixes

The 479 raw findings **deduplicate to ~25–30 genuine distinct defects**, which consolidate into the
**9 backlog items** below. The collapse is dominated by one fact: the ~245 "unhandled unwrap / panic
(DoS)" findings (clusters C1–C5) are **not 245 bugs** — they are ~3 root causes, the largest being a
single systemic mutex-poison issue (item **F4**). See `HELICOPS_META_ANALYSIS.md` for the full
signal-to-noise accounting.

## Fix backlog

| ID | Finding (cat / current file:line) | Severity | What's wrong | Fix | 0.4.0? | Status |
|----|---|---|---|---|---|---|
| **F1** | Secret→log / `cef-native/src/core/WalletService.cpp:440` | 🔴 **Critical** | `std::cout << "🔑 Mnemonic: " << response["mnemonic"]` — the **full BIP39 mnemonic** is printed on every `createWallet`. Per `cef-native/CLAUDE.md`, stdout is redirected to `build/bin/Release/debug_output.log`, so the seed phrase is **written to disk in cleartext**. Total wallet compromise. Also violates the project's own "never use `std::cout`" logging rule. | Delete the line (one line). Audit the rest of `createWallet`'s logging for other response-field leaks. | **IN** | ☐ |
| **F2** | Secret→log / `rust-wallet/src/handlers/certificate_handlers.rs:1710-1711, 1744` | 🔴 **Critical** | Full **32-byte cert-field symmetric key** logged untruncated at `log::info!` (hex **and** base64, plus "original symmetric key"). Full key compromise for that field. | Delete the log lines. | **IN** | ☐ |
| **F3** | Secret→log / ~13 sites: `crypto/brc2.rs:75,111,112,150,277,281` · `certificate/verifier.rs:310,324` · `handlers.rs:7346` · `certificate_handlers.rs:1445,1747,2261,2265` | 🟠 **High** | Private-key fragments, full HMAC scalar, ECDH shared secrets, master-key halves logged at `log::info!`/`std::cout` (production-visible, persists to disk). Violates `crypto/CLAUDE.md` invariant. | Delete, or gate every crypto-debug log behind a compile-time flag that is **OFF in release**. Pairs with **F8** sweep. | **IN** | ☐ |
| **F4** | Unhandled unwrap (DoS) — **systemic** / shared `Arc<Mutex<WalletDatabase>>`: ~194 sites `handlers.rs` + ~59 `certificate_handlers.rs`; root state `main.rs:~83`; narrower: `PENDING_TRANSACTIONS` (`handlers.rs:~4017`) + `sync_status` RwLock | 🟠 **High** | `std::sync::Mutex` **poisons** if a thread panics while holding the guard. The DB handle is shared across all handlers and `.lock().unwrap()`'d everywhere with **no `clear_poison`/`into_inner` anywhere**. One panic-while-holding-the-guard permanently poisons the mutex → every subsequent `.lock().unwrap()` panics → **durable, self-cascading DoS of the wallet core**. (R1 research: a *bare* Actix handler panic is only a per-request connection reset — low; the poison cascade is the real high.) | **Migrate `AppState.database` (and `PENDING_TRANSACTIONS` + `sync_status`) from `std::sync::Mutex`/`RwLock` → `parking_lot::Mutex`/`RwLock`** (does not poison; eliminates the class permanently). `tokio::sync::Mutex` locks (`utxo_selection_lock`, `create_action_lock`) don't poison — exempt. ⚠️ Touches `AppState` → follow CLAUDE.md invariant #4: understand all dependent handlers, migrate carefully, mechanical removal of `.unwrap()` on `.lock()` across ~253 sites. | **IN** | ☐ |
| **F5** | Command exec / `cef-native/src/core/ProfileManager.cpp:435-437` (entry: `simple_handler.cpp:~2913` `profiles_switch` IPC) | 🟠 **High** | macOS-only: `profileId` interpolated into `"/usr/bin/open … --args \"--profile=" + profileId + "\""` passed to `system()`. A `"` in `profileId` breaks out of the quoted literal → command injection. Windows branch uses `CreateProcessW` (safe). | Replace `system()` with `posix_spawn`/`execv` (argv array, no shell), **and** validate `profileId` against `GetProfiles()` before use. | **IN** | ☐ |
| **F6** | Injection / `cef-native/src/handlers/simple_render_process_handler.cpp` — `brc100_auth_request` (~1245-1265), `escapeJsonForJs` helper (~52-81), `tab_list_response` second escaper (~930-959) | 🟠 **High** | `brc100_auth_request` interpolates **dApp-controlled** domain/endpoint/body raw into single-quoted JS literals injected into the **trusted BRC-100 auth overlay UI** (127.0.0.1:5137) — genuine cross-context injection. `escapeJsonForJs` is incomplete (no `"`, `</script>`, U+2028/U+2029); a second ad-hoc escaper doesn't match its own quote style. | One hardened JS-string encoder (JSON-serialize + escape `< > &`, `</script>`, U+2028/9, `\uXXXX` for control/non-ASCII); route **all** injection sites through it. Prefer `JSON.parse` of a fully-escaped literal over string concatenation. | **IN** | ☐ |
| **F7** | Path traversal / `rust-wallet/src/backup.rs:1656,1667,1678,1928` (entry: `handlers.rs:12440,12455,12482` — `POST /wallet/backup`) | 🟡 **Medium** | `req.destination` is caller-supplied and written without validation → writes the live wallet DB (encrypted mnemonic + all rows) / JSON export to **any path**. Endpoint is **not domain-gated**. | **Validate path AND domain-gate** (decision 2026-06-09): canonicalize + allow-list the backup dir + reject `..`, **and** require domain approval / explicit user action before the route runs. | **IN** | ☐ |
| **F8** | Debug-artifact sweep (systemic) — incl. `rust-wallet/src/bin/extract_master_key.rs` (prints mnemonic at `:66`, extracts master privkey from DB) | 🟡 **Medium** | Beyond the cited sites, the secret-to-log cluster + a debug binary that dumps the master key suggest a systemic "debug artifacts shipping in the tree" problem. | **(a)** Remove/gitignore `extract_master_key.rs` (whole file, not just the print). **(b)** Sweep the tree for other key/seed/mnemonic logging + debug-only binaries/print sites so the F1–F3 deletions don't leave siblings re-introducing the leak. **(c)** Consider a CI grep-gate against `log::*` / `std::cout` of crypto material. | **IN** | ☐ |
| **F9** | Unhandled unwrap (input-validation nit) / `rust-wallet/src/handlers/certificate_handlers.rs:1669` | 🔵 Low | `fields.as_object().unwrap()` panics on a malformed (non-object) `req.fields` — caller-influenced per-request panic (no poison; guard already dropped → isolated). | `is_object()` check → return 400. Cheap. | opt | ☐ |

## Severity rollup

| Severity | # agreed (deduped) | # in 0.4.0 | # deferred/opt |
|----------|--------------------|------------|----------------|
| Critical | 2 (F1, F2) | 2 | 0 |
| High | 4 (F3, F4, F5, F6) | 4 | 0 |
| Medium | 3 (F7, F8 + extract_master_key, ) | 3 | 0 |
| Low | 1 (F9) | 0 | 1 (optional) |

> **Footprint note:** F4 alone covers ~253 source sites but is **one fix**. Do not expand it into 253
> tracker rows — that is exactly the count-inflation the audit committed.

## Notes
- **F1 + F2 are seed/key-on-disk** — the most severe items in the entire audit. The localhost-only
  bind does **not** downgrade them (the secret is already written to a logfile on the user's machine).
  Consider whether they warrant a patch release ahead of 0.4.0.
- Each code-touching fix follows the normal flow: kickoff review → plan → implement → test (Windows +
  macOS parity per the Testing Standards table). This tracker is the backlog, not a license to skip
  per-item planning. **F4 (parking_lot migration) and F6 (encoder) especially deserve their own
  kickoff** — both are cross-cutting.
- **F4 / F5 / F6 are macOS- or cross-platform-sensitive** — verify parity. F5 is macOS-only (Windows
  already safe); F6 touches the shared render handler.
- The TAAL hardcoded-key "critical" was a **fair call-out but a deliberate decision** — routed to
  `HELICOPS_FEEDBACK.md` (CLARIFY) and `development-docs/0.4.0/BROADCAST_AND_EXPLORER_REVIEW.md` for
  the real long-term decision. Not a 0.4.0 code fix beyond optional key rotation.
