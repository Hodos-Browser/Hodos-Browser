# B2 — Work item 0: table-by-table BRC-38 compatibility assessment

Date: 2026-08-22.
Our schema source: **code**, `rust-wallet/src/database/migrations.rs` (consolidated V1 DDL + guarded incremental ALTERs) and `rust-wallet/src/database/connection.rs` (migration runner + startup-repair ALTERs). What we actually back up: `rust-wallet/src/backup.rs::collect_payload` / `compress_for_onchain`.
BRC-38 source: `outpoints/0038.md` at commit `c1d12f2e173857ae67ca45ca483d6ed26565af11` of `bsv-blockchain/BRCs` (local clone; same commit B1 used). Field lists below were re-read from the spec file directly, not taken from the B1 digest.

Labels: **VERIFIED** = read in the cited file at the cited lines. **INFERRED** = my conclusion from verified facts.

---

## 0. How our effective schema is determined (VERIFIED)

`migrations.rs::create_schema_v1` (lines 13–521) creates the consolidated schema; the runner in `connection.rs` (lines ~789–997) then applies every incremental migration `v1_to_v2` … `v23_to_v24` in order, each guarded by a PRAGMA column-existence check; finally `connection.rs` lines 1000–1074 run unconditional "startup repair" ALTERs. So the **effective** live schema is V1 **plus**:

- `transactions` + `recipient`, `recipient_name` (migrations.rs 827–854; repair connection.rs 1044–1057)
- `certificates` + `publish_status`, `publish_txid`, `publish_vout` (repair only — connection.rs 1027–1042; **not in the V1 DDL at all**)
- `settings` + `default_per_tx_limit_cents`, `default_per_session_limit_cents`, `default_rate_limit_per_min` (v10), `default_max_tx_per_session` (v12), `default_identity_key_disclosure_allowed` (v19), `default_prefill_from_manifest` (v24), `backup_hash`, `last_backup_at` (repair, connection.rs 1059–1072)
- `domain_permissions` + `identity_key_disclosure_allowed` (v17), `bundled_scope_grant` (v22)
- plus non-backup tables added v14–v24 (peerpay_*, domain_*_permissions, permission_audit_log, bsv_price_cache, domain_manifest_snapshots)

## 0b. What the backup payload actually contains (VERIFIED, backup.rs)

`BackupPayload` (backup.rs 32–59) serializes **21 arrays/objects over 19 SQL tables**: wallet (singleton), users, addresses, output_baskets, transactions, outputs, proven_txs, proven_tx_reqs, certificates, certificate_fields, output_tags, output_tag_map, tx_labels, tx_labels_map, commissions, settings, sync_states, domain_permissions, cert_field_permissions — plus parent_transactions and block_headers, which `compress_for_onchain` clears to empty before writing (backup.rs 1015–1016).

Not backed up at all (in DB, absent from BackupPayload): messages, relay_messages, transaction_inputs, transaction_outputs, monitor_events, derived_key_cache, peerpay_received, peerpay_pending_verification, peerpay_outbox, domain_protocol_permissions, domain_basket_permissions, domain_counterparty_permissions, permission_audit_log, bsv_price_cache, domain_manifest_snapshots. (BRC-38 §6.5 in fact **forbids** including `monitor_events` — we already comply.)

---

## 1. THE TABLE — one row per backed-up table

Tier definitions from `0.4.0-beta.4/sprint-4-onchain-backup-sync/README.md` item 0: **1 Core** = derivation data, unspent outputs + spending instructions, baskets. **2 Assets & attestations** = certificates + keyrings, credentials, token outputs, labels, tags. **3 Wallet-local** = permissions, prefs, device labels, sync state.

Column lists: ours from migrations.rs DDL (line refs in §2); BRC-38 from 0038.md §7.x (re-read directly). "Extra" = our column with no BRC-38 field. "Missing" = BRC-38 field we don't have.

