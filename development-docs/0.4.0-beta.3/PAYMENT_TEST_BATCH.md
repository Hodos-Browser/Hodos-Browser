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

## ⛔ RULE (owner, 2026-09-17): "it needs a funded wallet" is NOT a reason to defer a test

> 👤 *"I have a million funded wallets. This is not a reason to defer a test. We have lots of funded
> wallets. We need to do these tests."*

**Funding is available on demand.** It is not scarce, it is not a scheduling constraint, and it may
never again appear in a deferral reason, a phase contract, a residual, or a status line. If a row
needs BSV, ask for a funded wallet and run it.

⇒ What this register actually batches is **rig setup** — dev stack up, a fault-injection build,
a second wallet, a live counterparty. Those are real costs. Funding is not one of them. ⛔ Any row
still reading *"deferred: needs a real payment"* is mis-labelled; re-state the real blocker or run it.

⚠️ This is what five consecutive deferrals of `R-GOLD` were actually resting on, and the sitting that
finally ran it took one afternoon and found three defects no code review had.

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
| **M1** | **`R-GOLD`** — the gold pill appears on the **originating tab** for an auto-approved payment | Every boundary since 2→3 | ⛔ `Tab::id` ≠ `CefBrowser::GetIdentifier()`; a pill on the wrong tab is a failure. Both paths: createAction silent-approve **and** the BRC-121 paid retry (`firePaymentSuccessIpc`) | ✅ **DONE 2026-09-16, Windows, owner watching.** `💰 OnWalletCallSuccess fired (2 cents from example.com, cefBrowserId=12 → tabId=2)` — the two ids DIFFER, which is the whole hazard, and the pill resolved to tab 2 = the payer. Owner was on another tab and confirmed the pill appeared on the paying tab. 0 prompt lines: genuinely silent. txid `cd61e4c5…` / `b6079187…`. ⛔ **The first attempt was VOID and is worth recording.** Payer `example.com` and bystander `example.org` both render a page titled *Example Domain* with the same favicon, so the owner could not tell the tabs apart — a pass and the exact failure look identical. Re-run with WhatsOnChain / Wikipedia / Example Domain, where the payer is the only one of its kind. 👤 Owner on the pill itself: *"a little subtle, but that's how it is for now"* — revisit on real user feedback. ✅ **BRC-121 paid-retry half DONE 2026-09-16** on a real 402 paywall (`now.bsvblockchain.tech`): `💰 OnWalletCallSuccess fired (0 cents from now.bsvblockchain.tech, cefBrowserId=19 → tabId=8, endpoint=pay402)` — fired from `firePaymentSuccessIpc`, identifiers differing again (19 → 8), resolved to the article's own tab. ⭐ Also confirmed independently on a real dApp earlier the same day (`teragun.com`, a genuine 125,078-sat purchase, `cefBrowserId=16 → tabId=6`). So **both paths this row names are now observed**, on real third-party sites. 🚨 **And the human half is why it mattered:** the logs read as a clean success while the owner saw a blank page for ~30 seconds and no feedback at all — see `TICKET_brc121_paid_retry_aborts_and_mints_a_payment_each_time.md`. The retry aborted TWICE (`ERR_ABORTED`) before succeeding, minting a payment each time; no money was lost (both abandoned payments logged `NOT broadcasting (funds preserved)`) but each left a `nosend` row and a spendable phantom output | ✅ yes |
| **M2** | **`R-COUNT`** — per-session spend counters reset on tab close | Every boundary since 2→3 | ✅ **DONE 2026-09-16, Windows.** The cleanest sequence of the sitting, read off the engine's own decisions:
`🛡️ engine Prompt (payment) … reason=session_cap` (session held 4c against a 2c cap) → `🛡️ session/close: cleared payment session counters for browser_id=12 (Rust counters dropped)` → `🔓 engine Silent (payment) … session_spent_now=2` after reopening. Same site, same amount, opposite outcomes, discriminated only by the tab close. Subject was the engine decision plus `session_spent_now`, never a UI total. ⭐ The "before" was free: the session already carried 4 cents from the M1 pill runs, so setting the cap to 2 made the next payment trip it without spending anything extra. Tab closed through the product's own `tab_close` IPC, not a debugger call | ✅ yes |
| **M3** | **`R-PERIM` T2** — the four privacy-perimeter gates end-to-end, incl. the **over-cap spend** | Every boundary (T1 green throughout) | ✅ **DONE 2026-09-16, Windows — all four gates, on the engine's `PermissionDecision` rather than any modal.**
1. **identity-key reveal** — BOTH directions: `Prompt … kind=IdentityKeyReveal` with the setting off, `Silent … kind=IdentityKeyReveal` with `identity_key_disclosure_allowed=1`. Same call, same site, flipped by the setting alone.
2. **key-linkage reveal** — `Prompt … kind=CounterpartyKeyLinkage`. ⬜ Its silent arm (a session opt-in) not arranged.
3. **sensitive cert fields** — ⭐ the sharpest evidence here: the SAME endpoint and site gave two DIFFERENT decision kinds, discriminated only by the field names. `userName` ⇒ `Prompt (cert) reason=cert_field_unapproved` (the ordinary scoped-grant path); `dateOfBirth` + `socialSecurityNumber` ⇒ `Prompt … kind=SensitiveCertField` (the unconditional perimeter gate). So the classifier is live and separate, not merely "certificates always prompt". ⚠️ The stored certs only carry email/photo/userName, so the sensitive names were REQUESTED rather than held — which is the right subject, since the gate classifies what is asked for.
4. **over-cap spend** — covered by M2 above, both directions.
Test permissions deleted afterwards so this row and `R-INTEXT` stay non-vacuous | ✅ the over-cap arm |
| **M4** | **`P8b-A1`** — a forced resolution failure means **nothing reaches the network** | **Phase 8b** | ⭐ **GREEN half costs nothing** — the assertion *is* "no transaction was broadcast". ⛔ Only the RED half spends. Subject: **WhatsOnChain for the txid**, not the HTTP response | ✅ **DONE 2026-09-16, Windows.** 🟢 **GREEN**: fault seam on, `/transaction/send` 5,000 sats ⇒ HTTP 500, *"signed but the wallet could not record which coins it spends, so it was NOT broadcast"*, and the txid `6940417551…` returns **404** on WhatsOnChain while a real txid from the same day returns 200 (the instrument control). 🔴 **RED**: all guards removed ⇒ HTTP 200 and `80d5821feffff…` **is on the network** (200). ⭐ **Two findings the row did not anticipate.** (1) There are **three** guards in series on this path, not one — neutering the first still failed closed at the second, which is defence in depth, measured. (2) The refused send's residue is **temporary and self-healing**, timeline measured for the first time: at t+0 a `nosend` row, a reserved input and a **spendable phantom change output**, balance understated by the fee; by **t+11m** `TaskCheckForProofs` marks it `failed` and deletes the phantom change, balance understated by the whole input (4.87M sats); by **t+19m** `TaskSweepReservations` releases the input and the balance is **exactly** its pre-test value. ⚠️ So a refused send understates the balance for up to ~19 minutes. No money at risk, nothing lost, and the delay is the deliberate P0.7 rule that a reservation is released only after the outpoint is positively observed unspent on chain. ⛔ Deviation from the row as written: the RED used the CURRENT tree with the guards removed rather than a pre-`2513296` binary. Same experiment, one variable, and it is what exposed the three-guards-in-series fact. 🧪 Seam: `HODOS_FAIL_SPEND_RESOLUTION`, gated on `HODOS_DEV` which `enforce_dev_safeguard` scrubs from any non-dev binary |
| **M5** | **`P8a` live** — the 1-sat floor against a real consolidation | Phase 8a (deferred by design) | ✅ **DONE 2026-09-16, Windows.** Real dust, not seeded rows: one transaction paid 21 × 700 sats + one 1-satoshi carrier to our own address (`10b79669…`), all genuinely mined. With 22 eligible candidates the consolidation consumed **21** inputs and the carrier outpoint `10b79669…:21` was **not among them** — read off the serialised transaction's input outpoints, which is the subject `R-DUST` demands, never a log line or a balance. ⭐ The count is what discriminates: 21 of 22, and the missing one is exactly the carrier.
⛔ **Nearly a vacuous pass, twice over.** (1) The twenty 700-sat outputs confirmed before the carrier did, and running then would have swept them, left the carrier alone, and looked green while testing nothing — the carrier was not in the candidate pool at all. (2) The carrier could only be made eligible by marking it `confirmed = 1` **by hand** (disclosed), because of the bug below.
🚨 **This row found something else entirely: `TICKET_bulk_utxo_sync_truncates_at_20_per_address.md`.** The indexer's BULK unspent endpoint returns at most 20 UTXOs per address (single-address returns 27 for the same address, same moment), so outputs 20 and 21 were never marked confirmed. A flat cap, not a value rule — which it convincingly impersonated, since one of the two stranded outputs was the carrier. ⚠️ beta.4's ordinals work puts many 1-satoshi outputs on one address and should not be built on top of this.
⬜ Live RED not run: it needs another 20+ dust seed, and `without_the_floor_the_carrier_is_selected` already shows the set growing at T1. The owner's nine REAL token carriers were never at risk — they sit in named baskets, which `get_spendable_confirmed_by_user` excludes before the floor is consulted | ✅ yes |
| **M6** | **`E5-a`** — one real payment writes one `payment.auto_approved` audit line | Phase 2 (`MAC_RELAY_P2_ROUND.md`) | Two wired call sites were proven by unit test + code read only, and **neither Windows nor Mac had ever observed it** | ✅ **DONE 2026-09-16, Windows** — rode along with M1, twice: `payment.auto_approved | example.com | cents=2 endpoint=/createAction` in `audit-<pid>.log`, once per auto-approved payment. ⭐ First time this line has been seen on either platform | ✅ yes |
| **M7** | **A send whose balance is mostly 1-sat outputs** | Phase 8a residual | The floor can turn a previously-successful send into `insufficient funds`. Intended, never exercised | ❌ fails by design |
| **M8** | **`P8c-A2`** — two concurrent `sendTransaction` calls produce **two** sends | **Phase 8c stage 2** | ✅ **DONE 2026-09-14, Windows, owner-authorised** — GREEN `d81a6892…fbf6e13b` + `796a9d93…de9217b1`, both on WhatsOnChain; RED (dedupe re-applied for one run, reverted) ⇒ both promises carried `eb4b3d41…b335dda5`, **one** send. `phase-8c-bridge-request-ids/PHASE_CONTRACT.md` §4l. ⛔ Subject is **two distinct txids**, not two resolved promises. RED: apply `getBalance`'s in-flight dedupe to `sendTransaction` ⇒ **one** send — the row that catches the single most dangerous way to "finish" 8c. ⚠️ Two sends means **two real transactions** | ✅ yes, ×2 |

