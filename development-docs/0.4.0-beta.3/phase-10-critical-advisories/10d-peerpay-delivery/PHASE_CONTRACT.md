# Phase 10d — a PeerPay that cannot be delivered is never broadcast, and one that fails to deliver is shown, retryable and recoverable · PHASE CONTRACT

**Workstream:** critical advisories (money path) · **Source:** `../../TICKET_peerpay_message_exceeds_messagebox_limit.md` (measured live 2026-09-15 during `P10a-A5`)
**Status:** ✅ **LANDED on Windows 2026-09-15** — every row GREEN with its RED seen except `P10d-A5`'s visual half (T3, owed; 🍎 Mac too). `R-PEERPAY-DELIVERY` added to `REGRESSION_SET.md` in its own commit. Ran after 10a, before 10b (owner).
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
- [ ] Wallet panel: one-line yellow banner while any `undeliverable` exists; the send's Activity row gets a yellow sub-line naming the cause ("message too large" / "relay unreachable") with **Retry** (rebuilds the message from the stored bytes, re-sends once, re-classifies) and **Copy details**
- [ ] **Copy details copies the payment claim block** — format fixed in `PAYMENT_CLAIM_BLOCK.md` (BRC-100 `internalizeAction` field names: `txid`, `outputIndex`, `senderIdentityKey`, `derivationPrefix`, `derivationSuffix`, plus `amountSatoshis`, `recipientIdentityKey`, `type`, `version`), built only by `handlers.rs :: payment_claim_block`, pinned by a golden-keys test. 👤 Owner 2026-09-15: **whatever 10d emits is what beta.5's claim tool reads**
- [ ] Housekeeping (backup) stays treasury-fee-exempt (already true, `D-6`); no new fee anywhere
- [x] ~~❔ Claim box~~ — 👤 **moved to beta.5** (owner 2026-09-15): `../../../0.4.0-beta.5/TOOLS_TAB_claim_a_payment.md`, a visible Tools tab in the advanced wallet

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
| `P10d-A1` | Selection for a bundle-carrying send returns clean coins first; a coin whose parent is > threshold comes last; ordinary selection order unchanged (control) | Pre-fix: the 433 KB-parent change is picked first (it was, `P10a-A5`; and the T1 shows largest-first ignores parent size) | T1 on `OutputRepository` with a seeded `parent_transactions` row of 300 KB and 3 coins; the two orderings compared | T1 | ✅ GREEN, RED seen (T1 evidence table) |
| `P10d-A2` | A PeerPay whose message would exceed the cap is refused **before** `broadcast_transaction`; the no-send action is aborted; `outputs` inputs released; error names bytes vs cap | Pre-fix: broadcast then 413 ×20 — **observed live 2026-09-15** (`3798109e…`, installed wallet log) | T1: `peerpay_wire_size(token)` against a hand-built 300 KB BEEF > cap ⇒ refuse path taken, `abort_action` called; T2 (dev wallet, `HODOS_DEV`-gated cap override to 64 KB, same seam pattern as M4): a real small PeerPay is refused with no txid on chain | T1 + T2 | ✅ GREEN, RED seen (T1 + T2 tables) |
| `P10d-A3` | Backup funding = previous backup outputs + the **smallest single coin** that covers the rest (smallest coins combined only when no single coin does — the backup's fee is estimated for one funding input, so extra inputs would underpay); the wallet's largest coin untouched unless it is the only sufficient one. *(Amended 2026-09-15 from "change ≤ one backup's cost", which the one-input fee estimate cannot safely deliver.)* | Pre-fix: largest-first — the installed wallet's backup change was 38,995,637 sats (measured) | T1 on the funding selection with 5 coins; T2: one dev-wallet backup (`POST /wallet/backup/onchain`, ~50K sats fee, real) ⇒ change vout small, largest coin still unspent | T1 + T2 (real fee) | ✅ GREEN, RED seen (T1 + T2 tables) |
| `P10d-A4` | `peerpay_outbox` on 413 ⇒ `undeliverable` after **one** attempt; on timeout ⇒ `pending` + schedule | Pre-fix: 413 retried ×20 then `exhausted` — observed live | T1 on the classifier + the outbox transition; T2: dev outbox seeded with the owner's real 1.76 MB payload addressed to the dev key ⇒ one attempt, `undeliverable`, no further attempts in the log | T1 + T2 | ✅ GREEN, RED seen (T1 + T2 tables) |
| `P10d-A5` | Header dot yellow + panel banner + Activity yellow line with cause, Retry and Copy details, for an `undeliverable` row; dismiss clears the dot; Copy yields the four fields | Pre-fix: orange dot, no words anywhere (owner saw it) | T3, seeded `undeliverable` row in the dev DB; Retry on a **deliverable** seeded row (small token to the dev key itself) ⇒ delivered, row `delivered`; Retry on the 1.76 MB row ⇒ stays `undeliverable` with the same cause | T3 + T2 | 🟡 data half ✅ GREEN, RED seen (T2 table); **visual T3 owed** |
| `P10d-A6` | The claim block has exactly the canonical fields, and its remittance fields deserialize straight into `PaymentRemittance` (the type beta.5's claim tool will call `internalizeAction` with) | Rename one field (`senderIdentityKey` → `senderKey`) ⇒ both tests fail | `payment_claim_block_tests` against the one builder, not a hand-typed example | T1 | ✅ **GREEN, RED seen** 2026-09-15 — rename ⇒ `claim_block_has_exactly_the_canonical_fields` and `claim_block_remittance_fields_are_internalize_action_names` both FAILED; restored ⇒ both ok. (Claim box itself moved to beta.5) |

| `P10d-A7` ⭐ 👤 2026-09-15 | **End to end, the sequence that failed:** fixed dev wallet runs a real on-chain backup, **then** sends a PeerPay (a few hundred sats) to the owner's installed wallet ⇒ the PeerPay's inputs have no parent ≥ `large_parent_bytes()` (a tenth of the cap); the message delivers (dev outbox has no row, or `delivered`); the installed wallet's poller credits it **once**, correct amount; the backup's change is small | **Observed live 2026-09-15 on the old build:** backup `8142e84f…` then PeerPay `3798109e…` ⇒ 433 KB parent selected, 413 ×20, recipient never told. Re-confirmed the same afternoon: the installed wallet's next backup `fad3a42c…` left a 38,347,126-sat change with a 436 KB parent as its first-picked coin (read-only query) | dev wallet log (selection + delivery), the BEEF parsed by the probe (every parent's size), installed wallet's `outputs` + `peerpay_received` rows (read-only), WhatsOnChain for both txids | T2 (real money: one backup fee + a few hundred sats) | ✅ GREEN, RED seen live (T2 table) |

**Two-sided rows:** A2 (over-cap refused) vs the dev-wallet small PeerPay that still sends (control inside A2); A1 (bundle sends re-ordered) vs ordinary sends unchanged; A7 (the whole sequence delivers) is the end-to-end control for A1–A4 together.

### T1 evidence — 2026-09-15 (uncommitted tree; `cargo test --release`, bin target **558 passed**, lib 468)

| Row | Tests | RED — the fix removed, same tests | Controls that stayed green in the RED run (not blind) |
|---|---|---|---|
| `P10d-A1` | `a1_bundle_send_skips_the_large_parent_coin`, `a1_large_parent_coin_is_used_last_not_never` | `has_large_parent` made always-false ⇒ both **FAILED** (the 38,347,126-sat backup change picked first) | `a1_ordinary_send_order_is_unchanged` ok |
| `P10d-A2` (T1 half) | `a2_the_measured_oversized_token_is_refused` (the live 1,764,588-byte token ⇒ ≈2.35 MB > 1 MiB), `a2_boundary_matches_the_server_rule` | size check made never-refuse ⇒ both **FAILED** | `a2_an_ordinary_token_fits`, `wire_body_len_equals_the_serialized_body` (estimate = a real serde render for 6 sizes) ok |
| `P10d-A3` (T1 half) | `a3_backup_funding_is_smallest_sufficient` | selector sorted largest-first ⇒ **FAILED** | `a3_smallest_sufficient_never_takes_a_token_reserved_output` (R-DUST floor) ok |
| `P10d-A4` (T1 half) | `a4_rejection_is_permanent_api_error_is_not` | `is_permanent` made always-false ⇒ **FAILED** | — |

⚠️ Found by its own test first: the initial `select_utxos_smallest_sufficient` test expected "combine two small coins"; the backup's fee is estimated for one funding input, so the rule was set to *smallest single coin first* and the test and `P10d-A3`'s wording amended to match.

### ⚠️ Design change found by the T2 rig — the large-parent line is derived from the cap

The fixed `LARGE_PARENT_BYTES = 100_000` would have made `P10d-A7` vacuous on the dev wallet: its own backup is only
**78,944 B**, under the line, so neither the old nor the new selection could fail there. The line is now
`large_parent_bytes() = messagebox_max_body_bytes() / 10` (≈ 105 KB at 1 MiB, unchanged in production), so the
dev-only cap override scales both together. A1's tests and their RED were re-run on the derived line (threshold
forced to `usize::MAX` ⇒ `a1_bundle_send_skips…` and `a1_large_parent_coin_is_used_last…` FAILED;
`a1_ordinary_send_order_is_unchanged` ok).

