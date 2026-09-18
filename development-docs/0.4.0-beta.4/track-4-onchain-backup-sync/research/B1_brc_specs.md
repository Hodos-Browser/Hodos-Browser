# B1 — BRC-38 / 39 / 40 / 42 / 43 digest (primary source)

Source: `github.com/bsv-blockchain/BRCs`, shallow clone at commit `c1d12f2e173857ae67ca45ca483d6ed26565af11` (2026-08-21, "BRC-169: certificate custody...").
Local copy: `C:/Users/archb/AppData/Local/Temp/claude/C--Users-archb-Marston-Enterprises-Standards-BRCs-drafts/8069cf43-d637-4ddb-9a45-1ef359a2624b/scratchpad/repos/BRCs`

Files read IN FULL (line counts):

- `outpoints/0038.md` (614 lines) — User Wallet Data Format
- `outpoints/0039.md` (330 lines) — encryption extension
- `outpoints/0040.md` (287 lines) — synchronization
- `key-derivation/0042.md` (153 lines) — BSV Key Derivation Scheme (BKDS)
- `key-derivation/0043.md` (92 lines) — Security Levels, Protocol IDs, Key IDs and Counterparties
- `key-derivation/0044.md` (lines 1–40, complete Specification section) — admin-reserved protocol IDs

All claims below are VERIFIED against these files unless explicitly labelled INFERRED. Line references are to the files above. BRC-38/39 use numbered sections (§N); BRC-40 has **named, unnumbered** sections — cited by name + line.

Registry note (VERIFIED): `README.md` lines 104–106 and `SUMMARY.md` lines 195–197 list BRC-38/39/40 in the registry index (the README table has no status column). Whether "listed" equals "reserved/accepted" in the maintainers' process is NOT checked here.

---

## 1. BRC-38: User Wallet Data Format

### 1.1 Canonical file form (§3, lines 55–63)

- UTF-8 JSON serialized per **RFC 8785 JSON Canonicalization Scheme (JCS)** (line 57).
- File extension SHOULD be `.brc38.json` (lines 59–61).
- If wrapped/compressed/encrypted, "the canonical BRC-38 payload remains the inner JSON document" (line 63).

### 1.2 Top-level document (§4, lines 65–113)

