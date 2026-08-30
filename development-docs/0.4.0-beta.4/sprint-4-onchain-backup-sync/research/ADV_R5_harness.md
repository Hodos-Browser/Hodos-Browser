# ADV_R5 — Harness Adequacy Adversary

**Lens R5.** The plan's §5 says "seems to work is a fail." This report attacks §5 itself: for
each harness test H1–H16, the scenario where the test **as written passes green while the guarantee
it pins is violated in production**. Every finding names the starting state, the step sequence, the
violated G#, and the exact clause of the H-test that lets the break through.

Premises re-verified against code (not relayed): refetch is best-effort and reports a *count*, not a
failure (`handlers.rs:15300-15334`, A1 §5.3); `services/call_class.rs` is a timeout policy only —
**no 429/backoff/retry logic exists anywhere** (grep clean); recovery marker selection is
`max_by_key` over WoC `unspent/all` (`handlers.rs:14765-14771`); KDF and GCM envelope confirmed
(`backup.rs:956-967, 1270-1291`); no rate-limit handling in the backup/recovery region.

Severity key: CRITICAL = loses money / unrecoverable wallet; HIGH = silent wrong state; MEDIUM =
visible failure / bad UX or cost; LOW.

---

## R5-1 (CRITICAL) — WoC rate-limit / ban mid-recovery is in no mock fault model; refetch swallows it and recovery still reports success

**Guarantee violated:** G1 (seed-only restore that can spend), G8 (re-fetchable bytes rehydrated
byte-identical), G10 (never-panic / typed skip).

**Starting state.** Profile C fixture (200 image ordinals) or profile B (500 token rows) backed up
as snapshot + 20 deltas. Recovery of a real wallet re-fetches raw_tx + TSC proof + locking scripts
for *every* txid in the payload — on the live production wallet that is 943 parent txs; even after
strips, hundreds of `tx/{txid}/hex` and proof calls in a tight loop (`refetch_stripped_data`,
`handlers.rs:15218-15318`).

**Steps.**
1. Fresh machine, seed only. Recovery decrypts the snapshot, imports, then enters the refetch loop.
2. After ~a few hundred rapid requests, real WoC returns HTTP 429 (or a temporary IP ban) for the
   remainder of the loop. `fetch_parent_transaction_from_api` sees `!status.is_success()` -> `Err`.
3. Every 429 is a `log::warn!` + `errors += 1`; the loop continues (`handlers.rs:15298-15316`).
   `refetch_stripped_data` returns `{"errors": 240}` and **recovery reports success** (A1 §5.3:
   "recovery reports success regardless").
4. Outputs whose `locking_script` was stripped and never rehydrated are now present with empty/wrong
   scripts; token outputs are unspendable; script verification of a later spend fails.

**Why the harness passes green.** §5's fault-injection menu is enumerated explicitly: "index lag …
JSON `scriptPubKey` truncation (E6), 404s, ARC 200-with-different-txid, `DOUBLE_SPEND_ATTEMPTED`."
**429 / rate-limit / mid-loop ban is not in the list**, and the mock chain is generated from
endpoint *contracts* (D12/H16) that describe success and error *shapes* but not a throughput ceiling
that trips only after N calls. H1's fixture recovery therefore runs the whole refetch loop against a
mock that answers all several-hundred calls instantly and never 429s -> `errors == 0` -> H1 PASS.
Even if a tester did inject it, H1's PASS clause accepts "refetch-failure count is 0 **or each
failure explicitly surfaced**" — a nonzero `errors` counter in a JSON blob *is* "surfaced," so H1
can be read green while the outputs are silently unspendable. H6's outage case is total-outage (all
calls fail -> "indexer unavailable"), not partial exhaustion mid-bulk-fetch.

**Proposed fix.** Add a first-class "rate-limit after N calls / mid-bulk 429 + Retry-After" fault to
the §5 mock and require it in H1 and H6. Make recovery **fail closed on any nonzero refetch error for
a non-re-derivable output** (token/BRC-42 outputs whose script did not rehydrate must block
success), not merely count it; H1's PASS clause must read "refetch error count for non-re-derivable
outputs is 0," deleting the "or surfaced" escape hatch. Add retry/backoff honoring Retry-After in the
refetch loop and contract it in H16.

