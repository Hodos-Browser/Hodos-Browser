# ADV_R2 — Multi-Device & Concurrency Adversary

**Lens:** R2. **Target:** `IMPLEMENTATION_PLAN.md` (delta chain + multi-device sync).
**Attack surface:** G3, D6/D7, §3.3, Phase 7, H4/H9. **Written:** 2026-08-24.
**Method:** read the plan in full; re-verified the concurrency-relevant premises against
`rust-wallet/src/handlers.rs` (backup/recovery region), `monitor/task_backup.rs`, the BRC draft
§5–§7, DELTA_ANALYSIS.md §3, and A1/A3/C1. Where I assert a code fact I cite the line; where the
plan defends I try to defeat the defense before reporting.

**Premises verified against code (2026-08-24, repo `6b4a4be`):**

- No multi-device machinery exists yet: `grep` for `device_id`, `BackupIntent`, read-before-write,
  poll-before-spend, delta-apply across `rust-wallet/src/` returns **nothing**. All of Phase 4/5/7
  is unbuilt; the harness tests below are the only thing standing between the design and the money.
- `adopt_onchain_backup` (`handlers.rs:13149-13228`) adopts a token's **outpoints** (PushDrop vout 0
  + marker vout 1) as the parent to spend next. **It never decrypts or applies the token's
  payload.** This is the only "read another device's write" primitive that exists, and it does not
  merge state. The plan's read-before-write "apply any token it has not yet seen" is therefore a
  *new* mechanism Phase 7 must build; it does not inherit a working primitive.
- Write-path adopt / spent-check selects the tip by **`max_by_key(height)` over the WoC
  `unspent/all` index** (`handlers.rs:13461-13466`, `14766-14771`), whose lag A3 E4 pins at
  30 s–5 min. This is the shared oracle both devices read.
- Recovery order (BRC §6 step 6): "apply that snapshot, then every delta after it **in `seq`
  order**." Merge semantics (D2, C1 §1.4) are **per-row last-writer-wins**, but the plan's §3.3
  **excludes `updated_at`** from the on-chain decision ("on-chain ordering is `seq`/`parent_txid`,
  not `updated_at`"). So on-chain there is no per-row LWW tiebreaker column at all.
- `seq` is "**Monotonic per wallet, across all devices**" (BRC §5 table) with **no allocator named
  anywhere** in plan, draft, or code.

---

## R2-1 — `seq` collisions make recovery order nondeterministic; row-level upsert/delete do not commute → different devices recover different state (CRITICAL)

**Violates:** G3 (converge to one tip; union survives), G4 (no silent data loss). **Defends:** §3.3,
BRC §6/§7, H4.

**Starting state.** Chain tip = snapshot `T10` (`seq=10`). Devices A and B both online, both read to
tip, both see `parent=T10, seq=10`. `seq` has no global allocator — each device computes
`seq = last_seen + 1 = 11` locally (there is no other way on a broadcast chain: allocation would
need consensus the chain cannot give *before* broadcast).

**Sequence.**

1. A writes delta `DA`: `seq=11, parent=T10`, changeset = {upsert row R (basket to "collectibles")}.
2. B, within one broadcast latency, writes delta `DB`: `seq=11, parent=T10`, changeset =
   {delete row R} (B spent the output R represents).
3. Both confirm. §7.1 fork detection fires (same `parent_txid`) — but **both carry `seq=11`**.
4. A fresh install (or a third device) now recovers. BRC §6: walk to snapshot `T10`, "apply every
   delta after it **in `seq` order**." Two deltas at `seq=11`. There is no total order. Recovery
   sorts by `seq`, ties, and applies them in **fetch/iteration order** — which depends on which
   marker the WoC index returns first.

**Break.** `upsert R` and `delete R` **do not commute**. Fetch-order {DA,DB} → R deleted. Fetch-order
{DB,DA} → R present as "collectibles". Two honest recoveries of the same chain yield different
wallet state (a spendable output present in one, absent in the other). That is silent divergence and,
in the "R present when it was really spent" direction, a **recovered output that will double-spend on
first use** (the E1-58cf9a3 inflation class, now via concurrency).

**Why the harness misses it.** H1 builds "1 snapshot + N deltas" as a **single-writer linear chain**
(N in {0,1,5,20}); seq is always consistent with parent order, no ties. H3 is a **single-producer**
property test ("two runs of the producer on identical state emit identical bytes") — it never places
two deltas at the same seq. H7 constructs its own **linear** chain. H4 exercises live two-device
forks but its PASS is "union of both devices' changes present after convergence" and "convergence in
≤2 rounds" — it asserts *post-convergence* state, and it never asserts **recovery determinism across
fetch order over an unconverged (dual-seq-11) chain**, which is exactly what a fresh install hits if
it recovers during the fork window. No test pins "same chain, two fetch orders, identical recovery."

**Proposed fix.** Make `seq` a **total order by construction**: define the recovery order as
`(seq, tiebreak)` where `tiebreak` is a deterministic function of the token (e.g. lexicographic
`txid`), and state it normatively in BRC §6. Add an H-test: build a chain containing a same-`seq`
fork, recover it under **both** fetch orders, assert byte-identical result. Better: forbid two tokens
sharing `seq` from ever both being "valid" — recovery must pick one branch by the same tiebreak the
writers use (see R2-2) and drop the other, and the property test must prove the pick is fetch-order
independent.

---

## R2-2 — Symmetric fork resolution has no fixed point without a deterministic tie-break; the plan defers the tie-break to H4, which cannot converge without it (CRITICAL)

**Violates:** G3 (≤2-round convergence, one tip). **Defends:** BRC §7.1, Phase 7, H4, Open Q2.

**The deferral.** Plan Phase 7: "tie-break rule decided from H4 data (README Open Q2 — likely
lower-`device_id` wins, decided after measurement, not before)." BRC §7.1: a device that detects a
fork "MUST re-read, re-apply, and re-write rather than continue on its own branch." The tie-break is
treated as a tuning constant to be *measured*.

