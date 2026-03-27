/**
 * Bridge mock for Playwright E2E tests.
 *
 * In CEF, `window.cefMessage` and `window.hodosBrowser` are injected by C++ V8.
 * In Playwright (regular Chromium), we must mock them so the React app renders
 * without throwing.
 *
 * This script is injected via page.addInitScript() BEFORE the app JS runs.
 */

export const BRIDGE_MOCK_SCRIPT = `
(function () {
  // ── cefMessage mock ───────────────────────────────────────────────
  if (!window.cefMessage) {
    window.cefMessage = {
      send: function (name, args) {
        console.log('[mock] cefMessage.send:', name, args);

        // Settings IPC
        if (name === 'settings_get_all') {
          setTimeout(function () {
            if (window.onSettingsResponse) {
              window.onSettingsResponse({
                version: 1,
                browser: {
                  homepage: 'about:blank',
                  searchEngine: 'duckduckgo',
                  zoomLevel: 0.0,
                  showBookmarkBar: false,
                  downloadsPath: '/tmp/downloads',
                  restoreSessionOnStart: false,
                  askWhereToSave: true,
                },
                privacy: {
                  adBlockEnabled: true,
                  thirdPartyCookieBlocking: true,
                  doNotTrack: false,
                  clearDataOnExit: false,
                  fingerprintProtection: true,
                },
                wallet: {
                  autoApproveEnabled: true,
                  defaultPerTxLimitCents: 10,
                  defaultPerSessionLimitCents: 300,
                  defaultRateLimitPerMin: 10,
                },
              });
            }
          }, 50);
        }

        // Settings set (no-op but acknowledge)
        if (name === 'settings_set') {
          console.log('[mock] settings_set:', args);
        }

        // Cookie blocking state
        if (name === 'cookie_blocking_get_state') {
          setTimeout(function () {
            if (window.onCookieBlockingStateResponse) {
              window.onCookieBlockingStateResponse({ enabled: true, thirdPartyOnly: true });
            }
          }, 50);
        }

        // Cookie get all
        if (name === 'cookie_get_all') {
          setTimeout(function () {
            if (window.onCookieGetAllResponse) {
              window.onCookieGetAllResponse([]);
            }
          }, 50);
        }

        // Cookie blocklist
        if (name === 'cookie_get_blocklist') {
          setTimeout(function () {
            if (window.onCookieBlocklistResponse) {
              window.onCookieBlocklistResponse([]);
            }
          }, 50);
        }

        // Cookie block log
        if (name === 'cookie_get_block_log') {
          setTimeout(function () {
            if (window.onCookieBlockLogResponse) {
              window.onCookieBlockLogResponse([]);
            }
          }, 50);
        }

        // Cookie blocked count
        if (name === 'cookie_get_blocked_count') {
          setTimeout(function () {
            if (window.onCookieBlockedCountResponse) {
              window.onCookieBlockedCountResponse({ count: 0 });
            }
          }, 50);
        }

        // Adblock blocked count
        if (name === 'adblock_get_blocked_count') {
          setTimeout(function () {
            if (window.onAdblockBlockedCountResponse) {
              window.onAdblockBlockedCountResponse({ count: 0 });
            }
          }, 50);
        }

        // Adblock site check
        if (name === 'adblock_check_site_enabled') {
          setTimeout(function () {
            if (window.onAdblockCheckSiteEnabledResponse) {
              window.onAdblockCheckSiteEnabledResponse({ domain: '', adblockEnabled: true });
            }
          }, 50);
        }

        // Adblock scriptlet check
        if (name === 'adblock_check_scriptlets_enabled') {
          setTimeout(function () {
            if (window.onAdblockCheckScriptletsEnabledResponse) {
              window.onAdblockCheckScriptletsEnabledResponse({ domain: '', scriptletsEnabled: true });
            }
          }, 50);
        }

        // Cookie site allowed check
        if (name === 'cookie_check_site_allowed') {
          setTimeout(function () {
            if (window.onCookieCheckSiteAllowedResponse) {
              window.onCookieCheckSiteAllowedResponse({ domain: '', allowed: false });
            }
          }, 50);
        }

        // Bookmark operations
        if (name === 'bookmark_get_all') {
          setTimeout(function () {
            if (window.onBookmarkGetAllResponse) {
              window.onBookmarkGetAllResponse({ bookmarks: [], total: 0 });
            }
          }, 50);
        }

        if (name === 'bookmark_is_bookmarked') {
          setTimeout(function () {
            if (window.onBookmarkIsBookmarkedResponse) {
              window.onBookmarkIsBookmarkedResponse({ bookmarked: false });
            }
          }, 50);
        }

        // Download state
        if (name === 'download_get_state') {
          setTimeout(function () {
            window.postMessage({ type: 'download_state_update', data: JSON.stringify([]) }, '*');
          }, 50);
        }

        // Tab list
        if (name === 'get_tab_list') {
          setTimeout(function () {
            window.postMessage({
              type: 'tab_list_response',
              data: JSON.stringify({
                tabs: [{ id: 1, title: 'New Tab', url: 'http://localhost:5137/newtab', isActive: true, isLoading: false }],
                activeTabId: 1,
              }),
            }, '*');
          }, 50);
        }

        // Profile operations
        if (name === 'profiles_get_all') {
          setTimeout(function () {
            if (window.onProfilesGetAllResponse) {
              window.onProfilesGetAllResponse(JSON.stringify({
                profiles: [
                  { id: 'default', name: 'Default', color: '#a67c00', isDefault: true }
                ],
                currentProfileId: 'default',
              }));
            }
          }, 50);
        }

        // Wallet status check (via IPC path)
        if (name === 'wallet_status_check') {
          setTimeout(function () {
            if (window.onWalletStatusResponse) {
              window.onWalletStatusResponse({ exists: true, needsBackup: false });
            }
          }, 50);
        }

        // Wallet balance (via IPC path)
        if (name === 'get_balance') {
          setTimeout(function () {
            if (window.onGetBalanceResponse) {
              window.onGetBalanceResponse({ balance: 5432100 });
            }
          }, 50);
        }

        // Address generate
        if (name === 'address_generate') {
          setTimeout(function () {
            if (window.onAddressGenerated) {
              window.onAddressGenerated({
                address: '1MockAddressForTesting123456789',
                publicKey: '02c5b4b8e7a9mockpublickey',
                privateKey: 'mock_private_key',
                index: 0,
              });
            }
          }, 50);
        }

        // Fingerprint check site
        if (name === 'fingerprint_check_site_enabled') {
          setTimeout(function () {
            if (window.onFingerprintCheckSiteEnabledResponse) {
              window.onFingerprintCheckSiteEnabledResponse({ domain: '', enabled: true });
            }
          }, 50);
        }

        // Most visited response for NewTabPage
        if (name === 'get_most_visited') {
          setTimeout(function () {
            window.postMessage({
              type: 'most_visited_response',
              data: JSON.stringify([]),
            }, '*');
          }, 50);
        }

        // Google suggest
        if (name === 'google_suggest') {
          // no-op, suggestions will just be empty
        }

        // Omnibox operations
        if (name === 'omnibox_create' || name === 'omnibox_show' || name === 'omnibox_hide' || name === 'omnibox_create_or_show') {
          // no-op
        }
      },
    };
  }

  // ── allSystemsReady flag ──────────────────────────────────────────
  window.allSystemsReady = true;

  // ── window.hodosBrowser mock ──────────────────────────────────────
  if (!window.hodosBrowser) {
    window.hodosBrowser = {};
  }

  // History (synchronous API)
  if (!window.hodosBrowser.history) {
    window.hodosBrowser.history = {
      get: function () { return []; },
      search: function () { return []; },
      searchWithFrecency: function () { return []; },
      delete: function () { return true; },
      clearAll: function () { return true; },
      clearRange: function () { return true; },
    };
  }

  // Navigation
  if (!window.hodosBrowser.navigation) {
    window.hodosBrowser.navigation = {
      navigate: function (url) { console.log('[mock] navigate:', url); },
    };
  }

  // Overlay
  if (!window.hodosBrowser.overlay) {
    window.hodosBrowser.overlay = {
      show: function () { console.log('[mock] overlay show'); },
      hide: function () { console.log('[mock] overlay hide'); },
      toggleInput: function (enable) { console.log('[mock] overlay toggleInput:', enable); },
      close: function () { console.log('[mock] overlay close'); },
    };
  }

  // Overlay panel
  if (!window.hodosBrowser.overlayPanel) {
    window.hodosBrowser.overlayPanel = {
      open: function (name) { console.log('[mock] overlayPanel open:', name); },
      toggleInput: function (enable) { console.log('[mock] overlayPanel toggleInput:', enable); },
    };
  }

  // Address
  if (!window.hodosBrowser.address) {
    window.hodosBrowser.address = {
      generate: function () {
        return Promise.resolve({
          address: '1MockAddressForTesting123456789',
          publicKey: '02c5b4b8e7a9mockpublickey',
          privateKey: 'mock_private_key',
          index: 0,
        });
      },
    };
  }

  // Wallet
  if (!window.hodosBrowser.wallet) {
    window.hodosBrowser.wallet = {
      getStatus: function () {
        return Promise.resolve({ exists: true, needsBackup: false });
      },
      create: function () {
        return Promise.resolve({
          success: true,
          mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
          address: '1MockAddressForTesting123456789',
          version: '1.0.0',
        });
      },
      load: function () {
        return Promise.resolve({
          success: true,
          address: '1MockAddressForTesting123456789',
          mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
          version: '1.0.0',
          backedUp: true,
        });
      },
      getInfo: function () {
        return Promise.resolve({
          version: '1.0.0',
          mnemonic: 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
          address: '1MockAddressForTesting123456789',
          backedUp: true,
        });
      },
      generateAddress: function () {
        return Promise.resolve({
          address: '1MockAddressForTesting123456789',
          publicKey: '02c5b4b8e7a9mockpublickey',
          privateKey: 'mock_private_key',
          index: 0,
        });
      },
      getCurrentAddress: function () {
        return Promise.resolve({
          address: '1MockAddressForTesting123456789',
          publicKey: '02c5b4b8e7a9mockpublickey',
          privateKey: 'mock_private_key',
          index: 0,
        });
      },
      getAddresses: function () {
        return Promise.resolve([{
          address: '1MockAddressForTesting123456789',
          publicKey: '02c5b4b8e7a9mockpublickey',
          privateKey: 'mock_private_key',
          index: 0,
        }]);
      },
      markBackedUp: function () {
        return Promise.resolve({ success: true });
      },
      getBackupModalState: function () {
        return Promise.resolve({ shown: false });
      },
      setBackupModalState: function () {
        return Promise.resolve({ success: true });
      },
      getBalance: function () {
        return Promise.resolve({ balance: 5432100, bsvPrice: 52.47 });
      },
      sendTransaction: function () {
        return Promise.resolve({ txid: 'mock_txid_abc123', success: true });
      },
      getTransactionHistory: function () {
        return Promise.resolve([]);
      },
    };
  }

  // Cookies
  if (!window.hodosBrowser.cookies) {
    window.hodosBrowser.cookies = {
      getAll: function () { return Promise.resolve([]); },
      deleteCookie: function () { return Promise.resolve({ success: true }); },
      deleteDomainCookies: function () { return Promise.resolve({ success: true }); },
      deleteAllCookies: function () { return Promise.resolve({ success: true }); },
      clearCache: function () { return Promise.resolve({ success: true }); },
      getCacheSize: function () { return Promise.resolve({ size: 0, formatted: '0 B' }); },
    };
  }

  // Cookie blocking
  if (!window.hodosBrowser.cookieBlocking) {
    window.hodosBrowser.cookieBlocking = {
      blockDomain: function () { return Promise.resolve({ success: true }); },
      unblockDomain: function () { return Promise.resolve({ success: true }); },
      getBlockList: function () { return Promise.resolve([]); },
      allowThirdParty: function () { return Promise.resolve({ success: true }); },
      removeThirdPartyAllow: function () { return Promise.resolve({ success: true }); },
      getBlockLog: function () { return Promise.resolve([]); },
      clearBlockLog: function () { return Promise.resolve({ success: true }); },
      getBlockedCount: function () { return Promise.resolve({ count: 0 }); },
      resetBlockedCount: function () { return Promise.resolve(); },
    };
  }

  // Bookmarks
  if (!window.hodosBrowser.bookmarks) {
    window.hodosBrowser.bookmarks = {
      add: function () { return Promise.resolve({ id: 1, success: true }); },
      get: function () { return Promise.resolve({ id: 1, url: '', title: '', folder_id: null, favicon_url: '', position: 0, created_at: 0, updated_at: 0, last_accessed: 0, tags: [] }); },
      update: function () { return Promise.resolve({ success: true }); },
      remove: function () { return Promise.resolve({ success: true }); },
      search: function () { return Promise.resolve({ bookmarks: [], total: 0 }); },
      getAll: function () { return Promise.resolve({ bookmarks: [], total: 0 }); },
      isBookmarked: function () { return Promise.resolve({ bookmarked: false }); },
      getAllTags: function () { return Promise.resolve([]); },
      updateLastAccessed: function () { return Promise.resolve({ success: true }); },
      folders: {
        create: function () { return Promise.resolve({ id: 1, success: true }); },
        list: function () { return Promise.resolve({ folders: [] }); },
        update: function () { return Promise.resolve({ success: true }); },
        remove: function () { return Promise.resolve({ success: true }); },
        getTree: function () { return Promise.resolve([]); },
      },
    };
  }

  // Omnibox
  if (!window.hodosBrowser.omnibox) {
    window.hodosBrowser.omnibox = {
      show: function () { console.log('[mock] omnibox show'); },
      hide: function () { console.log('[mock] omnibox hide'); },
      createOrShow: function () { console.log('[mock] omnibox createOrShow'); },
      getSuggestions: function () { return Promise.resolve([]); },
    };
  }

  // Google Suggest
  if (!window.hodosBrowser.googleSuggest) {
    window.hodosBrowser.googleSuggest = {
      fetch: function () { return 0; },
    };
  }

  // Identity (used by useHodosBrowser)
  if (!window.hodosBrowser.identity) {
    window.hodosBrowser.identity = {
      get: function () {
        return Promise.resolve({
          publicKey: '02c5b4b8e7a9mockpublickey',
          privateKey: 'mock_private_key',
          address: '1MockAddressForTesting123456789',
          backedUp: true,
        });
      },
      markBackedUp: function () {
        return Promise.resolve('success');
      },
    };
  }

  // BRC-100 (used by brc100 bridge)
  if (!window.hodosBrowser.brc100) {
    window.hodosBrowser.brc100 = {
      status: function () {
        return Promise.resolve({ available: true, version: '1.0.0', features: [] });
      },
      isAvailable: function () { return Promise.resolve(true); },
      generateIdentity: function () { return Promise.resolve(null); },
      validateIdentity: function () { return Promise.resolve(true); },
      selectiveDisclosure: function () { return Promise.resolve(null); },
      generateChallenge: function () { return Promise.resolve(null); },
      authenticate: function () { return Promise.resolve({ success: true }); },
      deriveType42Keys: function () { return Promise.resolve(null); },
      createSession: function () { return Promise.resolve(null); },
      validateSession: function () { return Promise.resolve(true); },
      revokeSession: function () { return Promise.resolve(true); },
      createBEEF: function () { return Promise.resolve(null); },
      verifyBEEF: function () { return Promise.resolve(true); },
      broadcastBEEF: function () { return Promise.resolve(null); },
      verifySPV: function () { return Promise.resolve({ valid: true }); },
      createSPVProof: function () { return Promise.resolve(null); },
    };
  }

  // Seed localStorage with wallet exists flag so WalletPanelPage shows the live wallet
  localStorage.setItem('hodos_wallet_exists', 'true');
  localStorage.setItem('hodos_identity_key', '02c5b4b8e7a9f4d6c2e8b3a7mock_identity_key_for_testing');

  console.log('[e2e] Bridge mock injected successfully');
})();
`;
