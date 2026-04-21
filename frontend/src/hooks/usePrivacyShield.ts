import { useState, useCallback, useEffect, useRef } from 'react';
import { useAdblock } from './useAdblock';
import { useCookieBlocking } from './useCookieBlocking';

declare global {
  interface Window {
    onCookieCheckSiteAllowedResponse?: (data: { domain: string; allowed: boolean }) => void;
    onFingerprintSiteEnabledResponse?: (data: { domain: string; enabled: boolean }) => void;
  }
}

export const usePrivacyShield = (domain: string) => {
  const adblock = useAdblock();
  const cookie = useCookieBlocking();

  // Whether third-party cookies are allowed (i.e. cookie blocking is bypassed) for this domain
  const [cookieSiteAllowed, setCookieSiteAllowed] = useState<boolean>(false);
  const checkPendingRef = useRef(false);

  // Per-site fingerprint protection state
  const [fingerprintSiteEnabled, setFingerprintSiteEnabledState] = useState(true);
  const [fingerprintNeedsReload, setFingerprintNeedsReload] = useState(false);

  // Cookie blocking is "enabled" when the site is NOT in the allow list
  const cookieBlockingEnabled = !cookieSiteAllowed;

  // Check cookie site allowed status via IPC
  const checkCookieSiteAllowed = useCallback((d: string) => {
    if (!d || checkPendingRef.current) return;
    checkPendingRef.current = true;

    const timeout = setTimeout(() => {
      checkPendingRef.current = false;
      delete window.onCookieCheckSiteAllowedResponse;
    }, 3000);

    window.onCookieCheckSiteAllowedResponse = (data) => {
      clearTimeout(timeout);
      checkPendingRef.current = false;
      setCookieSiteAllowed(data.allowed);
      delete window.onCookieCheckSiteAllowedResponse;
    };

    window.cefMessage?.send('cookie_check_site_allowed', [d]);
  }, []);

  // Check on mount and when domain changes
  useEffect(() => {
    if (domain) {
      checkCookieSiteAllowed(domain);
      adblock.checkSiteAdblock(domain);
      adblock.checkScriptlets(domain);
    }
  }, [domain, checkCookieSiteAllowed, adblock.checkSiteAdblock, adblock.checkScriptlets]);

  // Fetch per-site fingerprint enabled state when domain changes
  useEffect(() => {
    setFingerprintNeedsReload(false);

    const timeout = setTimeout(() => {
      delete window.onFingerprintSiteEnabledResponse;
    }, 3000);

    window.onFingerprintSiteEnabledResponse = (data) => {
      clearTimeout(timeout);
      if (data.domain === domain) {
        setFingerprintSiteEnabledState(data.enabled);
      }
      delete window.onFingerprintSiteEnabledResponse;
    };

    window.cefMessage?.send('fingerprint_get_site_enabled', [domain]);

    return () => {
      clearTimeout(timeout);
      delete window.onFingerprintSiteEnabledResponse;
    };
  }, [domain]);

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

  // Toggle per-site fingerprint protection
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
