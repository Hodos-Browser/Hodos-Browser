# C1 — wallet-toolbox (TypeScript): sync chunks, BRC-40, portable export/import

**Date:** 2026-08-22. **Author:** research subagent (task C1; supersedes the 16:29 draft from an
interrupted earlier run — same conclusions, this version completes the schema and decision sections).
**Source of truth:** `bsv-blockchain/ts-stack` monorepo, commit `8b074a0648` (2026-08-14), package
`packages/wallet/wallet-toolbox`. The standalone `bsv-blockchain/wallet-toolbox` repo is **archived**
(final commit `320a82b`, 2026-06-12; README is a permanent redirect to
`ts-stack/tree/main/packages/wallet/wallet-toolbox`; the archived snapshot has **no**
`src/storage/portable/`). All file paths below are relative to `packages/wallet/wallet-toolbox/` in
the local sparse clone at
`C:/Users/archb/AppData/Local/Temp/claude/C--Users-archb-Marston-Enterprises-Standards-BRCs-drafts/8069cf43-d637-4ddb-9a45-1ef359a2624b/scratchpad/repos/ts-stack`.
BRC spec texts from local clone of `bsv-blockchain/BRCs`, commit `c1d12f2` (2026-08-21) — BRC-38/39/40
are merged, indexed files (`outpoints/0038.md`, `0039.md`, `0040.md`; `README.md:104-106`), not
reservations.

Labels: **VERIFIED** = read in code/spec at the cited lines. **INFERRED** = my conclusion from
verified facts.

---

## 1. Sync machinery: what a sync chunk is on the wire

### 1.1 The two structures (VERIFIED — `src/sdk/WalletStorage.interfaces.ts:537-620`)

**`RequestSyncChunkArgs`** (consumer → producer):

| Field | Meaning |
|---|---|
| `fromStorageIdentityKey` | storageIdentityKey of the **producer** (data supplier) |
| `toStorageIdentityKey` | storageIdentityKey of the **consumer** |
| `identityKey` | wallet user's identity pubkey — whose data is requested; the identity anchor |
| `since?: Date` | watermark; **inclusive**: "must include items if 'updated_at' is greater or equal" (interface comment, line 553; enforced as `where('updated_at', '>=', since)` in `StorageKnex.ts:250,274,292,314,787` and `since > r.updated_at → skip` in `StorageIdb.ts:108`) |
| `maxRoughSize` | rough byte budget; "The item that exceeds the limit is included and ends adding more items" |
| `maxItems` | max records across **all** entity arrays combined |
| `offsets: {name, offset}[]` | per-entity resume offsets, must be in exact dependency order |

Defaults when the caller doesn't override (`EntitySyncState.makeRequestSyncChunkArgs`,
`src/storage/schema/entities/EntitySyncState.ts:302-334`): `maxRoughSize = 10_000_000`,
`maxItems = 1000`, `since = syncState.when`, offsets from the syncMap's per-entity `count`.

**`SyncChunk`** (producer → consumer), `WalletStorage.interfaces.ts:595-609`:

```ts
{ fromStorageIdentityKey, toStorageIdentityKey, userIdentityKey,
  user?: TableUser,
  provenTxs?: TableProvenTx[], provenTxReqs?: TableProvenTxReq[],
  outputBaskets?, txLabels?, outputTags?, transactions?, txLabelMaps?,
  commissions?, outputs?, outputTagMaps?, certificates?, certificateFields? }
```

The arrays hold **raw storage table rows** — the same `Table*` interfaces the database layer uses
(camelCase fields, `Date` objects for timestamps, `number[]` for binary; e.g. `TableOutput`,
`src/storage/schema/tables/TableOutput.ts`). There is no separate wire row form. Semantics per
property: `undefined`/omitted = entity type not attempted this chunk; `[]` = attempted, nothing
further; non-empty = merge these. `ProcessSyncChunkResult = { done, maxUpdated_at, updates,
inserts, error? }`. A `SyncProtocolVersion = '0.1.0'` type exists (line 535) but no wire field
carries it.

### 1.2 Entity order and chunk boundary rules (VERIFIED — `src/storage/methods/getSyncChunk.ts`, all 297 lines read)

