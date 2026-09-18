# Phase 11 item 11 — a BRC-121 payment tells the user it is happening · PHASE CONTRACT

**Workstream:** UI leftovers (money path) · **Ticket:** `../TICKET_brc121_paid_retry_aborts_and_mints_a_payment_each_time.md` · **Status:** ⬜ NOT STARTED
**Opened:** 2026-09-17 · **Owner:** Matthew · **Platforms:** Windows / macOS (both)
**Standard:** `../HARNESS.md`. **Runs FIRST in Phase 11**, ahead of the omnibox cluster.

---

## 0. What the kickoff established (2026-09-17) — read before §1

Measured from `debug_output-40428.log` 21703–21900 (excerpt preserved; ⚠️ dev logs rotate).

**The mechanism, exactly.** `/payment-pending` is reached through **`OnLoadError`**, not by any
deliberate "show the pending screen" call:

| Path | Rust | What happens | Feedback |
|---|---|---|---|
| **Prompt** | 202 | interceptor returns `false` → page sees a real 402 → CEF failed-load page → `OnLoadError` fires → `registerPendingBrc121Reload` marked the domain → swap in `/payment-pending` | ✅ the placeholder + the modal |
| **Silent** | 200 | payment mints → reload → `Async402ResourceHandler` serves the paid content | 🔴 **none — the load never fails, so there is no hook** |

⇒ ⭐ **The pending screen is triggered by a load FAILURE. The silent path succeeds, and success has no
feedback hook.** It is not an oversight in wiring; there is nowhere for it to hang.

⛔ **Correction carried in from the first write-up.** The tab *was* marked loading throughout —
`18:45:18.846 loading… → 18:46:06.866 done`, 48 s unbroken. So a tab throbber **was** spinning. What
was missing is anything saying **money is moving**, and anything **in the viewport**. ⇒ we are not
inventing a loading signal; we are adding the one thing the throbber cannot say.

**Not ours:** 34.5 s of the 41 was the site's own origin (`server-timing: cfEdge;dur=9,
cfOrigin;dur=34481`). ⛔ No work in this phase tries to make the paid retry faster.

**Reuse-first audit (CLAUDE.md kickoff step 3).**

| Candidate | Verdict |
|---|---|
| `PaymentPendingPage` (`frontend/src/pages/`, routed `App.tsx:173`) | ⚠️ Exists, but its copy is **"Waiting for your approval"** — modal-specific and wrong here. Reuse the *route + query-param shape*, not the component as-is |
| Toast / banner component | ❌ None exists in `frontend/src/components/`. Nothing to extend |
| `OnLoadingStateChange` → `TabManager::UpdateTabLoadingState` | ✅ The existing per-tab loading signal. Payment-unaware; do **not** overload it |
| `pay402_reuse_key(domain, server_key, url)` (CU-9) | ✅ Already exists — the key `A3` keys off. Do not invent a second registry |
| `release_unbroadcast_transaction` (`handlers.rs`, landed `271dadb`) | ✅ Already exists — `A4` calls it. Do not write a second cleanup |
| `s_brc121_pending_sats` / `registerPendingBrc121Reload` | ✅ Already carry domain + sats per payment; `A1` reads them |

## 👤 Owner decisions 2026-09-17 (do not re-ask)

1. **Surface = an in-viewport banner, no navigation.** ⛔ The reload → handler-install → paid-request
   chain is the money path and must not be perturbed; a mid-flight navigation is how a paid-for
   article never arrives. Not a full interim page.
2. **Timeout = tell them and keep waiting**, with a Stop. The payment is already minted; abandoning it
   wastes it, and their server did answer at 34.5 s.
3. Item 11 runs **before** items 1–10 and gets this contract to itself.

---

## 1. Goal

When Hodos spends money to load a paywalled page, the user can see that it is happening, on the
**silent auto-approved path** — and clicking again does not spend a second time.

## 2. Done means

- [ ] A silent BRC-121 payment puts a banner **in the tab's viewport** within ~1 s, naming the site and
      the amount, and it clears when the content arrives or the user stops it.
- [ ] Past ~10 s the banner says the site is slow; the request stays alive; a **Stop** is offered.
- [ ] Navigating away from, or re-clicking into, a URL with a payment already in flight does **not**
      mint a second payment — the existing unbroadcast payment is reused.
- [ ] An abandoned paid retry leaves **no** `nosend` row, **no** spendable phantom output and **no**
      held reservation.
- [ ] ⛔ The gold pill still fires on success, on the originating tab, from both paths.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-GOLD` | Gold pill on the **originating** tab for an auto-approved payment | `firePaymentSuccessIpc` lives in the function being edited; `cefBrowserId → tabId` translation is the hazard (12→2, 19→8, 16→6 all observed) |
| `R-ONE-CLICK-ONE-SPEND` | One user action ⇒ one spend | `A3` changes when a 402 mints vs reuses. ⚠️ The direct target of this phase |
| `R-COUNT` | Per-session counters reset on tab close | Banner lifecycle is per-tab; a leaked registry entry could outlive the tab |
| `R-INTEXT` | Internal traffic is not gated as external | New IPC for the banner must not be mistaken for site traffic |
| `R-DUST` | The 1-sat floor | `A4`'s release touches `outputs`; a wrong release could strand or free the wrong coin |

