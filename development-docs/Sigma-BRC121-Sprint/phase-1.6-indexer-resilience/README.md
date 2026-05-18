# Phase 1.6 — Indexer Resilience & Network Robustness

> Comprehensive audit + refactor of every external API dependency in the wallet, with the goal of reducing WhatsOnChain as a single-point-of-failure, maximizing what we get from ARC, and caching aggressively in our local DB.

**Status:** planning. Not started.
**Sequencing:** runs between Phase 1.5 close (Step 7 demo prep) and Phase 2 (Sigma Auth). Demo prep won't be reliable until this lands.

---

## Why this phase exists

Surfaced 2026-05-15 during SocialCert testing. Empirical evidence:

| What we tried | What happened |
|---|---|
| X verification, no publish | Sometimes succeeded, sometimes 45s timeout on `/acquireCertificate` |
| X verification, with publish | Failed reliably — publish flow makes multiple unprotected WoC calls |

Root cause: **the wallet treats `api.whatsonchain.com` as a hard dependency, with no API key, no timeouts, and no fallback.** When WoC is slow or rate-limiting us, critical flows hang until the C++ 45s wallet-request timeout fires. The cert lands in the DB eventually, but the user-facing flow has already failed by then.

Tier-1 mitigation shipped: `b9124bb` added an 8s timeout to the revocation check during acquireCertificate. That fixed one symptom. **The publish flow has 3+ more unprotected WoC calls and the same architectural fragility.**

The user's instinct (2026-05-18): "we want to reduce our WoC calls and dependencies as much as possible. We want to make sure we can be getting everything we can from ARC and make sure we are storing it in our db and using that." This phase exists to do that systematically rather than as one-off fixes.

---

## Goals

1. **Inventory every external API call** in the wallet (Rust + C++). One source of truth.
2. **Classify each call** by criticality, alternatives, and current caching behavior.
3. **Move what we can to ARC** — ARC is more reliable, has a working API key, and we already use it for broadcast + some merkle proofs.
4. **Cache aggressively** — for any data that doesn't change (tx hex, merkle proofs, block headers), one fetch should cover all future needs.
5. **Bounded timeouts everywhere** so a slow indexer can never hang the wallet.
6. **Fallback indexer chain** for load-bearing data (try ARC → WoC → JungleBus → fail cleanly).
7. **Background-defer where possible** — operations that don't need to block user-facing flow (e.g., publish) should run async with retry, not synchronously.

---

## Step plan

### Step 1 — Inventory (no code, just data)

Grep every external HTTP call site in `rust-wallet/src/**` and `cef-native/src/**`. Produce a table:

| File:Line | Provider | Endpoint | Caller | Synchronous on user path? | Has timeout? | Cached? |
|---|---|---|---|---|---|---|

Initial known offenders (non-exhaustive):

| Where | Endpoint | Currently |
|---|---|---|
| `cache_helpers.rs:15` | WoC `/tx/{txid}/hex` | ARC primary + WoC fallback (per CLAUDE.md) |
| `cache_helpers.rs:118` | WoC `/tx/{txid}/proof/tsc` | ARC primary + WoC fallback |
| `cache_helpers.rs:261, 295, 339` | WoC block hash/height | WoC only |
| `beef_helpers.rs:387` | WoC `/tx/hash/{txid}` | WoC only |
| `certificate/verifier.rs:455` | WoC `/tx/{txid}/outspend/{vout}` | WoC only, 8s timeout ✅ (b9124bb) |
| `certificate_handlers.rs:4853, 5471, 5836, 6021` | WoC `/tx/{txid}/hex` | WoC only (publish path) |
| `certificate_handlers.rs:5483, 5848, 6030` | WoC `/tx/{txid}/proof/tsc` | WoC only |
| `certificate_handlers.rs:5567` | WoC `/block/hash/{hash}` | WoC only |
| `handlers.rs:6128, 6192, 8325, 8962, 12341, 12565` | Various WoC tx/address calls | WoC only |
| `handlers.rs:8524` | GorillaPool MAPI tx broadcast | Backup path |
| `handlers.rs:8782` | TAAL ARC `/v1/tx` (with API key!) | Primary broadcast |
| `monitor/task_verify_double_spend.rs:198` | WoC tx lookup | 500ms delay between calls |
| `utxo_fetcher.rs` | WoC unspent listing | WoC only |
| `price_cache.rs` | CryptoCompare + CoinGecko | Already has fallback ✅ |
| `paymail.rs` | bsvalias hosts | Per-host, by design |
| `messagebox.rs` | MessageBox via AuthFetch | BRC-103 authenticated |
| `identity_resolver.rs` | BSV Overlay Services | Already cached 10min |
| `authfetch.rs` | BRC-103 servers | Per-target |

**Output:** a single `INVENTORY.md` table with every call site. ~1-2 hours.

### Step 2 — Analyze (decide what to do per call)

For each entry in the inventory:

