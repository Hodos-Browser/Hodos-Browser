# Phase 10b — one Approve resolves one request, everywhere a call can arrive · PHASE CONTRACT

**Workstream:** critical advisories (money path) · **Source:** `../../CRITICAL_UPDATES.md` §1.1 (CU-1) + §2 (CU-8, CU-9)
**Status:** ⬜ NOT STARTED — stub from the template; the kickoff fills `D-n`, verifies every symbol, and turns each ⬜ into a run.
**Opened:** 2026-09-15 · **Owner:** Matthew Archbold · **Platforms:** C++ shared (`HttpRequestInterceptor.cpp`) + React + Rust — both platforms; Mac rebuilds and eyeballs the modal
**Standard:** `../../HARNESS.md`.

---

## 0. Plan-vs-tree delta — filled at kickoff

`D-1`..: re-read `permission_service/state.rs` (single-use, body-bound approval ids), `HttpRequestInterceptor.cpp
:: tryHandlePendingResponse` / `openPaymentConfirmationModal` / `handleAuthResponse` / `popAllForDomain` /
`addRequestIfFirstForDomain`, `simple_handler.cpp :: getRequestIdForDomain` fallback, `BRC100AuthOverlayRoot.tsx`'s
Approve payload, `request_gate.rs` token acceptance and `dispatch_payment_with_amount`'s snapshot/record locking,
`handlers.rs :: pay_402` reuse-cache key. Enumerate **every** arrival path (IPC `wallet_call`, HTTP BRC-100
surface, BRC-121 retry, internal paymail/PeerPay) — decision 3.

## 1. Goal

One click approves exactly the request whose amount the user saw; a burst that crosses a limit produces
prompts the user can understand, never N silent signatures behind one prompt; and payments within the user's
limits are as silent as they are today.

## 2. Done means

- [ ] Every approve/deny message carries the `requestId` it answers; a message without one is rejected
- [ ] `popAllForDomain` fan-out remains **only** for connect-type prompts (`domain_approval`, `brc100_auth`, `manifest_connect_bundle`), where siblings are re-issued *without* a token and re-evaluated
- [ ] Kind prompts (payment, rate-limit, protocol, certificate, key) never resume a sibling with its own token
- [ ] Superseded prompts are handled per the design decision below — the user sees the burst **once**
- [ ] The counter snapshot, decision and record happen under one write lock (CU-8)
- [ ] The 402 reuse cache key includes the requesting domain and the server key (CU-9)

## 2a. The design question — the owner's burst requirement, made concrete

👤 Owner: fix bursts everywhere; do not spam the user; still answer the question by notifying the user once.

How the auto-approve engine and a burst meet today (from `CRITICAL_UPDATES.md` §1.1, verified at kickoff):

- **Within limits, a burst is already silent and stays silent.** Five $0.50 payments under a $1 per-tx cap and
  a $10 session cap ⇒ five `Silent` decisions, five 200s, no modal. Nothing here changes that; row `P10b-A3`
  guards it. Legitimate bursts (a site buying N resources at once, a batch payer) are exactly this case.
- **The defect is the burst that *crosses* a limit.** Session cap $10, five $3 payments: three go silent, the
  fourth and fifth each return 202 and each opens a modal; the second modal *replaces* the first on screen. One
  click, and both are signed because each sibling replays its own token. The user saw one amount.
- CU-8 widens it: concurrent calls can all read the counters before any records, so more than three go silent.

Options for what the user sees when a burst crosses a limit, for the owner to pick at kickoff:

| # | Shape | User sees | Site sees | Notes |
|---|---|---|---|---|
| **Q** | **Queue** kind prompts per domain; show the next when the current resolves | one modal at a time, each with its own amount, N modals for N over-limit requests | each request waits for its own answer | kindest to legitimate batch payers; N clicks for N over-limit payments is *correct*, not spam — each is real money |
| **D** | **Deny superseded** prompts immediately with a clear error; the site retries | one modal (the latest); earlier ones get a denial | earlier requests fail fast with "superseded"; a retry goes through the engine fresh | simplest, hardest to abuse; a site that batches must retry |
| **S** | **Summarise**: one modal listing all pending requests for the domain with their amounts and a total; Approve signs all *listed*, Deny denies all | one modal per burst, with the full list | all wait for one answer | matches "tell the user once"; the approval must bind to the *set* (a fresh request arriving after the modal opened is **not** included), and the modal must be re-rendered if the set changes |

