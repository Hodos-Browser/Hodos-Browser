# TICKET — wallet routing ignores the `127.0.0.1` host form on BRC-100 compatibility ports

**Status:** 🟢 SCHEDULED — verified 2026-08-18 (see §13) and split: **(a) → Phase 0.5**, **(b) → Phase 5**.
**Was:** 🔵 PROPOSED — not scheduled. Not yet cut into any sprint.
**Opened:** 2026-08-18
**Origin:** external interop failure against the BRC App Lab (Brandon Cryderman / HandCash)
**Research:** 9 specialist/review agents, read-only. All three reviewers returned `needs-rework` on
the first-draft plan. The plan below is the *revised* one.

> ⚠️ **Read §7 before §5.** The research turned up pre-existing findings on the money path that
> outrank the compatibility bug this ticket was opened for. If only one thing gets scheduled, it
> should be §7.1, not §5.

---

## 1. Summary

A BRC-100 dApp that addresses the wallet as `http://127.0.0.1:3321` instead of
`http://localhost:3321` is not intercepted by Hodos at all. The request falls through to the real
network stack. The dApp concludes no wallet exists.

The immediate fix is small. The research around it found that the C++ interception layer is not only
a permission gate — it is the component that *marks traffic as untrusted*. Traffic it fails to
intercept arrives at the Rust wallet with no `X-Requesting-Domain` header and is therefore treated as
an **internal, fully-trusted** call. That inverts the usual risk calculus: a missed match is not
"ungated", it is "trusted". Several consequences follow, and some of them are live today.

---

## 2. The why

**This is a general compatibility bug, not a favor to one site.** Any dApp that writes the IPv4
literal fails, and the failure mode is the worst one available to a discovery protocol: the wallet
appears not to exist. There is no error, no prompt, no log the developer will see. They conclude
Hodos doesn't work and we never hear about it. The App Lab is simply the first third-party exerciser
of the standard local substrate that made it visible.

**We are also, right now, handing dApps a different vendor's wallet.** `netstat` on the reference
machine shows MetaNet Client `LISTENING` on both `127.0.0.1:3321` and `127.0.0.1:2121` (PID 37360).
Because our gate does not match the IP form, an App Lab request in Hodos falls past
`CookieFilterResourceHandler` to the network and is answered by MetaNet Client — a different wallet,
a different identity key, no Hodos gate applied, no indication to the user. On any machine with both
installed, this is a silent cross-wallet routing hole. Fixing the gate closes it.

**BRC-100 is a standard.** A wallet answering on the standard local port is implementing the
standard. Wallet-agnostic dApps are the entire point of the protocol, and `127.0.0.1` is the more
robust spelling of the address, not a quirk — see §4.

---

## 3. Root cause

`cef-native/src/handlers/simple_handler.cpp:8072-8078`:

```cpp
if (hodos::IsWalletHostPort(url) ||
    url.find("localhost:3321") != std::string::npos ||
    url.find("localhost:2121") != std::string::npos ||
    url.find("localhost:8080") != std::string::npos ||
    url.find("messagebox.babbage.systems") != std::string::npos ||
    url.find("/.well-known/auth") != std::string::npos) {
    LOG_DEBUG_BROWSER("🌐 Intercepting wallet request from browser role: " + role_);
    return new HttpRequestInterceptor();
}
```

`hodos::IsWalletHostPort()` (`cef-native/include/core/PortConfig.h:53-58`) checks **both** host forms
— but only for our own ports (31301 release / 31401 under `HODOS_DEV=1`). The three
MetaNet-compatibility ports are raw substring matches that hardcode the literal hostname
`localhost`.

The App Lab's entire transport:

```js
const HTTPS_BASE = 'https://127.0.0.1:2121'
const HTTP_BASE  = 'http://127.0.0.1:3321'
const ORIGINATOR = location.host
// probe:  GET <base>/health, header Access-Control-Request-Private-Network: true,
//         2400ms AbortController, requires response.ok
// detect: HTTPS_BASE then HTTP_BASE; 1 attempt on load, 12 @700ms on click
// call:   POST <base>/<methodName>, headers {Content-Type, originator}
```

