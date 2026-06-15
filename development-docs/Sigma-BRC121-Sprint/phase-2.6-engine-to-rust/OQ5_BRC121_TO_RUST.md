# OQ5 — BRC-121 paid-retry cap/rate cascade → Rust

> **Status:** 🚧 IN PROGRESS (2026-06-15). Gates Phase 2.6-H (SessionManager + cache + dead-engine cleanup).
> **Why now:** 2.6-H wants to delete C++ `SessionManager` + `DomainPermissionCache`, but the BRC-121
> paid-retry path still decides cap/rate **in C++** using both. This migration moves that decision to
> Rust so 2.6-H can delete them with zero live readers. (Originally OQ5 was sequenced *after* 2.6-H;
> resequenced per "complete + test the migration, then clean up — do it right once".)

## Audit (current state, verified 2026-06-15)

`TryHandleBrc121_402` (`cef-native/src/core/HttpRequestInterceptor.cpp:5591-5910`) is a self-contained C++
decision cascade duplicating Rust's `dispatch_payment`:

1. Detect 402 + `x-bsv-sats`/`x-bsv-server`; parse sats; validate server pubkey.
2. **domain = payee host (URL host)**, NOT page origin (BRC-121-specific).
3. `DomainPermissionCache::getPermission(domain)` → not `approved` ⇒ `domain_approval` modal.
4. cents from `BSVPriceCache`; price ≤ 0 ⇒ `payment_confirmation` (price_unavailable).
5. `SessionManager` reads (`getSpentCents`/`getPaymentCount`/`checkRateLimit`) ⇒ per-tx / per-session /
   max-tx / rate gates ⇒ `payment_confirmation` or `rate_limit_exceeded` modal.
6. All pass ⇒ `POST /wallet/pay402` (today: **mints only, no decision**) ⇒ `PaidRetryContext` ⇒ reload ⇒
   `Async402ResourceHandler` issues paid retry + broadcasts after 200.

### Every C++ `SessionManager` touch + post-migration fate

| Site | What | After migration |
|---|---|---|
| 5709/5739/5740/5801 | BRC-121 cap reads | gone — Rust decides |
| 5498-5499 | BRC-121 success increments | gone — Rust records |
| 2487-2490, 2645-2646 | dead IPC engine cascade | gone — deleted in 2.6-H regardless |
| 1480 (`OnWalletCallSuccess`) | `recordSpending` on every auto-approved success | remove the one line, **keep the gold-pill IPC**; Rust records for both paths |
| 4306 | createAction over-cap approve re-issue | remove — redundant; Rust replay does `record_spending` (request_gate.rs:1023) |
| TabManager.cpp:183 / _mac.mm:193 | `clearSession` on tab close | gone when SM deleted (both platforms together, 2.6-H) |

**⇒ After this migration, C++ `SessionManager` has zero live readers/writers → fully deletable in 2.6-H.**

## Reuse anchors (no new structures)

- **`dispatch_payment`** (`rust-wallet/src/permission_service/request_gate.rs:986`) — already does cap/rate/
  price-eval/counters with Silent→record / Prompt→mint-approval-id / X-User-Approved→replay-record.
- **2.6-G domain-trust middleware** — gates any external domain on every route via `X-Requesting-Domain`,
  returns 202 for unknown.
- **X-Payment-\* header contract** (`PaymentCall::from_headers`) — the C++→Rust payment-decision channel.

## Locked design decisions (2026-06-15)

- **D1: `pay_402` runs `dispatch_payment`** before the mint. Silent→mint+200; Prompt→**202 + payload, no
  mint**; X-User-Approved replay→mint. (Chosen over a separate decide-only endpoint — max reuse, one round-trip.)
- **D3: C++ keeps computing cents** from `BSVPriceCache` and passes `X-Payment-Satoshis/Cents` +
  `X-Bsv-Price-Available` + `X-Browser-Id`. Mirrors `createAction`; `BSVPriceCache` is NOT on the 2.6-H
  deletion list. (Chosen over moving price into Rust — lower risk, consistent contract.)
- **D2 (implied): payee domain-trust** via the 2.6-G middleware — C++ sets `X-Requesting-Domain` = payee host
  so unknown payee ⇒ 202 (the existing connect path). Confirm no double-prompt with `pay_402`'s existing
  `check_domain_approved`.
- **D4: approve→replay** — replace the one-shot `s_brc121_approved_urls` URL bypass with carrying the Rust
  `approval_id`; the post-approve reload re-calls `pay_402` with `X-User-Approved` (mirrors createAction).

## What moves vs stays

| Moves to Rust | Stays in C++ (CEF-navigation-bound) |
|---|---|
| per-tx / per-session / max-tx / rate decision | 402 detection + header validation |
| session counters + `record_spending` | payee-host extraction |
| domain-trust gate (payee as X-Requesting-Domain) | modal surfacing + `/payment-pending` placeholder + reload |
| price-unavailable → prompt decision | gold-pill `firePaymentSuccessIpc` (PRESERVE) |
|  | carry approval_id: approve → reload → X-User-Approved replay |

## Implementation steps (small, ordered)

