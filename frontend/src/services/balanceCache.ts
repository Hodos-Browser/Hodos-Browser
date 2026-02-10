// localStorage-based cache for balance and BSV price.
// Shared across all CEF overlay subprocesses (same origin: localhost:5137).

const BALANCE_KEY = 'hodos:wallet:balance';
const PRICE_KEY = 'hodos:wallet:bsvPrice';

// Staleness thresholds (milliseconds) — 2x the poll interval
const BALANCE_MAX_AGE_MS = 60_000;   // 60s (balance polls every 30s)
const PRICE_MAX_AGE_MS = 600_000;    // 10min (price polls every 5min)

export interface CachedBalance {
  balance: number;    // satoshis
  updatedAt: number;  // Date.now() timestamp
}

export interface CachedPrice {
  price: number;      // USD per BSV
  updatedAt: number;
}

export function getCachedBalance(): CachedBalance | null {
  try {
    const raw = localStorage.getItem(BALANCE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (typeof parsed.balance !== 'number' || typeof parsed.updatedAt !== 'number') return null;
    return parsed;
  } catch {
    return null;
  }
}

export function setCachedBalance(balance: number): void {
  try {
    const entry: CachedBalance = { balance, updatedAt: Date.now() };
    localStorage.setItem(BALANCE_KEY, JSON.stringify(entry));
  } catch {
    // localStorage quota exceeded or unavailable — non-fatal
  }
}

export function getCachedPrice(): CachedPrice | null {
  try {
    const raw = localStorage.getItem(PRICE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (typeof parsed.price !== 'number' || typeof parsed.updatedAt !== 'number') return null;
    return parsed;
  } catch {
    return null;
  }
}

export function setCachedPrice(price: number): void {
  try {
    const entry: CachedPrice = { price, updatedAt: Date.now() };
    localStorage.setItem(PRICE_KEY, JSON.stringify(entry));
  } catch {
    // non-fatal
  }
}

export function isBalanceStale(): boolean {
  const cached = getCachedBalance();
  if (!cached) return true;
  return Date.now() - cached.updatedAt > BALANCE_MAX_AGE_MS;
}

export function isPriceStale(): boolean {
  const cached = getCachedPrice();
  if (!cached) return true;
  return Date.now() - cached.updatedAt > PRICE_MAX_AGE_MS;
}
