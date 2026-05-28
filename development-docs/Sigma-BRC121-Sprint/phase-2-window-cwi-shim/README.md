# Phase 2 — `window.CWI` / `window.yours` / `window.panda` Shim Implementation

**CWI** = **Chrome Wallet Interface** (BRC-100's canonical injected-provider API name).

## Purpose

Inject three JS provider objects into pages, all backed by the same V8→IPC→Rust pipeline:

- `window.CWI` — canonical BRC-100 (28 methods from `@bsv/sdk@2.0.13` `WalletInterface`). The future. Non-writable, non-configurable.
- `window.yours` — legacy translation layer. Calls translate to BRC-100 backend per `../phase-0.2-window-yours-shim-design/SHIM_TRANSLATION_SPEC.md`. Writable (so other extensions can override if installed).
- `window.panda` — alias to `window.yours`. Treechat still targets this name.

## Why all three

Yours Wallet's `brc100-remote` ships **only** `window.CWI`. Sites using legacy `window.yours`/`window.panda` will break on Yours v5+. Hodos keeps them working during the transition (~months) — a real interop differentiator.

## Patterns to adopt (from Brave research)

See `../BRAVE_WALLET_REFERENCE.md` for full detail. Top patterns to lift:

1. **V8 Proxy with `apply` traps on each method** — defends against `const r = CWI.request; r({...})`
2. **Non-writable, non-configurable property descriptors** for `window.CWI` (canonical)
3. **`window.yours` / `window.panda` writable** — Brave's `isMetaMask`-writable lesson; allows competing extensions to coexist
4. **No injection in private/incognito tabs**
5. **Iframe Permissions Policy gating** (`allow="bsv-wallet"`)
6. **Secure-context-only** (HTTPS or localhost)
7. **Hide-until-user-gesture** (stricter than Brave; matches Hodos privacy posture)
8. **Origin + favicon on every signing/spending prompt**
9. **EIP-6963-equivalent announce protocol** — propose `bsv:announceProvider` CustomEvent if not already specified anywhere

## Implementation steps

1. **Confirm BRC-100 audit complete** (`../phase-0.1-brc100-audit/AUDIT_RESULTS.md`). All 28 methods either implemented or stubbed with clear errors.
2. **Confirm shim spec complete** (`../phase-0.2-window-yours-shim-design/SHIM_TRANSLATION_SPEC.md`). All legacy methods have per-method translation rules.
3. **V8 injection in `simple_render_process_handler.cpp::OnContextCreated`** — three objects, Proxy-wrapped, descriptors per Brave patterns.
4. **IPC routing** — each method's IPC payload validated, dispatched to existing Rust handlers (mostly) or new ones (Phase 3 ordinal-aware paths).
5. **Permission gates** — per Q11/Q17, every call routes through `check_domain_approved()` plus the new BRC-100 sub-tier permissions (per-protocol, grouped, per-counterparty) where applicable.
6. **No-injection rules** — private/incognito mode check, secure-context check, iframe permissions-policy check.

## Out of scope for this phase

- Ordinal-specific `createAction` recognition (basket=`'1sat'` UTXO classification, Ordinal Lock script handling) — that's Phase 3.
- BSM/BRC-77 dedicated `/signMessage` endpoint — drop unless needed for content-signing demo.

## Reference sources

- `../BRAVE_WALLET_REFERENCE.md` — patterns
- `../YOURS_CWI_MIGRATION.md` — method definitions + comparison table
- `../phase-0.1-brc100-audit/AUDIT_RESULTS.md` (when complete)
- `../phase-0.2-window-yours-shim-design/SHIM_TRANSLATION_SPEC.md` (when complete)

## Status

Not started. Gated on Phase 0.1 + 0.2.
