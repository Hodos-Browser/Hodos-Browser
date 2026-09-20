# 🚨 An approved dApp reaches 30 internal wallet routes that cannot gate themselves, because exposure is a deny-list

**Found:** 2026-09-19, by reading the request chain end to end while assessing BRC-179 (merged 2026-09-18) against our wallet.
**Status:** ⬜ UNASSIGNED · **Track:** unassigned — ⭐ suggest **track 1**, same fail-closed argument applied to methods instead of outputs · **Filed by:** Claude, at the owner's request

> ⚠️ **Method note.** Everything below is **code reading**. I read every link in the chain —
> the V8 injection site, the bridge script, the C++ IPC arm, the forward worker, the Rust
> middleware, and the individual handlers — and quote the load-bearing lines. ⛔ **Nothing here
> was executed.** The owner was using the app, so no request was issued and no behaviour was
> observed. The route/handler inventory is **measured** (a script over `main.rs` and
> `handlers.rs`, reproduced in "How exposed are we"). **The claim that any specific call
> succeeds is an inference from reading, and the RED below is what would settle it.**

---

## What happens

An external https page that the user has approved once can call **any wallet HTTP path it names**,
including internal first-party routes that were never meant to leave the wallet UI. The endpoint
string travels from page JavaScript to the Rust router without ever being checked against a list of
what is allowed to be called.

The chain, in order:

**1. The bridge is injected into external pages.**
`cef-native/src/handlers/simple_render_process_handler.cpp` — external, main-frame, https pages
receive `WALLET_CALL_BRIDGE_SCRIPT`, then `CWI_SHIM_SCRIPT`.

**2. The bridge takes an arbitrary endpoint from the page.**
`cef-native/include/core/CWIShimScript.h :: WALLET_CALL_BRIDGE_SCRIPT`:

```js
window.__hodos_walletCall = function(method, endpoint, body, httpMethod) {
    if (typeof method !== 'string' || typeof endpoint !== 'string') {
        return Promise.reject(new Error('[Hodos] wallet_call: method and endpoint must be strings'));
    }
```

The only check on `endpoint` is that it is a string. ⚠️ And the bridge is not the floor: the raw
`cefMessage.send('wallet_call', [...])` underneath it is present on **every frame**, which the
P0.5-E1 comment in `simple_handler.cpp` already records — *"the about:blank child does NOT get
`__hodos_walletCall` … but it DOES get `cefMessage`, the raw IPC underneath. Fixing the bridge would
not have closed this."*

**3. The C++ arm forwards it verbatim.**
`cef-native/src/handlers/simple_handler.cpp`, the `wallet_call` arm: `endpoint` is
`args->GetString(2)`, passed to `HandleIpcWalletCall` with no inspection.
`cef-native/src/core/HttpRequestInterceptor.cpp :: runIpcEngineCascade` then states its own design:

> *"Phase 2.6-G — C++ is a thin proxy. Domain-trust runs as a Rust middleware and the per-handler
> kind gates (payment/scoped/cert/privacy) all run in Rust, so EVERY external IPC call forwards to
> Rust unconditionally"*

…and builds the request as `std::string url = hodos::WalletBaseUrl() + endpoint;` with
`headers["X-Requesting-Domain"] = origin`.

⭐ **Note what this bypasses.** `HttpRequestInterceptor :: isWalletEndpoint` — the substring list of
BRC-100 paths — belongs to the *resource-interceptor* path, not this one. The IPC path never
consults it. So `isWalletEndpoint` is not the exposure control it looks like.

**4. Rust gates the domain, not the method.**
`rust-wallet/src/main.rs :: domain_trust_mw` is wrapped at App level, so it sees every route. For an
external origin it applies exactly two things:

- `is_permission_surface(path)` — a **deny-list**: `/domain/`, `/wallet/session*`,
  `/wallet/reveal-mnemonic`, `/wallet/settings`, `/wallet/debug*`, `/wallet/delete*`,
  `/wallet/consolidate-dust*`, `/wallet/backup*`, `/wallet/recover*`, `/wallet/broadcast-nosend*`
  — and only when `is_mutation`, which is `POST || DELETE`.
- `permission_service::request_gate.rs :: domain_trust_gate`, whose first substantive line is
  `if trust == "approved" { return GateOutcome::Proceed; }`.

`Proceed` falls through to `next.call(req)` and the handler runs. The per-kind dispatch
(payment / scoped / cert / privacy) lives **inside individual handlers**, so a handler that has no
such dispatch has no second gate.

⇒ **The exposed set is "every route, minus ten hand-written prefixes, and only for POST/DELETE."**

## Why it matters

**One approval grants the whole internal surface.** The product's claim is per-domain, per-scope
permissioning; the connect modal asks about baskets, protocols and certificates. None of that
governs these routes. A site the user approved to, say, sign in, can also read the wallet's full
address list.

Three consequences, in the order I would rank them:

