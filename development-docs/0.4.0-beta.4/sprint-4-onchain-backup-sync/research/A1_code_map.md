# A1 — Backup code map: what the code actually does vs what the docs claim

Date: 2026-08-22. All paths relative to `C:/Users/archb/Hodos-Browser/` unless noted.
Every claim is labelled VERIFIED (read the cited code/data) or INFERRED (reasoned, not directly confirmed).

Re-verification pass 2026-08-22 evening: an independent second read of `backup.rs` (all 2,284 lines), the handlers.rs backup/recovery region, `task_backup.rs`, `migrations.rs`, `pushdrop.rs`, `crypto/pin.rs`, `crypto/brc42.rs` (header + ECDH/HMAC structure), and fresh read-only aggregates from the live DB confirmed every finding below. Corrections from that pass are marked **[v2]**: four column-level data-loss gaps added (§2), live-DB numbers refreshed (§4).

Doc under review: `development-docs/ONCHAIN_BACKUP_SYSTEM.md` (182 lines, read in full).

---

## 1. Function-level map

### 1a. Write path (on-chain backup) — VERIFIED

Entry points:
- HTTP: `POST /wallet/backup/onchain` → `wallet_backup_onchain` (`rust-wallet/src/handlers.rs:13121`, route registered `main.rs:1261`)
- Timer: `monitor/task_backup.rs:41 run()` — checks preconditions (wallet exists, unlocked, balance ≥ 3000 sats, `task_backup.rs:20`) then POSTs to the same local endpoint (`task_backup.rs:73`). Scheduled every 10800 s (3 h) or when the `backup_check_needed` flag is set (`monitor/mod.rs:77, 319-341`). The flag is set by `AppState::request_backup_check_if_significant` (`main.rs:484`) when a spend/receive is ≥ $3.00 USD at the cached price; call sites: `handlers.rs:6395, 10215`, `monitor/task_check_peerpay.rs:440`, `task_consolidate_dust.rs:398`, `task_sync_pending.rs:243,354`. Debounce (3-min quiet window, 10-min hard cap) lives in `main.rs:~460-480`.
- Pre-delete: `wallet delete` handler calls `do_onchain_backup` before deleting (`handlers.rs:2884-2908`); backup failure blocks the delete, "insufficient funds / locked" allows it with a warning.

Core: `do_onchain_backup` (`handlers.rs:13227-14111`):

| Step | What | Where |
|---|---|---|
| 1 | Get master keys. `get_master_private_key_from_db` re-derives from the cached mnemonic: BIP39 seed (empty passphrase) → `XPrv::new(seed)` → root private key bytes | `handlers.rs:13236-13248`; `database/helpers.rs:14-32` |
| 1.5 | c5b sweep: if >2 spendable `1-wallet-backup` outputs (stale phantom pairs), reconcile them | `handlers.rs:13251-13288` |
| 2 | `compress_for_onchain(conn, identity_key, ref_ts)` → gzip bytes; SHA-256 of those bytes compared with `settings.backup_hash`; **skip (Err "skipped…")** if equal. `ref_ts` = `settings.last_backup_at` (or now on first run) so time-tiered strips are stable between runs | `handlers.rs:13290-13324`; `backup.rs:1003-1241` |
| 2b | `encrypt_compressed(master_privkey, compressed)` → AES-256-GCM, output `nonce(12)‖ciphertext‖tag(16)` | `handlers.rs:13326`; `backup.rs:1270-1291` |
| 3 | Derive backup keypair: BRC-42 self-counterparty, invoice `"1-wallet-backup-1"` | `handlers.rs:13332-13344` |
| 4 | PushDrop locking script: `encode(&[encrypted_payload], &backup_pubkey, LockPosition::Before)` → `[pubkey, OP_CHECKSIG, <payload push>, OP_DROP]`. Single field, single output — **no chunking** | `handlers.rs:13347`; `script/pushdrop.rs` (encode fn) |
| 5 | P2PKH marker script at `pubkey_to_address(backup_pubkey)`, 546 sats | `handlers.rs:13354-13359` |
| 5b | Look up previous backup pair in DB: spendable outputs with `derivation_prefix='1-wallet-backup'`, suffix `'1'` (PushDrop) / `'marker'` | `handlers.rs:13361-13403` |
| 5c | Validate against chain: WoC `address/{addr}/unspent/all` on the marker address; if DB txid absent but other markers exist, adopt the highest-height marker (`adopt_onchain_backup`, `handlers.rs:13149`); if none, cross-validated spent-check (`reconcile::check_outpoint_spent`, WoC+GorillaPool, fail-closed) and possibly adopt the spending tx | `handlers.rs:13407-13525` |
| 5d | Orphan-marker sweep: markers at the address whose txid ≠ adopted primary become extra inputs (guards: skip if already spent in DB; skip unconfirmed <10 min old) | `handlers.rs:13527-13598` |
| 6 | Fee estimate; funding UTXO selection excluding the previous backup pair; outputs = PushDrop 1000 sats + marker 546 + change | `handlers.rs:13600-13667` |
| 7 | Reserve inputs: `mark_multiple_spent` with placeholder `pending-backup-{ms}` | `handlers.rs:13674-13690` |
| 8 | Build tx. Input order: [prev PushDrop][prev marker][extra markers][funding]. Output 0 PushDrop, 1 marker, 2 change (reuses highest existing address index — deliberately does not create a new address, to keep the backup hash stable) | `handlers.rs:13695-13755` |
| 9 | Sign: prev PushDrop as P2PK (sig only, backup key); markers as P2PKH (backup key); funding via `derive_key_for_output` | `handlers.rs:13757-13875` |
| 10 | Cache signed tx in `parent_transactions` for BEEF ancestry | `handlers.rs:13889-13894` |
| 11 | Build BEEF (ancestry per input via `beef_helpers::build_beef_for_txid`), broadcast **before** any DB records (crash → only placeholder reservation, cleaned at startup). On failure: suspected-double-spend marking, `rollback_backup`, `reconcile_missing_inputs` | `handlers.rs:13896-13996` |
| 12 | After broadcast: insert `transactions` row (`reference_number = "backup-{txid[..8]}"`, status `unproven`), insert outputs 0/1/2, link, swap placeholder→txid, create `proven_tx_reqs` row | `handlers.rs:13999-14074` |
| 13 | Recompute `compress_for_onchain` with `ref_ts=now`, store as new `backup_hash` + `last_backup_at`; force backup address (index -3) `pending_utxo_check=0` | `handlers.rs:14078-14107` |

