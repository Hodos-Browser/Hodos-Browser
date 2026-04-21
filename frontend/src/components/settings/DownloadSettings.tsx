import React, { useEffect, useCallback } from 'react';
import { Switch, Typography, Box, Button } from '@mui/material';
import FolderOpenIcon from '@mui/icons-material/FolderOpen';
import { SettingsCard, SettingRow } from './SettingsCard';
import { useSettings } from '../../hooks/useSettings';

declare global {
  interface Window {
    onDownloadFolderSelected?: (path: string) => void;
  }
}

const DownloadSettings: React.FC = () => {
  const { settings, updateSetting } = useSettings();

  // Listen for folder picker result from C++
  const handleFolderSelected = useCallback((path: string) => {
    if (path) {
      updateSetting('browser.downloadsPath', path);
    }
  }, [updateSetting]);

  useEffect(() => {
    window.onDownloadFolderSelected = handleFolderSelected;
    return () => {
      window.onDownloadFolderSelected = undefined;
    };
  }, [handleFolderSelected]);

  const handleBrowse = () => {
    if (window.cefMessage?.send) {
      window.cefMessage.send('download_browse_folder');
    }
  };

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
            <Button
              variant="outlined"
              size="small"
              startIcon={<FolderOpenIcon />}
              onClick={handleBrowse}
              sx={{
                color: '#a67c00',
                borderColor: '#a67c00',
                textTransform: 'none',
                fontSize: '0.8rem',
                '&:hover': { borderColor: '#c9a000', color: '#c9a000' },
              }}
            >
              Browse
            </Button>
          }
        />
        <SettingRow
          label="Ask where to save each file"
          description="When off, files download to the default folder without prompting"
          control={
            <Switch
              checked={settings.browser.askWhereToSave}
              onChange={(e) => updateSetting('browser.askWhereToSave', e.target.checked)}
              size="small"
            />
          }
        />
      </SettingsCard>
    </Box>
  );
};

export default DownloadSettings;