Required shape: `brc` (MUST = 38), `title` (MUST = `"User Wallet Data Format"`), `formatVersion` (MUST = 1), `exportedAt` (MUST be UTC finalization timestamp), `sourceStorage` (MUST be the storage's `settings` row in portable form: `created_at`, `updated_at`, `storageIdentityKey`, `storageName`, `chain`, `dbtype`, `maxOutputScript` — lines 75–83), `user` (MUST contain **exactly one** user row), `tables` (MUST contain **every** array listed, even if empty — line 113).

### 1.3 The 13 tables (§4 lines 88–100; row forms §7.2–§7.14)

The `tables` object has exactly these 13 arrays. §7 defines 14 row forms: §7.1 `user` is the top-level singleton, not in `tables`. Every row form starts with `created_at`, `updated_at`.

| # | `tables` key | Row form § | Every field (beyond `created_at`, `updated_at`) |
|---|---|---|---|
| 1 | `provenTxs` | §7.2 | `provenTxId`, `txid` (hex), `height`, `index`, `merklePath` (base64), `rawTx` (base64), `blockHash` (hex), `merkleRoot` (hex) |
| 2 | `provenTxReqs` | §7.3 | `provenTxReqId`, `provenTxId`, `status`, `attempts`, `notified`, `txid`, `batch`, `history` (object; MUST conform to `{"notes":[{"when","what"}]}`, lines 277–288), `notify` (object; MUST conform to `{"transactionIds":[int]}`, lines 290–295), `rawTx` (base64), `inputBEEF` (base64) |
| 3 | `outputBaskets` | §7.4 | `basketId`, `userId`, `name`, `numberOfDesiredUTXOs`, `minimumDesiredUTXOValue`, `isDeleted` |
| 4 | `transactions` | §7.5 | `transactionId`, `userId`, `provenTxId`, `status`, `reference` (base64), `isOutgoing`, `satoshis`, `description`, `version`, `lockTime`, `txid`, `inputBEEF` (base64), `rawTx` (base64) — **no `isDeleted`** |
| 5 | `commissions` | §7.6 | `commissionId`, `userId`, `transactionId`, `satoshis`, `keyOffset` (string), `isRedeemed`, `lockingScript` (base64) |
| 6 | `outputs` | §7.7 | `outputId`, `userId`, `transactionId`, `basketId`, `spendable`, `change`, `outputDescription`, `vout`, `satoshis`, `providedBy`, `purpose`, `type`, `txid`, `senderIdentityKey` (hex), `derivationPrefix` (base64), `derivationSuffix` (base64), `customInstructions` (optional), `spentBy`, `sequenceNumber`, `spendingDescription` (optional), `scriptLength`, `scriptOffset`, `lockingScript` (base64) — **no `isDeleted`** |
| 7 | `outputTags` | §7.8 | `outputTagId`, `userId`, `tag`, `isDeleted` |
| 8 | `outputTagMaps` | §7.9 | `outputTagId`, `outputId`, `isDeleted` — no primary ID of its own |
| 9 | `txLabels` | §7.10 | `txLabelId`, `userId`, `label`, `isDeleted` |
| 10 | `txLabelMaps` | §7.11 | `txLabelId`, `transactionId`, `isDeleted` — no primary ID of its own |
| 11 | `certificates` | §7.12 | `certificateId`, `userId`, `type` (base64), `serialNumber` (base64), `certifier` (hex), `subject` (hex), `verifier` (hex), `revocationOutpoint` (`<txid>.<vout>`), `signature` (hex), `isDeleted` |
| 12 | `certificateFields` | §7.13 | `userId`, `certificateId`, `fieldName`, `fieldValue`, `masterKey` (base64) — **no own primary ID**; natural key (`certificateId`, `fieldName`) per §8 ordering |
| 13 | `syncStates` | §7.14 | `syncStateId`, `userId`, `storageIdentityKey` (hex), `storageName`, `status`, `init`, `refNum`, `syncMap` (object with 12 entity entries `{entityName, idMap, count}`: provenTx, outputBasket, transaction, provenTxReq, txLabel, txLabelMap, output, outputTag, outputTagMap, certificate, certificateField, commission — lines 479–492), `when`, `satoshis`, `errorLocal` (object or omitted), `errorOther` (object or omitted); error objects MUST conform to `{code, description, stack?}` (lines 507–515) |

§7.1 `user` (top-level singleton): `created_at`, `updated_at`, `userId`, `identityKey` (hex), `activeStorage` (a storageIdentityKey).

### 1.4 Canonical value encoding (§5)

- **§5.1 Timestamps**: exact form `YYYY-MM-DDTHH:MM:SS.sssZ`, UTC only; offsets other than `Z` MUST NOT be used (lines 117–124).
- **§5.2 Binary**: `number[]` byte arrays exported as RFC 4648 base64 **with padding**, no whitespace. Exactly 8 fields are binary: `Commission.lockingScript`, `Output.lockingScript`, `ProvenTx.merklePath`, `ProvenTx.rawTx`, `ProvenTxReq.rawTx`, `ProvenTxReq.inputBEEF`, `Transaction.inputBEEF`, `Transaction.rawTx` (lines 127–140).
- **§5.3 JSON-in-string fields decoded to real JSON** in the export: `ProvenTxReq.history`, `ProvenTxReq.notify`, `SyncState.syncMap`, `SyncState.errorLocal`, `SyncState.errorOther` (lines 142–160).
- **§5.4 Optional fields**: "If a field is absent or undefined in the source dataset, it MUST be omitted from the exported JSON rather than emitted as `null`" (line 164). Consequence: JSON `null` never appears in a conforming export.

### 1.5 Export closure (§6)

- §6.1: exactly one `user` row + one `sourceStorage` object.
- §6.2: MUST include every row where `userId` matches, from: `output_baskets`, `transactions`, `commissions`, `outputs`, `output_tags`, `tx_labels`, `certificates`, `certificate_fields`, `sync_states` (lines 179–189).
- §6.3: MUST include `tx_labels_map` rows whose `txLabelId` references an exported label, and `output_tags_map` rows whose `outputTagId` references an exported tag; SHOULD verify map rows also reference exported transactions/outputs (lines 191–201).
- §6.4: MUST include `proven_tx_reqs` whose `txid` matches an exported transaction's txid, and `proven_txs` referenced by an exported transaction or exported req (lines 203–210).
- §6.5: storage-global tables MUST NOT be included; concretely `monitor_events` MUST NOT be included (lines 212–218; restated §12 line 607).

### 1.6 Array ordering (§8, lines 517–533)

`tables` arrays MUST be sorted ascending: by primary ID for the 11 tables that have one; `outputTagMaps` by (`outputId`, `outputTagId`); `txLabelMaps` by (`transactionId`, `txLabelId`); `certificateFields` by (`certificateId`, `fieldName`).

### 1.7 Relationship integrity (§9, lines 535–551)

Every foreign-key-like reference MUST resolve within the document (except intentionally optional unresolved refs). Nine minimum checks listed (userId equality for transactions/outputs; output/commission → transaction; maps → both parents; certificateField → certificate). If `user.activeStorage != sourceStorage.storageIdentityKey`, the exporter MUST still preserve the exact `activeStorage` value (line 551).

### 1.8 Import semantics (§10, lines 553–570)

Importers: MUST preserve row values and relationships **semantically**; MAY preserve numeric primary IDs exactly; MAY **remap numeric primary IDs** provided every internal reference is updated consistently; MUST preserve tombstones such as `isDeleted`; MUST preserve status fields exactly. `user.activeStorage` and `syncStates` MAY be treated as operationally sensitive (importer chooses whether to activate them after import), but MUST still be parsed and preserved (lines 565–570).

### 1.9 Confirm/deny items

- **No extension mechanism — CONFIRMED ABSENT.** `formatVersion` MUST equal 1 (line 109); `tables` MUST contain exactly the listed arrays (line 113); there is no reserved-field, vendor-extension, or `x-`-style provision anywhere in the 614 lines.
- **No preserve-unknown-fields rule — CONFIRMED ABSENT.** §10 says nothing about unknown fields or unknown tables; the spec neither permits nor forbids them, and gives importers no obligation to round-trip anything outside the defined forms.
- **Root-key material excluded — CONFIRMED.** §1 Scope (line 45): the spec does not define "root-key, profile, or encrypted snapshot material that is not part of the Wallet Toolbox storage schema". A BRC-38 blob therefore does NOT restore spendability by itself; the root key travels separately.
- **"does not yet define a single-file export" language — PRESENT.** Motivation, line 30: "Wallet Toolbox already has a rich schema that captures this information, but it does not yet define a canonical single-file export format. This specification fills that gap." Grammatically "it" = Wallet Toolbox. INFERRED: likely stale versus toolbox reality — BRC-39 line 104 already speaks of "the Wallet Toolbox Argon2id defaults this specification is intended to standardize", implying toolbox export machinery exists. The code-side check is A1/C1's territory.

### 1.10 Privacy (§11, lines 572–585)

Exports are "intentionally complete and sensitive" (may contain raw transactions, merkle proofs, labels, certificate contents, encrypted field material, derivation metadata, storage topology hints); implementations MUST treat blobs as highly sensitive user data.

---

## 2. BRC-39: Encrypted wrapper for BRC-38

### 2.1 Artifact (§3, lines 60–68)

Binary file; SHOULD be named `wallet.brc39`; SHOULD use media type `application/vnd.brc39.wallet`. Plaintext MUST be a complete canonical UTF-8 BRC-38 JSON document (§4.1).

### 2.2 Crypto profile (§4)

- **Password** (§4.2, lines 78–86): Unicode → NFC normalize → UTF-8 encode. No trimming, case folding, or whitespace normalization permitted.
- **KDF** (§4.3, lines 88–106): Argon2id MANDATORY for new exports. Derived key 32 bytes. Canonical defaults: iterations **7**, memoryKiB **131072** (128 MiB), parallelism **1**, hashLength **32**, saltLength **32**. Stronger params allowed but MUST be encoded in the header.
- **AEAD** (§4.4, lines 108–120): AES-256-GCM; key = the 32-byte Argon2id output; **nonce 32 bytes, random per export**; GCM tag 16 bytes. (INFERRED implementation caveat: 32-byte GCM nonces are non-default — most APIs assume 12-byte IVs; GCM supports other lengths via GHASH and WebCrypto accepts them, but the length must be handled explicitly.)

### 2.3 File layout (§5, lines 128–149) — all integers unsigned big-endian

| Field | Len | Value |
|---|---|---|
| Magic | 4 | ASCII `WDAT` (`57 44 41 54`, §6.1) |
| Format Version | 1 | `0x01` |
| Protector Type | 1 | `0x01` = password (`0x02`–`0xff` reserved, §6.3) |
| Inner Format | 1 | `0x26` (decimal 38) = BRC-38 |
| KDF Type | 1 | `0x01` = Argon2id (others reserved, §6.5) |
| Flags | 1 | `0x00`; all bits reserved, MUST be zero (§6.6) |
| Salt Length | 1 | `0x20` (32) |
| Nonce Length | 1 | `0x20` (32) |
| Argon2id Iterations | 4 | work factor |
| Argon2id Memory KiB | 4 | memory cost |
| Argon2id Parallelism | 1 | |
| Argon2id Hash Length | 1 | 32 |
| Reserved | 12 | MUST be all zero; non-zero → reject (§6.7) |
| Salt | var | Salt Length bytes |
| Nonce | var | Nonce Length bytes |
| CiphertextAndTag | var | AES-256-GCM ciphertext ‖ 16-byte tag |

### 2.4 Procedures and validation

- Export procedure: §7, 10 steps (lines 213–226). Import procedure: §8, 9 steps (lines 230–244); "Only after successful BRC-38 validation may the importer proceed to import wallet data into local storage" (line 244).
- Rejection list §11 (lines 272–290): bad magic/version/protector/inner-format/KDF/flags; zero salt or nonce length; non-zero reserved bytes; invalid Argon2id params (zero iterations/memoryKiB/parallelism, hashLength ≠ 32); GCM auth failure; plaintext not valid UTF-8 JSON; plaintext not a valid BRC-38 document.
- §9 (lines 248–250): MUST reject KDF Type ≠ `0x01`; no legacy-KDF import mode.
- §10.2 (lines 262–268): recipient-encrypted (BRC-78-style, BRC-42/43-derived) wallet exports are NOT standardized in this version; "future protector types MAY define such a mode". Relevant to us: password is the only standardized protector today — a seed-derived-key protector would be a new Protector Type extension, not current BRC-39.
- §12 (lines 292–301): MUST warn users about weak passwords; SHOULD default to params at least as strong as canonical defaults.

---

## 3. BRC-40: User Wallet Data Synchronization

BRC-40 has **no numbered sections**; citations are by section name + line.

### 3.1 Model (Abstract lines 7–9; Scope lines 31–41)

Chunked, resumable, incremental replication of one wallet user's records **between wallet storage providers** — a **Producer** (supplies records) and a **Consumer** (merges them). Transport-agnostic (line 41). Scope explicitly excludes: wallet-to-app APIs (BRC-100), **backup file formats**, and encryption at rest or in transit (lines 37–40).

**One wallet vs cross-vendor:** it is single-user, storage-provider-to-storage-provider sync ("synchronization between wallet storage providers for a single wallet user", line 33). The interoperability aim is that *independent storage backends* can exchange state "without proprietary adapters" (line 7) — cross-vendor in the storage-backend sense, but not a device-to-device merge protocol and not a migration file format; migration-as-a-file is BRC-38's job (0038.md line 14).

### 3.2 Record model (lines 52–77)

Exactly **12 entity names** in fixed order: `provenTx`, `outputBasket`, `outputTag`, `txLabel`, `transaction`, `output`, `txLabelMap`, `outputTagMap`, `certificate`, `certificateField`, `commission`, `provenTxReq` — plus an optional single `user` record (line 69). **`syncState` is NOT a synced entity**: BRC-38 backs up `syncStates`; BRC-40 never transmits them. Every record MUST belong to the requested `identityKey`, MUST include `created_at`/`updated_at`, MUST NOT contain `null` (omit instead — same rule as BRC-38 §5.4) (lines 71–75). JSON timestamps SHOULD be RFC 3339 / ISO 8601 strings (line 77).

### 3.3 Request and chunk (Request Structure lines 97–137; Producer Behavior lines 138–158)

Request fields: `fromStorageIdentityKey` (producer), `toStorageIdentityKey` (consumer), `identityKey` (user), optional `since` watermark (MUST be omitted for initial full sync), `maxItems` (bounds total records across all arrays), `maxRoughSize` (bounds approximate serialized size), `offsets` — one `{name, offset}` per entity **in the exact entity order**; missing/duplicated/out-of-order entries → producer MUST reject (line 136).

**A chunk** is one `SyncChunk` response (lines 160–183): the producer walks entity types in order; includes the `user` record if `since` absent or user `updated_at` later than `since`; returns records with `updated_at >= since`, skipping the first `offset` per entity; stops when maxItems or maxRoughSize is exhausted — including the record that exceeds maxRoughSize, then stopping (line 152). Entity-array semantics (lines 185–189): property **omitted** = not attempted this chunk; **`[]`** = attempted, exhausted at current since/offset; **non-empty** = records to merge. Producer MUST use a deterministic record order stable throughout a sync cycle (line 158).

### 3.4 Ordering, merge, completion

- **Completion** (lines 191–195): a sync cycle completes only when *all* entity-array properties are present and *all* empty. The optional `user` record does not affect completion.
- **Inclusive `since`** (lines 197–203): `updated_at >= since`; a resumed cycle typically repeats at least one record; consumers MUST merge idempotently and MUST NOT treat repeats as an error.
- **Merge** (lines 205–221): per record — match against a local record "under that entity's convergent equality rules" (⚠ **referenced but never defined** anywhere in the spec); update or insert; update the entity `idMap` (producer-local numeric ID → consumer-local numeric ID); track per-entity `maxUpdated_at`; bump per-entity `count`. If an existing producer ID would map to a different local ID than previously recorded, "synchronization MUST fail" (line 215). `idMap` MAY be omitted for relationship-identified entities: `certificateField`, `txLabelMap`, `outputTagMap` (lines 217–221) — exactly the three BRC-38 row forms without their own primary ID.
- **Advancing state** (lines 223–237): mid-cycle → retain `since`, retain updated counts, request next chunk with them. Cycle complete → `since` := max `updated_at` observed in the cycle, reset all counts to 0. An empty completed cycle MAY leave `since` unchanged.
- **Sync state durability** (lines 79–95): consumer MUST keep durable state per (wallet user, remote producer): producer `storageIdentityKey`/`storageName`, `since`, per-entity `count`, `maxUpdated_at`, `idMap`; interrupted sync MUST resume from stored state.
- **Auth** (lines 239–248): producer MUST only return data for the authenticated identity and MUST reject cross-user requests; auth protocol unspecified, but storage identity keys must not be forgeable within the sync context.
- **Errors** (lines 250–260): fail on unknown/unauthorized identityKey, malformed/out-of-order offsets, missing/invalid timestamps, any `null` in a returned entity, conflicting idMap assignment. On failure, preserve enough sync state to retry safely or report last durable state.
- **Interop notes** (lines 262–271): follows Toolbox's `getSyncChunk` / `processSyncChunk` / durable `syncState` / persistent `syncMap` model.

### 3.5 How BRC-38 relates to BRC-40 textually

- BRC-38 lists BRC-40 in its References (0038.md line 611); both say they are based on the Wallet Toolbox storage implementation.
- **BRC-40 never references BRC-38.** Its References are BRC-36, BRC-37, BRC-100 only (lines 283–287), and its Scope disclaims backup file formats. BRC-40 does not state that its entity records use the BRC-38 portable row forms; the alignment (entity names identical to BRC-38 §7.14's `syncMap` keys, same null-omission rule, same underlying schema) is structural common ancestry, not a normative link. VERIFIED absence of the cross-reference; "they share row forms" is INFERRED, not stated by either spec.

---

## 4. BRC-42 + BRC-43: key derivation and the invoice number

### 4.1 BRC-42 mechanics (0042.md, Specification lines 17–59)

- Each party has a secp256k1 master keypair ("Identity Keys", line 23).
- Shared secret = ECDH: own master private key × counterparty master public key (point multiplication) (line 27).
- **HMAC over the invoice number**, keyed by the shared secret; the invoice number "is treated as a string and is converted into a buffer using UTF-8 encoding" (line 39).
- HMAC output → scalar via big-endian encoding (line 29).
- Child public key (sender side): recipient master pubkey point + scalar·G (lines 43–48).
- Child private key (recipient side): recipient master privkey + scalar mod N (lines 52–55).
- ⚠ VERIFIED omissions in the spec text: it never names the HMAC hash function and never specifies how the ECDH shared point is encoded as the HMAC key. The test vectors (lines 75–145) pin behavior for implementers. (INFERRED from ecosystem knowledge, not verified here: implementations use the compressed point encoding and HMAC-SHA-256.)

### 4.2 BRC-43 invoice-number format (0043.md)

Format (lines 27–31): `<securityLevel>-<protocolID>-<keyID>`.

**Protocol ID rules** (lines 43–52): only letters, numbers and spaces; no multiple spaces; all lower case when used; max 280 chars; **min 5 chars**; must not end with `" protocol"`; leading/trailing spaces removed. All strings normalizing to the same value are the same protocol (line 54). **Key ID**: at least 1 byte, at most 1033 bytes (line 56). BRC-44 (0044.md line 19) reserves any protocol ID starting with `admin` for client-internal use.

**Security levels** (lines 37–39): `0` = no permission prompt, always allowed; `1` = per-application grant for the protocol, then usable with **any counterparty**; `2` = a **new counterparty-specific grant for every counterparty** under that protocol.

**Counterparty = self, mechanically** (line 35): "When there is only one party, we specify that their single key be used both as the sender and the recipient. This is known as self-derivation." In BRC-42 terms: sender master key = recipient master key = your own; ECDH shared secret = your privkey × your own pubkey; child private key = your master private key + HMAC-scalar mod N. Example 1 (lines 64–70) shows the client deriving the child public key as sender and the child private key as recipient from the same invoice number. Fully deterministic from the root key alone — exactly what a seed-derived backup address needs. (`anyone` = the constant private key `1`; line 35 — not relevant to backup.)

### 4.3 The correct derivation string for our backup

With protocol ID `wallet backup` — 13 chars: valid length, letters+space only, lowercase, does not end with " protocol", not `admin`-reserved, and (VERIFIED by grep across the whole BRCs repo) not claimed by any existing BRC — and key ID `1`, counterparty self:

- **Security level 1:** invoice number = `1-wallet backup-1`
- **Security level 2:** invoice number = `2-wallet backup-1`

**Observations on our docs' string `1-wallet-backup-1`:**

1. The leading `1` denotes **security level 1**; our BRC draft says level 2, which produces a string starting `2-`. Level 2 means counterparty-specific permission grants (0043.md line 39) — the strictest sensible choice for a backup key. One of the two (current-system doc vs BRC draft) must change; which reflects the code is A1's question.
2. The internal separator in `wallet-backup` is a **hyphen, which is not a legal protocol-ID character** (letters, numbers, spaces only — 0043.md line 46). `wallet-backup` cannot normalize to a valid BRC-43 protocol ID, and BRC-43-conforming tooling given protocol `wallet backup` would emit `1-wallet backup-1` with a space. So the literal `1-wallet-backup-1` is either produced by a hand-composed, non-BRC-43 code path, or is a doc typo. It is also ambiguous to parse, since hyphens both delimit components and appear inside the middle component.
3. Nothing here verifies what the code does — the specs say exactly: level 1 → `1-wallet backup-1`; level 2 → `2-wallet backup-1`.

---

## 5. What 38/39/40 constrain or enable for a DELTA format

**Enables:**

1. **Per-row watermarks are native.** Every BRC-38 row form carries `created_at`/`updated_at` (§7.2–§7.14), and BRC-40's entire delta selection is `updated_at >= since`. A delta chain can reuse this watermark mechanism directly.
2. **BRC-40's SyncChunk is already a changeset container.** Entity arrays in fixed dependency order (parents before children: provenTx → baskets/tags/labels → transaction → output → maps → certificates → …), omitted/empty/non-empty semantics, mandatory idempotent merge, and a defined completion condition. A delta-file format can adopt the SyncChunk shape (12 entity arrays + optional `user`) plus watermark metadata; the apply algorithm is then BRC-40's Consumer Merge Behavior nearly verbatim.
3. **Deterministic ordering exists.** BRC-38 §8 canonically sorts every table (JCS fixes object-key order); BRC-40 requires cycle-stable producer order. Canonical, byte-reproducible delta serialization is well-founded.
4. **Natural keys exist for the ID-less forms.** `certificateFields` (certificateId, fieldName), `txLabelMaps` (transactionId, txLabelId), `outputTagMaps` (outputId, outputTagId) — the same three entities BRC-40 permits merging without an idMap.

**Constrains:**

5. **Numeric primary IDs are storage-local and remappable.** BRC-38 §10 lets importers remap IDs; BRC-40 exists partly to maintain producer→consumer `idMap`s. A delta chain keyed on numeric IDs is coherent only relative to one fixed source storage. A delta format must either pin `sourceStorage.storageIdentityKey` across the chain (workable: our chain is written by one wallet against its own storage) or re-key on natural identifiers (`txid`, basket `name`, `tag`/`label` text, certificate (`type`, `serialNumber`)).
6. **Tombstones are partial.** `isDeleted` exists only on outputBaskets, outputTags, outputTagMaps, txLabels, txLabelMaps, certificates. `transactions`, `outputs`, `provenTxs`, `provenTxReqs`, `commissions`, `certificateFields`, `syncStates` have **no** tombstone; their lifecycle is state-based (`status`, `spendable`, `spentBy`, `isRedeemed`). A hard delete of a non-tombstoned row is unrepresentable as a row-form delta; a delta format inherits this limit.
7. **Null-omission forbids field-level deltas.** Both 38 (§5.4) and 40 (line 75) ban `null` and require omitting absent fields, so in a partial row "field omitted" is indistinguishable from "field cleared". The only spec-compatible delta granularity is the **whole row** (full portable row form replacing prior state) — exactly how BRC-40 records work. DELTA_ANALYSIS should treat row-level replacement as the unit of change.
8. **No extension point in BRC-38.** No extra top-level keys, tables, or unknown-field preservation (§1.9 above). A delta cannot ride inside a BRC-38 document; it must be a sibling format with its own header. Clean hook on the envelope side: BRC-39's `Inner Format` byte and Protector Type space are explicitly reserved for future values (§6.3, §6.4) — a new inner-format code for a delta payload could reuse the same `WDAT` envelope discipline.
9. **Inclusive `since` implies overlap.** Watermark-based deltas re-include boundary records; consumers must be idempotent (BRC-40 mandates this). Delta links in a chain should tolerate one-record overlap rather than assume disjointness.
10. **`syncStates` asymmetry.** Present in BRC-38 backups, absent from BRC-40's entity list. For a delta chain, syncStates are the writer's per-storage operational bookkeeping; BRC-38 §10 already flags them as operationally sensitive on import (importer may decline to activate, but MUST parse and preserve). Reasonable posture: include for full fidelity, expect importers not to activate.
11. **BRC-40's "convergent equality rules" are undefined.** If our delta BRC leans on BRC-40 merge semantics, we must define per-entity identity/equality rules ourselves (e.g. `provenTx` by `txid`; `output` by (`transactionId`, `vout`); `transaction` by `txid` or `reference`) — the spec names the slot but supplies no content.

---

## Checked

- `outpoints/0038.md` — read in full, lines 1–614 (§1–§12 + references).
- `outpoints/0039.md` — read in full, lines 1–330 (§1–§13 + implementation + references).
- `outpoints/0040.md` — read in full, lines 1–287 (all named sections + references).
- `key-derivation/0042.md` — read in full, lines 1–153 (spec, test vectors, references).
- `key-derivation/0043.md` — read in full, lines 1–92 (spec, rules, examples, implementation note).
- `key-derivation/0044.md` — lines 1–40 (complete Specification: `admin*` reservation).
- `README.md` lines 104–106 and `SUMMARY.md` lines 195–197 — BRC-38/39/40 registry listing.
- Repo-wide grep for `wallet backup` / `wallet-backup`: zero hits — no existing BRC claims that protocol ID.
- Repo commit: `c1d12f2e173857ae67ca45ca483d6ed26565af11`, 2026-08-21.

## NOT checked

- Any Hodos or wallet-toolbox **code** (derivation strings, export implementation) — A1/C1 territory; nothing here verifies what our code or the toolbox actually does.
- `wallet/0002.md` (BRC-2), `peer-to-peer/0078.md` (BRC-78), `wallet/0100.md` (BRC-100), `outpoints/0036.md`, `outpoints/0037.md` — cited by the read specs, not read.
- The ts-sdk / go-sdk implementations of BRC-42 (the HMAC-SHA-256 / compressed-point detail in §4.1 stays INFERRED).
- Whether toolbox today implements BRC-38 export (bears on the "does not yet define" staleness call — flagged as likely stale, INFERRED).
- BRC-42 test vectors were read, not executed.
- The BRC maintainers' process semantics of registry listing (listed vs reserved vs final).