The producer iterates chunkers in this exact order (with each entity's `maxDivider`):

1. `provenTx` (100) · 2. `outputBasket` (1) · 3. `outputTag` (1) · 4. `txLabel` (1) ·
5. `transaction` (25) · 6. `output` (25) · 7. `txLabelMap` (1) · 8. `outputTagMap` (1) ·
9. `certificate` (25) · 10. `certificateField` (25) · 11. `commission` (25) · 12. `provenTxReq` (100)

Three places agree on this order: the chunker array (`getSyncChunk.ts:39-218`),
`makeRequestSyncChunkArgs` (`EntitySyncState.ts:317-332`), and the merge order in
`processSyncChunk` (`EntitySyncState.ts:373-386`). BRC-40 §Record Model matches it too.

Rules, from the `addItems` closure (`getSyncChunk.ts:225-262`):

- Per-DB-query page size = `min(remainingItemCount, max(10, maxItems / maxDivider))` — heavy
  entities (provenTx, provenTxReq) are fetched in smaller pages.
- Each added item decrements `itemCount` and subtracts `JSON.stringify(item).length` from
  `roughSize`. When either hits ≤ 0, `done = true` **after including the offending item**.
- The offsets array is consumed positionally; a name mismatch throws
  `WERR_INVALID_PARAMETER('offsets', "in dependency order...")` (line 231).
- `preAdd()` initializes the entity's array to `[]` only once the entity is attempted — this is
  what produces the omitted-vs-empty distinction on the wire.
- The `user` row is included iff `since == null || user.updated_at > since` (strictly greater;
  line 37).
- `checkEntityValues` (lines 281-297) throws if any row has a non-`Date` `created_at`/`updated_at`
  or any `null` value — omitted fields must be absent, never null.

**Identity/anchoring:** rows carry the **producer's** `userId` and primary IDs; the consumer never
trusts them. The user is anchored by `identityKey` (producer resolves it via
`findUserByIdentityKey`, line 36); the consumer resolves its own `userId` and remaps all primary
IDs (§1.4). The watermark is the single `since` Date plus per-entity offsets — no per-row sequence
numbers, no vector clocks.

**No deterministic ORDER BY (VERIFIED absence):** the paged queries used by `getSyncChunk` (e.g.
`getProvenTxsForUserQuery`, `StorageKnex.ts:240-252`; the generic `find*` methods) apply
`limit`/`offset` and the `since` filter but no `orderBy`. Resumable offsets rely on the engine's
default row order (rowid/PK order in SQLite in practice). See §2 — BRC-40 *requires* a stable
deterministic order; the Knex implementation does not emit one explicitly.

### 1.3 Wire encoding over remoting (VERIFIED)

- Transport is JSON-RPC over a single Express POST endpoint (`src/storage/remoting/StorageServer.ts:5,517`).
  `getSyncChunk` and `processSyncChunk` are both whitelisted RPC methods (lines 79, 89).
- The server **clamps** client budgets: `args.maxItems` through `normalizedRpcLimit`, and
  `args.maxRoughSize` down to `maxRpcResponseBytes` (lines 640-650).
- On JSON serialization, `Date` becomes an ISO string and `number[]` binary stays a JSON array of
  numbers. The receiving side re-canonicalizes with `validateSyncChunkEntities`
  (`src/storage/remoting/entityValidationHelpers.ts:82-97` → `validateEntity`, lines 40-63):
  coerce `created_at`/`updated_at` back to `Date`, `null` → `undefined`, `Uint8Array`/`Buffer` →
  `number[]`. Client call site: `StorageClientBase.ts:605-607`.
- INFERRED: binary-as-number-array is a bulky encoding (one JSON number per byte); acceptable on an
  authenticated HTTP link, poor for an on-chain payload.

### 1.4 Merge/replay on the receiving side (VERIFIED)

`StorageProvider.processSyncChunk` (`src/storage/StorageProvider.ts:1165-1187`) wraps the whole
chunk in **one DB transaction**, loads the consumer's `sync_states` row for
(`args.fromStorageIdentityKey`, local `userId`), and delegates to
`EntitySyncState.processSyncChunk` (`src/storage/schema/entities/EntitySyncState.ts:363-427`):

- Builds 12 `MergeEntity` wrappers pairing each chunk array with the entity class's static
  `mergeFind` and that entity's `syncMap` slice.
- `chunk.user` is merged first via `EntityUser.mergeFind/mergeExisting`. Note: in
  `WalletStorageManager.syncFromReader` (`WalletStorageManager.ts:784-788`) the incoming
  `user.activeStorage` is overwritten with the local active value before merge — "Merging state
  from a reader cannot update activeStorage."
- Per row (`MergeEntity.merge`, `src/storage/schema/entities/MergeEntity.ts:39-67`):
  1. `mergeFind` looks up an existing local row by **natural key**, not by ID. Example,
     `EntityTransaction.mergeFind` (`EntityTransaction.ts:245-272`): prefer `(userId, txid)`,
     fall back to `(userId, reference)` — with a comment explaining txid is globally stable while
     `reference` is locally assigned by whichever storage first ingested the row.
  2. Found → `mergeExisting`; not found → `mergeNew` (insert with `userId` replaced by local,
     primary ID zeroed so the DB assigns one, FK ids remapped through `syncMap.*.idMap`; e.g.
     `EntityTransaction.mergeNew` remaps `provenTxId`, lines 274-279).
  3. `updateSyncMap(idMap, producerId, localId)` records the mapping; **remapping an existing
     producer ID to a different local ID throws `WERR_INTERNAL`** (`MergeEntity.ts:29-35`;
     `EntitySyncState.mergeIdMap` likewise throws `WERR_INVALID_PARAMETER` on conflict,
     `EntitySyncState.ts:229-247`). `certificateField`, `txLabelMap`, `outputTagMap` return
     `eiId: -1` and skip idMap (`EntityCertificateField.ts:121`, `EntityTxLabelMap.ts:105`,
     `EntityOutputTagMap.ts:105`); they match by relationship (e.g. certificateField by remapped
     `certificateId` + `fieldName`).
- **Conflict rule = per-row last-writer-wins on `updated_at`.** `mergeExisting` applies the
  incoming row only `if (ei.updated_at > this.updated_at)`, overwrites the listed mergeable
  fields, and sets `updated_at = max(both)` (`EntityTransaction.ts:281-312`,
  `EntityOutput.ts:307-334`). Fields documented as never updated: primary id, `userId`,
  `reference`.
- **Exception:** `EntityProvenTxReq.mergeExisting` (`EntityProvenTxReq.ts:622-640`) is not LWW —
  it **union-merges** `history` and `notify` regardless of timestamps, throws if the two rows have
  unequal non-empty `batch`, always writes, and returns `false` (never counted as an update).
- **Deletion handling: there is none.** No merge path deletes a row. Soft-delete flags
  (`isDeleted`) exist only on `output_baskets`, `output_tags`, `tx_labels`, both map tables, and
  `certificates`, and travel as ordinary columns under LWW. `outputs` and `transactions` have no
  tombstone; a row once synced exists on the consumer forever.
- **Progress/completion** (`EntitySyncState.processSyncChunk:404-421`): after merging, each
  entity's `count += stateArray.length` (counts become next request's offsets); `done` iff **every**
  entity array was present and empty (so the last chunk of every cycle is an all-empty chunk); on
  done, `when = maxUpdated_at` observed and all counts reset to 0; the sync state (including the
  JSON-stringified syncMap) is persisted in the same transaction
  (`updateStorage(writer, false, trx)`).
