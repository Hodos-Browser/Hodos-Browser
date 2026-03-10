import { useState, useCallback, useEffect, useRef } from 'react';

declare global {
  interface Window {
    onAdblockBlockedCountResponse?: (data: { count: number }) => void;
    onAdblockResetBlockedCountResponse?: (data: { success: boolean }) => void;
    onAdblockSiteToggleResponse?: (data: { domain: string; adblockEnabled: boolean; success: boolean }) => void;
    onAdblockScriptletToggleResponse?: (data: { domain: string; scriptletsEnabled: boolean; success: boolean }) => void;
    onAdblockCheckSiteEnabledResponse?: (data: { domain: string; adblockEnabled: boolean }) => void;
    onAdblockCheckScriptletsEnabledResponse?: (data: { domain: string; scriptletsEnabled: boolean }) => void;
  }
}

export const useAdblock = () => {
  const [blockedCount, setBlockedCount] = useState<number>(0);
  const [adblockEnabled, setAdblockEnabled] = useState<boolean>(true);
  const [scriptletsEnabled, setScriptletsEnabled] = useState<boolean>(true);
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

  // Toggle adblock for a domain via IPC → C++ local JSON
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

  // Toggle scriptlet injection for a domain via IPC → C++ local JSON
  const toggleScriptlets = useCallback((domain: string, enabled: boolean) => {
    return new Promise<boolean>((resolve) => {
      const timeout = setTimeout(() => {
        resolve(false);
        delete window.onAdblockScriptletToggleResponse;
      }, 5000);

      window.onAdblockScriptletToggleResponse = (data) => {
        clearTimeout(timeout);
        setScriptletsEnabled(data.scriptletsEnabled);
        resolve(data.success);
        delete window.onAdblockScriptletToggleResponse;
      };

      window.cefMessage?.send('adblock_scriptlet_toggle', [domain, enabled.toString()]);
    });
  }, []);

  // Check per-site scriptlet status via IPC → C++ local JSON
  const checkScriptlets = useCallback((domain: string) => {
    return new Promise<boolean>((resolve) => {
      const timeout = setTimeout(() => {
        resolve(true);
        delete window.onAdblockCheckScriptletsEnabledResponse;
      }, 3000);

      window.onAdblockCheckScriptletsEnabledResponse = (data) => {
        clearTimeout(timeout);
        setScriptletsEnabled(data.scriptletsEnabled);
        resolve(data.scriptletsEnabled);
        delete window.onAdblockCheckScriptletsEnabledResponse;
      };

      window.cefMessage?.send('adblock_check_scriptlets_enabled', [domain]);
    });
  }, []);

  // Check per-site adblock status via IPC → C++ local JSON
  const checkSiteAdblock = useCallback((domain: string) => {
    return new Promise<boolean>((resolve) => {
      const timeout = setTimeout(() => {
        resolve(true);
        delete window.onAdblockCheckSiteEnabledResponse;
      }, 3000);

      window.onAdblockCheckSiteEnabledResponse = (data) => {
        clearTimeout(timeout);
        setAdblockEnabled(data.adblockEnabled);
        resolve(data.adblockEnabled);
        delete window.onAdblockCheckSiteEnabledResponse;
      };

      window.cefMessage?.send('adblock_check_site_enabled', [domain]);
    });
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
    scriptletsEnabled,
    fetchBlockedCount,
    resetBlockedCount,
    toggleSiteAdblock,
    checkSiteAdblock,
    toggleScriptlets,
    checkScriptlets,
  };
};
