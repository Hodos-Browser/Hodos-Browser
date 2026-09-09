
// Safely define the shell → native message bridge
if (!window.hodosBrowser) window.hodosBrowser = {} as any;

if (!window.hodosBrowser.navigation) {
  window.hodosBrowser.navigation = {
    navigate: (url: string) => {
      if (window.cefMessage?.send) {
        window.cefMessage.send('navigate', [url]);
      } else {
        console.warn('⚠️ cefMessage bridge not available');
      }
    }
  };
}

// Debug: Check what hodosBrowser.overlay looks like
console.log("🔍 Bridge: window.hodosBrowser:", window.hodosBrowser);
console.log("🔍 Bridge: window.hodosBrowser.overlay:", window.hodosBrowser?.overlay);
console.log("🔍 Bridge: typeof overlay:", typeof window.hodosBrowser?.overlay);

// Only set methods if they don't already exist (don't override injected methods)
if (!window.hodosBrowser.overlay?.show) {
  if (!window.hodosBrowser.overlay) {
    (window.hodosBrowser as any).overlay = {};
  }
  // Ensure overlay object exists (defensive for macOS/non-overlay contexts)
  if (!window.hodosBrowser.overlay) {
    window.hodosBrowser.overlay = {} as any;
  }

  window.hodosBrowser.overlay.show = () => {
    console.log("🧠 JS: Sending overlay_show to native");
    console.log("Bridge is executing from URL:", window.location.href);

    // Check if there's a pending BRC-100 auth request
    const pendingAuth = (window as any).pendingBRC100AuthRequest;
    if (pendingAuth) {
      console.log("🔐 Found pending BRC-100 auth request, sending overlay_show_brc100_auth");
      window.cefMessage?.send('overlay_show_brc100_auth', [
        pendingAuth.domain,
        pendingAuth.method,
        pendingAuth.endpoint,
        pendingAuth.body,
        pendingAuth.type || 'domain_approval'
      ]);
      // Clear the pending request
      (window as any).pendingBRC100AuthRequest = null;
    } else {
      console.log("🔐 No pending auth request, sending overlay_show_settings");
      window.cefMessage?.send('overlay_show_settings', []);
    }
  };

  window.hodosBrowser.overlay.close = () => {
    console.log("🧠 JS: Sending overlay_close to native");
    window.cefMessage?.send('overlay_close', []);
  };

} else {
  // Check if this is our injected method (uses chrome.runtime.sendMessage)
  const methodString = window.hodosBrowser.overlay.show.toString();
  if (methodString.includes('chrome.runtime.sendMessage') && methodString.includes('test_overlay')) {
    console.log("🔍 Bridge: overlay.show is our injected method, not overriding");
  } else {
    console.log("🔍 Bridge: overlay.show exists but is not our injected method, not overriding");
  }
}

if (!window.hodosBrowser.overlay?.hide) {
  if (!window.hodosBrowser.overlay) {
    (window.hodosBrowser as any).overlay = {};
  }
  window.hodosBrowser.overlay.hide = () => window.cefMessage?.send?.('overlay_hide', []);
}

if (!window.hodosBrowser.overlay?.toggleInput) {
  if (!window.hodosBrowser.overlay) {
    (window.hodosBrowser as any).overlay = {};
  }
  window.hodosBrowser.overlay.toggleInput = (enable: boolean) =>
    window.cefMessage?.send?.('overlay_input', [enable]);
}

if (!window.hodosBrowser.overlay?.close) {
  if (!window.hodosBrowser.overlay) {
    (window.hodosBrowser as any).overlay = {};
  }
  window.hodosBrowser.overlay.close = () => {
    console.log("🧠 JS: Sending overlay_close to native");
    window.cefMessage?.send?.('overlay_close', []);
  };
}

console.log("🔍 initWindowBridge: Setting up hodosBrowser.address");
console.log("🔍 initWindowBridge: window.hodosBrowser.address exists:", !!window.hodosBrowser.address);

// Ensure address object exists (defensive for macOS where APIs are stubbed)
if (!window.hodosBrowser.address) {
  console.log("⚠️ initWindowBridge: address API not available, creating stub");
  window.hodosBrowser.address = {} as any;
}