`"https://127.0.0.1:2121".find("localhost:2121")` → `npos`. No term matches.

**Second, independent blocker.** `HttpRequestInterceptor::isWalletEndpoint()`
(`HttpRequestInterceptor.cpp:5068-5107`) is a 37-term substring allowlist with **no `/health`
entry** — and `GET <base>/health` is the App Lab's entire detection probe. A port-gate-only patch
ships and the board still reads "Bridge Unavailable". Both changes must land together.

---

## 4. `localhost` vs `127.0.0.1` — three real differences

| | `localhost` | `127.0.0.1` |
|---|---|---|
| Resolution | hostname, must be resolved | literal address, no lookup |
| IP family | on Windows often resolves to IPv6 `::1` **first** | always IPv4 loopback |
| Origin identity | `http://localhost:P` | a **different origin** from `http://127.0.0.1:P` |

The IP family difference is load-bearing: `netstat` confirms both MetaNet Client and our own Rust
wallet bind **IPv4 only** (`127.0.0.1:31301`, no `[::1]` listener). On a machine resolving `localhost`
to `::1`, the hostname form can resolve to an address where nothing listens. The IP literal always
hits.

**We already agree with this internally.** `CWIShimScript.h:258` sets
`ENDPOINT_BASE = 'http://127.0.0.1:31301'`, and `PortConfig.h:43` calls `127.0.0.1` "the canonical
one for outbound C++ calls". Our shim and our C++ use the IP form; only the compatibility gate uses
the hostname form. This is an internal inconsistency, not a disagreement with the dApp.

For CORS the two are distinct origins, which is why `rust-wallet/src/main.rs:923-926` enumerates all
four variants by hand.

---

## 5. Approach

**One structural predicate, replacing the substring gates.**

Add `cef-native/include/core/UrlMatch.h` with `hodos::ParseUrl()` (a `CefParseURL` wrapper producing
`UrlFields{scheme,host,port,path}`), `hodos::IsLoopbackHost()`, a compatibility-port table, and a
single `hodos::IsWalletOrigin()` predicate that becomes the only function permitted to answer "is
this wallet traffic".

`CefParseURL`/`CefURLParts` already ships in the vendored headers (`cef-binaries/include/cef_parser.h:60`)
and gives GURL-canonicalised fields on both platforms.

**Placement correction.** The first draft put `ParseUrl` in `PortConfig.h`. That file is included by
`SingleInstance.cpp` and `WalletService.cpp`, which are not CEF-API translation units — adding
`cef_parser.h` there couples the ports SSOT to libcef for everything that touches a port number, and
makes it un-unit-testable without libcef. Ports stay in `PortConfig.h`; parsing and origin matching
go in a sibling `core/UrlMatch.h`.

**"Add a fourth substring literal" is rejected, not deferred.** It is a smaller diff but strictly
worse behavior: it doubles the false-positive surface of a gate that already routes
`https://evil.com/?x=localhost:3321` into the wallet interceptor, and doubles the dev-server hijack
on 8080 by adding the second host form. The parsed gate is comparable new code and removes a defect
class instead of widening one.

### 5.1 The matcher must be BROADER on hosts, not narrower — critical design constraint

This is the single most important constraint and it is counter-intuitive.

A naive parsed matcher (`host == "localhost" || host == "127.0.0.1"`) is **narrower** than today's
substring match. `"http://x.localhost:31301/shutdown".find("localhost:31301")` **succeeds** today, so
that URL is currently intercepted. Chromium resolves `*.localhost` to loopback per RFC 6761, so the
request does reach the listener. Under a strict-equality parsed matcher it would **stop** being
intercepted — and per §6.1, un-intercepted means *internally trusted*.

**The structural fix, done naively, would introduce a privilege-escalation regression.**
`IsLoopbackHost()` must therefore accept everything Chromium will actually route to loopback:

- any parsed IPv4 in `127.0.0.0/8` (first octet 127)
- `[::1]`
- `localhost` **and any `*.localhost` suffix**

