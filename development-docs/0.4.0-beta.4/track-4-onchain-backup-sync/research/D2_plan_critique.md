# D2 — Adversarial completeness critique of IMPLEMENTATION_PLAN.md

**Task:** D2. **Date:** 2026-08-23. **Target:** `development-docs/0.4.0-beta.4/sprint-4-onchain-backup-sync/IMPLEMENTATION_PLAN.md` (read in full, 554 lines).
**Verdict: FIX-THEN-SHIP.** The plan satisfies the contract's structure completely — every clause has a
home, every A3 constraint is placed, every required harness scenario exists with pass/fail — but it
contains two HIGH gaps (one internal contradiction that makes G2/H9 fail by design; one cost-floor
claim that contradicts both A1 and the code) and a handful of medium fixes, all local edits.

Labels: VERIFIED = I read the cited file/lines myself. INFERRED = my conclusion from verified facts.

---

## 1. Contract check, clause by clause

### Clause 1 — Goals numbered and testable: PASS with two defects

G1–G10 are numbered, priority-ordered (correctness > cost > speed, stated), and each maps to named
H-tests. The five non-negotiables are all present: multi-device (G3), never-lose-money (G2), seed-only
recovery (G1), low cost (G6–G8), priority order (§1 preamble). Most goals are genuinely phrased so a
test can fail them (G1's "signed spend passes full script verification", G4's "diff == exclusion list
exactly", G6's "zero backup transactions").

**Defect 1 (HIGH) — G2 contradicts the plan's own §3.3.** G2 promises: every spendable output older
than **10 minutes** is recoverable, "zero windows", asserted "at every instant" by H9. But §3.3 keeps
the shipped triggers unchanged: the event trigger fires only when a spend/receive is **≥ $3.00 USD**
(A1 §1a: `request_backup_check_if_significant`, `main.rs:490` `usd_value >= 3.0`); anything smaller
waits for the **3-hour** periodic. A $1 receive is spendable money that stays unbackable for up to 3 h
— H9 fails G2 by design the first time the soak's op stream contains a small receive. Additionally,
`task_backup.rs:20` (A1 §1a — VERIFIED in A1, precondition list) requires **balance ≥ 3000 sats**
before any backup runs: a wallet whose entire balance is below that never backs up at all, another
structural G2 violation the plan never mentions. Fix: either (a) scope the bound honestly — 10 min for
≥ $3 changes, 3 h for smaller ones, wallets under the funding minimum excluded with the number stated —
and encode exactly that in H9's assertion; or (b) change the trigger so any new spendable output sets
the dirty flag and say so in §3.3. Either is fine; the current text is self-contradictory.

**Defect 2 (MEDIUM) — G7 is partially circular.** "Yearly cost per profile lands within the measured
cost table (§3.4)" cannot fail: §3.4's yearly numbers are produced by the same H8/H9 runs that would
be judged against them. The testable parts of G7 are the size-class assertion, the hard cap, and the
2× C-vs-A bound. Fix: after H8a fixes the fee reality, freeze a numeric yearly ceiling per profile and
make that the pass/fail bound.

Minor: G9 is a measurement goal, not a pass/fail requirement — the plan says so openly (decision made
"from that number, not a guess"), which I accept as honest framing.

### Clause 2 — Adopt/adapt/diverge on record: PASS with one missing decision

- **Payload vs BRC-38/39:** D1 is on record, grounded in B2's compat table, states the gate choice
  (option (b) shipped, (a) pursued), declares the stripped-profile divergence, and quotes B2
  accurately (see spot-checks 1–2). VERIFIED against B2 §1–§4.
- **BRC-39 has no decision record (MEDIUM).** The contract clause is "payload vs BRC-38/**39**". D1
  covers 38; D4 fixes the KDF; D8 fixes the numbering — but nowhere does the plan state whether our
  AES-GCM envelope adopts, adapts, or diverges from BRC-39 (the encryption extension), with reasons.
  Phase 8 even lists "container is not BRC-39" as an A2 landmine. One more D-decision is needed
  (almost certainly DIVERGE: shipped KDF ≠ BRC-39's derivation; say it and cite D4).
- **Delta format vs toolbox SyncChunks:** D2 delivers the preferred outcome at the level the evidence
  supports — row forms and merge semantics align, "one wallet feeds both rails" — and names two
  concrete blockers to literal container reuse (no delete records; stripped rows violate
  notNullable/`checkEntityValues`). Both blockers VERIFIED in C1 ("No deletions — fatal as-is",
  C1 what-does-not-align items 1–2). C2's Go-bug caveats (AllEntityNames order, `proveTxId` tag,
  vector-corpus authoring errors, identityKey-transport-auth) are all real C2 findings — VERIFIED.
- **Delta vs backup-cache log semantics:** D3 matches C3 exactly: opaque blob ("Nothing in this
  package interprets blob contents" — C3 §1, verbatim), `prevSha256` stored-not-verified (C3 §2),
  keep-newest-2 retention guard, `deviceId` partition mapping, "zero-knowledge" phrasing ban (C3
  flag 2), ts-client Open BSV v6 nuance. VERIFIED against C3. The no-license/no-vendoring constraint
  is restated at the top of the plan and inside D3 ("semantics only, no code").

### Clause 3 — Trigger + cadence reassessment: PASS with three defects

The deltas-per-snapshot rule is evaluated against the only real data A1 found (431/165/61 KB
snapshots), correctly concludes the count clause dominates (arithmetic checks out: 215 KB ÷ 1–2 KB
≈ 107–215 ≫ 20), and — as the contract demanded for the no-delta-data case — says plainly that delta
sizes are unmeasured, the rule **stays provisional**, and names H3+H9 as the settling tests. Polling
stays; the dirty flag's delta successor is specified with the canonicalization requirements from
constraint 2; the cost table exists per profile with *est.* marks and replacing tests.

**Defect 3 (HIGH) — the cost floor misquotes A1 and contradicts the code.** §3.4: "Fixed
per-broadcast floor: marker 546 sats + base tx overhead; no service fee exists on backup txs."
The no-service-fee half is right (VERIFIED: `handlers.rs:13602` "No Hodos service fee for wallet
backups"; the draft's 1,650-sat floor at draft line 37 does include "service fee", so the plan's
correction of the draft is sound). But the stated floor drops the **1000-sat PushDrop token output**:
`handlers.rs:13603` `let backup_output_sats: i64 = 1000;` (VERIFIED by direct read), and A1 §4 says
it explicitly — "plus the **1546 sats parked in token+marker** (recovered next cycle)". The 546
marker and the 1000 token have *identical* recovery semantics (both are spent by the next backup),
so counting one as "floor" and not the other is wrong under any accounting. H8's pass condition
("Every backup's cost == declared floor + rate × padded size, **exactly**") would fail against the
declared floor. Fix: steady-state burn = mining fee only; per-broadcast float = 1546 sats (token
1000 + marker 546), recovered next cycle; permanently parked only in the terminal (never-superseded)
pair. "Floor to be re-derived in H8" already hedges — the prose just has to stop asserting the wrong
constant meanwhile.

**Defect 4 (MEDIUM) — spliced measurement passes presented as one number.** §3.1/§3.3: "~60% is 11
stuck unconfirmed transactions' raw_tx (685,868 raw bytes → ~291 KB gzipped share)". A1's morning
pass measured **3** stuck txs, 532,846 raw bytes → ~291 KB gzip ≈ 60% of 431 KB; A1's [v2] evening
pass counted **11** rows, 685,868 bytes, with **no gzip share measured**. The plan pairs v2's
count/bytes with the morning's gzip/percentage — a combination A1 never measured. Direction is
unchanged (A1: "growing, not shrinking"), but a plan whose preamble says every number is relayed
from the reports should quote the two passes as two passes.

**Defect 5 (MEDIUM) — "their actual paid fees are extractable" is not A1's claim.** It sits inside
the list introduced as "Real numbers from A1 §4", but A1 never says fees are extractable — A1's
NOT-checked list says the runtime fee rate was *not* checked and its cost figures assume 1 sat/KB.
Extraction is probably feasible (backup txs' raw bytes + `parent_transactions` prev-outs are in the
DB), but that is the plan's INFERENCE and must be labelled as such, with the dependency (prev-out
values available for all 114+ txs) stated — H8a should have a fallback if some parents are absent.

**Defect 6 (MEDIUM) — the 16 KB leg of the rule is never evaluated.** The proposal under evaluation
is "0.5× / 20 deltas / **16 KB**" (DELTA_ANALYSIS.md §3: snapshot "when a delta would exceed a fixed
cap (say 16 KB compressed)"). §3.2 assesses the ratio and count clauses and goes silent on the
single-delta cap. It's the least evaluable leg without delta data — so say exactly that: provisional,
settled by H3's delta-size distribution.

### Clause 4 — Phases, acceptance criteria, README reconciliation: PASS

All eight phases have explicit **Acceptance:** blocks (checked each one individually — none missing).
The phases are sequentially shippable and the harness-first inversion of the README's "item 1 first"
is declared, justified by constraint 13, and recorded in §4.1's verdict table. The §4.1 table covers
README items 0, 0b, 1–7, the "already decided" block (all five bullets), Open Questions 1–7, T1–T7
(mapped to H-tests: T1→H1, T2→H10, T3→H8, T4→H4, T5→H11, T6→H13, T7→H12 — complete), and T8–T11
(kept verbatim, Phase 8). Verified against the README's actual item table and test table — nothing
in the README is left un-dispositioned. Function-name correction (`compress_for_onchain`, no
`prepare_backup_payload()`) matches A1 doc-claim table item 4.

**A3's 16 constraints:** §4.2 places every one. Fifteen have a design decision AND a test. Row 14
(credential namespacing) is "recorded, no code this sprint" — acceptable scoping, but "Fixed by
`deff765`" overstates A3: `deff765` fixed the shared dev/prod **Keychain service name on macOS**;
constraint 14 demands namespacing "per environment and per wallet identity on **every platform**",
which nobody verified. Say "partially closed by deff765 (env-level, macOS)" (LOW).

### Clause 5 — Test harness spec: PASS with one strictness defect

All eight contractually-required scenarios exist with setup / exact pass / exact fail / number-for-
the-BRC columns: seed-only restore-and-SPEND (H1, includes script verification against recorded
prev-outs and mock-ARC acceptance), delta replay byte-identity property (H3, canonical-form
normalization + idempotence + determinism), multi-device fork detection (H4), crash matrix (H5, kill
points from A1's step table), corruption/missing-chunk (H6, typed-error taxonomy), every-generation-
boundary restore (H7), cost accounting (H8), 90-day soak (H9). Plus junk-litter (H10), latency (H11),
padding (H12), strip-rehydrate (H13). Mock fault models trace to real episodes (E4 Bug B/C, E6
truncation, WoC 30 s–5 min lag) — all VERIFIED as real A3 episode content.

**Defect 7 (MEDIUM) — two pass conditions are still judgment calls.** (a) H9's "staleness bound held
**at every instant**" is not executable — recovery can only be sampled; the spec must state the check
cadence (e.g., run the recovery diff after every op batch or every simulated hour) or restate the
bound as "at every sampled checkpoint, with checkpoints at least every N sim-minutes". (b) H12's
"class distribution does not distinguish action types beyond the declared leak" names no criterion —
specify a concrete distinguisher test (e.g., a classifier over (size-class, timing) must not beat
chance by more than X on held-out traces, or a chi-square threshold). As written, both are "seems to
work" wearing a suit — exactly what §5's own preamble forbids.

Environment realism is good (fault-injectable mock of the exact endpoints A1 lists, deterministic
clock via the shipped `ref_ts` parameterization, live-mainnet smoke only for fee numbers). The
"zero backup tests in `rust-wallet/tests/`" premise is TRUE — VERIFIED by directory listing and grep:
no test file under `rust-wallet/tests/` references backup (matches are all `tests/fixtures/node_modules`
SDK internals); the only backup tests are unit tests inside `backup.rs` itself (A1: near-empty-payload
round trip), which is what Phase 1 exists to fix.

### Disagreements & unresolved: PASS

§6 quotes both sides in every entry I checked — Fix B vs FOLLOWUP (quotes match A3 §6/E9 verbatim),
doc-vs-code errors (all six match A1's doc-claim table), the "sync IS 38/39" phrasing (matches C1
flag 3), prevSha256 enforcement (matches C3 §2/flag 3), the 18-vs-19-table count. No averaging found.
§6.3 carries nine open items honestly, including the mnemonic-in-file-export finding (A3 §7.4) and
Bug C's unresolved root cause.

One inter-report discrepancy the plan did NOT surface (LOW): A1 item 23 says the `domain_permissions`
backup struct carries "only 5 of the table's columns"; B2 disagreement 4 says the SELECT takes "7
columns; the table has 12". Both agree on the 3 dropped columns the plan names, so Phase 2 is
unaffected, but §6.2 is the plan's own designated home for exactly this kind of contradiction and it
isn't there. The H2 manifest will settle the true count.

### What-was-NOT-checked: PASS

§7 aggregates each report's NOT-checked list (checked against all eight reports' own lists — faithful,
including the honest "this plan re-verified nothing against code" declaration and the commit pins;
the BRCs pin `c1d12f2` and ts-stack `8b074a0` match B2/C2). The "nobody checked" items (RelayX/Rock,
no profile-A/C fixtures exist) are correctly carried.

### Standing constraints: PASS

Draft-follows-code: stated twice, executed via Appendix A (10 concrete draft edits, each tied to a
D-decision); nowhere does the plan defer to the draft as truth — D4/D5 explicitly override it.
No-license: stated in the preamble and inside D3; the plan adopts semantics with citations and
forbids vendoring. Both constraints are genuinely honored, not just recited.

---

## 2. Spot-checks (12 claims; 6 taken to code/spec directly)

| # | Plan claim | Source check | Direct code/spec check | Verdict |
|---|---|---|---|---|
| 1 | D1: option (c) "is strictly worse than what we already have" (B2 §4) | B2 §4 has the sentence verbatim | — | ACCURATE |
| 2 | D1: 13 tables + user + sourceStorage, zero missing columns, 16 extras, one tier-1 orphan (`current_index`), one shape transform (`history`) | B2 §1 count-check + §3 verbatim | — | ACCURATE |
| 3 | D4: KDF = `SHA-256(master_privkey ‖ "hodos-wallet-backup-v1")`, BRC-42 only for address keypair | A1 §3a | **`backup.rs:956-967` read: exact match** | ACCURATE |
| 4 | D5: invoice literal `"1-wallet-backup-1"` | B1 via B2 disagreement 3 | **`connection.rs:390/550/716`, `handlers.rs:13332` read: exact literal** | ACCURATE |
| 5 | §3.4: "no service fee exists on backup txs" | A1 item 2, A3 disagreement 5 | **`handlers.rs:13602` comment read: confirmed** | ACCURATE |
| 6 | §3.4: "floor: marker 546 sats + base tx overhead" | A1 §4 says "1546 sats parked in token+marker" | **`handlers.rs:13603` `backup_output_sats = 1000` read: plan omits the token park** | **WRONG — Defect 3 (HIGH)** |
| 7 | §3.1: 431 KB live payload, 2.2× the 200 KB warning, no cap/chunking | A1 §3d/§4 (431,476 B) | **`backup.rs:1262-1264` read: `> 200_000` warn-only; no cap** | ACCURATE |
| 8 | §3.1/§3.3: "~60% is 11 stuck txs (685,868 B → ~291 KB gzip)" | A1 morning: 3 txs/532,846 B → 291 KB; A1 v2: 11 txs/685,868 B, no gzip figure | — | **SPLICED — Defect 4 (MEDIUM)** |
| 9 | D2 blockers: no delete records; stripped rows fail `checkEntityValues`/notNullable | C1 what-does-not-align 1–2, verbatim | — | ACCURATE |
| 10 | D2/D8: Go `AllEntityNames`/`proveTxId` bugs; vector corpus has authoring errors; Aug-2026 identityKey binding (`75c0aa4`) | C2 §1.1–1.2, addendum 1–3 | — | ACCURATE |
| 11 | D3: opaque blob; `prevSha256` stored verbatim never validated; keep-newest-2; "zero-knowledge" banned | C3 §1/§2/flags | — | ACCURATE |
| 12 | D1/D9: BRC-38 §5.4 omit-if-absent; §10 importers "MAY decline to activate" `syncStates` | B2 §1 | **`outpoints/0038.md` @ `c1d12f2` read: §5.4 line 164 confirmed; §10 actual words are "MAY treat … as operationally sensitive and choose whether to activate" — plan's quoted phrase is a paraphrase in quotation marks** | ACCURATE in substance; LOW quote-discipline nit (Defect 9) |
| 13 | Phase 1 premise: zero backup tests in `rust-wallet/tests/` | A1 source B, A3 §5 | **`rust-wallet/tests/` listed + grepped: confirmed (only fixtures/node_modules match "backup")** | ACCURATE |

Also re-verified at report level (not to code): G1's inflation citation (A3 E1-`58cf9a3` — real,
verbatim mechanism match), G5's BS-C2 fail-closed citation (A3 §7.1, `handlers.rs:14202`), D6's
stale-index TODO + input-0 chain walk (A1 §3c items 2–3), D7's FOLLOWUP quote and "no implementing
commit" (A3 §6/E11 — "no `BackupIntent` code exists"), all 16 constraints' text (A3 §5 — plan's
short forms are faithful), §4.1's 19-tables correction (A1 §2 + B2 §0b agree).

