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
const PRICE_POLL_MS = 300_000;    // 5min — respects external API rate limits

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
  const priceIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchBalance = useCallback(async (): Promise<number> => {
    if (!window.hodosBrowser?.wallet) {
      throw new Error('Bitcoin Browser wallet not available');
    }

    const response = await window.hodosBrowser.wallet.getBalance();
    const bal = response.balance;

    if (mountedRef.current) {
      setBalance(bal);
    }
    setCachedBalance(bal);
    return bal;
  }, []);

  const fetchUsdPrice = useCallback(async (): Promise<number> => {
    // Try CryptoCompare first (primary)
    try {
      const response = await fetch('https://min-api.cryptocompare.com/data/price?fsym=BSV&tsyms=USD', {
        method: 'GET',
        headers: { 'Accept': 'application/json' },
        mode: 'cors'
      });

      if (!response.ok) {
        throw new Error(`CryptoCompare API failed with status: ${response.status}`);
      }

      const data = await response.json();
      const price = parseFloat(data.USD);
      if (!price || price <= 0) {
        throw new Error('Invalid price data from CryptoCompare');
      }

      if (mountedRef.current) {
        setBsvPrice(price);
      }
      setCachedPrice(price);
      return price;

    } catch (primaryErr) {
      console.warn('CryptoCompare failed, trying CoinGecko...', primaryErr);

      // Fallback to CoinGecko
      const response = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=bitcoin-sv&vs_currencies=usd', {
        method: 'GET',
        headers: { 'Accept': 'application/json' },
        mode: 'cors'
      });

      if (!response.ok) {
        throw new Error(`CoinGecko API failed with status: ${response.status}`);
      }

      const data = await response.json();
      const price = parseFloat(data['bitcoin-sv']?.usd);
      if (!price || price <= 0) {
        throw new Error('Invalid price data from CoinGecko');
      }

      if (mountedRef.current) {
        setBsvPrice(price);
      }
      setCachedPrice(price);
      return price;
    }
  }, []);

  // Explicit refresh — for refresh button and post-send.
  // Shows isRefreshing but keeps current values visible.
  const refreshBalance = useCallback(async () => {
    setIsRefreshing(true);
    setError(null);

    try {
      const [balResult, priceResult] = await Promise.all([
        fetchBalance(),
        fetchUsdPrice()
      ]);

      if (mountedRef.current) {
        const usd = calculateUsdValue(balResult, priceResult);
        setUsdValue(usd);
        console.log(`Balance: ${balResult} sats, Price: $${priceResult}, USD: $${usd.toFixed(2)}`);
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
  }, [fetchBalance, fetchUsdPrice]);

  // On mount: initial refresh + start two independent pollers
  useEffect(() => {
    mountedRef.current = true;

    // Initial refresh (cached data is already displayed, this updates silently)
    const initTimeout = setTimeout(() => refreshBalance(), 100);

    // Balance poller — 30s interval
    balanceIntervalRef.current = setInterval(async () => {
      try {
        const bal = await fetchBalance();
        if (mountedRef.current) {
          // Recalculate USD with latest cached price
          const cached = getCachedPrice();
          if (cached) {
            setUsdValue(calculateUsdValue(bal, cached.price));
          }
        }
      } catch {
        // Silent — pollers should not surface errors
      }
    }, BALANCE_POLL_MS);

    // Price poller — 5min interval
    priceIntervalRef.current = setInterval(async () => {
      try {
        const price = await fetchUsdPrice();
        if (mountedRef.current) {
          // Recalculate USD with latest cached balance
          const cached = getCachedBalance();
          if (cached) {
            setUsdValue(calculateUsdValue(cached.balance, price));
          }
        }
      } catch {
        // Silent
      }
    }, PRICE_POLL_MS);

    return () => {
      mountedRef.current = false;
      clearTimeout(initTimeout);
      if (balanceIntervalRef.current) clearInterval(balanceIntervalRef.current);
      if (priceIntervalRef.current) clearInterval(priceIntervalRef.current);
    };
  }, [refreshBalance, fetchBalance, fetchUsdPrice]);

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