// Ensure overlay has all required functions (defensive for macOS)
if (!window.hodosBrowser.overlay?.toggleInput) {
  console.log("⚠️ initWindowBridge: overlay.toggleInput not available, creating stub");
  if (!window.hodosBrowser.overlay) {
    window.hodosBrowser.overlay = {} as any;
  }
  window.hodosBrowser.overlay.toggleInput = (_enable: boolean) => {
    console.log("⚠️ overlay.toggleInput called but not implemented on this platform");
  };
}

if (!window.hodosBrowser.overlay?.hide) {
  window.hodosBrowser.overlay.hide = () => {
    console.log("⚠️ overlay.hide called but not implemented on this platform");
  };
}

// Force override the existing function
console.log("🔍 initWindowBridge: Forcing override of address.generate function");
window.hodosBrowser.address.generate = () => {
  console.log("🔑 JS: Sending address_generate to native");
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      delete window.onAddressGenerated;
      delete window.onAddressError;
      reject(new Error('address_generate timed out'));
    }, 10000);

    // Set up response handlers
    window.onAddressGenerated = (data: any) => {
      clearTimeout(timeout);
      console.log("✅ Address generated:", data);
      resolve(data);
      delete window.onAddressGenerated;
      delete window.onAddressError;
    };

    window.onAddressError = (error: string) => {
      clearTimeout(timeout);
      console.error("❌ Address generation error:", error);
      reject(new Error(error));
      delete window.onAddressGenerated;
      delete window.onAddressError;
    };

    // Send the request
    window.cefMessage?.send('address_generate', []);
  });
};


// P2a: shared in-flight slot for getBalance — see the comment on getBalance below.
// (Phase 8c stage 3 removed `balanceInFlight` — the in-flight dedupe it backed is
// obsolete now that getBalance is routed by request id.)

