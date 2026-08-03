# Architecture Documentation

> **Last reviewed: 2026-08-03** (full rewrite against code — the previous
> contents described the pre-Phase-2.6 C++ permission engine and were
> substantially wrong).
>
> This folder carries **cross-layer** architecture: the flows that no single
> layer's `CLAUDE.md` owns because they cross the React / C++ / Rust boundary.

## The rule: one home per fact

| Kind of fact | Home |
|---|---|
| Shape, contracts, invariants, pointers | Root docs (`/CLAUDE.md`, `README.md`, `PROJECT_OVERVIEW.md`) |
| Inventory — file rosters, endpoint lists, counts, exhaustive tables | The per-directory `CLAUDE.md` for that layer |
| **A flow that crosses two or more layers** | **Here** |

If a fact appears both here and in a layer `CLAUDE.md`, **the layer doc wins
and the copy here is a bug.** These docs deliberately do *not* re-list the
110 wallet routes, the 32 `src/core/*.cpp` files, or the 14 overlays — they
point at the layer doc that owns each roster.

Cite `file.rs :: symbol_name`, not `file.rs:1234`. Symbols survive edits.

## Index

| Doc | Scope |
|---|---|
| [`AUTO_APPROVE_ENGINE.md`](./AUTO_APPROVE_ENGINE.md) | The permission decision flow end to end: what C++ collects, what Rust decides, how a prompt round-trips back. Matrix C branch order. |
| [`IPC_BRIDGE.md`](./IPC_BRIDGE.md) | The `wallet_call` / `wallet_response` process-message bridge that carries every page-originated wallet call. Message contract, chunking, promise correlation. |
| [`WALLET_API_MAP.md`](./WALLET_API_MAP.md) | Which wallet endpoints are permission-gated, by which dispatcher, and which shim method reaches them. The gate map — **not** an endpoint roster. |

`PERMISSION_GATES.md` was listed as "TBD" here for two months and never
written. It is not planned: everything it would have covered lives in
`AUTO_APPROVE_ENGINE.md` §2 or in the Rust layer docs.

## Where the authority actually lives

These docs are a narrative over code. When they disagree with code, code wins.

| Concern | Source of truth |
|---|---|
| Permission decision logic | `rust-wallet/crates/hodos_permission_engine/src/matrix_c.rs :: decide` |
| Decision types | `hodos_permission_engine/src/decision.rs` (`PermissionDecision`, `PromptType`, `EngineReason`) |
| Context fields the decision reads | `hodos_permission_engine/src/context.rs` (`PermissionContext`, `CallKind`, `TrustLevel`) |
| Gate dispatch + HTTP envelopes | `rust-wallet/src/permission_service/request_gate.rs` |
| Middleware wiring | `rust-wallet/src/main.rs :: domain_trust_mw` |
| Route registration | `rust-wallet/src/main.rs` (roster in `rust-wallet/CLAUDE.md`) |
| Wallet-call interception + modal orchestration | `cef-native/src/core/HttpRequestInterceptor.cpp` |
| IPC bridge entry | `cef-native/src/core/HttpRequestInterceptor.cpp :: HandleIpcWalletCall` |
| Page-facing shim | `cef-native/include/core/CWIShimScript.h` |
| Backend ports | `cef-native/include/core/PortConfig.h` |
| Modal rendering | `frontend/src/pages/BRC100AuthOverlayRoot.tsx` (type dispatch) |

## Layer docs (inventory lives there, not here)

| Layer | Doc |
|---|---|
| Rust wallet | `rust-wallet/CLAUDE.md`, `rust-wallet/src/CLAUDE.md`, `rust-wallet/src/database/CLAUDE.md` |
| C++ shell | `cef-native/CLAUDE.md`, `cef-native/src/core/CLAUDE.md`, `cef-native/src/handlers/CLAUDE.md`, `cef-native/include/core/CLAUDE.md` |
| React | `frontend/src/pages/CLAUDE.md`, `frontend/src/components/CLAUDE.md`, `frontend/src/hooks/CLAUDE.md` |

## Maintenance policy

| When | Action |
|---|---|
| Changing the Matrix C cascade in `matrix_c.rs` | Update `AUTO_APPROVE_ENGINE.md` §2 in the same commit |
| Adding or removing a `dispatch_*` call in a handler | Update the gate map in `WALLET_API_MAP.md` in the same commit |
| Changing the `wallet_call` / `wallet_response` message shape | Update `IPC_BRIDGE.md` §2 in the same commit |
| Adding a wallet endpoint | Update `rust-wallet/CLAUDE.md` (the roster). Only touch `WALLET_API_MAP.md` if the endpoint is permission-gated or shim-reachable |
| Adding a new cross-layer concern | New file here + a row in the index above |

Drift shows up as a symbol name in these docs that no longer greps. A doc
that names a deleted symbol is worse than no doc — it sends the next reader
looking for code that does not exist.
