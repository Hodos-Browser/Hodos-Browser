import { useState, useCallback, useEffect } from 'react';
// Removed useWallet import - using HD wallet system directly

// Helper function to calculate USD value from balance and price (reusable)
export const calculateUsdValue = (balanceSatoshis: number, bsvPriceUsd: number): number => {
  if (balanceSatoshis <= 0 || bsvPriceUsd <= 0) {
    return 0;
  }
  return (balanceSatoshis / 100000000) * bsvPriceUsd;
};

export const useBalance = () => {
  const [balance, setBalance] = useState(0);
  const [usdValue, setUsdValue] = useState(0);
  const [bsvPrice, setBsvPrice] = useState<number>(0); // BSV price in USD (separate from balance)
  const [isLoading, setIsLoading] = useState(true); // Start as true since we'll fetch on mount
  const [error, setError] = useState<string | null>(null);

  const fetchBalance = useCallback(async (): Promise<number> => {
    // Loading state managed by refreshBalance
    setError(null);

    try {
      // Call C++ bridge via window.bitcoinBrowser.wallet
      if (!window.bitcoinBrowser?.wallet) {
        throw new Error('Bitcoin Browser wallet not available');
      }

      // Get total balance across all addresses (no address parameter needed)
      const response = await window.bitcoinBrowser.wallet.getBalance();

      setBalance(response.balance);
      return response.balance;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to fetch balance';
      setError(errorMessage);
      throw new Error(errorMessage);
    }
  }, []);

  const fetchUsdPrice = useCallback(async (): Promise<number> => {
    try {
      console.log('🔍 Fetching BSV price from CryptoCompare API...');

      // Use only CryptoCompare API - no fallbacks
      const response = await fetch('https://min-api.cryptocompare.com/data/price?fsym=BSV&tsyms=USD', {
        method: 'GET',
        headers: {
          'Accept': 'application/json',
          'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        },
        mode: 'cors'
      });

      if (!response.ok) {
        throw new Error(`CryptoCompare API failed with status: ${response.status}`);
      }

      const data = await response.json();
      console.log('🔍 CryptoCompare response:', data);

      const price = parseFloat(data.USD);
      if (!price || price <= 0) {
        throw new Error('Invalid price data received from CryptoCompare API');
      }

      // Store price separately - USD value will be calculated when balance is available
      setBsvPrice(price);
      console.log(`💰 BSV Price: $${price}`);
      return price;

    } catch (err) {
      console.error('❌ Failed to fetch BSV price:', err);
      console.error('🔍 This indicates a problem with the CryptoCompare API - investigate network connectivity and API status');
      setBsvPrice(0);
      throw new Error(`Price fetch failed: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  }, []); // Remove balance dependency - now independent

  const refreshBalance = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      // Run both in parallel for better performance
      const [balanceResult, priceResult] = await Promise.all([
        fetchBalance(),
        fetchUsdPrice()
      ]);

      // Calculate USD value immediately with both results
      // This reduces state updates and eliminates the separate useEffect
      const calculatedUsdValue = calculateUsdValue(balanceResult, priceResult);
      setUsdValue(calculatedUsdValue);

      console.log(`💰 Balance: ${balanceResult} satoshis, Price: $${priceResult}, USD Value: $${calculatedUsdValue.toFixed(2)}`);
    } catch (err) {
      // Error handling already done in individual functions
      // But we should log if both fail
      const errorMessage = err instanceof Error ? err.message : 'Failed to refresh balance';
      setError(errorMessage);
      console.error('❌ Error in refreshBalance:', errorMessage);
      // Reset USD value on error
      setUsdValue(0);
    } finally {
      setIsLoading(false);
    }
  }, [fetchBalance, fetchUsdPrice]);

  // Auto-refresh balance every 30 seconds - DISABLED FOR DEBUGGING
  // useEffect(() => {
  //   const interval = setInterval(() => {
  //     refreshBalance();
  //   }, 30000);

  //   return () => clearInterval(interval);
  // }, [refreshBalance]);

  // Initial load - deferred to allow component to render first
  useEffect(() => {
    // Small timeout to allow panel to render before fetching
    // 100ms delay is imperceptible to user but allows render
    const timeoutId = setTimeout(() => {
      refreshBalance();
    }, 100);

    return () => clearTimeout(timeoutId);
  }, [refreshBalance]);

  return {
    balance,
    usdValue,
    isLoading,
    error,
    fetchBalance,
    fetchUsdPrice,
    refreshBalance
  };
};