`compress_for_onchain` internals (`backup.rs:1003-1241`), in order:
1. `collect_payload` (`backup.rs:372-855`) — reads all tables (see §2), mnemonic parameter passed as `""`.
2. Clears `parent_transactions` and `block_headers` (`backup.rs:1016-1017`).
3. `proven_tx_reqs`: clears `raw_tx`, `input_beef`; caps `history` JSON to last 5 timestamp keys (`backup.rs:1018-1043`).
4. `proven_txs`: clears `merkle_path` (`backup.rs:1044-1046`). (`raw_tx` was already emptied inside `collect_payload`, `backup.rs:559`.)
5. Spent-output strip, 7-day retention, six keep-clauses (`backup.rs:1047-1104`).
6. Dead-address strip, 30-day retention (`backup.rs:1105-1152`).
7. Transaction sliding window, 60-day retention on `completed`; drops orphaned `proven_tx_reqs` (`backup.rs:1154-1204`).
8. Nulls orphan FK references (`transaction_id`, `spent_by`, `basket_id`, `proven_tx_id`); drops orphan `commissions`, `tx_labels_map`, `output_tag_map` (`backup.rs:1206-1237`).
9. `compress_payload`: serde_json → gzip level 9 (`Compression::best()`); warns in log if > 200,000 bytes compressed (`backup.rs:1243-1268`).

### 1b. Read/recovery path — VERIFIED

`POST /wallet/recover/onchain` → `wallet_recover_onchain` (`handlers.rs:14979-15177`, route `main.rs:1263`):
1. Parse mnemonic → seed (empty passphrase) → `XPrv::new` root privkey + pubkey (`handlers.rs:14986-15006`).
2. Reject if a wallet already exists (`handlers.rs:15008-15017`).
3. `fetch_onchain_backup(master_privkey, master_pubkey)` (`handlers.rs:14704-14820`):
   - derive backup pubkey (BRC-42, invoice `"1-wallet-backup-1"`) → P2PKH address;
   - **WoC `address/{addr}/unspent/all`** — unspent outputs only;
   - pick "newest" marker: unconfirmed (height 0) treated as `i64::MAX`, else max height (`handlers.rs:14766-14771`);
   - fetch that txid's raw hex; PushDrop is assumed at **vout 0**; `extract_output_script` → `pushdrop::decode` → `fields[0]`;
   - `deserialize_from_onchain`: AES-GCM decrypt → gunzip → JSON parse (`backup.rs:1297-1333`).
