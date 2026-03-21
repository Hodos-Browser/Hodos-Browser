import React, { useState, useCallback, useMemo, useEffect } from 'react';
import { useTransaction } from '../hooks/useTransaction';
import type { TransactionData, TransactionResponse } from '../types/transaction';

// Identity key: 66-char hex starting with 02 or 03 (compressed public key)
const IDENTITY_KEY_REGEX = /^(02|03)[0-9a-fA-F]{64}$/;
// Legacy BSV address: starts with 1 or 3
const BSV_ADDRESS_REGEX = /^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/;
// Paymail: $handle or user@domain.tld
const PAYMAIL_REGEX = /^(\$[a-zA-Z0-9_]+|[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,})$/;

interface TransactionFormProps {
  onTransactionCreated: (result: TransactionResponse) => void;
  balance: number;
  bsvPrice: number;
  isLoading?: boolean;
  error?: string | null;
}

export const TransactionForm: React.FC<TransactionFormProps> = ({
  onTransactionCreated,
  balance,
  bsvPrice,
  isLoading = false,
  error
}) => {
  const { sendTransaction } = useTransaction();
  // Note: feeRate is hardcoded to '5' (medium) for simplicity in the light wallet
  const [formData, setFormData] = useState<TransactionData>({
    recipient: '',
    amount: '',
    feeRate: '5', // Default medium fee rate - not user-configurable in light wallet
    memo: ''
  });
  const [errors, setErrors] = useState<Partial<TransactionData>>({});
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Dual amount inputs — BSV is source of truth, USD mirrors via exchange rate
  const [usdInput, setUsdInput] = useState('');

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

  // Detect whether recipient is an identity key, BSV address, or paymail
  const isPeerPay = useMemo(() => IDENTITY_KEY_REGEX.test(formData.recipient.trim()), [formData.recipient]);
  const isPaymail = useMemo(() => PAYMAIL_REGEX.test(formData.recipient.trim()), [formData.recipient]);

  // Paymail resolution state
  const [paymailInfo, setPaymailInfo] = useState<{ valid: boolean; name?: string; avatar_url?: string; has_p2p?: boolean } | null>(null);
  const [isResolvingPaymail, setIsResolvingPaymail] = useState(false);

  // Debounced paymail resolution
  useEffect(() => {
    if (!isPaymail) {
      setPaymailInfo(null);
      setIsResolvingPaymail(false);
      return;
    }

    setIsResolvingPaymail(true);
    setPaymailInfo(null);

    const timer = setTimeout(async () => {
      try {
        const address = formData.recipient.trim();
        const resp = await fetch(`http://127.0.0.1:31301/wallet/paymail/resolve?address=${encodeURIComponent(address)}`);
        const data = await resp.json();
        setPaymailInfo(data);
      } catch {
        setPaymailInfo({ valid: false });
      } finally {
        setIsResolvingPaymail(false);
      }
    }, 500);

    return () => clearTimeout(timer);
  }, [isPaymail, formData.recipient]);

  const validateForm = useCallback((): boolean => {
    const newErrors: Partial<TransactionData> = {};

    // Validate recipient — accept BSV address OR identity key (PeerPay)
    const trimmed = formData.recipient.trim();
    if (!trimmed) {
      newErrors.recipient = 'Recipient is required';
    } else if (!BSV_ADDRESS_REGEX.test(trimmed) && !IDENTITY_KEY_REGEX.test(trimmed) && !PAYMAIL_REGEX.test(trimmed)) {
      newErrors.recipient = 'Enter a BSV address, identity key, or paymail';
    } else if (PAYMAIL_REGEX.test(trimmed) && paymailInfo && !paymailInfo.valid) {
      newErrors.recipient = 'Could not resolve this paymail address';
    }

    // Validate amount (BSV field is source of truth)
    const amount = parseFloat(formData.amount);
    if (!formData.amount.trim()) {
      newErrors.amount = 'Amount is required';
    } else if (isNaN(amount) || amount <= 0) {
      newErrors.amount = 'Amount must be a positive number';
    } else {
      if (formData.sendMax) {
        if (balance <= 0) {
          newErrors.amount = 'No balance to send';
        }
      } else {
        if (amount < 0.00000546) {
          newErrors.amount = 'Amount must be at least 546 satoshis (0.00000546 BSV)';
        } else if (amount * 100000000 > balance) {
          newErrors.amount = 'Insufficient balance';
        }
      }
    }

    // Fee rate is hardcoded to 5 sat/byte - no validation needed

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  }, [formData, balance, paymailInfo]);

  const handleInputChange = useCallback((field: keyof TransactionData, value: string) => {
    setFormData(prev => {
      const next = { ...prev, [field]: value };
      if (field === 'amount' && prev.sendMax) {
        next.sendMax = false;
      }
      return next;
    });

    if (errors[field]) {
      setErrors(prev => ({ ...prev, [field]: undefined }));
    }
  }, [errors]);

  // Handle BSV amount input — auto-populate USD
  const handleBsvAmountChange = useCallback((value: string) => {
    handleInputChange('amount', value);
    const parsed = parseFloat(value);
    if (!isNaN(parsed) && parsed > 0 && bsvPrice > 0) {
      const usd = convertSatoshisToUsd(parsed * 100000000, bsvPrice);
      setUsdInput(usd.toFixed(2));
    } else {
      setUsdInput('');
    }
  }, [handleInputChange, bsvPrice, convertSatoshisToUsd]);

  // Handle USD amount input — auto-populate BSV
  const handleUsdAmountChange = useCallback((value: string) => {
    setUsdInput(value);
    const parsed = parseFloat(value);
    if (!isNaN(parsed) && parsed > 0 && bsvPrice > 0) {
      const satoshis = convertUsdToSatoshis(parsed, bsvPrice);
      const bsv = (satoshis / 100000000).toFixed(8);
      handleInputChange('amount', bsv);
    } else {
      handleInputChange('amount', '');
    }
  }, [handleInputChange, bsvPrice, convertUsdToSatoshis]);

  const handleSubmit = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();

    if (!validateForm()) {
      console.log('❌ Form validation failed, not submitting');
      return;
    }

    setIsSubmitting(true);
    try {
      console.log('🚀 Form: Starting transaction send with:', formData);

      // BSV field is always source of truth
      const satoshiAmount = Math.round(parseFloat(formData.amount) * 100000000);

      let result: TransactionResponse;

      if (isPaymail) {
        // Paymail: send via bsvalias protocol
        console.log('📝 Sending via Paymail...');
        const resp = await fetch('http://127.0.0.1:31301/wallet/paymail/send', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            paymail: formData.recipient.trim(),
            amount_satoshis: satoshiAmount,
          }),
        });
        result = await resp.json();
      } else if (isPeerPay) {
        // PeerPay: send via identity key through BRC-29 MessageBox
        console.log('📝 Sending via PeerPay to identity key...');
        const resp = await fetch('http://127.0.0.1:31301/wallet/peerpay/send', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            recipient_identity_key: formData.recipient.trim(),
            amount_satoshis: satoshiAmount,
          }),
        });
        result = await resp.json();
      } else {
        // Standard BSV address send
        const transactionData = {
          ...formData,
          amount: (satoshiAmount / 100000000).toFixed(8),
        };
        console.log('📝 Sending transaction...');
        result = await sendTransaction(transactionData);
      }

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
      setUsdInput('');
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
  }, [formData, validateForm, sendTransaction, onTransactionCreated, isPaymail, isPeerPay]);

  const formatBalance = useCallback((satoshis: number): string => {
    const bsv = satoshis / 100000000;
    return bsv.toFixed(8);
  }, []);

  return (
    <div className="transaction-form">
      <div className="form-header">
        <h3>Send Bitcoin SV</h3>
      </div>

      {error && (
        <div className="error-message">
          <span className="error-icon">⚠️</span>
          {error}
        </div>
      )}

      <form onSubmit={handleSubmit} className="transaction-form-content">
        <div className="form-group">
          <label htmlFor="recipient">Recipient</label>
          <input
            id="recipient"
            type="text"
            value={formData.recipient}
            onChange={(e) => handleInputChange('recipient', e.target.value)}
            placeholder="BSV address, identity key, or paymail"
            className={errors.recipient ? 'error' : ''}
            disabled={isSubmitting || isLoading}
            autoComplete="off"
          />
          <span className="field-hint">
            {isPaymail
              ? (isResolvingPaymail
                  ? <><span className="loading-spinner-inline" /> Resolving paymail...</>
                  : paymailInfo?.valid
                    ? <span className="paymail-resolve-row">
                        {paymailInfo.avatar_url && <img src={paymailInfo.avatar_url} className="paymail-avatar" alt="" />}
                        <span>{paymailInfo.name || formData.recipient.trim()}{paymailInfo.has_p2p ? ' (P2P)' : ''}</span>
                      </span>
                    : paymailInfo === null
                      ? 'Verifying paymail...'
                      : 'Paymail not found'
                )
              : isPeerPay
                ? 'Sending via PeerPay (identity key detected)'
                : 'Enter BSV address, identity key, or paymail'
            }
          </span>
          {errors.recipient && <span className="field-error">{errors.recipient}</span>}
        </div>

        <div className="form-group">
          <label>Amount</label>
          <div className="dual-amount-stack">
            <div className="dual-amount-field">
              <label htmlFor="amount-usd" className="dual-amount-label">USD</label>
              <div className="dual-amount-input-row">
                <input
                  id="amount-usd"
                  type="number"
                  step="0.01"
                  min="0.01"
                  value={usdInput}
                  onChange={(e) => handleUsdAmountChange(e.target.value)}
                  placeholder="0.00"
                  className={errors.amount ? 'error' : ''}
                  disabled={isSubmitting || isLoading || bsvPrice <= 0}
                  autoComplete="off"
                />
                <button
                  type="button"
                  className="max-button"
                  onClick={() => {
                    const bsvMax = formatBalance(balance);
                    setFormData(prev => ({ ...prev, amount: bsvMax, sendMax: true }));
                    if (bsvPrice > 0) {
                      setUsdInput(convertSatoshisToUsd(balance, bsvPrice).toFixed(2));
                    }
                    if (errors.amount) {
                      setErrors(prev => ({ ...prev, amount: undefined }));
                    }
                  }}
                  disabled={isSubmitting || isLoading}
                >
                  MAX
                </button>
              </div>
            </div>
            <div className="dual-amount-field">
              <label htmlFor="amount-bsv" className="dual-amount-label">BSV</label>
              <input
                id="amount-bsv"
                type="number"
                step="0.00000001"
                min="0.00000546"
                value={formData.amount}
                onChange={(e) => handleBsvAmountChange(e.target.value)}
                placeholder="0.00000000"
                className={errors.amount ? 'error' : ''}
                disabled={isSubmitting || isLoading}
                autoComplete="off"
              />
            </div>
          </div>
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
            autoComplete="off"
          />
        </div>

        <button
          type="submit"
          className="submit-button"
          disabled={isSubmitting || isLoading || Object.keys(errors).length > 0}
        >
          {isSubmitting ? 'Sending...' : (isPaymail ? 'Send to Paymail' : isPeerPay ? 'Send via PeerPay' : 'Send Transaction')}
        </button>
      </form>
    </div>
  );
};
