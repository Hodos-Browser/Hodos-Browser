# A connect approved on the **IPC** transport still re-sends the site's call with an EMPTY body

**Found:** 2026-09-19 by the **owner**, at the keyboard, during the `W5`/`R-GOLD` sitting on Windows.
**Status:** OPEN — MEASURED, not fixed. **Severity:** MEDIUM. Fails closed (no money moves), but it
breaks the first wallet call of **every manifest-less dApp** immediately after the user clicks Allow.

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
