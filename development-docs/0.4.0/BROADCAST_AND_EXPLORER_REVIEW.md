# 0.4.0 — Miner Broadcast & Block-Explorer Call Review

**Created:** 2026-06-09 · **Status:** 📋 Scoping (not yet started)
**Owner:** TBD · **Trigger:** HelicOps audit call-out on the hardcoded TAAL ARC key, plus the
broader observation that the BSV broadcast/indexer ecosystem has moved since we wrote these calls.

---

## Why this doc exists

Our outbound chain calls — transaction **broadcast** (miner/ARC) and **block-explorer / indexer**
reads (raw tx, merkle proof, UTXO, headers, price) — were built incrementally and last had a
structural pass in **Phase 1.6d** (the `WalletServices` facade + `ProviderCollection` chains +
`CallClass` timeout matrix). Since then:

- **ARC** (the TAAL/BSV Association transaction-broadcast API) has continued to evolve — endpoint
  shapes, policy responses, callback/SSE semantics, and status codes.
- **Teranode** (the BSV Association's new high-throughput node) is now a real thing in the
  ecosystem and changes assumptions about who we broadcast to and how confirmations/proofs flow.
- Indexer providers (WhatsOnChain, GorillaPool, JungleBus, Bitails) have changed rate limits,
  auth expectations, and in some cases endpoint paths.
- API-key issuance practices are shifting (see §4) — relevant to the hardcoded-key finding.

The HelicOps audit flagged the hardcoded TAAL ARC key as a critical "hardcoded secret." **That was
a fair call-out.** It was a known, intentional decision (documented in memory
`project-taal-arc-key-hardcoded`), TAAL itself recommended the pattern, and there's currently no
env-var alternative on their side — but a live credential nonetheless sits in source/git history.
This review is the right home for revisiting that decision properly rather than band-aiding it.

---

## Current state (verify before relying — code is source of truth)

**Facade:** `rust-wallet/src/services/` — `WalletServices` (facade) → `ProviderCollection`
(per-operation ordered chains) → `providers/*` (per-service impls). Timeout policy is centralized
in `services/call_class.rs` (`IndexerSync` 8s / `IndexerAsync` 15s / `IndexerBulk` 30s /
`ThirdPartyNoFallback` 240s).

**Providers (`rust-wallet/src/services/providers/`):**

| Provider file | Service | Role today |
|---|---|---|
| `arc_gorillapool.rs` | GorillaPool ARC (keyless) | **Primary broadcast** |
| `arc_taal.rs` | TAAL ARC (**hardcoded `mainnet_…` key**, `arc_taal.rs:16`) | Broadcast fallback |
| `whatsonchain.rs` | WhatsOnChain | raw tx / TSC proof / UTXO / headers |
| `gorillapool_mapi.rs` | GorillaPool mAPI | (verify: still used? mAPI is legacy vs ARC) |
| `gorillapool_ordinals.rs` | GorillaPool ordinals | ordinals/token reads |
| `junglebus.rs` | JungleBus | indexer fallback / streaming |
| `bitails.rs` | Bitails | indexer fallback |

**Other call sites of note:**
- `fee_rate_cache.rs` → ARC `/v1/policy` (mining fee rate)
- `cache_helpers.rs` → ARC `/v1/tx/{txid}/bump` (BUMP proof) + WoC raw/proof via Services chain
- `monitor/task_check_for_proofs.rs` → proof acquisition (ARC → WoC)
- `price_cache.rs` → CryptoCompare + CoinGecko (not a chain service, but same "external call" hygiene)

---

## Review questions

### 1. ARC broadcast — are we current?
- [ ] Compare our ARC request/response handling against the **latest ARC API spec** (request
      headers, `X-CallbackUrl`/`X-WaitFor`/`X-MaxTimeout`/`X-SkipFeeValidation` etc.,
      `txStatus` enum values, error body shape). Have any of these changed since we built?
- [ ] Are we handling the full ARC `txStatus` lifecycle (`RECEIVED` → `STORED` → `ANNOUNCED_TO_NETWORK`
      → `SEEN_ON_NETWORK` → `MINED` / `REJECTED` / `DOUBLE_SPEND_ATTEMPTED`) correctly, including
      the newer statuses?
- [ ] **Extended Format (EF) / BEEF** — are we sending the format ARC currently prefers? Confirm
      our broadcast payload matches current ARC ingestion expectations.
- [ ] Fee policy: is `/v1/policy` still the right source, and is our fee-rate cache honoring it?

### 2. GorillaPool-primary / TAAL-fallback ordering — still right?
- [ ] Is keyless GorillaPool ARC still the most reliable primary? (Memory:
      `project-taal-arc-unreliable-for-primary` — TAAL key expires monthly between builds.)
- [ ] Should we add/relegate providers given current uptime/rate-limit reality?
- [ ] Adaptive soft-timeout broadcast (1.6d design) — is it behaving as designed in the field?

### 3. Teranode — what changes for us?
- [ ] What is Teranode's role for a wallet like ours in 2026 — do we broadcast to it, through ARC,
      or unchanged? (We are an SPV wallet; we don't run a node.)
- [ ] Do confirmation / merkle-proof acquisition paths change under Teranode-era infrastructure?
- [ ] Are there new endpoints (Teranode-backed ARC instances, new explorers) worth adding to the chain?

### 4. API-key handling (the TAAL-key finding's proper home)
- [ ] **Decision record:** the TAAL `mainnet_…` key is hardcoded at `arc_taal.rs:16`, intentionally,
      on TAAL's own recommendation, rotated manually at build time. Document this as an explicit,
      signed-off decision (not an accident) — and the residual risk (live key in git history).
- [ ] **Mitigations available now:** build-time injection (env var / CI secret) instead of a source
      literal? Does that meaningfully reduce exposure given the binary ships the key anyway?
- [ ] **Future:** the ecosystem is reportedly developing protocols for issuing API keys / paying
      behind paywalls that a wallet could automate (pay-per-call broadcast, BRC-29-style metered
      access). Track this — it's the real long-term fix (wallet mints/pays for its own broadcast
      credential rather than shipping a shared one). **Future work, not 0.4.0.**
- [ ] Should we rotate the current key as part of 0.4.0 regardless (it's now in an audit report)?

### 5. Block-explorer / indexer reads — best practices
- [ ] Rate-limit & auth posture per provider (WoC API key? GorillaPool? JungleBus?) — are we
      getting throttled in the field?
- [ ] Endpoint freshness: any provider paths we call that have changed/deprecated?
- [ ] Resilience: does the 1.6d 4-tier fallback chain still match each provider's current reliability?

---

## Out of scope
- Re-architecting the `WalletServices` facade (1.6d is recent and sound) — this is a **freshness +
  correctness** pass on the *calls*, not the structure.
- Running our own node / Teranode instance.

## Related
- `rust-wallet/src/services/` (facade + providers + call_class)
- Memory: `project-taal-arc-key-hardcoded`, `project-taal-arc-unreliable-for-primary`,
  `project-fallback-indexer-research`, Phase 1.6 design notes
- HelicOps finding: "Hardcoded secret in source" → `arc_taal.rs:16` (routed CLARIFY in
  `HelicOps/HELICOPS_FEEDBACK.md`, deferred here for the real decision)

---

## Research Findings (2026-06-16, spike `wc9dzecdq` — medium-weight, verifier-checked)

> Compared how **@bsv/sdk (ts-sdk)**, **@bsv/wallet-toolbox**, **Yours Wallet**, and **Dolphin Milk** broadcast. **Bottom line: Hodos is in the right camp** (wallet-toolbox model: GorillaPool-primary → TAAL-fallback, `UntilSuccess`), already more robust than Dolphin. The fixes below are correctness/freshness, not a redesign.

**🕹 "ARCADE" IS REAL (user was right — not a mishearing of ARC).** `github.com/bsv-blockchain/arcade` = a **P2P-first, Teranode-era, ARC-*compatible* broadcaster** (ARC's successor: single-binary+SQLite, listens to gossip, Teranode-only). Drop-in at the *client/API* level. **One real difference: path is `/tx` + `/tx/{txid}` (NOT ARC's `/v1/tx`)** — a provider-config flag, not a redesign. Same `X-Callback*`/`X-Skip*` headers, SSE `/events`, same status enum.

**Teranode impact (node-less SPV wallet):** essentially **none to our model.** We never talk to Teranode directly — we still POST to an ARC/Arcade endpoint a provider operates. Keep the ARC-shaped facade; add Arcade later as a provider variant (just the `/tx` path). Status-mapping logic carries over verbatim.

**Highest-priority Hodos actions (from the synthesis):**
1. **Broadcast success predicate (CVE-2026-40069 bug class):** ALLOWLIST success (`SEEN_ON_NETWORK`, `MINED`), don't denylist only `REJECTED`. Confirm `arc_status.rs` also fails on **`INVALID`, `MALFORMED`, `MINED_IN_STALE_BLOCK`, any ORPHAN** in extraInfo/txStatus, and treats an **unknown status as in-flight (not success).** ← single most important correctness item.
2. **HTTP 200 ≠ mempool acceptance.** On 200, only `DOUBLE_SPEND_ATTEMPTED` + `SEEN_IN_ORPHAN_MEMPOOL` are hard fails (set doubleSpend, capture `competingTxs`, don't retry on other miners). Gate *durable* success on `SEEN_ON_NETWORK`; poll `GET /v1/tx/{txid}` for `RECEIVED/STORED/ANNOUNCED`.
3. **Always submit full BEEF/EF ancestry** — `SEEN_IN_ORPHAN_MEMPOOL` = parent not found; full ancestry avoids it.
4. **Map TAAL 401/403 (expired key) → provider-DOWN → fail over** to GorillaPool, **NOT** a tx rejection. (Critical given the hardcoded key likely expired.)
5. **Rich result taxonomy:** distinguish rejection vs double-spend vs transport/serviceError (wallet-toolbox `{status, doubleSpend?, competingTxs?, serviceError?, notes[]}`).
6. **Keep GorillaPool-primary/TAAL-fallback** (upstream-canonical; GorillaPool `/v1/policy` live-verified keyless). **Add Bitails + WoC as 3rd/4th *broadcast* fallbacks** (`postBeef` only — keep Bitails demoted on reads per the 500-poisoning memory).
7. **Send `XDeployment-ID` header** (`hodos-{hex}`) for ARC traceability; optionally `X-WaitFor=SEEN_ON_NETWORK` + `X-MaxTimeout` (default 5s/max 30s — pull `bitcoin-sv/arc` `arc.yaml` to lock exact semantics).
8. **Trusted-header proof validation:** validate BUMP/TSC merkle roots against a trusted-header layer, **not** the same indexer that served the proof (wallet-toolbox uses a Chaintracks ChainTracker). Use WoC `GetMerkleProofTSC`.
9. **Drop GorillaPool mAPI** (deprecated in favor of ARC). WoC keyless ≈3 req/s; a WoC API key (platform.teranode.group) gives 10/20/40 tiers if read volume grows.
10. **Audit synchronous broadcast** (Dolphin E16: async/delayed broadcast returned local success while tx never propagated) + **concurrency stress-test** the Rust wallet (Dolphin E15: 96/100 createActions rejected at P=8 — UTXO-lock contention; relevant to BRC-121 paid-retry storms).

**TAAL key decision:** **De-hardcode it** → optional injected `Option<String>` defaulting to `None`/keyless. The whole first-party ecosystem (wallet-toolbox, Yours) treats ARC keys as caller-supplied config and runs **both ARCs keyless by default**; ARC only sends `Authorization: Bearer` when a key is set. Keep GorillaPool keyless as primary (safer — TAAL keys expire monthly). **Open:** whether TAAL's *server* accepts keyless in 2026 was NOT confirmed (couldn't fetch docs.taal.com/arc) — do one `docs.taal.com/arc` fetch or a keyless probe of TAAL `/v1/tx` before building any key-rotation tooling; if keyless/metered exists, drop the key entirely.

**Open items / verifier corrections:** ts-sdk double-spend `competingTxs` is on the **failure** path (`BroadcastFailure.more.competingTxs`), not success — capture from failure. Bitails is in `getMerklePath`+`postBeef` only, NOT `getRawTx`. TAAL & GorillaPool auth/rate-limit terms not confirmed from docs bodies (rendered empty). Pull `bitcoin-sv/arc` `arc.yaml` for exact wait-header semantics.

> **NEXT SESSION:** act on items 1–4 first (correctness + the expired-key failover); they're the real "we've had trouble understanding miner responses" fixes. This review's status can move from "Scoping" to "Findings in — ready to implement."