## 4. Evidence table

⛔ No empty RED or SUBJECT cells. ⭐ **Every GREEN half here is free** — a stubbed slow retry
reproduces the whole thing. Only `A5` spends, and it is cents.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P11-11-A1` | With the paid retry stubbed to hang 20 s, the banner is in the viewport within ~1 s naming host + sats | Revert the banner ⇒ the tab renders the **previous document**, unchanged and clickable, for the full 20 s | ⭐ **The tab's rendered document** (screenshot of the content tab), **never** a log line and never the tab throbber — the throbber was already running during the 48 s and proved nothing | T2 | ✅ **2026-09-17** |
| `P11-11-A2` | Past the 10 s threshold the banner states the site is slow and offers Stop; the request stays alive and still completes | Set the threshold above the stub delay ⇒ the escalated copy never appears | The banner's rendered text **and** that the upstream request is still in flight afterwards (it must not be cancelled by its own warning) | T2 | ✅ **2026-09-17** |
| `P11-11-A3` | With a payment in flight, a second navigation to the same URL mints **no** second payment — the existing one is reused | Revert to the pre-fix path ⇒ **two** `createAction`s / two `nosend` rows for one article, reproducing 2026-09-16 | ⭐ **Count of `nosend` rows + `createAction` calls for that URL**, not an HTTP status. ⚠️ The 2026-09-16 run is the naturally-occurring RED; reproduce it deliberately | T2 | ✅ **2026-09-17** (one residual, see below) |
| `P11-11-A4` | After an abandoned retry: zero `nosend` rows, zero spendable phantom outputs for that txid, reservation released | Remove the `release_unbroadcast_transaction` call ⇒ the row, the phantom output and the held reservation all persist (📏 measured 2026-09-16: 2 rows, 1 spendable output each, 1,419,268 sats reserved) | The wallet DB `transactions` + `outputs` rows and the reservation, **not** the log's "funds preserved" line — that line was already true while the leak existed | T2 | ✅ **2026-09-17** |
| `P11-11-A5` | 👤 A real 402 paywall, human watching: banner appears, article arrives, gold pill on the paying tab, **one** payment | Two-sided with `A3`: click again mid-flight ⇒ still exactly one broadcast txid | ⭐ **The owner's eyes + one txid on WhatsOnChain.** The 2026-09-16 sitting is why this row exists — 4 reviewers found none of the 3 defects a person clicking found | T3 | 🟡 **2026-09-17 — pill + reuse PASS, and it found a defect I introduced today** |
| `P11-11-A6` | `R-GOLD` still green both paths after the edit | Stub `firePaymentSuccessIpc`'s tab resolution to the raw `cefBrowserId` ⇒ pill lands on the wrong tab | `cefBrowserId → tabId` in the log **and** which tab shows the pill (they differ — that is the hazard) | T2 | ⬜ |

**Two-sided pairing:** `A3` ⇄ `A5` (must reuse, and must still deliver), `A2` (must warn, and must not
cancel itself).

### `P11-11-A1` — run record, 2026-09-17 (Windows)

**Same article, same seam, same baseline screenshot hash. One variable: the binary.**

| | 🔴 RED (pre-fix) | 🟢 GREEN (post-fix) |
|---|---|---|
| Article | `/articles/tetheral-reserve-bank` | **the same URL** (its one `paid_content` row deleted so it re-paid) |
| Baseline sha256[:16] | `712b0e48db0e6845` (357,778 B) | `712b0e48db0e6845` (357,778 B) — ⭐ identical, so the setup is provably the same |
| During the hold | **15/15 samples byte-identical to baseline** | changed at **t+2.1 s**, `c75880e8971b1abb`, and stayed changed |
| Banner | absent | `● Paying now.bsvblockchain.tech · 200 sats…` |
| Payment real? | 402 → 200 sats, held 20.010 s, `cfOrigin;dur=10896` | 402 → 200 sats, banner at 14:58:22.175, held 20.003 s, txid `9fe12b7367f5c51a…` |

⛔ **Two instrument defects found and fixed before the result was believed:**
1. **`Runtime.evaluate` QUEUES behind the pending navigation.** The first two attempts returned exactly
   one sample, at t+37.6 s and t+32.0 s — the renderer would not answer during the very window under
   test. ⇒ switched to `Page.captureScreenshot`, which the browser process serves.
2. 🚨 **A run was VACUOUS and looked perfect.** `PaidContentCache` served a previously-paid article
   from disk: no 402, no payment, no hold, article on screen at t+1.2 s. A table of 25 healthy-looking
   rows that tested **nothing**. ⇒ every run now uses a URL with no `paid_content` row, verified.

⭐ **Instrument control** (without it, "15 identical frames" could just mean a frozen capture): the
screenshot taken after completion is `5dccbfca96271e94` / 467,590 B — **different**. The instrument
can see change; the identical frames are real.

⚠️ Cost: 4 real payments across the runs (150–200 sats each), dev wallet.

**`R-GOLD` spot-check on the green run:** `OnWalletCallSuccess fired (… cefBrowserId=2 → tabId=1,
endpoint=pay402)` — ids differ and it resolved to the paying tab. Full `A6` row still owed.

**Preflight `-Full`: PASS**, every gate at baseline (G11 59, G12 4 — unchanged). ⛔ `HodosBrowserShell`
also built explicitly; `-Full` does not build it.

### `P11-11-A2` — run record, 2026-09-17 (Windows)

Rig: 25 s hold, real paywall, screenshots read as images.

| | Text on screen |
|---|---|
| **t+5 s** | `● Paying now.bsvblockchain.tech · 75 sats…` |
| **t+15 s** 🟢 | `● Paid now.bsvblockchain.tech · 75 sats. Waiting for the site to send the page — it has been 14s, which is slow for them, not a problem with your payment. Leave this page to stop waiting.` |
| **t+15.5 s** 🔴 | `● Paying now.bsvblockchain.tech · 100 sats…` — threshold recompiled to 300 s, **escalation never appears** |

⭐ **The half that matters most is the second subject, and it passed:** the warning did **not** cancel
the request it warns about. `upstream complete — status=200 bodyBytes=8819`, `cfOrigin;dur=10421`,
then `broadcast-nosend OK for txid=73eab7534e1514…`. A warning that killed a slow-but-succeeding
payment would turn the original defect into a guaranteed failure.

⚠️ The escalated copy deliberately says **"Paid … waiting for the site"** and names the elapsed
seconds. The 34.5 s that started this was the site's origin (`cfOrigin;dur=34481`), so the wording
puts the delay where it belongs and reassures about the money.

### ⚠️ A2 scope narrowed, deliberately — no clickable Stop

The contract said "offers Stop". **There is no Stop button**, and the reason is a privacy trade the
owner should know about rather than discover:

- The banner is **injected into the page's DOM**. A clickable Stop needs page JS to call back into
  C++, which means a new binding on **every** external page.
- `simple_render_process_handler.cpp` deliberately gives external pages **only** the `brc100`
  sub-object, commented *"to minimize fingerprint surface"*. A Hodos-only global function is a
  fingerprinting signal on a browser whose headline feature is fingerprint defence.
- Esc→`StopLoad` does not exist in this shell (checked: no `VK_ESCAPE` / `StopLoad` handler
  anywhere), so there was no existing affordance to point at either.

⇒ The banner says **"Leave this page to stop waiting"**, which is true today: a navigation cancels
the handler, `Cancel()` clears the banner, and `A3` will stop that navigation re-minting a payment.

👤 **DECIDED 2026-09-17 — do not re-ask.** *"I think the leave this page is a better stop control
anyway."* ⇒ "Leave this page to stop waiting" **is** the Stop control. No button, no native overlay
strip, no page-JS binding. A2 is complete as shipped and the row's wording is satisfied.

## 5. Blast radius

- `cef-native/src/core/HttpRequestInterceptor.cpp` — `TryHandleBrc121_402`, `Async402ResourceHandler`,
  `InstallAsync402HandlerIfPending`, `firePaymentSuccessIpc`, `s_brc121_pending_reloads` /
  `_pending_sats` / `_failed_urls`. ⚠️ **Shared C++** ⇒ relay note naming the files (root `CLAUDE.md`
  build rule: nothing compiles C++ on push).
- `rust-wallet/src/handlers.rs` — `pay_402`, `release_unbroadcast_transaction` (call site only; ⛔ no
  schema change, no crypto change — invariants 2 and 3).
- `frontend/src/` — the new banner + whatever it registers in `App.tsx`.
- **Touched but not about:** the prompt path's `/payment-pending` and `/payment-failed` swaps. ⛔ Leave
  them alone; `A1`'s RED must not be confused with breaking the modal path.

## 6. Out of scope

- ⛔ Making the paid retry faster. 34.5 s was their origin. 👤 Owner will raise it with the site.
- The gold pill's visual weight (👤 *"a little subtle, but that's how it is for now"* — revisit on real
  user feedback, not here).
- `/payment-pending`'s copy on the **prompt** path — it is correct there.
- The bulk-UTXO 20/address ticket (own ticket, beta.4 blocker).
- Items 1–10 of Phase 11.

## 7. Rollback

Revert the banner commit (frontend + its IPC) and the interceptor commit; `A3`/`A4` are separable
single-call-site changes and revert independently.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1 -Full` — ⛔⛔ **and `HodosBrowserShell` built explicitly**; `-Full` reports
      PASS while the shell does not compile (it builds `hodos_tests` only)
