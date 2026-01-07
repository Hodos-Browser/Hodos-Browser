import React, { useState, useEffect } from 'react';
import { Button } from '@mui/material';
import SettingsIcon from '@mui/icons-material/Settings';
import SendIcon from '@mui/icons-material/Send';
import CallReceivedIcon from '@mui/icons-material/CallReceived';

export default function WalletPanel() {
  const [balance, setBalance] = useState<number>(0);
  const [loading, setLoading] = useState(true);

  // Fetch balance on component mount
  useEffect(() => {
    const fetchBalance = async () => {
      try {
        if (window.hodosBrowser?.wallet?.getBalance) {
          // Set up callback for balance response
          window.onGetBalanceResponse = (data: any) => {
            console.log('Balance received:', data);
            if (data && typeof data.balance === 'number') {
              setBalance(data.balance);
            }
            setLoading(false);
          };

          window.onGetBalanceError = (error: any) => {
            console.error('Balance error:', error);
            setLoading(false);
          };

          // Request balance
          window.hodosBrowser.wallet.getBalance();
        }
      } catch (error) {
        console.error('Failed to fetch balance:', error);
        setLoading(false);
      }
    };

    fetchBalance();
  }, []);

  const handleSend = () => {
    console.log('Send button clicked (not implemented yet)');
    // TODO: Implement send functionality
  };

  const handleReceive = () => {
    console.log('Receive button clicked (not implemented yet)');
    // TODO: Implement receive functionality
  };

  const handleAdvanced = () => {
    console.log('Advanced button clicked - opening wallet page');
    // Open wallet page in new tab (like history does)
    if (window.hodosBrowser?.navigation?.navigate) {
      window.hodosBrowser.navigation.navigate('/wallet');
    }
  };

  return (
    <div style={{
      width: '240px',
      height: '200px',
      display: 'flex',
      flexDirection: 'column',
      padding: '16px',
      backgroundColor: '#ffffff',
      border: '1px solid #e0e0e0',
      borderRadius: '8px',
      boxShadow: '0 2px 8px rgba(0,0,0,0.1)',
      boxSizing: 'border-box',
      gap: '8px'
    }}>
      {/* Balance at top */}
      <div style={{
        fontSize: '14px',
        fontWeight: 600,
        color: '#333',
        textAlign: 'center',
        marginBottom: '4px'
      }}>
        Balance: {loading ? '...' : `${balance.toLocaleString()} sats`}
      </div>

      {/* Send button */}
      <Button
        variant="contained"
        color="primary"
        startIcon={<SendIcon />}
        onClick={handleSend}
        disabled
        fullWidth
        size="small"
        sx={{ fontSize: '12px' }}
      >
        Send
      </Button>

      {/* Receive button */}
      <Button
        variant="contained"
        color="secondary"
        startIcon={<CallReceivedIcon />}
        onClick={handleReceive}
        disabled
        fullWidth
        size="small"
        sx={{ fontSize: '12px' }}
      >
        Receive
      </Button>

      {/* Advanced button */}
      <Button
        variant="outlined"
        startIcon={<SettingsIcon />}
        onClick={handleAdvanced}
        fullWidth
        size="small"
        sx={{ fontSize: '12px' }}
      >
        Advanced
      </Button>
    </div>
  );
}
