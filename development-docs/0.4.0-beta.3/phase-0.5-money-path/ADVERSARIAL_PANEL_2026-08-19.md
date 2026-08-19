## Adversarial review — Phase 0.5 (`4dec940`, `d33741a`)

**Panel:** 3 lenses (*un-stamped money* · *forge an origin* · *does internal stay silent*), each finding then challenged by an independent refuter. **Method:** source reading only — `git show`, `grep`, `sed`, plus reads of the pinned `actix-cors 0.7.1` source, the Chromium/CEF tree at `C:\cef\cef150`, and two recorded log files from prior runs. **No build, no browser, no wallet process was run by this panel.** Every claim below is labelled.

**Verdict: DO NOT SIGN OFF.** Two of the phase's own written premises are false, and the endpoint the phase exists to gate is still reachable un-gated by two independent routes.

---

### HARNESS.md §6 — the four questions, answered

#### 1. Can I make this test pass with the feature removed?

**For three of the nine rows, yes — and one of them is structurally guaranteed to.**

| Row | Answer |
|---|---|
| `P0.5-G2` | **No.** The control is exact — same origin, same page, only the query string differs. Pre-fix `identity/navigation/history` were `object/object/object` with the substring and `undefined` without; post-fix both are `undefined`. Remove the fix and the row goes red. This row is sound. |
| `P0.5-G3` | **No.** `-NegativeControl` was observed at `3 > 2`. Sound. |
| `P0.5-E1` | **Partly.** The recorded RED (`about:blank` child ⇒ empty origin) is a *strict subset* of the reachable inputs. The fix's step-1 parse is unanchored, so an attacker-chosen frame URL never reaches the ancestor cascade at all (Finding 1). The row's GREEN is real for the input tested and **void as evidence that the boundary holds**. |
| `P0.5-R4` | **Yes, and it is the likely outcome.** `isPaymentEndpoint` = `{/createAction, /acquireCertificate, /sendMessage}` (`HttpRequestInterceptor.cpp:1599-1603`, read), and `d33741a` deliberately excludes `/transaction/send`. The pill can never fire for this endpoint. The obvious way to "test the gold pill" is to run a `createAction` — which passes with both commits reverted. That is a §6-Q1 void green waiting to happen (Finding 10). |
| `P0.5-C1` | **The row is honestly marked 🟡 CODE IN with the server-side effect ⬜ owed** — and the owed half is where the defect is (Finding 4). |
| `P0.5-R1/R2/R3` | Unrun. Cannot be assessed. |

#### 2. What is the subject?

The contract's subject discipline is the best part of this phase and it held. `P0.5-G2` was measured against the tab's **own** frame with `location.href` asserted in the same call, against a browser exposing **11 CDP targets all reporting `type:"page"`** — the trap that killed three farbling harnesses. `P0.5-G1` was correctly ruled **not judgeable in dev**, because `IsFrontendAvailable()` short-circuits the `&&` before the URL check — a refusal to claim a green, which is the right call.

**Two subject gaps the panel found:**

- **`P0.5-R1`/`R2`'s subject is the Rust log, and the wrong transport is being watched.** `REGRESSION_SET.md:16-19` names the discriminator as re-derived "at `simple_handler.cpp :: OnProcessMessageReceived` (**`wallet_call` arm**)". The wallet UI's own send does **not** go through that arm — it goes through the `send_transaction` arm at `simple_handler.cpp:5760`, which derives no origin at all (Finding 2). R-INTEXT half (a) as written measures a path the product does not use for this endpoint.
- **`P0.5-C1`'s subject is server-side effect** — correctly specified, never exercised, and it is the case the flag actually changes (Finding 4).

#### 3. What would I expect to see if this were broken — and did anyone look?

| If broken, you would see… | Looked for? |
|---|---|
| An attacker frame reaching Rust with **no** `X-Requesting-Domain` | ✅ Yes, for `about:blank`. ❌ **No** for any frame URL containing a non-scheme `://` — the input class that bypasses the new cascade entirely. |
| A page calling `/transaction/send` **without** `wallet_call` | ❌ **Nobody looked.** Contract §5 asserts "every caller is `WalletService::sendTransaction` … i.e. first-party today" as an assumption; it is false. |
| An external dApp's direct `fetch` to the wallet port returning **400** instead of 202 | ❌ **Nobody looked.** §4d's C1 reasoning traces only the header-*less* case — the one case `block_on_origin_mismatch` does not change. |
| A gold pill **failing** to fire on the newly-gated endpoint | ❌ Not looked for; the row is unrun and unsatisfiable as written. |
| The user's own send starting to **prompt** | ✅ Partially — §4b re-ran the internal half of the V8 surface in the same run. The Rust-side half (`R1`) is still owed, and the panel's own measurement (28,535 internal dispatches, all `127.0.0.1:5137`; 1 empty, and that one was the E1 probe itself) says the risk is very low but does not substitute for the run. |