---

## R5-2 (HIGH) — H3/H1 canonical form *excludes* the very columns where divergence hides; the excluded-volatile list is unaudited for correctness

**Guarantee violated:** G4 (no silent data loss; DB diff == exclusion list *exactly*).

**Setup.** H3 compares `canonical(replay(snapshot+deltas))` byte-identical to `canonical(continuous
state)`, where `canonical` = "sorted, ID-remapped, **volatile columns excluded per spec**." H1's
DB-diff PASS is "diff vs original == the H2 exclusion list exactly." Both comparisons **subtract the
excluded/volatile/re-derived columns before comparing**.

**The break.** `outputs.confirmed` was historically exactly this class (A1 §2 item 26: omitted ->
every recovered output landed `confirmed=1`, and confirmed-preferred UTXO selection then treated
unconfirmed coins as confirmed). Suppose a column is classified **re-derived** or **volatile** (so
it is excluded from `canonical`) but its re-derivation actually produces the *wrong value*:
- `outputs.confirmed` re-set to 1 on every recovered output.
- `proven_txs.merkle_path`: write path stored toolbox **binary**; refetch re-stores **TSC-JSON**
  (A1 §5.3 — "two formats in one column, consumers must handle both," flagged, never traced to
  consumers). A consumer that assumes binary breaks post-recovery.

Because these columns are on the volatile/re-derived list, **H3 subtracts them before the
byte-compare -> byte-identical -> PASS**, and **H1 subtracts them from the diff -> diff == exclusion
list -> PASS**. The divergence is invisible to both.

**Why H2 doesn't save it.** H2's PASS validates only that every **"travels"** column survives
round-trip; its FAIL is "any *travels* column that does not survive." **Re-derived and excluded
columns get no value-correctness check at all** — H2 only checks they are *classified*, not that
their re-derivation is *right*. H13 byte-matches `locking_script` specifically, but nothing
byte-matches `confirmed`, `merkle_path` format, `price_usd_cents`, or any future re-derived column
against its pre-wipe value. Who audits the exclusion list? No test does.

**Proposed fix.** Split the classification: a column may be "excluded from the *dirty/equivalence*
comparison" only if it is *also* asserted, by a dedicated post-recovery check, to re-derive to a
value functionally equivalent to the original (confirmed==confirmed, merkle_path in the format
consumers read). Add to H2 a "re-derived columns re-derive to the recorded original value" pass over
a fully-populated fixture — turning the exclusion list from an escape hatch into an audited contract.
Pin the `merkle_path` binary-vs-TSC-JSON format as a named consumer contract test.

---

## R5-3 (HIGH) — H4's fixed propagation delay never samples the WoC lag *tail*; 500 rounds is adequate for the window it samples and irrelevant to the window it doesn't

**Guarantee violated:** G3 (fork detected, <=2-round convergence, no lost row) — and the tie-break
rule (Q2) is *decided from H4 data*, so a coverage hole propagates into a shipped decision.

**Rough computation (the priority-d ask).** H4 scripts "both write within one delay window," repeated
"under randomized timing >=500 rounds," on a mock with a **single configurable propagation delay** D.
For the *same-parent simultaneous* fork inside a fixed D, 500 forced rounds is statistically ample:
if the true-simultaneity sub-band is p~=0.1 of the window, expected hits ~=50, P(zero)~=e^-50~=0. So
**500 is not the problem for the sampled window** — more rounds buy nothing.

The problem is the **unsampled** window. Real WoC index lag is **30 s–5 min with a heavy tail** (A1
§3c/E4 Bug A). At the tail, device B polls, sees a tip that is *many deltas stale*, and writes a
child of an **hours-old parent** — a **deep fork spanning N intervening deltas**, not a same-parent
fork. A fixed-D mock generates **zero** deep forks regardless of round count. The convergence proof
"<= 2 re-read/re-write rounds" and the tie-break rule are therefore validated only against
same-parent forks; a deep fork can require re-applying N deltas and may exceed 2 rounds or drop the
losing branch's unique rows (lost update).

