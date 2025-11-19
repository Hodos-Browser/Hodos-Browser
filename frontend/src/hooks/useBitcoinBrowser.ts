import { useCallback } from 'react';
import type { IdentityResult } from '../types/identity';
import type { AddressData } from '../types/address';

export function useBitcoinBrowser() {
  const getIdentity = useCallback(async (): Promise<IdentityResult> => {
    if (!window.bitcoinBrowser?.identity?.get) {
      throw new Error('bitcoinBrowser.identity.get not available');
    }
    const result = await window.bitcoinBrowser.identity.get();
    return result;
  }, []);

  const markBackedUp = useCallback(async (): Promise<string> => {
    if (!window.bitcoinBrowser?.identity?.markBackedUp) {
      throw new Error('bitcoinBrowser.identity.markBackedUp not available');
    }
    const result = await window.bitcoinBrowser.identity.markBackedUp();
    return result;
  }, []);

  const generateAddress = useCallback(async (): Promise<AddressData> => {
    if (!window.bitcoinBrowser?.address?.generate) {
      throw new Error('bitcoinBrowser.address.generate not available');
    }

    // Check if we're in an overlay (wallet, settings, backup) - direct V8 call
    const currentPath = window.location.pathname;
    if (currentPath.includes('/wallet') || currentPath.includes('/settings') || currentPath.includes('/backup')) {
      console.log('🔑 Direct V8 call for overlay browser');
      const result = await window.bitcoinBrowser.address.generate();
      return result;
    }

    // For main browser, use message-based communication
    console.log('🔑 Message-based call for main browser');

    return new Promise((resolve, reject) => {
      const handleResponse = (event: any) => {
        if (event.detail.message === 'address_generate_response') {
          try {
            const addressData = JSON.parse(event.detail.args[0]);
            window.removeEventListener('cefMessageResponse', handleResponse);
            resolve(addressData);
          } catch (err) {
            window.removeEventListener('cefMessageResponse', handleResponse);
            reject(err);
          }
        } else if (event.detail.message === 'address_generate_error') {
          window.removeEventListener('cefMessageResponse', handleResponse);
          reject(new Error(event.detail.args[0]));
        }
      };

      window.addEventListener('cefMessageResponse', handleResponse);

      // Call the V8 function which will send a message for main browser
      window.bitcoinBrowser.address.generate().catch(reject);

      // Timeout after 10 seconds
      setTimeout(() => {
        window.removeEventListener('cefMessageResponse', handleResponse);
        reject(new Error('Address generation timeout'));
      }, 10000);
    });
  }, []);

  const navigate = useCallback((path: string): void => {
    if (!window.bitcoinBrowser?.navigation?.navigate) {
      console.warn('bitcoinBrowser.navigation.navigate not available');
      return;
    }
    try {
      window.bitcoinBrowser.navigation.navigate(path);
    } catch (err) {
      console.error("Navigation error:", err);
    }
  }, []);

  const goBack = useCallback((): void => {
    console.log('🔙 Going back in browser history');
    window.cefMessage?.send('navigate_back', []);
  }, []);

  const goForward = useCallback((): void => {
    console.log('🔜 Going forward in browser history');
    window.cefMessage?.send('navigate_forward', []);
  }, []);

  const reload = useCallback((): void => {
    console.log('🔄 Reloading current page');
    window.cefMessage?.send('navigate_reload', []);
  }, []);

  return {
    getIdentity,
    markBackedUp,
    generateAddress,
    navigate,
    goBack,
    goForward,
    reload,
  };
}