| **M9** | **`P10a-A5` poller half** — a genuine PeerPay is credited **once, correct amount, via `TaskCheckPeerPay`** (message `amount` cross-check live, one `peerpay_received` row) | **Phase 10a** (2026-09-15) | 🟡 **Real send made and recovered, poller path still OWED.** Owner sent `3798109e5612bc7e8bc1ebffc02d5c60a0b6e0d96ac84bf916542a3157d4ab55` (613,685 sats) from the installed wallet to the dev wallet; the sender's MessageBox message is **1.76 MB** (BEEF as a JSON byte array) and MessageBox rejects it with **413** — `TICKET_peerpay_message_exceeds_messagebox_limit.md`. The sats were credited to the dev wallet by hand through `/internalizeAction` with the sender's queued remittance (the accept-side control the fix needed). ✅ **DONE 2026-09-16** — 👤 owner sent from their MetaNet wallet (the installed browser could not: old build, 436 KB parent). `TaskCheckPeerPay` polled, verified on-chain, and accepted **189,633 sats** from `02ed98c7…`, txid `2ec0f999…`, ONE `peerpay_received` row, `notification_type='receive'`. ⭐ The BACKGROUND POLLER did it: a manual `/wallet/peerpay/check` a minute later found 0 messages, so the credit cannot be attributed to the manual path. Original note: re-run through the poller once the sender ticket lands. Subject: the dev wallet's `outputs` row **and** the `peerpay_received` row written by the poller, not the internalize path | ✅ yes (done once; re-run needs a second small send) |

