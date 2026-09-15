# Phase 10d — a PeerPay that cannot be delivered is never broadcast, and one that fails to deliver is shown, retryable and recoverable · PHASE CONTRACT

**Workstream:** critical advisories (money path) · **Source:** `../../TICKET_peerpay_message_exceeds_messagebox_limit.md` (measured live 2026-09-15 during `P10a-A5`)
**Status:** ⬜ CONTRACTED 2026-09-15 — owner decisions taken the same day (below). Runs **after 10a, before 10b** (owner).
**Opened:** 2026-09-15 · **Owner:** Matthew Archbold · **Platforms:** Rust + React (both platforms; Mac rebuilds + `cargo test`; the yellow line needs Mac eyes)
**Standard:** `../../HARNESS.md`.

---

## 0. What the tree says (kickoff facts, all measured 2026-09-15)

| # | Fact |
|---|---|
| `D-1` | The only large transaction this wallet produces is the **on-chain backup** (433 KB on the installed wallet; grows with the DB). Every backup leaves one change output whose parent is that backup. Coin selection is largest-first (`output_repo.rs` `ORDER BY o.satoshis DESC`), so after a backup that change is picked **first** for the next send of any kind. It is the default path, not a rare one |
| `D-2` | BRC-62 requires every direct parent **in full** (mined parents included, with a BUMP); BRC-96 txid-only entries are for parents the recipient already holds. Our bundle (12 tx, 11 with BUMPs, 494,725 B) was correct and minimal. Nothing to trim on the BEEF side |
| `D-3` | The wire cost is fixed by interop: `"transaction"` **must stay a JSON int array** — the BSVA receiver does `new Uint8Array(token.transaction)` and a base64 string corrupts (our sender comment of 2026-03-09 records exactly this; we tried base64, it broke, we switched). Array ×3.6, then BRC-2 + base64 ×1.33 ⇒ **≈ 4.7× the BEEF bytes**; under the relay's 1 MiB cap the BEEF must stay under ≈ 220 KB. ⛔ Do not re-try base64 unilaterally; the upstream issues (Marston `Standards/BRCs/drafts/peerpay-messagebox-size-and-encoding/`) ask receivers to accept both first |
| `D-4` | The server measures `Buffer.byteLength(message.body)` — the `{"encryptedMessage": "<base64>"}` string (`ts-stack/infra/message-box-server/src/routes/sendMessage.ts`). We can compute that exact size locally before broadcasting |
| `D-5` | `peerpay_send` order today: `create_action(no_send)` → `broadcast_transaction` → build token → `sendMessage` → on failure queue `peerpay_outbox` → `TaskRetryPeerPayOutbox` 60 s ×10, 120 s ×10 → `exhausted`. A 413 is retried the same as a timeout. The header dot turns orange on `outbox_warning_count`; **no panel UI exists** for it (grep: nothing in `WalletPanel.tsx`/`wallet/*`) |
| `D-6` | `do_onchain_backup` (`handlers.rs :: do_onchain_backup`, step 6) already spends the previous backup's outputs (`previous_sats`) and already pays **no** treasury fee; its top-up funding is largest-first like every send — that is why the change was 39M sats. Backups are triggered by a hash of the whole DB ⇒ any self-send re-triggers a backup ⇒ a "de-taint" self-send would loop (owner caught this). **Decision: no de-taint transaction; backup funding picks smallest-sufficient coins instead** |
| `D-7` | Parent size is already local: `parent_transactions.raw_hex` per txid ⇒ `LENGTH(raw_hex)/2`. Selection can order on it with a join, no new data |
| `D-8` | The BRC-121 paid retry carries its BEEF base64 **in an HTTP header** (`ctx.beefBase64`, `HttpRequestInterceptor.cpp`); a large parent makes that header hundreds of KB, which servers reject. Same cause, second victim — **code reading, not observed** |
| `D-9` | Our receiver already accepts both encodings (`task_check_peerpay.rs :: parse_payment_token`, string or array) — nothing to change there |

## 1. Goal

A PeerPay send either delivers its message or never leaves the wallet; a delivery that fails permanently is shown to the user in yellow, can be retried, and can be handed to the recipient by hand.

## 2. Done means — owner decisions 2026-09-15

