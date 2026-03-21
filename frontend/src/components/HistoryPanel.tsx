import { useEffect, useState, useMemo } from 'react';
import { useHistory } from '../hooks/useHistory';
import {
  Box,
  List,
  ListItem,
  ListItemText,
  ListItemButton,
  TextField,
  Typography,
  CircularProgress,
  Paper,
  Chip,
  Divider,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogContentText,
  DialogActions,
  Pagination as MuiPagination
} from '@mui/material';
import { Delete, Clear, Search as SearchIcon } from '@mui/icons-material';
import { HodosButton } from './HodosButton';

type TimeRange = 'hour' | 'day' | 'week' | 'all';

const ITEMS_PER_PAGE = 20;

export function HistoryPanel() {
  const {
    history,
    loading,
    error,
    fetchHistory,
    searchHistory,
    deleteEntry,
    clearAllHistory,
    clearHistoryRange,
    chromiumTimeToDate,
    dateToChromiumTime
  } = useHistory();

  const [searchTerm, setSearchTerm] = useState('');
  const [timeRange, setTimeRange] = useState<TimeRange>('all');
  const [confirmClearOpen, setConfirmClearOpen] = useState(false);
  const [currentPage, setCurrentPage] = useState(1);

  useEffect(() => {
    console.log('📚 HistoryPanel mounted, fetching history...');
    fetchHistory({ limit: 5000, offset: 0 }); // Fetch all for client-side pagination
  }, [fetchHistory]);

  // Filter history by time range
  const filteredHistory = useMemo(() => {
    if (timeRange === 'all') return history;
    
    const now = Date.now();
    let cutoffTime: number;
    
    switch (timeRange) {
      case 'hour':
        cutoffTime = now - (60 * 60 * 1000);
        break;
      case 'day':
        cutoffTime = now - (24 * 60 * 60 * 1000);
        break;
      case 'week':
        cutoffTime = now - (7 * 24 * 60 * 60 * 1000);
        break;
      default:
        return history;
    }

    return history.filter(entry => {
      const entryDate = chromiumTimeToDate(entry.visitTime);
      return entryDate.getTime() >= cutoffTime;
    });
  }, [history, timeRange, chromiumTimeToDate]);

  // Pagination
  const totalPages = Math.ceil(filteredHistory.length / ITEMS_PER_PAGE);
  const paginatedHistory = useMemo(() => {
    const startIndex = (currentPage - 1) * ITEMS_PER_PAGE;
    return filteredHistory.slice(startIndex, startIndex + ITEMS_PER_PAGE);
  }, [filteredHistory, currentPage]);

  // Reset page when filter changes
  useEffect(() => {
    setCurrentPage(1);
  }, [timeRange, searchTerm]);

  const handleSearch = (term: string) => {
    setSearchTerm(term);
    if (term.trim()) {
      searchHistory({ search: term, limit: 5000, offset: 0 });
    } else {
      fetchHistory({ limit: 5000, offset: 0 });
    }
  };

  const handleDelete = (url: string) => {
    console.log('🗑️ Deleting history entry:', url);
    deleteEntry(url);
  };

  const handleClearClick = () => {
    setConfirmClearOpen(true);
  };

  const handleClearConfirm = () => {
    setConfirmClearOpen(false);
    
    if (timeRange === 'all') {
      clearAllHistory();
    } else {
      const now = new Date();
      let startDate: Date;
      
      switch (timeRange) {
        case 'hour':
          startDate = new Date(now.getTime() - (60 * 60 * 1000));
          break;
        case 'day':
          startDate = new Date(now.getTime() - (24 * 60 * 60 * 1000));
          break;
        case 'week':
          startDate = new Date(now.getTime() - (7 * 24 * 60 * 60 * 1000));
          break;
        default:
          clearAllHistory();
          return;
      }
      
      const startTime = dateToChromiumTime(startDate);
      const endTime = dateToChromiumTime(now);
      clearHistoryRange(startTime, endTime);
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

  const getTimeRangeLabel = () => {
    switch (timeRange) {
      case 'hour': return 'last hour';
      case 'day': return 'last 24 hours';
      case 'week': return 'last 7 days';
      default: return 'all time';
    }
  };

  const startIndex = (currentPage - 1) * ITEMS_PER_PAGE + 1;
  const endIndex = Math.min(currentPage * ITEMS_PER_PAGE, filteredHistory.length);

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', p: 2 }}>
      {/* Header */}
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2, flexWrap: 'wrap', gap: 1 }}>
        <Typography variant="h5" component="h2" sx={{ color: '#e0e0e0' }}>
          Browsing History
        </Typography>
        <Box sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
          <FormControl size="small" sx={{ minWidth: 140 }}>
            <InputLabel id="time-range-label" sx={{ color: '#888', '&.Mui-focused': { color: '#a67c00' } }}>Time Range</InputLabel>
            <Select
              labelId="time-range-label"
              value={timeRange}
              label="Time Range"
              onChange={(e) => setTimeRange(e.target.value as TimeRange)}
              sx={{
                bgcolor: '#1e1e1e',
                color: '#e0e0e0',
                '& .MuiOutlinedInput-notchedOutline': { borderColor: '#444' },
                '&:hover .MuiOutlinedInput-notchedOutline': { borderColor: '#666' },
                '&.Mui-focused .MuiOutlinedInput-notchedOutline': { borderColor: '#a67c00' },
                '& .MuiSvgIcon-root': { color: '#888' },
              }}
              MenuProps={{ PaperProps: { sx: { bgcolor: '#2a2a2a', color: '#e0e0e0' } } }}
            >
              <MenuItem value="hour">Last hour</MenuItem>
              <MenuItem value="day">Last 24 hours</MenuItem>
              <MenuItem value="week">Last 7 days</MenuItem>
              <MenuItem value="all">All time</MenuItem>
            </Select>
          </FormControl>
          <HodosButton
            variant="danger"
            size="small"
            onClick={handleClearClick}
          >
            <Clear fontSize="small" style={{ marginRight: 4 }} />
            Clear {timeRange !== 'all' ? getTimeRangeLabel() : 'All'}
          </HodosButton>
        </Box>
      </Box>

      {/* Search Bar */}
      <TextField
        fullWidth
        placeholder="Search history..."
        value={searchTerm}
        onChange={(e) => handleSearch(e.target.value)}
        InputProps={{
          startAdornment: <SearchIcon sx={{ mr: 1, color: '#888' }} />
        }}
        sx={{
          mb: 2,
          '& .MuiOutlinedInput-root': {
            bgcolor: '#1e1e1e',
            color: '#e0e0e0',
            '& fieldset': { borderColor: '#444' },
            '&:hover fieldset': { borderColor: '#666' },
            '&.Mui-focused fieldset': { borderColor: '#a67c00' },
          },
          '& .MuiInputLabel-root': { color: '#888' },
          '& .MuiInputLabel-root.Mui-focused': { color: '#a67c00' },
          '& input::placeholder': { color: '#888', opacity: 1 },
        }}
        size="small"
      />

      {/* Info */}
      {!loading && !error && (
        <Typography variant="body2" sx={{ mb: 1, color: '#888' }}>
          {filteredHistory.length} {filteredHistory.length === 1 ? 'entry' : 'entries'}
          {timeRange !== 'all' && ` (${getTimeRangeLabel()})`}
          {filteredHistory.length > ITEMS_PER_PAGE && ` • Showing ${startIndex}-${endIndex}`}
        </Typography>
      )}

      <Divider sx={{ mb: 2, borderColor: '#333' }} />

      {/* Loading State */}
      {loading && (
        <Box sx={{ display: 'flex', justifyContent: 'center', p: 4 }}>
          <CircularProgress sx={{ color: '#a67c00' }} />
        </Box>
      )}

      {/* Error State */}
      {error && (
        <Paper sx={{ p: 2, bgcolor: 'rgba(211, 47, 47, 0.15)', color: '#e57373' }}>
          <Typography variant="body2">{error}</Typography>
        </Paper>
      )}

      {/* History List */}
      {!loading && !error && (
        <Box sx={{ flex: 1, overflow: 'auto' }}>
          {paginatedHistory.length === 0 ? (
            <Box sx={{ textAlign: 'center', py: 4 }}>
              <Typography variant="body1" sx={{ color: '#888' }}>
                No history entries found
              </Typography>
            </Box>
          ) : (
            <List sx={{ p: 0 }}>
              {paginatedHistory.map((entry, index) => (
                <ListItem
                  key={`${entry.url}-${index}`}
                  disablePadding
                  sx={{
                    borderBottom: index < paginatedHistory.length - 1 ? '1px solid' : 'none',
                    borderColor: '#333',
                    '&:hover': { bgcolor: 'rgba(255,255,255,0.05)' },
                  }}
                  secondaryAction={
                    <HodosButton
                      variant="icon"
                      size="small"
                      onClick={() => handleDelete(entry.url)}
                      aria-label="Delete this entry"
                      title="Delete this entry"
                      sx={{ color: '#666', '&:hover': { color: '#e57373' } }}
                    >
                      <Delete fontSize="small" />
                    </HodosButton>
                  }
                >
                  <ListItemButton
                    onClick={() => {
                      console.log('📚 Navigating to:', entry.url);
                      window.hodosBrowser?.navigation?.navigate(entry.url);
                    }}
                    sx={{ '&:hover': { bgcolor: 'rgba(255,255,255,0.05)' } }}
                  >
                    <ListItemText
                      primary={
                        <Typography variant="body1" noWrap sx={{ color: '#e0e0e0' }}>
                          {entry.title || entry.url}
                        </Typography>
                      }
                      secondary={
                        <Box>
                          <Typography
                            component="span"
                            variant="body2"
                            sx={{ display: 'block', color: '#888' }}
                            noWrap
                          >
                            {entry.url}
                          </Typography>
                          <Box sx={{ display: 'flex', gap: 1, mt: 0.5, alignItems: 'center' }}>
                            <Typography component="span" variant="caption" sx={{ color: '#666' }}>
                              {formatDate(entry.visitTime)}
                            </Typography>
                            <Chip
                              label={`${entry.visitCount} ${entry.visitCount === 1 ? 'visit' : 'visits'}`}
                              size="small"
                              variant="outlined"
                              sx={{ borderColor: '#444', color: '#888' }}
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

      {/* Pagination */}
      {!loading && !error && totalPages > 1 && (
        <>
          <Divider sx={{ mt: 2, mb: 1, borderColor: '#333' }} />
          <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', py: 1 }}>
            <MuiPagination
              count={totalPages}
              page={currentPage}
              onChange={(_, page) => setCurrentPage(page)}
              color="primary"
              size="small"
              showFirstButton
              showLastButton
              sx={{
                '& .MuiPaginationItem-root': { color: '#e0e0e0' },
                '& .MuiPaginationItem-root.Mui-selected': { bgcolor: 'rgba(166, 124, 0, 0.15)' },
              }}
            />
          </Box>
        </>
      )}

      {/* Clear Confirmation Dialog */}
      <Dialog
        open={confirmClearOpen}
        onClose={() => setConfirmClearOpen(false)}
        PaperProps={{ sx: { bgcolor: '#1e1e1e' } }}
      >
        <DialogTitle sx={{ color: '#e0e0e0' }}>
          Clear browsing history?
        </DialogTitle>
        <DialogContent>
          <DialogContentText sx={{ color: '#888' }}>
            {timeRange === 'all'
              ? 'This will permanently delete all your browsing history. This action cannot be undone.'
              : `This will permanently delete your browsing history from the ${getTimeRangeLabel()}. This action cannot be undone.`
            }
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <HodosButton variant="secondary" size="small" onClick={() => setConfirmClearOpen(false)}>Cancel</HodosButton>
          <HodosButton variant="danger" size="small" onClick={handleClearConfirm}>
            Clear History
          </HodosButton>
        </DialogActions>
      </Dialog>
    </Box>
  );
}

export default HistoryPanel;
