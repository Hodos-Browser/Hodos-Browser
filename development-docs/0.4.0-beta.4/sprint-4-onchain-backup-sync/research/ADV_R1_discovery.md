# ADV_R1 — Adversarial review: Discovery & Indexer adversary (attacking D6 and everything that reads the chain)

**Lens:** R1. **Date:** 2026-08-24. **Target:** IMPLEMENTATION_PLAN.md D6 (parent-txid chain walk
normative, unspent-index bootstrap, recency check), read in full; cross-checked against A1 code map,
A3 retrospective (E4/E11/E13), the live code (`handlers.rs` backup/recovery region, `reconcile.rs`),
and DELTA_ANALYSIS.md.

**Method note:** every premise below was re-verified against the cited code lines, not taken from the
plan's relayed claims. Where the plan already defends (a D#/H#), I state the defense and then break it.

Seven findings. Two CRITICAL (silent stale restore; delta chain orphaned behind one unservable link),
four HIGH, one MEDIUM. The unifying theme: **D6's "cross-validated recency check" and the walk-only
discovery both rest on the exact same eventually-consistent WoC index that caused the 2026-04-11
incident, and the "second oracle" meant to break the tie (GorillaPool ordinals) does not index the
objects being checked.**

---

## R1-01 (CRITICAL) — D6's recency check is inert for backup outpoints: the "second oracle" (GorillaPool ordinals) does not index P2PKH markers or PushDrop tokens, so the cross-check collapses to WoC-alone and reproduces the 2026-04-11 stale restore

**Violates:** G5 (constraint 6 / D6 — "recovery fails closed on indexer errors; never restore a
superseded backup"), G2 (a lost device never loses spendable money → restoring a superseded snapshot
loses every output created after it), G1.

**Premises verified.**
- D6: before trusting the bootstrap tip, run `reconcile::check_outpoint_spent` (WoC + GorillaPool,
  fail closed) on the found marker; "if 'unspent' is actually spent, the index is stale: wait and
  retry, never restore."
- `reconcile.rs:63` — `GORILLAPOOL_BASE = "https://ordinals.gorillapool.io/api"`, endpoint
  `/txo/{txid}/{vout}/spend` (`reconcile.rs:213`). This is the **ordinals** indexer.
- `reconcile.rs:47-52` — the module's own comment: the GorillaPool response shape is "a Phase-1
  **assumption**"; "confirm the exact shape against the diverged dev wallet before c4 ships." It is
  unvalidated today.
- The object checked is either the 546-sat **P2PKH marker** (D6 text: "on the found marker") or, on
  the live write-path adopt at `handlers.rs:13483`, the 1000-sat **PushDrop** outpoint. Neither is a
  1-sat ordinal/inscription/BSV21 token. GorillaPool's ordinals txo index has no record of either.
- `reconcile.rs:106-133` (`decide_spent`): a provider with no record contributes `NoSignal`
  (`404`/empty/parse-fail all map to `NoSignal`, `reconcile.rs:200-206, 216-233`). WoC's `/spent`
  returns `404 -> NoSignal` for an unspent-or-not-yet-indexed outpoint (`reconcile.rs:139-146`).

**The break (starting state -> steps).**
1. Wallet has tip token M_new (spends M_old's PushDrop+marker as input[0]/[1]). A recovery is started
   on a fresh machine while WoC's address-unspent index is inside its 30 s-5 min lag window (A3 E4
   Bug A) and still serves **M_old's marker** as unspent.
2. Bootstrap (`fetch_onchain_backup`, `handlers.rs:14766-14771`) picks M_old as the tip.
3. D6's recency check runs `check_outpoint_spent(M_old_outpoint)`:
   - WoC `/tx/{txid}/{vout}/spent` — the spend of M_old by M_new is the *same block ingestion* the
     address index hasn't processed; WoC's spent index has not processed it either -> `404` ->
     `NoSignal`.
   - GorillaPool ordinals `/txo/.../spend` — M_old is a P2PKH marker (or PushDrop), not an ordinal ->
     no txo record -> `NoSignal`.
   - `decide_spent(NoSignal, NoSignal)` = **`Unknown`** (`reconcile.rs:120`).
