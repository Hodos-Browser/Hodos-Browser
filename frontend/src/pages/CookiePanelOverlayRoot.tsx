import React, { useEffect } from 'react';
import { Box, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import { CookiePanelOverlay } from '../components/CookiePanelOverlay';
import { HodosButton } from '../components/HodosButton';

/**
 * CookiePanelOverlayRoot - Right-side panel overlay for cookie management.
 *
 * Renders as CEF subprocess overlay (450px × full height) on right side.
 * Includes close button that hides the overlay via IPC.
 */
const CookiePanelOverlayRoot: React.FC = () => {
  // Set body data attribute for CEF-level cursor fix
  useEffect(() => {
    document.body.setAttribute('data-overlay', 'cookiepanel');
    return () => {
      document.body.removeAttribute('data-overlay');
    };
  }, []);

  const handleClose = () => {
    if (window.cefMessage) {
      window.cefMessage.send('cookie_panel_hide');
    }
  };

  return (
    <Box
      sx={{
        width: '100%',
        height: '100%',
        backgroundColor: '#1a1d23',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        boxShadow: '-2px 0 8px rgba(0, 0, 0, 0.2)',
      }}
    >
      {/* Header with close button */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          p: 2,
          borderBottom: 1,
          borderColor: '#2a2d35',
          backgroundColor: '#111827',
        }}
      >
        <Typography variant="h6" sx={{ fontWeight: 600, color: '#f0f0f0' }}>
          Cookie Management
        </Typography>
        <HodosButton variant="icon" size="small" onClick={handleClose} aria-label="Close">
          <CloseIcon sx={{ fontSize: 16 }} />
        </HodosButton>
      </Box>

      {/* Cookie panel content (compact, optimized for overlay) */}
      <Box
        sx={{
          flex: 1,
          overflow: 'hidden',
        }}
      >
        <CookiePanelOverlay />
      </Box>
    </Box>
  );
};

export default CookiePanelOverlayRoot;