Then rely on a path deny-list (§5.2 W2') for safety, not on host narrowness.

### 5.2 Workstreams

| ID | Title | Size | Deps |
|---|---|---|---|
| W0 | `core/UrlMatch.h` — parsed fields, `IsLoopbackHost` (per §5.1), compat-port table, `IsWalletOrigin()` | S | — |
| W1 | Replace the six-term gate at `simple_handler.cpp:8072-8078` with one `IsWalletOrigin()` call; drop 8080 | S | W0 |
| **W2'** | Add `/health` to `isWalletEndpoint()` **only** — do *not* delete the allowlist (see §7.2) | S | W1 |
| W3 | Instrumentation for the four questions unsettleable from source | S | W0 |
| W4 | Header hygiene (forward-allowlist, not deny-list), then CORS `Allow-Headers` widening — one commit | S | W2' |
| W6 | Make an unresolvable origin fail closed instead of falling into internal trust | M | W3 |
| W7 | Migrate remaining substring gates to parsed origins — incl. the `:5137` frontend gate | L | W0, W1 |
| W8 | Retire `hodos::IsWalletHostPort()` once no caller remains | S | W2', W7 |

**Cut line.** The staff-engineer review is right that this is two releases, not one:

- **Release N:** W0 + W1 + W2' + W3 — fixes the user-visible bug, adds instrumentation.
- **Release N+1:** W4 + W6 + W7 + W8 — driven by what N's logs actually said.

W6 depends on W3's shadow log. If the dogfood cycle is real, W6 cannot be in the same release; if it
isn't real, W6's stated mitigation ("the mitigation is sequencing") evaporates. Pick one explicitly.

### 5.3 Port 8080 — remove it

Three of five specialists proposed keeping 8080 in both host forms. That is the worst available
change: a symmetric denial of service against the most common dev-server port, where the natural
workaround (switch to `127.0.0.1`) would also break.

Evidence for removal: 8080 appears in no BRC, no MetaNet client, and no App Lab path;
`git log -S 'localhost:8080'` traces it to a single "initial commit from old repo";
`archived-docs/doc-discrepancies.md:88` already flags the reference as stale; and
`ARCHITECTURE_TECHNICAL.md:65` reassigns `:8080` to the planned Dolphin Milk agent.

Remove it. Do **not** dress the removal as evidence-gathering — a `LOG_WARNING` in a local file that
nobody aggregates is not telemetry. If a warn-and-fall-through branch is kept at all, it needs the
path predicate (a bare port match logs every subresource of every dev-server page) and a per-session
rate limit.

### 5.4 Do NOT add `Access-Control-Allow-Private-Network`

Two specialists recommended it; the PNA specialist refuted them and wins. This build is Chromium
150.0.7871.187 (`cef-binaries/include/cef_version.h:38`). PNA was superseded by **Local Network
Access**, enforced from Chrome 142, and LNA has **no server-side opt-in header** — the Intent to Ship
states it "does not require or support server-side headers for opting in". CEF's own CORS check on
resource-handler responses reads only `Access-Control-Allow-Origin` and
`Access-Control-Allow-Credentials`. MetaNet Client emitting `access-control-allow-private-network:
true` is PNA-era residue and is not why it works.

Adding it would leave a header in the tree that a future reader concludes is load-bearing. (Same
argument applies to admitting `ws`/`wss` to `IsWalletOrigin()` — WebSocket handshakes never reach
`GetResourceHandler`, so those scheme literals would be inert code that reads as coverage.)

---

## 6. Security review

### 6.1 Un-intercepted loopback traffic is *internally trusted*, not merely ungated

**Verified in this session.** `rust-wallet/src/main.rs:59-101`, `domain_trust_mw`:

```rust
// Internal call → no gate.
let domain = match domain {
    Some(d) => d,
    None => return Ok(next.call(req).await?.map_into_boxed_body()),
};
```

`X-Requesting-Domain` is injected by the C++ layer **only** for external origins. Its absence is read
as "the wallet UI is calling its own backend". So any request the C++ layer fails to intercept
reaches Rust as a fully-trusted internal call with no gate at all.

This is why §5.1 matters, and why "the fix widens the attack surface" is the wrong frame. Interception
*narrows* it: intercepted traffic is stamped and gated, un-intercepted traffic is trusted.

### 6.2 `actix-cors` is cosmetic for non-preflighted requests

**Verified in this session.** `rust-wallet/src/main.rs:922-929` has no
`.block_on_origin_mismatch(true)`. Without it, actix-cors omits the `ACAO` header on an origin
mismatch — which stops the *browser* reading the response, but the handler still **executes**. A
fire-and-forget `fetch(..., {mode:'no-cors'})` therefore takes effect server-side.

The first-draft plan cited this CORS config as the remaining backstop on the service-worker path and
forbade changing it. Both positions were wrong for the same reason. Adding
`.block_on_origin_mismatch(true)` is one line and does not break the intercepted path.

### 6.3 DNS rebinding bypasses the C++ layer entirely

A page served from `http://rebind.attacker.tld:31301` whose DNS re-resolves to `127.0.0.1` issues
**same-origin** requests — no `Origin` header, actix-cors inert, host not matched by any gate, so no
`X-Requesting-Domain`, so §6.1 grants internal trust. `127.0.0.1` is immune to rebinding; the
hostname surface is not.

Mitigation is server-side and cheap: a `Host`-header allowlist middleware in `rust-wallet` rejecting
any request whose `Host` is not exactly `127.0.0.1:<port>` or `localhost:<port>`. This belongs with
W6, which currently claims this ground while fixing only the intercepted half.

### 6.4 Other findings

- **`extractDomain()` discards the scheme** (`HttpRequestInterceptor.cpp:5121-5163` — it takes the
  substring between `://` and the next `/`). A grant given over HTTPS is spendable by the same host
  over plaintext HTTP.
- **`extractDomain()` reads the main-frame URL** (`:5125`), so a third-party ad iframe spends under
  the parent page's grants. Correct to defer to `request_initiator`, but it should be written down.
- **Synthesized responses hardcode `200`** (`HttpRequestInterceptor.cpp:1414-1418`) alongside
  `ACAO: *`. With a uniform 200 and permissive CORS, an origin can enumerate wallet state by reading
  bodies.
- **Header hygiene must be a forward-allowlist, not a deny-list.** `HttpRequestInterceptor.cpp:3653-3665`
  merges every page header into the outbound map; today's protection is an accident of multimap key
  ordering. A deny-list leaves the next engineer to remember a function in a different part of the
  file, with no compiler help.
- **8080 dev-server hijack is live today** for the `localhost` spelling: a dev server's
  `/socket.io/`, `/wallet/`, `/encrypt` paths are intercepted, while the identical `127.0.0.1:8080`
  server is untouched.

---

## 7. Findings that outrank this ticket

These are **pre-existing**, independent of the change, and were found while researching it.

### 7.1 🔴 The money path takes no request context — CONFIRMED

**Verified in this session.**

```rust
// rust-wallet/src/handlers.rs:9612
pub async fn send_transaction(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> HttpResponse {
    ...
    let send_max = req.send_max.unwrap_or(false);   // :9630
```

`send_transaction` takes **no `HttpRequest`**. It therefore cannot consult `X-User-Approved` or any
per-call approval, and the per-transaction caps that guard the `createAction` path are not in this
path. It honours `sendMax`. Its only gate is `domain_trust_mw`.

`reveal_mnemonic` (`handlers.rs:18799`) has the same shape — no `HttpRequest`, PIN-only, no visible
rate limit: an unthrottled PIN oracle against the seed phrase.

Combined with §6.1, an un-intercepted request to `/transaction/send` with `{"sendMax":true}` is
treated as internal and trusted. Combined with a single ordinary connect approval, the red-team
review traces the same call to a full-balance sweep that bypasses every payment cap, session limit
and modal.

**This should be its own ticket and it should be scheduled ahead of the compatibility fix.** Adding
`http_req: HttpRequest` and routing through the same `dispatch_payment` path `create_action` uses is
described as a two-line change.

### 7.2 🔴 Why W2 was cut down to W2'

The first-draft plan proposed deleting the 37-term path allowlist entirely, on the reasoning that
"those origins *are* the wallet". All three reviewers independently flagged this as **critical**: the
wallet's own privilege-granting control plane lives on the same origin.

`POST /domain/permissions` (`handlers.rs:10038`) is on the intercepted origin. Deleting the allowlist
exposes it to any origin the user has approved **once** — letting a page rewrite its own
`trust_level`, `per_tx_limit_cents` and `rate_limit_per_min`. Every downstream control in the plan is
enforced against rows the attacker could then edit. `/shutdown` is likewise on that origin.

**Revised to W2': add `/health` and nothing else.** If the allowlist is ever deleted, it must be
paired with a C++ deny-list on administrative path prefixes for external origins.

### 7.3 Agent-reported, NOT independently verified

Recorded so they can be triaged, with confidence labelled honestly. Each is an agent claim with a
file:line I have not re-checked myself:

- `https://evil.com/backup?x=127.0.0.1:5137` reportedly serves the genuine Hodos wallet UI from disk
  under the attacker's origin, including the seed-phrase route. Reported as the single most severe
  finding in the audit. **Verify first.**
- The inverse of the same gate: the real wallet UI embeddable in an attacker's iframe. W7 should
  require a main-frame load (`is_navigation`, `frame->IsMain()`).
- `listCertificates` (`certificate_handlers.rs:288`) reportedly returns all certificate field values
  **decrypted to plaintext** with no per-handler gate.
- `W7`'s renderer-process problem: `cef_browser_shell.cpp:4517-4523` reportedly states verbatim that
  "a sandboxed child does not see `HODOS_DEV`", which would make any `PortConfig.h` include from a
  renderer TU a silent no-op. If true, the browser process must pass the resolved port over IPC.
- BRC-99: `validate_and_normalize_basket_name` (`basket_repo.rs:62-66`) implements the reserved `p `
  namespace rule but is reportedly wired only into the write path — so `listOutputs` on
  `p unknown-scheme` would not refuse. That is a spec violation against Cryderman's own BRC-99 card.
- `listOutputs` reportedly calls `find_or_insert`, so any read on an unknown basket creates a row —
  letting a dApp pollute `output_baskets` by enumeration.

---

## 8. Test plan

**Settleable by reading — done:**
- gate semantics and the missing IP form (§3)
- `domain_trust_mw` internal-trust behaviour (§6.1)
- CORS config lacking `block_on_origin_mismatch` (§6.2)
- `send_transaction` / `reveal_mnemonic` signatures (§7.1)
- route inventory: 110 routes / 109 handlers, all 28 canonical BRC-100 methods present

**Requires running the browser — cannot be settled from source:**
1. **CEF https interception.** Does returning a `CefResourceHandler` for `https://127.0.0.1:2121`
   short-circuit before TLS, or does cert validation fire first?
   `cef-binaries/tests/ceftests/cors_unittest.cc` reportedly serves https from `GetResourceHandler`
   with no server, which is encouraging but has never been exercised in this codebase.
   *Fallback if it misbehaves: deliberately do not match 2121, so the probe fails fast and falls
   through to 3321.*
2. **The Chromium Local Network Access prompt.** Does it still fire when the request is served by a
   resource handler and never touches the network? If it does, the fix needs a second moving part.
3. **Does a CORS preflight reach `GetResourceHandler`?** The two specialists disagreed and the
   dissenting evidence was confounded (the "working today" path went over
   `window.__hodos_walletCall` IPC per `CWIShimScript.h:301-306`, so it never issued a fetch and never
   generated a preflight). Instrument at `HttpRequestInterceptor.cpp:3720` and look for OPTIONS.
4. **Merge gate for W1/W2'.** From a public https origin, `POST http://127.0.0.1:3321/createAction`
   on a domain with no prior grant must produce a 202 domain-approval envelope, and the Rust log must
   show `X-Requesting-Domain: <exact page host>`. This verifies the trade the sprint is making —
   giving up Chromium's coarse loopback prompt in exchange for our fine-grained engine.
5. **No-regression:** existing dApps on `localhost:3321`; `localhost:8080` and `127.0.0.1:8080` dev
   servers load byte-identically to a non-Hodos browser (a synthesized 404 also "loads" — compare
   bodies); BRC-104 handshake against the local wallet.
6. **macOS.** The plan has no macOS acceptance criteria, on a sprint whose own `SPRINT_PLAN.md` §5
   says in bold "Do not hold all Mac work to the end". W7 needs explicit mac coverage: wallet,
   wallet_panel, settings and backup overlays reaching the Rust wallet before and after.
7. **Perf.** Every request now pays a `CefParseURL` on the IO thread, where today it pays one to six
   allocation-free `find()` calls. Add a cheap prefilter (bail unless the spec starts `http` and
   contains `127.0.0.1` or `localhost`), parse once, and thread the `UrlFields` through rather than
   re-parsing. This file already carries scar tissue from a perf incident on this exact path.

---

## 9. Predicted App Lab scoreboard once reachable

The board is 36 cards over 30 distinct wire methods across 16 BRCs. 28 of the 30 are the canonical
`@bsv/sdk` `WalletInterface`; all 28 are registered in `main.rs:961-1134`.

| Outcome | Count | Notes |
|---|---|---|
| Pass / clean validation error | ~26 | Many args are deliberately empty; a clean 400 is a conformance pass |
| `404` — correct | 2 | `getBalance`, `getClaimedCloudHandle` — neither is BRC-100 |
| Known defects | 2+ | `getHeaderForHeight {height:0}` → HTTP 500 (genesis has no `previousblockhash`; `height:1` works). BRC-99 `p unknown-scheme` likely not refused (§7.3) |
| Unverified semantics | 4 | BRC-147 / 150 / 165 / 99 basket-profile behaviour |

**Safety note for whoever runs the board:** `createAction`, `signAction`, `internalizeAction`,
`relinquishOutput`, `relinquishCertificate` and `acquireCertificate` are fund-moving or
state-mutating. Run against an unfunded test wallet, not a funded one.

Also worth reporting upstream to Cryderman as a friendly opener: his BRC-42 card runs
`getPublicKey({identityKey:true})`, which performs no BKDS derivation at all — the derivation happens
on the card he labels BRC-43. And his `/health` probe is not a BRC-100 route, which is why the
reference wallet (MetaNet Client) fails his detector while answering every real method.

---

## 10. Out of scope — file separately

- `extractDomain()` → `request_initiator` migration (iframe spending under parent grants)
- BRC-99 reserved `p ` namespace on the read path
- `listOutputs` calling `find_or_insert` (basket pollution by enumeration)
- `counterparty` as a required serde field on the four crypto handlers when BRC-42/43 default it
- `getHeaderForHeight {height:0}` genesis 500
- `listCertificates` plaintext field disclosure (§7.3)
- DNT / `Sec-GPC` injection at `simple_handler.cpp:8017-8019` — `cef_request_handler.h:128` documents
  the request as unmodifiable there, meaning a user-facing privacy toggle may be inert
- `/socket.io/` routing (no socket.io route exists in Rust; every match resolves to a local 404)
- `CefRequestContextHandler` for the service-worker interception gap
- `ParsedTransaction::from_bytes` allocation in `beef.rs:963-966`, `:993-995`

---

## 11. Open questions

| Question | How to settle |
|---|---|
| Does a resource handler take over `https://` loopback pre-TLS? | Build W0+W1 with 2121 matched; load App Lab; watch for cert interstitial vs. synthesized response |
| Does the Chromium LNA prompt survive interception? | Same build; observe whether the prompt appears once traffic is intercepted |
| Does a preflight reach `GetResourceHandler`? | Instrument `HttpRequestInterceptor.cpp:3720`; look for OPTIONS in one App Lab session |
| Can `hodos_tests` link libcef so `UrlMatch.h` is unit-testable? | Check `cef-native/tests/CMakeLists.txt`; if not, make `ParseUrl` a pure-string function with an injectable parser |
| Is the `:5137` wallet-UI-under-attacker-origin claim real? | Reproduce `https://evil.com/backup?x=127.0.0.1:5137` before acting on §7.3 |
| Does `PortConfig.h` work in the renderer? | Read `cef_browser_shell.cpp:4517-4523` and test `HODOS_DEV=1` shim output |

---

## 12. Sprint fit — the honest answer

The staff-engineer review says **wrong sprint**, and the reasoning is sound.
`development-docs/0.4.0-beta.3/SPRINT_PLAN.md` §4 fixes the running order with Phase 0 being the
stray `{app}` log writes that can silently abort auto-update. beta.3 is the build users get. This
ticket proposes rewriting the routing predicate for every network request in the browser and
touching an internal-trust boundary.

Recommendation:

- **§7.1 (money path) — schedule now, ahead of this ticket.** It is small, it is live, and it does
  not depend on any of the routing work.
- **W0 + W1 + W2' — beta.3 tail item**, behind the existing release-integrity order, only if the cut
  line still has room. This is the piece that fixes the user-visible bug and closes the cross-wallet
  routing hole.
- **W4 / W6 / W7 / W8 — beta.4**, driven by W3's instrumentation.

Do not take the whole plan into beta.3.

---

## 13. Verification log — 2026-08-18, beta.3 kickoff session

Every claim below was re-checked against the tree by reading the cited code, not by trusting the
draft. **Status changed 🔵 PROPOSED → 🟢 SCHEDULED**, split across two phases — see
`SPRINT_PLAN.md` §3 (WS5) and §4.

### ✅ Confirmed verbatim

| Claim | Cited | Result |
|---|---|---|
| §3 six-term gate | `simple_handler.cpp:8072-8078` | exact |
| §3 `IsWalletHostPort` checks both forms, our port only | `PortConfig.h:53-58` | exact |
| §3 `isWalletEndpoint` has no `/health` | `HttpRequestInterceptor.cpp:5068` | exact (**38** terms, not 37 — immaterial) |
| §4 `127.0.0.1` is "the canonical one for outbound C++ calls" | `PortConfig.h:43` | exact |
| §4 CORS enumerates four origin variants by hand | `main.rs:922-930` | exact (ticket said `:923-926`) |
| §6.1 **absent `X-Requesting-Domain` ⇒ no gate at all** | `main.rs:65-75` | exact, verbatim |
| §6.2 no `block_on_origin_mismatch` anywhere | `main.rs:922-930` | exact; also `allow_any_method()` + `allow_any_header()` |
| §7.1 `send_transaction(state, body)` — no `HttpRequest` | `handlers.rs:9612` | exact |
| §7.2 `set_domain_permission(state, web::Json<…>)` | `handlers.rs:10038`, routed `main.rs:1077` | exact — also takes no `HttpRequest` |
| §2 cross-wallet routing hole | `netstat` | **live**: PID 37360 `LISTENING` on `127.0.0.1:3321` **and** `:2121`; our wallet on `:31301`, IPv4 only, no `[::1]` — confirming §4's IP-family point |

⭐ **§7.1 is stronger than written.** `send_transaction` spans `handlers.rs:9612-9951` and contains
**zero** occurrences of `permission`, `dispatch`, `approval`, `check_domain_approved` or
`X-User-Approved`. `dispatch_payment` is called at `handlers.rs:4320`, `:10951`, `:17625` and
`certificate_handlers.rs:738` — never here. The endpoint's only gate is `domain_trust_mw`.

⭐ **§5.1 is the best analysis in the document** and should survive any rewrite. Verified: a naive
strict-equality matcher **would** stop intercepting `x.localhost:31301`, which today's substring gate
does intercept — and per §6.1, no longer intercepted means *more* trusted. Keep it prominent.

### ⚠️ Corrected in place — not deleted, because both readings are natural

**§7.2's framing is inverted.** The ticket argues that *deleting* the allowlist would expose
`POST /domain/permissions`. Verified: `/domain/permissions` is **not in the allowlist today**, so it
is **never intercepted** → reaches Rust with no `X-Requesting-Domain` → §6.1 grants it internal
trust. Being *outside* the allowlist is the exposure; being inside it is what gets you gated.

⭐ Two mitigations the ticket does not credit, and they change the priority:
- `set_domain_permission` takes `web::Json`, which requires `Content-Type: application/json`, which
  forces a CORS **preflight**, which actix-cors fails for a foreign origin. The browser blocks it.
- The exposed shape is a handler taking `web::Bytes` (no content-type check), reachable as a *simple*
  request with `text/plain` and therefore no preflight — which is exactly `send_transaction`'s
  signature. §6.2 confirms the handler still executes when ACAO is withheld.

⇒ Revised conclusion: **keep W2' as written, and treat "un-intercepted ⇒ fail closed" (W6) as the
real fix.** `block_on_origin_mismatch(true)` is worth more than the ticket implies.

**§5.4 is right, and it is also load-bearing in a way the security section misses.** Chromium 150
enforces Local Network Access, so a *public* page cannot reach `127.0.0.1` without the user clearing
an LNA prompt. That materially bounds §6.1's and §6.3's real-world severity today. ⚠️ It is a control
we do **not** own — it moved once already (PNA → LNA) and can move again — so it belongs in the
threat model as a mitigating fact with an expiry, never as a reason not to fix W6.

### 🚨 Escalated — §7.3's first bullet is real, and worse than filed

The ticket lists this as agent-reported and says *"Verify first."* Verified, and it is **three** gates
across two files, all plain `find(...) != npos` substring matches on the **whole URL**:

| Site | Gate | What it admits |
|---|---|---|
| `simple_handler.cpp:7962` | `url.find("127.0.0.1:5137") != npos` | returns `LocalFileResourceRequestHandler` — serves the genuine Hodos frontend **from disk** under the requesting origin |
| `simple_render_process_handler.cpp:539` | `isOverlayBrowser` | overlay-privileged branch |
| `simple_render_process_handler.cpp:541` | `isInternalPage` | **the privileged V8 surface** |

`isInternalPage \|\| isOverlayBrowser` gates injection of `hodosBrowser.identity` (`get`,
`markBackedUp`), `hodosBrowser.navigation`, the history object, and `WALLET_CALL_BRIDGE_SCRIPT`
(`:575-721`, `:772`). The code's own comment at `:721` states the intent —
*"external pages only get BRC-100 + cefMessage"* — and the gate does not implement it. By contrast
the external path (`:777-788`) is properly hardened: main-frame only, `https://` only, shim not
bridge. ⇒ `https://evil.com/x?y=127.0.0.1:5137` matches all three.

⭐ **The backstop holds, and this is why it is a defence-in-depth failure rather than an incident.**
The browser-process `wallet_call` handler (`simple_handler.cpp:2037-2049`) **re-derives** the origin
from `frame->GetURL()` on the trusted side and routes through `IsInternalOrigin` + the permission
gate. It does not trust the renderer's claim. So the money path retains a real gate.

⛔ **But the hinge is a known-open defect.** `IsInternalOrigin("")` returns **`true`**
(`HttpRequestInterceptor.cpp:1016`) — carried open since the 2026-08-03 docs audit. Any frame URL
without `://` collapses to internal trust. That item is no longer a tidy-up; it is now load-bearing
for this one. *(The rest of that function is sound — `matchesHostOrHostColon` requires an exact host
or a `:` boundary, so `127.0.0.1.evil.com` is correctly refused.)*

⭐ **Reuse anchor — the correct predicate already exists.** `IsInternalFrontendUrl()`
(`simple_handler.cpp:137-142`) does `rfind(prefix, 0) == 0` on both host forms plus `hodos://`.
Three gates should be calling it. It lives in the browser process, so the render-process pair needs
either a shared header or the same four-line predicate — **not** a fourth spelling.

⛔ **Not reproduced.** Load `https://<any-https-origin>/?x=127.0.0.1:5137` in a dev build and check
(a) whether frontend files are served, and (b) whether `window.hodosBrowser.identity` is defined.
Negative control: the same page **without** the substring must get neither. Until that runs, this is
a code reading, not a finding.

### Sprint fit — §12's recommendation is adopted, with one change

§12 says schedule §7.1 ahead of the compatibility fix and hold W4/W6/W7/W8 for beta.4. Agreed, and
implemented as WS5 in `SPRINT_PLAN.md`. **The change:** §7.1 does not merely go "ahead" — it is
folded, with §6.2 and the three `:5137` gates, into a new **Phase 0.5**, because all three are small,
independent of the routing rewrite, and live on or beside the money path. Nothing in Phase 0.5
depends on W0.
