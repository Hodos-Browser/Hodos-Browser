import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Box, List, ListItem, ListItemButton, ListItemIcon, ListItemText, Typography, CircularProgress } from '@mui/material';
import HistoryIcon from '@mui/icons-material/History';
import SearchIcon from '@mui/icons-material/Search';
import { useOmniboxSuggestions } from '../hooks/useOmniboxSuggestions';
import type { Suggestion } from '../types/omnibox';

const OmniboxOverlayRoot: React.FC = () => {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const { suggestions, loading, search } = useOmniboxSuggestions();

  const suggestionsRef = useRef(suggestions);
  suggestionsRef.current = suggestions;

  // Keep a stable previous suggestions list to avoid flashing empty state
  const [displaySuggestions, setDisplaySuggestions] = useState<Suggestion[]>([]);
  const [contentVisible, setContentVisible] = useState(false);

  useEffect(() => {
    if (suggestions.length > 0) {
      setDisplaySuggestions(suggestions);
      setContentVisible(true);
    } else if (!loading && query) {
      // Only show empty state after loading finishes, with a brief delay
      const timer = setTimeout(() => {
        setDisplaySuggestions([]);
        setContentVisible(true);
      }, 150);
      return () => clearTimeout(timer);
    }
  }, [suggestions, loading, query]);

  // Fade in when query starts, fade out when cleared
  useEffect(() => {
    if (query) {
      setContentVisible(true);
    } else {
      setContentVisible(false);
      // Clear suggestions after fade out
      const timer = setTimeout(() => setDisplaySuggestions([]), 200);
      return () => clearTimeout(timer);
    }
  }, [query]);

  useEffect(() => {
    document.body.setAttribute('data-overlay', 'omnibox');
    document.body.style.background = 'transparent';
    document.documentElement.style.background = 'transparent';
    return () => {
      document.body.removeAttribute('data-overlay');
    };
  }, []);

  useEffect(() => {
    const handleQueryUpdate = (event: Event) => {
      const customEvent = event as CustomEvent<{ query: string }>;
      const newQuery = customEvent.detail.query;
      setQuery(newQuery);
      search(newQuery);
    };

    window.addEventListener('omniboxQueryUpdate', handleQueryUpdate);
    return () => {
      window.removeEventListener('omniboxQueryUpdate', handleQueryUpdate);
    };
  }, [search]);

  useEffect(() => {
    setSelectedIndex(-1);
  }, [suggestions]);

  useEffect(() => {
    const handleSelect = (event: Event) => {
      const customEvent = event as CustomEvent<{ direction: string }>;
      const direction = customEvent.detail.direction;
      const maxIndex = suggestionsRef.current.length - 1;

      setSelectedIndex(prev => {
        if (direction === 'down') {
          return prev < maxIndex ? prev + 1 : maxIndex;
        } else {
          return prev > -1 ? prev - 1 : -1;
        }
      });
    };

    window.addEventListener('omniboxSelect', handleSelect);
    return () => {
      window.removeEventListener('omniboxSelect', handleSelect);
    };
  }, []);

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

  useEffect(() => {
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
  }, [query]);

  return (
    <Box
      sx={{
        width: '100%',
        height: '100%',
        backgroundColor: '#1a1d23',
        overflow: 'hidden',
        opacity: contentVisible && query ? 1 : 0,
        transition: 'opacity 0.15s ease',
      }}
    >
      {loading && displaySuggestions.length === 0 ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', p: 2 }}>
          <CircularProgress size={20} sx={{ color: '#a67c00' }} />
        </Box>
      ) : displaySuggestions.length === 0 ? (
        <Box sx={{ p: 2 }}>
          <Typography variant="body2" sx={{ color: '#9ca3af' }}>
            No suggestions
          </Typography>
        </Box>
      ) : (
        <List dense sx={{ py: 0.5 }}>
          {displaySuggestions.map((suggestion, index) => (
            <SuggestionItem
              key={`${suggestion.type}-${suggestion.url}-${index}`}
              suggestion={suggestion}
              query={query}
              isFirst={index === 0}
              isSelected={index === selectedIndex}
              index={index}
            />
          ))}
        </List>
      )}
    </Box>
  );
};

const FaviconIcon: React.FC<{ url: string }> = ({ url }) => {
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);
  const domain = useMemo(() => {
    try { return new URL(url).hostname; } catch { return null; }
  }, [url]);

  if (!domain || failed) {
    return <HistoryIcon fontSize="small" sx={{ color: '#9ca3af' }} />;
  }

  return (
    <>
      {!loaded && <HistoryIcon fontSize="small" sx={{ color: '#9ca3af' }} />}
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
  index: number;
}

const SuggestionItem: React.FC<SuggestionItemProps> = ({ suggestion, query, isFirst, isSelected, index }) => {
  const handleClick = () => {
    if (window.cefMessage) {
      window.cefMessage.send('navigate', suggestion.url);
      window.cefMessage.send('omnibox_hide');
    }
  };

  const highlightedTitle = highlightMatch(suggestion.title, query);

  const secondaryText = suggestion.type === 'history'
    ? formatUrl(suggestion.url)
    : null;

  return (
    <ListItem
      disablePadding
      sx={{
        animation: `omniboxSlideIn 0.12s ease-out ${index * 0.02}s both`,
        '@keyframes omniboxSlideIn': {
          '0%': { opacity: 0, transform: 'translateY(-4px)' },
          '100%': { opacity: 1, transform: 'translateY(0)' },
        },
      }}
    >
      <ListItemButton
        onClick={handleClick}
        disableRipple
        sx={{
          py: 0.75,
          px: 1.5,
          cursor: 'pointer !important',
          userSelect: 'none',
          backgroundColor: isSelected ? '#1a1a2e' : 'transparent',
          transition: 'background-color 0.1s ease',
          '&:hover': {
            backgroundColor: isSelected ? '#1a1a2e' : '#1f2937',
          },
          '&:focus': {
            backgroundColor: isSelected ? '#1a1a2e' : 'transparent',
          },
          '&:active': {
            backgroundColor: '#1f2937',
          },
          '&.Mui-focusVisible': {
            backgroundColor: isSelected ? '#1a1a2e' : 'transparent',
          },
        }}
      >
        <ListItemIcon sx={{ minWidth: 36, cursor: 'pointer', color: '#9ca3af' }}>
          {suggestion.type === 'history' ? (
            <FaviconIcon url={suggestion.url} />
          ) : (
            <SearchIcon fontSize="small" sx={{ color: '#9ca3af' }} />
          )}
        </ListItemIcon>
        <ListItemText
          primary={highlightedTitle}
          secondary={secondaryText}
          sx={{ cursor: 'pointer' }}
          primaryTypographyProps={{
            variant: 'body2',
            noWrap: true,
            sx: { fontWeight: isFirst || isSelected ? 500 : 400, cursor: 'pointer', color: '#f0f0f0' }
          }}
          secondaryTypographyProps={{
            variant: 'caption',
            noWrap: true,
            sx: { cursor: 'pointer', color: '#9ca3af' }
          }}
        />
      </ListItemButton>
    </ListItem>
  );
};

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

function formatUrl(url: string): string {
  try {
    const parsed = new URL(url);
    let formatted = parsed.hostname + parsed.pathname;
    if (formatted.endsWith('/')) {
      formatted = formatted.slice(0, -1);
    }
    if (formatted.length > 60) {
      formatted = formatted.slice(0, 57) + '...';
    }
    return formatted;
  } catch {
    return url.slice(0, 60);
  }
}

export default OmniboxOverlayRoot;
