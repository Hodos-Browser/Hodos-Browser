# A BRC-121 payment is minted, then cannot be delivered: the BEEF header exceeds 100 KB

**Found:** 2026-09-17 by the owner, clicking a real paywalled article on the dev build minutes after
`P11-11-A1` landed. **Status:** 🔴 OPEN — measured, not fixed. **Severity:** high (money path, silent
to every automated check we have).

> 👤 *"I did see the banner notification, but it still didn't go to the site. I got an error saying
> exceeded maximum HTTP header buffer size of 100 kilobytes."*

⭐ The banner did its job — the owner could see a payment was in flight, which is exactly what A1 was
for. What it revealed is that the payment then **failed to deliver**.

---

## What happens

`pay_402` mints the BRC-29 payment and returns an **Atomic BEEF**, which the interceptor sends to the
paywalled server base64-encoded in the **`x-bsv-beef` request header**. Cloudflare caps total request
headers at **100 KB** and rejects anything larger with **431**.

📏 Measured, three consecutive `pay_402` calls on the same wallet, same site, same session:

| Time | txid | BEEF bytes | base64 ≈ | Result |
|---|---|---|---|---|
| 14:58:22 | `9fe12b73…` | 779 | ~1.0 KB | ✅ 200 |
| **15:20:35** | **`8ab21433…`** | **92,837** | **~124 KB** | 🔴 **431, then 400** |
| 15:22:03 | `83635c25…` | 849 | ~1.1 KB | ✅ 200 |

**110× spread between neighbouring payments.** The failing one is not an outlier in *amount* (150
sats, between the other two) — it is an outlier in **which coin got selected**, because BEEF carries
the selected coin's parent ancestry.

Server's own words, on the retry: `Exceeded maximum HTTP header buffer size of 100KB`.

## Why `preferSmallParents` did not save it

⛔ `pay_402` **already passes `prefer_small_parents: Some(true)`** (`handlers.rs` ~19292), so the
obvious first guess — "the 402 path never got P10d's fix" — is **wrong**. Do not write that fix again.

The real shape, and it is a clean lesson:

> ⭐ **P10d derived the small-parent threshold from the MessageBox limit (1 MiB). The BRC-121 path's
> real constraint is a 100 KB HTTP header — an order of magnitude tighter.** A coin that comfortably
> passes the PeerPay line still blows the 402 header, and base64 inflates it a further 33% on top.

⇒ The preference is real but **calibrated to the wrong ceiling for this path**, and it is a
*preference*, not a limit: nothing refuses when no sufficiently-small coin exists.

## Two further defects in the same sequence

### 1. 🟠 The auto-retry is provably futile here, and its comment says the opposite

`MAX_UPSTREAM_RETRIES = 1` fired and re-sent **byte-identical headers**, earning a 400. The code
comment justifies the retry as transient:

> *"Cloudflare … sometimes returns HTTP 431 … even though the same URL+headers will succeed seconds
> later."*

⛔ For an oversized header that is **deterministically false** — the same headers can never fit. The
retry costs a round trip and delays the error. It may still be right for *genuinely* transient 431s;
the fix is to distinguish them, not to delete the retry blindly.

### 2. 🔴 The user is shown the origin server's raw text

The failure surfaced as the site's own plain-text error. Hodos minted a payment, could not deliver it,
and said nothing in its own voice. ⚠️ A `/payment-failed` route with a Retry button is registered by
`RegisterBrc121FailedUrl` on this path — find out why it did not reach the screen (same class of
question as A1: the route exists, the hook may not).

## What is NOT wrong

- ⭐ **No money was lost.** The oversized payment was never broadcast. The later 100-sat payment
  broadcast cleanly (`SEEN_ON_NETWORK`).
- The banner (A1) worked correctly and is how this was caught at all.
- `pay_402` itself is fine — it built a valid BEEF. The problem is the transport ceiling.

## Why no automated check caught this

🚨 **Every green run we have used a small-parent coin by luck.** A1's own RED and GREEN both did
(779 and 779/849 bytes). There is no test anywhere that asserts anything about **BEEF size on the 402
path**, so the failure mode is invisible until a particular coin is selected. ⛔ It is not
reproducible on demand without controlling coin selection — which is exactly what the fix must add.

## Suggested shape of a fix (not implemented)

1. **Know the ceiling before minting — and ⛔ the ceiling is NOT 100 KB of BEEF.**

   👤 Owner 2026-09-17: *"Let me make sure it's under 100 kilobytes."* ⚠️ Targeting 100 KB **on the
   BEEF** would still fail, for two compounding reasons:

   | | |
   |---|---|
   | **base64** | `x-bsv-beef` carries `base64(BEEF)` = ×4/3. The failing 92,837-byte BEEF became **123,784 bytes** of header — 21,384 over the cap **before any other header is counted** |
   | **whole block** | Cloudflare's 100 KB applies to the **entire request header block**, not one header. We also send the page's own headers (User-Agent, **Cookie**, Accept-Language, Referer) plus four more `x-bsv-*`. Cookies on a logged-in site are several KB and vary per site and per user |

   📏 Derived budget: `max_BEEF ≈ (102400 − other_headers − margin) × 0.75`

   | other headers | max BEEF |
   |---|---|
   | 2 KB | ~72.8 KB |
   | 4 KB | ~71.2 KB |
   | 8 KB | ~68.2 KB |

   ⇒ the line is **~68–73 KB of raw BEEF**, and it is not a constant.

   ⭐ **P10d already paid for this exact lesson.** `10c772d` fixed the PeerPay size check by measuring
   **the whole request** (`wire_request_len`, the ~220 B envelope) against a real render, rather than
   checking the payload against the limit and guessing the margin. Same trap here, an order of
   magnitude tighter. ⇒ **Measure our actual assembled header block. Do not pick a number.**
2. **Refuse before minting, not after.** P10d's precedent on the PeerPay path is a 422 before
   broadcast; here the equivalent is: if no selection can fit the header budget, do not mint a
   payment that cannot be delivered. ⛔ Minting-then-failing leaves a `nosend` row and a phantom
   output every time — the same residue row `A4` is cleaning up.
3. **Distinguish a size 431 from a transient 431** before retrying. A size failure must not retry.
4. **Tell the user in Hodos's voice**, not the origin's plain text.
5. ⛔ **Negative control:** seed the wallet so the only fundable coin has a large parent, assert the
   payment is refused with a clear message and **no** `nosend` row — and assert the same wallet
   succeeds once a small-parent coin is available. Without controlling selection the test is luck.

## Cross-references

- `PHASE_CONTRACT_item11_brc121_feedback.md` — A1 (the banner that surfaced this), A4 (the `nosend` +
  phantom-output residue this creates one of every time).
- `TICKET_peerpay_message_exceeds_messagebox_limit.md` — **the sibling.** Same root (BEEF carries
  parent ancestry), different ceiling (1 MiB MessageBox body vs 100 KB HTTP header). ⭐ Read both
  together; the general defect is *we do not size-check BEEF against the transport that must carry it*.
- `phase-10-critical-advisories/10d-peerpay-delivery/` — where `preferSmallParents` came from.
- `handlers.rs :: pay_402` (~19292 for the options, ~19390 for the size log).
- `HttpRequestInterceptor.cpp :: Async402ResourceHandler` — `MAX_UPSTREAM_RETRIES` and its comment.

## Evidence

Dev logs, 2026-09-17: `wallet_rCURRENT.log` lines 352 / 1377 / 1592 (the three BEEF sizes);
`debug_output-34216.log` lines 6260–6286 (431 → retry → 400 + the server's message).
⚠️ Dev logs rotate.
