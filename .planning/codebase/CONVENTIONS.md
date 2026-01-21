# Coding Conventions

**Analysis Date:** 2026-01-20

## Naming Patterns

**Files:**
- TypeScript components: PascalCase.tsx (`WalletPanel.tsx`, `BRC100AuthModal.tsx`)
- TypeScript hooks: camelCase with `use` prefix (`useHodosBrowser.ts`, `useBalance.ts`)
- TypeScript types: camelCase.d.ts for ambient (`hodosBrowser.d.ts`), PascalCase.ts for exports (`TabTypes.ts`)
- Rust modules: snake_case.rs (`wallet_repo.rs`, `certificate_handlers.rs`)
- C++ files: PascalCase.cpp/.h (`HttpRequestInterceptor.cpp`, `AddressHandler.h`)
- macOS-specific C++: `*_mac.mm` suffix (`TabManager_mac.mm`)

**Functions:**
- TypeScript: camelCase (`generateAddress`, `markBackedUp`, `getIdentity`)
- TypeScript event handlers: `handle` prefix (`handleApprove`, `handleReject`)
- TypeScript callbacks: `on` prefix (`onApprove`, `onClose`)
- Rust: snake_case (`derive_child_private_key`, `compute_shared_secret`)
- C++ methods: PascalCase (CEF convention) (`Execute`, `GetResponseHeaders`)

**Variables:**
- TypeScript: camelCase (`whitelistDomain`, `loading`, `error`)
- Rust: snake_case (`security_level`, `protocol_id`)
- Rust constants: SCREAMING_SNAKE_CASE (`DEFAULT_SATS_PER_KB`, `MIN_FEE_SATS`)
- C++ member variables: trailing underscore (`responseData_`, `handler_`)

**Types:**
- TypeScript interfaces: PascalCase, Props suffix for component props (`BRC100AuthModalProps`)
- Rust structs: PascalCase (`WalletRepository`, `InvoiceNumber`)
- Rust errors: PascalCase with Error suffix (`Brc42Error`)
- C++ classes: PascalCase (`AsyncWalletResourceHandler`, `DomainVerifier`)

## Code Style

**Formatting:**
- TypeScript: 2-space indentation, single quotes, semicolons required
- Rust: 4-space indentation (rustfmt defaults)
- C++: 4-space indentation
- No Prettier config (relies on ESLint + tsc)
- No rustfmt.toml (uses defaults)
- No .clang-format (manual formatting)

**Linting:**
- TypeScript: ESLint v9+ flat config (`frontend/eslint.config.js`)
- TypeScript: typescript-eslint, react-hooks, react-refresh plugins
- TypeScript: Strict mode in tsconfig (`noUnusedLocals`, `noUnusedParameters`)
- Rust: `cargo clippy` available but not enforced
- Run: `npm run lint` (frontend), `cargo check` (Rust)

## Import Organization

**TypeScript Pattern:**
```typescript
// Framework imports
import React, { useState, useEffect, useCallback } from 'react';

// UI library imports
import { Button, Dialog, DialogTitle } from '@mui/material';
import SendIcon from '@mui/icons-material/Send';

// Type imports (using import type)
import type { IdentityResult } from '../types/identity';

// Local imports
import { useHodosBrowser } from '../hooks/useHodosBrowser';
import { AddressData } from '../types/address';
```

**Rust Pattern:**
```rust
// External crates
use actix_web::{web, App, HttpServer};
use actix_cors::Cors;

// Standard library
use std::path::PathBuf;

// Module declarations
mod handlers;
mod crypto;
mod database;

// Local re-exports
use domain_whitelist::DomainWhitelistManager;
use database::WalletDatabase;
```

**Path Aliases:**
- No path aliases configured (uses relative imports `../`)

## Error Handling

**Patterns:**
- Rust: Custom error types with `thiserror::Error` derive
- Rust: `.map_err()` chains for context
- Rust: `HttpResponse::BadRequest().json()` for HTTP errors
- TypeScript: try/catch in async hooks
- TypeScript: Error state in React components
- C++: Logging via `Logger` class, error codes in IPC

**Error Types:**
- Rust: Throw via `Err()`, return `Result<T, E>`
- TypeScript: Throw `Error`, catch at hook boundaries
- HTTP responses: `{"error": "message"}` JSON format

**Example (Rust):**
```rust
let protocol_id = match normalize_protocol_id(&protocol_id_str) {
    Ok(p) => p,
    Err(e) => {
        log::error!("Failed to normalize protocol ID: {}", e);
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Invalid protocol ID: {}", e)
        }));
    }
};
```

## Logging

**Framework:**
- Rust: `env_logger` with log levels (debug, info, warn, error)
- C++: Custom `Logger` class with timestamps
- TypeScript: `console.log/error/warn`

**Patterns:**
- Emoji prefixes used throughout for visual scanning:
  - `console.log('This is a message')` - Informational
  - `console.error('Error message')` - Errors
  - `log::info!("Message")` - Rust info level
- Log at service boundaries, state transitions, external calls
- Heavy debug logging (304+ console statements in frontend)

## Comments

**When to Comment:**
- Explain BRC protocol steps (e.g., "BRC-53: Step 2 - Certificate Signing Request")
- Document algorithm implementations with references to specs
- Mark incomplete implementations with TODO
- Avoid obvious comments

**Rust Doc Comments:**
```rust
//! Module-level documentation
//! Describes the module's purpose

/// Function documentation
/// Explains parameters and return values
pub fn derive_child_private_key(...) -> Result<...> {
```

**TODO Comments:**
- Format: `// TODO: description` (no username, use git blame)
- Often include context: `// TODO: Implement BRC-100 auth on macOS`

## Function Design

**Size:**
- Rust handlers: Some very large functions (need refactoring)
- `rust-wallet/src/handlers.rs` is 7500+ lines
- Target: Extract smaller, focused functions

**Parameters:**
- TypeScript: Destructure objects when multiple params
- Rust: Use structs for request bodies (`web::Json<Request>`)
- C++: Follow CEF callback signatures

**Return Values:**
- TypeScript: `Promise<T>` for async, explicit types
- Rust: `Result<T, E>` pattern, `HttpResponse` for handlers
- Always return early for guard clauses

## Module Design

**Exports:**
- TypeScript: Named exports preferred
- TypeScript: Default exports for React components
- Rust: `pub mod` for public modules, `pub use` for re-exports

**Barrel Files:**
- Not heavily used in this codebase
- Rust: `mod.rs` for module re-exports
- TypeScript: No `index.ts` barrel pattern observed

## Platform-Specific Code

**C++ Pattern:**
```cpp
#ifdef _WIN32
    // Windows-specific code
#endif

#ifdef __APPLE__
    // macOS-specific code
#endif
```

**File Pattern:**
- Shared: `FileName.cpp`
- macOS only: `FileName_mac.mm`

---

*Convention analysis: 2026-01-20*
*Update when patterns change*