| Our table | BRC-38 table (§7.x) | Verdict | Tier | What to do |
|---|---|---|---|---|
| `users` | `user` singleton (§7.1) | **Field diffs, small.** Renames: `identity_key`→`identityKey`, `active_storage`→`activeStorage`. **Value mismatch:** our `active_storage` defaults to the literal `'local'` (migrations.rs 111); §7.1 expects a `storageIdentityKey`. §4 requires **exactly one** user; our table allows many (we export all, backup.rs 390). | 1 | Adapter: map `'local'` → `settings.storage_identity_key`; assert single user on export. |
| `proven_txs` | `provenTxs` (§7.2) | **Maps cleanly.** Rename `tx_index`→`index`; BLOBs→base64 (already done). Caveat: our `block_hash`/`merkle_root` are `NOT NULL DEFAULT ''` (migrations.rs 123–124) and our on-chain payload **strips `merkle_path` to ''** (backup.rs 1043–1045) — §7.2 shows both as required `<hex>`/`<base64>`. Empty string is not "absent" under §5.4. | 1 | Adapter: omit `merklePath`/`blockHash`/`merkleRoot` when empty (treat stripped = absent-in-source, §5.4 reading — declare this in our BRC); re-fetchable per current doc. |
| `proven_tx_reqs` | `provenTxReqs` (§7.3) | **Maps cleanly.** Renames to camelCase; `history`/`notify` TEXT-JSON → real JSON objects (§5.3 requires decoded). Our on-chain payload strips `raw_tx`, `input_beef` and caps `history` to 5 entries (backup.rs 1018–1041) — same stripped-profile caveat as proven_txs. | 1 | Adapter + declared stripped profile. Note §7.3 `history` MUST conform to `{"notes":[...]}` — ours is a `{timestamp: note}` map (backup.rs 1030 sorts keys as timestamps). **Shape mismatch, not just encoding** — needs a transform or an honest deviation note. |
| `output_baskets` | `outputBaskets` (§7.4) | **Field diffs.** Renames fine. **Extra: `description`, `token_type`, `protocol_id`** (migrations.rs 206–208) — token metadata, no BRC-38 home. | 1 (basket core) / extras are 2 | Core fields → §7.4. `token_type`/`protocol_id`/`description` → extension section (tier-2 token metadata; losing them breaks token-basket semantics on restore). |
| `transactions` | `transactions` (§7.5) | **Field diffs.** Renames: `id`→`transactionId`, `reference_number`→`reference` (ours TEXT e.g. `backup-…`, §7.5 wants base64 — encode UTF-8→base64), `is_outgoing`→`isOutgoing`, `lock_time`→`lockTime`, `input_beef`→`inputBEEF`; `raw_tx` ours TEXT hex (BLOB fallback in backup.rs 453–459) → base64. **Extra: `block_height`, `confirmations`, `failed_at`, `price_usd_cents`, `recipient`, `recipient_name`.** Missing: none. | 1 (record + FK anchor for outputs) / extras 2–3 | Core → §7.5. `block_height`/`confirmations` recomputable from provenTx — drop. `failed_at` moot (we exclude failed txs from backup, backup.rs 448). `price_usd_cents`, `recipient`, `recipient_name` are user records → extension (tier 2). |
| `commissions` | `commissions` (§7.6) | **Maps cleanly.** Renames only (`key_offset`→`keyOffset`, `is_redeemed`→`isRedeemed`); locking_script already base64. | 2 | Direct adapter. |
| `outputs` | `outputs` (§7.7) | **Field diffs, one extra.** All 23 §7.7 fields present. Renames to camelCase. **Extra: `confirmed`** (migrations.rs 240, v14). Encoding: §7.7 `derivationPrefix`/`derivationSuffix` are `<base64>`; ours are plain TEXT (`'1-wallet-backup'`, `'1'`, `'marker'` — handlers.rs 14037, 14047) → base64-encode UTF-8 on export, lossless. Our on-chain payload strips `locking_script` from spent outputs (backup.rs 506–510) — stripped-profile caveat again. | 1 | Adapter; `confirmed` → extension (or drop: recomputable from chain — but it gates our UTXO-sync logic, so keep in extension). |
| `output_tags` | `outputTags` (§7.8) | **Maps cleanly.** Rename `id`→`outputTagId`. | 2 | Direct adapter. |
| `output_tag_map` | `outputTagMaps` (§7.9) | **Field diffs, trivial.** **Extra: surrogate `id` PK** (migrations.rs 263) — §7.9 has no own ID. Drop on export. §8 orders by (`outputId`,`outputTagId`); we export ORDER BY id (backup.rs 662) — re-sort. | 2 | Drop `id`, re-sort, rename. |
| `tx_labels` | `txLabels` (§7.10) | **Maps cleanly.** | 2 | Direct adapter. |
| `tx_labels_map` | `txLabelMaps` (§7.11) | **Maps cleanly.** §8 orders by (`transactionId`,`txLabelId`); we export ORDER BY txLabelId, transaction_id (backup.rs 694) — re-sort. | 2 | Re-sort, rename. |
| `certificates` | `certificates` (§7.12) | **Field diffs.** **Extra: `publish_status`, `publish_txid`, `publish_vout`** (connection.rs 1035–1041; exported at backup.rs 602–604) — our on-chain-publication pointers, no BRC-38 home. §7.12 `type`/`serialNumber` are `<base64>` — ours TEXT (BRC-52 values are base64 strings already; INFERRED compatible, verify a live row). | 2 | Core → §7.12; `publish_*` → extension (tier 2 — losing them orphans published certs). |
| `certificate_fields` | `certificateFields` (§7.13) | **Maps cleanly.** Composite natural key both sides; our export order (certificateId, field_name — backup.rs 629) already matches §8. | 2 | Direct adapter. |
| `sync_states` | `syncStates` (§7.14) | **Maps cleanly.** Renames: `ref_num`→`refNum`, `sync_when`→`when`; `sync_map`/`error_local`/`error_other` TEXT-JSON → objects (§5.3). | 3 | Adapter; importers MAY not activate (§10) — fine. |
| `settings` | `sourceStorage` top-level object (§4) — **not a table** | **Partial.** The 7 §4 fields all exist (migrations.rs 337–344 + repair). **Extra: `sender_display_name`, 6× `default_*` prefs, `backup_hash`, `last_backup_at`** — no home. | 3 (extras) | 7 core fields → `sourceStorage`; the rest → extension or drop (prefs re-enterable; `backup_hash`/`last_backup_at` are per-device state that must NOT travel). |
| `wallets` | — | **No BRC-38 home.** We back up only `id`, `current_index`, `backed_up`, `created_at`, `updated_at` (backup.rs 377; mnemonic/pin_salt/mnemonic_dpapi excluded in code). **`current_index` is the HD derivation cursor — tier-1 derivation data with no BRC-38 home.** BRC-38 §1 excludes root-key/profile material by design. | **1** (`current_index`) / 3 (rest) | `current_index` MUST travel → extension section. `backed_up` flag: drop (per-device). |
| `addresses` | — | **No BRC-38 home.** HD address cache; re-derivable from seed + `current_index`. Backed up today (stripped, backup.rs 402–418). | 3 | Keep in extension for fast restore, or drop and re-derive; either is safe **only if** `current_index` travels. |
| `domain_permissions` | — | **No BRC-38 home.** Note today's backup is already lossy: SELECT (backup.rs 806–807) omits `max_tx_per_session`, `identity_key_disclosure_allowed`, `bundled_scope_grant`. | 3 | Extension section, MAY-ignore on import. Fix the three dropped columns while we're in there, or accept the loss deliberately. |
| `cert_field_permissions` | — | **No BRC-38 home.** Exported keyed by domain string, not FK (backup.rs 822–828) — already portable-form. | 3 | Extension section. |
| `parent_transactions`, `block_headers` | — (BRC-38 §6.5 spirit: caches out) | Cleared to empty arrays in the payload (backup.rs 1015–1016). | 3 | Remove the empty arrays entirely in the new format. |

