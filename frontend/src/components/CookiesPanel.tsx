import { useState, useEffect, useMemo, useCallback } from 'react';
import {
  Box,
  Typography,
  TextField,
  Accordion,
  AccordionSummary,
  AccordionDetails,
  Chip,
  IconButton,
  Button,
  Snackbar,
  Alert,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogContentText,
  DialogActions,
  Collapse,
  Skeleton,
  Divider,
  Tooltip,
  Menu,
  MenuItem,
  ListItemIcon,
  ListItemText,
} from '@mui/material';
import {
  ExpandMore,
  Delete,
  Search as SearchIcon,
  Cookie,
  Shield as ShieldIcon,
  Block as BlockIcon,
  CheckCircle as CheckCircleIcon,
  DeleteSweep,
  Add as AddIcon,
  RemoveCircleOutline,
} from '@mui/icons-material';
import { useCookies } from '../hooks/useCookies';
import { useCookieBlocking } from '../hooks/useCookieBlocking';
import type { CookieData, DomainCookieGroup } from '../types/cookies';
import type { BlockedDomainEntry } from '../types/cookieBlocking';

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatSameSite(value: number): string {
  switch (value) {
    case 0: return 'Unspecified';
    case 1: return 'None';
    case 2: return 'Lax';
    case 3: return 'Strict';
    default: return 'Unknown';
  }
}

