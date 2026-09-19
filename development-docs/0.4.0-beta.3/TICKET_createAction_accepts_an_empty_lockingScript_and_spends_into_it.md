# `createAction` accepts an EMPTY `lockingScript`, builds an output with no spending conditions, broadcasts it, and reports success

**Found:** 2026-09-19 during the `W5`/`R-GOLD` sitting, by reading the **chain** rather than our own
response. **Status:** OPEN — measured, **not fixed**. 👤 **Production money-path code: asking before
changing** (root `CLAUDE.md` invariant 13).
**Severity:** MEDIUM-LOW as measured (100 sats, and the caller was malformed), but the failure mode is
"a buggy dApp makes the wallet spend into an unspendable output and get a success back".

## Measured, on mainnet

A `createAction` whose single output carried `lockingScript: ''` and **no** `address`:

```
txid 0d1a01e1c35ed72c8d1a8b92ff6421e98ae7882e0095f38311006fd5e582956d
  100 sats     -> (non-standard: EMPTY locking script)
1,000 sats     -> 1Q1A2rq6trBdptd3t6n53vB79mRN6JHEFT   (the documented Hodos service fee)
4,556,761 sats -> change
```

Our own response reported success and echoed `"scriptLength": 0`. The service fee and change were built
correctly — only the **requested** output was malformed.

## The question for the owner

The caller was wrong: BRC-100 expects a real locking script, and the request carried neither a script nor
an `address` (the struct accepts both — `handlers.rs :~4405` `script`/`lockingScript`, plus `address`).
⭐ **But should the wallet spend into it?** Today an empty script produces an output with no locking
conditions at all — nobody's coin, recoverable by anyone who bothers — and `createAction` returns a
normal success envelope with a txid.

⛔ Nothing here was changed. Candidate shapes, for the owner to choose:

1. **Reject** an output whose `script` is absent/empty **and** whose `address` is absent, before any
   coin selection. Cheapest, and it fails closed.
2. **Derive** P2PKH from `address` when the script is empty (what the demo's comment *assumed* happens).
   ⚠️ This is a behaviour change on the money path and needs its own decision — it makes a previously
   invalid request valid.
3. Leave it, and treat a malformed script as the caller's problem. ⚠️ Then say so explicitly in the
   handler, because the current silence reads like an oversight.

## 📏 How it was found, which is the reusable part

The test rig (`tmp/smoke-5b/payment.html`) passes `lockingScript: ''` with a comment claiming *"wallet
derives P2PKH from address-to-script"* — it validates the recipient field, then **never sends it**. Our
own success response looked fine. ⭐ Only asking the **chain** showed the coins had not gone to the
address at all. Working rule 7's *"ask the chain"* earns its place again: a wallet response is a claim
about the chain, never the chain.

## Cross-references

- `rust-wallet/src/handlers.rs :: create_action_internal`; output struct at `:~4405`.
- `tmp/smoke-5b/payment.html` — the malformed caller (fixture corrected 2026-09-19).
- Root `CLAUDE.md` invariant 13 (ask before changing production code reached via a failing test).
