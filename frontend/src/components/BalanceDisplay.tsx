import React from 'react';
import type { BalanceData } from '../types/transaction';

interface BalanceDisplayProps extends BalanceData {
  onRefresh?: () => void;
}

export const BalanceDisplay: React.FC<BalanceDisplayProps> = ({
  balance,
  usdValue,
  isLoading,
  onRefresh
}) => {
  const formatBalance = (satoshis: number): string => {
    const bsv = satoshis / 100000000;
    return bsv.toFixed(8);
  };

  const formatUsdValue = (value: number): string => {
    return value.toFixed(2);
  };

  return (
    <div className="balance-display">
      <div className="balance-header">
        <h2>Wallet Balance</h2>
        {onRefresh && (
          <button
            className="refresh-button"
            onClick={onRefresh}
            disabled={isLoading}
            title="Refresh balance"
          >
            {isLoading ? '⟳' : '↻'}
          </button>
        )}
      </div>

      <div className="balance-content">
        <div className="balance-primary">
          <span className="balance-amount">
            {isLoading ? '...' : formatBalance(balance)}
          </span>
          <span className="balance-currency">BSV</span>
        </div>

        <div className="balance-secondary">
          <span className="balance-usd">
            ${isLoading ? '...' : formatUsdValue(usdValue)}
          </span>
          <span className="balance-usd-label">USD</span>
        </div>
      </div>

      {isLoading && (
        <div className="balance-loading">
          <div className="loading-spinner"></div>
          <span>Updating balance...</span>
        </div>
      )}
    </div>
  );
};