---

## 3. Gap list (ranked)

| # | Sev | Where | Problem | Fix |
|---|---|---|---|---|
| 1 | HIGH | §1 G2 + §3.3 + H9 | 10-min staleness bound for ALL spendable money vs kept ≥$3/3-h triggers and the 3000-sat backup minimum → H9 fails G2 by design | Scope the bound (10 min for ≥$3; 3 h otherwise; funding minimum stated) or change the trigger; encode the chosen bound in H9 |
| 2 | HIGH | §3.4 floor + H8 | Floor "marker 546 + overhead" omits the 1000-sat token park; contradicts A1 §4 and `handlers.rs:13603`; H8's exact-cost assertion fails as declared | Burn = mining fee; float = 1546 sats recovered next cycle; terminal pair permanently parked |
| 3 | MED | §3.1, §3.3 | v2 count/bytes (11 tx, 685,868 B) spliced with morning gzip share (~291 KB / ~60% from 3 tx) — a number A1 never measured | Quote the two passes separately or recompute |
| 4 | MED | §3.1, H8a | "actual paid fees are extractable" presented as A1 data; it is plan inference; A1 explicitly did not check fee rates | Label INFERRED; state the prev-out dependency and an H8a fallback |
| 5 | MED | §2 | No adopt/adapt/diverge record for BRC-39 (contract says "payload vs BRC-38/**39**") | Add a D-decision (likely DIVERGE, grounded in D4/D8) |
| 6 | MED | §3.2 | 16 KB single-delta-cap leg of the rule never evaluated or restated | State it stays provisional; H3 delta-size distribution settles it |
| 7 | MED | §1 G7 | "lands within the measured cost table" is circular — cannot fail | Freeze numeric ceilings after H8a; keep the 2× C-vs-A clause |
| 8 | MED | §5 H9, H12 | "at every instant" unexecutable (no check cadence); padding-leak pass has no statistical criterion | Specify sampling cadence; specify a distinguisher test with a threshold |
| 9 | LOW | D9 | Paraphrase of 0038.md §10 inside quotation marks | Quote the spec's actual words |
| 10 | LOW | §4.2 row 14 | "Fixed by deff765" overstates A3 (env-level, macOS only; per-wallet-identity unverified, untested) | "Partially closed"; note the open half |
| 11 | LOW | §6.2 | A1-vs-B2 `domain_permissions` carried-column-count discrepancy (5 vs 7) not surfaced | Add to §6.2; H2 manifest settles it |

