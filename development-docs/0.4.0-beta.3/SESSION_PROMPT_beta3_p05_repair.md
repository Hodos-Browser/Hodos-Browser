# Session kickoff prompt — REPAIR Phase 0.5 (money path & trust boundary)

> Paste everything below the rule into a fresh session.
> **Written 2026-08-19** at the end of the session that executed Phase 0 and the first pass of
> Phase 0.5. That session's work is committed; its Phase 0.5 half is **not signed off** and must not
> be treated as done.

---

Repair **beta.3 Phase 0.5**. A prior session implemented it, an adversarial panel refuted it, and the
headline defect has since been **measured**. Your job is the structural fix, not a re-review.

## Read first, in this order

1. `development-docs/0.4.0-beta.3/phase-0.5-money-path/ADVERSARIAL_PANEL_2026-08-19.md` — the panel's
   full report. 23 surviving findings, 3 critical.
2. `development-docs/0.4.0-beta.3/phase-0.5-money-path/PHASE_CONTRACT.md` — §4a–§4d carry the
   evidence so far. ⛔ §5's "every caller is first-party today" is **FALSE**; see below.
3. `development-docs/0.4.0-beta.3/HARNESS.md` — the standard. §2 (four-column evidence), §6.
4. `CLAUDE.md` — invariants #2/#3/#13.

`git show 4dec940 d33741a` is the code under repair. `68990dd` records the panel.

## State: what is done, what is refuted

| | Status |
|---|---|
| **Phase 0** (stray `{app}` writes) | ✅ Code complete, verified on a dev run. `P0-A4`/`P0-A5` deferred to **beta.3 RC gates** (need an installed build). `P0-S1` closed NOT REPRODUCED |
| `P0.5-G2` (privileged V8 surface) | ✅ GREEN with an exact control, RED observed. **Sound** — panel agrees |
| `P0.5-G3` (`G2` gate 5→2) | ✅ GREEN, negative control `3 > 2`. **Sound** |
| `P0.5-G1` (frontend-from-disk) | 🟡 Code fixed; not judgeable in dev (`IsFrontendAvailable()` short-circuits). RED owed on a release-shaped build |
| `P0.5-E1` (`about:blank`) | 🔴 **REOPENED.** Fix is bypassable — Critical 1 |
| `send_transaction` gate | 🔴 **BYPASSED** by the IPC transport — Critical 2 |
| `P0.5-C1` (CORS) | 🟡 Code in; may 400 the direct-HTTP dApp transport (panel finding 4). Needs one measurement |
| `P0.5-R1..R4` | ⬜ Unrun |

## 🚨 The three criticals

### C1 — the origin parse is unanchored, so the E1 fix never runs

`cef-native/src/handlers/simple_handler.cpp:2060` — `originFromUrl` does `u.find("://")` over the
**whole** frame URL with no scheme anchor. `data:text/html,a://127.0.0.1:5137/<script>…` therefore
parses to origin `127.0.0.1:5137`, which `IsInternalOrigin` accepts. The ancestor walk (`:2075`) and
the `opaque-origin.invalid` sentinel (`:2085`) both run only `while (origin.empty())`, so **neither
is reached** — the attacker supplies a non-empty origin at step 1.

Also fools `hodos::IsInternalFrontendUrl` and `hodos::IsLoopbackUrl` (`PortConfig.h`) via userinfo
(`http://127.0.0.1:9@evil.com/`). Fixing only `:2060` leaves the render-process privilege leak open.

### C2 — the gate does not cover the transport the endpoint actually uses ⭐ MEASURED

