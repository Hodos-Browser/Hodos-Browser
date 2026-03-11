import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Box, List, ListItem, ListItemButton, ListItemIcon, ListItemText, Typography, CircularProgress } from '@mui/material';
import HistoryIcon from '@mui/icons-material/History';
import SearchIcon from '@mui/icons-material/Search';
import { useOmniboxSuggestions } from '../hooks/useOmniboxSuggestions';
import type { Suggestion } from '../types/omnibox';

/**
 * OmniboxOverlayRoot - Renders the omnibox autocomplete suggestions.
 *
 * Receives query updates via 'omniboxQueryUpdate' window event from address bar.
 * Receives arrow key navigation via 'omniboxSelect' window event.
 * Sends autocomplete suggestion back via cefMessage.
 */
const OmniboxOverlayRoot: React.FC = () => {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const { suggestions, loading, search } = useOmniboxSuggestions();

  // Track suggestions length for arrow key clamping
  const suggestionsRef = useRef(suggestions);
  suggestionsRef.current = suggestions;

  // Set body data attribute for CEF-level cursor fix
  useEffect(() => {
    document.body.setAttribute('data-overlay', 'omnibox');
    return () => {
      document.body.removeAttribute('data-overlay');
    };
  }, []);

  // Listen for query updates from address bar
  useEffect(() => {
    const handleQueryUpdate = (event: Event) => {
      const customEvent = event as CustomEvent<{ query: string }>;
      const newQuery = customEvent.detail.query;
      console.log('Omnibox received query update:', newQuery);
      setQuery(newQuery);
      search(newQuery);
    };

    window.addEventListener('omniboxQueryUpdate', handleQueryUpdate);
    return () => {
      window.removeEventListener('omniboxQueryUpdate', handleQueryUpdate);
    };
  }, [search]);

  // Reset selectedIndex when suggestions change (user typed new text)
  useEffect(() => {
    setSelectedIndex(-1);
  }, [suggestions]);

  // Listen for arrow key navigation from address bar
  useEffect(() => {
    const handleSelect = (event: Event) => {
      const customEvent = event as CustomEvent<{ direction: string }>;
      const direction = customEvent.detail.direction;
      const maxIndex = suggestionsRef.current.length - 1;

      setSelectedIndex(prev => {
        let next: number;
        if (direction === 'down') {
          next = prev < maxIndex ? prev + 1 : maxIndex;
        } else {
          next = prev > -1 ? prev - 1 : -1;
        }
        return next;
      });
    };

    window.addEventListener('omniboxSelect', handleSelect);
    return () => {
      window.removeEventListener('omniboxSelect', handleSelect);
    };
  }, []);

  // Send selected suggestion back to address bar via IPC when arrow keys change selection
  useEffect(() => {
    if (!window.cefMessage) return;

    if (selectedIndex >= 0 && selectedIndex < suggestions.length) {
      const selected = suggestions[selectedIndex];
      const text = selected.type === 'history' ? selected.url : selected.title;
      window.cefMessage.send('omnibox_autocomplete', text);
    } else if (selectedIndex === -1) {
      window.cefMessage.send('omnibox_autocomplete', '');
    }
  }, [selectedIndex]);

  // Reset focus when query changes (clears any persistent MUI focus states)
  useEffect(() => {
    // Force blur any focused elements when query changes
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
  }, [query]);

  // Autocomplete IPC is now sent directly from useOmniboxSuggestions hook
  // to avoid React useEffect deduplication when the same string is set twice

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
              isSelected={index === selectedIndex}
            />
          ))}
        </List>
      )}
    </Box>
  );
};

/**
 * Favicon icon with fallback to HistoryIcon on load error
 */
const FaviconIcon: React.FC<{ url: string }> = ({ url }) => {
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);
  const domain = useMemo(() => {
    try { return new URL(url).hostname; } catch { return null; }
  }, [url]);

  if (!domain || failed) {
    return <HistoryIcon fontSize="small" color="action" />;
  }

  return (
    <>
      {!loaded && <HistoryIcon fontSize="small" color="action" />}
      <img
        src={`https://www.google.com/s2/favicons?domain=${domain}&sz=16`}
        width={16}
        height={16}
        onLoad={() => setLoaded(true)}
        onError={() => setFailed(true)}
        style={{ display: loaded ? 'block' : 'none' }}
        alt=""
      />
    </>
  );
};

interface SuggestionItemProps {
  suggestion: Suggestion;
  query: string;
  isFirst: boolean;
  isSelected: boolean;
}

/**
 * Individual suggestion item with icon and highlighted text
 */
const SuggestionItem: React.FC<SuggestionItemProps> = ({ suggestion, query, isFirst, isSelected }) => {
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
          backgroundColor: isSelected ? '#e8e8e8' : 'transparent',
          '&:hover': {
            backgroundColor: isSelected ? '#e0e0e0' : 'action.hover',
          },
          '&:focus': {
            backgroundColor: isSelected ? '#e8e8e8' : 'transparent',
          },
          '&:active': {
            backgroundColor: 'action.hover',
          },
          '&.Mui-focusVisible': {
            backgroundColor: isSelected ? '#e8e8e8' : 'transparent',
          },
        }}
      >
        <ListItemIcon sx={{ minWidth: 36, cursor: 'pointer' }}>
          {suggestion.type === 'history' ? (
            <FaviconIcon url={suggestion.url} />
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
            sx: { fontWeight: isFirst || isSelected ? 500 : 400, cursor: 'pointer' }
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
