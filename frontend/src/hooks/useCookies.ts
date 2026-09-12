import { useState, useCallback } from 'react';
import type { CookieData, DomainCookieGroup, CookieDeleteResponse, CacheSizeResponse } from '../types/cookies';

// Phase 8c stage 3 batch 3 (2026-09-12): every call goes through the native, per-request-id
// bridge (`window.hodosBrowser.bridge.cookie*` / `cache*`).
//
// The previous version of this hook owned its own `window.on*` single-slot globals — a
// second copy of the pattern `initWindowBridge.ts` also carried (that copy had no callers
// and was deleted with this change). Two calls in flight together could steal each
// other's reply. The writes here had NO timeout at all, so a stolen reply hung the caller
// forever, and `fetchAllCookies` resolved `[]` after 5 s — partly to paper over CEF never
// calling the cookie visitor for an empty jar, which `CookieManager.cpp` now answers
// itself. A genuine failure now REJECTS, and the hook records it in `error`.
const native = () => {
  const b = window.hodosBrowser?.bridge;
  if (!b) throw new Error('cookies: native bridge unavailable');
  return b;
};

export const useCookies = () => {
  const [cookies, setCookies] = useState<CookieData[]>([]);
  const [domainGroups, setDomainGroups] = useState<DomainCookieGroup[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cacheSize, setCacheSize] = useState<number>(0);

  // Helper: group cookies by domain
  const groupByDomain = useCallback((cookieList: CookieData[]): DomainCookieGroup[] => {
    const groups = new Map<string, CookieData[]>();
    for (const cookie of cookieList) {
      // Remove leading dot from domain for grouping (e.g., ".google.com" -> "google.com")
      const domain = cookie.domain.startsWith('.') ? cookie.domain.slice(1) : cookie.domain;
      if (!groups.has(domain)) {
        groups.set(domain, []);
      }
      groups.get(domain)!.push(cookie);
    }
    return Array.from(groups.entries())
      .map(([domain, cookies]) => ({
        domain,
        cookies,
        totalSize: cookies.reduce((sum, c) => sum + c.size, 0),
        count: cookies.length,
      }))
      .sort((a, b) => b.count - a.count); // Sort by cookie count descending
  }, []);

  const fail = (e: unknown): never => {
    setError(e instanceof Error ? e.message : String(e));
    throw e;
  };

  const fetchAllCookies = useCallback(async (): Promise<CookieData[]> => {
    setLoading(true);
    setError(null);
    try {
      const data = await native().cookieGetAll();
      setCookies(data);
      setDomainGroups(groupByDomain(data));
      return data;
    } catch (e) {
      return fail(e);
    } finally {
      setLoading(false);
    }
  }, [groupByDomain]);

  const deleteCookie = useCallback(async (url: string, name: string): Promise<CookieDeleteResponse> => {
    try {
      const data = await native().cookieDelete(url, name);
      // Remove deleted cookie from local state
      setCookies(prev => {
        const updated = prev.filter(c => !(c.name === name && (c.domain === url || `https://${c.domain}` === url || `http://${c.domain}` === url)));
        setDomainGroups(groupByDomain(updated));
        return updated;
      });
      return data;
    } catch (e) {
      return fail(e);
    }
  }, [groupByDomain]);

  const deleteDomainCookies = useCallback(async (domain: string): Promise<CookieDeleteResponse> => {
    try {
      const data = await native().cookieDeleteDomain(domain);
      // Remove all cookies for this domain from local state
      setCookies(prev => {
        const updated = prev.filter(c => {
          const cookieDomain = c.domain.startsWith('.') ? c.domain.slice(1) : c.domain;
          return cookieDomain !== domain;
        });
        setDomainGroups(groupByDomain(updated));
        return updated;
      });
      return data;
    } catch (e) {
      return fail(e);
    }
  }, [groupByDomain]);

  const deleteAllCookies = useCallback(async (): Promise<CookieDeleteResponse> => {
    try {
      const data = await native().cookieDeleteAll();
      setCookies([]);
      setDomainGroups([]);
      return data;
    } catch (e) {
      return fail(e);
    }
  }, []);

  const clearCache = useCallback(async (): Promise<{ success: boolean }> => {
    try {
      return await native().cacheClear();
    } catch (e) {
      return fail(e);
    }
  }, []);

  const getCacheSize = useCallback(async (): Promise<CacheSizeResponse> => {
    try {
      const data = await native().cacheGetSize();
      setCacheSize(data.totalBytes);
      return data;
    } catch (e) {
      return fail(e);
    }
  }, []);

  return {
    cookies,
    domainGroups,
    loading,
    error,
    cacheSize,
    fetchAllCookies,
    deleteCookie,
    deleteDomainCookies,
    deleteAllCookies,
    clearCache,
    getCacheSize,
    groupByDomain,
  };
};
