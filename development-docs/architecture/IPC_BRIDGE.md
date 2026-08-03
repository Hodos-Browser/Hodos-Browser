# Wallet IPC Bridge

> **Last reviewed: 2026-08-03.** This file was a stub pointing at the Phase 2.5
> plan doc ("commits 1-4 landed; 5-7 pending"). Phase 2.5 closed at `0baec25`
> and Phase 2.6 rebuilt the far end of the bridge. This is the live design.

## 1. Why the bridge exists

The page-facing shim (`window.CWI`, `window.yours`, `window.panda`) originally
reached the wallet with `fetch('http://127.0.0.1:<port>/…')` from the renderer.
That works on Hodos's own UI (same origin) and fails on real dApps:

- **CSP** — sites like github.com forbid `connect-src` to a localhost port. The
  request never leaves the page.
- **CORS** — sites like treechat.io trigger a preflight that the wallet's
  localhost-only `actix-cors` config refuses.

Both blocks happen *inside the renderer*, before the C++ interceptor can see
the request, so nothing downstream — permission gate included — ever runs.

CEF process messages are not network requests. They are not subject to CSP or
CORS, and they arrive in the browser process, which is where the origin
attribution, the modal machinery and the wallet HTTP client already live.

## 2. Message contract

Three process messages. All are renderer↔browser only; there is no
renderer-to-renderer path.

### `wallet_call` — renderer → browser

Sent by `window.__hodos_walletCall(method, endpoint, body, httpMethod)`
(`cef-native/include/core/CWIShimScript.h`). Received in
`cef-native/src/handlers/simple_handler.cpp :: OnProcessMessageReceived`.

| # | Arg | Type | Meaning |
|---|---|---|---|
| 0 | `requestId` | string | Caller-assigned, per-frame monotonic counter |
| 1 | `methodName` | string | Friendly name for logs (`"createAction"`) |
| 2 | `endpoint` | string | Wallet route (`"/createAction"`, `"/wallet/encrypt-bie1"`) |
| 3 | `bodyJson` | string | `JSON.stringify(body ?? {})` |
| 4 | `httpMethod` | string | One of `GET` `POST` `DELETE` `PUT` `PATCH`; defaults to `POST` |

Arg 4 is optional on the wire — the handler accepts a 4-arg message and
defaults the verb. Fewer than 4 args is dropped with a warning.

### `wallet_response` — browser → renderer

Emitted by `HttpRequestInterceptor.cpp :: sendWalletResponseIpc` for payloads
at or under **256 KB** (`kWalletResponseChunkBytes`).

| # | Arg | Type |
|---|---|---|
| 0 | `requestId` | string |
| 1 | `ok` | bool |
| 2 | `payload` | string (JSON) |

The renderer forwards it to `window.__hodos_walletResponse(requestId, ok, payloadJson)`.

### `wallet_response_chunk` — browser → renderer

Same function, payloads over 256 KB.

| # | Arg | Type | Meaning |
|---|---|---|---|
| 0 | `requestId` | string | |
| 1 | `ok` | bool | |
| 2 | `idx` | int | 0-based chunk index |
| 3 | `total` | int | Chunk count |
| 4 | `totalLen` | string | Total byte length — **string**, to avoid 32-bit int overflow |
| 5 | `chunk` | string | The slice |

Chunk boundaries are backed off any UTF-8 continuation byte (`10xxxxxx`) so a
multibyte sequence is never split, which would corrupt per-chunk JS-string
escaping. `window.__hodos_walletResponseChunk` reassembles by `requestId` and
resolves the pending promise **only** when every chunk has arrived *and* the
reassembled byte length matches `totalLen`. A mismatch, bad framing, or a
30-second gap between chunks rejects the promise. It never truncates silently.

## 3. Promise correlation

`window.__hodos_walletCall` returns a promise and stores
`pending[requestId] = {resolve, reject, method, startedAt}` **synchronously
before** sending the IPC, so a response cannot race ahead of its own
registration. `__hodos_walletResponse` looks the id up, deletes the entry, and
settles. An unknown id logs an orphan warning and is dropped.

**50 MB request ceiling** (`MAX_PAYLOAD_BYTES`). Enforced in JS before the
message is sent, and the rejection names the likely culprit (a `createAction`
carrying a large `inputs.BEEF`). Note the asymmetry: 50 MB in, chunked without
limit out. Rust separately accepts 100 MB payloads on `/createAction`,
`/signAction` and `/wallet/import`, so the IPC ceiling — not the wallet — is
the binding constraint on the shim path.

**600-second modal timeout.** When a call blocks on a prompt,
`postIpcAuthTimeout` arms a delayed task (`kPromptAuthTimeoutMs = 600000`).
Approve, deny and timeout race on `PendingRequestManager::popRequest`, which is
atomic — exactly one wins, so a late approval after a timeout cannot
double-settle the promise.

