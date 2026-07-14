# MAC BACKUP NULLFAIL — STEP 1 RESULTS

**Analyzed 2026-07-13 by Mac Claude. All verification performed offline in a scratch directory.**

---

## Step 1 Outcome: RAW TX IS VALID — BEEF/BROADCAST LAYER PROBLEM

### Verification Method

Used `@bsv/sdk` (v2.1.2) for BIP143/ForkID preimage computation + the native `secp256k1` C library
(`npm:secp256k1`) for ECDSA signature verification. This is the same secp256k1 implementation used by
BSV nodes — fully independent from both the wallet's Rust code and from the earlier Python verification.

### Control Test: Tx 8 (last successful backup, accepted by network)

| Input | Script Type | scriptCode | sighash | Signature |
|-------|-------------|------------|---------|-----------|
| 0 (PushDrop) | Full 2611-byte locking script | `0dc516cf52d9b05743f7ac5128aac983026b3783641044c189ab41ef2b9827a7` | **✅ VALID** |
| 0 (PushDrop) | PREFIX-only (35 bytes) | `2cc5620bac75b4b41b5eca571de3400f9aecf61a978f2a71c7c8ca3202e0b5c1` | ❌ INVALID |
| 1 (Marker P2PKH) | Standard P2PKH | `8b8bb5d9f0e066eb9101c82f59f17baaad31bc1ac5f83eedd4c6624445966afd` | **✅ VALID** |
| 2 (Change P2PKH) | Standard P2PKH | `cea00ffeab440340cfa8ae94d0f7d9972bb4114fa5b27fe05c714cfbc5ec4f03` | **✅ VALID** |

**Control passes.** All 3 inputs of the network-accepted tx 8 verify. PREFIX-only does NOT verify,
confirming BSV nodes use the FULL PushDrop locking script as scriptCode.

### Test: Tx 9 (failed backup, NULLFAIL from network)

| Input | Script Type | scriptCode | sighash | Signature |
|-------|-------------|------------|---------|-----------|
| 0 (PushDrop) | Full 2648-byte locking script | `334b0baf72df1170b969ce576060c64ebe452b4b14d2d5e7a521f62044c8b0a3` | **✅ VALID** |
| 0 (PushDrop) | PREFIX-only (35 bytes) | `162c30dea4e73b9b6d61594ecab50f92f62f90b3173709947f9b5137a57cdda7` | ❌ INVALID |
| 1 (Marker P2PKH) | Standard P2PKH | `827d94271dd719db3e2667208bea32b4bb1ade29fccf79b6a58a68a072063438` | **✅ VALID** |
| 2 (Change P2PKH) | Standard P2PKH | `1a44576345ce97f2b935b6f52d3ae58e2dc1898b502af3104f8bde30b3da59a6` | **✅ VALID** |

**All 3 inputs of the network-rejected tx 9 are cryptographically VALID.** The wallet computed
the correct sighash using the full locking script, and the ECDSA signatures verify against the
secp256k1 C library.

### Decision

**Tx 8 verifies (control passes) AND tx 9 ALSO verifies → the raw tx IS valid → the problem is
the BEEF/broadcast layer, not signing.**

The wallet's sighash computation in `transaction/sighash.rs` is correct. The PushDrop scriptCode
path works correctly — it uses the full on-chain locking script (2648 bytes), which matches what
BSV nodes expect.

The NULLFAIL error must originate from something the nodes SEE that differs from the raw transaction
we verified. The BEEF envelope is the only intermediary between the signed transaction and the
broadcasting node.

---

## What This Rules Out

- ~~Wallet sighash bug~~ — sighash computation is correct (verified by independent SDK + secp256k1)
- ~~ScriptCode truncation~~ — wallet uses the full PushDrop script, PREFIX-only does NOT verify
- ~~Wrong key derivation~~ — the backup pubkey hashes to the correct marker address
- ~~Wrong satoshi value~~ — DB value matches on-chain (1000 sats)
- ~~Wrong locking script~~ — DB script == on-chain script (2648 bytes, 100% byte match)
- ~~DB-chain divergence~~ — DB tip == chain tip (no fork)

## What Remains: BEEF Layer Investigation

The BEEF envelope wraps the raw transaction with parent transaction ancestry for broadcast. If the
BEEF embeds incorrect parent tx data (different output scripts or values than what's on-chain), the
node's BEEF processor would compute a different sighash than the one the wallet signed.

### Recommended Next Step: Capture BEEF bytes

Add temporary instrumentation to `do_onchain_backup` (handlers.rs, Step 11) to save the serialized
BEEF hex to a file before broadcast:

```rust
// TEMPORARY DIAGNOSTIC — remove after investigation
std::fs::write("/tmp/backup_beef_debug.hex", &beef_hex).ok();
log::warn!("🔬 DIAG: BEEF saved to /tmp/backup_beef_debug.hex ({} bytes)", beef_hex.len());
```

Then parse the saved BEEF to extract:
1. The parent transaction for `7855796d...` — compare its raw bytes with the actual on-chain tx
2. The main transaction — compare with the raw_tx in the DB
3. Any BUMPs/merkle proofs — verify they're well-formed

### Alternative Quick Test

Try broadcasting the raw tx directly (without BEEF wrapping) via WoC's `/tx/raw` endpoint. If the
raw tx succeeds, the BEEF construction is definitively the problem. This is the fastest path to
confirmation.

A code-level fallback could be: when BEEF broadcast fails with NULLFAIL, retry with raw-tx-only
broadcast (no BEEF wrapper). This would unblock the backup immediately while the BEEF bug is
investigated.

---

## Verification Script Location

The full verification script is at:
`/private/tmp/claude-501/-Users-matt/23e0a5f6-7966-45d3-83ea-73217e041fe8/scratchpad/bsv-verify/verify4.mjs`

Dependencies: `@bsv/sdk`, `secp256k1` (native C binding). Run with `node verify4.mjs`.