**Break — it is a correctness prerequisite, not a constant.** The re-read-re-write rule is
**symmetric**: both devices run identical code. Walk the interleaving:

- Round 0: `A@seq11/parent=T10`, `B@seq11/parent=T10` — fork.
- Round 1: both detect the fork on their next read. §7.1 says each MUST abandon "its own branch" and
  re-write on top of what it read. A applies B's delta and writes `seq=12, parent=B11`; B applies
  A's delta and writes `seq=12, parent=A11`. **Two `seq=12` tokens, each child of a different
  `seq=11` token → a new fork.**
- Round 2: repeat. The protocol **ping-pongs forever**; every round manufactures the next fork. The
  only way out is a rule both devices compute *identically* to pick the **same** surviving branch
  (lower `device_id`, or lower `txid`) — i.e. exactly the tie-break the plan defers.

**Defeating the stated defense.** "Decide it after H4 measures" is circular: H4's PASS criterion is
"convergence to a single tip in ≤ 2 re-read/re-write rounds," and H4's "Number produced" is "decides
the tie-break rule (Q2)." H4 is expected to *discover* the rule by running a protocol that, without
the rule, **provably does not converge**. A randomized-timing run with no tie-break coded yields a red
H4 with no diagnostic pointing at "you need a total order on branches." The design hole is disguised
as a measurement task. Note the BRC's own Open Q7 already *asks* "is a deterministic tie-break
needed?" — the honest answer is yes, unconditionally, and it must be normative **before** Phase 7,
not an output of it.

**Why the harness misses it.** H4 as written assumes the tie-break exists (its PASS depends on
convergence). H9's two-device soak reuses H4's convergence assumption. Neither test has a case that
*starts* from a symmetric no-tie-break protocol and asserts non-convergence as the failure — so if
Phase 7 ships read-before-write but leaves Q2 "still measuring," H4 is simply flaky/red and the
system has no convergence guarantee in the field.

**Proposed fix.** Specify the tie-break as **normative now**: lower `device_id` wins; on equal
`device_id` (should be impossible) lower `txid`. Both writers and recovery use it. The losing branch's
writer re-applies onto the winner and continues; the winner does **not** move. This gives a fixed
point in one round. H4 then *measures* fork rate and resolution latency against a rule that actually
converges, instead of trying to invent the rule.

