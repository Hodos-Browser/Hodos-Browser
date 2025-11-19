// WalletPanelLayout.tsx

import React from 'react';
import {
  Drawer,
  Box,
  Typography,
  IconButton,
  Divider,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import AccountBalanceWalletIcon from '@mui/icons-material/AccountBalanceWallet';
import WalletPanel from './WalletPanelContent';

type Props = {
  onClose: () => void;
  open: boolean;
};

const WalletPanelLayout: React.FC<Props> = ({ onClose, open }) => {
  return (
    <Drawer
      anchor="right"
      open={open}
      onClose={onClose}
      variant="temporary"
      sx={{
        '& .MuiDrawer-paper': {
          width: '36%',  // Reduced to 36% for better balance
          bgcolor: '#2d5016', // Dark green background
          color: 'white',
        },
      }}
    >
      <Box display="flex" alignItems="center" p={2} justifyContent="space-between">
        <Box display="flex" alignItems="center">
          <AccountBalanceWalletIcon sx={{ mr: 1 }} />
          <Typography variant="h6">Bitcoin SV Wallet</Typography>
        </Box>
        <IconButton onClick={onClose} sx={{ color: 'white' }}>
          <CloseIcon />
        </IconButton>
      </Box>
      <Divider sx={{ bgcolor: '#d4c4a8' }} />

      {/* Add wallet UI content here */}
      <Box p={2}>
        <WalletPanel />
      </Box>
    </Drawer>
  );
};

export default WalletPanelLayout;
