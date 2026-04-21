import { useState, useEffect } from 'react';
import {
  Box,
  Typography,
  Button,
  Snackbar,
  Alert,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogContentText,
  DialogActions,
  Paper,
  Skeleton,
} from '@mui/material';
import { Storage, Delete, Cookie, Warning } from '@mui/icons-material';
import { useCookies } from '../hooks/useCookies';

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function CachePanel() {
  const {
    cookies,
    domainGroups,
    loading,
    cacheSize,
    fetchAllCookies,
    deleteAllCookies,
    clearCache,
    getCacheSize,
  } = useCookies();

  const [cacheSizeLoading, setCacheSizeLoading] = useState(true);
  const [confirmCacheClear, setConfirmCacheClear] = useState(false);
  const [confirmCookieClear, setConfirmCookieClear] = useState(false);
  const [toastOpen, setToastOpen] = useState(false);
  const [toastMessage, setToastMessage] = useState('');

  useEffect(() => {
    // Fetch cookies and cache size on mount
    fetchAllCookies().finally(() => {});
    getCacheSize().finally(() => {
      setCacheSizeLoading(false);
    });
  }, [fetchAllCookies, getCacheSize]);

  const showToast = (message: string) => {
    setToastMessage(message);
    setToastOpen(true);
  };

  const handleClearCache = async () => {
    setConfirmCacheClear(false);
    try {
      await clearCache();
      showToast('Cache cleared successfully');
      // Refresh cache size
      setCacheSizeLoading(true);
      await getCacheSize();
      setCacheSizeLoading(false);
    } catch {
      showToast('Failed to clear cache');
      setCacheSizeLoading(false);
    }
  };

  const handleDeleteAllCookiesClick = () => {
    setConfirmCookieClear(true);
  };

  const handleDeleteAllCookiesConfirm = async () => {
    setConfirmCookieClear(false);
    try {
      await deleteAllCookies();
      showToast('All cookies deleted');
    } catch {
      showToast('Failed to delete cookies');
    }
  };

  const uniqueDomainCount = domainGroups.length;

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', p: 2 }}>
      {/* Header */}
      <Typography variant="h5" component="h2" sx={{ mb: 3, color: '#e0e0e0' }}>
        Cache & Storage
      </Typography>

      {/* Stats cards */}
      <Box sx={{ display: 'flex', gap: 2, mb: 3, flexWrap: 'wrap' }}>
        {/* Cached Data card */}
        <Paper
          variant="outlined"
          sx={{
            flex: 1,
            minWidth: 240,
            p: 3,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            textAlign: 'center',
            borderRadius: 2,
            bgcolor: '#1e1e1e',
            borderColor: '#333',
          }}
        >
          <Storage sx={{ fontSize: 40, color: '#a67c00', mb: 1 }} />
          <Typography variant="subtitle2" sx={{ mb: 1, color: '#888' }}>
            Cached Data
          </Typography>
          {cacheSizeLoading ? (
            <Skeleton width={100} height={40} sx={{ bgcolor: '#2a2a2a' }} />
          ) : (
            <Typography variant="h4" sx={{ fontWeight: 600, color: '#e0e0e0' }}>
              {formatBytes(cacheSize)}
            </Typography>
          )}
          <Typography
            variant="caption"
            sx={{ mt: 1, maxWidth: 200, color: '#666' }}
          >
            Images, scripts, stylesheets, and other cached files
          </Typography>
        </Paper>

        {/* Cookies card */}
        <Paper
          variant="outlined"
          sx={{
            flex: 1,
            minWidth: 240,
            p: 3,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            textAlign: 'center',
            borderRadius: 2,
            bgcolor: '#1e1e1e',
            borderColor: '#333',
          }}
        >
          <Cookie sx={{ fontSize: 40, color: '#a67c00', mb: 1 }} />
          <Typography variant="subtitle2" sx={{ mb: 1, color: '#888' }}>
            Cookies
          </Typography>
          {loading ? (
            <Skeleton width={100} height={40} sx={{ bgcolor: '#2a2a2a' }} />
          ) : (
            <Typography variant="h4" sx={{ fontWeight: 600, color: '#e0e0e0' }}>
              {cookies.length}
            </Typography>
          )}
          <Typography
            variant="caption"
            sx={{ mt: 1, color: '#666' }}
          >
            {loading
              ? 'Loading...'
              : `${cookies.length} ${cookies.length === 1 ? 'cookie' : 'cookies'} across ${uniqueDomainCount} ${uniqueDomainCount === 1 ? 'domain' : 'domains'}`}
          </Typography>
        </Paper>
      </Box>

      {/* Action buttons */}
      <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap' }}>
        <Button
          variant="contained"
          color="primary"
          startIcon={<Storage />}
          onClick={() => setConfirmCacheClear(true)}
          size="large"
        >
          Clear Cache
        </Button>
        <Button
          variant="outlined"
          color="error"
          startIcon={<Delete />}
          onClick={handleDeleteAllCookiesClick}
          size="large"
        >
          Delete All Cookies
        </Button>
      </Box>

      {/* Clear Cache confirmation dialog */}
      <Dialog
        open={confirmCacheClear}
        onClose={() => setConfirmCacheClear(false)}
        PaperProps={{ sx: { bgcolor: '#1e1e1e' } }}
      >
        <DialogTitle sx={{ color: '#e0e0e0' }}>Clear browser cache?</DialogTitle>
        <DialogContent>
          <DialogContentText sx={{ color: '#888' }}>
            This will clear {formatBytes(cacheSize)} of cached data including
            images, scripts, and stylesheets. Pages may load slower until the
            cache is rebuilt.
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setConfirmCacheClear(false)}>Cancel</Button>
          <Button
            onClick={handleClearCache}
            color="primary"
            variant="contained"
          >
            Clear Cache
          </Button>
        </DialogActions>
      </Dialog>

      {/* Delete All Cookies confirmation dialog */}
      <Dialog
        open={confirmCookieClear}
        onClose={() => setConfirmCookieClear(false)}
        PaperProps={{ sx: { bgcolor: '#1e1e1e' } }}
      >
        <DialogTitle sx={{ display: 'flex', alignItems: 'center', gap: 1, color: '#e0e0e0' }}>
          <Warning color="warning" />
          Delete all cookies?
        </DialogTitle>
        <DialogContent>
          <DialogContentText sx={{ color: '#888' }}>
            This will sign you out of all websites. You'll need to log back in to
            sites like Google, YouTube, Twitter, and any other accounts you're
            currently signed into.
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setConfirmCookieClear(false)}>Cancel</Button>
          <Button
            onClick={handleDeleteAllCookiesConfirm}
            color="error"
            variant="contained"
          >
            Delete All Cookies
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

export default CachePanel;
