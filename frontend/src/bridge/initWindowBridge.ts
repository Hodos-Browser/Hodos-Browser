
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

// ⭐ MIGRATED — Phase 8c stage 3 batch 2. The one LIVE slot in the wallet namespace
// (WalletPanel's receive flow, via useAddress).
//
// This still force-overrides the native `address.generate` that C++ binds through
// `AddressHandler` — that handler makes a SYNCHRONOUS wallet call on the renderer thread,
// and this override is what has kept it unreachable. Keep overriding.
//
// Under the legacy bridge this and the (now deleted, caller-less) `wallet.generateAddress`
// shared one global slot pair (`onAddressGenerated` / `onAddressError`), so a call through
// either could steal the other's reply. This is now the only entry point.
window.hodosBrowser.address.generate = () => {
  if (!window.hodosBrowser?.bridge?.generateAddress) {
    return Promise.reject(new Error('address.generate: native bridge unavailable'));
  }
  return window.hodosBrowser.bridge.generateAddress();
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

    // ⛔ DELETED in Phase 8c stage 3 batch 2, not migrated: `create`, `load`,
    // `generateAddress`, `getCurrentAddress`, `getAddresses`, `getTransactionHistory`.
    // None had a reachable caller (`useWallet()` had no consumers; `create`'s only call
    // sat inside a commented-out block), and their C++ round trips are gone with them.
    // Wallet creation / recovery is the wallet overlay's `walletFetch` path, not this one.
    //
    // ⛔ DELETED with the backup overlay (Phase 8c O8, owner call 2026-09-12): `getInfo`,
    // `markBackedUp`, `getBackupModalState`, `setBackupModalState`. Their only consumer
    // was `BackupOverlayRoot`, whose only opener sat in a commented-out block, and the
    // Rust routes behind the first two do not exist. The first-run "write down your
    // recovery phrase" flow lives in `WalletPanelPage` (mnemonic + prevent-close + PIN).

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

// ⛔ DELETED in Phase 8c stage 3 batch 3 (2026-09-12): the `cookies` (6) and
// `cookieBlocking` (9) namespaces that used to live here. They had NO callers — the
// `useCookies` / `useCookieBlocking` hooks send the IPC themselves — and they set the
// SAME `window.on*` globals the hooks set, i.e. a second copy of the single-slot race.
// The hooks now call the native, per-request-id `hodosBrowser.bridge.cookie*` functions.

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
