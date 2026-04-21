import React, { useState, useCallback, useEffect } from 'react';
import { TransactionForm } from '../components/TransactionForm';
import { BalanceDisplay } from '../components/BalanceDisplay';
import { TransactionHistory } from '../components/TransactionHistory';
import { useTransaction } from '../hooks/useTransaction';
import { useBalance } from '../hooks/useBalance';
import type { TransactionResponse } from '../types/transaction';

export const SendPage: React.FC = () => {
  const { balance, usdValue, bsvPrice, isLoading: balanceLoading, isRefreshing: balanceRefreshing, refreshBalance } = useBalance();
  const {
    transactions,
    isLoading: transactionLoading,
    error: transactionError,
    sendTransaction: _sendTransaction,
  } = useTransaction();

  const [showHistory, setShowHistory] = useState(false);
  const [lastTransaction, setLastTransaction] = useState<TransactionResponse | null>(null);

  // Debug logging
  console.log('🎨 Rendering SendPage, lastTransaction:', lastTransaction);

  // Force re-render when lastTransaction changes
  useEffect(() => {
    console.log('🔄 useEffect: lastTransaction changed to:', lastTransaction);
  }, [lastTransaction]);

  const handleTransactionCreated = useCallback(async (result: TransactionResponse) => {
    console.log('🎉 handleTransactionCreated called with:', result);
    try {
      // The transaction creation is handled by the form itself
      // This callback is called after successful creation
      console.log('Transaction created successfully:', result);

      // Show success message
      console.log('📢 Showing success message...');
      console.log(`✅ Transaction created successfully!
Transaction ID: ${result.txid}
Fee: ${result.fee} satoshis
Status: ${result.status}`);

      // Set the last transaction for UI display
      console.log('🎨 Setting lastTransaction state to:', result);
      setLastTransaction(result);
      console.log('🎨 lastTransaction state set');

      // Alert doesn't work well in CEF overlays, so we rely on the visual success message

      // Refresh balance and transaction history (non-blocking)
      console.log('🔄 Refreshing balance...');
      refreshBalance().catch((err: unknown) => console.error('Balance refresh failed:', err));
      console.log('✅ Refresh operations started (non-blocking)');

    } catch (error) {
      console.error('❌ Transaction creation failed:', error);
      // Error is already handled by the hook and displayed in the form
    }
  }, [refreshBalance]);

  const handleRefresh = useCallback(async () => {
    await refreshBalance();
  }, [refreshBalance]);

  return (
    <div className="send-page">
      <div className="page-header">
        <h1>Bitcoin SV Wallet</h1>
        <div className="page-actions">
          <button
            className="toggle-history-button"
            onClick={() => setShowHistory(!showHistory)}
          >
            {showHistory ? 'Hide History' : 'Show History'}
          </button>
          <button
            className="refresh-button"
            onClick={handleRefresh}
            disabled={balanceLoading || balanceRefreshing || transactionLoading}
          >
            Refresh
          </button>
        </div>
      </div>

      <div className="page-content">
        <div className="main-section">
          <BalanceDisplay
            balance={balance}
            usdValue={usdValue}
            isLoading={balanceLoading}
            isRefreshing={balanceRefreshing}
            onRefresh={handleRefresh}
          />

          {/* Success Message */}
          {lastTransaction && (
            <div className="success-message" style={{
              background: '#10b981',
              color: 'white',
              padding: '16px',
              borderRadius: '8px',
              marginBottom: '16px',
              fontSize: '14px'
            }}>
              <strong>✅ Transaction Created Successfully!</strong>
              <br />
              <strong>Transaction ID:</strong> {lastTransaction.txid}
              <br />
              <strong>Fee:</strong> {lastTransaction.fee} satoshis
              <br />
              <strong>Status:</strong> {lastTransaction.status}
            </div>
          )}

          <TransactionForm
            onTransactionCreated={handleTransactionCreated}
            balance={balance}
            bsvPrice={bsvPrice}
            isLoading={transactionLoading}
            error={transactionError}
          />
        </div>

        {showHistory && (
          <div className="history-section">
            <TransactionHistory
              transactions={transactions}
              isLoading={transactionLoading}
            />
          </div>
        )}
      </div>

      {transactionError && (
        <div className="error-overlay">
          <div className="error-message">
            <span className="error-icon">⚠️</span>
            <span className="error-text">{transactionError}</span>
          </div>
        </div>
      )}
    </div>
  );
};
