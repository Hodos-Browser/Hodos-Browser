import { useState, useCallback } from 'react';
import type { CookieData, DomainCookieGroup, CookieDeleteResponse, CacheSizeResponse } from '../types/cookies';

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

  const fetchAllCookies = useCallback(() => {
    setLoading(true);
    setError(null);
    return new Promise<CookieData[]>((resolve, reject) => {
      // Timeout after 5 seconds (handles case where no cookies exist and callback never fires)
      const timeout = setTimeout(() => {
        setLoading(false);
        setCookies([]);
        setDomainGroups([]);
        resolve([]);
        delete window.onCookieGetAllResponse;
        delete window.onCookieGetAllError;
      }, 5000);

      window.onCookieGetAllResponse = (data: CookieData[]) => {
        clearTimeout(timeout);
        setCookies(data);
        setDomainGroups(groupByDomain(data));
        setLoading(false);
        resolve(data);
        delete window.onCookieGetAllResponse;
        delete window.onCookieGetAllError;
      };

      window.onCookieGetAllError = (errorMsg: string) => {
        clearTimeout(timeout);
        setError(errorMsg);
        setLoading(false);
        reject(new Error(errorMsg));
        delete window.onCookieGetAllResponse;
        delete window.onCookieGetAllError;
      };

      window.cefMessage?.send('cookie_get_all', []);
    });
  }, [groupByDomain]);

  const deleteCookie = useCallback((url: string, name: string) => {
    return new Promise<CookieDeleteResponse>((resolve, reject) => {
      window.onCookieDeleteResponse = (data: CookieDeleteResponse) => {
        // Remove deleted cookie from local state
        setCookies(prev => {
          const updated = prev.filter(c => !(c.name === name && (c.domain === url || `https://${c.domain}` === url || `http://${c.domain}` === url)));
          setDomainGroups(groupByDomain(updated));
          return updated;
        });
        resolve(data);
        delete window.onCookieDeleteResponse;
        delete window.onCookieDeleteError;
      };

      window.onCookieDeleteError = (errorMsg: string) => {
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCookieDeleteResponse;
        delete window.onCookieDeleteError;
      };

      window.cefMessage?.send('cookie_delete', [url, name]);
    });
  }, [groupByDomain]);

  const deleteDomainCookies = useCallback((domain: string) => {
    return new Promise<CookieDeleteResponse>((resolve, reject) => {
      window.onCookieDeleteDomainResponse = (data: CookieDeleteResponse) => {
        // Remove all cookies for this domain from local state
        setCookies(prev => {
          const updated = prev.filter(c => {
            const cookieDomain = c.domain.startsWith('.') ? c.domain.slice(1) : c.domain;
            return cookieDomain !== domain;
          });
          setDomainGroups(groupByDomain(updated));
          return updated;
        });
        resolve(data);
        delete window.onCookieDeleteDomainResponse;
        delete window.onCookieDeleteDomainError;
      };

      window.onCookieDeleteDomainError = (errorMsg: string) => {
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCookieDeleteDomainResponse;
        delete window.onCookieDeleteDomainError;
      };

      window.cefMessage?.send('cookie_delete_domain', [domain]);
    });
  }, [groupByDomain]);

  const deleteAllCookies = useCallback(() => {
    return new Promise<CookieDeleteResponse>((resolve, reject) => {
      window.onCookieDeleteAllResponse = (data: CookieDeleteResponse) => {
        setCookies([]);
        setDomainGroups([]);
        resolve(data);
        delete window.onCookieDeleteAllResponse;
        delete window.onCookieDeleteAllError;
      };

      window.onCookieDeleteAllError = (errorMsg: string) => {
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCookieDeleteAllResponse;
        delete window.onCookieDeleteAllError;
      };

      window.cefMessage?.send('cookie_delete_all', []);
    });
  }, []);

  const clearCache = useCallback(() => {
    return new Promise<{ success: boolean }>((resolve, reject) => {
      window.onCacheClearResponse = (data: { success: boolean }) => {
        resolve(data);
        delete window.onCacheClearResponse;
        delete window.onCacheClearError;
      };

      window.onCacheClearError = (errorMsg: string) => {
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCacheClearResponse;
        delete window.onCacheClearError;
      };

      window.cefMessage?.send('cache_clear', []);
    });
  }, []);

  const getCacheSize = useCallback(() => {
    return new Promise<CacheSizeResponse>((resolve, reject) => {
      window.onCacheGetSizeResponse = (data: CacheSizeResponse) => {
        setCacheSize(data.totalBytes);
        resolve(data);
        delete window.onCacheGetSizeResponse;
        delete window.onCacheGetSizeError;
      };

      window.onCacheGetSizeError = (errorMsg: string) => {
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCacheGetSizeResponse;
        delete window.onCacheGetSizeError;
      };

      window.cefMessage?.send('cache_get_size', []);
    });
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
