# Adversarial Review — On-chain Backup and Sync Implementation Plan

**Date:** 2026-08-24. **Ordered by:** owner (Matt), after noting the plan "seems good on the
surface". **Method:** five adversarial lenses (R1 discovery/indexer, R2 multi-device, R3 crypto,
R4 state machine, R5 harness adequacy) produced 39 findings; an independent skeptic pass verified
every premise against code, live endpoints, and plan text, refuting 3 and confirming 36.
**Inputs:** `research/ADV_R1_discovery.md` … `ADV_R5_harness.md`, the skeptic verdicts (source of
record for severity), the plan at rev 2026-08-23, code at `rust-wallet/src` (`handlers.rs`,
`backup.rs`, `monitor/`, `reconcile.rs`), and two live probes (GorillaPool ordinals spend endpoint
vs plain P2PKH; WoC mined-tx status of the stuck reqs).

**Bottom line:** 7 CRITICAL, 18 HIGH, 9 MEDIUM, 2 LOW findings survive skeptical verification.
None invalidates the plan's architecture (delta chain + parent-walk + harness-first ordering all
survive); nearly all are **specification holes the harness as written would not catch** — the
exact failure mode of every prior incident. Every surviving finding now has a defending edit in
IMPLEMENTATION_PLAN.md, marked *(adversarial review 2026-08-24)*. The KDF/address-migration
question (R3) is resolved into a decision-ready recommendation for the owner in §4 below —
recommendation only; the decision block in D4/D5 is marked PENDING OWNER DECISION.

---

## 1. Surviving findings (CONFIRMED), ranked

Each entry: scenario → violated guarantee → why the harness as written misses it → the plan change
that answers it (all changes are in IMPLEMENTATION_PLAN.md, marked *(adversarial review
2026-08-24)*).

### CRITICAL

**R4-2 — H14's mandated age-out of never-broadcast noSend txs re-creates Bug C.**
A counterparty holds a signed noSend tx spending our output O_A and has not broadcast it. No chain
or mempool oracle can attest more than "not visible now" — which is the truth for every
counterparty-held tx — so the three-oracle quorum is unanimous and H14's PASS clause *requires*
the age-out to fire. Shipped age-out semantics (TaskFailAbandoned) delete outputs and restore
inputs; O_A returns to spendable, the wallet spends it, the counterparty later broadcasts —
conflict manufactured by our own reconciliation, and the tombstone replays on every future
recovery (violates G1/G2). H14 grades this run green by construction.
→ **Fix:** new **D14**: age-out of a noSend req moves it to a new `abandoned-unbroadcast` state —
raw bytes leave the payload, but **inputs stay encumbered and outputs are never deleted** on any
timer/quorum-of-absence signal; only a chain-visible conflicting spend releases them. H14
PASS/FAIL rewritten to assert exactly this.

**R1-01 — D6's recency cross-check is inert for backup outpoints.**
Live-probed 2026-08-24: GorillaPool's ordinals `/txo/{txid}/{vout}/spend` returns 404 for plain
P2PKH outpoints → `NoSignal`; WoC `/spent` 404s for unspent-or-unindexed → `NoSignal`;
`decide_spent(NoSignal, NoSignal) = Unknown`. The "cross-validated" spent-check adds zero signal
in exactly the stale-index window it exists for; the 2026-04-11 stale-restore class reproduces
with the defense in place (violates G5 fail-closed, G10). The §5 mock omits both spent endpoints,
so H6/H7 test a stub.
→ **Fix:** D6 rewritten — recency is decided from the **full address history** (child-existence
check: a marker is stale iff any decryptable marker names it as parent), not from spent-status
oracles; D12 gains a hard selection requirement: the secondary provider **must index plain-P2PKH
spent status** or it cannot serve D6; the mock implements the real spent endpoints with their real
404 semantics.

