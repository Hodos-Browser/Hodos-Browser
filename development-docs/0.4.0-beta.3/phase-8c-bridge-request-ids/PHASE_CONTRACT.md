# Phase 8c — per-request ids for the wallet bridge · PHASE CONTRACT

**Workstream:** money-path correctness · **Ticket:** `TICKET_bridge_single_slot_callbacks_race.md`
**Status:** 🟢 **STAGES 1 + 2 DONE. STAGE 3 IN PROGRESS — batch 1 landed.**
⛔ **5 of 41 migrated** — `getStatus`, `sendTransaction`, `getBalance`, `getBackupModalState`,
`setBackupModalState`. **36 legacy slots remain.** `P8c-A2` (two successful sends ⇒ two txids) still
needs real money: `M8` in `../PAYMENT_TEST_BATCH.md`.
👤 **Owner, 2026-09-08:** *"Let's go with your recommendation of Option B."* — the 41 slots converge on
the **C++-side promise map** already shipping for the history API. ⛔ The ticket's proposed JS-side
`Map<id,{resolve,reject}>` is **not** what gets built; see §0.3 for why.
⚠️ Owner also noted *"this seems like a big change now"* — which is why §7's staging is not optional.
Stage 1 must land and be reviewed before stages 2–4 are attempted.
**Opened:** 2026-09-08 · **Owner:** Matthew Archbold · **Platforms:** Windows + macOS (shared C++ / TS)
**Standard:** `../HARNESS.md`. **Base:** `c68ed57`.

> ⭐ **Stage 1 landed 2026-09-08** (mechanism + `wallet.getStatus`, §4) and **stage 2 on 2026-09-09**
> (`wallet.sendTransaction`, §4b). 🚧 **Stage 3 batch 1 on 2026-09-09** (§4c). ⛔ **36 legacy slots
> remain**; stage 4 (deleting the `window.on*` declarations wholesale) not started.

---

## 👤 0a. OWED TO THE OWNER AT PHASE CLOSE — the standing register

> Asked for explicitly, 2026-09-09: *"keep track of anything you will need me to get eyes on at the
> end of the full phase."* ⛔ **Append here the moment something is found.** A decision that lives
> only in a commit message is a decision nobody will find.

