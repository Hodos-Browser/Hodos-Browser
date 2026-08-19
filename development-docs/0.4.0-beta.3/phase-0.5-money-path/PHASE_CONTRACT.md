# Phase 0.5 — money path & trust boundary · PHASE CONTRACT

**Workstream:** WS5(a) · **Ticket:** `../TICKET_loopback_host_form_wallet_routing.md` §6.2, §7.1, §7.3
**Status:** 🔴 NOT SIGNED OFF — adversarial panel returned DO-NOT-SIGN-OFF; 3 criticals, one MEASURED. Repair prompt: `../SESSION_PROMPT_beta3_p05_repair.md` · **Opened:** 2026-08-18 · **Amended:** 2026-08-19 (§4a–§4c, §5a) · **Platforms:** both (Rust = one binary; the C++ gates are cross-platform)
**Standard:** `../HARNESS.md`.

---

## 1. Goal

A fund-moving request from a web page is subject to the same approval engine as every other payment,
while a send the user initiates in their own wallet UI continues to complete **without any prompt**.

## 2. Done means

- [x] `send_transaction` takes `HttpRequest` and routes external callers through `dispatch_payment`,
      exactly as `create_action` does
- [ ] An internal caller (no `X-Requesting-Domain`) is **unchanged** — no modal, no new latency
- [ ] `sendMax` from an external origin is subject to the per-tx and per-session caps
- [x] `.block_on_origin_mismatch(true)` on the CORS layer
- [x] The three `:5137` **trust-boundary** substring gates use a prefix/origin match (plus a fourth, §4c)
- [x] T0 gate `G2` baseline driven **5 → 2**, with both residuals named in §6
- [x] `IsInternalOrigin("")` decided — **left as-is deliberately**; the defect was the derivation, §4d

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| **`R-INTEXT`** | **Internal never prompts, external always gates** | 🚨 **This is the phase's central risk.** Gating `send_transaction` naively — e.g. requiring `X-User-Approved` unconditionally — makes the user's own send start prompting. The discriminator must remain header-presence, the same one everything else uses |
| `R-PERIM` | The four privacy-perimeter gates | Touches `dispatch_payment`'s call surface |
| `R-COUNT` | Per-session counters | A newly-gated endpoint now increments them; it did not before |
| `R-GOLD` | Gold pill fires | A newly silent-approved path must still emit the pill, or a payment goes visually unannounced |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P0.5-R1` | User send from the wallet UI → **no modal**, tx broadcasts | Stub the internal branch to require approval → a modal appears | Frame URL `127.0.0.1:5137`; Rust log shows **no** `X-Requesting-Domain` | T2 | ⬜ |
| `P0.5-R2` | External page, over-cap send → **202 + modal** | Revert the `dispatch_payment` wiring → the send completes silently | Rust log shows `X-Requesting-Domain: <exact page host>` | T2 | ⬜ |
| `P0.5-R3` | External `sendMax:true` is capped | Same revert → full balance sweeps | Scratch wallet, funded with a token amount. ⛔ **Never the production wallet** | T2 | ⬜ |
| `P0.5-R4` | Gold pill fires on a newly silent-approved send | Stub the emit → no pill | Correct tab via `TabManager::GetTabIdForBrowserIdentifier` | T2 | ⬜ |
| `P0.5-C1` | Cross-origin simple POST no longer executes the handler | Remove `block_on_origin_mismatch` → the handler runs despite the browser hiding the response | **Server-side effect**, not the browser's error. A blocked read is not a blocked write | T2 | 🟡 **CODE IN** — verified from actix-cors 0.7.1 source, not docs: header-less requests hit `(None, _) => false` and still reach the handler; only a present-but-mismatched Origin short-circuits. Server-side effect test ⬜ owed |
| `P0.5-G1` | `https://<origin>/?x=127.0.0.1:5137` is served **nothing** from disk | ⛔ **Pre-fix this must SUCCEED** — if it does not, §7.3 is refuted and this row is withdrawn | **Release-shaped** build: `IsFrontendAvailable()` is true in production (`{app}\frontend\`), so this is not a dev-only defect | T2 | 🟡 **UNTESTABLE IN DEV** — see §4a. Code fixed; RED still owed on a release-shaped build |
| `P0.5-G2` | The same page gets **no** `window.hodosBrowser.identity` | Pre-fix it must be **defined**. Control: the same page *without* the substring must get neither | Renderer for **that page's** frame — not an overlay. `type:"page"` over CDP is not proof of which browser | T2 | ✅ **GREEN, RED observed** — §4b |
| `P0.5-G3` |  `preflight.ps1` gate `G2` passes at baseline **2** (from 5) | Add one new `find("127.0.0.1:5137")` → gate **exits non-zero** even at a non-zero baseline | `preflight.ps1 -NegativeControl` | T0 | ✅ **GREEN** `2 violations, at baseline`; 🔴 observed `3 > 2` |
| `P0.5-E1` | `IsInternalOrigin("")` behaves per the decision taken | Feed an origin-less frame → observe the decided outcome, not today's silent `true` | The Rust-side gate outcome | T1 | ✅ **GREEN, RED observed** — §4d. It was a live bypass, not a decision |

**Pairing:** `R1`/`R2` are the two halves of `R-INTEXT` and are each other's control. **Neither may be
signed off alone.** A fix that satisfies one by breaking the other is the specific failure this
phase is most likely to produce.

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
`IsInternalFrontendUrl` — so internal and external are true complements again. The deliberate
exclusion of loopback pages from the dApp shim, documented in the gating cascade at `:758-770`, is
preserved; only the matching is anchored.

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
**not** receive the wallet bridge — that is main-frame-only per the shim's gating cascade — but it
*does* receive `cefMessage`, the raw IPC underneath. Anyone reasoning about this boundary from the
bridge alone would conclude, wrongly, that subframes are already safe.

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
to the page that created it. It is still **external and gated**, which is the property that matters
here; a stricter opaque-origin model belongs with Phase 5's parsed predicate.

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

## 7. Rollback

Three independent reverts: the Rust handler signature, the one-line CORS change, and the C++ gates.
Each stands alone; none shares a commit with another.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] ⛔ `P0.5-G1` / `P0.5-G2` pre-fix reproduction attempted and its result recorded **either way** —
      a refuted premise is a valid outcome and must be written down, not quietly dropped
- [ ] `scripts/preflight.ps1` + `-NegativeControl` recorded
- [ ] `../REGRESSION_SET.md` in full at the 0.5 → 1 boundary, `R-INTEXT` **both halves**
- [ ] Adversarial review — ✅ **workflow panel** per `../HARNESS.md` §6, distinct lenses:
      *can I reach a money endpoint un-stamped* · *can I forge an origin* · *does the internal path still stay silent*
- [x] `G2` 5 → 2 in `../HARNESS.md` §9
- [ ] Commits cite row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight | **PASS** — exit 0, `-Full`, nothing skipped | 2026-08-19 | assistant |
| preflight -NegativeControl | **PASS** — all 5 gates seen to fail; `G2` at `3 > 2` | 2026-08-19 | assistant |
| regression set (0.5 → 1) | | | |
| adversarial review (panel) | | | |
