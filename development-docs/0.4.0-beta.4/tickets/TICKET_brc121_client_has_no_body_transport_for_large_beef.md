# 📦 Our 402 client can only carry a payment in a header, and the spec it speaks has no body path

**Found:** 2026-09-23, reading `bsv-blockchain/ts-stack` PR #569 and BRC-118 against our own `pay_402`.
**Status:** ⬜ UNASSIGNED · **Track:** unassigned — ⭐ a decision for the microscope pass, not a phase
now; it waits on an upstream answer · **Filed by:** Claude, at the owner's request

> ⚠️ **Method note.** ⛔ **This is not a defect ticket.** Our client does what BRC-121 (Simple 402
> Payments) specifies. It records that a body transport now exists for the *sibling* 402 protocol, that
> BRC-121 has none, and that the choice of what Hodos does about it is open. **Everything below is code
> reading** — BRC-118, the #569 diff, and our `handlers.rs :: pay_402` and
> `HttpRequestInterceptor.cpp`. ⛔ **Nothing was executed.** Not verified: whether any live BRC-105 server
> advertises multipart today.

---

## What happens

Hodos pays a 402 the BRC-121 way: the payment's Atomic BEEF goes base64 into an `x-bsv-beef` request
header, with `x-bsv-sender`, `x-bsv-nonce`, `x-bsv-time` and `x-bsv-vout`
(`cef-native/src/core/HttpRequestInterceptor.cpp`, the header insert near the
`ctx_.beefBase64` line). There is no other transport; BRC-121 defines none.

The sibling protocol, BRC-105 (HTTP Service Monetization Framework, Ty Everett and Brayden Langley),
carries its payment in an `x-bsv-payment` header and has the same growth problem. **BRC-118 (Multipart
Body Transport for BRC-105 Payments, John Calhoun, merged 2026-03-09)** fixes it for 105: the server
advertises `x-bsv-payment-transports: header,multipart` in its 402, and a client whose payment exceeds
about 8 KB sends it as a `multipart/form-data` part instead of a header. `ts-stack` PR #569 (opened
2026-09-23, unmerged) implements both ends in TypeScript, with the client measuring the whole request
before broadcast and aborting the prepared action if it will not fit. The same PR's README for the
BRC-121 package says BRC-118 *"is not implicitly negotiated here. Existing 402-pay wire behavior is
unchanged."*

We have no BRC-105 client and no multipart sender *(measured: grep for `x-bsv-payment-transports` and
`multipart` in `rust-wallet/src` — zero hits; `x-bsv-beef` appears in one file)*.

## Why it matters

A BRC-121 payment whose BEEF outgrows the header budget cannot be delivered, full stop. Our beta.3 fix
(`P11-11-A7`) makes that failure clean — 64 KB BEEF budget, small-parent coin selection, refuse before
minting — but it is a ceiling, not a cure: a wallet whose every coin has a large unconfirmed parent
cannot pay at all until something confirms. A body transport removes the ceiling. Today it exists only
for a protocol we do not speak.

## How exposed are we — answer this first

| If | Then |
|---|---|
| Deggen adopts BRC-118's negotiation into BRC-121 (our #261 asks this) | We implement the multipart sender against the 121 spec; one protocol, one sender |
| BRC-121 states a header bound instead | Nothing to build beyond aligning our 64 KB budget to the stated number |
| BRC-121 stays silent and live 402 servers move to 105+118 | We need a 105 client to pay them at all — bigger than a transport change, and a product decision |
| Nothing moves upstream | Our ceiling stands; the failure is clean and rare (first measured 2026-09-17, one wallet) |

Exposure today is **low**: no server we pay advertises multipart, and our clean refusal is shipped.
The exposure is future-facing and depends on which way the two 402 specs go.

## What already protects us, and how that shapes the fix

`handlers.rs :: pay_402` and the `P11-11-A7` budget: BEEF capped at 64 KB deliberately under the
≈68–73 KB the arithmetic allows against Cloudflare's 100 KB whole-header-block limit; base64 ×4/3 and the
page's own headers accounted for; `prefer_small_parents` on for this channel. So the *preflight* half of
BRC-118's client behaviour already exists here. What is missing is only the *transport* half — and a
server that accepts it.

## Proposed fix

**Decide, don't build yet.** Two options, in order of preference:

1. **Wait for #261.** If BRC-121 adopts the negotiation, implement: read `x-bsv-payment-transports` from
   the 402; when advertised and the base64 BEEF would exceed 8 KB, send the five `x-bsv-*` values as
   named multipart parts with the original body as a `body` part; keep the header form as the default.
   Reuse the `P11-11-A7` budget as the switch point rather than inventing a second number.
2. **Speak BRC-105 as well** — only if live servers we care about move there. Separate ticket if so.

**Deliberately out of scope:** PeerPay. Its limit is the MessageBox relay's message cap, not an HTTP
header; BRC-118 does nothing for it. Also out: raising our 64 KB budget.

## Test and negative control

⛔ Per `../../0.4.0-beta.3/HARNESS.md`: **a fix is not done until the check has been *seen* to fail.**

| | |
|---|---|
| **GREEN** | Against a server advertising `header,multipart`: a payment whose base64 BEEF exceeds 8 KB arrives as multipart, is verified, and returns 200 with the content; a small payment still goes in the header |
| **RED** | Same large payment against the same server with the advertisement stripped from its 402: the client refuses before minting (no broadcast, no `createAction` success), and says why |
| **SUBJECT** | The `payment-express-middleware` from #569 (or whichever 121 server ships the negotiation) running locally, with its request log showing which transport carried the payment; our wallet log showing the preflight decision |
| **Tier** | T2 (real HTTP, local server) |

**Standing invariant?** Yes if implemented: *"a 402 payment is never broadcast before the request that
carries it has been measured against the transport's limit."* That row already exists in spirit as
`P11-11-A7`; extend it rather than add one.

## Links

- BRC-118: `https://github.com/bsv-blockchain/BRCs/blob/master/payments/0118.md`
- BRC-121: `https://github.com/bsv-blockchain/BRCs/blob/master/payments/0121.md`
- Our issue: `https://github.com/bsv-blockchain/BRCs/issues/261` — the follow-up comment is drafted in
  `Marston Enterprises/Standards/BRCs/drafts/peerpay-messagebox-size-and-encoding/COMMENT_261_brc118_correction.md`
- ts-stack PR #569: `https://github.com/bsv-blockchain/ts-stack/pull/569`
- Our reading of #569: the same folder, `NOTES.md` §8
- Our budget: `rust-wallet/src/handlers.rs`, the `P11-11-A7` comment above `large_parent_bytes()`'s 402 sibling
- `../../X402_INTEGRATION.md` — the product-level record of which 402 flavour Hodos speaks