1. ✅ **Rust — DONE 2026-06-15 (uncommitted).** `pay_402` calls `dispatch_payment` before the reuse/mint,
   **header-gated** (`X-Payment-Satoshis` or `X-User-Approved` present) so it's dormant until the C++ flip;
   old/internal callers (neither header) hit the unchanged legacy mint path. `EarlyReturn`→202 prompt /
   403 deny; `Proceed`→fall through to mint. `cargo build --release` clean; 118 permission_service tests pass.
   No new unit test added — the decision lives entirely in `dispatch_payment` (already covered); the thin
   `pay_402` wiring is verified by the co-test (an actix handler that mints real txs can't be unit-tested).
2. ✅ **C++ — DONE 2026-06-15 (uncommitted).** `TryHandleBrc121_402` now injects `X-Requesting-Domain`=payee +
   `X-Browser-Id` + `X-Payment-Satoshis/Cents` + `X-Bsv-Price-Available` on the pay_402 POST (via the
   `SyncHttpClient::Post` headers overload); the entire C++ cap cascade + all `SessionManager` reads in this path
   are deleted. 200→proceed (existing mint path); 202→surface the modal from the Rust `promptPayload` (real
   bsvPrice used for display) + register reload; 403→fall through. Domain-trust gate kept C++-side for now (so
   `DomainPermissionCache` stays — its deletion is out of OQ5 scope; reassess at 2.6-H).
3. ✅ **C++ — DONE.** One-shot `s_brc121_approved_urls` set replaced with PENDING/ARMED approvalId maps;
   `MarkBrc121PaymentApproved` moves PENDING→ARMED; the post-approve reload pops ARMED + re-POSTs with
   `X-User-Approved`. Consent-safe: a reload without an Approve is not armed → fresh engine decision.
4. ✅ **C++ — DONE.** Removed orphaned `SessionManager` writes: `recordSpending` in `OnWalletCallSuccess`
   (kept the gold-pill IPC) + the createAction over-cap re-issue site + the two `firePaymentSuccessIpc`
   increments. **Verified:** the only remaining `SessionManager` refs in `HttpRequestInterceptor.cpp` are inside
   the dead `runIpcEngineCascade` (2.6-H deletes it). `cmake --build` clean; `HodosBrowser.exe` linked.
5. ✅ **Co-test PASSED 2026-06-15** on `now.bsvblockchain.tech`. Logs confirm: new C++ marker
   `BRC-121 → /wallet/pay402 (engine decision)` ×3; Rust had session counters to clear at tab-close
   (`session/close: cleared payment session counters for browser_id=9`) ⇒ `dispatch_payment`/`record_spending`
   ran; mint+200+single broadcast ×2; **gold pill fired** (`OnWalletCallSuccess … endpoint=pay402`) ×2;
   `domain_approval` flow worked; payment 1 hit the known server 431 limit (funds preserved, no broadcast/pill —
   documented, not a Hodos bug); no denials/errors/double-count. Over-cap prompt branches (b/c) not live-
   triggerable (100/75 sats = 0 cents, always under cap) but covered by the 118 permission_service + engine
   tests and share createAction's `dispatch_payment` path. **OQ5 CLOSED.** → 2.6-H.

## Risks & safeguards

- **Gold pill** — BRC-121 success must still fire `firePaymentSuccessIpc`→`OnWalletCallSuccess`→
  `payment_success_indicator`. Only the `recordSpending` line inside is removed; the IPC stays. Verify in co-test.
- **Double-prompt** — payee-as-`X-Requesting-Domain` + middleware vs `pay_402`'s `check_domain_approved`.
  Confirm a single domain_approval, not two.
- **pay402 reuse cache** — the (URL,sats) reuse path must run only on the Silent/approved branch, not before the
  decision (don't mint-reuse a payment the engine would prompt on).
- **Prompt payload parity** — `build_payment_prompt_payload` fields vs the BRC-121 modals' expected params
  (satoshis/cents/bsvPrice/exceededLimit/perTx/perSession/sessionSpent/rateLimit/maxTxPerSession/txCount).
  Extend the Rust payload if a field is missing rather than recomputing in C++.
- **Money-path** — C++ change can't be curl-tested; needs the browser co-test on a live BRC-121 server.

## Test plan

- Rust unit tests: pay_402 Silent / Prompt(per-tx, per-session, rate, max-tx, price_unavailable) / replay.
- Thorough co-test on `now.bsvblockchain.tech`: (a) under-cap→silent, gold pill; (b) over per-tx/session→
  payment_confirmation→approve→pays once, gold pill; (c) rate/max-tx→rate_limit_exceeded; (d) unapproved
  payee→domain_approval; (e) reload/back-button no double-pay. Watch wallet log for the Rust decision + a
  single broadcast.

## Then → 2.6-H

With BRC-121 deciding in Rust, C++ `SessionManager` + `DomainPermissionCache` (for caps) have no live readers.
2.6-H deletes them + the dead C++ engine (`PermissionEngine`/`PermissionGate`/`EngineShadow` + tests) + dead
Open()/runIpcEngineCascade cascades + shadow infra + `engine_shadow_log` migration, in one commit, both platforms.