Everything else checked clean. No averaged disagreements found; no reliance on the draft as truth
found; no license-tainted adoption found.

---

## Checked (directly, this task)

- `IMPLEMENTATION_PLAN.md` — all 554 lines.
- Research reports, in full: A1 (259 ln), A3 (230 ln), B2 (192 ln); targeted full-section reads of
  C1 (SyncChunk semantics, what-aligns/what-doesn't, flags 1–7), C2 (§1 divergences, §5 conformance,
  addendum 1–6), C3 (§1 opaque-blob, §2 chaining/retention, comparison table rows, flags).
- `0.4.0-beta.4/sprint-4-onchain-backup-sync/README.md` — items table (137–160), decided block + Open Q1–7 + tests
  T1–T7 (225–287).
- `DELTA_ANALYSIS.md` — §3 snapshot rule (lines 93–111 region).
- Code (Hodos @ working tree ≈ `6b4a4be`): `handlers.rs:13595-13610` (no-service-fee comment +
  `backup_output_sats = 1000`); `backup.rs:956-967` (KDF); `backup.rs:1255-1268` (200 KB warn, no
  cap); invoice-literal grep (`connection.rs:390/550/716`, `handlers.rs:13332,14696`);
  `rust-wallet/tests/` listing + backup grep.
- Spec (BRCs @ `c1d12f2`, scratchpad clone): `outpoints/0038.md` §5.4 (line 164), §10 (import
  semantics, full section); title lines of 0038/0040 (authorship/naming).
