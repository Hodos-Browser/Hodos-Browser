import React, { useEffect } from 'react';
import { Box, IconButton, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import SettingsIcon from '@mui/icons-material/Settings';

/**
 * SettingsOverlayRoot - Right-side panel overlay for settings.
 *
 * Renders as CEF subprocess overlay (450px × 450px) on right side,
 * matching the wallet and cookie panel pattern.
 */
const SettingsOverlayRoot: React.FC = () => {
  useEffect(() => {
    document.body.setAttribute('data-overlay', 'settings');
    return () => {
      document.body.removeAttribute('data-overlay');
    };
  }, []);

  const handleClose = () => {
    // Use dedicated settings_close message (same pattern as cookie_panel_hide)
    if (window.cefMessage?.send) {
      window.cefMessage.send('settings_close');
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
      {/* Header */}
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
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <SettingsIcon fontSize="small" sx={{ color: 'text.secondary' }} />
          <Typography variant="h6" sx={{ fontWeight: 600 }}>
            Settings
          </Typography>
        </Box>
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

      {/* Settings content */}
      <Box
        sx={{
          flex: 1,
          overflow: 'auto',
          p: 2,
        }}
      >
        <Typography variant="body2" color="text.secondary">
          Settings content will go here.
        </Typography>
      </Box>
    </Box>
  );
};

export default SettingsOverlayRoot;
