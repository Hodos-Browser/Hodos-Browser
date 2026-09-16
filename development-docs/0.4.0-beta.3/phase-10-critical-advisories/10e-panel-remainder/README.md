# Phase 10e — what the adversarial panel and the human sitting left open

**Opened:** 2026-09-16 · **Owner:** Matthew Archbold · **Platforms:** Rust + React (no C++ expected)
**Source:** `../ADVERSARIAL_PANEL.md` (four-reviewer panel, 2026-09-15) and its addendum (the human
sitting, 2026-09-16). **Standard:** `../../HARNESS.md`.

> 👤 **Owner decision 2026-09-16:** these stay in **Phase 10**, not 11/12/13. Each one is the
> unfinished part of a sub-phase that already exists — the ownership check *is* CU-6, and the consent
> stall *is* CU-1's surface — and Phase 11 (UI leftovers), 12 (adblock redirects) and 13 (bot
> detection) are not homes for money-path or consent-path defects.

---

## The work, in the order I'd do it

### 1. ✅ DONE 2026-09-16 — `internalizeAction`'s basket-insertion arm is ungated · `F1-10a` / `F2-10a`

> ✅ `07c69de` (ownership check; RED measured at **+99,637,619 sats and HTTP 200**) and `14a553a`
> (balance excludes non-default baskets; measured live −9 sats, all token carriers). `P10a-A6` re-closed.
> 👤 Owner decided to leave derivation-less outputs counted — see the commit for why, and for the
> dormant `recover_change_index_pure` that is the right answer if such a row ever appears.

**This is the one that matters.** An already-approved dApp can post any mined transaction with a
`basket insertion` spec and the wallet stores a **stranger's output** as its own.

⛔ **The obvious fix is already in place and does not help.** `internalize_action` *does* check the
chain before storing anything (`handlers.rs :: check_tx_exists_on_chain`, before the basket loop; if
the transaction is absent it broadcasts it rather than rejecting). The attack uses a **real, mined**
transaction belonging to someone else, so that check passes honestly. The missing question is not
*"is this real?"* but **"is this ours?"** — which the `wallet payment` arm already asks and the basket
arm does not.

Two independent halves; ⭐ **separate commits**, because the second changes a number the user reads:

| | Change | Why |
|---|---|---|
| **a** | Require the output to be ours before inserting a basket output — the same derived-address verification the `wallet payment` arm performs | Stops the write. Also lets `ERR_NO_OUTPUTS_OWNED` fire, which today it cannot, because the basket loop adds to `total_received` **before** that gate |
| **b** | Exclude non-default baskets and derivation-less rows from `calculate_balance` | 👤 Owner: *"if it's in a basket, it should almost never affect the balance."* Coin selection already excludes both (`get_spendable_by_user`: `derivation_prefix IS NOT NULL` **and** `basket_id IS NULL OR b.name = 'default'`); the balance excludes neither, so a basketed output is displayed as money that can never be spent |