- BRC draft `wallet-backup-and-sync-onchain.md` line 37 (the 1,650-sat floor decomposition) and
  `ONCHAIN_BACKUP_SYSTEM.md` output-layout/service-fee lines (grep-level).

## NOT checked

- A2 and B1 read only via the plan's and B2/C1's citations of them — I did not re-read A2/B1 in full;
  plan claims sourced solely to A2 (import backend "live, not rotted"; four landmines) and B1
  (BRC-43 invoice grammar) are taken at report level, not re-verified.
- C1/C2/C3 sections outside the targeted reads (e.g., C1 §1–§4 line-by-line, C3 auth-proof detail).
- No code execution anywhere (same limit as every upstream report).
- The live production DB (A1's 431 KB / row-count numbers taken from A1, not re-queried).
- `task_backup.rs:20` (3000-sat precondition) and `main.rs:490` ($3 threshold) — taken from A1's
  VERIFIED citations, not re-read; the G2 contradiction (gap 1) rests on A1's line-cited findings.
- go-wallet-toolbox and go-private-backup-cache clones (C2/C3 taken at report level; only BRCs and
  Hodos were opened directly).
- Whether the draft's 1,650-sat floor arithmetic is 546+1000+~104 exactly (draft line 37 read; its
  arithmetic not decomposed further).
