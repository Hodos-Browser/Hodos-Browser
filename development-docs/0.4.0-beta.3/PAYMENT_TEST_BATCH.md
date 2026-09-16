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

| **M9** | **`P10a-A5` poller half** — a genuine PeerPay is credited **once, correct amount, via `TaskCheckPeerPay`** (message `amount` cross-check live, one `peerpay_received` row) | **Phase 10a** (2026-09-15) | 🟡 **Real send made and recovered, poller path still OWED.** Owner sent `3798109e5612bc7e8bc1ebffc02d5c60a0b6e0d96ac84bf916542a3157d4ab55` (613,685 sats) from the installed wallet to the dev wallet; the sender's MessageBox message is **1.76 MB** (BEEF as a JSON byte array) and MessageBox rejects it with **413** — `TICKET_peerpay_message_exceeds_messagebox_limit.md`. The sats were credited to the dev wallet by hand through `/internalizeAction` with the sender's queued remittance (the accept-side control the fix needed). Re-run through the poller once the sender ticket lands, or 🍎 with Mac's wallet as sender. Subject: the dev wallet's `outputs` row **and** the `peerpay_received` row written by the poller, not the internalize path | ✅ yes (done once; re-run needs a second small send) |

| **M10** | **`P10d-A7` / `R-PEERPAY-DELIVERY` end-to-end half** — after a real on-chain backup, a PeerPay delivers and the recipient is credited once | **Phase 10d**; then **every release candidate** and any phase touching coin selection, backups, PeerPay or MessageBox | ✅ **DONE 2026-09-15, Windows** (dev wallet, cap override 250 KB): backup `8792aba6…3422` (change 8,057 sats), then PeerPay `b21e2c88…cb94` 700 sats ⇒ all parents ≤ 260 B, delivered first try, installed wallet credited once. Also spent in 10d: RED PeerPay `35911c48…d278` 700 sats (size check removed — must broadcast, did). ⚠️ **Must be re-run at the RC** on a wallet whose real backup is > 220 KB (the installed one) with **no** cap override — the dev run proves the mechanism at scaled proportions, not the production numbers. Subject: recipient's `outputs` + `peerpay_received` rows and WhatsOnChain parent sizes | ✅ yes (one backup fee + a few hundred sats) |

| **M11** | **`P10c-A5`** — a paymail send to a **genuine third-party handle** (a few hundred sats) still completes after the CU-2 guards, and the resolve preview still shows name/avatar | **Phase 10c** (2026-09-15) | 🟡 **OWED — owner must name a recipient handle.** 10c's accept side was proved against our own stub host (900 sats, three exact outputs, txid `48b77e6866847305fe3ec036feb672e3bbb1cb6d2d8cd33a05d10a7c4476052a`), which exercises the validator but **not** a real bsvalias implementation: a third-party host may legitimately return a shape our `MAX_P2P_OUTPUTS`/value bounds have never seen. Subject: the txid on chain **and** the form's preview, not the HTTP 200. ⭐ **Fold in panel `F3-10d` while the rig is up:** copy the claim block out of a **real** refused PeerPay and assert `senderIdentityKey` equals the wallet's identity key and both derivation strings are non-empty — the production builder reads them back out of the stored payload with `.unwrap_or("")`, so a parse failure emits a well-formed block full of empty strings, and beta.5's Tools tab is specified to parse exactly this. No row has ever seen a real one | ✅ yes (a few hundred sats) |

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

⇒ Gate the fault flag on **`HODOS_DEV`**, not on a bare env var. `main.rs :: enforce_dev_safeguard`
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
