# Coding Conventions

**Analysis Date:** 2026-01-28

## Naming Patterns

**Files:**
- React components: PascalCase with `.tsx` extension (e.g., `WalletPanel.tsx`, `BRC100AuthModal.tsx`)
- Utility hooks: `use` prefix in camelCase with `.ts` extension (e.g., `useHodosBrowser.ts`, `useBalance.ts`)
- Type definition files: `.d.ts` extension (e.g., `hodosBrowser.d.ts`, `address.d.ts`)
- CSS files: camelCase matching component name (e.g., `WalletPanel.tsx` pairs with `WalletPanel.css`)
- Rust modules: snake_case (e.g., `brc42.rs`, `cache_errors.rs`, `certificate/parser.rs`)

**Functions:**
- React components: PascalCase (e.g., `WalletPanel`, `BRC100AuthModal`)
- React hooks: camelCase with `use` prefix (e.g., `useHodosBrowser()`, `useBalance()`)
- Callback handlers: `handle` prefix in camelCase (e.g., `handleSendClick()`, `handleAuthApprove()`)
- Promise-based functions: descriptive verbs (e.g., `getIdentity()`, `generateAddress()`, `markBackedUp()`)
- Rust functions: snake_case (e.g., `derive_child_private_key()`, `compute_shared_secret()`, `estimate_transaction_size()`)

**Variables:**
- React state: camelCase, often prefixed with intention (e.g., `isLoading`, `isCopied`, `showSendForm`, `transactionResult`)
- Boolean flags: `is` or `has` prefix (e.g., `isGenerating`, `hasAddress`, `isLoading`)
- Event handlers: camelCase with callback pattern (e.g., `setAuthModalOpen`, `setAddressCopiedMessage`)
- Rust variables: snake_case throughout

**Types:**
- TypeScript interfaces: PascalCase, often `Props` suffix for component props (e.g., `WalletPanelProps`)
- Type aliases: PascalCase (e.g., `AddressData`, `TransactionResponse`, `HistoryEntry`)
- Rust struct names: PascalCase (e.g., `WalletDatabase`, `DomainWhitelistManager`, `AuthSessionManager`)
- Rust enum names: PascalCase (e.g., `CacheError`, `Brc42Error`, `SecurityLevel`)

## Code Style

**Formatting:**
- Frontend uses Vite with React plugin
- TypeScript target: ES2020
- Module resolution: bundler
- No explicit Prettier configuration enforced, but style is consistent with modern React practices
- 2-space indentation inferred from existing code
- Single quotes generally used for strings (though some mixed in existing code)

**Linting:**
- ESLint with `@eslint/js` and `typescript-eslint`
- React hooks validation via `eslint-plugin-react-hooks`
- React refresh warnings via `eslint-plugin-react-refresh` (warns on non-component exports)
- TypeScript strict mode enabled in `tsconfig.app.json`
- Config file: `frontend/eslint.config.js` (new flat config format)

**TypeScript Compiler Rules:**
- `strict: true` - All strict checks enabled
- `noUnusedLocals: true` - Error on unused variables
- `noUnusedParameters: true` - Error on unused parameters
- `noFallthroughCasesInSwitch: true` - Prevent switch fall-through
- `noUncheckedSideEffectImports: true` - Warn on import side effects
- `verbatimModuleSyntax: true` - Preserve import/export syntax for bundler

**Rust Conventions:**
- Edition: 2021
- Logging via `log` crate with macros: `log::info!()`, `log::warn!()`, `log::error!()`
- Error handling: `thiserror` crate with `#[derive(thiserror::Error)]`
- Doc comments: `///` for public items, examples in BRC-42 module show comprehensive documentation

## Import Organization

**Order (Frontend):**
1. React and core libraries (e.g., `import React, { useEffect } from 'react'`)
2. Third-party UI/navigation (e.g., `import { BrowserRouter } from 'react-router-dom'`)
3. MUI components (e.g., `import { Button } from '@mui/material'`)
4. Local components (e.g., `import WalletPanel from './components/WalletPanel'`)
5. Local hooks (e.g., `import { useBalance } from '../hooks/useBalance'`)
6. Local types (e.g., `import type { TransactionResponse } from '../types/transaction'`)
7. CSS files (e.g., `import './WalletPanel.css'`)

**Path Aliases:**
- No explicit path aliases configured in `tsconfig.app.json` - use relative imports throughout
- Convention: `../` for going up directories (e.g., `../hooks/`, `../types/`)

**Order (Rust):**
1. Standard library (e.g., `use std::path::PathBuf`)
2. External crates (e.g., `use actix_web::{web, App}`)
3. Internal modules (e.g., `use crate::crypto::brc42::derive_child_private_key`)

## Error Handling

**Frontend Patterns:**
- Try/catch blocks with error logging: `console.error('Failed to...', error)`
- Promise rejections logged with context emoji prefix (e.g., `💥 Error creating wallet`)
- Type guards for error messages: `error instanceof Error ? error.message : 'Unknown error'`
- Async operations with timeout patterns (e.g., 10-second timeout in `useHodosBrowser()`)
- Window callbacks for error responses: `window.onAddressError()`, `window.onSendTransactionError()`
- No try/catch in most cases; relies on Promise `.catch()` or error callback handlers