#### 4. Measurement or code reading?

**The commits' own labelling is honest and holds up.** `P0.5-G2` and `P0.5-E1` are real measurements with recorded log lines and a same-run control. `P0.5-C1` is explicitly labelled as verified against actix-cors **source**, not docs — correct discipline, and the panel independently re-read that source and confirms `(None, _) => false` at `middleware.rs:221` (the commit's characterisation is accurate; the reviewer's cite of `:224` was two lines off).

**This panel's own labelling:** Findings 1–3 and 5–14 are **code readings**. Finding 4 rests on a measurement someone else recorded (a production log showing `Origin: https://zanaadu.com` on an intercepted request) plus code reading of Chromium's `cors_url_loader.cc`; the link from that to what Rust receives is **unverified by anyone** and is stated as such. Finding 14 is a measurement (`grep -c $'\f'`) the panel re-ran itself.

---

## Surviving findings, severity order

### 🔴 1. CRITICAL — the fix's own parse is unanchored, so the new cascade never runs · **BLOCKS**

`cef-native/src/handlers/simple_handler.cpp:2060-2068`

**CODE READING.** `originFromUrl` does `u.find("://")` over the **whole** frame URL with no scheme validation, and the new ancestor walk (`:2075-2078`) and top-document fallback (`:2081-2083`) both run only `while (origin.empty())`. So any frame URL whose first `://` is not a scheme short-circuits the entire fix at step 1 with an attacker-chosen string:

- `<iframe src="about:blank?a://127.0.0.1:5137/">` — the commit's own PoC plus 21 characters, still same-origin scriptable by the attacker.
- `<iframe src="data:text/html,a://127.0.0.1:5137/<script>…">`.
- `http://127.0.0.1:9@evil.com/` — userinfo; Chromium does **not** strip credentials from the committed top-level document URL (`document_loader.cc:3541-3546` counts `Url().User()` on the committed document, which would be dead code if it did), and `CefFrameImpl::GetURL` returns that spec verbatim.

All three yield origin `127.0.0.1:5137` (or equivalent), which `IsInternalOrigin` accepts (`HttpRequestInterceptor.cpp:1015-1024`, verified), which routes to `runIpcCallDirect`, which omits `X-Requesting-Domain` (`:1940`), which `domain_trust_mw` (`main.rs:63-75`) and `dispatch_payment` (`request_gate.rs:1000`) both read as fully-trusted internal. `cefMessage` is injected into **every** V8 context — the internal-only block closes at `simple_render_process_handler.cpp:733`, injection is at `:736-741`, verified — so the frame has the API. The same primitive also forges any **external** origin (`data:text/html,a://trusted-dapp.example/`), inheriting another dApp's spend caps and identity-disclosure grant.

**Why it matters.** This refutes two written claims the phase rests on. `PHASE_CONTRACT.md:154-157`: "a `data:`/`blob:` frame … is still **external and gated**, which is the property that matters here" — for a crafted `data:` frame it is **internal**. `REGRESSION_SET.md:19`: `frame->GetURL()`, "which the renderer cannot forge" — the renderer chooses its own subframes' URLs and the parse is unanchored. Reachable endpoints include `/transaction/send` with `sendMax`, and `/wallet/export`, which gates on header *presence* only (`handlers.rs:16849-16862`) and serialises the wallet under a caller-chosen password — with read-back, because `wallet_response` calls the plain global `window.__hodos_walletResponse` in the calling frame (`simple_render_process_handler.cpp:985-989`), which the attacker defines.

