import { useCallback, useEffect, useState } from 'react';

interface PaidCacheSizeResponse {
  totalBytes: number;
  enabled: boolean;
}

interface PaidCacheClearResponse {
  success: boolean;
  totalBytes: number;
}

declare global {
  interface Window {
    onPaidCacheGetSizeResponse?: (data: PaidCacheSizeResponse) => void;
    onPaidCacheClearResponse?: (data: PaidCacheClearResponse) => void;
  }
}

// Phase 1 BRC-121 — Paid Content Cache: thin hook over the IPC pair
// `paid_cache_get_size` / `paid_cache_clear`. Mirrors the shape of
// useCookies' getCacheSize/clearCache so CachePanel can drop these in
// next to the existing Cache + Cookies cards.
export function usePaidCache() {
  const [totalBytes, setTotalBytes] = useState<number>(0);
  const [enabled, setEnabled] = useState<boolean>(true);
  const [loading, setLoading] = useState<boolean>(true);

  const refresh = useCallback(() => {
    return new Promise<PaidCacheSizeResponse>((resolve, reject) => {
      let done = false;
      const timeout = setTimeout(() => {
        if (done) return;
        done = true;
        delete window.onPaidCacheGetSizeResponse;
        reject(new Error('paid_cache_get_size timeout'));
      }, 3000);

      window.onPaidCacheGetSizeResponse = (data) => {
        if (done) return;
        done = true;
        clearTimeout(timeout);
        delete window.onPaidCacheGetSizeResponse;
        setTotalBytes(data.totalBytes);
        setEnabled(data.enabled);
        setLoading(false);
        resolve(data);
      };

      if (window.cefMessage?.send) {
        window.cefMessage.send('paid_cache_get_size', []);
      } else {
        done = true;
        clearTimeout(timeout);
        delete window.onPaidCacheGetSizeResponse;
        reject(new Error('cefMessage not available'));
      }
    });
  }, []);

  const clear = useCallback(() => {
    return new Promise<PaidCacheClearResponse>((resolve, reject) => {
      let done = false;
      const timeout = setTimeout(() => {
        if (done) return;
        done = true;
        delete window.onPaidCacheClearResponse;
        reject(new Error('paid_cache_clear timeout'));
      }, 3000);

      window.onPaidCacheClearResponse = (data) => {
        if (done) return;
        done = true;
        clearTimeout(timeout);
        delete window.onPaidCacheClearResponse;
        setTotalBytes(data.totalBytes);
        resolve(data);
      };

      if (window.cefMessage?.send) {
        window.cefMessage.send('paid_cache_clear', []);
      } else {
        done = true;
        clearTimeout(timeout);
        delete window.onPaidCacheClearResponse;
        reject(new Error('cefMessage not available'));
      }
    });
  }, []);

  useEffect(() => {
    refresh().catch(() => setLoading(false));
  }, [refresh]);

  return { totalBytes, enabled, loading, refresh, clear };
}
