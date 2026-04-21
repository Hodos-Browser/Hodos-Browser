# BSV-21 Implementation Plan B: Frontend/UI

---

## ⚠️ PENDING: UI/UX DESIGN REVIEW

**This plan contains implementation code, but the UI/UX design is NOT finalized.**

Before implementing:
1. Review `BSV21_UX_DESIGN_OUTLINE.md` with the design team
2. Decide on: Panel vs. Page vs. Hybrid approach
3. Create wireframes/mockups for approval
4. Update this plan with approved designs

**Key design decisions needed:**
- Where does token display live? (wallet panel, dedicated page, or both)
- How to display different token types (BSV-21, STAS, NFTs, stablecoins)
- Basket organization and visibility
- Transaction history scope and display

See: `development-docs/BSV21_UX_DESIGN_OUTLINE.md`

---

## Overview

This plan covers the React frontend implementation for BSV-21 token display and management. It can be developed in parallel with Plan A using mock data.

**Goal**: Display tokens, show balances, enable transfers via UI

**Developer**: Frontend/React focused
**Dependencies**: Plan A endpoints (can mock initially), **UI/UX design approval**
**Testing**: Vite dev server with mock responses

---

## Key Design Decisions

### 1. Integration with createAction (Matches Plan A)

Token transfers go through the existing `/createAction` endpoint, not a separate `/ordinals/transfer`. This:
- Maintains consistency with existing wallet operations
- Reuses the same signing/broadcasting infrastructure
- Allows unified transaction handling

### 2. Big Number Handling

Token amounts can exceed JavaScript's `Number.MAX_SAFE_INTEGER` (2^53). We:
- Keep amounts as **strings** in all state and API calls
- Use **BigInt** only for arithmetic comparisons
- Format for display using string manipulation (not floating point)

### 3. State Management

Token state is managed alongside existing wallet state:
- Tokens refresh when wallet address changes
- Token UTXOs update after transfers (local optimistic update + backend sync)
- Use existing wallet context where possible

### 4. Error Handling

Match backend error types for clear user feedback:
- Validation errors (invalid address, amount)
- Insufficient balance errors
- Network/API errors
- Transaction failures

---

## Architecture

```
frontend/src/
├── components/
│   ├── tokens/
│   │   ├── TokenList.tsx           # List of user's tokens
│   │   ├── TokenCard.tsx           # Individual token display
│   │   ├── TokenDetail.tsx         # Full token info modal/page
│   │   ├── TokenSendForm.tsx       # Transfer form
│   │   ├── TokenIcon.tsx           # Token icon display
│   │   ├── TokenTransactionHistory.tsx  # Token tx history
│   │   └── index.ts                # Barrel export
│   └── panels/
│       └── WalletPanelContent.tsx  # Modify to include tokens tab
├── hooks/
│   ├── useTokens.ts                # Fetch user's tokens
│   ├── useTokenBalance.ts          # Single token balance
│   ├── useTokenTransfer.ts         # Transfer mutation
│   └── useTokenHistory.ts          # Token transaction history
├── utils/
│   ├── tokenAmount.ts              # Big number formatting utilities
│   ├── tokenValidation.ts          # Input validation
│   └── tokenErrors.ts              # Error type handling
├── types/
│   └── tokens.d.ts                 # Token type definitions
├── mocks/
│   └── tokens.ts                   # Mock data for development
└── bridge/
    └── ordinals.ts                 # Bridge to wallet endpoints
```

---

## Phase 1: Type Definitions & Bridge

**Goal**: Define types and create bridge to wallet endpoints

### Tasks

- [ ] Create `frontend/src/types/tokens.d.ts`
- [ ] Create `frontend/src/bridge/ordinals.ts`
- [ ] Define token types matching backend responses
- [ ] Implement bridge functions with error handling

### Type Definitions

```typescript
// frontend/src/types/tokens.d.ts

// ============================================
// Token Balance & Info Types
// ============================================

/**
 * Token balance as returned by /ordinals/balance endpoint
 * Matches TokenUtxo from Plan A
 */
export interface TokenBalance {
  tokenId: string;        // Token ID (deploy txid_vout, e.g., "abc123...def_0")
  symbol: string | null;  // Token symbol (e.g., "TEST")
  amount: string;         // Raw amount as string (for big numbers)
  decimals: number;       // Decimal places (0-18)
  icon: string | null;    // Icon inscription origin
  utxoCount?: number;     // Number of UTXOs holding this token (optional)
}

/**
 * Detailed token metadata from /ordinals/token/{id}
 * Matches TokenMetadata from Plan A
 */
export interface TokenMetadata {
  tokenId: string;
  symbol: string | null;
  decimals: number;
  iconOrigin: string | null;
  maxSupply: string | null;
  deployHeight: number | null;
  cachedAt: string | null;  // ISO timestamp
}

/**
 * Token info from GorillaPool (external API response)
 */
export interface GorillaPoolTokenInfo {
  id: string;
  sym: string | null;
  max: string | null;
  dec: number;
  icon: string | null;
  height: number | null;
}

// ============================================
// Transfer Types (Integration with createAction)
// ============================================

/**
 * Token distribution for a transfer
 * Matches TokenDistribution from Plan A
 */
export interface TokenDistribution {
  address: string;        // Recipient BSV address
  amount: string;         // Raw amount as string
  omitMetadata?: boolean; // Skip inscription for this output (privacy)
}

/**
 * Token input mode - how to consume token UTXOs
 */
export type TokenInputMode = 'needed' | 'all';

/**
 * Configuration for splitting token change outputs
 */
export interface TokenSplitConfig {
  outputs: number;          // Number of change outputs (1-10)
  threshold?: string;       // Minimum tokens per output
  omitMetadata?: boolean;   // Skip inscription on change outputs
}

/**
 * Token transfer specification (sent to createAction)
 * Matches TokenTransferSpec from Plan A
 */
export interface TokenTransferSpec {
  tokenId: string;
  distributions: TokenDistribution[];
  inputMode?: TokenInputMode;
  splitConfig?: TokenSplitConfig;
  burn?: boolean;           // Burn remaining tokens (don't create change)
}

/**
 * Extended createAction request with token transfer
 */
export interface CreateActionWithTokenRequest {
  description: string;
  tokenTransfer: TokenTransferSpec;
}

/**
 * Result from createAction
 */
export interface CreateActionResult {
  txid: string;
  rawTx?: string;
}

// ============================================
// API Response Types
// ============================================

export interface TokensResponse {
  tokens: TokenBalance[];
  source: 'local' | 'gorillapool' | 'cache';
  cached?: boolean;
  cacheAgeSecs?: number;
}

export interface TokenTransferResult {
  success: boolean;
  txid?: string;
  error?: TokenTransferError;
}

// ============================================
// Error Types (Match Plan A OrdinalError)
// ============================================

export type TokenErrorCode =
  | 'INSUFFICIENT_BALANCE'
  | 'INSUFFICIENT_FUNDS'
  | 'INVALID_ADDRESS'
  | 'INVALID_AMOUNT'
  | 'ZERO_AMOUNT'
  | 'NO_TOKEN_UTXOS'
  | 'API_ERROR'
  | 'NETWORK_ERROR'
  | 'SIGNING_FAILED'
  | 'BROADCAST_FAILED'
  | 'VALIDATION_ERROR';

export interface TokenTransferError {
  code: TokenErrorCode;
  message: string;
  details?: {
    available?: string;
    required?: string;
    field?: string;
  };
}

// ============================================
// UI State Types
// ============================================

export interface TokenListState {
  tokens: TokenBalance[];
  loading: boolean;
  error: string | null;
  lastRefresh: number | null;
}

export interface TokenTransferState {
  step: 'form' | 'confirm' | 'sending' | 'success' | 'error';
  loading: boolean;
  error: TokenTransferError | null;
  txid: string | null;
}

export interface SelectedToken {
  token: TokenBalance;
  metadata: TokenMetadata | null;
}
```

### Bridge Implementation

