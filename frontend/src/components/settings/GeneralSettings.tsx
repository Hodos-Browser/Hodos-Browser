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
          label="Homepage"
          description="The page that opens when you click the home button"
          control={
            <input
              type="text"
              value={settings.browser.homepage}
              onChange={(e) => updateSetting('browser.homepage', e.target.value)}
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
              <MenuItem value="google">Google</MenuItem>
              <MenuItem value="bing">Bing</MenuItem>
              <MenuItem value="duckduckgo">DuckDuckGo</MenuItem>
              <MenuItem value="brave">Brave Search</MenuItem>
            </Select>
          }
        />
      </SettingsCard>

      <SettingsCard title="Appearance">
        <SettingRow
          label="Show bookmark bar"
          description="Display the bookmark bar below the address bar"
          control={
            <Switch
              checked={settings.browser.showBookmarkBar}
              onChange={(e) => updateSetting('browser.showBookmarkBar', e.target.checked)}
              size="small"
            />
          }
        />
      </SettingsCard>
    </Box>
  );
};

export default GeneralSettings;