- The driving loops `syncFromReader` / `syncToWriter` (`WalletStorageManager.ts:759-841`) simply
  loop: build args from stored sync state → `reader.getSyncChunk(args)` →
  `writer.processSyncChunk(args, chunk)` → until `r.done`. `updateBackups` (line 843) runs
  `syncToWriter` against every configured backup store; `setActive` (line 863) syncs then flips
  `activeStorage`.

**Consumer-held sync state (VERIFIED):** `sync_states` row per (user, remote storage):
`syncStateId, userId, storageIdentityKey, storageName, status ('success'|'error'|'identified'|'updated'|'unknown'), init, refNum (unique), syncMap (longtext JSON), when, satoshis, errorLocal, errorOther`
(DDL `KnexMigrations.ts:640-656`). The `syncMap` JSON is 12 `EntitySyncMap` slices:
`{entityName, idMap: Record<producerId, localId>, maxUpdated_at?, count}`
(`EntityBase.ts:89-126`, `createSyncMap` lines 128-205).

---

## 2. Relationship to BRC-40 as spec'd

BRC-40 "User Wallet Data Synchronization" (`BRCs/outpoints/0040.md`, 287 lines, read in full;
author Ty Everett) says outright: "This specification addresses that problem by defining the
synchronization behavior already used in Wallet Toolbox," and its Interoperability Notes name
`getSyncChunk`, `processSyncChunk`, `syncState`, `syncMap`.

**Verdict (VERIFIED point by point): BRC-40 is a faithful after-the-fact write-up of this code.**
Matches: the 12 entity names and exact order; request/response shapes field for field; inclusive
`since` (`updated_at >= since`); include-the-overshooting-item rule for `maxRoughSize`; omitted vs
empty-array semantics; completion = all arrays present and empty; user row on `updated_at` later
than `since`; consumer-durable sync state with `since`/counts/idMap; idMap conflict must fail
sync; the three idMap-exempt entities (`certificateField`, `txLabelMap`, `outputTagMap`) — the
spec's list matches the `eiId: -1` implementations exactly.