- [ ] `scripts/preflight.ps1 -NegativeControl`
- [ ] `../REGRESSION_SET.md` at this boundary, incl. the T2 halves
- [ ] 🍎 Mac relay row naming the shared C++ + React files
- [ ] `../HUMAN_TEST_QUEUE.md` row for `A5` landed **in the same commit** that marks it owed


### `P11-11-A3` — run record, 2026-09-17 (Windows)

🚨 **ROOT CAUSE, and it is not what the plan assumed.** The plan said to build reuse keyed on CU-9's
`pay402_reuse_key`. **That cache already existed and had never once worked.** Its liveness check read

```sql
SELECT new_status FROM transactions WHERE txid = ?1 LIMIT 1
```

and **there is no `new_status` column** — the dual `status`/`broadcast_status` pair was collapsed into
a single `status` long ago and this query was never updated. Proven against the live schema:
`no such column: new_status`. The call site ended `.unwrap_or(false)`, so the error became the answer
*"not reusable"* on **every** call, and every retry minted a fresh transaction.

⇒ ⭐ **That is the 2026-09-16 triple-mint.** Attempts 1 and 2 were the same URL, same 150 sats, 6.6 s
apart — squarely inside the 25 s TTL — and still produced two different txids.

**Fixed:** the column, and the arm that hid it. `QueryReturnedNoRows` is now the **only** benign
failure; any other error is logged as a bug and never read as a verdict.