---

## R2-3 — Lost update: a snapshot built from a stale local baseline silently reverts another device's committed delta; recovery treats the revert as canonical (CRITICAL)

**Violates:** G3 (union survives), G4 (no silent data loss). **Defends:** §3.3 dirty flag, §7.1
read-before-write, H4.

**Starting state.** Snapshot `S10` holds output `O` with `basket="default"`. Device A and B share
seed. A's local "last-backed-up payload" (§3.3: "keep the last-backed-up payload locally") reflects
`S10`.

**Sequence.**

1. B changes `O.basket` to "collectibles", writes delta `D11 (upsert O)`, `parent=S10, seq=11`.
   Confirmed on chain.
2. A never applies `D11`. Two ways this happens even with Phase 7 built:
   (a) `adopt_onchain_backup` — the only existing read primitive — adopts outpoints and **does not
   apply payload** (verified `handlers.rs:13149-13228`); if Phase 7's "apply on read" has any gap
   for the *backup-write* path (as opposed to the user-spend path), A's local `O` stays "default";
   (b) A's next backup is triggered by the **3-hour periodic safety net** (kept, §3.3) or the dirty
   flag, **not** by a user spend — so the "poll immediately before any spend" hook (BRC §7.2) is not
   even on this code path.
3. A's producer accumulates enough change (or a schema bump / the 0.5x/20/16 KB rule) to trigger a
   **snapshot**, not a delta. A snapshot is A's **entire current state**, which still has
   `O.basket="default"`. A reads to tip (`D11`) and writes `S_next (snapshot), parent=D11, seq=12`.
   The chain stays **linear — no fork detected** (A did read to tip; parent is correct).
4. Recovery per BRC §6: "from the newest valid token follow parent back to the **most recent
   snapshot**, apply it, then deltas after it." The most recent snapshot is `S_next` — which has
   `O="default"`. `D11` is **before** `S_next` and is never replayed. **B's change is gone.**

**Break.** No row was lost in the set sense (`O` exists), no fork, no double-spend — every H4-style
check passes — yet a committed change from device B was silently reverted by A's snapshot. This is
the classic lost-update, and the snapshot mechanism (a full-state rewrite) is *structurally* a
last-writer-wins over the entire wallet whenever the writer's baseline predates an unapplied remote
delta.

**Defeating the stated defense.** §7.1 read-before-write is the intended shield: A should apply `D11`
before snapshotting. But (i) "apply" for a full snapshot means A must **merge B's row-level delta
into A's live DB** before serializing — a strictly harder operation than adopt, and unbuilt; (ii) even
if built, the merge is per-row upsert of B's `D11` onto A's DB, which only fixes `O` if A's applier
runs on *every* backup trigger including the periodic safety net, not only before user spends. The
plan nowhere states that the backup writer itself runs the full remote-merge before building a
snapshot; §3.3 wires the poll to spends, and D6/D7 to recovery/crash, but the **snapshot producer's
baseline freshness under multi-device is unspecified.**

**Why the harness misses it.** H4 scripts **deltas** ("both write within one delay window") and checks
row **presence**, not column **value** ("union of both devices' changes present" is a set test). H4
has no case where one device writes a **snapshot** whose baseline predates the other's delta. H3 is
single-producer. H7 builds linear chains itself, so its snapshots are never constructed from a stale
multi-device baseline. H9's end-of-run recovery-diff would catch a *net* divergence only if the
scripted op stream happens to leave `O` changed at end-of-run on one device and not the other — it
does not deterministically exercise "snapshot-after-missed-delta," so it is a coin-flip, not a
guarantee.

**Proposed fix.** Normatively require the snapshot producer to **read-to-tip and merge all unapplied
remote deltas into local state before serializing any snapshot**, and forbid a snapshot whose
`parent` is not the current tip. Add an H-test: B writes `D11`; A (baseline `S10`) is forced to
snapshot; assert the snapshot's `parent=D11` **and** that recovery preserves `O="collectibles"`.
Make it a value-level (per-column) diff, not a row-presence diff.

