# Phase 5 — MEASUREMENTS

Run 2026-09-02 on the Windows box. Dev env: wallet **31401** (`RUST_LOG=debug`), Vite **5137**,
dev browser CDP **9322**, profile `Default`. ⛔ The owner's installed browser + wallet (31301) were
running throughout and were **not** touched. MetaNet Client left running (PID 37360, `127.0.0.1:3321`
and `:2121`).

Rig: `../phase-3.5-layout-window-scoping/p35drive.py`. The tab was navigated to `https://example.com/`
by a **renderer-initiated** `location.href` assignment from inside the existing tab — ⛔ deliberately
not `PUT /json/new` and not `Page.navigate`, because a CDP-created tab bypasses `OnBeforeBrowse` and
that is defect #1 of the four-harness family in `CLAUDE.md`.

---

## M1 — 🎯 §4 SETTLED: the cross-wallet routing hole is **CLOSED**

**The claim under test** (`SPRINT_PLAN.md` §WS5(b)): *"an App Lab request inside Hodos today falls
past our gate and is answered by a different vendor's wallet — different identity key, no Hodos gate,
no indication to the user."*

📏 **Refuted.** Phase 0.5 closed it. All four addressing forms a BRC-100 dApp can use are intercepted
and reach **our** wallet, correctly labelled.

Four `fetch()` calls issued from the real external page `https://example.com/`. Each was retried once
by the interceptor, so 8 requests total; every one appears in our Rust log.

| Probe | URL | C++ (`debug_output`) | 🎯 **Rust log — what our wallet received** |
|---|---|---|---|
| **A** | `http://127.0.0.1:3321/getVersion` | `Intercepting wallet request` → `Port redirection: …:3321 → …:31401` | `07:35:04.992` `path=/getVersion requesting_domain=example.com` |
| **B** | `https://127.0.0.1:2121/getVersion` | `Intercepting` → `Port redirection` → `Scheme downgrade: https → http` | `07:35:50.002` `path=/getVersion requesting_domain=example.com` |
| **C** | `http://127.0.0.1:3321/health` | `Intercepting` → `Port redirection` | `07:36:35.033` `path=/health requesting_domain=example.com` |
| **D** | `https://127.0.0.1:2121/health` | `Intercepting` → `Port redirection` → `Scheme downgrade` | `07:37:20.048` `path=/health requesting_domain=example.com` |

Aggregate over the run: **4 × `/getVersion` + 4 × `/health`, all `requesting_domain=example.com`.**
Each minted an engine decision, e.g.

```
🛡️ engine Prompt (domain-trust) minted approval id=befd41df… for domain=example.com
   endpoint=/getVersion type=DomainApproval reason=new_domain_no_manifest
```

### 🎯 SUBJECT — why this is not another false green

Two **independent** subjects agree, and neither is a C++ log line stating intent:

1. **Destination side.** `domain_trust_mw` in *our* Rust wallet logged all 8 requests. MetaNet Client
   cannot write to our log. If MetaNet had answered, these lines would not exist.
2. **Client side.** All four fetches returned `status=200` with body
   `{"error":"Wallet request timeout","status":"error"}` — **our** browser's synthesized envelope,
   emitted because the consent modal was left to time out. MetaNet Client's `getVersion` answer looks
   nothing like this. The page received *our* response.

⛔ The C++ `Scheme downgrade` log line is explicitly **not** relied on. `ADVERSARIAL_PANEL_2` finding
#29 established that line prints whether or not `SetURL` takes effect, and calling it evidence is the
false-green shape this harness exists to catch. Row B rests on the Rust line 5 ms later.

### 🔴 RED — and it is a *better* control than the one the prompt proposed

The session prompt proposed stopping MetaNet Client. That would only confirm the converse, and it
means touching the owner's installed software. 📏 A stronger control was available for free:

| | |
|---|---|
| **NC-1** | `http://127.0.0.1:3322/getNetwork` — a loopback port **one digit off 3321**, not in the gate list |
| **Result** | C++ logs `Resource request:` and **nothing else** — no `Intercepting wallet request`, no port redirection. **Zero** Rust log lines. Client: `TypeError: Failed to fetch` |

⇒ **the gate is what causes interception**, not some ambient property of loopback. That is the actual
causal claim, and it is now controlled. ✅ MetaNet Client never needed to be stopped.

### 🎯 Consequence for the phase

⛔ **Phase 5 is tidy-up, not an outage.** The user-visible interop bug and the cross-wallet hole were
both closed in Phase 0.5. What remains is the **defect class** — the matchers are still whole-URL
substring searches — and M2 shows that class is live and exploitable from any web page.

---

## M2 — 🚨 Row `P5-A4`'s RED, observed **before** any code change

The harness requires a RED that was *run*, not predicted. Captured now, while the old predicate is
still in place, so the fix has a control it cannot retroactively invent.

**Probe:** from `https://example.com/`, `fetch('https://example.com/getNetwork?x=127.0.0.1:3321')`