// Wallet methods
if (!window.hodosBrowser.wallet) {
  window.hodosBrowser.wallet = {
    // ⭐ MIGRATED — Phase 8c stage 1, the first of 41.
    //
    // No `window.onWalletStatusResponse` global, no `setTimeout` race, no `delete` on a
    // slot another in-flight call may own. `hodosBrowser.bridge.getStatus` is a NATIVE
    // function: C++ mints a request id, holds the promise in its own map keyed by that
    // id, and the browser process echoes the id back. Two concurrent calls get two
    // promises and two correct answers.
    //
    // ⚠️ The native binding is on `hodosBrowser.bridge`, not here, on purpose — C++
    // OnContextCreated runs before this file, and creating `hodosBrowser.wallet` there
    // would make the `if (!window.hodosBrowser.wallet)` guard above fail and silently
    // drop the other 40 methods. Each migrated method becomes one assignment like this.
    //
    // ⛔ The timeout is gone deliberately. The old one existed to stop a caller hanging
    // forever when its reply was stolen by another call — the bug itself. A reply that
    // arrives late is now discarded by request id (`TakeBridgeCall`), and a reply that
    // never arrives is a native failure that rejects the promise.
    getStatus: () => {
      if (!window.hodosBrowser?.bridge?.getStatus) {
        return Promise.reject(new Error('wallet.getStatus: native bridge unavailable'));
      }
      return window.hodosBrowser.bridge.getStatus();
    },

    create: () => {
      console.log("🆕 JS: Sending create_wallet to native");
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          delete window.onCreateWalletResponse;
          delete window.onCreateWalletError;
          reject(new Error('create_wallet timed out'));
        }, 10000);

        window.onCreateWalletResponse = (data: any) => {
          clearTimeout(timeout);
          console.log("✅ Wallet created:", data);
          resolve(data);
          delete window.onCreateWalletResponse;
          delete window.onCreateWalletError;
        };

        window.onCreateWalletError = (error: string) => {
          clearTimeout(timeout);
          console.error("❌ Wallet creation error:", error);
          reject(new Error(error));
          delete window.onCreateWalletResponse;
          delete window.onCreateWalletError;
        };

        window.cefMessage?.send('create_wallet', []);
      });
    },

    load: () => {
      console.log("📂 JS: Sending load_wallet to native");
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          delete window.onLoadWalletResponse;
          delete window.onLoadWalletError;
          reject(new Error('load_wallet timed out'));
        }, 10000);

        window.onLoadWalletResponse = (data: any) => {
          clearTimeout(timeout);
          console.log("✅ Wallet loaded:", data);
          resolve(data);
          delete window.onLoadWalletResponse;
          delete window.onLoadWalletError;
        };

        window.onLoadWalletError = (error: string) => {
          clearTimeout(timeout);
          console.error("❌ Wallet load error:", error);
          reject(new Error(error));
          delete window.onLoadWalletResponse;
          delete window.onLoadWalletError;
        };

        window.cefMessage?.send('load_wallet', []);
      });
    },

    getInfo: () => {
      console.log("🔍 JS: Sending get_wallet_info to native");
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          delete window.onGetWalletInfoResponse;
          delete window.onGetWalletInfoError;
          reject(new Error('get_wallet_info timed out'));
        }, 10000);

        window.onGetWalletInfoResponse = (data: any) => {
          clearTimeout(timeout);
          console.log("✅ Wallet info retrieved:", data);
          resolve(data);
          delete window.onGetWalletInfoResponse;
          delete window.onGetWalletInfoError;
        };

        window.onGetWalletInfoError = (error: string) => {
          clearTimeout(timeout);
          console.error("❌ Wallet info error:", error);
          reject(new Error(error));
          delete window.onGetWalletInfoResponse;
          delete window.onGetWalletInfoError;
        };

        window.cefMessage?.send('get_wallet_info', []);
      });
    },

    generateAddress: () => {
      console.log("📍 JS: Sending wallet address generation to native");
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          delete window.onAddressGenerated;
          delete window.onAddressError;
          reject(new Error('address_generate timed out'));
        }, 10000);

        window.onAddressGenerated = (data: any) => {
          clearTimeout(timeout);
          console.log("✅ Address generated:", data);
          resolve(data);
          delete window.onAddressGenerated;
          delete window.onAddressError;
        };

        window.onAddressError = (error: string) => {
          clearTimeout(timeout);
          console.error("❌ Address generation error:", error);
          reject(new Error(error));
          delete window.onAddressGenerated;
          delete window.onAddressError;
        };

        window.cefMessage?.send('address_generate', []);
      });
    },

    getCurrentAddress: () => {
      console.log("📍 JS: Sending get_current_address to native");
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          delete window.onGetCurrentAddressResponse;
          delete window.onGetCurrentAddressError;
          reject(new Error('get_current_address timed out'));
        }, 10000);

        window.onGetCurrentAddressResponse = (data: any) => {
          clearTimeout(timeout);
          console.log("✅ Current address retrieved:", data);
          resolve(data);
          delete window.onGetCurrentAddressResponse;
          delete window.onGetCurrentAddressError;
        };

        window.onGetCurrentAddressError = (error: string) => {
          clearTimeout(timeout);
          console.error("❌ Current address error:", error);
          reject(new Error(error));
          delete window.onGetCurrentAddressResponse;
          delete window.onGetCurrentAddressError;
        };

        window.cefMessage?.send('get_current_address', []);
      });
    },

    getAddresses: () => {
      console.log("📍 JS: Sending get_addresses to native");
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          delete window.onGetAddressesResponse;
          delete window.onGetAddressesError;
          reject(new Error('get_addresses timed out'));
        }, 10000);

        window.onGetAddressesResponse = (data: any) => {
          clearTimeout(timeout);
          console.log("✅ All addresses retrieved:", data);
          if (data.success) {
            resolve(data.addresses);
          } else {
            reject(new Error(data.error || "Failed to get addresses"));
          }
          delete window.onGetAddressesResponse;
          delete window.onGetAddressesError;
        };

        window.onGetAddressesError = (error: string) => {
          clearTimeout(timeout);
          console.error("❌ Get addresses error:", error);
          reject(new Error(error));
          delete window.onGetAddressesResponse;
          delete window.onGetAddressesError;
        };

        window.cefMessage?.send('get_addresses', []);
      });
    },

    markBackedUp: () => {
      console.log("✅ JS: Sending mark_wallet_backed_up to native");
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          delete window.onMarkWalletBackedUpResponse;
          delete window.onMarkWalletBackedUpError;
          reject(new Error('mark_wallet_backed_up timed out'));
        }, 10000);

        window.onMarkWalletBackedUpResponse = (data: any) => {
          clearTimeout(timeout);
          console.log("✅ Wallet marked as backed up:", data);
          resolve(data);
          delete window.onMarkWalletBackedUpResponse;
          delete window.onMarkWalletBackedUpError;
        };

        window.onMarkWalletBackedUpError = (error: string) => {
          clearTimeout(timeout);
          console.error("❌ Mark backed up error:", error);
          reject(new Error(error));
          delete window.onMarkWalletBackedUpResponse;
          delete window.onMarkWalletBackedUpError;
        };

        window.cefMessage?.send('mark_wallet_backed_up', []);
      });
    },

    // ⭐ MIGRATED — Phase 8c stage 3.
    //
    // 🚨 The legacy version called `resolve(null)` on timeout, NOT reject — so when a
    // concurrent caller stole its reply, this returned a silently wrong value with nothing
    // to catch. Measured: 2 of 3 concurrent callers got  (D-5). Now each caller has
    // its own promise, and a genuine failure rejects.
    getBackupModalState: () => {
      if (!window.hodosBrowser?.bridge?.getBackupModalState) {
        return Promise.reject(new Error('wallet.getBackupModalState: native bridge unavailable'));
      }
      return window.hodosBrowser.bridge.getBackupModalState();
    },

    // ⭐ MIGRATED — Phase 8c stage 3. First migrated method with a NON-STRING payload:
    // the boolean rides as arg 1 via `SetBool`, not stringified.
    setBackupModalState: (shown: boolean) => {
      if (!window.hodosBrowser?.bridge?.setBackupModalState) {
        return Promise.reject(new Error('wallet.setBackupModalState: native bridge unavailable'));
      }
      return window.hodosBrowser.bridge.setBackupModalState(shown);
    },

    // ⭐ MIGRATED — Phase 8c stage 3. This one RETIRES A WORKAROUND.
    //
    // P2a added an in-flight dedupe here because a single global callback slot could not
    // tell two concurrent callers apart — the measured 1-in-3 failure. Per-request routing
    // removes the need: two concurrent reads now get two correct answers instead of
    // sharing one, and the `balanceInFlight` module variable is gone with it.
    //
    // ⛔ The dedupe was only ever sound because a balance read is idempotent. It must
    // never be copied to a write — see sendTransaction above.
    getBalance: () => {
      if (!window.hodosBrowser?.bridge?.getBalance) {
        return Promise.reject(new Error('wallet.getBalance: native bridge unavailable'));
      }
      return window.hodosBrowser.bridge.getBalance();
    },

    // ⭐ MIGRATED — Phase 8c stage 2. The money path.
    //
    // 🚨 Two concurrent sends are TWO PAYMENTS. Under the old single-slot callback the
    // second caller's reply could resolve the first caller's promise — and the loser then
    // sat until its 10 s timeout. Per-request routing makes each send its own promise.
    //
    // ⛔ NEVER apply `getBalance`'s in-flight dedupe here. Deduping a read is fine;
    // deduping a send would silently collapse two distinct payments into one, which is
    // far worse than the race being fixed. That is the trap `P8c-A2` exists to catch.
    //
    // The payload stays a JSON string — same bytes the legacy path put on the wire, so
    // this is a routing change and nothing else.
    sendTransaction: (data: any) => {
      if (!window.hodosBrowser?.bridge?.sendTransaction) {
        return Promise.reject(new Error('wallet.sendTransaction: native bridge unavailable'));
      }
      return window.hodosBrowser.bridge.sendTransaction(JSON.stringify(data));
    },

    getTransactionHistory: () => {
      console.log("📜 JS: Sending get_transaction_history to native");
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          delete window.onGetTransactionHistoryResponse;
          delete window.onGetTransactionHistoryError;
          reject(new Error('get_transaction_history timed out'));
        }, 10000);

        window.onGetTransactionHistoryResponse = (data: any) => {
          clearTimeout(timeout);
          console.log("✅ Transaction history retrieved:", data);
          resolve(data);
          delete window.onGetTransactionHistoryResponse;
          delete window.onGetTransactionHistoryError;
        };

        window.onGetTransactionHistoryError = (error: string) => {
          clearTimeout(timeout);
          console.error("❌ Transaction history error:", error);
          reject(new Error(error));
          delete window.onGetTransactionHistoryResponse;
          delete window.onGetTransactionHistoryError;
        };

        window.cefMessage?.send('get_transaction_history', []);
      });
    }
  };
}


