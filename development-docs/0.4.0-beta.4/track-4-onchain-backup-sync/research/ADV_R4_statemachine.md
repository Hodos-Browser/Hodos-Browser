# ADV R4 — State machine & crash adversary: findings against IMPLEMENTATION_PLAN.md

**Lens:** D7 (intent record), Phase 2/4, §3.3, H5/H14 — the write path as a whole.
**Date:** 2026-08-24. **Method:** plan read in full (all 640 lines); A1/A3 read in full; premises
re-verified directly against `handlers.rs` (13285–14112 write path, broadcast/rollback region),
`main.rs:423-441` (AppState), `monitor/` (task_backup.rs, task_fail_abandoned.rs, destructive-write
grep across all 16 tasks), `database/connection.rs:30-80`. Every finding below states which code or
plan line the premise rests on. D2_plan_critique.md checked: none of these were raised there.

Severity: CRITICAL = loses money / unrecoverable wallet; HIGH = silent wrong state;
MEDIUM = visible failure / bad UX or cost; LOW.

---

## Verified ground facts the findings build on

1. **One DB connection, one mutex.** `AppState.database` is `Arc<Mutex<WalletDatabase>>`
   (`main.rs:424`); `WalletDatabase` wraps a single `Connection::open` (`connection.rs:43`, WAL).
   Monitor tasks lock the same mutex (`monitor/mod.rs:130,177,402,425`). `compress_for_onchain`
   runs inside **one** lock hold (`handlers.rs:13296-13311`) — so today's snapshot *collection* is
   torn-free **by accident of this topology**, not by any stated invariant.
2. **The write path drops and re-acquires the lock ~10 times** between the step-2 collect and the
   step-13 re-baseline (`handlers.rs:13296, 13367, 13625, 13676, 13725, 13890, 14004, 14086,
   14096`). Broadcast (network I/O, historically ~8.6–60 s) happens with the lock free.
3. **Step 13 re-baselines by re-collecting at `ref_ts=now` AFTER broadcast**
   (`handlers.rs:14078-14094`): the stored `backup_hash` is computed from the **post-broadcast DB**,
   not from the payload that was broadcast.
4. **On broadcast `Err`, the handler rolls back**: `rollback_backup` restores all reserved inputs
   (prev PushDrop token, prev marker, funding) to `spendable=1` and returns Err
   (`handlers.rs:13956-13996`; the in-code comment says so verbatim). The
   suspected-double-spend marking fires **only** if `is_double_spend_error(&e)`; a transport
   timeout or 5xx takes the plain rollback path.
5. **Failing a tx deletes its outputs and restores its inputs.** TaskFailAbandoned's documented
   order: "mark failed → delete ghost outputs → restore inputs" (`task_fail_abandoned.rs:8`), on a
   **pure timer** (300 s for unprocessed/unsigned; 600 s for a backup stuck in 'sending',
   `task_fail_abandoned.rs:19-24`). Failed txs are then excluded from the payload entirely
   (`backup.rs:445-446`, A1 §2 item 12).
6. **TaskBackup classifies any handler error containing the substring "skipped" as a clean Skip**
   (`task_backup.rs:89-92`), and `do_onchain_backup` returns the no-change case through the `Err`
   channel as `"skipped: no changes since last backup"` (`handlers.rs:13320`).
7. **The plan contains zero occurrences of** "reorg", "evict"(-ion), or any
   accepted-but-never-mined broadcast state; the §5 mock fault list is exactly: index lag, JSON
   truncation, 404, ARC 200-with-different-txid, DOUBLE_SPEND_ATTEMPTED-for-both. There is no
   "broadcast reported failure but the tx propagated" fault and no "accepted then dropped from
   mempool" fault.

---

## R4-1 — CRITICAL — Ambiguous broadcast outcome: rollback-on-timeout re-arms the E4 double-spend, and D7's intent record is silently defined out of the one case it exists for

**Starting state.** Healthy wallet, chain tip = token N−1. A backup cycle runs (no crash anywhere
— this finding needs no kill signal).

**Steps.**
1. Steps 1–10 complete; intent record (D7) written: (planned txid T_N, seq N, parent N−1,
   payload hash). Inputs reserved.
2. `broadcast_transaction` submits the BEEF. ARC **accepts and propagates T_N**, but the HTTP
   response is lost — timeout, connection reset, or gateway 5xx. The handler sees `Err(e)` where
   `is_double_spend_error(e)` is false.
3. Shipped code path (verified, fact 4): `rollback_backup` restores the prev token N−1 outpoint,
   the marker, and the funding UTXOs to `spendable=1`; handler returns Err; TaskBackup records a
   Failed outcome and will retry after backoff.