**🟢 GREEN — two navigations to the same URL, 8 s hold:**

| Time | Event |
|---|---|
| 17:14:43.231 | mint `feb48755…`, 849 BEEF bytes |
| 17:14:46.965 | **`pay_402 REUSE: returning existing nosend tx feb48755… (age 3996ms)`** ← the re-click |
| 17:15:02.076 | upstream 200, 14,712 bytes — article delivered |
| 17:15:02.639 | `broadcast-nosend OK` for `feb48755…` — **one** payment |

**🔴 RED observed** at the unit level: restoring `new_status` fails
`a3_the_reuse_status_query_runs_against_the_real_schema` with the exact production error.

**⚠️ RESIDUAL, measured not assumed.** The second navigation still installs its own
`Async402ResourceHandler`, which issues a late upstream request after the first succeeded. Its payment
is by then broadcast, so the server answers 402 and **one extra mint** follows (`2eb513d8…`).
⭐ It is **never broadcast** — no money is lost and the user paid 150 sats once — but it leaves exactly
one orphan `nosend` row. **That row is `A4`'s job**, and this run is a live fixture for it.
⬜ Suppressing the duplicate *handler* (rather than the duplicate mint) is a further step, recorded
here and not done.

**⚠️ Rig finding worth keeping — the freshness window is real.** A first run used a **25 s** hold. The
reused payment was then issued ~33 s after minting, and the server rejected it **402: stale**, against
BRC-121's 30 s `x-bsv-time` window. Our `PAY402_REUSE_TTL_MS` is 25 s, leaving ~5 s for the hold, the
RTT and clock skew. ⇒ the 25 s reuse TTL and the 30 s protocol window are **too close together**, which
is exactly the risk recorded in the x402 research. Not a defect introduced here; a real interaction the
rig surfaced, and an argument for a shorter reuse TTL or a fresh mint past a safety margin.