### T2 evidence — 2026-09-15, dev wallet `HODOS_DEV=1`, 31401, fixed build

| Row | GREEN — run | RED — run | SUBJECT |
|---|---|---|---|
| `P10d-A3` | `POST /wallet/backup/onchain` ⇒ backup `8792aba6…3422` (80,194 B): inputs = previous PushDrop 1,000 + marker 546 + **one** funding coin **16,080** sats; change **8,057** sats; the wallet's largest coin (`8059173e…:2`, 4,296,057 sats) still spendable | installed wallet, old build: backup `fad3a42c…` left **38,347,126** sats of change as the first-picked coin (read-only query, 2026-09-15) | WhatsOnChain inputs/outputs of the backup; dev DB `outputs` |
| `P10d-A7` | cap override 250 KB (large line 25 KB). Backup above, **then** `POST /wallet/peerpay/send` 700 sats to the installed wallet ⇒ `b21e2c88…cb94`: 4 inputs, parents **259 / 259 / 260 / 260 B** (the 4.29 M coin with the 78,944 B backup parent skipped); `message sent via MessageBox` first try; **installed wallet credited once**: `peerpay_received` id 423, 700 sats, `outputs` `b21e2c88…:0` 700 spendable; dev outbox 0 rows, 0 `pending-%` reservations | live 2026-09-15 on the old build (backup `8142e84f…` then PeerPay `3798109e…` ⇒ 433 KB parent, 413 ×20) | WhatsOnChain parent sizes; dev log; installed DB read-only |
| `P10d-A4` | the owner's real 1,764,588-byte payload seeded into the dev outbox as `pending` ⇒ **real** MessageBox answered `413 ERR_MESSAGE_BODY_TOO_LARGE`; log `🚫 Relay refused … marking undeliverable`; `status='undeliverable'`; one `undeliverable:{txid}` notice; **1 attempt** in the log after a further 35 s | installed wallet, old build: the same body retried **20×** then `exhausted` (its log) | dev log attempt count; dev DB outbox + notice rows |
| `P10d-A5` (data half) | `/wallet/peerpay/status` ⇒ `undeliverable_count 1`; `/wallet/activity` item ⇒ `outbox_undeliverable`, `outbox_cause "message_too_large"`, `outbox_message_bytes 2,352,871`, `outbox_cap_bytes 250,000`, `outbox_claim_block` with exactly the canonical fields. **Dismiss** ⇒ `undeliverable_count 0` and still 0 after the next retry tick (backfill does not re-raise a dismissed notice); Activity row keeps cause + Retry + Copy details | **same run:** after Dismiss `outbox_warning_count` stays **1** — the field the old header dot read, which is why the owner's dot could never be cleared | status + activity JSON |
| `P10d-A2` (T2) | cap override 2,000 B ⇒ `POST /wallet/peerpay/send` 700 ⇒ **HTTP 422 `ERR_PEERPAY_MESSAGE_TOO_LARGE`** (message 339,639 vs cap 2,000); spendable sats **29,110,536 before and after**; 0 reservations; outbox unchanged; the built tx `137f05bb…` marked `failed` and **WhatsOnChain 404** | **same cap, check removed** (`peerpay_message_fits(0, usize::MAX)` at the call site, rebuilt) ⇒ HTTP 200, broadcast `35911c48…d278` `SEEN_ON_NETWORK`, spendable **−1,900** | snapshot of dev DB before/after; WhatsOnChain |

