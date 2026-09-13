import { useState, useCallback, useEffect } from 'react';
import { useAdblock } from './useAdblock';
import { useCookieBlocking } from './useCookieBlocking';

// Phase 8c stage 3 batch 5 (2026-09-13): the two reads this hook owned
// (`cookie_check_site_allowed`, `fingerprint_get_site_enabled`) go through the native,
// per-request-id bridge. Their old `window.on*` slots neither resolved nor rejected on
// timeout — they just dropped the handler, and `checkCookieSiteAllowed` carried an
// in-flight dedupe (`checkPendingRef`) because the single slot could not tell two callers
// apart. Both are gone: each call owns its promise now.
//
// `fingerprint_set_site_enabled` is unchanged — it is fire-and-forget with no reply, so
// there is nothing to route.
const native = () => {
  const b = window.hodosBrowser?.bridge;
  if (!b) throw new Error('privacyShield: native bridge unavailable');
  return b;
};

// `refreshKey` lets a keep-alive overlay force a re-fetch of per-site shield state
// every time it's shown (not just when the domain changes). Without it, reopening an
// overlay for the SAME domain shows stale toggle state cached on first load — so a
// change made in one overlay (e.g. the site-info hub) wouldn't appear in another
// (e.g. the right Shield panel) until the domain changed.
export const usePrivacyShield = (domain: string, refreshKey: number = 0) => {
  const adblock = useAdblock();
  const cookie = useCookieBlocking();

  // Whether third-party cookies are allowed (i.e. cookie blocking is bypassed) for this domain
  const [cookieSiteAllowed, setCookieSiteAllowed] = useState<boolean>(false);

  // Per-site fingerprint protection state
  const [fingerprintSiteEnabled, setFingerprintSiteEnabledState] = useState(true);
  const [fingerprintNeedsReload, setFingerprintNeedsReload] = useState(false);

  // Cookie blocking is "enabled" when the site is NOT in the allow list
  const cookieBlockingEnabled = !cookieSiteAllowed;

  const checkCookieSiteAllowed = useCallback(async (d: string): Promise<boolean> => {
    if (!d) return false;
    const data = await native().cookieCheckSiteAllowed(d);
    setCookieSiteAllowed(data.allowed);
    return data.allowed;
  }, []);

  // Check on mount, when domain changes, and whenever refreshKey bumps (re-show).
  // Mount-time reads have no caller to reject to; a failure leaves the previous state.
  useEffect(() => {
    if (domain) {
      checkCookieSiteAllowed(domain).catch(() => {});
      adblock.checkSiteAdblock(domain).catch(() => {});
      adblock.checkScriptlets(domain).catch(() => {});
    }
  }, [domain, refreshKey, checkCookieSiteAllowed, adblock.checkSiteAdblock, adblock.checkScriptlets]);

  // Fetch per-site fingerprint enabled state when domain changes. The reply is keyed by
  // request id now, but the effect can still be superseded by a later domain before its
  // reply lands — `alive` plus the domain check keep a stale reply from being applied.
  useEffect(() => {
    setFingerprintNeedsReload(false);
    if (!domain) return;
    let alive = true;
    native().fingerprintGetSiteEnabled(domain).then((data) => {
      if (alive && data.domain === domain) {
        setFingerprintSiteEnabledState(data.enabled);
      }
    }).catch(() => {});
    return () => { alive = false; };
  }, [domain, refreshKey]);

  // Toggle cookie blocking for site
  const toggleCookieBlocking = useCallback(async (d: string, enable: boolean) => {
    if (enable) {
      // Enable blocking = remove from allow list
      await cookie.removeThirdPartyAllow(d);
      setCookieSiteAllowed(false);
    } else {
      // Disable blocking = add to allow list
      await cookie.allowThirdParty(d);
      setCookieSiteAllowed(true);
    }
  }, [cookie.allowThirdParty, cookie.removeThirdPartyAllow]);

  // Toggle per-site fingerprint protection (fire-and-forget IPC; no reply exists)
  const toggleFingerprintSite = useCallback((d: string, enabled: boolean) => {
    window.cefMessage?.send('fingerprint_set_site_enabled', [d, enabled.toString()]);
    setFingerprintSiteEnabledState(enabled);
    setFingerprintNeedsReload(true);
  }, []);

  // Master toggle: both enabled or both disabled
  const masterEnabled = adblock.adblockEnabled && cookieBlockingEnabled;

  const toggleMaster = useCallback(async (d: string, enable: boolean) => {
    // Toggle adblock
    await adblock.toggleSiteAdblock(d, enable);
    // Toggle scriptlets with adblock
    await adblock.toggleScriptlets(d, enable);
    // Toggle cookie blocking
    await toggleCookieBlocking(d, enable);
  }, [adblock.toggleSiteAdblock, adblock.toggleScriptlets, toggleCookieBlocking]);

  const totalBlockedCount = adblock.blockedCount + cookie.blockedCount;

  return {
    // Combined
    masterEnabled,
    toggleMaster,
    totalBlockedCount,

    // Adblock
    adblockEnabled: adblock.adblockEnabled,
    adblockBlockedCount: adblock.blockedCount,
    toggleSiteAdblock: adblock.toggleSiteAdblock,

    // Scriptlets (Sprint 10c)
    scriptletsEnabled: adblock.scriptletsEnabled,
    toggleScriptlets: adblock.toggleScriptlets,

    // Cookie blocking
    cookieBlockingEnabled,
    cookieBlockedCount: cookie.blockedCount,
    toggleCookieBlocking,

    // Fingerprint protection (per-site)
    fingerprintSiteEnabled,
    toggleFingerprintSite,
    fingerprintNeedsReload,

    // Cookie panel data (for expandable sections)
    blockedDomains: cookie.blockedDomains,
    blockLog: cookie.blockLog,
    fetchBlockList: cookie.fetchBlockList,
    fetchBlockLog: cookie.fetchBlockLog,
    clearBlockLog: cookie.clearBlockLog,
    blockDomain: cookie.blockDomain,
    unblockDomain: cookie.unblockDomain,
  };
};
