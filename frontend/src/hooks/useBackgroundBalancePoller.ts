import { useEffect, useRef } from 'react';
import { setCachedBalance, setCachedPrice } from '../services/balanceCache';

const BALANCE_POLL_MS = 30_000;   // 30s — matches useBalance
const PRICE_POLL_MS = 300_000;    // 5min — matches useBalance

/**
 * Background poller that keeps the localStorage balance/price cache warm.
 * Designed to run in MainBrowserView (always mounted in the main CEF process)
 * so that wallet overlay subprocesses read fresh cached data on open.
 */
export function useBackgroundBalancePoller() {
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;

    const fetchBalance = async () => {
      try {
        if (!window.hodosBrowser?.wallet) return;
        const response = await window.hodosBrowser.wallet.getBalance();
        if (mountedRef.current) {
          setCachedBalance(response.balance);
        }
      } catch {
        // Silent — background poller should not surface errors
      }
    };

    const fetchPrice = async () => {
      // Primary: CryptoCompare
      try {
        const response = await fetch(
          'https://min-api.cryptocompare.com/data/price?fsym=BSV&tsyms=USD',
          { method: 'GET', headers: { Accept: 'application/json' }, mode: 'cors' }
        );
        if (!response.ok) throw new Error('CryptoCompare failed');
        const data = await response.json();
        const price = parseFloat(data.USD);
        if (!price || price <= 0) throw new Error('Invalid price');
        if (mountedRef.current) setCachedPrice(price);
        return;
      } catch {
        // Fall through to CoinGecko
      }

      // Fallback: CoinGecko
      try {
        const response = await fetch(
          'https://api.coingecko.com/api/v3/simple/price?ids=bitcoin-sv&vs_currencies=usd',
          { method: 'GET', headers: { Accept: 'application/json' }, mode: 'cors' }
        );
        if (!response.ok) return;
        const data = await response.json();
        const price = parseFloat(data['bitcoin-sv']?.usd);
        if (price && price > 0 && mountedRef.current) {
          setCachedPrice(price);
        }
      } catch {
        // Silent
      }
    };

    // Initial fetch with delay to let the V8 bridge finish injecting
    const initTimeout = setTimeout(() => {
      fetchBalance();
      fetchPrice();
    }, 500);

    // Independent pollers on steady intervals
    const balanceInterval = setInterval(fetchBalance, BALANCE_POLL_MS);
    const priceInterval = setInterval(fetchPrice, PRICE_POLL_MS);

    return () => {
      mountedRef.current = false;
      clearTimeout(initTimeout);
      clearInterval(balanceInterval);
      clearInterval(priceInterval);
    };
  }, []);
}
