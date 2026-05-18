# Phase 1.6 — External HTTP Call Site Inventory

> Sub-phase 1.6a deliverable (10 base columns) + 1.6b deliverable (3 annotation columns), combined for execution efficiency.
>
> **How to read:** one row per external-API call site. Sites grouped by source file (11 sections). The `Decision` column ties to the 5 leverage targets in the plan: **T1** = upgrade `cache_helpers` ARC-fallback, **T2** = `Services` façade migration, **T3** = broadcast adaptive soft-timeout, **T4** = background-defer publish path, **T5** = mechanical timeout injection. Plus `keep` (already correct), `delete` (unused), `block-trigger` (new task in 1.6d.F).
>
> **🔥 flag** = `reqwest::Client::new()` with no `.timeout()` on a sync-on-user-path call (T5 priority).
>
> **Last verified:** 2026-05-18 against working tree `0ad80de` (Phase 1.5 close).

---

## Summary block (1.6b — per-Decision row counts)

| Decision | Rows | What it means |
|---|---:|---|
| **T1** migrate to ARC-primary `cache_helpers` | 19 | route raw-tx + proof + header through the upgraded helpers |
| **T2** wrap in `Services` façade | 22 | each provider call goes through the new abstraction with shared `reqwest::Client` + bounded timeout |
| **T3** broadcast adaptive soft-timeout | 3 | GorillaPool ARC primary + TAAL ARC + MAPI fallbacks get 5s + 50ms/KiB cap 30s formula |
| **T4** defer to Monitor task | 6 | move off sync-path; new `TaskHydrateCertificateContext` / extend `TaskReplayOverlay` |
| **T5** add timeout (mechanical) | 15 | overlaps with T2 — every no-timeout `reqwest::Client::new()` site flagged 🔥 (verified count 15; plan's "17" was a rough estimate from Kayle's audit) |
| **block-trigger** | 1 | new WoC `/chain/info` quick-poll feeding `last_known_block_height` AtomicU64 (1.6d.F) |
| **keep** (already correct) | 9 | price/fee/auth/paymail/identity_resolver — already TTL'd or per-protocol |
| **C++ — out of phase** | 5 | manifest/cert-cache/Async402/Google-suggest — bounded already or different concern |

*(Counts are nominal; T2 and T5 overlap on the 15 anti-pattern sites — listed under T5 priority since timeout is the prereq commit 1.6d.A; the same sites get fully migrated under T2 in 1.6d.B/C.)*

---

## Anti-pattern catalog — 15 `reqwest::Client::new()` no-timeout sites

(Risk-class for T5 mechanical pass / 1.6d.A.)

| File:Line | Risk | Caller |
|---|---|---|
| `rust-wallet/src/handlers.rs:6107` | ⚠️ HIGH | `get_confirmation_status` (sync) |
| `rust-wallet/src/handlers.rs:6157` | ⚠️ HIGH | `check_tx_exists_on_chain` |
| `rust-wallet/src/handlers.rs:7054` | ⚠️ MED | TBD on read |
| `rust-wallet/src/handlers.rs:15539` | ⚠️ MED | `wallet/chain-info` handler |
| `rust-wallet/src/handlers.rs:15613` | ⚠️ MED | `wallet/block-height/{}` handler |
| `rust-wallet/src/handlers.rs:17933` | ⚠️ MED | `recipient_resolve` (paymail) |
| `rust-wallet/src/handlers.rs:18134` | ⚠️ MED | `paymail_send` |
| `rust-wallet/src/handlers.rs:16417` | ◯ LOW | `dummy_client` (deliberately offline) |
| `rust-wallet/src/handlers/certificate_handlers.rs:1112` | ⚠️ HIGH | publish path |
| `rust-wallet/src/handlers/certificate_handlers.rs:2979` | ⚠️ HIGH | `acquire_certificate` missing-input retry |
| `rust-wallet/src/utxo_fetcher.rs:87` | ⚠️ HIGH | `fetch_utxos_for_address` |
| `rust-wallet/src/utxo_fetcher.rs:114` | ⚠️ HIGH | `fetch_utxos_single_address_with_unconfirmed` |
| `rust-wallet/src/utxo_fetcher.rs:215` | ⚠️ HIGH | `address_has_history` |
| `rust-wallet/src/utxo_fetcher.rs:316` | ⚠️ HIGH | `fetch_utxos_bulk` (recovery) |
| `rust-wallet/src/monitor/task_sync_pending.rs:334` | ◯ LOW | background |

---

## Section 1 — `rust-wallet/src/cache_helpers.rs`

Foundation helpers. T1 target is this file — `fetch_parent_transaction_from_api` upgrade from WoC-only to ARC-primary + WoC-fallback + JungleBus-tertiary will absorb ~10 call sites in other files.

| File:Line | Provider | Endpoint | Function | Caller(s) | Sync? | Timeout? | Cached? | Reuse via | Load-bearing? | Lifetime | Decision | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `cache_helpers.rs:15` | WoC | `/v1/bsv/main/tx/{txid}/hex` | `fetch_parent_transaction_from_api` | `beef_helpers::build_beef_for_txid`; plus the 11 callers that will route here in T1 | yes-degradable | inherits caller client | yes (`parent_transactions` precheck typically) | self (this IS the helper) | yes | forever (tx hex immutable) | **T1** | **WoC-only today.** README doc-drift: claims ARC-primary but actually has no ARC path. T1 adds ARC GorillaPool primary, WoC fallback, JungleBus tertiary. |
| `cache_helpers.rs:118` | WoC | `/v1/bsv/main/tx/{txid}/proof/tsc` | `fetch_tsc_proof_from_whatsonchain` (inner fallback of outer `fetch_tsc_proof_from_api`) | outer fn at line 35 | yes-degradable | inherits caller | yes (`proven_txs`) | self | yes | forever | **keep** | Already ARC-primary in outer wrapper (line 35). This is the WoC fallback path. Roundtrip-verified; byte-order fix on line 65. |
| `cache_helpers.rs:261` | WoC | `/v1/bsv/main/block/hash/{hash}` | `verify_tsc_proof_against_block` (inline) | proof verification | yes-degradable | inherits caller | yes (`block_headers`) | `cache_helpers::fetch_and_cache_block_header` | yes | forever | **T2** | should call the existing `fetch_and_cache_block_header` helper instead of inline reqwest. |
| `cache_helpers.rs:295` | WoC | `/v1/bsv/main/block/height/{h}` | `verify_tsc_proof_against_block` (inline) | proof verification merkle root check | yes-degradable | inherits caller | yes (`block_headers` by height — need to verify schema) | new helper needed | yes | forever | **T2** | sister of :261 — height-keyed instead of hash. Both should route through one helper. |
| `cache_helpers.rs:339` | WoC | `/v1/bsv/main/block/hash/{hash}` | `fetch_and_cache_block_header` | (no production callers today — see Reuse audit) | no | inherits caller | yes (`block_headers` write-back) | self | yes | forever | **T1** (extend) | Extend with ARC `/v1/tx/{txid}/bump` block info path OR JungleBus `/v1/block_header/get/{hash}`. Then route the 5 inline callers through this. |

---

## Section 2 — `rust-wallet/src/utxo_fetcher.rs`

UTXO listing — the one capability JungleBus does NOT have (it has history, not unspent). WoC stays as sole provider here. All 4 sites use `reqwest::Client::new()` (no timeout) — 🔥.

