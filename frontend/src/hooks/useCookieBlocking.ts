import { useState, useCallback, useEffect, useRef } from 'react';
import type {
  BlockedDomainEntry,
  BlockLogEntry,
  BlockDomainResponse,
  UnblockDomainResponse,
  AllowThirdPartyResponse,
  BlockedCountResponse,
  ClearBlockLogResponse,
} from '../types/cookieBlocking';

// Phase 8c stage 3 batch 3 (2026-09-12): every call goes through the native, per-request-id
// bridge (`window.hodosBrowser.bridge.cookie*`). See `useCookies.ts` for the history —
// this hook owned the same kind of `window.on*` single-slot globals, and four of its reads
// (`fetchBlockList`, `fetchBlockLog`, `fetchBlockedCount` …) RESOLVED a default on timeout,
// so a stolen reply produced a silently wrong value. A genuine failure now rejects.
//
// ⛔ Arguments cross the bridge as STRINGS, exactly as the legacy IPC sent them
// (`isWildcard.toString()`, `limit.toString()`); the browser-side handlers parse them.
const native = () => {
  const b = window.hodosBrowser?.bridge;
  if (!b) throw new Error('cookieBlocking: native bridge unavailable');
  return b;
};

export const useCookieBlocking = () => {
  const [blockedDomains, setBlockedDomains] = useState<BlockedDomainEntry[]>([]);
  const [blockLog, setBlockLog] = useState<BlockLogEntry[]>([]);
  const [blockedCount, setBlockedCount] = useState<number>(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fail = (e: unknown): never => {
    setError(e instanceof Error ? e.message : String(e));
    throw e;
  };

  const fetchBlockList = useCallback(async (): Promise<BlockedDomainEntry[]> => {
    setLoading(true);
    setError(null);
    try {
      const data = await native().cookieGetBlocklist();
      setBlockedDomains(data);
      return data;
    } catch (e) {
      return fail(e);
    } finally {
      setLoading(false);
    }
  }, []);

  const blockDomain = useCallback(async (domain: string, isWildcard: boolean): Promise<BlockDomainResponse> => {
    setError(null);
    let result: BlockDomainResponse;
    try {
      result = await native().cookieBlockDomain(domain, isWildcard.toString());
    } catch (e) {
      return fail(e);
    }
    // Re-fetch block list after successful block
    await fetchBlockList();
    return result;
  }, [fetchBlockList]);

  const unblockDomain = useCallback(async (domain: string): Promise<UnblockDomainResponse> => {
    setError(null);
    let result: UnblockDomainResponse;
    try {
      result = await native().cookieUnblockDomain(domain);
    } catch (e) {
      return fail(e);
    }
    // Re-fetch block list after successful unblock
    await fetchBlockList();
    return result;
  }, [fetchBlockList]);

  const allowThirdParty = useCallback(async (domain: string): Promise<AllowThirdPartyResponse> => {
    setError(null);
    try {
      return await native().cookieAllowThirdParty(domain);
    } catch (e) {
      return fail(e);
    }
  }, []);

  const removeThirdPartyAllow = useCallback(async (domain: string): Promise<AllowThirdPartyResponse> => {
    setError(null);
    try {
      return await native().cookieRemoveThirdPartyAllow(domain);
    } catch (e) {
      return fail(e);
    }
  }, []);

  const fetchBlockLog = useCallback(async (limit: number = 100, offset: number = 0): Promise<BlockLogEntry[]> => {
    setLoading(true);
    setError(null);
    try {
      const data = await native().cookieGetBlockLog(limit.toString(), offset.toString());
      setBlockLog(data);
      return data;
    } catch (e) {
      return fail(e);
    } finally {
      setLoading(false);
    }
  }, []);

  const clearBlockLog = useCallback(async (): Promise<ClearBlockLogResponse> => {
    setError(null);
    try {
      const data = await native().cookieClearBlockLog();
      setBlockLog([]);
      return data;
    } catch (e) {
      return fail(e);
    }
  }, []);

  const fetchBlockedCount = useCallback(async (): Promise<BlockedCountResponse> => {
    setError(null);
    try {
      const data = await native().cookieGetBlockedCount();
      setBlockedCount(data.count);
      return data;
    } catch (e) {
      return fail(e);
    }
  }, []);

  const resetBlockedCount = useCallback(async (): Promise<void> => {
    setError(null);
    try {
      await native().cookieResetBlockedCount();
      setBlockedCount(0);
    } catch (e) {
      fail(e);
    }
  }, []);

  // Poll blocked count periodically (every 10s while mounted) — matches useAdblock pattern.
  // A poll that fails (bridge unavailable, or the 45 s native deadline) records `error`
  // and is otherwise swallowed here: an interval tick has no caller to reject to.
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
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
    blockedDomains,
    blockLog,
    blockedCount,
    loading,
    error,
    fetchBlockList,
    blockDomain,
    unblockDomain,
    allowThirdParty,
    removeThirdPartyAllow,
    fetchBlockLog,
    clearBlockLog,
    fetchBlockedCount,
    resetBlockedCount,
  };
};