// overlayPanel methods removed - now using process-per-overlay architecture

// Omnibox API for address bar overlay control
if (!(window.hodosBrowser as any).omnibox) {
  (window.hodosBrowser as any).omnibox = {
    // Show overlay with current query
    show: (query: string) => {
      window.cefMessage?.send('omnibox_show', [query]);
    },

    // Hide overlay
    hide: () => {
      window.cefMessage?.send('omnibox_hide', []);
    },

    // Create or show overlay (preemptive)
    createOrShow: () => {
      window.cefMessage?.send('omnibox_create_or_show', []);
    },

    // Placeholder for future suggestion provider (Phase 2)
    // Will be implemented when suggestion pipeline is added
    getSuggestions: async (_query: string): Promise<any[]> => {
      // TODO: Phase 2 - query history and Google suggestions
      return [];
    },
  };
}

// Cookie Management API
if (!(window.hodosBrowser as any).cookies) {
  (window.hodosBrowser as any).cookies = {
    getAll: () => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          resolve([]);
          delete window.onCookieGetAllResponse;
          delete window.onCookieGetAllError;
        }, 5000);

        window.onCookieGetAllResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onCookieGetAllResponse;
          delete window.onCookieGetAllError;
        };
        window.onCookieGetAllError = (error: string) => {
          clearTimeout(timeout);
          reject(new Error(error));
          delete window.onCookieGetAllResponse;
          delete window.onCookieGetAllError;
        };
        window.cefMessage?.send('cookie_get_all', []);
      });
    },

    deleteCookie: (url: string, name: string) => {
      return new Promise((resolve, reject) => {
        window.onCookieDeleteResponse = (data: any) => {
          resolve(data);
          delete window.onCookieDeleteResponse;
          delete window.onCookieDeleteError;
        };
        window.onCookieDeleteError = (error: string) => {
          reject(new Error(error));
          delete window.onCookieDeleteResponse;
          delete window.onCookieDeleteError;
        };
        window.cefMessage?.send('cookie_delete', [url, name]);
      });
    },

    deleteDomainCookies: (domain: string) => {
      return new Promise((resolve, reject) => {
        window.onCookieDeleteDomainResponse = (data: any) => {
          resolve(data);
          delete window.onCookieDeleteDomainResponse;
          delete window.onCookieDeleteDomainError;
        };
        window.onCookieDeleteDomainError = (error: string) => {
          reject(new Error(error));
          delete window.onCookieDeleteDomainResponse;
          delete window.onCookieDeleteDomainError;
        };
        window.cefMessage?.send('cookie_delete_domain', [domain]);
      });
    },

    deleteAllCookies: () => {
      return new Promise((resolve, reject) => {
        window.onCookieDeleteAllResponse = (data: any) => {
          resolve(data);
          delete window.onCookieDeleteAllResponse;
          delete window.onCookieDeleteAllError;
        };
        window.onCookieDeleteAllError = (error: string) => {
          reject(new Error(error));
          delete window.onCookieDeleteAllResponse;
          delete window.onCookieDeleteAllError;
        };
        window.cefMessage?.send('cookie_delete_all', []);
      });
    },

    clearCache: () => {
      return new Promise((resolve, reject) => {
        window.onCacheClearResponse = (data: any) => {
          resolve(data);
          delete window.onCacheClearResponse;
          delete window.onCacheClearError;
        };
        window.onCacheClearError = (error: string) => {
          reject(new Error(error));
          delete window.onCacheClearResponse;
          delete window.onCacheClearError;
        };
        window.cefMessage?.send('cache_clear', []);
      });
    },

    getCacheSize: () => {
      return new Promise((resolve, reject) => {
        window.onCacheGetSizeResponse = (data: any) => {
          resolve(data);
          delete window.onCacheGetSizeResponse;
          delete window.onCacheGetSizeError;
        };
        window.onCacheGetSizeError = (error: string) => {
          reject(new Error(error));
          delete window.onCacheGetSizeResponse;
          delete window.onCacheGetSizeError;
        };
        window.cefMessage?.send('cache_get_size', []);
      });
    },
  };
}