**R1-02 — One unservable mid-chain link orphans the nearest snapshot behind it.**
Chain T0+d1..d20+T1+d21..; the indexer cannot serve d23's tx (pruned/404). The D6 walk is
parent-only: T(k−2)'s txid lives inside T(k−1), so a hole is unjumpable; "stop at the last
contiguous token and flag" leaves deltas-without-base or a weeks-old snapshot as the restore
(violates G1/G4). H6's FAIL clause ("silent partial restore reported as success") is evaded
because the break *is* reported — then the user writes on the partial state and the loss becomes
permanent.
→ **Fix:** D6 rewritten — bootstrap enumerates **all** markers via address history (spent +
unspent), so the walk can reassemble across a hole by txid-set lookup; **deltas-without-base is a
fail-closed typed error, never a restore offer**; H6 gains the mid-chain-hole case with exactly
that assertion.

**R2-3 — A snapshot from a stale baseline silently reverts another device's committed delta.**
B commits delta d (chain tip advances); A polls but `adopt_onchain_backup` adopts outpoints only —
it never decrypts or applies payloads (verified handlers.rs:13149-13228). A's snapshot cadence
fires; A serializes **its local DB** (missing d's row change) with parent = tip. The chain is
linear and valid — no fork for H4 to detect — and recovery replays nothing before the newest
snapshot: B's committed change is gone (violates G3 "no lost row", G4). H4's union check is
row-presence over scripted collisions; it never scripts snapshot-after-missed-delta.
→ **Fix:** new **D13**: the snapshot producer MUST read-to-tip and **merge all unapplied remote
deltas before serializing**; a snapshot whose parent is not the device's fully-applied tip is a
build error. H4 gains the snapshot-after-missed-delta script with a field-level (not
row-presence) union assertion.

**R4-1 — Ambiguous broadcast outcome re-arms the 2026-04-11 double-spend.**
ARC accepts T1 but the HTTP response is lost (timeout/reset). Code takes the plain rollback path —
all reserved inputs restored (handlers.rs:13954-13996) — and the plan says nothing about the
intent record on the Err path. Next cycle builds T2 from the same parent: same-parent conflict,
the exact E4 mechanism. The §5 fault list has kills and rejections but **no
accepted-with-lost-response**; H5 injects only kills. The shipped after-the-fact heals
(TaskVerifyDoubleSpend, c5b) act post-conflict, and Phase 4's rewrite could drop them.
→ **Fix:** D7 extended — broadcast outcome is **three-valued: OK / REJECTED(reason) / UNKNOWN**;
on UNKNOWN: no rollback, intent record retained, **re-broadcast the cached identical raw tx**
(idempotent — same txid annihilates the conflict) until the chain renders a verdict. The mock
gains the accepted-response-lost fault; H5 gains the row.

**R5-1 — WoC rate-limit mid-recovery is swallowed; recovery reports success and retry is wedged.**
Recovery re-fetches hundreds of stripped scripts in a tight loop against live WoC; 429s arrive;
each refetch failure is `log::warn` + counter, the loop continues, recovery reports success (H1's
own escape hatch "or each failure explicitly surfaced" blesses it). Token outputs are unspendable
(violates G1) and the fresh-wallet-only Conflict check (handlers.rs:15008-15017) blocks any retry
over the partial wallet.
→ **Fix:** H1's escape hatch deleted — any refetch failure = typed recovery FAILURE; Phase 2 adds
a **resumable recovery** path (retry completes refetch on an existing partial wallet; the Conflict
check distinguishes "partial recovery in progress" from "occupied wallet"); the mock gains a
first-class 429/Retry-After mid-bulk fault; refetch gains backoff.

**R3-1 — Any future address migration creates a silent-downgrade recovery surface.** *(conditional
— fires only if the D5 migration ever ships)*
Post-migration, recovery must check two addresses. Indexer lag or litter at the v2 address yields
"no valid v2 token found"; the legacy chain's final token authenticates perfectly; recovery
silently restores pre-migration state — post-migration funds gone (violates G1/G10). H15 pins
decrypt-compat only; no test covers two live addresses under lag.
→ **Fix:** feeds the §4 recommendation (reject migration absent a forcing break); the D5 PENDING
block records the binding rule for the forced case: **v2-aware recovery MUST fail closed when the
v2 address yields nothing but a legacy chain exists** — never silently restore legacy.

### HIGH

**R4-3 — The step-13 post-broadcast re-baseline absorbs broadcast-window mutations (live shipped
bug).** Payload built at t0; broadcast takes seconds; a receive lands at t0+2s; step 13
re-collects at ref_ts=now and stores *that* hash as the baseline (handlers.rs:14077-14094). The
receive is inside the baseline but was never broadcast; the next collect matches the baseline →
`Skipped` → health green forever on an idle wallet (violates G2, G5). §3.3's "keep the
last-backed-up payload locally" never pins the baseline instant, so Phase 5 inherits it. H3 is
single-threaded.
→ **Fix:** one sentence in §3.3 + D13: **the baseline is the exact collected state that was
broadcast**, captured at build time, never re-collected; H3 gains a mutation-during-broadcast
case.

**R3-2 — The plaintext header is trusted before GCM in the D6 walk.** The Phase-4 header (`seq`,
`parent_txid`, `kind`) is plaintext PushDrop fields; D6 runs bootstrap selection + recency on the
found marker *before* decryption. A keyless attacker mints a max-seq unconfirmed marker whose
parent_txid points at a real stale snapshot: recovery either walks to stale state or reports "no
backup" (violates G10, G5). H10's junk is random, not targeted.
→ **Fix:** D6 normative rule: **no plaintext header field may influence tip selection, recency,
or walk order before the token's GCM tag verifies under our key** (candidate iteration =
newest-that-decrypts); H10 gains the crafted-header litter case.

