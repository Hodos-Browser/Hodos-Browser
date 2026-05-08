# Phase 1 — BRC-121 Simple 402 Payments

Pure additive. ~150 LOC. Reuses **BRC-29 PeerPay derivation** (same protocol ID `3241645161d8`). No protocol unknowns. Independent of Phase 1.5 (which makes the auto-approve gates richer but doesn't change the BRC-29 payment plumbing).

> **Phase kickoff:** before any code is written, follow the kickoff workflow in root `CLAUDE.md` "Phase kickoff workflow" section: re-read this doc + linked sources, verify cited line numbers are still current, do a reuse-first audit, do a risk assessment, confirm test plan, hand back summary for confirmation. Don't go from this README to first commit without that step.

---

## Acceptance criteria

Localhost 402 demo server returns:
- HTTP `402 Payment Required`
- `x-bsv-sats: <amount>` header
- `x-bsv-server: <paymail or address>` header

Hodos:
1. Intercepts the 402 response
2. Builds a BRC-29 payment to the server's destination
3. Retries the original request with `x-bsv-payment: <BEEF>` header
4. Server validates BEEF → returns 200

**Round-trip <2 seconds.** No new prompts during steady state (auto-approved through existing payment gates).

---

## Reuse map (do not duplicate — call existing functions)

Before writing anything new, the kickoff review verifies these are still where this doc says they are. **Do not write new derivation, signing, or BEEF code.** Wire to what exists.

### BRC-29 derivation primitive

- `BRC-29 protocol ID` = `"3241645161d8"`. Already in use at `rust-wallet/src/handlers.rs:4348, 5991, 6077, 15299` and `monitor/task_check_peerpay.rs:201`. **Same constant; reuse the existing string.**
- `BRC-29 invoice number format` = `"2-3241645161d8-{prefix} {suffix}"` per `handlers.rs:4348`. Same primitive; reuse.
- `BRC-29 P2PKH script construction` lives in `handlers.rs` around line `4366` (search for "Created BRC-29 P2PKH script"). Reuse this builder.

### Payment-building entry points

- `create_action` at `rust-wallet/src/handlers.rs:3381` (large payload variant, 100 MB limit) — the canonical transaction builder. BRC-121 wraps a call into this with the right outputs.
- `create_action_internal` at `rust-wallet/src/handlers.rs:3577` — internal entry, also reachable.
- `peerpay_send` at `rust-wallet/src/handlers.rs:15224` — closest existing handler; the BRC-121 handler is a slimmer sibling. **Read this first to understand the shape, then write `pay_402` mirroring the structure.**
- `paymail_send` at `rust-wallet/src/handlers.rs:15637` — relevant if `x-bsv-server` is a paymail (most likely case). Reuses `paymail.rs`'s `PaymailClient` for capability discovery + P2P destination resolution.

### HTTP interception infrastructure

- `OnResourceResponse` at `cef-native/src/core/HttpRequestInterceptor.cpp:2056` — currently a no-op `return false`. **This is the entry point for 402 detection.** Fill it; do not write a parallel hook.
- `isWalletEndpoint` route table — add `/wallet/pay402` here. **Don't bypass the table; new routes go through it.**
- `AsyncWalletResourceHandler` — existing class. New endpoint reuses this handler shape.
- `SessionManager` per-tab spend cap + rate limit — already enforces auto-approve gates. **Just hit it normally; don't reinvent.**

### Payment success animation (preserve!)

- `payment_success_indicator` IPC at `HttpRequestInterceptor.cpp:1656-1681` already fires after every auto-approved successful payment. **The 402 path must trigger this through the same code path.** Don't introduce a parallel notification mechanism — just make sure your success path goes through `AsyncWalletResourceHandler::OnRequestComplete`-equivalent so the indicator fires.

### Service fee output

- Every outgoing tx already adds a 1000-sat output to `HODOS_FEE_ADDRESS` per the `HODOS_SERVICE_FEE_SATS` constant in `handlers.rs`. `create_action_internal` does this automatically. **The 402 handler must use `create_action_internal` (or `create_action`) so the fee is applied; do not bypass this.**

---

## Risk assessment

Things this phase could touch or break:

| Risk | Mitigation |
|---|---|
| Payment auto-approve regresses | Smoke test: existing `peerpay_send` flow still works after change |
| 402 path bypasses domain permission gates | Route through `isWalletEndpoint` route table; verify `check_domain_approved` fires |
| 402 path bypasses spend cap | Confirm SessionManager counters increment via `wasAutoApprovedPayment_ = true` flow |
| Payment animation doesn't fire on 402 success | Manual test: green-dot animates on the tab after 402 round-trip |
| `OnResourceResponse` change breaks other intercepted responses | Audit other call sites; ensure existing `return false` cases still hit |
| Win/Mac divergence | Test on both before merge; HTTP interception is platform-neutral, but the success path through animation needs both verified |
| BRC-29 protocol ID accidentally forked | Use the existing constant; do not redefine |
| Existing PeerPay flows break | Smoke test: send PeerPay payment after change; should still work |

---

## Implementation order

1. **Kickoff review** — verify all reuse-map line numbers still accurate (5 minutes of `grep`).
2. **`pay_402` Rust handler** — `handlers.rs`, mirroring `peerpay_send` shape but accepting `(server_paymail_or_address, sat_amount)`. Returns BEEF for the `x-bsv-payment` header.
3. **`/wallet/pay402` route** — `main.rs`, near peerpay routes.
4. **`isWalletEndpoint` entry** — `HttpRequestInterceptor.cpp`.
5. **`OnResourceResponse` 402 detection** — read `x-bsv-sats` + `x-bsv-server` headers; fire async `pay_402` call; on response, retry the original request with `x-bsv-payment` header.
6. **Auto-approve integration** — confirm 402 payments hit the existing `wasAutoApprovedPayment_` path; payment animation fires.
7. **Localhost demo server** — minimal Express/Actix server returning 402 for one path, validating BEEF, returning 200.
8. **Smoke test** — round-trip <2s on Win and Mac.

---

## Test plan

- **Unit:** `pay_402` handler builds correct BRC-29 output + service fee + change.
- **Integration:** localhost 402 demo round-trip succeeds; BEEF validates server-side.
- **Smoke:** existing PeerPay flow still works (regression check); auto-approve gates still respected; payment animation fires.
- **Cross-platform:** verify on both Windows and macOS builds before merge.

---

## What this phase does NOT do

- No new permission tables (Phase 1.5 territory).
- No V8 shim (Phase 2).
- No ordinal-aware logic (Phase 3).
- No demo writeups beyond the localhost test server (Phase 4).

---

## References

- `../README.md` — sprint context
- `../_DRAFT_RECOVERED_PLAN.md` — original plan, includes deeper code citations
- `../ARCHITECTURE.md` — sprint diagrams
- Root `CLAUDE.md` — invariants, testing standards, **phase kickoff workflow**
- `rust-wallet/src/CLAUDE.md` — handler groups, BRC-29 references
- `cef-native/src/core/CLAUDE.md` — HTTP interception, auto-approve flow
