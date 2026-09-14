# Phase 8c — per-request ids for the wallet bridge · PHASE CONTRACT

**Workstream:** money-path correctness · **Ticket:** `TICKET_bridge_single_slot_callbacks_race.md`
**Status:** 🟢 **STAGES 1–3 COMPLETE (2026-09-13); stage 4 absorbed into the batches.** Six batches after the
mechanism: wallet, backup-overlay deletion, cookies, bookmarks, adblock + privacy shield, paid cache.
**34 bridge natives; zero per-call `window.on*` slots remain** (`P8c-A8`). Open at phase close: the
owner-bound register rows ~~`O1` (two txids, real money)~~ — **done 2026-09-14, `P8c-A2` §4l** — ~~`O2` (a genuinely late reply), `O5` (30 s
deadline vs the slowest real call)~~ — **O2 + O5 closed 2026-09-14, `P8c-A9` (§4k)**, `O6` (macOS verification — relay M7/M8), and the G11 baseline
lowering that O8's deletion earned (its own commit, per working rule 6).
⛔ **8 native bridge functions, 9 JS methods migrated** — `getStatus`, `sendTransaction`, `getBalance`,
`getBackupModalState`, `setBackupModalState`, and (batch 2, 2026-09-12) `address.generate` +
`wallet.generateAddress` (one native `generateAddress` backs both), `getInfo`, `markBackedUp`.
**34 legacy slots remain** — of the **37** the tree actually held at batch-2 kickoff, not the 36 the
recipe said (`D-8`). `P8c-A2` (two successful sends ⇒ two txids) still needs real money: `M8` in
`../PAYMENT_TEST_BATCH.md`.
👤 **Owner, 2026-09-12:** batch 2 kickoff found the wallet namespace to be **one live slot and eight
dead ones** (`D-7`). Owner chose *"Migrate 3, delete 6 + dead C++"* — migrate every wallet method
with a call site, delete the rest and the four C++-only transaction handlers (`O3`/`O4` answered).
👤 **Owner, 2026-09-08:** *"Let's go with your recommendation of Option B."* — the 41 slots converge on
the **C++-side promise map** already shipping for the history API. ⛔ The ticket's proposed JS-side
`Map<id,{resolve,reject}>` is **not** what gets built; see §0.3 for why.
⚠️ Owner also noted *"this seems like a big change now"* — which is why §7's staging is not optional.
Stage 1 must land and be reviewed before stages 2–4 are attempted.
**Opened:** 2026-09-08 · **Owner:** Matthew Archbold · **Platforms:** Windows + macOS (shared C++ / TS)
**Standard:** `../HARNESS.md`. **Base:** `c68ed57`.

> ⭐ **Stage 1 landed 2026-09-08** (mechanism + `wallet.getStatus`, §4) and **stage 2 on 2026-09-09**
> (`wallet.sendTransaction`, §4b). 🚧 **Stage 3 batch 1 on 2026-09-09** (§4c), **batch 2 on
> 2026-09-12** (§4d — the wallet namespace is now fully off the legacy slots). ⛔ **34 legacy slots
> remain** (cookies 15, bookmarks 14, plus `cache` ×2 counted under cookies, and the 3 cookie-blocking
> reads); stage 4 (deleting the `window.on*` declarations wholesale) not started.

---

## 👤 0a. OWED TO THE OWNER AT PHASE CLOSE — the standing register

> Asked for explicitly, 2026-09-09: *"keep track of anything you will need me to get eyes on at the
> end of the full phase."* ⛔ **Append here the moment something is found.** A decision that lives
> only in a commit message is a decision nobody will find.