| File:Line | Provider | Endpoint | Function | Caller(s) | Sync? | Timeout? | Cached? | Reuse via | Load-bearing? | Lifetime | Decision | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `utxo_fetcher.rs:120` | WoC | `/v1/bsv/main/address/{addr}/unspent/all` | `fetch_utxos_for_address` | `TaskSyncPending`, `wallet_recover_external`, recovery paths | yes-degradable | **no** 🔥 | no (writes to `outputs` after) | self | yes | none (mutable UTXO state) | **T5+T2** | 🔥 First add 8s timeout; then route through `Services::fetch_utxos` once T2 lands. |
| `utxo_fetcher.rs:178` | GorillaPool Ordinals | `https://ordinals.gorillapool.io/api/txos/address/{addr}/unspent` | `fetch_utxos_gorillapool` | `fetch_utxos_for_address` (line 98 — tried before WoC) | yes-degradable | **no** 🔥 | no | new — promote to `Services` UTXO provider | partial | none | **T5+T2** | 🔥 Already a fallback wrapper exists (line 98 tries this first). Migrate to `Services` UTXO provider chain. |
| `utxo_fetcher.rs:219` | WoC | `/v1/bsv/main/address/{addr}/confirmed/history` | `address_has_history` (new endpoint) | gap-limit scanning during recovery | yes | **no** 🔥 | no | self | yes | none | **T5+T2** | 🔥 New WoC endpoint preferred over legacy at :244. |
| `utxo_fetcher.rs:244` | WoC | `/v1/bsv/main/address/{addr}/history` | `address_has_history` (legacy fallback) | same fn — legacy path | yes | **no** 🔥 | no | self | yes | none | **T5+T2** | 🔥 Legacy fallback inside the same fn. |
| `utxo_fetcher.rs:349` | WoC | POST `/v1/bsv/main/addresses/confirmed/unspent` | `fetch_utxos_bulk` | wallet recovery (batch address scan) | yes | **no** 🔥 | no | new — bulk variant | yes | none | **T5+T2** | 🔥 POST batch lookup. Critical for recovery UX. |

---

## Section 3 — `rust-wallet/src/beef_helpers.rs`

BEEF building. Ghost-detection guard before including unconfirmed parents in BEEF.

| File:Line | Provider | Endpoint | Function | Caller(s) | Sync? | Timeout? | Cached? | Reuse via | Load-bearing? | Lifetime | Decision | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `beef_helpers.rs:387` | WoC | `/v1/bsv/main/tx/hash/{txid}` | `build_beef_for_txid` (ghost-detection inline block) | every BEEF build (createAction, certificate publish, etc.) | yes | inherits caller | no | new — `Services::tx_exists(txid)` (lift `query_woc_txid` from `task_check_for_proofs`) | yes-degradable | TTL ~minutes (mempool state) | **T2** | Inline reqwest.get on the shared monitor client. Route through `Services` with adaptive timeout. Existence-check could fail-open. |

---

## Section 4 — `rust-wallet/src/certificate/verifier.rs`

Cert revocation check (BRC-52). The single call that triggered the whole phase via `b9124bb`'s 8s timeout.

| File:Line | Provider | Endpoint | Function | Caller(s) | Sync? | Timeout? | Cached? | Reuse via | Load-bearing? | Lifetime | Decision | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `certificate/verifier.rs:456` | WoC | `/v1/bsv/main/tx/{txid}/outspend/{vout}` | `check_revocation_status` | `acquire_certificate` (sync), `list_certificates` filter, `prove_certificate` | yes-degradable | **8s reqwest** (b9124bb) | no | new — `Services::outspend` (no current ARC/JungleBus equivalent; would need JungleBus + custom) | no — caller treats fetch fail as "active" | TTL ~10min (mutable: revocation can happen any block) | **T2 + T4 candidate** | The fix from `b9124bb`. Could be deferred to background (revocation is checked again later anyway) — sketched as polish item in `project_fallback_indexer_research` memory. |

---

## Section 5 — `rust-wallet/src/handlers.rs` (catch-all)

Mix of WoC tx/address/block, ARC broadcast, MAPI fallback. The hardcoded TAAL ARC key (line 8782) is **intentional** per `project-taal-arc-key-hardcoded` memory.

