# A paywalled article took 41 seconds, minted three payments, and showed the user nothing

**Found:** 2026-09-16, payment sitting, by the owner reading a real 402-paywalled article at
`now.bsvblockchain.tech`. **Status:** 🔴 OPEN — **diagnosed 2026-09-17, not yet fixed.**
**Severity:** high (user-visible, money path).

> 👤 **Owner, 2026-09-17:** *"It's not okay. We need to figure it out and put it in the plan to fix it
> somewhere."* Diagnosis is done and is below; the fix is scheduled — see **Plan** at the bottom.

> 👤 *"I can tell you on my end that it did not work. The page didn't load. I didn't see anything. I
> didn't get any feedback, nothing."* … then, half a minute later: *"it just now loaded, but that took
> like literally 30 seconds when it should take less than one second."*

⛔ **Correction, 2026-09-17 — this ticket originally said "a blank page for 30+ seconds". It was
wrong, and the difference is the whole diagnosis.** 👤 Owner: *"It didn't show a blank page. It just
stayed on the page. A blank page would have at least been some kind of feedback — not a good one, not
an acceptable one, but the user wouldn't have been able to click the article two more times if it was
on a blank page."*

The page **stayed exactly as it was, fully interactive, with the article links still clickable**. So
the user did the only reasonable thing and clicked again. That is not a cosmetic difference: the
clicks are what produced the aborts and the extra payments, which the first write-up attributed to
our own reload.

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

### 1. 🔴 The retry aborts, twice — because the USER clicked, and we gave them no reason not to

✅ **RESOLVED 2026-09-17 by reading the log the owner's account pointed at.** `debug_output-40428.log`
lines 21703–21893.

⛔ **The original hypothesis — "the reload that installs the next handler cancels the previous
handler's in-flight request" — is REFUTED.** Our own reload is logged (`💰 BRC-121: reloading …`) and
lands **9 ms** after payment, and the handler is installed *on that reload*; it cancels nothing.

The cancels are **user navigations**, and three independent facts say so:

| Evidence | Reading |
|---|---|
| Each abort is preceded, **2 ms earlier**, by a fresh top-level `Resource request … (role: tab_8)` + the site's Cloudflare RUM beacon — with **no `BRC-121: reloading` line before it** | A navigation we did not initiate |
| The gaps are **6.53 s** and **5.75 s** after the paid request was issued | Human reaction time, not a code race |
| Attempts 1 and 2 request `/articles/agentpay-hackathon`; **attempt 3 requests `/articles/chronicle-activates`** | The third click was a **different article**. Code cannot do that |

⇒ Exactly matches the owner's account: clicked, nothing happened, clicked again, nothing happened,
clicked a different article. **Each click cancelled the paid request already in flight**, and the
cancelled attempt's 402 was re-detected and re-paid.

⭐ So the abort is a **symptom of problem 2, not an independent bug.** Nothing cancels these requests
except a user who has been given no indication that anything is happening. Fix the feedback and the
aborts mostly stop existing — but the retry must **still** survive a navigation or clean up after one,
because a user may legitimately click away.

### 1b. 🟢 NOT our fault: the 34-second wait is the paywall site's own origin server

The successful attempt was issued at `18:45:32.292` and answered at `18:46:06.821`. Cloudflare's own
header on that response says where the time went:

```
server-timing: cfEdge;dur=9, cfOrigin;dur=34481
```

**Their edge took 9 ms. Their origin took 34,481 ms.** That is 34.5 of the ~41 seconds, and none of
it is ours. ⛔ Do not attempt to "speed up" the paid retry — there is nothing on our side to speed up.
What we owe the user is **telling them we are waiting**, and a timeout with an honest message if the
wait is unreasonable.

📏 Budget of the ~41 s: **34.5 s the site's origin**, ~6.5 s our first cancelled attempt, ~5.7 s our
second. Our share is real but it is the *repeat* cost, not the wait.

### 2. 🔴 The user gets NO feedback for the whole wait — and this is the ROOT problem

**This is the defect. Everything else in this ticket is downstream of it.**

For ~41 seconds the browser silently spent money three times while showing a page that looked
completely idle and stayed clickable. The user cannot tell *paying* from *broken* from *ignored me*.

