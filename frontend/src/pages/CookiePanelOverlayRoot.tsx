import React, { useEffect } from 'react';
import { Box, IconButton, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import CookiesPanel from '../components/CookiesPanel';

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
        backgroundColor: 'background.paper',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        boxShadow: '-2px 0 8px rgba(0, 0, 0, 0.1)',
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
          borderColor: 'divider',
          backgroundColor: 'background.default',
        }}
      >
        <Typography variant="h6" sx={{ fontWeight: 600 }}>
          Cookie Management
        </Typography>
        <IconButton
          onClick={handleClose}
          size="small"
          sx={{
            color: 'text.secondary',
            '&:hover': {
              color: 'text.primary',
              backgroundColor: 'action.hover',
            },
          }}
        >
          <CloseIcon fontSize="small" />
        </IconButton>
      </Box>

      {/* Cookie panel content (scrollable) */}
      <Box
        sx={{
          flex: 1,
          overflow: 'auto',
          p: 2,
        }}
      >
        <CookiesPanel />
      </Box>
    </Box>
  );
};

export default CookiePanelOverlayRoot;
