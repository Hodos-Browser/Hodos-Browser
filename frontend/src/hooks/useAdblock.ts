import { useState, useCallback, useEffect, useRef } from 'react';

declare global {
  interface Window {
    onAdblockBlockedCountResponse?: (data: { count: number }) => void;
    onAdblockResetBlockedCountResponse?: (data: { success: boolean }) => void;
    onAdblockSiteToggleResponse?: (data: { domain: string; adblockEnabled: boolean; success: boolean }) => void;
  }
}

export const useAdblock = () => {
  const [blockedCount, setBlockedCount] = useState<number>(0);
  const [adblockEnabled, setAdblockEnabled] = useState<boolean>(true);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Fetch blocked count via IPC
  const fetchBlockedCount = useCallback(() => {
    return new Promise<number>((resolve) => {
      const timeout = setTimeout(() => {
        resolve(0);
        delete window.onAdblockBlockedCountResponse;
      }, 3000);

      window.onAdblockBlockedCountResponse = (data: { count: number }) => {
        clearTimeout(timeout);
        setBlockedCount(data.count);
        resolve(data.count);
        delete window.onAdblockBlockedCountResponse;
      };

      window.cefMessage?.send('adblock_get_blocked_count', '');
    });
  }, []);

  // Reset blocked count via IPC
  const resetBlockedCount = useCallback(() => {
    return new Promise<void>((resolve) => {
      const timeout = setTimeout(() => {
        resolve();
        delete window.onAdblockResetBlockedCountResponse;
      }, 3000);

      window.onAdblockResetBlockedCountResponse = () => {
        clearTimeout(timeout);
        setBlockedCount(0);
        resolve();
        delete window.onAdblockResetBlockedCountResponse;
      };

      window.cefMessage?.send('adblock_reset_blocked_count', '');
    });
  }, []);

  // Toggle adblock for a domain via IPC → C++ → Rust
  const toggleSiteAdblock = useCallback((domain: string, enabled: boolean) => {
    return new Promise<boolean>((resolve) => {
      const timeout = setTimeout(() => {
        resolve(false);
        delete window.onAdblockSiteToggleResponse;
      }, 5000);

      window.onAdblockSiteToggleResponse = (data) => {
        clearTimeout(timeout);
        setAdblockEnabled(data.adblockEnabled);
        resolve(data.success);
        delete window.onAdblockSiteToggleResponse;
      };

      window.cefMessage?.send('adblock_site_toggle', [domain, enabled.toString()]);
    });
  }, []);

  // Check per-site adblock status from Rust backend directly
  const checkSiteAdblock = useCallback(async (domain: string) => {
    try {
      const resp = await fetch(`http://localhost:3301/adblock/site-toggle?domain=${encodeURIComponent(domain)}`);
      if (resp.ok) {
        const data = await resp.json();
        setAdblockEnabled(data.adblockEnabled);
        return data.adblockEnabled as boolean;
      }
    } catch {
      // Backend not available — default to enabled
    }
    setAdblockEnabled(true);
    return true;
  }, []);

  // Poll blocked count periodically (every 2s while mounted)
  useEffect(() => {
    fetchBlockedCount();
    pollRef.current = setInterval(() => {
      fetchBlockedCount();
    }, 2000);

    return () => {
      if (pollRef.current) {
        clearInterval(pollRef.current);
      }
    };
  }, [fetchBlockedCount]);

  return {
    blockedCount,
    adblockEnabled,
    fetchBlockedCount,
    resetBlockedCount,
    toggleSiteAdblock,
    checkSiteAdblock,
  };
};