// ==========================================
// Cookie Blocking API
// ==========================================
if (!(window.hodosBrowser as any).cookieBlocking) {
  (window.hodosBrowser as any).cookieBlocking = {
    blockDomain: (domain: string, isWildcard: boolean) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Block domain timeout'));
          delete window.onCookieBlockDomainResponse;
          delete window.onCookieBlockDomainError;
        }, 5000);

        window.onCookieBlockDomainResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onCookieBlockDomainResponse;
          delete window.onCookieBlockDomainError;
        };
        window.onCookieBlockDomainError = (error: string) => {
          clearTimeout(timeout);
          reject(new Error(error));
          delete window.onCookieBlockDomainResponse;
          delete window.onCookieBlockDomainError;
        };
        window.cefMessage?.send('cookie_block_domain', [domain, isWildcard.toString()]);
      });
    },

    unblockDomain: (domain: string) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Unblock domain timeout'));
          delete window.onCookieUnblockDomainResponse;
          delete window.onCookieUnblockDomainError;
        }, 5000);

        window.onCookieUnblockDomainResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onCookieUnblockDomainResponse;
          delete window.onCookieUnblockDomainError;
        };
        window.onCookieUnblockDomainError = (error: string) => {
          clearTimeout(timeout);
          reject(new Error(error));
          delete window.onCookieUnblockDomainResponse;
          delete window.onCookieUnblockDomainError;
        };
        window.cefMessage?.send('cookie_unblock_domain', [domain]);
      });
    },

    getBlockList: () => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          resolve([]);
          delete window.onCookieBlocklistResponse;
          delete window.onCookieBlocklistError;
        }, 5000);

        window.onCookieBlocklistResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onCookieBlocklistResponse;
          delete window.onCookieBlocklistError;
        };
        window.onCookieBlocklistError = (error: string) => {
          clearTimeout(timeout);
          reject(new Error(error));
          delete window.onCookieBlocklistResponse;
          delete window.onCookieBlocklistError;
        };
        window.cefMessage?.send('cookie_get_blocklist', []);
      });
    },

    allowThirdParty: (domain: string) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Allow third party timeout'));
          delete window.onCookieAllowThirdPartyResponse;
          delete window.onCookieAllowThirdPartyError;
        }, 5000);

        window.onCookieAllowThirdPartyResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onCookieAllowThirdPartyResponse;
          delete window.onCookieAllowThirdPartyError;
        };
        window.onCookieAllowThirdPartyError = (error: string) => {
          clearTimeout(timeout);
          reject(new Error(error));
          delete window.onCookieAllowThirdPartyResponse;
          delete window.onCookieAllowThirdPartyError;
        };
        window.cefMessage?.send('cookie_allow_third_party', [domain]);
      });
    },

    removeThirdPartyAllow: (domain: string) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Remove third party allow timeout'));
          delete window.onCookieRemoveThirdPartyAllowResponse;
          delete window.onCookieRemoveThirdPartyAllowError;
        }, 5000);

        window.onCookieRemoveThirdPartyAllowResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onCookieRemoveThirdPartyAllowResponse;
          delete window.onCookieRemoveThirdPartyAllowError;
        };
        window.onCookieRemoveThirdPartyAllowError = (error: string) => {
          clearTimeout(timeout);
          reject(new Error(error));
          delete window.onCookieRemoveThirdPartyAllowResponse;
          delete window.onCookieRemoveThirdPartyAllowError;
        };
        window.cefMessage?.send('cookie_remove_third_party_allow', [domain]);
      });
    },

    getBlockLog: (limit: number, offset: number) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          resolve([]);
          delete window.onCookieBlockLogResponse;
          delete window.onCookieBlockLogError;
        }, 5000);

        window.onCookieBlockLogResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onCookieBlockLogResponse;
          delete window.onCookieBlockLogError;
        };
        window.onCookieBlockLogError = (error: string) => {
          clearTimeout(timeout);
          reject(new Error(error));
          delete window.onCookieBlockLogResponse;
          delete window.onCookieBlockLogError;
        };
        window.cefMessage?.send('cookie_get_block_log', [limit.toString(), offset.toString()]);
      });
    },

    clearBlockLog: () => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Clear block log timeout'));
          delete window.onCookieClearBlockLogResponse;
          delete window.onCookieClearBlockLogError;
        }, 5000);

        window.onCookieClearBlockLogResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onCookieClearBlockLogResponse;
          delete window.onCookieClearBlockLogError;
        };
        window.onCookieClearBlockLogError = (error: string) => {
          clearTimeout(timeout);
          reject(new Error(error));
          delete window.onCookieClearBlockLogResponse;
          delete window.onCookieClearBlockLogError;
        };
        window.cefMessage?.send('cookie_clear_block_log', []);
      });
    },

    getBlockedCount: () => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          resolve({ count: 0 });
          delete window.onCookieBlockedCountResponse;
          delete window.onCookieBlockedCountError;
        }, 5000);

        window.onCookieBlockedCountResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onCookieBlockedCountResponse;
          delete window.onCookieBlockedCountError;
        };
        window.onCookieBlockedCountError = (error: string) => {
          clearTimeout(timeout);
          reject(new Error(error));
          delete window.onCookieBlockedCountResponse;
          delete window.onCookieBlockedCountError;
        };
        window.cefMessage?.send('cookie_get_blocked_count', []);
      });
    },

    resetBlockedCount: () => {
      return new Promise<void>((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Reset blocked count timeout'));
          delete window.onCookieResetBlockedCountResponse;
          delete window.onCookieResetBlockedCountError;
        }, 5000);

        window.onCookieResetBlockedCountResponse = () => {
          clearTimeout(timeout);
          resolve();
          delete window.onCookieResetBlockedCountResponse;
          delete window.onCookieResetBlockedCountError;
        };
        window.onCookieResetBlockedCountError = (error: string) => {
          clearTimeout(timeout);
          reject(new Error(error));
          delete window.onCookieResetBlockedCountResponse;
          delete window.onCookieResetBlockedCountError;
        };
        window.cefMessage?.send('cookie_reset_blocked_count', []);
      });
    },
  };
}