```typescript
// frontend/src/bridge/ordinals.ts

import type {
  TokenBalance,
  TokenMetadata,
  TokensResponse,
  TokenTransferSpec,
  TokenTransferResult,
  TokenTransferError,
  TokenErrorCode,
  CreateActionWithTokenRequest,
  CreateActionResult,
} from '../types/tokens';

const WALLET_BASE = 'http://localhost:3301';
const GORILLAPOOL_BASE = 'https://ordinals.gorillapool.io/api';

// ============================================
// Query Endpoints
// ============================================

/**
 * Fetch all BSV-21 tokens for an address (via GorillaPool)
 */
export async function getTokensForAddress(address: string): Promise<TokenBalance[]> {
  const response = await fetch(`${WALLET_BASE}/ordinals/tokens/${address}`);

  if (!response.ok) {
    throw await parseErrorResponse(response);
  }

  return response.json();
}

/**
 * Fetch local token balances from wallet database
 * This is the primary endpoint - returns tokens we know we own
 */
export async function getLocalTokenBalances(): Promise<TokensResponse> {
  const response = await fetch(`${WALLET_BASE}/ordinals/balance`);

  if (!response.ok) {
    throw await parseErrorResponse(response);
  }

  return response.json();
}

/**
 * Fetch detailed metadata for a specific token
 */
export async function getTokenMetadata(tokenId: string): Promise<TokenMetadata> {
  const response = await fetch(
    `${WALLET_BASE}/ordinals/token/${encodeURIComponent(tokenId)}`
  );

  if (!response.ok) {
    throw await parseErrorResponse(response);
  }

  return response.json();
}

/**
 * Trigger a sync of token UTXOs from GorillaPool
 * Call this after receiving tokens or to refresh state
 */
export async function syncTokenUtxos(): Promise<{ added: number; removed: number }> {
  const response = await fetch(`${WALLET_BASE}/ordinals/sync`, {
    method: 'POST',
  });

  if (!response.ok) {
    throw await parseErrorResponse(response);
  }

  return response.json();
}

// ============================================
// Transfer Endpoint (via createAction)
// ============================================

/**
 * Transfer tokens using the createAction endpoint
 * This integrates with the existing BRC-100 infrastructure
 */
export async function transferTokens(
  tokenId: string,
  distributions: { address: string; amount: string }[],
  options?: {
    inputMode?: 'needed' | 'all';
    splitOutputs?: number;
    burn?: boolean;
  }
): Promise<TokenTransferResult> {
  const transferSpec: TokenTransferSpec = {
    tokenId,
    distributions: distributions.map((d) => ({
      address: d.address,
      amount: d.amount,
    })),
    inputMode: options?.inputMode,
    burn: options?.burn,
  };

  if (options?.splitOutputs && options.splitOutputs > 1) {
    transferSpec.splitConfig = {
      outputs: options.splitOutputs,
    };
  }

  const request: CreateActionWithTokenRequest = {
    description: `Transfer ${tokenId.slice(0, 8)}... tokens`,
    tokenTransfer: transferSpec,
  };

  try {
    const response = await fetch(`${WALLET_BASE}/createAction`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const error = await parseErrorResponse(response);
      return {
        success: false,
        error,
      };
    }

    const result: CreateActionResult = await response.json();
    return {
      success: true,
      txid: result.txid,
    };
  } catch (err) {
    return {
      success: false,
      error: {
        code: 'NETWORK_ERROR',
        message: err instanceof Error ? err.message : 'Network error',
      },
    };
  }
}

// ============================================
// Utility Functions
// ============================================

/**
 * Get icon URL for a token inscription
 * Icons are stored as inscriptions, served via GorillaPool
 */
export function getTokenIconUrl(iconOrigin: string | null): string | null {
  if (!iconOrigin) return null;

  // GorillaPool serves inscription content
  return `${GORILLAPOOL_BASE}/inscriptions/origin/${iconOrigin}/content`;
}

/**
 * Parse error response from backend into typed error
 */
async function parseErrorResponse(response: Response): Promise<TokenTransferError> {
  let message = response.statusText;
  let code: TokenErrorCode = 'API_ERROR';

  try {
    const body = await response.json();

    if (body.message) {
      message = body.message;
    }

    // Map backend error messages to error codes
    if (message.includes('Insufficient token balance')) {
      code = 'INSUFFICIENT_BALANCE';
    } else if (message.includes('Insufficient funds')) {
      code = 'INSUFFICIENT_FUNDS';
    } else if (message.includes('Invalid address')) {
      code = 'INVALID_ADDRESS';
    } else if (message.includes('Invalid amount') || message.includes('parse')) {
      code = 'INVALID_AMOUNT';
    } else if (message.includes('Zero amount')) {
      code = 'ZERO_AMOUNT';
    } else if (message.includes('No token UTXOs')) {
      code = 'NO_TOKEN_UTXOS';
    } else if (message.includes('signing')) {
      code = 'SIGNING_FAILED';
    } else if (message.includes('broadcast')) {
      code = 'BROADCAST_FAILED';
    }

    return {
      code,
      message,
      details: body.details,
    };
  } catch {
    return {
      code,
      message,
    };
  }
}

/**
 * Check if GorillaPool API is healthy
 */
export async function checkGorillaPoolHealth(): Promise<boolean> {
  try {
    const response = await fetch(`${GORILLAPOOL_BASE}/health`, {
      method: 'GET',
      signal: AbortSignal.timeout(5000),
    });
    return response.ok;
  } catch {
    return false;
  }
}
```

---

## Phase 1.5: Utility Functions

**Goal**: Centralize big number handling, validation, and error utilities

### Tasks

- [ ] Create `frontend/src/utils/tokenAmount.ts`
- [ ] Create `frontend/src/utils/tokenValidation.ts`
- [ ] Create `frontend/src/utils/tokenErrors.ts`

### Big Number Formatting Utilities

```typescript
// frontend/src/utils/tokenAmount.ts

/**
 * Token amount utilities for handling big numbers safely.
 *
 * IMPORTANT: Token amounts can exceed Number.MAX_SAFE_INTEGER (2^53).
 * We keep amounts as strings and only use BigInt for comparisons.
 */

/**
 * Format a raw token amount for display with decimals
 *
 * @param rawAmount - The raw amount as a string (e.g., "100000000")
 * @param decimals - Number of decimal places (e.g., 8)
 * @returns Formatted string (e.g., "1.0" for amount=100000000, decimals=8)
 */
export function formatTokenAmount(rawAmount: string, decimals: number): string {
  if (decimals === 0) {
    return addThousandsSeparators(rawAmount);
  }

  // Ensure we have enough digits
  const padded = rawAmount.padStart(decimals + 1, '0');
  const intPart = padded.slice(0, -decimals) || '0';
  const decPart = padded.slice(-decimals);

  // Trim trailing zeros from decimal part
  const trimmedDec = decPart.replace(/0+$/, '');

  const formatted = trimmedDec ? `${intPart}.${trimmedDec}` : intPart;
  return addThousandsSeparators(formatted);
}

/**
 * Parse a display amount back to raw amount string
 *
 * @param displayAmount - User-entered amount (e.g., "1.5")
 * @param decimals - Number of decimal places
 * @returns Raw amount string (e.g., "150000000" for 1.5 with 8 decimals)
 */
export function parseDisplayAmount(displayAmount: string, decimals: number): string {
  // Remove thousands separators
  const cleaned = displayAmount.replace(/,/g, '');

  if (decimals === 0) {
    // Ensure it's a valid integer
    if (cleaned.includes('.')) {
      throw new Error('Token does not support decimals');
    }
    return cleaned;
  }

  const [intPart, decPart = ''] = cleaned.split('.');

  // Pad or truncate decimal part to match token decimals
  const paddedDec = decPart.padEnd(decimals, '0').slice(0, decimals);

  // Combine and remove leading zeros
  const raw = (intPart + paddedDec).replace(/^0+/, '') || '0';

  return raw;
}

/**
 * Add thousands separators to a number string
 */
function addThousandsSeparators(numStr: string): string {
  const [intPart, decPart] = numStr.split('.');
  const withSeparators = intPart.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  return decPart !== undefined ? `${withSeparators}.${decPart}` : withSeparators;
}

/**
 * Compare two token amounts (as strings)
 *
 * @returns -1 if a < b, 0 if a === b, 1 if a > b
 */
export function compareAmounts(a: string, b: string): -1 | 0 | 1 {
  const bigA = BigInt(a || '0');
  const bigB = BigInt(b || '0');

  if (bigA < bigB) return -1;
  if (bigA > bigB) return 1;
  return 0;
}

/**
 * Check if amount a is less than or equal to amount b
 */
export function isLessOrEqual(a: string, b: string): boolean {
  return compareAmounts(a, b) <= 0;
}

/**
 * Check if amount is greater than zero
 */
export function isPositive(amount: string): boolean {
  try {
    return BigInt(amount || '0') > 0n;
  } catch {
    return false;
  }
}

/**
 * Subtract two amounts (a - b), returns "0" if result would be negative
 */
export function subtractAmounts(a: string, b: string): string {
  const bigA = BigInt(a || '0');
  const bigB = BigInt(b || '0');
  const result = bigA - bigB;
  return result > 0n ? result.toString() : '0';
}

/**
 * Add two amounts
 */
export function addAmounts(a: string, b: string): string {
  return (BigInt(a || '0') + BigInt(b || '0')).toString();
}

/**
 * Format token amount with symbol for display
 *
 * @example formatWithSymbol("1000000", 8, "TEST") => "0.01 TEST"
 */
export function formatWithSymbol(
  rawAmount: string,
  decimals: number,
  symbol: string | null
): string {
  const formatted = formatTokenAmount(rawAmount, decimals);
  return symbol ? `${formatted} ${symbol}` : formatted;
}

/**
 * Validate that a string is a valid amount input
 * (allows partial input during typing)
 */
export function isValidAmountInput(value: string, decimals: number): boolean {
  if (value === '' || value === '.') return true;

  // Allow numbers with optional single decimal point
  const regex = decimals > 0
    ? /^\d*\.?\d*$/
    : /^\d*$/;

  if (!regex.test(value)) return false;

  // Check decimal places don't exceed token decimals
  const [, decPart] = value.split('.');
  if (decPart && decPart.length > decimals) return false;

  return true;
}
```

