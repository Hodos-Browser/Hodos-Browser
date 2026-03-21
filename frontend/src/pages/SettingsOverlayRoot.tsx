import React, { useEffect, useState } from 'react';
import {
  Box,
  Typography,
  Tabs,
  Tab,
  Switch,
  TextField,
  Select,
  MenuItem,
  Divider,
  Slider,
  FormControl,
  CircularProgress,
  Button,
  Card,
  CardContent,
  CardActions,
  Alert,
  Chip,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import SettingsIcon from '@mui/icons-material/Settings';
import SecurityIcon from '@mui/icons-material/Security';
import AccountBalanceWalletIcon from '@mui/icons-material/AccountBalanceWallet';
import WebIcon from '@mui/icons-material/Web';
import FileDownloadIcon from '@mui/icons-material/FileDownload';
import RefreshIcon from '@mui/icons-material/Refresh';
import { useSettings } from '../hooks/useSettings';
import { useImport } from '../hooks/useImport';
import { HodosButton } from '../components/HodosButton';

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
}

function TabPanel({ children, value, index }: TabPanelProps) {
  return (
    <Box
      role="tabpanel"
      hidden={value !== index}
      sx={{
        p: 2,
        height: '100%',
        overflow: 'auto',
        backgroundColor: '#0f1117',
        color: '#f0f0f0',
        '& .MuiTypography-body2': { color: '#f0f0f0' },
        '& .MuiTypography-caption': { color: '#9ca3af' },
        '& .MuiTypography-subtitle2': { color: '#f0f0f0' },
        '& .MuiDivider-root': { borderColor: '#2a2d35' },
        '& .MuiSwitch-root .MuiSwitch-track': { backgroundColor: '#6b7280' },
        '& .MuiSlider-root': { color: '#a67c00' },
        '& .MuiSlider-markLabel': { color: '#6b7280' },
        '& .MuiTextField-root .MuiOutlinedInput-root': {
          color: '#f0f0f0',
          '& fieldset': { borderColor: '#2a2d35' },
          '&:hover fieldset': { borderColor: '#a67c00' },
          '&.Mui-focused fieldset': { borderColor: '#a67c00' },
        },
        '& .MuiTextField-root .MuiInputBase-input': { color: '#f0f0f0' },
        '& .MuiSelect-select': { color: '#f0f0f0' },
        '& .MuiOutlinedInput-notchedOutline': { borderColor: '#2a2d35' },
        '& .MuiSelect-icon': { color: '#9ca3af' },
        '& .MuiCard-root': { backgroundColor: '#1a1d23', borderColor: '#2a2d35' },
        '& .MuiCardContent-root .MuiTypography-root': { color: '#f0f0f0' },
        '& .MuiChip-outlined': { borderColor: '#2a2d35', color: '#9ca3af' },
        '& .MuiButton-root': { color: '#a67c00' },
        '& .MuiButton-contained': { backgroundColor: '#a67c00', color: '#fff' },
        '& .MuiAlert-root': { backgroundColor: '#1a1d23' },
        '& .MuiCircularProgress-root': { color: '#a67c00' },
        '& .MuiIconButton-root': { color: '#9ca3af' },
      }}
    >
      {value === index && children}
    </Box>
  );
}

