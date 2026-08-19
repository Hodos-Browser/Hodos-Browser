# Session kickoff — CLOSE Phase 0.5, then wallet-reliability, then back to beta.3

> Paste everything below the rule into a fresh session.
> **Written 2026-08-19** at the end of the session that repaired Phase 0.5 (C1/C2/C3) and fixed the
> bulk-sync mempool blindness. That work is committed and evidenced. What follows is what is *left*.

---

Close **beta.3 Phase 0.5**, then two wallet-reliability items, then get back on the sprint.
⛔ **This is a repair-of-a-repair. The first Phase 0.5 implementation was refuted by its own
adversarial panel. Assume the same can happen to this one.**

## Read first, in this order

1. `development-docs/0.4.0-beta.3/phase-0.5-money-path/PHASE_CONTRACT.md` — **§4i is the live-run
   evidence and §5b the scope decision.** The evidence table is split into halves 0.5a / 0.5b.
2. `development-docs/0.4.0-beta.3/phase-0.5-money-path/ADVERSARIAL_PANEL_2026-08-19.md` — findings
   **4, 5, 6** are the ones still open. 1/2/3 are closed.
3. `development-docs/0.4.0-beta.3/HARNESS.md` §2, §6.
4. `CLAUDE.md` invariants **#2 (schema), #3 (crypto), #13 (test-vs-code)**.

`git log --oneline 637e216..HEAD` is the work under review: `8d0f270` `aa439d3` `9dc1586` `51b81b7`
`293f269`.

## ⛔ Verify before you trust

Every file:line in the docs above was written against a tree that has since moved five commits.
**Re-grep every citation before relying on it.** The previous two sessions were each derailed by
inheriting an untested claim.

## State

| | Status |
|---|---|
| C1 (scheme-anchored origin), C2 (default-deny IPC gate), C3 (peerpay/paymail gated) | ✅ landed, GREEN with RED observed live |
| `X1 X2 X3 X4 X5 E1 R1 R2 R3`, `G2 G3` | ✅ |
| `R4` | ⛔ withdrawn — no subject |
| `G1` | 🟡 needs a **release-shaped** build |
| **Finding 4 + 5 — CORS** | 🔴 **agreed fix, not written** |
| **Finding 6 — "0 sats"** | 🔴 open |
| Post-repair adversarial panel | ⬜ owed |

## Task 1 — CORS (findings 4 + 5). In-sprint, blocking.

**Measured:** `block_on_origin_mismatch(true)` returns **400 before any handler** for both
`Origin: http://127.0.0.1:31301` (what Chromium stamps on a re-issued POST) and
`https://zanaadu.com`. Negative control: comment the line out → both 200. **This currently breaks
external dApp interop and cannot ship.**

Agreed three-part fix:

1. **Strip, as a DENYLIST**, from `originalHeaders_` before the interceptor re-issues
   (`HttpRequestInterceptor.cpp` ~`:3651`): `Origin`, `X-Requesting-Domain`, `X-User-Approved`,
   `X-Browser-Id`, `X-Payment-*`.
   ⛔ **Must remain an allow-everything-else denylist** — `x-authrite-*` / `x-bsv-*` BRC-31 headers
   are forwarded deliberately and an allowlist would silently break dApp auth.
   This also closes **finding 5**: page-supplied headers currently *overwrite* the C++-derived trust
   headers (CEF serialises duplicates into a case-insensitive `SetHeader`; the page's value goes in
   last), so a page sending an empty `X-Requesting-Domain` is treated as **internal**.
2. **Allowlist** `http://127.0.0.1:31301` and `http://127.0.0.1:31401` in `main.rs` CORS.
3. **Keep** `block_on_origin_mismatch(true)`.

**Why (2) is safe:** `Origin` is a forbidden header name — page JS cannot set it, only the browser
can. A hostile page always gets its own origin stamped and stays blocked.
⚠️ **That claim is reasoning, not measurement. Test it**: try to set `Origin` from a page and
confirm it is refused.

**Sign-off needs a live dApp call** (zanaadu.com or the BRC-121 test site) proving interop is
restored. Do not claim it from reasoning.

## Task 2 — Finding 6, the "0 sats" prompt. In-sprint.

⛔ **There is NO price outage. Do not "fix" the price cache.** Measured all session: dev and prod
both returned $14.89–$15.25, three providers plus SQLite persistence, zero provider failures. The
wallet's own log names the real cause:
`no X-Payment-* headers — forcing price_unavailable prompt (C++ injection bug or non-engine caller)`.

`isPaymentEndpoint` is only `{/createAction, /acquireCertificate, /sendMessage}`, so
`/transaction/send`, `/wallet/peerpay/send` and `/wallet/paymail/send` arrive header-less, Rust
substitutes `{satoshis:0, cents:0, bsv_price_available:false}`, and `matrix_c.rs:240` prompts
`PriceUnavailable` **before** every cap branch. The modal then says *"verify the satoshi amount
above"* above a **0** — worst case a `sendMax` full-balance sweep displayed as **"0 sats"**.

