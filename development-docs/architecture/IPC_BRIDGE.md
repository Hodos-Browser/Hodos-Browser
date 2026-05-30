# Wallet IPC Bridge

> **Status:** Stub — work in progress in Phase 2.5.
>
> When Phase 2.5 lands (commits 5-7 complete + smoke-verified), this doc
> gets filled with the final architecture. For now it points at the
> active plan doc.

## Current state pointer

See [`../Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md`](../Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md)
for the design, current status (commits 1-4 landed; 5-7 pending), and
multi-session plan.

## Why this bridge exists

The shim (`window.CWI` / `window.yours` / `window.panda`) originally used
`fetch('http://127.0.0.1:31301/...')` for wallet calls. This works on the
internal frontend (`localhost:5137`, same-origin) but fails on external
dApps because:

- **CSP** — sites like github.com block `connect-src` to `127.0.0.1:31301`
  in the renderer before the request leaves the page
- **CORS** — sites like treechat.io trigger a CORS preflight that the
  wallet's localhost-only `actix-cors` config refuses

Both blocks happen before our C++ HTTP interceptor sees the request, so
the existing permission engine never runs and the wallet call dies in the
renderer.

The fix: route wallet calls through CEF's process-message IPC. IPC isn't
subject to CSP or CORS (it's not a network request from the renderer's
view) and routes through the browser process where our existing security
machinery already lives.

## Will fill in here once Phase 2.5 lands

- High-level diagram (renderer ↔ C++ bridge ↔ Rust wallet)
- `wallet_call` and `wallet_response` IPC message contracts
- Promise correlation pattern (`requestId` minting, `pending{}` map)
- 50 MB payload ceiling rationale
- How the C++ bridge handler runs the same engine the HTTP path uses
  (Phase 2.5 commits 5-6)
- Permission gate decision flow on the IPC path
- Payment success indicator (green-dot animation) preservation
- Performance characteristics vs the old HTTP path