**R1-03 — The recency check is three-valued; D6 branches on two.** A healthy unspent tip yields
`Unknown` by construction (both oracles NoSignal — see R1-01). Unknown→restore reproduces the
stale restore; Unknown→refuse bricks every healthy recovery. The plan text never says which.
→ **Fix:** D6 rewritten with an explicit three-branch decision table; `Unknown` resolves via the
history-based child-existence check, not spent-status; H7 asserts each branch.

**R1-05 — Write-path stale adopt + orphan-marker sweep can consume the real tip.** Shipped 5c/5d
(handlers.rs:13560-13600): adopt max-height marker over the lagging unspent list, sweep every
non-primary marker as input. Under deltas a "non-primary" marker can be another device's true
tip; sweeping it consumes the tip and forks onto a stale base (violates G3). No H-test constructs
the superseded-vs-tip race.
→ **Fix:** Phase 5 work item + new **H19**: the sweep may never consume a marker that decrypts
under our key and carries seq ≥ the local tip; sweep only markers proven superseded by chain
linkage.

**R1-06 — Bootstrap single-pick aborts on decode failure; one dust output blocks recovery.**
`fetch_onchain_backup` picks exactly one marker via `max_by_key` (unconfirmed → i64::MAX) and any
decode failure returns Err with no next-candidate iteration (handlers.rs:14765-14815). The
address is public P2PKH: one attacker output = recovery DoS (violates G1, G10). DELTA_ANALYSIS's
own "newest token that decrypts" rule was dropped by D6.
→ **Fix:** D6 rewritten — enumerate all candidates, iterate newest-that-decrypts; H10 gains the
outranking-dust case.

**R4-4 — D7's startup reconcile cannot distinguish crash-before from crash-after broadcast in the
lag window.** Intent present + txid not visible = both "never sent" and "sent, not yet indexed".
One sentence where a decision table is required; H5 never crosses kill points with an active lag
state; double-crash multi-intent is unspecified.
→ **Fix:** D7 extended with the reconcile **decision table** (intent × visibility × parent state
× cached-raw-tx) whose safe default is re-broadcast-cached-tx (idempotent), plus a multiplicity
rule; H5 kill matrix crossed with index-lag active + a double-crash row.

**R1-04 — The intent record is consulted only at startup; the normal trigger path never reads
it.** The 3-hour/receive triggers run against the same 30s–5min-lagging index; a ghost's
existence check misses it and the next write manufactures a same-parent conflict (violates G6).
→ **Fix:** D7 extended: the reconcile decision table runs **on every backup trigger path**, not
only startup; H5 asserts it on a non-startup trigger.

