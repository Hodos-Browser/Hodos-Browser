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
| `P11-11-A2` | Past the 10 s threshold the banner states the site is slow and offers Stop; the request stays alive and still completes | Set the threshold above the stub delay ⇒ the escalated copy never appears | The banner's rendered text **and** that the upstream request is still in flight afterwards (it must not be cancelled by its own warning) | T2 | ⬜ |
| `P11-11-A3` | With a payment in flight, a second navigation to the same URL mints **no** second payment — the existing one is reused | Revert to the pre-fix path ⇒ **two** `createAction`s / two `nosend` rows for one article, reproducing 2026-09-16 | ⭐ **Count of `nosend` rows + `createAction` calls for that URL**, not an HTTP status. ⚠️ The 2026-09-16 run is the naturally-occurring RED; reproduce it deliberately | T2 | ⬜ |
| `P11-11-A4` | After an abandoned retry: zero `nosend` rows, zero spendable phantom outputs for that txid, reservation released | Remove the `release_unbroadcast_transaction` call ⇒ the row, the phantom output and the held reservation all persist (📏 measured 2026-09-16: 2 rows, 1 spendable output each, 1,419,268 sats reserved) | The wallet DB `transactions` + `outputs` rows and the reservation, **not** the log's "funds preserved" line — that line was already true while the leak existed | T2 | ⬜ |
| `P11-11-A5` | 👤 A real 402 paywall, human watching: banner appears, article arrives, gold pill on the paying tab, **one** payment | Two-sided with `A3`: click again mid-flight ⇒ still exactly one broadcast txid | ⭐ **The owner's eyes + one txid on WhatsOnChain.** The 2026-09-16 sitting is why this row exists — 4 reviewers found none of the 3 defects a person clicking found | T3 | ⬜ |
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
