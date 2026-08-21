# Session kickoff — close beta.3 Phase 0.5, then Phase 0.6

> Paste everything below the rule into a fresh session.
> **Rewritten 2026-08-21** at the end of the session that cleared adversarial panel #2 and closed
> Task 2 items 1/2/3. That work is committed and evidenced. What follows is what is still open.

---

Close **beta.3 Phase 0.5**, then start **Phase 0.6**.

Eleven money-path defects were fixed across the last two sessions, each with a paired RED. The
remaining work is **two measurements, one release-shaped gate, and a re-panel** — plus one blocker
that belongs to the macOS session.

## Read first, in this order

1. `development-docs/0.4.0-beta.3/phase-0.5-money-path/PHASE_CONTRACT.md` — **§4n is the current
   state**, including the Task 0/1/2 dispositions. §2 is the "done means" checklist.
2. `development-docs/0.4.0-beta.3/phase-0.5-money-path/TASK2_OWNER_DECISIONS.md` — items **4 and 5**
   are the only ones still open. 1, 2, 3 are fixed; 6 is answered.
3. `development-docs/0.4.0-beta.3/HARNESS.md` §5, §6, §8 — you will need the evidence-table format.
4. `development-docs/0.4.0-beta.3/MAC_RELAY_BETA3.md` round **2026-08-21** — what the macOS session
   owes back, so you do not duplicate it.
5. `CLAUDE.md` invariants **#1 (keys never in JS)**, **#2 (schema)**, **#13 (test-vs-code)**.

`git log --oneline a197502..HEAD` is the work under review (8 commits).

## ⛔ Verify before you trust

**Re-grep every file:line before relying on it.** Two commit messages in this phase's history
asserted audits that were demonstrably FALSE, and a banner comment claimed a property the code did
not have. Both cost real sessions. A code reading is a hypothesis with an experiment attached — not
a fact.

## State

| | Status |
|---|---|
| Panel #2 Tier-1 (6 defects) | ✅ fixed + evidenced — `775d87e`, `9e51134` |
| Task 2 items **1, 2, 3** | ✅ fixed + evidenced — `c8558dc`, `13e6e2d` |
| Task 2 item **6** (macOS scope) | ✅ answered — **beta.3 SHIPS macOS** |
| Task 0 concurrency | ✅ **REFUTED by measurement** — do not re-raise |
| **Task 2 item 5** — two-phase action lifecycle | 🔴 **MEASURE FIRST. Biggest unexamined surface.** |
| **Task 3** — `P0.5-G1` release-shaped build | ⬜ |
| **Task 4** — whole evidence table + panel #3 | ⬜ **panel #3 must cover macOS** |
| Task 2 item 4 — loopback-port trust | ⬜ Phase 5 |
| macOS **E1** SSRF | 🔴 BLOCKER — **macOS session**, owner gates when they send the relay |
| Concurrency hardening (latent) | ⬜ follow-up |

---

## Task A — MEASURE the two-phase action lifecycle before writing anything

`sign_action` (`handlers.rs`) has signature `(state, body)` — **no `HttpRequest` parameter**, so it
structurally cannot read `X-Requesting-Domain` and has no gate. Same for `/processAction`,
`/abortAction`, `/internalizeAction`, `/wallet/broadcast-nosend`.

⛔ **"Structurally ungateable" is NOT evidence of an exploit.** Nobody — not the panel, not me — has
walked createAction's *second* phase. The open questions are: are `PENDING_TRANSACTIONS` references
domain-bound? Can `spends` or `options.noSend` change the effective spend between phase 1 and phase 2?

Adding an `HttpRequest` parameter to five handlers is mechanical. Knowing what the gate should then
*decide* is not. **Measure, then bring a recommendation.**

## Task B — `P0.5-G1` on a release-shaped build

The one row that cannot be judged in dev: `IsFrontendAvailable()` short-circuits because dev has no
`frontend/` beside the exe. Pairs naturally with Phase 0's RC gates.

## Task C — re-run the WHOLE evidence table, then panel #3

⛔ **The whole table, not the failing rows** — that discipline is the only reason the fourth `:5137`
gate was ever caught.

⛔ **Rebuild before you measure.** Today's C++ fixes (`PaymentCost.h`, `extractDomain`) are in the
dev binary as of 2026-08-21, but any later edit needs a rebuild or you are testing yesterday's DLL —
the "right value, wrong subject" failure mode this phase exists to prevent.

⛔ **Panel #3 MUST cover macOS.** Panel #2 examined exactly **one line** of the macOS tree.
`cef_browser_shell_mac.mm`, the `Create*OverlayMacOS` roster and `InstallClickOutsideMonitor` are
unaudited by anyone. "Panel #2 cleared" means the **Windows** money path was cleared.

**Add regression tests as you go.** There are still **zero** C++ tests for `IsInternalOrigin`,
`ResolveIpcOrigin` or `IpcMessageAllowedFromWebPage`. `cef-native/tests/payment_cost_test.cpp` is the
pattern (add to the explicit list in `tests/CMakeLists.txt`).

---

## Owner decisions already taken — DO NOT re-litigate

- **beta.3 ships macOS.** This promoted the `wallet_call` SSRF to a sign-off blocker (relay ask E1).
- **Concurrency TOCTOU is REFUTED as an exploit.** 420 concurrent requests, 11 rounds, exactly 1 pass
  every time against a test designed so the correct answer *is* 1. Incidental DB-mutex serialization.
  Recorded LATENT with a hardening follow-up. **Not a blocker. Do not re-open it as one.**
