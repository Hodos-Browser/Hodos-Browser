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
| **F1** | Secret→log / `cef-native/src/core/WalletService.cpp:440` | 🔴 **Critical** | `std::cout << "🔑 Mnemonic: " << response["mnemonic"]` — the **full BIP39 mnemonic** is printed on every `createWallet`. Per `cef-native/CLAUDE.md`, stdout is redirected to `build/bin/Release/debug_output.log`, so the seed phrase is **written to disk in cleartext**. Total wallet compromise. Also violates the project's own "never use `std::cout`" logging rule. | Delete the line (one line). Audit the rest of `createWallet`'s logging for other response-field leaks. | **IN** | ✅ |
| **F2** | Secret→log / `rust-wallet/src/handlers/certificate_handlers.rs:1710-1711, 1744` | 🔴 **Critical** | Full **32-byte cert-field symmetric key** logged untruncated at `log::info!` (hex **and** base64, plus "original symmetric key"). Full key compromise for that field. | Delete the log lines. | **IN** | ✅ |
| **F3** | Secret→log / ~13 sites: `crypto/brc2.rs:75,111,112,150,277,281` · `certificate/verifier.rs:310,324` · `handlers.rs:7346` · `certificate_handlers.rs:1445,1747,2261,2265` | 🟠 **High** | Private-key fragments, full HMAC scalar, ECDH shared secrets, master-key halves logged at `log::info!`/`std::cout` (production-visible, persists to disk). Violates `crypto/CLAUDE.md` invariant. | Delete, or gate every crypto-debug log behind a compile-time flag that is **OFF in release**. Pairs with **F8** sweep. | **IN** | ✅ |
| **F4** | Unhandled unwrap (DoS) — **systemic** / shared `Arc<Mutex<WalletDatabase>>`: ~194 sites `handlers.rs` + ~59 `certificate_handlers.rs`; root state `main.rs:~83`; narrower: `PENDING_TRANSACTIONS` (`handlers.rs:~4017`) + `sync_status` RwLock | 🟠 **High** | `std::sync::Mutex` **poisons** if a thread panics while holding the guard. The DB handle is shared across all handlers and `.lock().unwrap()`'d everywhere with **no `clear_poison`/`into_inner` anywhere**. One panic-while-holding-the-guard permanently poisons the mutex → every subsequent `.lock().unwrap()` panics → **durable, self-cascading DoS of the wallet core**. (R1 research: a *bare* Actix handler panic is only a per-request connection reset — low; the poison cascade is the real high.) | **Migrate `AppState.database` (and `PENDING_TRANSACTIONS` + `sync_status`) from `std::sync::Mutex`/`RwLock` → `parking_lot::Mutex`/`RwLock`** (does not poison; eliminates the class permanently). `tokio::sync::Mutex` locks (`utxo_selection_lock`, `create_action_lock`) don't poison — exempt. ⚠️ Touches `AppState` → follow CLAUDE.md invariant #4: understand all dependent handlers, migrate carefully, mechanical removal of `.unwrap()` on `.lock()` across ~253 sites. | **IN** | ☐ |
| **F5** | Command exec / `cef-native/src/core/ProfileManager.cpp:435-437` (entry: `simple_handler.cpp:~2913` `profiles_switch` IPC) | 🟠 **High** | macOS-only: `profileId` interpolated into `"/usr/bin/open … --args \"--profile=" + profileId + "\""` passed to `system()`. A `"` in `profileId` breaks out of the quoted literal → command injection. Windows branch uses `CreateProcessW` (safe). | Replace `system()` with `posix_spawn`/`execv` (argv array, no shell), **and** validate `profileId` against `GetProfiles()` before use. | **IN** | ☐ |
| **F6** | Injection / `cef-native/src/handlers/simple_render_process_handler.cpp` — `brc100_auth_request` (~1245-1265), `escapeJsonForJs` helper (~52-81), `tab_list_response` second escaper (~930-959) | 🟠 **High** | `brc100_auth_request` interpolates **dApp-controlled** domain/endpoint/body raw into single-quoted JS literals injected into the **trusted BRC-100 auth overlay UI** (127.0.0.1:5137) — genuine cross-context injection. `escapeJsonForJs` is incomplete (no `"`, `</script>`, U+2028/U+2029); a second ad-hoc escaper doesn't match its own quote style. | One hardened JS-string encoder (JSON-serialize + escape `< > &`, `</script>`, U+2028/9, `\uXXXX` for control/non-ASCII); route **all** injection sites through it. Prefer `JSON.parse` of a fully-escaped literal over string concatenation. | **IN** | ✅ |
| **F7** | Path traversal / `rust-wallet/src/backup.rs:1656,1667,1678,1928` (entry: `handlers.rs:12440,12455,12482` — `POST /wallet/backup`) | 🟡 **Medium** | `req.destination` is caller-supplied and written without validation → writes the live wallet DB (encrypted mnemonic + all rows) / JSON export to **any path**. Endpoint is **not domain-gated**. | **Validate path AND domain-gate** (decision 2026-06-09): canonicalize + allow-list the backup dir + reject `..`, **and** require domain approval / explicit user action before the route runs. | **IN** | ✅ |
| **F8** | Debug-artifact sweep (systemic) — incl. `rust-wallet/src/bin/extract_master_key.rs` (prints mnemonic at `:66`, extracts master privkey from DB) | 🟡 **Medium** | Beyond the cited sites, the secret-to-log cluster + a debug binary that dumps the master key suggest a systemic "debug artifacts shipping in the tree" problem. | **(a)** Remove/gitignore `extract_master_key.rs` (whole file, not just the print). **(b)** Sweep the tree for other key/seed/mnemonic logging + debug-only binaries/print sites so the F1–F3 deletions don't leave siblings re-introducing the leak. **(c)** Consider a CI grep-gate against `log::*` / `std::cout` of crypto material. | **IN** | ◑ |
| **F9** | Unhandled unwrap (input-validation nit) / `rust-wallet/src/handlers/certificate_handlers.rs:1669` | 🔵 Low | `fields.as_object().unwrap()` panics on a malformed (non-object) `req.fields` — caller-influenced per-request panic (no poison; guard already dropped → isolated). | `is_object()` check → return 400. Cheap. | opt | ✅ |

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