### Validation Utilities

```typescript
// frontend/src/utils/tokenValidation.ts

import type { TokenBalance, TokenTransferError, TokenErrorCode } from '../types/tokens';
import { parseDisplayAmount, isPositive, isLessOrEqual } from './tokenAmount';

/**
 * Validation result type
 */
export interface ValidationResult {
  valid: boolean;
  error?: TokenTransferError;
}

/**
 * Validate a BSV address format
 * Basic validation - full validation happens on backend
 */
export function validateAddress(address: string): ValidationResult {
  // BSV addresses are Base58Check encoded
  // Mainnet P2PKH: starts with '1', 25-34 chars
  // Testnet P2PKH: starts with 'm' or 'n'

  if (!address || address.trim() === '') {
    return {
      valid: false,
      error: {
        code: 'INVALID_ADDRESS',
        message: 'Address is required',
        details: { field: 'address' },
      },
    };
  }

  const trimmed = address.trim();

  // Basic length check
  if (trimmed.length < 25 || trimmed.length > 34) {
    return {
      valid: false,
      error: {
        code: 'INVALID_ADDRESS',
        message: 'Invalid address length',
        details: { field: 'address' },
      },
    };
  }

  // Check for valid Base58 characters (no 0, O, I, l)
  const base58Regex = /^[1-9A-HJ-NP-Za-km-z]+$/;
  if (!base58Regex.test(trimmed)) {
    return {
      valid: false,
      error: {
        code: 'INVALID_ADDRESS',
        message: 'Address contains invalid characters',
        details: { field: 'address' },
      },
    };
  }

  // Check prefix (mainnet P2PKH starts with '1')
  if (!trimmed.startsWith('1') && !trimmed.startsWith('m') && !trimmed.startsWith('n')) {
    return {
      valid: false,
      error: {
        code: 'INVALID_ADDRESS',
        message: 'Invalid address format',
        details: { field: 'address' },
      },
    };
  }

  return { valid: true };
}

/**
 * Validate a transfer amount
 */
export function validateAmount(
  displayAmount: string,
  token: TokenBalance
): ValidationResult {
  if (!displayAmount || displayAmount.trim() === '') {
    return {
      valid: false,
      error: {
        code: 'INVALID_AMOUNT',
        message: 'Amount is required',
        details: { field: 'amount' },
      },
    };
  }

  let rawAmount: string;
  try {
    rawAmount = parseDisplayAmount(displayAmount, token.decimals);
  } catch (err) {
    return {
      valid: false,
      error: {
        code: 'INVALID_AMOUNT',
        message: err instanceof Error ? err.message : 'Invalid amount format',
        details: { field: 'amount' },
      },
    };
  }

  // Check positive
  if (!isPositive(rawAmount)) {
    return {
      valid: false,
      error: {
        code: 'ZERO_AMOUNT',
        message: 'Amount must be greater than zero',
        details: { field: 'amount' },
      },
    };
  }

  // Check against balance
  if (!isLessOrEqual(rawAmount, token.amount)) {
    return {
      valid: false,
      error: {
        code: 'INSUFFICIENT_BALANCE',
        message: 'Insufficient token balance',
        details: {
          available: token.amount,
          required: rawAmount,
          field: 'amount',
        },
      },
    };
  }

  return { valid: true };
}

/**
 * Validate a complete transfer request
 */
export function validateTransfer(
  address: string,
  displayAmount: string,
  token: TokenBalance
): ValidationResult {
  const addressResult = validateAddress(address);
  if (!addressResult.valid) {
    return addressResult;
  }

  const amountResult = validateAmount(displayAmount, token);
  if (!amountResult.valid) {
    return amountResult;
  }

  return { valid: true };
}

/**
 * Get user-friendly error message for display
 */
export function getErrorMessage(error: TokenTransferError): string {
  switch (error.code) {
    case 'INSUFFICIENT_BALANCE':
      return 'You don\'t have enough tokens for this transfer';
    case 'INSUFFICIENT_FUNDS':
      return 'Not enough BSV to pay transaction fees';
    case 'INVALID_ADDRESS':
      return error.message || 'Invalid recipient address';
    case 'INVALID_AMOUNT':
      return error.message || 'Invalid amount';
    case 'ZERO_AMOUNT':
      return 'Amount must be greater than zero';
    case 'NO_TOKEN_UTXOS':
      return 'No tokens available to transfer';
    case 'NETWORK_ERROR':
      return 'Network error - please check your connection';
    case 'SIGNING_FAILED':
      return 'Failed to sign transaction';
    case 'BROADCAST_FAILED':
      return 'Failed to broadcast transaction';
    default:
      return error.message || 'An error occurred';
  }
}
```

### Error Display Utilities

```typescript
// frontend/src/utils/tokenErrors.ts

import type { TokenTransferError, TokenErrorCode } from '../types/tokens';

/**
 * Error severity levels for UI display
 */
export type ErrorSeverity = 'error' | 'warning' | 'info';

/**
 * Get severity level for an error code
 */
export function getErrorSeverity(code: TokenErrorCode): ErrorSeverity {
  switch (code) {
    case 'INSUFFICIENT_BALANCE':
    case 'INSUFFICIENT_FUNDS':
    case 'INVALID_ADDRESS':
    case 'INVALID_AMOUNT':
    case 'ZERO_AMOUNT':
      return 'error';

    case 'NETWORK_ERROR':
    case 'API_ERROR':
      return 'warning';

    default:
      return 'error';
  }
}

/**
 * Check if an error is retryable
 */
export function isRetryableError(code: TokenErrorCode): boolean {
  return code === 'NETWORK_ERROR' || code === 'API_ERROR' || code === 'BROADCAST_FAILED';
}

/**
 * Check if error is a validation error (user can fix input)
 */
export function isValidationError(code: TokenErrorCode): boolean {
  return (
    code === 'INVALID_ADDRESS' ||
    code === 'INVALID_AMOUNT' ||
    code === 'ZERO_AMOUNT' ||
    code === 'INSUFFICIENT_BALANCE'
  );
}

/**
 * Create a standardized error from an unknown error
 */
export function normalizeError(err: unknown): TokenTransferError {
  if (isTokenTransferError(err)) {
    return err;
  }

  if (err instanceof Error) {
    return {
      code: 'API_ERROR',
      message: err.message,
    };
  }

  return {
    code: 'API_ERROR',
    message: String(err),
  };
}

/**
 * Type guard for TokenTransferError
 */
export function isTokenTransferError(err: unknown): err is TokenTransferError {
  return (
    typeof err === 'object' &&
    err !== null &&
    'code' in err &&
    'message' in err
  );
}
```

---

## Phase 2: React Hooks

**Goal**: Create reusable hooks for token data

### Tasks

- [ ] Create `frontend/src/hooks/useTokens.ts`
- [ ] Create `frontend/src/hooks/useTokenBalance.ts`
- [ ] Create `frontend/src/hooks/useTokenTransfer.ts`
- [ ] Implement loading/error states
- [ ] Add refresh capability

### useTokens Hook

```typescript
// frontend/src/hooks/useTokens.ts

import { useState, useEffect, useCallback, useRef } from 'react';
import type { TokenBalance, TokensResponse } from '../types/tokens';
import { getLocalTokenBalances, syncTokenUtxos } from '../bridge/ordinals';
import { normalizeError } from '../utils/tokenErrors';

interface UseTokensResult {
  tokens: TokenBalance[];
  loading: boolean;
  syncing: boolean;
  error: string | null;
  source: 'local' | 'gorillapool' | 'cache' | null;
  lastRefresh: number | null;
  refresh: () => Promise<void>;
  sync: () => Promise<void>;
}

interface UseTokensOptions {
  /** Auto-refresh interval in milliseconds (0 to disable) */
  autoRefreshMs?: number;
  /** Sync on mount */
  syncOnMount?: boolean;
}

export function useTokens(options: UseTokensOptions = {}): UseTokensResult {
  const { autoRefreshMs = 0, syncOnMount = false } = options;

  const [tokens, setTokens] = useState<TokenBalance[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [source, setSource] = useState<'local' | 'gorillapool' | 'cache' | null>(null);
  const [lastRefresh, setLastRefresh] = useState<number | null>(null);

  const mountedRef = useRef(true);

  // Fetch tokens from local database
  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const response: TokensResponse = await getLocalTokenBalances();

      if (mountedRef.current) {
        setTokens(response.tokens);
        setSource(response.source);
        setLastRefresh(Date.now());
      }
    } catch (err) {
      if (mountedRef.current) {
        const normalized = normalizeError(err);
        setError(normalized.message);
      }
    } finally {
      if (mountedRef.current) {
        setLoading(false);
      }
    }
  }, []);

  // Sync from GorillaPool and refresh
  const sync = useCallback(async () => {
    setSyncing(true);

    try {
      await syncTokenUtxos();
      await refresh();
    } catch (err) {
      if (mountedRef.current) {
        const normalized = normalizeError(err);
        setError(normalized.message);
      }
    } finally {
      if (mountedRef.current) {
        setSyncing(false);
      }
    }
  }, [refresh]);

  // Initial load
  useEffect(() => {
    mountedRef.current = true;

    if (syncOnMount) {
      sync();
    } else {
      refresh();
    }

    return () => {
      mountedRef.current = false;
    };
  }, [syncOnMount, sync, refresh]);

  // Auto-refresh
  useEffect(() => {
    if (autoRefreshMs <= 0) return;

    const interval = setInterval(refresh, autoRefreshMs);
    return () => clearInterval(interval);
  }, [autoRefreshMs, refresh]);

  return {
    tokens,
    loading,
    syncing,
    error,
    source,
    lastRefresh,
    refresh,
    sync,
  };
}
```

