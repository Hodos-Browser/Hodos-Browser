import { useState, useCallback } from 'react';

interface WalletState {
  address: string | null;
  mnemonic: string | null;
  isInitialized: boolean;
  backedUp: boolean;
  version: string | null;
}

export const useWallet = () => {
  const [walletState, setWalletState] = useState<WalletState>({
    address: null,
    mnemonic: null,
    isInitialized: false,
    backedUp: false,
    version: null
  });

  const checkWalletStatus = useCallback(async () => {
    try {
      if (!window.bitcoinBrowser?.wallet) {
        throw new Error('Bitcoin Browser API not available');
      }

      const status = await window.bitcoinBrowser.wallet.getStatus();
      return status;
    } catch (error) {
      console.error('❌ Failed to check wallet status:', error);
      throw error;
    }
  }, []);

  const createWallet = useCallback(async () => {
    try {
      if (!window.bitcoinBrowser?.wallet) {
        throw new Error('Bitcoin Browser API not available');
      }

      const walletData = await window.bitcoinBrowser.wallet.create();

      setWalletState({
        address: walletData.address,
        mnemonic: walletData.mnemonic,
        isInitialized: true,
        backedUp: false,
        version: walletData.version
      });

      console.log('🔑 Wallet created with mnemonic:', walletData.mnemonic);
      return walletData;
    } catch (error) {
      console.error('❌ Failed to create wallet:', error);
      throw error;
    }
  }, []);

  const loadWallet = useCallback(async () => {
    try {
      if (!window.bitcoinBrowser?.wallet) {
        throw new Error('Bitcoin Browser API not available');
      }

      const walletData = await window.bitcoinBrowser.wallet.load();

      setWalletState({
        address: walletData.address,
        mnemonic: walletData.mnemonic,
        isInitialized: true,
        backedUp: walletData.backedUp,
        version: walletData.version
      });

      console.log('📂 Wallet loaded successfully');
      return walletData;
    } catch (error) {
      console.error('❌ Failed to load wallet:', error);
      throw error;
    }
  }, []);

  const getWalletInfo = useCallback(async () => {
    try {
      if (!window.bitcoinBrowser?.wallet) {
        throw new Error('Bitcoin Browser API not available');
      }

      const walletInfo = await window.bitcoinBrowser.wallet.getInfo();
      return walletInfo;
    } catch (error) {
      console.error('❌ Failed to get wallet info:', error);
      throw error;
    }
  }, []);

  const generateAddress = useCallback(async () => {
    try {
      if (!window.bitcoinBrowser?.wallet) {
        throw new Error('Bitcoin Browser API not available');
      }

      const addressData = await window.bitcoinBrowser.wallet.generateAddress();

      setWalletState(prev => ({
        ...prev,
        address: addressData.address
      }));

      console.log('📍 New address generated:', addressData.address);
      return addressData;
    } catch (error) {
      console.error('❌ Failed to generate address:', error);
      throw error;
    }
  }, []);

  const getCurrentAddress = useCallback(async () => {
    try {
      if (!window.bitcoinBrowser?.wallet) {
        throw new Error('Bitcoin Browser API not available');
      }

      const addressData = await window.bitcoinBrowser.wallet.getCurrentAddress();
      return addressData;
    } catch (error) {
      console.error('❌ Failed to get current address:', error);
      throw error;
    }
  }, []);

  const markBackedUp = useCallback(async () => {
    try {
      if (!window.bitcoinBrowser?.wallet) {
        throw new Error('Bitcoin Browser API not available');
      }

      const result = await window.bitcoinBrowser.wallet.markBackedUp();

      setWalletState(prev => ({
        ...prev,
        backedUp: true
      }));

      console.log('✅ Wallet marked as backed up');
      return result;
    } catch (error) {
      console.error('❌ Failed to mark wallet as backed up:', error);
      throw error;
    }
  }, []);

  return {
    ...walletState,
    checkWalletStatus,
    createWallet,
    loadWallet,
    getWalletInfo,
    generateAddress,
    getCurrentAddress,
    markBackedUp
  };
};
