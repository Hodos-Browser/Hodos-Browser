# Phase 10c — a paymail host cannot change what the user approved · PHASE CONTRACT

**Workstream:** critical advisories (money path) · **Source:** `../../CRITICAL_UPDATES.md` §1.2 (CU-2)
**Status:** ⬜ NOT STARTED — stub from the template; the kickoff fills `D-n`, verifies every symbol, and turns each ⬜ into a run.
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
| `D-4` | ⛔ **Rule 4 — the spec does not say what we inferred.** bsvalias.org is behind a Cloudflare JS challenge (403 "Just a moment"); the source repo `bitcoin-sv-specs/brfc-paymail` does not carry the P2P documents; the text was recovered from `moneybutton/paymail-client` `docs/paymail-07-p2p-payment-destination.md` (BRFC `2a40af698840` v1.1, `PRIOR_ART.md` 2026-09-15). It says only *"The server generates a list of outputs to receive the payment"*; its own example requests **1,000,100** sats and returns **10,000 + 20,000**. HTTPS appears in the example URL, never as a MUST. The reference client `bitcoin-sv/go-paymail :: GetP2PPaymentDestination` requires `https://` (`:83`) and checks `reference` / `outputs` / `script` non-empty — **and has no sum check either**. ⇒ `sum(outputs) == amount_satoshis` is **Hodos's own invariant** (the 10b one — a signature exists only for an amount the user saw), not the spec's. 👤 Owner confirms it as policy or this sub-phase does not proceed |
| `D-5` | **Confirmed live.** `TransactionForm.tsx` resolves handles through `/wallet/paymail/resolve` (`:92`, `:182`) and the send goes to `/wallet/paymail/send`. The resolve path shares `discover_capabilities`; the `https://` requirement is applied on the **send** path (P2P destination, receive-tx, basic destination), not to `public_profile_url`, so the preview is untouched |
| `D-6` | **Rig re-planned around `D-2`/`D-3`.** The stub paymail host runs locally on plain HTTP and is fronted by a `cloudflared` quick tunnel (a public `*.trycloudflare.com` name with a real certificate) so `.well-known` is reachable over HTTPS with no code seam; the domain typed into the wallet is the tunnel host. No money leaves: `P10c-A1`'s stub returns **5,000,000** sats (10× a 500,000-sat request that the dev wallet cannot fund either way), so the pre-fix RED is `create_action` **attempted** for 5,000,000 sats (its log line + the `insufficient funds` error), i.e. the wallet was ready to sign 10×; post-fix the reject fires before `create_action`. `P10c-A2`'s accept side would broadcast real sats to stub-supplied scripts, so it runs as T1 on the extracted validator; `P10c-A5` (a real handle, a few hundred sats) is the T2 accept-side control. `P10c-A3`'s stub advertises `http://127.0.0.1:<port>/p2p/...`: pre-fix the plain-HTTP listener logs the POST (RED seen), post-fix it sees nothing |
| `D-7` | **Reuse-first.** `paymail.rs` has a `#[cfg(test)]` module (`:639`) for `parse_paymail`; the new validator and scheme check are pure functions tested there. No new struct, no new module |

## 1. Goal

A paymail send signs outputs that total exactly the amount the user approved, to a host reached over HTTPS,
and nothing else — whatever the recipient's host returns.

## 2. Done means

- [ ] `sum(outputs.satoshis) == amount_satoshis` or the send is rejected before signing, with an error naming the mismatch
- [ ] Output count bounded; zero/negative values rejected
- [ ] Every capability URL on the send path must be `https://`
- [ ] The resolve path (`/wallet/paymail/resolve`, the form's name/avatar preview) is unchanged in behaviour

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-INTEXT` | internal never prompts | the paymail send is an *internal* `create_action`; the fix must reject in the handler, not by suddenly routing an internal call through the external gate (that would prompt the user's own send) |
| `R-DUST` | 1-sat floor | output validation must not reject legitimate small outputs the dust rules already allow |
| `R-GOLD` | gold pill | untouched (internal sends do not pill); run at the boundary |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — seen, and how | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P10c-A1` | Stub host returns outputs totalling **10×** the request ⇒ rejected before signing; error names the mismatch; nothing in `outputs`/`transactions` | Pre-fix, same stub ⇒ `create_action` is **attempted** for the 10× total (the wallet was ready to sign it); it fails only because the dev wallet cannot fund 5,000,000 sats | the stub's served body logged; the wallet log shows `create_action` reached (RED) or the reject before it (GREEN); `transactions` count unchanged in both | T2 (unfundable amount — see `D-6`; ~~`noSend`~~ does not exist on this endpoint, `D-2`) | ⬜ **planned:** tunnel-fronted stub (`D-6`), request 500,000 sats, stub returns one 5,000,000-sat output; RED on today's build recorded from the wallet log; GREEN after the validator |
| `P10c-A2` | Stub returns the exact amount split across **3** outputs ⇒ succeeds | Raise one of the three by 1 sat ⇒ rejected | tx outputs decoded and summed by the test | T1 (T2 would spend real sats to stub scripts — `D-6`; the T2 accept-side control is `A5`) | ⬜ **planned:** unit test on the extracted validator: `[a, b, c]` summing exactly ⇒ `Ok`; `a + 1` ⇒ `Err(mismatch)` naming both totals |
| `P10c-A3` | Stub `.well-known` advertises an `http://` P2P endpoint ⇒ rejected | Pre-fix ⇒ used | the request the stub *did not receive* (it listens on http and sees nothing) | T2 | ⬜ **planned:** the tunnel-fronted `.well-known` names `http://127.0.0.1:8766/p2p/{alias}@{domain.tld}`; a plain-HTTP listener on 8766 logs arrivals: RED = the POST arrives on today's build; GREEN = nothing arrives and the wallet returns `ERR_PAYMAIL_INSECURE_ENDPOINT` |
| `P10c-A4` | Output count above the bound, or a zero/negative output ⇒ rejected | Pre-fix ⇒ accepted | unit | T1 | ⬜ **planned:** unit tests on the validator: 101 outputs ⇒ `Err`; a `0`-sat output ⇒ `Err`; `PaymailOutput.satoshis` is `i64` on the tree, so a negative value is representable and gets its own case |
| `P10c-A5` | **Non-regression, real:** a paymail send to a genuine handle (a few hundred sats, owner's choice of recipient) completes as before and the resolve preview still shows name/avatar | — (A1 is the control for the reject side) | txid on chain; `PAYMENT_TEST_BATCH.md` | T2 (real money) | ⬜ **planned:** owner names the recipient handle; run once after the fix from `TransactionForm`; txid + the preview screenshot into the batch register |

**Two-sided rows:** A1 (mismatch rejected) and A2/A5 (exact sum accepted) are each other's control.

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
| preflight -Full | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
