# The wallet has no way to shed a large parent, so a coin can stay unsendable forever

**Opened:** 2026-09-17 · **Status:** 🔴 OPEN — not implemented · **Severity:** medium now, high once it bites
**Raised by:** 👤 owner, during `P11-11-A7`/`A8`.

> 👤 *"We can't just leave the wallet not able to send to all of these things. Especially a message box.
> We need to proactively do something, which would be to spend these big parents back to ourselves. I
> think we're already doing in the consolidation because we talked about this yesterday."*

⛔ **We are not.** That is the finding, and it is why this ticket exists.

---

## What is missing

`monitor/task_consolidate_dust.rs` filters on **`satoshis <= DUST_THRESHOLD_SATS` (1000)** — purely
value-based. It never looks at parent size.

🚨 And large-parent coins are typically **high**-value: the coin that started all of this was the
on-chain backup's change at **38,347,126 sats** with a 436 KB parent. Dust consolidation would never
touch it. ⇒ **Nothing in the wallet ever cleans up a large parent.**

What `P10d` shipped was *avoidance*, not cleanup:
- prefer small-parent inputs for bundle-carrying sends,
- fund the backup from the smallest sufficient coin so its change stays small.

Both reduce how often a bad coin is *created* or *chosen*. Neither removes one already held.

## Why it matters

A coin with a large parent is **permanently unusable for every size-capped channel** — PeerPay over
MessageBox (1 MiB body) and BRC-121 over an HTTP header (≈100 KB block). It spends fine on chain; it
just cannot be *delivered* through those channels. As a wallet is used, such coins accumulate and
never leave.

⇒ In the limit, a wallet whose coins all carry large parents can pay nobody by PeerPay and buy no
paywalled page, while showing a healthy balance. `P11-11-A8` makes the selector try much harder, and
`A7` makes the failure honest, but neither creates a clean coin.

## The mechanism that fixes it — and the catch the owner identified

👤 *"we know the problem that the beef, those need to be confirmed or else the beef is going to include
the grandparent, which is still the big one. So it defeats the purpose."*

⭐ **Exactly right, and the timing is the whole design.** Spending a large-parent coin back to
ourselves produces a new coin whose parent is the consolidating transaction — which *itself* has the
large transaction as its parent. While that consolidating tx is **unconfirmed**, BEEF must carry the
whole chain and nothing is gained.

**Once the consolidating transaction is mined it carries a BUMP (BRC-74 merkle proof), and BRC-62
stops the ancestry walk there.** The new coin's BEEF is then a few hundred bytes regardless of what
came before. So the cure works, but only after a block — typically ~10 minutes.

⇒ Any implementation must treat "consolidated" as **not done until confirmed**, or it will hand the
user a coin that is no better and report success.

## Suggested shape (not implemented)

1. **Detect**: a background pass that finds spendable coins whose parent exceeds the *tightest* channel
   line we care about, using the `parent_transactions` cache the selector already reads.
2. **Consolidate**: spend them to ourselves. ⚠️ Rules that already exist and must be honoured — the
   1-sat token floor (`P8a`), token-reserved values, and never touching non-default baskets.
3. ⛔ **Do not report success on broadcast.** The coin is only fixed when the consolidating tx has a
   merkle proof. `TaskCheckForProofs` already does that work; this should hang off it.
4. **Tell the user honestly while it is pending** — 👤 *"they won't be able to send again until they're
   confirmed in a block. Try again in 10 or 15 minutes."* The `pay_402` 422 already explains the cause
   and the remedy; it deliberately does **not** claim a consolidation is running, because none is.
5. ⚠️ **Cost:** each consolidation is a real transaction and pays the 1000-sat service fee, so it must
   not run on a hair trigger. Decide the threshold deliberately — 👤 owner's call.
6. ⛔ **Negative control:** seed a wallet whose only fundable coin has a large parent; assert PeerPay
   and `pay_402` both fail before, that the consolidation runs, that they STILL fail while it is
   unconfirmed (this is the arm that catches a fix which ignores confirmation), and that both succeed
   once it is mined.

## Cross-references

- `TICKET_brc121_beef_header_exceeds_100kb_and_payment_is_lost.md` — the 431 that exposed it.
- `TICKET_peerpay_message_exceeds_messagebox_limit.md` — the same root on the other channel.
- `phase-11-ui-leftovers/PHASE_CONTRACT_item11_brc121_feedback.md` — `A7` (per-channel line + hard
  stop), `A8` (cumulative ancestry budget). Both are avoidance; this ticket is the cleanup neither does.
- `rust-wallet/src/monitor/task_consolidate_dust.rs` — value-based, the reason this gap exists.
- `rust-wallet/src/handlers.rs :: select_utxos_within_ancestry_budget`, `BRC121_MAX_BEEF_BYTES`.
