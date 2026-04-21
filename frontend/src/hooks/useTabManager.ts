import { useState, useEffect, useCallback, useRef } from 'react';
import type { TabListResponse, TabManagerState } from '../types/TabTypes';

/**
 * Hook for managing browser tabs
 * Communicates with C++ TabManager backend via window.cefMessage
 */
export const useTabManager = () => {
  const [state, setState] = useState<TabManagerState>({
    tabs: [],
    activeTabId: -1,
    isLoading: true,
  });

  // Track recently closed tab IDs so incoming tab_list_response doesn't re-add them
  // during the async window between IPC close and CEF OnBeforeClose cleanup
  const recentlyClosedRef = useRef<Set<number>>(new Set());

  // Fetch tab list from backend
  const refreshTabList = useCallback(() => {
    if (window.cefMessage) {
      window.cefMessage.send('get_tab_list');
    }
  }, []);

  // Create a new tab
  const createTab = useCallback((url?: string) => {
    if (window.cefMessage) {
      window.cefMessage.send('tab_create', url || 'http://127.0.0.1:5137/newtab');
      // Refresh tab list after a short delay to get updated state
      setTimeout(refreshTabList, 500);
    }
  }, [refreshTabList]);

  // Close a tab — optimistic removal for instant visual feedback
  const closeTab = useCallback((tabId: number) => {
    if (window.cefMessage) {
      // Send close to C++ (backend creates NTP if this was the last tab)
      window.cefMessage.send('tab_close', tabId);

      // Suppress this tab from incoming tab_list_response until C++ confirms removal
      recentlyClosedRef.current.add(tabId);
      setTimeout(() => recentlyClosedRef.current.delete(tabId), 3000);

      // Remove tab from local state immediately
      setState(prev => {
        const remaining = prev.tabs.filter(t => t.id !== tabId);

        // If closing the last tab, don't update local state to empty —
        // C++ will create a new NTP tab and push an updated tab list
        if (remaining.length === 0) {
          return prev;
        }

        let newActiveId = prev.activeTabId;
        if (tabId === prev.activeTabId && remaining.length > 0) {
          const closedIndex = prev.tabs.findIndex(t => t.id === tabId);
          const newIndex = Math.min(closedIndex, remaining.length - 1);
          newActiveId = remaining[newIndex].id;
        }
        return {
          ...prev,
          tabs: remaining.map(t => ({ ...t, isActive: t.id === newActiveId })),
          activeTabId: newActiveId,
        };
      });
    }
  }, []);

  // Switch to a tab
  const switchToTab = useCallback((tabId: number) => {
    if (window.cefMessage) {
      window.cefMessage.send('tab_switch', tabId);
      // Update local state immediately for responsiveness
      setState(prev => ({
        ...prev,
        activeTabId: tabId,
        tabs: prev.tabs.map(tab => ({
          ...tab,
          isActive: tab.id === tabId,
        })),
      }));
    }
  }, []);

  // Switch to next tab (for Ctrl+Tab)
  const nextTab = useCallback(() => {
    const currentIndex = state.tabs.findIndex(t => t.id === state.activeTabId);
    if (currentIndex !== -1 && state.tabs.length > 1) {
      const nextIndex = (currentIndex + 1) % state.tabs.length;
      switchToTab(state.tabs[nextIndex].id);
    }
  }, [state.tabs, state.activeTabId, switchToTab]);

  // Switch to previous tab (for Ctrl+Shift+Tab)
  const prevTab = useCallback(() => {
    const currentIndex = state.tabs.findIndex(t => t.id === state.activeTabId);
    if (currentIndex !== -1 && state.tabs.length > 1) {
      const prevIndex = (currentIndex - 1 + state.tabs.length) % state.tabs.length;
      switchToTab(state.tabs[prevIndex].id);
    }
  }, [state.tabs, state.activeTabId, switchToTab]);

  // Switch to tab by index (for Ctrl+1-9)
  const switchToTabByIndex = useCallback((index: number) => {
    if (index >= 0 && index < state.tabs.length) {
      switchToTab(state.tabs[index].id);
    }
  }, [state.tabs, switchToTab]);

  // Close active tab
  const closeActiveTab = useCallback(() => {
    if (state.activeTabId !== -1) {
      closeTab(state.activeTabId);
    }
  }, [state.activeTabId, closeTab]);

  // Tear off tab to new window or merge into another window
  const tearOffTab = useCallback((tabId: number, screenX: number, screenY: number) => {
    window.cefMessage?.send('tab_tearoff', tabId, screenX, screenY);
  }, []);

  // Reorder tabs (drag-and-drop)
  const reorderTabs = useCallback((fromIndex: number, toIndex: number) => {
    setState(prev => {
      const newTabs = [...prev.tabs];
      const [moved] = newTabs.splice(fromIndex, 1);
      newTabs.splice(toIndex, 0, moved);
      // Send new order to C++ backend
      const newOrder = newTabs.map(t => t.id);
      window.cefMessage?.send('tab_reorder', JSON.stringify(newOrder));
      return { ...prev, tabs: newTabs };
    });
  }, []);

  // Listen for payment success indicators from C++ auto-approve engine
  useEffect(() => {
    const PAYMENT_BADGE_DURATION_MS = 6000;

    const handlePaymentIndicator = (event: MessageEvent) => {
      if (event.data?.type === 'payment_success_indicator') {
        try {
          const { cents, domain } = JSON.parse(event.data.data);
          const amount = cents > 0 ? `$${(cents / 100).toFixed(2)}` : '< $0.01';

          // Match payment to the specific tab that made the request.
          // Only badge ONE tab — find the first match by domain (tab.id != CEF browserId).
          setState(prev => {
            let matched = false;
            return {
              ...prev,
              tabs: prev.tabs.map(tab => {
                if (matched) return tab;
                const tabDomain = (() => {
                  try { return new URL(tab.url).hostname; } catch { return ''; }
                })();
                if (tabDomain === domain || ('.' + tabDomain).endsWith('.' + domain)) {
                  matched = true;
                  return {
                    ...tab,
                    paymentIndicator: { amount, timestamp: Date.now() },
                  };
                }
                return tab;
              }),
            };
          });

          // Auto-clear the indicator after badge duration
          setTimeout(() => {
            setState(prev => ({
              ...prev,
              tabs: prev.tabs.map(tab =>
                tab.paymentIndicator && Date.now() - tab.paymentIndicator.timestamp >= PAYMENT_BADGE_DURATION_MS
                  ? { ...tab, paymentIndicator: undefined }
                  : tab
              ),
            }));
          }, PAYMENT_BADGE_DURATION_MS);
        } catch (error) {
          console.error('Failed to parse payment indicator:', error);
        }
      }
    };

    window.addEventListener('message', handlePaymentIndicator);
    return () => window.removeEventListener('message', handlePaymentIndicator);
  }, []);

  // Listen for tab list updates from C++
  useEffect(() => {
    const handleTabListResponse = (event: MessageEvent) => {
      if (event.data && event.data.type === 'tab_list_response') {
        try {
          const response: TabListResponse = JSON.parse(event.data.data);
          // Filter out tabs that were optimistically closed but C++ hasn't confirmed yet
          const closed = recentlyClosedRef.current;
          const filteredTabs = closed.size > 0
            ? response.tabs.filter(t => !closed.has(t.id))
            : response.tabs;
          // If C++ no longer includes the tab, clear it from the suppression set
          const serverIds = new Set(response.tabs.map(t => t.id));
          closed.forEach(id => { if (!serverIds.has(id)) closed.delete(id); });
          setState({
            tabs: filteredTabs,
            activeTabId: response.activeTabId,
            isLoading: false,
          });
        } catch (error) {
          console.error('Failed to parse tab list response:', error);
        }
      }
    };

    window.addEventListener('message', handleTabListResponse);

    // Initial fetch
    refreshTabList();

    // Safety-net polling — C++ now pushes updates on create/close/title change,
    // so this only catches edge cases (favicon, loading state). F13 perf fix.
    const interval = setInterval(refreshTabList, 30000);

    return () => {
      window.removeEventListener('message', handleTabListResponse);
      clearInterval(interval);
    };
  }, [refreshTabList]);

  return {
    tabs: state.tabs,
    activeTabId: state.activeTabId,
    isLoading: state.isLoading,
    createTab,
    closeTab,
    switchToTab,
    nextTab,
    prevTab,
    switchToTabByIndex,
    closeActiveTab,
    reorderTabs,
    tearOffTab,
    refreshTabList,
  };
};
