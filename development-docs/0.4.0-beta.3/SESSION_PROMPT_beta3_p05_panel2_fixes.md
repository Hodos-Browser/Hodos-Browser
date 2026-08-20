# Session kickoff — clear adversarial panel #2, then close Phase 0.5

> Paste everything below the rule into a fresh session.
> **Written 2026-08-20** at the end of the session that landed the in-sprint fixes, fixed dApp
> interop, and ran adversarial panel #2. That work is committed and evidenced. What follows is what
> the panel says is still wrong — **including four defects that same session introduced.**

---

Clear the blockers from **adversarial panel #2**, then close **beta.3 Phase 0.5**.

⛔ **The previous session both repaired this phase and wrote new fixes, then panelled its own work.
Four of the six criticals below are defects IT introduced, and two of its commit messages assert
audits that are demonstrably FALSE. Assume the same can happen to you.**

## Read first, in this order

1. `development-docs/0.4.0-beta.3/phase-0.5-money-path/ADVERSARIAL_PANEL_2_2026-08-20.md` —
   **§3 of the completeness critic is the blocker list.** Start there, not at finding #1.
2. `development-docs/0.4.0-beta.3/phase-0.5-money-path/PHASE_CONTRACT.md` — **§4j** (the repair
   re-run), **§4k** (permission table, fixed), **§4m** (interop). §2 is the "done means" checklist.
3. `development-docs/0.4.0-beta.3/HARNESS.md` §5, §6, §8.
4. `CLAUDE.md` invariants **#1 (keys never in JS)**, **#2 (schema)**, **#13 (test-vs-code)**.

`git log --oneline 3947bd3..HEAD` is the work under review.
Raw panel output: `phase-0.5-money-path/adversarial_panel_2_raw.json` (37 survivors, 63 negatives).

## ⛔ Verify before you trust

**Re-grep every file:line in the panel and the contract before relying on it.** The panel read a
tree that has since moved, and three prior sessions were each derailed by inheriting an untested
claim. The panel labels every finding MEASUREMENT or CODE_READING — **most are code readings.**
Treat a code reading as a hypothesis with an experiment attached, not as a fact.

## State

| | Status |
|---|---|
| CORS (findings 4+5), finding 6, `stoi` guard, UTXO `confirmed`, §4k permission table | ✅ landed, GREEN with REDs run in-session (§4j, §4k) |
| ⭐ dApp interop — a real HandCash-built dApp now **connects** | ✅ `9b73bd7`, GREEN with RED (§4m) |
| `X1 X2 X3 X4 X5 E1 R1 R2 R3 R4 C1`, `G2 G3` | ✅ |
| `G1` | 🟡 needs a **release-shaped** build |
| **Panel #2 blockers** | 🔴 **6 critical + 2 false audit claims — this is your job** |
| **Concurrency** | ⬜ **never examined, potentially blocker-class — measure FIRST** |
| Post-fix re-panel | ⬜ owed |

---

## Task 0 — MEASURE THE CONCURRENCY QUESTION BEFORE WRITING ANY FIX

The completeness critic found this and **no lens looked at it.** `dispatch_payment_with_amount`
takes **three separate lock acquisitions** — snapshot (`request_gate.rs:~1141`), decide (`:~1152`),
then `increment_payment_rate_counter` + `record_spending` (`:~1166`, `:~1176`) — and
`HttpServer::new` (`main.rs:~964`) sets **no `.workers()`**, so actix runs `num_cpus` workers.

**Hypothesis:** N concurrent sub-cap payments from one `browser_id` all read `spent_cents` before
any write lands, all decide `Silent` ⇒ per-session cap, max-tx-per-session **and** rate limit all
defeated at once, with **no parsing trick and no extra approval**.

If it holds it outranks every fix below. It is currently a **code reading**. Measure it:
fire K parallel sub-cap payments at an **approved** domain and compare total spend against
`per_session_limit_cents`. ⛔ Use a scratch domain and amounts that sum to a trivial figure — see
the arithmetic warning below.

---

## Task 1 — the six Tier-1 regressions (four were introduced by the last session)