4. What happens to the intent record on the `Err` path is **specified nowhere in the plan**. D7
   says the intent is "consumed by a startup reconcile" — but there is no crash and no startup
   here. The natural implementation (delete the intent when the broadcast "failed", mirroring the
   rollback) erases the only durable evidence that T_N may exist.
5. Next cycle (minutes later, after backoff): the wallet builds T_N′ — **same parent outpoint
   N−1, same or overlapping funding inputs** — and broadcasts. Meanwhile T_N has propagated and
   possibly mined.
6. T_N vs T_N′ is a same-parent conflict: at best ARC rejects T_N′ E4-Bug-B style (200 +
   different txid — the collision guard catches the txid mismatch and treats it as another
   failure → loop), at worst both circulate and TaskVerifyDoubleSpend gets
   DOUBLE_SPEND_ATTEMPTED for both (E4 Bug C's exact input shape). The DB meanwhile believes
   N−1's token is spendable and T_N does not exist — the 2026-04-11 cascade preconditions,
   reconstructed step by step in the *new* design.

**Violated guarantees.** G3 (no accepted double-spend), G6 (a broadcast happened for a change
that was already backed up), G5 (the state does not converge — it oscillates through
conflict-rejection until an oracle-driven heal happens to win).

**Why the harness misses it.**
- **H5** injects *hard kills at step boundaries*. There is no kill here — the process stays up
  and takes a branch. "After broadcast/before intent-consume" as a kill point does not cover
  "broadcast returned Err yet the tx propagated": that is a **fault-model entry, not a kill
  point**, and fact 7 shows the mock has no such fault. Every H5 run either broadcasts cleanly or
  fails cleanly.
- **H6**'s ARC faults are 200-different-txid and DOUBLE_SPEND — both are *responses*. The
  no-response/ambiguous case is absent.
- **H16** contracts specify "exact semantics of what is returned" — an ambiguous outcome has no
  return to contract, and nothing in G12's language forces a timeout-semantics clause.

**Defense audit (rule 4).** D7's intent record is the plan's answer to broadcast/record atomicity
— but as written it is consumed by a *startup* reconcile, i.e., it only defends the crash
topology. The no-crash ambiguous-Err topology bypasses it entirely. The retained
suspected-double-spend marking (fact 4) is keyed on error *content*, not error *ambiguity*.
`reconcile_missing_inputs` heals only after the conflict has already been created.

**Fix.** Treat broadcast as three-valued: OK / REJECTED(reason) / **UNKNOWN**. On UNKNOWN: do
NOT roll back reservations, do NOT delete the intent; enter a "broadcast-pending" state that
polls (broadcaster tx-status by txid + indexer, under an H16 contract) until T_N is confirmed
present or provably absent past the propagation window; only then consume or retry. Add the
ambiguous-broadcast fault to the §5 mock and a dedicated H5 row: broadcast-Err-but-propagated,
then next cycle — PASS = no same-parent second broadcast ever occurs.

---

## R4-2 — CRITICAL — H14's mandated age-out of never-broadcast noSend txs re-creates E4 Bug C: three honest oracles cannot prove a counterparty-held tx dead

**Premise (verified).** A noSend action's raw tx is held by the counterparty/dApp, which may
broadcast it at any later time — the plan's own live evidence is exactly this: 4 `nosend` reqs
whose txs were mined **externally, months later** (§3.3). "Genuinely dead → aged out to `failed`
via the existing three-oracle quorum" (§3.3, Phase 2), and H14's PASS **requires** "the dead req
ages out to `failed`". Failing a tx deletes its outputs and restores its inputs to spendable
(fact 5), and failed txs vanish from the payload (fact 5).

**Steps.**
1. Wallet co-signs a noSend action A spending wallet input U, creating token output O_A for the
   wallet. Counterparty holds the fully-signed raw tx and does not broadcast (their prerogative —
   escrow-style flows sit unbroadcast for weeks by design).
2. Age-out horizon passes. The three-oracle quorum is consulted: WoC, GorillaPool, third oracle
   all answer **"unknown txid"** — which is the truth: not on chain, not in their mempools.
   Unanimous "unknown" satisfies any quorum rule the plan could implement, because **absence
   evidence is the only evidence a chain oracle can give about a tx that was never sent to it.**
3. Req ages to `failed`. Established failed-tx semantics run: O_A deleted, U restored
   `spendable=1`. Next delta carries the delete of O_A and the update of U as *genuine state
   transitions* (§3.2: "Delete records in deltas represent only genuine state transitions" —
   this transition is genuine from the producer's view). The chain now permanently records it;
   every synced device replays it.
4. Wallet spends U in a normal payment P.
5. Counterparty broadcasts A. Now A and P conflict on U (whichever mines, the other party holds
   a valid competing tx): if A mines first, P's outputs are phantoms and the wallet's payment
   recipient is double-spent against; if P mines first, the counterparty's A fails and O_A —
   an output the user may have *paid for* — never materializes, while the wallet long ago
   deleted every record of it. Either way, DOUBLE_SPEND_ATTEMPTED lands on both txs at
   TaskVerifyDoubleSpend — Bug C's exact ambiguous input, now by design.

**Violated guarantees.** G1/G2 (O_A's metadata — PushDrop derivation, custom_instructions —
was deleted and delta-tombstoned; if A later mines, the output is on chain but unrecoverable
as spendable from backup: the tombstone replays on recovery), G3 (accepted double-spend
exposure), G6 (extra broadcasts from the churn).

**Why the harness misses it.** H14's fixture set is exactly two cases: mined-externally
(reconciled) and "genuinely unmined past the age-out horizon" (must age out). **The fixture
that breaks the policy — aged out and THEN broadcast by the counterparty — is not in the
fixture list**, and H14's PASS line enforces the hazardous transition ("the dead req ages out
to `failed`"). H14's only guard is "age-out firing while any oracle still reports the tx in
mempool" — which never fires for a never-broadcast tx. H4/H9 never inject a foreign
counterparty broadcast of a wallet-cosigned tx.

**Defense audit (rule 4).** The plan's defense is (a) the three-oracle quorum and (b) "owner
sign-off on the age-out policy". (a) is defeated structurally: the quorum answers "is it on
chain/in mempool *now*", which is the wrong question — deadness of a counterparty-held tx is
unknowable from chain oracles; unanimity adds nothing. (b) is a deferral, not a defense — and
the plan hands the owner a policy space with a hidden fork (see R4-8 for the other horn)
without stating the fork exists.

**Fix.** `failed`-via-age-out for noSend must NOT restore inputs or delete outputs. Introduce a
distinct terminal-ish state `abandoned-unbroadcast`: raw_tx dropped from the payload (cost win
preserved) but inputs stay encumbered until either (i) the wallet itself double-spends U
deliberately with an explicit user action ("cancel this pending action" — the only party who
can genuinely kill A is us, by spending its inputs), or (ii) A appears on chain and flips to
the proof pipeline. H14 gains the third fixture: age-out → counterparty broadcasts → assert no
conflicting wallet spend was possible and O_A recovers.

---

## R4-3 — HIGH — The step-13 re-baseline absorbs every mutation that lands during the broadcast window: the change is never backed up, the dirty flag reads clean, and health shows green (E12 class, no fault injection needed)

**Premise (verified, facts 2–3, 6).** Step 2 collects the payload and hashes it; the lock is
then released; broadcast takes seconds to a minute; step 13 **re-collects the whole payload at
`ref_ts=now` and stores that hash as the new baseline** (`handlers.rs:14078-14094` — comment:
"captures the post-backup state"). Any DB mutation between step 2 and step 13 is therefore
**inside the stored baseline but was never inside any broadcast payload**.

**Steps.**
1. Backup N's step-2 collect completes at t₀; broadcast begins.
2. At t₀+20 s a PeerPay receive lands: BRC-42 counterparty output with `sender_identity_key`
   and `custom_instructions` — the exact non-re-derivable class G2 names. (The receive handler
   takes the DB mutex in one of the windows of fact 2.) `request_backup_check_if_significant`
   fires — the trigger side works as designed.
3. Step 13 runs at t₀+40 s: recomputed hash **includes the receive**; stored as baseline.
4. Debounced trigger fires 3 min later → `do_onchain_backup` → step-2 collect (includes the
   receive) → hash == stored baseline → `Err("skipped: no changes since last backup")`
   (`handlers.rs:13320`) → TaskBackup: `BackupOutcome::Skipped` (fact 6). Not a failure;
   consecutive-failure count 0; health green.
5. Wallet goes idle (user done for the month). Every 3-hour periodic check: skip. **The receive
   is in no on-chain payload and never will be until some unrelated change dirties the
   payload.** Three silent months are back — with the health surface reporting healthy the
   whole time, because nothing failed.
6. Device dies; recovery from chain restores state as of backup N: the PeerPay output's
   spending metadata is absent. G2 calls this "a failure equal to loss".

**Under the new design (§3.3), the bug survives by ambiguity.** The plan says: "Producer state:
keep the last-backed-up payload locally." It never pins **which instant** that payload
represents. The shipped precedent (fact 3) is "re-collect after broadcast" — if Phase 5's
producer follows it, every mutation in the broadcast window is absorbed into the delta baseline
and **never emitted in any delta**. The correct semantics — baseline := the exact bytes/rows
that were broadcast (step-2 collect) — is one line, but the plan neither states it nor cites
the absorb window as the reason. Neither A1 §3e nor A3's episode catalog names this window
(A1 describes the recompute as capturing "the tx's own side effects" — it captures everything
else's side effects too), so this is a live, un-cataloged silent-loss bug in the shipped code
that the plan is poised to carry forward.

**Violated guarantees.** G2 (spendable output older than 10 min unrecoverable, no declared
exclusion applies), G5 (failure invisible — it presents as success), and the E12
"three silent months" class the plan promises is impossible (H9's own words: "three silent
months must be impossible").

**Why the harness misses it.** H3 is a **single-threaded** property test: randomized op
sequences with snapshots "at arbitrary points" — arbitrary points *between* ops; nothing
mutates *during* produce/broadcast. H9's mock broadcaster has no specified latency — a mock
that answers in microseconds shrinks the absorb window to ~0, so the soak's "recovery-diff
after every op batch" samples a window that effectively never opens. H5's kill points bracket
the window but a *kill* in the window triggers the crash path, not the absorb path — the
absorb needs the cycle to *succeed* with a concurrent write.

**Defense audit (rule 4).** §3.3's dirty-flag redesign (canonical rows, ORDER BY, pinned
ref_ts, no volatile columns) defends determinism (M2) — none of it addresses *which snapshot
instant is the baseline*. Constraint 2's "computed identically pre- and post-write" is about
hash inputs, not about the baseline's timestamp identity.

**Fix.** One sentence in §3.3: *the baseline is the exact collected state that was broadcast;
the post-broadcast step re-derives the baseline from (broadcast payload + the backup tx's own
deterministic side effects), never from a fresh DB collect.* Plus: give the H9 mock a
configurable broadcast latency (default ≥ 10 s simulated) and add an H3 generator mode that
interleaves ops with an in-flight produce/broadcast; PASS = every interleaved op appears in a
subsequent delta.

---

## R4-4 — HIGH — D7's startup reconcile cannot distinguish "never broadcast" from "broadcast, not yet indexed": the crash-point enumeration the plan skipped

D7 specifies *that* an intent record exists and *that* a startup reconcile consumes it —
and nothing else. Enumerating the crash points (the task the plan's one sentence hides):

| # | Crash point | Startup state | Reconcile must decide | Hazard |
|---|---|---|---|---|
| 1 | after intent, before build/sign | intent, no raw tx cached | T_N unknowable — planned txid recorded but tx never existed | safe iff intent carries "signed=false"; plan doesn't say the intent records build progress |
| 2 | after sign, before broadcast | intent + cached raw tx (`parent_transactions`, step 10) | is T_N on chain? Truth: no | **indistinguishable from #3 inside the indexer lag window** |
| 3 | after broadcast, before record | intent, tx propagating; WoC unspent index blind to it for 30 s–5 min (D12's own lag window) | is T_N on chain? Truth: yes, invisible | reconcile queries the index, sees nothing, concludes #2 |
| 4 | after record, before intent consume | intent + matching DB rows | trivial: consume | safe (idempotent) |
| 5 | double crash: #2/#3, restart, reconcile decides "retry", new intent written, crash again | **two intents, same parent, different planned txids** | which (if either) is on chain; at most one can win | plan gives no intent multiplicity/supersession rule; H5 has no double-crash case |

The killer is #2 vs #3. Both present identically to the reconcile: intent present, planned
txid absent from the index. If reconcile resolves optimistically ("not found → not broadcast →
unreserve inputs, drop intent, rebuild"), then in topology #3 it has just recreated E11's
exact state — believed-spendable funding actually consumed on-chain — followed by R4-1's
same-parent conflict on the rebuild. If it resolves pessimistically ("wait"), the plan
specifies no wait duration, no second-oracle consultation, and no terminal state for the wait
— D6's recency check (cross-validated spent-check, fail closed) is specified **for recovery
discovery only**, not for the write-path reconcile.

**Violated guarantees.** G3/G6 (same-parent double broadcast), G5 (non-convergent oscillation
if the reconcile guesses wrong repeatedly).

**Why the harness misses it.** H5 lists the right kill points but runs each against a
**well-behaved mock**: nothing in H5's setup crosses kill points with the index-lag fault
state (the mock supports lag — H5 just never demands the product). "Restart with a stale DB"
(the E11 replay) is the one crossed case, and it's crossed with a *stale DB*, not with
*fresh-broadcast-invisible-to-index*. The pass criterion "wallet converges without manual
intervention" would be satisfied by an optimistic reconcile against a lag-free mock — green
harness, wrong reconcile.

**Defense audit (rule 4).** D7 + constraint 8 is the plan's defense for exactly this, and it
is underspecified into unsoundness: an intent record only helps if the reconcile's decision
procedure is lag-aware. Constraint 7 ("never select inputs from an index younger than the
propagation window") points at the rule but is mapped (§4.2) to D6/H5/H6, and no plan text
applies it to the intent reconcile.

**Fix.** Specify the reconcile decision table in Phase 4: intent present + txid not visible →
consult broadcaster status by txid (H16-contracted) AND wait out the declared propagation
window before concluding "not broadcast"; keep inputs reserved throughout; cached raw tx
(step 10) permits re-broadcast instead of rebuild — re-broadcasting the *same* txid is
idempotent and annihilates the same-parent conflict entirely. Intent table gets a uniqueness
rule: at most one live intent per parent outpoint; a superseding intent may only be written
after the prior one is resolved. H5 gains: every kill point × index-lag-active, plus the
double-crash row.

---

## R4-5 — HIGH — Reorg after the nosend/proof reconciliation strips raw_tx that can no longer be re-fetched: recovery in the window loses non-re-derivable transactions

**Premise (verified).** §3.3/Phase 2: mined nosend txs are routed into the proof pipeline;
once `proven_tx_id` is set, the next payload strips their raw_tx (`backup.rs:449-457` — the
proven-gated strip the plan keeps). G8 declares stripped bytes "re-hydrated on recovery,
byte-identical" — re-hydration is fetch-by-txid from the indexer (A1 §1b step 6). The plan
contains zero occurrences of "reorg" (fact 7); nothing specifies a proof-depth threshold
before stripping, and nothing transitions proven → unproven on proof invalidation.

**Steps.**
1. Counterparty broadcasts the wallet's noSend tx A; it mines in block B at height h (1 conf).
2. H14's required behavior fires "within one monitor cycle of proof availability": req →
   proven, `proven_tx_id` set.
3. Backup N runs: A's raw_tx stripped from the payload (H14's PASS asserts exactly this —
   "raw_tx absent from the next backup payload").
4. Block B is orphaned in a 1–2 block reorg. A returns to the mempool — or doesn't (the
   counterparty's broadcaster may not rebroadcast; mempools forget). A is now on no chain and
   in no reliable mempool; the wallet's DB still holds A's raw bytes, but **backup N — the
   newest on-chain state — does not.**
5. The device is lost in this window. Recovery: replay to backup N; `refetch_stripped_data`
   fetches A by txid → 404. Refetch is best-effort (A1 §5: warn + counter). A's outputs — a
   token the user holds — have no raw tx, no proof, and (post-Phase 3) no locking script:
   rehydrate-by-outpoint of a reorged-out tx also 404s.

**Violated guarantees.** G8 (byte-identical rehydration impossible), G1/G2 (spendable token
output unrecoverable from seed), G10's spirit (recovery reports the break — but reporting
doesn't restore the bytes; the money-relevant data is gone).

**Why the harness misses it.** H14's fixture set has no reorg case; H6's corruption suite
serves truncation/404/staleness but never *un-mines* a previously-served tx; the mock chain
has no reorg operation at all (fact 7). H1 recovers against a mock that still serves
everything it ever mined.

**Defense audit (rule 4).** The plan's proven-gate ("proof pipeline") is the defense — it is
calibrated for finality but fires at proof-existence, i.e., 1 conf. Nothing else applies.

**Fix.** Strip raw_tx only at a declared confirmation depth (e.g., ≥ 6 confs, constant in the
BRC); on proof invalidation (header chain no longer contains the proof's block), transition
proven → unproven, which re-includes raw_tx in the next payload by the existing gate. Add a
reorg op to the mock; H14 gains a mined→stripped→reorged fixture whose PASS is "raw_tx
returns to the payload within one cycle and recovery in the window still spends A's output".

---

## R4-6 — HIGH — Terminal-token mempool eviction on an idle wallet: broadcast-derived health stays green forever, and D6's recency check legitimately blesses the stale restore

**Steps.**
1. Wallet writes backup N (token T_N, parent T_N−1). ARC accepts: 200, same txid,
   SEEN_ON_NETWORK — the txid-collision guard passes (this is not Bug B). Record written,
   intent consumed, baseline stored. Health: last-success = now, tip = T_N, failures = 0.
2. T_N never mines — mempool eviction (fee policy shift, ancestor limit churn, node restarts;
   SEEN_ON_NETWORK is not a mining promise). No one rebroadcasts: the wallet believes it is
   done, and no monitor task tracks backup-tx confirmation as a *health* input (the plan's
   health surface is "last-success time, chain tip, consecutive-failure count" — all three
   derived from broadcast bookkeeping, none from chain confirmation of the tip).
3. The wallet is idle thereafter (the last backup of a session/era — precisely when a backup
   matters most). No further change → no further broadcast → **no "Missing inputs" failure
   ever fires to reveal the ghost tip.** Health is green for months. (If a later backup does
   run, it fails visibly — the eviction case degrades to MEDIUM; the idle case is the trap.)
4. Device dies. Recovery bootstrap (D6): address index shows marker of T_N−1 as newest —
   T_N was evicted, so it isn't there. The D6 recency check runs the cross-validated
   spent-check on T_N−1's marker: **all oracles truthfully answer "unspent"** (its spender
   was evicted). The check passes; recovery restores state N−1 **silently and per spec** —
   this is not a stale-index race, it is the chain's real state. Everything the user did
   between N−1 and N is gone, while the device that died reported green health including
   "tip = T_N" the whole time.

**Violated guarantees.** G5 ("any backup failure surfaces … within one monitor cycle" — this
failure never surfaces at all; MTTD = ∞, the exact E12 metric H9 promises to bound), G2
(staleness bound broken with no health signal).

**Why the harness misses it.** The mock broadcaster auto-accepts and the mock chain
auto-serves; there is no accepted-then-evicted fault (fact 7). H9's recovery-diff cadence
*would* catch the divergence — but only if the mock can evict; it can't, so every accepted
tx is durable in every soak. H5/H6: same gap.

**Defense audit (rule 4).** D7's read-before-write ghost-healing covers the *inverse* ghost
(on chain, not in DB). This is DB-ahead-of-chain, and the plan's only healer for that
direction is the next write's failure — which the idle wallet never attempts. D6's recency
check is defeated *honestly*: the restore isn't index-stale, it's chain-true.

**Fix.** Health must include tip *confirmation* state: "tip broadcast but unconfirmed for
> X blocks/hours" is a first-class unhealthy state with rebroadcast-from-cache (step 10's
`parent_transactions` copy) as the automatic remedy. Add eviction to the mock fault model;
H9 PASS gains "no green health while the tip is absent from the (mock) chain for > one
cycle".

---

## R4-7 — HIGH — The delta chain immortalizes single-signal destructive writes: the surviving E2/M7 deletion authority, now replicated to every device and burned into chain history

**The audit the lens demanded (plan-level, not old-code-level).** Which planned components
hold destructive authority over output/backup state, and on what signal quorum?

| Component (in the plan's end-state) | Destructive act | Quorum |
|---|---|---|
| noSend age-out (Phase 2, H14) | req → failed (outputs deleted, inputs restored — fact 5) | three-oracle — but see R4-2: quorum answers the wrong question |
| Retained failed-tx rollback (§4.2 row 7) | restore inputs spendable | three-oracle (dce3236) — genuinely defended |
| **TaskFailAbandoned — retained, unmentioned by the plan** | mark failed → **delete ghost outputs → restore inputs** (`task_fail_abandoned.rs:8`) | **timer alone** (300 s / 600 s) |
| TaskSyncPending, TaskReviewStatus, TaskVerifyDoubleSpend, dust consolidation (`spendable=1` writes verified across `monitor/`) | flip spendability, delete phantom rows | various single signals + guards; no unified authority |
| c5b sweep + orphan-marker sweep (steps 1.5/5d, retained in Phase 4's path) | consume/spend markers | WoC unspent + DB cross-ref + cooldown (single oracle + heuristics) |
| **Phase 5 delta producer** | **emits `deletes` for whatever the above did** | **none — it trusts the DB unconditionally** |

The plan's constraint-7 quorum is wired to exactly two of these (age-out, failed-rollback).
A3's M7 — "distributed, destructive authority over output state … no single reconciler" — has
**no consolidation item anywhere in the plan**: §4.2 maps constraint 12 to "Phase 2 hygiene"
(`let _ =`, locks), which is discipline about *how* writes are made, not *who* may make them.
So every monitor task's deletion authority survives — and Phase 5 **promotes** each such
deletion from a local, sync-healable DB mutation into a **permanent, replicated protocol
event**: §3.2 says "delete records in deltas represent only genuine state transitions", but
the producer has no way to distinguish a genuine transition from a buggy one; the
E2-e4b5d1d bug (TaskSyncPending deleting mempool-live outputs on a 30-min timer) executed
under the new design would be *faithfully* emitted as a delta delete, broadcast on-chain
forever, and replayed onto every device and every future recovery.

**The amplifier: cross-device delete/resurrect ping-pong = unbounded paid broadcasts that H9
certifies as correct.** Device A's buggy task deletes live output O → delta (delete O) →
device B replays it; B's own chain-reconcile (TaskReviewStatus/sync — O is on chain and live)
resurrects O → B's next delta (upsert O) → A deletes again… Every round is a *genuine* state
transition, so **G6/H9's "zero no-op broadcasts" passes** — these are logical changes; H3
passes — replay is byte-faithful to the (wrong) state; H4 passes — no fork, no lost row
(the row keeps coming back). The wallet pays mining fees indefinitely for a bug the harness
grades as healthy. (Fee scale is small per event, but E8's "~1200 sats each time" history
shows how these run unattended.)

**Violated guarantees.** G6 in spirit (broadcasts with no *user*-logical change), G7 (cost
no longer bounded by activity), G4/G1 (a wrong tombstone replays into every future
recovery — the deletion is now part of the truth).

**Why the harness misses it.** By construction: every H-test validates that deltas faithfully
record what the DB did. None validates that what the DB did was legitimate. There is no test
class "monitor task misbehaves; blast radius?" — H9's monitor churn is *healthy* churn.

**Fix.** (i) A delete-emission allowlist in the Phase 5 producer: only named state
transitions (user action, confirmed-spent via quorum, snapshot-construction strips — which
§3.2 already excludes from deltas) may emit `deletes`; a DB row deletion with no allowlisted
cause fails the produce loudly. (ii) Adopt the plan's own D3 borrowing properly: the
compaction-safety idea ("never delete the newest two generations") has a row-level analog —
a delete delta for an output that any oracle still reports live is refused. (iii) Add the
M7 consolidation item the §4.2 table silently lacks, or record its explicit rejection.

---

## R4-8 — MEDIUM — H14 has no terminal state for permanent oracle disagreement: a limbo req carries 431 KB-class raw bytes in every payload forever, and H14 passes that run

The other horn of R4-2's fork. H14 FAIL fires if "age-out fir[es] while any oracle still
reports the tx in mempool". So: one oracle's mempool view is stuck (nodes disagree about a
near-policy-limit tx for weeks — the live wallet's own history is 4 reqs stuck since July),
or one oracle of three is simply wrong forever. The req can neither age out (quorum blocked)
nor resolve (never mines). No plan text bounds this state: §3.3's backoff bounds *backup
failures*, not reconciliation limbo. Result: the raw_tx (the measured ~60%/291 KB class)
rides in **every** backup indefinitely — G7's "cost bounded by activity" is violated by a
single stuck req, silently, while every harness test passes: H14's PASS clauses are all
satisfiable with the limbo req still present (it is neither "mined and unproven" nor aged
out), and H9's cost ceilings are frozen from traces that contain no such req.

**Fix.** A declared limbo policy: after N cycles of quorum disagreement, the req's raw_tx
moves to the R4-2 `abandoned-unbroadcast` state (payload drops the bytes; inputs stay
encumbered; nothing destructive happens), surfaced in health. H14 gains a
permanent-disagreement fixture with a bounded-payload PASS criterion.

---

## R4-9 — MEDIUM — §3.3 excludes `updated_at` from the payload while D2 adopts per-row LWW merge: same-row concurrent edits are arbitrated by re-write race order, and H4 cannot see the lost field

D2 adopts "per-row last-writer-wins" (the mergeBRC38 engine's semantics — which arbitrate by
`updated_at`); §3.3 excludes `updated_at` from the dirty decision and argues "on-chain
ordering is `seq`/`parent_txid`, not `updated_at`, so excluding it is safe here". For
*dirty-marking* that's right. For *merge* it quietly changes the algorithm: when devices A
and B both edit row X (a label, a basket assignment, `custom_instructions`) in the same fork
window, resolution order is chain position of the re-written deltas — i.e., which device won
the re-write race after fork detection — not which user edit happened later. The losing edit
vanishes. H4's FAIL list has "lost update", but its PASS quantifies over *rows* ("union of
both devices' changes present … no lost row") — the row is present, one field version is
gone, and the test as written cannot distinguish that from legitimate LWW. Violates G3's
"no lost row (union of both devices' changes survives)" under its natural field-level
reading; silent, so HIGH-adjacent, but scope is convenience fields → MEDIUM.

**Fix.** Decide and write down the merge arbiter: either deltas carry a per-row logical
edit-timestamp (travels, unlike `updated_at`), or the BRC declares chain-order-wins and H4
adds a same-row concurrent-edit case whose PASS asserts the *declared* winner (and the UX
surfaces the overwrite).

---

## R4-10 — MEDIUM — DB-ahead-of-chain after an accepted-then-dropped broadcast: the plan heals only the inverse ghost, and the recovery path walks a chain the device's own DB contradicts

Same entry as R4-6 steps 1–2, but the wallet stays active: next cycle's read-before-write
finds chain tip T_N−1 while the DB parent is T_N. The plan defines fork detection
(same-parent divergence) and ghost-on-chain healing ("a ghost on chain becomes the parent
the next write adopts" — D7); it defines nothing for *my recorded parent does not exist
on chain*. Natural implementations either (a) trust the DB and build on T_N → broadcast
rejected (missing inputs) → backoff → quiescent error → **visible**, hence MEDIUM — but the
wallet is now wedged needing manual action, with T_N's raw bytes sitting unused in
`parent_transactions`, one rebroadcast away from self-healing; or (b) trust the chain and
rebuild on T_N−1 → emits a token whose delta baseline assumed T_N's content was on chain →
the chain now lacks delta N's changes until the next full re-diff (if the baseline follows
R4-3's correct semantics this self-corrects; if not, the changes are absorbed — compounding
R4-3). H5 can't see it (no eviction fault); H9 likewise. Fix: an explicit tip-reconcile
rule — if DB tip ∉ chain, rebroadcast the cached tx (idempotent, same txid) before ever
rebuilding; only rebuild after the broadcaster confirms the txid is unknown/rejected past
the propagation window.

---

## R4-11 — MEDIUM — Scenario (a) verdict: the plan nowhere pins the snapshot boundary; today's torn-read safety is an undocumented accident of the single-mutex topology, and H3 as written would stay green if a refactor breaks it

Direct answer to the lens question "where exactly does the plan solve E2's quiescence
problem": **nowhere, explicitly.** What actually protects collection today (verified,
facts 1): every reader/writer shares one `Arc<Mutex<WalletDatabase>>` around one SQLite
connection, and `compress_for_onchain` runs inside a single lock hold — so the ~15 monitor
tasks physically cannot interleave a collect. E2's *other* half (logically-pending nosend
state inside an atomic snapshot) is genuinely addressed by Phase 2/H14. But:

- No report states the single-connection invariant (A1 maps the code without asserting
  "all writers share the mutex"; A3-E2 says "no transactional snapshot boundary" — true as
  designed intent, accidentally false in current topology). The plan inherits safety it
  doesn't know it has.
- The codebase is actively moving toward a services facade (`AppState.services`, "dormant —
  1.6d.C wires call sites", `main.rs:429`) — the classic prelude to a connection pool. The
  day any writer gets its own connection (WAL mode already permits concurrent
  reader+writer), the Phase 5 changeset producer's multi-query collect becomes tearable:
  e.g., output row read as `spent_by=tx42` before tx42's transaction row exists in the
  read set → delta upserts a dangling reference → replay materializes a state that never
  existed → H1's diff or a recovery FK failure, on some other day, with no pointer back
  to the cause.
- **H3 as written cannot catch the regression**: its op sequences are generated *between*
  produces, never concurrently with one (see R4-3), so torn production is outside its
  sample space by construction. The plan's strongest property test is blind to the exact
  property at issue.

**Fix.** One sentence in §3.3/Phase 5: *the changeset producer reads its entire input
under a single database transaction/lock acquisition (snapshot isolation), asserted in
code*; plus the H3 concurrent-mutation generator mode from R4-3. Cheap now, invisible
later.

---

## Summary table

| ID | Sev | One line | G | H-gap |
|---|---|---|---|---|
| R4-1 | CRITICAL | Broadcast-timeout rollback re-arms E4 double-spend; intent record bypassed with no crash | G3/G5/G6 | no ambiguous-broadcast fault; H5 = kill points only |
| R4-2 | CRITICAL | Mandated noSend age-out re-creates Bug C; quorum can't prove counterparty-held tx dead | G1/G2/G3 | H14 lacks the aged-out-then-broadcast fixture; its PASS enforces the hazard |
| R4-3 | HIGH | Step-13 re-baseline absorbs broadcast-window changes; never backed up; health green | G2/G5 | H3 single-threaded; H9 mock broadcast instantaneous |
| R4-4 | HIGH | Intent reconcile can't tell never-broadcast from not-yet-indexed; double-crash unspecified | G3/G6/G5 | H5 never crosses kill points × index lag; no double-crash row |
| R4-5 | HIGH | Reorg after proof-gated strip makes raw_tx unfetchable; recovery loses the tx | G1/G2/G8 | no reorg op in mock; H14/H6 lack the fixture |
| R4-6 | HIGH | Evicted terminal token + idle wallet = green health forever; D6 blesses stale restore | G5/G2 | no eviction fault; health metrics all broadcast-derived |
| R4-7 | HIGH | Delta chain immortalizes single-signal deletions (TaskFailAbandoned et al.); ping-pong burns fees; M7 unaddressed | G4/G6/G7 | every H-test validates fidelity to the DB, none validates DB legitimacy |
| R4-8 | MEDIUM | No terminal state for oracle-disagreement limbo; 431 KB rides forever | G7 | H14 passes the limbo run |
| R4-9 | MEDIUM | LWW arbiter (`updated_at`) excluded from payload; same-row merge decided by re-write race | G3 | H4 checks row union, not field-level |
| R4-10 | MEDIUM | DB-ahead-of-chain ghost has no defined heal; wedge or baseline skew | G5/G6 | no eviction fault in H5/H9 |
| R4-11 | MEDIUM | Snapshot atomicity is an undocumented accident; H3 blind to concurrent produce | G4/G1 | H3 generates no concurrent mutation |