⚠️ `d33741a`'s warning applies: adding these routes to `isPaymentEndpoint` **without** teaching
`extractOutputSatoshis` their body shapes prices them at **0 cents and silently auto-approves** —
strictly worse. **Do both or neither.** There are **three** shapes:
`{toAddress, amount, sendMax}`, `{recipient_identity_key, amount_satoshis}`, `{paymail, amount_satoshis}`.

## Task 3 — Address rotation (out of sprint, owner-approved)

`get_current_address` returns the **highest-index** address, not an unused one, so it hands back the
just-created **change** address → address reuse. Fix: return highest-index **unused**, generate only
if none exists (≈one address per received payment, no bloat).
⚠️ `generate_address` is ~182 lines with BRC-42 derivation inline — extract a shared helper.
**Do not change derivation itself** (invariant #3).

## Task 4 — BananaBlocks as a second UTXO provider (out of sprint; DESIGN FIRST)

Motivation, measured: **WoC mempool indexing lags 40s–200s+** and once never surfaced a tx before it
confirmed. BananaBlocks exposes **WoC-shaped** paths incl.
`POST /api/v1/bsv/main/addresses/unconfirmed/unspent`; 60/min anonymous, **600/min free**, auth
optional. Note `utxo_fetcher::fetch_utxos_bulk` uses **raw reqwest and bypasses the `services`
provider chain** entirely — that is the architectural gap.

⛔ **The deconfliction rule, decided by the owner and non-negotiable:**
**discovery may take the UNION; reconciliation must require AGREEMENT.** The sync marks DB outputs
*absent from the API* as `external-spend`. With two providers, an output missing from one but
present in the other must **NOT** be marked spent — otherwise a lagging provider silently deletes
real money. Same principle as the existing "caches must not poison themselves with failure-derived
values" rule. **Bring a design before writing code.**

## Task 5 — Adversarial panel, then sign-off

Run a panel over `637e216..HEAD` with distinct lenses — *can a page still reach a money endpoint* ·
*can an origin still be forged* · *did the default-deny break the first-party UI* · *is the UTXO
merge double-counting or under-counting*. The first panel caught 23 findings including 3 criticals;
budget for it finding real things.

Then `P0.5-G1` on a release-shaped build, and sign off both halves together — **neither half signs
off alone**.

## Hard rules carried forward — these cost real time to learn

- ⛔ **A green is reported with its RED, and the RED must be the run you actually did.**
- ⛔ **Payment-gate tests need an APPROVED domain.** With an unknown domain, `domain_trust_mw`
  returns 202 *before* `dispatch_payment` runs, so the row goes green with the feature disabled.
  That already happened once. Discriminator: `promptType: payment_confirmation`, not `domain_approval`.
- ⛔ **A `data:` frame is opaque-origin** — the parent CANNOT script into it
  (`f.contentWindow.cefMessage` throws `SecurityError`). The frame must run its **own inline
  script**. A harness using the parent-reach shape reports green against vulnerable code.
- ⛔ **`addresses/unspent` is CONFIRMED-ONLY despite the name.** Do not re-propose it as a one-liner.
- ⛔ **Re-run the WHOLE evidence table after a fix**, not the failing row.
- ⚠️ Dev CDP is **9322** (9222 **+100** under `HODOS_DEV`); production is 9222 — **never drive
  production**. **11 CDP targets all report `type:"page"`** — select by URL, never index. Navigate
  tabs; never `PUT /json/new` (bypasses `OnBeforeBrowse`).
- ⚠️ Kill dev processes **by executable path** — prod shares the image name. Dev/prod deconfliction
  is verified working; both can run concurrently.
- ⚠️ `cargo build … | tail` / `| grep` **discards the exit code** — the `Finished` line is the
  evidence.
- ⚠️ **Shell escaping has corrupted this repo's source twice** (raw form feeds). Use `Edit`, or
  hex-only `perl` escapes tested on a copy first.
- ⚠️ Schema changes need **explicit owner approval** (invariant #2). Several tempting fixes here
  want a new column — ask first.

## Owner decisions already taken — do not re-litigate

- C1/C2/C3 folded into Phase 0.5; **one sign-off, two halves**.
- **No user-facing disclosure.** Handful of testers, ~$100 aggregate, and **no mitigation exists** —
  there is no wallet lock at all and DPAPI auto-unlocks a no-PIN wallet at startup. Ship a
  non-mechanism release-note line: *"fixes an issue where a website could initiate a payment without
  your approval; please update."*
- Sync/mempool fix was pulled in out-of-scope deliberately.

## Then: back to the beta.3 sprint

Order is `0 → 0.5 → 0.6 → 1 → 2 → 3 → 4 → 5 → 6` (`SPRINT_PLAN.md` §4). Phase 0 ✅, Phase 0.5
closing here. **Next is Phase 0.6 — the QR `bsv:` URI fix.** ⛔ 2 of the 4 sites hardcode `slice(8)`,
so widening the regex alone truncates the address and fails **closed**.