**Count check:** all **13** BRC-38 tables + `user` + `sourceStorage` have a populated counterpart in our schema. Nothing BRC-38 requires is absent from our database. The README's pre-read guess (five tables with no home; one that matters) is confirmed with one amendment: the tier-1 orphan is not a table but **one field, `wallets.current_index`** (plus, arguably, `output_baskets.token_type`/`protocol_id` at tier 2).

---

## 2. Field-level adapter list

### 2.1 Global transforms (every table)

| # | Transform | Direction | Notes |
|---|---|---|---|
| G1 | snake_case → camelCase per the renames in §1 | both | mechanical, table-driven |
| G2 | Unix-epoch INTEGER seconds → `YYYY-MM-DDTHH:MM:SS.sssZ` UTC string (0038.md §5.1) | both | all `created_at`/`updated_at`, plus `sync_when`→`when`. Ours are second-resolution → emit `.000Z`; on import, truncate. Lossy only in sub-second precision we never had. |
| G3 | SQLite INTEGER 0/1 → JSON boolean | both | `used`, `notified`, `isOutgoing`, `spendable`, `change`, `isDeleted`, `isRedeemed`, `init`, `backed_up`, `confirmed`, `pending_utxo_check` |
| G4 | BLOB → RFC 4648 base64 with padding (§5.2) | both | backup.rs already does this for `locking_script`, `raw_tx`, `input_beef`, `merkle_path` |
| G5 | SQL NULL → omit field entirely (§5.4); never emit `null` | export | serde needs `skip_serializing_if = "Option::is_none"` everywhere |
| G6 | TEXT containing JSON → embedded JSON object (§5.3) | both | `history`, `notify`, `sync_map`, `error_local`, `error_other` |
| G7 | JCS (RFC 8785) canonical serialization + §8 array ordering | export | two re-sorts needed vs today's export order: `outputTagMaps` by (`outputId`,`outputTagId`); `txLabelMaps` by (`transactionId`,`txLabelId`) |

