# The payment claim block — one format, written in beta.3, read in beta.5

**Status:** ⭐ **CANONICAL.** Written by beta.3 Phase 10d's *Copy details* (sender side). Read by the beta.5 *Claim a
payment* tool (receiver side, `../../../0.4.0-beta.5/TOOLS_TAB_claim_a_payment.md`). **Owner decision 2026-09-15:
whatever 10d emits is what beta.5 accepts.** Changing a field name here after beta.3 ships breaks every block a user has
already copied — add fields, bump `version`, never rename.

**Code that owns it:** `rust-wallet/src/handlers.rs :: payment_claim_block` (the only place a block is built) and its
golden-keys unit test `payment_claim_block_tests`. The beta.5 parser must be tested against a block produced by that
function, not a hand-typed example.

## Why it exists

A PeerPay payment is on chain but only the sender's message tells the recipient how to derive the key that spends it
(BRC-29). When the message cannot be delivered (MessageBox refused it, `TICKET_peerpay_message_exceeds_messagebox_limit.md`)
the sender copies this block and hands it to the recipient by any channel; the recipient's wallet claims the payment
from it. It carries no secret: the derivation strings are useless without the recipient's private key.

## Format — version 1

A single JSON object, UTF-8, copied as text:

```json
{
  "type": "hodos-payment-claim",
  "version": 1,
  "txid": "3798109e5612bc7e8bc1ebffc02d5c60a0b6e0d96ac84bf916542a3157d4ab55",
  "outputIndex": 0,
  "senderIdentityKey": "029dce5a3602abd74ddf0a9dd1a9e80dff2eb997bc0b01af70b5cde0e7d5166dba",
  "derivationPrefix": "/nlgzycY3JyqdYa3TrnKXQ==",
  "derivationSuffix": "hWTLW7coBngS4Py4wbimOw==",
  "amountSatoshis": 613685,
  "recipientIdentityKey": "020b95583e18ac933d89a131f399890098dc1b3d4a8abcdde3eec4a7b191d2521e"
}
```

| Field | Type | Meaning | Receiver may trust it for crediting? |
|---|---|---|---|
| `type` | string, exactly `hodos-payment-claim` | identifies the block | — reject anything else |
| `version` | integer, `1` | format version | — reject unknown versions |
| `txid` | 64 hex chars | the payment transaction | ✅ it is looked up on chain |
| `outputIndex` | integer | output that pays the recipient (PeerPay sends always use `0`) | ⚠️ **hint only** — scan every output for the one matching the derived key |
| `senderIdentityKey` | 66 hex chars (compressed pubkey) | the sender's identity key — the BRC-42 counterparty | ✅ required for derivation |
| `derivationPrefix` | string (base64) | BRC-29 derivation prefix | ✅ required for derivation |
| `derivationSuffix` | string (base64) | BRC-29 derivation suffix | ✅ required for derivation |
| `amountSatoshis` | integer | what the sender says they paid | ❌ **display only** — the credited amount is read from the chain |
| `recipientIdentityKey` | 66 hex chars | who the sender meant to pay | ❌ **check only** — if it is not this wallet's key, warn "this payment was not addressed to you" and still try (nothing credits unless the key derives) |

⭐ **Field names are BRC-100's, not invented.** `senderIdentityKey`, `derivationPrefix`, `derivationSuffix` are the
`internalizeAction` `paymentRemittance` names and `outputIndex` is the `InternalizeOutput` name
(`handlers.rs :: PaymentRemittance`, `InternalizeOutput`), so the claim tool maps a block to an `internalizeAction`
request field-for-field. `txid` and `amountSatoshis` match the names already used in `peerpay_outbox` / activity JSON.

## Rules for readers (beta.5)

1. Parse as JSON; reject if `type` or `version` is wrong, or a required field is missing or malformed.
2. Ignore unknown extra fields (so a later version can add without breaking version-1 readers).
3. Fetch the transaction and its proof by `txid` and build the Atomic BEEF locally; never ask the user for BEEF bytes.
4. Credit **only** through the hardened `internalizeAction` path (beta.3 10a: subject binding, no overwrite, non-200 on
   zero credit). A block for a payment already credited must hit the no-op path, not a second credit.
5. Show amount (from chain), sender key and date **before** the Claim button.

## Rules for writers (beta.3 10d)

1. Only `payment_claim_block` builds it.
2. `senderIdentityKey` is **this** wallet's identity key (the sender is us); `recipientIdentityKey` is the outbox row's
   recipient.
3. Sizes, causes and retry state are **not** part of the block — they live beside it in the activity item
   (`outbox_cause`, `outbox_message_bytes`, `outbox_cap_bytes`).