## 4. Browser-side dispatch

`simple_handler.cpp`'s `wallet_call` arm is a thin extractor. It reads the five
args, derives the origin from `frame->GetURL()` (host + optional port, no
scheme, no path), captures the frame and the CEF browser id, and delegates to
`HttpRequestInterceptor.cpp :: HandleIpcWalletCall`. It runs on `TID_UI`,
which is required — `CefFrame` methods are UI-thread only.

`HandleIpcWalletCall` branches three ways:

1. **Internal origin** (`IsInternalOrigin`) → `runIpcCallDirect`. Hodos's own
   UI does not gate itself.
2. **No wallet** (`WalletStatusCache`) → immediate `{code: "NO_WALLET"}` response.
3. **Everything else** → `runIpcEngineCascade`.

`runIpcEngineCascade` computes payment context if the endpoint is a payment
endpoint (`extractOutputSatoshis` + `BSVPriceCache` → satoshis, cents, price
availability), then posts to `TID_FILE_USER_BLOCKING` and forwards the call to
Rust **unconditionally** via `dispatchWalletHttpByMethod` (`SyncHttpClient`,
30 s timeout), with headers:

```
Content-Type: application/json
X-Requesting-Domain: <origin>            (external calls only)
X-Browser-Id, X-Payment-Satoshis,
X-Payment-Cents, X-Bsv-Price-Available   (payment endpoints only)
```

Rust answers, and the worker handles the three cases:

- **200** → build the payload, fire `OnWalletCallSuccess` if this was an
  auto-approved payment (the **gold pill**), hop to `TID_UI`, send
  `wallet_response`.
- **202** → hop to `TID_UI`, hand the envelope to `tryHandlePendingResponse`,
  which opens the modal named by `promptType` and enrolls a
  `PendingAuthRequest` with `resumeKind = kIpcResponse`. Critically it sets
  `ResumeContext::originalIpcRequestId = requestId` — **the page-supplied id,
  not a freshly minted one.** Get this wrong and the modal resolves against an
  id the shim's `pending{}` map has never seen, so the promise hangs forever.
  On resolution, `resumeIpcResponse` re-issues with `X-User-Approved` and
  delivers under the original id.
- **anything else** → error payload, `wallet_response` with `ok = false`.

## 5. Threading

| Stage | Thread | Why |
|---|---|---|
| `wallet_call` arrival, origin extraction, `HandleIpcWalletCall` | `TID_UI` | `CefFrame::GetURL` is UI-only |
| Wallet HTTP forward (`SyncHttpClient`) | `TID_FILE_USER_BLOCKING` | Synchronous and blocking; must not sit on UI or IO |
| Modal open, `PendingRequestManager` enrollment, `OnWalletCallSuccess`, `sendWalletResponseIpc` | `TID_UI` | CEF requirement for frame + browser calls |
| Timeout task | `TID_UI` (delayed) | Races atomically with approve/deny |

## 6. What the bridge does *not* carry

- **BRC-121 paid content.** The 402 retry chain runs in the resource-request
  pipeline (`Async402ResourceHandler`), not over this bridge.
- **Wallet UI traffic.** The wallet, settings, backup and notification
  overlays are internal origins; they call the wallet directly.
- **Rust background tasks.** `TaskCheckPeerPay` and friends call their own
  handlers in-process.
- **`window.hodosBrowser.*`.** The browser API (navigation, downloads,
  identity, tabs) uses its own `cefMessage.send()` messages — a separate
  surface from `wallet_call`.

## 7. Known gaps

- **`IsInternalOrigin("")` returns `true`.** An empty origin — a frame whose
  URL has no `://`, such as `about:blank` or a sandboxed iframe — is treated
  as internal and bypasses the permission engine entirely. This is tracked as
  an open security item, not an intended behavior.
- **`ENDPOINT_BASE = 'http://127.0.0.1:31301'` in `CWIShimScript.h`** is
  declared and never read — a leftover from the pre-bridge fetch path. It also
  hardcodes the release port, so it would be wrong under `HODOS_DEV=1`. Delete
  it rather than route it through `PortConfig.h`; the shim has no business
  knowing a port.

## Related

- [`AUTO_APPROVE_ENGINE.md`](./AUTO_APPROVE_ENGINE.md) — what happens after the call reaches Rust
- [`WALLET_API_MAP.md`](./WALLET_API_MAP.md) — which shim methods exist and where they land
- `cef-native/src/handlers/CLAUDE.md` — the full IPC message roster for `simple_handler.cpp`
- `cef-native/src/core/CLAUDE.md` — `HttpRequestInterceptor` internals, gold-pill chain
