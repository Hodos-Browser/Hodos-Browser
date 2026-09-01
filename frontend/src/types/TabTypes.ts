/**
 * Tab type definitions for HodosBrowser tab management
 * Synced with C++ TabManager backend
 */

export interface Tab {
  id: number;
  title: string;
  url: string;
  isActive: boolean;
  isLoading: boolean;
  favicon?: string;
  hasCertError?: boolean;
  /** Per-tab audio mute. Read from CEF's `IsAudioMuted()` on every tab-list push —
   *  the browser host owns this state, we never mirror it. Session-lived: it survives
   *  navigating the tab and dies with the tab. */
  muted?: boolean;
  paymentIndicator?: {
    amount: string;
    timestamp: number;
  };
}

export interface TabListResponse {
  tabs: Tab[];
  activeTabId: number;
}

export interface TabManagerState {
  tabs: Tab[];
  activeTabId: number;
  isLoading: boolean;
}