## Wave 0 closure — F1/F2/F3 fixed (2026-06-17)

Implemented as the **Wave 0 secret-log commit** on branch `0.4.0` (this tracker update is part of it).
**Approach: delete** (not compile-gate) — these are BRC-42/BRC-2 interop-debugging leftovers with no
production value; deletion leaves zero residual and realigns with the `crypto/CLAUDE.md` invariant
("private keys … never serialized to strings or logged").

**Verified before deleting:** F1 is real — `/wallet/create` returns `"mnemonic"` (`handlers.rs:2836`),
so `WalletService.cpp:440` wrote the live BIP39 seed to `debug_output.log` on every wallet creation.

**Actual deleted sites (audit file:line citations were drifted — corrected here):**
- **F1** — `WalletService.cpp:440` (mnemonic `std::cout`).
- **F2** — `certificate_handlers.rs:1727-1730` (plaintext value+bytes, field symmetric key hex+base64)
  and `1763-1764` (original symmetric key + stripped revelation key). *Audit cited `1710-1711,1744` —
  drifted ~20 lines; `1763-1764` were uncited.*
- **F3** — `brc2.rs:75,111,112,148,150,277,281` · `verifier.rs:276,310,317,324` ·
  `certificate_handlers.rs:1464,2280,2284` · `handlers.rs:7387`. `verifier.rs:322` renamed
  `hmac_secret`→`_hmac_secret` to keep its validation side-effect. *Kept `handlers.rs:7346` (sighash =
  public data, not a secret). Audit's `certificate_handlers.rs:1445/1747/2261/2265` were NOT log-leaks
  (error handler / comment / non-log code).*

**The thorough sweep found 6 sites the audit (syntactic SAST) missed** — all same-class, now deleted:
cert symmetric key `1763`, subject privkey `1766`, master/child privkeys `2280/2284`, signing privkey
`handlers.rs:7387`, and `verifier.rs:317` (`hmac_output` = same scalar as `:324`). **Lesson:** the F8 CI
grep-gate against `log::*`/`std::cout` of crypto material is the durable fix — a targeted grep beat the
audit tool. (A first naive regex missed two paren-containing log messages; a second robust pass caught
them — note for the grep-gate authoring.)

**Build/test:** `cargo check --release` clean (no new warnings); `cargo test --lib` 370/375 (4
pre-existing unrelated: 3 `selective_disclosure` FK-fixture + 1 `utxo_fetcher` network). C++ deletions
ride the next CEF build. **Mac-parity:** no delta (swept `_mac`/`.mm`; `AddressHandler` is a single
cross-platform file).