✅ **Cause found, 2026-09-17.** `/payment-pending` exists as a route (`frontend/src/App.tsx:173`,
`PaymentPendingPage`) but `frontend/src/CLAUDE.md` states its actual role: *"BRC-121 background
placeholder **rendered behind the domain-approval modal**"*. Every navigation to it in
`HttpRequestInterceptor.cpp` sits on the modal / `OnLoadError` path.

⇒ **On the silent auto-approved path there is no modal, so nothing ever navigates to it.** The run
was `💰 BRC-121 → /wallet/pay402 (0 cents, engine decision)` — auto-approved, no prompt. The pending
screen was never reachable. ⛔ The `PaymentPendingPage.tsx` lines in the log are Vite dev-server
module fetches for the React bundle, **not** the page being displayed; do not read them as evidence
it rendered.

🚨 **The design gap, stated plainly:** we built the "we are paying" screen only for the case where the
user was already looking at a modal and therefore already knew. The case where they are told *nothing*
is exactly the case with no feedback. ⭐ And silent auto-approval is the **common** path — the whole
point of the spending caps.

⚠️ The gold pill is not a substitute: it fires on **success**, at the end, and the owner has already
called it *"a little subtle"*. It says "you paid", not "we are working".

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

## Plan — what to build, in order

⛔ **Fix order follows causation, not severity.** Feedback first: it is the root, and it removes the
clicks that cause the aborts and the extra mints.

| # | Fix | Why this shape |
|---|---|---|
| **1** | **Show the user that a payment is in flight, on the SILENT path too.** Reuse `PaymentPendingPage` — it exists and is built for exactly this; the gap is only that nothing navigates to it without a modal. Must name the site and the amount | Root cause. Kills the re-clicks, which kills problems 3 and most of 1 |
| **2** | **A timeout with an honest message.** After N seconds say the site is slow and offer to keep waiting or stop — do **not** silently re-pay | The 34.5 s was the site's origin. We cannot fix their server; we can stop pretending nothing is happening |
| **3** | **A navigation away from a pending paid retry must clean up, not re-mint.** Cancel-and-release rather than abandon; and an unpaid 402 for a URL we already hold an unbroadcast payment for must **reuse** that payment, never mint a second | Defence in depth: a user may legitimately click away. ⚠️ Pairs with `CU-9`'s `pay402_reuse_key(domain, server_key, url)` — the reuse key already exists |
| **4** | **Release the abandoned transaction.** Call `release_unbroadcast_transaction` at `NOT broadcasting (funds preserved)` | Smallest, and independently correct — see problem 3 above |

**Negative controls** (⛔ each must be *observed* failing first, `HARNESS.md`):
1. Feedback: stub the paid retry to hang 20 s ⇒ **pre-fix** the page sits idle and clickable; **post-fix** the pending screen is up within ~1 s naming site and sats. Subject is **the tab's rendered document**, not a log line.
2. Re-mint: with the pending screen up, navigate away mid-flight ⇒ assert **one** `createAction` for one article, not two. Subject is the count of `nosend` rows, not the HTTP status.
3. Release: after an abandoned attempt, assert **zero** `nosend` rows and **zero** spendable phantom outputs for that txid, and the reservation released.

⚠️ **Where it lands is the owner's call** — see the three options put to them 2026-09-17. It is *not*
a beta.4 item: it is on the shipped money path in beta.3.

## Cross-references

- `PAYMENT_TEST_BATCH.md` M1 — the row this closes, and the row this ticket came out of.
- `debug_output-40428.log` lines 21703–21893 (`%APPDATA%\HodosBrowserDev\logs\`) — the whole sequence,
  including the `cfOrigin;dur=34481` header. ⚠️ Dev logs rotate; copy it before it ages out.
- `frontend/src/pages/PaymentPendingPage.tsx` + `App.tsx:173` — the screen that already exists.
- `HttpRequestInterceptor.cpp` — `TryHandleBrc121_402`, `Async402ResourceHandler`,
  `InstallAsync402HandlerIfPending`, `firePaymentSuccessIpc`.
- `handlers.rs :: release_unbroadcast_transaction` — the cleanup this path should reuse.
