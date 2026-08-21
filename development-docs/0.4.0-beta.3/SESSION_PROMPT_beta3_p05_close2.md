# Session kickoff — finish closing beta.3 Phase 0.5, then 0.6 → 0.7 → 0.8 → 0.9

> Paste everything below the rule into a fresh session.
> **Written 2026-08-21** at the end of the session that measured `/processAction`, closed `G1`,
> re-ran the whole table, ran panel #3 — and was told by panel #3 not to sign off.

---

Phase 0.5 is **not** closeable yet. Two measured defects are open, one of them introduced by a fix
that landed the same day, and one claim in the contract was **false and has been corrected**.

## Read first, in this order

1. `development-docs/0.4.0-beta.3/phase-0.5-money-path/PHASE_CONTRACT.md` — **§4s is the current
   state** (panel #3, DO NOT SIGN OFF). Then §4o, §4q, §4r for what landed.
2. `.../phase-0.5-money-path/ADVERSARIAL_PANEL_3_2026-08-21.raw.json` — 44 findings, **unsynthesized
   and mostly unverified**. The synthesizer and 15 verifiers died on a session limit.
3. `.../phase-0.5-money-path/probes/README.md` — the probes, with their safety rules.
4. `development-docs/0.4.0-beta.3/MAC_RELAY_BETA3.md` round **2026-08-21b** — what the macOS session owes.
5. `CLAUDE.md` invariants **#1, #2, #13**.

`git log --oneline 64e8691..HEAD` is this session's work (8 commits).

## ⛔ Verify before you trust — and that now includes me

**Re-grep every file:line.** This phase has produced **four** false audit claims. The fourth was
mine: §4q cited `probes/ungateable.py` as containing a `TestRequest` sibling check. It does not —
`grep -rn TestRequest probes/` returns zero. I ran it inline, never committed it, then cited the
committed file. **The check was also too shallow**: it asked whether a `dispatch_*` call sits
between `fn` entry and the synthetic request, never whether it is **unconditional**. That is
exactly what hid the `pay_402` blocker.

## Task 1 — 🚨 `/wallet/pay402` inverts the fail-closed rule (MEASURED, blocker)

```rust
let brc121_engine_headers_present = headers.contains_key("X-Payment-Satoshis")
                                 || headers.contains_key("X-User-Approved");
if brc121_engine_headers_present { dispatch_payment(...) }   // absent headers => NO GATE
```

`/wallet/pay402` is **not** in `hodos::IsPaymentEndpoint` (`PaymentCost.h`), and the external IPC
arm stamps `X-Payment-*` only `if (isPaymentEndpoint(...))`. So the absence *guarantees* the headers
are missing, which *guarantees* the gate is skipped. `check_domain_approved` runs but enforces trust
level only — **no caps**. An approved dApp mints and broadcasts an arbitrary-value BRC-121 payment
with no cap, no rate limit, no modal, no gold pill.

⛔ Do **not** just delete the conditional. BRC-121 has a legitimate paid-retry flow where C++
re-issues with `X-User-Approved`; work out what the unconditional gate should decide for each of
those paths first. `pay_402` mints a **nosend** BEEF and `broadcast_nosend` broadcasts it after the
paid retry returns 200 — both halves need thinking about together.

## Task 2 — `/%70rocessAction` defeats the C++ half of the `/processAction` fix (MEASURED, high)

`IsPaymentEndpoint("/%70rocessAction") == false`. actix routes the **decoded** path so Rust still
gates (fail-closed on money), but C++ never prices it ⇒ amount-blind `price_unavailable` prompt,
**no gold pill**, per-session dollar cap never advances.

⛔ This is panel #2's finding 1.3 in a new location. **Do not fix it by adding one more string.**
The contract has said "gate SUBTREES, never a list of exact strings" since panel #2, and I added a
substring match to a string list anyway. `is_permission_surface` in `main.rs` has the identical
shape and the identical hole. **Fix the pattern.**

## Task 3 — the rest of panel #3, and re-run it to completion

Also MEASURED: `IsInternalOrigin` trusts **any** loopback port while `IsInternalFrontendUrl`
requires `:5137`, **and the money path uses the looser one**; `IsWalletHostPort` is still an
unanchored substring search; `PaymentCost.h`'s "do both or neither" rule is already violated in-tree
by `/acquireCertificate` and `/sendMessage`, and `payment_cost_test.cpp` asserts only the violating
half — so the green suite **cannot** detect the failure that header exists to prevent.

Three **blocker**-rated CODE_READING findings never got a verification pass. Named in §4s. **Do not
act on them and do not dismiss them until they are run.** The one I would run first is the claim
that a page can navigate itself to `http://127.0.0.1:5137/brc100-auth?type=domain_approval&domain=…`
and drive a **real** approval prompt with a self-chosen domain — it needs an **installed** build,
and `simple_handler.cpp` is shared, so it is not macOS-specific.

Then re-run the panel to completion — an incomplete panel is INCOMPLETE, never a pass (HARNESS §8).

## Task 4 — the owner rows are DONE, do not re-ask

- `R1` ✅ owner send, 166,991 sats, no modal, zero `X-Requesting-Domain`, broadcast.
- `R4` ✅ **owner saw the gold pill** over the teragun.com tab. `OnWalletCallSuccess fired
  (1 cents … cefBrowserId=2 → tabId=1)`. `R-GOLD` holds.

## Then, in order: 0.6 → 0.7 → 0.8 → 0.9

| Phase | What |
|---|---|
| **0.6** | QR `bsv:` URI. ⛔ 2 of the 4 sites hardcode `slice(8)`; widening the regex alone truncates the address and fails **closed**. |
| **0.7** | A failed `createAction` strands the UTXOs it reserved. Contract written. ⛔ The sweeper must verify the outpoint is unspent, or the fix becomes a double-spend. |
| **0.8** | The connect modal shows nothing. bitgenius.net declares 4 protocol permissions; we render 0. ⚠️ Settle which manifest shape is canonical BEFORE writing the parser. |
| **0.9** | Hodos branding on Chromium prompts. ⛔ **Browser logo, not wallet logo.** ⛔ No auto-allow — owner-agreed. Save-password is group B and may not be interceptable at all; **check before promising it**. |

## Decided — do not re-open

macOS ships beta.3 · concurrency TOCTOU refuted · the 429 balance drop self-heals · Task 2 items
1/2/3 fixed · the §4o disclosure set and `is_permission_surface`-as-subtree go to **Phase 5** ·
no auto-allow for loopback.

## Environment

- Dev wallet **31401**, adblock **31402**, browser CDP **9322**, launched
  `HODOS_DEV=1 ./HodosBrowser.exe --profile=Default --remote-debugging-port=9322`.
  Production is live on **31301 / 9222** — **never drive it**; kill dev **by executable path only**.
- ⛔ **Do not launch the dev wallet via `Start-Process powershell … -WindowStyle Minimized`** — it
  got reaped mid-session and every subsequent connect returned `no_wallet`, which reads exactly like
  a product bug. Launch it as a tracked background process instead.
- ⚠️ A C++ rebuild needs the dev browser closed. `cargo build` needs the dev wallet stopped.
- ⚠️ `PYTHONIOENCODING=utf-8` for anything printing log lines — cp1252 will throw on the emoji.

## Hard rules that cost real time this session

- ⛔ **A green is reported with its RED, and the RED must be the run YOU did.**
- ⛔ **A `data:` frame is opaque-origin.** A probe starting `parent.x = …` throws before it fires,
  and "no deny line" is indistinguishable from "gate held". Make the liveness signal the evidence.
- ⛔ **A page `fetch` to a wallet endpoint is caught by the C++ INTERCEPTOR, not by CORS.** Measure
  CORS with curl straight to actix, and keep a control that reaches the handler.
- ⛔ **`/listActions` with `labels: []` returns 0 actions.** That is the filter, not a gate.
- ⛔ **Safe probe address:** valid base58, 25 bytes, version `0x00`, **broken checksum**. Address→
  script conversion is inside `create_action_internal`, i.e. AFTER the gate. A format-invalid
  address 400s BEFORE the gate and measures nothing.
- 🚨 **Every failed probe strands its UTXOs until Phase 0.7 lands.** Three of mine stranded
  20,403,314 sats. Check the balance before and after a probe run and release with
  `probes/`-style verification.
- 🚨 **CHECK YOUR SATOSHI→USD ARITHMETIC.** `sats / 100,000,000 × price`. Twice.
- ⛔ **Payment-gate tests need an APPROVED domain** — `teragun.com` has a 13-cent per-tx cap, which
  makes over-cap unmistakable. An unapproved domain makes `domain_trust_mw` answer first.
- ⛔ **Panel your own fresh work.** Panel #3 found more in my fixes than in the code it was sent to
  review, including a blocker one row below the line I had just edited.
