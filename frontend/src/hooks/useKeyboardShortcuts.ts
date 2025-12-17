import { useEffect } from 'react';

export interface KeyboardShortcutHandlers {
  // Tab management
  onNewTab?: () => void;
  onCloseTab?: () => void;
  onNextTab?: () => void;
  onPrevTab?: () => void;
  onSwitchToTab?: (index: number) => void;

  // Navigation
  onFocusAddressBar?: () => void;
  onReload?: () => void;

  // Browser
  onToggleDevTools?: () => void;
}

/**
 * Hook to handle global keyboard shortcuts
 * Matches Chrome's keyboard shortcut behavior
 */
export const useKeyboardShortcuts = (handlers: KeyboardShortcutHandlers) => {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      const shift = e.shiftKey;

      // Tab Management Shortcuts

      // Ctrl+T - New Tab
      if (ctrl && !shift && e.key === 't') {
        e.preventDefault();
        handlers.onNewTab?.();
        return;
      }

      // Ctrl+W - Close Current Tab
      if (ctrl && !shift && e.key === 'w') {
        e.preventDefault();
        handlers.onCloseTab?.();
        return;
      }

      // Ctrl+Tab - Next Tab
      if (ctrl && e.key === 'Tab' && !shift) {
        e.preventDefault();
        handlers.onNextTab?.();
        return;
      }

      // Ctrl+Shift+Tab - Previous Tab
      if (ctrl && e.key === 'Tab' && shift) {
        e.preventDefault();
        handlers.onPrevTab?.();
        return;
      }

      // Ctrl+1 through Ctrl+9 - Switch to Specific Tab
      if (ctrl && !shift && e.key >= '1' && e.key <= '9') {
        e.preventDefault();
        const index = parseInt(e.key) - 1;
        handlers.onSwitchToTab?.(index);
        return;
      }

      // Navigation Shortcuts

      // Ctrl+L or F6 - Focus Address Bar
      if ((ctrl && e.key === 'l') || e.key === 'F6') {
        e.preventDefault();
        handlers.onFocusAddressBar?.();
        return;
      }

      // Ctrl+R or F5 - Reload
      if ((ctrl && e.key === 'r') || e.key === 'F5') {
        e.preventDefault();
        handlers.onReload?.();
        return;
      }

      // Browser Shortcuts

      // F12 or Ctrl+Shift+I - Toggle DevTools
      if (e.key === 'F12' || (ctrl && shift && e.key === 'I')) {
        e.preventDefault();
        handlers.onToggleDevTools?.();
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handlers]);
};
