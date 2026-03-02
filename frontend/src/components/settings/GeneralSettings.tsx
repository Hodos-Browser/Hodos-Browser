import React from 'react';
import { Switch, Select, MenuItem, Typography, Box } from '@mui/material';
import { SettingsCard, SettingRow } from './SettingsCard';
import { useSettings } from '../../hooks/useSettings';

const GeneralSettings: React.FC = () => {
  const { settings, updateSetting } = useSettings();

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 3, color: '#e0e0e0' }}>
        General
      </Typography>

      <SettingsCard title="Startup">
        <SettingRow
          label="Restore previous session"
          description="Reopen tabs from your last browsing session on startup"
          control={
            <Switch
              checked={settings.browser.restoreSessionOnStart}
              onChange={(e) => updateSetting('browser.restoreSessionOnStart', e.target.checked)}
              size="small"
            />
          }
        />
      </SettingsCard>

      <SettingsCard title="Search Engine">
        <SettingRow
          label="Default search engine"
          description="Used when typing in the address bar"
          control={
            <Select
              value={settings.browser.searchEngine}
              onChange={(e) => updateSetting('browser.searchEngine', e.target.value)}
              size="small"
              sx={{
                minWidth: 160,
                fontSize: '0.85rem',
                color: '#e0e0e0',
                '.MuiOutlinedInput-notchedOutline': { borderColor: '#444' },
                '&:hover .MuiOutlinedInput-notchedOutline': { borderColor: '#666' },
                '&.Mui-focused .MuiOutlinedInput-notchedOutline': { borderColor: '#a67c00' },
                '.MuiSvgIcon-root': { color: '#888' },
              }}
              MenuProps={{
                PaperProps: { sx: { bgcolor: '#2a2a2a', color: '#e0e0e0' } },
              }}
            >
              <MenuItem value="duckduckgo">DuckDuckGo</MenuItem>
              <MenuItem value="google">Google</MenuItem>
            </Select>
          }
        />
      </SettingsCard>
    </Box>
  );
};

export default GeneralSettings;
