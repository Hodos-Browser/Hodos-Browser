import React, { useEffect, useRef } from 'react';
import { Box } from '@mui/material';
import Omnibox from '../components/Omnibox';

const OmniboxOverlayRoot: React.FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    console.log('🔍 Omnibox overlay mounted');

    // Focus will be handled by Omnibox component's onFocus
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

  // Listen for Escape key at container level
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
      ref={containerRef}
      sx={{
        width: '100%',
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        pt: 2,
        px: 2,
        bgcolor: 'transparent',
      }}
    >
      {/* Unified pill container wrapping the entire omnibox */}
      <Box
        sx={{
          width: '100%',
          maxWidth: '800px',
          borderRadius: 20,
          bgcolor: '#ffffff',
          boxShadow: '0 4px 16px rgba(0, 0, 0, 0.2)',
          overflow: 'visible',
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
