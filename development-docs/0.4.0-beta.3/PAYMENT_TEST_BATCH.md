# The payment-test batch — everything deferred because it needs a REAL payment

**Opened:** 2026-09-08, from the owner's question at the close of Phase 8b: *"Are we doing P8b-A1 at
the correct time, it still needs done or…?"* **Owner:** Matthew. **Platform:** Windows (macOS rows noted).

> ⭐ **Modelled directly on `INSTALL_TEST_BATCH.md`**, and for the same reason the owner gave there:
> *"we can keep track of it and do everything that requires install tests at the end to save time."*
> The expensive part is not the assertion, it is **standing up the rig** — dev stack running, dev
> wallet funded, a fault-injection build, and a willingness to spend real BSV.
>
> 🚨 **The forcing observation:** `R-GOLD` and `R-COUNT` have been marked *"needs a real payment"* at
> **every single boundary this sprint** — 2→3, 3→4, 4→5, 8→next. Four boundaries, same reason, no
> schedule. `P8b-A1` was about to become the fifth. A row that is owed everywhere and scheduled
> nowhere is a row that never runs.

⛔ **A row here is OWED, not waived.** Nothing may be reported as passed, skipped, or "covered by
unit tests". Per `HARNESS.md` §8 a skipped check is **SKIPPED** and its run is **INCOMPLETE**.

---

## When this batch runs

**Before the 0.4.0 release candidate**, in one sitting, after the last phase that adds to it.

⚠️ Unlike the install batch this is **not** strictly gated on the RC — several rows could run today.
It is batched because the *rig* is the cost, not because the rows are release-only.

---

## The batch

| # | Row | Owed by | What it asserts | Costs real BSV? |
|---|---|---|---|---|
| **M1** | **`R-GOLD`** — the gold pill appears on the **originating tab** for an auto-approved payment | Every boundary since 2→3 | ⛔ `Tab::id` ≠ `CefBrowser::GetIdentifier()`; a pill on the wrong tab is a failure. Both paths: createAction silent-approve **and** the BRC-121 paid retry (`firePaymentSuccessIpc`) | ✅ yes |
| **M2** | **`R-COUNT`** — per-session spend counters reset on tab close | Every boundary since 2→3 | Spend to just under the session cap, close the tab, reopen. Subject is `PermissionService.session_counters`, not a UI total | ✅ yes |
| **M3** | **`R-PERIM` T2** — the four privacy-perimeter gates end-to-end, incl. the **over-cap spend** | Every boundary (T1 green throughout) | The engine's `PermissionDecision`, not the modal's appearance | ✅ the over-cap arm |
| **M4** | **`P8b-A1`** — a forced resolution failure means **nothing reaches the network** | **Phase 8b** | ⭐ **GREEN half costs nothing** — the assertion *is* "no transaction was broadcast". ⛔ Only the RED half spends. Subject: **WhatsOnChain for the txid**, not the HTTP response | 🟡 RED half only |
| **M5** | **`P8a` live** — the 1-sat floor against a real consolidation | Phase 8a (deferred by design) | ⚠️ Needs ~4,550 sats of accumulated dust before the consolidator fires at all. Cheapest as a **seeded dev DB** rather than by waiting | ✅ yes |
| **M6** | **`E5-a`** — one real payment writes one `payment.auto_approved` audit line | Phase 2 (`MAC_RELAY_P2_ROUND.md`) | Two wired call sites are proven by unit test + code read only. ⚠️ **Neither Windows nor Mac has done this** | ✅ yes |
| **M7** | **A send whose balance is mostly 1-sat outputs** | Phase 8a residual | The floor can turn a previously-successful send into `insufficient funds`. Intended, never exercised | ❌ fails by design |
| **M8** | **`P8c-A2`** — two concurrent `sendTransaction` calls produce **two** sends | **Phase 8c stage 2** | ✅ **DONE 2026-09-14, Windows, owner-authorised** — GREEN `d81a6892…fbf6e13b` + `796a9d93…de9217b1`, both on WhatsOnChain; RED (dedupe re-applied for one run, reverted) ⇒ both promises carried `eb4b3d41…b335dda5`, **one** send. `phase-8c-bridge-request-ids/PHASE_CONTRACT.md` §4l. ⛔ Subject is **two distinct txids**, not two resolved promises. RED: apply `getBalance`'s in-flight dedupe to `sendTransaction` ⇒ **one** send — the row that catches the single most dangerous way to "finish" 8c. ⚠️ Two sends means **two real transactions** | ✅ yes, ×2 |

> ⚠️ **M8 added 2026-09-09, correcting a drift.** The stage-1 close-out said `P8c-A2` was *"already
> routed to `PAYMENT_TEST_BATCH.md`"*. It was not — the row existed only in the phase contract. That
> is precisely the silent-loss failure this register exists to prevent, and it happened here first.

## Rig — what has to be true once

1. Dev stack: `.\dev-wallet.ps1` (31401) · `cd frontend && npm run dev` (5137) · dev exe with
   `HODOS_DEV=1 --profile=Default --remote-debugging-port=9322`.
   ⛔ **Never** stop a Hodos process by image name — `.\scripts\stop-dev.ps1` is path-matched.
2. **A funded dev wallet.** Every row except M4-GREEN and M7 needs spendable BSV.
   ⚠️ Mainnet — there is no testnet path in this build.
3. **A fault-injection build** for M4. See below.
4. `RUST_LOG=hodos_wallet=debug` for any wallet-log assertion — 🚨 a zero can be a suppressed log,
   not an absence (`R-INTEXT` read 0 lines because `domain_trust_mw` logs at debug).

### ⭐ M4's seam — and why it is safe by construction

`P8b-A1` needs `update_spending_description_batch` to fail on demand, in a **release** build (the dev
browser is release), so `#[cfg(test)]` cannot reach it.

⇒ Gate the fault flag on **`HODOS_DEV`**, not on a bare env var. `main.rs :: enforce_dev_prod_isolation`
already guarantees a production binary **scrubs `HODOS_DEV` and forces prod**, so a shipped build can
never enter the branch. That reuses an existing safeguard instead of inventing a riskier one.

⛔ The RED half must run the **pre-fix binary** (before `2513296`) and be seen to broadcast anyway —
otherwise M4 proves nothing. That is one real transaction. Budget it.

## Ordering — cheapest first

1. **M4-GREEN** and **M7** — free, no funds move. Run these first; if M4-GREEN fails there is no point
   funding anything else.
2. **M6**, **M1**, **M2**, **M3** — one funded session; a single payment can satisfy M6 and M1
   together if the audit log and the pill are both watched on the same send.
3. **M5** — seeded dust DB.
4. **M4-RED** — last, on the pre-fix binary, deliberately.

## Cross-references

Every row above is also marked owed in its own home; ⛔ **this file does not replace those marks**, it
schedules them.

- `REGRESSION_SET.md` — boundary table + `R-GOLD` / `R-COUNT` / `R-PERIM`
- `phase-8b-placeholder-abort/PHASE_CONTRACT.md` §4 `P8b-A1`, §8 item 1
- `phase-8a-dust-guard/PHASE_CONTRACT.md` residuals 4 and 7
- `INSTALL_TEST_BATCH.md` — the sibling register; ⚠️ **different rig, do not merge them.** An install
  test needs a signed build and a clean machine; a payment test needs a funded wallet and a dev stack
