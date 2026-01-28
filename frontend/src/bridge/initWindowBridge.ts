
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
        pendingAuth.body
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
  window.hodosBrowser.overlay.toggleInput = (enable: boolean) => {
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
    // Set up response handlers
    window.onAddressGenerated = (data: any) => {
      console.log("✅ Address generated:", data);
      resolve(data);
      delete window.onAddressGenerated;
      delete window.onAddressError;
    };

    window.onAddressError = (error: string) => {
      console.error("❌ Address generation error:", error);
      reject(new Error(error));
      delete window.onAddressGenerated;
      delete window.onAddressError;
    };

    // Send the request
    window.cefMessage?.send('address_generate', []);
  });
};


// Wallet methods
if (!window.hodosBrowser.wallet) {
  window.hodosBrowser.wallet = {
    getStatus: () => {
      console.log("🔍 JS: Sending wallet_status_check to native");
      return new Promise((resolve, reject) => {
        window.onWalletStatusResponse = (data: any) => {
          console.log("✅ Wallet status retrieved:", data);
          resolve(data);
          delete window.onWalletStatusResponse;
          delete window.onWalletStatusError;
        };

        window.onWalletStatusError = (error: string) => {
          console.error("❌ Wallet status error:", error);
          reject(new Error(error));
          delete window.onWalletStatusResponse;
          delete window.onWalletStatusError;
        };

        window.cefMessage?.send('wallet_status_check', []);
      });
    },

    create: () => {
      console.log("🆕 JS: Sending create_wallet to native");
      return new Promise((resolve, reject) => {
        window.onCreateWalletResponse = (data: any) => {
          console.log("✅ Wallet created:", data);
          resolve(data);
          delete window.onCreateWalletResponse;
          delete window.onCreateWalletError;
        };

        window.onCreateWalletError = (error: string) => {
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
        window.onLoadWalletResponse = (data: any) => {
          console.log("✅ Wallet loaded:", data);
          resolve(data);
          delete window.onLoadWalletResponse;
          delete window.onLoadWalletError;
        };

        window.onLoadWalletError = (error: string) => {
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
        window.onGetWalletInfoResponse = (data: any) => {
          console.log("✅ Wallet info retrieved:", data);
          resolve(data);
          delete window.onGetWalletInfoResponse;
          delete window.onGetWalletInfoError;
        };

        window.onGetWalletInfoError = (error: string) => {
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
        window.onAddressGenerated = (data: any) => {
          console.log("✅ Address generated:", data);
          resolve(data);
          delete window.onAddressGenerated;
          delete window.onAddressError;
        };

        window.onAddressError = (error: string) => {
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
        window.onGetCurrentAddressResponse = (data: any) => {
          console.log("✅ Current address retrieved:", data);
          resolve(data);
          delete window.onGetCurrentAddressResponse;
          delete window.onGetCurrentAddressError;
        };

        window.onGetCurrentAddressError = (error: string) => {
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
        window.onGetAddressesResponse = (data: any) => {
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
        window.onMarkWalletBackedUpResponse = (data: any) => {
          console.log("✅ Wallet marked as backed up:", data);
          resolve(data);
          delete window.onMarkWalletBackedUpResponse;
          delete window.onMarkWalletBackedUpError;
        };

        window.onMarkWalletBackedUpError = (error: string) => {
          console.error("❌ Mark backed up error:", error);
          reject(new Error(error));
          delete window.onMarkWalletBackedUpResponse;
          delete window.onMarkWalletBackedUpError;
        };

        window.cefMessage?.send('mark_wallet_backed_up', []);
      });
    },

    getBackupModalState: () => {
      console.log("🔍 JS: Getting backup modal state");
      return new Promise((resolve) => {
        window.onGetBackupModalStateResponse = (data: any) => {
          console.log("✅ Backup modal state retrieved:", data);
          resolve(data);
          delete window.onGetBackupModalStateResponse;
        };

        window.cefMessage?.send('get_backup_modal_state', []);
      });
    },

    setBackupModalState: (shown: boolean) => {
      console.log("🔍 JS: Setting backup modal state to:", shown);
      return new Promise((resolve) => {
        window.onSetBackupModalStateResponse = (data: any) => {
          console.log("✅ Backup modal state set:", data);
          resolve(data);
          delete window.onSetBackupModalStateResponse;
        };

        window.cefMessage?.send('set_backup_modal_state', [shown]);
      });
    },

    getBalance: () => {
      console.log("💳 JS: Sending get_balance to native");
      return new Promise((resolve, reject) => {
        window.onGetBalanceResponse = (data: any) => {
          console.log("✅ Balance retrieved:", data);
          resolve(data);
          delete window.onGetBalanceResponse;
          delete window.onGetBalanceError;
        };

        window.onGetBalanceError = (error: string) => {
          console.error("❌ Balance retrieval error:", error);
          reject(new Error(error));
          delete window.onGetBalanceResponse;
          delete window.onGetBalanceError;
        };

        window.cefMessage?.send('get_balance', []);
      });
    },

    sendTransaction: (data: any) => {
      console.log("🚀 JS: Sending send_transaction to native");
      return new Promise((resolve, reject) => {
        window.onSendTransactionResponse = (data: any) => {
          console.log("✅ Transaction sent:", data);
          resolve(data);
          delete window.onSendTransactionResponse;
          delete window.onSendTransactionError;
        };

        window.onSendTransactionError = (error: string) => {
          console.error("❌ Transaction error:", error);
          reject(new Error(error));
          delete window.onSendTransactionResponse;
          delete window.onSendTransactionError;
        };

        window.cefMessage?.send('send_transaction', [JSON.stringify(data)]);
      });
    },

    getTransactionHistory: () => {
      console.log("📜 JS: Sending get_transaction_history to native");
      return new Promise((resolve, reject) => {
        window.onGetTransactionHistoryResponse = (data: any) => {
          console.log("✅ Transaction history retrieved:", data);
          resolve(data);
          delete window.onGetTransactionHistoryResponse;
          delete window.onGetTransactionHistoryError;
        };

        window.onGetTransactionHistoryError = (error: string) => {
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