### 1.1 🚨 Decoy `outputs` key ⇒ silent auto-approval of ANY amount
`cef-native/include/core/PaymentCost.h :: ExtractOutputSatoshis` tests
`contains("outputs") && is_array()` **first and returns from that branch unconditionally**. So
`{"outputs":[], "toAddress":"1…", "amount":100000000}` → 0 satoshis → `ComputePaymentCost` reports
`priceAvailable=true` with 0 cents → `matrix_c` `SilentWithinCaps`. Rust's `SendTransactionRequest`
has **no `deny_unknown_fields`**, so serde ignores the decoy and spends the real `amount`.
**Fix shape:** an empty/absent `outputs` must fall through, and a body carrying **two** amount
shapes must be treated as not-derivable (fail closed), not first-match. Add `deny_unknown_fields`
to the affected Rust request structs. **The existing test file does not cover a body with both
keys — that is why the negative control passed.**

### 1.2 🚨 `createAction` `options.sendMax` ⇒ full-wallet sweep, silently
Only *top-level* `sendMax` is handled. `handlers.rs:~4774` reads it from `options`, and
`:~5196` computes `total_output = total_input − fee − …`. Both C++ and Rust re-read the same wrong
quantity, so **both layers miss identically**.

### 1.3 🚨 §4k gate bypassed by ONE percent-encoded character
`/domain/%70ermissions`. **actix routes on the percent-DECODED path; the gate reads the RAW one**
(`main.rs:~102-108`; `actix-router-0.5.3/src/url.rs`, `actix-web-4.11.0/src/request.rs`). This one
is **MEASURED** against the pinned versions. Match the same path actix matches.

### 1.4 🚨 `POST /wallet/session/close` is page-callable and zeroes all three counters
`clear_session_for_browser` is one `guard.remove(&browser_id)` dropping `spent_cents`,
`payment_count_this_session` **and** `payment_requests_this_minute`.
⛔ **Missed because `81c054c` enumerated three path strings — while its own comment warned that
enumerating instead of gating a subtree is exactly how the original hole survived. Gate the
subtree. Do not add a fourth string.**

### 1.5 `window.yours.disconnect()` is silently broken
`81c054c`'s "verified at every call site" is **false**: `cef-native/include/core/CWIShimScript.h:547-557`
is **page-context JS** issuing `DELETE /domain/permissions`, which the new gate now 403s.
Decide: permit that one page-originated write, or change the shim. **Correct the commit-message
claim in the contract either way.**

### 1.6 `confirmed=1` is still live on `wallet_recover` / `wallet_rescan`
`4eacb51`'s "recovery.rs needs none" was **file-scoped, not dataflow-scoped**.
`output_repo.rs :: upsert_received_utxo_with_derivation` **omits `confirmed`** from its INSERT, so
it takes schema default 1. Called from `handlers.rs:~15389` and `:~15610`.

---

## Task 2 — owner decisions owed (pre-existing, NOT introduced by this phase)

