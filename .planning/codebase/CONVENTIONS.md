# Coding Conventions

**Analysis Date:** 2026-01-24

## Naming Patterns

**Files:**
- React components: `PascalCase.tsx` (`WalletPanel.tsx`, `BRC100AuthModal.tsx`, `TransactionForm.tsx`)
- Custom hooks: `camelCase.ts` with `use` prefix (`useBalance.ts`, `useAddress.ts`, `useHodosBrowser.ts`)
- Type definitions: `camelCase.d.ts` (`transaction.d.ts`, `identity.d.ts`, `hodosBrowser.d.ts`)
- Rust modules: `snake_case.rs` (`handlers.rs`, `wallet_repo.rs`, `beef_helpers.rs`, `utxo_sync.rs`)
- C++ files: `snake_case.cpp/.h` (`simple_handler.cpp`, `simple_render_process_handler.cpp`)

**Functions:**
- TypeScript: `camelCase` (`handleAuthApprove`, `generateAndCopy`, `fetchBalance`)
- React hooks: `use[Name]` prefix (`useBalance`, `useHodosBrowser`, `useTransaction`)
- Rust: `snake_case` (`derive_child_private_key`, `compute_shared_secret`, `estimate_transaction_size`)
- C++: `camelCase` or `PascalCase` depending on context (`Execute`, `OnContextCreated`, `escapeJsonForJs`)

**Variables:**
- TypeScript: `camelCase` (`balance`, `authModalOpen`, `transactionResult`)
- React state setters: `set[PropertyName]` (`setBalance`, `setAuthModalOpen`, `setShowSendForm`)
- Rust: `snake_case` (`master_privkey`, `child_key`, `transaction_size`)
- C++ globals: `g_[name]` prefix (`g_hwnd`, `g_header_hwnd`, `g_webview_hwnd`, `g_settings_overlay_hwnd`)

**Types:**
- TypeScript interfaces: `PascalCase`, no `I` prefix (`IdentityResult`, `AddressData`, `TransactionResponse`)
- TypeScript type aliases: `PascalCase` (`WalletPanelProps`, `BRC100AuthModalProps`)
- Rust structs: `PascalCase` (`AppState`, `WalletDatabase`, `Brc42Error`, `SecurityLevel`)
- Rust enums: `PascalCase` for name, `PascalCase` for variants (`SecurityLevel::NoPermissions`)
- Rust constants: `SCREAMING_SNAKE_CASE` (`DEFAULT_SATS_PER_KB`, `MIN_FEE_SATS`)
- C++ classes: `PascalCase` (`CefMessageSendHandler`, `DomainVerifier`, `AsyncWalletResourceHandler`)

## Code Style

**Formatting:**
- TypeScript/React: 2 spaces indentation (inferred from code)
- Rust: 4 spaces indentation (standard Rust)
- C++: 4 spaces indentation (inferred from code)
- No Prettier configuration found
- Line length: ~100-120 characters (no explicit config)

**Quotes:**
- TypeScript: Single quotes for strings (`'Hello'`, `'navigate'`)
- Rust: Single quotes for chars, double quotes for strings (`"Hello"`)
- C++: Double quotes preferred (`"message"`)

**Semicolons:**
- TypeScript: Required (enforced by ESLint)
- Rust: Per statement (standard Rust)
- C++: Per statement (standard C++)

**Linting:**
- ESLint 9.25.0 with TypeScript support (`frontend/eslint.config.js`)
- Plugins: `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh`, `typescript-eslint`
- Run command: `npm run lint`
- No Rust linter configuration (uses cargo clippy by default)
- No C++ linter configuration

## Import Organization

**TypeScript Order:**
1. React and React-related packages
2. Third-party libraries (MUI, React Router)
3. Local modules (bridge, hooks, components)
4. Type imports (inferred if used)

**Grouping:**
- No blank lines between groups (no explicit separation)
- No alphabetical sorting enforced

**Path Aliases:**
- No path aliases configured (uses relative imports: `./`, `../`)

**Rust:**
- External crates first (actix-web, serde, tokio)
- Internal modules second (crypto, database, transaction)
- Standard library third (std::collections, std::sync)

