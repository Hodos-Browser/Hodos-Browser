import React, { useState, useEffect } from 'react';
import BRC100AuthModal from '../components/BRC100AuthModal';

const BRC100AuthOverlayRoot: React.FC = () => {
  console.log('🔐 BRC100AuthOverlayRoot component rendering...');

  const [authModalOpen, setAuthModalOpen] = useState(false);
  const [authRequest, setAuthRequest] = useState<any>(null);

  useEffect(() => {
    console.log('🔐 BRC100AuthOverlayRoot component mounted');
    // Listen for auth request messages from CEF
    const handleMessage = (event: MessageEvent) => {
      if (event.data.type === 'brc100_auth_request') {
        const { domain, method, endpoint, body } = event.data.payload;
        setAuthRequest({
          domain: domain,
          appId: domain, // Use domain as app ID for now
          purpose: 'Authentication Request',
          challenge: body, // Use request body as challenge data
          sessionDuration: 30,
          permissions: ['Access identity certificate']
        });
        setAuthModalOpen(true);
      }
    };

    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, []);

  const handleAuthApprove = async (whitelist: boolean) => {
    console.log('🔐 BRC-100 Auth approved, whitelist:', whitelist);
    setAuthModalOpen(false);

    try {
      // Add domain to whitelist if requested
      if (whitelist && authRequest) {
        console.log('🔐 Adding domain to whitelist via CEF message:', authRequest.domain);
        if (window.cefMessage) {
          const whitelistData = {
            domain: authRequest.domain,
            permanent: true
          };
          window.cefMessage.send('add_domain_to_whitelist', [JSON.stringify(whitelistData)]);
        }
      }

      // Send approval response to HTTP interceptor
      if (window.cefMessage) {
        const responseData = {
          approved: true,
          whitelist: whitelist
        };
        window.cefMessage.send('brc100_auth_response', [JSON.stringify(responseData)]);
      }

      // Close overlay window
      if (window.bitcoinBrowser && window.bitcoinBrowser.overlay && window.bitcoinBrowser.overlay.close) {
        window.bitcoinBrowser.overlay.close();
      }
    } catch (error) {
      console.error('🔐 Error handling auth approval:', error);
    }
  };

  const handleAuthReject = () => {
    console.log('🔐 BRC-100 Auth rejected');
    setAuthModalOpen(false);

    try {
      // Send rejection response to HTTP interceptor
      if (window.cefMessage) {
        const responseData = {
          approved: false,
          whitelist: false
        };
        window.cefMessage.send('brc100_auth_response', [JSON.stringify(responseData)]);
      }

      // Close overlay window
      if (window.bitcoinBrowser && window.bitcoinBrowser.overlay && window.bitcoinBrowser.overlay.close) {
        window.bitcoinBrowser.overlay.close();
      }
    } catch (error) {
      console.error('🔐 Error handling auth rejection:', error);
    }
  };

  console.log('🔐 BRC100AuthOverlayRoot rendering, authRequest:', authRequest);

  return (
    <div style={{
      width: '100vw',
      height: '100vh',
      backgroundColor: 'rgba(0, 0, 0, 0.5)', // Semi-transparent background
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center'
    }}>
      {authRequest ? (
        <BRC100AuthModal
          open={authModalOpen}
          onClose={() => setAuthModalOpen(false)}
          onApprove={handleAuthApprove}
          onReject={handleAuthReject}
          request={authRequest}
        />
      ) : (
        <div style={{
          color: 'white',
          fontSize: '18px',
          textAlign: 'center'
        }}>
          🔐 BRC-100 Authentication Overlay Loading...
        </div>
      )}
    </div>
  );
};

export default BRC100AuthOverlayRoot;