### useTokenBalance Hook

```typescript
// frontend/src/hooks/useTokenBalance.ts

import { useState, useEffect, useCallback } from 'react';
import type { TokenBalance, TokenMetadata } from '../types/tokens';
import { getTokenMetadata } from '../bridge/ordinals';
import { formatTokenAmount } from '../utils/tokenAmount';
import { normalizeError } from '../utils/tokenErrors';

interface UseTokenBalanceResult {
  token: TokenBalance | null;
  metadata: TokenMetadata | null;
  formattedBalance: string;
  loading: boolean;
  error: string | null;
  refreshMetadata: () => Promise<void>;
}

/**
 * Hook for working with a single token's balance and metadata
 */
export function useTokenBalance(
  tokens: TokenBalance[],
  tokenId: string | null
): UseTokenBalanceResult {
  const [metadata, setMetadata] = useState<TokenMetadata | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Find the token in the list
  const token = tokenId
    ? tokens.find((t) => t.tokenId === tokenId) || null
    : null;

  // Formatted balance
  const formattedBalance = token
    ? formatTokenAmount(token.amount, token.decimals)
    : '0';

  // Fetch metadata
  const refreshMetadata = useCallback(async () => {
    if (!tokenId) {
      setMetadata(null);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const data = await getTokenMetadata(tokenId);
      setMetadata(data);
    } catch (err) {
      const normalized = normalizeError(err);
      setError(normalized.message);
    } finally {
      setLoading(false);
    }
  }, [tokenId]);

  // Fetch metadata when tokenId changes
  useEffect(() => {
    if (tokenId) {
      refreshMetadata();
    } else {
      setMetadata(null);
    }
  }, [tokenId, refreshMetadata]);

  return {
    token,
    metadata,
    formattedBalance,
    loading,
    error,
    refreshMetadata,
  };
}
```

### useTokenTransfer Hook

```typescript
// frontend/src/hooks/useTokenTransfer.ts

import { useState, useCallback } from 'react';
import type {
  TokenBalance,
  TokenTransferResult,
  TokenTransferError,
  TokenTransferState,
} from '../types/tokens';
import { transferTokens } from '../bridge/ordinals';
import { parseDisplayAmount } from '../utils/tokenAmount';
import { validateTransfer, getErrorMessage } from '../utils/tokenValidation';
import { normalizeError, isRetryableError } from '../utils/tokenErrors';

interface UseTokenTransferOptions {
  onSuccess?: (txid: string) => void;
  onError?: (error: TokenTransferError) => void;
}

interface UseTokenTransferResult {
  state: TokenTransferState;
  transfer: (
    token: TokenBalance,
    address: string,
    displayAmount: string
  ) => Promise<TokenTransferResult>;
  reset: () => void;
  canRetry: boolean;
}

const initialState: TokenTransferState = {
  step: 'form',
  loading: false,
  error: null,
  txid: null,
};

export function useTokenTransfer(
  options: UseTokenTransferOptions = {}
): UseTokenTransferResult {
  const { onSuccess, onError } = options;

  const [state, setState] = useState<TokenTransferState>(initialState);

  const transfer = useCallback(
    async (
      token: TokenBalance,
      address: string,
      displayAmount: string
    ): Promise<TokenTransferResult> => {
      // Validate first
      const validation = validateTransfer(address, displayAmount, token);
      if (!validation.valid && validation.error) {
        setState({
          step: 'error',
          loading: false,
          error: validation.error,
          txid: null,
        });
        onError?.(validation.error);
        return { success: false, error: validation.error };
      }

      // Convert to raw amount
      const rawAmount = parseDisplayAmount(displayAmount, token.decimals);

      setState({
        step: 'sending',
        loading: true,
        error: null,
        txid: null,
      });

      try {
        const result = await transferTokens(token.tokenId, [
          { address, amount: rawAmount },
        ]);

        if (result.success && result.txid) {
          setState({
            step: 'success',
            loading: false,
            error: null,
            txid: result.txid,
          });
          onSuccess?.(result.txid);
        } else {
          const error = result.error || {
            code: 'API_ERROR' as const,
            message: 'Transfer failed',
          };
          setState({
            step: 'error',
            loading: false,
            error,
            txid: null,
          });
          onError?.(error);
        }

        return result;
      } catch (err) {
        const error = normalizeError(err);
        setState({
          step: 'error',
          loading: false,
          error,
          txid: null,
        });
        onError?.(error);
        return { success: false, error };
      }
    },
    [onSuccess, onError]
  );

  const reset = useCallback(() => {
    setState(initialState);
  }, []);

  const canRetry = state.error !== null && isRetryableError(state.error.code);

  return {
    state,
    transfer,
    reset,
    canRetry,
  };
}
```

### useTokenHistory Hook

```typescript
// frontend/src/hooks/useTokenHistory.ts

import { useState, useEffect, useCallback } from 'react';

/**
 * Token transaction record (placeholder - actual structure depends on backend)
 */
export interface TokenTransaction {
  txid: string;
  tokenId: string;
  type: 'send' | 'receive';
  amount: string;
  address: string;  // To/from address
  timestamp: number;
  confirmations: number;
}

interface UseTokenHistoryResult {
  transactions: TokenTransaction[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  hasMore: boolean;
  loadMore: () => Promise<void>;
}

/**
 * Hook for fetching token transaction history
 *
 * NOTE: This depends on backend support for token history tracking.
 * Currently a placeholder - implement when backend endpoint exists.
 */
export function useTokenHistory(tokenId: string | null): UseTokenHistoryResult {
  const [transactions, setTransactions] = useState<TokenTransaction[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);

  const refresh = useCallback(async () => {
    if (!tokenId) {
      setTransactions([]);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      // TODO: Implement when backend endpoint exists
      // const response = await fetch(`${WALLET_BASE}/ordinals/history/${tokenId}`);
      // const data = await response.json();
      // setTransactions(data.transactions);
      // setHasMore(data.hasMore);

      // Placeholder: empty for now
      setTransactions([]);
      setHasMore(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch history');
    } finally {
      setLoading(false);
    }
  }, [tokenId]);

  const loadMore = useCallback(async () => {
    // TODO: Implement pagination when backend supports it
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return {
    transactions,
    loading,
    error,
    refresh,
    hasMore,
    loadMore,
  };
}
```

---

## Phase 3: Token Display Components

**Goal**: Create UI components for displaying tokens

### Tasks

- [ ] Create `frontend/src/components/tokens/TokenIcon.tsx`
- [ ] Create `frontend/src/components/tokens/TokenCard.tsx`
- [ ] Create `frontend/src/components/tokens/TokenList.tsx`
- [ ] Style with existing CSS patterns (or MUI)
- [ ] Handle loading/empty/error states

### TokenIcon Component

```tsx
// frontend/src/components/tokens/TokenIcon.tsx

import React, { useState } from 'react';
import { getTokenIconUrl } from '../../bridge/ordinals';

interface TokenIconProps {
  iconOrigin: string | null;
  symbol: string | null;
  size?: number;
}

export const TokenIcon: React.FC<TokenIconProps> = ({
  iconOrigin,
  symbol,
  size = 40
}) => {
  const [imgError, setImgError] = useState(false);
  const iconUrl = getTokenIconUrl(iconOrigin);

  // Fallback: show first letter of symbol
  const fallback = (
    <div
      style={{
        width: size,
        height: size,
        borderRadius: '50%',
        backgroundColor: '#3b82f6',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        color: 'white',
        fontWeight: 'bold',
        fontSize: size * 0.4,
      }}
    >
      {symbol?.[0]?.toUpperCase() || '?'}
    </div>
  );

  if (!iconUrl || imgError) {
    return fallback;
  }

  return (
    <img
      src={iconUrl}
      alt={symbol || 'Token icon'}
      width={size}
      height={size}
      style={{ borderRadius: '50%', objectFit: 'cover' }}
      onError={() => setImgError(true)}
    />
  );
};
```