function formatExpiry(cookie: CookieData): string {
  if (!cookie.hasExpires) return 'Session';
  if (cookie.expires == null) return 'Session';
  try {
    return new Date(cookie.expires).toLocaleString();
  } catch {
    return 'Invalid date';
  }
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

function truncateUrl(url: string, maxLen: number = 60): string {
  if (url.length <= maxLen) return url;
  return url.slice(0, maxLen) + '...';
}

export function CookiesPanel() {
  const {
    cookies,
    domainGroups,
    loading,
    error,
    fetchAllCookies,
    deleteCookie,
    deleteDomainCookies,
  } = useCookies();

  const {
    blockedDomains,
    blockLog,
    fetchBlockList,
    blockDomain,
    unblockDomain,
    fetchBlockLog,
    clearBlockLog,
  } = useCookieBlocking();

  const [searchTerm, setSearchTerm] = useState('');
  const [selectedCookie, setSelectedCookie] = useState<{
    domain: string;
    name: string;
    url: string;
  } | null>(null);
  const [expandedCookie, setExpandedCookie] = useState<string | null>(null);
  const [deletingItems, setDeletingItems] = useState<Set<string>>(new Set());
  const [toastOpen, setToastOpen] = useState(false);
  const [toastMessage, setToastMessage] = useState('');
  const [toastSeverity, setToastSeverity] = useState<'success' | 'info'>('success');
  const [confirmDomain, setConfirmDomain] = useState<string | null>(null);

  // Block menu anchor (for wildcard option)
  const [blockMenuAnchor, setBlockMenuAnchor] = useState<null | HTMLElement>(null);
  const [blockMenuDomain, setBlockMenuDomain] = useState<string>('');

  // Blocking log section
  const [logExpanded, setLogExpanded] = useState(false);

  // Managed domains section
  const [managedExpanded, setManagedExpanded] = useState(false);
  const [addDomainOpen, setAddDomainOpen] = useState(false);
  const [newDomainInput, setNewDomainInput] = useState('');

  // First-block warning
  const [hasShownBlockWarning, setHasShownBlockWarning] = useState(false);

  useEffect(() => {
    fetchAllCookies();
  }, [fetchAllCookies]);

  // Fetch block list on mount
  useEffect(() => {
    fetchBlockList();
  }, [fetchBlockList]);

  // Check if a domain is blocked (direct match or wildcard match)
  const isDomainBlocked = useCallback(
    (domain: string): BlockedDomainEntry | undefined => {
      // Direct match
      const direct = blockedDomains.find((d) => d.domain === domain);
      if (direct) return direct;
      // Wildcard match: check if any wildcard entry covers this domain
      for (const entry of blockedDomains) {
        if (entry.is_wildcard) {
          // Wildcard entry "example.com" matches "sub.example.com"
          if (domain === entry.domain || domain.endsWith('.' + entry.domain)) {
            return entry;
          }
        }
      }
      return undefined;
    },
    [blockedDomains]
  );

  // Filter domain groups based on search term
  const filteredGroups = useMemo((): DomainCookieGroup[] => {
    if (!searchTerm.trim()) return domainGroups;
    const term = searchTerm.toLowerCase();
    const result: DomainCookieGroup[] = [];
    for (const group of domainGroups) {
      // If domain matches, include the whole group
      if (group.domain.toLowerCase().includes(term)) {
        result.push(group);
        continue;
      }
      // Otherwise, filter cookies within the group by name
      const matchingCookies = group.cookies.filter((c) =>
        c.name.toLowerCase().includes(term)
      );
      if (matchingCookies.length > 0) {
        result.push({
          domain: group.domain,
          cookies: matchingCookies,
          totalSize: matchingCookies.reduce((sum, c) => sum + c.size, 0),
          count: matchingCookies.length,
        });
      }
    }
    return result;
  }, [domainGroups, searchTerm]);

  const showToast = useCallback((message: string, severity: 'success' | 'info' = 'success') => {
    setToastMessage(message);
    setToastSeverity(severity);
    setToastOpen(true);
  }, []);

  const handleCookieClick = useCallback(
    (cookie: CookieData, domain: string) => {
      const cookieKey = `${domain}:${cookie.name}`;
      const url = `https://${domain}${cookie.path}`;

      if (
        selectedCookie &&
        selectedCookie.domain === domain &&
        selectedCookie.name === cookie.name
      ) {
        // Already selected -- toggle expand
        setExpandedCookie((prev) => (prev === cookieKey ? null : cookieKey));
      } else {
        // Select this cookie and expand it
        setSelectedCookie({ domain, name: cookie.name, url });
        setExpandedCookie(cookieKey);
      }
    },
    [selectedCookie]
  );

  const handleDeleteSelected = useCallback(async () => {
    if (!selectedCookie) return;
    const cookieKey = `${selectedCookie.domain}:${selectedCookie.name}`;
    setDeletingItems((prev) => new Set(prev).add(cookieKey));

    try {
      await deleteCookie(selectedCookie.url, selectedCookie.name);
      setTimeout(() => {
        setDeletingItems((prev) => {
          const next = new Set(prev);
          next.delete(cookieKey);
          return next;
        });
      }, 300);
      showToast(`Cookie deleted: ${selectedCookie.name}`);
      setSelectedCookie(null);
      setExpandedCookie(null);
    } catch (err) {
      setDeletingItems((prev) => {
        const next = new Set(prev);
        next.delete(cookieKey);
        return next;
      });
      showToast('Failed to delete cookie');
    }
  }, [selectedCookie, deleteCookie, showToast]);

  const handleDomainDelete = useCallback(
    (e: React.MouseEvent, domain: string) => {
      e.stopPropagation();
      setConfirmDomain(domain);
    },
    []
  );

  const handleConfirmDomainDelete = useCallback(async () => {
    if (!confirmDomain) return;
    const group = domainGroups.find((g) => g.domain === confirmDomain);
    if (!group) return;

    // Add all cookies in the domain to deleting set
    const keys = group.cookies.map((c) => `${confirmDomain}:${c.name}`);
    setDeletingItems((prev) => {
      const next = new Set(prev);
      keys.forEach((k) => next.add(k));
      return next;
    });

    try {
      await deleteDomainCookies(confirmDomain);
      setTimeout(() => {
        setDeletingItems((prev) => {
          const next = new Set(prev);
          keys.forEach((k) => next.delete(k));
          return next;
        });
      }, 300);
      showToast(
        `Deleted ${group.count} cookies from ${confirmDomain}`
      );
    } catch (err) {
      setDeletingItems((prev) => {
        const next = new Set(prev);
        keys.forEach((k) => next.delete(k));
        return next;
      });
      showToast('Failed to delete domain cookies');
    }

    setConfirmDomain(null);
    setSelectedCookie(null);
    setExpandedCookie(null);
  }, [confirmDomain, domainGroups, deleteDomainCookies, showToast]);

  // Block domain handlers
  const handleBlockClick = useCallback(
    (e: React.MouseEvent<HTMLElement>, domain: string) => {
      e.stopPropagation();
      setBlockMenuAnchor(e.currentTarget);
      setBlockMenuDomain(domain);
    },
    []
  );

  const handleBlockExact = useCallback(async () => {
    setBlockMenuAnchor(null);
    try {
      await blockDomain(blockMenuDomain, false);
      showToast(`Domain blocked: ${blockMenuDomain}`);
      if (!hasShownBlockWarning) {
        setHasShownBlockWarning(true);
        setTimeout(() => {
          showToast(
            'Blocking takes effect on next page load. Some sites may not work correctly with cookies blocked.',
            'info'
          );
        }, 3500);
      }
    } catch (err) {
      showToast('Failed to block domain');
    }
  }, [blockMenuDomain, blockDomain, showToast, hasShownBlockWarning]);

  const handleBlockWildcard = useCallback(async () => {
    setBlockMenuAnchor(null);
    try {
      await blockDomain(blockMenuDomain, true);
      showToast(`Domain blocked: *.${blockMenuDomain}`);
      if (!hasShownBlockWarning) {
        setHasShownBlockWarning(true);
        setTimeout(() => {
          showToast(
            'Blocking takes effect on next page load. Some sites may not work correctly with cookies blocked.',
            'info'
          );
        }, 3500);
      }
    } catch (err) {
      showToast('Failed to block domain');
    }
  }, [blockMenuDomain, blockDomain, showToast, hasShownBlockWarning]);

  const handleUnblock = useCallback(
    async (e: React.MouseEvent, domain: string) => {
      e.stopPropagation();
      try {
        await unblockDomain(domain);
        showToast(`Domain unblocked: ${domain}`);
      } catch (err) {
        showToast('Failed to unblock domain');
      }
    },
    [unblockDomain, showToast]
  );

  // Log section handlers
  const handleToggleLog = useCallback(async () => {
    const willExpand = !logExpanded;
    setLogExpanded(willExpand);
    if (willExpand) {
      try {
        await fetchBlockLog(50, 0);
      } catch {
        // Error handled by hook
      }
    }
  }, [logExpanded, fetchBlockLog]);

  const handleClearLog = useCallback(async () => {
    try {
      await clearBlockLog();
      showToast('Blocking log cleared');
    } catch {
      showToast('Failed to clear log');
    }
  }, [clearBlockLog, showToast]);

  // Managed domains handlers
  const handleToggleManaged = useCallback(() => {
    setManagedExpanded((prev) => !prev);
  }, []);

  const handleAddDomain = useCallback(async () => {
    const domain = newDomainInput.trim().toLowerCase();
    if (!domain) return;
    try {
      await blockDomain(domain, false);
      showToast(`Domain blocked: ${domain}`);
      setNewDomainInput('');
      setAddDomainOpen(false);
      if (!hasShownBlockWarning) {
        setHasShownBlockWarning(true);
        setTimeout(() => {
          showToast(
            'Blocking takes effect on next page load. Some sites may not work correctly with cookies blocked.',
            'info'
          );
        }, 3500);
      }
    } catch {
      showToast('Failed to block domain');
    }
  }, [newDomainInput, blockDomain, showToast, hasShownBlockWarning]);

  const handleRemoveManagedDomain = useCallback(
    async (domain: string) => {
      try {
        await unblockDomain(domain);
        showToast(`Domain removed: ${domain}`);
      } catch {
        showToast('Failed to remove domain');
      }
    },
    [unblockDomain, showToast]
  );

  const confirmGroup = confirmDomain
    ? domainGroups.find((g) => g.domain === confirmDomain)
    : null;

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', p: 2 }}>
      {/* Header toolbar */}
      <Box
        sx={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          mb: 2,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5 }}>
          <Typography variant="h5" component="h2">
            Cookies
          </Typography>
          <Chip
            label={`${cookies.length} total`}
            size="small"
            variant="outlined"
            icon={<Cookie sx={{ fontSize: 16 }} />}
          />
          {blockedDomains.length > 0 && (
            <Chip
              label={`${blockedDomains.length} blocked`}
              size="small"
              variant="outlined"
              color="error"
              icon={<ShieldIcon sx={{ fontSize: 16 }} />}
            />
          )}
        </Box>
        <Button
          variant="outlined"
          color="error"
          size="small"
          startIcon={<Delete />}
          disabled={!selectedCookie}
          onClick={handleDeleteSelected}
        >
          Delete Selected
        </Button>
      </Box>

      {/* Search box */}
      <TextField
        fullWidth
        placeholder="Search by domain or cookie name..."
        value={searchTerm}
        onChange={(e) => setSearchTerm(e.target.value)}
        InputProps={{
          startAdornment: <SearchIcon sx={{ mr: 1, color: 'action.active' }} />,
        }}
        sx={{ mb: 2 }}
        size="small"
      />

      <Divider sx={{ mb: 2 }} />

      {/* Loading state */}
      {loading && (
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1.5 }}>
          <Skeleton variant="rectangular" height={56} sx={{ borderRadius: 1 }} />
          <Skeleton variant="rectangular" height={56} sx={{ borderRadius: 1 }} />
          <Skeleton variant="rectangular" height={56} sx={{ borderRadius: 1 }} />
          <Skeleton variant="rectangular" height={56} sx={{ borderRadius: 1 }} />
        </Box>
      )}

      {/* Error state */}
      {error && !loading && (
        <Box sx={{ p: 2, bgcolor: 'error.light', borderRadius: 1 }}>
          <Typography variant="body2" color="error.contrastText">
            {error}
          </Typography>
        </Box>
      )}

      {/* Empty state */}
      {!loading && !error && filteredGroups.length === 0 && (
        <Box sx={{ textAlign: 'center', py: 6 }}>
          <Cookie sx={{ fontSize: 48, color: 'text.disabled', mb: 1 }} />
          <Typography variant="body1" color="text.secondary">
            {searchTerm ? 'No cookies match your search' : 'No cookies found'}
          </Typography>
        </Box>
      )}

      {/* Domain groups */}
      {!loading && !error && filteredGroups.length > 0 && (
        <Box sx={{ flex: 1, overflow: 'auto' }}>
          {filteredGroups.map((group) => {
            const blockedEntry = isDomainBlocked(group.domain);
            const isBlocked = !!blockedEntry;

            return (
              <Accordion
                key={group.domain}
                TransitionProps={{ unmountOnExit: true }}
                sx={{
                  '&:before': { display: 'none' },
                  boxShadow: 'none',
                  border: '1px solid',
                  borderColor: isBlocked ? 'error.light' : 'divider',
                  mb: 1,
                  borderRadius: '8px !important',
                  overflow: 'hidden',
                }}
              >
                <AccordionSummary
                  expandIcon={<ExpandMore />}
                  sx={{
                    backgroundColor: isBlocked
                      ? 'rgba(211, 47, 47, 0.04)'
                      : 'transparent',
                    '& .MuiAccordionSummary-content': {
                      alignItems: 'center',
                      gap: 1,
                    },
                  }}
                >
                  {isBlocked && (
                    <ShieldIcon
                      fontSize="small"
                      sx={{ color: 'error.main', mr: 0.5 }}
                    />
                  )}
                  <Typography sx={{ fontWeight: 600, flex: 1 }}>
                    {group.domain}
                  </Typography>
                  {isBlocked && blockedEntry && (
                    <Chip
                      label={
                        blockedEntry.source === 'default'
                          ? 'Tracker'
                          : 'Blocked'
                      }
                      size="small"
                      color="error"
                      variant="outlined"
                      sx={{ height: 22, fontSize: '0.7rem', mr: 0.5 }}
                    />
                  )}
                  <Chip
                    label={`${group.count} ${group.count === 1 ? 'cookie' : 'cookies'}`}
                    size="small"
                    variant="outlined"
                    sx={{ mr: 1 }}
                  />
                  <Chip
                    label={formatBytes(group.totalSize)}
                    size="small"
                    variant="outlined"
                    color="secondary"
                    sx={{ mr: 1 }}
                  />
                  {/* Block / Unblock button */}
                  {isBlocked ? (
                    <Tooltip title={`Unblock ${group.domain}`}>
                      <IconButton
                        size="small"
                        color="success"
                        onClick={(e) => handleUnblock(e, blockedEntry!.domain)}
                      >
                        <CheckCircleIcon fontSize="small" />
                      </IconButton>
                    </Tooltip>
                  ) : (
                    <Tooltip title={`Block ${group.domain}`}>
                      <IconButton
                        size="small"
                        color="warning"
                        onClick={(e) => handleBlockClick(e, group.domain)}
                      >
                        <BlockIcon fontSize="small" />
                      </IconButton>
                    </Tooltip>
                  )}
                  <IconButton
                    size="small"
                    color="error"
                    onClick={(e) => handleDomainDelete(e, group.domain)}
                    title={`Delete all cookies for ${group.domain}`}
                  >
                    <Delete fontSize="small" />
                  </IconButton>
                </AccordionSummary>
                <AccordionDetails sx={{ p: 0 }}>
                  {group.cookies.map((cookie) => {
                    const cookieKey = `${group.domain}:${cookie.name}`;
                    const isSelected =
                      selectedCookie?.domain === group.domain &&
                      selectedCookie?.name === cookie.name;
                    const isExpanded = expandedCookie === cookieKey;
                    const isDeleting = deletingItems.has(cookieKey);

                    return (
                      <Box
                        key={cookieKey}
                        sx={{
                          transition:
                            'opacity 0.3s ease-out, transform 0.3s ease-out',
                          opacity: isDeleting ? 0 : 1,
                          transform: isDeleting
                            ? 'translateX(-20px)'
                            : 'translateX(0)',
                        }}
                      >
                        <Box
                          onClick={() => handleCookieClick(cookie, group.domain)}
                          sx={{
                            display: 'flex',
                            alignItems: 'center',
                            px: 2,
                            py: 1,
                            cursor: 'pointer',
                            bgcolor: isSelected
                              ? 'action.selected'
                              : 'transparent',
                            '&:hover': { bgcolor: 'action.hover' },
                            borderTop: '1px solid',
                            borderColor: 'divider',
                          }}
                        >
                          <Typography
                            variant="body2"
                            noWrap
                            sx={{
                              flex: 1,
                              fontFamily: 'monospace',
                              fontSize: '0.85rem',
                            }}
                          >
                            {cookie.name}
                          </Typography>
                          {isSelected && (
                            <Chip
                              label="selected"
                              size="small"
                              color="primary"
                              variant="outlined"
                              sx={{ ml: 1, height: 20, fontSize: '0.7rem' }}
                            />
                          )}
                        </Box>

                        {/* Cookie detail panel */}
                        <Collapse
                          in={isExpanded}
                          timeout={200}
                          easing="ease-in-out"
                        >
                          <Box
                            sx={{
                              px: 3,
                              py: 2,
                              bgcolor: 'grey.50',
                              borderTop: '1px solid',
                              borderColor: 'divider',
                            }}
                          >
                            <Box
                              sx={{
                                display: 'grid',
                                gridTemplateColumns: '120px 1fr',
                                gap: 1,
                                '& > :nth-of-type(odd)': {
                                  color: 'text.secondary',
                                  fontSize: '0.8rem',
                                  fontWeight: 600,
                                },
                                '& > :nth-of-type(even)': {
                                  fontSize: '0.85rem',
                                  wordBreak: 'break-all',
                                },
                              }}
                            >
                              <Typography>Name</Typography>
                              <Typography sx={{ fontFamily: 'monospace' }}>
                                {cookie.name}
                              </Typography>

                              <Typography>Value</Typography>
                              <Tooltip
                                title={
                                  cookie.value.length > 100
                                    ? cookie.value
                                    : ''
                                }
                                placement="top-start"
                              >
                                <Typography sx={{ fontFamily: 'monospace' }}>
                                  {cookie.value.length > 100
                                    ? `${cookie.value.slice(0, 100)}...`
                                    : cookie.value || '(empty)'}
                                </Typography>
                              </Tooltip>

                              <Typography>Domain</Typography>
                              <Typography>{cookie.domain}</Typography>

                              <Typography>Path</Typography>
                              <Typography sx={{ fontFamily: 'monospace' }}>
                                {cookie.path}
                              </Typography>

                              <Typography>Expires</Typography>
                              <Typography>{formatExpiry(cookie)}</Typography>

                              <Typography>Size</Typography>
                              <Typography>{formatBytes(cookie.size)}</Typography>

                              <Typography>HttpOnly</Typography>
                              <Box>
                                <Chip
                                  label={cookie.httponly ? 'Yes' : 'No'}
                                  size="small"
                                  color={cookie.httponly ? 'success' : 'default'}
                                  sx={{ height: 22, fontSize: '0.75rem' }}
                                />
                              </Box>

                              <Typography>Secure</Typography>
                              <Box>
                                <Chip
                                  label={cookie.secure ? 'Yes' : 'No'}
                                  size="small"
                                  color={cookie.secure ? 'success' : 'default'}
                                  sx={{ height: 22, fontSize: '0.75rem' }}
                                />
                              </Box>

                              <Typography>SameSite</Typography>
                              <Typography>
                                {formatSameSite(cookie.sameSite)}
                              </Typography>
                            </Box>
                          </Box>
                        </Collapse>
                      </Box>
                    );
                  })}
                </AccordionDetails>
              </Accordion>
            );
          })}
        </Box>
      )}

      <Divider sx={{ my: 2 }} />

      {/* Blocking Log Section */}
      <Accordion
        expanded={logExpanded}
        onChange={handleToggleLog}
        sx={{
          '&:before': { display: 'none' },
          boxShadow: 'none',
          border: '1px solid',
          borderColor: 'divider',
          borderRadius: '8px !important',
          overflow: 'hidden',
          mb: 1,
        }}
      >
        <AccordionSummary
          expandIcon={<ExpandMore />}
          sx={{ '& .MuiAccordionSummary-content': { alignItems: 'center', gap: 1 } }}
        >
          <ShieldIcon fontSize="small" sx={{ color: 'text.secondary' }} />
          <Typography sx={{ fontWeight: 600 }}>Blocking Log</Typography>
          {blockLog.length > 0 && (
            <Chip
              label={`${blockLog.length} entries`}
              size="small"
              variant="outlined"
              sx={{ ml: 1 }}
            />
          )}
        </AccordionSummary>
        <AccordionDetails sx={{ p: 0 }}>
          {blockLog.length === 0 ? (
            <Box sx={{ p: 3, textAlign: 'center' }}>
              <Typography variant="body2" color="text.secondary">
                No blocked cookie attempts recorded yet.
              </Typography>
            </Box>
          ) : (
            <Box>
              <Box sx={{ px: 2, py: 1, display: 'flex', justifyContent: 'flex-end' }}>
                <Button
                  size="small"
                  color="error"
                  startIcon={<DeleteSweep />}
                  onClick={handleClearLog}
                >
                  Clear Log
                </Button>
              </Box>
              {blockLog.map((entry, idx) => (
                <Box
                  key={`${entry.cookie_domain}-${entry.blocked_at}-${idx}`}
                  sx={{
                    px: 2,
                    py: 1,
                    borderTop: '1px solid',
                    borderColor: 'divider',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 1.5,
                  }}
                >
                  <Typography
                    variant="body2"
                    sx={{ fontWeight: 600, minWidth: 140 }}
                    noWrap
                  >
                    {entry.cookie_domain}
                  </Typography>
                  <Tooltip title={entry.page_url} placement="top">
                    <Typography
                      variant="body2"
                      color="text.secondary"
                      sx={{ flex: 1, fontFamily: 'monospace', fontSize: '0.8rem' }}
                      noWrap
                    >
                      {truncateUrl(entry.page_url)}
                    </Typography>
                  </Tooltip>
                  <Chip
                    label={
                      entry.reason === 'third_party'
                        ? 'Third-party'
                        : 'Domain blocked'
                    }
                    size="small"
                    variant="outlined"
                    color={entry.reason === 'third_party' ? 'warning' : 'error'}
                    sx={{ height: 22, fontSize: '0.7rem' }}
                  />
                  <Typography
                    variant="caption"
                    color="text.secondary"
                    sx={{ minWidth: 80, textAlign: 'right' }}
                  >
                    {formatRelativeTime(entry.blocked_at)}
                  </Typography>
                </Box>
              ))}
            </Box>
          )}
        </AccordionDetails>
      </Accordion>

      {/* Managed Domains Section */}
      <Accordion
        expanded={managedExpanded}
        onChange={handleToggleManaged}
        sx={{
          '&:before': { display: 'none' },
          boxShadow: 'none',
          border: '1px solid',
          borderColor: 'divider',
          borderRadius: '8px !important',
          overflow: 'hidden',
          mb: 1,
        }}
      >
        <AccordionSummary
          expandIcon={<ExpandMore />}
          sx={{ '& .MuiAccordionSummary-content': { alignItems: 'center', gap: 1 } }}
        >
          <BlockIcon fontSize="small" sx={{ color: 'text.secondary' }} />
          <Typography sx={{ fontWeight: 600 }}>Managed Domains</Typography>
          {blockedDomains.length > 0 && (
            <Chip
              label={`${blockedDomains.length} domains`}
              size="small"
              variant="outlined"
              sx={{ ml: 1 }}
            />
          )}
        </AccordionSummary>
        <AccordionDetails sx={{ p: 0 }}>
          {/* Add domain inline */}
          <Box sx={{ px: 2, py: 1, display: 'flex', alignItems: 'center', gap: 1 }}>
            {addDomainOpen ? (
              <>
                <TextField
                  size="small"
                  placeholder="Enter domain (e.g. tracker.com)"
                  value={newDomainInput}
                  onChange={(e) => setNewDomainInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') handleAddDomain();
                    if (e.key === 'Escape') {
                      setAddDomainOpen(false);
                      setNewDomainInput('');
                    }
                  }}
                  sx={{ flex: 1 }}
                  autoFocus
                />
                <Button size="small" variant="contained" onClick={handleAddDomain}>
                  Block
                </Button>
                <Button
                  size="small"
                  onClick={() => {
                    setAddDomainOpen(false);
                    setNewDomainInput('');
                  }}
                >
                  Cancel
                </Button>
              </>
            ) : (
              <Button
                size="small"
                startIcon={<AddIcon />}
                onClick={() => setAddDomainOpen(true)}
              >
                Add Domain
              </Button>
            )}
          </Box>
          <Divider />
          {blockedDomains.length === 0 ? (
            <Box sx={{ p: 3, textAlign: 'center' }}>
              <Typography variant="body2" color="text.secondary">
                No blocked domains. Add one above or block from the cookie list.
              </Typography>
            </Box>
          ) : (
            blockedDomains.map((entry) => (
              <Box
                key={entry.domain}
                sx={{
                  px: 2,
                  py: 1,
                  borderTop: '1px solid',
                  borderColor: 'divider',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 1,
                }}
              >
                <Typography
                  variant="body2"
                  sx={{ fontWeight: 500, flex: 1, fontFamily: 'monospace' }}
                >
                  {entry.is_wildcard ? `*.${entry.domain}` : entry.domain}
                </Typography>
                {entry.is_wildcard && (
                  <Chip
                    label="Wildcard"
                    size="small"
                    variant="outlined"
                    sx={{ height: 22, fontSize: '0.7rem' }}
                  />
                )}
                <Chip
                  label={entry.source === 'default' ? 'Tracker' : 'User'}
                  size="small"
                  variant="outlined"
                  color={entry.source === 'default' ? 'warning' : 'info'}
                  sx={{ height: 22, fontSize: '0.7rem' }}
                />
                <Tooltip title={`Remove ${entry.domain} from block list`}>
                  <IconButton
                    size="small"
                    color="error"
                    onClick={() => handleRemoveManagedDomain(entry.domain)}
                  >
                    <RemoveCircleOutline fontSize="small" />
                  </IconButton>
                </Tooltip>
              </Box>
            ))
          )}
        </AccordionDetails>
      </Accordion>

      {/* Block action menu (exact vs wildcard) */}
      <Menu
        anchorEl={blockMenuAnchor}
        open={Boolean(blockMenuAnchor)}
        onClose={() => setBlockMenuAnchor(null)}
      >
        <MenuItem onClick={handleBlockExact}>
          <ListItemIcon>
            <BlockIcon fontSize="small" />
          </ListItemIcon>
          <ListItemText>Block {blockMenuDomain}</ListItemText>
        </MenuItem>
        <MenuItem onClick={handleBlockWildcard}>
          <ListItemIcon>
            <ShieldIcon fontSize="small" />
          </ListItemIcon>
          <ListItemText>Block all subdomains (*.{blockMenuDomain})</ListItemText>
        </MenuItem>
      </Menu>

      {/* Domain deletion confirmation dialog */}
      <Dialog
        open={confirmDomain !== null}
        onClose={() => setConfirmDomain(null)}
      >
        <DialogTitle>
          Delete all cookies for {confirmDomain}?
        </DialogTitle>
        <DialogContent>
          <DialogContentText>
            This will delete {confirmGroup?.count ?? 0} cookies (
            {formatBytes(confirmGroup?.totalSize ?? 0)}). This action cannot be
            undone.
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setConfirmDomain(null)}>Cancel</Button>
          <Button
            onClick={handleConfirmDomainDelete}
            color="error"
            variant="contained"
          >
            Delete All
          </Button>
        </DialogActions>
      </Dialog>

      {/* Toast notification */}
      <Snackbar
        open={toastOpen}
        autoHideDuration={3000}
        onClose={() => setToastOpen(false)}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
      >
        <Alert
          onClose={() => setToastOpen(false)}
          severity={toastSeverity}
          variant="filled"
          sx={{ width: '100%' }}
        >
          {toastMessage}
        </Alert>
      </Snackbar>
    </Box>
  );
}

export default CookiesPanel;