**R4-5 — Reorg after the 1-conf raw_tx strip.** The proven-gated strip drops raw_tx at proof
existence = 1 conf (backup.rs:449-457); the plan contains zero occurrences of "reorg". A 1–2
block reorg leaves the newest payload without bytes that may be unfetchable (the counterparty may
not rebroadcast) (violates G1/G8). No un-mine operation exists in the mock.
→ **Fix:** D14: strip raw_tx only at a **declared confirmation depth** (constant in the BRC;
default ≥ 6); proven→unproven transition defined; mock gains un-mine/reorg; new **H17** reorg
suite.

**R4-6 — Terminal-token mempool eviction on an idle wallet: health green forever.** Tip
broadcast, never mined, evicted ~14 days later; the wallet is idle so no next write fails; health
is broadcast-derived only; D6's recency check *truthfully* blesses N−1 (chain-true staleness, not
an index race) (violates G2, G5). MTTD = ∞ on the metric H9 promises to bound.
→ **Fix:** Phase 2 health surface gains **tip confirmation state** (tip unconfirmed past a
horizon = red + rebroadcast); mock gains a simulated-time eviction horizon; H9 asserts it.

**R4-7 — The delta chain immortalizes timer-only destructive writes.** TaskFailAbandoned et al.
delete outputs / restore inputs on timers alone; the Phase 5 producer trusts the DB, so a buggy
monitor deletion becomes a permanent, replicated protocol event; a delete/resurrect ping-pong
grades healthy in every H (each round is a genuine DB transition).
→ **Fix:** D14: a **delete-emission allowlist** in the Phase 5 producer — only named,
chain-corroborated state transitions may emit delete records; timer-only transitions may never
emit deletes; H3 asserts no delete record without an allowlisted cause.

**R5-7 — H9's accelerated clock skips mempool eviction and price-cache staleness; the E11 class
is unsampled.** Zero "evict" in the mock; the ~14-real-day horizon that produced
believed-spendable-but-consumed funding never occurs in any soak.
→ **Fix:** the same mock eviction horizon as R4-6, tied to simulated time so soaks cross it; a
price-cache-empty window fault added; H9 setup lists both.

**R2-4 — The dirty flag cannot see chain divergence, and divergence is not a health input.** A
device diverged from the chain diffs against its own baseline, stays clean and green — the
relocated E12 class (three silent months) (violates G5). H9's MTTD is measured by the harness's
own oracle, not any product surface.
→ **Fix:** Phase 2/7: **divergence is a first-class health input** — any poll finding a foreign
tip that is not an ancestor/descendant of the local tip sets a user-visible flag; H9's MTTD is
measured off the product health surface.

**R2-5 — The intent record omits funding inputs.** D7's field list (token outpoint, seq, parent,
payload hash) does not cover the reserved funding UTXO F; a crashed device's reconcile can
re-reserve or re-spend F while a ghost holds it (damage bounded by network rejection + c5b, but
silent wrong state until a failed broadcast).
→ **Fix:** D7 extended: the intent record lists **every input outpoint the backup tx reserves**;
H5 asserts intent-record sufficiency under the two-device ghost interleaving.

**R2-6 — Two seed-sharing devices can double-spend the same user UTXO; G3 promises the wrong
ledger.** The backup chain cannot serialize user spends (debounce + build + lag ≫ poll), so
same-U selection is constructible; the loser's post-cascade DB state is untested. (The cascade
itself is defended: TaskVerifyDoubleSpend verifies before destructive action.)
→ **Fix:** G3 reworded to scope its promise to backup/sync state and *detection + bounded heal*
for wallet-UTXO races; D15 records that the chain is not a spend serializer; H4 gains loser-state
assertions; the verify-quorum's 6-hour promote-to-confirmed-regardless timer is removed (quorum
required — D14).