- **The 429 mempool balance drop SELF-HEALS.** It was first written up as possible money loss and
  that was **corrected** — balance returned to within 20,975 sats, exactly the three on-chain backups
  in between. Transient under-report, not destroyed outputs. **Do not re-escalate.**
- C1/C2/C3 folded into Phase 0.5; **one sign-off, two halves**.
- **No user-facing disclosure** for the original C2 defect; non-mechanism release-note line only.
- beta.3 is a **tag**, not a branch — work continues on `0.4.0`.

## Hard rules — these cost real time and real money to learn

- ⛔ **A green is reported with its RED, and the RED must be the run YOU did.**
- ⛔ **Panel your own fresh work, not just the code you were sent to review.** An independent pass
  found more in the reviewer's own fixes than in the work it was sent to verify.
- ⛔ **Ask what result would look identical if the defect were absent.** A concurrency harness here
  measured "2" against a correct answer of 2 — a number that cannot discriminate. Only a negative
  control caught it.
- 🚨 **CHECK YOUR SATOSHI→USD ARITHMETIC.** `sats / 100,000,000 × price`. Do it twice. A 10× error
  once cost 4,000,000 sats, unrecoverable.
- ⛔ **Address validation runs BEFORE the payment gate** (`handlers.rs :: send_transaction`). A
  malformed-*format* address 400s before the gate and measures **nothing**. Use a **valid-prefix /
  invalid-checksum** address — it passes the gate and dies safely at build.
- ⛔ **Payment-gate tests need an APPROVED domain**, or `domain_trust_mw` answers first and the row
  goes green with the feature disabled. Discriminator: `promptType: payment_confirmation`.
- 🚨 **`peerpay_send` broadcasts to ANY well-formed identity key with no reachability check.** Never
  put a made-up key in a peerpay probe.
- ⛔ **NEVER write a second derivation of a security value.** `extractDomain` hand-rolled its own
  origin parse and treated `https://127.0.0.1:31301@evil.com/` as **wallet-internal**. The correct
  helper already existed and was already unit-tested. **Call it.**
- ⛔ **actix routes the percent-DECODED path; `HttpRequest::path()` returns the RAW one.**
- ⛔ **Gate SUBTREES, never a list of exact strings.** Enumerating is how two holes survived.
- ⚠️ **Chromium strips credentials from `location.href`, but CEF's `GetURL()` KEEPS them.** A page can
  look clean from inside and still carry userinfo the C++ side sees.
- ⚠️ **CORRECTED: Local Network Access does NOT block wallet fetch from a public https page.**
  MEASURED 2026-08-21: `fetch("http://127.0.0.1:31401/wallet/status")` from `https://example.com/`
  returned **HTTP 200**. Do not assume that transport is closed.
- ⚠️ **`POST /domain/permissions` is `#[serde(rename_all = "camelCase")]`** — snake_case keys are
  silently dropped to `None`, no error, the row just does not change.
- ⚠️ **Approving a domain via the API bypasses the modal entirely.** Say so out loud when you do it.
- ⚠️ **`addresses/unspent` is CONFIRMED-ONLY despite the name.**
- ⚠️ **A `data:` frame is opaque-origin** — the frame must run its own inline script.

## Driving the dev stack

- Wallet **31401**, adblock **31402**, browser CDP **9322**. Production is live on **31301 / 9222**
  and the owner uses it — **never drive it.**
- ⚠️ **Kill dev by EXECUTABLE PATH only** — prod shares the image names `HodosBrowser.exe` /
  `hodos-wallet.exe`. Dev wallet is under `rust-wallet/target/release`; dev browser under
  `cef-native/build/bin/Release`. Both stacks run concurrently; that is verified working.
- ⛔ **The dev browser does NOT open CDP by default**, and without `--profile=` it parks on the
  profile picker with no tab. Launch:
  `HODOS_DEV=1 ./HodosBrowser.exe --profile=Default --remote-debugging-port=9322`
- ⚠️ **11 CDP targets, ALL `type:"page"`.** Ten are overlays on `:5137`; the **tab** is the one at
  `/newtab`. Select by URL, refuse on ambiguity, return `location.href` with every value. Navigate
  tabs; never `PUT /json/new`.
- ⚠️ A C++ rebuild needs the **dev browser closed** (it holds `HodosBrowser.dll`). Production is
  unaffected.
- ⚠️ `cargo build … | tail` **discards the exit code** — use `PIPESTATUS[0]` *and* the `Finished` line.
- ⚠️ **Shell escaping has corrupted this repo's source.** Use `Edit`, or write a Python script to a
  file and run it — **never a large heredoc**. The docs are CRLF: multi-line patterns need `\r\n`.
- ⭐ **Owner interjections mid-work are a debugging instrument.** In the last session the owner
  clicking *Block* on a domain-approval modal became the end-to-end control that proved the
  `extractDomain` fix. Answer them properly and keep working.

## Then: Phase 0.6

Order is `0 → 0.5 → 0.6 → 1 → 2 → 3 → 4 → 5 → 6` (`SPRINT_PLAN.md` §4). Phase 0 ✅.
**Next after 0.5 is Phase 0.6 — the QR `bsv:` URI fix.** ⛔ 2 of the 4 sites hardcode `slice(8)`, so
widening the regex alone truncates the address and fails **closed**.