Divergences found (all minor, but real):

1. **The code's own doc-comment contradicts the code and the spec.** The `offsets` comment in
   `WalletStorage.interfaces.ts:568-583` lists "0 ProvenTxs, 1 ProvenTxReqs, 2 OutputBaskets,
   3 TxLabels, 4 OutputTags, 5 Transactions, 6 TxLabelMaps, 7 Commissions, 8 Outputs,
   9 OutputTagMaps, 10 Certificates, 11 CertificateFields." The implemented order (§1.2) is
   different, and BRC-40 matches the implementation. The comment is stale; trust the code.
2. **Determinism requirement not implemented explicitly.** BRC-40: "The producer MUST use a
   deterministic record order that remains stable throughout a sync cycle so the consumer can
   safely resume by offset." The Knex paged queries emit no `ORDER BY` (§1.2). Works on SQLite by
   accident of rowid order; another engine could break offset resume while remaining "the
   reference implementation."
3. **Not in the spec:** `SyncProtocolVersion = '0.1.0'` exists in code only; BRC-40 defines no
   version field in any structure.
4. BRC-40 declares transport out of scope; the code's transport is the authenticated JSON-RPC
   endpoint with server-side clamping of `maxItems`/`maxRoughSize` (§1.3). Clamping is unspecified
   in BRC-40 but compatible (a producer may always return less than asked).

### 2.1 Ty Everett's claim ("toolbox remote-storage sync IS BRC-38/39")

What the code actually shows (VERIFIED):

- **Sync does not move BRC-38 row forms.** SyncChunk arrays are raw `Table*` rows: timestamps as
  `Date` (ISO strings on the JSON wire), binary as `number[]`, and `provenTxReq.history`/`notify`
  and `syncState.syncMap` remain **JSON-encoded strings** as stored. BRC-38 portable rows are a
  different serialization: base64 for the eight binary fields, strict `YYYY-MM-DDTHH:MM:SS.sssZ`
  strings, JSON-in-string fields **decoded to objects**, nulls omitted (`portable/index.ts`,
  `portableRow` lines 808-826; `binaryFieldsByKind`/`jsonFieldsByKind`/`dateFieldsByKind` lines
  137-168).
- **BRC-38 export is a separate serializer over the same schema.** `exportBRC38`
  (`portable/index.ts:170-234`) walks the same tables with the same `find*` methods and converts
  each row with `portableRow`. Same schema, different encoding, different closure rules: the
  export *filters* (provenTxReqs to those whose `txid` matches an exported transaction, provenTxs
  to those referenced, maps to exported parents — lines 175-195); sync sends all rows since the
  watermark with no closure filtering.
- **Where they genuinely meet:** BRC-38 **merge-mode import is implemented on the sync engine.**
  `mergeBRC38` (`portable/index.ts:461-532`) decodes the document back to table rows, constructs a
  synthetic `SyncChunk` with `maxItems`/`maxRoughSize = Number.MAX_SAFE_INTEGER` and all offsets 0,
  calls `storage.processSyncChunk` with it, then sends a second all-empty-arrays chunk to trigger
  the completion path. One merge machine serves both BRC-40 sync and BRC-38 import.

INFERRED: the precise version of the statement is: *remote-storage sync is BRC-40; BRC-38/39 are
the export file formats over the same schema; they share the row schema and the convergent-merge
engine, not the serialization.* Saying sync "is BRC-38/39" is loose — BRC-40's own scope section
says it "does not define backup file formats," and BRC-38's scope excludes sync. Our BRC should
cite BRC-40 for merge semantics and BRC-38 for row encoding, and not conflate them.

---

## 3. `storage/portable/index.ts` — exportBRC38/39, importBRC38/39

**Provenance (VERIFIED):** `gh api repos/bsv-blockchain/ts-stack/pulls/117` → title
"[codex] Add BRC-38/39 wallet portability", author `ty-everett`, merged **2026-05-14T19:41:23Z**.
Our prior survey's "PR #117, 2026-05-14" is the right date but the repo is **ts-stack**, not
wallet-toolbox (wallet-toolbox's own #117 is an unrelated sqlite dependency fix, merged
2026-02-05, author sirdeggen). File read in full (1016 lines).

**Document shape** (`BRC38WalletData`, lines 51-60): `{ brc: 38, title: 'User Wallet Data Format',
formatVersion: 1, exportedAt, sourceStorage (settings row), user (exactly one), tables }` with 13
arrays: the 12 sync entities' tables plus `syncStates`. Serialized by a local `canonicalize`
(lines 921-940): recursive, keys sorted by codepoint — JCS-style, matching BRC-38 §3's RFC 8785
requirement for the cases that arise here (INFERRED: full JCS number formatting is not
implemented, but all numbers in the payload are integers, where `JSON.stringify` agrees with JCS).

