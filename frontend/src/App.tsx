import React, { useEffect } from 'react';
import { Routes, Route } from 'react-router-dom';
import SettingsOverlayRoot from './pages/SettingsOverlayRoot';
import WalletOverlayRoot from './pages/WalletOverlayRoot';
import BackupOverlayRoot from './pages/BackupOverlayRoot';
import BRC100AuthOverlayRoot from './pages/BRC100AuthOverlayRoot';
import OmniboxOverlayRoot from './pages/OmniboxOverlayRoot';
import CookiePanelOverlayRoot from './pages/CookiePanelOverlayRoot';
import MainBrowserView from './pages/MainBrowserView';
import HistoryPage from './pages/HistoryPage';
import WalletPanelPage from './pages/WalletPanelPage';
import BRC100AuthModal from './components/BRC100AuthModal';
import { brc100 } from './bridge/brc100';
// Removed identity types - now using unified wallet system

const App = () => {
  console.log("🔍🔍🔍 APP COMPONENT RENDERING 🔍🔍🔍");
  console.log("🔍 Current pathname:", window.location.pathname);
  console.log("🔍 Full URL:", window.location.href);

  // BRC-100 auth modal state
  const [authModalOpen, setAuthModalOpen] = React.useState(false);
  const [authRequest, setAuthRequest] = React.useState<any>(null);

  // Wallet state tracking (currently unused but available for future features)
  // const [walletExists, setWalletExists] = useState(false);

  useEffect(() => {
    console.log("🔍 useEffect started");

    // Add global function for C++ to call
    (window as any).showBRC100AuthApprovalModal = (
      request: any
    ): Promise<{ approved: boolean; whitelist: boolean }> => {
      console.log('🔐 showBRC100AuthApprovalModal called with request:', request);
      return new Promise((resolve) => {
        setAuthRequest(request);
        setAuthModalOpen(true);

        // Store the resolve function to call when user responds
        (window as any).__authModalResolve = resolve;
      });
    };

    console.log('🔐 Global showBRC100AuthApprovalModal function registered');

    // COMMENTED OUT: Wallet status check and wallet creation/backup prompt on startup
    // This was previously used to create wallet.json and prompt users to save mnemonic
    // We have changed how wallet.json is created, so this check is disabled for now
    // TODO: Re-implement wallet initialization check in the future with new wallet creation flow
    /*
    const checkWalletStatus = async () => {
      console.log("🔍 checkWalletStatus started");

      // Wait for all systems to be ready (for overlay browsers)
      if (window.location.pathname !== '/') {
        await new Promise<void>((resolve) => {
          if (window.allSystemsReady) {
            console.log("🔍 All systems already ready");
            resolve();
          } else {
            console.log("🔍 Waiting for allSystemsReady event...");
            window.addEventListener('allSystemsReady', () => {
              console.log("🔍 allSystemsReady event received");
              resolve();
            }, { once: true });
          }
        });
      }

      // Wait for cefMessage to be ready
      for (let i = 0; i < 40; i++) {
        if (window.cefMessage && typeof window.cefMessage.send === 'function') {
          console.log("🔍 Backend ready after", i, "attempts");
          break;
        }
        await new Promise((r) => setTimeout(r, 50));
      }

      console.log("🔍 Backend check complete, cefMessage exists:", typeof window.cefMessage?.send);
      console.log("🔍 Current pathname:", window.location.pathname);

      // Only check on main page
      if (window.location.pathname === '/' && window.hodosBrowser?.wallet) {
        console.log("🔍 Running wallet status check via API");

        try {
          const walletStatus = await window.hodosBrowser.wallet.getStatus();
          console.log("🔍 Wallet status response:", walletStatus);

          if (walletStatus.needsBackup) {
            // Wallet needs backup - create wallet first, then show modal
            console.log("🔍 Wallet needs backup, creating wallet first...");
            try {
              await window.hodosBrowser.wallet.create();
              console.log("🔍 Wallet created successfully, showing backup modal");
              window.cefMessage?.send('overlay_show_backup', []);
            } catch (error) {
              console.error("💥 Error creating wallet:", error);
            }
          } else {
            // Wallet is backed up - do nothing
            console.log("🔍 Wallet is backed up, no action needed");
          }
        } catch (error) {
          console.error("💥 Error checking wallet status:", error);
        }

      } else {
        console.log("🔍 Skipping wallet check - path:", window.location.pathname, "wallet API ready:", !!window.hodosBrowser?.wallet);
      }
    };

    checkWalletStatus();
    */

    // Initialize BRC-100 API integration
    const initializeBRC100 = async () => {
      try {
        // Check if BRC-100 is available
        const isAvailable = await brc100.isAvailable();
        console.log("🔐 BRC-100 available:", isAvailable);

        if (isAvailable) {
          console.log("🔐 BRC-100 API initialized");
        }
      } catch (error) {
        console.warn("🔐 BRC-100 initialization failed:", error);
      }
    };

    initializeBRC100();

    // Cleanup function to remove event listeners
    return () => {
      console.log("🧹 App cleanup - removing event listeners");
      // Note: Event listeners are automatically cleaned up when the component unmounts
      // but this ensures we have explicit cleanup logging
    };
  }, []);

  // BRC-100 auth modal handlers
  const handleAuthApprove = (whitelist: boolean) => {
    setAuthModalOpen(false);
    if ((window as any).__authModalResolve) {
      (window as any).__authModalResolve({ approved: true, whitelist });
      (window as any).__authModalResolve = null;
    }
  };

  const handleAuthReject = () => {
    setAuthModalOpen(false);
    if ((window as any).__authModalResolve) {
      (window as any).__authModalResolve({ approved: false, whitelist: false });
      (window as any).__authModalResolve = null;
    }
  };

  return (
    <>
      <Routes>
        {/* <Route path="/" element={walletExists ? <MainBrowserView /> : <OverlayRoot />} /> */}
        <Route path="/" element={<MainBrowserView />} />
        <Route path="/history" element={<HistoryPage />} />
        <Route path="/wallet-panel" element={<WalletPanelPage />} />
        <Route path="/settings" element={<SettingsOverlayRoot />} />
        <Route path="/wallet" element={<WalletOverlayRoot />} />
        <Route path="/backup" element={<BackupOverlayRoot />} />
        <Route path="/brc100-auth" element={<BRC100AuthOverlayRoot />} />
        <Route path="/omnibox" element={<OmniboxOverlayRoot />} />
        <Route path="/cookie-panel" element={<CookiePanelOverlayRoot />} />
      </Routes>

      {/* BRC-100 Authentication Modal */}
      {authRequest && (
        <BRC100AuthModal
          open={authModalOpen}
          onClose={() => setAuthModalOpen(false)}
          onApprove={handleAuthApprove}
          onReject={handleAuthReject}
          request={authRequest}
        />
      )}
    </>
  );
};

export default App;
