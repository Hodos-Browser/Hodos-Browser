import { useState, useEffect, useCallback } from 'react';
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

  // Close a tab
  const closeTab = useCallback((tabId: number) => {
    if (window.cefMessage) {
      window.cefMessage.send('tab_close', tabId);
      // Refresh tab list after a short delay to get updated state
      setTimeout(refreshTabList, 500);
    }
  }, [refreshTabList]);

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

  // Listen for tab list updates from C++
  useEffect(() => {
    const handleTabListResponse = (event: MessageEvent) => {
      if (event.data && event.data.type === 'tab_list_response') {
        try {
          const response: TabListResponse = JSON.parse(event.data.data);
          setState({
            tabs: response.tabs,
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

    // Refresh periodically to keep in sync (every 2 seconds)
    const interval = setInterval(refreshTabList, 2000);

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
