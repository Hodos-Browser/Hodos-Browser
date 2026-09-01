import React, { useState, useEffect, useCallback } from 'react';
import { Box, Typography, Divider } from '@mui/material';
import RefreshIcon from '@mui/icons-material/Refresh';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import AddIcon from '@mui/icons-material/Add';
import BookmarkBorderIcon from '@mui/icons-material/BookmarkBorder';
import CloseIcon from '@mui/icons-material/Close';
import ArrowForwardIcon from '@mui/icons-material/ArrowForward';
import { tokens } from '../theme/tokens';

declare global {
  interface Window {
    setTabMenuContext?: (hasOthers: boolean, hasRight: boolean) => void;
  }
}

// ⚠️ Row and divider heights are pinned here because C++ sizes the overlay HWND from
// them (kTabMenuWidthDip / kTabMenuHeightDip in simple_app.cpp): 6 rows (6 x 32) +
// one divider (9) + container padding (2 x 4) = 209. Changing either without changing
// the other leaves dead space at the bottom of the menu, or clips the last row.
const ROW_HEIGHT = 32;

interface MenuItemRowProps {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  disabled?: boolean;
}

const MenuItemRow: React.FC<MenuItemRowProps> = ({ icon, label, onClick, disabled }) => (
  <Box
    onClick={disabled ? undefined : onClick}
    sx={{
      display: 'flex',
      alignItems: 'center',
      px: 2,
      height: ROW_HEIGHT,
      boxSizing: 'border-box',
      cursor: disabled ? 'default' : 'pointer',
      opacity: disabled ? 0.4 : 1,
      '&:hover': disabled ? {} : { backgroundColor: tokens.bgSurfaceHover },
      userSelect: 'none',
    }}
  >
    <Box
      sx={{
        width: 24,
        mr: 1.5,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        color: tokens.textSecondary,
      }}
    >
      {icon}
    </Box>
    <Typography sx={{ flex: 1, fontSize: '0.82rem', color: tokens.textPrimary }}>
      {label}
    </Typography>
  </Box>
);

/**
 * Overlay #15 — the tab context menu.
 *
 * Every item acts on the tab that was RIGHT-CLICKED, which C++ remembers when the menu
 * opens. This component deliberately does not know or send the tab id: if it did, the
 * id would have to survive a round trip through an overlay the user can dismiss, and
 * "which tab" would have two sources of truth.
 */
const TabContextMenuOverlayRoot: React.FC = () => {
  // Both bulk-close items are disabled until C++ says there is something to close, so a
  // menu that fails to receive its context offers less rather than more.
  const [hasOthers, setHasOthers] = useState(false);
  const [hasRight, setHasRight] = useState(false);

  useEffect(() => {
    window.setTabMenuContext = (others: boolean, right: boolean) => {
      setHasOthers(others);
      setHasRight(right);
    };
    // ⛔ Ask for the context instead of waiting to be told. On the FIRST right-click of a
    // session the overlay's browser is created by the very IPC that shows it, so C++'s
    // push runs before this component exists and the menu renders both bulk items greyed
    // out. 📏 Measured: first open after a fresh start, "Close other tabs" disabled with
    // 4 tabs open. The other overlays solve this by re-injecting at 300 ms and 600 ms;
    // pulling on mount happens exactly when the receiver is ready instead of guessing
    // how long Vite's module waterfall takes.
    window.cefMessage?.send('tab_context_menu_request_context');
    return () => {
      delete window.setTabMenuContext;
    };
  }, []);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        window.cefMessage?.send('tab_context_menu_hide');
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Body data attribute for the CEF-level cursor fix (same as the other dropdowns)
  useEffect(() => {
    document.body.setAttribute('data-overlay', 'tabmenu');
    return () => {
      document.body.removeAttribute('data-overlay');
    };
  }, []);

  const handleAction = useCallback((action: string) => {
    window.cefMessage?.send('tab_context_menu_action', [action]);
  }, []);

  return (
    <Box
      sx={{
        width: '100%',
        height: '100%',
        bgcolor: tokens.bgSurface,
        color: tokens.textPrimary,
        py: 0.5,
        overflow: 'hidden',
      }}
    >
      <MenuItemRow
        icon={<RefreshIcon sx={{ fontSize: 18 }} />}
        label="Reload"
        onClick={() => handleAction('reload')}
      />
      <MenuItemRow
        icon={<ContentCopyIcon sx={{ fontSize: 18 }} />}
        label="Duplicate"
        onClick={() => handleAction('duplicate')}
      />
      <MenuItemRow
        icon={<AddIcon sx={{ fontSize: 18 }} />}
        label="New tab to the right"
        onClick={() => handleAction('new_tab_right')}
      />
      <MenuItemRow
        icon={<BookmarkBorderIcon sx={{ fontSize: 18 }} />}
        label="Bookmark tab"
        onClick={() => handleAction('bookmark')}
      />

      <Divider sx={{ borderColor: tokens.borderDefault, my: 0.5 }} />

      <MenuItemRow
        icon={<CloseIcon sx={{ fontSize: 18 }} />}
        label="Close other tabs"
        disabled={!hasOthers}
        onClick={() => handleAction('close_others')}
      />
      <MenuItemRow
        icon={<ArrowForwardIcon sx={{ fontSize: 18 }} />}
        label="Close tabs to the right"
        disabled={!hasRight}
        onClick={() => handleAction('close_right')}
      />
    </Box>
  );
};

export default TabContextMenuOverlayRoot;