4. D6's rule fires only on `Spent`. `Unknown` is not `Spent`, so the check does **not** flag
   staleness -> recovery restores M_old. Every output the wallet created after M_old (all of M_new's
   deltas, all receives since) is silently absent. This is exactly the incident the code's own TODO
   at `handlers.rs:14730-14733` warns about, now with a "recency check" that added no signal.

**Why the defense fails, precisely.** The recency check can only catch staleness when WoC's `/spent`
index is *fresher* than WoC's `/unspent` index for the same outpoint — but both are the same provider
ingesting the same block, and there is no contract that `/spent` leads `/unspent`. The independent
oracle that could break the tie (GorillaPool) does not track the object type at all, so for backup
outpoints the "two-oracle staleness detection" is one oracle wearing a second hat that is always blank.

**Harness gap (why H6/H7 as written miss it).**
- Section 5's mock chain implements only `address/{addr}/unspent/all`, address history,
  `tx/{txid}/hex`, the TSC proof endpoint, and ARC submit. It does **not** implement WoC
  `/tx/{vout}/spent` or GorillaPool `/txo/.../spend`. So H6's "stale unspent index pointing at a
  superseded token -> refuses to restore and retries (D6)" and H7's "recency check flags staleness
  (given the second oracle)" are tested against endpoints the mock does not model. Whatever stub the
  harness wires for the second oracle will, by construction, be built to *return a clean answer* — it
  will never reproduce "the second oracle has no record of this outpoint type," which is the
  production reality.
- H7's PASS is explicitly conditioned "**given the second oracle**" — the plan already senses the
  dependency but never checks that the second oracle covers markers. It does not.