| # | Needs your eyes on | Why it cannot be settled by me | State |
|---|---|---|---|
| **O1** | **`P8c-A2` — two *successful* sends produce two txids.** `M8` in `../PAYMENT_TEST_BATCH.md` | Needs **real money**, twice. Its RED (apply `getBalance`'s dedupe ⇒ one send) is the row that catches the worst possible way to finish this phase | ⬜ owed |
| **O2** | **A genuinely *late* reply** — one that arrives *after* the deadline already rejected — is discarded, not misrouted | The deadline test proved "reply never arrives". "Arrives late" is a different path and needs an induced delay. Free to run; just not run yet | ⬜ owed |
| **O3** | **`create_transaction` is orphaned** — full C++ round trip, zero JS callers (`D-6`) | Deleting live-looking C++ is a call I should not make alone. Options: delete, keep as future API, or migrate for completeness | ⬜ your call |
| **O4** | **How many of the 41 do we actually migrate?** | Several (bookmark folder CRUD, cache size) are UI-driven and unlikely to race. My lean is *all* — a slot left behind is a slot the next person copies — but it is real work for little risk reduction | ⬜ your call |
| **O5** | **The 30 s deadline value** | Chosen as a backstop, not measured against the slowest real wallet call. If any legitimate call can exceed 30 s, this turns a slow success into a failure | ⬜ needs one real slow-call measurement |
| **O6** | **macOS parity.** Both changed files (`simple_render_process_handler.cpp`, `simple_handler.cpp`) are **shared**, not Windows-only | Unlike 8a/8b this is not Rust-only. Mac must verify the V8 binding and the promise map behave there | ⬜ relay owed |
| **O7** | ⚠️ **RED controls are a wasting asset.** `getBackupModalState` WAS the control for `P8c-A1a`; it is now migrated, and batch 1 used `getInfo` instead for `P8c-A1a` — the un-migrated sibling proving the harness detects the bug. Once migrated, a future RED needs a *different* legacy method | Only matters while un-migrated slots remain; ⛔ **at 0 remaining there is no legacy control left at all**, and the RED must become a deliberate stub | ⚠️ live |

## 0. Plan-vs-tree delta

### 0.1 ✅ `D-1` — the race is real, measured, and `getBalance` is genuinely fixed

Confirmed in `initWindowBridge.ts:436-470`: `getBalance` dedupes in flight, with a comment that
already says *"⚠️ sendTransaction and the other single-slot callbacks in this file have the same
shape. NOT fixed here on purpose."* The ticket's 1-in-3 measurement stands.

### 0.2 📏 `D-2` — the size, measured

| | Count |
|---|---|
| Distinct `window.on*Response` slots in `initWindowBridge.ts` | **41** |
| Distinct emit sites in C++ | **56** |
| `initWindowBridge.ts` total | **1,181 lines** |

⇒ The ticket's *"it wants its own phase"* is correct, and it may want **staging** inside that phase.
This is not a sitting.

### 0.3 🚨 `D-3` — **there are already three bridge patterns, and the ticket proposes a fourth**

The reuse-first audit (CLAUDE.md kickoff step 3) says prove the equivalent does not already exist.
It does — twice.

| # | Pattern | Where | Shape |
|---|---|---|---|
| 1 | **Single-slot globals** | `initWindowBridge.ts` + `simple_render_process_handler.cpp` | `window.on<Method>Response`, one slot per **method**. ⛔ The racy one, 41 of them |
| 2 | **`wallet_call` / `wallet_response`** | `services/walletApi.ts :: walletFetch` → `window.__hodos_walletCall` | Used by first-party wallet pages. Carries a request id; replaced direct `fetch` so dev and installed browsers can coexist |
| 3 | ⭐ **C++-side promise map** | `simple_render_process_handler.cpp:240-300`, the **history** API | `std::map<int, PendingHistoryRequest>` holding a real **V8 promise + its owning context**, resolved by id via `ResolveHistoryRequest`, with an explicit *"context gone"* check |

**The ticket proposes:** JS generates an id, keeps `Map<id, {resolve, reject}>`, C++ echoes it back,
JS dispatches. That is a **fourth** mechanism, and it is the weakest of the four — it keeps a
JS-side registry that leaks if the page navigates mid-request, which pattern 3 already handles by
holding the promise in C++ and checking `context->IsValid()`.

⇒ ⭐ **Pattern 3 is the reuse answer**: no `window.on*` global at all, no JS registry, promise
resolution happens inside the owning V8 context, and death-of-context is handled. It is already
shipping in this exact file.

### 0.4 ⚠️ `D-4` — the dedupe fix must not spread, and the contract must say so loudly

`getBalance`'s in-flight dedupe is correct **only** because a balance read is idempotent.
⛔ **Deduping `sendTransaction` would collapse two distinct payments into one** — worse than the bug
being fixed. This is the single most dangerous way to "finish" this phase, and it is exactly what a
careless pattern-match across 41 slots would produce.

## 1. Goal

No bridge reply is ever delivered to the wrong caller, and no caller reports a failure that did not
happen.

## 2. Done means (draft — depends on §8)

- [ ] No method resolves another method's — or another call's — reply.
- [ ] **Two concurrent `sendTransaction` calls produce two sends, not one.** ⛔ The row that catches
      a careless dedupe.
- [ ] A reply arriving after its caller timed out is **discarded**, not misrouted.
- [ ] The `window.on*Response` globals are gone (or reduced to a named, documented remainder).

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-INTEXT` | Internal never prompts, external always gates | ⚠️ **The discriminator rides this bridge.** `wallet_call`'s arm re-derives the frame origin in the browser process. Any re-plumbing must not let a renderer influence which origin is attributed |
| `R-GOLD` | Gold pill on auto-approved payment | The pill is emitted on wallet-call success. Re-routing replies re-routes the pill's trigger |
| `R-CLOSE` | Overlay close guards | `wallet_prevent_close` / `wallet_allow_close` are IPC on this surface |
| `R-DUST` | No incidental 1-sat spend | `sendTransaction` is in scope |

## 4. Evidence — stage 1 measured live, 2026-09-08

⭐ **The concurrency test is FREE.** `wallet.getStatus` is read-only, so unlike the payment batch this
needed only the dev stack. Run against a freshly built dev browser, hard-reloaded first.

⚠️ **Subject, asserted before measuring** — 11 CDP targets all report `type:"page"` (header + 10
overlays), the trap that faked an "intermittent per-session bug" earlier in 0.4.0:

| Check | Result |
|---|---|
| Target URL | `http://127.0.0.1:5137/` — the header, where `initWindowBridge` runs |
| `hodosBrowser.bridge.getStatus` exists | ✅ |
| …and is genuinely native | ✅ `toString()` contains `[native code]` — not a JS shim |
| `window.onWalletStatusResponse` | ✅ **gone** |

### `P8c-A1a` — 🟢🔴 GREEN **and** RED, same harness, both observed

The RED is not a stub or a reverted patch: it is **the identical experiment run against an
un-migrated sibling** (`getBackupModalState`), which still uses the legacy single-slot path. Same
page, same probe, same moment.

| | Migrated (`getStatus`) | Legacy (`getBackupModalState`) |
|---|---|---|
| 3 concurrent calls | **3 / 3 fulfilled, correct** | 1 / 3 correct |
| Wall clock | **9 ms** | **10,815 ms** |
| Values | `{"exists":true,"needsBackup":false}` ×3 | 🚨 `null`, `null`, `{"shown":false}` |

⇒ **A test that could not pass with the feature removed**, because the un-migrated method *is* the
feature removed.

### 🚨 `D-5` — the harm is worse than the ticket says, and it was measured

The ticket describes the losing caller as one that *"waits out its own timeout and reports a failure
that never happened."* For `getBackupModalState` that is **not** what happens:

```ts
const timeout = setTimeout(() => { delete window.onGetBackupModalStateResponse;
                                   resolve(null); }, 10000);   // ⛔ resolve, not reject
```

It **resolves `null`**. The losing callers get a **silently wrong value**, not an error — no rejection,
nothing in the console, nothing to catch. A caller doing `if (state?.shown)` takes the wrong branch
with no signal anywhere. ⇒ Every legacy slot must be checked for resolve-on-timeout while migrating;
"reports a spurious failure" understates the class.

### `P8c-A1b` — 🟢🔴 the **deadline**, measured. Owner-approved addition, 2026-09-08

👤 *"yes I am happy with bridge and option C, add the deadline now before stage 2."*

**Why it was needed.** Migrating dropped the legacy per-method `setTimeout`. The history pattern has
no timeout either — so without this, an unanswered call would hang its caller **forever** *and* leak
its `s_pendingBridgeCalls` entry for the life of the process. `App.tsx:102` awaits
`wallet.getStatus()` during startup, so that is a silent boot stall, not a visible error.

**Design.** `CefPostDelayedTask(TID_RENDERER, new BridgeCallDeadlineTask(id), 30s)`, armed at the call
site. ⭐ Safe by construction when the reply wins the race: `TakeBridgeCall` already no-ops on an id
no longer in the map, so a task that fires late finds nothing. No cancellation, no bookkeeping.
Follows the in-tree `EphemeralCookieManager :: GraceExpiredTask` pattern rather than a new one.

**Measured** — timeout temporarily 30s → 3s and `getStatus` pointed at an unhandled IPC name
(`wallet_status_check_DEADLINE_TEST`, uniquely named so a stray match is obvious), so no reply could
arrive:

| | Result |
|---|---|
| 2 concurrent calls | **both rejected at 3,005 ms** |
| Message | `getStatus: timed out after 3s` — carries the method name |
| Semantics | ⭐ **rejected, not `resolve(null)`** — the opposite of the legacy defect in `D-5` |
| Routing on the failure path | each call got **its own** rejection, so per-request routing holds when things go wrong, not just when they go right |

**Reverted, rebuilt, and the GREEN re-observed** — 3/3 fulfilled in 22 ms with the 30 s deadline
armed, and the legacy RED still reproduces (10,006 ms, two `null`s). `grep DEADLINE_TEST` returns 0.

⚠️ **Caught while reverting:** the rebuild failed `LNK1104` because the dev browser still held
`HodosBrowser.dll`. The first `grep -c error` on that build read **0** — the *pipeline's* count, not
the build's exit code. Checking `BUILD_RC` directly is what surfaced it. Same shape as the
`cmake --build … | tail` trap in `HARNESS.md`.

---

## 4b. Stage 2 — the money path, measured 2026-09-09

### ⛔ `D-6` — stage 2 is **`sendTransaction` only**. `createTransaction` is orphaned

§7 planned *"sendTransaction, createTransaction"*. **`create_transaction` has no JS caller.** The C++
round trip is complete and live — browser handler (`simple_handler.cpp :: create_transaction`),
`WalletService::createTransaction` on both platforms, and two render arms
(`create_transaction_response` / `_error`) — but `grep` over `frontend/src` returns **zero** matches
for `create_transaction` or `createTransaction`, and nothing ever sets `onCreateTransactionResponse`.

⇒ Migrating it would be migrating dead code. Stage 2 narrowed to `sendTransaction`.
⚠️ **Reported, not deleted** (working rule 3): removing it is not what this phase came for.

### `P8c-A2a` — 🟢 routing on the money path, **without moving any funds**

⭐ `P8c-A2` (two *successful* sends ⇒ two txids) needs real money and is `M8` in the payment batch.
But the **routing** can be proven for free: three concurrent sends with a deliberately invalid
payload, where the wallet refuses and no transaction is ever built.

| Check | Result |
|---|---|
| `bridge.sendTransaction` is native | ✅ `[native code]` |
| `onSendTransactionResponse` / `onSendTransactionError` | ✅ **both gone** |
| 3 concurrent sends, invalid recipient | **3 / 3 answered, 5 ms** — each got its own reply |

### `P8c-A2b` — 🟢 the **reject** path, and the hoisted id that makes it work

The run above travelled the *success* IPC carrying an error object, so it did **not** exercise
`RejectBridgeCall`. Forced separately by calling the native bridge with a malformed payload, so
`nlohmann::json::parse` throws in the browser process:

| Check | Result |
|---|---|
| 3 concurrent malformed sends | **3 / 3 rejected in 82 ms**, each with its own reason |
| Message | `sendTransaction: [json.exception.parse_error.101] …` — carries the method name |

🚨 **This is the row that proves the one thing I was careful about.** The browser handler reads the
request id **before** the `try`, precisely so the `catch` can echo it. The exception here fires
*inside* the try — so had the id been read after the parse, the catch would have had no id, and all
three callers would have hung to the **30 s deadline** instead of rejecting in 82 ms. Measured, not
reasoned.

### ⭐ The migration also retired a JS-injection site

The old error arm built JavaScript by concatenation:

```cpp
"if (window.onSendTransactionError) { window.onSendTransactionError('" + errorMessage + "'); }"
```

`errorMessage` was pasted **unescaped** into a single-quoted literal — a wallet error containing a
quote would have broken out of it. Routing by id constructs no JavaScript at all, so the hazard is
**deleted rather than escaped**. (Its sibling arm did use `escapeJsonForJs`; the error arm did not.)

---

## 4c. Stage 3 batch 1 — the three remaining *shapes*, measured 2026-09-09

Batch chosen by **shape, not by convenience**: after these, every remaining slot is a variant of
something already proven, so the rest of stage 3 is mechanical rather than exploratory.

| Method | Shape it proves |
|---|---|
| `getBalance` | a read that carried an **in-flight dedupe workaround** — now retired |
| `getBackupModalState` | the **resolve-on-timeout** offender from `D-5` |
| `setBackupModalState` | the first **non-string payload** (`bool` via `SetBool`, not stringified) |

⚠️ `getBalance`'s browser handler replies from an **async lambda** that is deliberately *captureless*
so it can be bound into a CEF task. The id is threaded through as a **parameter**, not a capture —
capturing would have broken that property and the comment that guards it.

### `P8c-A3a` — 🟢🔴 GREEN and RED, same harness

**Subject:** 5 methods report `[native code]`; all four legacy globals for this batch are gone.

| Run | Result |
|---|---|
| `getBalance` ×3 | **3/3 correct, 6 ms** — and these are now **3 real round trips**, not one shared by a dedupe |
| `getBackupModalState` ×3 | **3/3 correct, 1 ms** — previously **2 of 3 returned `null`** |
| `setBackupModalState(true)` ×3 | **3/3 `{"success":true}`, 0 ms** — bool payload lands |
| 🔴 **RED — `getInfo`, still legacy** | **10,013 ms; 2 of 3 REJECTED** `get_wallet_info timed out`, 1 fulfilled |

⭐ **The RED confirms `D-5`'s two flavours.** `getInfo` *rejects* on timeout; `getBackupModalState`
*resolved `null`*. Same single-slot bug, one loud and one silent — which is why every remaining slot
must be read for its timeout behaviour rather than assumed.

⚠️ **Test hygiene:** `setBackupModalState(true)` really does mutate dev state. It was set back to
`false` and re-read to confirm (`{"shown":false}`) before the stack came down.

### Still owed

| ID | | Tier |
|---|---|---|
| `P8c-A2` | Two concurrent `sendTransaction` calls produce **two** on-chain sends. RED: apply `getBalance`'s dedupe ⇒ **one**. ⛔ Subject is two distinct **txids**, not two resolved promises | T2 — **stage 2**, and it belongs in `../PAYMENT_TEST_BATCH.md` |
| `P8c-A3` | A reply arriving after its caller gave up is discarded, not misrouted | 🟢 **exercised as a side effect of `A1b`**: after the deadline rejected both calls their map entries were gone, so the mechanism is the same one proven there. ⚠️ A reply genuinely arriving *late* (rather than never) is still unobserved |

## 5. Blast radius

`frontend/src/bridge/initWindowBridge.ts` (1,181 lines, 41 slots) ·
`cef-native/src/handlers/simple_render_process_handler.cpp` (56 emit sites) · every caller of every
migrated method · ⚠️ **macOS shares both files** — this is not a Windows-only change, unlike 8a/8b.

## 6. Out of scope

The `wallet_call` path itself (pattern 2) — it already routes correctly and is not implicated.

## 7. Staging — REQUIRED, not optional

⛔ A 41-slot big-bang on the money path is the wrong shape. Suggested order:

1. ✅ **DONE** — mechanism built (`WalletBridgeV8Handler`, `s_pendingBridgeCalls`, `ResolveBridgeCall`/`RejectBridgeCall`), `wallet.getStatus` migrated end to end, GREEN/RED measured live (§4). **1 of 41.**
2. ✅ **DONE** — `sendTransaction` migrated, routing + reject path measured (§4b). ⛔ `createTransaction` was found ORPHANED (no JS caller) and excluded — see `D-6`. **2 of 41.**
3. 🚧 **IN PROGRESS.** Batch 1 done — `getBalance`, `getBackupModalState`, `setBackupModalState`, chosen to cover the three remaining SHAPES (§4c). **36 slots remain**, now mechanical. Each batch its own commit, the old global deleted as each lands.
4. `initWindowBridge.ts`'s `window.on*` declarations deleted last, as the proof nothing still uses them.

## 8. The decision — ✅ ANSWERED

**Which mechanism do the 41 slots converge on?**

| | Option A — the ticket's | Option B — pattern 3 (recommended) |
|---|---|---|
| Id lives | JS `Map<id, {resolve, reject}>` | C++ `std::map<int, Pending…>` holding the V8 promise |
| `window.on*` globals | deleted | **never involved** |
| Page navigates mid-request | ⚠️ JS map entry leaks; caller hangs to timeout | ✅ `context->IsValid()` check already drops it |
| New code | a new mechanism | ⭐ **extends one that already ships** (history API) |
| C++ churn | echo an id | build a promise + context per call |
| Risk | familiar, but a 4th pattern in one file | more C++ per method, but one pattern fewer overall |

⭐ **My recommendation: B.** It is the reuse-first answer, it already handles the failure mode
Option A leaves open, and it retires the `window.on*` surface rather than re-plumbing it. The cost is
more C++ per method — real, but paid once per slot and mechanical after the first.

⚠️ **Counter-argument worth weighing:** Option A is a smaller diff per slot and keeps the async shape
in TypeScript where it is easier to read and test. If the intent is to migrate all 41 quickly and
move on, A gets there sooner.

⛔ **DECIDED: Option B** (owner, 2026-09-08). The section above is kept as the record of what was
weighed, not as an open question.

**Second question, smaller:** do all 41 migrate, or only the ones that can actually overlap? Several
(bookmark folder CRUD, cache size) are UI-driven and unlikely to race. ⛔ Recommend migrating all
anyway — a slot left behind is a slot the next person copies — but it is worth a deliberate answer
rather than drift.
