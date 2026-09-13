import { useCallback, useEffect, useState } from 'react';

interface PaidCacheSizeResponse {
  totalBytes: number;
  enabled: boolean;
}

interface PaidCacheClearResponse {
  success: boolean;
  totalBytes: number;
}

// Phase 1 BRC-121 — Paid Content Cache: thin hook over the IPC pair
// `paid_cache_get_size` / `paid_cache_clear`. Mirrors the shape of
// useCookies' getCacheSize/clearCache so CachePanel can drop these in
// next to the existing Cache + Cookies cards.
//
// Phase 8c stage 3 batch 6 (2026-09-13): both calls go through the native, per-request-id
// bridge. This was the LAST per-call `window.on*` pair in the tree (contract P8c-A7 RED:
// 3 concurrent `paid_cache_get_size` under the old slot → 2 of 3 rejected at 3 s).
const native = () => {
  const b = window.hodosBrowser?.bridge;
  if (!b) throw new Error('paidCache: native bridge unavailable');
  return b;
};

export function usePaidCache() {
  const [totalBytes, setTotalBytes] = useState<number>(0);
  const [enabled, setEnabled] = useState<boolean>(true);
  const [loading, setLoading] = useState<boolean>(true);

  const refresh = useCallback(async (): Promise<PaidCacheSizeResponse> => {
    const data = await native().paidCacheGetSize();
    setTotalBytes(data.totalBytes);
    setEnabled(data.enabled);
    setLoading(false);
    return data;
  }, []);

  const clear = useCallback(async (): Promise<PaidCacheClearResponse> => {
    const data = await native().paidCacheClear();
    setTotalBytes(data.totalBytes);
    return data;
  }, []);

  useEffect(() => {
    refresh().catch(() => setLoading(false));
  }, [refresh]);

  return { totalBytes, enabled, loading, refresh, clear };
}
