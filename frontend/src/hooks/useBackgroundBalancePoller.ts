import { useEffect, useRef } from 'react';
import { setCachedBalance, setCachedPrice } from '../services/balanceCache';

const BALANCE_POLL_MS = 30_000;   // 30s — matches useBalance

/**
 * Background poller that keeps the localStorage balance/price cache warm.
 * Designed to run in MainBrowserView (always mounted in the main CEF process)
 * so that wallet overlay subprocesses read fresh cached data on open.
 *
 * Price comes from the backend /wallet/balance response (backend has its own
 * 5-min TTL price cache), so no separate price polling needed.
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
          // Price comes from backend alongside balance
          if (response.bsvPrice && response.bsvPrice > 0) {
            setCachedPrice(response.bsvPrice);
          }
        }
      } catch {
        // Silent — background poller should not surface errors
      }
    };

    // Initial fetch with delay to let the V8 bridge finish injecting
    const initTimeout = setTimeout(() => {
      fetchBalance();
    }, 500);

    // Balance poller on steady interval (price piggybacks on the same response)
    const balanceInterval = setInterval(fetchBalance, BALANCE_POLL_MS);

    return () => {
      mountedRef.current = false;
      clearTimeout(initTimeout);
      clearInterval(balanceInterval);
    };
  }, []);
}
