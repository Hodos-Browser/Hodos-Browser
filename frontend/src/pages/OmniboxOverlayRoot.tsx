import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Box, List, ListItem, ListItemButton, ListItemIcon, ListItemText, Typography, CircularProgress } from '@mui/material';
import HistoryIcon from '@mui/icons-material/History';
import SearchIcon from '@mui/icons-material/Search';
import { useOmniboxSuggestions } from '../hooks/useOmniboxSuggestions';
import { useFavicons, hostOf } from '../hooks/useFavicons';
import type { Suggestion } from '../types/omnibox';

const OmniboxOverlayRoot: React.FC = () => {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const { suggestions, loading, search } = useOmniboxSuggestions();

  // beta.3 Phase 7b — icons from our own store; no request per keystroke.
  const favicons = useFavicons(suggestions.map((sg) => hostOf(sg.url)));
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
              faviconSrc={favicons[hostOf(suggestion.url)]}
            />
          ))}
        </List>
      )}
    </Box>
  );
};

// beta.3 Phase 7b — `src` comes from OUR favicon store as a data: URI.
// ⛔ Was `google.com/s2/favicons?domain=…`, which fired on EVERY KEYSTROKE
// that matched a suggestion — a live feed of the user's typing to Google
// (and onward to t2.gstatic.com/faviconV2 with the full URL).
const FaviconIcon: React.FC<{ url: string; src?: string }> = ({ url, src }) => {
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);
  const domain = useMemo(() => {
    try { return new URL(url).hostname; } catch { return null; }
  }, [url]);

  if (!domain || failed || !src) {
    return <HistoryIcon fontSize="small" sx={{ color: '#9ca3af' }} />;
  }

  return (
    <>
      {!loaded && <HistoryIcon fontSize="small" sx={{ color: '#9ca3af' }} />}
      <img
        src={src}
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
  /** beta.3 Phase 7b — data: URI from our local favicon store, or undefined.
   *  ⛔ Passed DOWN rather than fetched per row: the parent asks once for every
   *  suggestion in one IPC. A per-row fetch would be slower than the Google
   *  request this replaces. */
  faviconSrc?: string;
}

const SuggestionItem: React.FC<SuggestionItemProps> = ({ suggestion, query, isFirst, isSelected, index, faviconSrc }) => {
  const handleClick = () => {
    if (window.cefMessage) {
      // beta.3 Phase 11 item 3 (`P11-I3`) — hand the URL to the HEADER before
      // navigating. 📏 Measured 2026-09-18: without this the page navigated in
      // 94 ms and the address bar still read the typed fragment 35 s later, because
      // clicking here deliberately does NOT blur the header input (the omnibox
      // WndProc returns MA_NOACTIVATE so the dropdown never steals the caret), so
      // `isEditingAddress` stayed true and the tab-sync effect returned early
      // forever. ⛔ The header cannot learn the URL any other way: `navigate` is
      // consumed by C++ and the tab list carries the URL only after OnTitleChange.
      window.cefMessage.send('omnibox_navigated', suggestion.url);
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
            <FaviconIcon url={suggestion.url} src={faviconSrc} />
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
