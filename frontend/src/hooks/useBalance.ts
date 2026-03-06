import { useState, useCallback, useRef } from 'react';
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

export const useBalance = () => {
  // Seed state from localStorage cache synchronously (instant display).
  // The background poller in MainBrowserView keeps this cache fresh (30s).
  // No auto-fetch on mount — cached data is already current.
  const cachedBal = getCachedBalance();
  const cachedPri = getCachedPrice();

  const [balance, setBalance] = useState(cachedBal?.balance ?? 0);
  const [bsvPrice, setBsvPrice] = useState(cachedPri?.price ?? 0);
  const [usdValue, setUsdValue] = useState(
    calculateUsdValue(cachedBal?.balance ?? 0, cachedPri?.price ?? 0)
  );

  const [isLoading] = useState(!cachedBal);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const mountedRef = useRef(true);

  // Fetch balance + price via CEF bridge (used by manual refresh only)
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

  // Explicit refresh — for refresh button and post-send only.
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
        setIsRefreshing(false);
      }
    }
  }, [fetchBalance, bsvPrice]);

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