4. No backup found → return without creating a wallet (`handlers.rs:15030-15041`).
5. Create wallet from mnemonic + optional PIN (`create_wallet_from_existing_mnemonic`), then **delete** the auto-created addresses/baskets/users rows and `import_to_db_with_ids(conn, payload, wallet_id, user_id)` (`handlers.rs:15043-15083`; `backup.rs:1352-1650`). Import runs in one SQL transaction, FK order, remapping all user/wallet ids to the fresh ones.
6. `refetch_stripped_data` (`handlers.rs:15183-15330`): for every txid in payload transactions+proven_txs, re-fetch raw hex (fills `transactions.raw_tx`, `proven_txs.raw_tx`, `parent_transactions`), re-fetch TSC merkle proof (fills `proven_txs.merkle_path`), and re-derive missing output `locking_script`s by parsing the fetched raw tx.
7. `reconcile_backup_tx(state, backup_txid, raw_tx)` (`handlers.rs:14508-14691`): the payload is pre-backup state, so the backup tx itself is missing — parse its raw bytes, create its `transactions` row, mark its inputs spent (with `spent_by` FK so `TaskReviewStatus` doesn't undo it), insert PushDrop/marker outputs, insert change output if it matches a known address script, and un-mark token/certificate outputs falsely flagged `external-spend`.
8. Store post-recovery `backup_hash` so TaskBackup doesn't immediately overwrite the on-chain backup with the degraded (stripped) recovered state (`handlers.rs:15111-15136`).
9. Start Monitor, set `recovery_just_completed` (triggers TaskCheckForProofs + TaskValidateUtxos), report balance (`handlers.rs:15139-15176`).

Also present (not on-chain): file-copy backup/restore (`/wallet/backup`, `/wallet/restore` — `handlers.rs:12971, 15342`; `backup.rs:1723, 1778`), password-encrypted file export/import (`/wallet/export`, `/wallet/import` — `handlers.rs:17211, 17297`; `backup.rs:878, 920` — this variant **includes the mnemonic** and uses PBKDF2 via `crypto::pin::derive_key_from_pin`), and mnemonic chain-scan recovery (`/wallet/recover` → `recovery.rs`).

---

## 2. Tables backed up, and doc-claim vs code-reality

### Exact list as implemented — VERIFIED (`backup.rs:372-855`, payload struct `backup.rs:31-59`)

`BackupPayload` contains **21 collections** (plus scalars `version`, `identity_key`, `mnemonic`):

1. wallet (single row) 2. users 3. addresses 4. output_baskets 5. transactions 6. outputs 7. proven_txs 8. proven_tx_reqs 9. certificates 10. certificate_fields 11. output_tags 12. output_tag_map 13. tx_labels 14. tx_labels_map 15. commissions 16. settings 17. sync_states 18. parent_transactions 19. block_headers 20. domain_permissions 21. cert_field_permissions

For the **on-chain** path, `parent_transactions` and `block_headers` are collected then cleared (`backup.rs:1016-1017`), so **19 collections carry data on-chain**. The docs' "~18" is close: the doc's Included-Tables list has 19 table names in 17 rows (it merges the tag/label map tables into their parents' rows) and omits parent_transactions/block_headers (listing them as excluded, which is correct for the on-chain path).

### DB tables NOT backed up at all — VERIFIED (schema: `database/migrations.rs`; live DB confirms)

`transaction_inputs`, `transaction_outputs`, `messages`, `relay_messages`, `monitor_events`, `derived_key_cache`, `peerpay_received`, `peerpay_pending_verification`, `peerpay_outbox`, `domain_protocol_permissions`, `domain_basket_permissions`, `domain_counterparty_permissions`, `permission_audit_log`, `bsv_price_cache`, `domain_manifest_snapshots`, `schema_version` — none appear in `collect_payload`. Notable: **`peerpay_received` (395 rows in the live production DB) and the three V18 BRC-100 permission child tables are silently lost on recovery.** The wallet's secret columns (`wallets.mnemonic`, `pin_salt`, `mnemonic_dpapi`) are excluded by the SELECT list (`backup.rs:376-386`).

### Doc claim vs code reality

| # | Doc claim (ONCHAIN_BACKUP_SYSTEM.md) | Code reality | Verdict |
|---|---|---|---|
| 1 | "wallet … Includes PIN salt, DPAPI blob" (Included Tables) | `collect_payload` selects only `id, current_index, backed_up, created_at, updated_at` (`backup.rs:376-386`). No pin_salt, no DPAPI blob, in either backup variant. PIN/DPAPI are re-created on recovery from the re-entered mnemonic+PIN. | **DIFFERS** (doc wrong; code is right to exclude them) |
| 2 | "Service fee: 1000 sats to Hodos treasury (standard for all wallet transactions)" (Storage Format) | "No Hodos service fee for wallet backups — this is infrastructure protecting the user" (`handlers.rs:13602-13603`). No fee output in the tx build. | **DIFFERS** |
| 3 | "The PushDrop script embeds the encrypted data in the **unlocking** script" | Data is in the **locking** script: `encode(...)` builds the output's locking script (`handlers.rs:13347`, `script/pushdrop.rs`), `tx.add_output(TxOutput::new(backup_output_sats, locking_script_bytes))` (`handlers.rs:13710`). | **DIFFERS** (terminology error in doc) |
| 4 | Optimizations "run in `backup.rs::prepare_backup_payload()`" | No such function exists. They run in `compress_for_onchain` (`backup.rs:1003`). | **DIFFERS** (stale name) |
| 5 | AES key = SHA256(master_privkey ‖ "hodos-wallet-backup-v1") | Exactly implemented (`backup.rs:960-967`). See §3a. | **MATCHES** |
| 6 | Backup address: BRC-42 invoice "1-wallet-backup-1", index -3 | Invoice matches (`handlers.rs:13332, 14711`); address stored in `addresses` with index -3 at wallet creation (`database/connection.rs:389-421`). | **MATCHES** |
| 7 | Output layout: PushDrop 1000 sats (vout 0), marker 546 (vout 1), change (vout 2) | `handlers.rs:13603, 13710-13716, 13603/13360` (`backup_output_sats=1000`, `marker_sats=546`). | **MATCHES** |
| 8 | Gzip before encrypt, "96-97% ratio" | Pipeline order matches (`backup.rs:1003→1243→1270`). Ratio is logged, not asserted; on the live production DB the observed payload is ~431 KB compressed (§4), consistent with high ratio on JSON but unverified as "96-97%". | **MATCHES** (ratio figure UNVERIFIED) |
| 9 | Excluded/stripped: parent_transactions, block_headers, proven_txs.merkle_path, proven_tx_reqs.raw_tx/input_beef, mnemonic | All implemented (`backup.rs:1016-1046`, `collect_payload` line 559 for proven_txs.raw_tx, mnemonic emptied at 1013-1015). | **MATCHES** |
| 10 | Strip rule 1: spent-output 7-day tiered strip, 6 clauses | Implemented with the same 6 clauses (`backup.rs:1047-1104`). Additional detail the doc omits: HD-self means prefix ∈ {"2-receive address","bip32"}; everything else (including NULL prefix) is preserved. | **MATCHES** |
| 11 | Strip rule 2: dead-address 30-day strip | Implemented (`backup.rs:1105-1152`) with same keep-set (index<0, unused, pending, active UTXOs, <30 d). | **MATCHES** |
| 12 | Strip rule 3: 60-day transaction sliding window + orphaned proven_tx_reqs drop | Implemented (`backup.rs:1154-1204`). Doc says "Non-completed transactions (sending, **failed**, etc.) are always kept" — but **failed txs never enter the payload**: `collect_payload` filters `status != 'failed'` at the SQL level (`backup.rs:445-446`). | **DIFFERS** (on "failed always kept") |
| 13 | Strip rule 4: history capped to last 5 entries | Implemented (`backup.rs:1021-1043`). | **MATCHES** |
| 14 | Confirmed transactions' raw_tx stripped ("re-fetchable") — implied by Excluded table | Implemented, more precisely: `transactions.raw_tx` is NULLed only when `proven_tx_id IS NOT NULL` (`backup.rs:449-457`); unconfirmed raw_tx **is** carried (base64) — and dominates real payload size (§4). Spent outputs' `locking_script` also stripped (`backup.rs:497-501`) — doc doesn't list this one. | **CODE ONLY** (locking-script strip undocumented) |
| 15 | Backup txs excluded from payload | `reference_number NOT LIKE 'backup-%'` on transactions (`backup.rs:445-446`); matching NOT EXISTS filters on outputs/proven_txs/proven_tx_reqs/parent_transactions (`backup.rs:494-496, 546-548, 573-575, 800-802`); `derivation_prefix != '1-wallet-backup'` on outputs (`backup.rs:493`). Doc never states this. | **CODE ONLY** |
| 16 | Event trigger "> $3 USD", 3-min debounce, 10-min cap; 3-hour periodic | Code: `usd_value >= 3.0` (`main.rs:490`), 3-min/10-min in `main.rs:~455-480`, 10800 s schedule (`monitor/mod.rs:77`). Note: if the price cache is empty AND stale-empty, the event trigger silently does nothing. | **MATCHES** (≥ vs >, trivial) |
| 17 | Recovery step 8 "Re-derives master address, default basket, DPAPI blob" | Auto-created address/basket/user rows are **deleted** and replaced by payload rows (`handlers.rs:15059-15066`); DPAPI/PIN comes from `create_wallet_from_existing_mnemonic`. Roughly right, mechanism differs. | **MATCHES** (loosely) |
| 18 | Recovery step 9 "Triggers full UTXO sync" | Code sets `recovery_just_completed` → TaskCheckForProofs + TaskValidateUtxos on next Monitor cycle (`handlers.rs:15141-15143`); no full sync is invoked inside the recover handler. | **DIFFERS** (overstated) |
| 19 | Orphan sweep as described | Implemented, plus guards A2/A3 the doc omits (`handlers.rs:13527-13598`). | **MATCHES** |
| 20 | "Zero coins required for recovery" | Recovery path does read-only WoC queries only; no spend needed. | **MATCHES** |
| 21 | Critical invariants 1–2 (PushDrop + BRC-42 counterparty outputs preserved) | Enforced by keep-clauses 2–3 of the spent-output strip (`backup.rs:1076-1083`). | **MATCHES** |
| 22 | Included table `sync_states` "Multi-device sync state" | Collected/imported, but 0 rows in both live DBs; the table is BRC-100 toolbox schema, nothing writes it in this codebase (grep: only backup + repo boilerplate). | **MATCHES** (but dead weight today — INFERRED) |
| 23 | (not in doc) `domain_permissions` import drops columns | Backup struct carries only 5 of the table's columns — `max_tx_per_session`, `identity_key_disclosure_allowed`, **and `bundled_scope_grant` [v2]** (V12/V17/V22 migrations) are **not** backed up; import re-inserts with defaults (`backup.rs:339-357` struct, `backup.rs:1565-1578` import step 15, migrations V12/V17/V22). Recovery silently re-locks sites the user had granted bundled scope or identity disclosure. | **CODE ONLY** (silent data loss on recovery) |
| 24 | (not in doc) **[v2]** `settings` import drops columns | `BackupSetting` (`backup.rs:287-296`) carries only `storage_identity_key, storage_name, chain, dbtype, max_output_script` + timestamps. NOT backed up: `sender_display_name` (V9), `default_per_tx_limit_cents` / `default_per_session_limit_cents` / `default_rate_limit_per_min` (V10), `default_max_tx_per_session` (V12), `default_identity_key_disclosure_allowed` (V19), `default_prefill_from_manifest` (V24). The user's default-limits preferences reset to schema defaults on recovery. (`backup_hash`/`last_backup_at` also excluded — that one is correct by design.) | **CODE ONLY** (silent data loss on recovery) |
| 25 | (not in doc) **[v2]** `transactions` import drops columns | `BackupTransaction` (`backup.rs:110-130`) omits `price_usd_cents` (V11) and `recipient` / `recipient_name` (V13). Recovered history loses the recorded USD price and recipient labels. | **CODE ONLY** (display-data loss on recovery) |
| 26 | (not in doc) **[v2]** `outputs.confirmed` not carried | `BackupOutput` (`backup.rs:132-161`) omits the `confirmed` column (V14, default 1). Import (`backup.rs:1460-1478`) inserts without it → every recovered output lands as `confirmed=1` even if it was unconfirmed at backup time. `idx_outputs_confirmed` consumers (confirmed-preferred UTXO selection, `get_spendable_confirmed_by_user`) then treat unconfirmed coins as confirmed until sync corrects them. | **CODE ONLY** (state flattened on recovery) |

---

## 3. Verification targets

### 3a. Encryption key derivation — code truth: SHA-256 concat, NOT BRC-42 — VERIFIED

`backup.rs:956-967`:

```rust
/// Derive the 32-byte AES key for on-chain backup encryption.
/// Uses SHA-256(master_privkey || "hodos-wallet-backup-v1") — simple and secure
/// since the input is already a 256-bit secret.
fn derive_onchain_backup_key(master_privkey: &[u8]) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(master_privkey);
    hasher.update(b"hodos-wallet-backup-v1");
    hasher.finalize().into()
}
```

- The doc (`ONCHAIN_BACKUP_SYSTEM.md`, Key Design Decisions) matches the code.
- The BRC draft (`Marston Enterprises/Standards/BRCs/drafts/wallet-backup-and-sync-onchain/wallet-backup-and-sync-onchain.md`) claims BRC-42 derivation (security level 2, protocol "wallet backup", counterparty self) for the encryption key. **That is not what ships.** BRC-42 is used only for the backup *address/signing* keypair. The BRC draft must either change to match the code or the code must migrate (a migration changes the decryption of every existing backup — old backups would need the legacy KDF as fallback).
- `master_privkey` here is the BIP32 **root** private key derived as `XPrv::new(BIP39 seed, empty passphrase)` (`database/helpers.rs:14-32`) — the same scalar whose pubkey is the wallet identity key.
- Cipher: AES-256-GCM, fresh random 12-byte nonce per backup, output `nonce‖ciphertext‖tag` (`backup.rs:1270-1291`).

### 3b. Backup address derivation — VERIFIED

`handlers.rs:13331-13344` (write) and `handlers.rs:14710-14719` (recovery):

```rust
let backup_invoice = "1-wallet-backup-1";
let backup_pubkey = crate::crypto::brc42::derive_child_public_key(
    &master_privkey, &master_pubkey, backup_invoice, ...
let backup_privkey = crate::crypto::brc42::derive_child_private_key(
    &master_privkey, &master_pubkey, backup_invoice, ...
```

BRC-42 self-counterparty (own master key on both sides), invoice string `"1-wallet-backup-1"` = level 1, protocol "wallet-backup", key id 1. `crypto/brc42.rs` implements ECDH shared secret → HMAC-SHA256(invoice) → scalar add, per the BRC-42 spec reference in its header. The resulting pubkey is hashed (SHA256→RIPEMD160) to a mainnet P2PKH address. The address is also stored in `addresses` at special index **-3** at wallet creation (`database/connection.rs:389-421`, repair path `connection.rs:668-712`), with `pending_utxo_check=0`, and is excluded from normal address sync (`address_repo.rs:196`, and re-forced to 0 after each backup, `handlers.rs:14101-14105`). Doc claim matches code.

Note the *invoice-format* nuance for the BRC draft: BRC-43 canonical invoice numbers are `<securityLevel>-<protocol>-<keyID>`; the code's string `"1-wallet-backup-1"` parses as level 1, protocol "wallet-backup", key 1 — the current-system doc calls it "1-wallet-backup-1" without decomposition, and the change-address invoice is `"2-receive address-{index}"` (`handlers.rs:13738`). INFERRED: nobody normalizes these against BRC-43; they are opaque strings to both sides of the derivation.

### 3c. Recovery discovery: current-UTXO-only — VERIFIED, with consequences for the delta design

`fetch_onchain_backup` queries **`https://api.whatsonchain.com/v1/bsv/main/address/{addr}/unspent/all`** (`handlers.rs:14734-14737`) — the *unspent* index only, not address history. From the returned UTXOs it takes the newest marker (unconfirmed preferred, then max height, `handlers.rs:14766-14771`) and reads PushDrop **vout 0 of that same txid**.

Consequences:
1. Because each backup tx spends the previous PushDrop+marker (`handlers.rs:13697-13704`), in steady state exactly one marker is unspent → recovery finds only the latest token. **Spent (superseded) backups are invisible to this query.**
2. For the planned delta design (DELTA_ANALYSIS.md / BRC draft), old tokens are spent-but-still-needed. This discovery routine cannot find them. Two code-verified escape hatches exist: (a) WoC has address *history* endpoints (not used anywhere in this repo for the backup address — INFERRED from grep), and (b) a **parent chain walk is already implicit in the tx format**: input 0 of every backup tx is the previous backup's PushDrop outpoint, so given the newest tx's raw bytes you can walk `input[0].txid` backwards fetching raw txs by txid — no address index needed. `reconcile_backup_tx` already parses inputs out of the raw backup tx (`handlers.rs:14516-14532`), so the parsing primitive exists.
3. Known stale-read hazard, acknowledged in code: WoC's unspent index lags 30 s–5 min; during propagation of a new backup the query can return the OLD marker and recovery silently restores stale state (TODO comment `handlers.rs:14730-14733`, referencing the 2026-04-11 incident report). There is no cross-check of the recovered payload's recency (no timestamp comparison, no second indexer).
4. Multiple-marker selection also exists on the write path (Step 5c) with its own copy of the "adopt most recent" logic (`handlers.rs:13453-13521`).

### 3d. Chunking — NOT implemented — VERIFIED

The entire encrypted payload is one PushDrop field in one output: `encode(&[encrypted_payload], ...)` (`handlers.rs:13347`). `script/pushdrop.rs` emits a single OP_PUSHDATA4-capable push per field; nothing splits the payload across fields, outputs, or transactions. The only size control is the log warning at 200,000 compressed bytes (`backup.rs:1261-1263`) — which the live production wallet already exceeds (§4). `settings.max_output_script` (default 500,000, `migrations.rs:341`) is **not** consulted by the backup path (grep: no reads of that column outside settings boilerplate — VERIFIED). Practical ceilings are therefore miner policy for nonstandard outputs and the WoC/ARC submission limits, none of which are checked before broadcast.

### 3e. Dirty-flag hash — VERIFIED

- **What is hashed:** the full gzip output of `compress_for_onchain` — i.e. SHA-256 over the *compressed, stripped, unencrypted* payload bytes (`handlers.rs:13313-13317`). Not the JSON, not the ciphertext (ciphertext would never match: random nonce).
- **Where stored:** `settings.backup_hash` (TEXT, hex) and `settings.last_backup_at` (INTEGER) — columns added by a repair migration in `database/connection.rs:1060-1071` (not in the V1 schema); accessors `settings_repo.rs:314-383`. `set_backup_hash` updates all rows (no WHERE) and inserts a default settings row if none exists.
- **Determinism trick:** the pre-check hashes with `reference_timestamp = last_backup_at`, so records aging past the 7/30/60-day strip thresholds *between* backups do not change the hash (no false-dirty). After a successful backup the hash is recomputed with `ref_ts = now` to capture the tx's own side effects as the new baseline (`handlers.rs:14078-14099`). Same trick post-recovery (`handlers.rs:15111-15136`) to stop TaskBackup from overwriting the on-chain backup with degraded recovered data.
- INFERRED: hash stability rests on serde_json field-order stability (struct order — stable) and SQL `ORDER BY` on each query (present) and gzip determinism for a fixed input/level (flate2 is deterministic for fixed input). If any collect query lost its ORDER BY, backups would re-broadcast on every cycle.

---

## 4. Real size data (replaces the BRC draft's estimates)

Source A — live production wallet DB, read-only aggregates (`C:/Users/archb/AppData/Roaming/HodosBrowser/wallet/wallet.db`, opened `mode=ro`; secret columns never read). VERIFIED:

- DB file: **367,509,504 bytes** (+5.7 MB WAL). Dev DB (`HodosBrowserDev`): 283,373,568 bytes.
- **The DB is 76% parent_transactions cache**: 943 rows, ~280.0 MB of raw hex (largest single cached tx ≈ 991 KB hex). transactions ≈ 30.9 MB (raw_tx blobs), outputs ≈ 23.2 MB, proven_tx_reqs ≈ 14.6 MB (raw_tx), proven_txs ≈ 5.9 MB. Everything the backup actually carries is a small fraction of the file.
- Row counts (production): transactions 350 (**114 of them are `backup-%` rows — 33% of the table is backup lineage**; 8 failed; 342 completed), outputs 1223 (72 spendable / 1151 spent), addresses 312 (298 used, 20 pending), proven_txs 427, proven_tx_reqs 101, certificates 12, certificate_fields 22, peerpay_received 395 (not backed up), sync_states 0, parent_transactions 943, block_headers 59.
- Strip effectiveness, simulated with the code's own rules (ref_ts = now): transactions 228 → **86** kept, outputs 866 → **717** kept, addresses 312 → **63** kept, proven_tx_reqs 65 → **57** kept.
- **Ground truth for current payload size: the live previous-backup PushDrop locking script is 431,363 bytes** (spendable output, `derivation_prefix='1-wallet-backup'`, vout 0) → encrypted payload ≈ 431.3 KB → compressed plaintext ≈ 431.3 KB − 28. This is **2.2× the code's own 200 KB warning threshold**, today, on a real wallet.
- Decomposition of that ~431 KB (measured): structured payload without unconfirmed raw_tx gzips to ≈ **165 KB**; the 3 unconfirmed (`proven_tx_id IS NULL`) non-backup transactions carry 532,846 bytes of raw_tx → ≈ 710 KB base64 → ≈ **291 KB** after gzip. So **~60% of the current on-chain payload is three stuck unconfirmed transactions' raw bytes** (these are certificate/xanaverse-token txs by basket: xanaverse-upvotes 21 spendable outputs / 468 KB of scripts, xanaverse-replies 105 KB, etc.). Spendable outputs' locking scripts total 1,048,326 bytes raw, of which 1-wallet-backup itself is 431 KB (excluded from payload) and `master`-prefix token outputs are 615 KB (included, spendable → scripts kept).
- Dev wallet simulated payload: JSON ≈ 339 KB → gzip ≈ **61 KB** (transactions 234→32, outputs 330→160, addresses 287→77).
- Fee math for the BRC cost table: a backup tx is ≈ payload + ~500 bytes of tx overhead. At the observed 431 KB and 1 sat/KB that's ≈ 432 sats/backup plus the 1546 sats parked in token+marker (recovered next cycle). At the dev wallet's 61 KB: ≈ 62 sats/backup.

**[v2] Refresh (same day, ~19:33 snapshot, read-only aggregates re-run):** wallet.db now 369,942,528 bytes (+2.4 MB since the morning pass); `backup-%` transactions 115 (+1 — another backup cycle ran); **live PushDrop locking script now 431,476 bytes** (was 431,363; still ≈431 KB, still 2.2× the 200 KB warning); parent_transactions 944 rows / 280,851,658 bytes of raw_hex; transactions 351, outputs 1226 (72 spendable), proven_txs 428, proven_tx_reqs 101, addresses 312, certificates 12, peerpay_received 395, sync_states 0. Unconfirmed (`proven_tx_id IS NULL`) non-backup transactions with raw_tx: 11 rows, 685,868 bytes total — the stuck-raw_tx share of the payload is growing, not shrinking. Also confirmed live: `domain_protocol_permissions` has 3 rows and `cert_field_permissions` 3 rows — the former is silently lost on recovery (V18 tables not in payload), the latter is carried. `schema_version` = 23 on this production DB (V24 not yet applied). All numbers VERIFIED against the live DB; no secret columns read at any point.

Source B — repo: no test fixtures or integration tests with real payload sizes exist (`rust-wallet/tests/` contains only JS SDK fixtures; the only size numbers in-repo are the runtime log lines in `compress_payload`). The `test_onchain_round_trip` unit test uses a near-empty payload. VERIFIED (searched).

---

## 5. Fragility inventory

Strip-rules ↔ recovery coupling (the "fragile" the docs mean):

1. **Spent-by-backup outputs must be excluded, or recovered balance inflates** — the outputs query excludes outputs spent by backup txs precisely because import nulls `spent_by` FKs and `TaskReviewStatus` would resurrect them as spendable (comment + SQL, `backup.rs:488-496`). Any new strip rule that nulls `spent_by` has the same trap.
2. **Payload is pre-backup state** — recovery must synthesize the backup tx itself (`reconcile_backup_tx`, `handlers.rs:14508`). If the change-output script doesn't match any address carried in the (address-stripped!) payload, the change output is silently skipped (`change_addr_index >= 0` check, `handlers.rs:14655-14668`) — funds appear only after a later sync. Coupling: address strip keeps "recent" addresses, and the backup change deliberately reuses the max existing index (`handlers.rs:13721-13723`) partly to keep this mapping alive.
3. **Stripped fields must be re-fetchable, and re-fetch is best-effort**: every failure in `refetch_stripped_data` is a warn + counter (`handlers.rs:15219-15320`); recovery reports success regardless. `proven_txs.raw_tx` and `merkle_path` are imported as **empty non-NULL blobs** (schema says `NOT NULL`), and refetch fills them with `LENGTH()=0` guards; the merkle proof is re-stored as **TSC JSON bytes**, while the write path stored the toolbox binary `merkle_path` — two formats in one column, consumers must handle both (INFERRED risk; not traced to consumers).
4. **Recovery trusts vout conventions**: PushDrop at vout 0, marker at vout 1, change at vout 2 are hard-coded on both sides (`handlers.rs:14775, 14604-14630`). Any layout change breaks old-backup recovery.
5. **Discovery races** (3c): stale WoC unspent index can restore a superseded backup; no recency cross-check.
6. **The `1-wallet-backup` derivation prefix is load-bearing** in at least five places: payload exclusion, funding-selection exclusion, send-path reconcile skip (D4), c5b sweep gate, and verify-endpoint filtering (`handlers.rs:14894-14926`).

unwrap/expect/panic in the live paths (all VERIFIED by line):

- `do_onchain_backup` change block: `wallet_repo.get_primary_wallet().map_err(...).unwrap()`, `wallet.unwrap()`, `wallet.id.unwrap()`, BRC-42 derive `.unwrap()`, `p2pkh_locking_script(...).unwrap()` (`handlers.rs:13728-13749`) — a poisoned DB row or derive error panics the request thread mid-backup *after* inputs are reserved (placeholder cleanup then relies on startup sweep).
- Signing: `SecretKey::from_slice(&backup_privkey).unwrap()` ×3 + funding variant (`handlers.rs:13772, 13795, 13816, 13859`); `Message::from_digest_slice(...).unwrap()` each time. BRC-42 output is effectively always a valid scalar, but a zero/overflow scalar would panic, not error.
- `Mutex::lock().unwrap()` throughout (poisoned-lock panic cascade) — dozens of sites in the backup/recovery region.
- Raw tx parsers can panic on truncated input via slice indexing: `raw_tx[pos..pos+8]`/`[pos..pos+36]` in `extract_output_script` (`handlers.rs:14156`), `extract_output_value_and_script` (`handlers.rs:14201`), and `reconcile_backup_tx` (`handlers.rs:14524-14526`) — `decode_varint` errors are handled but the fixed-width reads are unchecked. Input is WoC-fetched hex; a truncated HTTP body panics the recovery handler.
- `wallet_recover_onchain`: `SecretKey::from_slice(&master_privkey).unwrap()` (`handlers.rs:15005`); `refetch_stripped_data` has two `prepare(...).unwrap()` / `query_map(...).unwrap()` (`handlers.rs:15272-15279`).
- `backup.rs:907, 1992`: `SystemTime::duration_since(UNIX_EPOCH).unwrap()` (panics only if clock before 1970).

Error swallowing (`let _ =`):

- **Step 12 post-broadcast DB records: every insert is discarded** — transaction row (`INSERT OR IGNORE`, `handlers.rs:14009`), outputs 0/1/2 (`14033, 14043, 14054`), linking (`14065`), placeholder swap (`14069`), proven_tx_req (`14073`), hash/timestamp store (`14098-14099`). If any of these silently fail after a successful broadcast, the DB diverges from chain and the Step 5c adopt logic is the only healer.
- Input reservation `mark_multiple_spent` result ignored (`handlers.rs:13688`); `rollback_backup` ignores all its restore results (`handlers.rs:14114-14123`); `reconcile_backup_tx` output inserts ignored (`14611, 14622`); post-recovery hash store ignored (`15131`).
- `task_backup.rs` maps any handler error containing "skipped" to a clean Skip — an unrelated error message containing that substring would silently clear the dirty flag path (INFERRED, string-matching fragility, `task_backup.rs:93-99`).
- `settings_repo::get_backup_hash` returns `Ok(None)` on ANY query error (`settings_repo.rs:326`) — a broken settings table reads as "never backed up", forcing a fresh backup rather than surfacing the fault (fail-open, but in the safe direction).

---

## Checked (files/sections actually read)

- `rust-wallet/src/backup.rs` — **all 2,284 lines** (structs 1-360; collect_payload 362-855; encrypt/decrypt file variant 858-951; on-chain serialize/compress/encrypt/decrypt 953-1333; import 1336-1650; F7 path hardening + file backup/restore/verify/export 1652-2040; tests 2040-2284).
- `rust-wallet/src/handlers.rs` — backup/recovery regions in full: 12944-13110 (`wallet_backup` file endpoint), 13112-14211 (`wallet_backup_onchain`, `adopt_onchain_backup`, `do_onchain_backup`, `rollback_backup`, extract helpers), 14508-14957 (`reconcile_backup_tx`, `fetch_onchain_backup`, `wallet_backup_onchain_verify`), 14959-15335 (`wallet_recover_onchain`, `refetch_stripped_data`), 15342-15456 (`wallet_restore`); grep-level coverage of 2848-2980 (pre-delete backup), 6394-6395/10214-10215 (triggers), 17169-17330 (export/import).
- `rust-wallet/src/monitor/task_backup.rs` — all 110 lines. `monitor/mod.rs` lines 29, 57, 77, 158, 174-179, 319-341 (scheduling).
- `rust-wallet/src/main.rs` — 460-495 (`request_backup_check*`), 1260-1275 (routes).
- `rust-wallet/src/database/migrations.rs` — lines 1-700 in full (V1 schema: every CREATE TABLE + indexes), 700-1335 via structured grep of every migration V2-V24 (all ALTER/CREATE statements listed).
- `rust-wallet/src/database/connection.rs` — 380-425 (backup address creation), 1060-1071 (backup_hash/last_backup_at repair migration).
- `rust-wallet/src/database/settings_repo.rs` — 300-400 (hash/timestamp accessors).
- `rust-wallet/src/database/helpers.rs` — 14-50 (master key derivation).
- `rust-wallet/src/crypto/brc42.rs` — header + derivation function structure (1-130 region).
- `rust-wallet/src/script/pushdrop.rs` — `encode` in full + decode structure.
- `development-docs/ONCHAIN_BACKUP_SYSTEM.md` — all 182 lines.
- Live DBs (read-only, aggregates only, secret columns excluded): `AppData/Roaming/HodosBrowser/wallet/wallet.db` and `AppData/Roaming/HodosBrowserDev/wallet/wallet.db`.

## NOT checked

- `crypto/brc42.rs` line-by-line correctness against the BRC-42 spec (structure and HMAC/ECDH shape confirmed; scalar math not re-derived).
- `crate::reconcile::check_outpoint_spent`, `reconcile_missing_inputs`, `reconcile_spent_inputs` internals; `beef.rs` / `beef_helpers.rs`; `broadcast_transaction`; `recovery.rs` (chain-scan path); `derive_key_for_output`.
- `create_wallet_from_existing_mnemonic` internals (connection.rs 500-660 skimmed only around the backup-address block).
- Whether any consumer of `proven_txs.merkle_path` handles the TSC-JSON-vs-binary format split (flagged as INFERRED risk in §5.3).
- The C++/frontend side of triggers and UI; macOS data paths.
- `wallet_recover` (mnemonic chain-scan) and `wallet_recover_external` beyond their signatures.
- The 2026-04-11 incident report referenced by the code comment (file not opened).
- Exact BSV fee rate used at runtime (`fee_rate_cache` not read); cost figures in §4 use 1 sat/KB as the standard assumption.