const SettingsOverlayRoot: React.FC = () => {
  const [tabIndex, setTabIndex] = useState(0);
  const { settings, loading, updateSetting } = useSettings();
  const { 
    profiles, 
    loading: importLoading, 
    importing, 
    lastResult, 
    refresh: refreshProfiles,
    importBookmarks,
    importHistory,
    importAll,
  } = useImport();

  useEffect(() => {
    document.body.setAttribute('data-overlay', 'settings');
    return () => {
      document.body.removeAttribute('data-overlay');
    };
  }, []);

  const handleClose = () => {
    if (window.cefMessage?.send) {
      window.cefMessage.send('settings_close');
    }
  };

  const handleTabChange = (_: React.SyntheticEvent, newValue: number) => {
    setTabIndex(newValue);
  };

  // Format cents to dollars for display
  const centsToDollars = (cents: number): string => {
    return `$${(cents / 100).toFixed(2)}`;
  };

  // Common style for settings sections
  const sectionStyle = {
    mb: 3,
    '& > .MuiTypography-root': {
      mb: 1.5,
      fontWeight: 600,
      color: '#f0f0f0',
    },
  };

  const settingRowStyle = {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    py: 1,
    '&:not(:last-child)': {
      borderBottom: '1px solid',
      borderColor: '#2a2d35',
    },
  };

  if (loading) {
    return (
      <Box
        sx={{
          width: '100%',
          height: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: '#0f1117',
        }}
      >
        <CircularProgress size={32} sx={{ color: '#a67c00' }} />
      </Box>
    );
  }

  return (
    <Box
      sx={{
        width: '100%',
        height: '100%',
        backgroundColor: '#0f1117',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        boxShadow: '-2px 0 8px rgba(0, 0, 0, 0.2)',
      }}
    >
      {/* Header */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          p: 2,
          pb: 0,
          backgroundColor: '#111827',
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <SettingsIcon fontSize="small" sx={{ color: '#9ca3af' }} />
          <Typography variant="h6" sx={{ fontWeight: 600, color: '#f0f0f0' }}>
            Settings
          </Typography>
        </Box>
        <HodosButton variant="icon" size="small" onClick={handleClose} aria-label="Close">
          <CloseIcon sx={{ fontSize: 16 }} />
        </HodosButton>
      </Box>

      {/* Tabs */}
      <Tabs
        value={tabIndex}
        onChange={handleTabChange}
        variant="fullWidth"
        sx={{
          borderBottom: 1,
          borderColor: '#2a2d35',
          minHeight: 48,
          backgroundColor: '#111827',
          '& .MuiTab-root': {
            minHeight: 48,
            textTransform: 'none',
            fontWeight: 500,
            color: '#9ca3af',
            '&.Mui-selected': {
              color: '#f0f0f0',
            },
          },
          '& .MuiTabs-indicator': {
            backgroundColor: '#a67c00',
          },
        }}
      >
        <Tab icon={<WebIcon fontSize="small" />} label="Browser" iconPosition="start" />
        <Tab icon={<SecurityIcon fontSize="small" />} label="Privacy" iconPosition="start" />
        <Tab icon={<AccountBalanceWalletIcon fontSize="small" />} label="Wallet" iconPosition="start" />
        <Tab icon={<FileDownloadIcon fontSize="small" />} label="Import" iconPosition="start" />
      </Tabs>

      {/* Browser Settings */}
      <TabPanel value={tabIndex} index={0}>
        <Box sx={sectionStyle}>
          <Typography variant="subtitle2">Startup</Typography>
          
          <Box sx={settingRowStyle}>
            <Typography variant="body2">Homepage</Typography>
            <TextField
              size="small"
              value={settings.browser.homepage}
              onChange={(e) => updateSetting('browser.homepage', e.target.value)}
              placeholder="about:blank"
              sx={{ width: 180 }}
            />
          </Box>

          <Box sx={settingRowStyle}>
            <Typography variant="body2">Restore session on start</Typography>
            <Switch
              checked={settings.browser.restoreSessionOnStart}
              onChange={(e) => updateSetting('browser.restoreSessionOnStart', e.target.checked)}
              size="small"
            />
          </Box>
        </Box>

        <Divider sx={{ my: 2 }} />

        <Box sx={sectionStyle}>
          <Typography variant="subtitle2">Search</Typography>
          
          <Box sx={settingRowStyle}>
            <Typography variant="body2">Search engine</Typography>
            <FormControl size="small" sx={{ width: 180 }}>
              <Select
                value={settings.browser.searchEngine}
                onChange={(e) => updateSetting('browser.searchEngine', e.target.value)}
              >
                <MenuItem value="google">Google</MenuItem>
                <MenuItem value="bing">Bing</MenuItem>
                <MenuItem value="duckduckgo">DuckDuckGo</MenuItem>
                <MenuItem value="brave">Brave Search</MenuItem>
              </Select>
            </FormControl>
          </Box>
        </Box>

        <Divider sx={{ my: 2 }} />

        <Box sx={sectionStyle}>
          <Typography variant="subtitle2">Appearance</Typography>
          
          <Box sx={settingRowStyle}>
            <Typography variant="body2">Show bookmark bar</Typography>
            <Switch
              checked={settings.browser.showBookmarkBar}
              onChange={(e) => updateSetting('browser.showBookmarkBar', e.target.checked)}
              size="small"
            />
          </Box>

          <Box sx={{ py: 1 }}>
            <Typography variant="body2" gutterBottom>
              Zoom level: {(settings.browser.zoomLevel * 100).toFixed(0)}%
            </Typography>
            <Slider
              value={settings.browser.zoomLevel}
              onChange={(_, value) => updateSetting('browser.zoomLevel', value as number)}
              min={-0.5}
              max={0.5}
              step={0.1}
              marks={[
                { value: -0.5, label: '50%' },
                { value: 0, label: '100%' },
                { value: 0.5, label: '150%' },
              ]}
              size="small"
            />
          </Box>
        </Box>
      </TabPanel>

      {/* Privacy Settings */}
      <TabPanel value={tabIndex} index={1}>
        <Box sx={sectionStyle}>
          <Typography variant="subtitle2">Ad & Tracker Blocking</Typography>
          
          <Box sx={settingRowStyle}>
            <Box>
              <Typography variant="body2">Block ads</Typography>
              <Typography variant="caption" sx={{ color: '#9ca3af' }}>
                Block ads and trackers using privacy lists
              </Typography>
            </Box>
            <Switch
              checked={settings.privacy.adBlockEnabled}
              onChange={(e) => updateSetting('privacy.adBlockEnabled', e.target.checked)}
              size="small"
            />
          </Box>

          <Box sx={settingRowStyle}>
            <Box>
              <Typography variant="body2">Block third-party cookies</Typography>
              <Typography variant="caption" sx={{ color: '#9ca3af' }}>
                Prevent cross-site tracking
              </Typography>
            </Box>
            <Switch
              checked={settings.privacy.thirdPartyCookieBlocking}
              onChange={(e) => updateSetting('privacy.thirdPartyCookieBlocking', e.target.checked)}
              size="small"
            />
          </Box>
        </Box>

        <Divider sx={{ my: 2 }} />

        <Box sx={sectionStyle}>
          <Typography variant="subtitle2">Privacy Headers</Typography>
          
          <Box sx={settingRowStyle}>
            <Box>
              <Typography variant="body2">Send "Do Not Track"</Typography>
              <Typography variant="caption" sx={{ color: '#9ca3af' }}>
                Request sites not to track you
              </Typography>
            </Box>
            <Switch
              checked={settings.privacy.doNotTrack}
              onChange={(e) => updateSetting('privacy.doNotTrack', e.target.checked)}
              size="small"
            />
          </Box>
        </Box>

        <Divider sx={{ my: 2 }} />

        <Box sx={sectionStyle}>
          <Typography variant="subtitle2">On Exit</Typography>
          
          <Box sx={settingRowStyle}>
            <Box>
              <Typography variant="body2">Clear browsing data on exit</Typography>
              <Typography variant="caption" sx={{ color: '#9ca3af' }}>
                Clear history, cookies, and cache when closing
              </Typography>
            </Box>
            <Switch
              checked={settings.privacy.clearDataOnExit}
              onChange={(e) => updateSetting('privacy.clearDataOnExit', e.target.checked)}
              size="small"
            />
          </Box>
        </Box>
      </TabPanel>

      {/* Wallet Settings */}
      <TabPanel value={tabIndex} index={2}>
        <Box sx={sectionStyle}>
          <Typography variant="subtitle2">Auto-Approval</Typography>
          
          <Box sx={settingRowStyle}>
            <Box>
              <Typography variant="body2">Enable auto-approval</Typography>
              <Typography variant="caption" sx={{ color: '#9ca3af' }}>
                Automatically approve small transactions
              </Typography>
            </Box>
            <Switch
              checked={settings.wallet.autoApproveEnabled}
              onChange={(e) => updateSetting('wallet.autoApproveEnabled', e.target.checked)}
              size="small"
            />
          </Box>
        </Box>

        <Divider sx={{ my: 2 }} />

        <Box sx={sectionStyle}>
          <Typography variant="subtitle2">Spending Limits</Typography>
          
          <Box sx={{ py: 1 }}>
            <Typography variant="body2" gutterBottom>
              Per-transaction limit: {centsToDollars(settings.wallet.defaultPerTxLimitCents)}
            </Typography>
            <Slider
              value={settings.wallet.defaultPerTxLimitCents}
              onChange={(_, value) => updateSetting('wallet.defaultPerTxLimitCents', value as number)}
              min={1}
              max={100}
              step={1}
              marks={[
                { value: 1, label: '$0.01' },
                { value: 50, label: '$0.50' },
                { value: 100, label: '$1.00' },
              ]}
              size="small"
              disabled={!settings.wallet.autoApproveEnabled}
            />
          </Box>

          <Box sx={{ py: 1 }}>
            <Typography variant="body2" gutterBottom>
              Per-session limit: {centsToDollars(settings.wallet.defaultPerSessionLimitCents)}
            </Typography>
            <Slider
              value={settings.wallet.defaultPerSessionLimitCents}
              onChange={(_, value) => updateSetting('wallet.defaultPerSessionLimitCents', value as number)}
              min={100}
              max={1000}
              step={50}
              marks={[
                { value: 100, label: '$1' },
                { value: 500, label: '$5' },
                { value: 1000, label: '$10' },
              ]}
              size="small"
              disabled={!settings.wallet.autoApproveEnabled}
            />
          </Box>

          <Box sx={{ py: 1 }}>
            <Typography variant="body2" gutterBottom>
              Rate limit: {settings.wallet.defaultRateLimitPerMin} tx/min
            </Typography>
            <Slider
              value={settings.wallet.defaultRateLimitPerMin}
              onChange={(_, value) => updateSetting('wallet.defaultRateLimitPerMin', value as number)}
              min={1}
              max={30}
              step={1}
              marks={[
                { value: 1, label: '1' },
                { value: 15, label: '15' },
                { value: 30, label: '30' },
              ]}
              size="small"
              disabled={!settings.wallet.autoApproveEnabled}
            />
          </Box>
        </Box>
      </TabPanel>

      {/* Import Data */}
      <TabPanel value={tabIndex} index={3}>
        <Box sx={sectionStyle}>
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2 }}>
            <Typography variant="subtitle2">Import from Another Browser</Typography>
            <HodosButton variant="icon" size="small" onClick={refreshProfiles} disabled={importLoading} aria-label="Refresh profiles">
              <RefreshIcon sx={{ fontSize: 16 }} />
            </HodosButton>
          </Box>

          {importLoading ? (
            <Box sx={{ display: 'flex', justifyContent: 'center', py: 4 }}>
              <CircularProgress size={24} />
            </Box>
          ) : profiles.length === 0 ? (
            <Typography variant="body2" sx={{ py: 2, color: '#9ca3af' }}>
              No browser profiles detected. Make sure Chrome, Brave, or Edge is installed.
            </Typography>
          ) : (
            profiles.map((profile) => (
              <Card 
                key={profile.profilePath} 
                variant="outlined" 
                sx={{ mb: 2 }}
              >
                <CardContent sx={{ pb: 1 }}>
                  <Typography variant="subtitle2" gutterBottom>
                    {profile.browserName}
                  </Typography>
                  <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
                    {profile.hasBookmarks && (
                      <Chip 
                        size="small" 
                        label={`${profile.bookmarkCount} bookmarks`}
                        variant="outlined"
                      />
                    )}
                    {profile.hasHistory && (
                      <Chip 
                        size="small" 
                        label={`${profile.historyCount.toLocaleString()} history`}
                        variant="outlined"
                      />
                    )}
                  </Box>
                </CardContent>
                <CardActions sx={{ pt: 0 }}>
                  {profile.hasBookmarks && (
                    <Button 
                      size="small"
                      onClick={() => importBookmarks(profile.profilePath)}
                      disabled={importing}
                    >
                      Bookmarks
                    </Button>
                  )}
                  {profile.hasHistory && (
                    <Button 
                      size="small"
                      onClick={() => importHistory(profile.profilePath)}
                      disabled={importing}
                    >
                      History
                    </Button>
                  )}
                  {profile.hasBookmarks && profile.hasHistory && (
                    <Button 
                      size="small"
                      variant="contained"
                      onClick={() => importAll(profile.profilePath)}
                      disabled={importing}
                    >
                      Import All
                    </Button>
                  )}
                </CardActions>
              </Card>
            ))
          )}

          {importing && (
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mt: 2 }}>
              <CircularProgress size={16} />
              <Typography variant="body2" sx={{ color: '#9ca3af' }}>
                Importing...
              </Typography>
            </Box>
          )}

          {lastResult && (
            <Alert 
              severity={lastResult.success ? 'success' : 'error'} 
              sx={{ mt: 2 }}
            >
              {lastResult.success ? (
                <>
                  Imported {lastResult.bookmarksImported} bookmarks
                  {lastResult.foldersImported > 0 && ` (${lastResult.foldersImported} folders)`}
                  {lastResult.historyImported > 0 && `, ${lastResult.historyImported} history entries`}
                  {lastResult.skipped > 0 && `. Skipped ${lastResult.skipped} duplicates.`}
                </>
              ) : (
                lastResult.error
              )}
            </Alert>
          )}
        </Box>
      </TabPanel>
    </Box>
  );
};

export default SettingsOverlayRoot;