`simple_handler.cpp:5760`'s `send_transaction` IPC arm performs **no origin, role or frame check**.
It calls `WalletService::sendTransaction`, whose only header is `Content-Type`, so Rust sees no
`X-Requesting-Domain` and `dispatch_payment` returns `Proceed`. `cefMessage` is injected into
**every** V8 context (`simple_render_process_handler.cpp:736`, and the comment says so: *"external
pages only get BRC-100 + cefMessage"*), and `CefMessageSendHandler::Execute` has **no message-name
allowlist**.

**Measured 2026-08-19** from `https://example.com`:

```js
cefMessage.send('send_transaction',
  [JSON.stringify({toAddress:'1Q1A…', amount: 999999999999, sendMax:false})])
```
```
💸 /transaction/send called
   Amount: 999999999999 satoshis, sendMax: false
"POST /transaction/send HTTP/1.1" 500
```

No 202, no approval, **no modal**. The amount was deliberately beyond any balance so the request
died at UTXO selection — with a real amount it spends. Control (recorded earlier the same day): the
*same page* calling through `__hodos_walletCall` **hangs on a domain-approval modal**. Same page,
same operation, one transport gated and the other not.

⛔ At least ten other wallet arms share the same origin-free dispatch: `create_wallet` (`:3755`),
`mark_wallet_backed_up` (`:3805`), `get_wallet_info` (`:3851`), `load_wallet` (`:3899`),
`get_all_addresses` (`:3947`), `get_current_address` (`:3995`), `get_addresses` (`:4043`),
`address_generate` (`:5518`, `:5552`), `get_balance` (`:5722`), `get_transaction_history` (`:5909`).

### C3 — two unbounded fund-movers have no gate at all

`rust-wallet/src/handlers.rs:17106` `peerpay_send`, `:18149` `paymail_send`. Both take
`(state, body)` — **no `HttpRequest`** — so `dispatch_payment` is structurally impossible. Both take
attacker-controlled destination + amount and broadcast. Reachable because `wallet_call` concatenates
a page-supplied `endpoint` with no allowlist, and `isWalletEndpoint`'s `/wallet/` arm matches.

## The fix, in the order I would do it

⭐ **The structural lesson: gating arm-by-arm is HOW `:5760` was missed.** Do not patch one arm.

1. **One origin derivation, one spelling, applied once.** Lift the derivation out of the `wallet_call`
   arm into a shared helper. Make it **scheme-anchored** — only `http://`, `https://`, `hodos://`
   yield an origin; strip userinfo through the last `@` of the authority; everything else returns
   empty so the ancestor cascade and the `opaque-origin.invalid` sentinel actually run. Apply the
   same reduction inside `IsInternalFrontendUrl` / `IsLoopbackUrl`.
2. **Default-deny at the top of `OnProcessMessageReceived`.** Resolve the origin once, then reject
   every wallet-touching arm from a non-internal origin before dispatch. An allowlist of *which
   messages are wallet-touching* is the thing to get right; a new arm added later must fail closed by
   default, not by remembering to add a check.
3. **Rust: give `peerpay_send` / `paymail_send` `HttpRequest` + `dispatch_payment`**, as
   `send_transaction` now has. ⚠️ `{recipient_identity_key|paymail, amount_satoshis}` is a **third**
   body shape — `d33741a`'s "do both or neither" warning applies with an extra shape.
4. **Re-run E1 with the crafted `data:` URL as the new RED**, plus the `about:blank` case, plus an
   internal control in the same run.
5. **Panel finding 6** — the forced prompt renders **"0 sats"** and blames a price outage that is not
   occurring. That is a social-engineering surface on a money path, and a direct consequence of the
   fail-closed choice in `d33741a`.
6. **Panel finding 4** — measure whether `block_on_origin_mismatch(true)` now 400s a direct-HTTP dApp
   request that previously got a 202. One measurement; it may need reverting.

## Hard rules carried forward

- ⛔ **Do not report a cause you have not reproduced.** Two of this phase's premises died that way.
- ⛔ **A green is reported with its RED or not at all**, and the RED must be the run you actually did.
- ⛔ **Re-run the WHOLE evidence table after a fix, not the failing row.** That is the only reason the
  fourth `:5137` gate (`isExternalPage`) was caught.
- ⛔ **Never push a `v*` tag to `origin`.**
- ⚠️ `cmake --build … | tail` discards the exit code — capture `$?` separately.
- ⚠️ The dev browser needs `--profile=Default` or it launches the picker (which disables CDP).
- ⚠️ CDP dev port is **9322**; production is 9222 — **do not drive production.** Every overlay and the
  header report `type:"page"`; select the tab by URL.
- ⚠️ Kill dev processes **by executable path**, never by image name — prod shares the name.
- ⚠️ Heredoc/tooling collapses backslashes: prefer forward slashes in C++ test paths, and check that
  a `\f`/`\e` has not silently become a control character.

## Owner decisions open

1. **Scope** — fold C1–C3 into Phase 0.5, or split C2/C3 into their own phase?
2. **Disclosure** — reopened. C2 is a *shipping* unprompted-spend path in beta.1 **and** beta.2, not a
   new regression. `SPRINT_PLAN.md` §6.5 was closed for Phase 0 on the grounds that no key material
   leaked; that reasoning does not cover this.
3. `P0.5-R3`: design around the **202 decision** (no funds needed) or prove the full broadcast?
4. Phase 0's `P0-A4`/`P0-A5` RC gates — when the beta.3 RC is built.
