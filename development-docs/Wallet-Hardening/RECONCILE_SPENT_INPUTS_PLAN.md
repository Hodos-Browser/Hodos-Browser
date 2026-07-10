# Spent-Input Reconcile — Sprint Plan (generalized)

> **Priority: users must be able to spend their coins.** Ships in a near-term
> `0.3.0-beta.XX` (pure Rust wallet — independent of the 0.4.0 chromium bump).
>
> Generalizes the backup-only [`FIX_A_RECONCILE_PLAN.md`](./FIX_A_RECONCILE_PLAN.md)
> into ONE reconcile primitive that fixes BOTH the regular-send `"Missing inputs"`
> loop AND the backup-token divergence. Companion:
> [`ONCHAIN_BACKUP_REVIEW.md`](./ONCHAIN_BACKUP_REVIEW.md),
> [`FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md`](./FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md).

**Status:** Phase 1 research DONE + owner decisions locked (2026-07-10). Phase 2
design drafted → [`RECONCILE_PHASE2_DESIGN.md`](./RECONCILE_PHASE2_DESIGN.md)
(adversarial review pending; no code until it lands + owner approves).
**Branch:** commit on `0.4.0` (solo dev; easier to track).
**Adversarial review is mandatory at every phase** (design AND implementation).

> **Backup-anchor clarification (verified 2026-07-10, `connection.rs:389-420`):**
> the `-3 → backup` mapping is **correct and load-bearing** — the backup address is
> BRC-42-self-derived from invoice `"1-wallet-backup-1"` and stored in `addresses`
> at index `-3` (the deterministic anchor mnemonic-only recovery uses). The backup
> *outputs* carry `derivation_prefix="1-wallet-backup"`/`suffix="1"|"marker"`, which
> reconstructs the same invoice → one identical key, two table views. The reconcile
> must **skip** the −3 anchor / `"1-wallet-backup"` outputs when recovering *change*.

---

## 0. The problem (both symptoms, one root)

A user on **beta.26** hit `Transaction broadcast failed: … 500: Missing inputs` on a
normal send. Same error class as the backup-token loop. Root: **the DB tracks a
UTXO as spendable that is actually already spent on-chain (a "phantom")**, so every
send/backup re-selects it and the node rejects the tx. Today the wallet **auto-restores
the input** on this error (an old "safe default" that relied on `TaskValidateUtxos` —
**which was removed 2026-04-20**), so there is **no self-heal**: the user loops forever
and cannot spend. Their coins are safe (on-chain, seed-recoverable) but the DB must be
reconciled for the balance to be correct and spendable.

### What "500: Missing inputs" means (verified against our `arc_status.rs`)
Node-level `bad-txns-inputs-missingorspent`: for ≥1 input, the outpoint is neither a
live UTXO nor reachable via a known parent. Sub-causes:
- **(a) already spent** (phantom in our DB) — the likely case here.
- **(b) parent tx unknown to the node** (never confirmed / not in mempool).
- (c) never existed (unlikely from our own builder).

**It is NOT a missing-merkle-proof / incomplete-BEEF problem.** The error came from a
node's UTXO-set check (relayed by WhatsOnChain's fallback broadcast relay), which does
not consult our BEEF proofs. A genuine BEEF-proof failure surfaces as ARC's
`SEEN_IN_ORPHAN_MEMPOOL` (`arc_status.rs:94`, classified separately). WoC is primarily
an explorer with a broadcast *relay*; the authoritative structured miner signal is
**ARC** (GorillaPool/TAAL), tried first — the user's full logs will show ARC's line,
which is the one to trust. Our own code already flags "Missing inputs" as ambiguous
(`arc_status.rs:219`).

### Corrected behavior (do NOT auto-restore, do NOT auto-mark-spent)
On "Missing inputs": **check on-chain and act only on a positive, authoritative signal.**
`/spend`=200 with a spending txid → definitely spent. 404-parent → definitely
unconfirmed. **Fail closed** on any network error / lag / ambiguity (this is exactly the
fail-OPEN mistake that broke `TaskValidateUtxos`).

---

## 1. The shared primitive

`reconcile_spent_inputs(candidate_outpoints) -> { marked_spent, change_recovered }`

```
for each (txid, vout) in candidates:
  1. Authoritative on-chain check:
       /txo/{txid}/{vout}/spend  AND  does parent tx exist?
       - 404 parent / unconfirmed → input invalid; stop tracking (fail-closed if unsure)
       - spent by T (200)          → proceed
       - network error / ambiguous → DO NOTHING (fail closed)
  2. Require T CONFIRMED (reorg safety; ≥1 conf, prefer 2–3).
  3. For EVERY output of T (not just last-vout change):
       recover a SIGNABLE derivation for its script:
         a. exact match vs cached `addresses`      (instant; change often already cached)
         b. else BOUNDED gap-scan self-derivation   (see §2)
         c. counterparty-derived? (rare for change) — only if key recoverable
       - signable params recovered + output unspent → insert spendable, CORRECT derivation
       - NOT signable                                → do NOT insert (never NULL/master/guess)
  4. Mark the phantom (txid:vout) spent (spent_by = T) — steps 3+4 in ONE lock scope.
  5. Invalidate balance cache once.
```

**Ownership model (why we scan all outputs):** normal send → we own the change only;
send-to-self (owner does this for testing) → we own multiple/all; backup-token tx → we
own all. The reconcile asks "which outputs are ours (by derivation) and still unspent?"
— covering all three without assuming which vout is change.