⭐ Recommendation to discuss at kickoff: **S with a cap** (list up to N, deny the rest with "retry"), because it is
the only shape where "one notification per burst" and "one click never signs something the user did not see"
are both true. Q is the fallback if S's set-binding proves fragile. D is the fallback if either slips.
⛔ Whatever is chosen, the invariant is the same: **a signature exists only for an amount the user saw on the
modal that produced the click.**

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-INTEXT` | internal never prompts, external always gates | this rewrites the prompt/resume path; the T2 halves **must run**, not be code-read |
| `R-COUNT` | session counters reset on tab close by design | CU-8 changes the locking around the counters; the reset behaviour must be unchanged (CU-7's persisted ledger is *not* this phase) |
| `R-GOLD` | gold pill on every auto-approved payment | the silent path is untouched, but the resume path emits into the same `OnWalletCallSuccess`; verify the pill still fires once per approved request, on the right tab |
| `R-PERIM` | the four privacy gates | key-reveal and cert prompts share the fan-out being removed; each must still prompt and resolve singly |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — seen, and how | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P10b-A1` | Test dApp posts **2** `createAction` calls over the per-tx cap concurrently; one Approve ⇒ exactly **1** txid, the amount shown | Pre-fix, same page, same click ⇒ **2** txids (real money, cents; `PAYMENT_TEST_BATCH.md`) | Rust audit log lines + WhatsOnChain for both txids; the modal's rendered amount captured (CDP on the notification overlay) | T2 | ⬜ |
| `P10b-A2` | The second request is either shown next (Q), listed (S) or denied (D) — never broadcast without its own visible approval | — (this is A1's second half; pre-fix it *is* broadcast) | same | T2 | ⬜ |
| `P10b-A3` | **Non-regression:** 5 concurrent payments all within per-tx, session and rate limits ⇒ 5 silent txids, **0** modals, 5 gold pills | Lower the per-tx cap below the amount ⇒ the same 5 now prompt (proves the row can see a modal) | audit `payment.auto_approved` ×5; overlay never shown (log) | T2 | ⬜ |
| `P10b-A4` | **Non-regression (connect):** a fresh site fires 3 calls before connecting ⇒ **1** connect modal; after Approve all 3 are re-evaluated fresh | Pre-fix behaviour is the same (this is the model being kept); RED = stub `addRequestIfFirstForDomain` to always add ⇒ 3 modals | log of `addRequestIfFirstForDomain` decisions | T2 | ⬜ |
| `P10b-A5` | Approve message **without** `requestId` ⇒ rejected, nothing resolved | Pre-fix ⇒ resolved via the domain fallback | injected message via CDP on the overlay; C++ log line | T2 | ⬜ |
| `P10b-A6` | CU-8: 20 concurrent within-limit calls whose *sum* crosses the session cap ⇒ exactly the count that fits is silent, the rest prompt; never more silent than the cap allows | Pre-fix: more than the cap's worth go silent (race observed with a counter dump) | Rust counters read under the lock after the burst | T1 (engine test) + T2 | ⬜ |
| `P10b-A7` | CU-9: two connected sites request the same 402 URL + sats within 25 s ⇒ the second gets its own signed BEEF (or a denial), never the first site's | Pre-fix: the same unbroadcast BEEF is handed to the second site | cache key logged; BEEF bytes compared | T1/T2 | ⬜ |
| `P10b-A8` | Every arrival path (IPC, HTTP BRC-100, BRC-121 retry, internal paymail/PeerPay) is enumerated and each is shown to carry the `requestId` binding | A path left on the fallback is the RED — the enumeration must find zero | grep + one probe per path | T0/T2 | ⬜ |

**Two-sided rows:** A1 (over-limit ⇒ one signature per click) and A3 (within-limit ⇒ silent) are each other's
control, and together become `R-ONE-CLICK-ONE-SPEND` in `REGRESSION_SET.md` (own commit, rule 6).

## 5. Blast radius

`HttpRequestInterceptor.cpp` — the pending-request registry and every prompt kind's resume; `simple_handler.cpp`
IPC arm for `brc100_auth_response` and siblings; `BRC100AuthOverlayRoot.tsx` (every approve/deny sender; also the
DPI/modal-height work of 7a — a list-shaped modal must survive the small-screen row); Rust `request_gate.rs`
locking; `pay_402` cache. 🍎 Shared C++ + React ⇒ relay row, Mac eyes on the modal.

## 6. Out of scope

CU-7 (persisted spend ledger, rounding cents up) — decided at kickoff, likely beta.4. The loopback caller-auth
item. Changing default limits.

## 7. Rollback

Three commits (C++/React binding + fan-out; Rust lock; Rust cache key), each independently revertible; no schema.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1 -Full` — result + date below
- [ ] `scripts/preflight.ps1 -NegativeControl` — every T0 gate seen to fail
- [ ] `../../REGRESSION_SET.md` run at this boundary — T2 halves run
- [ ] `R-ONE-CLICK-ONE-SPEND` added in its own commit after the fix
- [ ] Adversarial review — four questions in writing
- [ ] Commit messages cite the row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight -Full | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