**R5-2 — The canonical form excludes the columns where divergence hides.** H2's FAIL covers only
"travels" columns; H3/H1 subtract excluded/re-derived columns before comparing, so a wrong
re-derivation (the shipped `outputs.confirmed=1` bug; the merkle_path binary-vs-TSC-JSON split)
is invisible to every planned test.
→ **Fix:** H2 classification split: excluded-from-equivalence ≠ exempt-from-correctness; every
re-derived/re-fetched column gets a **value-correctness oracle** in H1; the exclusion list is
audited with a reason per column.

**R5-4 — H15 pins decrypt-compat only; an old-schema JSON through current import is untested.**
The JSON inside the envelope evolves with the DB schema (V9→V24+) independently of the version
byte; the E5 class returns one generation later (violates G11 "recoverable", not just
"decryptable").
→ **Fix:** H15 gains an append-only corpus of **decrypted-payload JSON fixtures, one per
DB-schema generation ever broadcast**, each run through current import to a spendable wallet.

**R5-5 — Constraint 14 (E12, the 12.5M-sat class) has zero regression coverage.** §4.2 row 14
admits the per-wallet-identity half is open with no test; the harness has no credential-store
model.
→ **Fix:** minimum observability now: **key-identity mismatch** (backup address derived from a
key that does not match the loaded wallet identity) is a health assertion in H9; the full fix
stays an open item but is no longer silent.

**R5-6 — No tier ever runs seed-only recovery against the real network.** The core promise is
proven only against a mock generated from the team's own endpoint beliefs — A3's thesis is that
every incident lived in exactly that gap; §7 admits no field recovery has ever been recorded.
→ **Fix:** new **H18** live-recovery smoke (dev wallet, real chain, seed-only restore-and-spend,
per release); §5 gains the meta-gap statement verbatim.

### MEDIUM

**R4-8 — No terminal state for permanent oracle disagreement.** A req in limbo carries
431KB-class bytes in every payload forever; H14 passes that run (violates G7).
→ **Fix:** D14 limbo policy: after N cycles of quorum disagreement, raw bytes move to local-only
retention + health flag; the payload carries a stub; H14 asserts the bound.

**R4-9 — The LWW arbiter is excluded from the payload.** D2 adopts per-row LWW while §3.3
excludes `updated_at`; same-row concurrent edits resolve by re-write race; H4's row-presence PASS
cannot see the lost field.
→ **Fix:** D2 extended: deltas carry a **per-row logical version counter** as the merge arbiter
(`updated_at` stays excluded); H4 gains a same-row concurrent-edit case with a field-level
assertion.

**R4-10 — The DB-ahead-of-chain ghost has no defined heal.** D7 heals only ghost-on-chain; the
inverse (recorded tip absent from chain after eviction) wedges the walk or skews the baseline.
→ **Fix:** D7 extended: recorded-parent-absent → **re-broadcast the cached raw tx first**; only
if chain-rejected, rebuild from the last chain-visible ancestor; H17 covers it (reorg/eviction
suite).

**R4-11 — Snapshot atomicity is an undocumented accident of the single-mutex topology.** The
services facade will change the topology; H3 never interleaves ops with an in-flight produce.
→ **Fix:** D13 states the invariant (the producer reads its entire input from a single consistent
DB snapshot); H3 gains a concurrent-produce interleave case.

**R5-3 — H4 never samples the WoC lag tail.** Q2 is decided from one fixed propagation delay.
(Downgraded: stale-parent writes are network-rejected, so tail failures are loud.)
→ **Fix:** H4 draws lag per round from the H16-contracted distribution incl. the 30s–5min tail,
plus k-rounds-stale poll cases.

**R5-8 — The secondary oracle is unchosen; correlated lag and mid-recovery flips are
unrepresentable in the mock.**
→ **Fix:** D12 extended: the candidate must pass the P2PKH-spent-indexing requirement (R1-01);
the mock gains correlated-lag and answer-flip fault models; the H16 promotion gate references
both.

