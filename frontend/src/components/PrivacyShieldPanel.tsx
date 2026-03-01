import React from 'react';
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

interface PrivacyShieldPanelProps {
  domain: string;
}

const InfoTip: React.FC<{ tip: string }> = ({ tip }) => (
  <Tooltip title={tip} arrow placement="top" enterDelay={200}>
    <InfoOutlinedIcon sx={{ fontSize: 14, color: 'text.disabled', cursor: 'help', ml: 0.5 }} />
  </Tooltip>
);

const PrivacyShieldPanel: React.FC<PrivacyShieldPanelProps> = ({ domain }) => {
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
  } = usePrivacyShield(domain);

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
    <Box sx={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'auto' }}>
      {/* Domain display */}
      {domain && (
        <Box sx={{ px: 2, pt: 1.5, pb: 0.5 }}>
          <Typography variant="caption" color="text.secondary" sx={{ fontSize: '0.75rem' }}>
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
          py: 1.5,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center' }}>
          <Typography variant="body1" sx={{ fontWeight: 600, fontSize: '0.9rem' }}>
            Protection enabled
          </Typography>
          <InfoTip tip="Master switch for all privacy protections on this site." />
        </Box>
        <Switch
          checked={masterEnabled}
          onChange={handleMasterToggle}
          disabled={!domain}
          size="small"
        />
      </Box>

      <Divider />

      {/* Ad blocking row */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          pl: 3.5,
          pr: 2,
          py: 1.25,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center' }}>
          <Typography sx={{ fontSize: '0.85rem' }}>
            {adblockBlockedCount > 0
              ? `${adblockBlockedCount} tracker${adblockBlockedCount !== 1 ? 's' : ''} blocked`
              : 'Tracker blocking'}
          </Typography>
          <InfoTip tip="Blocks ads and tracking requests. Turning off may show more ads." />
        </Box>
        <Switch
          checked={adblockEnabled}
          onChange={handleAdblockToggle}
          disabled={!domain}
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
          py: 1.25,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center' }}>
          <Typography sx={{ fontSize: '0.85rem' }}>
            Scriptlet injection
          </Typography>
          <InfoTip tip="Overrides ad scripts on the page. Disable if a site behaves oddly." />
        </Box>
        <Switch
          checked={scriptletsEnabled}
          onChange={handleScriptletToggle}
          disabled={!domain || !adblockEnabled}
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
          py: 1.25,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center' }}>
          <Typography sx={{ fontSize: '0.85rem' }}>
            {cookieBlockedCount > 0
              ? `${cookieBlockedCount} cookie${cookieBlockedCount !== 1 ? 's' : ''} blocked`
              : 'Cookie blocking'}
          </Typography>
          <InfoTip tip="Blocks third-party tracking cookies. Disable if login fails." />
        </Box>
        <Switch
          checked={cookieBlockingEnabled}
          onChange={handleCookieToggle}
          disabled={!domain}
          size="small"
        />
      </Box>

      {/* Fingerprint protection row — always on, no toggle */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          pl: 3.5,
          pr: 2,
          py: 1.25,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center' }}>
          <Typography sx={{ fontSize: '0.85rem' }}>
            Fingerprint shield
          </Typography>
          <InfoTip tip="Randomizes your browser fingerprint to prevent cross-site tracking. Always on." />
        </Box>
        <Typography sx={{ fontSize: '0.75rem', color: 'text.secondary', fontStyle: 'italic' }}>
          Always on
        </Typography>
      </Box>

      <Divider />

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
          py: 1.25,
          cursor: 'pointer',
          '&:hover': { backgroundColor: 'action.hover' },
        }}
      >
        <Typography variant="body2" sx={{ fontSize: '0.85rem', color: 'text.secondary' }}>
          Privacy settings
        </Typography>
        <OpenInNewIcon fontSize="small" sx={{ color: 'text.secondary', fontSize: 16 }} />
      </Box>
    </Box>
  );
};

export default PrivacyShieldPanel;
