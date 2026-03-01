import React from 'react';
import { Typography, Box } from '@mui/material';
import { SettingsCard, SettingRow } from './SettingsCard';
import { useSettings } from '../../hooks/useSettings';

const DownloadSettings: React.FC = () => {
  const { settings, updateSetting } = useSettings();

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 3, color: '#e0e0e0' }}>
        Downloads
      </Typography>

      <SettingsCard title="Download Location">
        <SettingRow
          label="Default download folder"
          description={settings.browser.downloadsPath || 'System default (Downloads folder)'}
          control={
            <input
              type="text"
              value={settings.browser.downloadsPath}
              onChange={(e) => updateSetting('browser.downloadsPath', e.target.value)}
              placeholder="System default"
              style={{
                width: 240,
                padding: '6px 10px',
                border: '1px solid #444',
                borderRadius: 4,
                backgroundColor: '#2a2a2a',
                color: '#e0e0e0',
                fontSize: '0.85rem',
                outline: 'none',
              }}
              onFocus={(e) => (e.target.style.borderColor = '#a67c00')}
              onBlur={(e) => (e.target.style.borderColor = '#444')}
            />
          }
        />
      </SettingsCard>
    </Box>
  );
};

export default DownloadSettings;