**R5-9 — Constraint 11 is vacuous on the crash case: nothing kills mid-import.** A half-import
wedges the fresh-wallet-only retry.
→ **Fix:** H5 kill matrix extended to the **import path**; resumable recovery (the R5-1 fix)
makes the retry well-defined; H1 asserts recovery-after-killed-recovery.

**R1-07 — ARC 200-with-different-txid is a mock fault bound to no assertion.**
→ **Fix:** an H5 row binds it: the wallet records its **locally computed txid**; chain link and
intent matching are unaffected by ARC's response body.

**R3-3 — Size-class padding defends a weaker observer than the draft claims.** Backup txs spend
wallet UTXOs and return change (address linkable); §3.3 makes broadcasts ~1:1 with wallet events;
H12 never joins timing to wallet activity.
→ **Fix:** claim downgraded in the BRC (Appendix A item): padding hides payload size/type from a
same-address observer; it does not hide timing correlation or address linkage — stated plainly.

### LOW (kept on record; no action)

**R3-4 — The GCM nonce-collision and key-commitment analyses check out.** q²/2⁹⁷ bound fine;
Invisible Salamanders needs a second-key oracle the victim never provides. On record so nobody
re-opens it.

**R3-5 — The backup key derives from the raw BIP32 root, not a hardened child.** Hygiene debt
shared by both KDF options, not exploitable (one-way primitives), **not fixed by the proposed
migration** — recorded as out-of-scope-unless-forced so it cannot be used to re-justify the
migration R3-1 kills.

---

## 2. DEFENDED-ALREADY (attacks the plan/code survived — evidence the plan holds)

- **UTXO linkage is a real serializer for the backup chain itself:** every backup tx spends its
  parent's token outpoints, so two same-parent tokens are conflicting txs and at most one lands —
  this single structural fact refuted R2-1, R2-2, R2-7 and downgraded R5-3.
- **TaskVerifyDoubleSpend (the Bug C fix) holds:** it verifies each suspicion independently
  against the chain before any destructive action — it defeated R2-6's claimed cascade (finding
  downgraded from CRITICAL).
- **c5b `reconcile_missing_inputs` + network rejection bound the ghost-funding damage:** R2-5's
  money-loss claim collapsed to silent-wrong-state (downgraded from CRITICAL).
- **The shipped txid-collision guard** (record local txid) is present at the send sites — R1-07
  is a missing *test*, not a missing defense.
- **GCM-tag-as-origin-proof survives cryptanalysis** (R3-4): nonce bounds and the key-commitment
  attack surface both checked and dismissed.
- **Self-ECDH degeneracy for the BRC-42 backup address is sound** (R3 worked check): failure
  modes are ~2⁻¹²⁸ and fail loud, never a silent weak key.
- **Atomic single-transaction import (constraint 11) is real** on the content path — every H6
  content fault fails before the import transaction (the surviving crack, R5-9, is the kill case
  only).
- **The harness-first phase ordering survived all five lenses** — no finding argued for shipping
  a format change before H1/H2; every finding sharpened the harness instead.

## 3. REFUTED (attack constructed, then broken by the skeptic)

- **R2-1** (seq-collision nondeterministic recovery): "both confirm" is structurally impossible —
  same-parent tokens conflict on the parent's outpoints; at most one lands. Residual adopted:
  dual-genesis sentence in Phase 7 (D15).
- **R2-2** (fork resolution provably non-convergent): the chain itself is the deterministic
  arbiter — the losing branch's re-write is network-rejected; a fixed point exists without any
  device tie-break; H4-red also gates Phase 7. Salvaged kernel adopted: the tie-break is named
  normatively now (D15) and validated (not decided) by H4.
- **R2-7** (seq/parent order disagreement poisons recovery): the stale writer's tx spends
  long-spent outpoints and is rejected; the poisoned chain is not constructible. Residual adopted
  as hygiene: recovery order is parent-topological, seq = parent.seq+1, seq is a sanity check
  only (D15).

## 4. Crypto verdict (R3) + skeptic concurrence — DECISION-READY RECOMMENDATION

