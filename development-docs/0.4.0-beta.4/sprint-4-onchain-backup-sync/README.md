# On-chain Backup and Sync — delta chain + multi-device

**Opened:** 2026-08-19.
**Status:** 🔵 OUTLINE. Not scheduled into any sprint. This is the *shape* of the work; most rows
below need research and measurement before they become tickets. **We update the BRC as we
implement, test and learn — the draft follows the code, not the other way around.**

> **[2026-08-23]** `IMPLEMENTATION_PLAN.md` (this folder) is now the working plan, built from the
> eight `research/` reports and revised after critique. Where text below is marked
> *[superseded ...]* the plan wins; unmarked text stands. The core rule is unchanged: **the BRC
> draft follows the code.** Nothing below is deleted — history and decision records are annotated
> in [brackets], not erased.

> **What this is.** Today `rust-wallet/src/backup.rs` writes the whole (stripped) wallet state as one
> PushDrop token on every backup. This work changes that to a **linked chain of snapshots and
> deltas** at the same deterministic address, and uses that chain as the **cross-device sync log**.
> The design is specified in the BRC draft; this folder is the implementation, measurement and test
> plan.
>
> **Where the design lives:** `Marston Enterprises/Standards/BRCs/drafts/wallet-backup-and-sync-onchain/`
> — `wallet-backup-and-sync-onchain.md` (the BRC, revised 2026-08-19) and `DELTA_ANALYSIS.md` (why, with
> the numbers and the decisions on record).
> **Current implementation doc:** `../../ONCHAIN_BACKUP_SYSTEM.md` — the five strips, triggers,
> dirty-flag, recovery flow. ~~Still accurate for what ships today~~ *[superseded 2026-08-23: that
> doc has ≥ 6 verified factual errors — IMPLEMENTATION_PLAN.md §6.1. Cite code lines, not the doc,
> until it is rewritten post-Phase 2.]* This work builds on the shipped system, doesn't
> replace it.

---

## The design in one paragraph (plain terms)

A **snapshot** is a photocopy of the whole wallet database. A **delta** is a sticky note: "row 417
spent, row 418 added." Both are written as backup tokens to the one BRC-42 backup address every
wallet already derives from its seed. Every token carries the txid of the token it sits on top of,
so the history is a chain. Recovery = newest photocopy + every sticky note after it, in order, all
from the seed. Every device that shares the seed reads and writes the same chain — so the backup
chain *is* how devices find out what the other devices did. Rules: read to the tip before you
write; poll on a timer and right before any spend; every token says which device wrote it.

---

## Work item 0 — BRC-38 compatibility assessment. **Do this first. It gates everything else.**

**[2026-08-23: DONE — report B2. Decision gate: (a) amendment preferred and pursued, (b) envelope
ships now — IMPLEMENTATION_PLAN.md D1. Corrections from B2: we serialize 19 SQL tables (21 payload
members), not 18; the only tier-1 orphan is the single field `wallets.current_index`, not five
tables.]**

**Why it's first.** The BRC now says the backup payload SHOULD be a **BRC-38** document (Ty Everett's
*User Wallet Data Format*, written 2026 — no longer "reserved"). Our schema was *based on*
wallet-toolbox but has drifted: we back up 18 tables (`../../ONCHAIN_BACKUP_SYSTEM.md` § Included
Tables), BRC-38 defines exactly 13 and **MUSTs that list with no extension slot**. If the gap is
small, the rest of this plan proceeds as written. If it's large, **that is a big problem with a lot
of work**, and the sprint has to start by identifying it clearly and planning from that — not
discovering it halfway through item 3. Matt, 2026-08-19.

**Deliverable: a table, one row per table we back up**, with columns:
`our table` · `BRC-38 table (§ 7.x)` · `maps cleanly / field diffs / no BRC-38 home` · **`tier`** ·
`what to do`.

**Classify by tier, not just by "has a BRC-38 home"** (Matt, 2026-08-19 — this reframes the gap):