### TokenCard Component

```tsx
// frontend/src/components/tokens/TokenCard.tsx

import React from 'react';
import type { TokenBalance } from '../../types/tokens';
import { TokenIcon } from './TokenIcon';

interface TokenCardProps {
  token: TokenBalance;
  onClick?: (token: TokenBalance) => void;
}

export const TokenCard: React.FC<TokenCardProps> = ({ token, onClick }) => {
  // Format amount with decimals
  const formatAmount = (amount: string, decimals: number): string => {
    if (decimals === 0) return amount;

    const padded = amount.padStart(decimals + 1, '0');
    const intPart = padded.slice(0, -decimals) || '0';
    const decPart = padded.slice(-decimals);

    // Trim trailing zeros
    const trimmedDec = decPart.replace(/0+$/, '');

    return trimmedDec ? `${intPart}.${trimmedDec}` : intPart;
  };

  return (
    <div
      className="token-card"
      onClick={() => onClick?.(token)}
      style={{
        display: 'flex',
        alignItems: 'center',
        padding: '12px',
        borderRadius: '8px',
        backgroundColor: 'var(--bg-secondary, #1e1e1e)',
        cursor: onClick ? 'pointer' : 'default',
        gap: '12px',
      }}
    >
      <TokenIcon
        iconOrigin={token.icon}
        symbol={token.symbol}
        size={40}
      />

      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{
          fontWeight: 600,
          color: 'var(--text-primary, #fff)',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
        }}>
          {token.symbol || 'Unknown Token'}
        </div>
        <div style={{
          fontSize: '12px',
          color: 'var(--text-secondary, #888)',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
        }}>
          {token.id.slice(0, 8)}...{token.id.slice(-4)}
        </div>
      </div>

      <div style={{
        textAlign: 'right',
        fontWeight: 500,
        color: 'var(--text-primary, #fff)',
      }}>
        {formatAmount(token.amount, token.decimals)}
      </div>
    </div>
  );
};
```

### TokenList Component

```tsx
// frontend/src/components/tokens/TokenList.tsx

import React from 'react';
import type { TokenBalance } from '../../types/tokens';
import { TokenCard } from './TokenCard';
import './tokens.css';

interface TokenListProps {
  tokens: TokenBalance[];
  loading: boolean;
  syncing?: boolean;
  error: string | null;
  onTokenClick?: (token: TokenBalance) => void;
  onRefresh?: () => void;
  onSync?: () => void;
}

export const TokenList: React.FC<TokenListProps> = ({
  tokens,
  loading,
  syncing = false,
  error,
  onTokenClick,
  onRefresh,
  onSync,
}) => {
  if (loading && tokens.length === 0) {
    return (
      <div className="token-list-empty">
        <div className="token-list-spinner" />
        <span>Loading tokens...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="token-list-error">
        <div className="token-list-error-message">{error}</div>
        {onRefresh && (
          <button className="token-list-retry-btn" onClick={onRefresh}>
            Retry
          </button>
        )}
      </div>
    );
  }

  if (tokens.length === 0) {
    return (
      <div className="token-list-empty">
        <span>No BSV-21 tokens found</span>
        {onSync && (
          <button
            className="token-list-sync-btn"
            onClick={onSync}
            disabled={syncing}
          >
            {syncing ? 'Syncing...' : 'Sync from network'}
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="token-list">
      <div className="token-list-header">
        <span className="token-list-count">{tokens.length} token{tokens.length !== 1 ? 's' : ''}</span>
        {onSync && (
          <button
            className="token-list-sync-btn-small"
            onClick={onSync}
            disabled={syncing}
            title="Sync from network"
          >
            {syncing ? '⟳' : '↻'}
          </button>
        )}
      </div>
      <div className="token-list-items">
        {tokens.map((token) => (
          <TokenCard
            key={token.tokenId}
            token={token}
            onClick={onTokenClick}
          />
        ))}
      </div>
    </div>
  );
};
```

### TokenDetail Component

```tsx
// frontend/src/components/tokens/TokenDetail.tsx

import React from 'react';
import type { TokenBalance, TokenMetadata } from '../../types/tokens';
import { TokenIcon } from './TokenIcon';
import { formatTokenAmount } from '../../utils/tokenAmount';
import './tokens.css';

interface TokenDetailProps {
  token: TokenBalance;
  metadata: TokenMetadata | null;
  loading?: boolean;
  onSend?: () => void;
  onClose?: () => void;
}

export const TokenDetail: React.FC<TokenDetailProps> = ({
  token,
  metadata,
  loading = false,
  onSend,
  onClose,
}) => {
  const formattedBalance = formatTokenAmount(token.amount, token.decimals);
  const formattedMaxSupply = metadata?.maxSupply
    ? formatTokenAmount(metadata.maxSupply, token.decimals)
    : null;

  return (
    <div className="token-detail">
      {/* Header */}
      <div className="token-detail-header">
        {onClose && (
          <button className="token-detail-close" onClick={onClose}>
            ×
          </button>
        )}
        <TokenIcon
          iconOrigin={token.icon}
          symbol={token.symbol}
          size={64}
        />
        <h2 className="token-detail-symbol">
          {token.symbol || 'Unknown Token'}
        </h2>
      </div>

      {/* Balance */}
      <div className="token-detail-balance">
        <span className="token-detail-balance-value">{formattedBalance}</span>
        {token.symbol && (
          <span className="token-detail-balance-symbol">{token.symbol}</span>
        )}
      </div>

      {/* Actions */}
      <div className="token-detail-actions">
        {onSend && (
          <button className="token-detail-send-btn" onClick={onSend}>
            Send
          </button>
        )}
      </div>

      {/* Metadata */}
      <div className="token-detail-info">
        <h3>Token Information</h3>

        {loading ? (
          <div className="token-detail-loading">Loading metadata...</div>
        ) : (
          <dl className="token-detail-metadata">
            <div className="token-detail-row">
              <dt>Token ID</dt>
              <dd className="token-detail-id" title={token.tokenId}>
                {token.tokenId.slice(0, 16)}...{token.tokenId.slice(-8)}
              </dd>
            </div>

            <div className="token-detail-row">
              <dt>Decimals</dt>
              <dd>{token.decimals}</dd>
            </div>

            {formattedMaxSupply && (
              <div className="token-detail-row">
                <dt>Max Supply</dt>
                <dd>{formattedMaxSupply}</dd>
              </div>
            )}

            {metadata?.deployHeight && (
              <div className="token-detail-row">
                <dt>Deploy Height</dt>
                <dd>{metadata.deployHeight.toLocaleString()}</dd>
              </div>
            )}

            {token.utxoCount !== undefined && (
              <div className="token-detail-row">
                <dt>UTXOs</dt>
                <dd>{token.utxoCount}</dd>
              </div>
            )}
          </dl>
        )}
      </div>

      {/* Links */}
      <div className="token-detail-links">
        <a
          href={`https://whatsonchain.com/tx/${token.tokenId.split('_')[0]}`}
          target="_blank"
          rel="noopener noreferrer"
          className="token-detail-link"
        >
          View on WhatsOnChain ↗
        </a>
      </div>
    </div>
  );
};
```

### TokenTransactionHistory Component

```tsx
// frontend/src/components/tokens/TokenTransactionHistory.tsx

import React from 'react';
import type { TokenTransaction } from '../../hooks/useTokenHistory';
import { formatTokenAmount } from '../../utils/tokenAmount';
import './tokens.css';

interface TokenTransactionHistoryProps {
  transactions: TokenTransaction[];
  decimals: number;
  symbol: string | null;
  loading: boolean;
  error: string | null;
  onRefresh?: () => void;
}

