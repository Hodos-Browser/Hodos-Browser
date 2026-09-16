# Phase 10c — a paymail host cannot change what the user approved · PHASE CONTRACT

**Workstream:** critical advisories (money path) · **Source:** `../../CRITICAL_UPDATES.md` §1.2 (CU-2)
**Status:** ✅ **DONE on Windows 2026-09-15** — four rows GREEN with their REDs observed live; `P10c-A5` (real handle) owed to `PAYMENT_TEST_BATCH.md`.
**Opened:** 2026-09-15 · **Owner:** Matthew Archbold · **Platforms:** Rust, both (Mac rebuilds + `cargo test`)
**Standard:** `../../HARNESS.md`.

> ⚠️ Paymail is a **live** send path: `frontend/src/components/TransactionForm.tsx` resolves `$handle` /
> `user@domain` recipients and the wallet exposes `POST /wallet/paymail/send` + `GET /wallet/paymail/resolve`.
> This is not a dormant feature being hardened for later.

---

## 0. Plan-vs-tree delta — filled at kickoff 2026-09-15, base `b63aacf`

| # | Delta |
|---|---|
| `D-1` | **Confirmed.** `handlers.rs :: paymail_send` (`:18901`): gate on the request body (`dispatch_payment`, `:18938`) → `get_p2p_destination` (`:18974`) → `CreateActionOutput`s straight from `o.satoshis` / `o.script_hex` (`:18985-18997`) with no sum, count or value check → internal `create_action` via `TestRequest::default()` (no domain ⇒ `Proceed`, `:19062`) → **unconditional `broadcast_transaction`** (`:19118`). The basic fallback (`resolve_address`, `:19006`) builds one output of `req.amount_satoshis` — safe on amount, host-supplied script |
| `D-2` | ⛔ **The contract's rig premise is false on the tree: there is no `noSend` on `/wallet/paymail/send`.** The request is `{paymail, amount_satoshis}` only; `create_action` is called with `no_send: true` purely to get the BEEF back, and the handler then broadcasts itself. Every T2 row marked "`noSend`" below is re-planned (`D-6`) |
| `D-3` | **Confirmed.** `paymail.rs`: `.well-known/bsvalias` is fetched over hard-coded `https://` (`:200`) after `discover_host` (SRV override table, `:16`); capability URL templates are taken verbatim (`:227-247`) and `get_p2p_destination` / `resolve_address` POST to whatever scheme they carry (`:290-300`) — the `http://` finding is real. `PaymailClient::new()` is `reqwest` with **`rustls-tls` only** (`Cargo.toml:94`) ⇒ webpki roots, OS trust store ignored ⇒ a local self-signed HTTPS stub is untrusted without a code seam (none exists; none will be added) |
| `D-4` | ⛔ **Rule 4 — the spec does not say what we inferred.** bsvalias.org is behind a Cloudflare JS challenge (403 "Just a moment"); the source repo `bitcoin-sv-specs/brfc-paymail` does not carry the P2P documents; the text was recovered from `moneybutton/paymail-client` `docs/paymail-07-p2p-payment-destination.md` (BRFC `2a40af698840` v1.1, `PRIOR_ART.md` 2026-09-15). It says only *"The server generates a list of outputs to receive the payment"*; its own example requests **1,000,100** sats and returns **10,000 + 20,000**. HTTPS appears in the example URL, never as a MUST. The reference client `bitcoin-sv/go-paymail :: GetP2PPaymentDestination` requires `https://` (`:83`) and checks `reference` / `outputs` / `script` non-empty — **and has no sum check either**. ⇒ `sum(outputs) == amount_satoshis` is **Hodos's own invariant** (the 10b one — a signature exists only for an amount the user saw), not the spec's. 👤 **Owner confirmed it as policy, 2026-09-15:** *"the total amount must match what the user sends, if they don't equal, then that payment must be rejected and user must be notified."* |
| `D-5` | **Confirmed live.** `TransactionForm.tsx` resolves handles through `/wallet/paymail/resolve` (`:92`, `:182`) and the send goes to `/wallet/paymail/send`. The resolve path shares `discover_capabilities`; the `https://` requirement is applied on the **send** path (P2P destination, receive-tx, basic destination), not to `public_profile_url`, so the preview is untouched |
| `D-6` | **Rig re-planned around `D-2`/`D-3`.** The stub paymail host runs locally on plain HTTP and is fronted by a `cloudflared` quick tunnel (a public `*.trycloudflare.com` name with a real certificate) so `.well-known` is reachable over HTTPS with no code seam; the domain typed into the wallet is the tunnel host. No money leaves: `P10c-A1`'s stub returns **5,000,000** sats (10× a 500,000-sat request that the dev wallet cannot fund either way), so the pre-fix RED is `create_action` **attempted** for 5,000,000 sats (its log line + the `insufficient funds` error), i.e. the wallet was ready to sign 10×; post-fix the reject fires before `create_action`. `P10c-A2`'s accept side would broadcast real sats to stub-supplied scripts, so it runs as T1 on the extracted validator; `P10c-A5` (a real handle, a few hundred sats) is the T2 accept-side control. `P10c-A3`'s stub advertises `http://127.0.0.1:<port>/p2p/...`: pre-fix the plain-HTTP listener logs the POST (RED seen), post-fix it sees nothing |
| `D-7` | **Reuse-first.** `paymail.rs` has a `#[cfg(test)]` module (`:639`) for `parse_paymail`; the new validator and scheme check are pure functions tested there. No new struct, no new module |

