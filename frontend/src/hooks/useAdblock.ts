import { useState, useCallback, useEffect, useRef } from 'react';

// Phase 8c stage 3 batch 5 (2026-09-13): every call goes through the native, per-request-id
// bridge (`window.hodosBrowser.bridge.adblock*`).
//
// The previous version owned six `window.on*` single-slot globals, and EVERY one of them
// resolved a made-up default on timeout — 0 blocked, "toggle failed", "enabled" — so two
// calls in flight together handed one caller a silently wrong answer. Measured before this
// change (contract P8c-A6 RED): 3 concurrent `adblock_get_blocked_count`, 2 of 3 got the
// 3 s default. A genuine failure now REJECTS; the state setters only run on a real reply.
//
// ⛔ Booleans cross the bridge as the strings "true" / "false", exactly as the legacy IPC
// sent them; the browser handlers parse them.
const native = () => {
  const b = window.hodosBrowser?.bridge;
  if (!b) throw new Error('adblock: native bridge unavailable');
  return b;
};

export const useAdblock = () => {
  const [blockedCount, setBlockedCount] = useState<number>(0);
  const [adblockEnabled, setAdblockEnabled] = useState<boolean>(true);
  const [scriptletsEnabled, setScriptletsEnabled] = useState<boolean>(true);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Blocked count for the globally active tab (C++ resolves the tab; overlays have none)
  const fetchBlockedCount = useCallback(async (): Promise<number> => {
    const data = await native().adblockGetBlockedCount();
    setBlockedCount(data.count);
    return data.count;
  }, []);

  const resetBlockedCount = useCallback(async (): Promise<void> => {
    await native().adblockResetBlockedCount();
    setBlockedCount(0);
  }, []);

  // Toggle adblock for a domain → C++ local JSON
  const toggleSiteAdblock = useCallback(async (domain: string, enabled: boolean): Promise<boolean> => {
    const data = await native().adblockSiteToggle(domain, enabled.toString());
    setAdblockEnabled(data.adblockEnabled);
    return data.success;
  }, []);

  // Toggle scriptlet injection for a domain → C++ local JSON
  const toggleScriptlets = useCallback(async (domain: string, enabled: boolean): Promise<boolean> => {
    const data = await native().adblockScriptletToggle(domain, enabled.toString());
    setScriptletsEnabled(data.scriptletsEnabled);
    return data.success;
  }, []);

  const checkScriptlets = useCallback(async (domain: string): Promise<boolean> => {
    const data = await native().adblockCheckScriptletsEnabled(domain);
    setScriptletsEnabled(data.scriptletsEnabled);
    return data.scriptletsEnabled;
  }, []);

  const checkSiteAdblock = useCallback(async (domain: string): Promise<boolean> => {
    const data = await native().adblockCheckSiteEnabled(domain);
    setAdblockEnabled(data.adblockEnabled);
    return data.adblockEnabled;
  }, []);

  // Poll blocked count periodically (every 10s while mounted). An interval tick has no
  // caller to reject to, so a failed poll is swallowed; the next tick tries again.
  useEffect(() => {
    fetchBlockedCount().catch(() => {});
    pollRef.current = setInterval(() => {
      fetchBlockedCount().catch(() => {});
    }, 10000);

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