⭐ Path chosen for **subject discipline**: `/getNetwork` is in `isWalletEndpoint`'s list but had not
been used by any earlier probe, so a `path=/getNetwork` line is unambiguously *this* request. A reused
path would have been indistinguishable from M1's traffic.

📏 The full chain, from the live log:

```
🌐 Intercepting wallet request from browser role: tab_1
🌐 HTTP Request intercepted: GET https://example.com/getNetwork?x=127.0.0.1:3321
🌐 Port redirection:   https://example.com/getNetwork?x=127.0.0.1:3321
                    -> https://example.com/getNetwork?x=127.0.0.1:31401
🌐 Scheme downgrade for local wallet:  https://example.com/…  ->  http://example.com/…
🌐 Checking Socket.IO connection: http://example.com/getNetwork?… - localhost: true
🌐 Extracted endpoint: /getNetwork?x=127.0.0.1:31401
🌐 Creating CefURLRequest for GET http://127.0.0.1:31401/getNetwork?x=127.0.0.1:31401
```

and in Rust:

```
07:38:54.517  R-INTEXT trust: path=/getNetwork requesting_domain=example.com
07:38:54.658  🛡️ engine Prompt (domain-trust) … endpoint=/getNetwork
```

Client side: `status=200`, body `{"error":"Wallet request timeout","status":"error"}` — for a URL
addressed to **example.com**.

⇒ **A page's request to its own server was taken over and sent to our wallet, purely because of text
the page itself put in its own query string.** `example.com` is never contacted.

### 🚨 Worse than filed — two escalations

1. **The HTTPS→HTTP downgrade of an unrelated origin is real, not hypothetical.**
   `ADVERSARIAL_PANEL_2` finding #29 hedged it as *"if `SetURL` is ever effective on this path…"*.
   📏 That hedge is the wrong frame: `SetURL` being inert does not matter, because **every downstream
   decision reads the local `url` string**, and that string has been downgraded. Observed above on a
   `https://example.com/` request.
2. **`isSocketIOConnection` classified `example.com` as `localhost: true`** — a second predicate
   fooled by the same manufactured substring, in the same request.

⇒ 👤 **This retrospectively justifies OQ2** (anchor `redirectPort` in this phase). Fixing only the
outer gate leaves steps 2–7 of that chain intact, because `redirectPort` runs first and re-creates the
match the outer gate was hardened against.

---

## M3 — ✅ Ticket §11 open question #1, open since 2026-08-18, is SETTLED

> *"Does returning a `CefResourceHandler` for `https://127.0.0.1:2121` short-circuit before TLS, or
> does cert validation fire first?"*

📏 **It short-circuits.** Probes B and D were `https://` to a port where our plaintext wallet does not
listen, and both were served with **no certificate interstitial, no TLS error, and no user prompt** —
confirmed at the destination by the Rust log.

⇒ **`OQ5`'s fallback is not needed.** 2121 stays matched. Recorded here rather than left as an open
question, because it also governs the design of `IsWalletOrigin()`.

## M4 — ✅ No Chromium Local Network Access prompt appeared

Ticket §8.2's second unsettled question. 📏 A public `https://example.com` page reached `127.0.0.1`
across 10 requests with **no LNA prompt** observed and no request blocked before our handler.
⚠️ Consistent with "interception short-circuits before the network stack applies LNA", but this run
did not *isolate* that — do not upgrade it to a mechanism claim. It is an observation.

## M5 — ✅ No residue; the check stays repeatable

📏 `domain_permissions` after the run contains **14 rows, none of them `example.com`** — every modal
was left to time out, never approved. This is the P0.8-bitgenius vacuity trap the 2→3 boundary run
warned about, and it is avoided.

⚠️ **Noted for whoever runs the App Lab next:** `brc-cloud.bcryderman.workers.dev` **is already an
approved domain** in the dev wallet. An App Lab session will therefore show **no** consent prompt —
that is prior state, not a broken gate. Deny-and-verify against a fresh host instead.

## M6 — 📖 Observed, deliberately NOT fixed (working rule #3)

