import React, { useState, useEffect } from 'react';
import SettingsPanelLayout from '../components/panels/SettingsPanelLayout';
import BRC100AuthModal from '../components/BRC100AuthModal';

const SettingsOverlayRoot: React.FC = () => {
  const [settingsOpen, setSettingsOpen] = useState(true);
  const [authModalOpen, setAuthModalOpen] = useState(false);
  const [authRequest, setAuthRequest] = useState<any>(null);

  useEffect(() => {
    console.log("🔧 SettingsOverlayRoot mounted");
    console.log("🔧 cefMessage available:", !!window.cefMessage);
    console.log("🔧 cefMessage.send available:", !!(window.cefMessage && window.cefMessage.send));

    // Test cefMessage immediately
    if (window.cefMessage && window.cefMessage.send) {
      console.log("🔧 Testing cefMessage.send from settings overlay");
      window.cefMessage.send('test_settings_message', []);
    } else {
      console.log("❌ cefMessage not available in settings overlay");
    }

    // Auto-open settings panel when this component mounts
    console.log("🔧 Setting settingsOpen to true");
    setSettingsOpen(true);

    // Set up window trigger for settings panel
    window.triggerPanel = (panelName: string) => {
      console.log("🔧 Settings panel trigger received:", panelName);
      if (panelName === 'settings') {
        setSettingsOpen(true);
      }
    };

    // Check for pending BRC-100 auth request
    const pendingAuthRequest = (window as any).pendingBRC100AuthRequest;
    if (pendingAuthRequest) {
      console.log("🔐 BRC-100 auth request found, showing modal");
      setAuthRequest({
        domain: pendingAuthRequest.domain,
        appId: pendingAuthRequest.domain,
        purpose: 'Authentication Request',
        challenge: pendingAuthRequest.body,
        sessionDuration: 30,
        permissions: ['Access identity certificate']
      });
      setAuthModalOpen(true);
      // Clear the pending request
      (window as any).pendingBRC100AuthRequest = null;
    }
  }, []);

  const handleAuthApprove = (whitelist: boolean) => {
    console.log('🔐 BRC-100 Auth approved, whitelist:', whitelist);
    setAuthModalOpen(false);
    // TODO: Send response back to HTTP interceptor
    // TODO: Close overlay window
  };

  const handleAuthReject = () => {
    console.log('🔐 BRC-100 Auth rejected');
    setAuthModalOpen(false);
    // TODO: Send response back to HTTP interceptor
    // TODO: Close overlay window
  };

  console.log("🔧 SettingsOverlayRoot render - settingsOpen:", settingsOpen, "authModalOpen:", authModalOpen);

  return (
    <>
      <SettingsPanelLayout
        open={settingsOpen}
        onClose={() => {
          console.log("🔧 Settings closing");
          setSettingsOpen(false);
          // Use the new process-per-overlay close method
          console.log("🔧 Calling overlay_close message for settings overlay");
          window.cefMessage?.send('overlay_close', []);
        }}
      />
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

export default SettingsOverlayRoot;