**Money-safety framing for users:** funds are never lost (they're on-chain at T's
wallet-owned output, seed-recoverable); the reconcile makes the DB *reflect* them so the
balance is correct and spendable. The recovered amount can be **large** (if T was the
user's own unrecorded send, the change may be most of their balance).

---

## 2. Bounded gap-scan (derivation recovery) — the load-bearing detail

Goal: given an on-chain output's `hash160`, recover the derivation index so we can sign.

- **Do NOT scan from index 0.** Anchor at the wallet's known `current_index` (structural
  bound). A wallet-made change output sits **at or just above** `current_index`.
- **Cache-first:** address derivation and output tracking are separate tables — the
  change *address* is often already cached even when the *output* isn't → exact-script
  match against `addresses` resolves most cases instantly.
- **Bounded window only if uncached:** scan `[current_index − small_back, current_index +
  gap_limit]` with a BIP44-style `gap_limit` (~20, config up to ~50). NOT `[0,
  current_index]`.
- The block **timestamp does not bound the index** (no time→index map) — `current_index`
  is the correct anchor.
- **Signable-or-skip:** if no match in the window, treat as not-a-recoverable-self-address
  → do NOT insert. Never insert with NULL/NULL (→ master key), `"master"`, or a guessed
  index — that is the `mandatory-script-verify-flag-failed` wrong-key poison.
- Match `@bsv/wallet-toolbox`'s recovery gap-scan (gap limit + derivation scheme) rather
  than inventing our own — see Phase 1.

---

## 3. Triggers
1. **Reactive — regular send** hits "Missing inputs" → reconcile the selected inputs, then
   re-select once and retry (bounded). **Critical path.**
2. **Reactive — backup** divergence / "Missing inputs" (subsumes Fix A).
3. **Proactive — startup + Monitor self-healer** → heals already-broken wallets without
   waiting for a failed send. **Higher risk (runs on healthy wallets too) — same
   fail-closed guardrails, ship AFTER the reactive path is proven.**

---

## 4. Phase 1 — RESEARCH FIRST (parallel agents, before any design/code)
- **BRC-42 derivation in OUR code (linchpin):** exactly how Hodos derives self-receive,
  change, and counterparty addresses; whether change is ALWAYS self-derived (so gap-scan
  suffices); how `derive_key_for_output` maps params→key; confirm the index scheme.
- **Reference impls:** how `@bsv/sdk` and `@bsv/wallet-toolbox` handle UTXO validation,
  spent-input detection, change/output recovery, gap-limit scanning, BRC-42 derivation.
  Align to the standard.
- **Miner + explorer response semantics (verify, don't assume):** what ARC / ARCADE / WoC
  / BananaBlocks each return for spent / missing / orphan-mempool / unconfirmed-parent.
- **BSV ARCADE** (`github.com/bsv-blockchain/arcade`): what the endpoints are, which
  miners run ARC, whether to add them as broadcast providers (or even a PRIMARY).
  **Concrete v2 endpoints to evaluate (owner-provided 2026-07-10):**
  `https://arcade-v2-us-1.bsvblockchain.tech/` (US) and
  `https://arcade-v2-eu-1.bsvblockchain.tech/` (EU) — probe their API surface (ARC
  `/v1/tx`, `/v1/policy`, status), auth requirements, which miners back them, and
  latency/region so we can slot them into `services::WalletServices` broadcast + proof
  chains (redundancy, or primary with geo-failover US↔EU).
- **BananaBlocks** (keyless): evaluate `/tx/<txid>/beef` (ready-made BEEF — attacks the
  BEEF-fetch fragility), `/txo/<txid>/<vout>/spend`, `/tx/broadcast`, `/address/utxos` as
  fallback providers in our `services::WalletServices` / `IndexerProvider` chain.

## 5. Phase 2 — Design (with research in hand)
The `reconcile_spent_inputs` primitive; reactive-vs-proactive sequencing; pre-flight
(validate UTXOs before broadcast) vs post-failure; provider-redundancy additions.

## 6. Phase 3 — Adversarial red-team, then phased build
Red-team the design (fail-closed, gap-scan bounds, reorg safety, no-unsignable-inserts,
provider-response edge cases) before it becomes a build plan. Then small reversible
commits with unit + integration + a real-diverged-wallet fixture (the beta.26 user's
case, once we have his logs).

---

## 7. Guardrails (carried from prior red-teams — BLOCKING)
1. **Fail closed** on every ambiguous chain read; positive `/spend`=200 or 404-parent only.
2. **Never infer spent from absence** in a bulk/address query (the `TaskValidateUtxos` bug).
3. **Confirmation-depth gate** before marking/inserting (reorg safety).
4. **Signable-or-skip** derivation recovery; exact-script match / bounded gap-scan; never
   NULL/master/guessed.
5. **Scan all outputs** of the spending tx, not just assumed change.
6. **Atomic** insert-recovered + mark-phantom-spent in one lock scope; single cache invalidate.
7. **Bounded gap-scan** (anchor `current_index`, gap limit ~20–50, cache-first).
8. **Fallback-only providers** (BananaBlocks/ARCADE) until proven; never a single point of failure.

## 8. Open decisions (resolve after Phase 1)
- Reactive-first vs reactive+proactive-self-healer together (lean: reactive first).
- Pre-flight UTXO validation before every broadcast vs post-failure only (lean: post-failure first).
- Provider promotion (BananaBlocks/ARCADE fallback-only vs promoted).

## 9. Release
Ship the **critical-path reactive reconcile** in a `0.3.0-beta.XX` before the 0.4.0
chromium build. Pure Rust wallet — no chromium dependency. Verify on a real diverged
wallet (funded) before promote, per the standing update-stability principle.
