import React, { useState, useEffect } from 'react';
import { Switch, Typography, Box, Button, Chip, Collapse } from '@mui/material';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import OpenInNewIcon from '@mui/icons-material/OpenInNew';
import DeleteSweepIcon from '@mui/icons-material/DeleteSweep';
import { SettingsCard, SettingRow } from './SettingsCard';
import { useSettings } from '../../hooks/useSettings';
import { useCookieBlocking } from '../../hooks/useCookieBlocking';

function formatRelativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return 'Just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

const PrivacySettings: React.FC = () => {
  const [domainsExpanded, setDomainsExpanded] = useState(false);
  const [logExpanded, setLogExpanded] = useState(false);
  const { settings, updateSetting } = useSettings();
  const {
    blockedDomains,
    blockLog,
    fetchBlockList,
    fetchBlockLog,
    clearBlockLog,
    unblockDomain,
  } = useCookieBlocking();

  useEffect(() => {
    fetchBlockList();
    fetchBlockLog(100, 0);
  }, [fetchBlockList, fetchBlockLog]);

  const handleUnblock = async (domain: string) => {
    try {
      await unblockDomain(domain);
    } catch {
      // ignore
    }
  };

  const handleClearLog = async () => {
    try {
      await clearBlockLog();
      await fetchBlockLog(100, 0);
    } catch {
      // ignore
    }
  };

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 3, color: '#e0e0e0' }}>
        Privacy & Security
      </Typography>

      <SettingsCard title="Shields">
        <SettingRow
          label="Ad & tracker blocking"
          description="Block ads, trackers, and third-party scripts globally"
          control={
            <Switch
              checked={settings.privacy.adBlockEnabled}
              onChange={(e) => updateSetting('privacy.adBlockEnabled', e.target.checked)}
              size="small"
            />
          }
        />
        <SettingRow
          label="Third-party cookie blocking"
          description="Block cookies set by domains other than the site you're visiting"
          control={
            <Switch
              checked={settings.privacy.thirdPartyCookieBlocking}
              onChange={(e) => updateSetting('privacy.thirdPartyCookieBlocking', e.target.checked)}
              size="small"
            />
          }
        />
      </SettingsCard>

      <SettingsCard title="Fingerprinting">
        <SettingRow
          label="Fingerprint protection"
          description="Randomizes browser fingerprint (Canvas, WebGL, Audio) to prevent cross-site tracking"
          control={
            <Switch
              checked={settings.privacy.fingerprintProtection}
              onChange={(e) => updateSetting('privacy.fingerprintProtection', e.target.checked)}
              size="small"
            />
          }
        />
      </SettingsCard>

      <SettingsCard title="Tracking">
        <SettingRow
          label="Send 'Do Not Track' request"
          description="Ask websites not to track your browsing activity (DNT header)"
          control={
            <Switch
              checked={settings.privacy.doNotTrack}
              onChange={(e) => updateSetting('privacy.doNotTrack', e.target.checked)}
              size="small"
            />
          }
        />
      </SettingsCard>

      <SettingsCard title="">
        <Box
          onClick={() => setDomainsExpanded(!domainsExpanded)}
          sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', cursor: 'pointer', mx: -3, mt: -3, mb: domainsExpanded ? 1 : -3, px: 3, py: 1.5, borderRadius: domainsExpanded ? '8px 8px 0 0' : 2, '&:hover': { bgcolor: 'rgba(255,255,255,0.03)' } }}
        >
          <Typography sx={{ color: '#a67c00', fontSize: '1rem', fontWeight: 500 }}>
            Blocked Domains ({blockedDomains.length})
          </Typography>
          <ExpandMoreIcon sx={{ color: '#888', fontSize: 20, transform: domainsExpanded ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 0.2s' }} />
        </Box>
        <Collapse in={domainsExpanded}>
          {blockedDomains.length === 0 ? (
            <Typography sx={{ color: '#888', fontSize: '0.85rem' }}>No blocked domains</Typography>
          ) : (
            blockedDomains.map((entry) => (
              <Box key={entry.domain} sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', py: 1, borderBottom: '1px solid #333', '&:last-child': { borderBottom: 'none' } }}>
                <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                  <Typography sx={{ color: '#e0e0e0', fontSize: '0.88rem' }}>{entry.domain}</Typography>
                  <Chip label={entry.source === 'default' ? 'Tracker' : 'User'} size="small" sx={{ height: 20, fontSize: '0.7rem' }} />
                </Box>
                <Button size="small" onClick={() => handleUnblock(entry.domain)} sx={{ textTransform: 'none', color: '#a67c00' }}>
                  Unblock
                </Button>
              </Box>
            ))
          )}
        </Collapse>
      </SettingsCard>

      <SettingsCard title="">
        <Box
          onClick={() => setLogExpanded(!logExpanded)}
          sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', cursor: 'pointer', mx: -3, mt: -3, mb: logExpanded ? 1 : -3, px: 3, py: 1.5, borderRadius: logExpanded ? '8px 8px 0 0' : 2, '&:hover': { bgcolor: 'rgba(255,255,255,0.03)' } }}
        >
          <Typography sx={{ color: '#a67c00', fontSize: '1rem', fontWeight: 500 }}>
            Block Log ({blockLog.length})
          </Typography>
          <ExpandMoreIcon sx={{ color: '#888', fontSize: 20, transform: logExpanded ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 0.2s' }} />
        </Box>
        <Collapse in={logExpanded}>
          {blockLog.length > 0 && (
            <Box sx={{ display: 'flex', justifyContent: 'flex-end', mb: 1 }}>
              <Button size="small" startIcon={<DeleteSweepIcon />} onClick={handleClearLog} sx={{ textTransform: 'none', color: '#a67c00' }}>
                Clear
              </Button>
            </Box>
          )}
          {blockLog.length === 0 ? (
            <Typography sx={{ color: '#888', fontSize: '0.85rem' }}>No blocking activity</Typography>
          ) : (
            blockLog.map((entry, idx) => (
              <Box key={idx} sx={{ py: 1, borderBottom: '1px solid #333', '&:last-child': { borderBottom: 'none' } }}>
                <Typography sx={{ color: '#e0e0e0', fontSize: '0.88rem', fontWeight: 500 }}>{entry.cookie_domain}</Typography>
                <Typography sx={{ color: '#888', fontSize: '0.78rem', mt: 0.25 }}>
                  {entry.page_url.length > 80 ? entry.page_url.slice(0, 77) + '...' : entry.page_url}
                </Typography>
                <Box sx={{ display: 'flex', gap: 1, mt: 0.5, alignItems: 'center' }}>
                  <Chip label={entry.reason === 'blocked_domain' ? 'Domain' : '3rd-party'} size="small" sx={{ height: 18, fontSize: '0.65rem' }} />
                  <Typography sx={{ color: '#666', fontSize: '0.72rem' }}>{formatRelativeTime(entry.blocked_at)}</Typography>
                </Box>
              </Box>
            ))
          )}
        </Collapse>
      </SettingsCard>

      <SettingsCard title="Browsing Data">
        <SettingRow
          label="Clear data on exit"
          description="Automatically clear browsing history and cookies when closing the browser"
          control={
            <Switch
              checked={settings.privacy.clearDataOnExit}
              onChange={(e) => updateSetting('privacy.clearDataOnExit', e.target.checked)}
              size="small"
            />
          }
        />
        <Box
          onClick={() => window.cefMessage?.send('menu_action', ['history'])}
          sx={{
            mt: 2,
            display: 'flex',
            alignItems: 'center',
            gap: 1,
            cursor: 'pointer',
            color: '#a67c00',
            '&:hover': { textDecoration: 'underline' },
          }}
        >
          <Typography sx={{ fontSize: '0.88rem' }}>
            Manage browsing data
          </Typography>
          <OpenInNewIcon sx={{ fontSize: 14 }} />
        </Box>
      </SettingsCard>
    </Box>
  );
};

export default PrivacySettings;