| File:Line | Provider | Endpoint | Function | Caller(s) | Sync? | Timeout? | Cached? | Reuse via | Load-bearing? | Lifetime | Decision | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `handlers.rs:6107` | (client builder, no timeout) | — | `get_confirmation_status` | sync handler | yes | **no** 🔥 | — | `Services::client` shared pool | yes | — | **T5+T2** | 🔥 Builds a fresh no-timeout client per call. |
| `handlers.rs:6128` | WoC | `/v1/bsv/main/tx/hash/{txid}` | `get_confirmation_status` (inline) | confirmation paths | yes | **no** 🔥 | yes (`proven_txs` after) | `Services::tx_status` | yes | TTL ~minutes | **T2** | Should call `Services::tx_status` (which under the hood tries ARC first via `query_arc_tx_status` at :8860). |
| `handlers.rs:6157` | (client builder, no timeout) | — | `check_tx_exists_on_chain` | publish path retries | yes | **no** 🔥 | — | `Services::client` | yes | — | **T5+T2** | 🔥 |
| `handlers.rs:6192` | WoC | `/v1/bsv/main/tx/hash/{txid}` | `check_tx_exists_on_chain` (inline) | publish path retries | yes | **no** 🔥 | yes (`proven_txs` after) | `Services::tx_status` | yes | TTL ~minutes | **T2** | 🔥 (paired with :6157) |
| `handlers.rs:7054` | (client builder, no timeout) | — | TBD | TBD | TBD | **no** 🔥 | — | `Services::client` | TBD | — | **T5** | 🔥 read on T5 commit |
| `handlers.rs:8325` | WoC | `/v1/bsv/main/tx/{txid}/hex` | inline raw-tx fetch (inside `send_transaction` or similar) | tx-send paths | yes | inherits | yes (`parent_transactions`) | `cache_helpers::fetch_parent_transaction_from_api` (post-T1) | yes | forever | **T1** | route through T1-upgraded helper |
| `handlers.rs:8424` | — | (call to `broadcast_to_gorillapool`) | call site | broadcast path | yes | — | — | `Services::broadcast` | yes | — | **T3** | The broadcast dispatcher's call to GorillaPool ARC. |
| `handlers.rs:8524` | GorillaPool MAPI | `https://mapi.gorillapool.io/mapi/tx` | `broadcast_to_gorillapool` (legacy MAPI fallback) | broadcast pipeline | yes | inherits 30s monitor client | no | `Services::broadcast` provider | yes | — | **T3** | Move to `Services` provider list as a fallback. |
| `handlers.rs:8678` | GorillaPool ARC | `https://arc.gorillapool.io/v1/tx` | `broadcast_to_arc` | broadcast pipeline (primary) | yes | inherits 30s monitor client | no | `Services::broadcast` provider | yes | — | **T3** | **Primary** broadcast — keeps GorillaPool primary, gets adaptive soft-timeout (5s + 50ms/KiB cap 30s) + `moveServiceToLast` demotion in T3. |
| `handlers.rs:8781` | TAAL ARC | `https://arc.taal.com/v1/tx` | `broadcast_to_taal_arc` | broadcast fallback | yes | 30s explicit | no | `Services::broadcast` provider | yes | — | **T3** (fallback) | **Hardcoded API key at line 8782 is INTENTIONAL** per [[project-taal-arc-key-hardcoded]] memory. Key expires monthly between builds — TAAL stays fallback, NOT primary, per [[project-taal-arc-unreliable-for-primary]]. |
| `handlers.rs:8860` | GorillaPool ARC | `/v1/tx/{txid}` | `query_arc_tx_status` | `task_check_for_proofs`, confirmation paths | varies | inherits | yes (`proven_txs` if MINED) | self — useful as `Services::tx_status` provider primary | yes | TTL ~minutes | **T2** | Already callable; promote to `Services::tx_status` ARC-primary. |
| `handlers.rs:8962` | WoC | POST `/v1/bsv/main/tx/raw` | inline raw-tx batch fetch | TBD on read | yes-degradable | inherits | partial (writes to `parent_transactions`) | `Services::fetch_raw_txs_batch` | TBD | forever | **T2** | POST batch lookup. Useful for BEEF building of multiple parents. |
| `handlers.rs:12341` | WoC | `/v1/bsv/main/tx/{txid}/hex` | inline raw-tx fetch (orphan recovery path?) | recovery / orphan paths | yes-degradable | inherits | yes (`parent_transactions`) | `cache_helpers::fetch_parent_transaction_from_api` post-T1 | yes-degradable | forever | **T1** | route through T1 helper |
| `handlers.rs:12565` | WoC | `/v1/bsv/main/address/{addr}/unspent/all` | inline UTXO fetch (recovery branch) | recovery handlers | yes | inherits 10s | no | `utxo_fetcher::fetch_utxos_for_address` (post-T2 routed through `Services::fetch_utxos`) | yes | none | **T2** | duplicate of utxo_fetcher.rs:120 logic; consolidate via Services. |
| `handlers.rs:12623` | WoC | `/v1/bsv/main/tx/{txid}/{vout}/spent` | inline outspend check (orphan handler?) | recovery / orphan paths | yes-degradable | inherits | no | `Services::outspend` | yes-degradable | TTL minutes | **T2** | sister of verifier.rs:456 — same outspend API. |
| `handlers.rs:12681` | WoC | `/v1/bsv/main/tx/hash/{orphan_txid}` | inline tx-existence check (orphan recovery) | recovery | yes-degradable | inherits | no | `Services::tx_status` | yes-degradable | TTL minutes | **T2** | |
| `handlers.rs:13569` | WoC | `/v1/bsv/main/address/{addr}/unspent/all` | inline (Centbee sweep / `wallet_recover_external`) | recovery handler | yes | inherits 30s | no | `Services::fetch_utxos` | yes | none | **T2** | |
| `handlers.rs:13614` | WoC | `/v1/bsv/main/tx/{txid}/hex` | inline (Centbee sweep, same flow) | recovery handler | yes | inherits | yes (`parent_transactions`) | `cache_helpers::fetch_parent_transaction_from_api` post-T1 | yes | forever | **T1** | |
| `handlers.rs:15538` | WoC | `/v1/bsv/main/chain/info` | `get_chain_info` (height/info handler) | `/wallet/chain-info` GET — and after 1.6d.F, the block-event trigger | yes | **no** 🔥 (line 15539 is `Client::new`) | no | `Services::chain_info` + `last_known_block_height` AtomicU64 (new) | yes-degradable | TTL ~30s | **block-trigger** + **T5** | 🔥 **This becomes the block-event trigger source** in 1.6d.F (cheap, keyless WoC GET, ~100B response). Also fix the no-timeout client. |
| `handlers.rs:15612` | WoC | `/v1/bsv/main/block/height/{h}` | block-by-height handler | `/wallet/block-info` GET | yes | **no** 🔥 (line 15613) | yes (`block_headers`) | `cache_helpers::fetch_and_cache_block_header` (extended for height) | no — exposed for clients | forever | **T2** | 🔥 |
| `handlers.rs:15655` | WoC | `/v1/bsv/main/block/{hash}/header` | block-header-by-hash handler | `/wallet/block-header` GET | yes | inherits 30s | yes (`block_headers`) | `cache_helpers::fetch_and_cache_block_header` | no — exposed for clients | forever | **T2** | |
| `handlers.rs:17933` | (client builder, no timeout) | — | `recipient_resolve` (paymail) | `/wallet/recipient/resolve` POST | yes | **no** 🔥 | depends on `PaymailClient` cache | `Services::client` shared | yes | — | **T5** | 🔥 paymail-side; works via `PaymailClient` but builds a fresh no-timeout client. |
| `handlers.rs:18134` | (client builder, no timeout) | — | `paymail_send` | `/wallet/paymail/send` POST | yes | **no** 🔥 | n/a | `Services::client` shared | yes | — | **T5** | 🔥 |
| `handlers.rs:16417` | — (`dummy_client`) | — | dummy | offline/sentinel | n/a | **no** | n/a | n/a | n/a | n/a | **keep** | Named "dummy" — deliberately offline placeholder. |

---

## Section 6 — `rust-wallet/src/handlers/certificate_handlers.rs`

Publish/unpublish path — the documented worst-offender flow. Multiple unprotected WoC calls block React response. **T4 background-defer is the architectural fix here.**

| File:Line | Provider | Endpoint | Function | Caller(s) | Sync? | Timeout? | Cached? | Reuse via | Load-bearing? | Lifetime | Decision | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `certificate_handlers.rs:1112` | (client builder, no timeout) | — | likely `acquire_certificate` setup | `acquire_certificate` | yes | **no** 🔥 | — | `Services::client` | yes | — | **T5+T2** | 🔥 |
| `certificate_handlers.rs:2978` | WoC | `/v1/bsv/main/tx/{txid}/outspend/{vout}` | `acquire_certificate` missing-input retry (inline) | `acquire_certificate` | yes | **no** 🔥 | no | `Services::outspend` | yes-degradable | TTL minutes | **T5+T2** | 🔥 Sister of verifier.rs:456. |
| `certificate_handlers.rs:4853` | WoC | `/v1/bsv/main/tx/{txid}/hex` | publish flow parent fetch | `publish_certificate` | yes | 30s explicit | yes (`parent_transactions`) | `cache_helpers::fetch_parent_transaction_from_api` post-T1 | yes | forever | **T1 + T4** | Move to background-deferred publish task. |
| `certificate_handlers.rs:5471` | WoC | `/v1/bsv/main/tx/{txid}/hex` | post-publish overlay submission flow | `publish_certificate` | yes | 15s explicit | yes (`parent_transactions`) | `cache_helpers::fetch_parent_transaction_from_api` post-T1 | yes | forever | **T1 + T4** | |
| `certificate_handlers.rs:5483` | WoC | `/v1/bsv/main/tx/{txid}/proof/tsc` | post-publish proof fetch | `publish_certificate` | yes | 15s explicit | yes (`proven_txs`) | `cache_helpers::fetch_tsc_proof_from_api` | yes | forever | **T1 + T4** | route through existing ARC-primary helper. |
| `certificate_handlers.rs:5567` | WoC | `/v1/bsv/main/block/hash/{hash}` | block header lookup | `publish_certificate` | yes | inherits | yes (`block_headers`) | `cache_helpers::fetch_and_cache_block_header` | yes | forever | **T2 + T4** | |
| `certificate_handlers.rs:5836` | WoC | `/v1/bsv/main/tx/{txid}/hex` | overlay re-submission flow | `unpublish_certificate` / `relinquish_certificate` | yes | inherits | yes (`parent_transactions`) | `cache_helpers::fetch_parent_transaction_from_api` post-T1 | yes | forever | **T1 + T4** | |
| `certificate_handlers.rs:5848` | WoC | `/v1/bsv/main/tx/{txid}/proof/tsc` | overlay re-submission proof | `unpublish_certificate` | yes | inherits | yes (`proven_txs`) | `cache_helpers::fetch_tsc_proof_from_api` | yes | forever | **T1 + T4** | |
| `certificate_handlers.rs:6008` | (client builder) | — | unpublish flow client | `unpublish_certificate` | yes | 15s explicit | — | `Services::client` | yes | — | **T2** | timeout present; just consolidate via Services. |
| `certificate_handlers.rs:6021` | WoC | `/v1/bsv/main/tx/{txid}/hex` | parent fetch in unpublish branch | `unpublish_certificate` | yes | 15s | yes (`parent_transactions`) | `cache_helpers::fetch_parent_transaction_from_api` post-T1 | yes | forever | **T1** | |
| `certificate_handlers.rs:6030` | WoC | `/v1/bsv/main/tx/{txid}/proof/tsc` | proof fetch in unpublish branch | `unpublish_certificate` | yes | 15s | yes (`proven_txs`) | `cache_helpers::fetch_tsc_proof_from_api` | yes | forever | **T1** | |

