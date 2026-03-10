import React from 'react';
import { Typography, Box } from '@mui/material';
import { SettingsCard } from './SettingsCard';

const AboutSettings: React.FC = () => {
  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 3, color: '#e0e0e0' }}>
        About Hodos Browser
      </Typography>

      <SettingsCard title="Version Information">
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1.5 }}>
          <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
            <Typography sx={{ color: '#888', fontSize: '0.85rem' }}>Browser</Typography>
            <Typography sx={{ color: '#e0e0e0', fontSize: '0.85rem' }}>Hodos Browser 1.0.0</Typography>
          </Box>
          <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
            <Typography sx={{ color: '#888', fontSize: '0.85rem' }}>Engine</Typography>
            <Typography sx={{ color: '#e0e0e0', fontSize: '0.85rem' }}>Chromium (CEF 136)</Typography>
          </Box>
          <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
            <Typography sx={{ color: '#888', fontSize: '0.85rem' }}>Wallet Backend</Typography>
            <Typography sx={{ color: '#e0e0e0', fontSize: '0.85rem' }}>Rust + SQLite</Typography>
          </Box>
          <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
            <Typography sx={{ color: '#888', fontSize: '0.85rem' }}>Protocol</Typography>
            <Typography sx={{ color: '#e0e0e0', fontSize: '0.85rem' }}>BRC-100 (BSV)</Typography>
          </Box>
        </Box>
      </SettingsCard>

      <SettingsCard title="About">
        <Typography sx={{ color: '#888', fontSize: '0.85rem', lineHeight: 1.6 }}>
          Hodos Browser is a Web3 browser built on the Chromium Embedded Framework with a native
          Rust wallet backend. It implements the BRC-100 protocol suite for Bitcoin SV authentication
          and micropayments.
        </Typography>
        <Box sx={{ mt: 2 }}>
          <Typography sx={{ color: '#a67c00', fontSize: '0.85rem' }}>
            hodosbrowser.com
          </Typography>
        </Box>
      </SettingsCard>
    </Box>
  );
};

export default AboutSettings;