| Tier | What | Export | Import | Why |
|---|---|---|---|---|
| **1 Core** | derivation data (never secrets), unspent outputs + spending instructions, baskets | MUST | MUST | the user's money; a wallet that can't take this can't take the user |
| **2 Assets & attestations** | certificates + keyrings, BBS credentials, token outputs with `customInstructions`/provenance, labels, tags | MUST | **MUST preserve on round-trip; MAY decline to *activate*; MUST tell the user what it can't use** | the user's stuff — carry it unopened rather than lose it |
| **3 Wallet-local** | `domain_permissions`, `cert_field_permissions`, UI prefs, device labels, sync state | MAY | MAY ignore; SHOULD preserve | nice-to-have, vendor-specific, no user harm if dropped |

So: `wallet` / `domain_permissions` / `cert_field_permissions` → tier 3, fine outside BRC-38.
`credentials` → tier 2, needs a labelled home. The "five tables with nowhere to go" becomes **one
table that matters and four that don't.** *[2026-08-23: B2 measured the gap smaller still — all 13
BRC-38 tables + `user` + `sourceStorage` exist in our schema with zero missing columns; the only
tier-1 orphan is `wallets.current_index` (plan D1).]*

**BRC-38 as written has no tiers and no ignore rule** — § 6 MUSTs every row of every table, § 10 MUSTs
importers preserve everything, `tables` MUST be exactly the 13. The only flex is § 5.4 (omit absent
fields) and "MAY choose not to *activate* `activeStorage`/`syncStates`" *[2026-08-23: §10's
actual words: importers "MAY treat the following as operationally sensitive and choose whether
to activate them immediately after import" (0038.md §10, verified at c1d12f2)]*. So BRC-38 is the **tier-1
+ most-of-tier-2 payload**, and what's missing around it is: the preserve-unknown rule, tier 3, and
the capability declaration below.

Known before we start (from reading, not running):

| Ours | BRC-38 | Expected |
|---|---|---|
| `users`, `proven_txs`, `proven_tx_reqs`, `output_baskets`, `transactions`, `commissions`, `outputs`, `output_tags`, `output_tag_map`, `tx_labels`, `tx_labels_map`, `certificates`, `certificate_fields`, `sync_states` | § 7.1–7.14 | Should map; **field-level diffs likely** (we've added columns, e.g. `outputs.sender_identity_key`, `custom_instructions`, `confirmed`) |
| `settings` | `sourceStorage` (export metadata only) | Partial — ours holds user prefs too |
| `wallet` (PIN salt, DPAPI blob, `current_index`) | — | **No home** — and BRC-38 § 1 explicitly excludes root-key / profile material. Probably correct to keep out of the portable doc; decide |
| `domain_permissions` | — | **No home** — our per-site permission engine |
| `cert_field_permissions` | — | **No home** |
| *(soon)* `credentials` (BBS profile) | — | **No home** |
| `addresses` (HD cache) | — | Not in 38; re-derivable; probably fine to drop from the portable doc |

**Decision gate at the end of item 0** — pick one, write it down, then continue:

- **(a) Small amendment to BRC-38** — propose (i) a **`contains` declaration** listing what the
  document carries, keyed by BRC number or vendor name (`["brc-38/core", "brc-52/certificates",
  "brc-147/1sat", "brc-xxx/bbs-credentials", "hodos/domain-permissions"]`), and (ii) a
  **preserve-unknown rule**: members outside the 13 tables MUST be preserved on round-trip and MAY
  be ignored on import. Two paragraphs; makes 38 the core tier of a bigger picture rather than the
  whole picture. **Preferred.** The importing wallet declares what it *supports*, diffs against
  `contains`, and **shows the user before anything happens**: "contains 14 1Sat ordinals and 2 BBS
  credentials; this wallet supports ordinals, does not support BBS credentials — they will be kept
  but not usable here." That is how a user decides whether they *can* move, and how a wallet that
  is behind the newest thing stays *honest* rather than *incompatible*. Extensibility falls out:
  a new asset type is a new labelled section old wallets carry and new wallets open — no format
  redesign, ever.
- **(b) Envelope** — our token carries a BRC-38 document *plus* a sibling extensions object. Works
  without asking anyone; weaker interop story.
- **(c) Native payload for now**, `kind` flag says so, migrate later. Only if (a)/(b) turn out to
  need schema changes we can't afford this sprint.

Also out of item 0: the list of **field-level adapters** needed (our column ↔ 38 column), and
whether *import* from a foreign BRC-38 document is in scope for this sprint or the next.

## Work item 0b — delta-format prior art. **Added 2026-08-22; gates item 3's delta format.**

**[2026-08-23: DONE — reports C1/C2/C3; decisions D2/D3/D10 in IMPLEMENTATION_PLAN.md. Premise
corrections from the reports: the backup-cache blob is opaque ("Nothing in this package interprets
blob contents"), so their rail imposes no constraint on our chunk format; the server enforces only
`seq` contiguity — `prevSha256` is stored verbatim, never validated (accurate phrase:
"client-verified hash chain over ciphertext"); and "toolbox sync IS BRC-38/39" is imprecise — sync
moves raw rows per BRC-40, 38/39 are file formats, the shared merge engine is the overlap (plan
D10).]**

**Why this exists.** Two things became public this week:

1. **deggen's [`go-private-backup-cache`](https://github.com/bsv-blockchain/go-private-backup-cache)**
   (bsv-blockchain org, created 08-15, pushed 08-21; his answer in the HandCash-goes-non-custodial
   recovery thread) is a live **off-chain delta rail**: wallets push their DB as client-side-encrypted
   **delta chunks**, per-device append logs with contiguous `seq` + `prevSha256` chaining, generations
   with client-driven compaction (snapshot N+1, delete N-2). Same problem statement as ours, nearly
   verbatim (BRC-29 derivation metadata is unrecoverable from seed alone).
2. **Ty Everett publicly stated** (Metanet Meetup 2026-08-21, clip x.com/hbgnostic/status/2091192000376951142)
   that wallet-toolbox remote-storage sync **is BRC-38/39**: "synchronize and download all of my data
   out of remote wallet toolbox storage… put it into another one or run it myself."

So a delta format for wallet state **already exists in running code**, on the rail ours is the
on-chain sibling of. Item 3 must not invent a delta format in a vacuum.

**Deliverable, before item 3 freezes anything:**

- **(a) Read wallet-toolbox's sync machinery** — what a sync chunk actually is on the wire
  (`SyncChunk` et al.), how it relates to **BRC-40** (*User Wallet Data Synchronization*), what the
  chunk boundary and merge/replay rules are, and how it maps to the BRC-38 table list from item 0.
- **(b) Read `go-private-backup-cache`'s clients** (`client/client.go`, `ts-client/`
  `@bsv/backup-cache-client`) — is the blob toolbox-native (BRC-40-shaped?) or opaque/client-defined?
  Note the log semantics we'd share: contiguous seq, parent hash, generations-as-snapshots.
- **(c) Decision on record: adopt / adapt / diverge-with-reasons** for our token payload (item 2's
  header + item 3's changeset format). **If the chunk formats align, one wallet pushes the same
  encrypted chunks to either rail** — his HTTP log or our chain — and our BRC becomes the transport
  sibling of a running service instead of a competitor. That also answers item 7's "adopt what
  already works or lead" for the delta layer before implementation starts.

Caution: the repo has **no license** (same pattern as the 1sat stack) — read for format and
semantics; do not vendor code. Analysis on record:
`Marston Enterprises/Standards/BRCs/drafts/wallet-backup-and-sync-onchain/` + memory
`project_onchain_backup_delta_design`.

## Work items — recommended order

Ordered so that each step is independently shippable and measurable. 1 is the biggest single win
and the smallest change; ~~do it first regardless of the rest~~ *[superseded 2026-08-23: the test
harness comes first — IMPLEMENTATION_PLAN.md Phase 1, per retrospective constraint 13 (never ship
recovery-affecting changes without a round-trip test). Item 1 lands as Phase 3.]*

*[2026-08-23 item → phase map: 0 → done (plan D1); 0b → done (D2/D3); 1 → Phase 3; 2 → Phase 4
(extended: intent record, recency check, `prev_payload_sha256`, hard size cap); 3 → Phase 5;
4 → Phase 6; 5 → Phase 7; 6 → Phase 8 (H8a front-loaded to Phases 1–2); 7 → Phase 8. New Phases 1
(test harness) and 2 (payload completeness on the current format) precede all of them.]*

| # | Item | Size | Why this order |
|---|---|---|---|
| **0** | **BRC-38 compatibility assessment + decision gate** (above) | S–M | Gates the payload format for everything below |
| **0b** | **Delta-format prior art** — wallet-toolbox sync chunks (BRC-40?) + `go-private-backup-cache` blob shape (above) | S | Gates item 3's delta format; if chunks align, the same encrypted chunks feed both rails |
| **1** | **Strip inscription bytes** from token rows — back up outpoint + basket + tags + `custom_instructions` + derivation; re-hydrate `locking_script` by outpoint on recovery | S | Same "pure cache, re-fetch" rule the five existing strips already apply. Takes a 200-image-ordinal wallet from ~3 MB → ~100 KB *before* deltas. Becomes strip rule 6 in ~~`prepare_backup_payload()`~~ `compress_for_onchain` *[2026-08-23: the named function does not exist — A1 §2 item 4; plan Phase 3]* |
| **2** | **Token header** — `version \| kind \| seq \| parent_txid \| device_id` on every PushDrop backup token; snapshot-only at first (kind = 0), so the chain exists before deltas do | S | Lets 3 and 5 be added without a format break. Recovery already walks the address; it now follows parents |
| **3** | **Deltas** — row-level changeset producer (diff current payload vs last backed-up), snapshot-on-ratio rule (0.5× / 20 deltas / 16 KB cap), recovery replays snapshot + deltas | M | The normative format in the BRC (§ 7). Resolves the draft's old Open Question #2 *[2026-08-23: format decided by plan D2 — BRC-38 portable row forms + per-table `deletes`, BRC-40 merge semantics; the 0.5× / 20 / 16 KB constants all stay provisional until H3/H9 measure real deltas (plan §3.2)]* |
| **4** | **Size-class padding** — pad payloads to 1 / 4 / 16 KB classes | S | Deltas make size ≈ activity; one line of code, one line in the BRC |
| **5** | **Multi-device sync** — `device_id` assignment + label in settings; poll on timer; **poll before any spend / reserve**; read-before-write; fork detection + re-read-re-write | M–L | This is the part that needs the most testing. See "Open questions" |
| **6** | **Measure and redo the BRC cost table** with real payloads for profiles A / B / C | S | The current draft numbers are *estimates*; the BRC says so and must be corrected from data |
| **7** | **Cross-wallet proof — export/import against real wallets** (below) | M | Proves the portability model with other people's wallets, not just ours. Either we adopt what already works or we lead |

## Work item 7 — cross-wallet proof (Matt, 2026-08-19)

**[2026-08-23: stands → plan Phase 8. A2 verified the import backend is live, not rotted (the
shared `collect_payload` / `import_to_db_with_ids` machinery runs daily on the on-chain path; only
the UI is hidden); the four landmines are enumerated in plan Phase 8.]**

**Goal.** Prove the method with *other* wallets: export a Yours wallet / BSV Browser / MetaNet Client
database and import it into Hodos; import them into each other; export Hodos and import it into
one of those. Not a deep dive up front — **start by looking at what those wallets do and don't do
for import/export right now**, then build what we can as a proof-of-concept that goes into the
production product. Outcome is one of two: we **adopt** what's already working, or we **lead**.

**Step 1 — survey (results below once in).** For each wallet: does it export state at all (not
just seed/WIF)? what format? is it BRC-38-shaped? does it import anything beyond a seed? is it
wallet-toolbox-based (so BRC-38 is "free" for it)?

Survey done 2026-08-19 (code read, not guessed — VERIFIED unless marked):

| Wallet | Exports state? | Format | BRC-38? | Imports beyond seed? | Toolbox-based? | Source |
|---|---|---|---|---|---|---|
| **wallet-toolbox** (ts-stack, v2.10.2) | **YES — ships `exportBRC38` / `exportBRC39` / `importBRC38` / `importBRC39`** since 2026-05-14 (PR #117, ≈2.1.26+), in both `@bsv/wallet-toolbox` and `-client` | BRC-38 JSON exactly as spec'd; BRC-39 = magic `WDAT`, Argon2id + AES-GCM | **Yes, reference impl** | `importBRC38(storage, data, {mode:'merge'\|'restore'})` | — | `src/storage/portable/index.ts`, tests `test/storage/portable.test.ts`. **BRC-38's own text ("does not yet define a single-file export") is stale.** |
| **HandCash Desktop** (public since 2026-07-29, v1.2.264) | **YES — file + cloud** | **`.brc39`** (BRC-38 JSON inside), media type `application/vnd.brc39.wallet`; encrypted with a root-key-derived secret | **Yes — the only shipping wallet using 38/39 today** | `importBrc39FromFile`, `downloadAndRestoreBrc39Backup` | yes (`-client` 2.4.4) | `src/wallet/historyBackup.ts`, `HistoryBackupPanel.tsx` |
| **Yours Wallet** | YES — "Master Backup" ZIP | ZIP: `manifest.json` + encrypted keys blob + `<identity>/chunk-NNNN.bin` = **msgpack BRC-40 SyncChunks**. Chunks compressed, **not encrypted** (only keys are) | **No** — it's the BRC-40 sync stream on disk, not the 38 document | "Restore from Backup" replays chunks via `storage.syncFromReader` | yes (`-client` 2.4.4 / `@1sat/wallet`) | `src/backup/WalletBackupService.ts`, `masterExporter.ts`/`masterImporter.ts`; reader in `@1sat/wallet-browser` |
| **BSV Desktop** (bsv-blockchain, v2.8.2) | **No file.** "Backup" = add a remote/local *storage provider* and live-sync to it | live BRC-40 storage-to-storage sync; no artifact | No BRC-38/39 calls in repo | no | yes (`^2.4.4`) | `src/lib/services/WalletService.ts` (`addBackupStorageUrl`, `syncBackupStorage`) |
| **MetaNet Client** | — | — | — | — | — | **Repo archived 2025-10-27**, superseded by BSV Desktop. Drop from the matrix. |
| RelayX / Rock / others | not checked in depth | | | | | COULD NOT FIND quickly |

**What this changes (read before picking the first pair):**

1. **BRC-38/39 are no longer a spec without code — they're shipping.** Toolbox has the reference
   export/import; HandCash ships `.brc39` files in production. So the tier-1/tier-2 payload
   **exists and has two implementations.** Our job for that layer is *adopt*: read
   `storage/portable/index.ts`, match its row forms, and ship Import/Export of `.brc39`. That's the
   "two weeks, not six" branch.
2. **Two incompatible file formats exist in the wild right now** — HandCash's `.brc39` and Yours'
   ZIP-of-SyncChunks. Both toolbox-based, both carry full state, neither reads the other. **That is
   the portability problem in one sentence, and it's a concrete argument for the capability
   declaration + preserve-unknown work, which is where we lead.** (INFERRED: Yours adding `.brc39`
   is a thin wrapper since they're already on toolbox-client — same pattern HandCash used. Worth
   asking shruggr.)
3. **Yours' chunks are plaintext inside the ZIP.** Only the keys blob is encrypted. Worth knowing
   before anyone treats a Yours ZIP as a safe artifact.
4. **Nobody exports our tier 3** (permissions, device labels) — expected; that's the envelope /
   `contains` gap.

**First pair, decided by the survey:** **HandCash `.brc39` → Hodos import** (real production
artifact, pure BRC-38 inside, reference importer in toolbox to diff against). Then **Hodos `.brc39`
→ HandCash import** (their importer is in the open; T9 is possible). Then Yours ZIP as the
"non-38 format" case — and if that's a hassle, the honest finding is "two formats, one should win."

**Step 2 — pick the first pair.** Decided above: HandCash `.brc39` → Hodos, then reverse, then Yours
ZIP. *Their export → Hodos import* first. That exercises our
tier-1/tier-2 mapping against a real foreign document. Then the reverse. Then a non-toolbox wallet
(Yours) if it exports anything at all — if it doesn't, that's a finding, not a blocker.

**Step 3 — the PoC that ships.** Minimum: a Hodos **Import** that accepts a BRC-38 (or BRC-39)
document, shows the `contains`-vs-supports diff **before** touching the DB, imports tier 1, preserves
tier 2 it can't activate, and runs the liveness check (Open Q7) before the first spend. And a Hodos
**Export** that writes a BRC-38 document with `contains` + our tier-3 sections labelled. If the
amendment to 38 (decision gate (a)) isn't accepted by then, ship it as envelope (b) and keep the
inner document pure 38.

**Tests (add to the table above).**

| ID | Test | Proves |
|---|---|---|
| **T8** | Foreign export → Hodos import, tier-1 complete: every unspent output spendable afterwards; balance matches | we can take a user |
| **T9** | Hodos export → foreign import (where the foreign wallet has an import): same check from their side | they can take ours |
| **T10** | Round-trip through a wallet that doesn't support X (e.g. BBS credentials): X survives unchanged | preserve-unknown works |
| **T11** | Capability diff shown before import, and matches what actually happened | the user was told the truth |

**What this is not.** Not a promise to support every wallet's private format. If Yours exports
nothing structured, the finding is "Yours users move by seed + chain rescan, and lose labels" — and
that's a fair thing to say publicly, because it's the case for a standard.

## What's already decided (don't re-open without new evidence)

- **Deltas, not whole snapshots**, as the normative format. Matt: "no-brainer."
- **Read-before-write, then append** — *not* "a second device must write a snapshot." That earlier
  rule was wrong and is withdrawn; the parent pointer + read-to-tip makes plain appending safe.
- **`device_id` is a number on chain; the label is in encrypted settings.** Nothing identifying on
  chain; recovery restores the mapping.
- **No extra signature for proof-of-origin.** The AES-256-GCM tag already proves a token was
  written by this wallet; foreign tokens fail decryption and are skipped. The parent chain bounds
  the cost of an attacker littering the address.
- **Snapshot trigger is a ratio + count, not a calendar.** Self-tunes for static vs busy wallets.

*[2026-08-23: all five stand after the research pass; "no extra signature" is additionally
supported by C2's finding that BRC-40 delegates auth to the transport session — the GCM tag is the
on-chain equivalent (plan §4.1).]*

## Open questions — research before ticketing

1. **"Check before every action" — user-visible latency?** A poll = one WoC / indexer query for the
   backup address + fetch of any new token. Probably tens to a few hundred ms. Questions: do it
   synchronously before a spend (correct, slower) or optimistic with a conflict check at broadcast
   (faster, more complex)? Can we cache "chain tip unchanged since last poll" cheaply? **Measure
   first** (test T5 below) — don't design around a guess. *[2026-08-23: decided by H11's measured
   p50/p95 numbers — plan §5, Phase 7.]*
2. **Fork tie-break.** Two devices write in the same broadcast window → two children of one parent.
   The BRC says re-read-and-re-write. Is a deterministic tie-break (lower `device_id` wins) also
   needed so both devices converge on the same branch without a third round? Probably yes; decide
   after T4. *[2026-08-23: stands — decided after H4 (T4) data, not before; plan Phase 7.]*
3. **Delta producer state.** Diffing needs "what did I last back up." Keep the last payload locally
   (simple, doubles storage) or a per-row hash map (smaller, more code)? If local state is lost →
   write a snapshot. Fine either way; pick by measurement. *[2026-08-23: simple
   last-payload copy first, measure later — plan §3.3.]*
4. **Indexer dependence.** Recovery and polling both read through WoC today. A 20-delta chain is
   21 fetches. Acceptable for recovery; is it for a pre-spend poll? Ties to Q1. *[2026-08-23: H7/H11 measure
   it — plan §5.]*
5. **Interaction with `sync_states`.** The table exists for multi-device already. Does the chain
   *replace* it or *feed* it? Read `sync_states` usage before deciding.
   *[2026-08-23: answered — 0 rows in both live DBs, nothing writes it; the chain replaces it for
   our devices; the table is carried in the payload, not activated (plan D9).]*
6. **Chunking + deltas.** Large snapshots chunk today; deltas shouldn't need to (16 KB cap). Confirm
   the chunk header composes with the new token header. *[2026-08-23: superseded — no chunking
   exists in code (the draft described chunking that does not ship); Phase 4 adds a hard
   pre-broadcast size cap that fails closed, chunking honestly deferred (plan §6.1 item 6).]*
7. **Handover / deconfliction — the "user puts a seed into a second wallet" problem.** Not this
   sprint's scope to *solve*, but this sprint's scope to *not make worse*. Two wallets (ours and a
   vendor's, or two of ours) with one seed, not syncing, both spending = double-spends and a user
   who doesn't know what a UTXO is blaming the wallet. We should not try to dictate policy —
   everyone will say "user's responsibility," and they're right — but **build to the lowest common
   denominator with cheap gates:** (i) on import/recovery, **liveness check before first spend** —
   if the DB says unspent and the chain says spent, another wallet is active; stop and say so;
   (ii) show the user *what was recovered and what wasn't* before any action; (iii) if an on-chain
   backup chain exists at the seed's address and it wasn't written by this `device_id`, warn that
   another wallet may be live. These cost almost nothing and save developers the headaches later.
   Belongs in a separate **Wallet Portability / Handover BRC** (outline TBD — see
   `Standards/BRCs/README.md`); record here so item 5 leaves room for the hooks.
   *[2026-08-23: stands — out of scope except the three cheap gates, kept and asserted in H1/H9
   (plan §1 non-goals).]*

## Tests — these are the deliverable, not the code

The whole point is to stop estimating. Each test has a pass condition and produces a number for
the BRC.

| ID | Test | What it proves | Output for the BRC |
|---|---|---|---|
| **T1** | **Recovery from seed only, fresh machine**, with a chain of 1 snapshot + N deltas (N = 0, 1, 5, 20). Wipe DB, restore, diff against original DB | Recovery actually works; chain walking is correct; a break is reported not silently swallowed | Recovery time vs N; fetch count |
| **T2** | **Recovery with junk at the address** — send 50 random PushDrop tokens to the backup address from another wallet, then recover | Foreign tokens are skipped by GCM failure; chain following bounds the cost | Recovery time with/without litter |
| **T3** | **Cost measurement** — real payloads for profiles A (payments), B (500 text tokens), C (200 image ordinals): snapshot bytes, delta bytes, fee per tx, at 4 writes/day × 30 days | Replaces every *est.* in the BRC | The cost table |
| **T4** | **Two devices, same seed** — phone + laptop (or two profiles): spend on A, poll on B, confirm B sees it; then write from both within one second and confirm the fork is detected and resolved, with no double-spend and no lost row | Multi-device sync is real; fork handling works | Fork rate under stress; resolution rounds |
| **T5** | **Pre-spend poll latency** — time the read-to-tip + apply path on a cold and warm indexer, 100 runs | Answers Open Q1 with data | p50 / p95 added latency per spend |
| **T6** | **Strip-and-rehydrate** — back up a wallet with image ordinals under item 1, recover, confirm every `locking_script` re-fetched by outpoint byte-matches the original | Strip rule 6 is lossless | Bytes saved; rehydrate time |
| **T7** | **Padding** — confirm delta sizes land in the declared classes and the class distribution doesn't leak which action type happened | Privacy claim in BRC § 8 holds | Size histogram |

T1, T3 and T6 can run on one machine and should come first. T4 and T5 need two devices and are the
ones with the most unknowns.

*[2026-08-23: T1–T7 are mapped into the plan's H-test harness, kept and made stricter — T1→H1,
T2→H10, T3→H8, T4→H4, T5→H11, T6→H13, T7→H12 (plan §5). T8–T11 (item 7) kept verbatim for
Phase 8. The harness adds H2 (schema drift gate), H3 (delta replay property), H5 (crash matrix),
H6 (corruption), H7 (boundary restore), H9 (90-day soak).]*

## Files this will touch (from reading, not yet confirmed by doing)

- `rust-wallet/src/backup.rs` — `collect_payload()`, ~~`prepare_backup_payload()`~~ `compress_for_onchain` *(strip rule 6 — the named function does not exist; A1 §2 item 4)*,
  `serialize_for_onchain()` / `deserialize_from_onchain()` (token header), new delta producer,
  recovery chain walk
- `rust-wallet/src/monitor/task_backup.rs` — triggers, snapshot-on-ratio rule, polling
- `rust-wallet/src/database/` — last-backed-up state for diffing; `device_id` in settings;
  `sync_states` interaction
- wherever the pre-spend path lives in `createAction` / `send_transaction` — the poll hook (Q1)
- `../../ONCHAIN_BACKUP_SYSTEM.md` — update once items land

## Related

- BRC draft + analysis: `Marston Enterprises/Standards/BRCs/drafts/wallet-backup-and-sync-onchain/`
- The revocation-registry design that prompted this ("write deltas, not snapshots"):
  `Marston Enterprises/Standards/BRCs/drafts/bbs-unlinkable-credentials/NOTES.md`
- `../../ONCHAIN_BACKUP_SYSTEM.md` — current implementation
- `../../Wallet-Hardening/` — where the backup's threat model was last discussed
