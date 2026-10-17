import { useState, useCallback, useRef, useEffect } from 'react';
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

const CHECKING_RETRY_MS = 3_000;

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

  const [isLoading, setIsLoading] = useState(!cachedBal);
  // The wallet answered `checking`: it is still proving coins (after an update), so its
  // balance would leave them out. Show "checking" and ask again until it is whole.
  const [isChecking, setIsChecking] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const mountedRef = useRef(true);
  const hasAutoFetched = useRef(false);

  // Fetch balance + price via CEF bridge (used by manual refresh only)
  const fetchBalance = useCallback(async (): Promise<{ balance: number; price: number; checking: boolean }> => {
    if (!window.hodosBrowser?.wallet) {
      throw new Error('Bitcoin Browser wallet not available');
    }

    const response = await window.hodosBrowser.wallet.getBalance();
    const bal = response.balance;
    const price = response.bsvPrice ?? 0;
    const checking = response.checking === true;

    if (mountedRef.current) {
      setIsChecking(checking);
      if (!checking) setBalance(bal);
      if (price > 0) {
        setBsvPrice(price);
      }
    }
    if (!checking) setCachedBalance(bal);
    if (price > 0) {
      setCachedPrice(price);
    }
    return { balance: bal, price, checking };
  }, []);

  // Explicit refresh — for refresh button and post-send only.
  const refreshBalance = useCallback(async () => {
    setIsRefreshing(true);
    setError(null);

    try {
      const { balance: balResult, price: priceResult, checking } = await fetchBalance();

      if (mountedRef.current && !checking) {
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

  // Auto-fetch on mount when cache is empty (e.g., after wallet recovery)
  useEffect(() => {
    if (!cachedBal && !hasAutoFetched.current) {
      hasAutoFetched.current = true;
      fetchBalance()
        .then(({ balance: b, price: p, checking }) => {
          if (mountedRef.current) {
            const effectivePrice = p > 0 ? p : bsvPrice;
            if (!checking) setUsdValue(calculateUsdValue(b, effectivePrice));
            setIsLoading(false);
          }
        })
        .catch(() => {
          if (mountedRef.current) setIsLoading(false);
        });
    }
    return () => { mountedRef.current = false; };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // While the wallet is checking, ask again every few seconds until the balance is whole.
  // The next ask is scheduled only after the previous one ends: the shell answers each on
  // a shared file thread, so asks must never pile up (/code-review 179 round 2, #7).
  useEffect(() => {
    if (!isChecking) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const ask = () => {
      fetchBalance()
        .then(({ balance: b, price: p, checking }) => {
          if (!mountedRef.current) return;
          setError(null);
          if (!checking) setUsdValue(calculateUsdValue(b, p > 0 ? p : bsvPrice));
        })
        .catch((err: unknown) => {
          // The wallet did not answer: say so under the spinner and keep asking — never a
          // number, and the panel recovers by itself when the wallet does (delta review 2).
          if (mountedRef.current) setError(err instanceof Error ? err.message : 'Failed to refresh');
        })
        .finally(() => {
          if (!stopped) timer = setTimeout(ask, CHECKING_RETRY_MS);
        });
    };
    timer = setTimeout(ask, CHECKING_RETRY_MS);
    return () => { stopped = true; clearTimeout(timer); };
  }, [isChecking, fetchBalance, bsvPrice]);

  return {
    balance,
    usdValue,
    bsvPrice,
    isLoading,
    isRefreshing,
    isChecking,
    error,
    refreshBalance,
  };
};
