# Track 0 — `reqwest` 0.11 → 0.12+ (the wallet's TLS certificate validator)

**Opened:** 2026-09-15, by owner decision at the beta.3 Phase 9 dependency review
(`../../DevOps-CICD/DEPENDENCY_VERIFICATION.md`, "Freshness review — 2026-09-14").
**Status:** 🔭 SCOPE + RESEARCH NOTES ONLY. No phase contract, no design. It needs a real scoping pass
(`../../SCOPING_PROCESS.md`) before code — this file is the outline that pass starts from.
**Standard:** `../../0.4.0-beta.3/HARNESS.md` + `../HARNESS_DELTA.md`.
**Placement:** 👤 **first in beta.4, ahead of the four settled tracks** (owner, 2026-09-15). The Guard → 1Sat →
OpNS → Backup order is untouched; this sits in front of it because it is a dependency change on the money
path, not an asset-layer feature, and beta.3's release verification should not absorb it.

---

## Goal

> **Every outbound HTTPS call the wallet makes validates the server's certificate with a library that has
> no open advisories — without changing what the wallet sends, receives, or signs.**

## Why this exists — in plain terms

The wallet talks to outside servers (WhatsOnChain, GorillaPool/ARC, MessageBox, the price APIs). Each call
goes through **`reqwest`** (the HTTP client) and, underneath it, **`rustls-webpki`** — the piece that decides
whether the certificate the server presents really belongs to that server. That check is what stops a
network attacker from impersonating a server and feeding the wallet a wrong balance, a wrong price, or
swallowing a broadcast.

📏 `cargo audit` on 2026-09-14 (`rust-wallet/Cargo.lock`): the shipped validator is **`rustls-webpki 0.101.7`**
and it carries three advisories:

| Advisory | What it is | Patched in |
|---|---|---|
| RUSTSEC-2026-0098 | Name constraints for **URI names** incorrectly accepted | `≥ 0.103.12` |
| RUSTSEC-2026-0099 | Name constraints accepted for certificates asserting a name type they should not | `≥ 0.103.12` |
| RUSTSEC-2026-0104 | Reachable **panic** parsing a certificate revocation list | `≥ 0.103.13` |

Two of the three are "accepts what it should reject". Nobody has to have exploited them for that to be the
wrong lock on the door of a money-handling program. The fix exists **only** in the `0.103` line of the
validator, which ships with **`rustls 0.23`**, which ships with **`reqwest 0.12+`**. There is no way to take
the fix without moving the HTTP client a major version — hence a track, not a `cargo update`.

The same move also clears **RUSTSEC-2026-0258** (`h2 0.3.27`, HTTP/2 unbounded empty DATA frames, DoS) —
`h2 0.4` comes with `reqwest 0.12`.

## What is NOT in scope

- Any change to what the wallet signs, derives, or broadcasts. Invariant #3 stands: this is transport only.
- `adblock-engine` also uses `reqwest 0.11` (default features ⇒ schannel on Windows, not rustls). It has the
  `h2` advisory but not the webpki ones. Bump it in the same sitting **only if** the wallet bump proves
  uneventful; otherwise it is its own small item.
- Bumping anything else in `Cargo.toml` "while we are in there". One dependency, one track.

## Where the client is used — the surface the scoping pass must enumerate (starting list, from grep)

| Module | What it does over HTTPS | Why it matters for the bump |
|---|---|---|
| `rust-wallet/src/authfetch.rs` | BRC-103 AuthFetch: 401 challenge → signed nonces → re-send with auth headers | The most bespoke use — custom headers, response header reads, retries. Most likely to hit an API rename |
| `rust-wallet/src/messagebox.rs` | MessageBox send/receive/acknowledge over AuthFetch | Rides on the above; also the PeerPay auto-accept path (`monitor/task_check_peerpay.rs`) |
| `rust-wallet/src/price_cache.rs` | WhatsOnChain → CoinGecko → MEXC price chain | Fallback chain + 5-min TTL; timeouts and error classification must survive |
| UTXO / broadcast paths (`utxo_fetcher.rs`, ARC / WhatsOnChain broadcast in `handlers.rs`) | balances, mempool, broadcast, `check_tx_exists_on_chain` | **The money path.** ARC txStatus ladder semantics must be untouched (`reference_arc_tx_status_ladder`) |
| `handlers.rs :: pay_402` / BRC-121 | the paid retry to a 402 server | Custom headers again |
| `Cargo.toml` line: `reqwest = { version = "0.11", features = ["json", "rustls-tls"], default-features = false }` | | ⚠️ `cargo outdated` already warns: *"Feature rustls-tls of package reqwest has been obsolete in version 0.13.5"* — the feature is renamed |

⛔ **The scoping pass must produce the real list** (`grep -rn "reqwest" rust-wallet/src`), not trust this one.