### AddressHandler — investigated, FALSE ALARM (handled separately)
`AddressHandler.cpp:71/76` reference a phantom `addressData["privateKey"]`, but Rust
`/wallet/address/generate` returns only `{address,index,publicKey}` (`handlers.rs:9413-9417`). The
non-const `json["privateKey"].get<string>()` would **throw**, not leak — no key ever reached disk or
JS. Removed as a separate `fix(correctness)`: `AddressHandler.cpp:71/76`, `simple_app.cpp:479` (legacy
injected debug-JS), `frontend/src/types/address.d.ts:4`.

### New item — `privateKey` in the JS type surface (UNVERIFIED, needs investigation)
The `privateKey`-in-JS pattern also appears in `frontend/src/types/identity.d.ts`
(`IdentityData.privateKey`) and `frontend/src/bridge/brc100.ts:241` (`deriveType42Keys` typed to
**return** `privateKey`). Not yet verified whether the Rust side actually returns a key there (a real
Invariant-#1 leak) or whether these are more phantom types. **Owns its own investigation chunk.**

### F8 — debug-artifact removal (2026-06-17)
Deleted `rust-wallet/src/bin/extract_master_key.rs` (dumped the 32-byte master private key + first 20
chars of the mnemonic to stdout) and the archived `rust-wallet/archive/test-scripts/extract_key.ps1`
(same class) + its row in that dir's `CLAUDE.md`. `src/bin/` glob confirmed `extract_master_key` was
the **only** debug binary; the secret-log sibling sweep was done in the Wave 0 commit. `cargo check`
clean (auto-discovered bin, no `Cargo.toml` change). **(a)+(b) done; (c) CI grep-gate deferred to
PIPE-CI** (CI track).

## Wave 1 Track A closure — F7 + F9 fixed (2026-06-18)

Landed as one commit on branch `0.4.0` (this tracker update is part of it). Both ran the full
per-chunk harness (kickoff + cited-code verify → adversarial **design** review → implement → build +
unit tests → adversarial **code** review → mac-parity capture). Both gates passed; full Rust bin test
suite green (425 passed / 0 failed / 2 network-`#[ignore]`); mac-parity note in `MACOS_PORT_0_4_0.md`
(pure Rust, no Mac delta).

**Citation drift corrected:** F9 was at `certificate_handlers.rs:1687` (audit cited `:1669`).

**F9** — `acquire_certificate_issuance`: added `if !fields.is_object() { return 400 }` before the
`fields.as_object().unwrap()`. Confirmed it's the **only** caller-reachable panic in that path — every
`hex::decode` of request input (incl. `certifier`) already returns 400/502, and field *values* are
matched exhaustively. So the audit's F9 is the single guard; the skeptic's "sibling unwrap" concern
(certifier hex) was a **non-issue** (already guarded). Live smoke (send `{"fields":[1,2,3]}` → expect
400) deferred to next dev run.

**F7** — **the design changed during the adversarial design review**, and the change reverses the
audit's "allow-list **the backup dir** … require domain approval" only in emphasis:
- **Internal-only gate** (stronger than "domain approval"): `wallet_backup` + `wallet_restore` now
  reject any request carrying a non-empty `X-Requesting-Domain` (403). Backup/restore are
  user/wallet-internal **only** — no website, even an *approved* dApp, may dump or overwrite the
  wallet DB. (The universal `domain_trust_mw` already blocks *unapproved* domains; this handler gate
  additionally blocks approved ones.)
- **Confinement** to `<data_root>/backups/` (`backups_dir_for_db`) via **lexical** normalization
  (`lexical_normalize_abs` + `validate_backup_path`) — NOT `std::fs::canonicalize` (which requires the
  path to exist and would reject a brand-new backup destination). Rejects non-absolute paths, Windows
  verbatim/UNC/device prefixes, and `..` escapes; enforces component-wise `starts_with(root)`.
  Validated **before** any FS touch. The design review proved the audit's `..`-rejection alone is
  insufficient (an absolute path to a Startup folder needs no `..`) — confinement is the load-bearing
  control.
- **Restore folded in** (audit didn't cite it): same internal-only gate + path validation on the
  caller-supplied `backup_path`. Restore was reachable only by a raw client today (no UI caller).
- **`backups/`-only is interim.** Neither the user-facing "copy the file" button nor cloud backup is
  built yet, and there is **no live caller** of these two routes. When that button ships, the
  destination must come from the **OS save dialog driven by the C++ shell** (an authenticated,
  provably-user-chosen path) — not an HTTP-body string — at which point the `backups/` confinement
  relaxes for that dialog-returned path. See `MACOS_PORT_0_4_0.md`.

### Follow-up surfaced during F7 (NEW — not an original audit finding)

| ID | Item | Severity | Notes |
|----|------|----------|-------|
| **FU1** | **Real internal-auth boundary: C++↔Rust shared secret + CORS lockdown** | 🟠 High (architectural) | The "internal vs external" distinction across the whole wallet relies on *absence* of the `X-Requesting-Domain` header — which a raw local client (curl / local process / cross-origin page via a CSRF-style write) can simply omit. For F7 this is acceptable (a local process can already read `wallet.db` off disk; the header gate + CORS still block the *website* threat that F7 cares about), but it is **not** a robust authenticity signal in general. The durable fix: a per-launch random token minted by the C++ shell, handed to Rust at spawn, required (constant-time compared) on privileged routes — plus the **Q4 CORS lockdown** (echo-the-Origin, answer preflight, no `Allow-Credentials`, route on `request_initiator`). **Cross-cutting — touches AppState + every internal caller + the middleware; overlaps Q4.** Do as its own chunk, NOT folded into any single audit fix. |
| **FU2** | **Route remaining backend-sourced raw-interpolation sites through `escapeJsonForJs`** | 🔵 Low (defense-in-depth) | Surfaced by the F6 gate-2 review. `simple_render_process_handler.cpp` still has raw `+ var +`-into-`'…'` sites that bypass the canonical encoder — `address_generate_error` (~1304, `errorMessage`) and several `responseJson`/`json` dispatches (~973, 990, 1261, 1276, 1291, 1384, 1420, 1559). These are **backend-sourced** (Rust wallet / our own scanner), NOT dApp-controlled, so they're outside F6's "untrusted external data" charter and not a live vuln. But an embedded `'` in e.g. a backend error message would still break the dispatch — route them through `escapeJsonForJs` in a future defense-in-depth pass. Mechanical, low-risk. |

## Wave 1 Track A closure — F6 fixed (2026-06-18)

Landed as one commit on branch `0.4.0`. Ran the full per-chunk harness (kickoff + reuse-first audit → adversarial **design** review → implement → build + unit tests → adversarial **code** review → mac-parity capture). Both gates passed.

**Design pivoted twice during the chunk, both for good reasons the harness surfaced:**
1. The reuse-first kickoff found `escapeJsonForJs` is the **canonical encoder already used by ~50 sites** (not a minor helper), and is injection-safe for its actual use (single-quoted literals via `ExecuteJavaScript`, which has **no HTML parser**). So introducing a *parallel* `nlohmann::json` pattern at just 2 sites (the earlier "Option B") was rejected as inconsistent — the fix is to **reuse + harden the one canonical encoder** and route the deviant sites through it.
2. The adversarial design review then found the audit's literal *"escape `< > &`"* is **wrong for our context**: no caller feeds output to an HTML parser, and HTML-entity escaping would **corrupt** values consumed directly rather than `JSON.parse`'d (e.g. the BRC-100 auth `body`, which is raw JSON full of `"`). Dropped `< > &`.

**Final fix:** extracted `escapeJsonForJs` to header-only `cef-native/include/core/JsStringEscape.h` (CEF-free → unit-testable), **hardened** it (`"`→`\"`; NUL→` ` not `\0`; U+2028/U+2029→` `/` ` via 3-byte UTF-8 detection), and routed the 3 bypass sites through it: `brc100_auth_request` (5 dApp fields, previously **no** escaping), `tab_list_response` (replacing a weaker ad-hoc escaper that missed `'`), and `omnibox_select` (`direction`, internal but same class). New GoogleTest `js_string_escape_test.cpp` (15 tests pinning the attack cases). **Build:** `hodos_tests` 54/54 green; full `HodosBrowserShell` recompiles clean. **Mac-parity:** none (cross-platform C++). Live smoke (auth overlay / tab-title-apostrophe / omnibox) deferred to next dev run. Defense-in-depth follow-up logged as **FU2**.
