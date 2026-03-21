import React, { useEffect } from 'react';
import {
  Box,
  Typography,
  Switch,
  Divider,
  Tooltip,
} from '@mui/material';
import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';
import OpenInNewIcon from '@mui/icons-material/OpenInNew';
import { usePrivacyShield } from '../hooks/usePrivacyShield';
import { useSettings } from '../hooks/useSettings';

interface PrivacyShieldPanelProps {
  domain: string;
  showCount?: number;
}

const InfoTip: React.FC<{ tip: string }> = ({ tip }) => (
  <Tooltip title={tip} arrow placement="top" enterDelay={200}>
    <InfoOutlinedIcon sx={{ fontSize: 14, color: '#6b7280', cursor: 'help', ml: 0.5 }} />
  </Tooltip>
);

const PrivacyShieldPanel: React.FC<PrivacyShieldPanelProps> = ({ domain, showCount }) => {
  const {
    masterEnabled,
    toggleMaster,
    adblockEnabled,
    adblockBlockedCount,
    toggleSiteAdblock,
    scriptletsEnabled,
    toggleScriptlets,
    cookieBlockingEnabled,
    cookieBlockedCount,
    toggleCookieBlocking,
    fingerprintSiteEnabled,
    toggleFingerprintSite,
    fingerprintNeedsReload,
  } = usePrivacyShield(domain);

  const { settings, refresh } = useSettings();

  // Re-fetch global settings each time the panel is shown
  // showCount increments on every open, even if domain is the same
  useEffect(() => {
    if (domain) {
      refresh();
    }
  }, [domain, showCount, refresh]);

  // Global toggles from Settings > Privacy — when OFF, per-site toggles are ineffective
  const globalAdblockOff = !settings.privacy.adBlockEnabled;
  const globalCookieOff = !settings.privacy.thirdPartyCookieBlocking;
  const globalFingerprintEnabled = settings?.privacy?.fingerprintProtection !== false;
  const globalOverrideText = 'Disabled globally in Privacy Settings. Per-site toggle has no effect.';

  const handleMasterToggle = () => {
    if (domain) {
      toggleMaster(domain, !masterEnabled);
    }
  };

  const handleAdblockToggle = () => {
    if (domain) {
      toggleSiteAdblock(domain, !adblockEnabled);
    }
  };

  const handleCookieToggle = () => {
    if (domain) {
      toggleCookieBlocking(domain, !cookieBlockingEnabled);
    }
  };

  const handleScriptletToggle = () => {
    if (domain) {
      toggleScriptlets(domain, !scriptletsEnabled);
    }
  };

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'auto', backgroundColor: '#1a1d23' }}>
      {/* Domain display */}
      {domain && (
        <Box sx={{ px: 2, pt: 1.5, pb: 0.5 }}>
          <Typography variant="caption" sx={{ fontSize: '0.75rem', color: '#9ca3af' }}>
            {domain}
          </Typography>
        </Box>
      )}

      {/* Master toggle */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          px: 2,
          py: 1,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center' }}>
          <Typography variant="body1" sx={{ fontWeight: 600, fontSize: '0.9rem', color: '#f0f0f0' }}>
            Protection enabled
          </Typography>
          <InfoTip tip={globalAdblockOff && globalCookieOff
            ? globalOverrideText
            : "Master switch for all privacy protections on this site."} />
        </Box>
        <Switch
          checked={masterEnabled}
          onChange={handleMasterToggle}
          disabled={!domain || (globalAdblockOff && globalCookieOff)}
          size="small"
        />
      </Box>

      <Divider sx={{ borderColor: '#2a2d35' }} />

      {/* Ad blocking row */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          pl: 3.5,
          pr: 2,
          py: 0.75,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center' }}>
          <Typography sx={{ fontSize: '0.85rem', color: globalAdblockOff ? '#6b7280' : '#f0f0f0' }}>
            {adblockBlockedCount > 0 && !globalAdblockOff
              ? `${adblockBlockedCount} tracker${adblockBlockedCount !== 1 ? 's' : ''} blocked`
              : 'Tracker blocking'}
          </Typography>
          <InfoTip tip={globalAdblockOff ? globalOverrideText : "Blocks ads and tracking requests. Turning off may show more ads."} />
        </Box>
        <Switch
          checked={adblockEnabled && !globalAdblockOff}
          onChange={handleAdblockToggle}
          disabled={!domain || globalAdblockOff}
          size="small"
        />
      </Box>

      {/* Scriptlet injection row */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          pl: 3.5,
          pr: 2,
          py: 0.75,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center' }}>
          <Typography sx={{ fontSize: '0.85rem', color: globalAdblockOff ? '#6b7280' : '#f0f0f0' }}>
            Scriptlet injection
          </Typography>
          <InfoTip tip={globalAdblockOff ? globalOverrideText : "Overrides ad scripts on the page. Disable if a site behaves oddly."} />
        </Box>
        <Switch
          checked={scriptletsEnabled && !globalAdblockOff}
          onChange={handleScriptletToggle}
          disabled={!domain || !adblockEnabled || globalAdblockOff}
          size="small"
        />
      </Box>

      {/* Cookie blocking row */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          pl: 3.5,
          pr: 2,
          py: 0.75,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center' }}>
          <Typography sx={{ fontSize: '0.85rem', color: globalCookieOff ? '#6b7280' : '#f0f0f0' }}>
            {cookieBlockedCount > 0 && !globalCookieOff
              ? `${cookieBlockedCount} cookie${cookieBlockedCount !== 1 ? 's' : ''} blocked`
              : 'Cookie blocking'}
          </Typography>
          <InfoTip tip={globalCookieOff ? globalOverrideText : "Blocks third-party tracking cookies. Disable if login fails."} />
        </Box>
        <Switch
          checked={cookieBlockingEnabled && !globalCookieOff}
          onChange={handleCookieToggle}
          disabled={!domain || globalCookieOff}
          size="small"
        />
      </Box>

      {/* Fingerprint protection row — per-site toggle */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          pl: 3.5,
          pr: 2,
          py: 0.75,
        }}
      >
        <Box>
          <Box sx={{ display: 'flex', alignItems: 'center' }}>
            <Typography sx={{ fontSize: '0.85rem', color: '#f0f0f0' }}>
              Fingerprint shield
            </Typography>
            <InfoTip tip="Randomizes your browser fingerprint to prevent cross-site tracking. Disable for sites that break with fingerprinting on." />
          </Box>
          {!globalFingerprintEnabled ? (
            <Typography sx={{ fontSize: '0.7rem', color: '#9ca3af', fontStyle: 'italic' }}>
              Disabled in settings
            </Typography>
          ) : fingerprintNeedsReload ? (
            <Typography sx={{ fontSize: '0.7rem', color: '#e6a200', fontStyle: 'italic' }}>
              Reload page to apply
            </Typography>
          ) : null}
        </Box>
        <Switch
          size="small"
          checked={fingerprintSiteEnabled}
          onChange={() => toggleFingerprintSite(domain, !fingerprintSiteEnabled)}
          disabled={!globalFingerprintEnabled}
          sx={{
            '& .MuiSwitch-switchBase.Mui-checked': { color: '#a67c00' },
            '& .MuiSwitch-switchBase.Mui-checked + .MuiSwitch-track': { backgroundColor: '#a67c00' },
          }}
        />
      </Box>

      <Divider sx={{ borderColor: '#2a2d35' }} />

      {/* Link to full privacy settings */}
      <Box
        onClick={() => {
          window.cefMessage?.send('cookie_panel_hide', []);
          window.cefMessage?.send('menu_action', ['settings_privacy']);
        }}
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          px: 2,
          py: 0.75,
          cursor: 'pointer',
          '&:hover': { backgroundColor: '#1f2937' },
        }}
      >
        <Typography variant="body2" sx={{ fontSize: '0.85rem', color: '#9ca3af' }}>
          Privacy settings
        </Typography>
        <OpenInNewIcon fontSize="small" sx={{ color: '#9ca3af', fontSize: 16 }} />
      </Box>
    </Box>
  );
};

export default PrivacyShieldPanel;