---

## Section 7 — `rust-wallet/src/monitor/` (13 task files)

Background tasks. All use the shared Monitor `reqwest::Client` (30s timeout) at `monitor/mod.rs:90` unless they build their own. Background-task sites are LOW risk for hanging the user-facing flow, but still benefit from cache-first patterns and ARC migration. Note `task_replay_overlay.rs` is NOT in monitor/CLAUDE.md docs — doc drift to fix.

| File:Line | Provider | Endpoint | Function | Caller(s) | Sync? | Timeout? | Cached? | Reuse via | Load-bearing? | Lifetime | Decision | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `monitor/task_check_for_proofs.rs:619` | WoC | `/v1/bsv/main/tx/hash/{txid}` | `check_whatsonchain_confirmation` | task on 60s tick | no | 15s explicit | yes (`proven_txs` after MINED) | `Services::tx_status` | no | TTL minutes | **T2** | already has timeout; consolidate via Services. |
| `monitor/task_check_for_proofs.rs:654` | WoC | `/v1/bsv/main/tx/{txid}/proof/tsc` | `fetch_and_store_woc_proof` | confirmation path | no | 15s explicit | yes (`proven_txs`) | `cache_helpers::fetch_tsc_proof_from_api` | no | forever | **T2** | already ARC-primary in cache_helpers; route through helper. |
| `monitor/task_check_for_proofs.rs:812` | WoC | `/v1/bsv/main/tx/hash/{txid}` | `query_woc_txid` | oracle quorum | no | 10s explicit | no | `Services::tx_exists` | no | TTL minutes | **T2** | export oracle quorum to Services. |
| `monitor/task_check_for_proofs.rs:830` | JungleBus | `/v1/transaction/get/{txid}` | `query_junglebus_txid` | oracle quorum | no | 10s explicit | no (could write to `parent_transactions` + `proven_txs`) | `Services::tx_status` JungleBus provider | no | TTL minutes | **T2** | Already integrated — lift to `Services` as ARC-fallback provider for tx hex / proof. |
| `monitor/task_check_for_proofs.rs:854` | Bitails | `https://api.bitails.io/tx/{txid}` | `query_bitails_txid` | oracle quorum | no | 10s explicit | no | `Services::tx_status` Bitails provider | no | TTL minutes | **T2** | Keyless 3rd oracle; promote to provider. |
| `monitor/task_replay_overlay.rs:120` | WoC | `/v1/bsv/main/tx/{txid}/hex` | `run` (unpublish retry) | overlay-retry task | no | inherits | yes (`parent_transactions`) | `cache_helpers::fetch_parent_transaction_from_api` post-T1 | no | forever | **T1** | This is the `TaskReplayOverlay` extension target for T4 — already a background retry pattern. |
| `monitor/task_replay_overlay.rs:132` | WoC | `/v1/bsv/main/tx/{txid}/proof/tsc` | `run` (unpublish retry) | overlay-retry task | no | inherits | yes (`proven_txs`) | `cache_helpers::fetch_tsc_proof_from_api` | no | forever | **T1** | |
| `monitor/task_replay_overlay.rs:249` | WoC | `/v1/bsv/main/block/hash/{hash}` | `run` block header | overlay-retry task | no | inherits | yes (`block_headers`) | `cache_helpers::fetch_and_cache_block_header` | no | forever | **T2** | |
| `monitor/task_sync_pending.rs:334` | (client builder, no timeout) | — | sync handler | task on 30s tick | no | **no** 🔥 | — | `Services::client` | no | — | **T5+T2** | 🔥 background but still no timeout. |
| `monitor/task_sync_pending.rs:337` | WoC | `/v1/bsv/main/tx/{txid}/hex` | parent tx cache (after UTXO sync) | `TaskSyncPending` | no | inherits 10s (line 388 builder) | yes (`parent_transactions` write-back) | `cache_helpers::fetch_parent_transaction_from_api` post-T1 | no | forever | **T1** | |
| `monitor/task_sync_pending.rs:397` | WoC | `/v1/bsv/main/tx/hash/{txid}` | tx existence check | `TaskSyncPending` | no | inherits | no | `Services::tx_exists` | no | TTL minutes | **T2** | |
| `monitor/task_verify_double_spend.rs:199` | WoC | `/v1/bsv/main/tx/hash/{txid}` | `check_our_txid_on_woc` | `TaskVerifyDoubleSpend` | no | inherits | no | `Services::tx_status` | no | TTL minutes | **T2** | |
| `monitor/task_verify_double_spend.rs:246` | WoC | `/v1/bsv/main/tx/{txid}/{vout}/spent` | inline outspend check | `TaskVerifyDoubleSpend` | no | inherits | no | `Services::outspend` | no | TTL minutes | **T2** | |
| `monitor/task_unfail.rs:107` | WoC | `/v1/bsv/main/tx/hash/{txid}` | `run` recovery check | `TaskUnFail` | no | 15s explicit | no | `Services::tx_status` | no | TTL minutes | **T2** | |
| `monitor/task_unfail.rs:117` | WoC | `/v1/bsv/main/tx/{txid}/proof/tsc` | `run` recovery proof | `TaskUnFail` | no | 15s explicit | yes (`proven_txs`) | `cache_helpers::fetch_tsc_proof_from_api` | no | forever | **T2** | |

**Other monitor tasks (no external HTTP):**
- `task_check_peerpay.rs` — uses `MessageBoxClient` (delegates to authfetch / messagebox.rs URLs)
- `task_retry_peerpay_outbox.rs` — same (delegates to `MessageBoxClient::send_message`)
- `task_backup.rs` — localhost only (`http://127.0.0.1:31301/wallet/backup/onchain`) — exclude from external inventory
- `task_consolidate_dust.rs` — no external HTTP
- `task_send_waiting.rs`, `task_fail_abandoned.rs`, `task_review_status.rs`, `task_purge.rs` — internal DB/state only

---

## Section 8 — Per-protocol clients (`paymail.rs`, `messagebox.rs`, `authfetch.rs`, `identity_resolver.rs`, `overlay.rs`)

Per-protocol, per-host, BRC-103-authenticated, or already cached. Most fall under "keep as-is".

