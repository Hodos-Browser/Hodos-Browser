# TICKET — a PeerPay message can exceed MessageBox's 1 MiB cap, so the payment lands on chain but the recipient is never told

**Filed:** 2026-09-15, during beta.3 Phase 10a `P10a-A5` · **Owner:** Matthew Archbold · **Status:** 📌 **SCHEDULED as beta.3 Phase 10d** (`phase-10-critical-advisories/10d-peerpay-delivery/PHASE_CONTRACT.md`), ordered 10a → 10d → 10b → 10c by the owner 2026-09-15 — **measured**, not a code reading

> ⚠️ **Corrections after the first diagnosis (2026-09-15, later the same day, measured):** the sender had **zero**
> unconfirmed coins. The 495 KB bundle is 11 mined parents in full (BRC-62 requires them) and one of them is our own
> **433 KB on-chain backup transaction** whose change the send spent — coin selection is largest-first, so a backup's
> change is picked first for the next send. ⛔ **Base64 is not available to us:** the BSVA receiver does
> `new Uint8Array(token.transaction)` (our own sender comment of 2026-03-09 records that we tried base64 and it broke
> interop). The upstream issues live in Marston `Standards/BRCs/drafts/peerpay-messagebox-size-and-encoding/`.
> 👤 Owner decisions: keep the array; prefer small-parent inputs for bundle-carrying sends; refuse before broadcast;
> **no de-taint self-send** (it re-triggers a backup — loop); instead **backup funding picks smallest-sufficient coins**;
> housekeeping stays fee-exempt; yellow dot + banner + Activity line + Retry + Copy details; claim box = owner to decide.
**Severity:** 🔴 High on the money path — the sats leave the sender and sit at an address the recipient cannot derive
**Layer:** Rust `handlers.rs :: peerpay_send` (sender) · `monitor/task_retry_peerpay_outbox.rs` (retries the same oversized body forever, then `exhausted`)

## What was measured

The owner sent a PeerPay from the **installed** Windows wallet (release build) to the dev wallet's identity key:

| | |
|---|---|
| txid | `3798109e5612bc7e8bc1ebffc02d5c60a0b6e0d96ac84bf916542a3157d4ab55` — on chain, vout 0 = 613,685 sats to the BRC-29 derived address |
| sender log | `⚠️ MessageBox delivery failed, queuing for retry: API error: sendMessage failed (413): {"code":"ERR_MESSAGE_BODY_TOO_LARGE","description":"Message bodies must not exceed 1048576 bytes."}` — then `Outbox retry failed … (attempt 1..4)`, same 413 each time |
| queued payload | **1,764,588 bytes** of plaintext JSON: `{"customInstructions":{…},"transaction":[1,1,1,1,…],"amount":613685}` |
| the BEEF inside | **494,725 bytes** (Atomic BEEF with the full unconfirmed ancestry of the inputs) |
| recipient poller | `polled payment_inbox — 0 message(s)`, 26+ times. The recipient has **no way** to derive the key for vout 0 without `derivationPrefix`/`Suffix`, which only travel in the message |

So the money is on chain, provably the recipient's, and **invisible to the recipient**. The sender's UI showed a completed send.

## Why the message is bigger than the limit

Two multipliers, one of them ours:

1. **The BEEF carries every unconfirmed ancestor.** A wallet that keeps spending its own fresh change builds a chain of
   unconfirmed parents, and BRC-62 BEEF must include each of them in full (raw bytes) until one is mined and can be
   replaced by a merkle proof. The installed wallet's inputs had ~495 KB of that.
2. **We serialise those bytes as a JSON array of integers.** `peerpay_send` puts `"transaction": tx_array` — every byte
   becomes up to four characters (`,255`). 495 KB × ~3.6 = 1.76 MB. **Base64 would be ~660 KB and would have fit.**
   The receiving side already accepts both shapes (`task_check_peerpay.rs :: parse_payment_token` — *"accept base64
   string OR byte array"*), so the sender's choice is the whole overrun here.

## Fix shape (owner to schedule — likely beta.3 Phase 10 tail or beta.4 sprint 0's neighbours)

1. **Sender: encode `transaction` as base64** (one line; the receiver already parses it). Halves-to-thirds the body. Also
   what `wallet-toolbox`'s PeerPay message does — check its `MessageBoxClient` payload shape before choosing.
2. **Sender: refuse to broadcast when the message will not deliver.** Build the message **before** broadcasting; if it is
   over MessageBox's cap (1,048,576 bytes), fail the send with a clear error *before* any sats move — today the order is
   broadcast, then try to deliver, then queue a retry that can never succeed.
3. **Ancestry:** wait for / fetch merkle proofs for confirmed parents so BEEF carries BUMPs instead of raw parents
   (`beef_helpers.rs` already prefers proofs when present — the problem is only unconfirmed chains). Optionally prefer
   confirmed inputs in coin selection for PeerPay sends.
4. **Outbox:** a 413 is not transient — stop retrying it (mark `undeliverable`), surface it in the wallet UI, and keep the
   remittance so the user can hand it to the recipient out of band.

## Recovery performed (dev wallet, 2026-09-15)

The sender's queued remittance (`derivationPrefix` / `derivationSuffix`, read **read-only** from the installed DB's
`peerpay_outbox`) plus the BEEF were POSTed to the dev wallet's `/internalizeAction` as a `wallet payment` remittance
(sender identity key `029dce5a…`). Credited once, 613,685 sats, correct derivation. ⚠️ **This is a manual, one-off
recovery that required reading the sender's database** — an ordinary user pair has no such path. That is the defect.

## Acceptance (when scheduled)

| Row | RED | GREEN |
|---|---|---|
| A1 | Send with ~500 KB BEEF ancestry ⇒ today 413 + retries | Delivered (base64 body < 1 MiB), recipient poller credits it |
| A2 | Body still over the cap after encoding ⇒ today broadcast anyway | Send **refused before broadcast** with a named error |
| A3 | Outbox row on 413 ⇒ today retried ×20 then `exhausted`, silent | Marked undeliverable on the first 413, user told |

## Cross-references

`phase-10-critical-advisories/10a-peerpay-atomic-subject/PHASE_CONTRACT.md` (`P10a-A5`) · `PAYMENT_TEST_BATCH.md` M9 ·
`rust-wallet/src/monitor/CLAUDE.md` (TaskRetryPeerPayOutbox: 60 s ×10, 120 s ×10, then exhausted)