---

## R2-4 — The dirty flag is computed against a per-device local baseline, so it cannot detect chain divergence; H9's "MTTD ≤ one poll interval" rests on the poll, which is a separate, spend-only code path (HIGH)

**Violates:** G5 (failure visible), the H9 divergence-MTTD claim. **Defends:** §3.3, H9.

**Starting state.** `O` spendable on both devices. B spends `O` on device B, writes delta `D11
(delete O)`, confirmed.

**Sequence.**

1. A's poll interval has not fired, or A's poll (BRC §7.2) is bound to "before any spend/reserve" and
   A is not spending — A is idle-but-running. A's local DB still has `O` spendable.
2. A's **backup trigger** fires (3-hour safety net). §3.3: dirty iff (current collected state) diffs
   (A's **local** last-backed-up state). Both have `O` → **clean → no backup, and no signal.**
3. A's backup-health surface (G5: last-success, tip, failure count) reads **green**: last backup
   succeeded, no failures. Nothing in the dirty-flag path consults the chain tip. A is silently
   holding a spent output as spendable, and its own health says all-good.

**Break.** The dirty flag detects *local* change, never *remote* divergence — by construction it
compares against a local baseline. So the backup subsystem's health can be green while A's state has
diverged from the chain. The only thing that would catch it is the **poll**, which §3.3 wires to
spends and a timer — a *different* path from the dirty/health machinery, and one that (per R2-6) can
itself race.

**Defeating the stated defense.** H9 claims "divergence MTTD (must be ≤ one poll interval — three
silent months must be impossible)" (the E12 lesson). But MTTD ≤ poll-interval only holds if the poll
**and its divergence report are wired to the health surface**. The plan wires health to *backup*
outcomes (§3.3 backoff/last-success/failure-count) and the poll to *spend gating*; nowhere does it
say a poll that discovers a newer foreign tip **raises a health signal**. So a device can be diverged
for many poll intervals with green health, exactly the E12 silent-divergence class the plan claims to
have killed — just relocated from "no signal at all" to "signal exists but not on this path."

**Why the harness misses it.** H9 measures MTTD but the setup samples "a full recovery-diff run after
every op batch and at every simulated hour boundary" — recovery-diff is an **oracle the test harness
runs**, not the wallet's own health machinery. So H9 can report a small MTTD *for the test's own
oracle* while the **product's** health surface never fires. The pass condition "any silent failure
(health state green while a backup failed)" is scoped to *backup* failure, not to *divergence* — a
diverged-but-successfully-backing-up device is green and passes.

**Proposed fix.** Define divergence as a first-class health input: every poll that finds a foreign
tip newer than the local applied tip must update a `remote_tip / applied_tip` gap in the health
surface, and H9's silent-failure check must include "health green while local applied tip < chain
tip." Wire the poll and the dirty/health path to the same tip state.

---

## R2-5 — Ghost token + second device: the intent record carries the backup token but not the funding inputs, so restart-reconcile re-spends the funding UTXO after B has moved the tip past the ghost (CRITICAL)

**Violates:** G2 (money re-spent/lost), G3 (double-spend). **Defends:** D7 intent record + startup
reconcile, H5, H9.