**Export specifics (VERIFIED):**
- Certificates: the joined convenience property `fields` is deleted from the portable row
  (lines 222-226).
- Tables sorted exactly as BRC-38 §8 (`sortBRC38Tables`, lines 842-863).
- Relationship validation (`validateRelationships`, lines 764-778 plus helpers) enforces the
  BRC-38 §9 closure: every FK-like reference must resolve inside the document; duplicate primary
  IDs rejected; every row's `userId` must equal `user.userId`.
- Timestamp format enforced by exact regex (`assertIsoDate`, lines 898-904); base64 must be padded
  with no whitespace (`assertBase64`); nulls rejected anywhere in the document (`rejectNulls`).

**Import modes** (`importBRC38`, lines 250-264):
- Chain gate first: `sourceStorage.chain` must equal target chain or throw.
- **`restore`** (lines 429-459): target must be **empty** — counts of 15 tables including
  `monitor_events` must all be zero (`assertRestoreTargetEmpty`, lines 634-654). Inserts every row
  **verbatim, preserving numeric primary IDs**, in FK dependency order, in one transaction. One
  migration shim: `upgradeLegacyManagedChangeBasketDefault` on baskets.
- **`merge`** (lines 461-532): `findOrInsertUser` by identityKey, `findOrInsertSyncStateAuth` for
  the source storage, then the synthetic-SyncChunk trick (§2.1) — so merge import inherits all
  BRC-40 merge behavior: natural-key matching, ID remapping, LWW, no deletions. Afterwards it
  merges the *imported* `syncStates` rows, rewriting each imported syncMap's local-ID side through
  the import's own idMap (`remapSyncMap`, lines 577-589), and keeps the freshly built syncMap for
  the source storage itself rather than the imported one (lines 563-570).
- **Unknown/missing fields:** `validateBRC38` checks only known typed fields (dates/binary/JSON
  shape) and referential integrity; **unknown extra fields pass validation** and are copied
  through by `fromPortableRow` (lines 828-840) into the insert payload — INFERRED: they then
  either land in a same-named DB column or fail at the Knex/IDB layer; there is no explicit
  unknown-field rejection or stripping. Missing optional fields are omitted (DB defaults apply).
  Missing required fields fail either relationship validation (`requireNumber`/`requireString`)
  or the insert's NOT NULL constraint.

**BRC-39 envelope** (`encryptBRC39`/`decryptBRC39`, lines 284-372; constants 98-106): binary file =
33-byte header (`WDAT` magic, version 1, protector 1 = password, inner format 38, KDF 1 = Argon2id,
flags 0, saltLen, nonceLen, iterations u32be, memoryKiB u32be, parallelism, hashLength 32, 12
reserved zero bytes) + salt(32) + nonce(32) + AES-256-GCM ciphertext + 16-byte tag. Argon2id
defaults: iterations 7, memory 131072 KiB (128 MiB), parallelism 1, 32-byte key; password
NFC-normalized. **Export refuses parameters weaker than the defaults** (`validateExportKdfParams`,
lines 995-1002) while import accepts any valid header params. Matches BRC-39's spec text
(`BRCs/outpoints/0039.md` §4-5: same defaults, same header table, same 0x26 inner-format byte).

---

## 4. Authoritative storage schema (what our ~18 tables drifted from)

Source: `src/storage/schema/KnexMigrations.ts` (737 lines, read in full). The initial migration
`2024-12-26-001` creates 16 tables; later migrations add columns/indexes/tables. All tables get
`created_at`/`updated_at` timestamp(3).

**Core tables (initial migration, lines 464-660) — the BRC-38/40 universe:**

