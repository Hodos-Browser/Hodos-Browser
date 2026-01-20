import React, { useState, useEffect } from 'react';
import { Button } from '@mui/material';
import SettingsIcon from '@mui/icons-material/Settings';
import SendIcon from '@mui/icons-material/Send';
import CallReceivedIcon from '@mui/icons-material/CallReceived';
import { useWallet } from '../hooks/useWallet';

export default function WalletPanel() {
  const [balance, setBalance] = useState<number>(0);
  const [loading, setLoading] = useState(true);
  const wallet = useWallet();

  // Fetch balance on component mount
  useEffect(() => {
    const fetchBalance = async () => {
      try {
        const balanceData = await wallet.getBalance();
        if (balanceData && typeof balanceData.balance === 'number') {
          setBalance(balanceData.balance);
        }
        setLoading(false);
      } catch (error) {
        console.error('Failed to fetch balance:', error);
        setLoading(false);
      }
    };

    fetchBalance();
  }, [wallet]);

  const handleSend = async () => {
    try {
      const recipient = window.prompt('Enter recipient BSV address:');
      if (!recipient) return;

      const amountStr = window.prompt('Enter amount in satoshis:');
      if (!amountStr) return;

      const amount = parseInt(amountStr, 10);
      if (isNaN(amount) || amount <= 0) {
        console.error('Invalid amount. Must be a positive number.');
        window.alert('Invalid amount. Must be a positive number.');
        return;
      }

      // Basic validation for BSV address (starts with 1 or 3, length 26-35)
      if (!recipient.match(/^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/)) {
        console.error('Invalid BSV address format.');
        window.alert('Invalid BSV address format.');
        return;
      }

      const result = await wallet.sendTransaction(recipient, amount);
      console.log('Transaction sent successfully:', result);
      window.alert('Transaction sent successfully!');

      // Refresh balance after sending
      const balanceData = await wallet.getBalance();
      if (balanceData && typeof balanceData.balance === 'number') {
        setBalance(balanceData.balance);
      }
    } catch (error) {
      console.error('Failed to send transaction:', error);
      window.alert(`Failed to send transaction: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  };

  const handleReceive = async () => {
    try {
      const addressData = await wallet.getCurrentAddress();
      console.log('Receive address data:', addressData);

      // Parse the nested response structure: { success: true, address: { address: "...", ... } }
      let address: string | undefined;
      if (addressData && (addressData as any).success && (addressData as any).address) {
        address = (addressData as any).address.address;
      }

      if (address) {
        console.log('Displaying receive address:', address);
        window.alert(`Receive address:\n${address}`);
      } else {
        // Fallback: generate new address if none exists
        console.log('No current address, generating new one...');
        const newAddressData = await wallet.generateAddress();
        console.log('New address data:', newAddressData);

        // Handle same nested structure for generated address
        let newAddress: string | undefined;
        if (newAddressData && (newAddressData as any).success && (newAddressData as any).address) {
          newAddress = (newAddressData as any).address.address;
        } else if (newAddressData && (newAddressData as any).address) {
          newAddress = (newAddressData as any).address;
        }

        if (newAddress) {
          console.log('Displaying generated address:', newAddress);
          window.alert(`Receive address:\n${newAddress}`);
        }
      }
    } catch (error) {
      console.error('Failed to get receive address:', error);
      window.alert(`Failed to get receive address: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
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
