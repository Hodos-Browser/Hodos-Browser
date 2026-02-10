import React, { useState, useCallback, useEffect } from 'react';
import { useTransaction } from '../hooks/useTransaction';
import { getCachedPrice } from '../services/balanceCache';
import type { TransactionData, TransactionResponse } from '../types/transaction';

interface TransactionFormProps {
  onTransactionCreated: (result: TransactionResponse) => void;
  balance: number;
  isLoading?: boolean;
  error?: string | null;
}

type AmountInputMode = 'bsv' | 'usd';

export const TransactionForm: React.FC<TransactionFormProps> = ({
  onTransactionCreated,
  balance,
  isLoading = false,
  error
}) => {
  const { sendTransaction } = useTransaction();
  // Private key is handled by the Go daemon using the identity file
  // Note: feeRate is hardcoded to '5' (medium) for simplicity in the light wallet
  const [formData, setFormData] = useState<TransactionData>({
    recipient: '',
    amount: '',
    feeRate: '5', // Default medium fee rate - not user-configurable in light wallet
    memo: ''
  });
  const [errors, setErrors] = useState<Partial<TransactionData>>({});
  const [isSubmitting, setIsSubmitting] = useState(false);

  // USD conversion state — seed price from localStorage cache if available
  const [amountInputMode, setAmountInputMode] = useState<AmountInputMode>('bsv');
  const [bsvPrice, setBsvPrice] = useState<number>(() => getCachedPrice()?.price ?? 0);
  const [isFetchingPrice, setIsFetchingPrice] = useState(false);

  // Fetch BSV price when switching to USD mode
  const fetchBsvPrice = useCallback(async (): Promise<number> => {
    if (bsvPrice > 0) {
      return bsvPrice; // Use cached price if available
    }

    setIsFetchingPrice(true);
    try {
      console.log('🔍 Fetching BSV price for USD conversion...');
      const response = await fetch('https://min-api.cryptocompare.com/data/price?fsym=BSV&tsyms=USD', {
        method: 'GET',
        headers: {
          'Accept': 'application/json',
          'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        },
        mode: 'cors'
      });

      if (!response.ok) {
        throw new Error(`Price API failed with status: ${response.status}`);
      }

      const data = await response.json();
      const price = parseFloat(data.USD);

      if (!price || price <= 0) {
        throw new Error('Invalid price data received');
      }

      setBsvPrice(price);
      console.log(`💰 BSV Price: $${price}`);
      return price;
    } catch (err) {
      console.error('❌ Failed to fetch BSV price:', err);
      throw err;
    } finally {
      setIsFetchingPrice(false);
    }
  }, [bsvPrice]);

  // Fetch price when switching to USD mode
  useEffect(() => {
    if (amountInputMode === 'usd' && bsvPrice === 0) {
      fetchBsvPrice().catch(err => {
        console.error('Failed to fetch price:', err);
      });
    }
  }, [amountInputMode, bsvPrice, fetchBsvPrice]);

  // Convert USD to satoshis
  const convertUsdToSatoshis = useCallback((usdAmount: number, price: number): number => {
    if (price <= 0) return 0;
    const bsvAmount = usdAmount / price;
    return Math.floor(bsvAmount * 100000000); // Convert to satoshis
  }, []);

  // Convert satoshis to USD
  const convertSatoshisToUsd = useCallback((satoshis: number, price: number): number => {
    if (price <= 0) return 0;
    const bsvAmount = satoshis / 100000000;
    return bsvAmount * price;
  }, []);

  const validateForm = useCallback((): boolean => {
    const newErrors: Partial<TransactionData> = {};

    // Validate recipient address
    if (!formData.recipient.trim()) {
      newErrors.recipient = 'Recipient address is required';
    } else if (!/^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/.test(formData.recipient)) {
      newErrors.recipient = 'Invalid Bitcoin address format';
    }

    // Validate amount (handle both BSV and USD modes)
    const amount = parseFloat(formData.amount);
    if (!formData.amount.trim()) {
      newErrors.amount = 'Amount is required';
    } else if (isNaN(amount) || amount <= 0) {
      newErrors.amount = 'Amount must be a positive number';
    } else {
      let satoshis: number;

      if (amountInputMode === 'usd') {
        if (bsvPrice <= 0) {
          newErrors.amount = 'Price not available. Please switch to BSV mode or try again.';
        } else {
          satoshis = convertUsdToSatoshis(amount, bsvPrice);
          if (satoshis < 546) {
            newErrors.amount = 'Amount too small (minimum ~$0.01 USD)';
          } else if (satoshis > balance) {
            newErrors.amount = 'Insufficient balance';
          }
        }
      } else {
        // BSV mode
        if (amount < 0.00000546) { // Minimum 546 satoshis
          newErrors.amount = 'Amount must be at least 546 satoshis (0.00000546 BSV)';
        } else if (amount * 100000000 > balance) {
          newErrors.amount = 'Insufficient balance';
        }
      }
    }

    // Fee rate is hardcoded to 5 sat/byte - no validation needed

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  }, [formData, balance, amountInputMode, bsvPrice, convertUsdToSatoshis]);

  const handleInputChange = useCallback((field: keyof TransactionData, value: string) => {
    setFormData(prev => ({ ...prev, [field]: value }));

    // Clear error for this field when user starts typing
    if (errors[field]) {
      setErrors(prev => ({ ...prev, [field]: undefined }));
    }
  }, [errors]);

  const handleAmountModeToggle = useCallback(async () => {
    const newMode = amountInputMode === 'bsv' ? 'usd' : 'bsv';

    // If switching to USD, fetch price if needed
    if (newMode === 'usd' && bsvPrice === 0) {
      try {
        await fetchBsvPrice();
      } catch (err) {
        console.error('Failed to fetch price:', err);
        // Still allow switching, but validation will fail
      }
    }

    // Convert current amount when switching modes
    if (formData.amount) {
      const currentAmount = parseFloat(formData.amount);
      if (!isNaN(currentAmount) && currentAmount > 0) {
        if (amountInputMode === 'bsv' && bsvPrice > 0) {
          // Converting from BSV to USD
          const usdAmount = convertSatoshisToUsd(currentAmount * 100000000, bsvPrice);
          setFormData(prev => ({ ...prev, amount: usdAmount.toFixed(2) }));
        } else if (amountInputMode === 'usd' && bsvPrice > 0) {
          // Converting from USD to BSV
          const satoshis = convertUsdToSatoshis(currentAmount, bsvPrice);
          const bsvAmount = satoshis / 100000000;
          setFormData(prev => ({ ...prev, amount: bsvAmount.toFixed(8) }));
        }
      }
    }

    setAmountInputMode(newMode);
  }, [amountInputMode, bsvPrice, formData.amount, fetchBsvPrice, convertSatoshisToUsd, convertUsdToSatoshis]);

  const handleSubmit = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();

    if (!validateForm()) {
      console.log('❌ Form validation failed, not submitting');
      return;
    }

    setIsSubmitting(true);
    try {
      console.log('🚀 Form: Starting transaction send with:', formData);

      // Convert amount to satoshis if in USD mode
      let transactionData = { ...formData };
      if (amountInputMode === 'usd') {
        const usdAmount = parseFloat(formData.amount);
        const satoshis = convertUsdToSatoshis(usdAmount, bsvPrice);
        const bsvAmount = satoshis / 100000000;
        transactionData = {
          ...formData,
          amount: bsvAmount.toFixed(8) // Convert to BSV format for backend
        };
        console.log(`💰 Converted $${usdAmount.toFixed(2)} USD to ${bsvAmount.toFixed(8)} BSV (${satoshis} satoshis)`);
      }

      // Send transaction (create + sign + broadcast in one call)
      console.log('📝 Sending transaction...');
      const result = await sendTransaction(transactionData);

      // Check if transaction was successful
      if (result.success === false || result.status === 'failed') {
        console.error('❌ Transaction failed:', result.error || result.message);
        // Still call callback to show error message to user
        try {
          onTransactionCreated(result);
          console.log('🔄 Form: onTransactionCreated called with error result');
        } catch (err) {
          console.error('❌ Form: Callback failed with error result:', err);
          // Don't crash - just log the error
        }
        return; // Don't reset form or show success message
      }

      console.log('✅ Transaction sent successfully:', result);
      console.log('🎉 Transaction send successful:', result);

      // Reset form immediately (don't wait for callback)
      setFormData({
        recipient: '',
        amount: '',
        feeRate: '5',
        memo: ''
      });
      console.log('✅ Form: Form reset completed');
      setErrors({});
      console.log('✅ Form: Errors cleared');

      // Call callback with result
      try {
        onTransactionCreated(result);
        console.log('🔄 Form: onTransactionCreated called with result');
      } catch (err) {
        console.error('❌ Form: Callback failed:', err);
      }
    } catch (err) {
      console.error('❌ Form: Transaction flow failed:', err);
    } finally {
      console.log('🏁 Form: Setting isSubmitting to false');
      setIsSubmitting(false);
    }
  }, [formData, validateForm, sendTransaction, onTransactionCreated, amountInputMode, bsvPrice, convertUsdToSatoshis]);

  const formatBalance = useCallback((satoshis: number): string => {
    const bsv = satoshis / 100000000;
    return bsv.toFixed(8);
  }, []);

  return (
    <div className="transaction-form">
      <div className="form-header">
        <h3>Send Bitcoin SV</h3>
        <div className="balance-info">
          <span className="balance-label">Available Balance:</span>
          <span className="balance-amount">{formatBalance(balance)} BSV</span>
        </div>
      </div>

      {error && (
        <div className="error-message">
          <span className="error-icon">⚠️</span>
          {error}
        </div>
      )}

      <form onSubmit={handleSubmit} className="transaction-form-content">
        <div className="form-group">
          <label htmlFor="recipient">Recipient Address</label>
          <input
            id="recipient"
            type="text"
            value={formData.recipient}
            onChange={(e) => handleInputChange('recipient', e.target.value)}
            placeholder="Enter Bitcoin SV address"
            className={errors.recipient ? 'error' : ''}
            disabled={isSubmitting || isLoading}
          />
          {errors.recipient && <span className="field-error">{errors.recipient}</span>}
        </div>

        <div className="form-group">
          <div className="amount-label-container">
            <label htmlFor="amount">
              Amount ({amountInputMode === 'usd' ? 'USD' : 'BSV'})
            </label>
            <button
              type="button"
              className={`amount-mode-toggle ${amountInputMode === 'usd' ? 'usd-active' : ''}`}
              onClick={handleAmountModeToggle}
              disabled={isSubmitting || isLoading || isFetchingPrice}
              title={`Switch to ${amountInputMode === 'bsv' ? 'USD' : 'BSV'} input`}
            >
              {isFetchingPrice ? '⏳' : amountInputMode === 'bsv' ? '💵 USD' : '₿ BSV'}
            </button>
          </div>
          <div className="amount-input-container">
            <input
              id="amount"
              type="number"
              step={amountInputMode === 'usd' ? '0.01' : '0.00000001'}
              min={amountInputMode === 'usd' ? '0.01' : '0.00000546'}
              max={amountInputMode === 'usd'
                ? (bsvPrice > 0 ? convertSatoshisToUsd(balance, bsvPrice).toFixed(2) : undefined)
                : formatBalance(balance)}
              value={formData.amount}
              onChange={(e) => handleInputChange('amount', e.target.value)}
              placeholder={amountInputMode === 'usd' ? '0.00' : '0.00000000'}
              className={errors.amount ? 'error' : ''}
              disabled={isSubmitting || isLoading}
            />
            <button
              type="button"
              className="max-button"
              onClick={() => {
                if (amountInputMode === 'usd' && bsvPrice > 0) {
                  const maxUsd = convertSatoshisToUsd(balance, bsvPrice);
                  handleInputChange('amount', maxUsd.toFixed(2));
                } else {
                  handleInputChange('amount', formatBalance(balance));
                }
              }}
              disabled={isSubmitting || isLoading || (amountInputMode === 'usd' && bsvPrice === 0)}
            >
              MAX
            </button>
          </div>
          {amountInputMode === 'usd' && bsvPrice > 0 && formData.amount && !isNaN(parseFloat(formData.amount)) && parseFloat(formData.amount) > 0 && (
            <div className="amount-conversion-hint">
              ≈ {convertUsdToSatoshis(parseFloat(formData.amount), bsvPrice).toLocaleString()} satoshis
              {' '}({(convertUsdToSatoshis(parseFloat(formData.amount), bsvPrice) / 100000000).toFixed(8)} BSV)
            </div>
          )}
          {amountInputMode === 'bsv' && formData.amount && !isNaN(parseFloat(formData.amount)) && parseFloat(formData.amount) > 0 && bsvPrice > 0 && (
            <div className="amount-conversion-hint">
              ≈ ${convertSatoshisToUsd(parseFloat(formData.amount) * 100000000, bsvPrice).toFixed(2)} USD
            </div>
          )}
          {errors.amount && <span className="field-error">{errors.amount}</span>}
        </div>

        <div className="form-group">
          <label htmlFor="memo">Memo (Optional)</label>
          <input
            id="memo"
            type="text"
            value={formData.memo}
            onChange={(e) => handleInputChange('memo', e.target.value)}
            placeholder="Add a note to this transaction"
            disabled={isSubmitting || isLoading}
          />
        </div>

        <button
          type="submit"
          className="submit-button"
          disabled={isSubmitting || isLoading || Object.keys(errors).length > 0}
        >
          {isSubmitting ? 'Creating Transaction...' : 'Send Transaction'}
        </button>
      </form>
    </div>
  );
};