| # | Needs your eyes on | Why it cannot be settled by me | State |
|---|---|---|---|
| **O1** | **`P8c-A2` — two *successful* sends produce two txids.** `M8` in `../PAYMENT_TEST_BATCH.md` | Needs **real money**, twice. Its RED (apply `getBalance`'s dedupe ⇒ one send) is the row that catches the worst possible way to finish this phase | ✅ **done 2026-09-14** (§4l): GREEN two distinct txids `d81a6892…` / `796a9d93…`, both on chain; RED (dedupe re-applied, then reverted) ⇒ both promises carried `eb4b3d41…`, **one** send. Owner-authorised, self-send, ~3,600 sats in fees |
| **O2** | **A genuinely *late* reply** — one that arrives *after* the deadline already rejected — is discarded, not misrouted | The deadline test proved "reply never arrives". "Arrives late" is a different path and needs an induced delay. Free to run; just not run yet | ✅ **observed 2026-09-14** (`P8c-A9c`, §4k): reply held **50 s** by the rig seam ⇒ caller rejected at **45,011 ms** (`timed out after 45s`), the real reply landed 5 s later and the render log says `bridge response for unknown requestId 9 — discarded, not misrouted`. Nobody else received it |
| **O3** | **Orphaned round trips.** `create_transaction` (`D-6`) turned out to have company: `sign_transaction`, `broadcast_transaction` and `get_all_addresses` have **no JS sender**, and `wallet.create` / `load` / `generateAddress` / `getCurrentAddress` / `getAddresses` / `getTransactionHistory` have **no reachable caller** (`D-7`) | Deleting live-looking C++ is a call I should not make alone | ✅ **decided 2026-09-12** — delete all ten (batch 2, commit 2). Every batch-2 deletion is a JS method, its C++ handler, its render arm(s), and the `WalletService` method it orphans on **both** platforms |
| **O4** | **How many of the 41 do we actually migrate?** | Several (bookmark folder CRUD, cache size) are UI-driven and unlikely to race. My lean is *all* — a slot left behind is a slot the next person copies — but it is real work for little risk reduction | ✅ **answered 2026-09-12:** every slot with a call site is migrated; a slot with none is **deleted**, which is the stronger form of "never copied" |
| **O5** | **The 30 s deadline value** | Chosen as a backstop, not measured against the slowest real wallet call. If any legitimate call can exceed 30 s, this turns a slow success into a failure | ✅ **fixed 2026-09-14** (`P8c-A9`, §4k). 🚨 It was **not** a backstop: `kWalletBroadcastTimeoutMs` = 30 000 and the deadline = 30 000 were **equal**, so a broadcast that used its whole budget and then succeeded would have been reported "timed out" to the user while the money left. The deadline is now `kBridgeCallTimeoutMs` = **45 000** in `WalletService.h`, next to the number it must beat, with `static_assert(kBridgeCallTimeoutMs > kWalletBroadcastTimeoutMs + 5000)`. And `send_transaction` ran the wallet call **inline on the UI thread** (the P2a-A2 defect, on the money path) — now the `get_balance` shape. RED/GREEN measured |
| **O6** | **macOS parity.** Both changed files (`simple_render_process_handler.cpp`, `simple_handler.cpp`) are **shared**, not Windows-only | Unlike 8a/8b this is not Rust-only. Mac must verify the V8 binding and the promise map behave there | ⬜ relay owed — batch 2 also **collapsed** the `#ifdef _WIN32` / `#else` twin copies of the `address_generate` handler (they were byte-identical), and commit 2 deletes bodies in `WalletService_mac.cpp` — Mac's lane, flagged for the relay |
| **O7** | ⚠️ **RED controls are a wasting asset.** `getBackupModalState` WAS the control for `P8c-A1a`; batch 1 used `getInfo`; **batch 2 used `bookmarks.getAllTags`** (§4d — 2 of 3 rejected at 5,011 ms). The wallet namespace now has **no** legacy method left | Remaining legacy pool for a RED: cookies (15) and bookmarks (14). ⛔ **At 0 remaining there is no legacy control left at all**, and the RED must become a deliberate stub | ⚠️ live |
| **O8** | 🚨 **`getInfo` and `markBackedUp` were dead at the BACKEND too.** The Rust wallet has no `/wallet/info` and no `/wallet/markBackedUp` route (`main.rs` — both **404**, measured against the dev wallet). C++ wraps the miss as `{success:false, error:"Failed to get wallet info: {}"}`. Their only consumer, `BackupOverlayRoot`, is itself **unreachable**: its only opener (`overlay_show_backup`) is sent from a block App.tsx has commented out. So the whole backup-overlay chain — the page, the route, the IPC, `CreateBackupOverlayWithSeparateProcess` on both platforms, its HWND/WndProc/role slot — is dead | They were migrated anyway (batch 2): the routing is proven and the change is reversible. **Deleting the chain is an overlay-lifecycle change** (CLAUDE.md invariant 8) and cascades into `cef_browser_shell.cpp` / `cef_browser_shell_mac.mm` — not a call to make inside a bridge batch | ✅ **owner: delete (2026-09-12)** — *"I don't think we need it."* The recovery-phrase prompt is `WalletPanelPage`'s create flow. **Windows + shared half done** (§4e): page, route, the four methods and their natives/arms/handlers, `overlay_show_backup`, every `role_ == "backup"` arm, the HWND/WndProc/class registration, the app-file creator, the window-record HWND field. 🍎 **macOS half owed to Mac** (`MAC_RELAY_P8_ROUND.md` M8): the `.mm` creator and its six `GetBackupBrowser()` uses, `g_backup_overlay_window`, then the accessor/static/header decl and the `BrowserWindow` `backup_browser` / `backup_overlay_window` slots, which Windows kept only so the Mac build stays green |
| **O10** | **Scope: the single-slot pattern lives in 8 hooks too (`D-11`), not only the bridge file** | Doubles the phase (~72 slots vs the ticket's 41); each hook rewrite changes error semantics from "resolve a default on timeout" to "reject" | ✅ **decided 2026-09-12: all live slots, hooks included.** Remaining after batch 3: bookmarks (14, bridge-owned), adblock (6), privacy shield (3), paid cache (2), import (2), profiles (1), settings (1), site permissions (1), recently-closed (1) |
| **O9** | ⚠️ **`address_generate` blocks the UI thread and cannot reject.** The browser handler calls `WalletService::generateAddress()` **inline** — unlike `get_balance`, which P2a moved off-thread for exactly this reason. With the dev wallet stopped, three calls took **6,168 ms, serialised on the UI thread**, and every one **resolved `{}`** rather than rejecting, because `WalletService::makeHttpRequest` swallows transport failure. `useAddress` then reads `response.address` as `undefined`. ⇒ the `address_generate_error` arm (and `RejectBridgeCall` on this slot) is **unreachable in practice** | Pre-existing on both counts. Same family as `TICKET_wallet_backend_death_is_silent_and_unrecovered.md` | ✅ **fixed 2026-09-12 on owner's call** (`P8c-A4d`): off-thread via the `get_balance` shape, and a missing address is now a **rejection**. The "notice the wallet died and restart it" half is the death ticket, now **assigned to Phase 8 after 8c**. 📏 Side finding: `WalletService::isConnected()` is a **latch** — `WinHttpConnect` allocates a handle without touching the wallet, so it reads true with the wallet dead |
| **O11** | 📏 `BridgeCallDeadlineTask` logs its harmless no-op (call already answered) with the O2 words — `bridge error for unknown requestId N — discarded, not misrouted`, 63 of them in a 2-minute window vs one real late reply (§4k) | Pre-existing since batch 1; cosmetic but it degrades the one log line O2 relies on | ⬜ two-line fix (return early when the id is absent), any later 8c tidy commit |

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

### 0.5 🚨 `D-7` — the wallet namespace is ONE live slot, not eight (batch 2 kickoff, 2026-09-12)

The recipe said *"do the wallet namespace first — it holds `markBackedUp` and four of the five
remaining JS-injection sites."* Every caller was traced before believing it:

| Method | Reachable caller | Finding |
|---|---|---|
| `address.generate` | WalletPanel receive flow via `useAddress` | **Live.** The only one |
| `wallet.getInfo`, `wallet.markBackedUp` | `BackupOverlayRoot` only | The overlay is **unreachable** (its opener is inside a `/* */` block in `App.tsx`) — and the Rust routes they call **do not exist** (`O8`) |
| `wallet.create` | `App.tsx`, inside that same commented block | Dead, and **broken**: C++ posts no body, Rust demands a 4-digit PIN → 400 |
| `wallet.load`, `getCurrentAddress`, `generateAddress` | `useWallet` hook only | **`useWallet()` has zero consumers** |
| `wallet.getAddresses` | none | Dead |
| `wallet.getTransactionHistory` | none | Dead **and broken** — its C++ response arm dispatches a `CustomEvent` nobody listens for, so it could never resolve even single-threaded |
| C++-only `get_all_addresses`, `create_transaction`, `sign_transaction`, `broadcast_transaction` | no JS sender | Dead at both ends — three of the "five JS-injection sites" sit here |

⇒ Only **one** of the five unescaped JS-injection sites was on a live path (`onAddressError`).
The recipe's ordering rationale was built on dead code. Owner decision: migrate the three with a call
site, delete the rest (`O3`/`O4`).

### 0.6 📏 `D-8` — 37 slots, not 36, and one slot pair served two methods

Counted programmatically over `initWindowBridge.ts`: **37 distinct `window.on*Response`-type
globals** were still assigned at batch-2 kickoff, across 38 methods — `address.generate` and
`wallet.generateAddress` **shared** `onAddressGenerated` / `onAddressError`, so a call through either
could steal the other's reply. Both now resolve through one native `bridge.generateAddress`.

### 0.7 ⚠️ `D-9` — THREE timeout flavours, not two

`D-5` found *reject* and *resolve(null)*. The tree has a third: **no timeout at all.** Per method:

| Flavour | Methods |
|---|---|
| **resolve on timeout** (silent wrong value) | `cookies.getAll`, `cookieBlocking.getBlockList`, `getBlockLog`, `getBlockedCount` — all at 5 s |
| **no timeout** (hangs forever if the reply is stolen) | `cookies.deleteCookie`, `deleteDomainCookies`, `deleteAllCookies`, `clearCache`, `getCacheSize` |
| reject on timeout | everything else remaining (bookmarks at 5 s, `bookmarks.getAll` at 15 s, cookie-blocking writes at 5 s) |

The migrated path replaces all three with one behaviour: reject at the 30 s deadline, or on the
native `*_error` arm.

### 0.8 ⛔ `D-10` — `markBackedUp` was never a `resolve(null)`

The recipe and the batch-1 close-out both named `markBackedUp` as *"the last `resolve(null)`"*. It
**rejected** (`reject(new Error('mark_wallet_backed_up timed out'))`). Zero `resolve(null)` remained
anywhere in the wallet namespace; the real offenders are the four cookie reads in `D-9`. Caught by
the recipe's own rule — *read each one; do not assume* — applied to the recipe.

### 0.9 🚨 `D-11` — the pattern was COPIED into the hooks: ~31 more slots the ticket never counted (batch 3 kickoff, 2026-09-12)

The recipe's remaining list for cookies was the **bridge's** `cookies` / `cookieBlocking` namespaces
(15 methods). Neither had a caller. `useCookies.ts` and `useCookieBlocking.ts` send the IPC
themselves and set **the same** `window.on*` globals — a second, live copy of the single-slot race.
The same copy exists in `useAdblock` (6), `usePrivacyShield` (3, incl. `cookie_check_site_allowed`),
`usePaidCache` (2), `useImport` (2), `useProfiles` (1), `useSettings` (1, also set from
`MainBrowserView`), `useSitePermissions` (1) and `TabListOverlayRoot` (1). `useBookmarks` is the
exception — it really does go through the bridge namespace.

📏 Hook-owned timeout flavours, measured: cookie **writes have no timeout** (a stolen reply hangs
forever); all six adblock calls and four cookie reads **resolve a wrong default**; the privacy-shield
calls neither resolve nor reject. ⇒ the ticket's own warning — *"a slot left behind is a slot the next
person copies"* — had already happened before the ticket was written.

👤 **Owner, 2026-09-12: migrate all live slots, hooks included** (~46 over three to four batches).
Batch 3 = cookies end to end: delete the dead bridge copies, 15 bridge natives, request id threaded
through `CookieManager`'s async reply helpers, both hooks call the bridge.

### 0.10 🚨 `D-12` — the bridge's JSON→V8 converter flattened nested payloads (batch 4, 2026-09-13)

`ResolveBridgeCall` resolves through `jsonToV8`, which **by design** delivers any nested object or
array inside an object as its `.dump()` string — the `identity.get` contract, whose callers parse it
themselves. The legacy path evaluated the reply as a JSON **literal** (`window.on*(` + json + `)`), so
nested values arrived as real objects and arrays. ⇒ every bridge reply since stage 1 was a silent
contract change for any nested payload. Caught by batch 4's round trip: `bookmarks.getAll().bookmarks`
came back as a **string** and `.find` threw. Fix: `jsonToV8(j, deep)`; the bridge passes `deep = true`,
`identity.get` keeps its default. Stages 1–3 audited for exposure: `getStatus`, `getBalance`,
`generateAddress` and every cookie payload are flat or top-level arrays of flat objects; see the
`P8c-A6` row for `sendTransaction`.

### 0.11 📏 `D-13` — three of the "hook-owned slots" are push listeners, not per-call slots (batch 5 kickoff, 2026-09-13)

`useProfiles` (`onProfilesResult`), `useSettings` (`onSettingsResponse`) and `useSitePermissions`
(`onSitePermissionsResponse`) install their handler **once** in an effect and C++ re-emits the full
authoritative list after every get / set / reset. That is a subscription, not a request: there is no
per-call promise to misroute, and the last emit winning is the intended semantics. ⇒ `D-11`'s ~31
overcounts by 3. They stay as they are (a different shape; converting them to a proper event API is
not this phase's defect). The per-call remainder after batch 5: `usePaidCache` (2), `useImport` (2),
`TabListOverlayRoot` (1).

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
| `P8c-A2` | Two concurrent `sendTransaction` calls produce **two** on-chain sends. RED: apply `getBalance`'s dedupe ⇒ **one**. ⛔ Subject is two distinct **txids**, not two resolved promises | T2 — **stage 2**, and it belongs in `../PAYMENT_TEST_BATCH.md`. 🟢🔴 **Observed 2026-09-14, §4l** |
| `P8c-A3` | A reply arriving after its caller gave up is discarded, not misrouted | 🟢 **exercised as a side effect of `A1b`**: after the deadline rejected both calls their map entries were gone, so the mechanism is the same one proven there. ⚠️ A reply genuinely arriving *late* (rather than never) is still unobserved |

---

## 4d. Stage 3 batch 2 — the wallet namespace's live remainder, measured 2026-09-12

Batch chosen by **caller audit, not by the recipe's list** (`D-7`): `address.generate` (+ its alias
`wallet.generateAddress`), `getInfo`, `markBackedUp`. Same harness as batches 0–1: CDP against the
header, hard-reloaded (`Page.reload ignoreCache`) before every measurement.

**Subject, asserted first** — 11 CDP targets, exactly one at `http://127.0.0.1:5137/`:

| Check | Result |
|---|---|
| `bridge.generateAddress` / `getInfo` / `markBackedUp` are native | ✅ all three `[native code]` |
| `address.generate` and `wallet.generateAddress` both delegate to `bridge.generateAddress` | ✅ both |
| All six legacy globals of this batch | ✅ `undefined` |

### `P8c-A4a` — 🟢🔴 GREEN and RED, same harness

| Run | Result |
|---|---|
| **Address generation ×3 concurrent, MIXED entry points** (2× `address.generate`, 1× `wallet.generateAddress`) | **3 / 3 fulfilled, 35 ms, 3 DISTINCT addresses.** ⭐ These two methods shared one global slot pair under the legacy bridge — this row is the free analogue of the two-txid test: three callers, three different answers |
| `getInfo` ×3 | **3 / 3 answered, 7 ms** — each got its own reply. ⚠️ Every reply was `{success:false, error:"Failed to get wallet info: {}"}` because the Rust route does not exist (`O8`). The *routing* is what this batch proves; the payload was dead before it |
| `markBackedUp` ×3 | **3 / 3 answered, 7 ms** — same: own replies, each `{success:false}` for the same reason |
| 🔴 **RED — `bookmarks.getAllTags`, still legacy** | **5,011 ms; 2 of 3 REJECTED** `Bookmark getAllTags timeout`, 1 fulfilled — the harness sees the bug on a sibling that has not been touched |

### `P8c-A4b` — the wallet-down case, and why the error arm is unreachable here

Dev wallet stopped (path-matched kill of the **dev** exe only; the owner's installed wallet on 31301
answered 200 throughout):

| Run | Result |
|---|---|
| `address.generate` ×3 with no wallet | **3 / 3 FULFILLED with `{}` after 6,168 ms** |

Two findings, both pre-existing (`O9`): the three calls were **serialised on the browser UI thread**
(the handler is inline, not off-thread like `get_balance`), and they **resolved rather than
rejected** because `WalletService::makeHttpRequest` swallows transport failure. So on this slot the
`address_generate_error` arm — and therefore `RejectBridgeCall` — cannot fire. The hoisted request
id in that handler is correct by construction (same shape as stage 2's `P8c-A2b`, where it was
measured), but it is not exercisable without a stub. Recorded as such rather than claimed.

### `P8c-A4c` — 🟢 after commit 2 (the deletions), the surface is exactly what the contract says

Rebuilt and re-measured on the same rig: `hodosBrowser.wallet` holds **exactly** `getStatus`, `getInfo`,
`markBackedUp`, `getBackupModalState`, `setBackupModalState`, `getBalance`, `sendTransaction`;
`hodosBrowser.bridge` holds the 8 native functions; **zero** `window.on*` globals of the wallet /
address / transaction-history families remain on the page. `address.generate` ×3 concurrent →
**3 / 3 fulfilled, 3 distinct addresses, 32 ms**; `wallet.generateAddress` is `undefined`. The
harness's own mixed-entry-point row now throws on the deleted twin — which is the correct outcome
of the deletion, not a regression.

### `P8c-A4d` — 🟢🔴 `O9` fixed: `address_generate` off the UI thread, rejecting on wallet failure

Owner call 2026-09-12: *"it should … throw some wallet error."* Handler rewritten to the
`get_balance` shape (captureless lambda on `TID_FILE_USER_BLOCKING`, id threaded as a parameter,
reply hopped back to `TID_UI`), and a reply without an `address` is sent on the **error** arm.
Harness adds a concurrent `/json/list` probe: the browser process answers it on the UI thread, so a
blocked UI thread shows up as probe latency.

| Run (dev wallet **stopped**, path-matched kill; installed wallet on 31301 answered 200 throughout) | Result |
|---|---|
| 🟢 `address.generate` ×3 | **3 / 3 REJECTED** — `generateAddress: wallet did not return an address (is the wallet backend running?)` — wall 6,129 ms (WinHTTP connect timeout, serialised on the blocking pool). Final binary; the first cut measured 6,120 ms with the same outcome |
| 🟢 UI-thread probe during those 6 s | **51 samples, max 19 ms, median 16 ms** — the browser stayed responsive |
| 🔴 **RED — `wallet.getInfo` ×3, still INLINE on the UI thread**, same harness | 3 / 3 *fulfilled* `{success:false}`; probe **2 samples in 6 s, max 3,995 ms** — the stall the fix removes, observed on the same binary |
| 🟢 wallet **up**, `address.generate` ×3 | 3 / 3 fulfilled, **3 distinct addresses, 28 ms** — no regression |

⭐ The RED needed no rebuild: the batch-2 migration left `getInfo` inline, so it is the un-fixed
sibling on the same harness. ⚠️ Two things this does **not** do, on purpose: it does not restart the
wallet (that is the death ticket, process supervision, not per-call-site), and the rejection still
takes the transport timeout (~2 s per call) rather than being instant — a supervised wallet with a
known-dead state is what makes it instant.

📏 `isConnected()` guard tried and **removed**: `WinHttpConnect` never opens a connection, so the flag
is true with the wallet dead and the guard was a dead branch. The reply is the only signal.

### `P8c-A4e` — 🟢 `O8`: the backup-overlay chain deleted (Windows + shared half), nothing else moved

Owner call 2026-09-12 (*"I don't think we need it"*). Subject: the CDP target list **is** the overlay
roster on this build (every overlay is pre-created at startup), so it is the right instrument for "did
any other overlay disappear".

| Check | Result |
|---|---|
| CDP targets after the deletion | **11**, the same eleven as before it — header + 10 overlays; no `/backup` (there never was one live, which is the point) |
| `hodosBrowser.wallet` | exactly `getStatus`, `getBalance`, `sendTransaction` |
| `hodosBrowser.bridge` | exactly `getStatus`, `getBalance`, `sendTransaction`, `generateAddress` — **4 natives** |
| `window.on*` of the wallet / address / backup / history families | **none** |
| Surviving bridge, live | `getStatus` `{exists:true, needsBackup:false}` · `getBalance` keys `balance, bsvPrice` · `address.generate` ×3 → **3 distinct, 38 ms** |
| The deleted IPC, sent anyway (`cefMessage.send('overlay_show_backup')`) | no-op: 11 targets before and after, header still answers |

Removed: 513 C++ lines across `simple_handler.cpp`, `simple_render_process_handler.cpp`,
`simple_app.cpp`, `cef_browser_shell.cpp`, `BrowserWindow.h`, plus the page, the route, four bridge
methods and their types. ⚠️ **macOS half owed** — relay M8. `GetBackupBrowser()` and the
`BrowserWindow` backup slots are kept as null shims until it lands, so Mac's build stays green.

### `P8c-A5` — 🟢🔴 stage 3 batch 3: cookies + cookie blocking, end to end (2026-09-12)

15 natives; the two caller-less bridge namespaces deleted; `useCookies` / `useCookieBlocking` call
the bridge; request id threaded through `CookieManager`'s three async reply helpers. Same harness,
hard-reloaded header.

| Check | Result |
|---|---|
| 15 bridge natives `[native code]`; `hodosBrowser.cookies` / `cookieBlocking` | ✅ 15 / 15 · both `undefined` |
| `window.on(Cookie\|Cache)*` globals | ✅ none (only `onCookieCheckSiteAllowedResponse`, a later batch) |
| ×3 concurrent: `cookieGetAll` / `cookieGetBlocklist` / `cookieGetBlockedCount` / `cacheGetSize` / `cookieGetBlockLog` | **3 / 3 each, own replies**; 17 ms for 211 cookies, ≤1 ms for the rest — `cookieGetAll` is the one that goes through CEF's IO-thread visitor, so this is real concurrency, not UI-thread serialisation |
| block → list → unblock → list on `p8c-batch3-test.invalid` | ✅ in list after block, gone after unblock |
| 🚨 **Empty jar**: `cookieDeleteAll` (211 deleted) then `cookieGetAll` ×3 | **3 / 3 `[]` in 1 ms.** The legacy hook took **5 s** here and resolved `[]` from its timeout, because CEF never calls the visitor when there is nothing to visit (`cef_cookie.h`). Now answered from `CookieCollector`'s destructor |
| 🔴 RED — `bookmarks.getAllTags` ×3, still legacy | **5,011 ms, 2 of 3 rejected** — same harness sees the bug on the untouched sibling |
| Consumer: `/browser-data?tab=cookies` through the rewritten hooks | renders "No cookies found · 24 blocked" (the jar had just been emptied): cookie list **and** block list came through the hooks, nothing stuck on Loading |

Semantics that changed on purpose: a real failure now **rejects** (the hooks record it in `error`),
where the legacy hooks resolved `[]` / `{count:0}` or hung forever; the five fire-and-forget mount
fetches in the consumers got a `.catch(() => {})` for that reason. `CookieManager`'s reply log now
prints length only — it used to write every cookie value on the profile into the log at INFO.

⚠️ Dev-profile note: the empty-jar row wipes the dev cookie jar. Dev only, by design of the harness.

### `P8c-A6` — 🟢🔴 stage 3 batch 4: bookmarks (2026-09-13), and the `D-12` converter fix

Five natives behind the five `useBookmarks` calls; nine caller-less bookmark IPCs deleted at the
bridge, handler and arm; `BookmarkManager`'s folder/tag SQL untouched. The 15 s `getAll` timeout and
the hook's 3× retry retired (both were workarounds for a late reply being dropped by the slot).

| Check | Result |
|---|---|
| 5 natives `[native code]`; namespace is exactly `add, getAll, isBookmarked, remove, search`; no `onBookmark*` globals | ✅ |
| ×3 concurrent `getAll` / `isBookmarked` / `search` through the namespace | **3 / 3 own replies each, ≤2 ms** |
| 🚨 **Nested shapes** — `getAll().bookmarks`, `bookmark.tags`, `cookieGetBlockLog()` entries | **First run: `bookmarks` was a STRING** (`.find is not a function`). `D-12`: the converter dumped nested values. After `jsonToV8(…, deep = true)`: `bookmarks` array ✅, `tags` array ✅, block-log entries objects ✅ |
| add → isBookmarked → search → remove → isBookmarked | add ✅ (id 12), bookmarked ✅, removed ✅ — then **still bookmarked**: the first (failed) run had already added the same URL as id 11, and `search` returned that older duplicate first. Removing every entry for the URL ⇒ `isBookmarked` **false** ✅. Harness residue, not a defect |
| 🔴 RED — the hook-owned adblock slot, reproduced **verbatim from `useAdblock.ts`** (3 concurrent `adblock_get_blocked_count`, resolve-on-timeout at 3 s) | **3,010 ms: 2 of 3 got the TIMEOUT DEFAULT, 1 got data** — the silent-wrong-value flavour, live, on the next batch's own code |
| Consumer: the `/bookmarks` overlay through `useBookmarks` | list rendered (four real bookmarks), no error state |

⭐ **The RED control moved to the hooks**, as `O7` predicted: nothing in `initWindowBridge.ts` is
legacy any more, so from here every RED is a hook-owned slot reproduced verbatim — which is also the
kickoff evidence for the batch that migrates it.

### `P8c-A7` — 🟢🔴 stage 3 batch 5: adblock + privacy shield (2026-09-13)

Eight natives; `useAdblock` and `usePrivacyShield` rewritten onto the bridge; the
`checkPendingRef` in-flight dedupe retired; `fingerprint_set_site_enabled` left as the
fire-and-forget IPC it is (no reply exists to route).

| Check | Result |
|---|---|
| 8 natives `[native code]`; no `onAdblock*` / `onCookieCheckSiteAllowed*` / `onFingerprintSiteEnabled*` globals | ✅ |
| ⭐ **Batch 4's RED, re-run on the migrated native** — `adblockGetBlockedCount` ×3 | **3 / 3 real replies, 1 ms** (was: 2 of 3 handed the 3 s default) |
| ×3 concurrent `adblockCheckSiteEnabled` / `adblockCheckScriptletsEnabled` / `cookieCheckSiteAllowed` / `fingerprintGetSiteEnabled` | 3 / 3 own replies each, ≤1 ms |
| Site toggle off → check → on → check; scriptlets likewise, on `p8c-batch5.invalid` | false / false / true / true, both families ✅ |
| 🔴 RED — `usePaidCache.refresh` reproduced **verbatim** (reject at 3 s), ×3 concurrent | **3,003 ms, 2 of 3 rejected** `paid_cache_get_size timeout` — the next batch's bug, live |
| Consumer: `/privacy-shield` overlay with a domain set through the C++-injected hook | five switches rendered for `example.com`, all four rows present |

Semantics changed on purpose: the six adblock calls used to resolve a made-up default on timeout
(0 blocked, "toggle failed", "enabled") — the silent-wrong-value flavour, now measured twice — and
the two privacy reads dropped their slot without resolving at all. All eight now reject on a real
failure; click handlers and mount reads that fire-and-forget got a `.catch`, and hook state only moves
on a real reply.

### `P8c-A8` — 🟢 stage 3 batch 6: the paid-content cache pair, and the last per-call slot in the tree (2026-09-13)

| Check | Result |
|---|---|
| `paidCacheGetSize` / `paidCacheClear` native; **34 natives total**; no `onPaidCache*` globals | ✅ |
| ⭐ **Batch 5's RED, re-run on the migrated native** — `paidCacheGetSize` ×3 | **3 / 3 real replies, 3 ms** (was 2 of 3 rejected at 3,003 ms) |
| clear → size | 20,645 B → `{success:true, totalBytes:0}` → 0 B ✅ |
| 🟢 **`P8c-A3` observed directly** — two raw `paid_cache_get_size` sends with **no request id** (so the browser replies with id 0) bracketing one real bridge call | the real call resolved its own payload in 1 ms; `cef_debug.log` gained **exactly two** `bridge response for unknown requestId 0 — discarded, not misrouted` lines. An unknown-id reply is dropped, never delivered to whoever is waiting |
| Consumer: `/browser-data?tab=cache` through `usePaidCache` | "Paid Content 0 B" card rendered |

🔴 **RED for this batch = `P8c-A7`'s RED row**, measured one commit earlier on this exact slot
(3 concurrent `paid_cache_get_size` under the legacy code → 2 of 3 rejected). `O7` said the final
batch's RED would have to be a stub; it did not come to that, because batch 5 measured batch 6's slot
before batch 6 migrated it. ⭐ That is the pattern the whole stage settled into: each batch's RED
reproduces the *next* batch's hook code verbatim, so every slot was seen failing before it was fixed.

### ✅ Stage 3 is COMPLETE — no per-call `window.on*` slot remains in `frontend/src`

Per-call slots migrated or deleted, in order: wallet (5 + 3 migrated, 6 deleted), backup overlay
(4 deleted with the overlay), cookies (15), bookmarks (5 migrated, 9 deleted), adblock + privacy
shield (8), paid cache (2). Remaining `window.on*` assignments are all **subscription-shaped**
(`D-13`): `useSettings`, `useProfiles`, `useSitePermissions`, `useImport`, `TabListOverlayRoot`'s
recently-closed list, and `DownloadSettings`' native-folder-dialog callback — installed once per mount,
re-emitted into by C++, last emit wins by design. **Stage 4** ("delete the declarations wholesale")
was absorbed into the batches: every retired declaration in `hodosBrowser.d.ts` is already a
`⛔ REMOVED` comment naming the batch, so nothing is left to delete.

### O5 — two real latencies

`getInfo` **3 ms**, `address.generate` **10 ms** (single calls, warm). Neither is the slowest real
call; the 30 s deadline remains unmeasured against `send_transaction`'s broadcast. → **Resolved in §4k:** the
slowest real call is bounded by `kWalletBroadcastTimeoutMs` (30 s) and the deadline was *equal* to it.

### ⭐ What this batch retired

- The **last live JS-injection site** of the family: `onAddressError('" + errorMessage + "')`,
  unescaped, on the address path. Deleted, not escaped.
- The **duplicate** `address_generate_response` / `_error` arms in the render handler (the second
  copies were unreachable) and the byte-identical `#ifdef _WIN32` / `#else` twin handlers in the
  browser process, collapsed to one so the id echo is made once.
- A **false type**: `wallet.getInfo` was declared to resolve a flat `{version, mnemonic, address,
  backedUp}`; C++ has always sent `{success, wallet:{…}}`. `BackupOverlayRoot` read it flat, so its
  `mnemonic` was `undefined`. The type now says what the wire carries and the one consumer reads it
  nested — moot while the overlay is unreachable (`O8`), but the type must not lie.

## 4k. O2 + O5 — the money path off the UI thread, and a deadline that clears the broadcast timeout (2026-09-14)

### 🚨 `D-14` — the deadline was not a backstop; it was equal to the broadcast timeout

`kBridgeCallTimeoutMs` was a file-local `30000` in the render handler. `kWalletBroadcastTimeoutMs`
(`WalletService.h`) is `30000`. `send_transaction` → `WalletService::sendTransaction` →
`/transaction/send` with that timeout. So the one call that is *allowed* to be slow — because it is
broadcasting real money — had exactly zero margin: a send that used its budget and then succeeded
would be **rejected to the caller as "timed out"** while the transaction went out anyway. Worse, the
handler ran the wallet call **inline on the browser UI thread**, the P2a-A2 defect that P2 fixed for
`get_balance` and O9 fixed for `address_generate`, still live on the money path: every window in the
process froze for the length of the call.

**Fix.** (1) `kBridgeCallTimeoutMs = 45000` now lives in `WalletService.h` beside the transport timeouts
it must beat, with `static_assert(kBridgeCallTimeoutMs > kWalletBroadcastTimeoutMs + 5000, …)` — the
relationship is checked by the compiler, not by the next reader. The render handler includes it.
(2) `send_transaction` uses the `get_balance` shape: captureless lambda on `TID_FILE_USER_BLOCKING`,
request id and payload as parameters, reply hopped to `TID_UI`; every path replies with the id. The
512-byte compaction and the `Invalid response from wallet` shape are unchanged. (3) The
`received JSON = <full transaction body>` DEBUG line is gone — it put the destination and amount in
a plaintext log (P0-A1 family); the reply is logged by length only.
(4) A **rig-only seam**, `HODOS_BRIDGE_REPLY_DELAY_MS`, read once in the browser process (same family
as `HODOS_WALLET_SYNC_UI`), sleeps on the blocking thread before posting the send reply — the only way
to manufacture a *late* reply without money. Unset in production.

### `P8c-A9a` — 🟢🔴 off the UI thread, same harness, same conditions, only the binary differs

Subject: header page (`http://127.0.0.1:5137/`), one `wallet.sendTransaction({recipient:'p8c-o25-invalid',
amount:1})` — the wallet rejects the field, nothing can move — with a concurrent `/json/list` probe (the
UI-thread instrument from O9). **Dev wallet stopped by exe path**, so the call fails at transport.

| | send | probe |
|---|---|---|
| 🔴 **RED** — binary before this change (`d7f1287` lineage) | fulfilled `{success:false,error:"Invalid response from wallet"}` in **2,053 ms** | **one** sample, **2,061 ms** — the UI thread was held for the whole call |
| 🟢 **GREEN** — this change | same payload, 3,593 ms (transport failure path, wallet down) | **17** samples, max **18 ms**, median 17 ms |

### `P8c-A9b` — 🟢 O5: a reply slower than the OLD deadline now resolves

Wallet **up**, browser launched with `HODOS_BRIDGE_REPLY_DELAY_MS=35000`. The wallet answered in
milliseconds (its own error for the unknown `recipient` field — proof it was reached and nothing was
sent); the reply was then held 35 s.

| | |
|---|---|
| result | **fulfilled at 35,009 ms** with the wallet's error text, not a timeout |
| probe during the hold | 158 samples, max 21 ms |
| browser log | `holding send reply (requestId 9)` at :08.010 → `Send transaction reply (requestId 9, 151 bytes)` at :43.010 |

Under the previous 30 s deadline this exact reply would have been rejected at 30 s and the real
answer discarded — the O2 path below, on a *successful* send.

### `P8c-A9c` — 🟢 O2: a reply that arrives AFTER the deadline is discarded, not misrouted

Same, with `HODOS_BRIDGE_REPLY_DELAY_MS=50000`, request id **9**:

| when (07:17/18) | where | line |
|---|---|---|
| :08.655 | render | `bridge sendTransaction -> send_transaction (requestId 9)` |
| :08.657 | browser | `HODOS_BRIDGE_REPLY_DELAY_MS=50000 — holding send reply (requestId 9)` |
| :53.665 | render | `[WARN] bridge call 9 (sendTransaction) timed out — the browser process never replied` — caller rejected `timed out after 45s` at **45,011 ms** |
| :58.662 | browser | `Send transaction reply (requestId 9, 151 bytes)` — the real answer, 5 s late |
| :58.663 | render | `bridge response for unknown requestId 9 — discarded, not misrouted` |

Probe during the whole 45 s: 204 samples, max 22 ms. 🔴 The RED for this row is `P8c-A9b`: the same
mechanism, the same seam, a reply that beats the deadline **is** delivered — so "discarded" here is the
deadline, not a lost message.

### 📏 Side observation — the deadline task's no-op is logged with the same words (report, not fixed)

`BridgeCallDeadlineTask::Execute` always calls `RejectBridgeCall`, and when the call was answered long
ago `TakeBridgeCall` logs `bridge error for unknown requestId N — discarded, not misrouted` at DEBUG.
In the 2-minute window of `A9c` there were **63** such lines (one per bridge call, 45 s after each
poll), against **one** genuine late reply. The wording cannot tell a real late `_error` reply from the
deadline's harmless no-op; a reader grepping for the O2 line will find the no-ops first. Two-line
fix (return early when the id is absent) — **not** made here (rule 3; it is not this change's code).
Registered as `O11`.

## 4l. O1 — `P8c-A2` observed with real money, 2026-09-14 (owner: *"yes go do it"*)

Subject: two **concurrent** `wallet.sendTransaction({toAddress, amount: 1000})` from the header page
(`Promise.allSettled`, same tick), dev wallet (mainnet, 49 spendable outputs, 28,548,353 sats before),
destination = a fresh address of the same wallet, so the 1,000 sats come back. Header hard-reloaded
before each round; `bridge.sendTransaction` confirmed `[native code]` after the reload.

| | promise 1 | promise 2 | on chain (WhatsOnChain `tx/hash`) |
|---|---|---|---|
| 🟢 **GREEN** — the tree as pushed (`9b56ac5`) | `d81a6892…fbf6e13b`, 3,076 ms | `796a9d93…de9217b1`, 3,076 ms | **both 200**, two distinct txids |
| 🔴 **RED** — `getBalance`'s old in-flight dedupe re-applied to the send wrapper for one run, then reverted (`git diff` on the bridge file empty) | `eb4b3d41…b335dda5`, 1,383 ms | **the same** `eb4b3d41…b335dda5` | one tx on chain — the second caller was handed the first caller's txid and **no second send happened** |

Full txids: GREEN `d81a6892582708d28b1b87e66bf2fcc036b47c044f14a98cc36c84c3fbf6e13b` and
`796a9d9384b6277f58f3b3e18071fc0b02a41921c0d7f02a5f6f33f2de9217b1`; RED
`eb4b3d416d3eac7fab6515308f1840400a754d3a8257163cc52c2a11b335dda5`. Both GREEN transactions were
accepted, so the two concurrent `createAction`s selected **disjoint inputs** (the P0.7 reservation held
under concurrency — a fact this test was not designed for but did establish). Cost of the sitting:
three sends × (1,000-sat service fee + ~200 sat miner fee); balance 28,548,353 → 28,543,753 with the
self-send outputs credited.

⛔ **The RED is the row that matters.** It is the exact shape a "helpful" future edit would take —
*dedupe the send like the balance* — and it turns two user intents into one silent transaction. The
warning above `sendTransaction` in `initWindowBridge.ts` now has a measured txid behind it.

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
3. 🚧 **IN PROGRESS.** Batch 1 done — `getBalance`, `getBackupModalState`, `setBackupModalState`, chosen to cover the three remaining SHAPES (§4c). **Batch 2 done (2026-09-12)** — `address.generate` + `wallet.generateAddress`, `getInfo`, `markBackedUp` (§4d), plus a second commit deleting the six dead wallet methods, `useWallet.ts`, and the four dead C++-only transaction handlers (`O3`/`O4`). **34 slots remain** — cookies and bookmarks — now mechanical. Each batch its own commit, the old global deleted as each lands.
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
