import { useState, useCallback } from 'react';
import type { HistoryEntry, HistorySearchParams, HistoryGetParams } from '../types/history';

export const useHistory = () => {
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchHistory = useCallback((params: HistoryGetParams = { limit: 50, offset: 0 }) => {
    console.log('📚 useHistory: fetchHistory called with params:', params);
    setLoading(true);
    setError(null);

    try {
      if (!window.hodosBrowser?.history) {
        console.error('❌ useHistory: History API not available');
        throw new Error('History API not available');
      }

      console.log('🔄 useHistory: Calling window.hodosBrowser.history.get()');
      const entries = window.hodosBrowser.history.get(params);
      console.log('✅ useHistory: Retrieved', entries.length, 'entries');

      setHistory(entries);
    } catch (err) {
      console.error('❌ useHistory: Error fetching history:', err);
      const errorMessage = err instanceof Error ? err.message : 'Failed to fetch history';
      setError(errorMessage);
    } finally {
      setLoading(false);
    }
  }, []);

  const searchHistory = useCallback((params: HistorySearchParams) => {
    console.log('🔍 useHistory: searchHistory called with params:', params);
    setLoading(true);
    setError(null);

    try {
      if (!window.hodosBrowser?.history) {
        console.error('❌ useHistory: History API not available');
        throw new Error('History API not available');
      }

      console.log('🔄 useHistory: Calling window.hodosBrowser.history.search()');
      const results = window.hodosBrowser.history.search(params);
      console.log('✅ useHistory: Search returned', results.length, 'entries');

      setHistory(results);
    } catch (err) {
      console.error('❌ useHistory: Error searching history:', err);
      const errorMessage = err instanceof Error ? err.message : 'Failed to search history';
      setError(errorMessage);
    } finally {
      setLoading(false);
    }
  }, []);

  const deleteEntry = useCallback((url: string) => {
    console.log('🗑️ useHistory: deleteEntry called for URL:', url);

    try {
      if (!window.hodosBrowser?.history) {
        console.error('❌ useHistory: History API not available');
        throw new Error('History API not available');
      }

      const success = window.hodosBrowser.history.delete(url);
      console.log('✅ useHistory: Delete result:', success);

      if (success) {
        setHistory(prev => prev.filter(entry => entry.url !== url));
      }

      return success;
    } catch (err) {
      console.error('❌ useHistory: Error deleting entry:', err);
      const errorMessage = err instanceof Error ? err.message : 'Failed to delete entry';
      setError(errorMessage);
      return false;
    }
  }, []);

  const clearAllHistory = useCallback(() => {
    console.log('🗑️ useHistory: clearAllHistory called');

    try {
      if (!window.hodosBrowser?.history) {
        console.error('❌ useHistory: History API not available');
        throw new Error('History API not available');
      }

      const success = window.hodosBrowser.history.clearAll();
      console.log('✅ useHistory: Clear all result:', success);

      if (success) {
        setHistory([]);
      }

      return success;
    } catch (err) {
      console.error('❌ useHistory: Error clearing history:', err);
      const errorMessage = err instanceof Error ? err.message : 'Failed to clear history';
      setError(errorMessage);
      return false;
    }
  }, []);

  const clearHistoryRange = useCallback((startTime: number, endTime: number) => {
    console.log('🗑️ useHistory: clearHistoryRange called');

    try {
      if (!window.hodosBrowser?.history) {
        console.error('❌ useHistory: History API not available');
        throw new Error('History API not available');
      }

      const success = window.hodosBrowser.history.clearRange({ startTime, endTime });
      console.log('✅ useHistory: Clear range result:', success);

      if (success) {
        // Refresh history after clearing range
        fetchHistory();
      }

      return success;
    } catch (err) {
      console.error('❌ useHistory: Error clearing range:', err);
      const errorMessage = err instanceof Error ? err.message : 'Failed to clear history range';
      setError(errorMessage);
      return false;
    }
  }, [fetchHistory]);

  // Utility function to convert Chromium timestamp to JavaScript Date
  const chromiumTimeToDate = useCallback((chromiumTime: number): Date => {
    const unixTimestamp = (chromiumTime / 1000000) - 11644473600;
    return new Date(unixTimestamp * 1000);
  }, []);

  // Utility function to convert JavaScript Date to Chromium timestamp
  const dateToChromiumTime = useCallback((date: Date): number => {
    const unixTimestamp = Math.floor(date.getTime() / 1000);
    return (unixTimestamp + 11644473600) * 1000000;
  }, []);

  return {
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
  };
};
