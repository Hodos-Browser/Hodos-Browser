import React, { useEffect, useState } from 'react';
import { Box, List, ListItem, ListItemButton, ListItemIcon, ListItemText, Typography, CircularProgress } from '@mui/material';
import HistoryIcon from '@mui/icons-material/History';
import SearchIcon from '@mui/icons-material/Search';
import { useOmniboxSuggestions } from '../hooks/useOmniboxSuggestions';
import type { Suggestion } from '../types/omnibox';

/**
 * OmniboxOverlayRoot - Renders the omnibox autocomplete suggestions.
 *
 * Receives query updates via 'omniboxQueryUpdate' window event from address bar.
 * Sends autocomplete suggestion back via cefMessage.
 */
const OmniboxOverlayRoot: React.FC = () => {
  const [query, setQuery] = useState('');
  const { suggestions, loading, autocomplete, search } = useOmniboxSuggestions();

  // Listen for query updates from address bar
  useEffect(() => {
    const handleQueryUpdate = (event: Event) => {
      const customEvent = event as CustomEvent<{ query: string }>;
      const newQuery = customEvent.detail.query;
      console.log('🔍 Omnibox received query update:', newQuery);
      setQuery(newQuery);
      search(newQuery);
    };

    window.addEventListener('omniboxQueryUpdate', handleQueryUpdate);
    return () => {
      window.removeEventListener('omniboxQueryUpdate', handleQueryUpdate);
    };
  }, [search]);

  // Reset focus when query changes (clears any persistent MUI focus states)
  useEffect(() => {
    // Force blur any focused elements when query changes
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
  }, [query]);

  // Send autocomplete suggestion back to address bar
  useEffect(() => {
    if (autocomplete && window.cefMessage) {
      window.cefMessage.send('omnibox_autocomplete', autocomplete);
    }
  }, [autocomplete]);

  // Don't render anything if no query
  if (!query) {
    return null;
  }

  return (
    <Box
      sx={{
        width: '100%',
        height: '100%',
        backgroundColor: 'background.paper',
        boxShadow: 3,
        borderRadius: 1,
        overflow: 'hidden',
      }}
    >
      {loading && suggestions.length === 0 ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', p: 2 }}>
          <CircularProgress size={20} />
        </Box>
      ) : suggestions.length === 0 ? (
        <Box sx={{ p: 2 }}>
          <Typography variant="body2" color="text.secondary">
            No suggestions
          </Typography>
        </Box>
      ) : (
        <List dense sx={{ py: 0.5 }}>
          {suggestions.map((suggestion, index) => (
            <SuggestionItem
              key={`${query}-${suggestion.type}-${suggestion.url}-${index}`}
              suggestion={suggestion}
              query={query}
              isFirst={index === 0}
            />
          ))}
        </List>
      )}
    </Box>
  );
};

interface SuggestionItemProps {
  suggestion: Suggestion;
  query: string;
  isFirst: boolean;
}

/**
 * Individual suggestion item with icon and highlighted text
 */
const SuggestionItem: React.FC<SuggestionItemProps> = ({ suggestion, query, isFirst }) => {
  const handleClick = () => {
    // Navigate to the suggestion URL
    if (window.cefMessage) {
      if (suggestion.type === 'google') {
        // For Google suggestions, search for the term
        window.cefMessage.send('navigate', suggestion.url);
      } else {
        // For history, navigate directly
        window.cefMessage.send('navigate', suggestion.url);
      }
      // Hide overlay after navigation
      window.cefMessage.send('omnibox_hide');
    }
  };

  // Highlight matching text in title
  const highlightedTitle = highlightMatch(suggestion.title, query);

  // For history, show URL below title
  const secondaryText = suggestion.type === 'history'
    ? formatUrl(suggestion.url)
    : null;

  return (
    <ListItem disablePadding>
      <ListItemButton
        onClick={handleClick}
        disableRipple
        sx={{
          py: 0.75,
          px: 1.5,
          cursor: 'pointer !important',
          userSelect: 'none',
          '&:hover': {
            backgroundColor: 'action.hover',
          },
          '&:focus': {
            backgroundColor: 'transparent',
          },
          '&:active': {
            backgroundColor: 'action.hover',
          },
          '&.Mui-focusVisible': {
            backgroundColor: 'transparent',
          },
        }}
      >
        <ListItemIcon sx={{ minWidth: 36, cursor: 'pointer' }}>
          {suggestion.type === 'history' ? (
            <HistoryIcon fontSize="small" color="action" />
          ) : (
            <SearchIcon fontSize="small" color="action" />
          )}
        </ListItemIcon>
        <ListItemText
          primary={highlightedTitle}
          secondary={secondaryText}
          sx={{ cursor: 'pointer' }}
          primaryTypographyProps={{
            variant: 'body2',
            noWrap: true,
            sx: { fontWeight: isFirst ? 500 : 400, cursor: 'pointer' }
          }}
          secondaryTypographyProps={{
            variant: 'caption',
            noWrap: true,
            color: 'text.disabled',
            sx: { cursor: 'pointer' }
          }}
        />
      </ListItemButton>
    </ListItem>
  );
};

/**
 * Highlight matching characters in text
 */
function highlightMatch(text: string, query: string): React.ReactNode {
  if (!query) return text;

  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const matchIndex = lowerText.indexOf(lowerQuery);

  if (matchIndex === -1) return text;

  const before = text.slice(0, matchIndex);
  const match = text.slice(matchIndex, matchIndex + query.length);
  const after = text.slice(matchIndex + query.length);

  return (
    <>
      {before}
      <strong>{match}</strong>
      {after}
    </>
  );
}

/**
 * Format URL for display (remove protocol, truncate)
 */
function formatUrl(url: string): string {
  try {
    const parsed = new URL(url);
    let formatted = parsed.hostname + parsed.pathname;
    if (formatted.endsWith('/')) {
      formatted = formatted.slice(0, -1);
    }
    // Truncate if too long
    if (formatted.length > 60) {
      formatted = formatted.slice(0, 57) + '...';
    }
    return formatted;
  } catch {
    return url.slice(0, 60);
  }
}

export default OmniboxOverlayRoot;
