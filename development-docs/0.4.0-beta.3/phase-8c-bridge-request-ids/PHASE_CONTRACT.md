# Phase 8c — per-request ids for the wallet bridge · PHASE CONTRACT

**Workstream:** money-path correctness · **Ticket:** `TICKET_bridge_single_slot_callbacks_race.md`
**Status:** 🟢 **DECIDED — Option B. Ready to start at stage 1 (§7).**
👤 **Owner, 2026-09-08:** *"Let's go with your recommendation of Option B."* — the 41 slots converge on
the **C++-side promise map** already shipping for the history API. ⛔ The ticket's proposed JS-side
`Map<id,{resolve,reject}>` is **not** what gets built; see §0.3 for why.
⚠️ Owner also noted *"this seems like a big change now"* — which is why §7's staging is not optional.
Stage 1 must land and be reviewed before stages 2–4 are attempted.
**Opened:** 2026-09-08 · **Owner:** Matthew Archbold · **Platforms:** Windows + macOS (shared C++ / TS)
**Standard:** `../HARNESS.md`. **Base:** `c68ed57`.

> ⛔ **Nothing is implemented yet.** The kickoff changed what the fix should be; §8 records the
> decision that resolved it. Start at §7 stage 1.

---

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

## 4. Evidence table — ⬜ written at stage 1

Now that §8 is answered the subject is fixed: the C++ `std::map<int, Pending…>` and the V8 promise it
resolves. Full rows are written when stage 1 lands, against the real mechanism rather than a sketch.

One row holds regardless, and it is the important one:

| ID | 🟢 GREEN | 🔴 RED | 🎯 SUBJECT | Tier |
|---|---|---|---|---|
| `P8c-A1` | Two concurrent `sendTransaction` calls produce **two** on-chain sends | Apply `getBalance`'s dedupe to `sendTransaction` ⇒ **one** send. Must be seen | ⛔ Two distinct **txids**, not two resolved promises | T2 |

## 5. Blast radius

`frontend/src/bridge/initWindowBridge.ts` (1,181 lines, 41 slots) ·
`cef-native/src/handlers/simple_render_process_handler.cpp` (56 emit sites) · every caller of every
migrated method · ⚠️ **macOS shares both files** — this is not a Windows-only change, unlike 8a/8b.

## 6. Out of scope

The `wallet_call` path itself (pattern 2) — it already routes correctly and is not implicated.

## 7. Staging — REQUIRED, not optional

⛔ A 41-slot big-bang on the money path is the wrong shape. Suggested order:

1. **Build the mechanism once** and migrate **one** read-only method end to end, with `A1`'s harness.
2. **Money path next** — `sendTransaction`, `createTransaction` — while attention is on it.
3. The remaining ~38 in batches, each batch its own commit, the old global deleted as each lands.
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
