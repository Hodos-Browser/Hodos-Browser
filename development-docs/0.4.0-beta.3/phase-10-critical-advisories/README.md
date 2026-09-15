# Phase 10 — critical advisories (CU-3 → CU-1 → CU-2)

**Opened:** 2026-09-15, by owner decision, from `../CRITICAL_UPDATES.md` (three BSV Association advisories against
their TypeScript stack; Hodos ships none of those packages but implements the same protocols, and a code review
found the same bug shapes). **Status:** 🚧 IN PROGRESS — kickoff `42aac69` (2026-09-15); **10a ✅ Windows**; 10d contracted; 10b, 10c next. **Standard:** `../HARNESS.md`.
**Base:** `origin/0.4.0` at the Phase 9 close (`07f9fcf`) plus the 2026-09-15 dependency bumps (`ec6353e`).

> ⛔ **Everything in `CRITICAL_UPDATES.md` is a code reading.** Nothing was built, run or exploited. The kickoff
> for each sub-phase re-reads every cited line on today's tree (line numbers rot; symbols survive), and every
> fix lands only after its RED has been *seen* — a fabricated PeerPay credited, two txids from one click, a
> 5,000,000-sat paymail broadcast in `noSend`. A fix whose defect was never observed is a fix for a hypothesis.

## Why this phase runs before the UI tail

Three high-severity defects on the money path, all present in the shipped `v0.3.0-beta.29` by code pattern.
One of them (CU-3) runs on the background poller with no user action and can overwrite a coin we genuinely
own. Release readiness (Phase 9) closed the *promotion* gates; this phase closes *product* gates that a user
could hit on day one. The visual leftovers (now Phase 11) wait.

## Owner decisions taken 2026-09-15 — do not re-litigate at kickoff

| # | Decision |
|---|---|
| 1 | **No CU-3 stopgap.** Automatic PeerPay acceptance stays on. The real fix — bind the credited output to the transaction whose hash *is* the declared subject txid — keeps auto-accept and removes the defect. Disabling the poller was offered and declined |
| 2 | **Order: 10a CU-3 (+CU-6) → 10b CU-1 (+CU-8, +CU-9) → 10c CU-2.** CU-3 first because it needs no user action |
| 3 | **Bursts are fixed at every path a call can arrive on** — IPC `wallet_call`, the BRC-100 HTTP surface, BRC-121 paid retries, PeerPay/paymail internal calls. Not one arm |
| 4 | **The user is told about a burst once, not once per call.** Legitimate bursts exist (a site pays for N resources at once, a batch payer); the engine already lets those through silently when they are within limits. The fix must not turn a within-limits burst into N prompts, and must not turn an over-limit burst into one prompt that signs N. See 10b's design question |
| 5 | CU-4, CU-5, CU-7 are **re-read at the kickoff** and scheduled then — likely beta.4 sprint 0's neighbours, not this phase |
| 6 | §3's TAAL ARC key: **rotate now** (owner obtains the key; the code stops carrying it in source), and a per-Chromium-bump check lives in `CEF_VERSION_UPDATE_TRACKER.md` step 7 |

## Sub-phases — one contract, one rig, one RED, one commit each; one kickoff and one adversarial panel

| Sub-phase | Defect | Layer | Rig | Contract |
|---|---|---|---|---|
| **10a** | CU-3 fabricated PeerPay credited and auto-confirmed; CU-6 `internalizeAction` accepts the wrong transaction (shares the Atomic BEEF subject binding) | Rust: `beef.rs`, `monitor/task_check_peerpay.rs`, `monitor/task_sync_pending.rs`, `handlers.rs :: store_derived_utxo` / `internalize_action` | unit tests on hand-built Atomic BEEF envelopes + a live PeerPay from a second wallet | `10a-peerpay-atomic-subject/PHASE_CONTRACT.md` |
| **10b** | CU-1 one Approve releases every pending prompt for the domain; CU-8 check-then-act race on the counters; CU-9 402 reuse cache keyed on URL+sats only | C++ `HttpRequestInterceptor.cpp` (shared, both platforms) + React `BRC100AuthOverlayRoot.tsx` + Rust `request_gate.rs` / `handlers.rs :: pay_402` | a local test dApp page that fires concurrent `createAction` calls over the per-tx cap, dev wallet with real (small) money or `noSend` | `10b-one-click-one-spend/PHASE_CONTRACT.md` |
| **10c** | CU-2 a paymail P2P host can replace the approved amount with any outputs; `http://` capability URLs accepted | Rust `handlers.rs :: paymail_send`, `paymail.rs` | a local stub paymail host (bsvalias `.well-known` + P2P destination endpoint) returning crafted outputs; ⚠️ no `noSend` exists on this endpoint — see the contract's `D-2`/`D-6` | `10c-paymail-outputs/PHASE_CONTRACT.md` |
| **10d** ⭐ added 2026-09-15 | A PeerPay message over MessageBox's 1 MiB cap is broadcast anyway and never delivered (found live during `P10a-A5`: the owner's send, 495 KB BEEF with a 433 KB backup parent, 413 ×20, recipient never told); the header dot goes orange with no words | Rust `handlers.rs :: peerpay_send` / `do_onchain_backup`, `output_repo.rs` selection, `task_retry_peerpay_outbox.rs` + React header/panel/activity | dev wallet; the owner's real 1.76 MB payload seeded into the dev outbox; one real dev backup | `10d-peerpay-delivery/PHASE_CONTRACT.md` |