- Is the data **load-bearing** (operation cannot complete without it) or **optional** (we can graceful-degrade)?
- Is the data **immutable** (tx hex never changes, merkle proofs never change) → cache forever
- Is the data **mutable** (UTXO spent state, address balance) → cache with TTL or no cache
- Can ARC provide this? (ARC has: broadcast, BUMP merkle proofs via `/v1/tx/{txid}/bump`, policy/fee rate via `/v1/policy`. ARC does NOT have: arbitrary tx hex lookup, address UTXO listing, outspend status)
- Can JungleBus provide this? (JungleBus has: UTXO state, tx subscriptions, BAP lookups — full BSV indexer)
- Should the call be sync or async? (Publish steps should probably be background-deferred)

**Output:** decision matrix annotated on INVENTORY.md. ~2-3 hours.

### Step 3 — Design

Translate the analysis into a unified architecture:

1. **Timeout standards** — define a `WoCClient`/`IndexerClient` abstraction with per-operation-class default timeouts (lookups 5s, proofs 10s, broadcasts 30s, etc.)
2. **Fallback chain pattern** — `try_indexers!(arc, woc, junglebus)` macro or helper that tries each in order on timeout/failure
3. **Cache extension** — what new tables/fields go into the SQLite DB:
   - `parent_tx_cache` already exists (`parent_transactions` table) — make sure publish path actually uses it
   - Merkle proof cache via `proven_txs` (V16) — make sure publish path uses it
   - Block header cache via `block_headers` table — verify usage
   - Outspend cache? (mutable data — short TTL or rely on fresh fetches)
4. **Background-defer** for publish — break `/publishCertificate` into "mint immediately + queue publish task." Monitor's existing task scheduler can host the retry loop.
5. **API key configuration** — env vars for `WOC_API_KEY` (optional), document setup

**Output:** `DESIGN.md` with the unified architecture. ~3-4 hours.

### Step 4 — Implementation

Apply the design across the codebase. Likely a sequence of small commits:

- A: Introduce `IndexerClient` abstraction + standard timeouts. No call sites changed.
- B: Convert one-call path at a time. Start with the load-bearing publish flow (highest user impact).
- C: Audit cache usage — verify every fetch first checks local DB.
- D: Add background-defer for publish.
- E: Add API key configuration (env var, README docs).

Each commit independently testable. ~6-10 hours total over multiple sessions.

### Step 5 — Testing

Verify behavior under network unavailability:

- Acquire cert with WoC blackholed → succeeds via fallback or graceful-degrade
- Publish cert with WoC blackholed → succeeds via ARC/JungleBus
- BEEF construction with one parent tx unreachable → cached parents used
- Browser starts with WoC slow → no spurious modal prompts (already verified via 87470ac)

**Output:** new integration tests in `rust-wallet/tests/`. ~2-3 hours.

---

## Total scope estimate

**14-22 hours of focused work.** Roughly 2-3 working sessions plus iteration. Should land before Phase 1.5 Step 7 (demo prep) because demos will be brittle without it.

---

## Open questions for next session

1. **Where does ARC fit?** ARC documented endpoints are `/v1/tx` (broadcast), `/v1/policy` (fee rate), `/v1/tx/{txid}/bump` (BUMP merkle proof). Does ARC have an outspend / UTXO-state endpoint? If not, what's the alternative? (JungleBus probably, need to confirm)

2. **JungleBus integration scope** — we have a skill (`bsv-skills:junglebus`) but is it currently used anywhere in Rust code? If yes, where; if no, what's the integration shape?

3. **WoC API key cost/value** — $25/month removes rate-limit throttling. Worth it as part of the resilience strategy, OR redundant if we have proper fallback chain? Probably both — paying ~$25/mo for primary headroom AND having fallbacks for when WoC genuinely fails.

4. **Publish UX** — if we background-defer publish, how does the user see "publishing in progress" / "published" / "failed"? Need a UI for cert state ("local cert" vs "published cert" vs "publish pending").

5. **Idempotency of publish retries** — if a publish task crashes mid-broadcast, does the retry double-spend? Need to check the transaction-lifecycle states.

---

## What's NOT in this phase

- Sigma Auth integration (Phase 2)
- Ordinals support (Phase 3)
- Demo content / videos (Phase 4)
- BRC-100 permission engine work (Phase 1.5 — should be closed first)

---

## What we (probably) already do correctly

The user's "we should be doing most of this correctly already but....." instinct is right — we have partial implementations:

- `cache_helpers.rs::fetch_parent_transaction_from_api` does ARC-primary, WoC-fallback for tx hex
- `cache_helpers.rs::fetch_tsc_proof_from_api` does ARC-primary for proofs
- `parent_transactions` table caches tx hex
- `proven_txs` (V16) caches merkle proofs
- `block_headers` table caches headers
- Price cache has CryptoCompare/CoinGecko fallback
- Identity resolver caches lookups for 10min
- Monitor pattern handles background retry for several operations

**What's NOT consistent:** the publish-side cert code path (in `certificate_handlers.rs`) and several `handlers.rs` call sites go directly to WoC without using the existing ARC-first helpers. Step 2 of this phase will reveal exactly which.

---

## Memory references

- `project_fallback_indexer_research.md` (memory) — earlier framing of this work as Phase 1.5 polish item L; this phase supersedes it
- `project_phase15_session_handoff.md` (memory) — broader sprint context
- Phase 1.5 README — polish item L which becomes the seed of this phase
