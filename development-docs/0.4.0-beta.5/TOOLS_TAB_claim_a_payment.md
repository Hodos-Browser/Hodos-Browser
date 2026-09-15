# beta.5 — a Tools tab in the advanced wallet, starting with "Claim a payment"

**Filed:** 2026-09-15, from beta.3 Phase 10d · **Owner:** Matthew Archbold · **Status:** 📋 OUTLINE — not scoped, not scheduled inside beta.5
**Origin:** `../0.4.0-beta.3/TICKET_peerpay_message_exceeds_messagebox_limit.md` and
`../0.4.0-beta.3/phase-10-critical-advisories/10d-peerpay-delivery/PHASE_CONTRACT.md` (claim box moved out of beta.3).

## Owner decisions, 2026-09-15

1. The receiver-side claim tool is **not** in beta.3. beta.3 ships the sender half only (refuse before broadcast, the
   yellow "recipient not notified" line, Retry, **Copy details**).
2. It goes in beta.5, in the advanced wallet, as a card on a new **Tools** tab.
3. **Visible, not hidden** (recommended and accepted; revisit when scoped). Hiding adds no protection — a claim can only
   credit coins that are on chain and derive to this wallet's own keys — and the users who need it are the ones least
   likely to find a hidden control. Hidden features also drop out of testing.
4. ⛔ **The input format is fixed already**: the claim tool reads exactly the block beta.3's Copy details writes —
   `../0.4.0-beta.3/phase-10-critical-advisories/10d-peerpay-delivery/PAYMENT_CLAIM_BLOCK.md`, built only by
   `rust-wallet/src/handlers.rs :: payment_claim_block` and pinned by its golden-keys test. Do not redesign the fields
   here; blocks users copied from beta.3 onward must keep working.

## The Tools tab

- **Where:** advanced wallet sidebar (`frontend/src/components/wallet/WalletSidebar.tsx`, today Dashboard / Activity /
  Certificates / … / Settings) — a new **Tools** entry, placed just above Settings.
- **Shape:** one card per job. Each card has a title, one "when to use this" sentence, the control, and a result line. No
  jargon on the card face; details behind a "What does this do?" expander.
- **First step when scoped:** inventory the wallet endpoints that exist without a screen and decide which earn a card.
  Candidates seen in the route table (not checked for existing UI): `/wallet/rescan`, `/wallet/consolidate-dust`,
  `/wallet/peerpay/outbox-retry` (per-row Retry already lives in Activity), `/wallet/sync?full=true`. ⛔ `/wallet/debug/*`
  stay developer-only and do **not** get cards.

## Card 1 — Claim a payment

**When to use this:** "Someone paid you but their wallet could not notify yours. Paste the payment details they sent you."

### What the user enters

One paste box, not a form of fields. The sender's **Copy details** produces a single block; the receiver pastes it.

| From the block | Used for |
|---|---|
| `txid` | fetch the transaction and its proof from the chain; build the BEEF locally (never ask the user for BEEF) |
| `senderIdentityKey`, `derivationPrefix`, `derivationSuffix` | BRC-42 / BRC-29 derivation of the key the output pays |
| `outputIndex` | hint only — scan every output for the one matching the derived key |
| `amountSatoshis` | shown to the user; **never** used for crediting (the chain's value is) |
| `recipientIdentityKey` | if it is not this wallet, warn "this payment was not addressed to you"; still try (nothing credits unless the key derives) |

### Flow

1. Paste → parse → if invalid, one sentence saying which part is wrong.
2. Look up `txid` on chain → show **amount (from chain), sender key prefix, date** → **Claim** button.
3. Claim → the hardened `internalizeAction` path (beta.3 10a: subject binding, never overwrite an existing output,
   non-200 when nothing is credited).
4. Result line: "Claimed N sats" · "Already in your wallet" (already credited — the no-op path) · "Not yours: no output
   in this transaction pays your keys" · "Not found on chain yet — try again after it is mined".

### Acceptance (sketch — the real contract is written when beta.5 schedules it)

| Row | GREEN | RED |
|---|---|---|
| T-A1 | A block produced by `payment_claim_block` for a real payment to this wallet claims it once, with the chain's amount | Without the card the block has nowhere to go (the beta.3 state) |
| T-A2 | The same block pasted twice ⇒ second paste reports "already in your wallet", balance unchanged | Pre-10a `store_derived_utxo` would have rewritten the row |
| T-A3 | A block for someone else's payment ⇒ "not yours", nothing credited | — |
| T-A4 | A block whose `amountSatoshis` is edited upward ⇒ credited amount is still the chain's | trusting the field ⇒ inflated balance |
| T-A5 | The parser is tested against output of `payment_claim_block`, not a hand-typed example | a hand-typed fixture drifts from the writer |

## Out of scope for this card

Claiming non-PeerPay payments (paymail P2P, BRC-121). Receiving blocks automatically (link handlers, QR). Any change to
the block format beyond adding fields under a new `version`.
