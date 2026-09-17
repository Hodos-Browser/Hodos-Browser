# A paywalled article took 41 seconds, minted three payments, and showed the user nothing

**Found:** 2026-09-16, payment sitting, by the owner reading a real 402-paywalled article at
`now.bsvblockchain.tech`. **Status:** 🔴 OPEN — measured, not fixed. **Severity:** high (user-visible,
money path).

> 👤 *"I can tell you on my end that it did not work. The page didn't load. I didn't see anything. I
> didn't get any feedback, nothing."* … then, half a minute later: *"it just now loaded, but that took
> like literally 30 seconds when it should take less than one second."*

⭐ **The logs, read alone, say this SUCCEEDED.** Payment made, content fetched, broadcast confirmed,
gold pill fired. Only the person sitting in front of it knew it had failed. That gap is the whole
reason this row was owed to a human.

---

## Measured timeline, one article

| Time | What happened |
|---|---|
| 18:45:25.955 | paid retry #1 → `CefURLRequest error=-3` (ERR_ABORTED) → `NOT broadcasting (funds preserved)` |
| 18:45:26.050 | 402 detected again, **150 sats**, new payment minted `de1b4287…` |
| 18:45:32.153 | paid retry #2 → ERR_ABORTED again → `NOT broadcasting (funds preserved)` |
| 18:45:32.225 | 402 detected again, **100 sats**, new payment minted `4e08b460…` |
| 18:46:06.821 | paid retry #3 → **status=200, 11,842 bytes** |
| 18:46:07.750 | `broadcast-nosend OK for txid=4e08b460…` |

**≈41 seconds, three payments minted, one broadcast.**

## Three separate problems

### 1. 🔴 The retry aborts, twice, before it works

`ERR_ABORTED` is not a server timeout — it is our own request being cancelled. The sequence each time
is: pay → *"registered paid retry; triggering reload to install handler"* → reload → handler issues
the paid request → aborted. ⚠️ **Hypothesis, NOT yet proven:** the reload that installs the next
handler cancels the request the previous one has in flight, so the first two attempts kill each other
and only the third survives. ⛔ Do not fix from this paragraph — instrument the abort and confirm who
cancels, because "reload races handler" is exactly the kind of plausible story that turns out to be
something else.

### 2. 🔴 The user gets NO feedback for the whole wait

A blank page for 30+ seconds while money moves is the worst possible state on this path. The user
cannot tell "paying" from "broken", and the only thing that eventually appeared was the article.
⚠️ A `/payment-pending` route exists (`App.tsx`, rendered behind the domain-approval modal) and a
`/payment-failed` route is registered by the failure path — neither reached the screen here. Find out
why before adding anything new.

### 3. 🟠 Each attempt mints a payment, and the abandoned ones leak

Three `createAction`s for one purchase. The two abandoned ones are correct about the money — both
logged `NOT broadcasting (funds preserved)` and neither reached the network — but each left:

- a transaction row stuck at `status = nosend`, and
- a **spendable output** for a transaction that will never exist.

📏 After the session: `de1b4287…` and `f56ef113…` both `nosend` with 1 spendable output each, and
1,419,268 sats still reserved.

⭐ **This is the same shape as the phantom coin fixed earlier today** in
`handlers.rs :: release_unbroadcast_transaction`, but a different call site: that fix covers the
refusal path and `abortAction`, not the BRC-121 retry. The retry knows, at
`NOT broadcasting (funds preserved)`, that its transaction will never exist — exactly the condition
that makes cleanup provably safe. ⇒ it should call the same release.

## What DID work, and should not be lost

- ⭐ **No money was lost.** Two payments were abandoned and neither was broadcast. The guard worked.
- The 402 detection, payment minting, paid retry and `broadcast-nosend` chain all function.
- The **gold pill fires from this path too**:
  `💰 OnWalletCallSuccess fired (0 cents from now.bsvblockchain.tech, cefBrowserId=19 → tabId=8, endpoint=pay402)`
  — the paid-retry half of `R-GOLD`, which had never been observed. The identifiers differ again
  (19 → 8), and it resolved to the article's own tab.

## Cross-references

- `PAYMENT_TEST_BATCH.md` M1 — the row this closes, and the row this ticket came out of.
- `HttpRequestInterceptor.cpp` — `TryHandleBrc121_402`, `Async402ResourceHandler`,
  `InstallAsync402HandlerIfPending`, `firePaymentSuccessIpc`.
- `handlers.rs :: release_unbroadcast_transaction` — the cleanup this path should reuse.