## Research notes — what to read before designing (rule 4 and rule 5)

1. **`reqwest` changelog 0.11 → 0.12 → 0.13.** Known from the 0.12 release: hyper 1.0 underneath, `rustls 0.22/0.23`,
   TLS feature flags reorganised (`rustls-tls` → the `rustls` / `__rustls` family; read the exact names for the
   version chosen), `Response::json` / `bytes` unchanged, connector and timeout builder changes. Read it; do
   not infer.
2. **`rustls` 0.23 crypto provider.** `rustls 0.23` requires choosing a crypto provider (`ring` or `aws-lc-rs`);
   `reqwest` picks a default through its features. ⚠️ **`aws-lc-rs` needs a C/C++ toolchain and, on Windows,
   NASM** — which our CI runner may or may not have. `ring` is the safe default for us. Decide it explicitly and
   write down why. This is the one item most likely to turn a "bump" into a build fight.
3. **Root store.** `reqwest 0.11` with `rustls-tls` used `webpki-roots` (bundled Mozilla roots). Check what
   0.12/0.13 use by default (`webpki-roots` vs `rustls-native-certs` / platform verifier) — a change here
   changes *which* certificates are trusted, and on a user's machine a corporate proxy or an AV product's
   MITM root behaves differently under each. Decide and test both a normal host and a machine with a
   platform-installed root.
4. **`wallet-toolbox` (TypeScript/Go) — how do conforming wallets pin or verify server identity?** Per the
   root `CLAUDE.md` prior-art rule: check whether any BSV SDK layers certificate pinning or extra checks over
   plain TLS for ARC/WhatsOnChain. Expected answer: no, plain TLS with the platform store. Record it in
   `../../PRIOR_ART.md` either way.
5. **Chromium's stance** is irrelevant here — the wallet is a separate Rust process with its own TLS; libcef's
   BoringSSL is not on this path (`DEPENDENCY_VERIFICATION.md`, symbol coexistence).

## Outline of the track (for the scoping pass to confirm or cut)

| # | Step | Produces |
|---|---|---|
| 0.1 | **Enumerate** every `reqwest` use and every response-shape assumption (headers read, status codes branched on, timeouts, retries) | the real surface table above, with file :: symbol |
| 0.2 | **Choose the target** (`0.12.x` vs `0.13.x`), the crypto provider, and the root store — written with reasons | a one-page decision record |
| 0.3 | **Bump + compile** on a branch; fix renames; **no behaviour edits** beyond what the API forces | green `cargo build --release` + `cargo test` on both workspaces |
| 0.4 | **Live smoke against the dev wallet**: balance fetch, mempool-aware UTXO sync, a real broadcast (small), price chain incl. a forced fallback, BRC-103 auth exchange + MessageBox poll, one BRC-121 paid retry | evidence rows with GREEN / RED / SUBJECT |
| 0.5 | **Negative controls**: point the client at a host with a bad certificate (expired / wrong name) and prove it is **refused** — before and after — so the bump is seen to keep the door locked, not just to compile | the RED halves |
| 0.6 | `cargo audit` shows the four advisories gone; `DEPENDENCY_VERIFICATION.md` review table updated | close-out |

## Evidence rows the contract will need (names reserved)

`B4S0-A1` balance/UTXO parity before vs after · `B4S0-A2` broadcast + ARC status ladder unchanged · `B4S0-A3`
AuthFetch/MessageBox round trip · `B4S0-A4` price fallback chain · `B4S0-A5` bad-certificate hosts refused
(expired, wrong name, self-signed) · `B4S0-A6` `cargo audit` clean of the four ids. Standing rows from
`../../0.4.0-beta.3/REGRESSION_SET.md` that this touches: `R-INTEXT` (unchanged, but the transport under it
moves), `R-PERIM` (T1), `R-DUST` (T1).

## Risks worth naming now

- **Silent trust-store change** (note 3) — the class of change that "works on the build host" and fails on a
  user's machine behind a corporate proxy.
- **`aws-lc-rs` build requirements** (note 2) on the `windows-2022` and `macos-15` runners.
- **AuthFetch header handling** — the most hand-rolled HTTP in the wallet; any change in how `reqwest`
  normalises header names/values shows up here first.
- **Timeout semantics** — `reqwest 0.12` changed how connect vs total timeouts compose; the wallet's
  transport timeouts (2 s / 5 s / the 30 s broadcast, `kWalletBroadcastTimeoutMs` on the C++ side) must be
  re-measured, not assumed.

## Related

- `../../0.4.0-beta.3/TICKET_dependency_freshness_review.md` — the review that surfaced this
- `../../DevOps-CICD/DEPENDENCY_VERIFICATION.md` — policy item 6: a review reports, a bump is its own change
- Memory: `reference_arc_tx_status_ladder` (ANNOUNCED ≠ success), `project_cache_no_poison_on_failure`
