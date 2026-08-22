# Phase 0.5 — money path & trust boundary · PHASE CONTRACT

**Workstream:** WS5(a) · **Ticket:** `../TICKET_loopback_host_form_wallet_routing.md` §6.2, §7.1, §7.3
**Status:** 🔴 NOT SIGNED OFF — **REPAIR COMPLETE, BLOCKED ON A NEW CRITICAL.** Findings 4, 5 and 6 are fixed and GREEN with REDs run in-session (§4j); the first panel's 3 criticals remain MEASURED and closed. ✅ **§4k — the new critical found 2026-08-19 — is FIXED and GREEN with RED (`81c054c`)**, including an owner-clicked control proving the connect-approval path still writes. Still owed: `G1` on a release-shaped build and the post-repair panel. Repair prompt: `../SESSION_PROMPT_beta3_p05_repair.md` · **Opened:** 2026-08-18 · **Amended:** 2026-08-19 (§4a–§4c, §5a), 2026-08-19 **repair scope** (§2, §4 split into 0.5a/0.5b, §4e–§4g, §5b, §6) · **Platforms:** both (Rust = one binary; the C++ gates are cross-platform)
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
- [x] ⭐ **C1** — origin derivation is **scheme-anchored**: only `http://`, `https://`, `hodos://`
      yield an origin; userinfo stripped through the last `@` of the authority; everything else
      returns empty **so the ancestor cascade and the `opaque-origin.invalid` sentinel actually
      run**. One derivation, one spelling, applied once. The same reduction inside
      `hodos::IsInternalFrontendUrl` / `hodos::IsLoopbackUrl`, which are **also** fooled by userinfo
- [x] ⭐ **C2** — every wallet-touching IPC arm is gated at the **top of
      `OnProcessMessageReceived`**, not per-arm. A wallet arm added later **fails closed by
      default**, not by someone remembering to add a check
- [x] The `send_transaction` IPC arm specifically no longer reaches Rust header-free from a web page (`X2`, §4i)

**Half 0.5b — the Rust money path**

- [x] `send_transaction` takes `HttpRequest` and routes external callers through `dispatch_payment`,
      exactly as `create_action` does
- [x] An internal caller (no `X-Requesting-Domain`) is **unchanged** — no modal, no new latency (`R1`, §4i)
- [x] ⭐ **C3** — `peerpay_send` and `paymail_send` take `HttpRequest` and route through
      `dispatch_payment`. ⚠️ `{recipient_identity_key|paymail, amount_satoshis}` is a **third** body
      shape — `d33741a`'s "do both or neither" warning applies with an extra shape