### `P11-11-A4` — run record, 2026-09-17 (Windows)

**Shipped:** `POST /wallet/release-nosend` — the missing half of the pair the BRC-121 path already had.
It reuses `release_unbroadcast_transaction` (⛔ no third cleanup) and refuses anything that is not
`nosend`: disabling the outputs of a transaction that IS on chain would destroy real coins, so a
`completed`/`sending` txid gets a 409 instead. A row that is already gone is success (`alreadyGone`).

⭐ **The rig had to make the server genuinely refuse**, and the honest way turned out to be the
protocol itself: hold the paid retry **40 s** so the payment exceeds BRC-121's **30 s** `x-bsv-time`
window, and the real server answers **402**. ⚠️ 25 s was *not* enough — the server accepted it — which
is itself a useful datum about where the real margin sits.

| | 🟢 GREEN (release wired) | 🔴 RED (call commented out, rebuilt) |
|---|---|---|
| txid | `9b9d4504…` | `e4ed2cb5…` |
| transaction status | **`failed`** | **`nosend`** |
| its change output | **`spendable = 0`** | **`spendable = 1`** — the phantom coin |
| release log | `1 output(s) disabled, 1 input(s) restored`, 49 ms after the refusal | none — 0 release calls |
| after three attempts | released each time | **3 `nosend` rows accumulated**, one spendable output each |

⭐ **The RED reproduces 2026-09-16 exactly**: the leak compounds per attempt.

### ⭐ The distinction A3 forced, and it is the interesting part of A4

Release fires **only when `status > 0`** — the server answered and refused. On `status == 0` (no HTTP
response: our own request cancelled, which is what a user navigating away produces, and the whole of
the 2026-09-16 incident) we deliberately **keep** the transaction, because that is precisely the one
`A3`'s reuse cache hands back on the re-click. Releasing it would defeat the fix that stops the
re-click minting a second payment.

⇒ three cases, three owners: **`A3`** = the user comes back · **`A4`** = the server said no ·
**the sweepers** = nobody ever comes back.

### ⛔ Found while measuring — and my first reading of it was WRONG

A reservation `b00be69c…:2` (1,419,268 sats, `confirmed = 1`, `spendable = 0`) has been held under a
`pending-` placeholder for 26.9 hours. **I wrote it up as a live leak understating the balance. It is
not.** 👤 The owner escalated on that basis — correctly, given what I had written — and the
investigation settled it in three queries:

| | |
|---|---|
| Is the outpoint unspent? | ❌ **Spent.** The address has **zero** unspent outputs |
| What spent it? | `80d5821f…`, block 967057, 172 confirmations — its `vin[0]` **is** this outpoint |
| What is that transaction? | ⭐ **The `M4` RED** — our own fault-injection run, whose seam deliberately disabled the code that resolves the placeholder |

⇒ **No money lost, none missing from the balance**, and `TaskSweepReservations` declining for 27 hours
is the **correct** answer: releasing a spent coin back to `spendable = 1` is the `P0.7` double-spend
path. ⛔ Do not relax the sweeper.

The narrow real gap — a row at `spendable = 0, spent_by = NULL` that nothing ever reconciles, because
the sweeper can express *"observed unspent ⇒ release"* but not *"observed **spent** ⇒ close it out"* —
is ticketed at `../TICKET_reservation_can_be_held_indefinitely.md`, **severity LOW**, reachable in
production only if all three `create_action` guards fail at once.

⭐ **The lesson worth more than the ticket: ask the chain before calling a wallet state a leak.** One
`curl` would have prevented the whole scare, and I asserted a loss without it.