**Question (owner):** should we migrate the payload KDF to the draft's BRC-42 scheme and/or the
backup address derivation to the BRC-43-conformant string?

**R3 verdict: KEEP SHIPPED, for both. Skeptic: CONCUR, premises independently re-verified.**

- The shipped KDF — `SHA-256(master_privkey ‖ "hodos-wallet-backup-v1")` over the raw BIP32 root
  (backup.rs:958-967, helpers.rs:14-32) — and the draft's BRC-42 self-derivation are **equally
  sound**: BRC-42 with counterparty = self produces a shared secret that is a deterministic
  function of the master key alone; it adds no entropy, no second party, no security over a
  domain-separated SHA-256 of the same 256-bit secret. Migration between them is pure churn.
- The thing migration would "fix" — BRC-43 non-conformance of `"1-wallet-backup-1"` — is
  **cosmetic**: BRC-42 HMACs the literal invoice bytes without parsing BRC-43 structure.
- The clinching asymmetry is **R3-1**: migrating the address manufactures a dual-address recovery
  surface whose downgrade failure mode **silently restores pre-migration state** — a genuine
  money-loss vector, confirmed constructible under D6/H15 as written. The migration's only
  benefit is cosmetic; its failure mode loses money.
- R3-5 (root-key hygiene) is shared by both options and **not fixed by migrating** — it removes
  the last argument for the migration and is recorded as out-of-scope-unless-forced.

**Recommendation to the owner:** harden D4/D5 from "migration deferred (blocker list)" to
"**migration rejected absent a forcing cryptographic break**", with two preserved carve-outs:
(1) the BRC still documents the conformant derivation (`2-wallet backup-1`) for **new**
implementations of the standard — interop preserved without ever moving an existing wallet's
chain; (2) if a forcing break ever mandates migration, v2-aware recovery MUST fail closed when
the v2 address yields nothing but a legacy chain exists (R3-1's rule), and the migration gets its
own plan + harness phase. **The decision is the owner's**; the plan carries this as a PENDING
OWNER DECISION block inside D4/D5 and changes no decision text until sign-off.

---

## 5. Finding → plan-edit index

| Finding | Plan edit |
|---|---|
| R1-01, R1-02, R1-03, R1-06, R3-2 | D6 rewritten; D12 selection requirement; H6/H7/H10 extended; mock spent-endpoints |
| R1-04, R2-5, R4-1, R4-4, R4-10 | D7 extended (three-valued broadcast, full input list, decision table, all trigger paths, rebroadcast-cached-tx); H5 extended |
| R1-05 | Phase 5 sweep rule; new H19 |
| R1-07 | H5 txid-recording assertion |
| R2-3, R4-3, R4-11 | New D13 (producer integrity); §3.3 baseline sentence; H3/H4 extended |
| R2-4, R4-6, R5-5 | Phase 2 health surface extended (divergence, tip confirmation, key identity); H9 extended |
| R2-6 | G3 reworded; D15; H4 loser-state; promote-timer removed (D14) |
| R4-2, R4-5, R4-7, R4-8 | New D14 (destructive-write authority: abandoned-unbroadcast, conf-depth strip, delete allowlist, limbo policy); H14 rewritten; new H17 |
| R4-9 | D2 extended (row-version arbiter); H4 extended |
| R2-1/2/7 residuals | New D15 (ordering: tie-break, parent-topological recovery, dual-genesis) |
| R5-1, R5-9 | H1 tightened; resumable recovery (Phase 2); H5 import-path kills; mock 429 fault |
| R5-2 | H2 classification split + value-correctness oracles |
| R5-3 | H4 lag distribution |
| R5-4 | H15 schema-generation corpus |
| R5-6 | New H18 live-recovery smoke; §5 meta-gap statement |
| R5-7 | Mock eviction horizon + price-cache fault; H9 |
| R5-8 | D12 extended; mock correlated-lag/flip faults |
| R3-1, R3-4, R3-5 + crypto verdict | D4/D5 PENDING OWNER DECISION blocks; Appendix A |
| R3-3 | Appendix A privacy-claim downgrade |
