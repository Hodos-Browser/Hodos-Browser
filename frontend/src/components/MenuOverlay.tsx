import React from 'react';
import {
  Box,
  Typography,
  IconButton,
  Divider,
  ClickAwayListener,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import HistoryIcon from '@mui/icons-material/History';
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

interface MenuOverlayProps {
  onClose: () => void;
  onAction: (action: string) => void;
  currentZoom?: number;
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
      '&:hover': disabled ? {} : { backgroundColor: 'rgba(255,255,255,0.08)' },
      userSelect: 'none',
    }}
  >
    {icon && (
      <Box sx={{ width: 24, mr: 1.5, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#aaa' }}>
        {icon}
      </Box>
    )}
    <Typography sx={{ flex: 1, fontSize: '0.82rem', color: '#e0e0e0' }}>
      {label}
    </Typography>
    {shortcut && (
      <Typography sx={{ fontSize: '0.72rem', color: '#777', ml: 2 }}>
        {shortcut}
      </Typography>
    )}
  </Box>
);

const ZoomRow: React.FC<{ currentZoom: number; onAction: (a: string) => void }> = ({ currentZoom, onAction }) => (
  <Box sx={{ display: 'flex', alignItems: 'center', px: 2, py: 0.5, height: 36 }}>
    <ZoomOutIcon sx={{ fontSize: 16, color: '#aaa', mr: 0.5 }} />
    <IconButton size="small" onClick={() => onAction('zoom_out')} sx={{ color: '#e0e0e0', p: 0.5 }}>
      <RemoveIcon sx={{ fontSize: 16 }} />
    </IconButton>
    <Typography sx={{ mx: 1, minWidth: 40, textAlign: 'center', fontSize: '0.78rem', color: '#e0e0e0' }}>
      {currentZoom}%
    </Typography>
    <IconButton size="small" onClick={() => onAction('zoom_in')} sx={{ color: '#e0e0e0', p: 0.5 }}>
      <AddIcon sx={{ fontSize: 16 }} />
    </IconButton>
    <Box sx={{ flex: 1 }} />
    <IconButton size="small" onClick={() => onAction('zoom_reset')} title="Reset zoom" sx={{ color: '#aaa', p: 0.5 }}>
      <ZoomInIcon sx={{ fontSize: 16 }} />
    </IconButton>
    <IconButton size="small" onClick={() => onAction('fullscreen')} title="Fullscreen" sx={{ color: '#aaa', p: 0.5 }}>
      <FullscreenIcon sx={{ fontSize: 16 }} />
    </IconButton>
  </Box>
);

const MenuOverlay: React.FC<MenuOverlayProps> = ({ onClose, onAction, currentZoom = 100 }) => {
  const handleAction = (action: string) => {
    onAction(action);
    onClose();
  };

  return (
    <ClickAwayListener onClickAway={onClose}>
      <Box
        sx={{
          width: 280,
          bgcolor: '#1e1e1e',
          color: '#e0e0e0',
          py: 0.5,
          borderRadius: 1,
          boxShadow: '0 8px 24px rgba(0,0,0,0.5)',
          border: '1px solid #333',
          maxHeight: 'calc(100vh - 60px)',
          overflowY: 'auto',
        }}
        onKeyDown={(e) => {
          if (e.key === 'Escape') onClose();
        }}
      >
        {/* Section 1: Tab/Window */}
        <MenuItemRow
          icon={<AddIcon sx={{ fontSize: 18 }} />}
          label="New Tab"
          shortcut="Ctrl+T"
          onClick={() => handleAction('new_tab')}
        />

        <Divider sx={{ borderColor: '#333', my: 0.5 }} />

        {/* Section 2: Content Access */}
        <MenuItemRow
          icon={<HistoryIcon sx={{ fontSize: 18 }} />}
          label="History"
          shortcut="Ctrl+H"
          onClick={() => handleAction('history')}
        />
        <MenuItemRow
          icon={<DownloadIcon sx={{ fontSize: 18 }} />}
          label="Downloads"
          shortcut="Ctrl+J"
          onClick={() => handleAction('downloads')}
        />

        <Divider sx={{ borderColor: '#333', my: 0.5 }} />

        {/* Section 3: Page Actions */}
        <ZoomRow currentZoom={currentZoom} onAction={handleAction} />
        <MenuItemRow
          icon={<PrintIcon sx={{ fontSize: 18 }} />}
          label="Print..."
          shortcut="Ctrl+P"
          onClick={() => handleAction('print')}
        />
        <MenuItemRow
          icon={<SearchIcon sx={{ fontSize: 18 }} />}
          label="Find in Page"
          shortcut="Ctrl+F"
          onClick={() => handleAction('find')}
        />

        <Divider sx={{ borderColor: '#333', my: 0.5 }} />

        {/* Section 4: Developer Tools */}
        <MenuItemRow
          icon={<CodeIcon sx={{ fontSize: 18 }} />}
          label="Developer Tools"
          shortcut="F12"
          onClick={() => handleAction('devtools')}
        />

        <Divider sx={{ borderColor: '#333', my: 0.5 }} />

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
    </ClickAwayListener>
  );
};

export default MenuOverlay;
