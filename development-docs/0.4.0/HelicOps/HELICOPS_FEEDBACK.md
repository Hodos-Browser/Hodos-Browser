# Feedback for the HelicOps Team

**Created:** 2026-06-09 · **Shared:** 2026-06-22 · **Status:** ✅ Adjudicated
**Audience:** the HelicOps audit tool/team. **Spirit:** a genuine thank-you plus partnership
feedback — HelicOps is a young tool with real promise, and the notes below are meant to help it (and
our next audit together) get even sharper.

> Adjudicated via a 17-agent zero-trust verification of all 479 findings against current source, so we
> could give you precise, code-cited feedback. Companion docs: `AUDIT_FIX_TRACKER.md` (what we agreed
> with and are fixing), `HELICOPS_META_ANALYSIS.md` (report-as-product assessment).

## Overall — thank you, this was a valuable first run

The headline result speaks for itself: HelicOps surfaced **two critical secret-on-disk bugs we had
missed** — a full mnemonic and full certificate keys written to disk — plus several other real,
fixable issues (an arbitrary file-write in backup, a macOS command injection, and a systemic
JS-escaping gap). Those alone more than justified the audit, and we've already fixed or scheduled
every confirmed item (`AUDIT_FIX_TRACKER.md`). For an automated pass to catch the exact class of
"simple but catastrophic" bug humans overlook is exactly the value we hoped for.

We went deep on verification not to grade the tool, but because it's new and concrete, evidenced
feedback now is the fastest way to make it great. The single biggest opportunity we see is
**severity calibration via dataflow/taint analysis** plus **root-cause clustering** — most of the
over-firing traces to token pattern-matching without source→sink tracing, and adding that lifts the
signal-to-noise dramatically. Everything below is organized to support that, with per-finding
evidence; the highest-value wins are recapped in the final section (§5).

## 1. Findings we adjudicated as false positives (with evidence)