**Steps.** Device A does 6 spends over 4 minutes (6 deltas). Device B's WoC index is lagging at the
5-min tail; B polls, sees the pre-spend tip, spends, and writes a delta whose `parent_txid` is 6
deltas back. Both branches now exist; reconciling requires merging B's row with A's 6 -> the "<=2
round" and "lower-device_id-wins" rules were never exercised on this shape.

**Why the harness passes green.** H4's setup fixes `configurable propagation delay` to one value and
*scripts* the collision as "both write within one delay window" — it constructs same-parent
collisions by design and never draws D from the 30 s–5 min distribution, so the deep-fork case is
outside its sample space. Green on 500 same-parent forks; silent on the one lag regime that actually
caused E4.

**Proposed fix.** Draw D per round from the *contracted* WoC lag distribution (H16's lag window),
not a constant; add explicit deep-fork rounds (poll against a tip k deltas stale, k in {1,5,20});
assert convergence and no-lost-row for each k; only then let H4 decide Q2's tie-break.

---

## R5-4 (HIGH) — H15 pins decrypt-by-envelope-version but nothing pins *import of an old payload JSON shape*; fixtures are current-schema only

**Guarantee violated:** G11 (every payload version ever broadcast stays *recoverable* forever — not
merely decryptable).

**The gap.** H15's fixtures are "one per **payload version** ever broadcast … starting with the
shipped headerless gzip->AES-GCM under SHA-256(master ‖ …)." That axis is the **envelope/KDF**
version (the token `version` byte, D4). H15's PASS is "current build **recovers** every fixture
byte-exact; decode order is current-first-then-legacy." But the payload *inside* the envelope is a
`BackupPayload` JSON whose **DB schema evolves independently** (V9…V24, and counting). A token
broadcast months ago carries a JSON shape with fewer columns, older field names, and the old
`proven_tx_reqs.history` **map** shape (B2 §1; §6.2.3 — the map->notes-array transform is still
unresolved). The current import code (`import_to_db_with_ids`, expecting today's schema) must ingest
that old JSON.

**Steps.**
1. A user's newest on-chain token was written under app schema V18 (V18 permission tables present in
   a now-changed form; `history` stored as `{ts: note}` map; no `price_usd_cents`).
2. A KDF/envelope change ships later; H15 adds a V18-envelope fixture and it **decrypts** fine ->
   H15 green.
3. On real recovery, the decrypted V18 JSON is fed to V24 import: renamed/re-shaped fields
   (`history` map vs `{"notes":[…]}`) deserialize to defaults or error; permission rows import with
   wrong columns; the wallet restores in a subtly wrong state or the import throws.

**Why the harness passes green.** H15 tests **decrypt-compat**, and its own words ("recovers …
byte-exact") are ambiguous between "decrypts to the original bytes" and "imports to a correct DB" —
the fixture set is defined by *envelope version*, and there is **no fixture keyed by old DB-schema
JSON shape**. H2's fixtures are "the *live* schema" (current-schema only). H1/H3 build fresh payloads
in today's format. So **no test recovers an old-schema payload through current import code** — the
exact E5 serialization-drift class (certificate fields silently dropped) reappears one schema
generation later, invisible.

**Proposed fix.** Add an append-only corpus of **decrypted-payload JSON fixtures, one per DB-schema
generation ever broadcast** (not per envelope version), and require H15 (or a new H15b) to run each
through the *current* full import + refetch + a spend, asserting the recovered DB is correct — the
same round-trip H1 does, but on frozen old-shape inputs. Freeze the `history` map->notes transform
before Phase 5 (§6.2.3 is still open).

---

## R5-5 (HIGH) — Constraint 14 (per-wallet-identity credential namespacing) has no test and the mock cannot represent a shared OS credential store; every H passes with isolated fixture keys while E12 (12.5M sats) is unguarded

**Guarantee violated:** G1/G2 (recovery/backup must use the *right* key; a wrong-key wallet loses or
strands money).

**The gap.** E12 is the single largest-money episode in the whole history: dev and prod wallets
**shared the macOS Keychain service name**, a dev-wallet creation overwrote the production mnemonic,
and every prod session after June 25 signed with **dev keys** — funds migrated to dev-keyed
addresses, 12.5M sats swept back, **three months of silent divergence** before anyone noticed.
Constraint 14 (§4.2 row 14) is explicitly marked **"out of harness scope; open half recorded … no
test."**

**Steps.** Two wallets on one machine share the credential store; wallet B's unlock reads wallet A's
seed; B backs up and recovers A's chain under B's identity, or signs B's funding UTXO with A's key ->
HASH160 mismatch -> OP_EQUALVERIFY failure and unbroadcastable backups — for months, silently.

**Why the harness passes green.** The §5 fixtures are self-contained wallets with keys handed
directly into the mock; the harness has **no model of an OS credential store**, so it cannot express
"two wallets, one store, cross-contaminated key." There is no H-test at all — constraint 14 passes
**vacuously** (by the plan's own admission), and every other H (which uses correct isolated keys)
stays green while the real failure mode is untouched. This is the largest-money incident in A3 and
the harness is blind to it.

**Proposed fix.** Even if the fix is deferred, add a **health/observability** assertion to H9: a
signing-key-identity mismatch between the wallet identity and the key that produced the last backup
must surface in the health state within one cycle (closing E12's "no signal for 3 months" half). Add
a per-wallet-identity namespacing regression fixture (two wallet identities, one simulated store)
even as an ignored skeleton so the gap is tracked, not merely prose.

---

## R5-6 (HIGH) — The live-smoke tier never exercises seed-only recovery against the real network; the product's core promise is proven only against the mock, and §5 does not say so plainly

**Guarantee violated:** honest coverage of G1 (the whole point of the system).

**The claim under attack.** §5: "A small number of live-mainnet smoke runs (dev wallet) produce
**real fee numbers**; everything else runs against the mock." H11 smoke = poll latency; H16 smoke =
contract-conformance probing (diff observed vs contracted semantics). **No smoke run performs a full
seed-only restore-and-spend against live WoC/ARC.** Meanwhile §7 admits "no record exists of a real
user recovery succeeding in the field" and "H1 … is the first runtime proof" — but H1 runs on the
**mock**.

**Why this is a harness-adequacy failure, not just a nit.** Every real-network hazard that has
actually broken this system — index-lag flips *between* two calls in one recovery, JSON truncation
under load (E6), rate-limit/ban (R5-1), TLS resets, ARC 200-on-reject (E4 Bug B), propagation-window
races (E4 Bug A) — is *modeled* in the mock from a *contract* that a human wrote by reading docs.
The mock is the map; the incident history is entirely about the territory diverging from the map. A
green H1/H5/H6/H9 certifies the wallet against the team's *belief* about WoC, and the plan presents
that as recovery being proven while stating elsewhere it never ran for real.

**Proposed fix.** Add an explicit **live-recovery smoke** to the tier: dev wallet, real broadcast,
real seed-only restore-and-spend against live WoC+ARC, run on a cadence — and state in §5 that mock
green is *necessary, not sufficient*, with the live-recovery smoke as the sufficiency gate before any
release that touches recovery. Rewrite the coverage sentence to name what the smoke tier does and
does **not** cover.

---

## R5-7 (HIGH) — H9's accelerated clock skips mempool eviction (~2 weeks) and price-cache staleness; the E11 "Missing inputs" money-loss class is unsampled

**Guarantee violated:** G2 (never lose spendable money), G5 (bounded, visible failure).

**The gap.** H9 is "90 **simulated** days (accelerated clock)." Several production failures are keyed
to **wall-clock real time**, not simulated ticks, and acceleration compresses them out of existence:
- **Mempool eviction (~14 real days).** A backup tx broadcast but never mined is evicted from real
  mempools after ~2 weeks; its funding input (and the prev PushDrop/marker it spent) **reappear as
  unspent** in WoC's index while the DB still marks them spent (placeholder). This is exactly the
  E11 field bug: believed-spendable funding output actually consumed -> "Missing inputs" -> rollback
  -> retry forever (before backoff), or a fresh backup that double-spends the reappeared input. An
  accelerated mock advances *simulated* days in milliseconds and models no wall-time eviction, so the
  reappearance never happens in the soak.
- **Price-cache staleness.** The USD price cache going empty/stale is a real network dependency
  (A1 §2 item 16). The soak feeds a fixed price, so the empty-cache path is never entered. (Partly
  mitigated *iff* §3.3's "any new spendable output sets the dirty flag" fully removes the USD gate
  from dirty-marking — but H9 as written never proves the price-cache-empty path is harmless.)

**Why the harness passes green.** H9 checks "zero no-op broadcasts," "staleness bound held," "every
injected failure surfaces" — all against a mock whose mempool never evicts on wall time and whose
price never goes stale. The soak can run 90 green simulated days while the 14-real-day eviction that
caused E11 is structurally impossible to occur.

**Proposed fix.** Give the mock a mempool with a *simulated-time* eviction horizon tied to the same
accelerated clock (evict unconfirmed after 14 simulated days), and inject an unmined-backup scenario
into H9; assert the reappeared-input case converges via the D7 intent record + startup reconcile
without double-spend or unbounded retry. Add a price-cache-empty window to the soak and assert
dirty-marking is unaffected.

---

## R5-8 (MEDIUM) — The two-oracle recency check (D6) is validated against an unchosen secondary and a mock that returns *consistent* data on retry; correlated real-world lag and mid-recovery index flips are unsampled

**Guarantee violated:** G5 / D6 (stale-index restore refused; "indexer unavailable" != "no backup").

**The gap.** D6's stale-index defense and H7's "recency check flags staleness (**given the second
oracle**)" depend on a secondary provider that D12 says is "**exact service to be confirmed before
Phase 1**" — it does not exist yet, so H7 validates against a *mock* second oracle whose real
contract is unknown. Two failure shapes the mock can't represent:
1. **Correlated lag.** The mock models WoC and the secondary as *independent* states, so a
   disagreement is always detectable. Real secondary indexers often ingest from overlapping sources
   and lag *together* in the same direction during a reorg/propagation event -> both report the
   superseded token as unspent -> the two-oracle check agrees and **restores stale state**, exactly
   what D6 was meant to prevent.
2. **Mid-recovery flip.** WoC can return token T as unspent on the bootstrap query and, on the
   recency re-check moments later, as spent (index catching up) — "different data on retry." The mock
   serves a fixed state per configured fault, so it cannot express a flip *between two calls within
   one recovery* unless that exact transition is hand-scripted; H6's stale-index case is a single
   static state, not a transition.

**Why the harness passes green.** H7 and H6 both configure a *static* index state and an independent
mock oracle; they pass by construction on the cases they encode. The correlated-lag and
flip-mid-recovery cases are outside the mock's representational range, so no green/red signal exists
for them.

**Proposed fix.** Model the secondary oracle with a *correlated* lag component (shared-source
scenario) and require H7 to include a round where both oracles are stale in the same direction —
recovery must still refuse or flag, not restore. Add a "response changes between call 1 and call 2 of
the same recovery" fault to the §5 mock and require it in H6/H7. Gate the secondary's promotion (D12
shadow mode) on a recorded correlated-divergence analysis, not just per-call divergence.

---

## R5-9 (MEDIUM) — Constraint 11 (atomic import) is asserted by H1/H6 but no test crashes *mid-import*; H5's crash matrix covers only the write path

**Guarantee violated:** G4 / constraint 11 (mid-import failure -> prior state, never a half-state
that traps retry).

**The gap.** Constraint 11's guarantee is that a failure *during* import leaves the wallet in its
prior state. Its tests are listed as H1 and H6. H1 imports successfully (no injected mid-import
fault). H6 injects *content* faults (corrupt GCM, malformed JSON, missing parent) that fail *before*
the DB transaction — a typed error, no partial write. **H5's kill-point matrix is explicitly "every
step boundary of the write path"** (reserve -> broadcast -> record), **not the recovery/import
path.** So the case constraint 11 exists for — a hard kill *between* the FK-cleanup and the end of
the import transaction, or a mid-import disk/lock error — is injected by no test.

**Steps.** Recovery decrypts, deletes the auto-created rows, begins `import_to_db_with_ids`, and the
process is killed (or a lock error fires) after the delete but mid-INSERT. Recovery is
fresh-wallet-only and rejects an existing wallet on the next attempt (`handlers.rs:15008-15017`) — so
if the half-import left wallet rows, retry may be *blocked*, trapping the user with a half-restored
wallet and no clean path, the precise "half-state that traps retry" constraint 11 forbids.

**Why the harness passes green.** H1's PASS is about a *successful* diff; H6's PASS is *typed error on
corrupt input*; neither injects a crash inside the import transaction, and H5 by its own scope never
touches import. All three go green while the mid-import-crash guarantee is unexercised.

**Proposed fix.** Extend H5's kill-point matrix to the **import path** (kill after pre-cleanup delete,
mid-INSERT, before hash store) and assert: the next recovery attempt either resumes cleanly or is not
blocked by a half-created wallet — i.e. atomic rollback verified, not assumed.

---

## Cross-check: A3 constraints that could pass vacuously (priority f)

| # | Constraint | Load-bearing test? | Verdict |
|---|---|---|---|
| 1 | No monolithic payload | H13, H3 | Load-bearing. |
| 2 | Canonical change detection | H3, H9 | Load-bearing **but** the exclusion list is unaudited — R5-2. |
| 3 | No bookkeeping inside backed-up state | H2 manifest, H5 | Load-bearing (manifest + construction). |
| 4 | No raw IDs/FKs on wire | H3 remap, H1 | Load-bearing. |
| 5 | Schema drift gate | H2 | Load-bearing for *travels*; blind to re-derived-value correctness — R5-2. |
| 6 | No "newest" by indexer time | H7 | Load-bearing; deep-fork/flip gaps — R5-3, R5-8. |
| 7 | Indexers trusted per contract only | H5, H6 | Partial — 429/ban and mid-recovery flip absent — R5-1, R5-8. |
| 8 | Atomic-enough broadcast/record | H5 | Load-bearing. |
| 9 | Bounded retry | H5, H9 | Load-bearing; but mempool-eviction trigger unsampled — R5-7. |
| 10 | Indexer-down != no backup | H6 | Load-bearing. |
| 11 | Atomic import | H1, H6 | **Vacuous on the crash case** — no mid-import kill — R5-9. |
| 12 | Concurrency discipline / no `let _ =` | H5 | Weakly covered; a static-analysis/lint check, not a runtime H-test, is the honest vehicle. |
| 13 | Round-trip before shipping | H1 | Load-bearing. |
| 14 | Namespaced credential storage | — | **No test; vacuous by admission** — R5-5. |
| 15 | No signing-key inference | H9 full sync, H1 | Load-bearing *iff* H9's "full sync" drives the real `?full=true` repo path that caused BS-SYNC-1; if it is a simulated op, the regression assertion is vacuous — verify during Phase 7. |
| 16 | Backup health observability | H9 | Load-bearing. |

Two constraints pass **vacuously**: **14** (no test at all) and **11** (tested only on the
no-crash path). **15** is conditionally vacuous pending how H9 implements "full sync." **2/5/7/9** are
load-bearing but have the specific escape hatches in R5-1/2/7/8.

---

## Honest statement of the meta-gap

Every H-test except the fee/latency measurements runs against a **mock generated from human-written
contracts** (D12/H16). A3's entire thesis is that this component's failures come from **the real
endpoints being weaker than the team's model of them** (M5: index lag, JSON truncation,
ARC-200-on-reject, ambiguous DOUBLE_SPEND). A mock built from the model cannot, by construction,
surface a failure the model omits. The harness is a rigorous check that the wallet matches the team's
*belief* about the chain; it is **not** a check that the belief is right. The one tier that could
test the belief — live smoke — is scoped to fee numbers and contract probing and **never runs a
seed-only recovery for real** (R5-6). That is the load-bearing honesty gap the plan should state at
the top of §5, not leave implicit.
