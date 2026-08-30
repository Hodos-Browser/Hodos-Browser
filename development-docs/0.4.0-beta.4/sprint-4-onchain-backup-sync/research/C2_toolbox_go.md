# C2 — go-wallet-toolbox: sync machinery, BRC-38/39 presence, schema, divergence from TS

Date: 2026-08-22
Sources: shallow clones at scratchpad `repos/go-wallet-toolbox` (HEAD `9d188d5`, 2026-08-21, PR #1006) and `repos/wallet-toolbox-ts` (frozen at monorepo-migration commit `320a82b`, 2026-06-12), plus raw files fetched from `bsv-blockchain/ts-stack` `main` and `bitcoin-sv/BRCs` `master` on 2026-08-22.

## Bottom line (the "one format or two" question)

**Intended: ONE format. Actual today: one format with verified divergences that would break a real Go-to-TS sync.**

The Go toolbox deliberately mirrors the TS sync wire format field-for-field (its own comments say "based on the TS version" and reference TS consumer behavior), and both implementations are now nominally pinned to the same spec — **BRC-40 "User Wallet Data Synchronization"**, which exists in the `bitcoin-sv/BRCs` registry (`outpoints/0040.md`, author Ty Everett) and has cross-implementation conformance vectors vendored from `bsv-blockchain/ts-stack`. But I found three concrete wire-level divergences (a JSON field-name typo on `transaction.provenTxId`, a request `offsets` ordering mismatch that a TS producer hard-rejects, and an extra Go-only `aborted` transaction status), plus a remoting gap (no push endpoint in the V1 HTTP contract). Details and citations below.

**Implication for our delta chunks:** if we align with "toolbox chunks," align with **BRC-40 as specified** (entity names, order, omission semantics), not with either implementation's quirks. Both implementations treat BRC-40 as the contract and both are actively being fixed toward it; the Go quirks below are bugs against that spec, not a second format.

---

## 1. Does the Go toolbox implement sync? Wire shapes vs TS

**VERIFIED: yes, a complete SyncChunk protocol equivalent to TS, with matching JSON names for the envelope.**

Wire types live in `pkg/wdk/storage_request_sync_chunk_args.go`:

- `RequestSyncChunkArgs` (lines 6–30): `fromStorageIdentityKey`, `toStorageIdentityKey`, `identityKey`, `since,omitempty` (`*time.Time`), `maxRoughSize`, `maxItems`, `offsets` (`[]SyncOffsets`, each `{"name","offset"}`, lines 35–38).
- `SyncChunk` (lines 43–64): `fromStorageIdentityKey`, `toStorageIdentityKey`, `userIdentityKey`, `user,omitempty`, then twelve entity arrays: `outputBaskets`, `provenTxs`, `provenTxReqs`, `txLabels`, `outputTags`, `transactions`, `outputs`, `txLabelMaps`, `outputTagMaps`, `certificates`, `certificateFields`, `commissions`.
- `ProcessSyncChunkResult` (lines 96–101): `done`, `maxUpdated_at,omitempty`, `updates`, `inserts`. (TS adds an optional `error`; Go omits it.)
- `SyncMap`/`SyncMapEntity` in `pkg/wdk/sync_map.go` (lines 10, 54–68): `entityName`, `idMap`, `maxUpdated_at,omitempty`, `count` — matches TS `EntitySyncMap` usage.

These names match the TS interfaces in `src/sdk/WalletStorage.interfaces.ts` lines 490–575 of the frozen repo (same file exists in ts-stack at `packages/wallet/wallet-toolbox/src/sdk/WalletStorage.interfaces.ts`).

Go's intent to be TS-wire-compatible is explicit in source: `storage_request_sync_chunk_args.go:50–51` — "ATTENTION: The TS version keeps loading chunks (infinite-loop) if at least one of the entities is 'undefined'. That's why `omitempty` is not used below and the slices are pre-initialized in NewSyncChunk."

**Machinery (all read):**

- Producer: `pkg/storage/provider.go:961` `GetSyncChunk` → `pkg/storage/internal/sync/sync_chunk_action.go` drives per-entity chunkers (`chunkers.go`) with size/count limits (`chunking_state.go`); size is estimated by JSON-marshaling the running chunk (`sync_chunk_action.go:100–107`).
- Consumer: `pkg/storage/provider.go:1010` `ProcessSyncChunk` → `pkg/storage/internal/sync/chunk_processor.go` merges entities and maintains the idMap; upserts live in `pkg/internal/storage/repo/syncrepo/` (one file per entity).
- Orchestration: `pkg/storage/internal/sync/sync_to_writer.go` `ReaderToWriter.Sync` implements the full loop: find/insert sync state on the writer, parse stored `syncMap`, build offsets, `reader.GetSyncChunk`, `writer.ProcessSyncChunk`, repeat until `done` — the same cycle as TS `WalletStorageManager.updateBackups`.
- Manager: `pkg/storage/storage_manager.go` — `SyncToWriter` (line 157), `SetActive` (line 197) which backs up state and resolves conflicting actives, mirroring the TS manager.

### VERIFIED wire divergences (Go vs current ts-stack `main`)

1. **`transaction.provenTxId` typo in Go.** `pkg/wdk/table_transaction.go:15`:
   `ProvenTxID *int \`json:"proveTxId"\`` — missing the "n". TS (`ts-stack .../tables/TableTransaction.ts:9`) serializes `provenTxId`. The sibling Go types are correct (`table_proven_tx.go:13` and `table_proven_tx_req.go:14` both say `provenTxId`). Go↔Go round-trips fine (same typo both sides — `pkg/storage/sync_test.go` even asserts provenTxId round-trip); Go↔TS silently drops the transaction→provenTx link in both directions, because Go's decoder won't match `provenTxId` to `proveTxId` and TS won't read `proveTxId`. This is exactly the field the vendored regression vectors say MUST NOT be lost (`brc40_conformance_test.go:151–156`).

2. **Request `offsets` order.** BRC-40 (`outpoints/0040.md`, "Request Rules") says offsets "MUST be supplied in the exact entity order defined in this specification": `provenTx, outputBasket, outputTag, txLabel, transaction, output, txLabelMap, outputTagMap, certificate, certificateField, commission, provenTxReq`, and "If an offsets entry is missing, duplicated, or out of order, the producer MUST reject." The TS producer enforces this positionally (ts-stack `getSyncChunk.ts:235–237`: throws `WERR_INVALID_PARAMETER('offsets', 'in dependency order...')`; chunker order at lines 41–216 matches the spec). Go's consumer builds offsets from `wdk.AllEntityNames` (`sync_to_writer.go:119–133`), whose order is different (`pkg/wdk/entity_name.go:24–36`): `provenTx, outputBasket, transaction, provenTxReq, txLabel, txLabelMap, output, outputTag, outputTagMap, certificate, certificateField, commission`. Third element is `transaction` where TS expects `outputTag` → **a TS producer rejects a Go consumer's very first chunk request** (INFERRED from reading both sides; not executed). Go's own producer is tolerant — it looks offsets up by name (`sync_chunk_action.go:93–99`) and its validator (`pkg/internal/validate/validate_request_sync_chunk_args.go`) never inspects offsets — so TS-consumer→Go-producer works.

3. **Go-only `aborted` transaction status.** `pkg/wdk/tx_status.go:25` adds `TxStatusAborted = "aborted"`; ts-stack `sdk/types.ts:83–84` has only `completed|failed|unprocessed|sending|unproven|unsigned|nosend|nonfinal|unfail`. The Go project tracks this itself as an outbound-sync hazard (`docs/superpowers/plans/2026-07-20-aborted-tx-status.md:614`: "a Go wallet emitting `aborted` to an older TS peer relies on TS tolerating an unknown status").

4. **Entity-array omission semantics.** BRC-40 says unattempted entity arrays MUST be omitted and completion = all twelve present and empty. Go never omits: `NewSyncChunk` pre-initializes all twelve arrays and no `omitempty` (`storage_request_sync_chunk_args.go:50–85`), citing the TS consumer's infinite-loop on `undefined`. By my reading this cannot cause premature completion (a chunk truncated by limits always carries at least one non-empty array, so the consumer keeps cycling — INFERRED, not tested), but it is a MUST-level deviation from the spec text and it erases the spec's "attempted vs not attempted" distinction.

5. **Producer entity execution order differs from spec/TS.** Go chunker order (`pkg/storage/internal/sync/chunkers.go`): baskets, knownTxs (provenTx+provenTxReq in one pass), transactions, outputs, labels, labelMaps, tags, tagMaps, certificates, certificateFields, commissions. TS/spec order is the list in item 2. Both orders are internally dependency-safe, and merging is by entity name, so I found no correctness impact (INFERRED) — but Go also pages `provenTx` and `provenTxReq` as a single `known_tx` query whose page offset is `offsets[provenTxReq] + offsets[provenTx]` (`chunker_known_tx.go:50–55`), a paging scheme that only makes sense against Go's merged table (see §3).

6. **Request validation depth.** Go's `ValidRequestSyncChunkArgs` checks only presence of the three keys and non-zero `maxItems`/`maxRoughSize`; the BRC-40 error vectors for malformed/missing/out-of-order offsets are not enforced (and the Go conformance runner does not run the request/response channels — see §5).

### Remoting of sync

- Legacy JSON-RPC server (`pkg/storage/rpcserver/rpc_storage_provider.gen.go:167–196`) exposes GetSyncChunk, FindOrInsertSyncStateAuth, **and** ProcessSyncChunk — same as TS's old `StorageClient` JSON-RPC (`src/storage/remoting/StorageClient.ts:464–465` in the frozen repo).
- The new V1 HTTP contract (`/storage/v1/*`, described as "replaces the legacy JSON-RPC implementation (now deprecated)" in `pkg/storage/client.go:24–27`) exposes only `POST /storage/v1/sync/active`, `/sync/chunk` (GetSyncChunk), `/sync/state` (`pkg/storage/v1adapter/handler.go:99–101, 656–720`). **There is no processSyncChunk endpoint in the V1 contract** — confirmed against the vendored ts-stack adapter-conformance vectors (endpoint list includes `/storage/v1/sync/chunk|state|active` only), and the Go V1 client stubs it: `client.go:233–235` returns "ProcessSyncChunk not fully implemented in V1 client". So under V1, a Go wallet can pull from a remote reader into local storage, but cannot push chunks into a remote writer; push requires the deprecated JSON-RPC path. The `/sync/chunk` handler binds `identityKey` to the authenticated peer (`handler.go:684–689`), so a storage server can only export the caller's own data.

## 2. BRC-38/39 export/import machinery

**VERIFIED: absent.** `grep -rni "brc-38|brc38|brc-39|brc39"` over all `.go`, `.md`, `.json` in the repo returns nothing. There is no file-format export/import (no single-blob wallet export, no encryption-extension code). The only replication machinery is the BRC-40 chunk sync above.

Registry side-note (verified 2026-08-22 against `bitcoin-sv/BRCs` `master` `SUMMARY.md` lines 195–197): **BRC-38 "User Wallet Data Format", BRC-39 "User Wallet Data Format Encryption Extension", and BRC-40 "User Wallet Data Synchronization" are all present in the registry** (`outpoints/0038.md`, `0039.md`, `0040.md`, author Ty Everett). BRC-38 itself says the toolbox "does not yet define a canonical single-file export format. This specification fills that gap" — i.e., the spec exists, implementations don't, in Go (and per the C1 task, check TS independently).

## 3. Go storage schema vs TS schema

**The Go database schema is NOT a copy of the TS schema.** The TS-shaped `wdk.Table*` structs are a wire/API layer; `pkg/internal/storage/repo/` translates between them and a differently-normalized GORM schema in `pkg/internal/storage/database/models/`. Divergences (all VERIFIED by reading the model files):

| Area | TS (`src/storage/schema/tables/`) | Go (`.../database/models/`) |
|---|---|---|
| Proven txs | Two tables, `proven_txs` (numeric `provenTxId` PK) + `proven_tx_reqs` | **One table `known_tx`, PK = txid string** (`known_tx.go:9–36`); no numeric ID stored |
| Numeric wire IDs | Native | Synthesized via `numeric_id_lookup` (`numeric_id_lookup.go`: `(table_name, string_id) → num_id`), joined in at sync time (`syncrepo/sync_knowntx.go`) |
| Labels/tags | `tx_labels`/`output_tags` with numeric IDs + map tables | `Label`/`Tag` keyed by `(name, userID)` composite PK; map table keyed by `(TransactionID, LabelName, LabelUserID)` (`labels.go`, `tx_labels.go`) |
| Soft deletes | none (`isDeleted` flags on map rows) | `gorm.DeletedAt` on labels/maps/notes |
| Go-only tables | — | `user_utxos` (UTXO-selection cache with heavy index engineering, `user_utxo.go`), `key_value`, `tx_notes`, `chaintracks_*` |
| TS-only table | `monitor_events` | absent |
| Outputs | has `sequenceNumber`, `spendingDescription`, `scriptLength`, `scriptOffset` | absent from both `models/output.go` wire struct `wdk.TableOutput` — those four optional TS fields don't exist in Go |
| KnownTx.Notify | `proven_tx_reqs.notify` JSON | carried as an opaque blob "stored and returned unchanged for sync round-trip compatibility" (`known_tx.go:19–23`) |

Consequence: "toolbox chunk" content is defined by the **wire** Table* shapes, not by either DB schema — Go proves the wire format can be fed from a different schema. The idMap machinery (BRC-40's remote-to-local numeric ID mapping) is what absorbs the schema differences.

## 4. Import/export/backup machinery beyond sync

**VERIFIED: none.** No dump/restore/export commands in `cmd/` or `tools/` (grep for export/dump/restore/backup). "Backup" in this codebase means exactly one thing: a secondary storage provider kept current via `WalletStorageManager.SyncToWriter` / `SetActive` (`storage_manager.go:21–35, 157, 197–275`). No file artifact, no encryption, no on-chain anything.

## 5. Maturity

**Complete-looking and actively hardened — this is not scaffolding.**

- Recent commits touching the sync path: an 11-part fix series in July 2026 (`[03/11] fix: provenTx/provenTxReq idMap in processSyncChunk` #948, `[04/11] KnownTx notify opaque payload` #946, `[10/11] sync certificates/certificateFields/commissions` #953), and HEAD itself is "Conformance/vector drift check (#1006)" (2026-08-20). Repo overall commits near-daily through Aug 2026.
- Cross-impl conformance: `conformance/vectors/sync/brc40-user-state.json` (24 vectors) vendored from ts-stack, pinned in `conformance/SOURCE` (upstream SHA `8b074a0`, fetched 2026-08-19), run by `pkg/internal/storage/repo/syncrepo/brc40_conformance_test.go`. Coverage caveat: the Go runner exercises only the `brc40/mergeExisting` (transactions/outputs/provenTxs) and `brc40/flow` channels (lines 116, 349); the `brc40/requestSyncChunk` / `brc40/syncChunk` request-validation vectors are not wired up — consistent with the thin validator (§1.6). Second caveat: the vendored vectors' `offsets[].name` values are PascalCase-plural (`ProvenTxs`...), which matches neither BRC-40's camelCase-singular names nor either implementation's runtime — the vector corpus itself has a naming inconsistency.
- Stale-chunk guard implemented and tested: `syncrepo/sync_output.go:166–196` uses strict `updated_at <` compare with a TOCTOU-safe `WHERE` (equal timestamps skip); `brc40_guard_test.go` (505 lines, 10 tests) plus the conformance regression vectors (`spendable` must not flip false→true, `spent_by`/`proven_tx_id` must not be cleared by stale chunks). These are money-correctness properties directly relevant to our design.
- Test volume around sync: `pkg/storage/sync_test.go` (578 lines, 11 tests incl. the #852 idMap round-trip test), `storage_manager_test.go` (319 lines), `sync_cert_commission_test.go`, `sync_knowntx_notify_test.go`, `fk_parents_test.go`, plus fixtures/assertion helpers under `pkg/storage/internal/testabilities/`.
- Known holes, in the code's own words: `chunk_processor.go:279` "UserIDs can mismatch because the translation is not implemented (in TS also)"; `:417` tags backup support TODO; `:438` `ReservedByID` cannot be deduced from the output; `storage_manager.go:158` "TODO: add locking mechanism to ensure that the active storage is not being modified while syncing"; `client.go:235` V1 remote push unimplemented (§1); batch broadcast and provenTx `index` TODOs in `sync_knowntx.go:271,292`.

## What this means for the BRC draft (item 0b / delta prior art)

1. Cite **BRC-40** as the sync prior art, not "the toolbox sync format" — it is now a registry spec with two implementations and a shared vector corpus. Our delta-chunk design should state its relationship to BRC-40's model (updated_at watermark + per-entity offsets + idMap) explicitly.
2. If we target chunk-level compatibility, target the spec: camelCase-singular entity names, the spec's exact offsets order, omit-unattempted-arrays semantics. Do not copy Go's always-present arrays or Go's `AllEntityNames` order, and do not rely on `transaction.provenTxId` surviving a Go endpoint until the `proveTxId` typo is fixed upstream.
3. The Go repo demonstrates that BRC-40 chunks are schema-independent (known_tx merge + numeric_id_lookup). That strengthens the case that our on-chain delta payload can carry toolbox-chunk-shaped records without committing to any DB schema.
4. Worth an upstream report: the `proveTxId` tag typo (`pkg/wdk/table_transaction.go:15`) and the consumer offsets-order mismatch (`pkg/wdk/entity_name.go:24–36` vs BRC-40 order) — both are silent Go↔TS interop breakers.

---

## Checked (files actually read, with line refs)

Go toolbox (HEAD `9d188d5`):
- `pkg/wdk/storage_request_sync_chunk_args.go` (whole file, 1–101)
- `pkg/wdk/sync_map.go` (whole file, 1–77)
- `pkg/wdk/entity_name.go` (whole file, 1–36)
- `pkg/wdk/table_transaction.go`, `table_proven_tx.go`, `table_output.go`, `table_sync_state.go` (whole files); `table_proven_tx_req.go:14` (grep)
- `pkg/wdk/tx_status.go:7–36`
- `pkg/storage/internal/sync/`: `chunker.go`, `chunkers.go`, `chunker_known_tx.go` (1–80), `sync_chunk_action.go` (whole), `sync_to_writer.go` (1–133), `chunk_processor.go` (1–60 + TODO lines 279/417/438)
- `pkg/storage/provider.go:961–1050`
- `pkg/storage/client.go:1–100, 233–235`; `client_gen.go:80–94`
- `pkg/storage/v1adapter/handler.go:39–41, 99–101, 650–720`
- `pkg/storage/rpcserver/rpc_storage_provider.gen.go:167–196`
- `pkg/storage/storage_manager.go:21–275 (selected), method list`
- `pkg/internal/validate/validate_request_sync_chunk_args.go` (whole)
- `pkg/internal/storage/repo/syncrepo/`: `brc40_conformance_test.go` (1–320), `sync_knowntx.go` (1–60 + 271/292), `sync_output.go` (guard lines 82–196 via grep), directory listing
- `pkg/internal/storage/database/models/`: `known_tx.go`, `user_utxo.go` (1–40), `key_value.go`, `numeric_id_lookup.go`, `labels.go`, `tx_labels.go`, `certificate.go` (1–30), `tx_note.go` (all read)
- `conformance/README.md`, `conformance/SOURCE`, `conformance/vectors/sync/brc40-user-state.json` (parsed: ids, channels, offsets names, response keys), `conformance/vectors/wallet/storage/adapter-conformance.json` (parsed endpoint list)
- `plans/brc40-stale-chunk-guard.md`, `docs/superpowers/plans/2026-07-20-aborted-tx-status.md:614`
- `pkg/storage/sync_test.go:15–40`; test file line counts via `wc -l`
- Commit history via GitHub API (repo-wide last 8; `pkg/storage/internal/sync` last 10)

TS (frozen `wallet-toolbox` `320a82b`):
- `src/sdk/WalletStorage.interfaces.ts:470–600`
- `src/storage/methods/getSyncChunk.ts:1–260`
- `src/storage/schema/tables/` listing; `TableTransaction.ts`, `TableProvenTx.ts`, `TableOutput.ts` (whole)
- `src/storage/schema/entities/EntityBase.ts:131–173` (entity names)
- `src/storage/remoting/StorageClient.ts:464–465`

ts-stack `main` (raw fetch, 2026-08-22):
- `packages/wallet/wallet-toolbox/src/storage/methods/getSyncChunk.ts` (chunker names 41–216, order check 235–237)
- `.../tables/TableTransaction.ts:9,43` (`provenTxId`)
- `.../sdk/types.ts:83–84` (`TransactionStatus` union)

BRC registry (raw fetch, 2026-08-22):
- `SUMMARY.md:195–197`; `outpoints/0040.md` (abstract through "Inclusive since Semantics"); `outpoints/0038.md` (abstract/motivation)

## NOT checked

- Did not build or run any Go or TS code; all "would reject / would drop" interop claims are from reading both sides, labelled INFERRED where they predict runtime behavior.
- ts-stack's conformance runner (`conformance/runner/ts/dispatchers/sync.ts`) — referenced by Go comments, not read; so I don't know how TS interprets the PascalCase vector offset names.
- ts-stack's current StorageClient/remoting code (whether TS still has JSON-RPC processSyncChunk or moved fully to V1 HTTP) — only the frozen June 2026 repo and the endpoint vectors were checked.
- Go `chunk_processor.go` full merge logic beyond the first 60 lines + TODOs; per-entity syncrepo upserts other than knowntx/output.
- BRC-39 (`outpoints/0039.md`) content; BRC-40 sections after "Inclusive since Semantics" (ID-mapping and merge sections skimmed via impl, not read in the spec).
- Whether TS actually infinite-loops on omitted entity arrays (Go's comment claims it; contradicts BRC-40's omission semantics; unverified).
- CI/codecov actual coverage numbers for the sync packages.

---

# Addendum — independent second pass (2026-08-22, evening)

A second, independent read of the same Go checkout (`9d188d5`) was done without first consulting the report above. It **re-verified the load-bearing claims** and adds the items below. Where the two passes touched the same code, they agree.

## Claims re-verified from the code (this pass)

- `pkg/wdk/table_transaction.go:15` — `ProvenTxID *int` tagged `json:"proveTxId"` (typo, missing "n") CONFIRMED by direct read. Sibling tags `provenTxId` in `table_proven_tx.go:13` and `table_proven_tx_req.go:14` are correct.
- `pkg/wdk/tx_status.go:25` — `TxStatusAborted = "aborted"` CONFIRMED (plus `TxUpdateStatusAborted` at line 264).
- `syncrepo/sync_output.go:165–199` — stale-chunk guard CONFIRMED: pre-flight `First` on `(user_id, transaction_id, vout)`, skip when `!model.UpdatedAt.After(existing.UpdatedAt)` (equal loses), then `WHERE id = ? AND updated_at < ?` on the UPDATE as a TOCTOU guard; `Select("*")` so zero values (e.g. cleared `BasketName` on relinquish) overwrite.
- The ATTENTION comment (TS infinite-loop on omitted entity arrays), `NewSyncChunk` pre-initialization, `AllEntityNames` order, thin request validator, `chunker_known_tx.go` merged paging (`offsets[provenTxReq] + offsets[provenTx]`), V1 client stubs (`SetActive` line 121–123, `ProcessSyncChunk` 233–237), and the absence of any BRC-38/39 machinery — all re-confirmed by direct reads at the same line numbers cited above.

## New evidence not in the main report

1. **Adapter-conformance vector 12 has upstream authoring errors, acknowledged in Go's test.** `conformance/vectors/wallet/storage/adapter-conformance.json` vector `wallet.storage.adapterconformance.12` (lines 366–400) expects a `/storage/v1/sync/chunk` response containing `"users"` (plural) and `"syncStates"` keys. `pkg/storage/adapter_conformance_test.go:161–166` skips body assertion for exactly that vector with the comment: "Vector 12 body has upstream authoring errors: it declares 'users' (plural) and 'syncStates' which don't exist in the SyncChunk type in either Go or TS. The TS getSyncChunk implementation never populates those fields. Body assertion is skipped until ts-stack vector 12 is corrected upstream." Same lesson as the PascalCase offsets names: **the vector corpus is not yet a clean normative source; the registry spec text (outpoints/0040.md) plus the TS implementation are the ground truth pair.** (VERIFIED)

2. **The vector-12 request body also uses a different request shape** — `{identityKey, fromStorageIdentityKey, since, paged:{limit,offset}}` rather than `RequestSyncChunkArgs` with `offsets`/`maxItems`/`maxRoughSize`. Go's handler ignores the unknown fields and decodes into `RequestSyncChunkArgs` (`v1adapter/handler.go:678–695`). More corpus drift. (VERIFIED)

3. **Sync-surface security fix is recent and relevant to our BRC.** Commit `75c0aa4` "fix(storage): bind sync identityKey to the authenticated peer (#999)" (Aug 2026). The rationale comment (`rpcserver/rpc_storage_provider.go:79–106`, mirrored in `v1adapter/handler.go:684–689`): `RequestSyncChunkArgs` carries no AuthID; its `identityKey` alone selects whose data is exported (GetSyncChunk) or written (ProcessSyncChunk), so any authenticated peer could previously name someone else's identity key. Now a blank identityKey is filled from the authenticated BRC-103 peer and a differing one is rejected. **Takeaway for our draft: BRC-40 chunk requests are not self-authenticating; authorization is delegated entirely to the transport session.** On-chain we have no session, which is one more reason the delta envelope must carry its own authentication. (VERIFIED code + commit)

4. **Sync defaults:** `MaxSyncChunkSize = 10_000_000` bytes, `MaxSyncItems = 1000` (`pkg/wdk/sync_to_writer_options.go:9–13`) — matching the BRC-40 vector's example request values. Useful sizing precedent for our chunk targets. (VERIFIED)

5. **`SetActive` conflict-merge algorithm** (`storage_manager.go:197–275`): when stores disagree about which storage is active, the manager merges every conflicting active into the new active via `SyncToWriter`, then pushes the merged state to all other stores, then flips the `activeStorage` pointer. `MakeAvailable` (lines 95–140) partitions stores into active/backups/conflictingActives by comparing each store's recorded `user.activeStorage`. This is the toolbox's answer to the split-brain problem our multi-device design also faces — resolution by sync-merge, not by fencing. Note the same file's TODOs: no locking around either operation. (VERIFIED)

6. **Producer-side sync test inventory** beyond `sync_test.go`: `pkg/storage/provider_get_sync_chunk_test.go` (451 lines, 7 tests: offsets handling, no-offsets, offsets past maxItems, since == now, since in past, maxItems truncation, one-by-one paging). Together with the 11 `sync_test.go` tests, 19 test funcs in `syncrepo/*_test.go`, and the conformance/guard suites, the Go sync path is the most test-covered subsystem in the repo. (VERIFIED)

7. **Schema detail:** `OutputBasket` PK is composite `(Name varchar(300), UserID)` (`models/output_baskets.go:15–16`) — baskets, like labels/tags, are natural-keyed in Go; numeric `basketId` on the wire comes from `numeric_id_lookup`. `models/user_utxo.go` documents its three composite/partial indexes (incl. a `WHERE reserved_by_id IS NULL` partial index) with a measured 130 ms → <0.1 ms claim-query fix — reinforcing that `user_utxos` is a perf-only cache outside the sync wire. (VERIFIED)

## Corrections to the main report

None found. Every claim spot-checked in this pass held at the cited lines.

## Additional NOT checked (this pass)

- Did not re-fetch ts-stack, the frozen TS repo, or the BRC registry; TS-side and registry claims in the main report were not re-verified here (their local clones exist in the scratchpad `repos/` directory: `ts-stack`, `wallet-toolbox-ts`, `wallet-toolbox`, `BRCs`).
- Did not run any tests.