**Fix direction (one place, per the commit's own principle of fixing at the derivation):** require an **anchored** scheme before an authority is treated as an origin — only `rfind("https://",0)==0` / `rfind("http://",0)==0` yield one, `hodos://` maps explicitly, everything else returns empty so the cascade and the `opaque-origin.invalid` sentinel actually get to run. Strip through the last `@` of the authority. Apply the same reduction inside `hodos::IsInternalFrontendUrl` and `hodos::IsLoopbackUrl`, which are *also* fooled by userinfo — fixing only `:2062` leaves the render-process privilege leak open.

**`P0.5-E1` must be reopened, not amended.** The row asserts an origin-less frame is gated; an attacker-chosen frame URL is the same subject, and it has not been seen RED.

---

### 🔴 2. CRITICAL — the new gate cannot fire on the transport that actually calls the endpoint · **BLOCKS**

`cef-native/src/handlers/simple_handler.cpp:5760`

**CODE READING, verified by the panel directly.** The `send_transaction` IPC arm performs **no origin, role or frame check** — the panel read `:5755-5795` and there is no origin variable in it. It calls `WalletService::sendTransaction` (`WalletService.cpp:698`) → `makeHttpRequest("POST", "/transaction/send", …)`, whose only header is `Content-Type: application/json` (`:154-160`; macOS `WalletService_mac.cpp:121-123` identical). No `X-Requesting-Domain` ⇒ `dispatch_payment` hits `None => return GateOutcome::Proceed` at `request_gate.rs:1000`. Because `cefMessage` is on every page and `CefMessageSendHandler::Execute` has no message-name allowlist, `cefMessage.send('send_transaction', [JSON.stringify({toAddress:'1…', sendMax:true})])` from **any web page** spends, with no prompt and no engine evaluation.

This is also the transport the first-party UI uses (`initWindowBridge.ts:486`), so attacker and user are **byte-identical at the wallet**.

**Why it matters.** `PHASE_CONTRACT.md:161-162` §5 states: "every caller is `WalletService::sendTransaction` via `simple_handler.cpp:5740`, i.e. **first-party today**." That is the load-bearing claim of the whole phase and it is false — that call site is page-reachable. Rows `P0.5-R2`/`R3` can go green while the primary money path stays open. The commit body's own justification ("a web page can reach it — `sendMax` included") is true of a door the fix does not cover.

**Broader than one arm.** The same origin-free dispatch carries at least ten other wallet arms reachable from any page: `create_wallet` (`:3755`), `mark_wallet_backed_up` (`:3805`), `get_wallet_info` (`:3851`), `load_wallet` (`:3899`), `get_all_addresses` (`:3947`), `get_current_address` (`:3995`), `get_addresses` (`:4043`), `address_generate` (`:5518`, `:5552`), `get_balance` (`:5722`), `get_transaction_history` (`:5909`). Patching `:5760` alone leaves a wallet-wide disclosure surface. **The right fix is to lift the E1 origin derivation out of the `wallet_call` arm into a helper applied at the top of `OnProcessMessageReceived`, and make every wallet arm fail closed on a non-internal origin** — gating arm-by-arm is exactly how `:5760` was missed.

---

### 🔴 3. CRITICAL — two unbounded fund-movers have no payment gate at all · **BLOCKS**

`rust-wallet/src/handlers.rs:17106` (`peerpay_send`), `:18149` (`paymail_send`)

**CODE READING, signatures verified by the panel.** Neither takes `HttpRequest`, so `dispatch_payment` is *structurally impossible* for them. Both take attacker-controlled **destination and amount** (`{recipient_identity_key|paymail, amount_satoshis}`); `peerpay_send` builds a real `CreateActionRequest` with `satoshis: Some(req.amount_satoshis)` (`:17201-17205`) and broadcasts at `:17293`. The only validation is `amount_satoshis <= 0`. Their sole gate is `domain_trust_mw`, which returns `Proceed` for any `trust_level = approved` domain (`request_gate.rs:804-809`) — so they bypass the per-tx cap, the per-session cap **and** the session counters entirely, which is strictly worse than the hole `d33741a` just closed. They are named by any page, because `wallet_call` concatenates a page-supplied `endpoint` onto the wallet base URL with no allowlist (`HttpRequestInterceptor.cpp:1943`, `:2083`), and are matched by `isWalletEndpoint`'s `/wallet/` arm on the HTTP path.

Not `isPaymentEndpoint` either, so these spends are **ungated *and* visually unannounced** — R-GOLD is violated for them.

**Why it matters.** This is a direct answer to the phase's §1 goal, "a fund-moving request from a web page is subject to the same approval engine as every other payment." Gating `/transaction/send` while `peerpay_send` and `paymail_send` remain wide open makes the phase's title broader than its content. *(Downgrade note: `broadcast_nosend` takes only `{txid}` and `wallet_consolidate_dust` / `wallet_backup_onchain` ignore the body — fee-burn/griefing, not theft. Do not lump them with the two above.)*

**Trap for whoever fixes it:** `{recipient_identity_key, amount_satoshis}` is a **third** body shape. `d33741a`'s own "do both or neither" warning applies verbatim, with an extra shape.

---

### 🟠 4. HIGH — `block_on_origin_mismatch(true)` 400s the direct-HTTP transport before any handler runs · **BLOCKS (needs one measurement before ship)**

`rust-wallet/src/main.rs:935`

**MEASUREMENT (recorded log, not this panel's run) + CODE READING.** A production log shows a real BRC-100 dApp call from `zanaadu.com` carrying `Header: Origin = https://zanaadu.com`, and `HttpRequestInterceptor.cpp:3651-3664` forwards `originalHeaders_` into the re-issued `CefURLRequest` with **no filter**. Chromium then, on a POST, *overwrites* `Origin` with `request_initiator->Serialize()` (`cors_url_loader.cc:923-974`), and CEF sets `request_initiator` from the **target** URL (`browser_urlrequest_impl.cc:285`) ⇒ Rust receives `Origin: http://127.0.0.1:31301`. On GET, the page's real origin survives verbatim. **All three possible values — the dApp origin, `http://127.0.0.1:31301`, and `null` (also seen in the log) — are absent from the four-entry allowlist** (`main.rs:922-925`), so `inner.rs:90-100` returns `OriginNotAllowed` and `middleware.rs:213-222` short-circuits without calling the inner service. Note `block_on_origin_mismatch` **defaults to false** (`builder.rs:480`), so pre-change these took `Ok(false)`: handler ran, CORS headers merely omitted. That is why nothing was ever seen to break.

The failure is also **silent and mangled**: `GetResponseHeaders` hardcodes `SetStatus(200)` (`:1416-1419`) and `OnRequestComplete` only special-cases 202 (`:3501`), so the page gets HTTP 200 carrying the plain-text CORS error body. Worse, at `:3542-3559` a 400 still has `UR_SUCCESS`, the non-JSON body makes `json::parse` throw, the `catch(...)` leaves `isError = false`, and `OnWalletCallSuccess(..., wasAutoApprovedPayment=true, ...)` fires — **the gold pill lights for a payment that never reached the wallet**. No spend is recorded, so this is a false positive, not a double-spend, but it makes a named load-bearing safeguard lie.

The IPC bridge is **unaffected** (both `runIpcCall*` paths use `SyncHttpClient`, which sends no `Origin`), so the wallet UI does not brick and R-INTEXT half (b) *as written* survives. What breaks is every external dApp using `@bsv/sdk WalletClient`'s default transport, and any first-party page doing a direct `fetch`.

**This is `P0.5-C1`'s owed server-side effect test, and it is owed before ship, not after.** Two-liner: `curl -i -X POST -H "Content-Type: application/json" -H "Origin: http://127.0.0.1:31301" --data "{}" http://127.0.0.1:31401/getVersion` — expect 400; negative control is the same call with no `-H Origin` returning real JSON, and reverting the one line. Then the netlog half (`--log-net-log`, read `HTTP_TRANSACTION_SEND_REQUEST_HEADERS`) to settle which `Origin` the network service actually puts on the wire.

⚠️ Per CLAUDE.md invariant #13 the evidence points at **production code**. The obvious repair (allowlisting `127.0.0.1:31301`/`31401`) admits an origin only the interceptor's own re-issue can produce, and deserves a decision rather than a reflex. **Stop and ask.**

---

### 🟠 5. HIGH (latent) — page-supplied headers *overwrite* the C++-derived trust headers · **BLOCKS the "fail-closed" argument; fix is cheap**

`cef-native/src/core/HttpRequestInterceptor.cpp:3651-3664`

**CODE READING.** The loop `headers.insert(...)` merges `originalHeaders_` into the same map that already carries `X-Requesting-Domain` (`:3623`) and `X-Payment-*` (`:3643-3648`), with no denylist. The reviewer's original "multimap keeps both, actix reads the first" is **wrong, and the truth is worse**: CEF serialises duplicates (`http_header_utils.cc:17-32`) into `net::HttpRequestHeaders::AddHeadersFromString`, which per line calls `SetHeader` → `SetHeaderInternal`, a **case-insensitive overwrite** (`http_request_headers.cc:202-207, 164-200, 291-318`). Exactly one value reaches actix, and it is the **last** emitted — the page's, since the trusted headers go in first.

Consequences: a page-supplied `X-Requesting-Domain: ` (empty) makes both `main.rs:63-70` and `dispatch_payment` take the "no domain ⇒ internal, no gate" branch — full trust escalation on this transport. A page-supplied `X-Payment-Satoshis/Cents/X-Bsv-Price-Available` inverts the deliberate fail-closed branch into `SilentWithinCaps` (the repo's own test `bsv_price_available_with_zero_cents_is_silent`, `matrix_c.rs:782-794`, asserts precisely that).

**Why it matters.** The commit's fail-closed argument for omitting `/transaction/send` from `isPaymentEndpoint` is stated as a property of the code; it is a property of Chromium's CORS preflight on a path that *explicitly forwards attacker-controlled headers*. This phase's own C1 commit message argues that barrier is not sufficient ("a blocked read is not a blocked write"). Fix: strip `X-Requesting-Domain`, `X-User-Approved`, `X-Browser-Id`, `X-Payment-*` from `originalHeaders_` before the merge.

---

### 🟠 6. HIGH — the forced prompt renders **"0 sats"** and blames a price-feed outage that is not occurring · **BLOCKS (UX/social-engineering on a money path)**

`rust-wallet/crates/hodos_permission_engine/src/matrix_c.rs:240` → `frontend/src/pages/BRC100AuthOverlayRoot.tsx:557-563, 577-582, 1211`

**CODE READING, chain verified by the panel.** With no `X-Payment-*`, `request_gate.rs:1056-1067` substitutes `{satoshis:0, cents:0, bsv_price_available:false, browser_id:0}`; `matrix_c.rs:240` then returns `Prompt(PaymentConfirmation, PriceUnavailable)` **before** the rate (`:250`), max-tx (`:262`), per-tx (`:273`) and session-cap (`:281`) checks. The modal renders `formatSatoshis(0)` = **"0 sats"** with copy reading *"The BSV/USD price feed is currently unavailable… Verify the satoshi amount above before approving"* — for a request that may be `sendMax:true`. The price cache is fine; C++ simply never injected the header because `isPaymentEndpoint` excludes the endpoint.

To be fair to the change: always-prompting is *stricter* than any cap, and the endpoint went from ungated to always-prompting in this very commit. The defect is not "caps skipped" — it is that a sweep the user did not initiate is presented as **0 sats** under a false cause, with an instruction to verify a number the code just zeroed.

**Contract corrections owed:** §2 "Done means — `sendMax` from an external origin is subject to the per-tx and per-session caps" and row `P0.5-R3` "External `sendMax:true` is capped" are **not achievable with this code** — the cap branches are unreachable for this endpoint. Rewrite both to say *forced prompt*, or do "both" (extend `isPaymentEndpoint` **and** teach `extractOutputSatoshis` the `{toAddress, amount, sendMax}` shape).

Related, same root: `browser_id = 0` (the `X-Browser-Id` injection shares the `isPaymentEndpoint` gate). Every external `/transaction/send` lands in a shared counter bucket that `session_close` (`handlers.rs:184`) never clears — an **R-COUNT** inversion. Moot today only because `:240` returns before the counters are read.

---

### 🟡 7. MEDIUM — `IsInternalOrigin` trusts **any** loopback port; `IsInternalFrontendUrl` requires `:5137` · *follow-up (Phase 5 W0), but name it*

`cef-native/src/core/HttpRequestInterceptor.cpp:1015-1024`

**CODE READING.** `matchesHostOrHostColon` accepts `127.0.0.1`/`localhost` followed by `':'` and **any** port. A page at `http://localhost:8000` gets no privileged V8 surface but *does* get `cefMessage`, and its `wallet_call` resolves to `localhost:8000` ⇒ internal ⇒ header-less ⇒ fully trusted, on any endpoint (no allowlist on that path). Any local dev server, local app HTTP surface, or reflected XSS in one becomes a full wallet-control surface — including `/transaction/send` with `sendMax`, with no prompt and no pill.

**Correction to how this was first reported:** `4dec940` did **not** create the divergence — the pre-fix gate was `url.find("127.0.0.1:5137") != npos`, which `localhost:8000` also fails, and the port-agnostic behaviour is documented as deliberate at `:1931-1937` ("or any loopback caller"). The defect is the *policy*, not a missed unification. But `PHASE_CONTRACT.md:23`/§4d's justification for leaving `IsInternalOrigin` alone covers **only** the empty-origin/HTTP-path case; the port dimension is nowhere in the contract, and `TICKET_loopback_host_form_wallet_routing.md:537-539` asserts "the rest of that function is sound," which is true for the suffix shape and false for this one. **Add it to §6 as a named residual.**

---

### 🟡 8. MEDIUM — `opaque-origin.invalid` is one shared, approvable trust identity · *follow-up, cheap to close now*

`cef-native/src/handlers/simple_handler.cpp:2087, :2093`

**CODE READING.** The panel confirmed the sentinel **cannot become trusted** — a clean negative result, checked against `IsInternalOrigin`, `IsInternalFrontendUrl`, `IsLoopbackUrl`, `IsWalletHostPort` and a grep of the Rust permission service. The defect is the opposite: it is an ordinary domain string downstream. `POST /domain/permissions` (`handlers.rs:10077`) validates only `!domain.is_empty()`, and `domain_permissions` is keyed `UNIQUE(user_id, domain)` (`migrations.rs:468-481`). One approval — especially via `DomainPermissionForm`, which sets persistent caps — creates a permanent row every other site's unattributable frame inherits, along with `PermissionService.session_counters` and the V18 protocol/basket/counterparty scoped-grant child tables (FK to `domain_permissions(id)`).

Reachable via `window.open()` / `window.open('about:blank','_blank','popup=1')` — `OnBeforePopup` returns `false` (default popup) for empty target URL at `:1923` and for `CEF_WOD_NEW_POPUP` at `:1931`. *(Bare `window.open('about:blank')` maps to `NEW_FOREGROUND_TAB`, is converted to a tab, and returns `null` — the original PoC one-liner does not work.)*

Fail-closed to a shared bucket is not fail-closed once the user can say yes. **§4d justifies the sentinel only as "can never collide with a real host" and never addresses that all origin-less top-level frames collapse into one *approvable* identity — an unexplained residual under HARNESS §4.** Cheapest close: reject the literal in `set_domain_permission` and short-circuit it to `Deny` in `domain_trust_gate`; or insert an opener step (`CefBrowser::GetOpenerIdentifier()`) before the sentinel. Do **not** make it per-request unique — that mints unbounded promptable pseudo-domains.

---

### 🟡 9. MEDIUM — the derived origin is scheme-blind: `http://dapp.example` inherits `https://dapp.example`'s grants · *follow-up*

`simple_handler.cpp:2060-2068` · **CODE READING.** Scheme discarded, port retained, so it is the sole conflation. A user who grants spending caps and silent identity-key disclosure to the https site grants them to the plaintext twin; any active network attacker on that twin inherits the wallet permissions. The CWI shim is https-only (`simple_render_process_handler.cpp:791-792`) but `cefMessage` is not.

**Not introduced by `d33741a`** — the deleted inline block was behaviourally identical; the commit hoisted it into a lambda. But §6 defers only "`extractDomain()`'s scheme-blindness", naming one function in one file, so **the new derivation's identical property is not covered by that deferral and should be added to §6.**

---

### 🟡 10. MEDIUM — `P0.5-R4` is vacuous and invites a false green · **BLOCKS sign-off of that row (withdraw in writing)**

**CODE READING.** No `/transaction/send` call can emit `payment_success_indicator`: every one of the six `OnWalletCallSuccess` call sites derives `wasAutoApprovedPayment` as `ok && isPaymentEndpoint(endpoint) && !isError` (`:2125, :3054, :3177, :3294, :3548`), and the sixth (`:4516`) hardcodes `endpoint="pay402"`. Stronger still: the row has **no subject** — an external `/transaction/send` can never be *silently* approved at all, because `matrix_c.rs:240` unconditionally prompts and the only `silent(SilentWithinCaps)` is at `:289`, unreachable past it. The external outcome ladder is Deny / Prompt / user-approved replay.

Per HARNESS §1 ("scope changes amend the contract in the same commit"), **withdraw `P0.5-R4` with a written reason** — a withdrawn row with a reason is a valid outcome; a row quietly greened on `/createAction` is a §6-Q1 void. Leave R-GOLD to `REGRESSION_SET.md:35-41` at the 0.5→1 boundary. File separately: an `X-User-Approved` replay on `/transaction/send` moves funds and emits no pill while a byte-identical `/createAction` replay does.

---

### 🟡 11. MEDIUM — the privileged V8 surface has no main-frame check, and the 5137 frontend has no framing protection · *follow-up (already filed as W7, beta.4)*

`simple_render_process_handler.cpp:553, :587-733, :784-785` · **CODE READING + MEASUREMENT (grep).** `isInternalPage` is computed from `frame->GetURL()` with **no** `frame->IsMain()`, while the external dApp arm eleven lines below (`:791`) *does* check it. `grep -nE "X-Frame-Options|frame-ancestors|Content-Security-Policy"` across `cef-native/src`, `LocalFileResourceHandler.h`, `frontend/index.html`, `frontend/vite.config.ts` returns **zero** hits, and there is no frame-busting in `frontend/src`.

**This refutes `PHASE_CONTRACT.md:132-134`'s parenthetical** — "that bridge is main-frame-only per the shim's gating cascade" is true of the *external* arm only. For the `about:blank` frame it describes, the stated conclusion is still correct (that frame is external), so no security conclusion in the commit collapses; but the sentence over-generalises and should be struck in place per HARNESS §8.

Impact is bounded by same-origin policy — the framing page cannot script the cross-origin subframe, so this is **clickjacking of money UI**, not API theft. Any W7 fix must add the frame check at `:587` as well as `:784`; the `history.clearAll`/`identity`/`navigation` surface is inside the same ungated condition.

---

### 🟡 12. MEDIUM — anchored but not host-terminated; and `wallet_call` takes an arbitrary endpoint string · *follow-up, one line each*

**CODE READING.** `PortConfig.h:74-78, 88-98` anchor at the prefix but never check the character *after* it, so `http://localhost.evil.com/` is "loopback" and `http://127.0.0.1:51370/` is "internal frontend". The `IsLoopbackUrl` half is **functional, not security** — its only consumer computes `isExternalPage`, so a match *removes* the CWI shim from the attacker's own page (real sites like `localhost.run` silently lose the provider). The `IsInternalFrontendUrl` half is a genuine grant, bounded in release because `simple_handler.cpp:8011-8013` serves Hodos's own frontend from disk cross-origin. The terminator already exists in-tree at `HttpRequestInterceptor.cpp:1017-1022` — reuse it, and it closes the userinfo variant of Finding 1 at the same time. `PortConfig.h:62-63` calls the prefix match a "TRUST BOUNDARY"; prefix-anchoring is necessary, not sufficient.

Separately, both IPC paths build `hodos::WalletBaseUrl() + endpoint` from a page-supplied string with no allowlist, no leading-`/` check, no charset filter (`:1943`, `:2083`). On Windows the `@evil.com` host-hijack does **not** work (`SyncHttpClient::ParseUrl` has no userinfo concept and always takes `127.0.0.1`), but two other hazards do: `endpoint="9/x"` reaches `127.0.0.1:50875` via `stoi` overflow-and-wrap with the body readable by the page, and `endpoint="99999999999"` throws an uncaught `std::out_of_range` on a CEF worker thread — **a one-line browser-process crash from any web page**. On macOS `CURLOPT_URL` *does* honour userinfo, so the off-host retarget is live there pending measurement. Note this contradicts CLAUDE.md's "`isWalletEndpoint` route table is the entry point for all new wallet endpoints; new endpoints go through the table, never around it" — the IPC transport goes around it entirely, which is how Findings 2 and 3 stay reachable.

---

### 🟢 13. LOW — `X-User-Approved` is bound to id + body hash only · *follow-up*

`rust-wallet/src/permission_service/state.rs:261-287` · **CODE READING.** `consume_and_verify` checks `expires_at` and `body_hash` and never compares the stored `domain`/`endpoint` against the replaying request (they are used only for `record_spending` at `request_gate.rs:1022` and in log lines). All four dispatchers omit the binding; `dispatch_cert_disclosure`'s header-present branch (`:580-608`) is weaker still — `consume_pending_approval`, no body hash at all, trusting `X-Cert-Approved-Fields`, whose "never page-supplied" premise (`:296-298`) is contradicted by Finding 5's header-forwarding loop. Defence-in-depth only: ids are 128-bit CSPRNG, single-use, 10-minute TTL, and the IPC transport gives a page no header channel. But an approvalId **does** leak to the page on two fall-through paths (`HttpRequestInterceptor.cpp:2102-2105`, `:3520-3525`), so it is not unconditionally secret. Two equality checks close the class.

**Answering the specific question the phase asked:** the createAction rebuild non-determinism is a **non-issue**. The approval is bound to the **outer** `/transaction/send` body, which C++ re-issues verbatim, and the inner `create_action` keeps a header-less `TestRequest` (`handlers.rs:9747-9748`), so the synthesised body never participates in hash matching. The commit's reasoning there is sound.

---

### ⚪ 14. INFORMATIONAL

- **`hodos://` arm is host-unconstrained** (`PortConfig.h:77`) over a scheme nothing registers (`grep` for `AddCustomScheme|OnRegisterCustomSchemes|CefSchemeRegistrar` ⇒ zero hits, and `NavigationHandler.cpp:28-32` rewrites it pre-navigation, with two more strippers downstream). Dead today; the day someone registers the scheme, `hodos://evil.com/` becomes fully privileged with no host check and the comment above it will read as though that were verified. Delete the arm or host-constrain it, and rewrite the comment — it cites the rewrite as the reason a frame *can* carry the scheme, when the rewrite is why it cannot.
- **Two committed comments contain a raw form feed.** **MEASUREMENT — the panel re-ran `grep -c $'\f'`: exactly one each in `cef-native/include/core/PortConfig.h` (line **59**, not 58 — the claim was off by one) and `cef-native/src/handlers/simple_handler.cpp:8009`.** Both are new in `4dec940` (`git log -S$'\f'`). `PortConfig.h:59` reads `serves {app}<FF>rontend// from disk` with the next comment line spliced onto it (162 chars); `simple_handler.cpp:8009` has the form feed alone. Cosmetic — a form feed does not terminate a `//` comment and neither line ends in a backslash, and neither matches the G2 pattern, so no gate or behaviour moves. But these are the documentation for a trust-boundary predicate and they now name a path that does not exist. Cause is consistent with a shell escape layer: adjacent `\index.html` survived because `\i` is not a recognised escape. Fix via `Edit`, not a heredoc. Adversarial extension: a scan of all 325 added lines for non-TAB/LF control bytes found **exactly these two** — no other damage.

---

## What survived refutation (record the negative results)

Per HARNESS §8, "not reproduced" is a valid complete state. The panel attacked and **failed to break**:

1. **The internal path is intact.** **MEASUREMENT** (greps and log analysis the panel re-ran or verified): all 31 distinct first-party `5137` URL literals in `cef-native/` begin `http://127.0.0.1:5137`, so every overlay, the header and every internal tab page still match the new prefix predicate; `http://localhost:5137` appears only inside `PortConfig.h` itself. In 28,536 recorded internal `wallet_call` dispatches, exactly one had an empty origin, and reading its context shows it dated 2026-08-19 09:43:04 from a single-tab `example.com` session — the `P0.5-E1` probe itself. **Stronger and more durable than the frontend grep the reviewer used:** the internal IPC dispatch builds its header map from scratch (`HttpRequestInterceptor.cpp:1929-1942` sets `Content-Type` only), so no first-party call can carry an `Origin` regardless of what the frontend does. `block_on_origin_mismatch` cannot touch it.
2. **`opaque-origin.invalid` cannot become trusted** — checked against every loopback predicate in both processes (Finding 8).
3. **The `send_transaction` signature change is safe** — one call site (`main.rs:1080`), `web::Bytes` correctly last, no stale two-arg callers.
4. **`blob:` and top-level http/https URLs are not forgeable** — `blob:https://evil.com/uuid` parses correctly to `evil.com`, and `https://evil.com/?x=a://127.0.0.1/` parses to `evil.com`, because a real scheme always puts its `://` first. Finding 1 requires a **non-special-scheme** frame, which is exactly the class §4d believed it had covered.
5. **`history.pushState` cannot introduce credentials** (`history_util.cc:24-27`) and **credentialed iframes are blocked** (`navigation_request.cc :: CheckCredentialedSubresource`) — do not re-litigate these as vectors for Finding 1's userinfo variant; the live one is a top-level navigation.

---

## Sign-off recommendation

**Blocking for beta.3 — Findings 1, 2, 3, 4, 5, 6, 10.** 1–3 are un-gated money or wallet-control paths reachable from an ordinary web page; 4 may have taken out the primary dApp interop transport and is exactly `P0.5-C1`'s owed test; 5 makes the fail-closed argument rest on a control in a different layer that this phase's own commit message says is insufficient; 6 puts a false "0 sats" in front of a possible balance sweep; 10 is a row that cannot be honestly greened.

**Follow-up — Findings 7, 8, 9, 11, 12, 13, 14.** 7, 9 and 8 should at minimum be **added to §6 as named residuals with reasons** before sign-off (HARNESS §4: an unexplained residual is a defect, not a baseline); 12's `std::stoi` crash and 11 are cheap and belong with W7/Phase 5.

**Contract corrections owed in the same commit as any scope change:** §5's "every caller … is first-party today" (false, Finding 2); §4d's "still external and gated" for `data:` frames (false, Finding 1) and its "main-frame-only" parenthetical (over-general, Finding 11); §4c's "internal and external are true complements again" (not host-terminated, Finding 12); §2 and `P0.5-R3`'s cap language (unreachable, Finding 6); `P0.5-E1` **reopened**, `P0.5-R4` **withdrawn with a reason**.

**What this panel does and does not license.** It read code and recorded artifacts. **It did not run the browser, the wallet, or any build.** It therefore cannot substitute for a single one of the owed T2 rows — `P0.5-R1`, `R2`, `R3`, `R4`, `C1`'s server-side effect, or `G1` on a release-shaped build. A code reading that says a path is open is not the same as having seen it open; every blocking finding above ships with the specific experiment that would convert it, and Findings 1, 2 and 4 each have a one-command repro. Conversely, the negative results above are the panel's strongest output and were the phase's stated central risk: **the evidence says the internal path did not regress**, and that is worth recording explicitly rather than assuming.