**Order (owner, 2026-09-15): 10a ✅ → 10d → 10b → 10c.** 10a landed on Windows as `57812cf` + `a91a34a` (A5's poller half owed, `PAYMENT_TEST_BATCH.md` M9). 10d runs before 10b because its RED is live right now and it touches the same PeerPay path 10a just hardened.

⚠️ **Paymail is live in the UI** (`TransactionForm.tsx` resolves handles and posts to `/wallet/paymail/send`), so
10c is a shipped path, not a dormant one.

## Harness requirements, restated so nobody skips them

- Each sub-phase contract has all seven `HARNESS.md` §1 sections, `D-n` deltas from the kickoff re-read, and an
  evidence table whose RED cells name the run that was done, not the run that would fail.
- **Negative controls are the defect itself.** 10a's RED is a fabricated envelope credited; 10b's is two txids from
  one click; 10c's is a 10× broadcast in `noSend`. All three must be *observed* before the fix, on the dev wallet,
  and the same probe must go green after.
- **Two-sided rows where the fix could over-reject:** 10a-E (a genuine PeerPay is still credited once, correct
  amount), 10b-C (five within-limits payments still go silent with zero modals), 10b-D (a fresh site's three
  pre-connect calls still collapse to one connect modal), 10c-B (a host that splits the exact amount across three
  outputs still succeeds).
- **`REGRESSION_SET.md` grows by one row, `R-ONE-CLICK-ONE-SPEND`** (10b-A + 10b-C), added in its **own commit**
  after 10b lands (working rule 6: the instrument is not edited by the change it measures), and run at every
  later boundary.
- Boundary regression at the end of the phase: `R-INTEXT`, `R-GOLD`, `R-COUNT`, `R-PERIM`, `R-DUST` **T2 halves
  actually run** this time — 10b changes the prompt/resume path that `R-INTEXT` and `R-COUNT` measure, and 10a
  changes what enters the `outputs` table that `R-DUST` guards. Phase 9 recorded these as INCOMPLETE; this phase
  cannot.
- `preflight -Full` per commit; `-NegativeControl` after any gate touch; every commit cites its row IDs.
- **Adversarial panel** (`HARNESS.md` §6, "money path / trust boundary ⇒ ✅ adversarial panel"): after all three
  land, a fan-out review with the four questions, the same shape as `phase-0.5-money-path/ADVERSARIAL_PANEL_2`.
- 🍎 **Mac:** 10a and 10c are Rust-only (rebuild + `cargo test` after rebase; 10a-E's live PeerPay can be Windows ↔
  Mac, which is the best two-wallet rig we have). 10b touches `HttpRequestInterceptor.cpp` (shared) and React —
  relay row names the files; the modal change is visible on macOS and needs their eyes.

## Rig notes

- Dev stack only: wallet on 31401, Vite 5137, dev exe `--profile=Default`, CDP 9322. Never the installed build.
- 10b's test dApp: a static page served from a scratch local port (⚠️ not 5137 and not a loopback port the
  wallet gate treats as internal — check `IsInternalOrigin` and `LegacyWalletGateMatch` first; Phase 5's finding
  is that a matcher miss leaves traffic *trusted*). The domain-approval flow must run for it like any site.
- Money: 10b needs real broadcasts to prove "one click, one txid" end to end — cents, on the dev wallet, recorded
  in `PAYMENT_TEST_BATCH.md`. 10a-E is a real PeerPay of a few hundred sats between two wallets. 10c stays in
  `noSend`; nothing from a stub host is ever broadcast.

## Out of scope for the phase

- CU-4/5/7 (decided at kickoff), the loopback caller-authentication item (`AUDIT_FIX_TRACKER.md` FU1), pinning the
  internal origin to `127.0.0.1:5137` (Phase 0.5 panel), the mnemonic-clipboard note.
- Pricing the *built* transaction at the gate instead of the request body (CU-2 follow-up) — recorded, larger change.