| Finding (cat / file:line) | Claim | Why it's wrong | Evidence |
|---|---|---|---|
| Weak randomness for crypto — **all 18** (`backup.rs`, `crypto/brc2.rs`, `crypto/pin.rs`, `wallet_repo.rs`, `handlers.rs:562,15283-4`, `certificate_handlers.rs`, …) | `rand::thread_rng()` / `rand::random()` = weak crypto randomness, medium. | `rand::thread_rng()` and `rand::random()` **are** CSPRNGs (ChaCha-block RNG, OS-seeded) in `rand` ≥0.8 — the correct choice for keys/nonces/IVs/salts. Rule appears to match the `rand::` token prefix. | rand crate docs (ThreadRng/StdRng); one flagged site is even in `archive/old-tests/`. |
| Mutable global state — **33** (16 in `third_party/quirc/*.c`, 12 in `simple_handler.h`, `AdblockCache.h`, `Tab.h`, `simple_app.h`, a TS `displayName`) | High-risk mutable globals. | Detector fires on **forward declarations, base-class lists, member-field declarations, and a TypeScript `displayName` string** — none are mutable global state. 16 are in vendored third-party QR code (`quirc`) we don't own. The few real statics are the **deliberate, documented CEF singleton pattern** (`cef-native/src/core/CLAUDE.md` → "Singleton Pattern"). | `core/CLAUDE.md` Meyer's-singleton section; quirc is upstream `dlbeer/quirc`. |
| Command exec (critical) — `simple_handler.cpp:7400` `popen("pbcopy")` & `:7654` `popen("pbpaste")` | Critical command execution via shell. | Constant command name, **no user input in the command string**; clipboard data is piped via `fwrite`/read, never shell-interpreted. The in-code comment literally says *"avoids shell escaping / injection issues."* This is the **safe** pattern. | `simple_handler.cpp:7399` comment; macOS-only branch. |
| SQL injection (critical) — `handlers.rs:2938` `DROP TABLE IF EXISTS "{}"` | String-built SQL → injection. | `table` iterates an **internal hardcoded `table_names` list** (a wallet-wipe routine), not request input. Table/identifier names **cannot** be bound parameters in SQL anyway — interpolation of a vetted internal constant is the only option. No external taint. | `handlers.rs:2937` `for table in &table_names`. |
| Unsafe serialization / unsafe code (high) — `crypto/dpapi.rs` (6) | Unsafe memory-safety risk. | Windows DPAPI is a **C FFI** — `unsafe` + `DATA_BLOB` struct (de)serialization is **required and idiomatic**, bounds-checked, mirrors Chrome/Edge. Not a defect. | `crypto/dpapi.rs`; `CryptProtectData`/`CryptUnprotectData` ABI. |
| Prototype pollution (high) — most of 7 (`WalletPanelPage.tsx`, `TabBar.tsx`) | Prototype pollution. | Mostly **numeric array index writes** and bracket-writes with **locally-sourced trusted keys** (e.g. the user's own `/listOutputs` basket name) — no attacker-controlled key merged into an object prototype. Detector doesn't require an untrusted key into a non-array object. | per-site verification in cluster C10. |
| Insecure deserialization (high) — `useBitcoinBrowser.ts:42` + others | Untrusted deserialization. | Plain `JSON.parse` of **CEF IPC** payloads (browser-process → render, not arbitrary web `postMessage`) with no reviver/revival sink. Also `useBitcoinBrowser.ts` **does not exist** in current source (CLAUDE.md references `useHodosBrowser.ts`). | `frontend/src/bridge/`; CEF IPC is C++-originated. |
| Path traversal (high) — ~15 of 22 (`action_storage.rs`, `json_storage.rs` dead `migrate_json_to_database`, `Logger.cpp`, `generate-appcast.py`, test code) | Unvalidated file path traversal. | Internal/fixed paths, **dead migration code** (zero callers), `#[cfg(test)]` code, a build script, and one **HTTP URL miscategorized as a file path**. Only `backup.rs` (4 sites, →**F7**) is real. | per-site verification in cluster C9. |
| Blocking-in-async (high) — most of 16 (`adblock-engine/src/engine.rs`) | Blocking call stalls executor. | Filter-list file reads on **startup + 6-hour reload**, not a hot path; some fire on a **type signature** (`db: &Mutex<…>`) mistaken for a blocking call. Negligible executor impact. | `engine.rs` reload cadence. |
| Untrusted→code-exec (high) — `handlers.rs` `process_action` site | Injection sink. | This is a **Rust unwrap/panic**, no JS sink — category misfile; already covered as a DoS/unwrap finding. Double-counted. | cluster C8 cross-check. |

**Bare-unwrap "DoS" cluster (284):** not listed line-by-line here — see §3 (severity) and `AUDIT_FIX_TRACKER.md` **F4**. The large majority are infallible/guarded/constant unwraps or per-request resets (low), **not** high-severity DoS.

## 2. Findings that needed context we didn't provide

| Finding | What they missed | The actual intent |
|---|---|---|
| Hardcoded secret (critical) — TAAL ARC key, `services/providers/arc_taal.rs:16` | That it's deliberate. | **Fair call-out, accepted as CLARIFY.** Intentional, TAAL-*recommended*, rotated manually at build time; no env-var alternative on TAAL's side today. A live key does sit in git history — tracked honestly in `development-docs/0.4.0/BROADCAST_AND_EXPLORER_REVIEW.md` for the real long-term fix (wallet auto-mints/pays for its own broadcast credential as ecosystem paywall protocols mature). Our brief should have disclosed this key like it disclosed the service-fee address. |
| `popen`/`system` flags | The platform split. | Windows uses safe `CreateProcessW`; only the macOS `system()` branch (**F5**) is exploitable. A platform-aware analyzer would rate them differently. |
| Singletons / "mutable global state" | The architecture. | Process-per-overlay + Meyer's singletons are the **deliberate, documented** isolation model (`cef-native/CLAUDE.md` invariant #5, `core/CLAUDE.md`). Our brief listed the singletons but didn't flag them as intentional-by-design. |
| dpapi `unsafe` | At-rest encryption design. | Win32 DPAPI FFI necessarily uses `unsafe`; macOS Keychain side is a known stub. Brief mentioned this but the tool had no FFI exemption. |

## 3. Ambiguous / low-confidence findings (severity miscalibration)

| Finding | Issue | Suggested calibration |
|---|---|---|
| 284× "unhandled unwrap / panic (DoS), **high**" | Flat high-severity, no runtime model. A bare Actix handler panic is caught at the tokio task boundary (`catch_unwind`) → **per-request connection reset on a localhost-only/CORS-locked/CEF-fronted port**, worker survives, no cumulative degradation. | Split into 3 tiers: **(i)** request-data unwrap = Low; **(ii)** `std::sync::Mutex .lock().unwrap()` on a shared handle = **High** (poison cascade — the real bug, which the flat rule both buried and under-explained); **(iii)** infallible/guarded constant = Info/none. |
| C6 secret-to-log rated flat **high** | Under-rated. | Full-mnemonic-to-disk and full-32-byte-key-to-disk should **auto-escalate to Critical** (total compromise). |
| C11 "critical" tier | This run, the critical tier was mostly false-positive with no confirmed-critical findings. | Highest-payoff calibration: gating critical/high behind confirmed attacker-reachable taint would make this tier the most trusted part of the report. |
| QR / screen-capture splice — `simple_render_process_handler.cpp:~1003,1023` | Genuinely ambiguous (we agree). Raw-JSON splice with zero escaping; code comment asserts "our own scanner output." But QR/capture payloads decode **attacker-chosen bytes**. | Fold into the **F6** encoder; don't trust the scanner-output boundary. We're treating it as CLARIFY pending confirmation. |
| `beef.rs:1268` recursion | Filed as perf. Bounded by parsed BUMP level count (tiny for real blocks), but a crafted proof declaring a huge level count is a **parse-validation/DoS** concern, not perf. | Re-file as robustness; confirm BUMP level-count is capped on parse. |

## 4. Process / tooling feedback (for HelicOps itself)

1. **Add dataflow/taint analysis before assigning severity.** Every over-rated cluster (C1–C5 DoS, C9 path-traversal, C11 criticals, C8 injection) stems from token pattern-matching with no source→sink tracing.
2. **Implement root-cause COLLAPSE.** ~245 unwrap findings map to ~3 defects. Reporting them as 245 flat-high line-items inflates counts and can obscure the one actionable fix (poison-safe locking). Clustering "N instances → 1 remediation" and reporting the mechanism rather than each symptom line would make the report much easier to act on.
3. **Model the framework runtime.** Teach the analyzer Actix/tokio panic isolation (bare panic ≠ worker crash) **and** that `std::sync::Mutex` poisoning + shared handle + no `clear_poison` **is** the real durable DoS. Severity = frequency × data-scale × reachability — none currently modeled.
4. **Highest-impact detector tuning.** A few detectors over-fired this run and would gain the most from refinement: *Mutable global state* (fired only on false positives here — forward decls, base-class lists, params, a TS `displayName`). *Weak randomness* (whitelist `rand`≥0.8 `ThreadRng`/`random`; fire only on `SmallRng`/`seed_from_u64`/Xoshiro). *Prototype pollution* (require bracket-assignment into a non-array object with an untrusted key). *Insecure deserialization* (skip plain `JSON.parse` without a reviver). *memcpy/unsafe* (require absence of a clamped/validated length).
5. **Snippet capture — the highest-leverage trust fix.** The most severe findings (the mnemonic + private-key C++ leaks) and ~12 injection findings carried a placeholder snippet `"requires login"` that doesn't appear anywhere in our source. Because a reviewer skimming snippets could overlook the most important real bugs, we'd suggest either fixing C++ snippet extraction or always preferring the `file:line` pointer over the snippet when they disagree.
6. **Anchor on stable signatures, not line numbers.** Cited lines drifted **+550 to +2900** here (and some files moved entirely, e.g. the TAAL key relocated `handlers.rs` → `services/providers/arc_taal.rs`). Anchor on function name + content hash so findings survive normal file growth. Emit wider snippet windows for re-anchoring; suppress unanchorable bare-token findings.
7. **Respect scope/reachability.** Honor `#[cfg(test)]` gating (≥8 FPs), detect dead/zero-caller code (the `migrate_json_to_database` FPs), exclude vendored `third_party/`, and distinguish a held-across-`await` lock from a scoped-and-dropped one.
8. **Declare coverage explicitly.** State files-scanned (108) vs files-in-repo (far larger — `handlers.rs` alone is ~18.5k lines), mark which subsystems got only syntactic scanning, and state plainly that **no protocol/semantic analysis was performed**. (We told you not to expect BSV/BRC depth — that's fine — but it should be *declared*, not implied by "complete coverage.")

## 5. What worked well (genuine, high-value)

- **The two critical secret-on-disk leaks (F1, F2)** — full mnemonic to `debug_output.log` and full
  cert symmetric keys at info level. These are real, severe, and we'd missed them. **This is the
  single best outcome of the audit** and exactly the class of "dumb but catastrophic" bug an
  automated pass should catch. Even mis-rated as plain "high," surfacing them was the win.
- **The secret-to-log cluster (C6) broadly** — ~18/20 real. High-signal, directly actionable.
- **The render-process injection cluster (C8)** — despite garbage snippets, it correctly pointed at a
  real systemic gap: inconsistent/incomplete JS-string neutralization (`escapeJsonForJs`). One genuine
  cross-context injection (`brc100_auth_request`) and a real encoder-hardening task (**F6**).
- **The `/wallet/backup` arbitrary-write (F7)** and **macOS `ProfileManager` command injection (F5)**
  are both real and worth fixing — found without protocol knowledge, purely from structure.
- **Cross-file analysis intent** (taint + "missing neutralization vs comparable safe sites") is the
  right direction — it just needs real dataflow behind it to stop over-firing.