**Starting state.** Chain tip snapshot `T10`. Device A holds funding UTXO `F` (spendable in A's DB).
D7 intent record (Phase 4) carries **"planned token outpoint, seq, parent txid, payload hash"** — I
verified the plan's wording (§ D7, Phase 4 acceptance); it does **not** list the funding inputs.

**Sequence (the E11 window, now across two devices).**

1. A builds backup `TA` spending `T10 + F`, producing token `T11 (seq=11)`. A **broadcasts** `TA`,
   then is hard-killed **before Step 12 DB write** (verified live window: broadcast at
   `handlers.rs:13957`, DB records after; Fix B intent record is still unbuilt). A's DB: `T10` still
   "tip," `F` still "spendable," no `T11`.
2. Device B polls, WoC index catches up, B sees `T11` unspent, `adopt_onchain_backup(T11)` — adopts
   outpoints only. B writes `TB` spending `T11 + F_b`, producing `T12 (seq=12, parent=T11)`. Chain
   tip is now `T12`; **`T11` is spent.**
3. A restarts. Startup reconcile consumes the intent record: "planned token `T11`." Reconcile queries
   the unspent index for the backup lineage, finds `T11` **spent** (by `TB`), walks/adopts the
   successor `T12` as the new parent. Backup lineage healed. **But the intent record never named
   `F`.** Reconcile has no record that `TA` consumed `F`, and A's DB still marks `F` spendable.
4. A's next user spend (or next backup funding selection) selects `F` → broadcasts a tx double-spending
   `F` (already consumed by `TA`) → ARC `DOUBLE_SPEND_ATTEMPTED`. This is precisely A3 E11's field
   failure ("adopt logic found the newer token but **funding selection still used the stale DB**"),
   made worse: A's own token `T11` is already gone, so "adopt the ghost as parent" lands on a *third*
   token and gives A even less reason to look at `F`.

**Break.** The plan's D7 claim — "read-before-write additionally heals ghost tokens structurally (a
ghost on chain becomes the parent the next write adopts)" — heals the **token lineage** and is silent
on the **funding input**. The A3 E11 root cause was the funding UTXO, not the token. An intent record
that omits the consumed funding outpoints cannot reconstruct which UTXOs `TA` spent, so it cannot mark
`F` spent. G2 breaks.

