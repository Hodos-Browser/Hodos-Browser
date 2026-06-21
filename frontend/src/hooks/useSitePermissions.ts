import { useState, useEffect, useCallback } from 'react';

// Web-content (OS-capability) permissions managed in the site-info hub. Mirrors the
// C++ SitePermissionStore tri-state. Codes must match kSitePermCaps in
// simple_handler.cpp.
export type SitePermState = 'ask' | 'allow' | 'block';
export interface SitePermission { code: string; state: SitePermState; }

declare global {
  interface Window {
    onSitePermissionsResponse?: (data: { host: string; permissions: SitePermission[] }) => void;
  }
}

// `refreshKey` re-fetches on each overlay re-show (keep-alive overlay would otherwise
// show stale state — same pattern as usePrivacyShield).
export const useSitePermissions = (host: string, refreshKey: number = 0) => {
  const [permissions, setPermissions] = useState<SitePermission[]>([]);

  // Persistent response handler — C++ re-emits the full authoritative list after
  // every get/set/reset, so the UI stays in sync with the store.
  useEffect(() => {
    window.onSitePermissionsResponse = (data) => {
      if (data && Array.isArray(data.permissions)) {
        setPermissions(data.permissions);
      }
    };
    return () => { delete window.onSitePermissionsResponse; };
  }, []);

  // (Re)fetch on host change / re-show.
  useEffect(() => {
    if (host) {
      window.cefMessage?.send('site_permissions_get', [host]);
    } else {
      setPermissions([]);
    }
  }, [host, refreshKey]);

  const setPermission = useCallback((code: string, state: SitePermState) => {
    if (!host) return;
    // Optimistic update; C++ re-emits the authoritative list to confirm.
    setPermissions(prev => prev.map(p => (p.code === code ? { ...p, state } : p)));
    window.cefMessage?.send('site_permissions_set', [host, code, state]);
  }, [host]);

  const resetPermissions = useCallback(() => {
    if (!host) return;
    setPermissions(prev => prev.map(p => ({ ...p, state: 'ask' as SitePermState })));
    window.cefMessage?.send('site_permissions_reset', [host]);
  }, [host]);

  return { permissions, setPermission, resetPermissions };
};