| **M10** | **`P10d-A7` / `R-PEERPAY-DELIVERY` end-to-end half** — after a real on-chain backup, a PeerPay delivers and the recipient is credited once | **Phase 10d**; then **every release candidate** and any phase touching coin selection, backups, PeerPay or MessageBox | ✅ **DONE 2026-09-15, Windows** (dev wallet, cap override 250 KB): backup `8792aba6…3422` (change 8,057 sats), then PeerPay `b21e2c88…cb94` 700 sats ⇒ all parents ≤ 260 B, delivered first try, installed wallet credited once. Also spent in 10d: RED PeerPay `35911c48…d278` 700 sats (size check removed — must broadcast, did). ⚠️ **Must be re-run at the RC** on a wallet whose real backup is > 220 KB (the installed one) with **no** cap override — the dev run proves the mechanism at scaled proportions, not the production numbers. Subject: recipient's `outputs` + `peerpay_received` rows and WhatsOnChain parent sizes | ✅ yes (one backup fee + a few hundred sats) |

| **M11** | **`P10c-A5`** — a paymail send to a **genuine third-party handle** (a few hundred sats) still completes after the CU-2 guards, and the resolve preview still shows name/avatar | **Phase 10c** (2026-09-15) | 🟡 **OWED — owner must name a recipient handle.** 10c's accept side was proved against our own stub host (900 sats, three exact outputs, txid `48b77e6866847305fe3ec036feb672e3bbb1cb6d2d8cd33a05d10a7c4476052a`), which exercises the validator but **not** a real bsvalias implementation: a third-party host may legitimately return a shape our `MAX_P2P_OUTPUTS`/value bounds have never seen. ✅ **DONE 2026-09-16, Windows.** 👤 Owner named `archie@handcash.io`. Resolve returned `valid:true, name:"Archie", avatar_url:…, has_p2p:true`; the send of 500 sats produced txid `8a52f675…` (**200** on WhatsOnChain), HandCash returned one output for exactly 500, and it accepted the P2P notification (`receive-tx submitted OK`) — so the recipient was told, not just paid. Balance moved 29,104,196 → 29,102,496 = 500 + 1,000 service fee + 200. ⭐ **This also settled how bad `F1-10c` really was:** HandCash echoes the 546-satoshi probe EXACTLY, so the regression would have been invisible to the largest provider and would only have bitten a host that answers with a different total — which is what BRFC `2a40af698840`'s own worked example does. Real, but less severe than first reported. Original subject: the txid on chain **and** the form's preview, not the HTTP 200. ⭐ **Fold in panel `F3-10d` while the rig is up:** copy the claim block out of a **real** refused PeerPay and assert `senderIdentityKey` equals the wallet's identity key and both derivation strings are non-empty — the production builder reads them back out of the stored payload with `.unwrap_or("")`, so a parse failure emits a well-formed block full of empty strings, and beta.5's Tools tab is specified to parse exactly this. No row has ever seen a real one | ✅ yes (a few hundred sats) |

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
