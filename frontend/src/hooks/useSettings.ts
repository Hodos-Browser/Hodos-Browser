import { useState, useEffect, useCallback } from 'react';

// Settings types matching C++ structs
export interface BrowserSettings {
  homepage: string;
  searchEngine: string;
  zoomLevel: number;
  showBookmarkBar: boolean;
  downloadsPath: string;
  restoreSessionOnStart: boolean;
}

export interface PrivacySettings {
  adBlockEnabled: boolean;
  thirdPartyCookieBlocking: boolean;
  doNotTrack: boolean;
  clearDataOnExit: boolean;
}

export interface WalletSettings {
  autoApproveEnabled: boolean;
  defaultPerTxLimitCents: number;
  defaultPerSessionLimitCents: number;
  defaultRateLimitPerMin: number;
}

export interface AllSettings {
  version: number;
  browser: BrowserSettings;
  privacy: PrivacySettings;
  wallet: WalletSettings;
}

// Default settings (same as C++)
const defaultSettings: AllSettings = {
  version: 1,
  browser: {
    homepage: 'about:blank',
    searchEngine: 'google',
    zoomLevel: 0.0,
    showBookmarkBar: false,
    downloadsPath: '',
    restoreSessionOnStart: false,
  },
  privacy: {
    adBlockEnabled: true,
    thirdPartyCookieBlocking: true,
    doNotTrack: false,
    clearDataOnExit: false,
  },
  wallet: {
    autoApproveEnabled: true,
    defaultPerTxLimitCents: 10,
    defaultPerSessionLimitCents: 300,
    defaultRateLimitPerMin: 10,
  },
};

export function useSettings() {
  const [settings, setSettings] = useState<AllSettings>(defaultSettings);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Load settings on mount
  useEffect(() => {
    // Set up response handler
    window.onSettingsResponse = (data: AllSettings) => {
      console.log('📋 Settings received:', data);
      setSettings(data);
      setLoading(false);
      setError(null);
    };

    // Request settings from backend
    if (window.cefMessage?.send) {
      console.log('📋 Requesting settings from backend...');
      window.cefMessage.send('settings_get_all');
    } else {
      console.warn('⚠️ cefMessage not available, using default settings');
      setLoading(false);
    }

    return () => {
      window.onSettingsResponse = undefined;
    };
  }, []);

  // Update a single setting
  const updateSetting = useCallback((key: string, value: string | number | boolean) => {
    console.log(`📋 Updating setting: ${key} = ${value}`);
    
    // Convert value to string for IPC
    const valueStr = typeof value === 'boolean' 
      ? (value ? 'true' : 'false')
      : String(value);

    if (window.cefMessage?.send) {
      window.cefMessage.send('settings_set', key, valueStr);
    }

    // Update local state immediately for responsiveness
    setSettings(prev => {
      const newSettings = { ...prev };
      const [section, field] = key.split('.') as [keyof AllSettings, string];
      
      if (section === 'browser' && field in newSettings.browser) {
        newSettings.browser = { ...newSettings.browser, [field]: value };
      } else if (section === 'privacy' && field in newSettings.privacy) {
        newSettings.privacy = { ...newSettings.privacy, [field]: value };
      } else if (section === 'wallet' && field in newSettings.wallet) {
        newSettings.wallet = { ...newSettings.wallet, [field]: value };
      }
      
      return newSettings;
    });
  }, []);

  // Refresh settings from backend
  const refresh = useCallback(() => {
    setLoading(true);
    if (window.cefMessage?.send) {
      window.cefMessage.send('settings_get_all');
    }
  }, []);

  return {
    settings,
    loading,
    error,
    updateSetting,
    refresh,
  };
}

// Extend window type for TypeScript
declare global {
  interface Window {
    onSettingsResponse?: (data: AllSettings) => void;
  }
}
