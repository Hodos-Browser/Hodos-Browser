# Architecture Documentation

> Centralized location for architectural reference docs. Every CLAUDE.md
> file in the repo should link here when describing system boundaries,
> data flow, or security gates.

## Why this folder exists

Architecture documentation was previously scattered across:

- Per-area `CLAUDE.md` files (cef-native, rust-wallet, etc.) — describing
  their own layer
- Sprint-specific docs in `development-docs/<sprint>/`
- Inline code comments in load-bearing files

This was hard to find and prone to drift. This folder centralizes the
cross-cutting docs that describe HOW THE SYSTEM IS PUT TOGETHER — security
boundaries, request flows, endpoint contracts. Layer-specific docs stay
in their CLAUDE.md files; cross-layer concerns live here.

## Maintenance policy

| When | Action |
|---|---|
| Adding a new wallet endpoint | Update `WALLET_API_MAP.md` in the same commit |
| Changing the auto-approve engine logic | Update `AUTO_APPROVE_ENGINE.md` in the same commit |
| Adding a new architectural concern that spans layers | New file in this folder + add to index below |
| Architecture changes that break docs in this folder | Doc update is part of the PR's "definition of done" |
| Quarterly | One-pass review — `git log -- development-docs/architecture/` since last review, verify against current code |

Drift detection: if you grep for an endpoint, method, or invariant in code
and it's not in the architecture docs (or vice versa), file a follow-up
issue.

## Index

| Doc | Status | Purpose |
|---|---|---|
| `WALLET_API_MAP.md` | Skeleton (Phase 2.5-A will fill) | Every Rust wallet endpoint × what it does × which permission gate(s) fire × which shim call(s) reach it |
| `AUTO_APPROVE_ENGINE.md` | Skeleton (Phase 2.5-A will fill) | Current C++ `PermissionEngine` design, decision matrix, modal dispatch flow |
| `IPC_BRIDGE.md` | Stub (pointer to Phase 2.5 plan doc) | Wallet IPC bridge architecture (Phase 2.5 work in progress) |
| `PERMISSION_GATES.md` | TBD | Higher-level security model — domain_permissions table, X-Requesting-Domain header, cascade order |

## Related docs OUTSIDE this folder

| Doc | Why it's not here |
|---|---|
| `../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md` | Future vision — explicit "if starting over today" thinking saved for Phase 4+ planning. Not current state, so not in `architecture/`. |
| `cef-native/CLAUDE.md` and below | Layer-specific reference (CEF internals). |
| `rust-wallet/CLAUDE.md` and below | Layer-specific reference (Rust wallet internals). |
| `../Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md` | Sprint-specific plan (will reference this folder's docs heavily). |
| `../../CLAUDE.md` (root) | Project orientation + invariants. Should link here. |

## Source-of-truth pointers

When the docs in this folder describe code, they cite specific files +
line numbers. Some critical ones:

- Rust route table: `rust-wallet/src/main.rs` (search for `.route(`)
- Permission engine: `cef-native/src/core/PermissionEngine.cpp`
- HTTP interceptor + gate cascade: `cef-native/src/core/HttpRequestInterceptor.cpp::AsyncWalletResourceHandler::Open()`
- Domain permission cache: `cef-native/src/core/HttpRequestInterceptor.cpp::DomainPermissionCache`
- Domain permissions table: `rust-wallet/src/database/migrations.rs` (`domain_permissions`)
- Shim injection: `cef-native/include/core/CWIShimScript.h`
- Wallet IPC bridge (new): `cef-native/src/handlers/simple_handler.cpp` (`wallet_call` message handler)

When the code moves, the docs follow.