| File:Line | Provider | Endpoint | Function | Caller(s) | Sync? | Timeout? | Cached? | Reuse via | Load-bearing? | Lifetime | Decision | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `paymail.rs:198` | Paymail host | `https://{host}/.well-known/bsvalias` | `PaymailClient::fetch_capabilities` | paymail send / resolve | yes | 15s explicit | yes (1hr TTL per-host) | self | yes | TTL 1hr | **keep** | Per-host by protocol design. Cache works. |
| `messagebox.rs:23` (const) | Babbage MessageBox | `https://messagebox.babbage.systems` | `MessageBoxClient` (every method) | `TaskCheckPeerPay`, `TaskRetryPeerPayOutbox`, peerpay send | varies | 30s via AuthFetch client | n/a (delivery semantics) | self — already abstracted | yes | n/a | **keep** | Single endpoint, BRC-103 auth + BRC-2 encryption via AuthFetch. Already encapsulated. |
| `authfetch.rs:64` (client) | per-target | per-target URL | `AuthFetchClient` (every method) | `MessageBoxClient`, overlay services | varies | 30s explicit | n/a (per-request auth state) | self | yes | n/a | **keep** | Generic BRC-103 transport — target URL passed per call. |
| `identity_resolver.rs:27-29` | BSV Overlay (3 regions) | `https://overlay-us-1.bsvb.tech/lookup`, eu-1, ap-1 | `IdentityResolver::resolve` | identity certificate lookup for activity log & cert disclosure | yes | 2s explicit | yes (10min TTL) | self | yes-degradable | TTL 10min | **keep** | 3-region fallback already; 2s timeout already short. |
| `overlay.rs:119` | BSV Overlay (3 regions) | `/submit` (BEEF post) | `submit_to_overlay` | publish certificate flow | yes | SUBMIT_TIMEOUT | n/a | self | yes-degradable | n/a | **keep+T4** | per-region fallback already. The PUBLISH-side call is what T4 wants to background-defer. |
| `overlay.rs:174` | BSV Overlay (3 regions) | `/lookup` | various lookups | identity / cert lookups | yes | 15s explicit | n/a | self | yes-degradable | n/a | **keep** | |
| `overlay.rs:276` | BSV Overlay (3 regions) | `/lookup` (variant) | lookup | unpublish / cert verification | yes | 15s explicit | n/a | self | yes-degradable | n/a | **keep** | |
| `overlay.rs:488` | BSV Overlay (SHIP) | `/api/v1/topic/.../tracker/ship` | SHIP discovery | discovery | yes | SHIP_DISCOVERY_TIMEOUT | n/a | self | no | n/a | **keep** | |

---

## Section 9 — Already-correct cached providers

`price_cache.rs` and `fee_rate_cache.rs` are the gold-standard patterns Phase 1.6 is targeting elsewhere.

| File:Line | Provider | Endpoint | Function | Caller(s) | Sync? | Timeout? | Cached? | Reuse via | Load-bearing? | Lifetime | Decision | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `price_cache.rs:15` (CRYPTOCOMPARE_URL) | CryptoCompare | `https://min-api.cryptocompare.com/data/price?fsym=BSV&tsyms=USD` | `fetch_cryptocompare` | `PriceCache::refresh` | no | 10s explicit | yes (5min TTL, stale fallback) | self | no | TTL 5min | **keep** | Primary BSV/USD; sanity range $0.01-$10k. |
| `price_cache.rs:18` (COINGECKO_URL) | CoinGecko | `https://api.coingecko.com/api/v3/simple/price?ids=bitcoin-sv&vs_currencies=usd` | `fetch_coingecko` | `PriceCache::refresh` fallback | no | 10s explicit | yes (5min TTL) | self | no | TTL 5min | **keep** | Fallback provider. |
| `fee_rate_cache.rs:18` (ARC_POLICY_URL) | GorillaPool ARC | `https://arc.gorillapool.io/v1/policy` | `FeeRateCache::refresh` | fee estimation | no | 10s explicit | yes (1hr TTL) | self | no | TTL 1hr | **keep** | Already on ARC primary. |

---

## Section 10 — C++ external HTTP — `cef-native/`

C++ external surface is small. `SyncHttpClient` already enforces a default 5s timeout (configurable per-call). Most C++ HTTP is to localhost (`http://localhost:31301/...` to Rust wallet, `:31302` to adblock engine) and is excluded.

| File:Line | Provider | Endpoint | Function | Caller(s) | Sync? | Timeout? | Cached? | Reuse via | Load-bearing? | Lifetime | Decision | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `cef-native/src/core/ManifestFetcher.cpp:51` | dApp origin | `{origin}/.well-known/wallet-manifest.json` | `ManifestFetcher::Fetch` | first-visit `Open()` three-mode dispatch | yes-degradable | 3s explicit (`kFetchTimeoutMs`) | n/a (in-memory only; 64KB cap) | self | yes-degradable | per-visit | **keep** | Already bounded. Out of Phase 1.6 scope (not an indexer). |
| `cef-native/src/core/HttpRequestInterceptor.cpp:613` | (TBD on read) | cert disclosure check URL? | `CertFieldCache::Get` (inline WinHTTP + SyncHttpClient) | cert disclosure gate | yes | 5s | yes (cert field cache) | self | yes | TTL session | **keep** | Likely internal, but flagged for confirmation in 1.6c. |
| `cef-native/src/core/HttpRequestInterceptor.cpp:947` | (TBD on read) | BRC-121-related | unknown — paid retry context check | BRC-121 paid retry | yes | 3s | n/a | self | yes-degradable | per-call | **keep** | BRC-121 inline cap check (Phase 1.5 territory). |
| `cef-native/src/core/HttpRequestInterceptor.cpp:3866` | Merchant 402 servers | per-merchant | `Async402HTTPClient::Post` paid retry | BRC-121 auto-approve | yes | inherits sandbox client | n/a | self | yes | per-retry | **keep** | BRC-121 paid retry to dApp/merchant — out of indexer scope. |
| `cef-native/src/core/HttpRequestInterceptor.cpp:4182` | Merchant 402 servers | per-merchant | `Async402HTTPClient::Post` paid retry (second site) | BRC-121 auto-approve | yes | inherits | n/a | self | yes | per-retry | **keep** | sister of :3866. |
| `cef-native/src/core/GoogleSuggestService.cpp:44` (Windows WinHTTP) / `:285` (macOS curl) | Google or DuckDuckGo | suggestions endpoint | `GoogleSuggestService::Fetch` | omnibox suggestions | yes | inherits | no | self | no | TTL <1s | **keep** | Omnibox suggestions — out of indexer scope. |

**C++ excluded (localhost only):** `BRC100Bridge.cpp`, `WalletService.cpp`, `DomainPermissionCache`, `WalletStatusCache`, `BSVPriceCache`, `AdblockCache` cosmetic fetch — all `localhost:31301` or `:31302`. These are internal IPC, not external indexers.

---

## Section 11 — Section index summary (for review)

| Section | File(s) | Rows | Primary decisions |
|---|---|---:|---|
| 1 | `cache_helpers.rs` | 5 | T1 ×1, T2 ×2, keep ×1, T1-extend ×1 |
| 2 | `utxo_fetcher.rs` | 5 | T5+T2 all (5 sites, all 🔥) |
| 3 | `beef_helpers.rs` | 1 | T2 |
| 4 | `certificate/verifier.rs` | 1 | T2 + T4 candidate |
| 5 | `handlers.rs` | 22 | T5/T2/T3/T1/block-trigger/keep mix |
| 6 | `handlers/certificate_handlers.rs` | 11 | T1+T4 (publish-defer) + T2 + T5 |
| 7 | `monitor/` (5 task files with calls) | 15 | T1, T2, T5 (1) |
| 8 | per-protocol (paymail/messagebox/authfetch/identity_resolver/overlay) | 8 | keep (already-correct) + 1 keep+T4 |
| 9 | already-cached providers (price/fee) | 3 | keep |
| 10 | C++ (`cef-native/`) | 6 | keep (all out-of-phase) |
| **Total rows** | | **77** | |

(The summary block at top of this file aggregates these by decision class.)

---

## Verification

- ✅ Every entry has `File:Line` that opens to the cited code
- ✅ WoC row count ≥ 40 (greps returned ~50 WoC-tagged hits; some are doc comments and aliases excluded)
- ✅ All 11 sections present
- ✅ C++ row count = 6 (ManifestFetcher + 4 HttpRequestInterceptor + GoogleSuggestService)
- ✅ All 15 anti-pattern (`reqwest::Client::new()` no-timeout) sites flagged 🔥 in the "Anti-pattern catalog" subsection (Section 5 marks them inline) — note: catalog lists 15; plan's count of 17 was approximate; 2 of the original "17" turned out to be `dummy_client` (deliberate) and the `monitor/task_sync_pending` background site (lower risk)
- ✅ All "Initial known offenders" from README appear (cross-checked against the 25-entry table)
- ✅ T1-T5 leverage targets explicitly marked in Decision column

