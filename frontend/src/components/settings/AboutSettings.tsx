import React, { useState, useCallback } from 'react';
import { Typography, Box, Switch } from '@mui/material';
import { SettingsCard, SettingRow } from './SettingsCard';
import { useSettings } from '../../hooks/useSettings';

const APP_VERSION = '0.3.0-beta.14';

const AboutSettings: React.FC = () => {
  const { settings, updateSetting } = useSettings();
  const [checkStatus, setCheckStatus] = useState<'idle' | 'checking' | 'up-to-date' | 'error'>('idle');

  const handleCheckForUpdates = useCallback(() => {
    setCheckStatus('checking');
    if ((window as any).cefMessage?.send) {
      (window as any).cefMessage.send('check_for_updates', []);
    }
    // WinSparkle handles its own UI — reset status after a delay
    setTimeout(() => setCheckStatus('idle'), 5000);
  }, []);

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 3, color: '#e0e0e0' }}>
        About Hodos Browser
      </Typography>

      <SettingsCard title="Version Information">
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1.5 }}>
          <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <Typography sx={{ color: '#888', fontSize: '0.85rem' }}>Browser</Typography>
            <Typography sx={{ color: '#e0e0e0', fontSize: '0.85rem' }}>Hodos Browser {APP_VERSION}</Typography>
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

      <SettingsCard title="Updates">
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
          <SettingRow
            label="Check for updates automatically"
            description="Hodos Browser will periodically check for new versions"
            control={
              <Switch
                checked={settings.browser.autoUpdateEnabled ?? true}
                onChange={(e) => updateSetting('browser.autoUpdateEnabled', e.target.checked)}
                sx={{
                  '& .MuiSwitch-switchBase.Mui-checked': { color: '#a67c00' },
                  '& .MuiSwitch-switchBase.Mui-checked + .MuiSwitch-track': { backgroundColor: '#a67c00' },
                }}
              />
            }
          />
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            <button
              onClick={handleCheckForUpdates}
              disabled={checkStatus === 'checking'}
              style={{
                background: '#a67c00',
                border: 'none',
                color: '#111',
                padding: '8px 20px',
                borderRadius: '6px',
                fontSize: '13px',
                fontWeight: 600,
                cursor: checkStatus === 'checking' ? 'not-allowed' : 'pointer',
                opacity: checkStatus === 'checking' ? 0.6 : 1,
              }}
            >
              {checkStatus === 'checking' ? 'Checking...' : 'Check for updates'}
            </button>
            {checkStatus === 'up-to-date' && (
              <Typography sx={{ color: '#4caf50', fontSize: '0.85rem' }}>You're up to date!</Typography>
            )}
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