1. **Privacy — the whole point of the permission engine is bypassed for wallet-level reads.**
   `GET /wallet/addresses` (`handlers.rs :: get_all_addresses`) returns every address the wallet
   owns, which links them to each other for anyone doing chain analysis. `GET /wallet/balance`
   returns the total. `GET /wallet/activity` returns history. `GET /wallet/tokens` returns holdings.
   None of these is a BRC-100 method and none consults a basket or protocol grant.
2. **Availability — `POST /shutdown`** (`handlers.rs :: shutdown`) is three lines and calls
   `data.shutdown.cancel()`. There is no legitimate dApp use for it at all.
3. **Disclosure — `GET /wallet/settings`** returns the user's display name and every global default:
   per-tx, per-session and rate limits, `default_max_tx_per_session`,
   `default_identity_key_disclosure_allowed`, prefill and quiet-mode defaults. ⭐ The **write** to
   this path is already gated first-party-only, for exactly the escalation reason recorded in
   `main.rs`; the gate is `POST || DELETE`, so **the read of the same values is not covered**. A site
   learns precisely what caps it is operating under.

⚠️ **`POST /wallet/unlock` — narrower than it first looks, and worth stating carefully.**
`handlers.rs :: wallet_unlock` accepts a 4-digit PIN and has **no attempt counter, no lockout and no
delay** (the `rate_limit_per_min` fields in `handlers.rs` are the domain payment rate limit, not a
PIN control). That is a 10,000-guess space. **But** the handler returns `409 Conflict` when
`db.is_unlocked()`, and the `reveal-mnemonic` comment in `main.rs` records that DPAPI/Keychain
auto-unlock at startup means the wallet is normally already unlocked. ⇒ The brute-force window is a
**locked** wallet — a user who declined the credential store, or the drifted-credential case that
`wallet_unlock`'s own comment describes. Real, conditional, and it should not depend on that.

## How exposed are we — answer this first

**Measured**, by mapping `main.rs` routes to handler signatures in `handlers.rs`: of 105 routed
handlers, **80 are not refused by the first-party gate**, and **30 of those take no `HttpRequest`
parameter**, so they structurally cannot gate themselves even if someone wanted them to.

| If | Then |
|---|---|
| The user has **never approved** the site | Not exposed. `domain_trust_gate` returns a 202 connect prompt or 403 for unknown/blocked trust before any handler runs |
| The user approved the site **once**, for anything | Exposed to all 30. Approval is domain-scoped and carries no method scope |
| The call is **POST/DELETE on one of the ten deny-listed prefixes** | Refused, 403 `permission_table_is_first_party_only`. This part works and is well tested |
| The call is a **GET on a deny-listed prefix** | Not refused — the gate is POST/DELETE only. `GET /wallet/settings` is the live instance |
| The page uses a **subframe** | Same, via raw `cefMessage`. The main-frame restriction is on the bridge, not on the IPC |

⛔ **What I did not verify:** that any of these calls actually returns 200 in a running build. The
whole finding is a reading of the chain. It is possible something downstream refuses that I did not
find — the RED below is deliberately the cheapest way to settle it.

**The 30, for the fix to work against:**

`POST /shutdown` · `POST /admin/prepare-unpublish` · `GET /wallet/activity` ·
`POST /wallet/address-to-script` · `GET /wallet/address/current` · `POST /wallet/address/generate` ·
`GET /wallet/addresses` · `GET /wallet/balance` · `GET /wallet/bsv-price` ·
`POST /wallet/certificate/cleanup` · `POST /wallet/certificate/publish` ·
`POST /wallet/certificate/unpublish` · `POST /wallet/cleanup` · `POST /wallet/create` ·
`GET /wallet/paymail/resolve` · `POST /wallet/peerpay/check` · `POST /wallet/peerpay/dismiss` ·
`POST /wallet/peerpay/outbox-retry` · `GET /wallet/peerpay/status` · `GET /wallet/recipient/resolve` ·
`GET /wallet/recipient/suggest` · `POST /wallet/release-nosend` · `POST /wallet/rescan` ·
`GET /wallet/settings` · `GET /wallet/status` · `POST /wallet/sync` · `GET /wallet/sync-status` ·
`POST /wallet/sync-status/seen` · `GET /wallet/tokens` · `POST /wallet/unlock`

## What already protects us, and how that shapes the fix

⭐ **This is a gap to close, not a system to build.** Four things already work:

1. **Domain trust is real and it is Rust-authoritative.** Unknown and blocked origins never reach a
   handler. The exposure needs a prior user approval.
2. **`domain_trust_mw` is already wrapped at App level**, so a single middleware sees every route.
   The place to put an allowlist already exists and is already the sole chokepoint.
3. **The path-normalisation work is done and was hard-won.** `RequestPathForMatching`,
   `percent_decode_str`, and the raw-vs-decoded double match all exist because of the
   `/domain/%70ermissions` bypass measured on 2026-08-20. An allowlist inherits all of it.
4. **The deny-list itself is correct for what it covers**, and its four build-out rounds are
   documented in place.