1. **The synthesized timeout envelope returns HTTP `200`.** All five timed-out probes got
   `status=200` with an `{"error":…}` body. Already filed as ticket §6.4 (*"synthesized responses
   hardcode 200"*). Reported, not touched — it is not what this phase came for.
2. **Each probe was retried exactly once** by the interceptor after the modal timed out (~45 s), so 5
   probes produced 10 wallet requests. Not investigated; noted so the doubled counts above are not
   read as a defect.

---

## M7 — ⬜ NOT MEASURED FROM HERE

| # | What | Why |
|---|---|---|
| **O1** | The App Lab board itself | Fund-moving methods (ticket §9). Needs an unfunded test wallet and a human |
| **O2** | 🍎 macOS — M1 and M3 both | Not runnable from the Windows box. Relay only |
| **O3** | `R-GOLD` / `R-COUNT` / `payment.auto_approved` | One real payment closes all three; owed at every boundary so far |
| **O4** | Stopping MetaNet Client as an additional control | ⛔ Not run, and **not needed** — NC-1 (M1) is the stronger control and costs the owner nothing. Recorded so the omission is a decision, not a gap |

---

# Post-fix run — 2026-09-02, after `1743b20`

Same rig, same page, rebuilt shell. ⭐ Every row below has its pre-fix counterpart above, taken on the
**same binary lineage minutes earlier**, so the before/after is a real A/B and not two descriptions.

## M8 — 🟢 `P5-A4` GREEN: the site answers its own request again

| | `https://example.com/getNetwork?x=127.0.0.1:3321` |
|---|---|
| **Before** (M2) | `status=200`, body `{"error":"Wallet request timeout"}` — **our wallet answered**, after a 45 s consent modal |
| **After** | `status=404`, body `<!doctype html>…<title>Example Domain</title>` — **example.com answered**, in 54 ms |
| 🎯 **SUBJECT** | 📏 `getNetwork` appears in our Rust log **0 times** post-fix. Before, it appeared with `requesting_domain=example.com`. The destination confirms the client. |

`P5-A7` rides along: `https://example.com/health` also returns the site's own page.

## M9 — 🟢 `P5-A1` still GREEN: real bridge traffic is untouched

The whole risk of this change was breaking the thing Phase 0.5 fixed. 📏 `http://127.0.0.1:3321/getVersion`
from the same external page still reaches our wallet:

```
08:09:38.338  R-INTEXT trust: path=/getVersion requesting_domain=example.com
```

## M10 — 🟢 `P5-A6` W3 shadow log: **one** disagreement, and it is the intended one

Both predicates ran on every request; only the new one decided. Across `example.com`,
`github.com`, `youtube.com` (plus its service worker and every subresource) and
`en.wikipedia.org/wiki/Bitcoin` — all of which loaded normally, titles verified over CDP:

```
🔀 P5 gate disagreement: new=no old=yes authority=example.com role=tab_1     ← the exploit URL
```

**Total: 1.** Zero from ordinary browsing. Zero `new=yes old=no`, i.e. the deliberate broadening to
`*.localhost` and `127.0.0.0/8` admitted nothing unexpected in practice.

🔴 **RED for the shadow log itself:** the one line above *is* it — the log was observed to fire on a
known-disagreeing URL. A shadow log that never fires is the farbling-harness failure shape.

## M11 — 🔴🟢 `R-INTEXT` COMPLETE — all four cells, first time in this sprint

⭐ **The injected RED has been owed at every phase boundary of beta.3** — both prior boundaries
recorded *"the injected RED was not re-run; it needs a code change."* This is the phase that was
already changing that code, so it was run. Two one-line stubs, built, observed, **reverted, rebuilt,
and the GREEN halves re-observed** to prove the revert.

| | 🟢 GREEN (shipped code) | 🔴 RED (stubbed) |
|---|---|---|
| **(a) internal** | 7 × `requesting_domain=<none:internal>`, **0 prompts** | Always stamp in `runIpcCallDirect` ⇒ 🚨 **the wallet prompts for its own backend calls**: `engine Prompt … for domain=127.0.0.1:5137 endpoint=/wallet/peerpay/status type=DomainApproval` |
| **(b) external** | `requesting_domain=example.com` ⇒ `engine Prompt … endpoint=/getVersion` | Suppress in `startAsyncHTTPRequest` ⇒ `requesting_domain=<none:internal>`, **`200` in 20 ms carrying the wallet's real answer** (`HodosWallet-Rust v0.0.1` + capability list). No 202, no modal |

⇒ One header is the entire difference between *gated and prompted* and *silent, instant, fully
trusted*. That is what `REGRESSION_SET.md` asserts, and it is now **observed** rather than reasoned.

⭐ **Incidental finding, free:** under RED (a), `/wallet/balance` kept reporting `<none:internal>`
while `/wallet/peerpay/status` flipped. 📏 The two internal calls use **different transports**, and
only one was stubbed. Worth knowing — a future R-INTEXT stub that touches one transport will look
like a partial result rather than a bug.

🎯 **SUBJECT:** every cell is read from the **Rust** log — what the wallet received — never from the
C++ log or the page. Revert verified two ways: `grep` for the marker returns **0**, and
`git diff --stat` against `1743b20` is empty.

## M12 — 📖 Observed, deliberately NOT fixed (working rule #3)

3. 🚨 **`scripts/stop-dev.ps1` fails outright when invoked as `powershell -File`.**
   `[string]$RepoRoot = (Split-Path -Parent $PSScriptRoot)` is a *parameter default*, and
   `$PSScriptRoot` is empty during parameter binding under Windows PowerShell 5.1 — so the script
   dies with `Cannot bind argument to parameter 'Path'` before stopping anything.
   📏 `& '.\scripts\stop-dev.ps1'` (the form `CLAUDE.md` documents) works and was used throughout.
   ⚠️ **Worth its own ticket:** this is the tool that exists so nobody hand-writes a kill, written
   the day after a name-matched kill took down the owner's production wallet. A safety tool that
   fails on a natural invocation invites exactly the hand-written fallback it was built to prevent —
   and `-File` is how a script or an agent would most naturally call it. ⛔ Not fixed here: it is
   another change's file, and per working rule #6 the instrument does not move inside the change it
   measures.