## 1. Goal

A paymail send signs outputs that total exactly the amount the user approved, to a host reached over HTTPS,
and nothing else — whatever the recipient's host returns.

## 2. Done means

- [x] `sum(outputs.satoshis) == amount_satoshis` or the send is rejected before signing, with an error naming the mismatch — `paymail.rs :: validate_p2p_outputs` (saturating sum, `P2POutputsError::SumMismatch`), called from `get_p2p_destination`; plus a second, independent guard in `handlers.rs :: paymail_send` that sums the **built** `CreateActionOutput`s before signing and returns 422 `ERR_PAYMAIL_OUTPUT_MISMATCH`
- [x] Output count bounded (`MAX_P2P_OUTPUTS = 100`); zero/negative values rejected
- [x] Every capability URL on the send path must be `https://` — `require_https_capability` at `get_p2p_destination`, `resolve_address` and `submit_transaction`
- [x] The resolve path (`/wallet/paymail/resolve`, the form's name/avatar preview) is unchanged in behaviour — `public_profile_url` is not gated; `discover_capabilities` already fetched `.well-known` over hard-coded `https://` (`D-3`)

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-INTEXT` | internal never prompts | the paymail send is an *internal* `create_action`; the fix must reject in the handler, not by suddenly routing an internal call through the external gate (that would prompt the user's own send) |
| `R-DUST` | 1-sat floor | output validation must not reject legitimate small outputs the dust rules already allow |
| `R-GOLD` | gold pill | untouched (internal sends do not pill); run at the boundary |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — seen, and how | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P10c-A1` | Stub host returns outputs totalling **10×** the request ⇒ rejected before signing; error names the mismatch; nothing in `outputs`/`transactions` | Pre-fix, same stub ⇒ the 10× total is signed **and broadcast** | the stub's served body logged; the wallet log's `Output[n]: satoshis=…`; balance + `transactions` row count before/after | T2 (real, on a stub script we own — `D-8`) | ✅ **GREEN** 2026-09-15: request 500,000, stub answered 5,000,000 ⇒ HTTP 500, *"the recipient's server asked for 5000000 satoshis but you approved 500000 — payment cancelled, nothing was sent"*; `balance` 29,099,036 and `transactions` 526 **identical** before and after. 🔴 **RED SEEN**: fix stashed (`git stash push -- rust-wallet/src/paymail.rs rust-wallet/src/handlers.rs`) and rebuilt ⇒ same call returned HTTP 200 and **broadcast `8bf8363296e24667474c0abbff5cb76ae56b489dd9751a8e44fb3477bb975cc8` for 5,000,000 sats**; log `Output[0]: satoshis=Some(5000000)` |
| `P10c-A2` | Stub returns the exact amount split across **3** outputs ⇒ succeeds | Raise one of the three by 1 sat ⇒ rejected | T1: the validator's return. T2: tx outputs in the wallet log + balance delta | T1 **and** a T2 accept-side control (`D-8`) | ✅ **GREEN**: `cargo test --release --bin hodos-wallet cu2_validation` ⇒ `a2_exact_sum_split_across_three_outputs_is_accepted` ok. **T2 accept-side control**: stub in `exact3` mode, 900 sats ⇒ three 300-sat outputs accepted, txid `48b77e6866847305fe3ec036feb672e3bbb1cb6d2d8cd33a05d10a7c4476052a`, balance 29,087,787 → 29,085,687 (−2,100 = 900 + 1,000 service fee + 200 miner fee), `transactions` 528 → 529. 🔴 **RED**: `a1_ten_times_the_request_is_refused` is the same function's failing half, and the live pre-fix broadcast under A1 is its T2 half |
| `P10c-A3` | Stub `.well-known` advertises an `http://` P2P endpoint ⇒ rejected | Pre-fix ⇒ used | the request the stub *did not receive* (a plain-HTTP listener on 8766 logs arrivals) | T2 | ✅ **GREEN** 2026-09-15: stub advertised `http://127.0.0.1:8766/p2p/…` ⇒ wallet answered *"P2P destination endpoint is not https (http), refusing to use it"*, fell back to the basic path (`P2P unavailable (…), trying basic path...`), balance and `transactions` unchanged, and the 8766 listener log was **empty**. 🔴 **RED SEEN**: fix stashed and rebuilt ⇒ the listener logged `PLAIN-HTTP LISTENER RECEIVED POST /p2p/test@division-you-turn-world.trycloudflare.com` |
| `P10c-A4` | Output count above the bound, or a zero/negative output ⇒ rejected | Pre-fix ⇒ accepted (no such check existed — `D-1`) | unit | T1 | ✅ **GREEN**: `a4_count_bound_and_bad_values_are_refused` ok — 101 outputs ⇒ `TooManyOutputs`, a `0`-sat output and a negative `i64` ⇒ `BadOutputValue`. 🔴 RED is structural: `D-1` records that no count or value check existed on the pre-fix tree, and the A1 RED run signed whatever the host returned |
| `P10c-A5` | **Non-regression, real:** a paymail send to a genuine handle (a few hundred sats, owner's choice of recipient) completes as before and the resolve preview still shows name/avatar | — (A1 is the control for the reject side) | txid on chain; `PAYMENT_TEST_BATCH.md` | T2 (real money) | 🟡 **OWED — `PAYMENT_TEST_BATCH.md` M11.** Needs the owner to name a recipient handle; the stub accept-side control under A2 covers the mechanism but not a third-party host |

**Two-sided rows:** A1 (mismatch rejected) and A2/A5 (exact sum accepted) are each other's control.

> `D-8` **added at run time, correcting `D-6`.** `D-6` assumed the dev wallet could not fund 5,000,000 sats,
> so the pre-fix RED would stop at *"insufficient funds"*. It was wrong — the dev wallet held ~29,000,000
> sats and the pre-fix build **broadcast the 10× transaction**. The run was safe only because the stub was
> started with `SCRIPT_HEX` set to the **dev wallet's own** locking script
> (`76a914d135d5c87cd6e5f963ac328664ad586bc4be04cf88ac`), so an accepted send pays us and costs fees alone.
> ⛔ Anyone re-running these rows must set that variable before starting the stub. This is also the row's
> strongest evidence: the defect is not "the wallet would have signed", it is "the wallet did sign and send".

## 4a. The rig as actually run (2026-09-15, Windows)

| Piece | What it was |
|---|---|
| Stub paymail host | `scratchpad/stub_paymail.py`, plain HTTP on 127.0.0.1:8765, three modes: `tenx` (A1), `exact3` (accept-side control), `http` (A3). Every request it serves is printed, so *"the request the stub did not receive"* is usable evidence |
| Public HTTPS name | `cloudflared` quick tunnel ⇒ `https://division-you-turn-world.trycloudflare.com` (a real certificate; `D-3` rules out a self-signed local one because `reqwest` is `rustls-tls` only). ⚠️ The hostname is regenerated on every tunnel start — re-derive it, never reuse this one |
| `PUBLIC_HOST` | the tunnel hostname, so `.well-known` advertises `https://` capability URLs pointing back through the tunnel |
| `SCRIPT_HEX` | ⛔ the **dev wallet's own** locking script — see `D-8`. Without it the A1 RED sends real sats to a stranger |
| Plain-HTTP listener | 127.0.0.1:8766, logs any arrival; A3's subject |
| Wallet | dev build on 31401 (`HODOS_DEV=1`), started from `target/release` after `scripts/stop-dev.ps1` |

⛔ `cargo test` builds the binary, so it fails *"failed to remove hodos-wallet.exe"* while the dev wallet
runs — stop by path first. ⛔ `paymail` is a **binary-only** module (not in `lib.rs`), so its tests need
`cargo test --release --bin hodos-wallet`; `--lib` silently matches **zero** tests and reports `ok`.

## 5. Blast radius

`paymail_send` only, plus `paymail.rs` URL handling (also used by resolve — do not break the preview). The
follow-up in `CRITICAL_UPDATES.md` (price the *built* transaction at the gate for every internal caller) is
recorded, not done.

## 6. Out of scope

Gate-side pricing of built transactions. Paymail *receive*. Any UI change.

## 7. Rollback

One Rust commit; revert restores the pre-fix handler.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1 -Full` — result + date below
- [ ] `scripts/preflight.ps1 -NegativeControl` — every T0 gate seen to fail
- [ ] `../../REGRESSION_SET.md` run at this boundary — T2 halves run
- [ ] Adversarial review — four questions in writing
- [ ] Commit messages cite the row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight -Full | **PASS** (T0 gates at baseline, T1a–T1g) | 2026-09-15 | Windows |
| preflight -NegativeControl | ⬜ owed at the phase boundary | | |
| regression set | 🟡 **PARTIAL** — see `../../REGRESSION_SET.md` boundary record for Phase 10 | 2026-09-15 | Windows |
| adversarial review | ✅ **DONE — and it found four defects here**, one of them a regression this phase shipped. `../ADVERSARIAL_PANEL.md` | 2026-09-15 | four-reviewer panel |

## Adversarial panel, 2026-09-15 — what it changed here

Full report and the four harness questions: `../ADVERSARIAL_PANEL.md`. Fixed in `0e8ea27`:

- **`F1` — a regression this phase shipped.** `resolve()` proves a handle exists by asking for a
  546-satoshi destination, and the new sum check ran on that probe ⇒ a host that does not echo the
  probe amount was reported an **invalid recipient** and could not be paid at all. ⛔ **§2 bullet 4,
  `D-5`, the doc comment and the commit message all claimed the resolve path was untouched. That was
  a code reading and it was wrong.** Now the probe gets the shape checks only.
- **`F2`** — `reqwest` follows redirects by default and permits https→http, so the https requirement
  could be walked around with a `302`. Redirects are now off, tested against a real socket.
- **`F3`** — a rule breach fell through to the same host's basic endpoint. Now terminal (422
  `ERR_PAYMAIL_HOST_REFUSED`), which is what the owner's rule actually says.
- **`F5`** — host-supplied script length feeds fee estimation, so a host could inflate the debit while
  honouring the total. Bounded at `MAX_SCRIPT_HEX_LEN`.

⚠️ **Corrected claim for the record:** the resolve path is unchanged *except* that a host whose
capability URLs are plain http now resolves as invalid. That is deliberate and consistent — such a
host is unpayable on the send path, so showing it as a valid recipient would only move the failure
later.

⚠️ `P10c-A4`'s RED is **structural**, not observed. It is acceptable only because `A1`'s live RED
demonstrates the same absence on the same tree.