Seeded test rows removed after the run (outbox + notice for `b21e2c88…`). Real money spent: one dev backup fee,
PeerPay `b21e2c88…` 700 + fee, RED PeerPay `35911c48…` 700 + fee — both 700-sat payments credited to the owner's
installed wallet. Note: the claim block shown in the A5 run carries the **seeded** payload's derivation strings
(a test artefact), not `b21e2c88…`'s real ones.

**Still owed:** `P10d-A5` **visual** half (T3 — a person looks at the yellow dot, banner and Activity line in the dev
browser; 🍎 Mac the same); `preflight -Full` result below.

### ⭐ Standing check `R-PEERPAY-DELIVERY` — added to `REGRESSION_SET.md` in its **own commit after 10d** (rule 6)

Two halves, so the cheap one runs at every boundary and the real-money one is scheduled, not skipped:

| Half | What runs | Cost |
|---|---|---|
| **T1 + refuse (every boundary)** | `cargo test` rows A1/A3/A4/A6, plus the refuse path on the dev wallet with the dev-only cap override (`HODOS_DEV=1 HODOS_MESSAGEBOX_MAX_BODY_BYTES=<small>`) ⇒ a PeerPay is refused, **no txid is broadcast**, balance and reserved-coin count unchanged | free — nothing leaves the wallet |
| **End to end (release candidate + any phase touching selection, backup, PeerPay or MessageBox)** | `P10d-A7`: backup, then PeerPay to a second wallet, recipient credited once | one backup fee + a few hundred sats, recorded in `PAYMENT_TEST_BATCH.md` |