⛔ **And that history is the argument for the fix.** The comments in `main.rs` record the deny-list
being extended four times, each round finding the previous round's omissions — *"the completeness
critic found the `/wallet/debug` fix left its own siblings open."* A deny-list is only as good as the
last person who remembered. Every route added from here is exposed by default.

## Proposed fix

**The floor — an allowlist in the middleware. Small, and it ships alone.**

In `domain_trust_mw`, replace the deny-list question *"is this path first-party only?"* with
*"is this path in the set an external origin may call?"* and refuse everything else with the
existing 403 envelope. The allowlist is the BRC-100 method surface plus whatever the CWI/yours/panda
shim legitimately needs — derived from `CWIShimScript.h`'s method table and the BRC-100 route list,
not invented.

- Applies to **all verbs**, which closes the `GET /wallet/settings` class by construction.
- Keep `is_permission_surface` as-is underneath it. Defence in depth, and its tests stay green.
- Keep the 403 body string `permission_table_is_first_party_only` for paths it already refuses — the
  §4k and panel-#2 evidence rows assert on that discriminator.

**The system — BRC-179, if we want it.** A declared manifest with tiers, a canonical fingerprint and
a build pin. ⭐ The floor above *is* BRC-179's decision rule (`DENY_UNKNOWN`, fail-closed) without its
machinery. Adopting the fingerprint buys auditability and a provenance seal, and nothing else that
this ticket needs. ⛔ Separate decision, separate release, and BRC-179 is implemented by nobody —
checked 2026-09-19 against `ts-stack`, `go-wallet-toolbox`, `wallet-toolbox` and `py-wallet-toolbox`
at that day's heads, zero hits.

**Deliberately out of scope:** rate-limiting or locking out `POST /wallet/unlock` (a real defect,
but its own ticket); removing `/shutdown` and `/admin/prepare-unpublish` from the HTTP surface
entirely (tempting, and it is a different argument about whether they should be routes at all);
reworking what the CWI shim exposes; anything about BRC-179's fingerprint.

## Test and negative control

⛔ Per `../../0.4.0-beta.3/HARNESS.md`: **a fix is not done until the check has been *seen* to fail.**

| | |
|---|---|
| **GREEN** | From an **approved** dApp origin, `window.__hodos_walletCall('x','/wallet/addresses',{},'GET')` and the same for `/shutdown`, `/wallet/settings`, `/wallet/balance` each return **403** and the handler does not run — no `📋 GET /wallet/addresses called` line in `debug_output-<pid>.log`, and the wallet process is still alive after the `/shutdown` attempt. The BRC-100 surface is unaffected: `getPublicKey`, `createAction`, `listOutputs` behave exactly as before |
| **RED** | ⛔ **Run this first, before writing the fix — it is also what settles the finding.** Same calls from the same approved origin on **today's** binary. Expected: 200 with the address list, 200 with the settings object, and the backend gone. 📏 If any of them refuses, the reading above is wrong somewhere and this ticket needs correcting before it needs fixing |
| **SUBJECT** | The **handler's own log line** in `debug_output-<pid>.log` — proof the request reached Rust — and for `/shutdown`, whether the wallet process is still running. ⛔ Not the HTTP status seen by the page: a 403 from CORS and a 403 from the gate look identical to the caller, and the page cannot tell whether the handler ran before the reply |
| **Tier** | T2 (a real page, a real approved grant, the real binary) with a T1 unit half over the allowlist predicate |

**Standing invariant?** ⭐ **Yes — propose a row for `../REGRESSION_ADDITIONS.md`.** Something close
to *"an external origin reaches only declared methods"*: the complete allowlist is this ticket's
deliverable, and the invariant must outlive it, because the failure mode is a route added later
being exposed by default. That is the same shape as `R-BEEFOUT` and the same reason.

## Links

- [BRC-179](https://github.com/bsv-blockchain/BRCs/blob/master/wallet/0179.md) — the supply-side
  argument this ticket is an instance of. §14 on ordering ahead of BRC-100/103, §10 on fail-closed
- `../../0.4.0-beta.3/phase-0.5-money-path/ADVERSARIAL_PANEL_2_2026-08-20.md` — findings #28/#34, the
  path-normalisation work this fix inherits
- `rust-wallet/src/main.rs :: domain_trust_mw` · `:: is_permission_surface` — the chokepoint and the
  current deny-list
- `rust-wallet/src/permission_service/request_gate.rs :: domain_trust_gate` — the approved
  short-circuit
- `cef-native/include/core/CWIShimScript.h :: WALLET_CALL_BRIDGE_SCRIPT` — the unvalidated endpoint
- `cef-native/src/core/HttpRequestInterceptor.cpp :: runIpcEngineCascade` — the thin-proxy forward
- `Marston Enterprises/Standards/BRCs/drafts/permission-necessity-and-denial-feedback/` — our own
  draft on the demand side; its 2026-09-19 BRC-179 section is the companion to this ticket
