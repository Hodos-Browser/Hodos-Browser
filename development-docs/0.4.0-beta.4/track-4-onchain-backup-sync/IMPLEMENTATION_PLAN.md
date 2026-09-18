# Implementation Plan — On-chain Backup and Sync (delta chain + multi-device)

**Written:** 2026-08-22 (task D1). **Revised:** 2026-08-23 (task D3 — critique applied; all 11
gaps fixed, none rejected). **Revised:** 2026-08-24 — adversarial review applied (see
`ADVERSARIAL_REVIEW.md`: 36 confirmed findings, 3 refuted; every confirmed finding has a defending
edit below, each marked *(adversarial review 2026-08-24)*; new decisions D13–D15, new tests
H17–H19). D4/D5 signed off by owner 2026-08-24 — migrations REJECTED absent a forcing
cryptographic break; a follow-up adversarial review is scheduled at sprint kickoff.
**Status:** ACTIVE. README.md reconciled 2026-08-23: the §4.1 verdicts are now annotated into the
README itself.

**Inputs.** The four context docs (work-plan README, ONCHAIN_BACKUP_SYSTEM.md, the BRC draft,
DELTA_ANALYSIS.md) and the eight research reports in `research/` (A1 code map, A2 export/import,
A3 retrospective, B1 BRC digest, B2 BRC-38 compat, C1 toolbox TS, C2 toolbox Go, C3 backup cache),
all read in full. Every code/spec claim below is relayed from those reports, which verified them
against code and primary sources; claims are cited to the report that verified them. This plan
itself re-verified nothing against code — that is stated plainly in §7.

**Two standing constraints, restated:**

1. **The BRC draft follows the code.** Where this plan and the draft disagree, the plan wins and the
   draft gets updated (the update list is Appendix A). The draft is not treated as truth anywhere
   below.
2. **`go-private-backup-cache` has no license** (root; `ts-client/` alone is Open BSV v6 — C3).
   Its log *semantics* inform decisions D6/D7; its code is never vendored, copied, or adapted.

---

## 1. Goals and requirements

Numbered, each phrased so a test can pass or fail it. The test IDs (H*) are defined in §5.
Priority order is fixed: **correctness beats cost beats speed** — where two goals conflict, the
lower-numbered priority class wins (G1–G5 and G11–G12 correctness, G6–G8 cost, G9–G10 speed/UX).

| # | Requirement (testable form) | Test |
|---|---|---|
| **G1** | **Recovery = seed only.** On a fresh machine, given only the 12-word mnemonic, the wallet restores from chain and can **successfully spend**: a signed spend of (a) an HD-derived output, (b) a token/BRC-42-counterparty output passes full script verification and is accepted for broadcast. Recovered balance equals the pre-wipe balance exactly — inflation is a failure equal to loss (A3 E1-58cf9a3). *(adversarial review 2026-08-24: also proven against the live network once per release — H18.)* | H1, H18 |
| **G2** | **A lost device never loses spendable money.** At every instant in a simulated 90-day run, recovering from chain reproduces every spendable output (with its spending metadata: derivation, `custom_instructions`, `sender_identity_key`) that became spendable more than **10 minutes** earlier (the existing debounce hard cap). §3.3 changes the shipped trigger to make this true: **any** new spendable output sets the dirty flag (the shipped >= $3 event threshold, `main.rs:490`, is dropped for dirty-marking). One declared exclusion: a wallet whose balance is below the 3,000-sat funding minimum (`task_backup.rs:20`) cannot pay for a backup at all; H9 asserts the bound over every funded state and asserts the exclusion is surfaced in the health state, never silent. Zero other windows where money older than the bound is unrecoverable. | H9 |
| **G3** | **Multi-device sync works.** Two devices sharing a seed: a spend on A is visible on B before B's next spend (poll-before-spend); concurrent writes produce a *detected* fork, never silent divergence; both devices converge to one tip within ≤ 2 re-read/re-write rounds; no accepted double-spend **on the backup chain**; no lost row **or lost field** (field-level union of both devices' changes survives, including changes committed by one device that the other has polled but not yet applied). *(adversarial review 2026-08-24, R2-6/R2-3/R4-9: scope corrected — the backup chain cannot serialize **user** spends; two seed-sharing devices CAN race the same wallet UTXO inside the debounce+build+lag window. For wallet-UTXO races the guarantee is detection + bounded heal via the verify quorum (D14/D15) and a specified, tested loser-device end state — never the pre-review implication that the race cannot happen.)* | H4, H19 |
| **G4** | **No silent data loss on recovery.** The diff between original and recovered DB is exactly the documented tier-3 exclusion list — nothing else. Every table **column** in the live schema is classified {travels, re-derived/re-fetched, excluded-with-reason}; an unclassified column fails CI. (Closes the A1 §2 column-loss class: `bundled_scope_grant`, `settings` defaults, `price_usd_cents`, `outputs.confirmed`, `peerpay_received`, V18 permission tables.) | H1, H2 |
| **G5** | **Failure is visible and convergent.** Any backup failure surfaces in a user-visible health state within one monitor cycle; retries back off and reach a quiescent error state — never an unbounded rebuild-and-broadcast loop (A3 E11). Recovery fails **closed** on indexer errors: "indexer unavailable" is never reported as "no backup exists" (A3 BS-C2). *(adversarial review 2026-08-24, R2-4/R4-6/R5-5: the health surface is broadened beyond broadcast bookkeeping to four inputs — (1) last-success/tip/consecutive-failure as before; (2) **tip confirmation state**: tip broadcast but unconfirmed past a declared horizon = red + rebroadcast, so an evicted terminal token cannot stay green on an idle wallet; (3) **chain divergence**: any poll that finds a foreign tip that is not an ancestor/descendant of the local tip sets a user-visible flag — the dirty flag diffs a local baseline and structurally cannot see divergence; (4) **key-identity mismatch**: backup address not derived from the loaded wallet identity = red (the E12 observability half). H9's MTTD is measured off this product surface, not the harness's own oracle.)* | H5, H6, H9 |
| **G6** | **No-change means no broadcast.** Over a 90-day soak with background monitor churn, wall-clock passage, and restarts, zero backup transactions are broadcast when no logical wallet change occurred. (Kills the E7/E8/BS-M1 false-dirty class.) | H3, H9 |
| **G7** | **Cost is bounded by activity, not by wallet size or age.** A delta's size is a function of the changes since last backup; the snapshot cadence follows the §3 rule; yearly cost per profile lands within **numeric ceilings frozen after H8a** (until they are frozen the §3.4 table is descriptive only — judging H8/H9 runs against a table those same runs produce would be circular, so the frozen ceilings, not the table, are the pass/fail bound), and profile C (200 image ordinals) costs within 2× of profile A once inscription bytes are stripped. No payload is broadcast above its declared size class; nothing is broadcast above the hard cap — the build fails closed instead (today the live wallet broadcasts 431 KB against its own 200 KB warning with no cap; A1 §3d/§4). | H8, H9, H12 |
| **G8** | **Every re-fetchable byte stays off-chain.** Inscription/locking-script bytes of held tokens, raw txs of mined txs, merkle paths, headers are excluded from payloads and re-hydrated on recovery, byte-identical. | H13 |
| **G9** | **Recovery and pre-spend polling are fast enough, measured.** Recovery from 1 snapshot + 20 deltas completes in bounded fetches (≤ N+1, measured); pre-spend poll p95 latency is measured and the sync-vs-optimistic decision (README Open Q1) is made from that number, not a guess. | H7, H11 |
| **G10** | **Chain robustness.** 50 junk tokens at the backup address, truncated indexer responses, corrupted payloads, missing parents: recovery skips/reports (typed error, last-contiguous-token restore + explicit break report) and **never panics** (the unchecked `raw_tx[pos..pos+8]` slice-index class, A1 §5). *(adversarial review 2026-08-24: "junk tokens" now includes adversarial litter — attacker P2PKH dust markers and crafted plaintext headers, not only random PushDrop junk — and 1–2 block reorgs after a raw_tx strip, H17.)* | H6, H10, H17 |
| **G11** | **No format change ever breaks an existing user's recovery.** Every payload version ever broadcast to mainnet stays decryptable and recoverable forever: recovery tries the current decode first, then every legacy decode (today: the shipped headerless gzip→AES-GCM envelope under `SHA-256(master_privkey ‖ "hodos-wallet-backup-v1")`), in declared order, before reporting failure. Any KDF/envelope/header change ships only together with its fallback path and an H15 fixture proving a pre-change token still recovers. *(Owner directive 2026-08-24: "We must have this legacy fallback so that we do not break users wallets.")* | H15 |
| **G12** | **No external lookup without a written, tested contract.** Every external endpoint the wallet consumes (WoC unspent/history/tx/proof, ARC, GorillaPool, any future secondary indexer) has a checked-in contract: exact URL + params, exact semantics of what is and is not returned (unspent vs history vs bulk; pagination and its caps; index-lag and unconfirmed-visibility behavior; error shapes), a recorded fixture, and a conformance test. No call site outside the contract allowlist (CI-enforced); changing a lookup means changing the contract test *first*. A secondary provider enters only via shadow mode (D12). *(Root: backup has completely broken wallets because call semantics were assumed, not known — the 2026-04-11 incident class; owner directive 2026-08-24.)* | H16 |

Non-goals this sprint: solving seed-handover between vendors (separate Portability/Handover BRC —
README Open Q7; we only keep the cheap gates: liveness-check-before-first-spend after recovery,
recovered-vs-not report, foreign-`device_id` warning), and activating foreign BRC-38 imports
(parse/validate/preserve only — B2 §5).

---

## 2. Adopt / adapt / diverge decisions

On record, with the research grounding. "PREFERRED OUTCOME" from the task — chunks aligned with
toolbox so one wallet feeds both rails — is achieved **at the row-form and semantics level, not the
container level**; the two concrete blockers to literal container reuse are named in D2.

### D1 — Snapshot payload vs BRC-38/39: **ADAPT — strict BRC-38 document inside an envelope (item-0 gate option (b) now, pursuing (a))**

- **What:** token payload = `{ "document": <strict BRC-38 JSON per 0038.md>, "extensions": { "hodos/wallet": {current_index}, "hodos/baskets-ext": …, "hodos/permissions": …, … } }`, gzip → AES-GCM as today. Submit the two-paragraph BRC-38 amendment (`contains` declaration + preserve-unknown rule + a §5.4 sentence blessing the stripped profile); if it merges, `extensions` moves inside the document with zero data migration (B2 §4).
- **Why:** B2's compat table found the **small-gap branch**: all 13 BRC-38 tables + `user` + `sourceStorage` exist in our schema with zero missing columns; every diff is a rename, an encoding transform, or one of 16 extra columns; the only tier-1 orphan is a single field, `wallets.current_index`; the only genuine shape transform is `proven_tx_reqs.history` (map → notes-array). Option (c) native-for-now "is strictly worse than what we already have" (B2 §4).
- **Diverge, declared:** our on-chain document is a **stripped profile** (empty `merklePath`, capped `history`, no spent-output `lockingScript`, no inscription bytes after Phase 3). Conformance rests on reading §5.4 "absent in source" as covering stripped-and-refetchable data; our BRC and the amendment PR must declare this profile explicitly rather than rely on the reading (B2 §3.4).
- **Never travels:** `settings.backup_hash` / `last_backup_at` (per-device baseline; importing them suppresses the next backup — B2 §2.3).

### D2 — Delta format vs toolbox SyncChunks: **ADOPT row forms and merge semantics; DIVERGE on container. Two concrete blockers to literal reuse.**

