import React, { useEffect } from 'react';
import { Box } from '@mui/material';
import Omnibox from '../components/Omnibox';

const OmniboxOverlayRoot: React.FC = () => {
  useEffect(() => {
    console.log('🔍 Omnibox overlay mounted');
  }, []);

  const handleNavigate = (url: string) => {
    console.log('🔍 Navigating to:', url);

    // Send navigate message via IPC
    if (window.cefMessage) {
      window.cefMessage.send('omnibox_navigate', [url]);
    }
  };

  const handleEscape = () => {
    console.log('🔍 Closing omnibox overlay');

    // Send close message via IPC
    if (window.cefMessage) {
      window.cefMessage.send('omnibox_close', []);
    }
  };

  // Listen for Escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        handleEscape();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <Box
      sx={{
        width: '100%',
        height: '100%',
        bgcolor: 'transparent',
        position: 'relative',
      }}
    >
      {/* Position address bar exactly where it is in the header */}
      {/* TabBar: 40px, Toolbar: 54px (9px padding top), nav buttons: ~140px */}
      <Box
        sx={{
          position: 'absolute',
          top: 49, // 40px TabBar + 9px toolbar padding
          left: 148, // 8px toolbar padding + 140px nav buttons
          right: 128, // Space for wallet/history/settings buttons (3 buttons + padding)
        }}
      >
        <Omnibox
          onNavigate={handleNavigate}
          initialValue=""
        />
      </Box>
    </Box>
  );
};

export default OmniboxOverlayRoot;
