import { useState, useCallback, useRef, useEffect } from 'react';
import { useDebounce } from './useDebounce';
import { rankAndMergeSuggestions, getAutocompleteSuggestion } from '../utils/suggestionRanker';
import type { HistoryEntryWithFrecency, Suggestion } from '../types/omnibox';

interface UseOmniboxSuggestionsResult {
  suggestions: Suggestion[];
  loading: boolean;
  autocomplete: string | null;
  search: (query: string) => void;
}

export function useOmniboxSuggestions(): UseOmniboxSuggestionsResult {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [loading, setLoading] = useState(false);
  const [autocomplete, setAutocomplete] = useState<string | null>(null);

  // Track current query for stale request detection
  const currentQueryRef = useRef<string>('');
  const pendingGoogleRequestRef = useRef<number | null>(null);

  // Listen for Google Suggest responses
  useEffect(() => {
    const handleGoogleResponse = (event: Event) => {
      const customEvent = event as CustomEvent<{ suggestions: string[]; requestId: number }>;
      const { suggestions: googleSuggestions, requestId } = customEvent.detail;

      // Ignore stale responses
      if (pendingGoogleRequestRef.current !== requestId) {
        console.log('Ignoring stale Google response', requestId, 'expected', pendingGoogleRequestRef.current);
        return;
      }

      pendingGoogleRequestRef.current = null;

      // Get current history results (they should already be set)
      // We need to merge with the current suggestions that are history-only
      const currentHistory = suggestions.filter(s => s.type === 'history');

      // Re-rank with Google results
      const historyEntries: HistoryEntryWithFrecency[] = currentHistory.map(s => ({
        url: s.url,
        title: s.title,
        visitCount: 0,
        lastVisitTime: 0,
        frecencyScore: s.score
      }));

      const merged = rankAndMergeSuggestions(historyEntries, googleSuggestions, currentQueryRef.current);
      setSuggestions(merged);

      const autocompleteSuggestion = getAutocompleteSuggestion(merged, currentQueryRef.current);
      setAutocomplete(autocompleteSuggestion);
      setLoading(false);
    };

    window.addEventListener('googleSuggestResponse', handleGoogleResponse);
    return () => {
      window.removeEventListener('googleSuggestResponse', handleGoogleResponse);
    };
  }, [suggestions]);

  // Main search function
  const performSearch = useCallback((query: string) => {
    currentQueryRef.current = query;

    if (!query || query.length < 1) {
      setSuggestions([]);
      setAutocomplete(null);
      setLoading(false);
      return;
    }

    setLoading(true);

    // Fetch history immediately (synchronous V8 call)
    let historyResults: HistoryEntryWithFrecency[] = [];
    try {
      if (window.hodosBrowser?.history?.searchWithFrecency) {
        historyResults = window.hodosBrowser.history.searchWithFrecency({ query, limit: 6 });
      }
    } catch (error) {
      console.warn('History search failed:', error);
    }

    // Create initial suggestions from history
    const historySuggestions = rankAndMergeSuggestions(historyResults, [], query);
    setSuggestions(historySuggestions);

    // Set autocomplete from history immediately
    const autocompleteSuggestion = getAutocompleteSuggestion(historySuggestions, query);
    setAutocomplete(autocompleteSuggestion);

    // Always fetch Google suggestions for queries >= 2 chars
    // (We'll limit display to 6 total in the ranking logic)
    if (query.length >= 2) {
      try {
        if (window.hodosBrowser?.googleSuggest?.fetch) {
          const requestId = window.hodosBrowser.googleSuggest.fetch(query);
          pendingGoogleRequestRef.current = requestId;
          // Response will come via event listener
        } else {
          setLoading(false);
        }
      } catch (error) {
        console.warn('Google suggest request failed:', error);
        setLoading(false);
      }
    } else {
      setLoading(false);
    }
  }, []);

  // Debounce the search for Google API (200ms), but history is immediate
  const debouncedGoogleFetch = useDebounce((query: string) => {
    if (query.length >= 2 && currentQueryRef.current === query) {
      // Fetch Google suggestions (already handled in performSearch above)
      // This debounced version is redundant but kept for future use
    }
  }, 200);

  // Combined search: history immediate, Google debounced
  const search = useCallback((query: string) => {
    performSearch(query);
    debouncedGoogleFetch(query);
  }, [performSearch, debouncedGoogleFetch]);

  return {
    suggestions,
    loading,
    autocomplete,
    search
  };
}