### 2.2 Per-field renames/transforms (beyond case conversion)

| Ours | BRC-38 | Transform |
|---|---|---|
| `users.active_storage` = `'local'` | `user.activeStorage` = storageIdentityKey | substitute `settings.storage_identity_key` on export; reverse on import if it equals our key |
| `proven_txs.tx_index` | `provenTxs[].index` | rename (`index` is a SQL keyword — that's why ours differs) |
| `proven_txs.block_hash`/`merkle_root` = `''` | required `<hex>` | omit when empty + declare stripped profile |
| `proven_tx_reqs.history` `{ts: note}` map | `{"notes":[{"when","what"}]}` (0038.md 277–288) | **shape transform**: each `(ts, note)` → `{"when": iso(ts), "what": note}`; reverse on import |
| `transactions.id` | `transactionId` | rename |
| `transactions.reference_number` TEXT | `reference` `<base64>` | base64(UTF-8) on export; decode on import |
| `transactions.raw_tx` TEXT hex | `rawTx` `<base64>` | hex→bytes→base64 (backup.rs 453–459 already handles both storage forms) |
| `outputs.derivation_prefix`/`_suffix` TEXT | `<base64>` | base64(UTF-8); lossless round-trip |
| `output_tags.id` | `outputTagId` | rename |
| `output_tag_map.id` | — | drop on export; regenerate on import |
| `certificates.type`, `serial_number` TEXT | `<base64>` | pass through if already base64 (verify against a live row — INFERRED) |
| `sync_states.ref_num` | `refNum` | rename |
| `sync_states.sync_when` | `when` | rename + G2 |

### 2.3 Our extra columns (no BRC-38 field — need extension home or explicit drop)

| Table | Columns | Disposition |
|---|---|---|
| `output_baskets` | `description`, `token_type`, `protocol_id` | **keep — tier-2 token metadata** |
| `transactions` | `block_height`, `confirmations` | drop (recompute from provenTx) |
| `transactions` | `failed_at` | moot (failed txs excluded from backup) |
| `transactions` | `price_usd_cents`, `recipient`, `recipient_name` | **keep — tier-2 user records** |
| `outputs` | `confirmed` | keep (drives UTXO-sync logic) |
| `certificates` | `publish_status`, `publish_txid`, `publish_vout` | **keep — tier-2 publication pointers** |
| `settings` | `sender_display_name`, `default_*` ×6 | keep in extension (prefs) or drop |
| `settings` | `backup_hash`, `last_backup_at` | **must NOT travel** (per-device baseline; importing it would suppress the next backup) |

### 2.4 Defaults for BRC-38 fields on IMPORT into our schema

All 13 tables' BRC-38 fields have columns with us, so "38 fields we don't have" is the empty set. What import must synthesize is **our NOT NULL extras**:

| Our column | Default on import of a foreign BRC-38 doc |
|---|---|
| `outputs.confirmed` | `1` if the output's tx has a `provenTxId`, else `0`, then let TaskReviewStatus re-verify against chain |
| `certificates.publish_status` | `'unpublished'` (DDL default) |
| `output_baskets.description`/`token_type`/`protocol_id` | NULL |
| `transactions.block_height`/`confirmations` | from provenTx if present, else NULL/`0` |
| `transactions.recipient`/`recipient_name`/`price_usd_cents` | NULL |
| `wallets.current_index` | **no source in a foreign doc** — must gap-scan `addresses`-equivalent or start high; this is exactly why it belongs in the standard's extension |
| all numeric PKs | remap via AUTOINCREMENT (BRC-38 §10 explicitly permits), maintaining an idMap during the import transaction |

---

## 3. Size of the gap — honest verdict

**This is the "small gap, proceed as planned" branch.** Justification, from the diffs found, not the prior:

1. **Table coverage is complete.** All 13 BRC-38 tables + user + sourceStorage exist in our schema with every BRC-38 field present. Zero missing columns. (VERIFIED, §1.)
2. **Every diff is one of three mechanical kinds:** (a) rename/case, (b) encoding (epoch→ISO, BLOB→base64, TEXT-JSON→object, hex→base64), (c) extra columns. One genuine shape transform exists (`proven_tx_reqs.history` map→notes-array) and one value substitution (`active_storage` `'local'`). No structural, relational, or semantic incompatibility anywhere in the 13 tables.
3. **The extras are few and enumerable:** 16 extra columns across 5 core tables, 4 wallet-local tables, and one surrogate ID. All fit in a declared extension section.
4. **The two real problems are not schema problems:**
   - **`wallets.current_index`** — tier-1 derivation data with no BRC-38 home. One field. Any of the three gate options carries it; it just must be carried *somewhere labelled*.
   - **The stripped profile.** Our on-chain payload deliberately empties `merklePath`, `proven_tx_reqs.rawTx`/`inputBEEF`, spent-output `lockingScript`, caps `history`. A strict reading of §5.4 ("absent in the source dataset" → omit) makes a stripped export *conformant JSON* but semantically thinner than a toolbox export. Not a blocker — a paragraph in our BRC declaring the profile, since every stripped item is re-fetchable from chain.

What would have made it the "big problem" branch — a 13-table set we couldn't populate, type systems that don't round-trip, or tier-1 data structurally unrepresentable — did not materialize. The 24-migration drift turned out to be almost entirely *additive*.

---

## 4. Item-0 decision gate recommendation

**Recommend (a) — small amendment to BRC-38 (contains declaration + preserve-unknown) — with (b) as the shipping posture until the amendment lands.** Argued from the diffs:

- The extras are **column-level and table-level, both small**: 16 columns + 4 tables + 1 singleton field. Option (a)'s preserve-unknown rule is *exactly shaped* for this: unknown members outside the 13 tables MUST be preserved, MAY be ignored. Our entire extension load fits in one vendor-keyed member (`hodos/...`) plus candidates for real BRCs (`token_type`/`protocol_id` alignments, BBS credentials).
- **Nothing we found needs 38's core to change.** No field of the 13 tables needs redefinition; the amendment is purely additive (a `contains` array + one preservation paragraph). That is the cheapest possible ask of the maintainers and the strongest argument it can merge.
- **(c) native-for-now is strictly worse than what we already have**: we are one adapter file away from strict-38 emission for the core; staying native buys nothing and costs interop.
- **Why not pure (b):** an envelope with a sibling extensions object works unilaterally, but any foreign importer discards the sibling — and the sibling holds tier-1 (`current_index`) and tier-2 (token metadata, cert publication) data. The user-facing honesty story in the README ("contains X; this wallet supports Y") requires the `contains` declaration to be *inside* the standard so foreign wallets know to preserve-and-report.
- **Practical sequencing:** our token payload is already a wrapper (JSON → gzip → AES-GCM), so ship `{ "document": <strict BRC-38>, "extensions": { "hodos/wallet": {current_index}, "hodos/baskets-ext": …, … } }` now — that *is* (b) — and submit the two-paragraph amendment; if it merges, `extensions` moves inside the BRC-38 document as `contains`-declared members with **zero data migration** (same members, new location, `formatVersion` still 1 per the amendment's own terms or 2 if maintainers prefer).

One caution the diffs force: the amendment must also bless the **stripped profile** question (empty/omitted re-fetchable fields), or our on-chain documents remain conformant-but-surprising. Fold it into the same PR as a sentence on §5.4.

---

## 5. Foreign BRC-38 import this sprint — realistic?

**Parse + validate + preserve: yes. Full activating import: no — defer activation to next sprint.**

- **Feasible now (the diffs are not the obstacle):** schema-wise we can accept every field of a conformant document today; the adapter of §2 is the same table both directions; §10 explicitly permits primary-ID remapping, which our AUTOINCREMENT import must do anyway. JCS parse, §5 decoding, §9 relationship checks, insert-with-idMap — bounded, testable work.
- **Genuinely hard, and not about field diffs:**
  1. **Identity bootstrap.** A foreign document's `user.identityKey` is not our wallet's key. Our schema tolerates a second user row (`users.identity_key` UNIQUE, not singleton), but every handler assumes one wallet = one user = one mnemonic. Importing *another* identity's outputs raises spendability questions BRC-38 is silent on (root key travels separately, §1 line 45).
  2. **Activation semantics.** Marking imported outputs spendable without chain re-verification is how a wallet shows money it cannot spend. Correctness-beats-cost says: import → hold everything inactive → re-verify UTXOs against chain → activate tier 1. That verification pipeline is real work beyond the adapter.
  3. `wallets.current_index` has no source in a foreign document (§2.4) — gap-scanning logic needed before HD receive addresses are safe to hand out.
- **Recommendation:** this sprint, build the export adapter (needed for item 1 regardless) and an import that *parses, validates, preserves, and reports* — the tier-2 "carry it unopened" behavior. Activation (spend imported tier-1) is its own item with a chain-verification gate.

---

## Disagreements with existing docs (rule 3: quote, cite, state)

1. **ONCHAIN_BACKUP_SYSTEM.md § Included Tables** says: "wallet | Excludes mnemonic (re-entered on recovery). **Includes PIN salt, DPAPI blob**, current_index, backed_up flag". The code disagrees: `backup.rs:375-377` — comment "`// Wallet (no mnemonic, no pin_salt)`", SELECT is `id, current_index, backed_up, created_at, updated_at` only. **PIN salt and DPAPI blob are NOT backed up.** The code is right to exclude them (device-bound secrets); the doc is stale.
2. **0.4.0-beta.4/sprint-4-onchain-backup-sync/README.md item 0** says "we back up 18 tables". Actual count from `BackupPayload` (backup.rs 32–59): **19 SQL tables** serialized (21 members counting the two arrays cleared to empty). Off-by-one from the doc's combined rows; harmless but the item-0 table above is the corrected inventory.
3. **Invoice string, now settled from code** (B1 left this to us): `connection.rs:390/550/716` — `let backup_invoice = "1-wallet-backup-1";` and `handlers.rs:14037` stores `derivation_prefix = "1-wallet-backup"`. So the current-system doc matches the code, and B1's spec finding applies to the **code**, not just the doc: the literal string is security level 1 (BRC draft says level 2) and contains a hyphen inside the protocol ID, which BRC-43 forbids (letters/numbers/spaces only). Conformant BRC-43 tooling would produce `2-wallet backup-1`. Changing it changes the derived backup address — a migration event (old chain at old address must be found or re-anchored). This belongs on the BRC draft's blocker list, not silently fixed.
4. **ONCHAIN_BACKUP_SYSTEM.md** does not mention that the `domain_permissions` backup silently drops `max_tx_per_session`, `identity_key_disclosure_allowed`, `bundled_scope_grant` (backup.rs 806–807 SELECTs only 7 columns; the table has 12 — migrations.rs 468–482 + v17 + v22). Tier 3, but restore-after-recovery loses three permission dimensions users set. Decide: fix or document.

---

## Checked

- `rust-wallet/src/database/migrations.rs` — lines 1–521 (full V1 DDL), 525–600 (v2–v4), 827–900 (v13–v14), 900–1060 (v15–v18), 1060–1120 + 1176–1335 (v19–v24). Every CREATE/ALTER enumerated via grep of the whole file.
- `rust-wallet/src/database/connection.rs` — lines 789–997 (migration runner order), 1000–1074 (startup-repair ALTERs: certificates publish_*, transactions recipient*, settings backup_hash/last_backup_at, domain_permissions max_tx), 312–317, 390–391, 550–551, 716–717 (invoice strings).
- `rust-wallet/src/backup.rs` — lines 25–100 (BackupPayload + structs), 370–545 (collect_payload: wallet/users/addresses/baskets/transactions/outputs), 595–650 (certificates/fields/tags), 720–850 (commissions tail/settings/sync_states/parent_txs/block_headers/domain perms/payload assembly), 970–1070 (compress_for_onchain stripping), grep of all SELECTs.
- `rust-wallet/src/database/migration.rs` — read in full (206 lines; it is the one-time JSON→SQLite import, **not** schema migrations — references legacy tables like `transaction_labels` that no longer exist in V1 DDL).
- `outpoints/0038.md` @ c1d12f2 — §4 (lines 65–113), §5.4 (160–166), §7.1–7.14 (220–515, re-read directly), §8 (517–533). Commit hash verified by `git log -1`.
- `development-docs/ONCHAIN_BACKUP_SYSTEM.md` — lines 1–120 (architecture, included/excluded tables, optimizations).
- `development-docs/0.4.0-beta.4/sprint-4-onchain-backup-sync/README.md` — lines 1–120 (item 0 brief, tier definitions, decision gate).
- B1 digest `research/B1_brc_specs.md` — read in full; every B1 claim I depend on at field level was re-verified against 0038.md directly.

## NOT checked

- `rust-wallet/src/database/models.rs` and the individual `*_repo.rs` files — DDL taken as authoritative for columns; Rust struct types not cross-checked against DDL.
- Restore path (`backup.rs` restore/import functions, ~lines 1340–2284) — export side only; my §5 import assessment is INFERRED from schema + spec, not from reading the current restore code.
- Whether live `certificates.type`/`serial_number` values are actually base64 strings (flagged INFERRED in §1/§2.2).
- BRC-39/40/42/43 files — not re-read here; taken from B1 where cited, and nothing in my deliverables depends on their field level.
- wallet-toolbox's own export code (bears on `history` shape and derivation prefix conventions upstream).
- A1's parallel schema read — not available to me at time of writing; the §1 column lists are my own read for cross-checking against theirs.
