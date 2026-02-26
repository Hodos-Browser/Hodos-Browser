import { useState, useEffect, useCallback } from 'react';

// Types matching C++ structs
export interface DetectedProfile {
  browserName: string;
  profilePath: string;
  profileName: string;
  hasBookmarks: boolean;
  hasHistory: boolean;
  bookmarkCount: number;
  historyCount: number;
}

export interface ImportResult {
  success: boolean;
  error: string;
  bookmarksImported: number;
  foldersImported: number;
  historyImported: number;
  skipped: number;
}

export function useImport() {
  const [profiles, setProfiles] = useState<DetectedProfile[]>([]);
  const [loading, setLoading] = useState(true);
  const [importing, setImporting] = useState(false);
  const [lastResult, setLastResult] = useState<ImportResult | null>(null);

  // Load profiles on mount
  useEffect(() => {
    // Set up response handlers
    window.onImportProfilesResult = (data: DetectedProfile[]) => {
      console.log('📂 Profiles detected:', data);
      setProfiles(data);
      setLoading(false);
    };

    window.onImportComplete = (result: ImportResult) => {
      console.log('📦 Import complete:', result);
      setLastResult(result);
      setImporting(false);
    };

    // Request profile detection
    if (window.cefMessage?.send) {
      console.log('📂 Requesting profile detection...');
      window.cefMessage.send('import_detect_profiles');
    } else {
      console.warn('⚠️ cefMessage not available');
      setLoading(false);
    }

    return () => {
      window.onImportProfilesResult = undefined;
      window.onImportComplete = undefined;
    };
  }, []);

  // Refresh profile detection
  const refresh = useCallback(() => {
    setLoading(true);
    if (window.cefMessage?.send) {
      window.cefMessage.send('import_detect_profiles');
    }
  }, []);

  // Import bookmarks only
  const importBookmarks = useCallback((profilePath: string) => {
    setImporting(true);
    setLastResult(null);
    if (window.cefMessage?.send) {
      console.log('📚 Starting bookmark import from:', profilePath);
      window.cefMessage.send('import_bookmarks', profilePath);
    }
  }, []);

  // Import history only
  const importHistory = useCallback((profilePath: string, maxEntries: number = 10000) => {
    setImporting(true);
    setLastResult(null);
    if (window.cefMessage?.send) {
      console.log('📜 Starting history import from:', profilePath);
      window.cefMessage.send('import_history', profilePath, String(maxEntries));
    }
  }, []);

  // Import both bookmarks and history
  const importAll = useCallback((profilePath: string, maxHistoryEntries: number = 10000) => {
    setImporting(true);
    setLastResult(null);
    if (window.cefMessage?.send) {
      console.log('📦 Starting full import from:', profilePath);
      window.cefMessage.send('import_all', profilePath, String(maxHistoryEntries));
    }
  }, []);

  return {
    profiles,
    loading,
    importing,
    lastResult,
    refresh,
    importBookmarks,
    importHistory,
    importAll,
  };
}

// Extend window type for TypeScript
declare global {
  interface Window {
    onImportProfilesResult?: (data: DetectedProfile[]) => void;
    onImportComplete?: (result: ImportResult) => void;
  }
}