**Rust Patterns:**
- Custom error types using `thiserror`: `#[derive(thiserror::Error)]`
- Error type: `pub enum [ModuleName]Error { ... }` with variants for specific failures
- Associated type: `pub type [ModuleName]Result<T> = Result<T, [ModuleName]Error>`
- Implementations of `From<>` for automatic conversion from underlying error types
- Error context: Descriptive enum variants (e.g., `InvalidPrivateKey(String)`, `DerivationFailed(String)`)
- Use `?` operator for error propagation in functions returning `Result`

**Example Rust Error Pattern** (`src/cache_errors.rs`):
```rust
#[derive(Debug)]
pub enum CacheError {
    Database(rusqlite::Error),
    Api(String),
    InvalidData(String),
}

impl From<rusqlite::Error> for CacheError {
    fn from(err: rusqlite::Error) -> Self {
        CacheError::Database(err)
    }
}

pub type CacheResult<T> = Result<T, CacheError>;
```

## Logging

**Framework:**
- Frontend: `console` (no logging framework)
- Rust: `log` crate with `env_logger` configured in `main.rs`

**Patterns:**
- Frontend uses emoji prefixes for visual distinction in development logs:
  - `🔍` for debug/inspection
  - `🔐` for security/auth operations
  - `💥` for errors
  - `🧠` for native/C++ bridge communication
  - `✅` for success
  - `📍` for address operations
  - `💳` for balance/transaction operations
  - `🧹` for cleanup

- Frontend logs persist in dev tools - no level filtering
- Rust logging: `log::info!()`, `log::warn!()`, `log::error!()` with structured format
- Rust log level controlled by `env_logger::Env::default().default_filter_or("info")`
- All logs to console in Rust (no file logging currently)

## Comments

**When to Comment:**
- Complex algorithm explanation: Yes (see BRC-42 module for example)
- Business logic: Yes, especially around fee calculation and protocol rules
- Why decisions: Yes (e.g., comments explaining deviations from spec)
- Self-evident code: No
- Type hints already in signatures: No

**JSDoc/TSDoc:**
- Not systematically used in frontend
- No mandatory doc comments on functions
- Type definitions live in `.d.ts` files with minimal comments

**Rust Documentation:**
- Module-level doc comments: `//!` (mandatory for public modules)
- Function doc comments: `///` for public functions
- Example: `src/crypto/brc42.rs` has comprehensive docs with BRC spec reference links
- Document assumptions and invariants (e.g., "Shared secret is 33-byte compressed point")

## Function Design

**Size:**
- React components: 100-300 lines typical (smaller hooks, larger panels)
- Hooks: 20-100 lines
- Rust functions: 20-80 lines typical, up to 150+ for complex handlers

**Parameters:**
- Frontend React components accept single destructured props object: `({ onClose, data })`
- Hooks accept no parameters or destructured options
- Rust functions: Pass by reference for non-Copy types (`&[u8]`), owned values for small structs
- Async functions widely used: `async fn handler() -> Result<T>`

**Return Values:**
- React hooks return objects with multiple named returns (e.g., `{ balance, usdValue, isLoading, refreshBalance }`)
- Frontend APIs return Promises wrapping typed responses
- Rust functions return `Result<T, ErrorType>` for fallible operations
- Success responses typed explicitly (e.g., `Promise<AddressData>`)

**Async Patterns:**
- Frontend: Promises with `.then()` or `await` in async functions
- Error handling in async: try/catch or `.catch()` on promises
- React hooks: `useEffect` with cleanup functions
- Rust: Async via Tokio with `#[actix_web::main]` for web handlers

## Module Design

**Exports:**
- Frontend: Default export for React components (`export default WalletPanel`)
- Frontend: Named exports for utilities and hooks (`export function useHodosBrowser()`)
- Rust: Public items explicitly marked with `pub` keyword
- Re-exports of common types in public modules (e.g., `pub use certificate_handlers::{ ... }`)

**Barrel Files:**
- Frontend: No barrel files (index.ts/index.tsx) used
- Each module imported directly by path
- Components organized by feature/layout (e.g., `components/panels/`, `hooks/`)

**Organization:**
- Feature-based structure in frontend (`components/`, `hooks/`, `bridge/`, `pages/`)
- Layer-based structure in Rust (`src/handlers.rs`, `src/database/`, `src/crypto/`)
- Related functionality grouped (e.g., BRC protocol modules together)

## Special Patterns

**Bridge Communication (Frontend):**
- Two-way communication pattern: Frontend → C++ → Rust, Rust → C++ → Frontend
- Message-based: `window.cefMessage?.send(channel, args[])`
- Response callbacks: `window.onAddressGenerated = (data) => {}`
- No request/response correlation ID system (single outstanding request at a time assumed)

**State Management (Frontend):**
- Component-level state via `useState`
- Hook composition for cross-cutting concerns (e.g., `useBalance`, `useAddress`)
- No Redux or global state management
- Props drilling for nested components

**Database Access (Rust):**
- Centralized via `WalletDatabase` in `AppState`
- Repositories pattern: `WalletRepository`, `AddressRepository`, `UtxoRepository`
- Connection pooling via `Mutex` (single-threaded access)
- Migrations in `src/database/migrations.rs`

---

*Convention analysis: 2026-01-28*