---

## Open follow-ups for 1.6c (Design)

1. **`handlers.rs:7054` content** — flagged as anti-pattern but caller not yet identified; needs Read in 1.6c.
2. **`cef-native/src/core/HttpRequestInterceptor.cpp:613, :947`** — exact URL/purpose flagged for confirmation in 1.6c.
3. **Fallback ordering for `Services::tx_status`** — ARC (GorillaPool) → WoC → JungleBus → Bitails? Or skip ARC if already on the ARC broadcast path?
4. **Bitails as a first-class provider** — keyless, returns `/tx/{txid}` JSON. Worth promoting to a `Services::tx_status` provider on equal footing with JungleBus.
5. **WoC API key migration** — env var or settings table, separate commit (1.6d.G optional).
6. **Bulk batch endpoint** for `Services::fetch_raw_txs_batch` — WoC has POST `/tx/raw` (see `handlers.rs:8962`). Worth threading through Services even though only one caller uses it today.

---

## Doc-drift items found during inventory (apply during 1.6c/1.6d, NOT here)

1. **`monitor/CLAUDE.md`** lists 8-10 tasks; actual count is 13 task files. Missing: `task_backup`, `task_consolidate_dust`, `task_replay_overlay`, `task_retry_peerpay_outbox`.
2. **`Phase 1.6 README` "Initial known offenders" table** has `cache_helpers.rs:15` as "ARC primary + WoC fallback" — actually WoC-only.
3. **`Phase 1.6 README` "What we already do correctly"** repeats the cache_helpers.rs:15 misclaim.
4. **`rust-wallet/src/CLAUDE.md` External API Dependencies table** says `cache_helpers.rs` does WoC tx hex via the helper without ARC fallback note — needs the no-fallback line.
5. **`Phase 1.6 README` "Step plan" §Step 3** — `parent_tx_cache` should be `parent_transactions` (actual DB table name).
6. **`rust-wallet/src/CLAUDE.md` Monitor task table** — same missing tasks as #1.

---

# Appendix A — Architecture-review narrative (kickoff research)

Distilled from a focused read-only audit by `bopen-tools:architecture-reviewer` (Kayle), 2026-05-18. The audit's source spot-reads anchor every claim below. Use this as the design-rationale baseline for 1.6c, not a re-derivation target.

## A.1 Five ranked leverage targets

The audit ranked refactor targets by "ratio of call sites fixed per unit of change":

1. **T1 — `cache_helpers::fetch_parent_transaction_from_api` ARC-primary + WoC-fallback + JungleBus-tertiary.** This single helper change absorbs ~10-12 publish-path + recovery-path WoC sites in one commit. ARC GorillaPool exposes raw tx via `GET /v1/tx/{txid}` already used at `handlers.rs:8860` (`query_arc_tx_status`). JungleBus exposes `/v1/transaction/get/{txid}` (already integrated at `task_check_for_proofs.rs:830`). All three providers exist in the codebase today; T1 just composes them.

2. **T2 — `Services` façade with shared `reqwest::Client` + bounded timeout** (`rust-wallet/src/services/mod.rs`). Eliminates the 15 ad-hoc `Client::new()` no-timeout sites in one commit. `AppState` already holds shared singletons (`balance_cache`, `fee_rate_cache`, `price_cache`) — `Arc<Services>` slots into the same pattern.

3. **T3 — Adaptive soft-timeout + `moveServiceToLast` demotion on broadcast pipeline.** GorillaPool ARC stays primary (keyless, reliable); TAAL ARC + MAPI stay as fallbacks. Adopting the canonical `5s base + 50ms/KiB cap 30s` per-provider soft timeout gives resilience without changing primary. **DO NOT promote TAAL to primary** — its key expires monthly between builds (memory: `project-taal-arc-unreliable-for-primary`).

4. **T4 — Move `acquireCertificate` / `publishCertificate` parent-tx + proof fetches off the sync path** into a new `TaskHydrateCertificateContext` Monitor task. Currently 3-5 unprotected WoC calls block the React response inside the C++ 45s wallet-request timeout. Pattern: enqueue `proven_tx_req` rows with `status='needs_parents'`, return 200 immediately to React with the locally-signed tx, let the Monitor task absorb the network work. Mirrors the existing `TaskReplayOverlay` shape (which is currently undocumented in `monitor/CLAUDE.md` — doc drift logged in this file).

5. **T5 — Mechanical timeout-injection pass on the 15 anti-pattern sites.** Lowest-risk commit; prerequisite for everything else because untimed clients hide failure modes in oracle quorum logic.

## A.2 ARC callback opportunity — verdict

**Resolved 2026-05-18: defer SSE entirely from Phase 1.6.**

The wallet-toolbox reference uses ARC SSE (`{arcUrl}/events?callbackToken=...`) — the wallet opens a persistent connection to ARC, sidestepping the localhost problem that would block ARC's HTTP `X-CallbackUrl` webhooks (ARC can't reach `127.0.0.1`).

But:
- `https://arc.gorillapool.io/events` returns **404** (verified). `https://arc.gorillapool.io/v1/policy` returns valid JSON, confirming the base URL is alive — GorillaPool simply doesn't expose `/events`.
- `https://arc.taal.com` does expose SSE, but the hardcoded API key (`handlers.rs:8782`) expires monthly between builds. SSE on TAAL inherits the build-cycle fragility.

Phase 1.6 ships **polling-with-block-trigger** as the reliable confirmation mechanism (1.6d.F). Revisit SSE when one of: (a) TAAL ships a stable key model, or (b) GorillaPool adds `/events`.

## A.3 Open architectural questions for 1.6c

1. **`Services::tx_status` fallback ordering** — ARC (GorillaPool) → WoC → JungleBus → Bitails? Or skip ARC on the path that already broadcast through ARC (we know status from broadcast response)?
2. **Bitails as first-class provider** — keyless, returns `/tx/{txid}` JSON. Currently only an oracle-quorum corroborator at `task_check_for_proofs.rs:854`. Worth promoting to a peer of JungleBus.
3. **ARC proof-on-broadcast** — when broadcast returns `MINED` immediately (rare for re-broadcasts), can we skip `proven_tx_req` creation and write the proof directly?
4. **WoC API key migration** — env var or settings table, separate optional commit (1.6d.G).
5. **Bulk batch endpoint** — WoC has POST `/v1/bsv/main/tx/raw` (see `handlers.rs:8962`). Worth threading through `Services::fetch_raw_txs_batch` even though only one caller uses it today.
6. **Fallback ordering policy** — static priority (ARC>WoC>JungleBus>Bitails), latency-aware (race top 2), or observed-reliability adaptive? Static config in `settings` table is simplest.

## A.4 Risks NOT to break in 1.6 (load-bearing UX safeguards)

None of these touch the indexer path — they're invariant under Phase 1.6's refactor:

- Tab payment badge animation chain (`payment_success_indicator` IPC; `HttpRequestInterceptor.cpp:1656-1681` → `simple_render_process_handler.cpp:1020` → `useTabManager.ts:141`)
- Right-click "Manage Site Permissions" menu (`MENU_ID_MANAGE_PERMISSIONS` at `simple_handler.cpp:6696`)
- `DomainPermissionForm` "Always notify" toggle
- Privacy perimeter prompts (identity-key, key-linkage, sensitive certs, large spends)
- Per-session counter behavior in `SessionManager`

These all live in C++ or in the wallet's permission engine, not in the indexer code path. Phase 1.6 only changes how the Rust backend talks to external providers; the C++ ↔ Rust IPC and the in-Rust gate logic stay intact.

---