- [x] `.block_on_origin_mismatch(true)` on the CORS layer
- [x] Finding 4 resolved — allowlist the two re-issue origins, keep `block_on_origin_mismatch(true)`. Owner decision taken 2026-08-19. **GREEN with RED, §4j** (live dApp call, not reasoning). ~~Needs an
      owner decision (CLAUDE.md #13), not a reflex fix
- [x] Finding 6 resolved — the three fund-movers are priced, and `sendMax` is resolved by Rust from the spendable balance. **GREEN with RED, §4j**: 42,449,558 sats / `per_tx_limit` with the fix in, `0 sats` / `price_unavailable` with it out. ~~the forced prompt must not render **"0 sats"** under a false
      price-outage cause~~

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
| `P0.5-X1` ⭐**NEW** | A frame at `data:text/html,a://127.0.0.1:5137/<script>…` is stamped **external and gated** | ⛔ Pre-fix it must reach Rust **header-free** (internal). This crafted `data:` URL is the new RED — the `about:blank` case alone is a strict subset and was never the reachable input class | Rust-side gate outcome **and** the browser-process origin log line, in the same run as an internal control | T1 | ✅ **GREEN, RED observed — §4i.** Self-scripting crafted `data:` frame attributed to `example.com` and DENIED; pre-fix build dispatched it |
| `P0.5-X2` ⭐**NEW** | `cefMessage.send('send_transaction', …)` from `https://example.com` is **refused before dispatch** | ⛔ Pre-fix: **MEASURED spending, no prompt** — §4g | The **browser-process** IPC dispatch, not Rust alone. Rust cannot distinguish the transports; both arrive header-free | T1 | ✅ **GREEN, RED observed — §4i.** Refused at the gate, nothing reached the wallet; pre-fix the SAME call logged `💸 /transaction/send … 999999999999 satoshis` |
| `P0.5-X3` ⭐**NEW** | A **newly added** wallet arm with no explicit check is **denied** from an external origin | ⛔ Add a throwaway arm that calls a wallet endpoint → pre-fix it dispatches | The default-deny allowlist, not any one arm. This row is the whole point of C2 | T1 | ✅ **GREEN, RED observed — §4i.** `get_balance` denied; pre-fix it disclosed `Balance: 41924349 satoshis` to example.com |
| `P0.5-X4` ⭐**NEW** | `http://127.0.0.1:5137@evil.com/` is **not** internal to `IsInternalFrontendUrl` / `IsLoopbackUrl` | ⛔ Pre-fix both return **true** (verified: both are `rfind(pfx,0)==0`, so userinfo prefix-matches) | The two `PortConfig.h` predicates, unit-testable without CEF | T0 | ✅ **GREEN, RED observed** — 15 unit tests; 8 fail against the pre-fix bodies (commit `aa439d3`) |
| `P0.5-G1` | `https://<origin>/?x=127.0.0.1:5137` is served **nothing** from disk | ⛔ **Pre-fix this must SUCCEED** — if it does not, §7.3 is refuted and this row is withdrawn | **Release-shaped** build: `IsFrontendAvailable()` is true in production (`{app}\frontend\`), so this is not a dev-only defect | T2 | ✅ **GREEN, RED OBSERVED — §4p.** Release-shaped layout; pre-fix build served `{app}\frontend\index.html` (title **"Hodos Browser"**) onto `https://example.com`. Subject proven, not asserted |
| `P0.5-G2` | The same page gets **no** `window.hodosBrowser.identity` | Pre-fix it must be **defined**. Control: the same page *without* the substring must get neither | Renderer for **that page's** frame — not an overlay. `type:"page"` over CDP is not proof of which browser | T2 | ✅ **GREEN, RED observed** — §4b |
| `P0.5-G3` |  `preflight.ps1` gate `G2` passes at baseline **2** (from 5) | Add one new `find("127.0.0.1:5137")` → gate **exits non-zero** even at a non-zero baseline | `preflight.ps1 -NegativeControl` | T0 | ✅ **GREEN** `2 violations, at baseline`; 🔴 observed `3 > 2` |
| `P0.5-E1` | An origin-less **or origin-forged** frame is gated | Feed both an `about:blank` child **and** the crafted `data:` URL → each must be seen ungated pre-fix | The Rust-side gate outcome | T1 | ✅ **GREEN, RED observed — §4i.** `about:blank` child now inherits `example.com` and is denied; pre-fix empty origin ⇒ `IsInternalOrigin("")==true` ⇒ dispatched. Superseded by `X1` |

### Half 0.5b — Rust money path

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P0.5-R1` | User send from the wallet UI → **no modal**, tx broadcasts | Stub the internal branch to require approval → a modal appears | Frame URL `127.0.0.1:5137`; Rust log shows **no** `X-Requesting-Domain` | T2 | ✅ **GREEN — §4i.** Owner-run send, 201,274 sats: no modal, no gate line, no `X-Requesting-Domain`, no IPC denial, `POST /transaction/send 200`, broadcast `SEEN_ON_NETWORK` |
| `P0.5-R2` | External page, over-cap send → **202 + modal** | Revert the `dispatch_payment` wiring → the send completes silently | Rust log shows `X-Requesting-Domain: <exact page host>` | T2 | ✅ **GREEN — §4i.** External page → 202 + modal, owner saw and denied it; Rust logged `domain=example.com` (exact page host) |
| `P0.5-R3` ✏️**REWRITTEN** | External `sendMax:true` → **forced prompt** (not a cap evaluation) | Same revert → full balance sweeps | Scratch wallet, funded with a token amount. ⛔ **Never the production wallet.** The dev wallet (`HodosBrowserDev`, 41,931,019 sats) is a valid scratch wallet — the production wallet is a **separate DB** (§4h) | T2 | ✅ **GREEN as rewritten — §4i.** `sendMax:true` from an approved domain → 202 forced prompt. ⚠️ Rendered as **"0 sats"** (finding 6) |
| `P0.5-X5` ⭐**NEW** | `peerpay_send` / `paymail_send` from an external origin → **202 + modal** | ⛔ Pre-fix: both spend with **no gate of any kind** — they take no `HttpRequest`, so `dispatch_payment` is structurally impossible | Both endpoints, both body shapes. ⚠️ A **third** body shape — do both or neither | T2 | ✅ **GREEN, RED observed** — commit `9dc1586`. ⛔ First measurement was a FALSE GREEN (unknown domain ⇒ domain-trust fired first); re-run against an APPROVED domain |
| `P0.5-C1` | Cross-origin simple POST no longer executes the handler | Remove `block_on_origin_mismatch` → the handler runs despite the browser hiding the response | **Server-side effect**, not the browser's error. A blocked read is not a blocked write | T2 | ✅ **GREEN, RED observed — §4e + §4j.** Finding 4 is now CLOSED: the two re-issue origins are allowlisted and the page's own trust headers stripped, `block_on_origin_mismatch(true)` KEPT. Live dApp POST+GET → 200; with the two allowlist lines removed the POST returns **200 carrying a CORS error body**. Exact control: `:31302` / `:31400`, one digit away, still 400 |
| `P0.5-R4` ✏️**UN-WITHDRAWN** | Gold pill fires on a newly silent-approved send | Pre-finding-6 the pill could not fire for this endpoint at all | The **tab** badge (`Tab::id`), driven by `OnWalletCallSuccess` | T2 | ✅ **GREEN — §4j, observed live by the owner.** ⛔ The 2026-08-19 withdrawal rested on "`isPaymentEndpoint` excludes `/transaction/send`, so the pill can never fire" — **finding 6 kills that premise.** Log: `OnWalletCallSuccess fired (79 cents … /transaction/send)` and `(63 cents … /wallet/peerpay/send)`. R-GOLD holds on the newly-silent path. ⛔ The first-party send form still does NOT fire the pill and should not — it goes through `WalletService`, which has **zero** references to `OnWalletCallSuccess`, and the pill is a per-tab badge while the wallet overlay is not a tab |
| `P0.5-X6` 🚨**NEW 2026-08-21** | `POST /processAction` from an external origin is subject to the SAME gate as `/createAction` | ⛔ **RED OBSERVED — it is not.** Identical body/headers/domain: `/createAction` → `202 engine Prompt … per_tx_limit`; `/processAction` → **no gate line at all**, straight to build. Also driven from a real page via `__hodos_walletCall` | Rust log ordering: the INNER `📋 /createAction called` that `process_action` triggers, with no `engine Prompt/Silent/Deny` between it and `FULL REQUEST`. Page probe returns `location.href` | T1 | ✅ **GREEN, RED OBSERVED — §4o + §4q.** Owner approved the fix 2026-08-21. Post-fix: `202 engine Prompt … endpoint=/processAction`, from curl AND from the page. `R-INTEXT` re-proved: header-free caller unchanged |

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

### 4i. 🟢 LIVE RUN — the whole 0.5a table, GREEN and RED, same day, same machine

**MEASUREMENT, 2026-08-19.** Dev browser (`cef-native/build/bin/Release`, `HODOS_DEV=1`,
`--profile=Default`) on **CDP 9322**. Production browser ran concurrently on 9222 throughout and was
never driven; dev processes were killed **by executable path**, never image name (52 production
processes still alive after each kill, prod CDP still bound).

⚠️ **SUBJECT DISCIPLINE.** The browser exposed **11 CDP targets, every one `type:"page"`** — the
header and ten overlays included. The harness (`scratchpad/cdp.py`) therefore selects targets **by
URL only** and refuses on ambiguity, and every `evaluate()` returns `location.href` alongside its
value so each result carries proof of which document produced it. Tabs were **navigated**, never
created via `PUT /json/new` (CDP-created tabs bypass `OnBeforeBrowse`). For `X1` the crafted frame's
existence was additionally proven out of `Page.getFrameTree` before the assertion was read.

**RED was produced by rebuilding the shell from `8d0f270` (pre-fix `simple_handler.cpp` +
`PortConfig.h`), relaunching, and re-firing the identical calls** — not by reasoning about what the
old code would have done.

| Probe, from `https://example.com` | 🔴 PRE-FIX build | 🟢 POST-FIX build |
|---|---|---|
| `cefMessage.send('send_transaction', {amount: 999999999999})` (`X2`) | dispatched → wallet logged `💸 /transaction/send called` · `Amount: 999999999999 satoshis` | `🛡️ IPC DENIED: 'send_transaction' from external origin 'example.com'` — **nothing reached the wallet** |
| `cefMessage.send('get_balance')` (`X3`) | dispatched → wallet logged `✅ Balance: 41924349 satoshis` **disclosed to the page** | `🛡️ IPC DENIED: 'get_balance' …` |
| self-scripting `data:text/html,a://127.0.0.1:5137/<script>…` frame (`X1`) | dispatched | attributed to **`example.com`**, denied |
| `about:blank` child, `contentWindow.cefMessage` (`E1`) | empty origin ⇒ `IsInternalOrigin("")==true` ⇒ dispatched | attributed to **`example.com`**, denied |

**Controls in the same runs — the gate discriminates, it does not merely block:**

| Control | Result |
|---|---|
| `wallet_call` from `example.com` (allowlisted) | ✅ reached the wallet and was **engine**-gated: `🔒 IPC: domain example.com trust_level: unknown` → `minted approval … type=DomainApproval`. dApp interop intact |
| Overlays + header throughout | ✅ `🔒 IPC internal origin 127.0.0.1:5137 — direct dispatch`, continuously, unaffected |

⭐ **Two false starts, recorded because they are the interesting part:**

1. `f.contentWindow.cefMessage` on a **`data:`** frame throws
   `SecurityError: Blocked a frame with origin "https://example.com" from accessing a cross-origin
   frame` — a `data:` frame is opaque-origin, so the parent cannot script into it. **The parent-reach
   PoC shape does not work for `data:`; the frame must run its own inline script.** A harness that
   only tried the parent-reach shape would have reported X1 green against *vulnerable* code.
2. A second probe fired at the same domain while an approval was pending returned nothing — it was
   **queued** by `hasPendingForDomain`, not dropped. Sequencing artefact, not a defect.

**`P0.5-R1` — the pairing control, run by the owner in the wallet UI.** Real send, **201,274 sats**
to the wallet's own address:

| Check | Result |
|---|---|
| Approval modal | **none** — no `engine Prompt`, no `minted approval` in the Rust log |
| `X-Requesting-Domain` | **absent** (internal path) |
| Any `IPC DENIED` during the send | **none** — the new default-deny gate blocked nothing the UI needed |
| HTTP | `POST /transaction/send 200`, 2.05 s |
| Broadcast | `gorillapool_mapi accepted … SEEN_ON_NETWORK` |
| Balance | 41,924,349 → 41,721,875 = **−202,474** exactly (201,274 + 1,000 service fee + 200 mining fee) |

⇒ **R-INTEXT holds on both halves in the same session**: eleven wallet IPC arms are now default-denied
to web pages, and the first-party UI — which uses those same arms — is untouched.

**`P0.5-R2`** — external over-cap send via the allowlisted bridge → 202 + modal; **the owner saw the
prompt and denied it**; Rust logged `domain=example.com`, the exact page host.

**`P0.5-R3`** — from an *approved* scratch domain (so domain-trust passes and only the payment gate
can decide): both `amount:5000000` and `sendMax:true` → 202 `payment_confirmation` /
`price_unavailable`. Forced prompt, as rewritten.

🚨 **Finding 6 is now observed on FOUR calls, and the worst is `sendMax:true`.** A full-balance sweep
is presented to the user as `{"satoshis":0,"cents":0,"bsvPrice":0}` — **"0 sats"** — under a
price-outage cause that is not occurring (the same session's price cache returned **$14.89–$14.95**).
Endpoints affected: `/transaction/send` (both amount and sendMax), `/wallet/peerpay/send`,
`/wallet/paymail/send`. This is the single most user-hostile thing left in the phase.

**Dev-DB hygiene:** both scratch domain-permission rows (`approved-test.example`,
`r2-payment.example`) were deleted after use; `example.com` never persisted a row (deny writes none).

⚠️ **Out of scope, observed and NOT chased** (CLAUDE.md #13 — do not touch production code reached
via an incidental finding): the R1 send inserted its pending change output under the **unsigned**
txid `c86ebe1d…`, while the transaction that broadcast is `6e222710…` (the txid changes once
signatures are added; `c86ebe1d` is not and never will be on-chain). Balance is nonetheless exactly
right, and this matches the documented **ghost output** lifecycle `TaskFailAbandoned` exists to clean.
Separately, an incoming **unconfirmed** output to the wallet's own address is not credited by
`/wallet/sync` (full or partial): sync calls `fetch_all_utxos`, and the codebase has a *separate*
`fetch_utxos_single_address_with_unconfirmed` that it does not use. Both are **pre-existing** —
`git diff 637e216..HEAD` touches no UTXO/sync/balance/monitor file. **File as tickets, not here.**

### 4j. 🟢 SECOND LIVE RUN — the repair (findings 4/5/6, stoi, UTXO), independently re-run

**MEASUREMENT, 2026-08-19 evening**, by a different session from the one that wrote C1/C2/C3.
Dev wallet **31401**, dev browser **CDP 9322**; production ran throughout on 31301/9222 and was
never driven (verified: 51 prod browser processes alive after each dev kill, prod wallet 200 on
31301). Dev processes killed **by executable path** every time.

⚠️ **Every RED below was produced in THIS session** — none inherited from §4i.

**Finding 4 + 5 — CORS.** Subject: a real `https://` page's own `fetch`, asserted with
`location.href` in the same call.

| | 🟢 fix in | 🔴 two allowlist lines removed, rebuilt |
|---|---|---|
| POST `/getVersion` from `https://example.com` | **200** + real JSON | **200 carrying `Origin is not allowed to make this request`** |
| GET `/getVersion` from the same page | **200** + real JSON | **200** + real JSON (survives — see below) |

Three results in one control:

1. The allowlist entries are what admit the POST ⇒ **Chromium does stamp a loopback origin on the
   re-issue.** Panel finding 4 called that link "unverified by anyone"; it is now measured.
2. The GET survives *without* the allowlist because the **header strip** removed the page's own
   `Origin` ⇒ the two halves cover **different verbs** and neither alone is sufficient.
3. It reproduces finding 4's "silent and mangled" claim: the page receives **HTTP 200 carrying a
   plain-text CORS error body**, because `GetResponseHeaders` hardcodes `SetStatus(200)`.

Rust-boundary matrix (real `curl`, not the PowerShell alias): `127.0.0.1:31301` **200**,
`:31401` **200**, `:5137` **200**, no-Origin **200**; `https://zanaadu.com` **400**,
`https://evil.example` **400**, `null` **400**. ⭐ **Exact control:** `127.0.0.1:31302` and
`:31400` — one digit away — **400**. So it is the allowlist entries specifically, not "loopback is
trusted".

⚠️ **Two denylist entries the agreed fix sketch MISSED**, both of which would have left the hole
open on the headers that matter: **`X-Bsv-Price-Available`** is a trust header living *under* the
`x-bsv-` prefix this loop forwards on purpose (needs an EXACT strip, not a prefix strip, or the
strip takes the whole BRC-31 family with it), and **`X-Cert-Approved-Fields`** is injected on the
cert-replay path and Rust trusts it.

**Finding 6 — the "0 sats" prompt.** Same call, same wallet, minutes apart, one token changed
(`if send_max` → `if false && send_max`, rebuilt):

| | `satoshis` | `cents` | `engineReason` |
|---|---|---|---|
| 🟢 fix in | **42,449,558** | **670** | `per_tx_limit` |
| 🔴 fix out | **0** | 0 | `price_unavailable` |

⭐ **The cap branches are now REACHABLE for this endpoint.** §2's struck bullet said they were
unreachable and rewrote `R3` to "forced prompt". With the amount resolved, the engine reaches
`per_tx_limit` — so the *original* claim is achievable after all, by a different mechanism than
first written. `R3` stands as rewritten (a forced prompt still occurs); the note is that the reason
is now a real cap evaluation, not a false price outage.

**Finding 6, the C++ half** (browser transport only — ⛔ `curl` CANNOT test this: it has no C++
header injection, would show the OLD behaviour, and would read as a false failure):
`/transaction/send` priced at **79 cents**, `/wallet/peerpay/send` at **63 cents** — endpoints that
before this commit reached Rust unpriced and always rendered "0 sats".

**0.5a re-run in full** (`HARNESS` §5 — the whole table, not the failing row), from
`https://example.com`, all in one run:

| Probe | Verdict |
|---|---|
| `send_transaction`, `get_balance`, `address_generate`, `get_transaction_history` | 🛡️ **DENIED** — external origin |
| `find_result_js`, `qr_found`, `cosmetic_class_id_query` | ✅ dispatched, **no denial** |
| `about:blank` child (`E1`) | 🛡️ DENIED — attributed to the parent |
| crafted self-scripting `data:` frame (`X1`) | 🛡️ DENIED — attributed to the parent (frame existence proven out of `Page.getFrameTree`) |
| overlays + header throughout | ✅ `IPC internal origin 127.0.0.1:5137 — direct dispatch`, continuous |

⭐ **C2's four-name allowlist is INDEPENDENTLY CONFIRMED COMPLETE.** Re-derived from scratch: all
`CefProcessMessage::Create` sites split by process (only 5 are renderer→browser, one being the
generic `cefMessage.send`), then every C++/JS string literal containing `cefMessage.send`, then
every `ExecuteJavaScript` site targeting a *tab* frame. Exactly four names originate in page
context. `adblock-engine/` contains zero `cefMessage` references. **`find_result_js` and `qr_found`
had never been exercised** and are now measured passing. This run is also a within-run control: same
page, same API, some names denied and some allowed ⇒ the gate discriminates by name+origin rather
than blanket-blocking.

**`P0.5-R4` — UN-WITHDRAWN, and green.** The row was withdrawn on the premise that
`isPaymentEndpoint` excludes `/transaction/send` so the pill can never fire. Finding 6 kills that
premise. **Observed live by the owner**, and in the browser log:
`💰 OnWalletCallSuccess fired (79 cents from example.com … endpoint=/transaction/send)` and
`(63 cents … /wallet/peerpay/send)`. R-GOLD holds on the newly-silent path.
⛔ The **first-party send form still does not and should not fire the pill** — it goes through
`WalletService`, which has **zero** references to `OnWalletCallSuccess` (verified). The pill is a
per-**tab** badge keyed on `Tab::id`; the wallet overlay is not a tab.

**Unit tests.** 211 C++ pass (14 new in `tests/payment_cost_test.cpp`), 1 pre-existing skip
(`UpdateStagerRig.StagesFromLocalFeed`, needs a rig — named, because a skip is never a pass); 33
`hodos_permission_engine` pass.
⭐ **Negative control, and it earned its keep.** Deleting the `sendMax` arm does **not** make
satoshis 0 — it makes it **1**, the decoy `amount:1` in the sweep body. The failure mode is not
"obviously broken", it is "plausible one-satoshi payment, priced at 0 cents, silently approved,
entire balance gone". Seen RED for that exact reason, then green on restore.

### 4k. ✅ CLOSED — an approved dApp could rewrite the whole permission table (fixed `81c054c`)

**MEASURED 2026-08-19 from a REAL third-party dApp** (`brc-cloud.bcryderman.workers.dev`, approved
by the owner with one ordinary click). A page-context call through the wallet bridge created:

```json
{"domain":"escalation-probe.invalid","trustLevel":"approved",
 "perTxLimitCents":9999999,"identityKeyDisclosureAllowed":true}
```

**No modal. No user interaction. Silent success.** Row read back from the DB to confirm, then deleted.

`handlers.rs :: set_domain_permission` has **no gate of its own** — zero `check_domain_approved`,
zero `dispatch_*` (grepped). Its only protection is `domain_trust_mw`, which checks the **calling**
domain. So once the user approves **any** dApp — the most ordinary action in the product — that dApp
can:

- approve **itself** with a `$99,999.99` per-tx cap ⇒ silent unlimited spending, payment caps defeated
- set `identityKeyDisclosureAllowed: true` ⇒ silent identity-key reveal (a privacy-perimeter gate)
- approve **any other domain** ⇒ collaborator sites the user never saw

It cannot set `blocked` (validation admits only `approved`/`unknown`), which is irrelevant to the attack.

⚠️ **Scope of the proof:** the unrestricted **write** is measured. It was NOT chained to an actual
silent spend — that costs money, and self-targeting is the identical call with a different domain
string. Not introduced by this phase; long-standing.

**FIXED 2026-08-20, `81c054c`.** `domain_trust_mw` now refuses POST/DELETE on
`/domain/permissions*` plus `/wallet/session-approve` / `-revoke` when the request carries
`X-Requesting-Domain` — i.e. when it came from a web page.

⭐ **Why "has the header" == "came from a web page", verified at every call site:** every
legitimate writer is first-party and reaches Rust header-free. The C++ modal-approval writes use
`SyncHttpClient::Post` / `CefRequest` with **Content-Type only**, and the wallet UI goes down the
internal IPC path, which builds its header map from scratch. `X-Requesting-Domain` is stamped
solely on dApp-request forwarding paths.

⛔ **Gated ONCE over the whole subtree, not per handler** — a sub-permission endpoint added later is
refused by default rather than by someone remembering. Gating arm-by-arm is how the
`send_transaction` IPC arm was missed. Reads are deliberately **not** blocked (a narrower privacy
question, filed separately rather than widened into this fix).

| | Result |
|---|---|
| 🔴 **RED** (pre-fix) | page call RESOLVED, row `id=86` written, `perTxLimitCents 9999999`, `identityKeyDisclosureAllowed true` |
| 🟢 **GREEN** (post-fix) | identical page call → `REJECTED permission_table_is_first_party_only`, **no row** |
| 🟢 **GREEN, hardest case** | re-tested with the site **fully approved incl. `bundledScopeGrant`** → still refused |

⭐ **THE PAIRING CONTROL — the failure this fix could itself have caused, and the one `curl` cannot
prove.** The dApp was revoked, a wallet call fired to raise a real connect modal, and **the owner
clicked Allow**:

```
🔐 Domain permission sync write ... -> status 200      (NOT 403)
🔐 Drained 1 pending request(s) ... (1 resumed)
```

Row `id=88` created; the parked `CWI.getVersion()` then returned real wallet JSON. ⇒ **The gate
blocks the SITE's write and permits the BROWSER's write on the user's behalf.** That distinction is
the entire permission model, and it is now measured in both directions on the same domain minutes
apart.

### 4l. Environment findings from the live run — file, do not chase

- 🚨 **`peerpay_send` broadcasts to any well-formed identity key with no reachability check.**
  Demonstrated **accidentally**: a probe passed a fabricated recipient key, and 4,000,000 sats were
  derived to an address whose private key nobody holds and **broadcast** (`668f4fc1…`,
  SEEN_ON_NETWORK). Unrecoverable. A user who mistypes an identity key silently destroys funds.
  The contract already noted its only validation is `amount_satoshis <= 0`; this makes the
  consequence concrete. **Dev wallet only; net −4,002,400 sats ≈ $0.63.**
- ⚠️ **The C++ `DomainPermissionCache` goes stale on a direct `POST /domain/permissions`.** C++ read
  `example.com` as `trust_level: blocked` while the DB said `approved`. Harmless *here* only because
  C++ is a thin proxy and forwards regardless, with Rust authoritative — but the `blocked` in those
  log lines is **not what the engine decided on**, which is a trap for anyone reading them later.
- ⚠️ **Chromium 150 gates the direct-fetch transport behind a Local Network Access prompt.** A public
  `https://` site must be granted permission before it may reach `127.0.0.1` at all. Observed live:
  the request fails with `Failed to fetch` and **no network event**, so it never reaches our CORS
  layer. Every dApp on `@bsv/sdk WalletClient`'s default transport now needs that one-time grant.
  The IPC bridge is unaffected.
- ⛔ **`brc-cloud.bcryderman.workers.dev` cannot work with Hodos regardless of the CORS fix, and it
  was a MISLEADING symptom.** Its own CSP pins `connect-src` to `https://127.0.0.1:2121` (HandCash
  bridge) and `http://127.0.0.1:3321` (MetaNet), and the page contains **zero** references to
  `window.CWI` — it probes those two ports and nothing else. So the browser blocks a fetch to our
  port before the network stack. Hodos's injected provider *does* work on that page
  (`CWI.getVersion()` returned real wallet JSON), but the site never looks for it.
  ⇒ If this site motivated the CORS investigation, the 400s were real but were **never** what
  stopped it.
- ⭐ **Full connect cascade proven on a real third-party dApp:** unknown domain → 202 → Hodos connect
  modal → owner approved → `🔐 Drained 1 pending request(s) … (1 resumed)` → wallet responded.
- ⚠️ **`POST /domain/permissions` is `#[serde(rename_all = "camelCase")]`.** snake_case keys are
  silently dropped to `None` — no error, the row just does not change. Cost one confused cycle.

### 4m. ⭐ OUT-OF-SCOPE FIX, owner-approved — foreign wallet-bridge interception (commit `9b73bd7`)

**Not Phase 0.5** (dApp interop, not the money path or trust boundary). Recorded here because it
was found by this phase's live harness and fixed while the diagnosis was hot; it belongs to the
interop ticket line, and **it does not gate 0.5 sign-off**.

**The symptom.** The HandCash App Lab (`brc-cloud.bcryderman.workers.dev/app-lab`, built by the
HandCash devs) reported **"Bridge unavailable"** against Hodos. Hodos is supposed to intercept ANY
local wallet-bridge call and re-point it at our wallet — that re-pointing is the entire reason this
browser owns its own ports. It never fired.

⛔ **TWO WRONG DIAGNOSES, both recorded because both were plausible and both cost time:**

1. *"The site's CSP blocks us."* **False.** Its CSP permits `https://127.0.0.1:2121` and
   `http://127.0.0.1:3321`; the only thing CSP blocked was a probe **invented by the tester** to
   31401 — a request the site would never make. Concluding from the failure of a synthetic request
   that the real path was broken is the same class of error as the farbling harnesses in
   `feedback_negative_control_required`: **measuring the wrong subject.**
2. *"The site hardcodes competitor ports, so it cannot work with us."* **False, and backwards** —
   intercepting exactly that case is the product's job.

**Three defects, each invisible until the one before it was fixed:**

| # | Defect | Why it hid the next one |
|---|---|---|
| 1 | Foreign-bridge arms were bare `url.find("localhost:3321")` literals — **one host spelling only** | Request never reached the interceptor at all, so 2 and 3 were unobservable |
| 2 | `redirectPort` rewrites host:port but **not the scheme** — `https://…:2121` became `https://…:31401` against an HTTP-only wallet | TLS handshake failure looks identical to "no bridge" |
| 3 | **`/health` was not in `isWalletEndpoint`** — the FIRST call any bridge-probing dApp makes | Correctly re-pointed, then dropped one line later: `Not a wallet endpoint, allowing normal processing` |

⭐ **Defect 1 is exactly the trap `hodos::IsWalletHostPort` exists to prevent** — `cef-native/CLAUDE.md`
already says the two spellings "must move in lockstep" — and these three literals bypassed it. Fixed
with `hodos::IsLoopbackHostPort(url, port)` in `PortConfig.h`, beside `IsWalletHostPort`, so the pair
cannot drift again. **Route every future foreign-bridge port through the helper, never a literal.**

⛔ **Defect 3's fix is HOST-SCOPED on purpose.** Every other arm of `isWalletEndpoint` is a bare path
substring — fine for a distinctive name like `/createAction`, **not** for `/health`. Unscoped it
would hijack the health endpoint of every ordinary website the user visits and route it to the
wallet. The arm must stay qualified by `IsWalletHostPort`.

**EVIDENCE — paired, same machine, rebuild the only variable.**

🔴 **RED (pre-fix):** both `127.0.0.1` forms produced **no interception line whatsoever**;
site banner **"Bridge unavailable"**.

🟢 **GREEN (post-fix)** — same probe, all four rewrites logged:

```
http://localhost:3321/getVersion  -> http://localhost:31401/getVersion
http://127.0.0.1:3321/getVersion  -> http://127.0.0.1:31401/getVersion
https://localhost:2121/getVersion -> https://...:31401 -> http://localhost:31401
https://127.0.0.1:2121/getVersion -> https://...:31401 -> http://127.0.0.1:31401
```

Site banner flips to **"Connected"**, and its own BRC-100 runner produces a call our wallet serves:
`POST /getVersion HTTP/1.1" 200 221`.

⇒ **A third-party dApp hardcoded to the HandCash and MetaNet bridges now talks to Hodos.**
Read-only functions only were exercised; the BRC-29 payment runner was deliberately not clicked.

⚠️ **This supersedes the brc-cloud bullet in §4l**, which concluded the site "cannot work with Hodos
regardless of the CORS fix". That conclusion was drawn from the wrong subject and is **withdrawn** —
struck in place per `HARNESS` §8 rather than deleted, because the reasoning error is the instructive
part. What survives from that bullet: the site genuinely contains **zero** references to
`window.CWI`, so it is a bridge-probing client rather than an injected-provider client, and Hodos's
injected provider does also work on that page (`CWI.getVersion()` returned real wallet JSON).

### §4n — adversarial panel #2 disposition (2026-08-20)

Panel #2 returned **37 survivors, 6 criticals, DO NOT SIGN OFF** — and named
concurrency as a modality no lens had examined. Task 0 was to settle that FIRST,
because if it held it outranked every other fix.

#### Task 0 — the concurrency TOCTOU: **REFUTED as an exploitable path**

The panel's structural reading is CORRECT: `dispatch_payment_with_amount` takes
three separate lock acquisitions — snapshot, decide, then
`increment_payment_rate_counter` + `record_spending` — and `HttpServer::new`
(`main.rs:964` → `.bind()` `:1208` → `.run()` `:1209`) sets **no `.workers()`**,
so actix runs `num_cpus` workers. Verified current. **But it does not reproduce.**

| | |
|---|---|
| Subject | `POST /transaction/send`, APPROVED scratch domain, `X-Payment-*` stamped by hand as C++ would |
| Safety | valid-prefix / invalid-checksum `toAddress`, so every request that passes the gate dies at address decode. ⚠️ Address validation runs BEFORE the gate (`handlers.rs:9646`), so a malformed-FORMAT address measures nothing — the checksum failure is what lands after it |
| Design | caps set so the CORRECT answer is exactly **1** (`perSession=5c`, each pay `4c`) |
| Overlap proven | 20 requests completed inside a **4 ms** window, durations to 7.7 ms, 24 CPUs |
| **Harness negative control** | `perSession=1000c` → **20 of 20 passed**. The harness CAN report >1 |
| **Result** | **420 concurrent requests over 11 rounds** (K=20 ×6, K=60 ×5) → **exactly 1 passed, every time** |

**Why it does not win.** The global `Mutex<WalletDatabase>` is acquired inside
`dispatch_payment` immediately BEFORE the snapshot. A competing thread must
complete a SQLite read (~100 µs) before it can snapshot, which is far longer than
the winner's snapshot→record window (~2 µs). The serialization is **incidental**,
not designed.

⛔ **This is LATENT, not closed.** It is a race that is hard to win, not one that
cannot be won — a slower DB, a faster snapshot, or a future refactor that moves
the DB read off that path re-opens it. **Recommended follow-up:** make
check-and-record atomic under one write lock. Not a beta.3 blocker.

> ⚠️ My first cut of this test was worthless and I nearly reported it: with
> `perSession=10c` and `4c` payments the correct answer is **2**, and I measured
> 2 — a number that cannot discriminate. Same failure family as the three farbling
> harnesses. The negative control is what forced the redesign.

#### Task 1 — the six Tier-1 regressions

| # | Finding | Status | Evidence |
|---|---|---|---|
| 1.1 | Decoy `outputs` key ⇒ 0 cents, `priceAvailable=true` ⇒ silent | ✅ FIXED `9e51134` | 9 unit tests GREEN; **RED = 6 fail on reverted header**; decoy → 400 live |
| 1.2 | `createAction` `options.sendMax` ⇒ silent full sweep | ✅ FIXED `9e51134` | unit GREEN + RED, same run |
| 1.3 | `/domain/%70ermissions` bypasses §4k | ✅ FIXED `775d87e` | **MEASURED**: pre-fix 200 + row rewritten → post-fix 403 + row unchanged |
| 1.4 | `POST /wallet/session/close` resets all three counters | ✅ FIXED `775d87e` | **MEASURED**: pre-fix cap reset → post-fix 403, cap held; first-party still 200 |
| 1.5 | `window.yours.disconnect()` broken by the §4k gate | ✅ FIXED `775d87e` | self-revoke 200; **negative control**: other domain 403 |
| 1.6 | `confirmed=1` live on `wallet_recover` / `wallet_rescan` | ✅ FIXED `9e51134` | code fix + build; ⬜ NOT exercised against a real mempool-only UTXO |

**Two false commit claims, corrected for the record:**

- `81c054c` "verified at every call site" — **FALSE.** It audited C++ call sites and
  missed the page-context JS that C++ itself injects (`CWIShimScript.h`). Its own
  §4k row was also **overstated**: the gate it describes was bypassable by one
  percent-encoded character.
- `4eacb51` "recovery.rs needs none" — **FALSE at the dataflow.** File-scoped audit;
  `upsert_received_utxo_with_derivation` omitted `confirmed` from its INSERT.

#### Task 2 — owner decisions TAKEN 2026-08-21

| # | Item | Decision |
|---|---|---|
| 6 | **Does beta.3 ship macOS?** | ✅ **YES.** This promotes the macOS `wallet_call` SSRF from follow-up to **SIGN-OFF BLOCKER**. Relayed to the macOS session as ask E1 (`MAC_RELAY_BETA3.md`, round 2026-08-21) with verified citations, a repro and a cross-platform negative control. **Windows fails closed only by accident** — `ParseUrl`'s digits-only port check, inside `#ifdef _WIN32` — so it cannot be fixed from this side. |
| 2 | `/wallet/reveal-mnemonic` to page context | ✅ **FIXED in beta.3** — `c8558dc`. RED: page origin → **401, handler REACHED**; only the PIN stopped it, and the no-PIN branch has none. GREEN: 403, handler never runs. First-party still 401 on a wrong PIN. |
| 3 | `POST /wallet/settings` page-callable | ✅ **FIXED in beta.3** — `c8558dc`. RED: page origin → **200, global defaults REWRITTEN** (per_tx 1000 → 999999, per_session 5000 → 999999). GREEN: 403. Defaults restored after the RED. |
| 1 | `IsInternalOrigin("")` + second derivation | ⬜ still owed — empty-origin half recommended for beta.3, port half to Phase 5. **The C1 banner claim is FALSE either way and must be corrected.** |
| 4 | Loopback-port trust | ⬜ Phase 5 headline |
| 5 | Two-phase action lifecycle | ⬜ **measure before fixing** — largest unexamined surface |

⛔ **macOS is now in scope for sign-off.** Panel #2 examined exactly ONE line of the macOS tree
(the curl SSRF). `cef_browser_shell_mac.mm`, the `Create*OverlayMacOS` roster and
`InstallClickOutsideMonitor` are **unaudited by anyone**. "Panel #2 cleared" means the *Windows*
money path was cleared. CLAUDE.md invariant #9 parity verification is outstanding, and the C++
`PaymentCost.h` change + its 9 tests have **not been built on macOS**.

#### Task 2 item 1 — MEASURED, then FIXED (`13e6e2d`), 2026-08-21

The panel filed this as `IsInternalOrigin("") == true`. **That framing was wrong.** The IPC half
already fails closed (`ResolveIpcOrigin` substitutes `opaque-origin.invalid`). The real defect was a
**userinfo bypass on the HTTP transport**, and it is now measured, not read.

| | Input `https://127.0.0.1:31301@example.com/` |
|---|---|
| **CONTROL** `https://example.com/` | `Extracted domain: example.com` — correct, both before and after |
| **RED** (pre-fix) | `Extracted domain: 127.0.0.1:31301@example.com` → `🔒 Internal origin … — bypassing domain check` |
| **GREEN** (post-fix) | main-frame URL **identical**; `Extracted domain: example.com`; `Internal origin` × **0** |

**Reachability settled affirmatively:** from that document
`fetch("http://127.0.0.1:31401/wallet/status")` returned **HTTP 200**, so the Chromium 150 Local
Network Access gate does **not** close this path. `extractDomain` has fired 17 times historically,
every time for a real public https origin.

⚠️ **The page's own view does not betray it.** Chromium strips credentials from `location.href`
(which read plain `https://example.com/`) while CEF's `GetURL()` keeps them.

⭐ **END-TO-END, OWNER-WITNESSED:** `POST /transaction/send` from the spoofed page now raises a
domain-approval modal and **the owner clicked Block** — `"User rejected authentication"`. Pre-fix no
modal would have appeared, because the domain check was skipped outright. Balance unchanged either
side: `38,341,860`.

Fixed by **calling** `hodos::OriginFromUrl` — the derivation that already existed, was already used
by the IPC path, and was already unit-tested against this exact input. Empty now fails closed via
the same opaque sentinel; safe because `extractDomain` produced an empty result **zero** times
across the 204 MB dev log.

#### Task 2 item 5 — MEASURED 2026-08-21. The lifecycle is fine; **`/processAction` is not.**

⛔ **Read the disposition before the mechanism, because both panel hypotheses were WRONG and the
thing that is actually broken is a third endpoint neither of them named.**

| Panel hypothesis | Verdict |
|---|---|
| `PENDING_TRANSACTIONS` references are not domain-bound | ✅ **TRUE, and it does not matter on its own.** `PendingTransaction` (`handlers.rs`) has **no** domain field and `sign_action` looks up by reference alone — but the key is `action-{uuid v4}`, 122 bits, returned only to the caller that created it. **MEASURED:** `/signAction` with `action-00000000-0000-4000-8000-000000000000` → `404 Transaction reference not found`; existence is the *only* check performed. See the `/listActions` finding below for what removes the secrecy. |
| `options.noSend` changes the effective spend between phase 1 and phase 2 | ❌ **REFUTED BY MEASUREMENT.** The phase-1 gate is noSend-blind: `IsPaymentEndpoint`/`ComputePaymentCost` (`PaymentCost.h`) never read it, and `dispatch_payment` prices from the `X-Payment-*` headers. **MEASURED:** the identical over-cap body with `options.noSend:true` still returned `202 … engineReason:"per_tx_limit"`, cents=17, exactly as with noSend absent. Flipping noSend at `signAction` therefore promotes an *already-approved-at-that-amount* action from unbroadcast to broadcast; it cannot raise the amount. Filed as a residual (approving a nosend action is not the same consent as approving a broadcast), **not** an escalation. |
| `spends` changes the effective spend | ❌ **REFUTED at the dataflow.** `spends` writes only `tx.inputs[idx].set_script()` and `.sequence`. The **outputs are immutable** from phase 1, and the wallet-input signing loop re-signs indices `num_user_inputs..` unconditionally, overwriting any page-supplied script there. No lever on amount. |

### 🚨 What the measurement found instead — `P0.5-X6`

`/processAction` (`handlers.rs :: process_action`) is a **create + sign + broadcast in one call** that
takes `(state, body)` — no `HttpRequest` — and then **manufactures one**:

```rust
let internal_req = actix_web::test::TestRequest::default().to_http_request();
let create_response = create_action(state.clone(), internal_req, …).await;
```

That synthetic request carries no headers, so `create_action`'s `dispatch_payment` takes its
`None => Proceed` branch and `check_domain_approved` finds no domain. **The entire payment gate is
skipped — not bypassed by a trick, erased by construction.**

**MEASURED, paired, same body, same headers, same approved domain, 15 ms apart:**

| | `POST /createAction` (CONTROL) | `POST /processAction` (SUBJECT) |
|---|---|---|
| HTTP | **202** `{"promptType":"payment_confirmation","engineReason":"per_tx_limit"}` | **400** `Invalid address: Address checksum mismatch` |
| Wallet log | `🛡️ engine Prompt (payment) minted approval id=ba033101… endpoint=/createAction reason=per_tx_limit` | `📋 /processAction called` → `Broadcast: true` → `📋 /createAction called` → **no gate line of any kind** → `Failed to convert address` |
| What stopped it | the gate | **only my deliberately-broken checksum** |

**Reachable from an ordinary web page — MEASURED end to end**, not reasoned. From
`https://teragun.com/` in the dev browser (CDP 9322, target selected by URL, `location.href`
returned with the result):

```js
window.__hodos_walletCall('processAction', '/processAction',
  {outputs:[{satoshis:1000000, address:'1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW'}]}, 'POST')
```
→ `📋 /processAction called` · `Broadcast: true` · `📋 /createAction called` · **no modal, no gate
line** · died at the address checksum.
**CONTROL from the same page, same body, 22 s later:** `/createAction` →
`🛡️ engine Prompt (payment) minted approval id=197752a0… reason=per_tx_limit`, modal raised,
**owner asked about it unprompted and clicked Deny.**

**Why the transports do not save us.** `wallet_call` is necessarily on the C2 web-page allowlist and
takes the endpoint string **verbatim from the page**; `IsPaymentEndpoint` (`PaymentCost.h`) does not
list `/processAction`, so C++ never classifies it as a spend and forwards it silently.

**Precondition and severity.** Exactly **one ordinary "Connect" click**. MEASURED: from an
unapproved origin `domain_trust_mw` answers first — `202 domain_approval / new_domain_no_manifest`.
From an **approved** origin the per-tx cap, the per-session cap, `max_tx_per_session` and the rate
limit are **all void**, and no gold pill fires because `OnWalletCallSuccess` never sees a payment.
This is the same defect class as the `send_transaction` one this phase exists to close, and it
survived for the same reason: **the fix enumerated endpoints instead of gating a subtree.**

### 🚨 Adjacent, found by the same measurement — `/listActions` hands over the whole wallet

`/listActions` also takes no `HttpRequest`. **MEASURED from `X-Requesting-Domain: teragun.com`:**
`{"totalActions":242,...}` — the **complete transaction history**: txids, `referenceNumber`,
amounts, descriptions, labels, inputs, and with `includeOutputs:true` every **recipient address**.
Byte-identical to the first-party response. Same probe against the read surface:

| Endpoint | dApp origin result |
|---|---|
| `GET /wallet/balance` | 200 — `{"balance":19387214,…}` |
| `GET /wallet/addresses` | 200 — **53,199 bytes**: every address, its **public key**, and its `used` flag |
| `GET /wallet/activity` | 200 — spend history |
| `GET /wallet/tokens` | 200 — token outputs |

`/wallet/addresses` is a complete **key-linkage graph** — the thing the BRC-72 privacy perimeter
exists to protect — disclosed wholesale with no prompt. And `/listActions` returning
`referenceNumber` is what removes the "unguessable UUID" protection above.

⚠️ My first `/listActions` probe returned `{"totalActions":0}` and I nearly recorded "not
reproduced". That was my own `labels: []` filter, not a gate. **Re-run with the filter removed
before believing an empty result.**

### The systemic shape — 73 of 107 routes

`probes/ungateable.py` cross-references every route registered in `main.rs` against its
handler's signature: **107 routes registered, 34 take `HttpRequest`, 73 do not.** Those 73 cannot
gate themselves at all. Most are harmless (`/health`, `/getVersion`), but the class includes
`/processAction`, `/signAction`, `/abortAction`, `/internalizeAction`, `/wallet/broadcast-nosend`,
`/acquireCertificate`, `/wallet/certificate/{publish,unpublish}` (1000-sat service fee each),
`/wallet/consolidate-dust`, `/wallet/backup/onchain`, `/listActions`, `/wallet/addresses`,
`/proveCertificate`, `/discoverByAttributes`, `/wallet/{delete,recover,rescan,unlock}` and
`/shutdown`. ⛔ Only `/processAction`, `/listActions` and the four read endpoints above were
**measured**; the rest of that list is **CODE_READING** and must not be reported otherwise.

`domain_trust_mw` is the only place a decision can be made for any of them, and today it makes
exactly two: the `is_permission_surface` refusal and the domain-trust gate. There is **no payment
gate and no privacy-perimeter gate in the middleware**, and `is_permission_surface` is a **list of
exact strings** — the pattern this phase has already been bitten by twice.

### Recommendation — ⛔ NOT DONE, owner decision owed (CLAUDE.md #13)

1. **beta.3 blocker: `/processAction`.** Smallest correct fix is to give `process_action` an
   `HttpRequest` and **forward it** to `create_action` instead of manufacturing one — the gate then
   runs unchanged, `X-User-Approved` replay included. ⚠️ Verify the four other production
   `TestRequest` sites (`send_transaction`, `peerpay_send`, `paymail_send`, `pay_402`) still gate
   *above* their synthetic request; three were fixed this phase and `pay_402` says so in a comment
   that must be re-checked, not trusted. Add `/processAction` to `IsPaymentEndpoint` in the same
   commit so C++ prices it and the gold pill can fire.
2. **beta.3 or Phase 5, owner's call: the disclosure set.** `/listActions`, `/wallet/addresses`,
   `/wallet/activity`, `/wallet/balance`, `/wallet/tokens`. This is a policy question, not a bug fix
   — some dApp read access is presumably intended — so it needs a decision on *what an approved site
   may read*, not a reflex 403.
3. **Structural, Phase 5: stop enumerating.** The predicate belongs in the middleware as a
   **default-deny subtree** mirroring C2's IPC gate, not as a growing string list in
   `is_permission_surface`.
4. **Residual:** approving a `noSend` action is not consent to broadcast it; `signAction` and
   `/wallet/broadcast-nosend` can both flip that with no further prompt.

### 4p. ✅ `P0.5-G1` — CLOSED on a release-shaped build. RED reproduced, subject proven

§4a said this could not be judged in dev because `IsFrontendAvailable()` is false without a
`frontend/` beside the exe. It was made judgeable rather than deferred: `npm run build` →
`frontend/dist` copied to `cef-native/build/bin/Release/frontend/`, which is exactly the production
layout. `IsFrontendAvailable()` has **no** dev/release condition — it checks only for
`<exe_dir>\frontend\index.html` — so this reproduces the production precondition faithfully.
It also **caches in a static**, so the directory must exist *before* launch.

**Two builds, one line different**, everything else identical:

| Build | Predicate at `simple_handler.cpp :: GetResourceRequestHandler` | `https://example.com/?x=127.0.0.1:5137` | Control `https://example.com/` |
|---|---|---|---|
| **A — RED** | `url.find("127.0.0.1:5137") != npos` (the pre-fix form, recovered from `4dec940`) | 🔴 `origin: https://example.com`, **`title: "Hodos Browser"`**, `Hodos_Gold_Icon` present — the wallet UI's `index.html` served **from disk onto the attacker's origin** | `title: "Example Domain"` |
| **B — GREEN** | `hodos::IsInternalFrontendUrl(url)` | ✅ `title: "Example Domain"` | `title: "Example Domain"` |

§7.3 is therefore **confirmed, not refuted** — the SPA fallback in
`LocalFileResourceRequestHandler` does serve `index.html` for a query-string match.

⛔ **SUBJECT CONTROL — the green would otherwise be vacuous.** If the `frontend/` directory had
been missing in build B, `IsFrontendAvailable()` returns false, the handler never engages, and both
URLs return `Example Domain` **for the wrong reason** — which is exactly what §4a measured
in dev and correctly refused to call a pass. Proof that it engaged in build B: `__g1_marker.txt`
was planted in `{exe}\frontend\` only. From inside the browser,
`fetch("http://127.0.0.1:5137/__g1_marker.txt")` returned **`G1-SUBJECT-MARKER-a7f3c2`**, while
Vite answers that same URL from outside the browser with its dev-server `index.html`. The marker
can only have come from disk ⇒ the local-file handler was live.

The release-shaped `frontend/` and the marker were **removed afterwards** and the dev browser
relaunched against Vite, so the dev tree is back to its normal state.

### 4q. ✅ `P0.5-X6` FIXED — owner approved 2026-08-21

Owner decision: **fix in beta.3**, narrow form. Two halves, one change:

| | |
|---|---|
| Rust | `process_action` now takes `HttpRequest` and calls `dispatch_payment(…, "/processAction")` **before** anything else. The inner `create_action` call keeps its synthetic request **deliberately** — `create_req` is re-serialised and is NOT the caller's body, so forwarding `http_req` would make every `X-User-Approved` replay 403 on `body_mismatch`. What was wrong was never the synthetic request; it was that nothing gated above it. |
| C++ | `/processAction` added to `IsPaymentEndpoint` (`PaymentCost.h`) so the `X-Payment-*` headers are stamped. **No fifth body shape** — it is the same `{outputs:[{satoshis}]}` `/createAction` uses. |

**GREEN, and the RED is the run I did 22 minutes earlier on the pre-fix binary:**

| | Pre-fix (RED, 08:45 / 08:47) | Post-fix (GREEN, 09:07 / 09:21) |
|---|---|---|
| curl, approved domain, over-cap | no gate line; `400 Address checksum mismatch` | `202` — `🛡️ engine Prompt (payment) minted approval id=e777a6cf… endpoint=/processAction reason=per_tx_limit` |
| From `https://teragun.com/` via `__hodos_walletCall` | no modal, `Broadcast: true`, straight to build | `🛡️ engine Prompt (payment) … endpoint=/processAction reason=per_tx_limit` |

⭐ `reason=per_tx_limit` — **not** `price_unavailable` — is the discriminator proving the **C++
half** landed too: the call was priced at 17 cents against teragun's 13-cent cap, which is only
possible if `X-Payment-*` were stamped. Had only the Rust half shipped, this row would still be
green but for the weaker fail-closed reason, and the difference is exactly the "do both or neither"
rule.

⛔ **`R-INTEXT` re-proved in the same run** — this is the pairing a `/processAction` fix is most
likely to break. A header-free (first-party) `POST /processAction` is **unchanged**: no gate line,
straight through to `create_action`, dying at the probe checksum exactly as before.

**Unit tests:** `IsPaymentEndpoint.ProcessActionMatches` and
`ComputePaymentCost.ProcessActionUsesTheCreateActionOutputsShape` added to
`cef-native/tests/payment_cost_test.cpp`. **NEGATIVE CONTROL RUN:** with the one-line
`PaymentCost.h` change reverted and the target rebuilt, **both fail**; restored, the full suite is
**221 passed / 1 skipped / 0 failed (222 tests)**.

**Re-audit of the other four synthetic-request sites** (the prompt warned not to trust `pay_402`'s
own comment): mechanically re-derived, not read — `probes/ungateable.py`'s sibling check walks each
`TestRequest::default().to_http_request()` site back to its enclosing `fn` and looks for a
`dispatch_*` call in between. `send_transaction` → `dispatch_payment_with_amount`; `peerpay_send`,
`paymail_send`, `pay_402`, and now `process_action` → `dispatch_payment`. Each was then checked to
pass the **real** `&http_req` and `&body`, not a stand-in. All five clean.

⚠️ **Still owed from §4o, deferred by owner decision to Phase 5:** the disclosure set
(`/listActions`, `/wallet/addresses`, `/wallet/activity`, `/wallet/balance`, `/wallet/tokens`) and
the structural move of the predicate into a **default-deny subtree** in `domain_trust_mw`.
`is_permission_surface` remains a list of exact strings.

⚠️ **Owed, and NOT written:** there are still **zero** C++ tests for `IsInternalOrigin`,
`ResolveIpcOrigin` or `IpcMessageAllowedFromWebPage`. `ResolveIpcOrigin` and
`IpcMessageAllowedFromWebPage` are `static` in `simple_handler.cpp` and `IsInternalOrigin` lives in
the CEF-dependent `HttpRequestInterceptor.cpp`, so testing them means extracting them to a
header-only TU — the `JsStringEscape.h` / `PaymentCost.h` move. That is a production refactor of
the C2 gate, and doing it in the same session that changed the money path is exactly the kind of
rushed edit this phase exists to avoid. **Named here so it is visible, not quietly dropped.**

### 4r. Task 4 — THE WHOLE TABLE, re-run 2026-08-21 after the fix

⛔ The whole table, not the failing rows. Run against the **rebuilt** binaries (wallet rebuilt
09:06, C++ 09:19), because a fix measured against yesterday's DLL is the "right value, wrong
subject" failure this phase exists to prevent.

| Row | Result |
|---|---|
| `X1` | ✅ crafted `data:` frame resolved as **`example.com`**, both messages DENIED |
| `X2` | ✅ `🛡️ IPC DENIED: 'send_transaction' from external origin 'example.com'`; **0** `/transaction/send` lines in the wallet log |
| `X3` | ✅ `🛡️ IPC DENIED: 'get_balance' …` |
| `X4` | ✅ in preflight T1c — 222 tests, 221 passed, 1 skipped, 0 failed |
| `G1` | ✅ **closed today** — §4p, release-shaped, RED reproduced |
| `G2` | ✅ `hodosBrowser.identity` is `undefined` **with and without** the `:5137` substring in the page URL |
| `G3` | ✅ `preflight.ps1 -Full` → **PASS, nothing skipped**; `-NegativeControl` → **every one of the 5 gates seen to fail** on an injected violation |
| `E1` | ✅ `about:blank` child inherited `example.com` and was DENIED 19 ms after its parent |
| `R1` | 🟡 **half.** Header-free internal caller **unchanged** on both `/transaction/send` and `/processAction`: no gate line, straight to build. The owner-run wallet-UI send was **NOT** re-run — see below |
| `R2` | ✅ `202 … per_tx_limit` |
| `R3` | ✅ `202`, and the payload reads **`satoshis: 17,938,546 / cents: 319 / exceededLimit: "both"`** — **not** "0 sats". Finding 6 confirmed live |
| `X5` | ✅ `peerpay_send` and `paymail_send` both `202 … per_tx_limit` |
| `C1` | ✅ see the control set below |
| `R4` | ⬜ gold pill — needs the owner |
| `X6` | ✅ §4q |
| panel #2 1.3 / 1.4, Task 2 items 2 / 3 | ✅ all four `403 permission_table_is_first_party_only` |
| false-green control | ✅ an **unapproved** origin gets `domain_approval / new_domain_no_manifest` — `domain_trust_mw` answers first, which is why every payment row above uses an **approved** domain |

Six `🛡️ engine Prompt (payment)` lines for six external rows, **zero** for the two
internal ones. `R-INTEXT` holds in both directions.

**`C1` — measured where it actually lives, with three controls.** ⚠️ My first attempt measured
the wrong layer and I nearly recorded it: a page-context `fetch` to a wallet endpoint is caught by
the **C++ interceptor**, which forwarded it with `X-Requesting-Domain: example.com`, got Rust's 202,
and opened a **domain-approval modal** — so the handler did not run, but **CORS was never the
deciding layer**. Re-measured with curl, straight to actix:

| | `Origin` | Content-Type | Result | Handler ran? |
|---|---|---|---|---|
| SUBJECT | `https://evil.example` | `text/plain` (simple, no preflight) | `400 Origin is not allowed to make this request` | **0** |
| CONTROL A | `http://127.0.0.1:5137` | `text/plain` | `400 Invalid JSON request: Content type error` — a **different error at a different layer**, so CORS admitted it | 0 |
| CONTROL B | `http://127.0.0.1:31400` (one digit off) | `text/plain` | `400 Origin is not allowed…` | **0** |
| CONTROL C | `http://127.0.0.1:5137` | `application/json` | **`200`** | **1** |

Control C is what makes the subject row mean something: the harness **can** report a handler that
ran, and it reported **0** for the foreign origin.

#### ⛔ Two things this re-run got wrong before it got them right

1. **`X1`'s first probe measured NOTHING and looked green.** The frame's inline script began with
   `parent.__x1 = "ran"` — and a `data:` frame is **opaque-origin**, so that throws a SecurityError
   and kills the script before it reaches `cefMessage`. The iframe appended, no deny line appeared,
   and "no deny line" is indistinguishable from "gate held". Fixed by making the liveness signal
   itself the evidence: the frame sends `__x1_probe_marker`, a message name that exists nowhere, so
   it can only be DENIED — and the deny line prints the **resolved origin**, which is the quantity
   the row is about. Liveness independently confirmed via `postMessage` (`frameScriptRan: "yes"`).
2. **`/transaction/send` returns `500` for an invalid address where `/processAction` returns `400`.**
   My assertion was over-specific about the status code. CLAUDE.md #13: the property under test is
   "the internal caller is ungated", and it held — no gate line, reached address validation. The
   status-code inconsistency is **pre-existing** and untouched by this phase. **Test-only fix**;
   filed as cosmetic, not chased.

#### Still owed — these two need the owner, and neither can be faked

| Row | Why |
|---|---|
| `R1` (full) | A real send **from the wallet UI**, which must complete with **no modal**. Only the owner can drive the overlay; a curl with no header proves the Rust half but not that the UI still works end to end. |
| `R4` | The **gold pill** on the tab badge after a silent-approved payment. It is a visual artifact on a real tab. |

⚠️ A domain-approval modal for `example.com` was raised by the mis-aimed C1 probe at 09:37 and
**timed out unanswered after 45 s** (`⏱️ Wallet HTTP request timeout`). Nothing was approved.

### 4s. 🛑 PANEL #3 — **DO NOT SIGN OFF.** It broke my own work from today.

**Run 2026-08-21**, 14 lenses (8 macOS, 4 on this session's fixes, 2 regression/harness).
**44 raw findings.** ⚠️ **The run is INCOMPLETE**: 16 of 51 agents died on a session limit,
including **the synthesizer**, so 15 verification passes never ran and there is no synthesis.
Raw output preserved at `ADVERSARIAL_PANEL_3_2026-08-21.raw.json`. Per HARNESS §8 this is
**INCOMPLETE, not a pass** — the panel must be re-run to completion before sign-off.

#### ⛔⛔ First, a FALSE CLAIM OF MY OWN, corrected

§4q said the five synthetic-request sites were *"mechanically re-derived, not read —
`probes/ungateable.py`'s sibling check walks each `TestRequest…` site back to its enclosing `fn`
and looks for a `dispatch_*` call in between."*

**That sentence is FALSE in two ways, and the panel caught both.**

1. **The cited artifact does not contain the check.** `grep -rn TestRequest probes/` returns
   **zero** hits. The committed `ungateable.py` only enumerates routes from `main.rs`. I ran the
   sibling check as a throwaway inline script, never committed it, and then described the committed
   file as containing it. **This is the fourth false audit claim in this phase and the first one
   that is mine.** The rule I have been applying to `81c054c` and `4eacb51` applies to me.
2. **The check itself was too shallow to support the claim.** It asked only *"is there a
   `dispatch_*` call between `fn` entry and the synthetic request?"* It never asked whether that
   call was **unconditional**. For `pay_402` it is not — see below. So even had the script been
   committed, "all five clean" would still have been wrong.

#### 🚨 BLOCKER, **MEASURED** — `/wallet/pay402` inverts the fail-closed rule

Every other gated endpoint fails **closed** when the `X-Payment-*` headers are absent:
`PaymentCall::from_headers` returns `None`, `dispatch_payment` substitutes
`bsv_price_available=false`, and `matrix_c.rs` renders a PriceUnavailable prompt. `pay_402` is the
one handler that does the opposite:

```rust
let brc121_engine_headers_present = http_req.headers().contains_key("X-Payment-Satoshis")
                                 || http_req.headers().contains_key("X-User-Approved");
if brc121_engine_headers_present { dispatch_payment(...) }   // ⛔ absent headers => NO GATE
```

And `hodos::IsPaymentEndpoint` (`PaymentCost.h`) **does not list `/wallet/pay402`**, while the
external IPC arm stamps `X-Payment-*` only `if (isPaymentEndpoint(...))`. So the absence of the
endpoint from the list *guarantees* the headers are missing, which *guarantees* the gate is
skipped. `check_domain_approved` runs but enforces trust level only, no caps.

⇒ **an approved dApp mints and broadcasts an arbitrary-value BRC-121 payment with no cap, no rate
limit, no modal and no gold pill.** Same endpoint list, same "do both or neither" rule, same
failure I fixed for `/processAction` this morning — one row down.

#### 🟠 HIGH, **MEASURED** — today's `/processAction` fix is defeated by one encoded character

`/%70rocessAction` → `IsPaymentEndpoint=false` (compiled probe against `PaymentCost.h` at
`8946f1a`). actix routes the **decoded** path, so Rust still runs `process_action` and its gate,
but C++ never prices it ⇒ amount-blind `price_unavailable` prompt, **no gold pill**, and the
per-session dollar cap never advances. Fails closed on the money, open on the UX and the counters.
**This is panel #2's finding 1.3 (`/domain/%70ermissions`) in a new location** — I added a
substring match to a string list on the same day the contract records "gate SUBTREES, never a list
of exact strings."

#### Other MEASURED findings

| Sev | Finding |
|---|---|
| HIGH | `IsInternalOrigin` trusts **any** loopback port while `IsInternalFrontendUrl` requires `:5137` — the two predicates disagree and **the money path uses the looser one**. Known-open as Task 2 item 4, now compiled and measured rather than read. |
| MED | `hodos::IsWalletHostPort` is still an **unanchored substring search**: `IsWalletHostPort("https://evil.com/?q=127.0.0.1:31301") == 1`. The exact pattern `PortConfig.h`'s own banner forbids for the `:5137` gate, never applied to these two helpers. |
| MED | `PaymentCost.h`'s "do both or neither" rule is **already violated in-tree**: `/acquireCertificate` and `/sendMessage` are listed in `IsPaymentEndpoint` but have no body shape in `ExtractOutputSatoshis`. `payment_cost_test.cpp` asserts only the violating half, so the 221-green suite **cannot detect** the failure the header was extracted to prevent. |
| LOW | `OriginFromUrl`'s authority scan terminates only on `/ ? #`, so a backslash, TAB, LF or SPACE before the last `@` yields an internal origin. Latent — every caller today passes a Chromium-canonicalized spec. |

#### Unverified, and that matters

The three **blocker**-rated CODE_READING findings below never got a verification pass (their
refuters died on the limit). **Do not act on them, and do not dismiss them, until they are run:**
a page framing the internal wallet UI and driving its send form by forged `postMessage`;
the modal query string injected into the notification overlay as a JS string literal;
`createAction options.sendWith` broadcasting arbitrary local txids.

macOS produced 11 findings across 4 lenses — all **CODE_READING by construction**, since the panel
ran on Windows. They belong to the macOS session with named experiments attached.

#### Verdict

🛑 **DO NOT SIGN OFF Phase 0.5.** One MEASURED blocker on the money path, one MEASURED
high against a fix that landed today, one false claim of mine corrected, and an incomplete panel.

#### Still owed before sign-off

| | |
|---|---|
| Task 2 — six owner decisions | 🟡 **1, 2, 3, 6 CLOSED**; **5 MEASURED** — the lifecycle is clean, but the measurement found `/processAction` (§4o, `P0.5-X6`), a **blocker-class ungated fund-mover**, plus an ungated disclosure set. **Fix NOT written — owner decision owed.** **4 still owed** (Phase 5) |
| Task 3 — `P0.5-G1` on a release-shaped build | ✅ **CLOSED — §4p.** RED reproduced, GREEN, subject proven with a disk-only marker |
| Task 4 — whole evidence table re-run | 🟡 **DONE for everything drivable without the owner — §4r.** `R1` (full, wallet-UI send) and `R4` (gold pill) still owed |
| Task 4 — **panel #3** | 🛑 **RUN, INCOMPLETE, DO NOT SIGN OFF — §4s.** 44 findings; synthesizer + 15 verifiers died on a session limit. MEASURED blocker on `/wallet/pay402`; today's `/processAction` fix defeated by one encoded character; a FALSE claim in §4q corrected. **Re-run to completion.** |
| Concurrency hardening (latent) | ⬜ follow-up |

#### ⚠️ Unrelated finding surfaced during this session — NOT caused by these fixes

**⛔ CORRECTED 2026-08-21 — I OVERSTATED THIS. It self-heals; no money was lost.**
The balance came back: **38,362,835 → 16,586,118 → 38,341,860**. The residual 20,975 sats is
*exactly* the three on-chain wallet backups that ran in between (6994 + 6989 + 6992), so the
recovery is complete. The 429 → confirmed-only fallback causes a **TRANSIENT BALANCE
UNDER-REPORT that resolves on the next successful mempool read** — it does NOT destroy outputs.
`Marked 1 outputs as spent` is real but evidently reversible by the next sync.
Still worth a ticket (an under-reported balance can make a legitimate send fail with
"Insufficient funds", which I saw during the concurrency probes), but it is **NOT** the
money-loss event I first described.

The dev wallet's spendable balance fell **38,362,835 → 16,586,118 sats** during the
session, in windows where no wallet call was made. Mechanism visible in the log:
`addresses/unconfirmed/unspent` returned **429 Too Many Requests** → *"Mempool read
unavailable for this chunk — confirmed UTXOs only this tick"* → `Marked 1 outputs as
spent (spent_by=None)`. So a **rate-limited mempool read degrades to a confirmed-only
view, and reconciliation then marks real outputs spent.** This is the exact failure
the UTXO memory warns about (*discovery = union, reconciliation = agreement*).
Whether the pre-drop figure was an overstatement being corrected, or real money being
written off, is **NOT established** — it needs its own investigation. My probes are
exonerated: the sequential run began and ended at 38,362,835 with zero movement, and
every probe died at address-checksum or was refused 202/403.

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

1. ~~**Finding 4** (§4e) — allowlist, revert, or accept broken interop?~~ ✅ **MADE 2026-08-19: allowlist `127.0.0.1:31301`/`31401` + strip page-supplied trust headers, keep `block_on_origin_mismatch(true)`.** Landed `d0ee6db`, evidenced §4j. ⚠️ The strip needed **two entries the agreed sketch missed** — `X-Bsv-Price-Available` (lives under the deliberately-forwarded `x-bsv-` prefix, so it needs an EXACT strip) and `X-Cert-Approved-Fields`.
2. ~~🚨 **NEW, BLOCKING — §4k permission-table escalation.** An approved dApp can `POST /domain/permissions` for ANY domain with ANY caps and `identityKeyDisclosureAllowed:true`, silently. Measured from a real third-party dApp. Fix in beta.3 or accept for the tester base? `set_domain_permission` has no gate of its own; the cheapest close is to refuse the write when the request carries an `X-Requesting-Domain` at all (i.e. dApp-originated), which needs no schema change.~~ ✅ **MADE 2026-08-20: fix in beta.3.** Landed `81c054c`, evidenced §4k, including an owner-clicked approval control proving the first-party path still writes.
3. `P0.5-R3` — design around the **202 decision** (no funds needed) or prove the full broadcast?
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
| repair re-run, all rows + REDs (§4j) | **GREEN, every RED run in-session** | 2026-08-20 | assistant |
| §4k permission-table escalation | ✅ **FIXED + GREEN with RED** — `81c054c`; owner-clicked approval control passed | 2026-08-20 | assistant + owner |
| foreign-bridge interop (§4m, out of scope) | **GREEN with RED** — real dApp connects, `9b73bd7` | 2026-08-20 | assistant |
| adversarial review (post-repair) | | | |

### 4t. Panel #3 verification + two money-path fixes (2026-08-21, session 2) — commit `7a35b1c`

The session that recorded §4s handed off two MEASURED defects and three unverified blockers.
This session fixed the two, verified the three (all now MEASURED, not code-read), measured the
"run-first" self-nav item, and corrected one false claim in the handoff prompt itself.

#### Task 1 — `/wallet/pay402` gate inversion — FIXED, RED+GREEN both mine

- **RED (pre-fix binary, 13:18):** approved domain `teragun.com`, no `X-Payment-*` -> **no gate
  line**, straight to BRC-42 derivation. Paired control 68 ms later WITH the headers -> `202
  per_tx_limit`. The gate worked; it was being skipped.
- **GREEN (rebuilt, 13:33):** same call -> `202`; control A (headers) unchanged; control B
  (internal, no `X-Requesting-Domain`) unchanged at 500, no gate line (`R-INTEXT` holds).
- Fix: removed the `brc121_engine_headers_present` conditional (a dead rollout gate); added
  `/wallet/pay402` to `IsPaymentEndpoint` AND taught `ExtractOutputSatoshis` the fifth body
  shape (top-level `{satoshis}`) — do-both-or-neither. Page-driven confirmation from a real
  teragun.com tab: `endpoint=/wallet/pay402 reason=per_tx_limit` (the discriminator proving the
  C++ half priced it).

#### Task 2 — `/%70rocessAction` encoded-path desync — FIXED as a PATTERN

- New `hodos::RequestPathForMatching` (PortConfig.h): cut query/fragment, then percent-decode
  ONCE (matching actix-router). Called INSIDE `IsPaymentEndpoint` and `isWalletEndpoint`.
- `isWalletEndpoint` was worse than the panel said: it missed the encoded path too, so the
  request was not intercepted AT ALL and reached the wallet with no `X-Requesting-Domain` (read
  as first-party). Cutting the query also retired a pre-existing false positive:
  `/wallet/status?next=/createAction` was being priced as a payment.
- Page-driven GREEN: `POST /%70rocessAction` (raw in the actix log) -> `endpoint=/processAction
  reason=per_tx_limit`, identical to the plain-path control in the same run.
- **NEGATIVE CONTROL:** 12 new tests built + run against the UNFIXED headers first -> **7 failed**;
  restored -> **232 passed / 1 skipped / 0 failed**.

#### CORRECTION — the handoff prompt's Task 2 premise was FALSE (fifth false audit claim)

The prompt said `is_permission_surface` in `main.rs` "has the identical shape and the identical
hole." It has NEITHER. Two of its four arms are `starts_with` subtrees and the predicate runs
over BOTH raw and decoded paths. **MEASURED:** `POST /wallet/%73ettings` from an approved dApp
-> **403**; the same encoded path with no domain header -> **400** (handler ran), proving actix
routes it and the gate is not vacuous. No change made there.

#### Task 3 — the three unverified blockers, now MEASURED

1. **Modal query-string JS injection at 127.0.0.1:5137 — CONFIRMED, MEASURED (both legs).**
   dApp-controlled `basket` from `/listOutputs` flows verbatim into a `showNotification('...')`
   literal that escapes only `'` (not backslash). Sent a `backup-`-prefixed basket (forces a 202
   via `is_protected_basket`, so the payload reaches the overlay deterministically) carrying a
   backslash-quote breakout. First payload was a SyntaxError (missing the `}` that closes the
   template `if`-block — the finding's own example includes it); corrected, the injected code
   ran. **Leg A (fetch):** the injected `fetch(/wallet/status?<mk>)` produced a domain-trust
   prompt for `domain=127.0.0.1:5137 endpoint=/wallet/status` — a request no page made.
   **Leg B (wallet_call, the escalation):** the injected `cefMessage.send('wallet_call', [...,
   '/wallet/status?<mk>', ...])` reached the wallet as `GET /wallet/status?<mk>` **200, no
   prompt, no gate** — internal-origin direct dispatch. `escapeJsonForJs` (escapes backslash)
   exists in-tree and is the correct encoder; it is unused here. **FIX OWED.**

2. **`createAction options.sendWith` — CONFIRMED. Gate-blindness MEASURED; broadcast path read.**
   MEASURED: `outputs:[{satoshis:1}] + sendWith:[bogus]` from approved teragun -> **no modal**
   (silent auto-approve; reached address conversion), while `outputs:[{satoshis:5000000}]` no
   sendWith -> `202 per_tx_limit`. The gate prices only `outputs[].satoshis`, blind to sendWith.
   Broadcast path read-verified: `parent_transactions.get_by_txid` has NO status filter;
   `get_local_parent_tx` filters only `status != 'completed'` — neither checks nosend/ownership/
   aborted, unlike `broadcast_nosend`. A real withheld tx was NOT broadcast (that is the harm).
   **FIX OWED.**

3. **Cross-origin iframe of the wallet UI + forged postMessage — CONFIRMED, all four legs
   (three MEASURED on a production-layout build).** Staged `frontend/` next to the exe (a
   disk-only marker proved `LocalFileResourceRequestHandler` was live), then from a teragun.com
   tab:
   - **SERVE:** `<iframe src=127.0.0.1:5137/wallet-panel>` loaded a cross-origin document
     (`contentWindow.document` threw SecurityError; `onload` fired) — no `X-Frame-Options`.
   - **TRUST:** eval inside the framed OOPIF (`parentIsCrossOrigin: X-ORIGIN`) -> it holds
     `hodosBrowser` (identity/wallet/address/history...), `cefMessage`, `__hodos_walletCall`.
   - **DRIVE:** `iframe.contentWindow.postMessage({type:'qr_scan_result',...})` from teragun
     prefilled the frame's send-form input with the attacker address `1BitcoinEater...`.
   - **SPEND:** one clickjacked click (unautomatable; the form is armed).
   Root causes read-verified: `GetResourceRequestHandler` dispatches on `IsInternalFrontendUrl`
   only (`request_initiator` unused); `LocalFileResourceHandler.h` sets no framing headers;
   `isInternalPage` has no `frame->IsMain()` guard; `WalletPanel.tsx` message handler has no
   `event.origin` check (a fix must still allow `origin===''` — the legit C++ QR path posts with
   an empty origin). **FIX OWED.**

#### The "run-first" item — internal-UI self-navigation — MEASURED

A page navigated its own tab to `http://127.0.0.1:5137/brc100-auth?type=domain_approval&domain=
evil-attacker-p3.com`. It rendered the REAL connect prompt for the attacker-named domain
(internal origin, holds the wallet bridge). Clicking Allow -> `POST /domain/permissions
domain=evil-attacker-p3.com` **200** (first-party, ungated) and a live `approved` grant appeared
in `domain_permissions`. **Grant deleted afterward; DB clean.** Reduces to one deceptive click on
a genuine-looking prompt for a domain the attacker fully controls. **FIX OWED.**

#### Other MEASURED (panel, MED/LOW) — confirmed, not blockers

- `IsPaymentEndpoint` lists `/acquireCertificate` and `/sendMessage` but `ExtractOutputSatoshis`
  has no body shape for either -> priced 0 -> silent under a live price. Confirmed by reading.
  Fix direction (price vs delist) depends on whether those endpoints should be metered — **owner
  decision owed (CLAUDE.md #13); not changed.** `IsInternalOrigin` loopback-port breadth,
  `IsWalletHostPort` unanchored substring, and `OriginFromUrl` authority-scan were verified by
  the panel's compiled probes and are read-consistent with the source; not re-measured here.

#### Stranding incident — caused, verified, released, balance whole

A user-approved 5,000,000-sat probe prompt drove the `X-User-Approved` replay through
`create_action_internal`, which RESERVED the 5M input (placeholder `pending-1787341796717`,
`spendable=0`) and then died at the broken-checksum probe address — Phase 0.7 strand. Balance
fell 38,775,868 -> 33,775,868 (stable across reads, no 429). **Lesson: the "address->script is
after the gate" safety model holds for a GATE bypass, but a user-APPROVED probe reserves UTXOs
BEFORE the checksum guard fires, so it strands.** Verified `d03d8e9af606...:0` unspent on-chain
(WoC spend-query 404, 283 confs), released via the wallet's own `restore_by_spending_description`
SQL with the wallet stopped (WAL-safe), restarted -> balance back to **38,775,868**. Zero
stranded remaining. Production (31301) never touched.

#### Verdict — still DO NOT SIGN OFF

Two money-path defects fixed and committed. Three blockers + the self-nav item are now MEASURED
and **FIX OWED** — none can sign off Phase 0.5. Panel #3 must still be re-run to completion.

### 4u. Panel #3 blockers — ALL FOUR FIXED, each with a negative control (2026-08-21, session 2)

Owner directed "fix the 4 blockers first." Done, one commit each, isolated and reversible.

| # | Blocker | Commit | Negative control |
|---|---|---|---|
| 1 | Modal query-string JS injection at 127.0.0.1:5137 | `99cd651` | Same wallet_call-injection probe that produced `GET /wallet/status?WCINJ 200` now yields **0** injected calls; the 202 basket prompt still fires (feature intact). |
| 2 | `createAction sendWith` broadcasts arbitrary local txids | `7d06d68` | Unit test `sendwith_only_broadcasts_nosend_status`: stub the predicate to the pre-fix behaviour (`status != 'completed'`) → **RED** on "status 'failed' must be refused"; restored → passes. |
| 3 | Cross-origin iframe of the wallet UI | `4b66183` | Production-layout build: teragun.com framing `127.0.0.1:5137/wallet-panel` now gives a `chrome-error://` "refused to connect" frame with **no** `__hodos_walletCall` bridge — vs the RED where the frame committed the wallet UI with the full bridge + a prefilled attacker recipient. |
| 4 | Internal-UI self-navigation writes an attacker-named grant | `ee8f836` | Dev: self-nav a tab to `brc100-auth?domain=evil-selfnav-p3b.com` + Allow → **REFUSED** `from role 'tab_1'`, zero `POST /domain/permissions`, no DB row — vs the RED (`domain=evil-attacker-p3.com` wrote a grant). **Positive control:** the same IPC from the notification overlay (role `notification`) still writes the grant — real approval flow intact. |

#### The fixes

1. **Modal injection.** `buildExtraParamsFromPayload` now `urlEncode()`s every
   dApp-controlled value (basket, protocol*, counterparty, kind, verifier, keyID,
   protocol JSON, exceededLimit) and `openCertificateDisclosureModal` encodes each
   `fields` entry — matching the certType/certifier already encoded there. Defence
   in depth: `CreateNotificationOverlay` on BOTH platforms now `escapeJsonForJs()`
   the whole query instead of the hand-rolled `'`-only loop. React's URLSearchParams
   decodes for display — no regression (verified: `applyParams` uses URLSearchParams).

2. **sendWith.** Each sendWith txid is gated on `status == 'nosend'` before
   broadcast (via `sendwith_status_is_broadcastable`), mirroring `broadcast_nosend`.
   The legit flow (a prior `noSend=true` tx at status=nosend, then a second
   createAction sendWith'ing it) still passes; failed/aborted/completed/etc. are
   refused. A live probe was deliberately not run — reaching the sendWith block
   requires the main createAction to broadcast a real tx, and the RED requires
   broadcasting a real `failed` tx (= the double-spend harm). **Still owed** (not
   this fix): pricing the sendWith amount, and scoping to txids the requesting
   domain owns — the latter needs a domain column on `transactions` (schema,
   invariant #2), owner decision owed.

3. **Cross-origin iframe.** `LocalFileResourceRequestHandler` now emits
   `X-Frame-Options: SAMEORIGIN` + CSP `frame-ancestors 'self'` on every internal
   response, so Chromium refuses to commit the wallet UI in any cross-origin
   frame — for EVERY internal page, not just WalletPanel. Defence in depth:
   `WalletPanel.tsx`'s message handler drops any event whose origin is neither ''
   (the empty origin the C++ QR path uses — verified at
   simple_render_process_handler.cpp:903) nor `window.location.origin`. **Residual
   (minor):** `hodosBrowser`/`cefMessage` still appear on the blocked
   `chrome-error://` frame, but it is a non-scriptable cross-origin error page with
   no bridge — inert. Tightening `isInternalPage` to `frame->IsMain()` is a sensible
   follow-up but not a live surface once framing is blocked.

4. **Self-nav grant.** `add_domain_permission` and `add_domain_permission_advanced`
   now refuse any sender whose `role_` is not `notification`/`brc100auth`. These
   IPCs come solely from BRC100AuthOverlayRoot's Allow, which runs in those overlay
   roles; a self-navigated tab is `tab_<id>` and is refused.

#### Environment

Every fix built, negative-controlled, and committed with the dev stack killed by
exe path only. Two production-layout cycles (staged `frontend/` next to the exe,
removed afterward — Vite mode restored, disk marker gone). C++ suite **232 passed /
1 skipped**. Balance held at **38,775,868** throughout (the only movement all
session was one scheduled 7,160-sat on-chain backup); the single stranded 5M UTXO
from a user-approved probe was released (§4t). Production (31301) never driven.

#### Still owed before Phase 0.5 sign-off

- Panel #3 **re-run to completion** (it died on a session limit — HARNESS §8: incomplete ≠ pass).
- The §4o disclosure set + `is_permission_surface`-as-subtree → **Phase 5** (owner-deferred).
- sendWith pricing + domain-ownership scoping (needs schema) — **owner decision owed**.
- `/acquireCertificate` + `/sendMessage` do-both-or-neither (price vs delist) — **owner decision owed**.
- The macOS CODE_READING findings (11) belong to the macOS session with named experiments.

### 4v. Panel #3 RE-RUN — COMPLETE, 2 findings, both FIXED (2026-08-21, session 2) — commit `f033f75`

Panel #3 died on a session limit (synthesizer + 15 verifiers never ran) — HARNESS §8: incomplete ≠
pass. Re-run as an 8-lens adversarial workflow (find → adversarial-verify → synthesize) against the
FIXED code, explicitly panelling this session's own six fixes. **This run completed cleanly: 11
agents, 0 errored, 0 empty.** Raw at `ADVERSARIAL_PANEL_RERUN_2026-08-21.json`.

13 raw findings (2 high, 3 med, 7 low, 1 none). Two survived adversarial verification as CONFIRMED;
both now fixed with negative controls. Both were reachable-in-principle from an approved dApp, and
one was a bug in this session's own normalizer fix — the panel-your-own-fresh-work rule paying off.

#### Finding 1 [HIGH] — `/wallet/debug/broadcast-nosend` is an ungated fund-mover

The sendWith fix (`7d06d68`) hardened the `broadcast_nosend` sibling and commented "the two sibling
broadcast paths cannot diverge again" — but a THIRD sibling, `debug_broadcast_nosend`, broadcasts
ANY status. Its own comment claims to "Verify the transaction ... is in nosend status"; the code read
the status, **logged it, and never checked it.** Registered unconditionally (no HODOS_DEV gate) and
absent from `is_permission_surface`. It reverses `mark_failed` cleanup, so a `failed` tx (inputs
already restored spendable) is resurrected into a double-spend.
- **RED (MEASURED):** approved dApp `POST /wallet/debug/broadcast-nosend` (bogus txid) → **404**
  (handler ran, no gate); `/wallet/settings` from the same origin → 403.
- **Fix, two layers:** `is_permission_surface` gains a `/wallet/debug` **subtree** arm (closes all
  three debug endpoints + any future one); `debug_broadcast_nosend` now enforces `status=='nosend'`
  via `sendwith_status_is_broadcastable` (matches its own comment + the two siblings).
- **GREEN (MEASURED):** approved dApp → all three `/wallet/debug/*` now **403**; first-party (no
  `X-Requesting-Domain`) still reaches the handler (404 bogus — dev tooling intact); first-party + a
  real `failed` txid → `{skipped, "status is 'failed', not 'nosend'"}` **200, NOT broadcast**.
  Proved without broadcasting the failed tx; balance held 38,775,868.

#### Finding 2 [MED, my own fix] — `RequestPathForMatching` read `://` over the whole target first

The comment said "cut query FIRST" but the code ran `find("://")` over the raw target before the
query-cut. `/createAction?z=a://b` matched the `://` inside the query, treated it as an authority,
found no path slash, returned "" → `IsPaymentEndpoint("")==false` → **gold pill did not fire and the
per-session cap did not advance**, while actix (which drops the query) still routed `/createAction`.
Rust fail-closes to a `price_unavailable` prompt (so not silent — hence MED), but the primary
anti-silent-payment safeguard was stripped.
- **Fix:** cut query/fragment FIRST over the whole target, then scan for the scheme only within
  `[0, end)`.
- **NEGATIVE CONTROL:** 2 new unit tests (`QueryEmbeddedSchemeDoesNotHidePaymentEndpoint`,
  `QuerySchemeIsCutBeforeSchemeDetection`) RED against the unfixed header, GREEN after; full C++
  suite **234 passed / 1 skipped**.

#### Refuted by the panel — do NOT reopen

- **Fix #5 EditPermissionsForm HTTP-write "bypass": REFUTED (low).** Consummation needs the real
  first-party Hodos SPA running after navigation (the attacker JS context is destroyed) AND a human
  Save click on a card naming the target domain; clickjacking is closed by fix #4's
  `X-Frame-Options: SAMEORIGIN` + CSP `frame-ancestors 'self'`. No silent escalation; does not reopen §4k.
- **Fix #2 modal query-string injection: VERIFIED CORRECT** on both platforms.

#### Recorded for hardening (not blocking)

- **LOW latent:** `PaymentCall::from_headers` (request_gate.rs) fails OPEN on a partial `X-Payment-*`
  set — `X-Payment-Satoshis` present with `X-Bsv-Price-Available` missing yields
  `{cents:0, price_available:true}` → Silent. **Not page-reachable today** (all three C++ stamp sites
  emit together; the Open-path denylist strips page copies). Hardening: make a missing
  `X-Bsv-Price-Available` default to `false`, or require all three together.
- **macOS parity** items from the macos-parity lens belong to the mac session (CODE_READING here).

#### Where Phase 0.5 stands after the re-run

The panel is now **COMPLETE** and its two CONFIRMED findings are **FIXED and measured**. What remains
before sign-off is owner-gated, not blocker-open: sendWith **pricing** + domain-ownership scoping
(needs a `transactions` domain column, schema), `/acquireCertificate` + `/sendMessage`
do-both-or-neither (price vs delist), the §4o disclosure set + `is_permission_surface`-as-subtree
(Phase 5), and the macOS session's parity work incl. the `wallet_call` CRLF-method SSRF. A final
confirmation panel over `f033f75` (the two fresh fixes) is a reasonable belt-and-suspenders before
sign-off but is optional — each fix carries its own measured/tested negative control.

### 4w. Confirmation panel over `f033f75` — 2 fixes CLEAN, 1 adjacent gap FIXED (`3d5e0f7`)

Focused confirmation panel over the two f033f75 fixes (complete: 6 agents, 0 err). Raw in the workflow
record. Verdict: **both fixes are internally SOUND and regression-free** — FIX A's `/wallet/debug`
subtree arm has no bypass (checks both raw+decoded, first-party early-returns) and no over-match; FIX B's
`RequestPathForMatching` reorder correctly handles every regression case (real absolute URLs, fragments,
double-encoding) with no legit endpoint stopping to match.

**But the completeness critic caught an adjacent gap:** FIX A's own reasoning (a `(state, _body)` handler
with no `HttpRequest` can't self-gate, and a dApp has no business calling a fund-mover) applies to
SIBLINGS the fix left open — a whole class, not just `/wallet/debug`. Systematic sweep found the
dApp-reachable, no-`HttpRequest`, fund-moving/destructive set absent from `is_permission_surface`:
`wallet_delete` (HIGH — deletes the wallet), `wallet_consolidate_dust` + `wallet_backup_onchain` (MED —
unprompted broadcast/fee-burn), `broadcast_nosend` (LOW). The sibling `wallet_export` DOES take
`HttpRequest` and rejects `X-Requesting-Domain` — proof these were omissions. (`sign_action` /
`internalize_action` are BRC-100 dApp-facing by design and gated at their flow entry / inbound-only;
`wallet_recover_external` needs attacker-supplied external keys — not an exfil of user funds.)

**Fixed (`3d5e0f7`):** five SUBTREE arms in `is_permission_surface` — `/wallet/delete`,
`/wallet/consolidate-dust`, `/wallet/backup`, `/wallet/recover`, `/wallet/broadcast-nosend`. Every
internal/scheduled caller is header-free, verified: `task_consolidate_dust::run_inner` and
`wallet_delete`'s `do_onchain_backup` are direct fn calls; `task_backup`'s `POST /wallet/backup/onchain`
and the BRC-121 `BroadcastTask`'s `POST /wallet/broadcast-nosend` both send Content-Type only — so
first-party is untouched.

**MEASURED.** RED (pre-arms binary, approved dApp, safe endpoints only): `/wallet/broadcast-nosend` → 404
(handler ran), `/wallet/recover-external` → 400 (deserialize ran) — not gated. GREEN (post-arms): all five
→ **403** from an approved dApp (gate fires before the handler, so firing `/wallet/delete` was safe).
POSITIVE CONTROLS: first-party (no header) still reaches every handler (404/400); dApp-legit endpoints
unaffected (`/wallet/status` 200, `/wallet/pay402` 202 — still a payment gate). Destructive endpoints
NOT fired pre-fix; their RED is the same `is_permission_surface` mechanism already measured on
`/wallet/debug`. Balance held (movement all session = two scheduled 3-hourly backups only).

⚠️ **The systematic `is_permission_surface`-as-subtree audit is STILL a Phase 5 item** — this closed the
concrete dApp-reachable fund-mover class the panel named, but the owner-deferred structural move (a
default-deny subtree in `domain_trust_mw` rather than an ever-growing arm list) remains owed.
