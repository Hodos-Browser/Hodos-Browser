# A connect approved on the **IPC** transport still re-sends the site's call with an EMPTY body

**Found:** 2026-09-19 by the **owner**, at the keyboard, during the `W5`/`R-GOLD` sitting on Windows.
**Status:** ✅ **FIXED on macOS 2026-09-19** — both evidence rows GREEN, RED reproduced first and a
negative control run. **Severity:** MEDIUM. Fails closed (no money moves), but it broke the first
wallet call of **every manifest-less dApp** immediately after the user clicked Allow.

> ## ✅ The fix — macOS, 2026-09-19
>
> **Shape, not code, from the HTTP arm.** HTTP re-enters the resource handler's own pipeline; the IPC
> equivalent of that pipeline is **`runIpcEngineCascade`**, so the `kInternal && frame` arm now
> re-enters it instead of calling `resumeInternalResponse` with a stub. That one move fixes all three
> faults at once: it sends the real body, it computes the payment cost and injects
> `X-Payment-Satoshis` / `-Cents` / `-Bsv-Price-Available`, and it routes a follow-up 202 into its own
> modal via `tryHandlePendingResponse` rather than leaking it to the page.
>
> **The body is carried in a new field, `PendingAuthRequest::resumeBody`** — `body` is still blanked
> for connect entries, deliberately, because `body` is what the modal overlay is shown
> (`sendAuthRequestDataToOverlay` arg 3) and a connect prompt has no business rendering the call's
> payload into the overlay's renderer. ⭐ So the ticket's "do not just delete the blank" holds: the
> blank stays, and the resume stops depending on it.
>
> ⚠️ **No re-prompt loop**, and it is load-bearing: `addDomainPermission()` is POSTed to Rust *before*
> the drain runs (`simple_handler.cpp`), the same ordering the HTTP arm already relies on, so Rust
> sees the domain as approved and any further 202 is a *different* gate.
>
> ⛔ **Scoped to the connect drain only.** The two `resumeInternalResponse` call sites in
> `handleAuthResponse` are the **kind-prompt** path: those entries carry a real body *and* a
> single-use `X-User-Approved` in `headersOnApprove`. Re-entering the cascade there would drop the
> token and loop. They are correct as they are.
>
> ### 📏 Measured on macOS, https://example.com (`trustLevel: unknown`), dev balance 0
>
> | | pre-fix (negative control: stash + rebuild + re-sign) | post-fix |
> |---|---|---|
> | wallet log | `📋 Raw request body (0 bytes):` | `📋 Raw request body (166 bytes):` |
> | page | `Invalid JSON: EOF while parsing a value at line 1 column 0` | `Insufficient funds: no UTXOs available` |
> | resume log | — | `🔐 kInternal+frame resume: re-entering the IPC cascade … bodyBytes=166` |
>
> ⭐ The RED is **byte-for-byte the owner's Windows signature**, reproduced on macOS — so it was never
> platform-specific. ⭐ "Insufficient funds" is the money-safe GREEN: the body parsed and the call
> failed at coin selection, so nothing could be spent.
>
> ### 📏 Row 2 — the over-cap row this ticket asked for
>
> Connect → Allow, then the same first call at 200,000,000 sats:
> `modal 2: "is requesting a payment · $34.98 · This payment of $34.98 exceeds your per-transaction
> limit of $10.00 for this site."` — and **`page result while modal 2 is up: null`**, i.e. the 202 did
> NOT reach the page as a 2xx. Denied ⇒ `User rejected authentication`. That null is the assertion
> that the false-gold-pill outcome cannot happen.
>
> ✅ **HTTP transport re-checked on the same build** (both arms share the enrolment code I touched):
> `fetch('http://localhost:3321/createAction')` → connect → Allow → `Insufficient funds`. Round i's
> fix is intact.

> ⭐ This is the arm macOS explicitly flagged as unmeasured when they fixed the HTTP half in `8f857d5`:
> *"⛔ Not covered: the **IPC** transport (`window.CWI`) — `kInternal` entries with a **frame** still go
> through `resumeInternalResponse` and are unchanged. Whether a connect on the IPC path has the same
> empty-body problem is ⛔ not measured."* It does.

## Measured

Windows dev build, rebased onto `8f857d5` and rebuilt. `https://hodos-test.local:8443`, unknown site,
`window.CWI.createAction` for 100 sats. Connect modal → **Allow**:

```
page   17:36:39  {"error":"[Hodos] createAction failed: Invalid JSON: EOF while parsing a value at line 1 column 0"}
wallet           /createAction called
wallet           Raw request body (0 bytes):
wallet           JSON parse error: EOF while parsing a value at line 1 column 0
```

Byte-for-byte the signature macOS measured on the HTTP transport.

## Mechanism — the same root line, a different resume arm

`openDomainApprovalModal` enrols the entry with `req.body = "";  // historical`
(`HttpRequestInterceptor.cpp :1369`). `8f857d5` fixed the HTTP arm only:

```cpp
if (req.resumeKind == ResumeKind::kInternal && req.handler) return ForwardPendingWalletRequest(req.handler);  // HTTP - FIXED
if (req.resumeKind == ResumeKind::kInternal && req.frame)   resumeInternalResponse(req, kApprovedStub);       // IPC  - NOT FIXED
```

An IPC entry carries a **frame**, not a handler, so it still reaches `resumeInternalResponse`, which
re-sends `req.body` — the blank.

⚠️ **This arm matters more than the one already fixed.** `window.CWI` is the canonical BRC-100 provider
surface injected into every https dApp page, so this is the *normal* dApp path, not a fallback.
`openManifestConnectBundleModal` does not blank the body, so a site **with** a manifest is unaffected.

## ⛔ Do not "just delete the blank" — macOS proved that is wrong

The HTTP fix deliberately routes through the handler's pipeline instead, because `resumeInternalResponse`
also (a) sends **no `X-Payment-*` headers**, so Rust fails closed into a price-unavailable **202**, and
(b) delivers a 202 to the page **as a 2xx** — which would light the **GOLD PILL for a payment that never
happened**. Restoring the body alone converts a visible parse error into a silent false payment
indicator. 🚨 That is strictly worse.

⚠️ The IPC arm has no handler to delegate to, so the HTTP fix's shape does not transfer directly. Whoever
takes this must decide how an IPC resume gets the payment headers and how it routes a follow-up 202 —
`resumeIpcResponse` (the `kIpcResponse` arm) may already hold the answer, since it is the path the
kind-prompt approve flow uses. Not traced.

## Evidence a fix must turn green

The RED above: same click, and the wallet must log a non-zero body and the page must receive the call's
own result. ⭐ And a second row the HTTP fix needed: an over-cap amount on that first post-connect call
must open the **payment modal**, not deliver a raw 202 to the page.

## Cross-references

- `MAC_RELAY_BETA3.md` rounds `2026-09-19h` §4 (the HTTP find) and `2026-09-19i` (the fix + its
  "not covered" list).
- `cef-native/src/core/HttpRequestInterceptor.cpp :: ResumeDrainedApprovedRequest`, `:: openDomainApprovalModal`.
