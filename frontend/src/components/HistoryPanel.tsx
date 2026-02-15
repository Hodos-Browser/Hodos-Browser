import { useEffect, useState } from 'react';
import { useHistory } from '../hooks/useHistory';
import {
  Box,
  List,
  ListItem,
  ListItemText,
  ListItemButton,
  TextField,
  IconButton,
  Typography,
  CircularProgress,
  Button,
  Paper,
  Chip,
  Divider
} from '@mui/material';
import { Delete, Clear, Search as SearchIcon } from '@mui/icons-material';

export function HistoryPanel() {
  const {
    history,
    loading,
    error,
    fetchHistory,
    searchHistory,
    deleteEntry,
    clearAllHistory,
    chromiumTimeToDate
  } = useHistory();

  const [searchTerm, setSearchTerm] = useState('');

  useEffect(() => {
    console.log('📚 HistoryPanel mounted, fetching history...');
    fetchHistory({ limit: 100, offset: 0 });
  }, [fetchHistory]);

  const handleSearch = (term: string) => {
    setSearchTerm(term);
    if (term.trim()) {
      searchHistory({ search: term, limit: 100, offset: 0 });
    } else {
      fetchHistory({ limit: 100, offset: 0 });
    }
  };

  const handleDelete = (url: string) => {
    console.log('🗑️ Deleting history entry:', url);
    deleteEntry(url);
  };

  const handleClearAll = () => {
    if (window.confirm('Are you sure you want to clear all browsing history?')) {
      clearAllHistory();
    }
  };

  const formatDate = (chromiumTime: number): string => {
    try {
      const date = chromiumTimeToDate(chromiumTime);
      return date.toLocaleString();
    } catch (err) {
      console.error('Error formatting date:', err);
      return 'Unknown date';
    }
  };

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', p: 2 }}>
      {/* Header */}
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
        <Typography variant="h5" component="h2">
          Browsing History
        </Typography>
        <Button
          variant="outlined"
          color="error"
          startIcon={<Clear />}
          onClick={handleClearAll}
          size="small"
        >
          Clear All
        </Button>
      </Box>

      {/* Search Bar */}
      <TextField
        fullWidth
        placeholder="Search history..."
        value={searchTerm}
        onChange={(e) => handleSearch(e.target.value)}
        InputProps={{
          startAdornment: <SearchIcon sx={{ mr: 1, color: 'action.active' }} />
        }}
        sx={{ mb: 2 }}
        size="small"
      />

      {/* Info */}
      {!loading && !error && (
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
          {history.length} {history.length === 1 ? 'entry' : 'entries'}
        </Typography>
      )}

      <Divider sx={{ mb: 2 }} />

      {/* Loading State */}
      {loading && (
        <Box sx={{ display: 'flex', justifyContent: 'center', p: 4 }}>
          <CircularProgress />
        </Box>
      )}

      {/* Error State */}
      {error && (
        <Paper sx={{ p: 2, bgcolor: 'error.light', color: 'error.contrastText' }}>
          <Typography variant="body2">{error}</Typography>
        </Paper>
      )}

      {/* History List */}
      {!loading && !error && (
        <Box sx={{ flex: 1, overflow: 'auto' }}>
          {history.length === 0 ? (
            <Box sx={{ textAlign: 'center', py: 4 }}>
              <Typography variant="body1" color="text.secondary">
                No history entries found
              </Typography>
            </Box>
          ) : (
            <List sx={{ p: 0 }}>
              {history.map((entry, index) => (
                <ListItem
                  key={`${entry.url}-${index}`}
                  disablePadding
                  sx={{
                    borderBottom: index < history.length - 1 ? '1px solid' : 'none',
                    borderColor: 'divider'
                  }}
                  secondaryAction={
                    <IconButton
                      edge="end"
                      onClick={() => handleDelete(entry.url)}
                      size="small"
                      title="Delete this entry"
                    >
                      <Delete fontSize="small" />
                    </IconButton>
                  }
                >
                  <ListItemButton
                    onClick={() => {
                      console.log('📚 Navigating to:', entry.url);
                      window.hodosBrowser?.navigation?.navigate(entry.url);
                    }}
                  >
                    <ListItemText
                      primary={
                        <Typography variant="body1" noWrap>
                          {entry.title || entry.url}
                        </Typography>
                      }
                      secondary={
                        <Box>
                          <Typography
                            component="span"
                            variant="body2"
                            color="text.secondary"
                            sx={{ display: 'block' }}
                            noWrap
                          >
                            {entry.url}
                          </Typography>
                          <Box sx={{ display: 'flex', gap: 1, mt: 0.5, alignItems: 'center' }}>
                            <Typography component="span" variant="caption" color="text.secondary">
                              {formatDate(entry.visitTime)}
                            </Typography>
                            <Chip
                              label={`${entry.visitCount} ${entry.visitCount === 1 ? 'visit' : 'visits'}`}
                              size="small"
                              variant="outlined"
                            />
                          </Box>
                        </Box>
                      }
                    />
                  </ListItemButton>
                </ListItem>
              ))}
            </List>
          )}
        </Box>
      )}
    </Box>
  );
}

export default HistoryPanel;
