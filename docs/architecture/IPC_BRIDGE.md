# Wallet IPC Bridge

> **Last reviewed: 2026-09-30** (accuracy pass against the website audit of
> 2026-09-30). **2026-08-03:** this file was a stub pointing at the Phase 2.5
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
| 4 | `httpMethod` | string | The shim sends one of `GET` `POST` `DELETE` `PUT` `PATCH`; defaults to `POST` |

Arg 4 is optional on the wire — the handler accepts a 4-arg message and
defaults the verb. Fewer than 4 args is dropped with a warning. The five-verb
list is enforced only in the shim (`VALID_VERBS`). The browser process checks
the verb with `PortConfig.h :: IsValidWalletMethod`, which accepts any 1–8
uppercase ASCII letters, so a page that calls `cefMessage.send('wallet_call', …)`
directly can send another verb.

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
carrying a large `inputs.BEEF`). It is enforced in JS only: a page that calls
`cefMessage.send('wallet_call', …)` directly skips it. Note the asymmetry:
50 MB in, chunked without limit out. Rust accepts 100 MB raw bodies app-wide
(`web::PayloadConfig`, `main.rs`; `/createAction`, `/signAction` and
`/wallet/import` also set it on their own resources), and caps JSON-extractor
bodies at 10 MB (`web::JsonConfig`). `/createAction` reads raw bytes, so its
Rust limit is 100 MB. On the shim path the IPC ceiling, not the wallet, is the
binding constraint.

**600-second modal timeout.** When a call blocks on a prompt,
`postIpcAuthTimeout` arms a delayed task (`kPromptAuthTimeoutMs = 600000`).
Approve, deny and timeout race on `PendingRequestManager::popRequest`, which is
atomic — exactly one wins, so a late approval after a timeout cannot
double-settle the promise.

## 4. Browser-side dispatch

`simple_handler.cpp`'s `wallet_call` arm is a thin extractor. It reads the five
args, takes the origin (host + optional port, no scheme, no path) that
`simple_handler.cpp :: ResolveIpcOrigin` derived for the message, captures the
frame and the CEF browser id, and delegates to
`HttpRequestInterceptor.cpp :: HandleIpcWalletCall`. `ResolveIpcOrigin` reads
`frame->GetURL()` first; for a frame with no origin of its own (`about:blank`,
`data:`, `blob:`) it walks up the parent frames, then falls back to the main
frame. If none yields an origin it returns `opaque-origin.invalid`, which is
always gated as external. It runs on `TID_UI`, which is required — `CefFrame`
methods are UI-thread only.

`HandleIpcWalletCall` branches four ways:

1. **Internal origin** (`IsInternalOrigin`) → `runIpcCallDirect`. Hodos's own
   UI does not gate itself.
2. **Wallet service unreachable** (`WalletStatusCache` reports `FetchFailed`) →
   immediate `{code: "WALLET_UNAVAILABLE"}` response.
3. **No wallet** (`WalletStatusCache` reports `DoesNotExist`) → immediate
   `{code: "NO_WALLET"}` response.
4. **Everything else** → `runIpcEngineCascade`.

> ⚠️ **Known issue.** `IsInternalOrigin` treats any `localhost:*` or
> `127.0.0.1:*` origin as Hodos's own UI, so a page served from a local port
> takes branch 1 and reaches Rust with no `X-Requesting-Domain`. Scheduled for
> beta.7 Track 5 (phase P2):
> `development-docs/0.4.0-beta.7/tickets/TICKET_headerless_loopback_requests_are_trusted_as_wallet_ui.md`.

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

- **BRC-121 (Simple 402 Payments) paid content.** The 402 retry chain runs in the resource-request
  pipeline (`Async402ResourceHandler`), not over this bridge.
- **Gating for wallet UI traffic.** The wallet UI does use the bridge
  (`frontend/src/services/walletApi.ts :: walletFetch` calls
  `window.__hodos_walletCall`), but its overlays are internal origins, so
  their calls take branch 1 (`runIpcCallDirect`) and reach Rust with no
  `X-Requesting-Domain` and no permission gate.
- **Rust background tasks.** `TaskCheckPeerPay` and friends call their own
  handlers in-process.
- **`window.hodosBrowser.*`.** The browser API (navigation, downloads,
  identity, tabs) uses its own `cefMessage.send()` messages — a separate
  surface from `wallet_call`.

## 7. Known gaps

- **`IsInternalOrigin("")` returns `true`.** The function still treats an
  empty origin as internal, because on the HTTP path an absent
  `X-Requesting-Domain` means an internal call. The IPC path no longer passes
  it an empty origin: `ResolveIpcOrigin` (§4) falls back to ancestor frames and
  then to `opaque-origin.invalid`. The wider loopback issue remains: any
  `localhost:*` or `127.0.0.1:*` page, and any local process that omits the
  header, is trusted as the wallet UI. A fix is planned for a later release.
- **The endpoint is not checked against an allow-list.** `wallet_call` forwards
  whatever path the page names. An approved dApp can reach any wallet route
  that is not on the Rust first-party-only list
  ([`WALLET_API_MAP.md`](./WALLET_API_MAP.md) §1). A fix is planned for a later release.
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