- [ ] `"transaction"` stays a JSON int array (documented spec deviation, `D-3`); receiver accepts both (already)
- [ ] For bundle-carrying sends (PeerPay, BRC-121 402) coin selection prefers inputs with **small parents**; ordinary sends unchanged
- [ ] The PeerPay message is built and sized **before** broadcast; over the cap ⇒ the no-send action is aborted (inputs released), nothing broadcast, error names the size and the cap
- [ ] Backup funding picks **smallest-sufficient** coins (previous backup outputs first, then smallest) so the backup's change is small and never the first-picked coin; **no de-taint transaction** (loop, `D-6`)
- [ ] `peerpay_outbox`: a 413 / other 4xx is **permanent** ⇒ `status = 'undeliverable'` on the first one, no further retries; timeouts / 5xx stay `pending` with today's schedule
- [ ] Header dot: **yellow** for undeliverable (was orange), red stays for failed payments, green for received
- [ ] Wallet panel: one-line yellow banner while any `undeliverable` exists; the send's Activity row gets a yellow sub-line naming the cause ("message too large" / "relay unreachable") with **Retry** (rebuilds the message from the stored bytes, re-sends once, re-classifies) and **Copy details** (txid, sender key, derivation prefix + suffix, amount — what a recipient needs to internalize)
- [ ] Housekeeping (backup) stays treasury-fee-exempt (already true, `D-6`); no new fee anywhere
- [ ] ❔ **Claim box** (receiver-side "Claim a payment" field that feeds `/internalizeAction`) — owner to say in or out; contracted as the last row, skippable

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-INTEXT` | internal never prompts | `peerpay_send` reorders create → size → broadcast; the internal `create_action` call must stay domain-less |
| `R-DUST` | 1-sat floor at the four paths | new ORDER BY in selection must not change the floor's inputs; the smallest-sufficient backup funding must not pick sub-floor coins |
| `R-GOLD` | gold pill | untouched; boundary run |
| 10a's `P10a-A3` | a receive never rewrites | Retry re-sends the same message; a second delivery of an already-credited payment must be the no-op path, not a rewrite |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — seen, and how | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P10d-A1` | Selection for a bundle-carrying send returns clean coins first; a coin whose parent is > threshold comes last; ordinary selection order unchanged (control) | Pre-fix: the 433 KB-parent change is picked first (it was, `P10a-A5`; and the T1 shows largest-first ignores parent size) | T1 on `OutputRepository` with a seeded `parent_transactions` row of 300 KB and 3 coins; the two orderings compared | T1 | ⬜ |
| `P10d-A2` | A PeerPay whose message would exceed the cap is refused **before** `broadcast_transaction`; the no-send action is aborted; `outputs` inputs released; error names bytes vs cap | Pre-fix: broadcast then 413 ×20 — **observed live 2026-09-15** (`3798109e…`, installed wallet log) | T1: `peerpay_wire_size(token)` against a hand-built 300 KB BEEF > cap ⇒ refuse path taken, `abort_action` called; T2 (dev wallet, `HODOS_DEV`-gated cap override to 64 KB, same seam pattern as M4): a real small PeerPay is refused with no txid on chain | T1 + T2 | ⬜ |
| `P10d-A3` | Backup funding = previous backup outputs + smallest-sufficient coins; change ≤ one backup's cost; the wallet's largest coin untouched | Pre-fix: largest-first — the installed wallet's backup change was 38,995,637 sats (measured) | T1 on the funding selection with 5 coins; T2: one dev-wallet backup (`POST /wallet/backup/onchain`, ~50K sats fee, real) ⇒ change vout small, largest coin still unspent | T1 + T2 (real fee) | ⬜ |
| `P10d-A4` | `peerpay_outbox` on 413 ⇒ `undeliverable` after **one** attempt; on timeout ⇒ `pending` + schedule | Pre-fix: 413 retried ×20 then `exhausted` — observed live | T1 on the classifier + the outbox transition; T2: dev outbox seeded with the owner's real 1.76 MB payload addressed to the dev key ⇒ one attempt, `undeliverable`, no further attempts in the log | T1 + T2 | ⬜ |
| `P10d-A5` | Header dot yellow + panel banner + Activity yellow line with cause, Retry and Copy details, for an `undeliverable` row; dismiss clears the dot; Copy yields the four fields | Pre-fix: orange dot, no words anywhere (owner saw it) | T3, seeded `undeliverable` row in the dev DB; Retry on a **deliverable** seeded row (small token to the dev key itself) ⇒ delivered, row `delivered`; Retry on the 1.76 MB row ⇒ stays `undeliverable` with the same cause | T3 + T2 | ⬜ |
| `P10d-A6` | ❔ Claim box: pasting Copy-details output into the receiver's field internalizes the payment once | — (new surface; A5's Copy is its RED half: without the box the details have nowhere to go) | T2 on the dev wallet with the owner's real remittance (already credited ⇒ must be the no-op path, `P10a-A3`) | T2 | ⬜ owner decides in/out |

**Two-sided rows:** A2 (over-cap refused) vs the dev-wallet small PeerPay that still sends (control inside A2); A1 (bundle sends re-ordered) vs ordinary sends unchanged.

## 5. Blast radius

Coin selection (every send) — the ORDER BY change is gated on a flag only bundle-carrying callers set; the default path must produce byte-identical selections (A1 control). `peerpay_send` reorder touches the abort path. Backup builder step 6 only. `TaskRetryPeerPayOutbox` classification. `peerpay/status` gains `undeliverable_count`. Header dot colour in `MainBrowserView.tsx`; panel banner; Activity row. 🍎 React + Rust ⇒ relay row, Mac eyes on the yellow line.

## 6. Out of scope

Base64 on the wire (upstream first, `D-3`). Funding backups from a reserved coin. Reducing backup size. The 402 header-size case (`D-8`) beyond a log line. Paymail. Trimming BEEF (`D-2`).

## 7. Rollback

Three Rust commits (selection + refuse; backup funding; outbox classification + status) and one React commit, each independently revertible. No schema change (`status` is a TEXT column; `'undeliverable'` is a new value).

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1 -Full`
- [ ] `-NegativeControl` if any gate is touched
- [ ] Regression set at the Phase 10 boundary
- [ ] Adversarial panel with 10a/10b/10c
- [ ] Commit messages cite the row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight -Full | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
