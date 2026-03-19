import React, { useState, useEffect, useCallback } from 'react';
import { Box, IconButton, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import PrivacyShieldPanel from '../components/PrivacyShieldPanel';

declare global {
  interface Window {
    setShieldDomain?: (domain: string) => void;
  }
}

const PrivacyShieldOverlayRoot: React.FC = () => {
  const [domain, setDomain] = useState<string>('');
  const [showCount, setShowCount] = useState(0);

  // Register callback for C++ JS injection (keep-alive pattern)
  useEffect(() => {
    window.setShieldDomain = (d: string) => {
      setDomain(d);
      setShowCount(c => c + 1);  // Force refresh even if domain unchanged
    };

    // Fallback: read from URL param on first load
    const params = new URLSearchParams(window.location.search);
    const urlDomain = params.get('domain');
    if (urlDomain) {
      setDomain(urlDomain);
    }

    return () => {
      delete window.setShieldDomain;
    };
  }, []);

  // Set body data attribute for CEF-level cursor fix
  useEffect(() => {
    document.body.setAttribute('data-overlay', 'privacyshield');
    return () => {
      document.body.removeAttribute('data-overlay');
    };
  }, []);

  const handleClose = useCallback(() => {
    if (window.cefMessage) {
      window.cefMessage.send('cookie_panel_hide');
    }
  }, []);

  return (
    <Box
      sx={{
        width: '100%',
        height: '100%',
        backgroundColor: '#1a1d23',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        boxShadow: '-2px 0 8px rgba(0, 0, 0, 0.1)',
        borderRadius: '8px',
      }}
    >
      {/* Header */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          px: 2,
          py: 1.5,
          borderBottom: 1,
          borderColor: '#2a2d35',
          backgroundColor: '#111827',
        }}
      >
        <Typography variant="subtitle1" sx={{ fontWeight: 600, fontSize: '0.95rem', color: '#f0f0f0' }}>
          Privacy Shield
        </Typography>
        <IconButton
          onClick={handleClose}
          size="small"
          sx={{
            color: '#9ca3af',
            '&:hover': {
              color: '#f0f0f0',
              backgroundColor: '#1f2937',
            },
          }}
        >
          <CloseIcon fontSize="small" />
        </IconButton>
      </Box>

      {/* Panel content */}
      <Box sx={{ flex: 1, overflow: 'hidden' }}>
        <PrivacyShieldPanel domain={domain} showCount={showCount} />
      </Box>
    </Box>
  );
};

export default PrivacyShieldOverlayRoot;