// ==========================================
// BOOKMARK API
// ==========================================
if (!(window.hodosBrowser as any).bookmarks) {
  (window.hodosBrowser as any).bookmarks = {
    add: (url: string, title: string, folderId?: number, tags?: string[]) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Bookmark add timeout'));
          delete window.onBookmarkAddResponse;
        }, 5000);

        window.onBookmarkAddResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onBookmarkAddResponse;
        };

        window.cefMessage?.send('bookmark_add', [url, title, folderId?.toString() ?? '', JSON.stringify(tags ?? [])]);
      });
    },

    get: (id: number) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Bookmark get timeout'));
          delete window.onBookmarkGetResponse;
        }, 5000);

        window.onBookmarkGetResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onBookmarkGetResponse;
        };

        window.cefMessage?.send('bookmark_get', [id.toString()]);
      });
    },

    update: (id: number, fields: { title?: string; url?: string; folderId?: number | null; tags?: string[] }) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Bookmark update timeout'));
          delete window.onBookmarkUpdateResponse;
        }, 5000);

        window.onBookmarkUpdateResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onBookmarkUpdateResponse;
        };

        window.cefMessage?.send('bookmark_update', [id.toString(), JSON.stringify(fields)]);
      });
    },

    remove: (id: number) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Bookmark remove timeout'));
          delete window.onBookmarkRemoveResponse;
        }, 5000);

        window.onBookmarkRemoveResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onBookmarkRemoveResponse;
        };

        window.cefMessage?.send('bookmark_remove', [id.toString()]);
      });
    },

    search: (query: string, limit?: number, offset?: number) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Bookmark search timeout'));
          delete window.onBookmarkSearchResponse;
        }, 5000);

        window.onBookmarkSearchResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onBookmarkSearchResponse;
        };

        window.cefMessage?.send('bookmark_search', [query, (limit ?? 50).toString(), (offset ?? 0).toString()]);
      });
    },

    getAll: (folderId?: number, limit?: number, offset?: number) => {
      return new Promise((resolve, reject) => {
        // 15s (was 5s): on slow Win10 the synchronous SQLite read + IPC round-trip can run
        // long under a saturated UI thread; a too-short timeout dropped the response and
        // left the list empty. useBookmarks.refresh() also retries on top of this.
        const timeout = setTimeout(() => {
          reject(new Error('Bookmark getAll timeout'));
          delete window.onBookmarkGetAllResponse;
        }, 15000);

        window.onBookmarkGetAllResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onBookmarkGetAllResponse;
        };

        window.cefMessage?.send('bookmark_get_all', [(folderId ?? -1).toString(), (limit ?? 50).toString(), (offset ?? 0).toString()]);
      });
    },

    isBookmarked: (url: string) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Bookmark isBookmarked timeout'));
          delete window.onBookmarkIsBookmarkedResponse;
        }, 5000);

        window.onBookmarkIsBookmarkedResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onBookmarkIsBookmarkedResponse;
        };

        window.cefMessage?.send('bookmark_is_bookmarked', [url]);
      });
    },

    getAllTags: () => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Bookmark getAllTags timeout'));
          delete window.onBookmarkGetAllTagsResponse;
        }, 5000);

        window.onBookmarkGetAllTagsResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onBookmarkGetAllTagsResponse;
        };

        window.cefMessage?.send('bookmark_get_all_tags', []);
      });
    },

    updateLastAccessed: (id: number) => {
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Bookmark updateLastAccessed timeout'));
          delete window.onBookmarkUpdateLastAccessedResponse;
        }, 5000);

        window.onBookmarkUpdateLastAccessedResponse = (data: any) => {
          clearTimeout(timeout);
          resolve(data);
          delete window.onBookmarkUpdateLastAccessedResponse;
        };

        window.cefMessage?.send('bookmark_update_last_accessed', [id.toString()]);
      });
    },

    folders: {
      create: (name: string, parentId?: number) => {
        return new Promise((resolve, reject) => {
          const timeout = setTimeout(() => {
            reject(new Error('Bookmark folder create timeout'));
            delete window.onBookmarkFolderCreateResponse;
          }, 5000);

          window.onBookmarkFolderCreateResponse = (data: any) => {
            clearTimeout(timeout);
            resolve(data);
            delete window.onBookmarkFolderCreateResponse;
          };

          window.cefMessage?.send('bookmark_folder_create', [name, (parentId ?? -1).toString()]);
        });
      },

      list: (parentId?: number) => {
        return new Promise((resolve, reject) => {
          const timeout = setTimeout(() => {
            reject(new Error('Bookmark folder list timeout'));
            delete window.onBookmarkFolderListResponse;
          }, 5000);

          window.onBookmarkFolderListResponse = (data: any) => {
            clearTimeout(timeout);
            resolve(data);
            delete window.onBookmarkFolderListResponse;
          };

          window.cefMessage?.send('bookmark_folder_list', [(parentId ?? -1).toString()]);
        });
      },

      update: (id: number, name: string) => {
        return new Promise((resolve, reject) => {
          const timeout = setTimeout(() => {
            reject(new Error('Bookmark folder update timeout'));
            delete window.onBookmarkFolderUpdateResponse;
          }, 5000);

          window.onBookmarkFolderUpdateResponse = (data: any) => {
            clearTimeout(timeout);
            resolve(data);
            delete window.onBookmarkFolderUpdateResponse;
          };

          window.cefMessage?.send('bookmark_folder_update', [id.toString(), name]);
        });
      },

      remove: (id: number) => {
        return new Promise((resolve, reject) => {
          const timeout = setTimeout(() => {
            reject(new Error('Bookmark folder remove timeout'));
            delete window.onBookmarkFolderRemoveResponse;
          }, 5000);

          window.onBookmarkFolderRemoveResponse = (data: any) => {
            clearTimeout(timeout);
            resolve(data);
            delete window.onBookmarkFolderRemoveResponse;
          };

          window.cefMessage?.send('bookmark_folder_remove', [id.toString()]);
        });
      },

      getTree: () => {
        return new Promise((resolve, reject) => {
          const timeout = setTimeout(() => {
            reject(new Error('Bookmark folder getTree timeout'));
            delete window.onBookmarkFolderGetTreeResponse;
          }, 5000);

          window.onBookmarkFolderGetTreeResponse = (data: any) => {
            clearTimeout(timeout);
            resolve(data);
            delete window.onBookmarkFolderGetTreeResponse;
          };

          window.cefMessage?.send('bookmark_folder_get_tree', []);
        });
      },
    },
  };
}