| Table | Columns (beyond timestamps) |
|---|---|
| `users` | userId PK, identityKey (unique, 130) — `activeStorage` (130, nullable) added 2025-01-21, made notNullable 2025-02-22 |
| `proven_txs` | provenTxId PK, txid (unique), height, index, merklePath BLOB, rawTx BLOB, blockHash, merkleRoot — all notNullable |
| `proven_tx_reqs` | provenTxReqId PK, provenTxId FK, status (dflt 'unknown'), attempts, notified, txid (unique), batch?, history longtext dflt '{}', notify longtext dflt '{}', rawTx BLOB notNull, inputBEEF BLOB — plus `wasBroadcast`, `rebroadcastAttempts` (2026-04-30) |
| `transactions` | transactionId PK, userId FK, provenTxId FK, status, reference (unique), isOutgoing, satoshis bigint, version?, lockTime?, description, txid?, inputBEEF BLOB?, rawTx BLOB? |
| `commissions` | commissionId PK, userId FK, transactionId FK, satoshis, keyOffset, isRedeemed, lockingScript BLOB notNull |
| `outputs` | outputId PK, userId FK, **transactionId FK notNullable**, basketId FK?, spendable, change, vout, satoshis bigint, providedBy, purpose, type, outputDescription?, txid?, senderIdentityKey?, derivationPrefix?(32→200 in 2025-02-28), derivationSuffix?, customInstructions?(2500), spentBy FK?, sequenceNumber?, spendingDescription?, scriptLength?, scriptOffset?, lockingScript BLOB?, **unique(transactionId, vout, userId)** |
| `output_baskets` | basketId PK, userId FK, name, numberOfDesiredUTXOs, minimumDesiredUTXOValue, isDeleted, unique(name,userId) |
| `output_tags` | outputTagId PK, userId FK, tag, isDeleted, unique(tag,userId) |
| `output_tags_map` | outputTagId FK, outputId FK, isDeleted, unique(outputTagId,outputId) — no own PK |
| `tx_labels` | txLabelId PK, userId FK, label, isDeleted, unique(label,userId) |
| `tx_labels_map` | txLabelId FK, transactionId FK, isDeleted, unique(txLabelId,transactionId) — no own PK |
| `certificates` | certificateId PK, userId FK, serialNumber, type, certifier, subject, verifier?, revocationOutpoint, signature, isDeleted, unique(userId,type,certifier,serialNumber) |
| `certificate_fields` | userId FK, certificateId FK, fieldName, fieldValue, masterKey — no own PK, unique(fieldName,certificateId) |
| `sync_states` | see §1.4 |
| `settings` | storageIdentityKey, storageName, chain, dbtype, maxOutputScript — single row; BRC-38's `sourceStorage` |
| `monitor_events` | id PK, event, details — storage-global, excluded from BRC-38 by spec |

**Later, non-BRC tables (VERIFIED, migration keys at lines 15-19, 91-236):** `auth_sessions`
(2026-07-14), `action_batches` + `action_batch_outputs` + `action_batch_blobs` (2026-07-15;
manifest retention 2026-07-26), `payment_replays` (2026-08-04). **None of these appear in
SyncChunk, BRC-38, or BRC-40.** Note `action_batches` is `userId`-scoped user state that the
"complete, portable" BRC-38 export silently omits — the spec universe has already drifted behind
the live schema (absence from `getSyncChunk.ts` and `portable/index.ts` is VERIFIED; the drift
framing is INFERRED).

**Our drift from this baseline** (VERIFIED against
`C:/Users/archb/Hodos-Browser/rust-wallet/src/database/migrations.rs`, table listing + `outputs`
DDL at lines 216-250): snake_case columns (`user_id`, `locking_script`) vs toolbox camelCase;
INTEGER epoch timestamps vs datetime; `outputs.transaction_id` **nullable** (for externally
received outputs — toolbox requires notNullable); extra `confirmed` column; `UNIQUE(txid, vout)`
instead of `UNIQUE(transactionId, vout, userId)`; plus tables toolbox has no concept of:
`wallets`, `addresses`, `parent_transactions`, `block_headers`, `transaction_inputs`,
`transaction_outputs`, `messages`, `relay_messages`, `derived_key_cache`, `domain_permissions`,
`cert_field_permissions`, `peerpay_received`.

---

## 5. Decision question: could encrypted toolbox SyncChunks be our on-chain delta payload?

Short answer (INFERRED from the verified facts above): **adopt the shape and the merge semantics;
do not adopt the literal SyncChunk.** Four of six load-bearing properties align; the two that
don't are fatal to literal reuse.

**What aligns:**

1. **Row-level upserts keyed by natural identity.** SyncChunk rows are full-row upserts merged by
   natural key with convergent ID remapping — exactly the `upsert` half of the row-level changeset
   in DELTA_ANALYSIS.md §3. BRC-40's merge rules (idempotent, inclusive-since, per-row LWW) are
   precisely what replaying a delta chain after a snapshot needs, and they are already written
   down in a public spec we can cite instead of inventing vocabulary.
2. **Watermark semantics.** `since` + "all rows with `updated_at >= since`" is a clean definition
   of a delta's row set; producing a delta is the same query `getSyncChunk` runs. Our
   `seq`/`parent_txid` chain carries the interval implicitly.
