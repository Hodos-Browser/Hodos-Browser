# TICKET — the wallet bridge's single-slot callbacks lose replies when two calls overlap

**Filed:** 2026-08-26, during beta.3 Phase 2a (`phase-2-logging-syncio/PHASE_CONTRACT.md`)
**Severity:** ⚠️ correctness — spurious user-visible failures; **not** money-losing
**Status:** OPEN — `getBalance` fixed, the rest of the pattern is not
**Sprint:** 📌 Phase 8 (money-path correctness) — bundled 2026-08-31. ⛔ Read its "NOT fixed and why" first: deduping `sendTransaction` would collapse two payments into one.

---

## The shape

`frontend/src/bridge/initWindowBridge.ts` resolves each native call through a **single global
callback slot**, and `simple_render_process_handler.cpp` invokes exactly that global:

```cpp
std::string js = "if (window.onGetBalanceResponse) { window.onGetBalanceResponse("
               + responseJson + "); }";
```

```ts
window.onGetBalanceResponse = (data) => { ...; resolve(data);
                                          delete window.onGetBalanceResponse; };
```

There is **one slot per method, not one per request**. If two calls to the same method are in
flight, the first reply resolves one promise and deletes the handlers; the second caller waits out
its own timeout and reports a failure that never happened.

⛔ Note the contrast with the IPC bridge proper (`wallet_call`), which carries a **`requestId`** and
routes replies correctly. That mechanism already exists — see
[`reference_ipc_modal_requestid_invariant`]. These older direct handlers predate it.

## Measured

Phase 2a moved the `get_balance` wallet call off the browser UI thread. That fixed a 30 s
whole-browser freeze — and, as a side effect, made **concurrent** balance calls reachable, because
the UI thread had been serialising them by accident.

With the wallet stubbed to hang, `window.hodosBrowser.wallet.getBalance()` driven from CDP while
the background poller was also running:

| Run | Result |
|---|---|
| 1 | clean error at 4.92 s |
| 2 | 🔴 `get_balance timed out` at 10.01 s — the reply went to the poller's promise |
| 3 | clean error at 4.24 s |

**1 in 3.** The race pre-dates Phase 2a; Phase 2a raised its probability.

## Fixed here

`getBalance` only, by **deduping in flight** — a second caller joins the first call's promise.
Correct for a read-only query, and it leaves the IPC contract untouched.

## ⛔ NOT fixed, and why

Every other single-slot pair in the same file has the identical shape:
`onWalletStatusResponse`, `onSendTransactionResponse`, `onGetTransactionHistoryResponse`,
`onSetBackupModalStateResponse`, `onAddressGenerateResponse`, …

⛔ **Do not "fix" these by copying the dedupe.** It is only sound for idempotent reads.
**Two sends are not the same send** — deduping `sendTransaction` would silently collapse two
distinct payments into one, which is far worse than the bug. The correct general fix is a
**per-request id**, mirroring what `wallet_call` already does:

1. JS generates an id, keeps a `Map<id, {resolve, reject}>`, and sends it with the message.
2. C++ echoes the id back and JS dispatches on it.
3. The global `window.on*Response` shims go away.

That is a bridge-wide change touching both layers and every caller, so it wants its own phase.

## Acceptance

- [ ] No method resolves another method's — or another call's — reply
- [ ] Two concurrent `sendTransaction` calls produce **two** sends, not one (⛔ the row that
      catches a careless dedupe)
- [ ] A reply arriving after its caller timed out is discarded, not misrouted
- [ ] 🔴 Negative control: remove the id echo in C++ → the mismatch is detected and surfaces, rather
      than silently resolving the wrong promise

## Related

- `phase-2-logging-syncio/PHASE_CONTRACT.md` — Phase 2a, which surfaced this
- `phase-2-logging-syncio/MEASUREMENTS.md` M5 — the freeze that moving the call off-thread fixed