**Defeating the stated defense.** One could argue Fix A's `reconcile_spent_inputs` /
`reconcile_missing_inputs` (implemented, A3 §5) heals `F` independently of the intent record. But those
reconcile against **the chain by walking the wallet's own txs** — and A has **no DB record of `TA`**
(killed before Step 12), so there is no A-side tx to reconcile `F` from. The only durable evidence
that `F` was spent is `TA` on chain, which A can't associate with `F` without either its own DB row
(absent) or an intent record that lists `F` (absent by the plan's field list). The second device
having advanced the tip removes the last easy signal (`T11` unspent) that would have prompted A to
re-derive `TA`.

**Why the harness misses it.** H5 (crash matrix) is **single-device** ("one backup and one full
recovery"; the E11 replay is "kill after broadcast, restart with a stale DB" — one wallet). H4 is
two-device but injects **no crash**. H9 is the only place they combine, but its crash model is "the
H5 model" (single-device kill points) and its pass checks recovery-diff + no-double-spend at op-batch
boundaries — it does not script the specific interleaving "A ghosts a token, **B advances the tip past
it**, A restarts." Critically, no test asserts the **contents** of the intent record are sufficient to
reconstruct the consumed funding inputs; H5's pass is "no funds lost beyond ≤1 orphaned marker pair,"
which is about the token float, not the funding UTXO.

**Proposed fix.** The intent record MUST list **every input outpoint** `TA` reserves (funding
included), not just the planned token. Startup reconcile: for each intent whose token or any input is
spent on chain by a tx not in the local DB, **mark all listed inputs spent** and adopt the resulting
tx. Add a two-device H-test: A broadcasts+dies, B writes a backup consuming A's new token, A restarts,
assert A never selects `F` and the ARC double-spend never occurs.

---

## R2-6 — Poll-before-spend guards the backup chain, not the shared wallet UTXO set; two devices spend the same funding UTXO inside the debounce+latency+index-lag window, and the loser's DB is corrupted by the E4-Bug-C cascade (CRITICAL)

**Violates:** G3 (no accepted double-spend / silent divergence), G2 via cascade. **Defends:** §3.3
poll-before-spend, BRC §7.2, H4.

**The conflation.** The plan models concurrency as "two devices writing children of one parent = a
detectable fork" — a *backup-token* problem, always benign (detected, re-resolved). The real money
risk is two devices spending the **same wallet UTXO** for **user payments**. Both share the seed →
both derive the same addresses → both independently sync and see the same spendable set.

**Sequence.**

1. Output `U` (100k-sat received payment) spendable, known to both A and B.
2. A polls the **backup address** (BRC §7.2 poll target) → clean tip. B polls the backup address →
   clean tip. Inside the WoC 30 s–5 min unspent-index lag, neither poll can reflect the other's
   *pending* spend of `U` anyway.
3. A spends `U` → Alice. B spends `U` → Bob. Both broadcast. Network accepts one; the other gets
   `DOUBLE_SPEND_ATTEMPTED`.

**Break.** Poll-before-spend cannot prevent this. The poll reads the **backup chain**, but B's spend
of `U` is not on the backup chain yet: the backup is **debounced 3 min** (§3.3, kept) and written
*after* the spend, so there is an unavoidable window `(spend → debounce → build → broadcast → index
lag)` during which the other device has zero on-chain evidence that `U` is gone. BRC §7.2's
"apply any delta written by another device before spending" is only as fresh as the other device's
**last confirmed backup**, which structurally lags its actual spends. The mechanism the plan leans on
for G3 is aimed at the wrong ledger.

Worse, the losing device then hits **A3 E4 Bug C**: `TaskCheckForProofs` receives
`DOUBLE_SPEND_ATTEMPTED` and (in shipped behavior) treats it as terminal — deletes the losing tx's
outputs and **restores its inputs to spendable**, leaving the DB tracking phantoms. The loser then
writes a **backup from this corrupted state** (dirty flag fired: outputs changed), propagating the
corruption onto the chain and, via sync, potentially back to the winner.

**Defeating the stated defense.** "poll-before-spend is non-negotiable" (§3.3) and G3 "no accepted
double-spend" — note the network rejects one tx, so no double-spend is *accepted on chain*; the plan
could claim G3 holds. But G3 also promises "concurrent writes produce a *detected* fork, never silent
divergence," and this scenario produces **silent local divergence** on the loser (phantom outputs,
restored-spendable inputs) that is **not** a backup-chain fork and is **not** detected by fork logic
at all — it is detected, if ever, only by a later reconcile. And G2 (never lose spendable money) fails
through the cascade corrupting the loser's spendable set.

**Why the harness misses it.** H4 scripts "spend on A / poll on B; then both write within one delay
window" and checks "zero accepted double-spends" — but the *spend* there is oriented to backup-chain
convergence; H4 does **not** script both devices selecting the **same user UTXO for a user payment**
and then assert the loser's local DB survives the `DOUBLE_SPEND_ATTEMPTED` cascade intact. The mock
*can* inject "DOUBLE_SPEND_ATTEMPTED for both txs (E4 Bug C)" — but as a **single-device fault**, not
as the emergent product of a two-device shared-UTXO race, and no test asserts the post-cascade DB
equals the correct post-loss state (loser should mark `U` spent-by-winner, keep its own change absent,
and **not** restore inputs). H9's two-device soak scripts "op streams" per device but does not
guarantee a colliding same-UTXO selection, and its no-double-spend check is chain-level (the network
rejected one), so it reads green while the loser's DB is silently wrong.

**Proposed fix.** State explicitly that the backup chain **cannot** serialize user spends and that
concurrent same-UTXO spends across devices are possible; the design must either (a) accept it and make
the **loser's recovery from `DOUBLE_SPEND_ATTEMPTED` non-destructive** (mark spent-by-other, never
restore inputs on a first-seen conflict — the E4 Bug C fix must be load-bearing and tested per-device),
or (b) add a real cross-device spend-reservation (out of scope this sprint, but then G3's double-spend
promise must be narrowed in writing to "backup-token double-spend," not "wallet double-spend"). Add an
H-test: two devices select the same `U`, assert loser DB = correct post-loss state and its subsequent
backup does not carry phantoms.

---

## R2-7 — `seq` order and `parent_txid` order can disagree (stale/offline writer); recovery orders by `seq` but the chain links by parent — undefined which wins (HIGH)

**Violates:** G4 (silent data loss / wrong state), G3. **Defends:** BRC §5/§6, D6, §7.1.

**Starting state.** Device B offline a month. Chain advanced by A to `seq=50` (snapshot at 40).
B's last-seen `seq=10`.

**Sequence.**