Bring these back with a recommendation; do not fix unilaterally (CLAUDE.md #13).

1. **`IsInternalOrigin("") == true` + a SECOND unhardened derivation at `extractDomain`**
   (`HttpRequestInterceptor.cpp:~5173`). ⇒ C1's banner claim *"ONE derivation, applied ONCE"* is
   **FALSE**. The contract must be corrected regardless of whether the code changes.
2. **`/wallet/reveal-mnemonic` after one Allow click** on the no-PIN branch — returns the BIP39
   phrase to page context. **Conflicts with Invariant #1 as written.**
3. **`POST /wallet/settings` page-callable** — rewrites `default_per_tx/per_session/rate` that every
   *future* approval inherits, plus `default_identity_key_disclosure_allowed`.
4. **Any page on any loopback port gets full first-party IPC trust** — Phase 5's `IsWalletOrigin()`
   territory, but the panel rates it higher than "follow-up".
5. **The two-phase action lifecycle:** `sign_action` has **no `HttpRequest` parameter**, so it
   structurally cannot be gated. Same for `processAction` / `abortAction` / `internalizeAction` /
   `broadcast-nosend`.
6. **macOS** — the curl-userinfo SSRF promotes to blocker **if beta.3 ships macOS**. Ask.

---

## Task 3 — `P0.5-G1` on a release-shaped build

The one row that cannot be judged in dev: `IsFrontendAvailable()` short-circuits because dev has no
`frontend/` beside the exe. Pairs naturally with Phase 0's RC gates.

## Task 4 — re-run the WHOLE evidence table, then re-panel

⛔ **The whole table, not the failing rows** — that discipline is the only reason the fourth `:5137`
gate was ever caught. Then run panel #3 over the new range. Panel #2 found six page-reachable money
paths; a repair of this size that is only self-reviewed repeats the mistake this phase exists to fix.

**Add regression tests as you go.** The panel found **zero** C++ tests for `IsInternalOrigin`,
`ResolveIpcOrigin` or `IpcMessageAllowedFromWebPage` — the phase's headline control is untested on
the C++ side. `cef-native/tests/payment_cost_test.cpp` is the pattern (add to the explicit list in
`tests/CMakeLists.txt`).

---

## Hard rules carried forward — these cost real time and real money to learn

- ⛔ **A green is reported with its RED, and the RED must be the run YOU did.**
- 🚨 **CHECK YOUR SATOSHI→USD ARITHMETIC.** A 10× error picked probe amounts believed to be over a
  $1.00 cap that were actually under it ⇒ two real transactions broadcast, **4,000,000 sats
  (~$0.63) unrecoverable**. The same 10× error was then repeated in the damage report.
  `sats / 100,000,000 × price`. Do it twice.
- 🚨 **`peerpay_send` broadcasts to ANY well-formed identity key with no reachability check.** A
  fabricated recipient key destroys the funds. Never put a made-up key in a peerpay probe.
- ⛔ **Payment-gate tests need an APPROVED domain**, or `domain_trust_mw` answers first and the row
  goes green with the feature disabled. Discriminator: `promptType: payment_confirmation`.
- ⛔ **A `data:` frame is opaque-origin** — the parent cannot script into it. The frame must run its
  **own inline script**.
- ⛔ **`addresses/unspent` is CONFIRMED-ONLY despite the name.**
- ⛔ **`POST /domain/permissions` is `#[serde(rename_all = "camelCase")]`** — snake_case keys are
  silently dropped to `None`, no error, the row just does not change.
- ⚠️ **Approving a domain via the API bypasses the modal entirely.** Say so out loud when you do it;
  it is a trust grant the owner never saw.
- ⚠️ Dev CDP **9322**, prod **9222** — never drive production. **11 CDP targets all report
  `type:"page"`** — select by URL, refuse on ambiguity, and return `location.href` with every value.
  Navigate tabs; never `PUT /json/new`.
- ⚠️ **Kill dev processes by executable path** — prod shares the image name `HodosBrowser.exe` /
  `hodos-wallet.exe`. Both stacks run concurrently and that is verified working.
- ⚠️ `cargo build … | tail` **discards the exit code** — use `PIPESTATUS[0]` *and* the `Finished` line.
- ⚠️ **Shell escaping has corrupted this repo's source.** Use `Edit`, or write a Python script to a
  file and run it — **never a large heredoc**. The docs are CRLF: multi-line patterns need `\r\n`.
- ⚠️ **Chromium 150 gates the direct-fetch transport behind a Local Network Access prompt.** A public
  https page must be granted permission before it may reach `127.0.0.1` at all; the failure is
  `Failed to fetch` with **no network event**, and it never reaches our CORS layer.
- ⭐ **Test the request the SITE actually makes.** Two wrong conclusions about the HandCash App Lab
  ("its CSP blocks us", "it hardcodes competitor ports so it cannot work") both came from probing a
  URL the site would never call. Right value, wrong subject — the farbling failure mode.
- ⭐ **Owner interjections mid-work are a debugging instrument.** In the last session they caught a
  gold-pill event, identified the Chromium LNA prompt, and surfaced the §4k escalation. Answer them
  properly and keep working.

## Owner decisions already taken — do not re-litigate

- C1/C2/C3 folded into Phase 0.5; **one sign-off, two halves**.
- Finding 4 → **allowlist `127.0.0.1:31301`/`:31401` + strip page-supplied trust headers, KEEP
  `block_on_origin_mismatch(true)`.** Landed.
- Finding 6 → **Option A**: Rust resolves `sendMax` from the spendable balance, and the six
  duplicated pricing sites were unified onto one helper.
- §4k → **fix in beta.3.** Landed (then found bypassable — Task 1.3/1.4).
- Interop fix → **do it now**, out of scope, owner-approved. Landed.
- **No user-facing disclosure** for the original C2 defect; non-mechanism release-note line only.
- beta.3 is a **tag**, not a branch — work continues on `0.4.0`.

## Then: back to the beta.3 sprint

Order is `0 → 0.5 → 0.6 → 1 → 2 → 3 → 4 → 5 → 6` (`SPRINT_PLAN.md` §4). Phase 0 ✅.
**Next after 0.5 is Phase 0.6 — the QR `bsv:` URI fix.** ⛔ 2 of the 4 sites hardcode `slice(8)`, so
widening the regex alone truncates the address and fails **closed**.
