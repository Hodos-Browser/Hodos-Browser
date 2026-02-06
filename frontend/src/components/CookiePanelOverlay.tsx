import { useState, useEffect } from 'react';
import {
  Box,
  Typography,
  TextField,
  List,
  ListItem,
  ListItemText,
  IconButton,
  Chip,
  Divider,
  InputAdornment,
  Tabs,
  Tab,
  Button,
  Snackbar,
  Alert,
} from '@mui/material';
import {
  Search as SearchIcon,
  Block as BlockIcon,
  CheckCircle as CheckCircleIcon,
  DeleteSweep,
} from '@mui/icons-material';
import { useCookies } from '../hooks/useCookies';
import { useCookieBlocking } from '../hooks/useCookieBlocking';

/**
 * Compact cookie management overlay optimized for 450px width panel.
 * Shows cookies by domain with quick block/unblock actions.
 */
export function CookiePanelOverlay() {
  const {
    domainGroups,
    loading,
    fetchAllCookies,
    deleteDomainCookies,
  } = useCookies();

  const {
    blockedDomains,
    blockLog,
    fetchBlockList,
    fetchBlockLog,
    blockDomain,
    unblockDomain,
    clearBlockLog,
  } = useCookieBlocking();

  const [searchQuery, setSearchQuery] = useState('');
  const [activeTab, setActiveTab] = useState(0);
  const [toast, setToast] = useState<{ message: string; severity: 'success' | 'error' } | null>(null);

  useEffect(() => {
    fetchAllCookies();
    fetchBlockList();
    fetchBlockLog(50, 0);
  }, [fetchAllCookies, fetchBlockList, fetchBlockLog]);

  // Filter domain groups by search query
  const filteredDomains = domainGroups.filter(group =>
    group.domain.toLowerCase().includes(searchQuery.toLowerCase())
  );

  // Check if domain is blocked
  const isDomainBlocked = (domain: string) => {
    return blockedDomains.some(b => b.domain === domain);
  };

  const handleBlockDomain = async (domain: string) => {
    try {
      await blockDomain(domain, false);
      await fetchBlockList();
      setToast({ message: `Blocked ${domain}`, severity: 'success' });
    } catch (err) {
      setToast({ message: 'Failed to block domain', severity: 'error' });
    }
  };

  const handleUnblockDomain = async (domain: string) => {
    try {
      await unblockDomain(domain);
      await fetchBlockList();
      setToast({ message: `Unblocked ${domain}`, severity: 'success' });
    } catch (err) {
      setToast({ message: 'Failed to unblock domain', severity: 'error' });
    }
  };

  const handleDeleteDomain = async (domain: string) => {
    try {
      await deleteDomainCookies(domain);
      await fetchAllCookies();
      setToast({ message: `Deleted cookies from ${domain}`, severity: 'success' });
    } catch (err) {
      setToast({ message: 'Failed to delete cookies', severity: 'error' });
    }
  };

  const handleClearBlockLog = async () => {
    try {
      await clearBlockLog();
      await fetchBlockLog(50, 0);
      setToast({ message: 'Block log cleared', severity: 'success' });
    } catch (err) {
      setToast({ message: 'Failed to clear log', severity: 'error' });
    }
  };

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      {/* Tabs */}
      <Tabs
        value={activeTab}
        onChange={(_, newValue) => setActiveTab(newValue)}
        variant="fullWidth"
        sx={{ borderBottom: 1, borderColor: 'divider', flexShrink: 0 }}
      >
        <Tab label={`Cookies (${domainGroups.length})`} />
        <Tab label={`Blocked (${blockedDomains.length})`} />
        <Tab label="Log" />
      </Tabs>

      {/* Tab 0: Cookies by Domain */}
      {activeTab === 0 && (
        <Box sx={{ display: 'flex', flexDirection: 'column', flex: 1, overflow: 'hidden' }}>
          {/* Search */}
          <Box sx={{ p: 2, flexShrink: 0 }}>
            <TextField
              fullWidth
              size="small"
              placeholder="Search domains..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              InputProps={{
                startAdornment: (
                  <InputAdornment position="start">
                    <SearchIcon fontSize="small" />
                  </InputAdornment>
                ),
              }}
            />
          </Box>

          {/* Domain list */}
          <List sx={{ flex: 1, overflow: 'auto', py: 0 }}>
            {loading ? (
              <ListItem>
                <ListItemText primary="Loading..." />
              </ListItem>
            ) : filteredDomains.length === 0 ? (
              <ListItem>
                <ListItemText
                  primary="No cookies found"
                  secondary={searchQuery ? 'Try a different search' : 'Browse websites to collect cookies'}
                />
              </ListItem>
            ) : (
              filteredDomains.map((group) => {
                const isBlocked = isDomainBlocked(group.domain);
                return (
                  <Box key={group.domain}>
                    <ListItem
                      sx={{
                        bgcolor: isBlocked ? 'rgba(211, 47, 47, 0.04)' : 'transparent',
                      }}
                      secondaryAction={
                        <Box>
                          <IconButton
                            size="small"
                            onClick={() => isBlocked ? handleUnblockDomain(group.domain) : handleBlockDomain(group.domain)}
                            color={isBlocked ? 'success' : 'error'}
                            title={isBlocked ? 'Unblock domain' : 'Block domain'}
                          >
                            {isBlocked ? <CheckCircleIcon fontSize="small" /> : <BlockIcon fontSize="small" />}
                          </IconButton>
                          <IconButton
                            size="small"
                            onClick={() => handleDeleteDomain(group.domain)}
                            title="Delete all cookies from domain"
                          >
                            <DeleteSweep fontSize="small" />
                          </IconButton>
                        </Box>
                      }
                    >
                      <ListItemText
                        primary={
                          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                            <Typography variant="body2" sx={{ fontWeight: 500 }}>
                              {group.domain}
                            </Typography>
                            {isBlocked && (
                              <Chip label="Blocked" size="small" color="error" sx={{ height: 20, fontSize: '0.7rem' }} />
                            )}
                          </Box>
                        }
                        secondary={`${group.count} cookie${group.count !== 1 ? 's' : ''}`}
                      />
                    </ListItem>
                    <Divider />
                  </Box>
                );
              })
            )}
          </List>
        </Box>
      )}

      {/* Tab 1: Blocked Domains */}
      {activeTab === 1 && (
        <Box sx={{ display: 'flex', flexDirection: 'column', flex: 1, overflow: 'hidden' }}>
          <List sx={{ flex: 1, overflow: 'auto', py: 0 }}>
            {blockedDomains.length === 0 ? (
              <ListItem>
                <ListItemText
                  primary="No blocked domains"
                  secondary="Block domains from the Cookies tab"
                />
              </ListItem>
            ) : (
              blockedDomains.map((blocked) => (
                <Box key={blocked.domain}>
                  <ListItem
                    secondaryAction={
                      <IconButton
                        size="small"
                        onClick={() => handleUnblockDomain(blocked.domain)}
                        color="success"
                        title="Unblock domain"
                      >
                        <CheckCircleIcon fontSize="small" />
                      </IconButton>
                    }
                  >
                    <ListItemText
                      primary={
                        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                          <Typography variant="body2" sx={{ fontWeight: 500 }}>
                            {blocked.domain}
                          </Typography>
                          <Chip
                            label={blocked.source === 'default' ? 'Tracker' : 'User'}
                            size="small"
                            color={blocked.source === 'default' ? 'warning' : 'primary'}
                            sx={{ height: 20, fontSize: '0.7rem' }}
                          />
                        </Box>
                      }
                      secondary={`Blocked since ${new Date(blocked.created_at).toLocaleDateString()}`}
                    />
                  </ListItem>
                  <Divider />
                </Box>
              ))
            )}
          </List>
        </Box>
      )}

      {/* Tab 2: Block Log */}
      {activeTab === 2 && (
        <Box sx={{ display: 'flex', flexDirection: 'column', flex: 1, overflow: 'hidden' }}>
          <Box sx={{ p: 2, display: 'flex', justifyContent: 'flex-end', flexShrink: 0 }}>
            <Button
              size="small"
              startIcon={<DeleteSweep />}
              onClick={handleClearBlockLog}
              disabled={blockLog.length === 0}
            >
              Clear Log
            </Button>
          </Box>

          <List sx={{ flex: 1, overflow: 'auto', py: 0 }}>
            {blockLog.length === 0 ? (
              <ListItem>
                <ListItemText
                  primary="No blocking activity"
                  secondary="Blocked cookies will appear here"
                />
              </ListItem>
            ) : (
              blockLog.map((entry, idx) => (
                <Box key={idx}>
                  <ListItem>
                    <ListItemText
                      primary={
                        <Typography variant="body2" sx={{ fontWeight: 500 }}>
                          {entry.cookie_domain}
                        </Typography>
                      }
                      secondary={
                        <Box component="span" sx={{ display: 'block' }}>
                          <Typography variant="caption" component="span" display="block" color="text.secondary">
                            {entry.page_url.length > 50 ? entry.page_url.slice(0, 47) + '...' : entry.page_url}
                          </Typography>
                          <Box sx={{ display: 'flex', gap: 1, mt: 0.5 }}>
                            <Chip
                              label={entry.reason === 'blocked_domain' ? 'Domain blocked' : 'Third-party'}
                              size="small"
                              sx={{ height: 18, fontSize: '0.65rem' }}
                            />
                            <Typography variant="caption" color="text.disabled">
                              {formatRelativeTime(entry.blocked_at)}
                            </Typography>
                          </Box>
                        </Box>
                      }
                    />
                  </ListItem>
                  <Divider />
                </Box>
              ))
            )}
          </List>
        </Box>
      )}

      {/* Toast notifications */}
      {toast && (
        <Snackbar
          open={true}
          autoHideDuration={3000}
          onClose={() => setToast(null)}
          anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
        >
          <Alert onClose={() => setToast(null)} severity={toast.severity} sx={{ width: '100%' }}>
            {toast.message}
          </Alert>
        </Snackbar>
      )}
    </Box>
  );
}

function formatRelativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return 'Just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} hour${hours > 1 ? 's' : ''} ago`;
  const days = Math.floor(hours / 24);
  return `${days} day${days > 1 ? 's' : ''} ago`;
}
