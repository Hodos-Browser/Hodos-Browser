# Phase 0.5 — money path & trust boundary · PHASE CONTRACT

**Workstream:** WS5(a) · **Ticket:** `../TICKET_loopback_host_form_wallet_routing.md` §6.2, §7.1, §7.3
**Status:** 🔴 NOT SIGNED OFF — **IN REPAIR.** Adversarial panel returned DO-NOT-SIGN-OFF; 3 criticals, all now MEASURED. Repair prompt: `../SESSION_PROMPT_beta3_p05_repair.md` · **Opened:** 2026-08-18 · **Amended:** 2026-08-19 (§4a–§4c, §5a), 2026-08-19 **repair scope** (§2, §4 split into 0.5a/0.5b, §4e–§4g, §5b, §6) · **Platforms:** both (Rust = one binary; the C++ gates are cross-platform)
**Standard:** `../HARNESS.md`.

> ⭐ **Scope change, owner-approved 2026-08-19: C1, C2 and C3 are folded into this phase.**
> The panel's three criticals are not follow-ups — C1 and C2 are one structural fix (a single
> scheme-anchored origin derivation applied once, default-deny), and leaving C3 open would close a
> phase whose §1 goal sentence is still false. Rationale and the rejected alternative (splitting
> C2/C3 into their own phase) are in §5b.
>
> ⛔ **The structural lesson, and the reason this phase was refuted: gating arm-by-arm is HOW
> `simple_handler.cpp:5760` was missed.** No fix in this repair may be a per-arm patch.
>
> **Disclosure posture — owner decision, 2026-08-19: no user-facing advisory.** C2 is a shipping
> unprompted-spend path reachable in every public build back to `v0.3.0-beta.1` (§4g), but the
> tester base is a handful of users with ~$100 aggregate, and §4f established there is **no
> mitigation a user could apply even if told** — so an advisory would publish a working exploit
> while offering no remedy. Fixed silently in beta.3, with a non-mechanism line in the release
> notes ("fixes an issue where a website could initiate a payment without your approval; please
> update"). Revisit if beta.3 slips materially.

---

## 1. Goal

A fund-moving request from a web page is subject to the same approval engine as every other payment,
while a send the user initiates in their own wallet UI continues to complete **without any prompt**.

## 2. Done means

**Half 0.5a — the C++ trust boundary (origin derivation + transport coverage)**

- [x] The three `:5137` **trust-boundary** substring gates use a prefix/origin match (plus a fourth, §4c)
- [x] T0 gate `G2` baseline driven **5 → 2**, with both residuals named in §6
- [x] `IsInternalOrigin("")` decided — **left as-is deliberately**; the defect was the derivation, §4d
- [ ] ⭐ **C1** — origin derivation is **scheme-anchored**: only `http://`, `https://`, `hodos://`
      yield an origin; userinfo stripped through the last `@` of the authority; everything else
      returns empty **so the ancestor cascade and the `opaque-origin.invalid` sentinel actually
      run**. One derivation, one spelling, applied once. The same reduction inside
      `hodos::IsInternalFrontendUrl` / `hodos::IsLoopbackUrl`, which are **also** fooled by userinfo
- [ ] ⭐ **C2** — every wallet-touching IPC arm is gated at the **top of
      `OnProcessMessageReceived`**, not per-arm. A wallet arm added later **fails closed by
      default**, not by someone remembering to add a check
- [ ] The `send_transaction` IPC arm specifically no longer reaches Rust header-free from a web page

**Half 0.5b — the Rust money path**

- [x] `send_transaction` takes `HttpRequest` and routes external callers through `dispatch_payment`,
      exactly as `create_action` does
- [ ] An internal caller (no `X-Requesting-Domain`) is **unchanged** — no modal, no new latency
- [ ] ⭐ **C3** — `peerpay_send` and `paymail_send` take `HttpRequest` and route through
      `dispatch_payment`. ⚠️ `{recipient_identity_key|paymail, amount_satoshis}` is a **third** body
      shape — `d33741a`'s "do both or neither" warning applies with an extra shape
- [x] `.block_on_origin_mismatch(true)` on the CORS layer
- [ ] Finding 4 resolved — §4e measured that this **400s the direct-HTTP dApp transport**. Needs an
      owner decision (CLAUDE.md #13), not a reflex fix
- [ ] Finding 6 resolved — the forced prompt must not render **"0 sats"** under a false
      price-outage cause

⛔ **Struck 2026-08-19 — refuted by panel finding 6, and the correction is not cosmetic:**
~~`sendMax` from an external origin is subject to the per-tx and per-session caps.~~ With no
`X-Payment-*` headers, `matrix_c.rs:240` returns `Prompt(PaymentConfirmation, PriceUnavailable)`
**before** the rate, max-tx, per-tx and session-cap branches — so the cap branches are
**unreachable** for this endpoint and this bullet was never achievable as written. The achieved
property is a **forced prompt**, which is stricter than any cap but is a different claim. `P0.5-R3`
is rewritten to match.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| **`R-INTEXT`** | **Internal never prompts, external always gates** | 🚨 **This is the phase's central risk.** Gating `send_transaction` naively — e.g. requiring `X-User-Approved` unconditionally — makes the user's own send start prompting. The discriminator must remain header-presence, the same one everything else uses |
| `R-PERIM` | The four privacy-perimeter gates | Touches `dispatch_payment`'s call surface |
| `R-COUNT` | Per-session counters | A newly-gated endpoint now increments them; it did not before |
| `R-GOLD` | Gold pill fires | A newly silent-approved path must still emit the pill, or a payment goes visually unannounced |

## 4. Evidence table

### Half 0.5a — C++ trust boundary

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P0.5-X1` ⭐**NEW** | A frame at `data:text/html,a://127.0.0.1:5137/<script>…` is stamped **external and gated** | ⛔ Pre-fix it must reach Rust **header-free** (internal). This crafted `data:` URL is the new RED — the `about:blank` case alone is a strict subset and was never the reachable input class | Rust-side gate outcome **and** the browser-process origin log line, in the same run as an internal control | T1 | ⬜ |
| `P0.5-X2` ⭐**NEW** | `cefMessage.send('send_transaction', …)` from `https://example.com` is **refused before dispatch** | ⛔ Pre-fix: **MEASURED spending, no prompt** — §4g | The **browser-process** IPC dispatch, not Rust alone. Rust cannot distinguish the transports; both arrive header-free | T1 | ⬜ |
| `P0.5-X3` ⭐**NEW** | A **newly added** wallet arm with no explicit check is **denied** from an external origin | ⛔ Add a throwaway arm that calls a wallet endpoint → pre-fix it dispatches | The default-deny allowlist, not any one arm. This row is the whole point of C2 | T1 | ⬜ |
| `P0.5-X4` ⭐**NEW** | `http://127.0.0.1:5137@evil.com/` is **not** internal to `IsInternalFrontendUrl` / `IsLoopbackUrl` | ⛔ Pre-fix both return **true** (verified: both are `rfind(pfx,0)==0`, so userinfo prefix-matches) | The two `PortConfig.h` predicates, unit-testable without CEF | T0 | ⬜ |
| `P0.5-G1` | `https://<origin>/?x=127.0.0.1:5137` is served **nothing** from disk | ⛔ **Pre-fix this must SUCCEED** — if it does not, §7.3 is refuted and this row is withdrawn | **Release-shaped** build: `IsFrontendAvailable()` is true in production (`{app}\frontend\`), so this is not a dev-only defect | T2 | 🟡 **UNTESTABLE IN DEV** — see §4a. Code fixed; RED still owed on a release-shaped build |
| `P0.5-G2` | The same page gets **no** `window.hodosBrowser.identity` | Pre-fix it must be **defined**. Control: the same page *without* the substring must get neither | Renderer for **that page's** frame — not an overlay. `type:"page"` over CDP is not proof of which browser | T2 | ✅ **GREEN, RED observed** — §4b |
| `P0.5-G3` |  `preflight.ps1` gate `G2` passes at baseline **2** (from 5) | Add one new `find("127.0.0.1:5137")` → gate **exits non-zero** even at a non-zero baseline | `preflight.ps1 -NegativeControl` | T0 | ✅ **GREEN** `2 violations, at baseline`; 🔴 observed `3 > 2` |
| `P0.5-E1` | An origin-less **or origin-forged** frame is gated | Feed both an `about:blank` child **and** the crafted `data:` URL → each must be seen ungated pre-fix | The Rust-side gate outcome | T1 | 🔴 **REOPENED 2026-08-19** — the GREEN in §4d is real *for `about:blank`* and **void as evidence that the boundary holds**. The step-1 parse is unanchored, so an attacker-chosen frame URL never reaches the cascade. Superseded by `P0.5-X1`; keep this row until X1 is green |

### Half 0.5b — Rust money path

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P0.5-R1` | User send from the wallet UI → **no modal**, tx broadcasts | Stub the internal branch to require approval → a modal appears | Frame URL `127.0.0.1:5137`; Rust log shows **no** `X-Requesting-Domain` | T2 | ⬜ |
| `P0.5-R2` | External page, over-cap send → **202 + modal** | Revert the `dispatch_payment` wiring → the send completes silently | Rust log shows `X-Requesting-Domain: <exact page host>` | T2 | ⬜ |
| `P0.5-R3` ✏️**REWRITTEN** | External `sendMax:true` → **forced prompt** (not a cap evaluation) | Same revert → full balance sweeps | Scratch wallet, funded with a token amount. ⛔ **Never the production wallet.** The dev wallet (`HodosBrowserDev`, 41,931,019 sats) is a valid scratch wallet — the production wallet is a **separate DB** (§4h) | T2 | ⬜ |
| `P0.5-X5` ⭐**NEW** | `peerpay_send` / `paymail_send` from an external origin → **202 + modal** | ⛔ Pre-fix: both spend with **no gate of any kind** — they take no `HttpRequest`, so `dispatch_payment` is structurally impossible | Both endpoints, both body shapes. ⚠️ A **third** body shape — do both or neither | T2 | ⬜ |
| `P0.5-C1` | Cross-origin simple POST no longer executes the handler | Remove `block_on_origin_mismatch` → the handler runs despite the browser hiding the response | **Server-side effect**, not the browser's error. A blocked read is not a blocked write | T2 | ✅ **GREEN, RED observed — §4e.** ⚠️ But the same measurement **confirmed panel finding 4**: it also 400s the direct-HTTP dApp transport. Owner decision owed before this row signs off |
| ~~`P0.5-R4`~~ | ~~Gold pill fires on a newly silent-approved send~~ | — | — | — | ⛔ **WITHDRAWN 2026-08-19 — the row has no subject.** No `/transaction/send` call can emit `payment_success_indicator`: all six `OnWalletCallSuccess` sites derive `wasAutoApprovedPayment` as `ok && isPaymentEndpoint(endpoint) && !isError`, and `isPaymentEndpoint` is `{/createAction, /acquireCertificate, /sendMessage}` — `d33741a` deliberately excludes `/transaction/send`. Stronger: an external send can never be **silently** approved at all, because `matrix_c.rs:240` prompts unconditionally. The obvious way to "test the pill" is a `createAction`, which **passes with both commits reverted** — a HARNESS §6-Q1 void green waiting to happen. R-GOLD stays covered by `../REGRESSION_SET.md:35-41` at the 0.5→1 boundary. Filed separately: an `X-User-Approved` replay on `/transaction/send` moves funds and emits no pill while a byte-identical `/createAction` replay does |

**Pairing — three pairs, none may be signed off alone:**

1. `R1`/`R2` are the two halves of `R-INTEXT` and are each other's control. A fix that satisfies one
   by breaking the other is the specific failure this phase is most likely to produce.
2. ⭐ **`X2`/`R1`** — the same pairing one layer up. `X2` closes the ungated transport; `R1` proves
   the closure did not catch the wallet's own UI, which uses **that same arm**.
3. ⭐ **Half 0.5a / half 0.5b** — 0.5a without 0.5b leaves `peerpay_send`/`paymail_send` open; 0.5b
   without 0.5a leaves the derivation forgeable so the gate can be walked around. **Neither half
   signs off alone, and beta.3 ships only when both are green.**

### 4a. `P0.5-G1` — attempted, and it cannot be judged in a dev build

Ran on the dev build 2026-08-19: both the test URL and the control returned the real `Example Domain`
HTML, so nothing was served from disk. **That does not refute §7.3.** The dev build has no
`frontend/` next to its exe, so `IsFrontendAvailable()` is false and short-circuits the `&&` before
the URL check is ever reached. The row's own SUBJECT column already demanded a **release-shaped**
build; this attempt simply could not exercise it.

The path remains mechanically plausible and is fixed regardless: `ExtractPath` strips the query
string, leaving an empty path, and `LocalFileResourceRequestHandler` carries an explicit **SPA
fallback that serves `index.html` when the file is not found**. ⇒ RED owed on a release-shaped build;
pairs naturally with Phase 0's RC gates.

### 4b. `P0.5-G2` — REPRODUCED, fixed, both halves re-run

Measured over CDP against the tab's own frame, asserting `location.href` in the same call.

⚠️ **Subject discipline mattered here.** This browser exposes **11 CDP targets and every one reports
`type:"page"`** — the header and nine overlays included. Taking "the first page target" would have
measured an overlay. That is precisely how three farbling harnesses died.

| | `…/?x=127.0.0.1:5137` | `…/` (control) |
|---|---|---|
| **PRE-FIX** `identity` / `navigation` / `history` | **object / object / object** | undefined / undefined / undefined |
| **POST-FIX** `identity` / `navigation` / `history` | undefined / undefined / undefined | undefined / undefined / undefined |

Same origin, same page, **only the query string differs** — so the control is exact rather than
approximate. Post-fix the two are identical in behaviour, and both still receive
`__hodos_walletCall`, which is the correct dApp surface.

Internal surfaces re-checked in the same run — the other half of the pair, because a fix that closes
the leak by breaking the wallet UI is the failure mode this phase is most likely to produce:

| Subject | identity | navigation | history | walletCall |
|---|---|---|---|---|
| `wallet-panel` overlay | object | object | object | function |
| header (`/`) | object | object | object | function |
| `menu` overlay | object | object | object | function |

### 4c. ⚠️ A FOURTH gate of the same family — found by running the fix

`isExternalPage` (`simple_render_process_handler.cpp:514-516`) was
`url.find("127.0.0.1") == npos && url.find("localhost") == npos` — the same unanchored shape, and
**not** caught by `G2`, whose pattern requires `:5137`. It stayed invisible while the internal gates
were *also* substring checks, because a URL containing the string then satisfied **both**
classifications.

Fixing only the three named gates therefore created a **new state**: the test page was neither
internal nor external, fell through both branches, and lost `__hodos_walletCall` and the CWI shim
entirely — a functional regression introduced by the security fix. Caught by re-running the **whole**
evidence table rather than the failing row (`HARNESS.md` §5).

Repaired with `hodos::IsLoopbackUrl()` — anchored scheme+host prefixes, in `PortConfig.h` beside
`IsInternalFrontendUrl` — ~~so internal and external are true complements again.~~ The deliberate
exclusion of loopback pages from the dApp shim, documented in the gating cascade at `:758-770`, is
preserved; only the matching is anchored.

⛔ **Correction 2026-08-19 (panel finding 12) — "true complements" is over-claimed.** Both predicates
anchor at the prefix but never check the character *after* it, so `http://localhost.evil.com/` is
"loopback" and `http://127.0.0.1:51370/` is "internal frontend". Prefix-anchoring is **necessary, not
sufficient** — `PortConfig.h:62-63` calls this a "TRUST BOUNDARY" and it is not yet one. The
host-terminator already exists in-tree at `HttpRequestInterceptor.cpp:1017-1022`; reuse it rather
than writing a fifth spelling. Closing this also closes the userinfo variant of C1, which is why
`P0.5-X4` is in half 0.5a rather than deferred.

### 4d. 🚨 `P0.5-E1` — REPRODUCED as a live bypass, then closed

The contract carried this as a decision to be *taken*. It is not a decision; it was an exploitable
hole, and it is now measured on both sides.

**The exploit needs no trick at all — just an iframe.** From `https://example.com`:

```js
var f = document.createElement('iframe');   // no src  =>  about:blank
document.body.appendChild(f);
f.contentWindow.cefMessage.send('wallet_call', ['id','status','/wallet/status','{}','GET']);
```

| | browser-process log |
|---|---|
| **RED (pre-fix)** | `🔒 IPC internal origin  — direct dispatch` ← note the **empty** origin |
| **GREEN (post-fix)** | `🔒 IPC: domain example.com trust_level: unknown` ← attributed to the parent, gated |
| **Control, same run** | overlays + header still `🔒 IPC internal origin 127.0.0.1:5137 — direct dispatch` |

`about:blank` has no `://`, so the old parse produced an empty `origin`; `IsInternalOrigin("")`
returns `true`; the call took `runIpcCallDirect` and reached Rust with **no `X-Requesting-Domain`**,
which `domain_trust_mw` reads as fully-trusted wallet-internal.

⭐ **The API that matters is `cefMessage`, not `__hodos_walletCall`.** The `about:blank` child does
**not** receive the wallet bridge — ~~that is main-frame-only per the shim's gating cascade~~ — but
it *does* receive `cefMessage`, the raw IPC underneath. Anyone reasoning about this boundary from the
bridge alone would conclude, wrongly, that subframes are already safe.

⛔ **Correction 2026-08-19 (panel finding 11) — the struck parenthetical over-generalises.**
Main-frame gating is true of the **external dApp arm** (`simple_render_process_handler.cpp:791`)
only. `isInternalPage` is computed at `:553` from `frame->GetURL()` with **no** `frame->IsMain()`
check, eleven lines above the arm that does have one. The conclusion drawn in this section is
unaffected — that frame is external either way — but the stated *reason* was wrong, so it is struck
in place per HARNESS §8 rather than silently edited. The missing main-frame check on the privileged
surface is a named residual in §6.

**Severity, relative to the rest of this phase.** Worse than `P0.5-G2`: that one required the
attacker to place a magic string in their own URL; this one requires only an iframe. It is the exact
path `REGRESSION_SET.md` flags as *"the one path by which external can silently become internal"* —
now measured rather than suspected.

**The fix is at the derivation, not the predicate.** `IsInternalOrigin("")` is left alone
deliberately: it is also fed by the HTTP path, where an absent `X-Requesting-Domain` legitimately
means internal, so flipping it would break every real internal call. What was wrong was
*manufacturing* an empty origin for a frame that has a perfectly good security origin — an
`about:blank` child inherits its parent's. So the derivation now resolves:

1. this frame's URL → 2. nearest ancestor with a real origin → 3. the top-level document

and, if none of those yields one, **fails closed** with `opaque-origin.invalid` (RFC 2606 reserves
`.invalid`, so it can never collide with a real host) — stamped as an external domain and gated,
rather than silently becoming internal.

⚠️ Attribution note, deliberate: a `data:`/`blob:` frame is *opaque*-origin per spec, so inheriting
the ancestor's origin is slightly more permissive than a strict reading — such a frame is attributed
to the page that created it. ~~It is still **external and gated**, which is the property that matters
here;~~ a stricter opaque-origin model belongs with Phase 5's parsed predicate.

🚨 **REFUTED 2026-08-19 (panel finding 1, C1) — this was the load-bearing claim of §4d and it is
false.** For a **crafted** `data:` frame it is **internal**. `originFromUrl`
(`simple_handler.cpp:2060`) does `u.find("://")` over the whole frame URL with no scheme anchor, so
`data:text/html,a://127.0.0.1:5137/` parses to origin `127.0.0.1:5137`, which `IsInternalOrigin`
accepts. The ancestor walk (`:2075`) and the `opaque-origin.invalid` sentinel (`:2085`) both run only
`while (origin.empty())` — so **neither is ever reached**: the attacker supplies a non-empty origin
at step 1. The same primitive also forges any **external** origin
(`data:text/html,a://trusted-dapp.example/`), inheriting another dApp's spending caps and
identity-disclosure grant.

⭐ **What survived, and must not be re-litigated:** `blob:https://evil.com/uuid` parses correctly to
`evil.com`, and `https://evil.com/?x=a://127.0.0.1/` parses to `evil.com` — a *real* scheme always
puts its `://` first. The bypass requires a **non-special-scheme** frame, which is exactly the class
this section believed it had covered. `history.pushState` cannot introduce credentials and
credentialed iframes are blocked; the live userinfo vector is a **top-level navigation**.

### 4e. `P0.5-C1` — the owed server-side test, RUN. Green, **and it confirms panel finding 4**

**MEASUREMENT, 2026-08-19**, dev wallet on **31401** (never production — §4h). Real `curl` 8.2.1
under Git Bash, **not** the PowerShell `Invoke-WebRequest` alias that lies about connection state.
Endpoint `/getVersion`; body `{}`.

| `Origin:` sent | Feature ON (`block_on_origin_mismatch(true)`) | 🔴 Feature OFF (line commented out, rebuilt) |
|---|---|---|
| `http://127.0.0.1:31301` — what Chromium puts on a dApp **POST** | **400** `Origin is not allowed to make this request` | **200** + real JSON |
| `https://zanaadu.com` — a real dApp origin, survives on **GET** | **400** | **200** |
| *(none)* — the IPC transport's shape | 200 + real JSON | 200 |
| `http://127.0.0.1:5137` — allowlisted control | 200 | — |

**The GREEN is real and the negative control is exact** — one line removed flips both 400s to 200,
so the block is caused by that line and nothing else. The row's owed half is discharged.

🚨 **But the same run confirms the defect.** Both origins a real dApp transport can produce are
**absent from the four-entry allowlist** (`main.rs:922-925`), so an external dApp using
`@bsv/sdk WalletClient`'s default transport now gets a **400 before any handler runs**, where it
previously got a 202. Pre-change, `block_on_origin_mismatch` **defaults to false**, so these took
`Ok(false)`: handler ran, CORS headers merely omitted — which is why nothing was ever seen to break.

⚠️ **The IPC bridge is unaffected** — both `runIpcCall*` paths use `SyncHttpClient`, which sends no
`Origin` (row 3 above, measured). The wallet UI does not brick. What breaks is external dApp
interop.

⛔ **Owner decision owed, CLAUDE.md invariant #13.** The evidence points at **production code**. The
obvious repair — allowlisting `127.0.0.1:31301`/`31401` — admits an origin only the interceptor's own
re-issue can produce. That is a judgement call, not a reflex. **Not yet made.**

### 4f. 🚨 There is **no** wallet lock — so no mitigation exists, and this sizes C2's severity

**MEASUREMENT + CODE READING, 2026-08-19.** Asked because a "keep your wallet locked" mitigation
would have changed both the severity and the disclosure posture. It does not exist:

- **No lock endpoint.** `/wallet/unlock` is routed (`main.rs:1059`); nothing locks.
- The only `cached_mnemonic = None` in the tree is `clear_cached_mnemonic()`
  (`database/connection.rs:233`), whose single caller is `handlers.rs:2964` — **after wallet
  deletion**.
- `is_unlocked()` is just `cached_mnemonic.is_some()` (`:141-143`).
- On Windows a **no-PIN wallet auto-unlocks at startup via DPAPI** (`:194-201`). Measured: the dev
  wallet reported `{"exists":true,"locked":false}` on a **cold process start**.
- A PIN wallet unlocks once and stays unlocked for the whole process lifetime.

⇒ The only user-available action is "do not run the browser with funds in it." This is the finding
that decided the disclosure posture (see the header note): an advisory can offer no remedy.

### 4g. 🚨 C2 — reproduced on the current tree with an **exact paired control**

**MEASUREMENT, 2026-08-19.** Dev wallet 31401. Every amount deliberately far above the dev balance
(41,931,019 sats) so the request dies at UTXO selection and **nothing can move** — the same
technique as the original PoC.

Identical endpoint, identical body, **one variable — the header**:

| Request to `/transaction/send` | Result |
|---|---|
| 🔴 **No `X-Requesting-Domain`** — the shape `WalletService::sendTransaction` produces | **HTTP 500.** No gate, no 202, no approval. Log: `💸 /transaction/send called` → `Total needed: 1000000001199 satoshis` → `Insufficient cached balance`. It reached **UTXO selection**; with an amount ≤ balance it spends |
| 🟢 **`X-Requesting-Domain: evil.example`** added | **HTTP 202** `{"status":"pending","promptType":"domain_approval","engineReason":"new_domain_no_manifest"}`. Log: `🛡️ engine Prompt (domain-trust) minted approval id=… endpoint=/transaction/send`. Never reached UTXO selection |

**The gate `d33741a` added works.** It simply never sees the transport a web page actually uses:
`simple_handler.cpp:5760` performs no origin/role/frame check, `WalletService` sends only
`Content-Type`, and `request_gate.rs:1000` reads a missing header as `None => return
GateOutcome::Proceed`. This is the C2 mechanism, end to end, on the tree as committed at `637e216`.

**Reach — CODE READING, not reproduced.** Both preconditions are present in the last *publicly
released* build: `v0.3.0-beta.29` carries the `send_transaction` arm with **no** origin check, and
`cefMessage` injection on external pages entered at `4fad37b`, whose earliest containing tag is
**`v0.3.0-beta.1`**. ⚠️ Labelled honestly: this is a reading of the tagged trees. **Nobody has run
the PoC against a released binary**, and per this phase's own rule that measurement was *dropped, not
skipped* — the owner's no-disclosure decision retired the only question it would have answered.

### 4h. Environment — dev/prod deconfliction VERIFIED, and the scratch wallet already exists

**MEASUREMENT, 2026-08-19.** Recorded because `P0.5-R3` forbids the production wallet and the next
session must not have to re-derive this. Production browser + wallet ran **concurrently** with the
dev wallet throughout; the dev wallet was started, killed and restarted **three times** and 31301
was confirmed still listening after each.

| | Production | Dev |
|---|---|---|
| Wallet port | **31301** | **31401** |
| Adblock port | **31302** | 31402 |
| Wallet DB | `Roaming\HodosBrowser\wallet\wallet.db` (350 MB) | `Roaming\HodosBrowserDev\wallet\wallet.db` (217 MB) |
| Balance at time of test | 85,260,262 sats | **41,931,019 sats** |

⇒ **Deconfliction works as designed; no change needed.** The dev wallet's ~$6 is a valid
"scratch wallet, funded with a token amount" for `P0.5-R3` — one does not need to be created.

⚠️ Two traps confirmed the hard way, both already in the repair prompt: kill dev processes **by
executable path** (`Get-Process -Id <pid> | Select Path` before `Stop-Process`) — prod and dev share
the image name `HodosBrowser.exe`; and `cargo build … | grep` **discards the exit code**, so the
`Finished` line is the evidence, not `$?`.

*(Unrelated, observed in the same port scan: **CDP 9222 is LISTENING on the production browser** —
`../TICKET_cdp_port_open_in_release.md`, now observed live rather than inferred. Not this phase.)*

## 5. Blast radius

- `rust-wallet/src/handlers.rs :: send_transaction` (9612–9951) — signature change; ~~every caller is
  `WalletService::sendTransaction` via `simple_handler.cpp:5740`, i.e. first-party today.~~
  ⛔ **FALSE — refuted by measurement 2026-08-19.** That call site is the `send_transaction` **IPC
  arm** (`simple_handler.cpp:5760`), which performs no origin check, and `cefMessage` is injected
  into every V8 context with no message-name allowlist. A page at `https://example.com` reached
  `/transaction/send` unprompted (`Amount: 999999999999 satoshis`, HTTP 500 at UTXO selection only
  because the amount exceeded the balance). This was the load-bearing assumption of the phase and it
  was carried forward from the contract without being tested. Struck, not deleted — it is exactly the
  kind of plausible premise this project keeps shipping on.
- `rust-wallet/src/main.rs` — CORS builder (`:922-930`). One line, affects every route.
- `cef-native/src/handlers/simple_handler.cpp:7962` — frontend-from-disk gate.
- `cef-native/src/handlers/simple_render_process_handler.cpp:539,541` — the privileged V8 surface
  (`hodosBrowser.identity`, `.navigation`, history, `WALLET_CALL_BRIDGE_SCRIPT`).
- ⭐ **Reuse, do not re-spell:** `IsInternalFrontendUrl()` (`simple_handler.cpp:137-142`) is already
  the correct predicate. It lives in the browser process, so the render-process pair needs a shared
  header or the same four lines — **not a fourth spelling**.

**Added by the repair scope (2026-08-19):**

- 🚨 `cef-native/src/handlers/simple_handler.cpp :: OnProcessMessageReceived` — **the whole IPC
  dispatch**, not one arm. This is the largest surface in the phase and the one most likely to cause
  a functional regression: at least **eleven** wallet arms currently dispatch with no origin check
  (`create_wallet` `:3755`, `mark_wallet_backed_up` `:3805`, `get_wallet_info` `:3851`, `load_wallet`
  `:3899`, `get_all_addresses` `:3947`, `get_current_address` `:3995`, `get_addresses` `:4043`,
  `address_generate` `:5518`/`:5552`, `get_balance` `:5722`, `send_transaction` `:5760`,
  `get_transaction_history` `:5909` — **all line numbers re-verified against `637e216`**). Every one
  of these is also used by the **first-party wallet UI**, so the allowlist must admit internal
  callers or the UI breaks. `R1` is the control for exactly this.
- `cef-native/src/handlers/simple_handler.cpp:2060` — `originFromUrl`, lifted out of the
  `wallet_call` arm into a shared helper (C1).
- `cef-native/include/core/PortConfig.h:74-98` — `IsInternalFrontendUrl` / `IsLoopbackUrl`; anchor +
  host-terminate + strip userinfo (C1, finding 12).
- 🚨 `rust-wallet/src/handlers.rs :: peerpay_send` (`:17106`) and `:: paymail_send` (`:18149`) —
  signature change to take `HttpRequest` (C3). ⚠️ **A third body shape**
  (`{recipient_identity_key|paymail, amount_satoshis}`). `peerpay_send` builds a real
  `CreateActionRequest` and broadcasts; its only validation today is `amount_satoshis <= 0`.
- `cef-native/src/core/HttpRequestInterceptor.cpp:3651-3664` — strip `X-Requesting-Domain`,
  `X-User-Approved`, `X-Browser-Id`, `X-Payment-*` from `originalHeaders_` **before** the merge
  (finding 5). Page-supplied headers currently **overwrite** the C++-derived trust headers: CEF
  serialises duplicates into `AddHeadersFromString`, which is a case-insensitive **overwrite**, and
  the page's value goes in last.
- `frontend/src/pages/BRC100AuthOverlayRoot.tsx` — the "0 sats" prompt (finding 6).

### 5b. Scope decision — fold C1/C2/C3, one sign-off, two halves

**Owner-approved 2026-08-19.** The alternative considered and rejected was splitting C2/C3 into a
separate phase.

- **C1 and C2 are one fix, not two.** The repair is a single scheme-anchored derivation applied once
  at the top of `OnProcessMessageReceived` with a default-deny allowlist. Split, you land either a
  helper with no enforcement or enforcement over a forgeable derivation — reconstructing the exact
  partial-coverage failure that caused this refutation.
- **C3 is separable in code, not in claim.** It is Rust, a different file, a different mechanism
  (missing `HttpRequest`, not a missing origin check). But §1 says *"a fund-moving request from a web
  page is subject to the same approval engine as every other payment."* Close the phase with
  `peerpay_send`/`paymail_send` open and that sentence is still false — the panel's precise
  criticism, that the title is broader than the content.
- **Splitting buys nothing on the critical path.** beta.3 cannot ship with C2 open, so C2's phase
  gates the release either way; a boundary would add ceremony without moving the date.

⚠️ **Accepted cost:** this is no longer the "three small additive gates, reversible" phase that
`../SPRINT_PLAN.md` §4 used to justify putting it second. It is now the largest phase in beta.3.
That was surfaced to the owner and accepted rather than discovered later.

### 5a. Scope change — amended in the same commit that changed the scope

**A fourth gate, `isExternalPage`, is now in scope** (§4c). Same defect class as the three named
ones, adjacent to them, and leaving it would have shipped a functional regression this phase's own
fix introduced. `G2` cannot see it (its pattern requires `:5137`); **widening `G2` was considered and
rejected** — re-baselining a gate mid-phase to cover a different pattern destroys the meaning of the
number it just moved. A dedicated gate for unanchored loopback checks belongs with Phase 5's
`IsWalletOrigin()` work, where the parsed predicate lands.

Also corrected: §6 named the residual as `TabManager.cpp:168`; it is **`:176`**.

## 6. Out of scope

- **W0/W1/W2'/W3** — the parsed `IsWalletOrigin()` predicate and `/health`. That is **Phase 5**.
- **W4/W6/W7/W8** — beta.4.
- Residual `G2` violations, knowingly allowed at target **2**, listed so they are not mistaken for
  oversights:
  - `TabManager.cpp:176` and `TabManager_mac.mm:191` — `find(...) == npos` used to *exclude* internal
    URLs from history. Sloppy, and it means an external URL merely containing the string would be
    skipped from history — a nuisance, not a trust boundary. Goes with W7 in beta.4.
  - *(`simple_handler.cpp:1153` was on this list until the gate was written: `find(X) != 0` **is** a
    prefix check, so `G2` now excludes it by construction along with `rfind(X, 0) == 0`. Correct code
    should not be flagged.)*
- `extractDomain()`'s scheme-blindness and main-frame-URL sourcing — filed, deliberately deferred.

### Named residuals added 2026-08-19 (panel findings 7, 8, 9, 11, 12, 13)

⛔ HARNESS §4: **an unexplained residual is a defect, not a baseline.** These are known, deliberately
left, and named with reasons so they are not mistaken for oversights — and so nobody re-derives them
from scratch.

- **Finding 7 — `IsInternalOrigin` trusts *any* loopback port** (`HttpRequestInterceptor.cpp:1015-1024`).
  A page at `http://localhost:8000` gets `cefMessage`, resolves to internal, and reaches Rust
  header-free on **any** endpoint. ⚠️ **Not introduced by this phase** — the pre-fix gate was
  `find("127.0.0.1:5137")`, which `localhost:8000` also failed, and the port-agnostic behaviour is
  deliberate and documented at `:1931-1937`. The defect is the **policy**, not a missed unification.
  → **Phase 5 (W0)**, where the parsed `IsWalletOrigin()` predicate lands.
  ⛔ `../TICKET_loopback_host_form_wallet_routing.md:537-539`'s "the rest of that function is sound"
  is true for the suffix shape and **false for the port dimension** — correct it there.
- **Finding 8 — `opaque-origin.invalid` is one shared, *approvable* trust identity**
  (`simple_handler.cpp:2087`). It **cannot become trusted** (verified against every loopback
  predicate in both processes — a clean negative result). The defect is the opposite: it is an
  ordinary domain string downstream, `POST /domain/permissions` validates only `!domain.is_empty()`,
  and `domain_permissions` is `UNIQUE(user_id, domain)` — so one approval creates a permanent row
  that **every other site's unattributable frame inherits**, along with session counters and the V18
  scoped-grant child tables. Fail-closed to a shared bucket stops being fail-closed once the user can
  say yes. → Cheapest close (reject the literal in `set_domain_permission`) is **in scope for this
  repair if it stays one line**; the opener-walk alternative is Phase 5. ⛔ Do **not** make it
  per-request unique — that mints unbounded promptable pseudo-domains.
- **Finding 9 — the derived origin is scheme-blind**: `http://dapp.example` inherits
  `https://dapp.example`'s spending caps and silent identity-key disclosure. Not introduced by
  `d33741a` (the deleted inline block was behaviourally identical). §6's existing deferral names
  *`extractDomain()`* only, so the new derivation's identical property was **not** covered — named
  here to close that gap. → Phase 5.
- **Finding 11 — the privileged V8 surface has no main-frame check**
  (`simple_render_process_handler.cpp:553`), and the `:5137` frontend has **no framing protection**
  (measured: zero hits for `X-Frame-Options`/`frame-ancestors`/CSP across `cef-native/src`,
  `LocalFileResourceHandler.h`, `frontend/index.html`, `frontend/vite.config.ts`, and no
  frame-busting in `frontend/src`). Impact is bounded by same-origin policy — this is **clickjacking
  of money UI**, not API theft. → **W7, beta.4.** ⚠️ Any W7 fix must add the check at `:587` as well
  as `:784`; the `history.clearAll`/`identity`/`navigation` surface sits inside the same ungated
  condition.
- **Finding 12 (second half) — `wallet_call` takes an arbitrary endpoint string.** Both IPC paths
  build `hodos::WalletBaseUrl() + endpoint` from a page-supplied string with **no allowlist, no
  leading-`/` check, no charset filter** (`HttpRequestInterceptor.cpp:1943`, `:2083`). Two live
  hazards: `endpoint="9/x"` reaches `127.0.0.1:50875` via `stoi` overflow-and-wrap with the body
  readable by the page, and `endpoint="99999999999"` throws an uncaught `std::out_of_range` on a CEF
  worker thread — **a one-line browser-process crash from any web page**. On macOS `CURLOPT_URL`
  honours userinfo, so an off-host retarget is live there pending measurement (Windows
  `SyncHttpClient::ParseUrl` has no userinfo concept and always takes `127.0.0.1`).
  ⛔ **This contradicts CLAUDE.md's "`isWalletEndpoint` route table is the entry point for all new
  wallet endpoints; new endpoints go through the table, never around it"** — the IPC transport goes
  around it entirely, which is *how C2 and C3 stay reachable*. → The `stoi` crash is cheap and
  **in scope for this repair**; the endpoint allowlist is Phase 5.
- **Finding 13 — `X-User-Approved` is bound to id + body hash only**
  (`permission_service/state.rs:261-287`); `consume_and_verify` never compares the stored
  `domain`/`endpoint` against the replaying request. Defence-in-depth only — ids are 128-bit CSPRNG,
  single-use, 10-minute TTL, and the IPC transport gives a page no header channel — but an
  approvalId **does** leak to the page on two fall-through paths
  (`HttpRequestInterceptor.cpp:2102-2105`, `:3520-3525`). Two equality checks close the class.
  → beta.4. ⭐ **Settled, do not re-open:** the createAction rebuild non-determinism is a **non-issue**
  — the approval binds the **outer** `/transaction/send` body, which C++ re-issues verbatim, and the
  inner `create_action` keeps a header-less `TestRequest`.
- **Finding 14 — two committed comments contain a raw form feed** (`PortConfig.h:59`,
  `simple_handler.cpp:8009`), both new in `4dec940`, cause consistent with a shell escape layer.
  Cosmetic — a form feed does not terminate a `//` comment — but they are the documentation for a
  trust-boundary predicate and now name a path that does not exist. ⛔ **Fix via `Edit`, not a
  heredoc.** A scan of all 325 added lines found **exactly these two**. → **In scope for this
  repair** (it is two characters).

## 7. Rollback

Three independent reverts: the Rust handler signature, the one-line CORS change, and the C++ gates.
Each stands alone; none shares a commit with another.

⚠️ **Amended 2026-08-19 — the repair scope is NOT as cleanly revertible, and that is a real cost.**

| Change | Revertible alone? |
|---|---|
| Rust handler signatures (`send_transaction`, + C3's two) | ✅ Yes — one call site each, verified |
| One-line CORS | ✅ Yes |
| The four `:5137` gates | ✅ Yes |
| Header-strip (finding 5) | ✅ Yes |
| ⛔ **C1 derivation + C2 default-deny** | ❌ **No — they are one change.** Reverting C1 alone leaves the default-deny gate keyed on a forgeable origin, which is **worse than either state**: it looks gated and is not. Revert both or neither |

⛔ **Consequence for sequencing:** land C1 and C2 in the **same commit**. This is the one place in
the phase where the repo's usual "small, independently revertible steps" rule is deliberately
overridden, and the reason is recorded here so a future reader does not "tidy" them apart.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] ⛔ **Both halves green.** 0.5a alone leaves C3's fund-movers open; 0.5b alone leaves the
      derivation forgeable so the gate can be walked around. **Neither half signs off alone**
- [ ] ⛔ `P0.5-G1` / `P0.5-G2` pre-fix reproduction attempted and its result recorded **either way** —
      a refuted premise is a valid outcome and must be written down, not quietly dropped
- [ ] ⛔ **The WHOLE evidence table re-run after the last fix, not the failing row.** That is the only
      reason the fourth `:5137` gate (`isExternalPage`, §4c) was ever caught
- [ ] `scripts/preflight.ps1` + `-NegativeControl` recorded
- [ ] `../REGRESSION_SET.md` in full at the 0.5 → 1 boundary, `R-INTEXT` **both halves**
- [x] Adversarial review — ✅ **workflow panel** per `../HARNESS.md` §6, distinct lenses:
      *can I reach a money endpoint un-stamped* · *can I forge an origin* · *does the internal path still stay silent*
      → **DO NOT SIGN OFF**, 23 findings, 3 criticals. `ADVERSARIAL_PANEL_2026-08-19.md`
- [ ] ⭐ **Re-run the panel after the repair.** The first panel refuted the first implementation; a
      repair of this size that is *only* self-reviewed repeats the mistake this phase exists to fix
- [x] `G2` 5 → 2 in `../HARNESS.md` §9
- [ ] Commits cite row IDs
- [ ] ⭐ Release note line drafted (non-mechanism) per the disclosure decision in the header

**Owner decisions still open (blocking sign-off, not blocking implementation):**

1. **Finding 4** (§4e) — allowlist `127.0.0.1:31301`/`31401`, revert `block_on_origin_mismatch`, or
   accept broken direct-HTTP dApp interop for beta.3? Production code, CLAUDE.md #13. ⬜ **Not made.**
2. `P0.5-R3` — design around the **202 decision** (no funds needed) or prove the full broadcast?
   ⬜ Not made. *(§4h: the dev wallet is a valid scratch wallet either way.)*
3. Phase 0's `P0-A4`/`P0-A5` RC gates — scheduled when the beta.3 RC is built. ⬜

| Item | Result | Date | By |
|---|---|---|---|
| preflight | **PASS** — exit 0, `-Full`, nothing skipped | 2026-08-19 | assistant |
| preflight -NegativeControl | **PASS** — all 5 gates seen to fail; `G2` at `3 > 2` | 2026-08-19 | assistant |
| `P0.5-C1` server-side effect (§4e) | **GREEN, RED observed** — 400 vs 200 on the one-line revert. ⚠️ Confirms finding 4 | 2026-08-19 | assistant |
| C2 reproduction, paired control (§4g) | **RED CONFIRMED** — 500-at-UTXO header-free vs 202 with header, same body | 2026-08-19 | assistant |
| lock-state mitigation check (§4f) | **NO MITIGATION EXISTS** — no lock endpoint; DPAPI auto-unlock | 2026-08-19 | assistant |
| dev/prod deconfliction (§4h) | **VERIFIED** — separate DBs, concurrent, prod never touched | 2026-08-19 | assistant |
| regression set (0.5 → 1) | | | |
| adversarial review (panel) | **DO NOT SIGN OFF** — 23 findings, 3 critical | 2026-08-19 | panel |
| adversarial review (post-repair) | | | |