**C++:**
- CEF headers first (#include "include/...")
- System headers second (<windows.h>, <iostream>)
- Third-party headers third (OpenSSL, nlohmann_json)

## Error Handling

**TypeScript Patterns:**
- Try/catch on async functions
- Promise.catch() for rejected promises
- `instanceof Error` checks for error type detection
- Error logging with console.error()

Example from `frontend/src/hooks/useHodosBrowser.ts`:
```typescript
try {
  const result = await someOperation();
  return result;
} catch (error) {
  console.error('Operation failed:', error);
  throw error;
}
```

**Rust Patterns:**
- `Result<T, Error>` for fallible operations
- Custom error types with `thiserror` crate
- `.map_err()` for error transformation
- **Critical Issue**: 61 instances of `.lock().unwrap()` on mutex (panics if poisoned)

Example from `rust-wallet/src/crypto/brc42.rs`:
```rust
pub fn compute_shared_secret(
    private_key: &[u8],
    public_key: &[u8],
) -> Result<Vec<u8>, Brc42Error> {
    if private_key.len() != 32 {
        return Err(Brc42Error::InvalidPrivateKey("...".to_string()));
    }
    // ...
}
```

**C++ Patterns:**
- Exception handling at boundaries
- Logging errors with LOG_ERROR macro
- Return codes for some operations

## Logging

**Framework:**
- TypeScript: `console.log`, `console.error`
- Rust: `log` crate with `env_logger` (info!, debug!, error! macros)
- C++: Custom `Logger` class in `cef-native/src/core/Logger.cpp`

**Patterns:**
- **Emoji-based logging**: Extensive use of emojis for visual clarity
  - 🔍 (search/inspection)
  - 🔐 (security/crypto)
  - 🦀 (Rust-specific)
  - ✅ (success)
  - ❌ (failure)
  - 💥 (error)
  - 🔑 (keys/auth)
  - 📤 (sending/output)
  - 📥 (receiving/input)
- **Descriptive messages**: Include context (e.g., `"🔍 Bridge: window.hodosBrowser:"`)
- **Log at boundaries**: Service boundaries, external calls, state transitions

**TypeScript Example:**
```typescript
console.log('🔍 Fetching balance from wallet...');
console.error('❌ Failed to fetch balance:', error);
```

**Rust Example:**
```rust
log::info!("✅ Address generated: {}", address);
log::error!("💥 Failed to derive key: {:?}", error);
```

**C++ Example:**
```cpp
LOG_INFO("✅ Window created successfully");
LOG_ERROR("❌ Failed to initialize CEF");
```

## Comments

**When to Comment:**
- Explain "why", not "what" (code should be self-explanatory)
- Document business rules and protocol requirements
- Reference BRC specifications in crypto modules
- Mark TODOs with clear action items

**TypeScript/JavaScript:**
- Minimal comments (code clarity preferred)
- TODO comments for missing features
- No JSDoc by default (type system provides documentation)

**Rust:**
- `//!` for module-level documentation with spec references
- `///` for function documentation with examples
- Inline comments for complex algorithms

Example from `rust-wallet/src/crypto/brc42.rs`:
```rust
/// Compute ECDH shared secret between sender's private key and recipient's public key
///
/// **BRC-42 Spec**: Step 1 for both sender and recipient
///
/// # Arguments
/// * `private_key` - 32-byte private key
/// * `public_key` - 33-byte compressed public key
```

**C++:**
- `//` for inline comments
- `/* */` for multi-line comments
- Minimal comments (self-documenting code)

**TODO Comments:**
- Format: `// TODO: description` (no username)
- Link to issue if applicable: `// TODO: Fix race condition (issue #123)`
- Examples:
  - `rust-wallet/src/handlers.rs` line 312: `// TODO: Add nonce tracking to prevent replay attacks`
  - `rust-wallet/src/handlers.rs` line 149: `// TODO: Dynamic fee rate fetching from MAPI`

## Function Design

**Size:**
- TypeScript: Keep components under ~200 lines
- Rust: Functions typically under 50 lines (handlers.rs violates this extensively)
- Extract helpers for complex logic

**Parameters:**
- TypeScript: Destructure object parameters for React components
  ```typescript
  export default function WalletPanel({ onClose }: WalletPanelProps) { ... }
  ```
- Rust: Max 3-4 parameters, use struct for more
  ```rust
  pub fn create_transaction(inputs: Vec<Input>, outputs: Vec<Output>, fee: u64) { ... }
  ```

**Return Values:**
- TypeScript: Explicit returns, early returns for guard clauses
- Rust: Implicit return for last expression, explicit `return` for early exit
- C++: Explicit returns, void for side-effect functions

## Module Design

**TypeScript Exports:**
- Named exports preferred: `export function generateAddress() { ... }`
- Default exports for React components: `export default function WalletPanel() { ... }`
- No barrel files (`index.ts` not used)

**Rust Exports:**
- Public API via `pub` keyword
- Module re-exports in `mod.rs`: `pub use self::brc42::*;`
- Internal functions without `pub`

**C++ Exports:**
- Header files (.h) declare interfaces
- Implementation files (.cpp) define behavior
- No explicit export lists

## Patterns & Idioms

**TypeScript/React:**
- Functional components with hooks (no class components)
- useState for local state
- useCallback for memoized callbacks
- Ternary operators for conditional rendering
- Destructuring in function parameters

**Rust:**
- Iterator adapters and functional style
- Arc<Mutex<T>> for shared mutable state
- Result-based error handling
- Documentation comments with examples
- Module organization with public re-exports

**C++:**
- Reference counting with CefRefPtr<T>
- Macro-based logging
- Handler classes inheriting from CEF interfaces
- Global state for window handles (g_* prefix)

---

*Convention analysis: 2026-01-24*
*Update when patterns change*
