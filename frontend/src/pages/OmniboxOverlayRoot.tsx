import React, { useEffect } from 'react';
import Omnibox from '../components/Omnibox';

console.log('🔍🔍🔍 OmniboxOverlayRoot.tsx FILE LOADED 🔍🔍🔍');

export default function OmniboxOverlayRoot() {
  console.log('🔍🔍🔍 OmniboxOverlayRoot COMPONENT RENDERING 🔍🔍🔍');

  useEffect(() => {
    console.log('🔍🔍🔍 Omnibox overlay MOUNTED (useEffect ran) 🔍🔍🔍');
  }, []);

  const focusInput = () => {
    const input = document.querySelector('input');
    if (input) {
      // Blur then focus to ensure focus event fires
      input.blur();
      setTimeout(() => {
        input.focus();
        console.log('🔍 Input focused via querySelector');
        console.log('🔍 Active element:', document.activeElement);
        console.log('🔍 Input is active:', document.activeElement === input);
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
        backgroundColor: 'rgba(0, 0, 0, 0.01)', // Nearly invisible backdrop for click detection
        pointerEvents: 'auto',
      }}
    >
      {/* Replicate exact header layout to position omnibox correctly */}
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100vw',
        pointerEvents: 'none', // Clicks pass through invisible parts
      }}>
        {/* Invisible TabBar spacer - matches TabBar height */}
        <div style={{ height: 40 }} />

        {/* Replicate Toolbar layout exactly from MainBrowserView.tsx */}
        <div style={{
          display: 'flex',
          height: 54,
          paddingLeft: 8,  // px: 1 in MUI = 8px
          paddingRight: 8,
          gap: 6,  // gap: 0.75 in MUI = 6px
          alignItems: 'center',
        }}>
          {/* Invisible spacers matching navigation buttons (flexShrink: 0, small size) */}
          {/* Back button spacer */}
          <div style={{ width: 34, height: 34, flexShrink: 0 }} />

          {/* Forward button spacer */}
          <div style={{ width: 34, height: 34, flexShrink: 0 }} />

          {/* Refresh button spacer */}
          <div style={{ width: 34, height: 34, flexShrink: 0 }} />

          {/* ACTUAL OMNIBOX - only this element is interactive */}
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              flex: 1,
              minWidth: 0,
              pointerEvents: 'auto',
            }}
          >
            <Omnibox
              onNavigate={handleNavigate}
              initialValue=""
            />
          </div>

          {/* Invisible spacers matching right-side buttons */}
          {/* Wallet button spacer */}
          <div style={{ width: 34, height: 34, flexShrink: 0 }} />

          {/* History button spacer */}
          <div style={{ width: 34, height: 34, flexShrink: 0 }} />

          {/* Settings button spacer */}
          <div style={{ width: 34, height: 34, flexShrink: 0 }} />
        </div>
      </div>
    </div>
  );
}