export const TokenTransactionHistory: React.FC<TokenTransactionHistoryProps> = ({
  transactions,
  decimals,
  symbol,
  loading,
  error,
  onRefresh,
}) => {
  if (loading && transactions.length === 0) {
    return (
      <div className="token-history-loading">
        Loading transaction history...
      </div>
    );
  }

  if (error) {
    return (
      <div className="token-history-error">
        <span>{error}</span>
        {onRefresh && (
          <button onClick={onRefresh}>Retry</button>
        )}
      </div>
    );
  }

  if (transactions.length === 0) {
    return (
      <div className="token-history-empty">
        No transactions yet
      </div>
    );
  }

  return (
    <div className="token-history">
      <h3 className="token-history-title">Transaction History</h3>
      <ul className="token-history-list">
        {transactions.map((tx) => (
          <li key={tx.txid} className="token-history-item">
            <div className="token-history-icon">
              {tx.type === 'send' ? '↑' : '↓'}
            </div>
            <div className="token-history-details">
              <div className="token-history-type">
                {tx.type === 'send' ? 'Sent' : 'Received'}
              </div>
              <div className="token-history-address">
                {tx.type === 'send' ? 'To: ' : 'From: '}
                {tx.address.slice(0, 8)}...{tx.address.slice(-4)}
              </div>
              <div className="token-history-time">
                {new Date(tx.timestamp).toLocaleDateString()}
              </div>
            </div>
            <div className={`token-history-amount ${tx.type}`}>
              {tx.type === 'send' ? '-' : '+'}
              {formatTokenAmount(tx.amount, decimals)}
              {symbol && ` ${symbol}`}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
};
```

---

## Phase 4: Token Send Form

**Goal**: Create UI for sending tokens

### Tasks

- [ ] Create `frontend/src/components/tokens/TokenSendForm.tsx`
- [ ] Implement address validation
- [ ] Implement amount validation (max balance, decimals)
- [ ] Show confirmation before sending
- [ ] Display transaction result

### TokenSendForm Component

```tsx
// frontend/src/components/tokens/TokenSendForm.tsx

import React, { useState } from 'react';
import type { TokenBalance } from '../../types/tokens';
import { useTokenTransfer } from '../../hooks/useTokenTransfer';
import { TokenIcon } from './TokenIcon';

interface TokenSendFormProps {
  token: TokenBalance;
  onSuccess?: (txid: string) => void;
  onCancel?: () => void;
}

export const TokenSendForm: React.FC<TokenSendFormProps> = ({
  token,
  onSuccess,
  onCancel,
}) => {
  const [toAddress, setToAddress] = useState('');
  const [amount, setAmount] = useState('');
  const [step, setStep] = useState<'form' | 'confirm' | 'result'>('form');

  const { transfer, loading, error, lastResult } = useTokenTransfer();

  // Format display amount
  const formatAmount = (amt: string, decimals: number): string => {
    if (decimals === 0) return amt;
    const padded = amt.padStart(decimals + 1, '0');
    const intPart = padded.slice(0, -decimals) || '0';
    const decPart = padded.slice(-decimals).replace(/0+$/, '');
    return decPart ? `${intPart}.${decPart}` : intPart;
  };

  // Convert display amount to raw amount
  const toRawAmount = (displayAmount: string, decimals: number): string => {
    if (decimals === 0) return displayAmount;

    const [intPart, decPart = ''] = displayAmount.split('.');
    const paddedDec = decPart.padEnd(decimals, '0').slice(0, decimals);
    const raw = intPart + paddedDec;

    // Remove leading zeros
    return raw.replace(/^0+/, '') || '0';
  };

  const maxAmount = formatAmount(token.amount, token.decimals);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (step === 'form') {
      setStep('confirm');
      return;
    }

    if (step === 'confirm') {
      const rawAmount = toRawAmount(amount, token.decimals);
      const result = await transfer({
        tokenId: token.id,
        amount: rawAmount,
        toAddress,
      });

      setStep('result');

      if (result.success) {
        onSuccess?.(result.txid);
      }
    }
  };

  const handleSetMax = () => {
    setAmount(maxAmount);
  };

  // Validation
  const isValidAddress = toAddress.length >= 25 && toAddress.length <= 35;
  const rawAmount = toRawAmount(amount || '0', token.decimals);
  const isValidAmount = BigInt(rawAmount) > 0n && BigInt(rawAmount) <= BigInt(token.amount);
  const canSubmit = isValidAddress && isValidAmount;

  if (step === 'result') {
    return (
      <div style={{ padding: '20px' }}>
        {lastResult?.success ? (
          <>
            <div style={{ color: '#22c55e', marginBottom: '16px', fontSize: '18px' }}>
              Transfer Successful!
            </div>
            <div style={{ fontSize: '12px', color: '#888', wordBreak: 'break-all' }}>
              TXID: {lastResult.txid}
            </div>
          </>
        ) : (
          <div style={{ color: '#ef4444' }}>
            Transfer Failed: {lastResult?.error || error}
          </div>
        )}
        <button
          onClick={onCancel}
          style={{ marginTop: '16px', padding: '10px 20px' }}
        >
          Close
        </button>
      </div>
    );
  }

  if (step === 'confirm') {
    return (
      <div style={{ padding: '20px' }}>
        <h3 style={{ marginBottom: '16px' }}>Confirm Transfer</h3>

        <div style={{ marginBottom: '12px' }}>
          <strong>Token:</strong> {token.symbol || token.id}
        </div>
        <div style={{ marginBottom: '12px' }}>
          <strong>Amount:</strong> {amount}
        </div>
        <div style={{ marginBottom: '12px', wordBreak: 'break-all' }}>
          <strong>To:</strong> {toAddress}
        </div>

        <div style={{ display: 'flex', gap: '10px', marginTop: '20px' }}>
          <button
            onClick={() => setStep('form')}
            disabled={loading}
            style={{ flex: 1, padding: '10px' }}
          >
            Back
          </button>
          <button
            onClick={handleSubmit}
            disabled={loading}
            style={{
              flex: 1,
              padding: '10px',
              backgroundColor: '#3b82f6',
              color: 'white',
            }}
          >
            {loading ? 'Sending...' : 'Confirm'}
          </button>
        </div>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} style={{ padding: '20px' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '20px' }}>
        <TokenIcon iconOrigin={token.icon} symbol={token.symbol} size={48} />
        <div>
          <div style={{ fontWeight: 600, fontSize: '18px' }}>
            Send {token.symbol || 'Token'}
          </div>
          <div style={{ fontSize: '12px', color: '#888' }}>
            Balance: {maxAmount}
          </div>
        </div>
      </div>

      <div style={{ marginBottom: '16px' }}>
        <label style={{ display: 'block', marginBottom: '4px', fontSize: '14px' }}>
          Recipient Address
        </label>
        <input
          type="text"
          value={toAddress}
          onChange={(e) => setToAddress(e.target.value)}
          placeholder="Enter BSV address"
          style={{
            width: '100%',
            padding: '10px',
            borderRadius: '4px',
            border: '1px solid #333',
            backgroundColor: '#1e1e1e',
            color: '#fff',
          }}
        />
      </div>

      <div style={{ marginBottom: '20px' }}>
        <label style={{ display: 'block', marginBottom: '4px', fontSize: '14px' }}>
          Amount
        </label>
        <div style={{ display: 'flex', gap: '8px' }}>
          <input
            type="text"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            placeholder="0.00"
            style={{
              flex: 1,
              padding: '10px',
              borderRadius: '4px',
              border: '1px solid #333',
              backgroundColor: '#1e1e1e',
              color: '#fff',
            }}
          />
          <button
            type="button"
            onClick={handleSetMax}
            style={{ padding: '10px 16px' }}
          >
            Max
          </button>
        </div>
      </div>

      {error && (
        <div style={{ color: '#ef4444', marginBottom: '16px' }}>
          {error}
        </div>
      )}

      <div style={{ display: 'flex', gap: '10px' }}>
        {onCancel && (
          <button
            type="button"
            onClick={onCancel}
            style={{ flex: 1, padding: '10px' }}
          >
            Cancel
          </button>
        )}
        <button
          type="submit"
          disabled={!canSubmit}
          style={{
            flex: 1,
            padding: '10px',
            backgroundColor: canSubmit ? '#3b82f6' : '#333',
            color: canSubmit ? 'white' : '#666',
            cursor: canSubmit ? 'pointer' : 'not-allowed',
          }}
        >
          Continue
        </button>
      </div>
    </form>
  );
};
```

---

## Phase 5: Wallet Panel Integration

**Goal**: Add tokens tab to existing wallet panel

### Tasks

- [ ] Modify `frontend/src/components/panels/WalletPanelContent.tsx`
- [ ] Add "Tokens" tab alongside existing tabs
- [ ] Integrate TokenList component
- [ ] Add token detail/send modal
- [ ] Connect to user's address from wallet state

### WalletPanelContent Integration

```tsx
// Modifications to WalletPanelContent.tsx

import { useTokens } from '../../hooks/useTokens';
import { TokenList } from '../tokens/TokenList';
import { TokenSendForm } from '../tokens/TokenSendForm';

// Inside the component:

// Add to state
const [activeTab, setActiveTab] = useState<'balance' | 'tokens' | 'send'>('balance');
const [selectedToken, setSelectedToken] = useState<TokenBalance | null>(null);

// Get user's address from existing wallet state
const userAddress = walletState?.address || null;

// Fetch tokens
const { tokens, loading: tokensLoading, error: tokensError, refresh: refreshTokens } = useTokens(userAddress);

// In the render, add tabs:
<div className="wallet-tabs">
  <button
    className={activeTab === 'balance' ? 'active' : ''}
    onClick={() => setActiveTab('balance')}
  >
    Balance
  </button>
  <button
    className={activeTab === 'tokens' ? 'active' : ''}
    onClick={() => setActiveTab('tokens')}
  >
    Tokens ({tokens.length})
  </button>
  <button
    className={activeTab === 'send' ? 'active' : ''}
    onClick={() => setActiveTab('send')}
  >
    Send
  </button>
</div>

// Tab content
{activeTab === 'tokens' && (
  <TokenList
    tokens={tokens}
    loading={tokensLoading}
    error={tokensError}
    onTokenClick={(token) => setSelectedToken(token)}
    onRefresh={refreshTokens}
  />
)}

// Token send modal
{selectedToken && (
  <Modal onClose={() => setSelectedToken(null)}>
    <TokenSendForm
      token={selectedToken}
      onSuccess={() => {
        setSelectedToken(null);
        refreshTokens();
      }}
      onCancel={() => setSelectedToken(null)}
    />
  </Modal>
)}
```

---

## Phase 6: Mock Data for Development

**Goal**: Enable frontend development without backend

### Tasks

- [ ] Create `frontend/src/mocks/tokens.ts`
- [ ] Implement mock fetch functions
- [ ] Add environment flag to switch between mock/real

### Mock Data

```typescript
// frontend/src/mocks/tokens.ts

import type { TokenBalance, TokenInfo } from '../types/tokens';

export const mockTokens: TokenBalance[] = [
  {
    id: 'abc123def456789012345678901234567890123456789012345678901234_0',
    symbol: 'TEST',
    amount: '100000000000',
    decimals: 8,
    icon: null,
  },
  {
    id: 'def456abc789012345678901234567890123456789012345678901234567_0',
    symbol: 'PEPE',
    amount: '420690000000',
    decimals: 8,
    icon: 'xyz789_0',
  },
  {
    id: 'ghi789xyz012345678901234567890123456789012345678901234567890_1',
    symbol: 'MEME',
    amount: '1000000',
    decimals: 0,
    icon: null,
  },
];

export const mockTokenInfo: Record<string, TokenInfo> = {
  'abc123def456789012345678901234567890123456789012345678901234_0': {
    id: 'abc123def456789012345678901234567890123456789012345678901234_0',
    symbol: 'TEST',
    maxSupply: '21000000000000000',
    decimals: 8,
    icon: null,
    deployHeight: 850000,
  },
};

// Mock API functions
export async function mockGetTokens(address: string): Promise<TokenBalance[]> {
  await new Promise(resolve => setTimeout(resolve, 500)); // Simulate latency
  return mockTokens;
}

export async function mockGetTokenInfo(tokenId: string): Promise<TokenInfo> {
  await new Promise(resolve => setTimeout(resolve, 300));
  const info = mockTokenInfo[tokenId];
  if (!info) throw new Error('Token not found');
  return info;
}
```

### Environment Switch

```typescript
// frontend/src/bridge/ordinals.ts

import { mockGetTokens, mockGetTokenInfo } from '../mocks/tokens';

const USE_MOCKS = import.meta.env.DEV && import.meta.env.VITE_USE_MOCKS === 'true';

export async function getTokens(address: string): Promise<TokenBalance[]> {
  if (USE_MOCKS) {
    return mockGetTokens(address);
  }
  // ... real implementation
}
```

---

## Testing Strategy

### Unit Tests

```bash
cd frontend
npm test
```

### Visual Testing

```bash
# With mocks
VITE_USE_MOCKS=true npm run dev

# With real backend (requires Plan A)
npm run dev
```

### Test Cases

1. Token list renders correctly with 0, 1, many tokens
2. Token card displays correct formatted amounts
3. Send form validates address and amount
4. Loading/error states display correctly
5. Tab switching works
6. Modal open/close works

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| **Types** | | |
| `frontend/src/types/tokens.d.ts` | Create | Comprehensive type definitions |
| **Bridge** | | |
| `frontend/src/bridge/ordinals.ts` | Create | API bridge (uses createAction) |
| **Utilities** | | |
| `frontend/src/utils/tokenAmount.ts` | Create | Big number formatting |
| `frontend/src/utils/tokenValidation.ts` | Create | Input validation |
| `frontend/src/utils/tokenErrors.ts` | Create | Error handling utilities |
| **Hooks** | | |
| `frontend/src/hooks/useTokens.ts` | Create | Token list hook with sync |
| `frontend/src/hooks/useTokenBalance.ts` | Create | Single token hook |
| `frontend/src/hooks/useTokenTransfer.ts` | Create | Transfer mutation hook |
| `frontend/src/hooks/useTokenHistory.ts` | Create | Transaction history hook |
| **Components** | | |
| `frontend/src/components/tokens/index.ts` | Create | Barrel exports |
| `frontend/src/components/tokens/TokenIcon.tsx` | Create | Icon with fallback |
| `frontend/src/components/tokens/TokenCard.tsx` | Create | Token card display |
| `frontend/src/components/tokens/TokenList.tsx` | Create | Token list with sync |
| `frontend/src/components/tokens/TokenDetail.tsx` | Create | Token detail modal |
| `frontend/src/components/tokens/TokenSendForm.tsx` | Create | Transfer form |
| `frontend/src/components/tokens/TokenTransactionHistory.tsx` | Create | Tx history display |
| `frontend/src/components/tokens/tokens.css` | Create | Component styles |
| **Integration** | | |
| `frontend/src/components/panels/WalletPanelContent.tsx` | Modify | Add tokens tab |
| **Mocks** | | |
| `frontend/src/mocks/tokens.ts` | Create | Mock data for dev |

---

## Phase 7: CSS Styling

**Goal**: Create consistent styling that matches the existing wallet panel

### Tasks

- [ ] Create `frontend/src/components/tokens/tokens.css`
- [ ] Use existing CSS variables from the project
- [ ] Ensure dark theme compatibility
- [ ] Make responsive for wallet panel width

### CSS Implementation

```css
/* frontend/src/components/tokens/tokens.css */

/* ============================================
   CSS Variables (use existing project vars)
   ============================================ */

.token-list,
.token-card,
.token-detail,
.token-send-form {
  --token-bg-primary: var(--bg-primary, #121212);
  --token-bg-secondary: var(--bg-secondary, #1e1e1e);
  --token-bg-hover: var(--bg-hover, #2a2a2a);
  --token-text-primary: var(--text-primary, #ffffff);
  --token-text-secondary: var(--text-secondary, #888888);
  --token-accent: var(--accent-color, #3b82f6);
  --token-accent-hover: var(--accent-hover, #2563eb);
  --token-success: var(--success-color, #22c55e);
  --token-error: var(--error-color, #ef4444);
  --token-border: var(--border-color, #333333);
  --token-radius: 8px;
}

/* ============================================
   Token List
   ============================================ */

.token-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.token-list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 4px;
  color: var(--token-text-secondary);
  font-size: 12px;
}

.token-list-count {
  font-weight: 500;
}

.token-list-items {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.token-list-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 40px 20px;
  color: var(--token-text-secondary);
  text-align: center;
}

.token-list-error {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 20px;
  text-align: center;
}

.token-list-error-message {
  color: var(--token-error);
}

.token-list-spinner {
  width: 24px;
  height: 24px;
  border: 2px solid var(--token-border);
  border-top-color: var(--token-accent);
  border-radius: 50%;
  animation: token-spin 0.8s linear infinite;
}

@keyframes token-spin {
  to { transform: rotate(360deg); }
}

.token-list-sync-btn,
.token-list-retry-btn {
  padding: 8px 16px;
  background: var(--token-bg-secondary);
  border: 1px solid var(--token-border);
  border-radius: var(--token-radius);
  color: var(--token-text-primary);
  cursor: pointer;
  transition: background 0.2s;
}

.token-list-sync-btn:hover:not(:disabled),
.token-list-retry-btn:hover:not(:disabled) {
  background: var(--token-bg-hover);
}

.token-list-sync-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.token-list-sync-btn-small {
  padding: 4px 8px;
  background: transparent;
  border: none;
  color: var(--token-text-secondary);
  cursor: pointer;
  font-size: 16px;
  transition: color 0.2s;
}

.token-list-sync-btn-small:hover:not(:disabled) {
  color: var(--token-accent);
}

/* ============================================
   Token Card
   ============================================ */

.token-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  background: var(--token-bg-secondary);
  border-radius: var(--token-radius);
  cursor: pointer;
  transition: background 0.2s;
}

.token-card:hover {
  background: var(--token-bg-hover);
}

.token-card-info {
  flex: 1;
  min-width: 0;
}

.token-card-symbol {
  font-weight: 600;
  color: var(--token-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.token-card-id {
  font-size: 12px;
  color: var(--token-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.token-card-balance {
  text-align: right;
  font-weight: 500;
  color: var(--token-text-primary);
}

/* ============================================
   Token Icon
   ============================================ */

.token-icon {
  border-radius: 50%;
  object-fit: cover;
}

.token-icon-fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  background: var(--token-accent);
  color: white;
  font-weight: bold;
}

/* ============================================
   Token Detail
   ============================================ */

.token-detail {
  display: flex;
  flex-direction: column;
  gap: 20px;
  padding: 20px;
}

.token-detail-header {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  position: relative;
}

.token-detail-close {
  position: absolute;
  top: 0;
  right: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--token-text-secondary);
  font-size: 24px;
  cursor: pointer;
  transition: color 0.2s;
}

.token-detail-close:hover {
  color: var(--token-text-primary);
}

.token-detail-symbol {
  margin: 0;
  font-size: 24px;
  font-weight: 600;
  color: var(--token-text-primary);
}

.token-detail-balance {
  text-align: center;
  padding: 16px;
  background: var(--token-bg-secondary);
  border-radius: var(--token-radius);
}

.token-detail-balance-value {
  font-size: 32px;
  font-weight: 700;
  color: var(--token-text-primary);
}

.token-detail-balance-symbol {
  margin-left: 8px;
  font-size: 18px;
  color: var(--token-text-secondary);
}

.token-detail-actions {
  display: flex;
  gap: 12px;
}

.token-detail-send-btn {
  flex: 1;
  padding: 12px;
  background: var(--token-accent);
  border: none;
  border-radius: var(--token-radius);
  color: white;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s;
}

.token-detail-send-btn:hover {
  background: var(--token-accent-hover);
}

.token-detail-info {
  padding: 16px;
  background: var(--token-bg-secondary);
  border-radius: var(--token-radius);
}

.token-detail-info h3 {
  margin: 0 0 12px 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--token-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.token-detail-metadata {
  margin: 0;
}

.token-detail-row {
  display: flex;
  justify-content: space-between;
  padding: 8px 0;
  border-bottom: 1px solid var(--token-border);
}

.token-detail-row:last-child {
  border-bottom: none;
}

.token-detail-row dt {
  color: var(--token-text-secondary);
}

.token-detail-row dd {
  margin: 0;
  color: var(--token-text-primary);
  font-family: monospace;
}

.token-detail-id {
  cursor: pointer;
}

.token-detail-links {
  text-align: center;
}

.token-detail-link {
  color: var(--token-accent);
  text-decoration: none;
  font-size: 14px;
}

.token-detail-link:hover {
  text-decoration: underline;
}

/* ============================================
   Token Send Form
   ============================================ */

.token-send-form {
  padding: 20px;
}

.token-send-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
}

.token-send-title {
  font-weight: 600;
  font-size: 18px;
  color: var(--token-text-primary);
}

.token-send-balance {
  font-size: 12px;
  color: var(--token-text-secondary);
}

.token-send-field {
  margin-bottom: 16px;
}

.token-send-label {
  display: block;
  margin-bottom: 4px;
  font-size: 14px;
  color: var(--token-text-secondary);
}

.token-send-input {
  width: 100%;
  padding: 10px;
  background: var(--token-bg-secondary);
  border: 1px solid var(--token-border);
  border-radius: var(--token-radius);
  color: var(--token-text-primary);
  font-size: 14px;
}

.token-send-input:focus {
  outline: none;
  border-color: var(--token-accent);
}

.token-send-input.error {
  border-color: var(--token-error);
}

.token-send-amount-row {
  display: flex;
  gap: 8px;
}

.token-send-max-btn {
  padding: 10px 16px;
  background: var(--token-bg-secondary);
  border: 1px solid var(--token-border);
  border-radius: var(--token-radius);
  color: var(--token-text-secondary);
  cursor: pointer;
  transition: all 0.2s;
}

.token-send-max-btn:hover {
  color: var(--token-text-primary);
  border-color: var(--token-accent);
}

.token-send-error {
  margin-bottom: 16px;
  padding: 12px;
  background: rgba(239, 68, 68, 0.1);
  border: 1px solid var(--token-error);
  border-radius: var(--token-radius);
  color: var(--token-error);
  font-size: 14px;
}

.token-send-actions {
  display: flex;
  gap: 10px;
}

.token-send-cancel-btn {
  flex: 1;
  padding: 12px;
  background: var(--token-bg-secondary);
  border: 1px solid var(--token-border);
  border-radius: var(--token-radius);
  color: var(--token-text-primary);
  cursor: pointer;
  transition: background 0.2s;
}

.token-send-cancel-btn:hover {
  background: var(--token-bg-hover);
}

.token-send-submit-btn {
  flex: 1;
  padding: 12px;
  background: var(--token-accent);
  border: none;
  border-radius: var(--token-radius);
  color: white;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s;
}

.token-send-submit-btn:hover:not(:disabled) {
  background: var(--token-accent-hover);
}

.token-send-submit-btn:disabled {
  background: var(--token-border);
  color: var(--token-text-secondary);
  cursor: not-allowed;
}

/* Confirm step */
.token-send-confirm {
  padding: 20px;
}

.token-send-confirm h3 {
  margin: 0 0 16px 0;
}

.token-send-confirm-row {
  margin-bottom: 12px;
}

.token-send-confirm-row strong {
  color: var(--token-text-secondary);
}

/* Result step */
.token-send-result {
  padding: 20px;
  text-align: center;
}

.token-send-success {
  color: var(--token-success);
  font-size: 18px;
  margin-bottom: 16px;
}

.token-send-txid {
  font-size: 12px;
  color: var(--token-text-secondary);
  word-break: break-all;
}

.token-send-failed {
  color: var(--token-error);
}

/* ============================================
   Token History
   ============================================ */

.token-history {
  padding: 16px;
}

.token-history-title {
  margin: 0 0 12px 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--token-text-secondary);
  text-transform: uppercase;
}

.token-history-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.token-history-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 0;
  border-bottom: 1px solid var(--token-border);
}

.token-history-item:last-child {
  border-bottom: none;
}

.token-history-icon {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--token-bg-secondary);
  border-radius: 50%;
  font-size: 18px;
}

.token-history-details {
  flex: 1;
  min-width: 0;
}

.token-history-type {
  font-weight: 500;
  color: var(--token-text-primary);
}

.token-history-address {
  font-size: 12px;
  color: var(--token-text-secondary);
  font-family: monospace;
}

.token-history-time {
  font-size: 11px;
  color: var(--token-text-secondary);
}

.token-history-amount {
  font-weight: 500;
  font-family: monospace;
}

.token-history-amount.send {
  color: var(--token-error);
}

.token-history-amount.receive {
  color: var(--token-success);
}

.token-history-empty,
.token-history-loading,
.token-history-error {
  padding: 20px;
  text-align: center;
  color: var(--token-text-secondary);
}
```

---

## Success Criteria

- [ ] Token list displays user's tokens
- [ ] Token icons load or show fallback
- [ ] Amounts formatted correctly with decimals
- [ ] Send form validates inputs
- [ ] Transfer flow works end-to-end (with backend)
- [ ] Works with mock data for development
- [ ] Responsive on wallet panel size

---

## Remaining Open Questions

1. **Tab placement**: New tab or subtab under existing wallet section?
   - *Recommendation*: Add as a new tab in wallet panel ("Balance" | "Tokens" | "Send")

2. **Transaction history backend**: Depends on Plan A adding history tracking endpoint
   - *Workaround*: Placeholder component ready, enable when backend supports it

3. **Auto-refresh frequency**: How often to poll for token updates?
   - *Recommendation*: Manual sync button, no auto-refresh (reduce API load)
   - *Alternative*: 60-second interval if user prefers

4. **Token icon caching**: Cache icons locally or always fetch from GorillaPool?
   - *Recommendation*: Let browser cache handle it (GorillaPool sets cache headers)

### Resolved Questions (from design phase)

| Question | Resolution |
|----------|------------|
| Separate endpoint or createAction? | **createAction** - unified transaction flow |
| How to handle big numbers in JS? | **Strings everywhere** - only use BigInt for comparison |
| Missing type definitions? | **Added** - comprehensive types matching Plan A |
| Missing validation? | **Added** - tokenValidation.ts with typed errors |
| Missing error handling? | **Added** - tokenErrors.ts with error codes |
| Token detail component? | **Added** - TokenDetail.tsx with metadata display |
| Transaction history? | **Added** - placeholder hook + component |
| CSS styling? | **Added** - tokens.css with CSS variable integration |

---

## Integration with Existing Code

When implementing, reference these existing files:
- `frontend/src/components/WalletPanel.css` - Existing wallet styling patterns
- `frontend/src/hooks/useBalance.ts` - Similar hook pattern for BSV balance
- `frontend/src/hooks/useHodosBrowser.ts` - Bridge pattern example
- `frontend/src/index.css` - CSS variable definitions

---

**Created**: January 2025
**Updated**: January 2025 (comprehensive update)
**Status**: Planning - Ready for Implementation
**Assignee**: TBD (Frontend developer)
**Depends On**: Plan A (for real data), can use mocks initially
