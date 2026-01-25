import React, { useEffect } from 'react';
import Omnibox from '../components/Omnibox';

export default function OmniboxOverlayRoot() {
  useEffect(() => {
    console.log('🔍 Omnibox overlay mounted');
  }, []);

  const focusInput = () => {
    const input = document.querySelector('input');
    if (input) {
      // Blur then focus to ensure focus event fires
      input.blur();
      setTimeout(() => {
        input.focus();
        console.log('🔍 Input focused');
      }, 10);
    }
  };

  const handleNavigate = (url: string) => {
    console.log('🔍 Navigating to:', url);

    // Send navigate message via IPC
    if (window.cefMessage) {
      window.cefMessage.send('omnibox_navigate', [url]);
    }
  };

  const handleClose = () => {
    console.log('🔍 Closing omnibox overlay');

    // Send close message via IPC
    if (window.cefMessage) {
      window.cefMessage.send('omnibox_close', []);
    }
  };

  const handleBackgroundClick = (e: React.MouseEvent) => {
    console.log('🔍 Background clicked, target:', e.target, 'currentTarget:', e.currentTarget);
    // Only close if clicking the background, not the omnibox itself
    if (e.target === e.currentTarget) {
      console.log('🔍 Click was on background, calling handleClose()');
      handleClose();
    } else {
      console.log('🔍 Click was on omnibox content, ignoring');
    }
  };

  // Use visibility API to detect when overlay becomes visible
  useEffect(() => {
    const checkVisibility = () => {
      if (!document.hidden) {
        console.log('🔍 Document visible, focusing input');
        setTimeout(() => focusInput(), 50);
      }
    };

    // Focus on mount
    setTimeout(() => focusInput(), 150);

    // Listen for visibility changes
    document.addEventListener('visibilitychange', checkVisibility);

    // Also listen for window focus
    window.addEventListener('focus', checkVisibility);

    // Poll for visibility (backup for CEF context where events might not fire)
    const pollInterval = setInterval(() => {
      if (!document.hidden) {
        const input = document.querySelector('input');
        if (input && document.activeElement !== input) {
          console.log('🔍 Polling detected unfocused input, focusing');
          focusInput();
        }
      }
    }, 200); // More aggressive polling

    return () => {
      document.removeEventListener('visibilitychange', checkVisibility);
      window.removeEventListener('focus', checkVisibility);
      clearInterval(pollInterval);
    };
  }, []);

  // Listen for Escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        handleClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <div
      onClick={handleBackgroundClick}
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100vw',
        height: '100vh',
        margin: 0,
        padding: 0,
        overflow: 'hidden',
        cursor: 'default',
        backgroundColor: 'rgba(0, 0, 0, 0.01)', // Nearly invisible backdrop to catch clicks
      }}
    >
      {/* Position address bar exactly where it is in the header */}
      {/* TabBar: 40px, Toolbar: 54px (9px padding top), nav buttons: ~140px */}
      <div
        style={{
          position: 'absolute',
          top: 49, // 40px TabBar + 9px toolbar padding
          left: 148, // 8px toolbar padding + 140px nav buttons
          right: 128, // Space for wallet/history/settings buttons (3 buttons + padding)
          cursor: 'text',
        }}
      >
        <Omnibox
          onNavigate={handleNavigate}
          initialValue=""
        />
      </div>
    </div>
  );
}
