# MAC BACKUP NULLFAIL — NEXT DIAGNOSTIC STEPS (for Mac Claude)

**From Windows Claude (lead). Goal: settle ONE question — is the NULLFAIL a WALLET SIGHASH bug (our
signing produces a sighash real nodes won't reproduce) or a BEEF/BROADCAST-layer problem? READ-ONLY on
the real wallet. Back up before anything. Report findings + push.**

## Background (settled so far)

- Backup broadcast fails NULLFAIL (`mandatory-script-verify-flag-failed / Signature must be zero for failed
  CHECKSIG`) since ~Apr 15. NOT divergence, NOT c5b, NOT insufficient funds. Key is fine (on-chain marker
  sits at HASH160(backup_pubkey), verified).
- Code hunt PROVED: backup signs input 0 (the previous PushDrop) against the **stored/fetched on-chain
  script + value** — no reconstruction (`handlers.rs:13324-13325`). A real, since-fixed truncation bug
  existed (old adopt path read WoC JSON `scriptPubKey.hex`, which truncates large scripts — comment at
  `handlers.rs:12717`).
- **The contradiction:** July's failing attempt took the "trusting DB" path, and you found DB script ==
  on-chain byte-for-byte + value == 1000. If that's really what's signed, the sighash should match the
  node's and it should NOT fail — yet it does. Only the two tests below resolve it.
- **Trap to avoid:** your earlier "signatures are valid" used a hand-rolled sighash. If that hand-rolled
  code shares the wallet's flaw, it "verifies" a tx the network rejects. Use an INDEPENDENT library below.

---

## STEP 1 — OFFLINE reference verification (DO THIS FIRST; zero risk, no wallet run, no copy needed)

Use `@bsv/sdk` (independent, trusted BIP143/ForkID impl) — NOT a hand-rolled sighash — to verify tx 9's
input-0 signature against the REAL on-chain source output.

1. Inputs you already have: tx 9 raw hex (DB `transactions` id=9, txid `76c47e92...`, 3144 bytes); the
   spent output = `7855796d...:0` PushDrop, on-chain locking script (2648 bytes), value 1000.
2. In a scratch Node project: `npm i @bsv/sdk`. Load tx 9 via `Transaction.fromHex(...)`. For input 0, set
   its `sourceTransaction` (or `sourceOutput` with the on-chain 2648-byte `lockingScript` + `satoshis:
   1000`). Extract the 72-byte unlock signature already in tx 9's input 0.
3. Compute the sighash with `@bsv/sdk` (`TransactionSignature.format` / the SDK's preimage) using
   SIGHASH_ALL | FORKID, scriptCode = the on-chain locking script, value = 1000, and ECDSA-verify the
   signature against the pubkey decoded from the PushDrop script (`02b9d142...`).
4. Cross-check: do the SAME for tx 8 input 0 (the last SUCCESSFUL backup, spent tx7's PushDrop) — it MUST
   verify (nodes accepted it). That confirms your @bsv/sdk harness is correct before trusting the tx-9 result.

**Decision:**
- tx 8 verifies (control passes) AND **tx 9 FAILS** → the wallet's PushDrop sighash/signing is genuinely
  wrong → root cause is in OUR signing code (`transaction/sighash.rs` / the PushDrop scriptCode path), and
  it applies to July rebuilds too. **This is the expected result.** Report the exact divergence you can see
  (e.g., strip DER/sighash-flag correctly; if the sig only verifies with a DIFFERENT scriptCode — e.g.
  `<pubkey> OP_CHECKSIG` prefix only, or a CODESEPARATOR-trimmed script — that names the bug).
- tx 9 VERIFIES against @bsv/sdk too → the raw tx is valid → the problem is the **BEEF/broadcast layer**,
  not signing. Then STEP 2 / BEEF capture is warranted.

If you can, also try: does tx 9's input-0 sig verify if scriptCode = ONLY `<pushdrop pubkey> OP_CHECKSIG`
(the prefix before the data pushes) instead of the full 2648-byte script? If THAT is what verifies, we've
found it — the wallet signs a truncated/prefix scriptCode while nodes use the full script.

---

## STEP 2 — LIVE preimage dump (ONLY if Step 1 is inconclusive)

Needed only to capture what the *running* wallet feeds the signer on a fresh July rebuild.

**Safety first:**
- **Back up the whole installed data dir:** `cp -R "~/Library/Application Support/HodosBrowser" ~/HodosBrowser-BACKUP-$(date +%s)`. (Use a fixed timestamp string — do it manually.)
- You COPY the installed wallet INTO the dev dir; you do NOT modify or replace the installed wallet. Nothing
  to restore afterward — just delete the dev copy when done.

**Procedure:**
1. Copy state into the dev dir: `cp -R "~/Library/Application Support/HodosBrowser/"* "~/Library/Application Support/HodosBrowserDev/"` (create HodosBrowserDev if absent). This gives the dev build this wallet's full state.
2. Apply the diagnostic patch (Windows Claude will push a `diag/backup-nullfail` branch, OR add it yourself)
   — a temporary log block at `handlers.rs:13324` that logs, for input 0: `prev_script_bytes.len()`, full
   `hex::encode(prev_script_bytes)`, `prev_sats`, source (DB vs adopt), the full BIP143 preimage, the
   sighash, `PublicKey::from_secret_key(secp, backup_privkey)` (pk-from-key) AND the pubkey decoded from
   the PushDrop script (`pushdrop::decode(...).locking_public_key`). Do the same 3-line dump for inputs 1
   and 2 (script + value) so we can see which input diverges.
3. Build the instrumented wallet: `cd rust-wallet && cargo build --release`.
4. Run it with `HODOS_DEV=1` (launcher script). **If the copied wallet will NOT unlock (Keychain), STOP and
   report — do NOT run the instrumented binary against the real installed dir.**
5. Trigger one backup: `curl -X POST http://localhost:<dev-wallet-port>/wallet/backup/onchain`.
6. Capture the `🔬 DIAG` lines from the dev wallet log.
7. Compare the dumped input-0 script hex + value against the REAL on-chain `7855796d...:0` (WoC raw tx).

**Decision from the dump:**
- dumped script SHORTER / hex ≠ on-chain → scriptCode divergence CONFIRMED (owner's chain-truth fix is the
  root fix).
- script + value match on-chain but preimage still differs → tx-BODY divergence (hashPrevouts/Sequence/
  Outputs) — the wallet's sighash covers a different tx than it serializes.
- pk-from-key ≠ pk-from-script → key problem (unlikely; marker already excludes it).

**Cleanup:** delete the dev-dir copy; revert the diagnostic patch. Installed wallet was never touched.

---

## Report back

Write to `development-docs/Wallet-Hardening/MAC_BACKUP_NULLFAIL_RESULTS.md` and push to `origin/0.4.0`:
Step-1 outcome (tx8 control + tx9 result + which scriptCode verifies), and Step-2 dump if run. State
plainly: wallet-sighash bug or BEEF-layer, with the evidence.
