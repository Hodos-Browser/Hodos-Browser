import React, { useState, useEffect } from 'react';
import { HodosButton } from '../components/HodosButton';
// Removed identity types - now using unified wallet system

const BackupOverlayRoot: React.FC = () => {
  const [showBackupModal, setShowBackupModal] = useState(false);
  const [wallet, setWallet] = useState<{
    address: string;
    mnemonic: string;
    version: string;
    backedUp: boolean;
  } | null>(null);
  const [systemsReady, setSystemsReady] = useState(() => {
    // Initialize based on current state
    const ready = !!window.allSystemsReady;
    console.log("💾 Initial systemsReady state:", ready);
    return ready;
  });

  // Always show modal when this component mounts - it's only mounted when we need to show the backup modal
  useEffect(() => {
    console.log("💾 BackupOverlayRoot mounted - always showing modal");
    setShowBackupModal(true);
  }, []);

  // Modal state management functions
  const showModal = async () => {
    console.log("💾 Showing backup modal...");
    setShowBackupModal(true);

    try {
      await window.hodosBrowser.wallet?.setBackupModalState(true);
      console.log("💾 Backup modal state saved to C++ backend");
    } catch (error) {
      console.error("💾 Failed to save backup modal state:", error);
    }
  };

  const closeModal = async () => {
    console.log("💾 Backup modal closing");
    setShowBackupModal(false);

    try {
      await window.hodosBrowser.wallet?.setBackupModalState(false);
      console.log("💾 Backup modal state cleared in C++ backend");
    } catch (error) {
      console.error("💾 Failed to clear backup modal state:", error);
    }

    window.cefMessage?.send('overlay_close', []);
  };

  useEffect(() => {
    console.log("💾 BackupOverlayRoot mounted, systemsReady:", systemsReady);

    // Wait for all systems to be ready
    const waitForSystemsReady = () => {
      return new Promise<void>((resolve, reject) => {
        if (window.allSystemsReady) {
          console.log("💾 All systems already ready");
          setSystemsReady(true);
          resolve();
        } else {
          console.log("💾 Waiting for allSystemsReady event...");
          window.addEventListener('allSystemsReady', () => {
            console.log("💾 allSystemsReady event received");
            setSystemsReady(true);
            resolve();
          }, { once: true });

          // Timeout after 10 seconds
          setTimeout(() => {
            reject(new Error('All systems not ready after 10 seconds'));
          }, 10000);
        }
      });
    };

    const loadWallet = async () => {
      console.log("💾 Loading wallet for backup modal...");

      // Wait for all systems to be ready
      await waitForSystemsReady();
      console.log("💾 All systems ready, loading wallet");

      try {
        // Load existing wallet (should already be created by main app)
        const walletInfo = await window.hodosBrowser.wallet!.getInfo();
        console.log("💾 Loaded wallet:", walletInfo);

        setWallet({
          address: walletInfo.address || "No address generated",
          mnemonic: walletInfo.mnemonic,
          version: walletInfo.version || "1.0.0",
          backedUp: walletInfo.backedUp || false
        });

        // Show modal immediately since wallet is ready
        console.log("💾 Showing backup modal...");
        setShowBackupModal(true); // Set local state immediately
        await showModal(); // Also update C++ backend state

        // Force CEF to repaint after modal shows
        setTimeout(() => {
          console.log("💾 Forcing CEF repaint...");
          window.cefMessage?.send('force_repaint', []);
        }, 100);

      } catch (error) {
        console.error("💾 Failed to load wallet:", error);
        const errorMessage = error instanceof Error ? error.message : String(error);
        setWallet({
          address: "Error loading wallet",
          mnemonic: errorMessage || "Bitcoin Browser wallet API not available",
          version: "1.0.0",
          backedUp: false
        });
        setShowBackupModal(true);
      }
    };

    loadWallet().catch((error) => {
      console.error("💾 Failed to load wallet:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setWallet({
        address: "API Error",
        mnemonic: errorMessage || "Bitcoin Browser wallet API not available",
        version: "1.0.0",
        backedUp: false
      });
      setShowBackupModal(true);
    });

    // Cleanup function
    return () => {
      console.log("🧹 BackupOverlayRoot cleanup - removing event listeners");
      // Event listeners are automatically cleaned up when component unmounts
    };
  }, []);

  // Handle modal display when wallet is ready
  useEffect(() => {
    if (wallet && !showBackupModal) {
      console.log("💾 Wallet ready, showing backup modal in 100ms...");
      setTimeout(() => {
        setShowBackupModal(true);
      }, 100);
    }
  }, [wallet, showBackupModal]);

  // Fallback: Always show modal if wallet exists (regardless of showBackupModal state)
  useEffect(() => {
    if (wallet && systemsReady) {
      console.log("💾 Fallback: Ensuring modal is shown for wallet");
      setShowBackupModal(true);
    }
  }, [wallet, systemsReady]);

  console.log("💾 BackupOverlayRoot render - systemsReady:", systemsReady, "showBackupModal:", showBackupModal, "wallet:", wallet);
  console.log("💾 window.allSystemsReady:", window.allSystemsReady);
  console.log("💾 Render conditions - systemsReady:", systemsReady, "wallet exists:", !!wallet, "showBackupModal:", showBackupModal);
  console.log("💾 Will render modal:", systemsReady && wallet && showBackupModal);

  // Show loading screen until all systems are ready AND wallet is loaded
  if (!systemsReady || !wallet) {
    return (
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        backgroundColor: 'rgba(0, 0, 0, 0.8)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        color: 'white',
        fontSize: '24px',
        fontFamily: 'Arial, sans-serif'
      }}>
        <div style={{ textAlign: 'center' }}>
          <div style={{ marginBottom: '20px' }}>🔧</div>
          <div>{!systemsReady ? 'Initializing Systems...' : 'Creating Wallet...'}</div>
          <div style={{ fontSize: '16px', marginTop: '10px', opacity: 0.7 }}>
            {!systemsReady ? 'Setting up V8 context and APIs' : 'Please wait while we set up your HD wallet'}
          </div>
        </div>
      </div>
    );
  }


  console.log("💾 Rendering backup modal with wallet:", wallet);
  console.log("💾 Wallet details:", {
    exists: !!wallet,
    mnemonic: wallet?.mnemonic?.substring(0, 20) + "...",
    address: wallet?.address,
    version: wallet?.version,
    backedUp: wallet?.backedUp
  });
  console.log("💾 Modal should be visible now! Check if you can see it on screen.");
  console.log("💾 CEF Overlay Debug Info:");
  console.log("💾 - window.location.href:", window.location.href);
  console.log("💾 - document.body:", document.body);
  console.log("💾 - document.body.children.length:", document.body?.children?.length);
  console.log("💾 - document.documentElement:", document.documentElement);
  console.log("💾 - window.innerWidth:", window.innerWidth, "window.innerHeight:", window.innerHeight);

  // Test if DOM is actually updating
  setTimeout(() => {
    const testDiv = document.querySelector('[data-test="backup-modal"]') as HTMLElement;
    console.log("💾 DOM Test - Found test div:", testDiv);
    if (testDiv) {
      console.log("💾 DOM Test - Div is visible:", testDiv.offsetWidth > 0 && testDiv.offsetHeight > 0);
      console.log("💾 DOM Test - Div style:", window.getComputedStyle(testDiv).display);

      // CEF-specific debugging
      console.log("💾 CEF Debug - testDiv.offsetWidth:", testDiv.offsetWidth);
      console.log("💾 CEF Debug - testDiv.offsetHeight:", testDiv.offsetHeight);
      console.log("💾 CEF Debug - testDiv.getBoundingClientRect():", testDiv.getBoundingClientRect());
      console.log("💾 CEF Debug - testDiv.parentElement:", testDiv.parentElement);
      console.log("💾 CEF Debug - testDiv.parentElement?.tagName:", testDiv.parentElement?.tagName);
    }
  }, 100);

  return (
    <div
      data-test="backup-modal"
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100vw',
        height: '100vh',
        backgroundColor: 'rgba(0, 0, 0, 0.8)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 9999,
        fontFamily: 'Arial, sans-serif'
      }}>
      {/* Simple backup modal without Material-UI */}
      <div style={{
        backgroundColor: '#1a1d23',
        color: '#f0f0f0',
        padding: '30px',
        borderRadius: '10px',
        maxWidth: '500px',
        width: '90%',
        boxShadow: '0 4px 20px rgba(0,0,0,0.5)',
        fontFamily: 'Arial, sans-serif'
      }}>
        <h2 style={{ margin: '0 0 20px 0', color: '#a67c00' }}>Wallet Backup Required</h2>

        <p style={{ margin: '0 0 20px 0', color: '#9ca3af' }}>
          Your wallet has been created! Please save your seed phrase in a safe place.
          This is the only way to recover your wallet if you lose access.
        </p>

        <div style={{ marginBottom: '20px' }}>
          <label style={{ display: 'block', marginBottom: '8px', fontWeight: 'bold', color: '#f0f0f0' }}>
            Seed Phrase (Mnemonic):
          </label>
          <textarea
            value={wallet.mnemonic}
            readOnly
            style={{
              width: '100%',
              height: '80px',
              padding: '10px',
              border: '1px solid #2a2d35',
              borderRadius: '4px',
              fontFamily: 'monospace',
              fontSize: '14px',
              resize: 'none',
              backgroundColor: '#111827',
              color: '#f0f0f0',
            }}
          />
          <HodosButton
            variant="secondary"
            onClick={() => {
              navigator.clipboard.writeText(wallet.mnemonic);
              alert('Seed phrase copied to clipboard!');
            }}
            style={{ marginTop: '8px' }}
          >
            Copy to Clipboard
          </HodosButton>
        </div>

        <div style={{ marginBottom: '20px' }}>
          <label style={{ display: 'block', marginBottom: '8px', fontWeight: 'bold', color: '#f0f0f0' }}>
            Wallet Address:
          </label>
          <input
            type="text"
            value={wallet.address}
            readOnly
            style={{
              width: '100%',
              padding: '10px',
              border: '1px solid #2a2d35',
              borderRadius: '4px',
              fontFamily: 'monospace',
              fontSize: '14px',
              backgroundColor: '#111827',
              color: '#f0f0f0',
            }}
          />
        </div>

        <div style={{ marginBottom: '20px' }}>
          <label style={{ display: 'flex', alignItems: 'center', cursor: 'pointer', color: '#f0f0f0' }}>
            <input
              type="checkbox"
              checked={wallet.backedUp}
              onChange={async (e) => {
                if (e.target.checked) {
                  try {
                    console.log("💾 Marking wallet as backed up...");
                    const result = await window.hodosBrowser?.wallet?.markBackedUp?.();
                    console.log("💾 Mark backed up result:", result);

                    if (result?.success) {
                      // Update local state
                      setWallet(prev => {
                        if (!prev) return null;
                        return {
                          ...prev,
                          backedUp: true
                        };
                      });
                      console.log("💾 Local wallet state updated to backed up");
                    }
                  } catch (error) {
                    console.error("💾 Error marking wallet as backed up:", error);
                  }
                }
              }}
              style={{ marginRight: '8px' }}
            />
            I have safely backed up my seed phrase
          </label>
        </div>

        <div style={{ textAlign: 'right' }}>
          <HodosButton
            variant="primary"
            onClick={closeModal}
            disabled={!wallet.backedUp}
          >
            {wallet.backedUp ? 'Done' : 'Please backup first'}
          </HodosButton>
        </div>
      </div>
    </div>
  );
};

export default BackupOverlayRoot;
