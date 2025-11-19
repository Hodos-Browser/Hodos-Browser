import { useState, useCallback } from 'react';
// Removed useWallet import - private keys handled by Go daemon
import type { TransactionData, Transaction, TransactionResponse } from '../types/transaction';

export const useTransaction = () => {
  const [transactions] = useState<Transaction[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const sendTransaction = useCallback(async (data: TransactionData): Promise<TransactionResponse> => {
    setIsLoading(true);
    setError(null);

    try {
      if (!window.bitcoinBrowser?.wallet) {
        throw new Error('Bitcoin Browser wallet not available');
      }

      const response = await window.bitcoinBrowser.wallet.sendTransaction({
        toAddress: data.recipient,
        amount: Math.round(parseFloat(data.amount) * 100000000),
        feeRate: parseInt(data.feeRate)
      });

      return response;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to send transaction';
      setError(errorMessage);
      throw new Error(errorMessage);
    } finally {
      setIsLoading(false);
    }
  }, []);

  return {
    transactions,
    isLoading,
    error,
    sendTransaction
  };
};