3. **Entity coverage.** The 12 sync entities map 1:1 onto the toolbox-derived core of our ~18
   tables. For those tables, using BRC-38 **portable row forms** (base64 binary, strict ISO dates,
   omitted nulls) as the delta row encoding buys real spec alignment — and is a far better
   on-chain encoding than SyncChunk's number[]-per-byte JSON.
4. **Omitted-vs-empty and completion semantics** give a ready-made answer for "how does recovery
   know a delta segment is complete" if one delta ever spans multiple tokens.

**What does not align:**

1. **No deletions — fatal as-is.** SyncChunk/BRC-40 has no delete record and the merge engine
   never removes a row (§1.4). Our cost story rests on strips: spent-output strip, dead-address
   strip, 60-day transaction window (ONCHAIN_BACKUP_SYSTEM.md; DELTA_ANALYSIS.md §1). Replaying
   literal SyncChunks reconstructs a monotonically growing database — recovery would resurrect
   everything the strips exist to shed, with no way to express "this row left the working set."
   Our delta format needs the `delete`-by-primary-key array (DELTA_ANALYSIS.md §3); that is an
   **extension beyond** BRC-40, not a profile of it.
2. **Our stripped rows are not valid toolbox rows.** We strip `proven_txs.merkle_path`,
   `proven_tx_reqs.raw_tx`/`input_beef` (and, per the pending recommendation, inscription
   `locking_script` bytes). In toolbox those are notNullable columns
   (`KnexMigrations.ts:474-475,490`), BRC-38 §5.2 lists them among the required binary fields, and
   `checkEntityValues` refuses nulls in chunks. A "toolbox SyncChunk" missing them is not one —
   any interop claim would be false advertising. What we'd actually ship is "SyncChunk-shaped rows
   minus re-fetchable fields," which is a new format that cites, not reuses.
3. **Schema drift both ways.** Our extra columns (`outputs.confirmed`, nullable
   `transaction_id`) and extra tables (`wallets`, `addresses`, `domain_permissions`,
   `cert_field_permissions`, and the rest of §4) have no SyncChunk entity; toolbox's newer
   `action_batches` isn't in the format either. Either way a field-mapping layer sits between our
   DB and the payload — at which point the payload is our own row form anyway.
4. **Anchoring model differs.** SyncChunk ordering/resume lives in consumer-held mutable state
   (sync_states: since + offsets + idMap) negotiated per request/response pair. On-chain there is
   no request side: ordering must be carried by the tokens (`seq`, `parent_txid` —
   DELTA_ANALYSIS.md §3), and `updated_at`-clock LWW across devices is weaker than our
   read-before-write + parent-pointer fork detection. Irrelevant on-chain:
   `fromStorageIdentityKey`/`toStorageIdentityKey` (no storage pair), idMap (single logical DB,
   IDs are ours), maxItems/maxRoughSize (we bound by token size classes).

**Recommendation for the BRC draft (INFERRED):** define the delta payload as *BRC-38 portable row
forms for the tables both schemas share*, extended with (a) additional table sections for
Hodos-specific tables, (b) a `deletes` array per table, (c) omission of re-fetchable binary fields
with re-hydration rules — and cite BRC-40 for merge/replay semantics (idempotent upsert,
natural-key matching, inclusive watermark). That keeps every real point of compatibility, is
honest about the extensions, and avoids shipping SyncChunk's wire weaknesses (number[] binary, no
deletes, consumer-held resume state) on-chain.

---

## Checked (files/sections actually read)

Toolbox (`ts-stack@8b074a0648`, `packages/wallet/wallet-toolbox/`):
- `src/sdk/WalletStorage.interfaces.ts` lines 150-170, 520-650 (SyncChunk, RequestSyncChunkArgs, ProcessSyncChunkResult, SyncStatus, SyncProtocolVersion)
- `src/storage/methods/getSyncChunk.ts` — all 297 lines
- `src/storage/schema/entities/EntityBase.ts` — all 210 lines (EntitySyncMap, SyncMap, createSyncMap)
- `src/storage/schema/entities/EntitySyncState.ts` lines 125-427 read; 1-124 grepped (makeRequestSyncChunkArgs, processSyncChunk, mergeIdMap, syncChunkSummary)
- `src/storage/schema/entities/MergeEntity.ts` — all 68 lines
- `src/storage/schema/entities/EntityTransaction.ts` lines 240-340 (mergeFind/mergeNew/mergeExisting)
- `src/storage/schema/entities/EntityOutput.ts` lines 307-335; `EntityProvenTxReq.ts` lines 622-641; `EntityCertificateField.ts` lines 104-121; `EntityTxLabelMap.ts`/`EntityOutputTagMap.ts` eiId returns (lines 93-105)
- `src/storage/WalletStorageManager.ts` lines 750-950 (syncFromReader, syncToWriter, updateBackups, setActive)
- `src/storage/StorageProvider.ts` lines 1165-1199 (processSyncChunk transaction wrapper)
- `src/storage/StorageKnex.ts` lines 240-260 + grep for `since`/`orderBy` usage
- `src/storage/remoting/StorageServer.ts` lines 630-650 + grep (RPC whitelist lines 79/89, POST endpoint line 517, clamping); `StorageClientBase.ts` lines 595-640; `entityValidationHelpers.ts` lines 1-97
- `src/storage/portable/index.ts` — all 1016 lines
- `src/storage/schema/KnexMigrations.ts` — all 737 lines
- `src/storage/schema/tables/TableUser.ts`, `TableOutput.ts` — full

