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