⛔ **Designed not to break anything else** (owner concern 2026-09-15):
1. **Nothing oversized is ever broadcast on purpose.** The failing condition is produced by *shrinking the cap* for one dev launch, never by building a large transaction. A refused send is aborted before broadcast, so there is no stranded payment, no outbox row and no notification left behind.
2. **The override cannot reach users.** It is read only under `HODOS_DEV=1`, which a production binary scrubs (`main.rs :: enforce_dev_safeguard`); it is set on the launch command for that run, not in any profile or file.
3. **Each run asserts it left the wallet clean**: no `undeliverable` / `pending` outbox rows created, no `pending-%` reservations, balance unchanged (refuse half) or changed by exactly the send + fee (end-to-end half).
4. **The end-to-end half does not force a large parent** — it runs the ordinary backup the wallet makes anyway, which is exactly the real-world trigger.

### Sending rule until 10d ships in an installed build (👤 owner asked 2026-09-15)

An **old-build** wallet (installed Windows, or Mac before its rebuild) must not be used as a **PeerPay sender** when its largest confirmed coin is a backup's change: it selects largest-first and will fail the same way. Check read-only first — largest selectable coin and its parent's size. 2026-09-15 result for the installed Windows wallet: **do not send** (38,347,126-sat coin, 436 KB parent). Receiving on an old build is unaffected.

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
| preflight -Full | **PASS** — all checks ran and passed (T0 gates, cargo test ×2, hodos_tests, frontend build) | 2026-09-15 | Windows |
| preflight -NegativeControl | n/a — no gate pattern or baseline touched (rule 6) | 2026-09-15 | Windows |
| regression set | | | |
| adversarial review | | | |
