import React, { useState, useEffect, useCallback } from 'react';
import {
  Box,
  Typography,
  Divider,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import HistoryIcon from '@mui/icons-material/History';
import BookmarkBorderIcon from '@mui/icons-material/BookmarkBorder';
import DownloadIcon from '@mui/icons-material/Download';
import PrintIcon from '@mui/icons-material/Print';
import SearchIcon from '@mui/icons-material/Search';
import ZoomInIcon from '@mui/icons-material/ZoomIn';
import ZoomOutIcon from '@mui/icons-material/ZoomOut';
import FullscreenIcon from '@mui/icons-material/Fullscreen';
import CodeIcon from '@mui/icons-material/Code';
import SettingsIcon from '@mui/icons-material/Settings';
import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';
import CloseIcon from '@mui/icons-material/Close';
import RemoveIcon from '@mui/icons-material/Remove';
import { HodosButton } from '../components/HodosButton';
import { tokens } from '../theme/tokens';

declare global {
  interface Window {
    setMenuZoomLevel?: (level: number) => void;
  }
}

interface MenuItemRowProps {
  icon?: React.ReactNode;
  label: string;
  shortcut?: string;
  onClick: () => void;
  disabled?: boolean;
}

const MenuItemRow: React.FC<MenuItemRowProps> = ({ icon, label, shortcut, onClick, disabled }) => (
  <Box
    onClick={disabled ? undefined : onClick}
    sx={{
      display: 'flex',
      alignItems: 'center',
      px: 2,
      py: 0.75,
      cursor: disabled ? 'default' : 'pointer',
      opacity: disabled ? 0.5 : 1,
      '&:hover': disabled ? {} : { backgroundColor: tokens.bgSurfaceHover },
      userSelect: 'none',
    }}
  >
    {icon && (
      <Box sx={{ width: 24, mr: 1.5, display: 'flex', alignItems: 'center', justifyContent: 'center', color: tokens.textSecondary }}>
        {icon}
      </Box>
    )}
    <Typography sx={{ flex: 1, fontSize: '0.82rem', color: tokens.textPrimary }}>
      {label}
    </Typography>
    {shortcut && (
      <Typography sx={{ fontSize: '0.72rem', color: tokens.textMuted, ml: 2 }}>
        {shortcut}
      </Typography>
    )}
  </Box>
);

const ZoomRow: React.FC<{ currentZoom: number; onAction: (a: string) => void }> = ({ currentZoom, onAction }) => (
  <Box sx={{ display: 'flex', alignItems: 'center', px: 2, py: 0.5, height: 36 }}>
    <ZoomOutIcon sx={{ fontSize: 16, color: tokens.textSecondary, mr: 0.5 }} />
    <HodosButton variant="icon" size="small" onClick={() => onAction('zoom_out')} aria-label="Zoom out">
      <RemoveIcon sx={{ fontSize: 16 }} />
    </HodosButton>
    <Typography sx={{ mx: 1, minWidth: 40, textAlign: 'center', fontSize: '0.78rem', color: tokens.textPrimary }}>
      {currentZoom}%
    </Typography>
    <HodosButton variant="icon" size="small" onClick={() => onAction('zoom_in')} aria-label="Zoom in">
      <AddIcon sx={{ fontSize: 16 }} />
    </HodosButton>
    <Box sx={{ flex: 1 }} />
    <HodosButton variant="icon" size="small" onClick={() => onAction('zoom_reset')} aria-label="Reset zoom">
      <ZoomInIcon sx={{ fontSize: 16 }} />
    </HodosButton>
    <HodosButton variant="icon" size="small" onClick={() => onAction('fullscreen')} aria-label="Fullscreen">
      <FullscreenIcon sx={{ fontSize: 16 }} />
    </HodosButton>
  </Box>
);

const MenuOverlayRoot: React.FC = () => {
  const [currentZoom, setCurrentZoom] = useState(100);

  // Register callback for C++ to inject zoom level
  useEffect(() => {
    window.setMenuZoomLevel = (level: number) => {
      setCurrentZoom(level);
    };
    return () => {
      delete window.setMenuZoomLevel;
    };
  }, []);

  // Escape key to close
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        window.cefMessage?.send('menu_hide');
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Set body data attribute for CEF-level cursor fix
  useEffect(() => {
    document.body.setAttribute('data-overlay', 'menu');
    return () => {
      document.body.removeAttribute('data-overlay');
    };
  }, []);

  const handleAction = useCallback((action: string) => {
    window.cefMessage?.send('menu_action', [action]);
  }, []);

  return (
    <Box
      sx={{
        width: '100%',
        height: '100%',
        bgcolor: tokens.bgSurface,
        color: tokens.textPrimary,
        py: 0.5,
        overflowY: 'auto',
      }}
    >
      {/* Section 1: Tab/Window */}
      <MenuItemRow
        icon={<AddIcon sx={{ fontSize: 18 }} />}
        label="New Tab"
        shortcut="Cmd+T"
        onClick={() => handleAction('new_tab')}
      />

      <Divider sx={{ borderColor: tokens.borderDefault, my: 0.5 }} />

      {/* Section 2: Content Access */}
      <MenuItemRow
        icon={<HistoryIcon sx={{ fontSize: 18 }} />}
        label="History"
        shortcut="Cmd+H"
        onClick={() => handleAction('history')}
      />
      <MenuItemRow
        icon={<BookmarkBorderIcon sx={{ fontSize: 18 }} />}
        label="Bookmarks"
        shortcut="Cmd+D"
        onClick={() => handleAction('bookmarks')}
      />
      <MenuItemRow
        icon={<DownloadIcon sx={{ fontSize: 18 }} />}
        label="Downloads"
        shortcut="Cmd+J"
        onClick={() => handleAction('downloads')}
      />

      <Divider sx={{ borderColor: tokens.borderDefault, my: 0.5 }} />

      {/* Section 3: Page Actions */}
      <ZoomRow currentZoom={currentZoom} onAction={handleAction} />
      <MenuItemRow
        icon={<PrintIcon sx={{ fontSize: 18 }} />}
        label="Print..."
        shortcut="Cmd+P"
        onClick={() => handleAction('print')}
      />
      <MenuItemRow
        icon={<SearchIcon sx={{ fontSize: 18 }} />}
        label="Find in Page"
        shortcut="Cmd+F"
        onClick={() => handleAction('find')}
      />

      <Divider sx={{ borderColor: tokens.borderDefault, my: 0.5 }} />

      {/* Section 4: Developer Tools */}
      <MenuItemRow
        icon={<CodeIcon sx={{ fontSize: 18 }} />}
        label="Developer Tools"
        shortcut="F12"
        onClick={() => handleAction('devtools')}
      />

      <Divider sx={{ borderColor: tokens.borderDefault, my: 0.5 }} />

      {/* Section 5: Settings + Exit */}
      <MenuItemRow
        icon={<SettingsIcon sx={{ fontSize: 18 }} />}
        label="Settings"
        onClick={() => handleAction('settings')}
      />
      <MenuItemRow
        icon={<InfoOutlinedIcon sx={{ fontSize: 18 }} />}
        label="About Hodos"
        onClick={() => handleAction('about')}
      />
      <MenuItemRow
        icon={<CloseIcon sx={{ fontSize: 18 }} />}
        label="Exit"
        onClick={() => handleAction('exit')}
      />
    </Box>
  );
};

export default MenuOverlayRoot;
