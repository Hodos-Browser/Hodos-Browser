import React from 'react';
import {
  Drawer,
  Box,
  Typography,
  IconButton,
  Divider,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import SettingsIcon from '@mui/icons-material/Settings';

type Props = {
  onClose: () => void;
  open: boolean;
};

const SettingsPanelLayout: React.FC<Props> = ({ onClose, open }) => {
  console.log("🔧 SettingsPanelLayout render - open:", open);

  return (
    <Drawer
      anchor="right"
      open={open}
      onClose={onClose}
      variant="temporary"
      sx={{
        '& .MuiDrawer-paper': {
          width: 280,
          bgcolor: 'grey.900',
          color: 'white',
        },
      }}
    >
      <Box display="flex" alignItems="center" p={2} justifyContent="space-between">
        <Box display="flex" alignItems="center">
          <SettingsIcon sx={{ mr: 1 }} />
          <Typography variant="h6">Settings</Typography>
        </Box>
        <IconButton onClick={onClose} sx={{ color: 'white' }}>
          <CloseIcon />
        </IconButton>
      </Box>
      <Divider sx={{ bgcolor: 'grey.700' }} />

      <Box p={2}>
        <Typography>Settings content will go here.</Typography>
      </Box>
    </Drawer>
  );
};

export default SettingsPanelLayout;
