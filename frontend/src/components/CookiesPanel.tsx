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
} from '@mui/material';
import {
  ExpandMore,
  Delete,
  Search as SearchIcon,
  Cookie,
} from '@mui/icons-material';
import { useCookies } from '../hooks/useCookies';
import type { CookieData, DomainCookieGroup } from '../types/cookies';

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
  const [confirmDomain, setConfirmDomain] = useState<string | null>(null);

  useEffect(() => {
    fetchAllCookies();
  }, [fetchAllCookies]);

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

  const showToast = useCallback((message: string) => {
    setToastMessage(message);
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
          {filteredGroups.map((group) => (
            <Accordion
              key={group.domain}
              TransitionProps={{ unmountOnExit: true }}
              sx={{
                '&:before': { display: 'none' },
                boxShadow: 'none',
                border: '1px solid',
                borderColor: 'divider',
                mb: 1,
                borderRadius: '8px !important',
                overflow: 'hidden',
              }}
            >
              <AccordionSummary
                expandIcon={<ExpandMore />}
                sx={{ '& .MuiAccordionSummary-content': { alignItems: 'center', gap: 1 } }}
              >
                <Typography sx={{ fontWeight: 600, flex: 1 }}>
                  {group.domain}
                </Typography>
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
          ))}
        </Box>
      )}

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
          severity="success"
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
