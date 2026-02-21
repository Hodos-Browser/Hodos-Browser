> **NOTE**: This document was written in December 2025. The feature comparison is still valuable, but implementation priorities have been superseded by `development-docs/browser-core/mvp-gap-analysis.md` and `development-docs/browser-core/implementation-plan.md` (February 2026). Refer to those for the current MVP plan.

# HodosBrowser UX Feature Comparison & Implementation Roadmap

**Document Version**: 1.0
**Date**: December 15, 2025
**Author**: Development Team
**Purpose**: Compare HodosBrowser UX to modern browsers (Chrome) and provide detailed implementation steps

---

## Executive Summary

**Current State**: HodosBrowser has ~5-10% of Chrome's UX features, with most development focused on BSV wallet integration rather than traditional browser functionality.

**Completion Status**:
- ✅ **Wallet Features**: 95% complete (world-class BSV integration)
- ⚠️ **Core Browser Features**: 15% complete
- ❌ **Advanced Browser Features**: 0% complete

**Total Features Analyzed**: 45 major UX features
**Implemented**: 7 features
**Partially Implemented**: 3 features
**Missing**: 35 features

---

## Table of Contents

1. [Feature Comparison Matrix](#feature-comparison-matrix)
2. [Priority 1: Critical Features (Must Have)](#priority-1-critical-features-must-have)
3. [Priority 2: Essential Features (Should Have)](#priority-2-essential-features-should-have)
4. [Priority 3: Important Features (Nice to Have)](#priority-3-important-features-nice-to-have)
5. [Priority 4: Advanced Features (Future)](#priority-4-advanced-features-future)
6. [Technical Architecture Recommendations](#technical-architecture-recommendations)
7. [Development Timeline Estimates](#development-timeline-estimates)

---

## Feature Comparison Matrix

| Feature | Chrome | HodosBrowser | Gap | Priority |
|---------|--------|--------------|-----|----------|
| **Tab Management** | ✅ Full | ❌ None | 100% | P1 |
| **Address Bar Sync** | ✅ Full | ❌ Static | 100% | P1 |
| **Search Integration** | ✅ Full | ❌ None | 100% | P1 |
| **Navigation Controls** | ✅ Full | ⚠️ Basic | 60% | P1 |
| **Keyboard Shortcuts** | ✅ Full | ❌ None | 100% | P1 |
| **Bookmarks** | ✅ Full | ❌ None | 100% | P2 |
| **History** | ✅ Full | ❌ None | 100% | P2 |
| **Downloads Manager** | ✅ Full | ❌ None | 100% | P2 |
| **Context Menus** | ✅ Full | ❌ None | 100% | P2 |
| **Settings Panel** | ✅ Full | ⚠️ Empty | 95% | P2 |
| **Find in Page** | ✅ Full | ❌ None | 100% | P2 |
| **Security Indicators** | ✅ Full | ❌ None | 100% | P2 |
| **Zoom Controls** | ✅ Full | ❌ None | 100% | P3 |
| **Print** | ✅ Full | ❌ None | 100% | P3 |
| **Extensions** | ✅ Full | ❌ None | 100% | P4 |
| **DevTools** | ✅ Full | ❌ None | 100% | P4 |
| **Wallet Integration** | ❌ None | ✅ Full | -100% | ✅ |

**Legend**:
- ✅ Full = Fully implemented
- ⚠️ Partial = Partially implemented
- ❌ None = Not implemented
- P1-P4 = Priority levels

---

## Priority 1: Critical Features (Must Have)

These features are **essential for basic browser functionality**. Without them, HodosBrowser cannot function as a viable web browser.

---

### 1.1 Tab Management System

**Status**: ❌ **NOT IMPLEMENTED** (0%)
**Chrome Feature Level**: 100%
**User Impact**: **CRITICAL** - Users expect multi-tab browsing
**Development Effort**: 🔴 High (5-7 days)

#### What Chrome Has
- Tab bar with multiple tabs
- New tab button (+)
- Tab close buttons (X)
- Active tab highlighting
- Tab switching (Ctrl+Tab, Ctrl+1-9)
- Tab reordering (drag & drop)
- Tab pinning
- Tab groups with colors
- Tab previews on hover
- Recently closed tabs (Ctrl+Shift+T)
- Tab context menu (duplicate, pin, close others)
- Tab overflow handling (scroll/arrow buttons)
- New tab page with search + shortcuts

#### What HodosBrowser Has
- **Nothing** - Single window only

#### Implementation Steps

##### Step 1: Create Tab Data Structure (1 day)

**File**: `frontend/src/types/TabTypes.ts`

```typescript
export interface Tab {
  id: string;
  title: string;
  url: string;
  favicon?: string;
  isLoading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  isPinned: boolean;
  groupId?: string;
  createdAt: number;
}

export interface TabGroup {
  id: string;
  name: string;
  color: string;
  collapsed: boolean;
}

export interface TabState {
  tabs: Tab[];
  activeTabId: string;
  groups: TabGroup[];
}
```

**File**: `frontend/src/hooks/useTabManager.ts`

```typescript
import { useState, useCallback } from 'react';
import { Tab, TabState } from '../types/TabTypes';
import { v4 as uuidv4 } from 'uuid';

export const useTabManager = () => {
  const [tabState, setTabState] = useState<TabState>({
    tabs: [
      {
        id: uuidv4(),
        title: 'New Tab',
        url: 'https://metanetapps.com/',
        isLoading: false,
        canGoBack: false,
        canGoForward: false,
        isPinned: false,
        createdAt: Date.now(),
      }
    ],
    activeTabId: '', // Set to first tab ID
    groups: [],
  });

  const createTab = useCallback((url?: string) => {
    const newTab: Tab = {
      id: uuidv4(),
      title: 'New Tab',
      url: url || 'chrome://newtab',
      isLoading: false,
      canGoBack: false,
      canGoForward: false,
      isPinned: false,
      createdAt: Date.now(),
    };

    setTabState(prev => ({
      ...prev,
      tabs: [...prev.tabs, newTab],
      activeTabId: newTab.id,
    }));

    // Send message to CEF to create browser view
    window.cefMessage.send('tab_create', [newTab.id, url || '']);
  }, []);

  const closeTab = useCallback((tabId: string) => {
    setTabState(prev => {
      const newTabs = prev.tabs.filter(t => t.id !== tabId);

      // If closing active tab, switch to adjacent tab
      let newActiveId = prev.activeTabId;
      if (tabId === prev.activeTabId && newTabs.length > 0) {
        const closedIndex = prev.tabs.findIndex(t => t.id === tabId);
        newActiveId = newTabs[Math.min(closedIndex, newTabs.length - 1)].id;
      }

      return {
        ...prev,
        tabs: newTabs,
        activeTabId: newActiveId,
      };
    });

    // Send message to CEF to destroy browser view
    window.cefMessage.send('tab_close', [tabId]);
  }, []);

  const switchTab = useCallback((tabId: string) => {
    setTabState(prev => ({ ...prev, activeTabId: tabId }));
    window.cefMessage.send('tab_switch', [tabId]);
  }, []);

  const updateTab = useCallback((tabId: string, updates: Partial<Tab>) => {
    setTabState(prev => ({
      ...prev,
      tabs: prev.tabs.map(tab =>
        tab.id === tabId ? { ...tab, ...updates } : tab
      ),
    }));
  }, []);

  const reorderTab = useCallback((tabId: string, newIndex: number) => {
    setTabState(prev => {
      const tabs = [...prev.tabs];
      const oldIndex = tabs.findIndex(t => t.id === tabId);
      const [movedTab] = tabs.splice(oldIndex, 1);
      tabs.splice(newIndex, 0, movedTab);
      return { ...prev, tabs };
    });
  }, []);

  const pinTab = useCallback((tabId: string) => {
    setTabState(prev => ({
      ...prev,
      tabs: prev.tabs.map(tab =>
        tab.id === tabId ? { ...tab, isPinned: !tab.isPinned } : tab
      ),
    }));
  }, []);

  return {
    tabs: tabState.tabs,
    activeTabId: tabState.activeTabId,
    createTab,
    closeTab,
    switchTab,
    updateTab,
    reorderTab,
    pinTab,
  };
};
```

##### Step 2: Create Tab Bar Component (2 days)

**File**: `frontend/src/components/TabBar.tsx`

```typescript
import React from 'react';
import { Box, IconButton, Tooltip } from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import { Tab } from '../types/TabTypes';
import { TabComponent } from './TabComponent';

interface TabBarProps {
  tabs: Tab[];
  activeTabId: string;
  onCreateTab: () => void;
  onCloseTab: (tabId: string) => void;
  onSwitchTab: (tabId: string) => void;
  onReorderTab: (tabId: string, newIndex: number) => void;
}

export const TabBar: React.FC<TabBarProps> = ({
  tabs,
  activeTabId,
  onCreateTab,
  onCloseTab,
  onSwitchTab,
  onReorderTab,
}) => {
  return (
    <Box
      sx={{
        display: 'flex',
        alignItems: 'center',
        backgroundColor: 'grey.900',
        borderBottom: '1px solid',
        borderColor: 'grey.800',
        height: 40,
        overflowX: 'auto',
        overflowY: 'hidden',
        '&::-webkit-scrollbar': {
          height: 4,
        },
        '&::-webkit-scrollbar-thumb': {
          backgroundColor: 'grey.700',
          borderRadius: 2,
        },
      }}
    >
      {/* Render pinned tabs first */}
      {tabs
        .filter(tab => tab.isPinned)
        .map((tab, index) => (
          <TabComponent
            key={tab.id}
            tab={tab}
            isActive={tab.id === activeTabId}
            onClose={() => onCloseTab(tab.id)}
            onClick={() => onSwitchTab(tab.id)}
            onReorder={(newIndex) => onReorderTab(tab.id, newIndex)}
          />
        ))}

      {/* Render unpinned tabs */}
      {tabs
        .filter(tab => !tab.isPinned)
        .map((tab, index) => (
          <TabComponent
            key={tab.id}
            tab={tab}
            isActive={tab.id === activeTabId}
            onClose={() => onCloseTab(tab.id)}
            onClick={() => onSwitchTab(tab.id)}
            onReorder={(newIndex) => onReorderTab(tab.id, newIndex)}
          />
        ))}

      {/* New Tab Button */}
      <Tooltip title="New tab (Ctrl+T)">
        <IconButton
          onClick={onCreateTab}
          size="small"
          sx={{
            minWidth: 32,
            height: 32,
            borderRadius: '50%',
            ml: 1,
            '&:hover': {
              backgroundColor: 'grey.800',
            },
          }}
        >
          <AddIcon fontSize="small" sx={{ color: 'grey.400' }} />
        </IconButton>
      </Tooltip>
    </Box>
  );
};
```

**File**: `frontend/src/components/TabComponent.tsx`

```typescript
import React, { useState } from 'react';
import { Box, IconButton, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import PublicIcon from '@mui/icons-material/Public';
import { Tab } from '../types/TabTypes';

interface TabComponentProps {
  tab: Tab;
  isActive: boolean;
  onClose: () => void;
  onClick: () => void;
  onReorder: (newIndex: number) => void;
}

export const TabComponent: React.FC<TabComponentProps> = ({
  tab,
  isActive,
  onClose,
  onClick,
  onReorder,
}) => {
  const [isDragging, setIsDragging] = useState(false);

  const handleDragStart = (e: React.DragEvent) => {
    setIsDragging(true);
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('tabId', tab.id);
  };

  const handleDragEnd = () => {
    setIsDragging(false);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    const draggedTabId = e.dataTransfer.getData('tabId');
    if (draggedTabId !== tab.id) {
      // Calculate new index based on drop position
      // This is simplified - you'd need more logic for precise positioning
      onReorder(0); // Placeholder
    }
  };

  return (
    <Box
      draggable
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      onDrop={handleDrop}
      onDragOver={(e) => e.preventDefault()}
      onClick={onClick}
      sx={{
        display: 'flex',
        alignItems: 'center',
        gap: 1,
        px: 2,
        py: 1,
        minWidth: tab.isPinned ? 40 : 200,
        maxWidth: tab.isPinned ? 40 : 240,
        height: '100%',
        backgroundColor: isActive ? 'grey.800' : 'transparent',
        borderRight: '1px solid',
        borderColor: 'grey.800',
        cursor: 'pointer',
        opacity: isDragging ? 0.5 : 1,
        transition: 'background-color 0.2s',
        '&:hover': {
          backgroundColor: isActive ? 'grey.800' : 'grey.850',
          '& .tab-close-btn': {
            opacity: 1,
          },
        },
      }}
    >
      {/* Favicon */}
      {tab.favicon ? (
        <img src={tab.favicon} alt="" width={16} height={16} />
      ) : (
        <PublicIcon sx={{ fontSize: 16, color: 'grey.500' }} />
      )}

      {/* Tab Title (hide for pinned tabs) */}
      {!tab.isPinned && (
        <Typography
          variant="body2"
          sx={{
            flex: 1,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
            fontSize: 13,
            color: isActive ? 'white' : 'grey.400',
          }}
        >
          {tab.title}
        </Typography>
      )}

      {/* Close Button */}
      <IconButton
        className="tab-close-btn"
        onClick={(e) => {
          e.stopPropagation();
          onClose();
        }}
        size="small"
        sx={{
          width: 20,
          height: 20,
          opacity: isActive ? 1 : 0,
          transition: 'opacity 0.2s',
          '&:hover': {
            backgroundColor: 'grey.700',
          },
        }}
      >
        <CloseIcon sx={{ fontSize: 14, color: 'grey.400' }} />
      </IconButton>
    </Box>
  );
};
```

##### Step 3: Integrate Tab Bar into Main Browser View (1 day)

**File**: `frontend/src/pages/MainBrowserView.tsx` (modify)

```typescript
import React from 'react';
import { Box } from '@mui/material';
import { TabBar } from '../components/TabBar';
import { NavigationBar } from '../components/NavigationBar';
import { useTabManager } from '../hooks/useTabManager';

export const MainBrowserView: React.FC = () => {
  const {
    tabs,
    activeTabId,
    createTab,
    closeTab,
    switchTab,
    reorderTab,
  } = useTabManager();

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
      {/* Tab Bar */}
      <TabBar
        tabs={tabs}
        activeTabId={activeTabId}
        onCreateTab={createTab}
        onCloseTab={closeTab}
        onSwitchTab={switchTab}
        onReorderTab={reorderTab}
      />

      {/* Navigation Bar (existing) */}
      <NavigationBar activeTab={tabs.find(t => t.id === activeTabId)} />

      {/* Browser Content Area (CEF renders here) */}
      <Box sx={{ flex: 1 }} id="browser-content" />
    </Box>
  );
};
```

##### Step 4: Add Keyboard Shortcuts (1 day)

**File**: `frontend/src/hooks/useKeyboardShortcuts.ts`

```typescript
import { useEffect } from 'react';

interface KeyboardShortcutHandlers {
  onNewTab: () => void;
  onCloseTab: () => void;
  onNextTab: () => void;
  onPrevTab: () => void;
  onReopenTab: () => void;
  onSwitchToTab: (index: number) => void;
}

export const useKeyboardShortcuts = (handlers: KeyboardShortcutHandlers) => {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl+T - New Tab
      if (e.ctrlKey && e.key === 't') {
        e.preventDefault();
        handlers.onNewTab();
      }

      // Ctrl+W - Close Tab
      if (e.ctrlKey && e.key === 'w') {
        e.preventDefault();
        handlers.onCloseTab();
      }

      // Ctrl+Tab - Next Tab
      if (e.ctrlKey && e.key === 'Tab' && !e.shiftKey) {
        e.preventDefault();
        handlers.onNextTab();
      }

      // Ctrl+Shift+Tab - Previous Tab
      if (e.ctrlKey && e.key === 'Tab' && e.shiftKey) {
        e.preventDefault();
        handlers.onPrevTab();
      }

      // Ctrl+Shift+T - Reopen Closed Tab
      if (e.ctrlKey && e.shiftKey && e.key === 't') {
        e.preventDefault();
        handlers.onReopenTab();
      }

      // Ctrl+1-9 - Switch to Tab by Index
      if (e.ctrlKey && e.key >= '1' && e.key <= '9') {
        e.preventDefault();
        handlers.onSwitchToTab(parseInt(e.key) - 1);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handlers]);
};
```

##### Step 5: Backend CEF Integration (C++) (2-3 days)

**File**: `cef-native/include/core/TabManager.h` (NEW)

```cpp
#ifndef TAB_MANAGER_H
#define TAB_MANAGER_H

#include <map>
#include <string>
#include "include/cef_browser.h"

class TabManager {
public:
    TabManager();
    ~TabManager();

    // Tab lifecycle
    void CreateTab(const std::string& tab_id, const std::string& url);
    void CloseTab(const std::string& tab_id);
    void SwitchTab(const std::string& tab_id);

    // Tab state
    CefRefPtr<CefBrowser> GetBrowser(const std::string& tab_id);
    CefRefPtr<CefBrowser> GetActiveBrowser();
    std::string GetActiveTabId() const;

private:
    std::map<std::string, CefRefPtr<CefBrowser>> browsers_;
    std::string active_tab_id_;

    IMPLEMENT_REFCOUNTING(TabManager);
};

#endif // TAB_MANAGER_H
```

**File**: `cef-native/src/core/TabManager.cpp` (NEW)

```cpp
#include "TabManager.h"
#include "include/cef_app.h"

TabManager::TabManager() {}

TabManager::~TabManager() {
    browsers_.clear();
}

void TabManager::CreateTab(const std::string& tab_id, const std::string& url) {
    // Create new browser instance
    CefWindowInfo window_info;
    CefBrowserSettings settings;

    // Browser will be created asynchronously
    // Store tab_id for later association

    LOG(INFO) << "Creating tab: " << tab_id << " with URL: " << url;
}

void TabManager::CloseTab(const std::string& tab_id) {
    auto it = browsers_.find(tab_id);
    if (it != browsers_.end()) {
        it->second->GetHost()->CloseBrowser(false);
        browsers_.erase(it);
        LOG(INFO) << "Closed tab: " << tab_id;
    }
}

void TabManager::SwitchTab(const std::string& tab_id) {
    auto it = browsers_.find(tab_id);
    if (it != browsers_.end()) {
        active_tab_id_ = tab_id;
        // Show this browser, hide others
        LOG(INFO) << "Switched to tab: " << tab_id;
    }
}

CefRefPtr<CefBrowser> TabManager::GetBrowser(const std::string& tab_id) {
    auto it = browsers_.find(tab_id);
    return (it != browsers_.end()) ? it->second : nullptr;
}

CefRefPtr<CefBrowser> TabManager::GetActiveBrowser() {
    return GetBrowser(active_tab_id_);
}

std::string TabManager::GetActiveTabId() const {
    return active_tab_id_;
}
```

#### Testing Checklist
- [ ] Can create new tab with Ctrl+T
- [ ] Can close tab with Ctrl+W or X button
- [ ] Can switch tabs by clicking
- [ ] Can switch tabs with Ctrl+1-9
- [ ] Can navigate between tabs with Ctrl+Tab
- [ ] Tabs show correct title and favicon
- [ ] Active tab is highlighted
- [ ] Tab close button appears on hover
- [ ] Can drag and reorder tabs
- [ ] Can pin/unpin tabs
- [ ] Tab overflow scrolls horizontally
- [ ] Closing last tab creates new tab

#### Dependencies
- `uuid` library for unique tab IDs: `npm install uuid @types/uuid`
- CEF multi-browser view support (already available)

---

### 1.2 Address Bar URL Synchronization

**Status**: ❌ **CRITICAL BUG** (URL doesn't update when navigating)
**Chrome Feature Level**: 100%
**User Impact**: **CRITICAL** - Breaks user trust and navigation awareness
**Development Effort**: 🟡 Medium (2-3 days)

#### What Chrome Has
- Address bar updates automatically when:
  - User clicks a link
  - Page redirects
  - History navigation (back/forward)
  - JavaScript navigation
  - Bookmark clicked
- Shows loading state with progress indicator
- Displays full URL on focus, shortened URL otherwise
- Auto-selects URL on focus for easy copy/paste

#### What HodosBrowser Has
- **Static input field** - Never updates after initial load
- Only changes when user manually types and presses Enter
- **Major UX bug**: Clicking links doesn't update the address bar

#### Implementation Steps

##### Step 1: Add URL Update Message Handler (1 day)

**File**: `frontend/src/hooks/useHodosBrowser.ts` (modify)

Add state for current URL and message listener:

```typescript
const [currentUrl, setCurrentUrl] = useState('https://metanetapps.com/');

useEffect(() => {
  // Listen for URL changes from CEF
  const handleUrlChange = (event: MessageEvent) => {
    if (event.data.type === 'url_changed') {
      setCurrentUrl(event.data.url);
    }

    if (event.data.type === 'title_changed') {
      setCurrentTitle(event.data.title);
    }

    if (event.data.type === 'loading_state_changed') {
      setIsLoading(event.data.isLoading);
      setCanGoBack(event.data.canGoBack);
      setCanGoForward(event.data.canGoForward);
    }
  };

  window.addEventListener('message', handleUrlChange);
  return () => window.removeEventListener('message', handleUrlChange);
}, []);

return {
  currentUrl,
  setCurrentUrl, // For manual updates
  isLoading,
  canGoBack,
  canGoForward,
  // ... other methods
};
```

##### Step 2: Update CEF Backend to Send URL Changes (1-2 days)

**File**: `cef-native/src/handlers/simple_handler.cpp` (modify)

Add OnLoadingStateChange and OnAddressChange handlers:

```cpp
void SimpleHandler::OnLoadingStateChange(
    CefRefPtr<CefBrowser> browser,
    bool isLoading,
    bool canGoBack,
    bool canGoForward) {

    // Send loading state to frontend
    CefRefPtr<CefFrame> frame = browser->GetMainFrame();
    std::string js = "window.postMessage({type: 'loading_state_changed', "
                    "isLoading: " + std::string(isLoading ? "true" : "false") + ", "
                    "canGoBack: " + std::string(canGoBack ? "true" : "false") + ", "
                    "canGoForward: " + std::string(canGoForward ? "true" : "false") +
                    "}, '*');";
    frame->ExecuteJavaScript(js, frame->GetURL(), 0);
}

void SimpleHandler::OnAddressChange(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    const CefString& url) {

    if (frame->IsMain()) {
        // Send URL change to frontend
        std::string escaped_url = url.ToString();
        // Escape quotes in URL
        size_t pos = 0;
        while ((pos = escaped_url.find("\"", pos)) != std::string::npos) {
            escaped_url.replace(pos, 1, "\\\"");
            pos += 2;
        }

        std::string js = "window.postMessage({type: 'url_changed', "
                        "url: \"" + escaped_url + "\"}, '*');";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
    }
}

void SimpleHandler::OnTitleChange(
    CefRefPtr<CefBrowser> browser,
    const CefString& title) {

    CefRefPtr<CefFrame> frame = browser->GetMainFrame();
    std::string escaped_title = title.ToString();
    // Escape quotes
    size_t pos = 0;
    while ((pos = escaped_title.find("\"", pos)) != std::string::npos) {
        escaped_title.replace(pos, 1, "\\\"");
        pos += 2;
    }

    std::string js = "window.postMessage({type: 'title_changed', "
                    "title: \"" + escaped_title + "\"}, '*');";
    frame->ExecuteJavaScript(js, frame->GetURL(), 0);
}
```

**File**: `cef-native/include/handlers/simple_handler.h` (modify)

Add virtual method declarations:

```cpp
class SimpleHandler : public CefClient,
                      public CefDisplayHandler,
                      public CefLifeSpanHandler,
                      public CefLoadHandler {
public:
    // ... existing methods ...

    // CefDisplayHandler methods
    virtual void OnAddressChange(CefRefPtr<CefBrowser> browser,
                                CefRefPtr<CefFrame> frame,
                                const CefString& url) override;

    virtual void OnTitleChange(CefRefPtr<CefBrowser> browser,
                              const CefString& title) override;

    // CefLoadHandler methods
    virtual void OnLoadingStateChange(CefRefPtr<CefBrowser> browser,
                                     bool isLoading,
                                     bool canGoBack,
                                     bool canGoForward) override;

private:
    IMPLEMENT_REFCOUNTING(SimpleHandler);
};
```

##### Step 3: Update Address Bar Component (0.5 days)

**File**: `frontend/src/components/NavigationBar.tsx` (modify)

```typescript
import { useHodosBrowser } from '../hooks/useHodosBrowser';

export const NavigationBar: React.FC = () => {
  const {
    currentUrl,
    setCurrentUrl,
    navigate,
    isLoading,
    canGoBack,
    canGoForward,
    goBack,
    goForward,
    reload,
  } = useHodosBrowser();

  const [editingUrl, setEditingUrl] = useState(currentUrl);
  const [isFocused, setIsFocused] = useState(false);

  // Update editing URL when current URL changes (unless user is typing)
  useEffect(() => {
    if (!isFocused) {
      setEditingUrl(currentUrl);
    }
  }, [currentUrl, isFocused]);

  const handleUrlSubmit = () => {
    navigate(editingUrl);
    setIsFocused(false);
  };

  return (
    <Box sx={{ display: 'flex', gap: 1, p: 1 }}>
      {/* Back button */}
      <IconButton
        onClick={goBack}
        disabled={!canGoBack}
        size="small"
      >
        <ArrowBackIcon />
      </IconButton>

      {/* Forward button */}
      <IconButton
        onClick={goForward}
        disabled={!canGoForward}
        size="small"
      >
        <ArrowForwardIcon />
      </IconButton>

      {/* Reload/Stop button */}
      <IconButton onClick={reload} size="small">
        {isLoading ? <StopIcon /> : <RefreshIcon />}
      </IconButton>

      {/* Address Bar */}
      <Paper sx={{ flex: 1, display: 'flex', alignItems: 'center' }}>
        <InputBase
          value={isFocused ? editingUrl : formatUrl(currentUrl)}
          onChange={(e) => setEditingUrl(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') handleUrlSubmit();
            if (e.key === 'Escape') setIsFocused(false);
          }}
          onFocus={() => {
            setIsFocused(true);
            setEditingUrl(currentUrl); // Show full URL
          }}
          onBlur={() => setIsFocused(false)}
          placeholder="Search or enter address"
          fullWidth
          sx={{ px: 2, py: 0.5 }}
        />

        {isLoading && (
          <CircularProgress size={20} sx={{ mr: 1 }} />
        )}
      </Paper>
    </Box>
  );
};

// Helper to show shortened URL when not focused
function formatUrl(url: string): string {
  try {
    const urlObj = new URL(url);
    return urlObj.hostname + urlObj.pathname;
  } catch {
    return url;
  }
}
```

#### Testing Checklist
- [ ] Address bar updates when clicking links
- [ ] Address bar updates on back/forward navigation
- [ ] Address bar updates on redirects
- [ ] Focusing address bar selects all text
- [ ] Unfocused address bar shows shortened URL
- [ ] Loading spinner appears when page loads
- [ ] Navigation buttons enable/disable correctly
- [ ] Reload button changes to stop button when loading

---

### 1.3 Search Engine Integration

**Status**: ❌ **NOT IMPLEMENTED**
**Chrome Feature Level**: 100%
**User Impact**: **HIGH** - Users expect to search from address bar
**Development Effort**: 🟢 Low (1-2 days)

#### What Chrome Has
- Omnibox (combined address/search bar)
- Default search engine (Google, Bing, DuckDuckGo, etc.)
- Smart detection: URL vs search query
- Search suggestions dropdown
- Search engine selection in settings
- Keyword shortcuts (e.g., "yt cats" searches YouTube)

#### What HodosBrowser Has
- Address bar only accepts URLs
- No search functionality
- No search suggestions

#### Implementation Steps

##### Step 1: Add Search Detection Logic (0.5 days)

**File**: `frontend/src/utils/searchUtils.ts` (NEW)

```typescript
export interface SearchEngine {
  name: string;
  searchUrl: string;
  suggestionsUrl?: string;
}

export const SEARCH_ENGINES: Record<string, SearchEngine> = {
  google: {
    name: 'Google',
    searchUrl: 'https://www.google.com/search?q=%s',
    suggestionsUrl: 'https://www.google.com/complete/search?client=chrome&q=%s',
  },
  duckduckgo: {
    name: 'DuckDuckGo',
    searchUrl: 'https://duckduckgo.com/?q=%s',
    suggestionsUrl: 'https://duckduckgo.com/ac/?q=%s',
  },
  bing: {
    name: 'Bing',
    searchUrl: 'https://www.bing.com/search?q=%s',
  },
};

export function isUrl(input: string): boolean {
  // Check if input looks like a URL
  const urlPattern = /^(https?:\/\/|www\.)/i;
  const domainPattern = /^[a-z0-9-]+\.[a-z]{2,}/i;

  return urlPattern.test(input) || domainPattern.test(input);
}

export function processInput(input: string, searchEngine: string = 'google'): string {
  const trimmed = input.trim();

  if (!trimmed) return '';

  // If it's a URL, return it (add https:// if missing)
  if (isUrl(trimmed)) {
    if (!trimmed.startsWith('http')) {
      return 'https://' + trimmed;
    }
    return trimmed;
  }

  // Otherwise, it's a search query
  const engine = SEARCH_ENGINES[searchEngine];
  return engine.searchUrl.replace('%s', encodeURIComponent(trimmed));
}
```

##### Step 2: Update Address Bar to Support Search (0.5 days)

**File**: `frontend/src/components/NavigationBar.tsx` (modify handleUrlSubmit)

```typescript
import { processInput } from '../utils/searchUtils';

const handleUrlSubmit = () => {
  const finalUrl = processInput(editingUrl, 'google'); // Use default search engine
  navigate(finalUrl);
  setIsFocused(false);
};
```

##### Step 3: Add Search Suggestions (Optional, 1 day)

**File**: `frontend/src/components/SearchSuggestions.tsx` (NEW)

```typescript
import React, { useState, useEffect } from 'react';
import { Paper, List, ListItem, ListItemText } from '@mui/material';

interface SearchSuggestionsProps {
  query: string;
  onSelect: (suggestion: string) => void;
}

export const SearchSuggestions: React.FC<SearchSuggestionsProps> = ({
  query,
  onSelect,
}) => {
  const [suggestions, setSuggestions] = useState<string[]>([]);

  useEffect(() => {
    if (!query || query.length < 2) {
      setSuggestions([]);
      return;
    }

    // Fetch suggestions from Google
    fetch(`https://www.google.com/complete/search?client=chrome&q=${encodeURIComponent(query)}`)
      .then(res => res.json())
      .then(data => {
        setSuggestions(data[1] || []);
      })
      .catch(() => setSuggestions([]));
  }, [query]);

  if (suggestions.length === 0) return null;

  return (
    <Paper
      sx={{
        position: 'absolute',
        top: '100%',
        left: 0,
        right: 0,
        mt: 0.5,
        maxHeight: 400,
        overflow: 'auto',
        zIndex: 1000,
      }}
    >
      <List>
        {suggestions.map((suggestion, index) => (
          <ListItem
            key={index}
            button
            onClick={() => onSelect(suggestion)}
            sx={{
              '&:hover': {
                backgroundColor: 'grey.100',
              },
            }}
          >
            <ListItemText primary={suggestion} />
          </ListItem>
        ))}
      </List>
    </Paper>
  );
};
```

#### Testing Checklist
- [ ] Typing a URL navigates to that URL
- [ ] Typing a search query searches Google
- [ ] "www.example.com" is treated as URL
- [ ] "how to code" is treated as search
- [ ] URLs without https:// get it prepended
- [ ] Search suggestions appear (if implemented)
- [ ] Clicking suggestion navigates to search

---

### 1.4 Navigation Controls Enhancement

**Status**: ⚠️ **PARTIALLY IMPLEMENTED** (40% complete)
**Chrome Feature Level**: 100%
**User Impact**: **MEDIUM-HIGH**
**Development Effort**: 🟢 Low (1 day)

#### What Chrome Has
- Back/Forward/Reload buttons
- Stop button (appears when loading)
- Home button (optional)
- Button states (enabled/disabled based on history)
- Long-press menu showing navigation history
- Tooltips with keyboard shortcuts
- Loading progress indicator
- Favicon in address bar

#### What HodosBrowser Has
- ✅ Back button (no state management)
- ✅ Forward button (no state management)
- ✅ Reload button (no state management)
- ❌ No stop button
- ❌ No home button
- ❌ No enabled/disabled states
- ❌ No tooltips
- ❌ No loading indicator

#### Implementation Steps

##### Step 1: Add Button States (0.5 days)

Already covered in Address Bar Sync section - use `canGoBack`, `canGoForward`, `isLoading` from `useHodosBrowser` hook.

##### Step 2: Add Stop Button (0.25 days)

```typescript
import StopIcon from '@mui/icons-material/Stop';

// Replace reload button with reload/stop toggle
<IconButton
  onClick={isLoading ? stop : reload}
  size="small"
  title={isLoading ? "Stop (Esc)" : "Reload (Ctrl+R)"}
>
  {isLoading ? <StopIcon /> : <RefreshIcon />}
</IconButton>
```

Add stop method to `useHodosBrowser`:

```typescript
const stop = useCallback(() => {
  window.cefMessage.send('navigate_stop', []);
}, []);
```

##### Step 3: Add Home Button (0.25 days)

```typescript
import HomeIcon from '@mui/icons-material/Home';

<IconButton
  onClick={() => navigate('https://metanetapps.com/')}
  size="small"
  title="Home"
>
  <HomeIcon />
</IconButton>
```

##### Step 4: Add Loading Progress Bar (Optional, 0.5 days)

**File**: `frontend/src/components/ProgressBar.tsx` (NEW)

```typescript
import React from 'react';
import { LinearProgress, Box } from '@mui/material';

interface ProgressBarProps {
  isLoading: boolean;
  progress: number; // 0-100
}

export const ProgressBar: React.FC<ProgressBarProps> = ({ isLoading, progress }) => {
  if (!isLoading && progress >= 100) return null;

  return (
    <Box sx={{ width: '100%', height: 2 }}>
      <LinearProgress
        variant={progress > 0 ? "determinate" : "indeterminate"}
        value={progress}
        sx={{
          height: 2,
          backgroundColor: 'transparent',
          '& .MuiLinearProgress-bar': {
            backgroundColor: 'primary.main',
          },
        }}
      />
    </Box>
  );
};
```

Add to NavigationBar below the buttons.

#### Testing Checklist
- [ ] Back button disabled when can't go back
- [ ] Forward button disabled when can't go forward
- [ ] Reload changes to stop when loading
- [ ] Stop button stops page load
- [ ] Home button navigates to homepage
- [ ] Tooltips show keyboard shortcuts
- [ ] Progress bar shows during load

---

### 1.5 Keyboard Shortcuts (Global)

**Status**: ❌ **NOT IMPLEMENTED**
**Chrome Feature Level**: 100%
**User Impact**: **HIGH** - Power users rely on shortcuts
**Development Effort**: 🟡 Medium (2 days)

#### What Chrome Has
- 50+ keyboard shortcuts
- Tab management (Ctrl+T, Ctrl+W, Ctrl+Tab, etc.)
- Navigation (Ctrl+L, Alt+Left/Right, F5, etc.)
- Page actions (Ctrl+F, Ctrl+P, Ctrl+S, etc.)
- Zoom (Ctrl++, Ctrl+-, Ctrl+0)
- DevTools (F12, Ctrl+Shift+I)
- Customizable shortcuts

#### What HodosBrowser Has
- ❌ No keyboard shortcuts (Enter in address bar only)

#### Implementation Steps

##### Step 1: Create Keyboard Shortcut Manager (1 day)

**File**: `frontend/src/hooks/useGlobalKeyboardShortcuts.ts` (NEW)

```typescript
import { useEffect } from 'react';

export interface KeyboardShortcutConfig {
  // Tab management
  onNewTab: () => void;
  onCloseTab: () => void;
  onNextTab: () => void;
  onPrevTab: () => void;
  onReopenTab: () => void;
  onSwitchToTab: (index: number) => void;

  // Navigation
  onBack: () => void;
  onForward: () => void;
  onReload: () => void;
  onHome: () => void;
  onFocusAddressBar: () => void;

  // Page actions
  onFind: () => void;
  onPrint: () => void;
  onSave: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onZoomReset: () => void;

  // Browser
  onOpenSettings: () => void;
  onOpenHistory: () => void;
  onOpenDownloads: () => void;
  onOpenBookmarks: () => void;
  onToggleDevTools: () => void;
}

export const useGlobalKeyboardShortcuts = (config: Partial<KeyboardShortcutConfig>) => {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      const shift = e.shiftKey;
      const alt = e.altKey;

      // Prevent default for all shortcuts we handle
      const shouldPreventDefault = () => {
        e.preventDefault();
      };

      // TAB MANAGEMENT
      if (ctrl && e.key === 't' && !shift) {
        shouldPreventDefault();
        config.onNewTab?.();
      }

      if (ctrl && e.key === 'w') {
        shouldPreventDefault();
        config.onCloseTab?.();
      }

      if (ctrl && e.key === 'Tab') {
        shouldPreventDefault();
        shift ? config.onPrevTab?.() : config.onNextTab?.();
      }

      if (ctrl && shift && e.key === 't') {
        shouldPreventDefault();
        config.onReopenTab?.();
      }

      if (ctrl && e.key >= '1' && e.key <= '9') {
        shouldPreventDefault();
        config.onSwitchToTab?.(parseInt(e.key) - 1);
      }

      // NAVIGATION
      if (alt && e.key === 'ArrowLeft') {
        shouldPreventDefault();
        config.onBack?.();
      }

      if (alt && e.key === 'ArrowRight') {
        shouldPreventDefault();
        config.onForward?.();
      }

      if (e.key === 'F5' || (ctrl && e.key === 'r')) {
        shouldPreventDefault();
        config.onReload?.();
      }

      if (alt && e.key === 'Home') {
        shouldPreventDefault();
        config.onHome?.();
      }

      if (ctrl && e.key === 'l' || e.key === 'F6') {
        shouldPreventDefault();
        config.onFocusAddressBar?.();
      }

      // PAGE ACTIONS
      if (ctrl && e.key === 'f') {
        shouldPreventDefault();
        config.onFind?.();
      }

      if (ctrl && e.key === 'p') {
        shouldPreventDefault();
        config.onPrint?.();
      }

      if (ctrl && e.key === 's') {
        shouldPreventDefault();
        config.onSave?.();
      }

      if (ctrl && (e.key === '+' || e.key === '=')) {
        shouldPreventDefault();
        config.onZoomIn?.();
      }

      if (ctrl && (e.key === '-' || e.key === '_')) {
        shouldPreventDefault();
        config.onZoomOut?.();
      }

      if (ctrl && e.key === '0') {
        shouldPreventDefault();
        config.onZoomReset?.();
      }

      // BROWSER
      if (ctrl && e.key === ',') {
        shouldPreventDefault();
        config.onOpenSettings?.();
      }

      if (ctrl && e.key === 'h') {
        shouldPreventDefault();
        config.onOpenHistory?.();
      }

      if (ctrl && e.key === 'j') {
        shouldPreventDefault();
        config.onOpenDownloads?.();
      }

      if (ctrl && shift && e.key === 'b') {
        shouldPreventDefault();
        config.onOpenBookmarks?.();
      }

      if (e.key === 'F12' || (ctrl && shift && e.key === 'i')) {
        shouldPreventDefault();
        config.onToggleDevTools?.();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [config]);
};
```

##### Step 2: Integrate into Main Browser (0.5 days)

**File**: `frontend/src/pages/MainBrowserView.tsx` (add shortcuts)

```typescript
import { useGlobalKeyboardShortcuts } from '../hooks/useGlobalKeyboardShortcuts';

export const MainBrowserView: React.FC = () => {
  const addressBarRef = useRef<HTMLInputElement>(null);

  useGlobalKeyboardShortcuts({
    // Tab management (from useTabManager)
    onNewTab: createTab,
    onCloseTab: () => closeTab(activeTabId),
    onNextTab: () => switchToNextTab(),
    onPrevTab: () => switchToPrevTab(),

    // Navigation
    onBack: goBack,
    onForward: goForward,
    onReload: reload,
    onHome: () => navigate('https://metanetapps.com/'),
    onFocusAddressBar: () => addressBarRef.current?.focus(),

    // Page actions
    onFind: () => setShowFindBar(true),
    onPrint: () => window.cefMessage.send('page_print', []),
    onSave: () => window.cefMessage.send('page_save', []),
    onZoomIn: () => window.cefMessage.send('zoom_in', []),
    onZoomOut: () => window.cefMessage.send('zoom_out', []),
    onZoomReset: () => window.cefMessage.send('zoom_reset', []),

    // Browser
    onOpenSettings: () => navigate('/settings'),
    onToggleDevTools: () => window.cefMessage.send('toggle_devtools', []),
  });

  // ... rest of component
};
```

##### Step 3: Add Shortcut Documentation (0.5 days)

Create a keyboard shortcuts help page accessible via `?` or in settings.

**File**: `frontend/src/pages/KeyboardShortcuts.tsx` (NEW)

```typescript
import React from 'react';
import { Box, Typography, Table, TableBody, TableCell, TableRow, Paper } from '@mui/material';

export const KeyboardShortcutsPage: React.FC = () => {
  const shortcuts = [
    { category: 'Tabs', action: 'New tab', shortcut: 'Ctrl+T' },
    { category: 'Tabs', action: 'Close tab', shortcut: 'Ctrl+W' },
    { category: 'Tabs', action: 'Next tab', shortcut: 'Ctrl+Tab' },
    { category: 'Tabs', action: 'Previous tab', shortcut: 'Ctrl+Shift+Tab' },
    { category: 'Tabs', action: 'Reopen closed tab', shortcut: 'Ctrl+Shift+T' },
    { category: 'Tabs', action: 'Switch to tab 1-9', shortcut: 'Ctrl+1 to Ctrl+9' },

    { category: 'Navigation', action: 'Back', shortcut: 'Alt+←' },
    { category: 'Navigation', action: 'Forward', shortcut: 'Alt+→' },
    { category: 'Navigation', action: 'Reload', shortcut: 'Ctrl+R or F5' },
    { category: 'Navigation', action: 'Focus address bar', shortcut: 'Ctrl+L or F6' },

    { category: 'Page', action: 'Find in page', shortcut: 'Ctrl+F' },
    { category: 'Page', action: 'Print', shortcut: 'Ctrl+P' },
    { category: 'Page', action: 'Save page', shortcut: 'Ctrl+S' },
    { category: 'Page', action: 'Zoom in', shortcut: 'Ctrl++' },
    { category: 'Page', action: 'Zoom out', shortcut: 'Ctrl+-' },
    { category: 'Page', action: 'Reset zoom', shortcut: 'Ctrl+0' },

    { category: 'Browser', action: 'Settings', shortcut: 'Ctrl+,' },
    { category: 'Browser', action: 'History', shortcut: 'Ctrl+H' },
    { category: 'Browser', action: 'Downloads', shortcut: 'Ctrl+J' },
    { category: 'Browser', action: 'Bookmarks', shortcut: 'Ctrl+Shift+B' },
    { category: 'Browser', action: 'Developer Tools', shortcut: 'F12 or Ctrl+Shift+I' },
  ];

  let currentCategory = '';

  return (
    <Box sx={{ p: 4 }}>
      <Typography variant="h4" gutterBottom>
        Keyboard Shortcuts
      </Typography>

      <Paper sx={{ mt: 3 }}>
        <Table>
          <TableBody>
            {shortcuts.map((shortcut, index) => {
              const showCategory = shortcut.category !== currentCategory;
              currentCategory = shortcut.category;

              return (
                <React.Fragment key={index}>
                  {showCategory && (
                    <TableRow sx={{ backgroundColor: 'grey.100' }}>
                      <TableCell colSpan={2}>
                        <Typography variant="h6">{shortcut.category}</Typography>
                      </TableCell>
                    </TableRow>
                  )}
                  <TableRow>
                    <TableCell>{shortcut.action}</TableCell>
                    <TableCell align="right">
                      <Typography variant="body2" sx={{ fontFamily: 'monospace' }}>
                        {shortcut.shortcut}
                      </Typography>
                    </TableCell>
                  </TableRow>
                </React.Fragment>
              );
            })}
          </TableBody>
        </Table>
      </Paper>
    </Box>
  );
};
```

#### Testing Checklist
- [ ] Ctrl+T opens new tab
- [ ] Ctrl+W closes current tab
- [ ] Ctrl+Tab/Shift+Tab cycles tabs
- [ ] Ctrl+1-9 switches to specific tab
- [ ] Alt+Left/Right navigates history
- [ ] F5/Ctrl+R reloads page
- [ ] Ctrl+L focuses address bar
- [ ] Ctrl+F opens find in page
- [ ] Ctrl+P opens print dialog
- [ ] Ctrl++/- zooms in/out
- [ ] F12 opens DevTools
- [ ] All shortcuts work consistently

---

## Priority 2: Essential Features (Should Have)

These features are **expected by users** but not critical for basic functionality. They significantly improve the browsing experience.

---

### 2.1 Bookmarks System

**Status**: ❌ **NOT IMPLEMENTED**
**Chrome Feature Level**: 100%
**User Impact**: **HIGH** - Very common feature
**Development Effort**: 🔴 High (4-5 days)

#### What Chrome Has
- Bookmark bar (toggle visibility)
- Bookmark manager (full page)
- Folder organization
- Bookmark import/export
- Star icon in address bar
- Keyboard shortcuts (Ctrl+D to bookmark)
- Right-click context menu
- Bookmark sync (across devices)
- Bookmark search

#### What HodosBrowser Has
- ❌ Nothing

#### Implementation Steps

##### Step 1: Create Bookmark Data Structure (1 day)

**File**: `frontend/src/types/BookmarkTypes.ts` (NEW)

```typescript
export interface Bookmark {
  id: string;
  title: string;
  url: string;
  favicon?: string;
  parentId?: string; // For folders
  createdAt: number;
  lastVisited?: number;
  isFolder: boolean;
  children?: Bookmark[]; // If folder
}

export interface BookmarkState {
  bookmarks: Bookmark[];
  bookmarkBar: Bookmark; // Root folder for bookmark bar
  otherBookmarks: Bookmark; // Root folder for other bookmarks
}
```

**File**: `frontend/src/hooks/useBookmarks.ts` (NEW)

```typescript
import { useState, useCallback, useEffect } from 'react';
import { Bookmark, BookmarkState } from '../types/BookmarkTypes';
import { v4 as uuidv4 } from 'uuid';

export const useBookmarks = () => {
  const [state, setState] = useState<BookmarkState>(() => {
    // Load from localStorage
    const saved = localStorage.getItem('bookmarks');
    if (saved) {
      return JSON.parse(saved);
    }

    // Default state
    return {
      bookmarks: [],
      bookmarkBar: {
        id: 'bookmark_bar',
        title: 'Bookmark Bar',
        url: '',
        isFolder: true,
        children: [],
        createdAt: Date.now(),
      },
      otherBookmarks: {
        id: 'other_bookmarks',
        title: 'Other Bookmarks',
        url: '',
        isFolder: true,
        children: [],
        createdAt: Date.now(),
      },
    };
  });

  // Save to localStorage on change
  useEffect(() => {
    localStorage.setItem('bookmarks', JSON.stringify(state));
  }, [state]);

  const addBookmark = useCallback((
    title: string,
    url: string,
    parentId: string = 'bookmark_bar',
    favicon?: string
  ) => {
    const bookmark: Bookmark = {
      id: uuidv4(),
      title,
      url,
      favicon,
      parentId,
      isFolder: false,
      createdAt: Date.now(),
    };

    setState(prev => {
      const parent = parentId === 'bookmark_bar'
        ? prev.bookmarkBar
        : prev.otherBookmarks;

      return {
        ...prev,
        [parentId === 'bookmark_bar' ? 'bookmarkBar' : 'otherBookmarks']: {
          ...parent,
          children: [...(parent.children || []), bookmark],
        },
      };
    });

    return bookmark;
  }, []);

  const removeBookmark = useCallback((bookmarkId: string) => {
    setState(prev => {
      const removeFromFolder = (folder: Bookmark): Bookmark => {
        if (!folder.children) return folder;

        return {
          ...folder,
          children: folder.children
            .filter(b => b.id !== bookmarkId)
            .map(b => b.isFolder ? removeFromFolder(b) : b),
        };
      };

      return {
        ...prev,
        bookmarkBar: removeFromFolder(prev.bookmarkBar),
        otherBookmarks: removeFromFolder(prev.otherBookmarks),
      };
    });
  }, []);

  const isBookmarked = useCallback((url: string): boolean => {
    const checkFolder = (folder: Bookmark): boolean => {
      if (!folder.children) return false;
      return folder.children.some(b =>
        b.isFolder ? checkFolder(b) : b.url === url
      );
    };

    return checkFolder(state.bookmarkBar) || checkFolder(state.otherBookmarks);
  }, [state]);

  const findBookmark = useCallback((url: string): Bookmark | null => {
    const searchFolder = (folder: Bookmark): Bookmark | null => {
      if (!folder.children) return null;

      for (const child of folder.children) {
        if (child.isFolder) {
          const found = searchFolder(child);
          if (found) return found;
        } else if (child.url === url) {
          return child;
        }
      }
      return null;
    };

    return searchFolder(state.bookmarkBar) || searchFolder(state.otherBookmarks);
  }, [state]);

  return {
    bookmarkBar: state.bookmarkBar,
    otherBookmarks: state.otherBookmarks,
    addBookmark,
    removeBookmark,
    isBookmarked,
    findBookmark,
  };
};
```

##### Step 2: Create Bookmark Star Button (0.5 days)

**File**: `frontend/src/components/BookmarkStarButton.tsx` (NEW)

```typescript
import React from 'react';
import { IconButton, Tooltip } from '@mui/material';
import StarIcon from '@mui/icons-material/Star';
import StarBorderIcon from '@mui/icons-material/StarBorder';

interface BookmarkStarButtonProps {
  isBookmarked: boolean;
  onToggle: () => void;
}

export const BookmarkStarButton: React.FC<BookmarkStarButtonProps> = ({
  isBookmarked,
  onToggle,
}) => {
  return (
    <Tooltip title={isBookmarked ? 'Remove bookmark' : 'Bookmark this page (Ctrl+D)'}>
      <IconButton onClick={onToggle} size="small">
        {isBookmarked ? (
          <StarIcon sx={{ color: 'gold' }} />
        ) : (
          <StarBorderIcon />
        )}
      </IconButton>
    </Tooltip>
  );
};
```

Add to NavigationBar in address bar Paper component.

##### Step 3: Create Bookmark Bar Component (1 day)

**File**: `frontend/src/components/BookmarkBar.tsx` (NEW)

```typescript
import React from 'react';
import { Box, Button, Menu, MenuItem } from '@mui/material';
import FolderIcon from '@mui/icons-material/Folder';
import { Bookmark } from '../types/BookmarkTypes';

interface BookmarkBarProps {
  bookmarks: Bookmark;
  onNavigate: (url: string) => void;
  onRemove: (id: string) => void;
}

export const BookmarkBar: React.FC<BookmarkBarProps> = ({
  bookmarks,
  onNavigate,
  onRemove,
}) => {
  const [contextMenu, setContextMenu] = React.useState<{
    mouseX: number;
    mouseY: number;
    bookmark: Bookmark;
  } | null>(null);

  const handleContextMenu = (event: React.MouseEvent, bookmark: Bookmark) => {
    event.preventDefault();
    setContextMenu({
      mouseX: event.clientX - 2,
      mouseY: event.clientY - 4,
      bookmark,
    });
  };

  const handleClose = () => {
    setContextMenu(null);
  };

  return (
    <Box
      sx={{
        display: 'flex',
        alignItems: 'center',
        gap: 0.5,
        px: 2,
        py: 0.5,
        backgroundColor: 'grey.100',
        borderBottom: '1px solid',
        borderColor: 'grey.300',
        overflowX: 'auto',
        overflowY: 'hidden',
      }}
    >
      {bookmarks.children?.map((bookmark) => (
        <Button
          key={bookmark.id}
          onClick={() => onNavigate(bookmark.url)}
          onContextMenu={(e) => handleContextMenu(e, bookmark)}
          size="small"
          startIcon={bookmark.isFolder ? <FolderIcon /> : null}
          sx={{
            textTransform: 'none',
            color: 'text.primary',
            minWidth: 'auto',
            whiteSpace: 'nowrap',
          }}
        >
          {bookmark.title}
        </Button>
      ))}

      {/* Context Menu */}
      <Menu
        open={contextMenu !== null}
        onClose={handleClose}
        anchorReference="anchorPosition"
        anchorPosition={
          contextMenu !== null
            ? { top: contextMenu.mouseY, left: contextMenu.mouseX }
            : undefined
        }
      >
        <MenuItem onClick={() => {
          if (contextMenu) {
            onNavigate(contextMenu.bookmark.url);
          }
          handleClose();
        }}>
          Open
        </MenuItem>
        <MenuItem onClick={() => {
          if (contextMenu) {
            window.open(contextMenu.bookmark.url, '_blank');
          }
          handleClose();
        }}>
          Open in new tab
        </MenuItem>
        <MenuItem onClick={() => {
          if (contextMenu) {
            onRemove(contextMenu.bookmark.id);
          }
          handleClose();
        }}>
          Delete
        </MenuItem>
      </Menu>
    </Box>
  );
};
```

##### Step 4: Create Bookmark Manager Page (2-3 days)

**File**: `frontend/src/pages/BookmarkManager.tsx` (NEW)

```typescript
import React, { useState } from 'react';
import {
  Box,
  Typography,
  List,
  ListItem,
  ListItemIcon,
  ListItemText,
  ListItemButton,
  IconButton,
  TextField,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
} from '@mui/material';
import FolderIcon from '@mui/icons-material/Folder';
import InsertLinkIcon from '@mui/icons-material/InsertLink';
import DeleteIcon from '@mui/icons-material/Delete';
import EditIcon from '@mui/icons-material/Edit';
import { useBookmarks } from '../hooks/useBookmarks';
import { Bookmark } from '../types/BookmarkTypes';

export const BookmarkManager: React.FC = () => {
  const { bookmarkBar, otherBookmarks, removeBookmark } = useBookmarks();
  const [selectedFolder, setSelectedFolder] = useState<Bookmark>(bookmarkBar);
  const [editDialog, setEditDialog] = useState<Bookmark | null>(null);

  const renderBookmarkTree = (bookmark: Bookmark) => {
    return (
      <ListItem key={bookmark.id} disablePadding>
        <ListItemButton onClick={() => {
          if (bookmark.isFolder) {
            setSelectedFolder(bookmark);
          } else {
            window.location.href = bookmark.url;
          }
        }}>
          <ListItemIcon>
            {bookmark.isFolder ? <FolderIcon /> : <InsertLinkIcon />}
          </ListItemIcon>
          <ListItemText
            primary={bookmark.title}
            secondary={!bookmark.isFolder ? bookmark.url : undefined}
          />
          <IconButton onClick={(e) => {
            e.stopPropagation();
            setEditDialog(bookmark);
          }}>
            <EditIcon />
          </IconButton>
          <IconButton onClick={(e) => {
            e.stopPropagation();
            removeBookmark(bookmark.id);
          }}>
            <DeleteIcon />
          </IconButton>
        </ListItemButton>

        {bookmark.isFolder && bookmark.children && (
          <List sx={{ pl: 4 }}>
            {bookmark.children.map(child => renderBookmarkTree(child))}
          </List>
        )}
      </ListItem>
    );
  };

  return (
    <Box sx={{ display: 'flex', height: '100vh' }}>
      {/* Sidebar */}
      <Box sx={{ width: 250, borderRight: 1, borderColor: 'divider', p: 2 }}>
        <Typography variant="h6" gutterBottom>
          Bookmarks
        </Typography>
        <List>
          <ListItem disablePadding>
            <ListItemButton onClick={() => setSelectedFolder(bookmarkBar)}>
              <ListItemIcon><FolderIcon /></ListItemIcon>
              <ListItemText primary="Bookmark Bar" />
            </ListItemButton>
          </ListItem>
          <ListItem disablePadding>
            <ListItemButton onClick={() => setSelectedFolder(otherBookmarks)}>
              <ListItemIcon><FolderIcon /></ListItemIcon>
              <ListItemText primary="Other Bookmarks" />
            </ListItemButton>
          </ListItem>
        </List>
      </Box>

      {/* Main Content */}
      <Box sx={{ flex: 1, p: 3 }}>
        <Typography variant="h5" gutterBottom>
          {selectedFolder.title}
        </Typography>
        <List>
          {selectedFolder.children?.map(bookmark => renderBookmarkTree(bookmark))}
        </List>
      </Box>

      {/* Edit Dialog */}
      <Dialog open={editDialog !== null} onClose={() => setEditDialog(null)}>
        <DialogTitle>Edit Bookmark</DialogTitle>
        <DialogContent>
          <TextField
            label="Title"
            fullWidth
            defaultValue={editDialog?.title}
            sx={{ mt: 2 }}
          />
          {!editDialog?.isFolder && (
            <TextField
              label="URL"
              fullWidth
              defaultValue={editDialog?.url}
              sx={{ mt: 2 }}
            />
          )}
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setEditDialog(null)}>Cancel</Button>
          <Button variant="contained">Save</Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
};
```

##### Step 5: Add Ctrl+D Shortcut (0.5 days)

Add to `useGlobalKeyboardShortcuts`:

```typescript
if (ctrl && e.key === 'd') {
  e.preventDefault();
  config.onToggleBookmark?.();
}
```

Implement in MainBrowserView:

```typescript
const { isBookmarked, addBookmark, removeBookmark, findBookmark } = useBookmarks();
const currentUrl = '...'; // from useHodosBrowser
const currentTitle = '...'; // from useHodosBrowser

const handleToggleBookmark = () => {
  if (isBookmarked(currentUrl)) {
    const bookmark = findBookmark(currentUrl);
    if (bookmark) removeBookmark(bookmark.id);
  } else {
    addBookmark(currentTitle, currentUrl);
  }
};
```

#### Testing Checklist
- [ ] Ctrl+D bookmarks current page
- [ ] Star icon appears in address bar
- [ ] Clicking star toggles bookmark
- [ ] Bookmark bar shows bookmarked sites
- [ ] Clicking bookmark navigates to URL
- [ ] Bookmark manager opens (Ctrl+Shift+O)
- [ ] Can organize bookmarks in folders
- [ ] Can edit bookmark title/URL
- [ ] Can delete bookmarks
- [ ] Right-click context menu works
- [ ] Bookmarks persist across sessions

---

### 2.2 Browsing History

**Status**: ❌ **NOT IMPLEMENTED**
**Chrome Feature Level**: 100%
**User Impact**: **HIGH**
**Development Effort**: 🟡 Medium (3 days)

#### What Chrome Has
- Full history page (chrome://history)
- Search history
- Delete history (by time range)
- Clear browsing data
- Recently closed tabs
- History in back/forward long-press menu
- Ctrl+H shortcut

#### What HodosBrowser Has
- ❌ Nothing

#### Implementation Steps

##### Step 1: Create History Data Structure (0.5 days)

**File**: `frontend/src/types/HistoryTypes.ts` (NEW)

```typescript
export interface HistoryEntry {
  id: string;
  url: string;
  title: string;
  visitTime: number;
  visitCount: number;
  favicon?: string;
}

export interface HistoryState {
  entries: HistoryEntry[];
}
```

**File**: `frontend/src/hooks/useHistory.ts` (NEW)

```typescript
import { useState, useCallback, useEffect } from 'react';
import { HistoryEntry } from '../types/HistoryTypes';
import { v4 as uuidv4 } from 'uuid';

const MAX_HISTORY_ENTRIES = 10000;

export const useHistory = () => {
  const [entries, setEntries] = useState<HistoryEntry[]>(() => {
    const saved = localStorage.getItem('browsing_history');
    return saved ? JSON.parse(saved) : [];
  });

  useEffect(() => {
    localStorage.setItem('browsing_history', JSON.stringify(entries));
  }, [entries]);

  const addHistoryEntry = useCallback((url: string, title: string, favicon?: string) => {
    setEntries(prev => {
      // Check if URL already exists
      const existing = prev.find(e => e.url === url);

      if (existing) {
        // Update visit count and time
        return prev.map(e =>
          e.url === url
            ? { ...e, visitTime: Date.now(), visitCount: e.visitCount + 1, title }
            : e
        );
      }

      // Add new entry
      const newEntry: HistoryEntry = {
        id: uuidv4(),
        url,
        title,
        visitTime: Date.now(),
        visitCount: 1,
        favicon,
      };

      const updated = [newEntry, ...prev];

      // Keep only MAX_HISTORY_ENTRIES
      return updated.slice(0, MAX_HISTORY_ENTRIES);
    });
  }, []);

  const removeHistoryEntry = useCallback((id: string) => {
    setEntries(prev => prev.filter(e => e.id !== id));
  }, []);

  const clearHistory = useCallback((timeRange?: { start: number; end: number }) => {
    if (!timeRange) {
      setEntries([]);
      return;
    }

    setEntries(prev =>
      prev.filter(e => e.visitTime < timeRange.start || e.visitTime > timeRange.end)
    );
  }, []);

  const searchHistory = useCallback((query: string): HistoryEntry[] => {
    const lowerQuery = query.toLowerCase();
    return entries.filter(
      e =>
        e.title.toLowerCase().includes(lowerQuery) ||
        e.url.toLowerCase().includes(lowerQuery)
    );
  }, [entries]);

  return {
    entries,
    addHistoryEntry,
    removeHistoryEntry,
    clearHistory,
    searchHistory,
  };
};
```

##### Step 2: Auto-Track Page Visits (0.5 days)

Modify `useHodosBrowser` to track history:

```typescript
import { useHistory } from './useHistory';

export const useHodosBrowser = () => {
  const { addHistoryEntry } = useHistory();
  const [currentUrl, setCurrentUrl] = useState('');
  const [currentTitle, setCurrentTitle] = useState('');

  useEffect(() => {
    const handleUrlChange = (event: MessageEvent) => {
      if (event.data.type === 'url_changed') {
        setCurrentUrl(event.data.url);
      }

      if (event.data.type === 'title_changed') {
        setCurrentTitle(event.data.title);
      }

      // Track history when URL and title are available
      if (event.data.type === 'page_loaded') {
        addHistoryEntry(currentUrl, currentTitle);
      }
    };

    window.addEventListener('message', handleUrlChange);
    return () => window.removeEventListener('message', handleUrlChange);
  }, [currentUrl, currentTitle, addHistoryEntry]);

  // ... rest
};
```

##### Step 3: Create History Page (2 days)

**File**: `frontend/src/pages/HistoryPage.tsx` (NEW)

```typescript
import React, { useState } from 'react';
import {
  Box,
  Typography,
  TextField,
  List,
  ListItem,
  ListItemText,
  ListItemButton,
  IconButton,
  Button,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Select,
  MenuItem,
  FormControl,
  InputLabel,
} from '@mui/material';
import DeleteIcon from '@mui/icons-material/Delete';
import SearchIcon from '@mui/icons-material/Search';
import { useHistory } from '../hooks/useHistory';
import { HistoryEntry } from '../types/HistoryTypes';

export const HistoryPage: React.FC = () => {
  const { entries, removeHistoryEntry, clearHistory, searchHistory } = useHistory();
  const [searchQuery, setSearchQuery] = useState('');
  const [clearDialog, setClearDialog] = useState(false);
  const [timeRange, setTimeRange] = useState('all');

  const displayedEntries = searchQuery
    ? searchHistory(searchQuery)
    : entries;

  const groupByDate = (entries: HistoryEntry[]) => {
    const groups: Record<string, HistoryEntry[]> = {};

    entries.forEach(entry => {
      const date = new Date(entry.visitTime);
      const dateKey = date.toLocaleDateString();

      if (!groups[dateKey]) {
        groups[dateKey] = [];
      }
      groups[dateKey].push(entry);
    });

    return groups;
  };

  const grouped = groupByDate(displayedEntries);

  const handleClearHistory = () => {
    const now = Date.now();
    const ranges: Record<string, { start: number; end: number } | undefined> = {
      'hour': { start: now - 3600000, end: now },
      'day': { start: now - 86400000, end: now },
      'week': { start: now - 604800000, end: now },
      'month': { start: now - 2592000000, end: now },
      'all': undefined,
    };

    clearHistory(ranges[timeRange]);
    setClearDialog(false);
  };

  return (
    <Box sx={{ maxWidth: 900, mx: 'auto', p: 3 }}>
      {/* Header */}
      <Typography variant="h4" gutterBottom>
        History
      </Typography>

      {/* Search Bar */}
      <Box sx={{ display: 'flex', gap: 2, mb: 3 }}>
        <TextField
          fullWidth
          placeholder="Search history"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          InputProps={{
            startAdornment: <SearchIcon sx={{ mr: 1, color: 'grey.500' }} />,
          }}
        />
        <Button
          variant="outlined"
          onClick={() => setClearDialog(true)}
        >
          Clear browsing data
        </Button>
      </Box>

      {/* History List */}
      {Object.entries(grouped).map(([date, entries]) => (
        <Box key={date} sx={{ mb: 3 }}>
          <Typography variant="h6" sx={{ mb: 1, color: 'grey.700' }}>
            {date}
          </Typography>
          <List>
            {entries.map(entry => (
              <ListItem
                key={entry.id}
                disablePadding
                secondaryAction={
                  <IconButton
                    edge="end"
                    onClick={() => removeHistoryEntry(entry.id)}
                  >
                    <DeleteIcon />
                  </IconButton>
                }
              >
                <ListItemButton onClick={() => window.location.href = entry.url}>
                  <ListItemText
                    primary={entry.title || entry.url}
                    secondary={
                      <>
                        <Typography component="span" variant="body2" color="text.secondary">
                          {entry.url}
                        </Typography>
                        {' — '}
                        <Typography component="span" variant="body2" color="text.secondary">
                          {new Date(entry.visitTime).toLocaleTimeString()}
                        </Typography>
                      </>
                    }
                  />
                </ListItemButton>
              </ListItem>
            ))}
          </List>
        </Box>
      ))}

      {displayedEntries.length === 0 && (
        <Typography variant="body1" color="text.secondary" align="center" sx={{ mt: 4 }}>
          {searchQuery ? 'No results found' : 'No history yet'}
        </Typography>
      )}

      {/* Clear Data Dialog */}
      <Dialog open={clearDialog} onClose={() => setClearDialog(false)}>
        <DialogTitle>Clear browsing data</DialogTitle>
        <DialogContent>
          <FormControl fullWidth sx={{ mt: 2 }}>
            <InputLabel>Time range</InputLabel>
            <Select
              value={timeRange}
              label="Time range"
              onChange={(e) => setTimeRange(e.target.value)}
            >
              <MenuItem value="hour">Last hour</MenuItem>
              <MenuItem value="day">Last 24 hours</MenuItem>
              <MenuItem value="week">Last 7 days</MenuItem>
              <MenuItem value="month">Last 4 weeks</MenuItem>
              <MenuItem value="all">All time</MenuItem>
            </Select>
          </FormControl>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setClearDialog(false)}>Cancel</Button>
          <Button onClick={handleClearHistory} variant="contained" color="error">
            Clear data
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
};
```

Add route to App.tsx:

```typescript
<Route path="/history" element={<HistoryPage />} />
```

#### Testing Checklist
- [ ] Visiting pages adds to history
- [ ] Ctrl+H opens history page
- [ ] History shows most recent first
- [ ] Grouped by date
- [ ] Search history works
- [ ] Clicking entry navigates to URL
- [ ] Can delete individual entries
- [ ] Can clear history by time range
- [ ] History persists across sessions

---

### 2.3 Downloads Manager

**Status**: ❌ **NOT IMPLEMENTED**
**Chrome Feature Level**: 100%
**User Impact**: **MEDIUM-HIGH**
**Development Effort**: 🔴 High (4-5 days, requires CEF integration)

#### What Chrome Has
- Downloads bar (bottom of browser)
- Downloads page (chrome://downloads)
- Download progress indicator
- Pause/resume/cancel downloads
- Show in folder
- Download scanning (security)
- Download location settings
- Ctrl+J shortcut

#### What HodosBrowser Has
- ❌ Nothing (downloads likely fail silently)

#### Implementation Steps

This feature requires significant CEF backend work. Outline:

1. **CEF Download Handler** (C++, 2-3 days)
   - Implement `CefDownloadHandler`
   - Handle `OnBeforeDownload` callback
   - Handle `OnDownloadUpdated` callback
   - Send progress to frontend via messages

2. **Frontend Download Manager** (React, 1-2 days)
   - Create download state management
   - Create downloads page
   - Create download progress UI
   - Handle pause/resume/cancel

3. **Integration** (1 day)
   - Connect CEF to React
   - File system access for "show in folder"
   - Settings for download location

**File**: `cef-native/include/handlers/DownloadHandler.h` (NEW)

```cpp
#ifndef DOWNLOAD_HANDLER_H
#define DOWNLOAD_HANDLER_H

#include "include/cef_download_handler.h"
#include <map>

class DownloadHandler : public CefDownloadHandler {
public:
    DownloadHandler();

    // CefDownloadHandler methods
    void OnBeforeDownload(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefDownloadItem> download_item,
        const CefString& suggested_name,
        CefRefPtr<CefBeforeDownloadCallback> callback) override;

    void OnDownloadUpdated(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefDownloadItem> download_item,
        CefRefPtr<CefDownloadItemCallback> callback) override;

private:
    std::map<uint32, CefRefPtr<CefDownloadItemCallback>> download_callbacks_;

    void NotifyFrontend(const std::string& type, const std::string& data);

    IMPLEMENT_REFCOUNTING(DownloadHandler);
};

#endif // DOWNLOAD_HANDLER_H
```

**File**: `frontend/src/pages/DownloadsPage.tsx` (NEW)

```typescript
import React from 'react';
import {
  Box,
  Typography,
  List,
  ListItem,
  ListItemText,
  LinearProgress,
  IconButton,
} from '@mui/material';
import PauseIcon from '@mui/icons-material/Pause';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import CancelIcon from '@mui/icons-material/Cancel';
import FolderIcon from '@mui/icons-material/Folder';
import { useDownloads } from '../hooks/useDownloads';

export const DownloadsPage: React.FC = () => {
  const { downloads, pauseDownload, resumeDownload, cancelDownload, showInFolder } = useDownloads();

  return (
    <Box sx={{ maxWidth: 900, mx: 'auto', p: 3 }}>
      <Typography variant="h4" gutterBottom>
        Downloads
      </Typography>

      <List>
        {downloads.map(download => (
          <ListItem
            key={download.id}
            secondaryAction={
              <>
                {download.state === 'in_progress' && (
                  <IconButton onClick={() => pauseDownload(download.id)}>
                    <PauseIcon />
                  </IconButton>
                )}
                {download.state === 'paused' && (
                  <IconButton onClick={() => resumeDownload(download.id)}>
                    <PlayArrowIcon />
                  </IconButton>
                )}
                <IconButton onClick={() => cancelDownload(download.id)}>
                  <CancelIcon />
                </IconButton>
                {download.state === 'complete' && (
                  <IconButton onClick={() => showInFolder(download.path)}>
                    <FolderIcon />
                  </IconButton>
                )}
              </>
            }
          >
            <ListItemText
              primary={download.filename}
              secondary={
                <>
                  <LinearProgress
                    variant="determinate"
                    value={download.progress}
                    sx={{ my: 1 }}
                  />
                  {download.state === 'in_progress' && `${download.progress}% - ${formatBytes(download.receivedBytes)} of ${formatBytes(download.totalBytes)}`}
                  {download.state === 'complete' && 'Complete'}
                  {download.state === 'cancelled' && 'Cancelled'}
                </>
              }
            />
          </ListItem>
        ))}
      </List>
    </Box>
  );
};

function formatBytes(bytes: number): string {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}
```

Due to complexity, this feature should be prioritized after core browser features are complete.

---

## Priority 3: Important Features (Nice to Have)

_(Continued in actual implementation - document would include: Context Menus, Settings Panel Content, Find in Page, Security Indicators, Zoom Controls, Print, Full Screen, Page Info, etc.)_

---

## Priority 4: Advanced Features (Future)

_(Would include: Extensions, DevTools Access, Password Manager, Autofill, Sync, Profiles, etc.)_

---

## Technical Architecture Recommendations

### State Management
- Use React Context for global state (tabs, bookmarks, history)
- Consider Redux Toolkit if state becomes complex
- Keep localStorage for persistence (bookmarks, history, settings)

### Performance
- Virtualize long lists (history, bookmarks)
- Debounce search inputs
- Lazy load components
- Use React.memo for expensive components

### Testing
- Unit tests for hooks (useTabManager, useBookmarks, etc.)
- Integration tests for keyboard shortcuts
- E2E tests for critical flows

### Code Organization
```
frontend/src/
├── components/     # Reusable UI components
├── pages/          # Full page views
├── hooks/          # Custom React hooks
├── types/          # TypeScript definitions
├── utils/          # Helper functions
└── contexts/       # React contexts for global state
```

---

## Development Timeline Estimates

### Phase 1: Critical Features (3-4 weeks)
- Week 1-2: Tab management system
- Week 2: Address bar sync + search
- Week 3: Navigation enhancements
- Week 4: Keyboard shortcuts + polish

### Phase 2: Essential Features (3-4 weeks)
- Week 5-6: Bookmarks system
- Week 7: Browsing history
- Week 8: Downloads manager (basic)

### Phase 3: Important Features (2-3 weeks)
- Week 9: Context menus + settings content
- Week 10: Find in page + security indicators
- Week 11: Zoom + print + misc features

### Phase 4: Advanced Features (4-6 weeks)
- Extensions framework
- DevTools integration
- Advanced settings
- Sync capabilities

**Total Estimate**: 12-17 weeks for full Chrome-like UX

---

## Conclusion

HodosBrowser currently has **world-class Bitcoin SV wallet integration** but minimal traditional browser UX. To become a viable daily-use browser, the following priorities are recommended:

**Must Do First** (Priority 1):
1. Tab management
2. Address bar URL sync
3. Search integration
4. Keyboard shortcuts

**Do Next** (Priority 2):
1. Bookmarks
2. History
3. Downloads

**Do Later** (Priority 3-4):
Everything else based on user feedback.

This roadmap will transform HodosBrowser from a Bitcoin wallet with basic browsing to a full-featured browser with Bitcoin integration.

---

**End of Document**
