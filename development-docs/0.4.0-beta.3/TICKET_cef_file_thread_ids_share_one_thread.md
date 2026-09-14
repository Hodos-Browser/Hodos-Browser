# 🎫 Every `CefPostTask(TID_FILE_*)` in the browser process runs on ONE shared thread — a slow task stalls them all

**Found:** 2026-09-14, beta.3 Phase 8c `O12` (contract §4m, `D-15`), while closing the deadline-task log noise.
**Status:** 🔴 **OPEN — unassigned.** Filed from 8c on the owner's Q5 answer (2026-09-14): keep it out of 8d, open a ticket.
**Sprint:** beta.3 · **Platforms:** both (libcef + Chromium behaviour, not ours).

## 1. What happens

libcef creates the three file runners with `base::ThreadPool::CreateSingleThreadTaskRunner(...)` in
SHARED mode (`libcef/browser/chrome/chrome_browser_main_extra_parts_cef.cc :: PreMainMessageLoopRun`), and
Chromium keys shared single-thread runners by *environment*: `USER_VISIBLE` and `USER_BLOCKING` are both
`FOREGROUND` (`base/task/thread_pool/pooled_single_thread_task_runner_manager.cc ::
GetEnvironmentIndexForTraits`). ⇒ `TID_FILE_USER_VISIBLE` and `TID_FILE_USER_BLOCKING` are **one FIFO
thread** in this process; `TID_FILE_BACKGROUND` is a second shared thread at BEST_EFFORT. `cef_types.h`
documents the ids as priorities and says nothing about sharing.

📏 **Measured (8c `P8c-A10b`):** a wallet send held 50 s on `TID_FILE_USER_BLOCKING` stalled a concurrent
`get_balance` until it finished — the wallet panel's balance rejected at the 45 s bridge deadline
(45,013 ms). Moved to `TID_FILE_USER_VISIBLE`: identical (45,008 ms). Fixed **for the send only** by a
dedicated `CefThread` (`hodos-wallet-send`): balances then answered in 2–6 ms during the held send.

## 2. What is still on the shared thread

`simple_handler.cpp` posts to `TID_FILE_USER_BLOCKING` for: `get_balance` (every overlay's poller),
`address_generate`, cookie enumeration/deletion (`CookieManager`), the adblock cosmetic fetch, the
wallet-cache warm-up. Their wallet transport timeouts are 2 s (balance) / 5 s (default), so nothing there
blocks for 30 s *by design* — but a **hung** wallet on `get_balance` (P2's M5 scenario) now stalls every
other file task for 2 s per poll, from every overlay. That is P2's threat model one level down: the UI no
longer freezes; the file queue does.

## 3. Options (rule 5: how others do it)

| | Shape | Cost |
|---|---|---|
| A | Leave it. Document the rule "anything that may block for seconds gets its own `CefThread`" (done: `reference_cef_file_thread_ids_are_one_thread`) | 0 — the measured stall is bounded by the 2 s / 5 s transport timeouts |
| B | One dedicated `CefThread` for **all** wallet HTTP calls (`hodos-wallet-io`), leaving cookies/adblock on the shared thread | small; wallet calls serialise among themselves (they already share the WinHTTP session) |
| C | `base::ThreadPool::PostTask` with `MayBlock()` (a real pool, no single-thread sharing) | ⛔ not exposed through the CEF C API / wrapper — would need a fork patch |

Recommendation: **A now, B when a measured stall appears** — no user-visible defect has been measured on
the residual; the send was the only 30 s call.

## 4. Related

- `phase-8c-bridge-request-ids/PHASE_CONTRACT.md` §4m (`D-15`, `P8c-A10a/b`, `O12`).
- `phase-8d-wallet-supervision/PHASE_CONTRACT.md` `D-6` — why the supervisor is a `std::thread`.
- `phase-2-logging-syncio/` — the UI-thread version of the same problem, fixed by moving off `TID_UI`.
