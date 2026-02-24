import React, { useState, useEffect } from 'react';
import {
  Box,
  Typography,
  Switch,
  Divider,
  Collapse,
  List,
  ListItem,
  ListItemText,
  Chip,
  Button,
  IconButton,
} from '@mui/material';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import ExpandLessIcon from '@mui/icons-material/ExpandLess';
import {
  CheckCircle as CheckCircleIcon,
  DeleteSweep,
} from '@mui/icons-material';
import { usePrivacyShield } from '../hooks/usePrivacyShield';

interface PrivacyShieldPanelProps {
  domain: string;
}

const PrivacyShieldPanel: React.FC<PrivacyShieldPanelProps> = ({ domain }) => {
  const {
    masterEnabled,
    toggleMaster,
    adblockEnabled,
    adblockBlockedCount,
    toggleSiteAdblock,
    cookieBlockingEnabled,
    cookieBlockedCount,
    toggleCookieBlocking,
    blockedDomains,
    blockLog,
    fetchBlockList,
    fetchBlockLog,
    clearBlockLog,
    unblockDomain,
  } = usePrivacyShield(domain);

  const [domainsExpanded, setDomainsExpanded] = useState(false);
  const [logExpanded, setLogExpanded] = useState(false);

  // Fetch expandable data when expanded
  useEffect(() => {
    if (domainsExpanded) {
      fetchBlockList();
    }
  }, [domainsExpanded, fetchBlockList]);

  useEffect(() => {
    if (logExpanded) {
      fetchBlockLog(50, 0);
    }
  }, [logExpanded, fetchBlockLog]);

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

  const handleUnblock = async (d: string) => {
    try {
      await unblockDomain(d);
      await fetchBlockList();
    } catch {
      // ignore
    }
  };

  const handleClearLog = async () => {
    try {
      await clearBlockLog();
      await fetchBlockLog(50, 0);
    } catch {
      // ignore
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
        <Typography variant="body1" sx={{ fontWeight: 600, fontSize: '0.9rem' }}>
          Protection enabled
        </Typography>
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
          px: 2,
          py: 1.25,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <Typography sx={{ fontSize: '0.85rem' }}>
            {adblockBlockedCount > 0
              ? `${adblockBlockedCount} tracker${adblockBlockedCount !== 1 ? 's' : ''} blocked`
              : 'Tracker blocking'}
          </Typography>
        </Box>
        <Switch
          checked={adblockEnabled}
          onChange={handleAdblockToggle}
          disabled={!domain}
          size="small"
        />
      </Box>

      {/* Cookie blocking row */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          px: 2,
          py: 1.25,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <Typography sx={{ fontSize: '0.85rem' }}>
            {cookieBlockedCount > 0
              ? `${cookieBlockedCount} cookie${cookieBlockedCount !== 1 ? 's' : ''} blocked`
              : 'Cookie blocking'}
          </Typography>
        </Box>
        <Switch
          checked={cookieBlockingEnabled}
          onChange={handleCookieToggle}
          disabled={!domain}
          size="small"
        />
      </Box>

      <Divider />

      {/* Blocked domains expandable */}
      <Box
        onClick={() => setDomainsExpanded(!domainsExpanded)}
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
          Blocked domains ({blockedDomains.length})
        </Typography>
        {domainsExpanded ? (
          <ExpandLessIcon fontSize="small" sx={{ color: 'text.secondary' }} />
        ) : (
          <ExpandMoreIcon fontSize="small" sx={{ color: 'text.secondary' }} />
        )}
      </Box>
      <Collapse in={domainsExpanded}>
        <List sx={{ py: 0, py: 0 }}>
          {blockedDomains.length === 0 ? (
            <ListItem dense>
              <ListItemText
                secondary="No blocked domains"
                secondaryTypographyProps={{ fontSize: '0.75rem' }}
              />
            </ListItem>
          ) : (
            blockedDomains.map((blocked) => (
              <ListItem
                key={blocked.domain}
                dense
                secondaryAction={
                  <IconButton
                    size="small"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleUnblock(blocked.domain);
                    }}
                    title="Unblock"
                  >
                    <CheckCircleIcon fontSize="small" color="success" />
                  </IconButton>
                }
              >
                <ListItemText
                  primary={
                    <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
                      <Typography variant="body2" sx={{ fontSize: '0.78rem' }}>
                        {blocked.domain}
                      </Typography>
                      <Chip
                        label={blocked.source === 'default' ? 'Tracker' : 'User'}
                        size="small"
                        sx={{ height: 16, fontSize: '0.6rem' }}
                      />
                    </Box>
                  }
                />
              </ListItem>
            ))
          )}
        </List>
      </Collapse>

      <Divider />

      {/* Block log expandable */}
      <Box
        onClick={() => setLogExpanded(!logExpanded)}
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
          Block log
        </Typography>
        {logExpanded ? (
          <ExpandLessIcon fontSize="small" sx={{ color: 'text.secondary' }} />
        ) : (
          <ExpandMoreIcon fontSize="small" sx={{ color: 'text.secondary' }} />
        )}
      </Box>
      <Collapse in={logExpanded}>
        {blockLog.length > 0 && (
          <Box sx={{ display: 'flex', justifyContent: 'flex-end', px: 2, pt: 0.5 }}>
            <Button
              size="small"
              startIcon={<DeleteSweep />}
              onClick={handleClearLog}
              sx={{ fontSize: '0.7rem', textTransform: 'none' }}
            >
              Clear
            </Button>
          </Box>
        )}
        <List sx={{ py: 0, py: 0 }}>
          {blockLog.length === 0 ? (
            <ListItem dense>
              <ListItemText
                secondary="No blocking activity"
                secondaryTypographyProps={{ fontSize: '0.75rem' }}
              />
            </ListItem>
          ) : (
            blockLog.map((entry, idx) => (
              <ListItem key={idx} dense>
                <ListItemText
                  primary={
                    <Typography variant="body2" sx={{ fontSize: '0.78rem', fontWeight: 500 }}>
                      {entry.cookie_domain}
                    </Typography>
                  }
                  secondary={
                    <Box component="span" sx={{ display: 'block' }}>
                      <Typography variant="caption" component="span" display="block" color="text.secondary" sx={{ fontSize: '0.68rem' }}>
                        {entry.page_url.length > 45 ? entry.page_url.slice(0, 42) + '...' : entry.page_url}
                      </Typography>
                      <Box sx={{ display: 'flex', gap: 0.5, mt: 0.25 }}>
                        <Chip
                          label={entry.reason === 'blocked_domain' ? 'Domain' : '3rd-party'}
                          size="small"
                          sx={{ height: 14, fontSize: '0.58rem' }}
                        />
                        <Typography variant="caption" color="text.disabled" sx={{ fontSize: '0.6rem' }}>
                          {formatRelativeTime(entry.blocked_at)}
                        </Typography>
                      </Box>
                    </Box>
                  }
                />
              </ListItem>
            ))
          )}
        </List>
      </Collapse>
    </Box>
  );
};

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

export default PrivacyShieldPanel;
