import { useState, useCallback, useEffect, useRef } from 'react';
import {
  getCachedBalance, setCachedBalance,
  getCachedPrice, setCachedPrice,
} from '../services/balanceCache';

// Helper function to calculate USD value from balance and price (reusable)
export const calculateUsdValue = (balanceSatoshis: number, bsvPriceUsd: number): number => {
  if (balanceSatoshis <= 0 || bsvPriceUsd <= 0) {
    return 0;
  }
  return (balanceSatoshis / 100000000) * bsvPriceUsd;
};

const BALANCE_POLL_MS = 30_000;   // 30s — matches Rust backend cache TTL

export const useBalance = () => {
  // Seed state from localStorage cache synchronously (instant display)
  const cachedBal = getCachedBalance();
  const cachedPri = getCachedPrice();

  const [balance, setBalance] = useState(cachedBal?.balance ?? 0);
  const [bsvPrice, setBsvPrice] = useState(cachedPri?.price ?? 0);
  const [usdValue, setUsdValue] = useState(
    calculateUsdValue(cachedBal?.balance ?? 0, cachedPri?.price ?? 0)
  );

  // isLoading: true ONLY when no cached data exists (truly first load)
  const [isLoading, setIsLoading] = useState(!cachedBal);
  // isRefreshing: true during any background or explicit refresh
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Refs for cleanup
  const mountedRef = useRef(true);
  const balanceIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Fetch balance + price from backend (single call)
  const fetchBalance = useCallback(async (): Promise<{ balance: number; price: number }> => {
    if (!window.hodosBrowser?.wallet) {
      throw new Error('Bitcoin Browser wallet not available');
    }

    const response = await window.hodosBrowser.wallet.getBalance();
    const bal = response.balance;
    const price = response.bsvPrice ?? 0;

    if (mountedRef.current) {
      setBalance(bal);
      if (price > 0) {
        setBsvPrice(price);
      }
    }
    setCachedBalance(bal);
    if (price > 0) {
      setCachedPrice(price);
    }
    return { balance: bal, price };
  }, []);

  // Explicit refresh — for refresh button and post-send.
  // Shows isRefreshing but keeps current values visible.
  const refreshBalance = useCallback(async () => {
    setIsRefreshing(true);
    setError(null);

    try {
      const { balance: balResult, price: priceResult } = await fetchBalance();

      if (mountedRef.current) {
        const effectivePrice = priceResult > 0 ? priceResult : bsvPrice;
        const usd = calculateUsdValue(balResult, effectivePrice);
        setUsdValue(usd);
        console.log(`Balance: ${balResult} sats, Price: $${effectivePrice}, USD: $${usd.toFixed(2)}`);
      }
    } catch (err) {
      if (mountedRef.current) {
        const msg = err instanceof Error ? err.message : 'Failed to refresh';
        setError(msg);
        console.error('Balance refresh error:', msg);
      }
    } finally {
      if (mountedRef.current) {
        setIsLoading(false);
        setIsRefreshing(false);
      }
    }
  }, [fetchBalance, bsvPrice]);

  // On mount: initial refresh + start balance poller
  useEffect(() => {
    mountedRef.current = true;

    // Initial refresh (cached data is already displayed, this updates silently)
    const initTimeout = setTimeout(() => refreshBalance(), 100);

    // Balance + price poller — 30s interval (price comes from backend with 5-min TTL)
    balanceIntervalRef.current = setInterval(async () => {
      try {
        const { balance: bal, price } = await fetchBalance();
        if (mountedRef.current) {
          const effectivePrice = price > 0 ? price : bsvPrice;
          setUsdValue(calculateUsdValue(bal, effectivePrice));
        }
      } catch {
        // Silent — pollers should not surface errors
      }
    }, BALANCE_POLL_MS);

    return () => {
      mountedRef.current = false;
      clearTimeout(initTimeout);
      if (balanceIntervalRef.current) clearInterval(balanceIntervalRef.current);
    };
  }, [refreshBalance, fetchBalance, bsvPrice]);

  return {
    balance,
    usdValue,
    bsvPrice,
    isLoading,
    isRefreshing,
    error,
    refreshBalance,
  };
};
