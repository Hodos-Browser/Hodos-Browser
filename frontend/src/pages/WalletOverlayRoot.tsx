import React, { useState, useEffect } from 'react';
import WalletPanelLayout from '../components/panels/WalletPanelLayout';

const WalletOverlayRoot: React.FC = () => {
  const [walletOpen, setWalletOpen] = useState(true);

  useEffect(() => {
    console.log("💰 WalletOverlayRoot mounted");
    console.log("💰 cefMessage available:", !!window.cefMessage);
    console.log("💰 cefMessage.send available:", !!(window.cefMessage?.send));

    // Test cefMessage immediately
    if (window.cefMessage?.send) {
      console.log("💰 Testing cefMessage.send from wallet overlay");
      // Could add a test message here if needed
    } else {
      console.log("❌ cefMessage not available in wallet overlay");
    }

    // Auto-open wallet panel when this component mounts
    console.log("💰 Setting walletOpen to true");
    setWalletOpen(true);

    // Set up window trigger for wallet panel
    window.triggerPanel = (panelName: string) => {
      console.log("💰 Wallet panel trigger received:", panelName);
      if (panelName === 'wallet') {
        setWalletOpen(true);
      }
    };
  }, []);

  console.log("💰 WalletOverlayRoot render - walletOpen:", walletOpen);

  return (
    <>
      <WalletPanelLayout
        open={walletOpen}
        onClose={() => {
          console.log("💰 Wallet closing");
          setWalletOpen(false);
          // Use the new process-per-overlay close method
          console.log("💰 Calling overlay_close message for wallet overlay");
          window.cefMessage?.send('overlay_close', []);
        }}
      />
    </>
  );
};

export default WalletOverlayRoot;