### `P11-11-A5` — run record, 2026-09-17 (Windows, 👤 owner clicking, no fault seam)

👤 *"The banner did show. I clicked it again. I saw the banner refresh… The page finally did load. It
felt like a long time. It felt like over a minute… I did see the gold pill only once, once the page
did load."*

📏 **40.4 s** first 402 → article. Not impatience: the owner was nearly right.

**🟢 What passed**

- **A1/A2 confirmed by a human**: banner appeared, refreshed on the re-click, and changed wording —
  the escalation, seen in the wild.
- **⭐ A3 confirmed with real money**: the re-click at `18:24:31.893` produced
  `pay_402 REUSE … (age 2532ms)` — **no second mint.** On 2026-09-16 that same click minted a second
  payment. The fix works on the path it was written for.
- **A5's pill assertion**: exactly one pill, `cefBrowserId=15 → tabId=4` — ids differ and it resolved
  to the paying tab.
- One article, **75 sats, paid once**. No money lost.

**🔴 THE FIND — a race between `A3` and `A4`, both landed today, by me**

```
18:24:49.696  wallet  REUSE returns 39357fc3…      ← hands the payment back
18:24:49.700  shell   RELEASED 39357fc3…           ← 4 ms later, marks it failed
18:24:49.705  shell   issues the paid request with 39357fc3…   ← already dead
18:24:59.918  server  402                           ← guaranteed
18:24:59.982  wallet  fresh mint 02f8d9f2…          ← the wasted extra mint
```

`A4` releases a transaction that `A3` handed out 4 ms earlier. The next attempt is then **certain** to
fail, costing a full round trip (~10 s of the owner's 40) and one unnecessary `createAction`.

⛔ **Neither automated row could have caught it.** `A4`'s test used a single navigation, so no reuse
was in play; `A3`'s test had no definitive server refusal inside the window. **It requires a re-click
AND a refusal — which is what a person does and a script did not.** Second time this sprint the human
row has found what the automated ones could not.

⚠️ No money is lost (the extra mint is never broadcast), but it is a real defect on the money path and
it is **mine, from today**.

**Suggested fix (not implemented):** `release_nosend` should evict the `pay402_reuse` entry in the same
lock as the status change, and the release should complete **before** the refused response reaches the
page — today `releaseNosendAsync()` posts to `TID_FILE_USER_BLOCKING` and the response continues
immediately, which is the race. ⇒ ordering, plus belt-and-braces eviction.

**🟠 Also visible:** three attempts for one article. The first 402 at `18:24:49` is likely the
duplicate-handler residual already recorded under `A3` — the second navigation installs its own handler
and issues the **same** payment a second time, which a server may fairly reject as a replay.

**📏 Where the 40 s went:** their origin `17.7 s + 10.2 s + 9.7 s` = **37.6 s across three attempts** —
~93 % theirs. But two of the three attempts should not have happened.

### 👤 The gold pill — owner asked how to make it less subtle (2026-09-17)

Current: **10 px**, bottom edge of the tab, centred, `rgba(166,124,0,.95)`, 6 s `paymentBadgeFade`
(~4 s at full opacity), amount only. 👤 *"a little subtle"* — said twice now, 2026-09-16 and today.

Why it reads as subtle: smallest type in the UI · in the **tab strip** while the user is looking at the
**page** · arrives at the exact moment the article appears and takes the attention · a flash, not a record.

| Option | Cost | Trade |
|---|---|---|
| Bigger type / contrast / dwell / a pulse | minutes | Still in a tab; smallest gain |
| ⭐ **A transient chip in the TOOLBAR, by the address bar** | small | Still browser chrome ⇒ **unsuppressable**, but in the region the eye actually uses. **Recommended** |
| Reuse the A1 banner as a success confirmation | small | Most visible — but the banner is **page DOM**, so a hostile site could suppress the one indicator that says money moved |

⛔ **The tension to hold on to:** the pill's real virtue is that it lives in the **header browser, out of
the page's reach**. Anything moved into the viewport to make it louder trades that away. ⇒ the answer is
not "louder where it is" but "unsuppressable, where the eye already is". And the flash need not carry all
the weight — Activity is the durable record; the pill is the alert.

⬜ Not scheduled. 👤 Owner: *"I don't want to focus on that too much right now."*
