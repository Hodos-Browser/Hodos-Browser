import { useState } from 'react';
import { createPortal } from 'react-dom';
import { IconButton, Menu, MenuItem, ListItemIcon, ListItemText, Divider } from '@mui/material';
import SettingsIcon from '@mui/icons-material/Settings';
import HistoryIcon from '@mui/icons-material/History';
import SettingsOutlinedIcon from '@mui/icons-material/SettingsOutlined';

interface SettingsMenuProps {
  onHistoryClick: () => void;
  onSettingsClick: () => void;
}

export function SettingsMenu({ onHistoryClick, onSettingsClick }: SettingsMenuProps) {
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
  const open = Boolean(anchorEl);

  const handleClick = (event: React.MouseEvent<HTMLElement>) => {
    setAnchorEl(event.currentTarget);
  };

  const handleClose = () => {
    setAnchorEl(null);
  };

  const menuComponent = (
    <Menu
        anchorEl={anchorEl}
        open={open}
        onClose={handleClose}
        anchorOrigin={{
          vertical: 'bottom',
          horizontal: 'right',
        }}
        transformOrigin={{
          vertical: 'top',
          horizontal: 'right',
        }}
        slotProps={{
          paper: {
            elevation: 3,
            sx: {
              mt: 8,
              minWidth: 200,
              maxHeight: 'none',
              borderRadius: 1,
              overflow: 'visible',
              zIndex: 2147483647, // Maximum possible z-index
              position: 'fixed',
              isolation: 'isolate',
              '& .MuiList-root': {
                padding: '8px 0',
                maxHeight: 'none',
                overflow: 'visible',
              },
            },
          },
        }}
        MenuListProps={{
          sx: {
            maxHeight: 'none',
            overflow: 'visible',
          },
        }}
      >
        <MenuItem
          onClick={() => {
            onHistoryClick();
            handleClose();
          }}
          dense
        >
          <ListItemIcon>
            <HistoryIcon fontSize="small" />
          </ListItemIcon>
          <ListItemText>History</ListItemText>
        </MenuItem>

        <Divider />

        <MenuItem
          onClick={() => {
            onSettingsClick();
            handleClose();
          }}
          dense
        >
          <ListItemIcon>
            <SettingsOutlinedIcon fontSize="small" />
          </ListItemIcon>
          <ListItemText>Settings</ListItemText>
        </MenuItem>
      </Menu>
  );

  return (
    <>
      <IconButton
        onClick={handleClick}
        size="small"
        sx={{
          flexShrink: 0,
          color: 'rgba(0, 0, 0, 0.6)',
          '&:hover': {
            backgroundColor: 'rgba(0, 0, 0, 0.04)',
            color: 'rgba(0, 0, 0, 0.87)',
          }
        }}
        aria-label="settings"
      >
        <SettingsIcon fontSize="small" />
      </IconButton>

      {open && createPortal(menuComponent, document.body)}
    </>
  );
}

export default SettingsMenu;