1. B comes back. §7.1 requires read-to-tip before writing. But B's read can **fail or be skipped**:
   B's 3-hour safety-net backup can fire when B's indexer is momentarily down (the read is
   best-effort; on WoC error the write path today "trusts DB as-is," `handlers.rs:13440`). B then
   writes with the only parent it knows — a **stale** one — and a **stale `seq`** (e.g. `seq=11`,
   parent an ancient token).
2. Now the chain has a branch whose `seq` (11) is far **below** its chain position relative to
   `T50`. Recovery (BRC §6): "from the newest valid token follow `parent_txid` back to the most
   recent snapshot, apply deltas **in `seq` order**." The two orderings — causal (parent links) and
   numeric (`seq`) — **disagree**. The plan never says which is authoritative. If recovery trusts
   `seq`, it can apply B's `seq=11` delta *after* the `seq=40` snapshot's baseline in a position the
   parent chain never sanctioned, or *before* deltas it causally follows.

**Break.** `seq` "monotonic per wallet across devices" is an **assumption the design cannot enforce**
— there is no allocator (R2-1), and an offline/stale device violates it the moment its read fails.
Once `seq` is not guaranteed consistent with `parent_txid`, "apply in `seq` order" and "walk the
parent chain" are two different orders, and recovery has undefined behavior at the disagreement.
Silent wrong state on recovery (G4).

**Defeating the stated defense.** D6 makes the parent walk normative for **discovery**; §7.1 makes
read-before-write the guard. But D6's walk still hands recovery a *set* of tokens that §6 then orders
by `seq`; the plan does not resolve seq-vs-parent disagreement, and §7.1's read is best-effort (code
trusts DB on indexer error). So the guard has a hole exactly when it matters (offline/degraded read).

**Why the harness misses it.** H1/H7 build chains where `seq` is consistent with parent order by
construction. H4's forks are **short-timing** (both devices current, `seq=N+1` collisions per R2-1),
never a **month-stale** writer emitting a low `seq` deep under a high-`seq` tip. Nothing generates a
seq-inconsistent-with-parent chain and asserts a defined recovery.

**Proposed fix.** Make recovery order **purely the parent chain** (topological), using `seq` only as a
sanity/tiebreak assertion that must be monotonic along the walked path — and **reject/flag** any token
whose `seq` is not greater than its parent's (a stale write is then a detected anomaly, not silent
mis-ordering). Forbid writing on a stale parent: if read-to-tip fails, the write MUST abort (fail
closed), never proceed on a month-old parent. Add an H-test: inject a stale-`seq` token, assert
recovery order is parent-topological and the anomaly is reported.

---

## Summary of harness gaps this lens exposes

- **H4** checks row **presence** and post-convergence tips, not (i) recovery determinism across fetch
  order on an unconverged same-`seq` fork [R2-1], (ii) per-**column** value survival [R2-3], (iii)
  same-user-UTXO spend races and the loser's post-cascade DB [R2-6]. Its ≤2-round convergence PASS
  presupposes the tie-break it is supposed to output [R2-2].
- **H5** is single-device; the two-device ghost-token + tip-advance interleaving and the
  **intent-record-contents** sufficiency (funding inputs) are untested [R2-5].
- **H9** measures MTTD with the harness's own recovery-diff oracle, not the wallet's health surface,
  and its silent-failure check is scoped to backup failure, not divergence [R2-4]; its two-device
  crash model inherits H5's single-device kill points [R2-5].
- **Recovery ordering** (BRC §6 "in `seq` order") is untested against any chain where `seq`
  disagrees with `parent_txid` [R2-1, R2-7].

Two design decisions must move from "deferred to measurement" to "normative before Phase 7":
a **total order on branches / tokens** (deterministic tie-break) [R2-1, R2-2], and an **intent record
that lists all consumed inputs** [R2-5]. Two guarantees must be narrowed or re-mechanized in writing:
G3's "no double-spend" cannot cover **wallet-UTXO** double-spends via the backup poll [R2-6], and the
snapshot producer's **baseline-freshness under multi-device** must be specified or snapshots will
silently revert remote deltas [R2-3].
