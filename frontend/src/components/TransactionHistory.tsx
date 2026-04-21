import React, { useState } from 'react';
import type { Transaction } from '../types/transaction';

interface TransactionHistoryProps {
  transactions: Transaction[];
  isLoading?: boolean;
}

export const TransactionHistory: React.FC<TransactionHistoryProps> = ({
  transactions,
  isLoading = false
}) => {
  const [selectedTransaction, setSelectedTransaction] = useState<Transaction | null>(null);

  const formatAmount = (satoshis: number): string => {
    const bsv = satoshis / 100000000;
    return bsv.toFixed(8);
  };

  const formatTimestamp = (timestamp: number): string => {
    const date = new Date(timestamp);
    return date.toLocaleString();
  };

  const getStatusIcon = (status: Transaction['status']): string => {
    switch (status) {
      case 'confirmed':
        return '✅';
      case 'pending':
        return '⏳';
      case 'failed':
        return '❌';
      default:
        return '❓';
    }
  };

  const getStatusColor = (status: Transaction['status']): string => {
    switch (status) {
      case 'confirmed':
        return 'status-confirmed';
      case 'pending':
        return 'status-pending';
      case 'failed':
        return 'status-failed';
      default:
        return 'status-unknown';
    }
  };

  const TransactionDetailModal: React.FC<{ transaction: Transaction; onClose: () => void }> = ({
    transaction,
    onClose
  }) => (
    <div className="transaction-modal-overlay" onClick={onClose}>
      <div className="transaction-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Transaction Details</h3>
          <button className="close-button" onClick={onClose}>×</button>
        </div>

        <div className="modal-content">
          <div className="detail-row">
            <span className="detail-label">Transaction ID:</span>
            <span className="detail-value transaction-id">{transaction.txid}</span>
          </div>

          <div className="detail-row">
            <span className="detail-label">Status:</span>
            <span className={`detail-value ${getStatusColor(transaction.status)}`}>
              {getStatusIcon(transaction.status)} {transaction.status}
            </span>
          </div>

          <div className="detail-row">
            <span className="detail-label">Amount:</span>
            <span className="detail-value">{formatAmount(transaction.amount)} BSV</span>
          </div>

          <div className="detail-row">
            <span className="detail-label">Recipient:</span>
            <span className="detail-value address">{transaction.recipient}</span>
          </div>

          <div className="detail-row">
            <span className="detail-label">Fee:</span>
            <span className="detail-value">{formatAmount(transaction.fee)} BSV</span>
          </div>

          <div className="detail-row">
            <span className="detail-label">Confirmations:</span>
            <span className="detail-value">{transaction.confirmations}</span>
          </div>

          <div className="detail-row">
            <span className="detail-label">Timestamp:</span>
            <span className="detail-value">{formatTimestamp(transaction.timestamp)}</span>
          </div>

          {transaction.memo && (
            <div className="detail-row">
              <span className="detail-label">Memo:</span>
              <span className="detail-value">{transaction.memo}</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );

  if (isLoading) {
    return (
      <div className="transaction-history">
        <div className="history-header">
          <h3>Transaction History</h3>
        </div>
        <div className="loading-state">
          <div className="loading-spinner"></div>
          <span>Loading transactions...</span>
        </div>
      </div>
    );
  }

  if (transactions.length === 0) {
    return (
      <div className="transaction-history">
        <div className="history-header">
          <h3>Transaction History</h3>
        </div>
        <div className="empty-state">
          <div className="empty-icon">📝</div>
          <p>No transactions yet</p>
          <span>Your transaction history will appear here</span>
        </div>
      </div>
    );
  }

  return (
    <div className="transaction-history">
      <div className="history-header">
        <h3>Transaction History</h3>
        <span className="transaction-count">{transactions.length} transactions</span>
      </div>

      <div className="transaction-list">
        {transactions.map((transaction) => (
          <div
            key={transaction.txid}
            className="transaction-item"
            onClick={() => setSelectedTransaction(transaction)}
          >
            <div className="transaction-main">
              <div className="transaction-status">
                <span className={`status-icon ${getStatusColor(transaction.status)}`}>
                  {getStatusIcon(transaction.status)}
                </span>
                <span className="transaction-amount">
                  {formatAmount(transaction.amount)} BSV
                </span>
              </div>

              <div className="transaction-details">
                <div className="transaction-recipient">
                  {transaction.recipient.slice(0, 8)}...{transaction.recipient.slice(-8)}
                </div>
                <div className="transaction-time">
                  {formatTimestamp(transaction.timestamp)}
                </div>
              </div>
            </div>

            <div className="transaction-meta">
              <span className="transaction-fee">
                Fee: {formatAmount(transaction.fee)} BSV
              </span>
              <span className="transaction-confirmations">
                {transaction.confirmations} confirmations
              </span>
            </div>
          </div>
        ))}
      </div>

      {selectedTransaction && (
        <TransactionDetailModal
          transaction={selectedTransaction}
          onClose={() => setSelectedTransaction(null)}
        />
      )}
    </div>
  );
};