Specs (`bsv-blockchain/BRCs@c1d12f2`): `outpoints/0040.md` all 287 lines; `outpoints/0038.md` all
614 lines; `outpoints/0039.md` via structural grep (§3-5 header/KDF constants checked against
code); `README.md:104-106`.

Ours: `ONCHAIN_BACKUP_SYSTEM.md` lines 1-75, 140-182 + strip greps; `DELTA_ANALYSIS.md` all 209
lines; `rust-wallet/src/database/migrations.rs` table listing + outputs DDL lines 216-250.

GitHub API: `repos/bsv-blockchain/ts-stack/pulls/117`, `repos/bsv-blockchain/wallet-toolbox/pulls/117`.

## NOT checked

- `StorageIdb.ts` beyond line 108 (IndexedDB find/paging details; assumed symmetric with Knex).
- `EntityProvenTx`, `EntityCertificate`, `EntityOutputBasket`, `EntityTxLabel`, `EntityOutputTag`,
  `EntityCommission`, `EntityUser` mergeExisting bodies (assumed LWW like Transaction/Output —
  INFERRED, not verified).
- Whether the StorageServer RPC route validates the inbound `chunk` argument to `processSyncChunk`
  the way the client validates outbound chunks (checked outbound path only).
- Actual runtime behavior of an import containing unknown fields (traced code path only).
- go-wallet-toolbox parity (task C2's territory), and the venue/wording of Ty Everett's public
  statement (took the task's paraphrase as given).
- BRC-39 spec lines beyond the structural grep; the `argon2id` implementation in
  `src/utility/hashWasm`.
- `sync/StorageMySQLDojoReader.ts` (legacy Dojo import path, not relevant).
- Whether RFC 8785 number-formatting edge cases can arise in `canonicalize` (asserted integers-only
  by inspection of the row forms, not by test).

## Flags

1. **Stale doc-comment in toolbox code**: `RequestSyncChunkArgs.offsets` comment lists a different
   entity order than the implementation and BRC-40. Code and spec agree; the comment is wrong.
   Trust the code order.
2. **BRC-40 determinism gap**: the spec requires stable deterministic producer ordering; the Knex
   sync queries emit no ORDER BY. Works by engine accident. If our draft cites BRC-40's offset
   resume, don't inherit this — our chain ordering (`seq`/`parent_txid`) doesn't need offsets.
3. **"Toolbox sync IS BRC-38/39" is imprecise**: sync is BRC-40 moving raw rows; BRC-38/39 are
   file formats; the shared merge engine is the real overlap. Phrase carefully in the draft and in
   any reply to Ty.
4. **BRC-38's "complete" export already trails the live schema**: `action_batches` (user-scoped,
   2026-07-15) is exported nowhere. Their spec has the same drift problem our draft addresses —
   useful precedent, to be mentioned diplomatically.
5. **No deletions in BRC-40/SyncChunk** is the single biggest mismatch with our strip-based
   design — any claim that our delta payload "is" toolbox sync chunks would be false. Scope
   compatibility claims to row forms + merge semantics for the shared tables.
6. **Repo relocation**: wallet-toolbox is archived; cite
   `bsv-blockchain/ts-stack/packages/wallet/wallet-toolbox` in the draft's references. PR #117
   (BRC-38/39 portability, ty-everett, merged 2026-05-14) is on **ts-stack**, not the archived
   repo — our prior survey's repo attribution needs that one-word correction.
7. **Memory correction**: the memory note "38/39/40 are written, not reserved" is confirmed — all
   three are full merged documents in `bsv-blockchain/BRCs` as of commit `c1d12f2` (2026-08-21).
   Also note the BRCs repo now lives under the `bsv-blockchain` org (clone remote verified), not
   `bitcoin-sv`.