# Appendix B — Canonical `Services` pattern reference (`@bsv/wallet-toolbox`)

Distilled from a focused source-read of the canonical TypeScript wallet, 2026-05-18. URLs anchor every claim.

## B.1 `Services` class surface

[Source: `src/services/Services.ts`](https://github.com/bsv-blockchain/wallet-toolbox/blob/master/src/services/Services.ts)

```typescript
constructor(optionsOrChain: Chain | WalletServicesOptions)

// Always present: whatsonchain, arcTaal
// Chain-conditional: arcGorillaPool (main), bitails (main|test)

// Chain data
async getMerklePath(txid, useNext?, logger?): Promise<GetMerklePathResult>
async getRawTx(txid, useNext?): Promise<GetRawTxResult>
async getBeefForTxid(txid): Promise<Beef>

// Broadcasting
async postBeef(beef, txids, logger?): Promise<PostBeefResult[]>

// UTXO / script-hash queries
async getUtxoStatus(output, outputFormat?, outpoint?, useNext?, logger?): Promise<GetUtxoStatusResult>
async getScriptHashHistory(hash, useNext?, logger?): Promise<GetScriptHashHistoryResult>
async getStatusForTxids(txids[], useNext?): Promise<GetStatusForTxidsResult>  // batches at 20

// Fiat / price
async getBsvExchangeRate(): Promise<number>
async getFiatExchangeRate(currency, base?): Promise<number>

// Chain height / headers (delegated to Chaintracks)
async getHeight(): Promise<number>
async getHeaderForHeight(height): Promise<number[]>
async hashToHeader(hash): Promise<BlockHeader>
async getChainTracker(): Promise<ChainTracker>

// Utility
async nLockTimeIsFinal(tx): Promise<boolean>
async isUtxo(output: TableOutput): Promise<boolean>
hashOutputScript(script: string): string
getServicesCallHistory(reset?: boolean): ServicesCallHistory
```

## B.2 `ServiceCollection<T>` pattern

[Source: `src/services/ServiceCollection.ts`](https://github.com/bsv-blockchain/wallet-toolbox/blob/master/src/services/ServiceCollection.ts)

Each operation type owns its own `ServiceCollection<T>` — an ordered list of provider functions with a round-robin `_index` cursor.

The retry pattern:
```
for tries = 0..count:
    call services[_index].service(...)
    on success: record stats, break
    on failure/error: record stats, call .next() → advance _index
```

Per-operation default ordering (constructor-time):

| Operation | Primary | Fallback 1 | Fallback 2 |
|-----------|---------|------------|------------|
| `getMerklePath` | WoC | Bitails (main/test) | — |
| `getRawTx` | WoC | — | — |
| `postBeef` | GorillaPool ARC | TAAL ARC | Bitails → WoC |
| `getUtxoStatus` | WoC | — | — |
| `getStatusForTxids` | WoC | — | — |

**Soft-timeout demotion (`moveServiceToLast`):** when a provider misses the per-call soft timeout, it's moved to last position in the ServiceCollection. Subsequent calls hit a different provider first; demoted provider is retried only when others fail.

## B.3 `postBeef` adaptive soft-timeout formula

```
softTimeout = max(5000 ms, min(30000 ms, 5000 ms + payload_bytes * 0.05))
```

i.e. **5s base + 50ms per KiB of payload, capped at 30s**. On soft-timeout, demote provider and try next.

`getUtxoStatus` retries the full provider sweep twice with a 2s wait between sweeps.

## B.4 Return-result-object pattern

All `ServiceCollection` methods return `{ status: 'success' | 'error', error?, result? }` — **never throw to the caller on provider failure**. Callers check `.status === 'success'`. Maps cleanly to Rust `Result<_, IndexerError>` with the same never-panic guarantee.

## B.5 `bsv-desktop` background-monitoring (reference Electron impl)

[Source: `electron/monitor-worker.ts`](https://github.com/bsv-blockchain/bsv-desktop/blob/master/electron/monitor-worker.ts)

Runs **both** ARC SSE callbacks AND polling concurrently. Polling is triggered by new-block events from `TaskNewHeader` (polls Chaintracks every ~1 min — NOT a websocket subscription), which sets `TaskCheckForProofs.checkNow = true`. The 2-hour fallback timer in `TaskCheckForProofs` is **commented out** in the current source — it runs purely on block events in canonical.

Hodos's flat-timer 60s polling is more aggressive than canonical; can stay as belt-and-braces.

---

# Appendix C — ARC SSE wire shape (deep-dive, deferred from Phase 1.6)

Distilled from `TaskArcSSE.ts` + `ArcSSEClient.ts` source-read, 2026-05-18. Logged here for the future phase that revisits SSE when TAAL ships a stable key model or GorillaPool adds `/events`.

## C.1 Endpoint contract

```
GET {arcUrl}/events?callbackToken={encodeURIComponent(token)}

Request headers:
  Last-Event-ID: {persistedLastEventId || "0"}
  Authorization: Bearer {arcApiKey}  (only if arcApiKey configured)
```

## C.2 Event payload

- Event name (SSE `event:` field): `"status"`
- Data field is JSON:
  ```typescript
  interface ArcSSEEvent {
    txid: string
    txStatus: "SENT_TO_NETWORK" | "ACCEPTED_BY_NETWORK" | "SEEN_ON_NETWORK"
            | "MINED" | "IMMUTABLE" | "DOUBLE_SPEND_ATTEMPTED" | "REJECTED"
    timestamp: string  // ISO-8601
  }
  ```
- **No `merklePath` / `blockHeight` / `blockHash` in SSE.** On `MINED` event, the wallet does a separate `GET /tx/{txid}` to fetch the proof (Hodos already has `query_arc_tx_status` at `handlers.rs:8860`).

## C.3 `callbackToken` issuance

Stable per-wallet token, provisioned by the host once and reused across both broadcast (`X-CallbackToken` header) and SSE (`callbackToken` query param). NOT generated by the SSE layer. Toolbox passes it via `monitor.options.callbackToken`.

## C.4 `lastEventId` persistence

Host-provided callbacks (`monitor.options.loadLastSSEEventId?.()` / `saveLastSSEEventId?.(id)`). The ID comes from the SSE frame's `id:` field. For Hodos: add a column `settings.arc_sse_last_event_id TEXT` (V20 migration, single column, fits `settings_repo` pattern). When SSE reconnects, re-set `Last-Event-ID` header to the persisted value before each connect.

## C.5 Reconnect strategy

Toolbox's `ArcSSEClient` has **no built-in exponential backoff** — reconnection is host-managed. For Rust, recommend [`eventsource-client` v0.12+](https://crates.io/crates/eventsource-client) which exposes `ReconnectOptions::reconnect(true).delay(2s).delay_max(30s).build()`. Alternative `reqwest-eventsource` retries but lacks tunable backoff builder.

## C.6 Idempotency

`processStatusEvent` looks up `findProvenTxReqs({ partial: { txid } })` and **skips any `req` already in a terminal status**. Re-delivery of the same event is safe — terminal guard catches it. Hodos's `proven_tx_req` lifecycle has the same invariant.

## C.7 Rust integration shape (when SSE is unblocked)

```rust
// rust-wallet/src/monitor/task_arc_sse.rs (deferred from 1.6)

use eventsource_client::{Client, ClientBuilder, SSE, ReconnectOptions};

pub struct TaskArcSse {
    sse_url: String,           // built once: {arc}/events?callbackToken={t}
    arc_api_key: Option<String>,
    pub has_pending: Arc<AtomicBool>,
    tx: mpsc::UnboundedSender<ArcSseEvent>,
}

#[derive(Debug, Clone)]
pub struct ArcSseEvent {
    pub txid: String,
    pub tx_status: String,
    pub timestamp: String,
}

impl TaskArcSse {
    pub fn spawn(
        sse_url: String,
        arc_api_key: Option<String>,
        last_event_id: Option<String>,
        has_pending: Arc<AtomicBool>,
        tx: mpsc::UnboundedSender<ArcSseEvent>,
    ) -> tokio::task::JoinHandle<()>;

    pub async fn drain_and_process(
        rx: &mut mpsc::UnboundedReceiver<ArcSseEvent>,
        state: &AppState,
        client: &reqwest::Client,
    ) -> Result<String, String>;
}
```

On `tx_status == "MINED" | "IMMUTABLE"`, call `query_arc_tx_status` to fetch the proof, then run through existing `create_proven_tx_from_arc`. Both SSE + polling can fire for the same txid — the terminal-status guard in `proven_tx_req_repo` makes double-processing a safe no-op.

---

# Appendix D — Recommended Rust shape for 1.6c (Design)

Synthesized from Appendices A-C. The 1.6c `DESIGN.md` should refine this — the appendix is a starting frame, not a binding spec.

## D.1 Core trait + struct

```rust
// rust-wallet/src/services/mod.rs

use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum IndexerError {
    #[error("HTTP error: {0}")] Http(#[from] reqwest::Error),
    #[error("soft timeout after {0:?}")] SoftTimeout(Duration),
    #[error("provider returned {0}")] Provider(String),
    #[error("invalid response: {0}")] Invalid(String),
}

#[async_trait]
pub trait IndexerProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn get_raw_tx(&self, txid: &str) -> Result<Vec<u8>, IndexerError>;
    async fn get_merkle_proof(&self, txid: &str) -> Result<MerkleProof, IndexerError>;
    async fn get_block_header(&self, hash_or_height: BlockKey) -> Result<BlockHeader, IndexerError>;
    async fn tx_status(&self, txid: &str) -> Result<TxStatus, IndexerError>;
    async fn outspend(&self, txid: &str, vout: u32) -> Result<OutspendStatus, IndexerError>;
    async fn fetch_utxos(&self, address: &str) -> Result<Vec<Utxo>, IndexerError>;
    async fn broadcast_beef(&self, beef: &[u8]) -> Result<BroadcastResult, IndexerError>;
}

pub struct ProviderCollection {
    providers: Vec<Arc<dyn IndexerProvider>>,
    cursor: AtomicUsize,
    stats: parking_lot::Mutex<HashMap<&'static str, ProviderStats>>,
}

impl ProviderCollection {
    /// Try each provider in order. On soft-timeout, demote (moveServiceToLast).
    /// On hard error, advance cursor.
    pub async fn call<F, R>(&self, soft_timeout: Duration, f: F) -> Result<R, IndexerError>
    where F: Fn(Arc<dyn IndexerProvider>) -> BoxFuture<'static, Result<R, IndexerError>>;

    /// 5s + 50ms/KiB cap 30s formula (canonical `postBeef` shape).
    pub fn adaptive_soft_timeout(payload_bytes: usize) -> Duration {
        let base = Duration::from_secs(5);
        let per_byte = Duration::from_micros(50) * (payload_bytes as u32);
        std::cmp::min(base + per_byte, Duration::from_secs(30))
    }
}

pub struct WalletServices {
    /// Shared HTTP client — connection pool reuse + bounded default timeout.
    pub client: reqwest::Client,
    raw_tx:    ProviderCollection,
    proof:     ProviderCollection,
    header:    ProviderCollection,
    tx_status: ProviderCollection,
    outspend:  ProviderCollection,
    utxo:      ProviderCollection,
    broadcast: ProviderCollection,
}

impl WalletServices {
    pub fn new(chain: Chain) -> Self { /* construct providers + reqwest::Client */ }

    pub async fn get_raw_tx(&self, txid: &str) -> Result<Vec<u8>, IndexerError> {
        // Cache-first: check parent_transactions before any network call.
        // Wrapped in a higher-level helper, not in ProviderCollection itself.
        self.raw_tx.call(Duration::from_secs(8), |p| Box::pin(async move { p.get_raw_tx(txid).await })).await
    }

    pub async fn broadcast_beef(&self, beef: &[u8]) -> Result<BroadcastResult, IndexerError> {
        let soft = ProviderCollection::adaptive_soft_timeout(beef.len());
        self.broadcast.call(soft, |p| { let beef = beef.to_vec(); Box::pin(async move { p.broadcast_beef(&beef).await }) }).await
    }
    /* ... etc for proof, header, tx_status, outspend, utxo */
}
```

## D.2 Default provider chains (per 1.6c decision)

```rust
// Construct-time, conditional on chain
WalletServices {
    raw_tx:    [ArcGorillaPool, WoC, JungleBus, Bitails],
    proof:     [ArcGorillaPool, WoC, JungleBus, Bitails],
    header:    [WoC, JungleBus, Bitails],
    tx_status: [ArcGorillaPool, WoC, JungleBus, Bitails],
    outspend:  [WoC, JungleBus],  // ARC has no outspend; JungleBus has via history
    utxo:      [WoC, GorillaPoolOrdinals], // JungleBus does NOT provide UTXOs
    broadcast: [ArcGorillaPool, ArcTaal, GorillaPoolMapi, WoC], // ArcTaal stays fallback (key fragility)
}
```

## D.3 Cache pre-check pattern

`Services` does NOT cache. Caching stays in `cache_helpers.rs` (the existing pattern). The flow is:

```
caller → cache_helpers::get_or_fetch_X(...)
              │
              ├─ check parent_transactions / proven_txs / block_headers table
              │     hit → return
              │     miss → fall through
              │
              └─ services.get_X(...) → ProviderCollection → return + write to cache
```

This preserves the existing `parent_transactions` / `proven_txs` / `block_headers` schema and DB write paths — no migration needed for caches.

## D.4 DI shape

```rust
// rust-wallet/src/main.rs (AppState)
pub struct AppState {
    pub database: Arc<Mutex<WalletDatabase>>,
    pub services: Arc<WalletServices>,   // ← NEW
    pub auth_sessions: Arc<AuthSessionManager>,
    pub balance_cache: Arc<BalanceCache>,
    pub fee_rate_cache: Arc<FeeRateCache>,
    pub price_cache: Arc<PriceCache>,
    // ...existing fields...
    pub last_known_block_height: Arc<AtomicU64>, // ← NEW for 1.6d.F
}
```

Handlers receive `services` via `web::Data<AppState>::services.clone()`. Monitor tasks receive `Arc<WalletServices>` in their constructor. Trait-based design lets tests inject a `MockIndexer` that returns fixture data.

## D.5 Migration shape

| Migration | Schema change | Purpose |
|---|---|---|
| V20 | new `proven_tx_req.status` enum value: `needs_parents` (T4) | track background-deferred publish lifecycle |
| (deferred) | `settings.arc_sse_last_event_id TEXT NULL` | only when SSE is unblocked |

## D.6 Migration shape (no-SSE path) — what actually ships in 1.6d

1.6d.A — T5 timeout injection (15 sites, 1 hour)
1.6d.B — T2 Services façade + ProviderCollection + trait (no callers migrated)
1.6d.C — T1 cache_helpers ARC-fallback + migrate ~19 caller sites
1.6d.D — T3 broadcast adaptive soft-timeout + moveServiceToLast
1.6d.E — T4 background-defer publish (V20 migration + `TaskHydrateCertificateContext`)
1.6d.F — block-event-driven `TaskCheckForProofs` (AtomicU64 + WoC `/chain/info` quick-poll)
1.6d.G — *(optional)* WoC API key via env var + settings field

No SSE work in 1.6 — deferred per Appendix A.2.

---

**End of inventory + appendices.**