- **Adopt:** delta = per-table arrays of **BRC-38 portable row forms** (base64 binary, strict ISO timestamps, omitted nulls, natural-key identity) for the 13 shared tables, citing **BRC-40** for merge/replay semantics: idempotent upsert, natural-key `mergeFind` (e.g. transaction by `(userId, txid)`), inclusive watermark, per-row last-writer-wins — the same engine `mergeBRC38` already drives via a synthetic SyncChunk (C1 §2.1, §5). Entity dependency order = BRC-40's 12-name order **as specified**, not either implementation's quirks (C2: Go's `AllEntityNames` order and `proveTxId` JSON tag are bugs against the spec; the conformance vector corpus itself has authoring errors — spec text + TS implementation are the ground-truth pair).
- **Blocker 1 (fatal to literal reuse): no delete records.** BRC-40/SyncChunk has no deletion mechanism at all — no merge path removes a row; `outputs` and `transactions` have no tombstone (C1 §1.4). Our cost model rests on strips; replaying literal SyncChunks resurrects everything the strips shed. Our delta therefore carries a per-table `deletes` array (natural keys), an **extension beyond** BRC-40, not a profile of it.
- **Blocker 2 (fatal to literal reuse): our stripped rows are not valid toolbox rows.** `merklePath`, `provenTxReq.rawTx`/`inputBEEF` are notNullable columns and BRC-38-required binary fields; `checkEntityValues` refuses nulls in chunks (C1 §5). A "SyncChunk" without them is false advertising.
- Also diverged, with reasons: SyncChunk's `number[]`-per-byte binary encoding (bulky on-chain — C1 §1.3); consumer-held resume state (`since`/offsets/idMap) is meaningless on a broadcast chain — our ordering is `seq` + `parent_txid` (C1 §5.4); BRC-40 chunk requests are not self-authenticating — authorization is delegated to the transport session (Go bound identityKey to the authenticated peer only in Aug 2026, C2 addendum §3) — on-chain we have no session, so the AES-GCM tag on a seed-derived key is our per-token authentication (draft Security §5, confirmed sound).
- **Merge arbiter carried in-band** *(adversarial review 2026-08-24, R4-9)*: D2 adopts per-row
  LWW, but toolbox's LWW arbiter is `updated_at` — which §3.3 rightly excludes from the payload
  (monitor churn). Without an in-band arbiter, same-row concurrent edits on two devices would be
  decided by re-write race order, invisibly to H4's row-presence check. Decision: every delta row
  carries a **per-row logical version counter** (monotonic, incremented on each local edit,
  max+1-on-merge), and per-row LWW arbitrates on that counter, never on wall clocks. `updated_at`
  stays excluded. H4 gains a same-row concurrent-edit case with a field-level assertion.
- **Result:** a toolbox-side consumer of our deltas is an *adapter* (row forms already match; add delete handling), not a rewrite. "One wallet feeds both rails" holds — see D3.

### D3 — Delta transport vs `go-private-backup-cache` log semantics: **ADOPT two ideas, MIRROR one field, DIVERGE where chain ≠ server**

- **The blob is opaque** — raw octet-stream, "Nothing in this package interprets blob contents", zero toolbox/BRC-38/40 references repo-wide (C3 §1). So their rail imposes **no constraint** on our chunk format: whatever we emit feeds both rails unchanged. Their blob *encryption* is unspecified (prose "counterparty self" only, no protocol string, no framing — C3 §3); if chunk-level commonality ever matters, **our BRC supplies the missing content spec** — the dependency points from them to us.
- **Adopt (semantics only, no code):** (i) the compaction safety rule — their server refuses to delete the newest two generations so a failed compaction never strands a user; our structural equivalent is that snapshots link into the chain rather than truncating it — cite as design evidence in the BRC. (ii) Cross-implementation test vectors pinning byte compatibility (their authproof vector pattern) — we produce vectors for our token header + payload envelope.
- **Mirror:** carry **`prev_payload_sha256`** (SHA-256 of the previous chunk's ciphertext) in our payload envelope, so the identical encrypted chunk chains identically on their HTTP log (`prevSha256`) and on our chain (where `parent_txid` remains the structural link). Decision lands with the Phase 4 header. Our `device_id` maps to their per-device `deviceId` partition key for free.
- **Diverge:** their per-device logs *partition around* the multi-writer problem and leave merge unspecified; our single shared chain makes merge the protocol — that is the point (C3 §5). Their GDPR-erase route is structurally impossible on-chain: superseded ciphertext is readable forever in spent history. The BRC Security section states this plainly and it raises the weight of padding (Phase 6) and any future key-rotation story.
- House rule: their "zero-knowledge" label is never repeated; we write "the server stores only ciphertext under a pseudonym" (C3 flag 2).

### D4 — Encryption KDF: **draft follows code**

Shipped code derives the payload key as `SHA-256(master_privkey || "hodos-wallet-backup-v1")`; BRC-42
is used only for the backup address/signing keypair (A1 §3a — VERIFIED against `backup.rs:960-967`).
The draft's §3 (BRC-42, level 2, "wallet-backup") describes something that does not ship. Decision:
**v1 of the chain format specifies the shipped KDF** (it is sound: 256-bit secret input, domain
separation string). The token header's `version` byte governs any future KDF change; a migration
would require the legacy KDF as decrypt fallback for every existing on-chain backup and is out of
scope. Draft §3/§4 rewritten (Appendix A).

**Hardened 2026-08-24 (owner directive):** the legacy decrypt fallback is not merely a migration
precondition — it is **permanent** (G11). No release may drop the ability to decrypt any token
version ever broadcast to mainnet; H15 pins this with checked-in binary fixtures that are only ever
appended to, never regenerated.

> **OWNER DECISION 2026-08-24 — ACCEPTED (Matt): KDF migration REJECTED absent a forcing
> cryptographic break.** Standing principle attached to this and every recommendation below:
> **decide by what is best, not what is most expedient** (owner) — accepted here because the
> review showed best and expedient coincide: migration adds compat risk and zero security.
> Provisional only in that the sprint-kickoff re-review may re-open it with new technical
> evidence. *(Full argument: ADVERSARIAL_REVIEW.md §4.)*
> The review (R3, skeptic concurring after independent re-verification) recommends hardening this
> decision from "KDF migration out of scope / on the blocker list" to **"KDF migration REJECTED
> absent a forcing cryptographic break."** Grounds: the shipped KDF and the draft's BRC-42
> self-derivation are *equally* sound — BRC-42 with counterparty = self is a deterministic
> function of the master key alone, adding no entropy and no security over a domain-separated
> SHA-256 of the same 256-bit secret — so migrating is pure churn with real compat risk. The one
> hygiene note that survives (R3-5: key derives from the raw BIP32 root, not a hardened child) is
> shared by **both** options, is not exploitable (one-way primitives), and is **not fixed by
> migrating** — recorded here as out-of-scope-unless-forced precisely so it cannot later be used
> to re-justify a migration. **Migration sketch if ever forced:** new `version` byte; new-KDF
> encrypt for all new tokens; legacy KDF retained as permanent decrypt fallback (G11); H15 fixture
> appended before first new-format broadcast; no address change implied.

### D5 — Backup address derivation string: **keep as shipped; document the deviation; migration rejected** *(owner 2026-08-24)*

Code uses invoice `"1-wallet-backup-1"`: security level 1 (draft says 2) and a hyphen inside the
protocol ID, which BRC-43 forbids (letters/numbers/spaces only) — B1 §4.3, settled against code by
B2 (disagreement 3). Changing the string changes the derived backup address = every existing wallet's
chain moves = a migration event needing its own plan (old chain must be found or re-anchored).
Decision: **do not silently fix.** v1 keeps the shipped string, documented in the BRC as a legacy,
non-BRC-43-conformant derivation with the exact literal given; a conformant derivation
(`2-wallet backup-1`) is specified for new implementations of the standard, with the note that the
two produce different addresses. Address migration: **REJECTED absent a forcing cryptographic
break** (owner decision 2026-08-24; previously “named blocker for a later sprint”).

> **OWNER DECISION 2026-08-24 — ACCEPTED (Matt): address migration REJECTED absent a forcing
> cryptographic break** (same best-not-expedient principle as D4; the review showed migration
> would *create* the R3-1 money-loss surface to buy a cosmetic conformance fix). Both carve-outs
> stand: the conformant `2-wallet backup-1` derivation stays documented for NEW implementations,
> and the fail-closed dual-address rule below binds any future forced migration. Sprint-kickoff
> re-review may re-open with new technical evidence. *(Full argument: ADVERSARIAL_REVIEW.md §4.)*
> The review recommends converting "address migration is a named blocker for a later sprint" into
> **"address migration REJECTED absent a forcing cryptographic break."** The clinching finding is
> **R3-1 (CRITICAL, conditional):** any migration creates a dual-address recovery surface whose
> downgrade failure mode — indexer lag or litter at the v2 address reads as "no v2 backup", the
> legacy chain's final token authenticates perfectly — **silently restores pre-migration state and
> loses post-migration funds**, and H15 (decrypt-compat only) cannot catch it. The thing migration
> would fix — BRC-43 non-conformance — is cosmetic: BRC-42 HMACs the literal invoice bytes without
> parsing BRC-43 structure. Two carve-outs preserved either way: (1) the BRC keeps documenting the
> conformant `2-wallet backup-1` derivation for NEW implementations of the standard (interop
> without ever moving an existing wallet's chain); (2) **binding rule if a break ever forces
> migration:** v2-aware recovery MUST fail closed when the v2 address yields nothing but a legacy
> chain exists — never silently restore legacy — and the migration gets its own plan + harness
> phase with a dual-address-under-lag test.

### D6 — Recovery discovery: **address-history enumeration + decrypt-before-trust + parent-txid chain walk; recency by child-existence** *(rewritten, adversarial review 2026-08-24 — R1-01, R1-02, R1-03, R1-06, R3-2)*

Today recovery queries only WoC `unspent/all` — spent (superseded) tokens are invisible, and the
known stale-index race can silently restore a superseded backup (in-code TODO citing the 2026-04-11
incident; A1 §3c). Under deltas, old-but-needed tokens are spent by design, so the unspent query
cannot feed recovery at all. The escape hatch already exists structurally: input 0 of every backup
tx is the previous token's outpoint, so a walk by txid needs no address index (A1 §3c).

The 2026-08-23 version of this decision (single bootstrap marker from the unspent index +
spent-status cross-check via `reconcile::check_outpoint_spent`) was **broken four ways** by the
adversarial review and is superseded:

- **The spent-status cross-check is inert** (R1-01, CRITICAL, live-probed 2026-08-24): GorillaPool's
  ordinals `/txo/{txid}/{vout}/spend` 404s for plain P2PKH outpoints → `NoSignal`; WoC `/spent`
  404s for unspent-or-unindexed → `NoSignal`; `decide_spent(NoSignal,NoSignal) = Unknown`. The
  "cross-validated" check adds zero signal in exactly the stale-index window it exists for.
- **A healthy tip yields `Unknown` by construction**, and the old text branched on only two of the
  three values (R1-03, HIGH) — either horn (restore / refuse) is wrong.
- **Single-pick bootstrap is a one-dust-output DoS** (R1-06, HIGH): `max_by_key` + abort-on-decode-
  failure at a public P2PKH address (handlers.rs:14765-14815).
- **A parent-only walk cannot jump a mid-chain hole** (R1-02, CRITICAL): T(k−2)'s txid lives inside
  T(k−1); one unservable link orphans the nearest snapshot behind it.

**Decision (normative):**

1. **Bootstrap = full address history**, spent + unspent (per the H16-contracted history endpoint),
   never the unspent list alone. Every candidate marker is enumerated. (Restores the DELTA_ANALYSIS
   rule D6 had narrowed away.)
2. **Decrypt before trust:** *no plaintext header field (`seq`, `parent_txid`, `kind`, anything)
   may influence tip selection, recency, or walk order before the token's AES-GCM tag verifies
   under our key* (R3-2). Candidate iteration is **newest-that-decrypts**: sort candidates
   newest-first, skip (and count) every candidate that fails decode/decrypt, never abort on one
   failure. Junk and adversarial litter are thereby inert.
3. **Tip selection / recency = child-existence over the decrypted set:** a marker is superseded iff
   some *decryptable* marker names it as parent. The tip is the decryptable marker no decryptable
   marker names as parent. The three-valued spent-status oracle is demoted to a corroborating
   signal with an explicit table: `Spent` on the candidate tip → index gave us a stale history or a
   fork — re-fetch history, retry, never restore stale; `ExplicitUnspent` → proceed;
   `Unknown`/`NoSignal` → **decide from child-existence alone** (the modal healthy outcome; neither
   restore-blindly nor brick).
4. **Walk with reassembly:** walk `parent_txid` (header) / input-0 (pre-header legacy) backward;
   a missing link is first looked up in the already-enumerated history set by txid (holes in the
   *tx-serving* endpoint are healed by the *history* set and vice versa). If the chain is still
   broken: restore is offered **only** from a state that is a complete snapshot + contiguous
   deltas; **deltas-without-base is a fail-closed typed error, never a restore offer**, and a
   partial older-than-tip restore is presented as exactly that, requiring explicit user
   acknowledgment before any write is permitted on top of it.
5. **Two-oracle requirement forwarded to D12:** the secondary provider must actually index
   plain-P2PKH spent status / address history, or it cannot serve this decision (the GorillaPool
   ordinals API cannot).

(A3 constraints 6, 7, 10. Tests: H6 mid-chain-hole + deltas-without-base; H7 three-branch recency
table; H10 adversarial litter incl. crafted headers and outranking dust.)

### D7 — Broadcast/record ordering: **persist intent before broadcast (Fix B), resolving the live doc disagreement**

`FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md` says the crash-window fix is needed; the same-day
`FOLLOWUP_RECORD_BEFORE_BROADCAST_TOKENS.md` says broadcast-first is "correct as-is"; no
implementing commit exists (A3 §6). The followup's own argument holds only while every backup
output is self-derivable, and it "explicitly flags that record-before-broadcast becomes mandatory
the day we hold non-self-derivable outputs" (A3 E9). Decision: Phase 4 persists a durable
**backup intent record** before broadcast; read-before-write additionally heals ghost tokens
structurally (a ghost on chain becomes the parent the next write adopts). "Correct as-is" is
retired. (Constraint 8.)

**Extended (adversarial review 2026-08-24 — R4-1, R4-4, R1-04, R2-5, R4-10):** the one-sentence
version above was defeated four ways (ambiguous broadcast outcome; startup-only consumption;
missing funding inputs; no heal for the inverse ghost). The full decision:

- **Intent record contents:** planned token outpoint, seq, parent txid, payload hash, **and every
  input outpoint the backup tx reserves — funding inputs included** (R2-5), plus the **complete
  signed raw tx bytes** (which fix the txid).
- **Broadcast outcome is three-valued: OK / REJECTED(reason) / UNKNOWN** (R4-1, CRITICAL — the
  accepted-but-response-lost case is the 2026-04-11 mechanism). On UNKNOWN: **no rollback, no
  input restore, no rebuild**. The intent record is retained and the cached identical raw tx is
  re-broadcast (idempotent — the same txid annihilates any would-be same-parent conflict) until
  the chain renders a verdict. Only REJECTED(permanent) triggers rollback.
- **Reconcile is a decision table, not a sentence** (R4-4), over (intent present?) ×
  (txid visible on chain/mempool?) × (parent still tip?) × (cached raw tx present?): txid visible
  → complete the record, consume intent; not visible → **re-broadcast the cached raw tx** (safe in
  both the crashed-before-broadcast and broadcast-not-yet-indexed worlds — indistinguishable
  inside the 30 s–5 min lag window, so the action must be safe in both); rejected-stale-parent →
  rebuild from the current tip. **Multiplicity rule:** multiple live intents (double crash) are
  processed oldest-first, each to a terminal state, before any new build.
- **Consulted on every trigger path** (R1-04): the decision table runs before *any* backup build —
  startup, 3-hour net, receive-trigger — not only at startup. A live intent always blocks a new
  build.
- **Inverse ghost heal** (R4-10): recorded parent absent from chain (eviction/reorg) →
  re-broadcast the cached raw tx first; only if the chain rejects it, rebuild from the last
  chain-visible ancestor and mark the baseline dirty. Symmetric to the ghost-on-chain heal.

(Tests: H5 extended — kill points crossed with active index lag, UNKNOWN-outcome fault,
double-crash multi-intent row, non-startup trigger row, ARC 200-with-different-txid bound to the
record-local-txid assertion; H17 for the eviction/reorg legs.)

### D8 — BRC numbering: **request a new number; never claim 38/39/40**

BRC-38 (User Wallet Data Format), BRC-39 (encryption extension), BRC-40 (User Wallet Data
Synchronization) are **written, merged registry documents** by Ty Everett, with two implementations
and a conformance corpus — not reservations (B1, C1 flag 7, C2). The draft's §9 "BRC-38/39/40
(reserved)" and Open Q1 ("should we claim BRC-40?") are answered: no. Our draft is BRC-XX until
assigned.

### D9 — `sync_states` (README Open Q5): **the chain replaces it for our devices; the table is carried, not activated**

`sync_states` has 0 rows in both live DBs and nothing in the codebase writes it (A1 §2 item 22).
Cross-device state under this design is the chain itself (tip + per-device positions derivable).
The table stays in the payload for BRC-38 conformance; §10's actual words — importers "MAY treat
the following as operationally sensitive and choose whether to activate them immediately after
import" (0038.md §10, verified at c1d12f2) — describe exactly our posture.

### D10 — Terminology and claims discipline

"Toolbox sync IS BRC-38/39" (the item-0b premise, quoting Ty's meetup statement) is imprecise:
sync is BRC-40 moving raw rows; 38/39 are file formats; the shared merge engine is the overlap
(C1 §2.1). The BRC and any reply to Ty phrase it that way. BRC-38's own "complete" export already
omits the user-scoped `action_batches` table — their spec has the same drift problem our
amendment addresses; cite diplomatically (C1 flag 4).

### D11 — Encryption envelope vs BRC-39: **DIVERGE — the on-chain container is not BRC-39 and never claims to be**

BRC-39 is a merged registry document (magic `WDAT`, Argon2id + AES-GCM around a BRC-38 document —
B1; README item-7 survey). Our shipped on-chain envelope is gzip → AES-GCM under
`SHA-256(master_privkey || "hodos-wallet-backup-v1")` (D4): different KDF, different framing, no
`WDAT` container, seed-derived rather than password-derived. Decision on record: the chain
container **diverges** from BRC-39 — grounded in D4 (draft follows code) and D8 (never claim
38/39/40) — and the BRC states it plainly, which turns A2's Phase 8 landmine ("container is not
BRC-39") into a documented property instead of a surprise. `.brc39` *file* interop is Phase 8
item-7 work through the toolbox reference implementation and does not change the chain container.

### D12 — External data providers: **contract-first; secondary indexer only in shadow mode** *(owner directive 2026-08-24)*

- The §5 mock chain is generated from the same endpoint contracts G12 requires, so every test
  exercises the *documented* semantics, not an idealized indexer.
- Freezing WoC semantics into contracts is the **first harness task (Phase 1)**:
  `address/{addr}/unspent/all` (what "unspent" means inside the 30 s–5 min index-lag window;
  whether unconfirmed outputs appear; pagination/row caps on bulk pulls), address *history*
  (depth limits, ordering), `tx/{txid}/hex`, and the TSC proof endpoint. Where the docs are
  ambiguous, the contract is settled **empirically** (probe, record the fixture) before any code
  depends on the behavior. The current unspent-only discovery is probably unspent *for a reason*;
  nothing about the lookups changes until the contract for the replacement is proven (owner).
- A **secondary lookup provider** (owner proposal 2026-08-24 — exact service to be confirmed
  before Phase 1; candidate class: a second indexer API or self-hosted node) is wanted for D6's
  recency cross-check (two-oracle staleness detection) and G5 availability — but it enters only
  through the G12 pipeline: identical contract suite green → **compare-only shadow mode** logging
  every divergence from the primary over a soak period → promotion only after the divergence
  analysis is on record. No call-site change before that.
- **Hard selection requirement** *(adversarial review 2026-08-24, R1-01/R5-8)*: the secondary MUST
  index **plain-P2PKH spent status and address history** — the live probe showed the GorillaPool
  ordinals API 404s plain P2PKH outpoints, which is why the shipped "cross-validated" recency
  check is inert for backup markers. A candidate that cannot answer for our marker outputs is
  disqualified regardless of other merits. The mock models the secondary with **correlated lag**
  (shared-source delay component, not independent faults) and **mid-recovery answer flips**, so
  H16's promotion gate exercises the failure shapes a real second indexer actually has.

### D13 — Producer integrity: pinned baseline, merge-before-snapshot, consistent reads *(new, adversarial review 2026-08-24 — R4-3, R2-3, R4-11)*

Three silent-loss holes shared one root: the plan never said what state the producer's outputs are
a function of.

- **The baseline is the exact collected state that was broadcast** (R4-3 — live shipped bug: step
  13 re-collects at `ref_ts=now` *after* broadcast, handlers.rs:14077-14094, so any mutation
  landing in the multi-second broadcast window enters the baseline without ever entering a
  payload, and the wallet reports `Skipped`/green forever). The baseline is captured at build
  time, from the build's own collection, and is **never re-collected**. One sentence; closes a
  standing silent-loss bug on the shipped path *and* the Phase 5 inheritance of it.
- **Merge-before-snapshot** (R2-3, CRITICAL): the snapshot producer MUST read-to-tip and **apply
  every unapplied remote delta before serializing**. `adopt_onchain_backup` adopting outpoints
  without applying payloads (handlers.rs:13149-13228) does not satisfy this. A snapshot whose
  parent is not the producing device's *fully-applied* tip is a build error (fail closed) — a
  stale-baseline snapshot is a linear, valid-looking chain extension that silently reverts other
  devices' committed deltas and no fork detector can see it.
- **Consistent reads** (R4-11): the changeset/snapshot producer reads its entire input from a
  single consistent DB snapshot (one transaction / one lock scope). Today's torn-read safety is an
  accident of the single-mutex topology (main.rs:423-424) and the services facade will change that
  topology; this sentence makes it an invariant a refactor must preserve.

(Tests: H3 mutation-during-broadcast + concurrent-produce interleave; H4
snapshot-after-missed-delta with field-level union assertion.)

### D14 — Destructive-write authority and retention: nothing deletes or restores on a timer; bytes strip only at depth *(new, adversarial review 2026-08-24 — R4-2, R4-5, R4-7, R4-8, R2-6)*

The delta chain **immortalizes** every DB write: a wrong monitor deletion, once emitted as a
delete record, replays on every device and every future recovery. So write authority must be
specified, not inherited from shipped monitor behavior:

- **`abandoned-unbroadcast` state** (R4-2, CRITICAL): aging out a `noSend`/counterparty-held req
  must NOT restore its inputs or delete its outputs — no oracle can distinguish "dead" from
  "counterparty holds it and will broadcast later"; unanimous absence is not evidence of death.
  Age-out drops the raw bytes from the payload and marks the req `abandoned-unbroadcast`;
  **inputs stay encumbered** until a chain-visible conflicting spend (or the tx itself) appears.
  H14's age-out clause is rewritten to assert exactly this. Supersedes the §3.3 "aged out to
  `failed`" wording.
- **Confirmation-depth strip** (R4-5): raw_tx bytes leave the payload only at a **declared
  confirmation depth** (constant in the BRC; default ≥ 6 confs — provisionally accepted by owner
  2026-08-24, to be confirmed with reorg-depth data at sprint kickoff, best over expedient), not at proof-existence = 1 conf
  (backup.rs:449-457). A proven→unproven transition (reorg) is defined and re-inflates the
  payload. The mock gains an un-mine operation; H17 covers it.
- **Delete-emission allowlist** (R4-7): the Phase 5 producer emits a delete record **only** for
  named, chain-corroborated state transitions on an explicit allowlist. Timer-only transitions
  (TaskFailAbandoned et al.) may never emit deletes or input restores into the chain. H3 asserts
  every delete record carries an allowlisted cause.
- **Limbo bound** (R4-8): after N cycles of oracle-quorum disagreement, a req's raw bytes move to
  local-only retention with a health flag; the payload carries a stub (G7 protected). H14 asserts
  the bound.
- **Quorum means quorum** (R2-6 residual): the verify path's 6-hour promote-to-confirmed-
  regardless timer is removed; promotion requires the quorum, with the quiescent-error state (G5)
  as the no-quorum outcome.

### D15 — Multi-device ordering: the chain arbitrates; tie-break named now; recovery order is parent-topological *(new, adversarial review 2026-08-24 — R2-1/R2-2/R2-7 residuals, R2-6)*

The refuted findings still yielded normative hygiene worth writing down:

- **The chain is the arbiter.** Same-parent writes conflict on the parent token's outpoints; the
  network accepts at most one. Fork resolution = loser re-reads and re-writes on the survivor.
  The device-side tie-break exists only for the transient both-unconfirmed window and is named
  **normatively now** rather than derived circularly from H4: **lower `device_id` defers**
  (backs off and re-polls before re-writing). H4 *validates* this rule; it does not decide it
  (Open Q2 is answered subject to H4 falsification, not left open).
- **Recovery order is parent-topological** (R2-7 residual): replay follows the `parent_txid`
  chain; `seq` is assigned as parent's seq + 1 at build time and is a **sanity check only** — a
  linear chain with non-monotonic seq is a typed error, never re-sorted by seq.
- **Dual-genesis** (R2-1 residual): two concurrent first-ever backups share no parent and both can
  land. Rule: on discovering a second genesis under our key, the older-block genesis (tie: lower
  txid) is canonical; the other device re-writes its state as deltas on the canonical chain.
  Phase 7 implements; H4 gains a dual-genesis round.
- **The backup chain is not a spend serializer** (R2-6): stated plainly here so no later design
  leans on it — user-UTXO races between seed-sharing devices are possible by construction; the
  defense is TaskVerifyDoubleSpend's independent verification (retained, see §4.2 row 7) plus a
  specified loser end state (H4 asserts the loser's DB: losing tx marked failed, double-spent
  inputs marked spent-by-other, no output deletion cascade).

---

## 3. Trigger and cadence reassessment

### 3.1 The measured baseline (replaces the draft's estimates where data exists)

Real numbers from A1 §4 (live production DB, read 2026-08-22 — the only measured payload data any
report found; the repo has no size fixtures and `test_onchain_round_trip` uses a near-empty payload):

- **Live payload: 431,476 bytes** (PushDrop locking script on-chain today) — **2.2× the code's own
  200 KB warning**, broadcast anyway because no hard cap or chunking exists (A1 §3d).
- Decomposition — **two A1 passes, quoted as two passes** (A1 never measured the combination):
  morning pass — **3** stuck unconfirmed transactions' raw_tx, 532,846 raw bytes → ~291 KB gzipped
  share ≈ **60%** of the payload; evening v2 pass — **11** stuck unconfirmed txs, 685,868 raw
  bytes, **gzip share not measured** (from the larger raw bytes the share is at least the
  morning's ~60%, *est.*; direction: growing). The structured payload without the stuck raw_tx
  gzips to **~165 KB** (morning pass). Dev wallet: **61 KB**.
- 114–115 `backup-%` transactions exist in the production DB — 33% of the transactions table is
  backup lineage. That their actual paid fees can be extracted is **INFERRED — this plan's own
  claim, not relayed from A1** (A1's NOT-checked list says runtime fee rates were not checked; its
  figures assume 1 sat/KB): fee = total input value minus total output value, which needs the
  prev-out value of every input of all 114+ txs, i.e. every parent present in
  `parent_transactions`. H8a carries a fallback for missing parents (§5) and, if it succeeds,
  replaces fee-rate guesses.
- Strip effectiveness (code's own rules, simulated): transactions 228→86, outputs 866→717,
  addresses 312→63 kept.

### 3.2 Deltas-per-snapshot rule (0.5× / 20 deltas / 16 KB): **kept, provisional, with one measured observation**

Evaluated against the real data: with a 431 KB snapshot the 0.5× ratio allows ~215 KB of deltas —
at the draft's *estimated* 1–2 KB per delta, the **20-delta count triggers first by ~5–10×**; the
same holds at the post-cleanup ~100–165 KB snapshot size and at the dev wallet's 61 KB. So for
every wallet we have measured, the ratio clause is nearly inert and the count clause governs
replay/fetch cost, which is the right dominant control. **But no delta has ever been produced —
delta sizes are estimates.** The rule therefore stays **provisional** exactly as specified, and the
test that settles it is **H3 + H9**: the property/soak runs produce the measured delta-size
distribution and observed deltas-per-snapshot per profile; the rule's constants are re-derived from
those numbers before the BRC's §7 is frozen. The rule's **third leg** — a single delta whose
compressed size would exceed ~16 KB forces a snapshot instead (DELTA_ANALYSIS.md §3) — is
provisional on exactly the same footing: no delta has ever been produced, so none has ever
approached the cap, and H3's measured delta-size distribution settles the 16 KB constant the same
way it settles the ratio and the count. (This is the honest answer the task demanded: real
snapshot data exists, real delta data does not yet.)

Aging-out rows and the rule: time-tiered strips apply **at snapshot construction only** and are
evaluated at a pinned reference timestamp (the shipped `ref_ts` trick, A1 §3e). **Deltas never emit
age-based deletes** — a row aging past 7/30/60-day thresholds is not a wallet change and must not
make a delta dirty (A3 E8). Delete records in deltas represent only genuine state transitions.

### 3.3 Triggers, polling, and what the dirty flag becomes

- **Changed from shipped (forced by G2):** the event trigger's ≥ $3 USD threshold (`main.rs:490`)
  is removed for dirty-marking — **any new spendable output sets the dirty flag**. The 3-min
  debounce and 10-min hard cap are kept, so the G2 staleness bound holds for every receive, not
  only ≥ $3 ones. The 3,000-sat funding minimum (`task_backup.rs:20`) stays — a backup costs sats
  to write — and is G2's one declared exclusion, surfaced in the health state, never silent.
  **Kept as shipped:** 3-hour periodic safety net, pre-delete backup (A1 §1a).
- **Poll before any spend stays** (task non-negotiable; BRC §7.2). Whether the poll is synchronous
  or optimistic-with-conflict-check is decided by H11's p50/p95 numbers, not in advance (README
  Open Q1). Poll on timer additionally.
- **The dirty flag becomes the changeset-emptiness check.** Today: SHA-256 of the whole gzipped
  payload vs `settings.backup_hash`. Under deltas: the producer diffs current collected state
  against the last-backed-up state and is "dirty" iff the changeset (upserts + deletes) is
  non-empty. Requirements carried over from the retrospective (constraint 2): comparison runs over
  **canonical row content** — explicit ORDER BY on every query (already fixed once, 30c6c89; H2
  guards it), pinned `ref_ts`, and **volatile bookkeeping columns (`updated_at` churn from monitor
  tasks) excluded from the dirty decision** — on-chain ordering is `seq`/`parent_txid`, not
  `updated_at`, so excluding it is safe here even though toolbox LWW uses it (D2); same-row
  concurrent-edit arbitration instead uses the per-row logical version counter carried in deltas
  (D2 arbiter bullet) *(adversarial review 2026-08-24, R4-9)*. Producer state:
  keep the last-backed-up payload locally (simple; ~100–430 KB today, shrinking after Phases 2–3);
  if it is lost, write a snapshot (README Open Q3 — pick simple first, measure later).
  **Baseline pinning (adversarial review 2026-08-24, R4-3 — fixes a live shipped bug):** the
  stored baseline is **the exact collected state that was broadcast**, captured at build time —
  never re-collected after broadcast (the shipped step-13 re-collect at `ref_ts=now`,
  handlers.rs:14077-14094, silently absorbs every mutation that lands during the multi-second
  broadcast window and must not survive into any phase). D13 is the normative home of this rule.
- **Backoff:** consecutive backup failures back off exponentially to a quiescent, user-visible
  error state; retry ceiling before requiring manual action (constraint 9, closes E11).
- **Stuck-unconfirmed raw_tx — ROOT CAUSE FOUND (live check 2026-08-24; supersedes the earlier
  "genuinely non-refetchable" reading):** the stuck rows (now 12) are two distinct classes.
  **(a) 8 `failed` rows (July 2026)** — correctly failed, already excluded from the payload
  (failed txs filtered at SQL level, A1 §2 item 14), but their raw bytes are retained in the DB
  forever: a DB-bloat cleanup item, not a payload bug. **(b) 4 `completed` rows that ARE MINED** —
  verified against WoC 2026-08-24: `9f36ba…` (the 260 KB tx) and `198507…` in block 962,735 with
  987 confirmations, `0f82d8…` in 963,573 with 149 — whose `proven_tx_reqs` rows sit at
  `status='nosend', attempts=0, history='{}'`. These are BRC-100 `noSend` actions broadcast
  externally by the counterparty/dApp; nothing ever transitions a `nosend` req, TaskCheckForProofs
  skips it, `proven_tx_id` stays NULL, and the proven-gated strip (`backup.rs:449-457`) hauls
  their full raw_tx into **every** backup — the measured ~60% of the live 431 KB payload. So the
  payload rule "carry unconfirmed raw_tx" stays correct *for genuinely unmined txs*; the fix is
  upstream and now concrete: a reconciliation step checks `nosend` (and any terminal-unproven)
  reqs against the chain — mined → routed into the normal proof pipeline (req → proven → raw_tx
  strips on the next backup **at the D14 confirmation depth, not at 1 conf** *(adversarial review
  2026-08-24, R4-5)*); apparently dead → **`abandoned-unbroadcast`** per D14: raw bytes leave the
  payload, but **inputs stay encumbered and outputs are never deleted** on quorum-of-absence alone
  — no oracle can distinguish "dead" from "counterparty holds it and will broadcast later", and
  restoring inputs on a timer is exactly the Bug C mechanism *(adversarial review 2026-08-24,
  R4-2 — supersedes this bullet's earlier "aged out to `failed`" wording)*. Reqs in persistent
  oracle disagreement hit the D14 limbo bound (local-only retention + health flag) so payloads
  stay bounded *(R4-8)*. Phase 2 implements; **H14** pins it. Expected effect: payload ~431 KB →
  ~165 KB immediately (A1 §4 decomposition), before Phase 3's strip rule 6. Owner sign-off on the
  age-out policy recorded in Phase 2.

### 3.4 Cost table per wallet profile

Replaces estimates with data where data exists; remaining estimates are marked *est.* with the test
that replaces them. Fee-rate is a measured input (H8a extracts actual fees paid by the 114 historical
backup txs); the two bracketing assumptions shown because they differ by 250×.

| Profile | Snapshot (compressed) | Typical delta | Per-backup cost @1 sat/KB | Per-backup @250 sat/KB | Yearly (4/day, delta mode) | Replaced by |
|---|---|---|---|---|---|---|
| A — payments-only, active | ~15 KB *est.* | 1–2 KB *est.* | *est.* | *est.* | *est.* ~$3.50 | H8b/H9 profile-A trace |
| B — mixed (**≈ the live production wallet, measured**) | **431 KB measured** today; ~165 KB excl. stuck raw_tx (measured); ~100 KB *est.* after Phases 2–3 | 1–2 KB *est.* | ~432 sats ≈ $0.0002 | ~108 K sats ≈ $0.054 | *est.* until H9 | H8a (historical actuals) + H9 |
| — dev wallet (light mixed, measured) | **61 KB measured** | — | ~62 sats | ~15 K sats | — | H8a |
| C — collector, 200 image ordinals | ~3 MB *est.* → ~100 KB *est.* after item 1 | 1–2 KB *est.* | *est.* | *est.* | *est.* ~$8 | H8b with the profile-C fixture; H13 measures the strip win |

Cost floor, restated: **steady-state burn per broadcast = mining fee only.** The per-broadcast
**float is 1,546 sats** — the 1,000-sat PushDrop token (`backup_output_sats`, `handlers.rs:13603`)
plus the 546-sat marker — and both are spent (recovered) by the next backup cycle with identical
recovery semantics, so counting the marker and not the token is wrong under any accounting (A1 §4:
"1546 sats parked in token+marker (recovered next cycle)"). Sats stay permanently parked only in
the terminal never-superseded token+marker pair. **No service fee exists on backup txs** — the
draft/current-doc's 1,650-sat floor includes a 1,000-sat *fee* the code explicitly waives
(`handlers.rs:13602`, A1 §2 item 2; A3 disagreement 5); the shipped 1,000 sats is the token's own
value, not a fee. Fee rate and float accounting re-derived in H8; after H8a, numeric yearly
ceilings per profile are frozen from the historical actuals and become G7's pass/fail bound.

---

## 4. Implementation and testing phases

Each phase independently shippable, with explicit acceptance criteria. Ordering rationale: the
retrospective's constraint 13 (never ship recovery-affecting changes without a round-trip test) puts
the harness **before** any format change — this deliberately supersedes the README's "item 1 first"
ordering. A3's 16 constraints are woven in below and cross-referenced in §4.2.

### Phase 1 — Test harness foundation + drift gate. *(new; gates everything)*

Build the §5 harness skeleton: mock indexer/broadcaster, deterministic clock, fixture wallets
(profiles A/B/C), and land **H1 (round-trip restore-and-spend) against the CURRENT shipped format**
plus **H2 (schema drift gate)** in CI. Today `rust-wallet/tests/` contains zero backup tests
(A3 §5, verified at repo level).
**Acceptance:** H1 green on the shipped format (or red with filed bugs — a red H1 on today's code
is a *finding*, not a blocker to landing the test); H2 red listing today's known-unclassified
columns; both wired into CI. *(Constraints 5, 13.)*

### Phase 2 — Payload completeness + write-path hygiene on the current format. *(new)*

Fix what H2 exposes, on the shipped single-snapshot format so fixes are decoupled from format risk:
- Column-level losses: `domain_permissions` (3 of 12 columns dropped), `settings` defaults +
  `sender_display_name`, `transactions.price_usd_cents/recipient/recipient_name`,
  `outputs.confirmed` (all recovered outputs currently land `confirmed=1`) — A1 §2 items 23–26.
- Table-level decisions by tier: `peerpay_received` (395 live rows, silently lost — carry: BRC-42
  counterparty data is non-re-derivable, invariant 2), V18 permission child tables (3 live rows —
  carry or record tier-3 exclusion), others recorded in the H2 manifest.
- Write-path hygiene: no `let _ =` on state-mutating results in the backup path (every post-broadcast
  insert is currently discarded — A1 §5); replace reachable `unwrap()` panics in the backup/recovery
  handlers with typed errors, including the truncated-raw-tx slice indexing.
- Backoff + health surface: consecutive-failure counter, last-success, tip txid exposed to UI;
  retries converge (G5). *(adversarial review 2026-08-24, R4-6/R2-4/R5-5: the surface gains the
  three further inputs G5 now names — tip **confirmation** state with a rebroadcast-on-stall rule
  (an evicted terminal token on an idle wallet must go red, never stay green forever), chain
  **divergence** from poll results (foreign tip not ancestor/descendant of local tip), and
  **key-identity mismatch** (backup address vs loaded wallet identity — the E12 observability
  half, closing the testable part of constraint 14).)*
- **Recovery is resumable** *(adversarial review 2026-08-24, R5-1/R5-9)*: any refetch failure
  during recovery is a typed recovery FAILURE (never "success with warnings"); refetch gets
  backoff + rate-limit handling (429/Retry-After); and the fresh-wallet-only Conflict check
  (handlers.rs:15008-15017) learns to distinguish "partial recovery in progress — resume" from
  "occupied wallet — refuse", so a rate-limited or killed recovery can complete on retry instead
  of wedging.
- **Stuck-unconfirmed lifecycle fix (root cause verified — §3.3):** reconcile `nosend` /
  terminal-unproven `proven_tx_reqs` against the chain: mined → proof pipeline (raw_tx then
  strips from the next payload at the D14 confirmation depth); apparently dead →
  **`abandoned-unbroadcast` per D14** — bytes leave the payload, inputs stay encumbered, no
  timer-only output deletion (owner sign-off on the age-out policy). *(adversarial review
  2026-08-24, R4-2/R4-5/R4-8.)* Failed-tx raw-byte retention in the DB: cleanup decision
  recorded. Acceptance: **H14** green.
**Acceptance:** H1 diff shrinks to the documented exclusion list; H2 green; H5 (crash matrix) run
against the hardened path shows bounded retry and no panic; live payload size drops (stuck-tx fix)
— measured before/after recorded for H8. *(Constraints 9, 12, 16; groundwork for 2.)*

### Phase 3 — Strip rule 6: inscription bytes off-chain, rehydrate on recovery. *(README item 1, kept)*

Back up outpoint + basket + tags + `custom_instructions` + derivation for token rows; drop
`locking_script` bytes; re-hydrate by outpoint on recovery. Note the README's named function
`prepare_backup_payload()` does not exist — the strip lands in `compress_for_onchain`
(A1 §2 item 4).
**Acceptance:** H13 byte-match green (every re-fetched script identical); H1 still green including
the token-output spend; profile-C fixture snapshot ≤ ~150 KB *(number recorded for H8)*.
*(Constraint 1 — structural separation of re-fetchable data.)*

### Phase 4 — Token header + chain + crash safety. *(README item 2, kept, extended)*

`version | kind | seq | parent_txid | device_id` on every token; snapshot-only (`kind=0`) so the
chain exists before deltas; plus the additions this research forced:
- `prev_payload_sha256` in the payload envelope (D3 harmonization; decide and record).
- Recovery: history-enumeration bootstrap + decrypt-before-trust + parent-walk with reassembly +
  child-existence recency per **D6 as rewritten** *(adversarial review 2026-08-24)*; fail-closed
  error taxonomy (constraint 10), including deltas-without-base as a typed fail-closed error.
- **Backup intent record persisted before broadcast** + reconcile decision table **on every
  trigger path** (D7 as extended *(adversarial review 2026-08-24)*: three-valued broadcast
  outcome, full reserved-input list incl. funding inputs, cached raw tx + rebroadcast-first heals,
  multi-intent rule; constraint 8). The intent/tip bookkeeping lives in a dedicated table
  **excluded from the payload by construction**, not by SQL filter (constraint 3).
- Hard pre-broadcast size cap: over-cap builds fail closed with a typed error (G7). Chunking stays
  unimplemented and the draft says so honestly (today's draft describes chunking that does not
  exist — A1 §3d); revisit only if a measured need appears.
- Backup address row carries explicit derivation metadata; full sync excludes it structurally —
  no more index-convention inference (BS-SYNC-1; constraint 15).
**Acceptance:** H1 green through the new header; H5 crash matrix green at every kill point incl.
between broadcast and record, **crossed with active index lag, plus the UNKNOWN-broadcast,
double-crash, and non-startup-trigger rows** *(adversarial review 2026-08-24)*; H6 corruption
suite green incl. the mid-chain-hole case; H7 restore-from-every-boundary green on snapshot-only
chains incl. the three recency branches; T2/H10 litter test green incl. adversarial litter;
**H17 (reorg/eviction) green**. Old-format recovery still works (version byte).
*(Constraints 3, 6, 7, 8, 10, 15.)*

### Phase 5 — Deltas. *(README item 3, kept; format now decided, not invented)*

Row-level changeset producer per D2 (BRC-38 portable row forms + per-table `deletes` + per-row
version counters + extension tables), snapshot-on-ratio rule per §3.2, dirty flag per §3.3,
recovery replays snapshot + deltas in **parent-topological order with `seq` as sanity check**
(D15) *(adversarial review 2026-08-24)*. Additions from the review:
- **Delete-emission allowlist** (D14, R4-7): only chain-corroborated allowlisted transitions emit
  delete records; timer-only monitor transitions never do.
- **Merge-before-snapshot + pinned baseline + consistent reads** (D13, R2-3/R4-3/R4-11) are
  producer requirements, not conventions.
- **Write-path adopt/sweep redesign** *(R1-05)*: the shipped adopt-by-max-height +
  sweep-any-non-primary-marker logic (handlers.rs:13560-13600) is unsafe under deltas — a
  "non-primary" marker can be another device's true tip or a crash ghost's successor. The sweep
  may never consume a marker that decrypts under our key and carries seq ≥ the local tip; only
  markers proven superseded by chain linkage are sweepable. **H19** pins the
  superseded-vs-tip race.
**Acceptance:** H3 property green (snapshot+deltas byte-identical to continuous state after
normalization, over randomized op sequences; replay idempotent; delete records all allowlisted;
mutation-during-broadcast and concurrent-produce cases green); H1 green with N = 0/1/5/20 deltas;
H7 green from every boundary; **H19 green**; measured delta-size distribution published (feeds
§3.2 constants and the BRC §7). *(Constraints 1, 2, 4.)*

### Phase 6 — Size-class padding. *(README item 4, kept)*

Pad payloads to 1 / 4 / 16 KB classes (snapshots to their own classes).
**Acceptance:** H12 histogram green — all payloads in declared classes; H12's concrete
distinguisher test (chi-square + classifier margin, thresholds in §5) passes. Number for BRC §8.

### Phase 7 — Multi-device sync. *(README item 5, kept)*

`device_id` assignment + label in encrypted settings; poll on timer + before any spend/reserve;
read-before-write; fork detection + re-read-re-write. *(adversarial review 2026-08-24, D15:)* the
tie-break is **named normatively now** — lower `device_id` defers; the chain remains the primary
arbiter — and H4 *validates* it rather than deciding it (supersedes the earlier "decided from H4
data" wording, which was circular: H4 could not converge without the rule it was meant to
produce). Dual-genesis rule implemented (D15). Divergence-as-health-input wired into the poll
path (G5 input 3). Poll-discovered remote deltas are applied (not merely adopted) before this
device's next snapshot (D13).
**Acceptance:** H4 green (fork detected, ≤2-round convergence, no backup-chain double-spend,
field-level union preserved incl. the snapshot-after-missed-delta and same-row concurrent-edit
cases, loser-device end state asserted, dual-genesis round green) *(adversarial review
2026-08-24)*; H11 latency numbers published and the sync-vs-optimistic decision recorded; H9 soak
green with two devices. *(Constraints 6, 7; G3.)*

### Phase 8 — Measurement close-out + cross-wallet proof. *(README items 6 + 7, kept)*

H8/H9 numbers replace every *est.* in the BRC cost table (item 6 — partially front-loaded since H8a
runs in Phase 1–2). Item 7 proceeds as the README specifies (HandCash `.brc39` → Hodos first),
informed by A2: the import backend is **live, not rotted** (shared `collect_payload` /
`import_to_db_with_ids` machinery exercised daily by the on-chain path; only the UI is hidden), and
the four landmines are known — container is not BRC-39, payload is not BRC-38 (Phase 4/5 closes
this), the mnemonic-matches-identity-key hard check must be redesigned for foreign docs, import is
fresh-wallet-only/destructive. Run item 7's import testing through the chunked bridge path to retire
the unrecorded large-export round-trip test (A2 §3).
**Acceptance:** T8–T11 as defined in the README (kept verbatim); BRC cost table contains zero
unmarked estimates.

### 4.1 README reconciliation — what stands, what is superseded

| README item/claim | Verdict |
|---|---|
| Item 0 (BRC-38 compat + gate) | **Done** (B2). Gate decided: (a) preferred, ship (b) — D1. Correction: 19 SQL tables serialized, not 18; the tier-1 orphan is one field (`current_index`), not five tables. |
| Item 0b (delta prior art) | **Done** (C1/C2/C3). Decisions D2/D3 on record. Premise correction: blob is opaque; "sync IS 38/39" → sync is BRC-40 (D10). |
| Item 1 (strip inscription bytes) | **Stands** → Phase 3. Function-name correction (`compress_for_onchain`). |
| Item 1 "do it first regardless" | **Superseded**: harness first (Phase 1), per constraint 13. |
| Item 2 (token header) | **Stands** → Phase 4, extended with intent record, recency check, `prev_payload_sha256`, size cap. |
| Item 3 (deltas) | **Stands** → Phase 5; format decided by D2, resolving "must not invent in a vacuum". |
| Item 4 (padding) | **Stands** → Phase 6. |
| Item 5 (multi-device) | **Stands** → Phase 7. |
| Item 6 (measure cost table) | **Stands**, front-loaded: H8a historical extraction starts in Phase 1–2. |
| Item 7 (cross-wallet proof) | **Stands** → Phase 8; A2 findings folded in (backend live; landmines enumerated). |
| "What's already decided" block | All five stand. "No extra signature" is additionally supported by C2's finding that BRC-40 delegates auth to transport — our GCM-tag-as-origin-proof is the on-chain equivalent. |
| Open questions 1–7 | Q1 → H11 decides. Q2 → answered normatively (D15: lower `device_id` defers; the chain arbitrates), H4 validates *(adversarial review 2026-08-24 — supersedes "H4 decides")*. Q3 → simple last-payload copy first (§3.3), baseline pinned per D13. Q4 → H7/H11 measure. Q5 → answered (D9). Q6 → superseded by Phase 4's hard cap; chunking honestly deferred. Q7 → out of scope except the three cheap gates (kept, in H1/H9 assertions). |
| Tests T1–T7 | Mapped into H-tests (see §5 table); all kept, made stricter. T8–T11 kept verbatim for Phase 8. |
| "Still accurate for what ships today" (re ONCHAIN_BACKUP_SYSTEM.md) | **Superseded** — the doc has ≥6 verified factual errors (§6.1); cite code lines, not the doc, until it is rewritten post-Phase 2. |

### 4.2 Retrospective constraints → where each lands

A3 §5's sixteen "must not repeat" constraints, each with a design decision and/or test:

| # | Constraint (short) | Design decision | Test |
|---|---|---|---|
| 1 | No monolithic payload forcing lossy strips | Phase 3 structural strip; Phase 5 deltas | H13, H3 |
| 2 | Canonical change detection | §3.3 dirty-flag redesign (ORDER BY, pinned ref_ts, no volatile cols) | H3 determinism, H9 zero no-op broadcasts |
| 3 | No backup bookkeeping inside backed-up state | Phase 4 dedicated intent/tip table, excluded by construction | H2 manifest, H5 |
| 4 | No raw autoincrement IDs/FKs on the wire | D2: natural-key portable row forms | H3 (remap property), H1 |
| 5 | Schema drift gate | H2 in CI | H2 |
| 6 | No "newest" by indexer timestamps | D6 chain walk; parent-topological order normative (D15); decrypt-before-trust *(adversarial review 2026-08-24)* | H7, H10 |
| 7 | Indexer/broadcaster trusted per contract only | D6 child-existence recency (spent-status demoted — R1-01); retained txid-collision guard (now pinned by an H5 assertion — R1-07) + verify quorum with promote-timer removed (D14); mock fault models incl. 429, UNKNOWN-broadcast, eviction, reorg, correlated secondary *(adversarial review 2026-08-24)* | H5, H6, H16, H17 |
| 8 | Atomic-enough broadcast/record | D7 intent record + three-valued broadcast outcome + decision table on every trigger path *(adversarial review 2026-08-24)* | H5 |
| 9 | Bounded retry | Phase 2 backoff + quiescent error | H5, H9 |
| 10 | Indexer-down ≠ no backup | Phase 4 fail-closed error taxonomy | H6 |
| 11 | Atomic import | Shipped single-transaction import retained (A1 §1b); **kill-mid-import added to the H5 matrix and recovery made resumable — the constraint was previously untested on the one case it exists for** *(adversarial review 2026-08-24, R5-9)* | H1, H5, H6 |
| 12 | Concurrency discipline, no `let _ =` | Phase 2 hygiene | H5 |
| 13 | Round-trip test before shipping | Phase 1 first | H1 |
| 14 | Namespaced credential storage | **Partially closed** by `deff765` (A3 E12): env-level namespacing (dev/prod service name), **macOS only**. The other half — per-wallet-identity namespacing, on every platform — is unverified and **open**, with no test; recorded as an open item, no new code this sprint. *(adversarial review 2026-08-24, R5-5: the observability half is now testable — key-identity mismatch is a G5 health input asserted in H9, so the E12 class can no longer pass silently even while the storage fix stays open.)* | H9 (key-identity assertion); storage half still open |
| 15 | No signing-key inference from index conventions | Phase 4 explicit derivation metadata on the backup address | H9 soak includes a full sync; H1 |
| 16 | Backup health observability | Phase 2 health surface | H9 (injected failures must surface) |

---

## 5. Test harness spec

**This is the deliverable that matters most.** Strict pass/fail; "seems to work" is a fail. The
harness lives at `rust-wallet/tests/backup/` (today: zero backup tests exist anywhere in
`rust-wallet/tests/` — A3, verified by grep at repo level).

**Environment.** A local mock chain service implementing exactly the endpoints the wallet uses
(WoC `address/{addr}/unspent/all`, address history, `tx/{txid}/hex`, TSC proof endpoint, **and the
spent-status endpoints `reconcile::check_outpoint_spent` actually consumes, with their real
404/NoSignal semantics for plain P2PKH — the review live-probed these and the pre-review mock
omitted them entirely, so H6/H7 were testing a stub** *(adversarial review 2026-08-24, R1-01)*;
ARC submit), with scriptable fault injection: index lag (the 30 s–5 min WoC window is a
first-class simulated state, **drawn from the H16-contracted distribution including the tail**),
JSON `scriptPubKey` truncation (E6), 404s, ARC 200-with-different-txid (E4 Bug B — **bound to the
H5 record-local-txid assertion, no longer an unasserted fault**, R1-07), `DOUBLE_SPEND_ATTEMPTED`
for both txs in a conflict (E4 Bug C), and *(adversarial review 2026-08-24)*: **429/Retry-After
rate-limiting mid-bulk** (R5-1); **broadcast accepted-with-lost-response** (UNKNOWN outcome,
R4-1); **mempool eviction on a simulated-time horizon** (~14 simulated days, so H9 soaks cross
it — R4-6/R5-7); **price-cache-empty windows** (R5-7); **un-mine/reorg of depth 1–2** (R4-5);
**correlated secondary-oracle lag and mid-recovery answer flips** (R5-8); **adversarial litter**
— attacker P2PKH dust markers and crafted plaintext headers at the backup address (R1-06/R3-2).
Deterministic injected clock (the `ref_ts` parameterization already exists — A1 §3e). Fixture
wallets: profile A (300 tx / 100 UTXOs), profile B (mixed + 500 text-token rows), profile C
(200 image ordinals), each containing HD outputs, BRC-42 counterparty outputs (PeerPay-shaped),
PushDrop token outputs, certificates including one with `publish_*` set, domain permissions with
the V17/V22 columns set. A small number of live-mainnet smoke runs (dev wallet) produce real fee
numbers **and, per release, one real seed-only restore-and-spend (H18)**; everything else runs
against the mock.

**Declared limitation of this harness** *(adversarial review 2026-08-24, R5-6 — stated verbatim
per the review)*: the mock is generated from the same endpoint contracts the team wrote, so every
mock-tier test proves the wallet correct **against the team's model of the network, not against
the network**. Every prior incident lived exactly in the gap between that model and the territory
(A3's thesis). H16's live-probe drift checks and H18's live recovery smoke are the only tests that
cross the gap; they are mandatory tiers, not optional extras, and a release with H18 red does not
ship.

| ID | (README) | Test | Setup | PASS (exact) | FAIL (exact) | Number produced for the BRC |
|---|---|---|---|---|---|---|
| **H1** | T1 | **Fresh install → seed only → full restore → SPEND** | Fixture wallet; run backup(s); destroy the DB and all local state; recover with mnemonic only against the mock chain; chains of 1 snapshot + N deltas, N ∈ {0,1,5,20} (N>0 from Phase 5); *(adversarial review 2026-08-24)* plus a run under the 429-mid-bulk fault and a recovery-after-killed-recovery run | Recovery reports success **and** refetch-failure count is 0 — **the former "or each failure explicitly surfaced" escape hatch is deleted: any refetch failure = typed recovery FAILURE, and a failed/killed recovery is resumable to completion on retry (Phase 2), never wedged by the fresh-wallet-only check** *(adversarial review 2026-08-24, R5-1/R5-9)*; recovered balance == original **to the satoshi, both directions**; every original spendable output spendable with correct derivation; **every re-derived/re-fetched column value-correct against the mock's ground truth, not just present — the exclusion list exempts columns from equivalence, never from correctness** *(R5-2)*; a built+signed spend of (a) an HD output and (b) a token/BRC-42 output passes **full script verification** against recorded prev-out scripts and is accepted by mock ARC; post-recovery liveness check runs before first spend (Q7 gate); DB diff vs original == the H2 exclusion list exactly | Any panic; balance mismatch (inflation is failure — E1-58cf9a3); any undocumented missing/extra row; script verification failure; recovery success reported despite any swallowed refetch failure; a wrong re-derived value (e.g. the shipped `outputs.confirmed=1` class); an unresumable partial recovery | Recovery wall time and fetch count vs N |
| **H2** | new | **Schema drift gate (CI)** | Enumerate live schema via PRAGMA; compare against a checked-in coverage manifest classifying every `table.column` → travels / re-derived / excluded(tier, reason). *(adversarial review 2026-08-24, R5-2)*: the classification is **two-axis** — equivalence (in/out of the canonical diff) and correctness (how the value is verified) are independent; every re-derived/re-fetched column names its correctness oracle, and every excluded column carries an audited reason | Every column classified on both axes; payload code and manifest agree (round-trip a fully-populated row set: every "travels" column survives; every re-derived column's oracle exists in H1) | Any unclassified column (i.e. a migration landed without a backup decision); any "travels" column that does not survive round-trip; any re-derived column with no correctness oracle | The coverage table (BRC appendix; the item-0 deliverable kept current) |
| **H3** | new | **Delta replay equivalence (property test)** | Randomized operation sequences (receive, spend, label, basket, certificate op, permission change), snapshots triggered per the §3.2 rule at arbitrary points; ≥1000 seeded cases; *(adversarial review 2026-08-24)* including **mutations injected during a simulated broadcast window** (R4-3) and **ops interleaved with an in-flight produce** (R4-11) | For every seed: `canonical(replay(snapshot + deltas))` **byte-identical** to `canonical(continuous state)`, where `canonical` = the normative serialization (sorted, ID-remapped, volatile columns excluded per spec — with those columns still correctness-checked per H2's split); applying any delta twice == once (idempotence); two runs of the producer on identical state emit identical bytes (determinism); **a mutation landing during the broadcast window appears in the next delta, never in the baseline (D13); every delete record carries an allowlisted cause (D14); the producer's output is a function of one consistent DB snapshot** | Any byte diff; any nondeterminism between runs; idempotence violation; a broadcast-window mutation absorbed into the baseline; a delete record with no allowlisted cause; torn reads under interleaving | Delta-size distribution per op type; observed deltas-per-snapshot per profile (settles the §3.2 rule) |
| **H4** | T4 | **Multi-device concurrent writes, fork detection + recovery** | Two wallet processes, same seed, shared mock chain; **propagation delay drawn per round from the H16-contracted lag distribution incl. the 30 s–5 min tail, plus k-rounds-stale poll cases** *(adversarial review 2026-08-24, R5-3)*; scripted: spend on A / poll on B; then both write within one delay window; repeated under randomized timing ≥500 rounds; *(adversarial review 2026-08-24)* plus scripted cases: **snapshot-after-missed-delta** (R2-3), **same-row concurrent edit** (R4-9), **same-user-UTXO race** (R2-6), **dual-genesis** (D15) | B sees A's spend before B's next spend; every same-parent fork is detected by both devices; convergence to a single tip in ≤ 2 re-read/re-write rounds; zero accepted double-spends **on the backup chain**; **field-level** union of both devices' changes present after convergence — incl. a delta committed by one device and not yet applied by the other when the other snapshots (the snapshot must contain it, per D13); same-row concurrent edits resolve by the D2 row-version arbiter, deterministically; the user-UTXO race loser ends in the specified state (losing tx failed, inputs marked spent-by-other, no deletion cascade); the tie-break rule (D15) is **validated** | Silent divergence (a device continues on its own branch); lost update **or lost field**; accepted backup-chain double-spend; convergence > 2 rounds; a snapshot that reverts a committed delta; an unspecified loser-device end state | Fork rate vs write rate; resolution rounds; tie-break validation data (Q2) |
| **H5** | new | **Crash mid-backup matrix** | Injected hard-kill points at every step boundary of the write path (before reserve, after reserve, after broadcast/before intent-consume, mid-DB-record, before hash store — the A1 §1a step table); restart, startup reconcile, then one backup and one full recovery; also the E11 replay: kill after broadcast, restart with a stale DB. *(adversarial review 2026-08-24)*: the matrix is **crossed with an active index-lag state** at every kill point (R4-4); added rows: **broadcast-UNKNOWN** (accepted-with-lost-response, R4-1); **double-crash multi-intent** (R4-4); **reconcile on a non-startup trigger** (3-hour/receive path, R1-04); **two-device ghost interleaving** against the full-input-list intent record (R2-5); **ARC 200-with-different-txid** (R1-07); **kill-mid-import** at every import step boundary (R5-9) | After every kill point (lag active or not): wallet converges without manual intervention **per the D7 decision table — on UNKNOWN or not-visible the action is re-broadcast-cached-raw-tx, never rollback-and-rebuild**; the intent record covers every reserved input incl. funding; the recorded txid is the locally computed one regardless of ARC's response body; multiple intents drain oldest-first to terminal states; retries bounded by backoff (quiescent error if unrecoverable); no funds lost beyond ≤ 1 orphaned marker pair per crash, swept by the next cycle; chain remains walkable; recovery after the crash equals recovery without it; a killed import is resumable to a complete wallet | Unbounded retry (the E11 loop); any rollback-restored input while the broadcast outcome is UNKNOWN (the E4 re-arm); a same-parent second build while an intent is live; unwalkable chain; balance error; ghost divergence not healed by reconcile; a wedged half-import; panic | Max sats leaked per crash class (documented bound) |
| **H6** | new | **Corrupted / missing chunk handling** | Mock serves, in separate cases: truncated raw-tx hex; PushDrop payload with corrupted GCM tag; valid-GCM but malformed JSON; missing parent tx (404); stale unspent index pointing at a superseded token; total indexer outage; *(adversarial review 2026-08-24, R1-02)* **a mid-chain link unservable by the tx endpoint (a) while present in history, (b) while absent everywhere** | Each case yields the specified **typed error**; mid-chain hole case (a) heals by history-set reassembly (D6 rule 4) and recovery completes; case (b): **deltas-without-base is a fail-closed typed error, never a restore offer**, and a restore to an older complete state is offered only with an explicit break report + user acknowledgment gate before any write on top *(R1-02 — "reported break" alone is not a pass)*; stale-index case refuses to restore and retries (D6); outage reads "indexer unavailable", never "no backup exists" (constraint 10); zero panics (the `raw_tx[pos..pos+8]` class is dead) | Any panic; silent partial restore reported as success; **a partial restore that accepts writes without the acknowledgment gate**; deltas applied without their base snapshot; stale state restored; outage steering user to create a fresh wallet | The error taxonomy table for the BRC |
| **H7** | new | **Restore from every generation boundary** | Build a chain snapshot₀ + d₁..dₖ + snapshot₁ + dₖ₊₁..; for every token t, simulate an indexer whose newest visible token is t; recover. *(adversarial review 2026-08-24, R1-01/R1-03)*: the mock serves the **real** spent-status semantics (P2PKH → NoSignal/404), and cases cover **all three recency branches** — `Spent`, `ExplicitUnspent`, and `Unknown` (the modal healthy outcome) | Recovery from tip t == the true state at t, for every t; recency decided by **child-existence over the decrypted history set** (D6 rule 3): a lagging index that hides newer tokens from *history* is flagged when any decrypted marker's parent pointer or the secondary oracle contradicts it; a healthy tip yielding `Unknown` spent-status **restores normally** (no brick); a `Spent` tip triggers refetch-retry, never a stale restore | Wrong state from any boundary; mid-chain replay producing merged state; unflagged stale restore; **a healthy-tip recovery refused because spent-status was Unknown; any recency decision taken from a plaintext header before GCM verification** | Fetch count vs chain length (Q4/Q8 data) |
| **H8** | T3 | **Cost accounting** | (a) Historical: extract all 114+ `backup-%` txs from the production DB — actual fees paid (fee = total input value minus total output value; prev-out values from `parent_transactions` where present, else fetched by txid from the indexer; parents unavailable anywhere are excluded and counted, and the fee numbers marked partial — the extractability claim is the plan's own INFERRED claim, §3.1), payload sizes over time. (b) Harness: per-backup cost assertions over H9 traces for profiles A/B/C | Every backup's mining fee == measured rate × padded tx size, exactly; per-broadcast float == 1,546 sats (1,000 token + 546 marker) and every non-terminal float is recovered by the next cycle's spend; no payload above its size class; no broadcast above the hard cap (build fails closed instead); yearly projections computed from traces, not assumptions; after (a), numeric yearly ceilings per profile frozen (they become G7's bound) | Any unexplained cost; any non-terminal float not recovered; any over-class or over-cap broadcast | **The cost table** (§3.4 with every *est.* replaced) + the measured fee rate and float accounting (correcting both the phantom 1,000-sat service fee and the 546-only floor) + the frozen yearly ceilings per profile |
| **H9** | new | **Soak — months of realistic activity** | 90 simulated days per profile (accelerated clock), scripted op streams, background monitor churn, restarts, random crash injection (H5 model) and indexer faults (H6 model); Phase 7: run with two devices; includes at least one full sync. *(adversarial review 2026-08-24)*: the simulated clock **crosses the mock's mempool-eviction horizon and price-cache-empty windows** (R5-7); scripted scenarios include **terminal-token eviction on an idle wallet** (R4-6), **device divergence with no local dirty change** (R2-4), and **key-identity mismatch** (R5-5) | **Zero** no-op broadcasts (G6); staleness bound held (G2 as scoped: any-new-output dirty flag; the sub-funding-minimum exclusion surfaced in health state, never silent), checked by a **full recovery-diff run after every op batch and at every simulated hour boundary** — recovery can only be sampled, so this cadence is the declared meaning of "at every instant" and the pass quantifies over every sample; every injected failure surfaces in the health state (G5/constraint 16) — **including the four broadened G5 inputs: an evicted unconfirmed tip goes red + rebroadcast within the declared horizon even with zero wallet activity; a diverged device flags divergence within one poll interval; a key-identity mismatch flags immediately; and MTTD is measured off the product health surface, not the harness's own recovery-diff oracle** *(R4-6/R2-4/R5-5)*; snapshot cadence matches the §3.2 rule; end-of-run recovery diff == exclusion list; full sync does not poison the backup address (BS-SYNC-1 regression) | Any false-dirty broadcast; any staleness-bound violation; any silent failure (health state green while a backup failed, a tip sat evicted, a device sat diverged, or the key identity mismatched); end-of-run divergence | Observed deltas/snapshot; yearly cost per profile; divergence MTTD from the product surface (must be ≤ one poll interval — three silent months must be impossible) |
| **H10** | T2 | **Junk at the address** | 50 random PushDrop tokens sent to the backup address by a foreign key; then recover. *(adversarial review 2026-08-24, R1-06/R3-2)*: plus **adversarial litter** — (a) a plain P2PKH dust output that outranks every real marker under the shipped `max_by_key` ordering (incl. unconfirmed → i64::MAX), (b) a keyless attacker token with a **crafted plaintext header** (max seq, unconfirmed, `parent_txid` pointing at a real stale snapshot) | All junk skipped by GCM failure via **newest-that-decrypts iteration — one undecryptable candidate never aborts discovery** (D6 rule 2); no plaintext header field of an unverified token influences selection, recency, or walk order; chain walk unaffected; recovery result identical to the no-junk run in all cases | Junk processed; **recovery aborted or wrong tip because of any non-decrypting output, dust or crafted**; recovery failure or wrong state | Recovery time with/without litter |
| **H11** | T5 | **Pre-spend poll latency** | Time read-to-tip + apply, cold and warm, 100 runs each, mock + live smoke | Numbers produced; decision recorded (sync vs optimistic) with the numbers attached | — (measurement test; failing to record the decision is the failure) | p50/p95 added latency per spend (Q1) |
| **H12** | T7 | **Padding** | Payload-size capture over an H9 trace | Every payload lands exactly in a declared class; the distinguisher test passes: (i) chi-square test of size-class × action-type independence over the H9 trace, α = 0.01, rejects nothing beyond the declared leak, and (ii) a held-out classifier predicting action type from (size class, inter-backup interval) scores ≤ chance + 5 percentage points once the declared leak is conditioned out | Any off-class payload; chi-square rejection or classifier margin over the threshold | Size histogram (BRC §8) |
| **H13** | T6 | **Strip-and-rehydrate** | Profile-C fixture through Phase 3; wipe; recover | Every re-hydrated `locking_script` **byte-identical** to the original; every token spendable | Any byte mismatch; any unspendable token | Bytes saved (3 MB-class → measured); rehydrate time |
| **H14** | new | **Proof-lifecycle reconciliation (the stuck-tx class)** *(PASS/FAIL rewritten, adversarial review 2026-08-24 — R4-2/R4-5/R4-8)* | Fixture: a `noSend` action whose tx the mock chain mines externally (wallet never broadcasts it); plus a req left unmined past the age-out horizon **whose tx the mock later broadcasts from the "counterparty" after the age-out fired** (the R4-2 scenario); plus a req in **permanent oracle disagreement** (one oracle claims mempool forever) | Mined tx's req transitions `nosend` → proof pipeline → `proven_tx_id` set within one monitor cycle of proof availability; its raw_tx leaves the payload **only at the D14 confirmation depth, never at 1 conf**; the aged-out req lands in **`abandoned-unbroadcast`: raw bytes out of the payload, inputs STILL ENCUMBERED, outputs intact** — and when the counterparty later broadcasts, the wallet reconciles with **zero self-inflicted conflict and no tombstone replay on recovery**; the limbo req hits the D14 bound: after N cycles its bytes move to local-only retention + health flag and the payload is bounded; at end of run **zero** `completed`-and-mined txs older than one cycle remain unproven | Any mined tx still unproven past the cycle bound; payload carrying raw bytes proven at depth; **any age-out that restores inputs or deletes outputs on timer/quorum-of-absence alone (the Bug C re-arm — this was the pre-review PASS condition and is now the FAIL condition)**; a limbo req riding in payloads unboundedly; a conflict manufactured against a late counterparty broadcast | Payload before/after on the live-shaped fixture (the 431 KB → ~165 KB claim, measured) |
| **H15** | new | **Decrypt-compat across every token version (G11)** | Checked-in binary fixtures, one per payload version ever broadcast to mainnet (starting with the shipped headerless gzip→AES-GCM under `SHA-256(master ‖ "hodos-wallet-backup-v1")`); the fixture set is append-only, never regenerated. *(adversarial review 2026-08-24, R5-4)*: plus an append-only corpus of **decrypted-payload JSON fixtures, one per DB-schema generation ever broadcast** (V9→current) — the JSON shape evolves with migrations independently of the envelope version byte, and G11 promises "recoverable", not merely "decryptable" | Current build recovers **every** binary fixture byte-exact; decode order is current-first-then-legacy as declared; **every schema-generation JSON fixture imports through the current import path to a spendable wallet** (the E5 class cannot return one generation later); deleting a legacy decode path or schema adapter fails CI | Any fixture that no longer recovers; any schema-generation JSON that no longer imports; any release lacking a legacy path | The supported-version table (BRC changelog appendix) |
| **H16** | new | **External-endpoint contract conformance (G12)** | For every endpoint in the contract registry: replay recorded fixture through the conformance test; smoke tier probes live WoC and diffs observed vs contracted semantics (lag window, pagination caps, unconfirmed visibility, error shapes, **rate-limit behavior, and spent-status semantics for plain P2PKH — the review's live probe found the two shipped oracles both NoSignal on markers, R1-01**); CI scans call sites against the allowlist | Every consumed endpoint has contract + fixture + green test; live-probe drift is either empty or triggers a recorded contract update — never silently absorbed; zero uncontracted call sites; a secondary provider passes the identical suite **plus the D12 P2PKH-spent/history indexing requirement plus** a reviewed shadow-mode divergence log — with the mock's correlated-lag and answer-flip models exercised (R5-8) — before promotion (D12) | Any uncontracted call site; drift silently ignored; secondary promoted without shadow soak or without the P2PKH indexing requirement | The endpoint-semantics table — exactly what each call does and does not return |
| **H17** | new *(adversarial review 2026-08-24 — R4-5, R4-6, R4-10)* | **Reorg / eviction suite** | Mock gains un-mine (reorg depth 1–2) and mempool-eviction (simulated ~14-day horizon) operations. Cases: (a) reorg of a counterparty tx after its raw_tx was stripped; (b) reorg of a backup token below the tip; (c) terminal backup token evicted, wallet idle; (d) terminal token evicted, wallet active (next write's parent absent — the D7 inverse-ghost heal) | (a) never occurs by construction — bytes strip only at the D14 confirmation depth, and a proven→unproven transition re-inflates the payload before the depth is reached again; recovery in the reorg window still restores every non-re-derivable tx; (b) chain walk survives (parents re-fetched or re-broadcast); (c) health goes red + rebroadcast within the declared horizon (G5 input 2); (d) the write path re-broadcasts the cached raw tx first, and only rebuilds from the last chain-visible ancestor on chain rejection — baseline stays consistent | Stripped bytes unrecoverable after a ≤2-block reorg; a walk wedged by an evicted parent; health green over an evicted tip; a rebuild that skips the rebroadcast-first rule or skews the baseline | Reorg-window recovery results; eviction MTTD |
| **H18** | new *(adversarial review 2026-08-24 — R5-6)* | **Live seed-only recovery smoke (per release)** | Dev wallet on real mainnet: real backup broadcast, wait for real indexer visibility, then on a clean environment restore from the 12 words **against live WoC/ARC** and build+broadcast one real spend of a recovered output | Restore completes against the real network; recovered balance matches; the spend is accepted by the real network; run recorded (date, txids, wall time) in the release notes — the first recorded field recovery, repeated every release | Any divergence from the mock-tier result; a release shipped with this red or unrun | Field-recovery record; live vs mock wall-time ratio |
| **H19** | new *(adversarial review 2026-08-24 — R1-05)* | **Multi-writer adopt/sweep safety (superseded-vs-tip race)** | Two devices under deltas: A holds a stale local tip (lagging unspent list); B has advanced the chain; A's write path runs marker adopt + orphan sweep (the shipped 5c/5d logic region, handlers.rs:13560-13600); repeated across lag values and with a crash-ghost successor marker present | A never sweeps a marker that decrypts under the wallet key and carries seq ≥ A's local tip; A adopts B's true tip (after applying its deltas, D13) rather than consuming it; the chain never forks onto a stale base; every sweep victim is provably superseded by chain linkage | The real tip consumed as a sweep input; a write extending a stale parent adopted from the lagging index; delta history destroyed | Sweep decision audit log format for the BRC |

README T8–T11 (cross-wallet: foreign import completeness, our export into foreign wallets,
preserve-unknown round-trip, capability-diff honesty) are kept verbatim for Phase 8 and are not
renumbered here.

---

## 6. Disagreements & unresolved

Quoted both sides; not averaged.

### 6.1 Docs vs code (settled by code; docs to be corrected)

1. **ONCHAIN_BACKUP_SYSTEM.md** — six verified errors (A1 §2, A3 §7, B2): claims the wallet row
   "Includes PIN salt, DPAPI blob" (code: `backup.rs:375-377` selects neither — comment "no
   mnemonic, no pin_salt"); claims "Service fee: 1000 sats to Hodos treasury" (code:
   `handlers.rs:13602` "No Hodos service fee for wallet backups"); says data is in the "unlocking
   script" (it is the locking script); names `prepare_backup_payload()` (does not exist —
   `compress_for_onchain`); says failed txs are "always kept" (excluded at SQL level); says recovery
   "Triggers full UTXO sync" (it sets `recovery_just_completed` for two tasks). **Until rewritten,
   cite code lines, not this doc.**
2. **BRC draft vs code — encryption key:** draft §3 "keys MUST be derived using BRC-42 … Security
   Level 2, Protocol 'wallet-backup'"; code: `SHA-256(master_privkey || "hodos-wallet-backup-v1")`
   (A1 §3a). Resolved by D4: draft follows code.
3. **BRC draft vs code — derivation string:** draft says level 2; code says `"1-wallet-backup-1"`
   (level 1, illegal hyphen per BRC-43). Resolved by D5: keep, document, migration on blocker list.
4. **BRC draft internal contradiction:** §5 specifies PushDrop; §6 "Storing Backups" still says
   "Output 0: OP_FALSE OP_RETURN with encrypted backup data" — a 2026-03 leftover. Draft fix,
   Appendix A.
5. **BRC draft vs registry:** §9 calls BRC-38/39/40 "reserved"; they are merged, written documents
   with implementations (B1, C1, C2). Resolved by D8.
6. **BRC draft vs code — chunking:** draft §5 describes chunking ("follows the same header with a
   chunk_index/total_chunks pair"); no chunking exists in code, and the live wallet broadcasts
   431 KB in a single push (A1 §3d). Resolved: Phase 4 hard cap; chunking honestly deferred.
7. **Work-plan README vs C3:** README 0b describes deggen's rail as "contiguous `seq` +
   `prevSha256` chaining" as parallel enforced features; code: the server enforces **only** seq
   contiguity; `prevSha256` is stored verbatim, never validated — "client-verified hash chain over
   ciphertext" is the accurate phrase (C3 §2, flag 3).
8. **Item-0b premise vs C1:** Ty's statement "toolbox remote-storage sync **is** BRC-38/39" vs
   code: sync moves raw rows per BRC-40; 38/39 are file formats; the shared merge engine is the
   overlap. Phrase per D10.

### 6.2 Doc vs doc (live disagreements this plan resolves or carries)

1. **Fix B:** `FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md` — crash-window fix designed and needed — vs
   `FOLLOWUP_RECORD_BEFORE_BROADCAST_TOKENS.md` (same date) — "This path is correct as-is and does
   NOT need changing." No implementing commit exists (A3 §6). **Resolved by D7** (intent record,
   Phase 4); "correct as-is" was only ever "eventually self-healing at some sat cost" (A3 §7.2).
2. **Table count:** README "we back up 18 tables" vs code: 19 SQL tables serialized, 21 payload
   members (A1 §2, B2 §0b agree). Corrected in §4.1.
3. **`proven_tx_reqs.history` shape:** our stored shape is a `{timestamp: note}` map; BRC-38
   requires `{"notes":[{"when","what"}]}` (B2 §1). A real transform — and B2 flags that our stored
   shape may itself diverge from wallet-toolbox convention; **unresolved**: check upstream before
   freezing the adapter (Phase 5).

4. **`domain_permissions` backed-up column count:** A1 §2 item 23 says the backup struct carries
   "only 5 of the table's columns"; B2 disagreement 4 says the SELECT takes 7 of 12. The two
   reports agree on the 3 dropped columns that matter, so Phase 2's fix list is unaffected; the H2
   manifest enumerates the live schema and settles the true count. **Unresolved between reports;
   carried.**

### 6.3 Unresolved — carried forward explicitly

1. **Delta sizes are unmeasured** — the §3.2 rule constants are provisional until H3/H9 report
   (the one place the plan still rests on the draft's 1–2 KB estimate).
2. **Fork tie-break rule** (Q2): **named normatively in D15** (lower `device_id` defers; the
   chain arbitrates); H4 validates it — no longer "decided after H4" *(adversarial review
   2026-08-24)*.
3. **Sync-vs-optimistic pre-spend poll** (Q1): decided by H11 numbers.
4. **Stuck-unconfirmed lifecycle policy** (§3.3): the *structure* is now specified (D14:
   `abandoned-unbroadcast`, inputs stay encumbered, limbo bound) *(adversarial review
   2026-08-24)*. Owner 2026-08-24: **structure ACCEPTED; the constants (age-out horizon, limbo
   bound N) remain owner-pending** — to be set at sprint kickoff from measurement/analysis, by
   what is best rather than what is expedient; until then the payload keeps carrying the raw
   bytes.
5. **BRC-38 amendment acceptance** is not in our control; D1's envelope posture is
   amendment-independent by design.
6. **Address migration** (D5) and **KDF migration** (D4): **REJECTED absent a forcing
   cryptographic break — owner decision 2026-08-24** (see the decision blocks in D4/D5); each
   remains a format-version event if a break ever forces it.
7. **Bug C's trigger** (why ARC returned DOUBLE_SPEND_ATTEMPTED for the winning tx, 2026-04-11) has
   three hypotheses and no recorded resolution (A3 §6); the H6 fault model reproduces the
   *symptom*; the root cause stays open upstream.
8. **How a foreign `.brc39` import obtains a seed at all** (A2 landmine; BRC-38 excludes root-key
   material) — open design question for Phase 8, flagged now.
9. **Mnemonic in the file-export payload:** the password-encrypted file export includes the real
   mnemonic while the module header claims it never does (A3 §7.4). Not an on-chain issue, but the
   shared `BackupPayload` struct means Phase 2's manifest must classify it explicitly.

---

## 7. What was NOT checked

Aggregated from the eight reports' NOT-checked lists, plus this plan's own limits.

**This plan's own limit:** the author of this plan read the four context docs and eight research
reports in full but **re-verified nothing against code, specs, or live data directly**. Every
VERIFIED label above is the verifying report's, at its cited lines, as of 2026-08-22 at repo commit
`6b4a4be` (Hodos), `c1d12f2` (BRCs), `8b074a0648` (ts-stack), `9d188d5` (go-wallet-toolbox),
`ca2136e` (go-private-backup-cache).

From the reports, the load-bearing gaps:

- **No runtime execution anywhere:** no report built or ran the wallet, the toolbox (TS or Go), or
  the backup cache; all "would reject / would break" interop claims are from reading both sides
  (C1, C2, C3). H1 in Phase 1 is the first runtime proof.
- **A1:** BRC-42 scalar math not re-derived against the spec; reconcile/BEEF/broadcast internals
  not read; whether any consumer of `proven_txs.merkle_path` handles the TSC-JSON-vs-binary format
  split (flagged risk); runtime fee rates (cost figures use 1 sat/KB assumption); the C++/frontend
  trigger side; macOS paths.
- **A2:** whether export/import handlers execute correctly today (last observed working
  2026-06-25); PBKDF2 iteration count; the bridge's large-export round-trip was never recorded as
  run.
- **A3:** full diffs of most commits (messages + stats read); C++ shutdown path claims taken from
  FIX_B §0; the episode catalog is a **lower bound** — silent failures got no commits; field-wallet
  divergence prevalence and total sats burned were never quantified; no record exists of a real
  user recovery succeeding in the field.
- **B1/B2:** BRC-42 test vectors read, not executed; whether live `certificates.type`/
  `serial_number` values are actually base64 (INFERRED-compatible; verify a live row in Phase 2);
  our restore-path code not read by B2 (its import assessment is schema+spec inference); BRC
  maintainers' process semantics of registry listing.
- **C1:** the seven entity `mergeExisting` bodies not read (assumed LWW like Transaction/Output);
  runtime behavior of imports containing unknown fields (traced, not run); `StorageIdb` paging.
- **C2:** whether TS actually infinite-loops on omitted entity arrays (Go's comment claims it,
  contradicting BRC-40 — unverified); ts-stack's current remoting state; Go merge logic beyond the
  read regions.
- **C3:** server route-assembly ordering (guard-before-auth rests on doc comments); the e2e/security
  test files; the published `@bsv/auth` package compatibility (repo's claims verified, not the
  compatibility itself); retention-guard gap arithmetic against a test.
- **Nobody checked:** RelayX/Rock export behavior (README item 7 "not checked in depth" stands);
  actual profile-A and profile-C payloads (no such fixture wallets exist yet — Phase 1 builds them).

---

## Appendix A — BRC draft update list (draft follows code + this plan)

1. §3/§4: KDF rewritten to the shipped `SHA-256(master_privkey || "hodos-wallet-backup-v1")`;
   version byte governs future KDF changes (D4).
2. §6: derivation documented as shipped (`"1-wallet-backup-1"`, level 1, non-BRC-43-conformant,
   with the conformant alternative and the address-migration consequence) (D5). Remove the
   OP_FALSE OP_RETURN remnant from "Storing Backups"; PushDrop throughout (§6.1.4).
3. §1.2: payload = envelope of strict BRC-38 + extensions; declare the stripped profile; reference
   the amendment PR (D1).
4. §7: delta format = BRC-38 portable row forms + per-table `deletes` + extension tables +
   re-fetchable-field omission with re-hydration rules; cite BRC-40 for merge/replay semantics;
   entity order per BRC-40 as specified (D2). Snapshot-rule constants marked provisional pending
   H3/H9.
5. §5: header discussion gains `prev_payload_sha256` (D3); chunking paragraph replaced with the
   hard-cap rule and an honest "chunking not defined in v1".
6. §6 recovery: address-history enumeration; decrypt-before-trust (newest-that-decrypts; no
   plaintext header field trusted before GCM verification); parent-walk with history-set
   reassembly; child-existence recency with the three-valued spent-status table;
   deltas-without-base fail-closed; error taxonomy from H6 (D6 as rewritten) *(adversarial review
   2026-08-24)*.
7. §9/Open Q1: renumber — never claim 38/39/40; update "reserved" language (D8).
8. Cost table: replace with §3.4 (measured column now; every *est.* replaced by Phase 8).
9. Security Considerations: add the no-erase asymmetry vs server-side rails (D3); keep
   GCM-tag-as-origin-proof, now also grounded in C2's transport-auth finding.
10. References: cite `bsv-blockchain/ts-stack/packages/wallet/wallet-toolbox` (wallet-toolbox repo
    archived), BRC-38/39/40 as merged registry documents, and `go-private-backup-cache` semantics
    (with the license caveat, no code reuse).
11. *(adversarial review 2026-08-24, R3-3)* §8 privacy claim **downgraded to what padding
    delivers**: size-class padding hides payload size/type from an observer of the backup address;
    it does **not** hide timing correlation with wallet activity or the funding/change linkage
    between the backup address and the main UTXO set (backup txs spend wallet UTXOs and return
    change; §3.3 makes broadcasts ~1:1 with wallet events). State the residual leak plainly
    instead of implying a stronger observer model.
12. *(adversarial review 2026-08-24)* §7 delta format additionally carries the per-row logical
    version counter (D2 arbiter) and the delete-emission allowlist rule (D14); recovery §6 replay
    order is parent-topological with `seq` as sanity check (D15); Security Considerations gains
    the abandoned-unbroadcast rationale (no oracle can prove a counterparty-held tx dead — D14)
    and the confirmation-depth strip constant (R4-5).