**Proposed fix.** (a) Do not use the ordinals indexer as the recency oracle for non-ordinal backup
outpoints; the second oracle must be a UTXO/spent index that actually covers P2PKH+nonstandard outputs
(a second BSV node/indexer, per D12 — but wired *before* D6 is trusted, not "shadow mode later").
(b) Make recency a *positive* proof, not a spent-check: the restored token must carry a `seq`, and
recovery must confirm no higher `seq` exists at the address via **address history** (R1-02's fix),
failing closed on `Unknown`. (c) Add the two spent-check endpoints to the G12/H16 contract registry
and model their lag *correlated with* the unspent index in the mock, so "correlated lag defeats the
cross-check" is a first-class harness state.

---

## R1-02 (CRITICAL) — One unservable mid-chain link orphans everything behind it, including the nearest snapshot; the delta chain makes a partial walk unrecoverable while reporting a benign "break"

**Violates:** G1 (recovery = seed only -> full restore + spend), G2 (money older than the break
silently unrecoverable), G10 (chain robustness — the plan claims "last-contiguous-token restore" is
safe; under deltas it is not).

**Premises verified.**
- D6: discovery = bootstrap a recent marker via the **unspent** index, then **walk `parent_txid`
  backward by txid**. The unspent index shows only the tip (each backup spends the prior token —
  A1 section 3c, verified: `handlers.rs:13697-13704` input[0]/[1] = prev PushDrop+marker).
- DELTA_ANALYSIS.md:89-91 — recovery follows `parent_txid` backward "**to the most recent snapshot**,
  then apply the deltas forward"; "A break in the chain -> stop at the last contiguous token and flag."
- The walk is the *only* path back: bootstrap (unspent) sees only the tip; to get token T(k-1)'s raw
  bytes you must fetch it by the txid found inside T(k), and to get T(k-2) you must first decode
  T(k-1). A hole is not jumpable — you cannot learn T(k-2)'s txid without T(k-1).
- WoC 404s on valid txids are documented (A3 E1 `addf1a1` "dead txids that 404 on WoC"); large token
  txs are real (live PushDrop scripts 431 KB; a 991 KB cached tx — A1 section 4), so `tx/{txid}/hex`
  fetch timeouts/failures mid-walk are a live risk, and reorgs can transiently orphan a tip.

**The break (starting state -> steps).**
1. Chain over the wallet's life is: ... snapshot S (kind=0) . d1 . d2 . ... . d19 . **snapshot S'** .
   d20 . d21 . (tip). Money that only ever appeared in S (a token received once, long ago, never
   re-serialized into a later snapshot because strips aged its address row out of later payloads)
   lives structurally in S.
2. WoC 404s (or times out on a 431 KB body) for **S'** during recovery — one link.
3. Walk from tip: d21 -> d20 -> S' ... fetch of S' fails. "Stop at the last contiguous token and flag"
   (DELTA_ANALYSIS.md:91). The contiguous run from the tip is {d21, d20} — **deltas with no snapshot
   base**. There is nothing to apply them to.
4. Recovery either reconstructs an empty/garbage wallet or restores only what two deltas contain, and
   reports "partial recovery, chain break at S'." The user is told this is a handled break, not a loss.
   S (and everything in it) is **unreachable** — the walk cannot pass S' to reach S, even though S is
   perfectly servable.

**Why the plan's defense fails.** The old whole-DB design had every token self-sufficient: any one
unspent token fully restored the wallet, so a 404 on an *old* token cost nothing. The delta chain
*introduces* a single-point-of-failure: the reachability of the entire pre-break history now depends
on every intervening tx being individually servable. G10 ("recovery skips/reports ... last-contiguous
token restore") was written for junk/corruption at the *tip*, not for a hole *between the tip and the
nearest snapshot*, which is uniquely fatal under deltas.

**Harness gap.**
- H6 injects "missing parent tx (404)" and PASSES on "restore to the last contiguous token **and
  report the break**." It does **not** assert that the restored state is a coherent, spendable wallet
  — so a deltas-without-base result *passes* H6 while losing money. The FAIL column ("silent partial
  restore reported as success") is evaded because the break *is* reported; the defect is that the
  reported break hides unrecoverable funds behind a servable snapshot.
- H7 ("restore from every generation boundary") builds `snapshot0 + d1..dk + snapshot1 + ...` and
  simulates a lagging index whose newest visible token is t — it never injects a **404 on an interior
  token that sits between the visible tip and the nearest older snapshot**, which is the failure. H1
  chains are `1 snapshot + N deltas` (N <= 20) with no interior snapshot, so H1 never exercises "a
  needed snapshot is behind a hole" either.

**Proposed fix.** Bootstrap from the marker **address history** (all markers, spent + unspent), fetch
each token, decode its header, and **reassemble the chain by (`seq`, `parent_txid`)** — using the
sequential walk only as a fast path. Then a single unservable token costs only its own delta, the
reassembly can still reach an older snapshot around a hole, and "the newest snapshot is servable" is
sufficient for recovery. (DELTA_ANALYSIS.md:86 already assumes "walk the marker address history" — D6
narrowed this to unspent-bootstrap+walk and lost the robustness. Restore it.) Also: any recovery whose
reachable contiguous run contains **no snapshot** must fail closed, never report success.

---

## R1-03 (HIGH) — The recency check is three-valued but D6 branches on two; `Unknown` (the modal outcome for a healthy tip) is unhandled, forcing a dilemma that either restores stale or bricks healthy recovery

**Violates:** G5 (fail-closed/convergent), G1 (recovery must actually succeed on a good wallet).

**Premises verified.** `decide_spent` returns `Spent | Unspent | Unknown` (`reconcile.rs:70-101`). For
a **genuinely fresh, unspent** tip marker: WoC `/spent` -> `404 -> NoSignal`; GorillaPool (doesn't
track the marker — R1-01) -> `NoSignal`; `decide_spent(NoSignal, NoSignal) = Unknown`
(`reconcile.rs:120`). So `Unknown` is the *normal* result for a healthy tip, not an exceptional one.

**The break.** D6's prose only distinguishes "if 'unspent' is actually spent ... never restore" from
the implied else (restore). That is a two-way branch over a three-valued result. Two ways to implement
it, both lose:
- **`Unknown` -> restore** (treat "not proven spent" as OK): then the stale marker of R1-01, which
  also yields `Unknown`, is restored. Stale restore. G2/G5 violated.
- **`Unknown` -> refuse (fail closed)**: then *every* recovery of a healthy wallet fails, because a
  healthy tip yields `Unknown` (the marker is not an ordinal, so GorillaPool can never emit
  `ExplicitUnspent`, and WoC `/spent` on an unspent outpoint is always `404`). Recovery from seed is
  permanently blocked -> G1 violated; the wallet is effectively unrecoverable exactly when nothing is
  wrong.

D6 never says which, so this is an unresolved decision that determines whether the system loses money
or bricks. The plan cannot pick either as written; it needs R1-01's fix (an oracle that can emit a
positive Unspent for a marker, or a `seq`-max positive proof) before the `Unknown` branch is even
definable.

**Harness gap.** H6/H7 (per R1-01) cannot produce the real `Unknown` distribution because the mock
does not model the spent-check endpoints or the "second oracle doesn't cover this outpoint type"
state. A hand-stubbed oracle that returns clean `Spent`/`Unspent` hides that `Unknown` is the common
path, so neither horn of the dilemma surfaces in CI.

---

## R1-04 (HIGH) — D7's intent record does not defeat the E11 ghost during index lag: the visibility check the intent relies on is the same laggy unspent index, and the intent is consumed only at startup

**Violates:** G2, D7/constraint 8 (atomic-enough broadcast/record), constraint 6/7.

**Premises verified.**
- D7: persist a durable **intent record** (planned token outpoint, seq, parent txid, payload hash)
  before broadcast, "consumed by a **startup reconcile**"; read-before-write "heals ghost tokens
  structurally (a ghost on chain becomes the parent the next write adopts)."
- E11 (A3, VERIFIED): a backup broadcast succeeded but the process was killed before the DB write; the
  believed-spendable funding output was already consumed on-chain; funding selection used the stale DB
  -> permanent "Missing inputs" loop. Fix B (persist intent) was designed, never implemented.
- Ghost/heal detection depends on *seeing* the ghost on chain. The wallet's chain reads are the WoC
  unspent index (`fetch_onchain_backup`, adopt Step 5c) — the index with the 30 s-5 min lag.

**The break (two-intents realization).**
1. Backup builds fully-signed tx T (txid computable pre-broadcast), writes intent(T), broadcasts. Node
   accepts T. Process is killed before consuming the intent / writing DB records (E9/E11 window).
2. Restart. Startup reconcile reads intent(T) and asks the chain "is T on-chain?" via the unspent
   index — which is still inside its lag window and does **not** show T's marker yet. Reconcile
   concludes the broadcast failed.
3. Reconcile (or the next dirty trigger) does read-before-write: the unspent index still shows the
   *old* tip P (T's parent) -> read-before-write adopts P -> builds **T2** spending P's outpoint ->
   two children of P (T and T2) -> double-spend; whichever loses leaves an orphan, and if T already
   confirmed, T2 is rejected and the wallet re-enters a rollback/retry cycle. The intent record did
   not prevent this because the visibility check that would have vetoed T2 uses the same stale index.
4. **Running-process variant:** the intent is spec'd "consumed by a startup reconcile." A process that
   does *not* restart — the 3-hour periodic net or the next-receive trigger (section 3.3) — builds the
   next backup through the normal path, which the plan never says consults the intent table. So a
   ghost created mid-session is not healed until a restart; the normal trigger double-spends against it
   during the lag window.

**Why the D7 defense fails.** D7 makes the *record* durable but leaves the *decision* ("did my
broadcast land?") to the laggy index. Durability of intent is necessary but not sufficient; the heal
requires either (a) trusting the locally-computed txid of a tx you signed and broadcast (you *know* T
exists once ARC accepted it — see R1-07) rather than re-deriving existence from a lagging index, or
(b) a hard rule that no new backup may build while an unconsumed intent exists whose tx is not yet
*confirmed* (fail closed until the lag resolves), enforced on the **normal** trigger path, not only
at startup.

**Harness gap.** H5 injects hard-kills at every step boundary and "the E11 replay: kill after
broadcast, restart with a stale DB." But the crash axis and the **index-lag** fault axis (H6's model)
are separate in section 5; H5's setup does not require the mock's unspent index to still be lagging
*at restart time*. If the mock makes a broadcast tx immediately visible in `unspent/all` (the default,
absent an explicit lag injection), the restart reconcile sees the ghost and heals -> H5 passes, and
the production-fatal combination (crash-restart *while the discovery index still lags*) is never
constructed. Nor does any H assert the **non-startup** trigger path consults the intent table.

---

## R1-05 (HIGH) — Write-path stale adopt + orphan-marker sweep can consume the *real* tip under deltas, forking the chain onto a stale snapshot and destroying delta history

**Violates:** G2, G3 (no lost row / detected fork), constraint 6/7.

**Premises verified.**
- Write path Step 5c adopts the highest-height marker as primary; Step 5d sweeps *other* markers at
  the address (txid != adopted primary) as extra inputs, guarded only by "skip if already spent in DB"
  and "skip unconfirmed < 10 min old" (A1 section 1a steps 5c/5d; `handlers.rs:13407-13598`).
- Input order (verified `handlers.rs:13695-13716`): `[prev PushDrop][prev marker][extra
  markers][funding]`. "Extra markers" = the swept ones — they are *spent as inputs* by the new tx.
- D6's recency check is specified for **recovery discovery**, not for the write-path adopt/sweep. The
  write-path adopt has only its own single-outpoint spent-check (R1-01's inert oracle again).

**The break.**
1. Device made M_new (the true tip) recently; it is unconfirmed, and WoC's address index has not yet
   listed it. M_old is confirmed and still listed (lag window).
2. A backup triggers. Step 5c queries the address, sees only M_old's marker as unspent (M_new not yet
   indexed), adopts **M_old** as primary.
3. M_new confirms and appears in the index a moment later, *or* was already faintly visible; Step 5d
   now sees a marker whose txid != primary (M_old) — namely **M_new's marker** — and, once it is
   >10 min old, sweeps it as an "extra marker" input.
4. The new backup is built on parent M_old (stale snapshot) **and spends M_new's marker**, orphaning
   M_new's PushDrop and every delta chained on it. The chain now forks off the stale base; M_new's
   history is destroyed on-chain (its marker is consumed, its token superseded by a tx that never
   descended from it). A later recovery walking from the new tip reaches M_old, never M_new.

**Why undefended.** The plan hardened *recovery* discovery (D6) but inherited the write-path
adopt/sweep unchanged (section 3.3 keeps "pre-delete backup" and the c5b/orphan sweep). Under the old
whole-DB design a swept "orphan" marker carried no unique state (every token was a full snapshot), so
sweeping it was harmless dust cleanup. Under **deltas**, a non-primary unspent marker may be the live
tip carrying unique deltas — sweeping it is data loss, not cleanup. The sweep's safety invariant
("non-primary markers are orphans") is false in the delta world.

**Harness gap.** H10 (litter) sends *foreign* junk and asserts "chain walk unaffected" — it never
constructs a *self-produced* superseded-vs-tip race where the adopt picks the stale marker and the
sweep eats the real one. H5's crash matrix does not combine the sweep with an active lag state. H9's
soak injects faults but its recovery-diff cadence ("after every op batch / hourly") can miss a
tip-eaten-then-reforked window that self-heals its *balance* while having silently dropped a token's
delta lineage.

**Proposed fix.** Under deltas the orphan sweep must never consume a marker that is (or could be) a
chain tip: restrict sweeping to markers provably *behind* the adopted primary in `seq`, and apply
D6's recency/positive-`seq` proof to the **write-path adopt** too, not only recovery.

---

## R1-06 (HIGH) — Bootstrap picks a single marker by `max_by_key` and aborts on decode failure; a public backup address lets an attacker place one higher-ranked junk marker and block recovery

**Violates:** G1 (recovery from seed must succeed), G10 (junk at the address must be skipped).

**Premises verified.**
- `fetch_onchain_backup` (`handlers.rs:14766-14777`): `marker_utxo = utxos.iter().max_by_key(height,
  unconfirmed->i64::MAX)`; takes that **one** txid; on PushDrop/GCM/decode failure it returns `Err` —
  it does **not** iterate to the next-best marker.
- The backup address is BRC-42-derived from the seed but becomes **public on-chain** the moment the
  first backup is broadcast (the marker is a P2PKH to `pubkey_to_address(backup_pubkey)`, plainly
  visible). Anyone watching the chain can send P2PKH dust to it.
- D6 says only "find *a* recent marker via the address index (bootstrap)" — singular, no
  iterate-on-failure requirement.

**The break.**
1. Attacker observes victim's backup address on-chain (or the victim's own earlier failed cycle left
   orphan markers — A3 E2's 18 orphaned markers are a benign version of the same input).
2. Attacker sends one P2PKH output to the backup address, **unconfirmed** (height 0 -> ranked
   `i64::MAX`) or at a height above the real tip.
3. Victim recovers. Bootstrap's `max_by_key` selects the attacker's marker. Its tx has no valid
   PushDrop-vout-0 backup payload -> GCM/decode fails -> `fetch_onchain_backup` returns `Err` ->
   recovery aborts. While the attacker keeps a higher-ranked marker present, recovery is blocked — an
   unrecoverable-wallet DoS (fails closed, so no money moves, but G1 is unmet).

**Harness gap.** H10 sends "50 random **PushDrop tokens**" — but bootstrap queries
`address/{addr}/unspent/all`, which indexes by address, i.e. **P2PKH markers**; a nonstandard PushDrop
output locked to the backup *pubkey* is not the P2PKH-to-address the bootstrap enumerates, so H10's
litter may not even appear in the selection set. And H10 does not require any junk item to **outrank
the real tip** in `max_by_key`, nor does it assert the bootstrap **iterates past** a decode failure to
the next candidate. So H10 can pass while the single-pick, abort-on-failure bootstrap remains
exploitable. D6 does not close this because "find a recent marker" is under-specified.

**Proposed fix.** Bootstrap must enumerate *all* markers at the address (history, not just unspent —
this also serves R1-02), and for each, in `seq`/recency order, attempt decode; the first that
GCM-verifies is the chain entry point. Junk (GCM-fail) is skipped structurally, matching what H10
*claims* to test. Update H10 to send P2PKH markers that outrank the tip.

---

## R1-07 (MEDIUM) — ARC 200-with-different-txid (E4 Bug B) is an injectable mock fault bound to no assertion; nothing in the H-suite pins that the wallet records its locally-computed txid, not ARC's returned one

**Violates:** G12/constraint 7 (verify returned txid == submitted txid), G2 (a wrong recorded txid
poisons the intent record and the chain link).

**Premises verified.**
- Section 5 environment lists "ARC 200-with-different-txid (E4 Bug B)" as a scriptable fault. No
  H-test row (H1-H16) has a PASS/FAIL clause asserting the wallet validates the ARC-returned txid
  against the tx it signed. H5 is the crash matrix; H6 is read-path corruption; H8 is cost. None cover
  broadcast-response txid validation.
- A3 E4 Bug B (VERIFIED): ARC returns `status:200, txStatus:SEEN_ON_NETWORK, txid:<existing>` when
  *rejecting* a conflicting tx; the handler read `200` as success and recorded the returned txid. The
  live guard is `handlers.rs:8905-8934` (txid-collision detection) — but that is a specific patch, not
  a general "trust only the local txid" invariant, and it is not exercised by any planned test.

**The break.** Under D7 the intent records a "planned token outpoint" = the txid of the tx the wallet
built and signed (deterministic pre-broadcast). If, on broadcast, ARC returns a *different* txid and
any code path records ARC's txid (as Bug B did) instead of the local one, then: the intent's planned
outpoint and the DB's recorded outpoint disagree; the next backup's `parent_txid` link and the
recovery walk key off a txid that may not be the wallet's tx at all; and the startup reconcile compares
intent(local txid) against a DB row (ARC txid) and cannot match -> either a false "failed" (->
R1-04 double-spend) or a false "succeeded" pointing at a stranger's tx. The whole delta chain's
integrity rests on the recorded txid being the wallet's own signed tx, and no test guarantees it.

**Harness gap.** The fault is injectable but **unbound** — a mock capability with no oracle. Add an
explicit assertion (attach to H5 or a new row): after a broadcast where ARC returns a mismatched txid,
the wallet MUST (a) treat the local signed-tx txid as authoritative, (b) never write ARC's txid into
`transactions`/intent/parent-link, and (c) surface the mismatch as a failure, not a success. This
structurally kills Bug B under D7 but is currently only assumed.

---

## Summary table

| ID | Sev | One-line | Violates | Harness gap |
|---|---|---|---|---|
| R1-01 | CRITICAL | GorillaPool ordinals doesn't index backup markers -> recency check collapses to WoC-alone -> stale restore | G5(D6/c6), G2, G1 | Section 5 mock omits `/spent` + `/txo/spend`; H6/H7 stub a clean 2nd oracle that can't reproduce "no record for this outpoint type" |
| R1-02 | CRITICAL | One unservable mid-chain token orphans the nearest snapshot behind it; deltas-without-base unrecoverable, reported as benign "break" | G1, G2, G10 | H6 passes on "report the break" without asserting a coherent wallet; H7 never 404s an interior token between tip and older snapshot |
| R1-03 | HIGH | Recency check is 3-valued; D6 branches on 2; `Unknown` (modal for a healthy tip) unhandled -> stale-restore or brick | G5, G1 | mock can't produce the real `Unknown` distribution |
| R1-04 | HIGH | D7 intent defeated by index lag at reconcile; existence-check uses the laggy index; intent consumed only at startup | G2, D7/c8 | H5 crash axis and H6 lag axis separate; lag not required active at restart; no test that the normal trigger consults intent |
| R1-05 | HIGH | Write-path stale adopt + orphan sweep consumes the real tip under deltas -> chain forks onto stale snapshot | G2, G3, c6/c7 | H10 uses foreign junk, not a self-produced superseded-vs-tip race; sweep+lag never combined |
| R1-06 | HIGH | Single-pick `max_by_key` bootstrap aborts on decode fail; public backup address -> one higher-ranked junk marker blocks recovery | G1, G10 | H10 sends PushDrop *tokens* not P2PKH *markers*; doesn't require junk to outrank tip or assert iterate-past-junk |
| R1-07 | MEDIUM | ARC 200-with-different-txid injectable but bound to no assertion; nothing pins "record local txid, not ARC's" | G12/c7, G2 | fault has no oracle in any H row |

**Cross-cutting recommendation.** Three of these (01, 03, 04) and half of 05 are the same root defect
the retrospective named M5 ("external oracles trusted beyond their contract") — D6 layered a "recency
check" on top of the same WoC index without a genuinely independent, *outpoint-appropriate* second
oracle, and without modeling correlated lag. The mock must treat the spent-check endpoints as
first-class contracted endpoints (G12/H16) with a **correlated-lag** fault mode, and the discovery
design should move from unspent-bootstrap+walk to **history-reassembly by (`seq`,`parent_txid`)**,
which simultaneously closes R1-02 and R1-06 and makes a positive `seq`-max recency proof possible for
R1-01/R1-03.
