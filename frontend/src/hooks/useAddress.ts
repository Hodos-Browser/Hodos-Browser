import { useState, useCallback } from 'react';

export const useAddress = () => {
  const [currentAddress, setCurrentAddress] = useState<string>('');
  const [isGenerating, setIsGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const generateAddress = useCallback(async (): Promise<string> => {
    console.log('🔄 useAddress: generateAddress called');
    setIsGenerating(true);
    setError(null);

    try {
      if (!window.hodosBrowser?.address) {
        console.error('❌ useAddress: Bitcoin Browser API not available');
        throw new Error('Bitcoin Browser API not available');
      }

      console.log('🔄 useAddress: Calling window.hodosBrowser.address.generate()');
      const response = await window.hodosBrowser.address.generate();
      console.log('✅ useAddress: Response received:', response);
      console.log('🔍 useAddress: Response type:', typeof response);
      console.log('🔍 useAddress: Response is null/undefined:', response === null || response === undefined);
      if (response) {
        console.log('🔍 useAddress: Response keys:', Object.keys(response));
        console.log('🔍 useAddress: Response.address:', response.address);
      }

      const address = response.address;
      console.log('✅ useAddress: Address extracted:', address);

      setCurrentAddress(address);
      return address;
    } catch (err) {
      console.error('❌ useAddress: Error in generateAddress:', err);
      const errorMessage = err instanceof Error ? err.message : 'Failed to generate address';
      setError(errorMessage);
      throw new Error(errorMessage);
    } finally {
      setIsGenerating(false);
    }
  }, []);

  const copyToClipboard = useCallback(async (text: string): Promise<void> => {
    try {
      await navigator.clipboard.writeText(text);
      console.log('Address copied to clipboard:', text);
    } catch (err) {
      console.error('Failed to copy to clipboard:', err);
      throw new Error('Failed to copy to clipboard');
    }
  }, []);

  const generateAndCopy = useCallback(async (): Promise<string> => {
    console.log('🔄 useAddress: generateAndCopy called');
    const address = await generateAddress();
    console.log('✅ useAddress: address generated:', address);
    await copyToClipboard(address);
    console.log('✅ useAddress: address copied to clipboard');
    return address;
  }, [generateAddress, copyToClipboard]);

  return {
    currentAddress,
    isGenerating,
    error,
    generateAddress,
    copyToClipboard,
    generateAndCopy
  };
};
