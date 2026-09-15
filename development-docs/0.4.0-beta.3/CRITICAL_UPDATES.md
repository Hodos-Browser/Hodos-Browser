# beta.3 — critical updates from the 2026-09 BSV wallet advisories

**Filed:** 2026-09-15 · **Owner:** Matthew Archbold · **Status:** 📌 **SCHEDULED 2026-09-15 as beta.3 Phase 10** (`phase-10-critical-advisories/`: 10a CU-3+CU-6 → 10b CU-1+CU-8+CU-9 → 10c CU-2). 👤 Owner decisions: **no CU-3 stopgap** — automatic PeerPay acceptance stays on and the real fix (subject binding) keeps it; burst handling is fixed at **every** call path and the user is told about a burst **once**; CU-4/CU-5/CU-7 re-read at the Phase 10 kickoff and scheduled then. §3's ARC key: rotate now, and a per-Chromium-bump check added to `CEF_VERSION_UPDATE_TRACKER.md`.
**Trigger:** three BSV Association advisories against their TypeScript wallet stack. Hodos ships none of
those packages; `frontend/package-lock.json` has no `@bsv/*` entry *(measured)*. Our Rust wallet and C++
shell implement the same protocols, so the code was reviewed for the **same bug shapes**.

| Advisory | Severity | Published | What it covers |
|---|---|---|---|
| [GHSA-5vmp-9hjc-rfwp](https://github.com/bsv-blockchain/ts-stack/security/advisories/GHSA-5vmp-9hjc-rfwp) | critical | 2026-09-10 | Eight findings: spending-approval reuse, BRC-121 Atomic BEEF subject confusion, ignored certificate and signature verification, relay fail-open, credentials over HTTP |
| [GHSA-36f9-7rg5-cpf8](https://github.com/bsv-blockchain/ts-stack/security/advisories/GHSA-36f9-7rg5-cpf8) (CVE-2026-56744) | high | 2026-07-06 | Storage-supplied output scripts signed without checking them against the requested outputs |
| [GHSA-vjpq-xx5g-qvmm](https://github.com/bsv-blockchain/ts-sdk/security/advisories/GHSA-vjpq-xx5g-qvmm) | medium | 2026-02-17 | BRC-104 auth signature data built from concatenated base64 strings |

> ⛔ **Method, and its limits.** Every finding here is a **code reading** on `origin/0.4.0` at `ec6353e`
> (2026-09-15). **Nothing was built, run or exploited.** Four parallel read-only reviews produced the
> candidates.
> - **CU-1, CU-2 and CU-3** were then re-read by hand at the cited lines, and the `v0.3.0-beta.29` tag
>   was checked for the same code patterns.
> - Items in §2 are reviewer readings that were **not** independently re-read; they are marked as such.
> - No item has a RED observed yet. Each fix needs its RED before its GREEN, per `HARNESS.md`.

---

## 0. Summary

| ID | Defect | Severity | Attacker needs | In `v0.3.0-beta.29`? | Fix size |
|---|---|---|---|---|---|
| **CU-1** | One Approve click releases **every** pending prompt for that domain | 🔴 High | A connected site plus one click | Pattern present *(read)* | S–M (C++ and React) |
| **CU-2** | A paymail P2P host can replace the approved amount with any outputs | 🔴 High | Control of the recipient's paymail host | Pattern present *(read)* | S (Rust) |
| **CU-3** | A fabricated PeerPay payment is credited and later marked confirmed | 🔴 High | The victim's identity key only | Pattern present *(read)* | S (Rust) |
| CU-4 | Certificate signature check skipped for a non-txid `revocationOutpoint` | 🟠 Medium | A connected site, or a certifier URL it supplies | not checked | S |
| CU-5 | Identity search labels unverified overlay certificates ("via SocialCert") | 🟠 Medium–High | Getting a certificate admitted to `ls_identity` | not checked | S–M |
| CU-6 | `internalizeAction` logs a subject mismatch and records the wrong transaction | 🟠 Medium | A counterparty or dApp | not checked | S |
| CU-7 | Session spend counters reset on navigation or tab close; sub-cent spends count as 0 | 🟠 Medium | A connected site | not checked | M |
| CU-8 | Check-then-act race between the limit snapshot and the spend record | 🟡 Low–Medium | A connected site sending concurrent calls | not checked | S |
| CU-9 | 402 reuse cache keyed on URL and sats only (no domain, no server key) | 🟡 Low | A second connected site within 25 s | not checked | XS |

~~**Recommended order:** CU-1 → CU-3 → CU-2 inside beta.3. CU-3's stopgap (§1.3) can ship first if the
full fix slips.~~ 👤 **Owner order (2026-09-15): CU-3 → CU-1 → CU-2, no stopgap.** CU-3 first because it needs no user action and runs on the poller; the stopgap is declined because the real fix keeps auto-accept. CU-6, CU-8, CU-9 ride with 10a/10b; CU-4, CU-5, CU-7 are decided at kickoff.

---

## 1. Must fix before beta.3 ships

### CU-1 — one Approve releases every pending prompt for the domain

#### The defect *(read, re-verified by hand)*

1. Each over-limit payment makes Rust return **202** with its own single-use approval ID, bound to a
   sha256 of that request body (`permission_service/state.rs:301-363`). This part is sound.
2. C++ turns each 202 into a pending entry plus a modal (`tryHandlePendingResponse`,
   `HttpRequestInterceptor.cpp:3103`), storing `X-User-Approved = <that request's id>` in
   `headersOnApprove` (`:3135-3137`). `openPaymentConfirmationModal` (`:1464-1471`) adds **one entry and
   one modal per request**, with no per-domain dedupe.
3. The notification overlay is **reused**. Each new modal replaces the visible one by JS injection
   (`simple_app.cpp`, keep-alive path). **The user sees whichever prompt was posted last.**
4. The payment Approve button sends **no request ID**: `JSON.stringify({ approved: true })`
   (`BRC100AuthOverlayRoot.tsx:917-922`). C++ falls back to `getRequestIdForDomain`
   (`simple_handler.cpp:5192-5198`).
5. `handleAuthResponse` resolves that entry, then `popAllForDomain(domain)` and **resumes every
   sibling** (`HttpRequestInterceptor.cpp:3617-3658`). Each sibling replays its **own** stored approval
   ID, so `request_gate.rs:1057-1070` accepts each one and returns `Proceed` without re-running the cap.

**Result:** N prompted payments, one visible prompt showing one amount, one click, N signed transactions.
It also covers any other prompt kind pending for the domain (rate-limit, protocol grant, certificate
disclosure, key reveal), because they share the same fan-out.

#### How this interacts with the auto-approve engine

**Payments within the user's settings never reach this bug, and the fix doesn't change them** *(read)*:

- The engine's `Silent` decision returns `Proceed` in Rust (`request_gate.rs:1150-1175`). The handler
  runs, the response is 200, and **no 202, pending entry or modal is ever created**. `popAllForDomain`
  can only return requests that were already denied auto-approval.
- So a site that sends ten requests, all inside the per-transaction cap, session cap and rate limit,
  gets ten silent payments today and ten silent payments after the fix.

**Where auto-approve and this bug meet:** a burst that **crosses** a limit partway through.

> Example: session cap $10, site sends five $3 payments at once. The first three go Silent ($9). The
> fourth and fifth exceed the session cap and each return 202, giving two pending entries and two
> modals, with the second modal replacing the first. The user approves "one $3 payment" and **both**
> are sent. With larger or unequal amounts the gap is unbounded: the site can post the small one last.

⚠️ CU-8 (the race) makes the same burst worse: concurrent requests can all read the counters before any
records, so more than three may go Silent.

**The existing fan-out is correct for connect prompts, and the fix must keep it.** `domain_approval`,
`brc100_auth` and `manifest_connect_bundle` use `addRequestIfFirstForDomain`
(`HttpRequestInterceptor.cpp:1247-1300`). Duplicate requests from a not-yet-connected site queue behind
**one** modal. On Approve, the queued siblings are re-issued **without** an approval token, so the
engine evaluates each one fresh (`:3122-3137`, the `isDomainTrustPrompt` branch). That is the model the
payment path should follow.

#### Fix shape

1. **Bind the approval to one request.** The React modal receives `requestId` in its query and sends it
   back in `brc100_auth_response` (and in every approve/deny message for kind prompts). C++ resolves
   **only** that ID. A missing ID fails closed: a denial, not the legacy domain lookup.
2. **Stop the fan-out for kind prompts.** `popAllForDomain` stays for connect-type prompts only. For
   payment, rate-limit, protocol, certificate and key prompts, siblings are **not** resumed with their
   tokens.
3. **Show pending prompts one at a time.** Today a second 202 overwrites the visible modal, so after
   fix 2 the earlier requests would sit invisible until timeout. Queue kind prompts per domain and show
   the next when the current one resolves, **or** deny superseded prompts immediately with a clear
   error so the site retries. A retry goes through the engine fresh: Silent if now within limits,
   otherwise a new prompt. **Owner decision:** queue or deny. Queueing is kinder to legitimate batch
   payers; denying is simpler and harder to abuse.
4. **The modal must show the amount of the request it approves.** This follows from 1 but should be
   asserted.

#### Acceptance

| Row | RED (before) | GREEN (after) |
|---|---|---|
| CU-1-A | Connected test page posts 2 payments over the per-tx cap concurrently; one Approve → **2** txids | One Approve → exactly **1** txid, and it is the amount shown |
| CU-1-B | — | The second request either shows its own modal next or returns a denial; it never broadcasts without its own click |
| CU-1-C (auto-approve non-regression) | — | 5 concurrent payments all within per-tx, session and rate limits → 5 Silent txids, **0** modals |
| CU-1-D (connect non-regression) | — | Fresh site fires 3 calls before connecting → **1** connect modal; after Approve all 3 are evaluated fresh (Silent or prompted per limits) |
| CU-1-E | Approve message with no `requestId` resolves a request | Rejected, nothing resolved |

Regression-set candidate: **R-ONE-CLICK-ONE-SPEND** (CU-1-A plus CU-1-C) in `REGRESSION_SET.md`.

---

### CU-2 — paymail P2P destination can replace the approved amount

#### The defect *(read, re-verified by hand)*

- `paymail_send` gates on `amount_satoshis` from the request body (`handlers.rs:18940`, priced in
  `PaymentCost.h`).
- It then asks the recipient's paymail host for P2P outputs (`handlers.rs:18976-18998`,
  `paymail.rs:315-349`) and builds `CreateActionOutput`s straight from `o.satoshis` and `o.script_hex`.
- **No check** compares the sum of those outputs with `amount_satoshis`, and none limits their count or
  script type.
- The outputs go to `create_action` through an internal request with **no** `X-Requesting-Domain`
  (`handlers.rs:19062-19063`). `dispatch_payment` treats that as internal and returns `Proceed`
  (`request_gate.rs:1036-1043`).

**Attack:** the user, or a connected site calling the endpoint, sends 1,000 sats to `x@evil.example`.
That host returns outputs totalling 5,000,000 sats. The wallet signs and broadcasts with no further
prompt. A compromised host for a legitimate recipient does the same.

**Also** *(reviewer reading, not re-verified)*: capability URLs from `.well-known/bsvalias` have no
scheme check (`paymail.rs:230-248`), so an `http://` P2P endpoint would let a network attacker swap the
outputs.

#### Fix shape

1. Reject unless `sum(outputs.satoshis) == amount_satoshis`. A host that splits the amount across
   outputs still sums to it. *(Inferred from the purpose of the bsvalias P2P destination capability;
   confirm against the spec text before coding.)*
2. Limit the output count (suggest ≤ 100) and reject a zero or negative value in any output.
3. Require `https://` for every capability URL used on the send path.
4. Consider pricing the **built** transaction at the gate rather than the request body. That closes
   this whole class for any internal caller. Larger change, so list it as a follow-up.

#### Acceptance

| Row | RED | GREEN |
|---|---|---|
| CU-2-A | Local stub paymail host returns outputs totalling 10× the request → broadcast (use `noSend` or testnet) | Rejected before signing, with an error naming the mismatch |
| CU-2-B | — | Stub returns outputs summing exactly to the request, split across 3 outputs → succeeds |
| CU-2-C | Stub capability URL is `http://` → used | Rejected |

---

### CU-3 — fabricated PeerPay payment is credited, then auto-confirmed

#### "Don't we check every MessageBox payment against the chain already?"

**Yes, and the check runs. It checks the wrong thing.** *(read, re-verified by hand)*

Atomic BEEF (BRC-95) is a 36-byte header (magic plus **declared subject txid**) followed by a BEEF
bundle of transactions. `task_check_peerpay.rs` uses two different parts of that envelope and never
ties them together:

| Step | Line | Uses |
|---|---|---|
| Parse | `:230` `from_atomic_beef_bytes` | Returns the **declared** txid from the header. `beef.rs:82-101` never checks that any transaction in the bundle hashes to it |
| Find our output | `:240-284` | `beef.main_transaction()` = **the last transaction in the bundle** (`beef.rs:232`). Value, vout and script come from here |
| On-chain check | `:319` `check_tx_exists_on_chain(subject_txid)` | The **declared** txid. The function (`handlers.rs:6946`) asks only whether that txid is mined or in mempool; it never fetches outputs |
| Store | `:372-377` `store_derived_utxo(subject_txid, found_vout, found_satoshis, <last-tx script>)` | Declared txid **plus** value and script from the last transaction |

**Why the check doesn't protect us:** a txid commits to a transaction's outputs, so "this txid is on
chain" proves the amount **only if the amount was read from the transaction with that txid**. Here it
wasn't.

**Attack** (anyone who knows the victim's identity key can post to their `payment_inbox`):
1. The declared subject is **any real mined txid** T1, so the on-chain check passes (`:320`).
2. The last transaction T2 is **fabricated and never broadcast**. It pays, say, 1,000,000 sats to the
   BRC-29 key derived from the attacker's chosen prefix and suffix, which our derivation matches.
3. The wallet inserts `T1:vout` worth 1,000,000 sats, `spendable = 1`, `confirmed = 0`, and records a
   `peerpay_received` notification (`:399`).
4. About 30 minutes later, `task_sync_pending.rs` stale handling asks WhatsOnChain for **T1's
   confirmation count only** and runs `mark_output_confirmed` (`output_repo.rs:528`).
5. Coin selection now includes it (`output_repo.rs:146`: `transaction_id IS NULL AND confirmed = 1`).

**Consequences:**
- **Phantom balance and a false "payment received"**, usable to defraud a user who delivers goods
  against it *(read)*.
- **Spends that select the phantom fail.** The sighash is over the wrong value and script, so the
  wallet's sends break until the row is removed *(inferred)*.
- **A real coin can be overwritten.** If `T1:vout` already exists in our `outputs` table,
  `store_derived_utxo` takes the UPDATE branch (`handlers.rs:7271-7287`), overwrites
  `sender_identity_key`, `derivation_prefix`, `derivation_suffix` and `custom_instructions`, and sets
  `spendable = 1`. The wallet may then derive the wrong key for a coin we really own, and a spent row
  could be re-marked spendable *(inferred; recoverable by repair, not verified)*.
- The PeerPay poller runs automatically on the monitor schedule (`monitor/mod.rs:310-315`) with
  auto-accept. **No user action or permission is involved** *(read)*.

#### Fix shape

1. **Bind the subject.** Find the transaction whose `sha256d` equals the declared txid. Reject if none
   exists. Read the output, value and script **from that transaction only**. Put this in
   `Beef::from_atomic_beef_bytes`, or a new `atomic_subject()`, so every caller gets it: CU-6, the
   identity resolver and the overlay parsers share the same parser.
2. **Reject trailing bytes** after the last transaction, and reject plain BEEF where Atomic is required.
3. **Never let a receive overwrite an existing row.** If `txid:vout` exists, refuse and log; don't
   UPDATE derivation fields or `spendable`.
4. **Promotion must verify the output, not the txid.** Stale-row confirmation must fetch the output
   (value plus script, or hash of script) and compare it with the stored row before
   `mark_output_confirmed`.
5. **Use the message's `amount` as a cross-check.** It's read at `:180` and never used; a mismatch
   should reject.

**Stopgap if the fix slips:** disable automatic PeerPay acceptance (the `check_peerpay` monitor task)
in the beta.3 build, and keep the manual check endpoint behind an explicit user action.

#### Acceptance

| Row | RED | GREEN |
|---|---|---|
| CU-3-A | Unit: Atomic BEEF with header = txid of tx A, bundle last = tx B paying our derived key → `store_derived_utxo` called with B's value | Rejected: subject not found in bundle |
| CU-3-B | Unit: valid envelope plus 1 trailing byte → accepted | Rejected |
| CU-3-C | Unit: receive for an existing `txid:vout` → row updated | Refused, row unchanged |
| CU-3-D | Unit: stale unconfirmed row whose txid is mined but whose stored value doesn't match chain → marked confirmed | Not promoted; flagged |
| CU-3-E (non-regression) | — | A genuine PeerPay from a second Hodos or BSV Desktop wallet is credited once, with the correct amount |

---

## 2. Should fix: reviewer readings, not re-verified by hand

Each needs a re-read before its fix is scoped.

**CU-4 — certificate signature check skipped** *(two independent reviewers)*
- `certificate_handlers.rs:854-882` (direct) and `:2537-2559` (issuance) verify the certifier signature
  **only if** `revocationOutpoint` begins with a 32-byte hex txid; otherwise
  `log::info!("Skipping signature verification…")`.
- In issuance there is more:
  - A missing server signature proceeds (`:1292-1295`, `:1567-1569`).
  - A server `identityKey` ≠ requested certifier only warns (`:1266-1269`).
  - The `/signCertificate` response signature is a TODO (`:2422`).
  - `certifierUrl` has no HTTPS check.
- `prove_certificate` and `publish_certificate` don't re-verify.
- **Impact:** a connected site can store a forged certificate that the UI labels "SocialCert", which
  can then be proven or published.
- **Fix:** verify unconditionally (port ts-sdk's handling of placeholder outpoints, or reject what
  can't be verified). Enforce the certifier-key match and HTTPS. Verify before prove and publish.

**CU-5 — identity resolver trusts unverified overlay certificates**
- `identity_resolver.rs:392-521` parses overlay certificates, decrypts public fields and builds labels
  like "X/Twitter via SocialCert" from the **unverified** `certifier` field.
- It never checks the signature, never checks the certifier against the trusted set (used only as a
  query filter, `:281`), and never checks `subject == queried key`.
- It feeds `recipient_resolve` and recipient search (`handlers.rs:19271-19281`, `:19507-19519`).
- **Impact:** a user types "alice", is offered the attacker's key "via SocialCert", and pays it.
- **Severity** depends on whether the `tm_identity` overlay verifies at admission (not checked).
- **Fix:** verify signature → trusted certifier → subject match → then decrypt.

**CU-6 — `internalizeAction` accepts the wrong transaction**
- `handlers.rs:12077-12081` logs `Subject TXID mismatch` and continues with the last transaction.
- Plain BEEF, raw transactions and trailing bytes are accepted, and 200 is returned even when nothing
  was credited (`:12565-12567`).
- Basket-insertion outputs are stored without an ownership check (`:12604-12669`).
- **Fix:** share CU-3's subject binding, reject on mismatch, and return an error when
  `total_received == 0`.

**CU-7 — spend accounting loses history**
- Counters are in memory, keyed by browser ID and domain (`state.rs:48-64`). They reset when a tab
  navigates to another domain and back (`:613-620`), when the tab closes (`:583-589`), and on restart.
- Cents are truncated (`PaymentCost.h`), so sub-cent payments add 0 to the session cap.
- **Impact:** a connected site takes silent payments, resets by navigating or opening a tab, and
  repeats. Bounded per cycle, unbounded overall.
- Related to `ADVERSARIAL_PANEL_2_2026-08-20.md` (session-close reset), which covered the close path.
- **Fix:** a persisted per-domain spend ledger with a rolling window, and round cents up.

**CU-8 — check-then-act race on limits**
- `dispatch_payment_with_amount` reads a counter snapshot, decides, then records under a separate
  write lock (`request_gate.rs:1150-1175`). Actix runs parallel workers.
- **Fix:** snapshot, decide and record under one write lock.

**CU-9 — 402 reuse cache key**
- The key is URL plus sats (`handlers.rs:18712`), with no requesting domain or server key.
- Within 25 s another connected site could obtain an unbroadcast signed BEEF for the same URL and
  amount. Money still goes only to the original payee.
- **Fix:** add the requesting domain and `x-bsv-server` to the key.

---

## 3. Already known and tracked elsewhere (for completeness)

| Item | Where tracked | Note |
|---|---|---|
| Loopback wallet API has no caller authentication. A local process omitting `X-Requesting-Domain` is treated as internal, including `/wallet/export` | `0.4.0/HelicOps/AUDIT_FIX_TRACKER.md` **FU1** (per-launch shared secret) | Same shape as GHSA-5vmp finding 3. Not new |
| Content on **any** `localhost` / `127.0.0.1` port is an internal origin (XSS in a local web app → wallet IPC) | `phase-0.5-money-path/ADVERSARIAL_PANEL_2_2026-08-20.md` | Pin the internal origin to `127.0.0.1:5137` |
| TAAL ARC API key hardcoded in `rust-wallet/src/services/providers/arc_taal.rs:16` | `0.4.0/archive/SPRINT_0_4_0_MASTER_PLAN.md` (AUDIT-ADJ-DROPPED, "decide TAAL key rotation", pending) | ⚠️ **The key is in the public `Hodos-Browser/Hodos-Browser` repo** *(measured via GitHub API, 2026-09-15)*. Rotate |
| `TICKET_debug_log_unfiltered_in_production.md` says OPEN | — | Reviewer reading: fixed by `fa0c143` (2026-08-26). Ticket status is stale |

## 4. Checked and not vulnerable *(reviewer readings)*

- **BRC-104 signed data (GHSA-vjpq).** Nonces are decoded separately then concatenated
  (`handlers.rs:757-779`). The issuance client also accepts the legacy form as a fallback
  (`certificate_handlers.rs:1536-1553`); remove the fallback once issuers are patched.
- **Amount-scoped approval caching (finding 1a).** No spending decision is cached. Approvals are
  single-use, body-bound and expire after 600 s.
- **Token issuance bypass (finding 2).** Not applicable; there is no token auto-approve.
- **402 payment reuse.** Re-sends the same unbroadcast transaction and never signs a second spend.
- **Secrets in process arguments (finding 5).** Wallet and adblock children launch with no command-line
  secrets.
- **Outbound service URLs.** All hardcoded hosts use HTTPS, and no API keys go in query strings.
- **Outgoing BEEF.** Built with the signed transaction last and a matching header txid.

## 5. Not checked

- **End-to-end exploitability** of any item, on any build.
- **`v0.3.0-beta.29`:** the CU-1, CU-2 and CU-3 code patterns are present, but the surrounding gate
  model at that tag wasn't traced.
- **`tm_identity`** overlay admission rules (decides CU-5 severity).
- **Mnemonic copied to the clipboard** without clearing (`WalletPanelPage.tsx:405`) and crash-dump
  contents.
- **The C++ gate** on `/acquireCertificate`, `/wallet/certificate/publish` and the local message relay
  routes.
