import React, { useEffect, useRef } from 'react';
import { Box } from '@mui/material';
import Omnibox from '../components/Omnibox';

const OmniboxOverlayRoot: React.FC = () => {
  const omniboxRef = useRef<HTMLDivElement>(null);

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

  const handleClose = () => {
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
        handleClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Close overlay when clicking outside the omnibox
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (omniboxRef.current && !omniboxRef.current.contains(e.target as Node)) {
        console.log('🔍 Click outside omnibox detected, closing overlay');
        handleClose();
      }
    };

    // Add listener with a small delay to avoid closing immediately on mount
    const timer = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 100);

    return () => {
      clearTimeout(timer);
      document.removeEventListener('mousedown', handleClickOutside);
    };
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
        ref={omniboxRef}
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