⚠️ **Do not make (b) identical to `get_spendable_by_user`.** The balance deliberately **includes**
`nosend` change so the user sees expected change after an overlay/nosend transaction
(`output_repo.rs :: calculate_balance`'s own comment). Add the basket and derivation filters only.

⚠️ Measure the displayed balance **before and after** (b) on the dev wallet and record both numbers.
A balance that moves for a reason nobody wrote down is its own incident.

~~`P10a-A6` stays **not GREEN** until (a) lands and its probe grows a `basket insertion` arm.~~ ✅ Both done.

### 2. ✅ DONE 2026-09-16 — one site can blank the consent surface for ten minutes · `F2-10b`

> ✅ `d2c1e0a`. Measured two-sided with two real origins over CDP, no human click: pre-fix the second
> origin's connect prompt REPLACED the payment prompt and answering it left the overlay **empty** with
> **zero** queue-advance lines; post-fix it is *"queued behind the prompt on screen"* and advances when
> the first is answered. All three connect openers now share one `enqueueConnectPrompt`.

The three connect openers bypass `enqueuePrompt` and post their modal unconditionally, while the
queue's "may I show the next one?" test (`anyLiveShownLocked`) is **global**. So a second origin — an
iframe to a domain the page controls is enough — raises a connect prompt that paints over a shown
payment prompt, and once the user answers it the queue sees the painted-over entry still flagged
shown and refuses to advance. Everything waits, invisibly, until that entry's 10-minute expiry.

Fails closed, so it is a consent **outage**, not a spend bypass. Fix is routing the connect openers
through `enqueuePrompt` so one mechanism owns what is on screen. ⚠️ C++, shared, so it needs a Mac
relay note.

### 3. 🟠 The *"1 of N"* line never appears in the common case · new, from the human sitting

`queuedFromSite` is computed once at enqueue/take time and frozen into the modal, so the **first**
prompt of a burst always shows 0 and a **two**-request burst can never show the line at all. 👤 The
owner asked for that line specifically in the 10b design decision. Informational only.

Likely same fix as item 2: if one mechanism owns the screen, it can also recount on each show.

### 4. 🟡 A transport margin on the PeerPay size check · `F2-10d`

`wire_body_len` is exact for `message.body`, but the relay also runs `bodyParser.json({ limit: 1 MiB })`
over the **whole** request, and our body sits inside a ~223-byte wrapper. There is a narrow window
where we say OK and the relay returns 413 **after** the broadcast.

⛔ **Rule 6:** this moves the boundary that `a2_boundary_matches_the_server_rule` pins, so the margin
and that test's baseline do not land in the same commit as anything else.

### 5. 🟡 Small, cheap, and easy to lose

- `F3-10b` — `isConnectPromptType` and `isDomainTrustPrompt` disagree about `brc100_auth`. Currently
  unreachable (`PromptType::Brc100Auth` is never constructed) but **one enum arm from re-opening
  CU-1**. Either delete the arm or add the type to `isDomainTrustPrompt`.
- `F3-10a` — the rejected-sender dedupe is keyed on sender only and is **permanent, not per session**,
  so a dismissed sender can never notify again while a fresh key costs an attacker nothing. 👤 Owner's
  call on how loud this surface should be.
- `F9-10d` — four docs cite `main.rs :: enforce_dev_prod_isolation`; the function is
  `enforce_dev_safeguard`.
- Uneven gap between the two ghost buttons in the Activity row (different internal padding). 👤 Owner
  accepted it for now.

## Explicitly NOT in 10e

- **`F4-10b`**, the per-domain session-cap reset — pre-existing, and CU-7's persisted ledger in beta.4
  is the real fix. Add the multi-domain arm to `a6_one_lock…` when that lands.
- **A detail view for rejected incoming payments.** 👤 Owner and I agreed: a rejected attempt has no
  txid and no amount, so there is nothing to link to today. Giving refusals somewhere to live, with
  the sender key and a timestamp, is a real feature for beta.4 — not a button bolted on now.
- **`F8-10b`**, one Approve triggering N BRC-121 reloads. Each is within the user's own limits and each
  was a navigation the user made. Recorded, not scheduled.

## Owed evidence that is not code

- ✅ `HUMAN_TEST_QUEUE.md` **W7** — PASSED 2026-09-16. ⚠️ It exercised the IPC path, which already popped
  correctly; the `F1-10b` fix was for the **HTTP** path, so that half still wants a run.
- `HUMAN_TEST_QUEUE.md` **W4** (display-scaling matrix, belongs at the RC) and **W5** (gold pill +
  session counters, needs the funded sitting).
- `PAYMENT_TEST_BATCH.md` **M11** — a real third-party paymail handle, now also carrying the real
  claim-block assertion (`F3-10d`, half-answered by the human sitting).
- `REGRESSION_SET.md` — R-GOLD, R-COUNT, R-PERIM's T2 arm and R-CLOSE, owed at this boundary as at
  the previous four.
