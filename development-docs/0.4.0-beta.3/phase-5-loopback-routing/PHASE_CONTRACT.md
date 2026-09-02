# Phase 5 — loopback routing & trust boundary · PHASE CONTRACT

**Workstream:** WS5(b) · **Opened:** 2026-09-01 · **Status:** 🟢 **LANDED 2026-09-02** — `c4603e1` contract · `1743b20` fix · `36f7a66` gate `G12`. All 9 evidence rows resolved; `R-INTEXT`’s injected RED run in both directions for the first time this sprint.
**Was:** 🟢 **CONFIRMED 2026-09-02** — all five open questions decided by the owner (§6). §4 rows `A1`/`A2` **GREEN**, `A4` RED **captured pre-fix** — see `MEASUREMENTS.md`
**Owner:** Matthew · **Platform:** Windows (macOS relayed, not claimed)
**Standard:** `../HARNESS.md`. Inherits the beta.3 harness in full.
**Ticket:** `../TICKET_loopback_host_form_wallet_routing.md` (W0 · W1 · W2' · W3 of §5.2)

---

## 0. What the kickoff changed, before anything was built

⛔ **The plan's scope is wrong in both directions.** Two items are already done; three items the plan
does not name are the actual work. Recorded here per `HARNESS.md` §1, and in the same commit as the
contract.

### 0.1 Already done — delete from scope

| `SPRINT_PLAN.md` §WS5(b) / prompt §3 says | 📏 Measured in the tree, 2026-09-01 |
|---|---|
| *"`/health` added to `isWalletEndpoint`"* | ✅ **ALREADY THERE.** `HttpRequestInterceptor.cpp :: isWalletEndpoint` first arm, host-scoped via `hodos::IsWalletHostPort`, with its own comment. **W2' is a verification row, not a code row.** |
| *"the six-term substring gate"* replaced by `IsWalletOrigin()` | 🟡 **Half done, and the half that landed is not the half that matters** — see 0.2. `simple_handler.cpp :: GetResourceRequestHandler` now reads `IsWalletHostPort ‖ IsLoopbackHostPort(url,"3321") ‖ (…,"2121") ‖ (…,"8080") ‖ find(messagebox…) ‖ find(/.well-known/auth)`. The bare `localhost:3321` literals are gone. |

### 0.2 🚨 The prompt's own delta table is itself under-measured

Prompt §3 records the live remnant as *"`url.find("messagebox.babbage.systems")` and
`url.find("/.well-known/auth")` — unanchored"*. 📏 **That is 2 of 7.**

`hodos::IsLoopbackHostPort` (`PortConfig.h`) is:

```cpp
return url.find("localhost:" + port) != npos || url.find("127.0.0.1:" + port) != npos;
```

— a **whole-URL substring search**, exactly like the literals it replaced. Phase 0.5 fixed the
*host-form* gap (`127.0.0.1` was invisible); it did **not** touch the *unanchored* defect class.
`hodos::IsWalletHostPort` is the same shape. So:

| # | Site | Shape | Named in the plan? |
|---|---|---|---|
| 1 | `IsWalletHostPort(url)` | whole-URL substring | ⛔ no |
| 2–4 | `IsLoopbackHostPort(url, "3321"/"2121"/"8080")` | whole-URL substring | ⛔ no |
| 5 | `find("messagebox.babbage.systems")` | whole-URL substring | ✅ yes |
| 6 | `find("/.well-known/auth")` | whole-URL substring | ✅ yes |
| **7** | **`redirectPort`** (`HttpRequestInterceptor.cpp:3869-3884`) | whole-URL **rewrite** | ⛔ **no — and it is the load-bearing one** |

⭐ **`redirectPort` is the discovery of this kickoff.** It rewrites the *first* `localhost:<digits>` or
`127.0.0.1:<digits>` **anywhere in the URL — query string included** — to our wallet port, and it runs
*before* every downstream predicate. Two consequences, opposite in sign:

- 🚨 It can **manufacture** the substring `IsWalletHostPort` then tests for. A page's own
  `fetch('/health?x=127.0.0.1:3321')` enters the interceptor on arm 3, has its query rewritten to
  `…:31401`, and now satisfies the `/health` arm's "scoped to our own host:port" guard. Adjudicated in
  `../phase-0.5-money-path/ADVERSARIAL_PANEL_2_2026-08-20.md` findings #28/#34 as real, pre-existing
  and low-payload (`/health` returns three constants) — but the *shape* is what this phase exists to
  remove. **An `IsWalletOrigin()` that leaves `redirectPort` unanchored is decorative.**
- ✅ It is also **why the App Lab probe works at all.** `http://127.0.0.1:3321/health` is rewritten to
  `…:31401/health` *before* `isWalletEndpoint` runs, so the host-scoped `/health` arm fires for the
  compat ports. ⚠️ I initially read the `/health` arm as scoped to our own port and therefore missed
  by the App Lab's probe, and wrote that down as a finding. It is **wrong**, and reading
  `redirectPort` is what corrected it. Recorded because the wrong reading is the natural one.

### 0.3 The ticket's own W0 design is refuted by measurement

Ticket §5 specifies W0 as a **new `cef-native/include/core/UrlMatch.h` wrapping `CefParseURL`**, placed
outside `PortConfig.h` so the ports SSOT does not couple to libcef and stays unit-testable.

| Ticket §11 open question | 📏 Settled 2026-09-01 |
|---|---|
| *"Can `hodos_tests` link libcef so `UrlMatch.h` is unit-testable?"* | ⛔ **No.** `cef-native/tests/CMakeLists.txt` links gtest, a named list of production `.cpp`, `nlohmann_json`, OpenSSL and platform HTTP/crypto libs. **libcef appears nowhere.** A `CefParseURL` wrapper would be untestable — the exact outcome §5's "placement correction" was written to avoid. |

⭐ **Reuse anchor — the parser the ticket asked for already exists, and is already tested.** Phase 0.5
built a pure-string, scheme-anchored, userinfo-stripping derivation *in `PortConfig.h` itself*:

- `hodos::OriginFromUrl(url)` → authority, or empty (fails closed on `data:`, `about:blank`, userinfo)
- `hodos::AuthorityHasHost(authority, host)` → exact-or-`host:` termination
- covered by **59 assertions** in `cef-native/tests/port_config_origin_test.cpp`

⇒ **W0 becomes: build `IsWalletOrigin()` on those two. No new header, no `CefParseURL`, no libcef.**
📏 And note what is *not* covered: **no test anywhere touches `IsWalletHostPort` or
`IsLoopbackHostPort`** — the two predicates this phase rewrites.

### 0.4 The gate registry does not see any of this

📏 `G2` (`scripts/preflight.ps1`) is `(find|rfind)\("(http://)?(127\.0\.0\.1|localhost):5137` — the
**frontend** port only. None of the seven sites above are visible to any T0 gate. See §6-OQ4.

---

## 1. Goal

A dApp that addresses the wallet by any spelling Chromium routes to loopback reaches **our** wallet,
gated; and a URL that merely *contains* a loopback host:port in text the page controls does not.

## 2. Done means

1. §4 of the session prompt is **settled by measurement with a named negative control** — either the
   cross-wallet routing hole is closed (and we say so with evidence) or it is live (and we fix it).
2. One predicate, `hodos::IsWalletOrigin()`, is the only function permitted to answer *"is this wallet
   traffic"* at the resource-dispatch gate — built on the existing tested primitives, unit-tested, and
   **broader on hosts than today** per ticket §5.1.
3. `redirectPort` operates on the **authority**, not the whole URL string.
4. No URL is *newly* un-intercepted without that having been observed in the shadow log (§4 row `A6`)
   and named — because per the sprint's load-bearing insight, un-intercepted means **trusted**.
5. `R-INTEXT`'s injected RED is run in **both** directions, for the first time in this sprint.

## 3. Invariants preserved

| From `REGRESSION_SET.md` | Why it is at risk here |
|---|---|
| **`R-INTEXT`** ⭐ | This phase rewrites the predicate that decides whether `X-Requesting-Domain` is ever attached. It is the check most directly on the line. |
| `R-GOLD` | The gold pill fires from `OnWalletCallSuccess` inside the interceptor. A URL that stops being intercepted stops producing a pill for a payment that still happens. |
| `R-PERIM` | Perimeter decisions are Rust-side and keyed on the header this phase controls. |
| `R-CLOSE`, `R-COUNT`, `R-UPDATE` | Not touched; run at the boundary regardless. |

## 4. Evidence table

⛔ Every row's RED is *"observed to fail"*, not *"would fail"*. Rows are ordered so `A1` runs first —
it decides whether the rest of the phase is urgent or tidy-up.

| ID | 🟢 GREEN | 🔴 RED | 🎯 SUBJECT | Tier |
|---|---|---|---|---|
| **P5-A1** 🟢 **GREEN 2026-09-02** (M1) | From a real external https page in the dev browser, `fetch('http://127.0.0.1:3321/getVersion')` **and** the `https://127.0.0.1:2121` form are answered by **our** wallet | ⭐ **Replaced with a stronger, free control** (M1): probe `127.0.0.1:3322` — a loopback port one digit off 3321, not in the gate list. Observed: **no interception, zero Rust lines**, client `Failed to fetch`. ⇒ the gate is the cause. ⛔ MetaNet Client was **not** stopped and did not need to be | **Our Rust log** — `domain_trust_mw`'s `requesting_domain=<exact page host>` line for `/getVersion`. ⛔ Not the C++ log, not the page's console: the C++ log records intent, the page cannot tell which wallet answered | T2 |
| **P5-A2** 🟢 **GREEN 2026-09-02** (M1/M3) | `https://127.0.0.1:2121/health` is served by our resource handler with no cert interstitial | Ticket §8.1's fallback is the RED's consequence, not a control: if TLS fires first, **stop matching 2121** and record it | The rendered page / DevTools network entry for the *2121* request specifically, not the 3321 retry that follows it | T2 |
| **P5-A3** 🟢 **GREEN** (32 unit tests) | `IsWalletOrigin()` matches `127.0.0.0/8`, `[::1]`, `localhost` **and `*.localhost`**, on our port and each compat port | Unit test: with the `*.localhost` arm removed, `http://x.localhost:31401/createAction` must go **false** — and that is a privilege escalation, not a tidy-up (§5.1) | `hodos_tests` — a new `wallet_origin_test.cpp` beside `port_config_origin_test.cpp`. No libcef (§0.3) | T1 |
| **P5-A4** 🔴 **RED CAPTURED 2026-09-02** (M2) | `https://evil.example/?x=127.0.0.1:31401` and `…/health?x=127.0.0.1:3321` are **not** wallet traffic | Same URLs against the pre-change predicate return **true** — asserted in the same test file, so the fix's own control ships with it | `hodos_tests` assertions, plus one live fetch whose response body is the *site's*, not `{"backend":"rust-wallet"}` | T1 + T2 |
| **P5-A5** 🟢 **GREEN** (M2 pre-fix / M8 post-fix) | `redirectPort` rewrites only the authority | RED: with the anchor removed, `?x=127.0.0.1:3321` in a query is rewritten to `:31401` — observed in the interceptor log line the lambda already emits | The rewritten `url` string in `🌐 Port redirection:`, not the request object (`SetURL` in `GetResourceHandler` is documented inert — panel #2 finding, `cef_resource_request_handler.h`) | T1 + T2 |
| **P5-A6** 🟢 **GREEN** (M10 — 1 disagreement, the intended one) | **W3 shadow log.** Old and new predicates both evaluated; every **disagreement** logged (URL host:port + which side said yes) with the old one still deciding. After a Standard test basket + App Lab + a known dApp, the disagreement set is empty or every entry is named in §6 | RED: inject a URL known to disagree (`https://example.com/?x=127.0.0.1:31401`) and confirm it **appears** in the shadow log. A shadow log that never fires is the farbling-harness failure shape | The **disagreement** count, not the match count. `LogSafeUrl` applies — host only, never path or query | T2 |
| **P5-A7** 🟢 **GREEN** (M8) | `/health` on our port and on 3321/2121 reaches the wallet; `/health` on an ordinary site does not | RED: `https://example.com/health` returns the *site's* body. This is the arm's stated purpose and it has never been observed | Response **body**, compared against `curl` outside the browser. A synthesized 404 also "loads" | T2 |
| **P5-A8** 🔴 **RED OBSERVED** (M11) | **`R-INTEXT` RED, internal-as-external.** Stub the origin derivation to always emit `X-Requesting-Domain` → the user's own send from the wallet UI **starts prompting** | This row *is* a RED. Its GREEN is the standing `R-INTEXT` (a) | Rust log: header **present** where it must be absent | T2 |
| **P5-A9** 🔴 **RED OBSERVED** (M11) | **`R-INTEXT` RED, external-as-internal.** Suppress the header → an external over-cap send **goes silent** | This row *is* a RED. Its GREEN is the standing `R-INTEXT` (b) | Rust log: header **absent** where it must be present, and **no** 202 | T2 |
| **P5-B1** 🟡 **RELAYED** | 🍎 macOS relay entry written to `MAC_RELAY_*`, naming `A1`/`A2` as the two Mac-shaped unknowns | n/a — relay, never claimed | `MAC_RELAY_P5_ROUND.md` | — |

⬜ **Boundary regression 4 → 5** — `R-INTEXT` (with its RED, finally), `R-PERIM` T1, `R-UPDATE` T1;
`R-GOLD` / `R-COUNT` / `R-CLOSE` owed as at every prior boundary unless a real payment happens.
⬜ `pwsh scripts/preflight.ps1` + `-NegativeControl`.

## 5. Blast radius — what this touches that it is not about

| Site | Exposure |
|---|---|
| `simple_handler.cpp :: GetResourceRequestHandler` (~9074) | The gate itself. **Every network request in the browser passes through it.** |
| `simple_handler.cpp` ~8959 — trusted-overlay direct bypass | ⚠️ **Dual use.** The same `IsWalletHostPort` is used here to *grant privilege* (skip all handlers for overlay roles). Widening the predicate for interception must not silently widen this. ⭐ **The two directions are opposite: the untrusted-stamping gate must be BROAD, the privilege-granting gate must be NARROW.** They are different predicates and this contract keeps them different. |
| `HttpRequestInterceptor.cpp` — `redirectPort`, https downgrade, `/health` arm, `isSocketIOConnection` | All four consume `IsWalletHostPort`. |
| `IsInternalOrigin("")` → **`true`** (`HttpRequestInterceptor.cpp:1070`, verified) | ⚠️ Named in the prompt §6 and confirmed. A frame URL with no `://` collapses to internal. This phase does **not** fix it (that is W6/beta.4) but works adjacent to it; `A8`/`A9` are the rows that would surface a regression. |
| `extractDomain()` reads the **main-frame** URL | Unchanged. An iframe still spends under the parent's grants (ticket §10, beta.4). |
| `PaidContentCache` skip test (`simple_handler.cpp` ~8977) | Another unanchored loopback substring. ⛔ Out of scope — reported, not fixed (working rule #3). |
| `CookieBlockManager` / `EphemeralCookieManager` | 14 more unanchored `find("localhost")` / `find("127.0.0.1")` sites (12 + 2). ⛔ Out of scope, W7/beta.4. |
| `TabManager.cpp:177`, `TabManager_mac.mm:191` | The two named `G2` residuals. ⛔ Unchanged, W7/beta.4. |

## 6. Out of scope

⛔ **W4 / W6 / W7 / W8 are beta.4** — `SPRINT_PLAN.md` is explicit. Specifically **not** in this phase:
header forward-allowlist, `block_on_origin_mismatch` follow-ups, `IsInternalOrigin("")`, the `:5137`
gate migration, retiring `IsWalletHostPort`, the cookie-manager substring family, `extractDomain` →
`request_initiator`. Also out: `TICKET_wallet_backend_death_is_silent_and_unrecovered` and
`TICKET_token_outputs_destroyed_by_dust_paths` (both Phase 8, owner-decided).

### 👤 Open questions — **ALL FIVE DECIDED by the owner, 2026-09-02**

Per prompt §11: *"Ask before narrowing a predicate."* Each of these is a narrowing. The decisions:

| # | 👤 Decision | Note |
|---|---|---|
| **OQ1** | ✅ **Drop 8080.** | Owner: *"what are we even using it for — yes, drop it"*. 📏 Answer: nothing. Traced to *"initial commit from old repo"* |
| **OQ2** | ✅ **Anchor `redirectPort` in this phase.** | 🚨 Retrospectively vindicated by `MEASUREMENTS.md` M2 — the manufactured-substring chain is **live**, and it also silently downgrades an unrelated origin from HTTPS to HTTP |
| **OQ3** | ✅ **Accept `*.localhost`** (broad for stamping, narrow for granting) | ⚠️ The owner's reading was *"only requests from our browser can reach our wallet"* — **inverted**, and corrected in session: websites *should* reach the wallet, they must arrive **labelled**. The two predicates stay separate for that reason |
| **OQ4** | ✅ **Add gate `G12`**, as its own commits | Explained and accepted once the mechanism was clear |
| **OQ5** | ✅ Fallback approved — ⚡ **but not needed.** | 📏 `MEASUREMENTS.md` M3 settles it: a resource handler **does** take over `https://` loopback pre-TLS. **2121 stays matched.** Ticket §11 question #1 closed after 15 days open |

The original framing of each, kept for the record:

| # | Question | Both readings | Recommendation |
|---|---|---|---|
| **OQ1** | **Drop port 8080?** Ticket §5.3 says remove it; the prompt's §5 scope fence does not name it | **Keep:** a dApp might use it. **Drop:** 8080 is Spring Boot / Tomcat / `http-server` / countless dev setups, and `/health`, `/encrypt`, `/getVersion`, `/wallet/` on a developer's own server are hijacked today — panel #2 finding #17, MEDIUM. 📏 `git log -S` traces 8080 to *"initial commit from old repo"*; it appears in no BRC and no App Lab path. 📏 Trust does **not** change either way: a page *served from* `127.0.0.1:8080` already gets internal trust from `IsInternalOrigin` regardless of this gate | **Drop it.** ⭐ It is the only entry in the gate with no product justification |
| **OQ2** | **`redirectPort` anchoring — this phase or beta.4?** It is not named in W0/W1 | **Defer:** it is scope the plan did not ask for. **Do it:** it is the single highest-leverage line, it is what manufactures the substring the new predicate would otherwise still be fooled by, and shipping `IsWalletOrigin()` while leaving it unanchored ships a fix that reads as done and is not | **Do it in this phase.** ⛔ Without it, `A4` cannot go green for the `?x=…3321` case |
| **OQ3** | **Does `IsWalletOrigin()` accept `*.localhost`?** | **Yes** (ticket §5.1): without it, `http://x.localhost:31401/createAction` stops being intercepted, goes to the real network, Chromium resolves it to loopback per RFC 6761, and it reaches the wallet with **no header ⇒ internal trust**. That is a privilege escalation shipped by a "tidy-up". **No:** it is a strange spelling nobody uses, and P0.5's `AuthorityHasHost` deliberately rejects it | **Yes — accept it.** ⚠️ This creates two deliberately different loopback notions in one header (broad for stamping, narrow for granting). §5 records why; it must be commented at both sites or the next reader "unifies" them and reintroduces this |
| **OQ4** | **A new T0 gate (`G12`) for unanchored loopback host:port substrings?** | `HARNESS.md` §4 explicitly allows a gate to land before its fix. But working rule #6 forbids the instrument moving inside the change it measures ⇒ this is **2 extra commits** (gate at measured baseline, then baseline↓ after the fix), each with `-NegativeControl` | **Ask.** Worth it — `G2` proved this class recurs — but it is real extra work and it is the owner's call whether Phase 5 carries it |
| **OQ5** | **If `A2` fails** (TLS fires before the resource handler on `https://…:2121`) | Ticket §8.1's fallback: **stop matching 2121**, so the App Lab's probe fails fast and falls through to its 3321 retry | Adopt the fallback, record it, do not invent a second mechanism |

## 7. Rollback

One commit. `IsWalletOrigin()` is a new inline predicate in `PortConfig.h` plus one changed `if` in
`simple_handler.cpp`, one changed lambda in `HttpRequestInterceptor.cpp`, and one new test file.
`git revert` restores the six-term gate and the unanchored `redirectPort` exactly. The shadow log
(`A6`) is additive and independently revertible.

---

## 8. What stays manual / not runnable from this session

| # | What | Why |
|---|---|---|
| **O1** | The App Lab board itself (`brc-cloud.bcryderman.workers.dev/app-lab`) beyond the probe | ⛔ Ticket §9: `createAction`, `signAction`, `internalizeAction`, `relinquishOutput/Certificate`, `acquireCertificate` are fund-moving. Run against an **unfunded** test wallet only |
| **O2** | 🍎 macOS — `A1` and `A2` both | Not runnable from the Windows box. Relayed, never claimed |
| **O3** | `R-GOLD` / `R-COUNT` / the `payment.auto_approved` audit line | One real payment closes all three; owed at every boundary so far |
| **O4** | Anything needing a real mouse click | ⛔ `SendInput` clicks are dropped in this agent environment (P4 M11). Mostly irrelevant — this phase is network-level |


---

## 9. Sign-off — 2026-09-02

| | |
|---|---|
| **Commits** | `c4603e1` contract + pre-fix measurement · `1743b20` the fix · `36f7a66` gate `G12` (separate, per working rule #6) · this docs commit |
| **preflight** | `-Full` **PASS** — 8 gates + 7 T1 checks, nothing skipped |
| **`-NegativeControl`** | **PASS** — every gate seen to fail, including new `G12` (`5 > 4`) |
| **Unit tests** | 374 pass / 1 pre-existing skip (`UpdateStagerRig.StagesFromLocalFeed`). 32 new, each paired with the legacy predicate as its own control |
| **Boundary 4 → 5** | `REGRESSION_SET.md` run log. `R-INTEXT` 🟢🔴 complete; `R-GOLD` / `R-COUNT` / `R-CLOSE` owed as before |

### `G12` residuals — 4, all named, all deliberate

Per `HARNESS.md` §4, a residual at a non-zero baseline is listed by file:line in the owning contract.

| File:line | Why it is allowed |
|---|---|
| `PortConfig.h` — `IsWalletHostPort` (2 lines) | Kept **only** to back `hodos::LegacyWalletGateMatch`, the W3 shadow predicate. Decides nothing |
| `PortConfig.h` — `IsLoopbackHostPort` (2 lines) | Same |

⇒ All four die together with `LegacyWalletGateMatch` in **beta.4, ticket W8**, which drives `G12` to
its target of 0. ⛔ Until then, deleting them silently removes the shadow comparison's control.

### What this phase did NOT do, stated plainly

- ⬜ **`R-GOLD` / `R-COUNT`** — need one real payment. This phase changed the predicate that decides
  whether a request is intercepted at all, and the gold pill fires from inside that path. The shadow
  log (M10) shows nothing that was intercepted stopped being intercepted, which is evidence but not
  the payment.
- ⬜ **The App Lab board** — fund-moving methods; needs an unfunded test wallet and a human.
  ⚠️ `brc-cloud.bcryderman.workers.dev` is **already an approved domain** in the dev wallet, so an
  App Lab run will show no consent prompt. That is prior state, not a broken gate.
- ⬜ **🍎 macOS** — relayed in `MAC_RELAY_P5_ROUND.md`, never claimed.
- ⛔ **W4 / W6 / W7 / W8** — beta.4, unchanged. `IsInternalOrigin("")` still returns `true`
  (re-verified at `HttpRequestInterceptor.cpp:1070`); that is W6.

### 📖 Reported, not fixed (working rule #3)

1. 🚨 **`scripts/stop-dev.ps1` fails when invoked as `powershell -File`** — `$PSScriptRoot` is empty
   during parameter binding, so it dies before stopping anything. The documented `& '.\scripts\stop-dev.ps1'`
   form works and was used throughout. ⚠️ **Worth its own ticket:** this is the tool that exists so
   nobody hand-writes a process kill, written the day after a name-matched kill took down the owner's
   production wallet. A safety tool that fails on a natural invocation invites the hand-written
   fallback it was built to prevent. Detail: `MEASUREMENTS.md` M12.3.
2. **The synthesized timeout envelope returns HTTP `200`** with an `{"error":…}` body — already filed
   as ticket §6.4.
3. **14 more unanchored `find("localhost")` / `find("127.0.0.1")` sites** in `CookieBlockManager` (12)
   and `EphemeralCookieManager` (2). Same defect class, different shape (bare host, no port), so
   `G12` does not count them. They belong to W7 in beta.4.
