import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { useTransaction } from '../hooks/useTransaction';
import type { TransactionData, TransactionResponse } from '../types/transaction';
import { HodosButton } from './HodosButton';

// Identity key: 66-char hex starting with 02 or 03 (compressed public key)
const IDENTITY_KEY_REGEX = /^(02|03)[0-9a-fA-F]{64}$/;
// Legacy BSV address: starts with 1 or 3
const BSV_ADDRESS_REGEX = /^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/;
// Paymail: $handle or user@domain.tld
const PAYMAIL_REGEX = /^(\$[a-zA-Z0-9_]+|[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,})$/;

interface Suggestion {
  value: string;
  display_name: string | null;
  avatar_url: string | null;
  type: string;
  source: string;
  unverified?: boolean;
}

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
  const [formData, setFormData] = useState<TransactionData>({
    recipient: '',
    amount: '',
    feeRate: '5',
    memo: ''
  });
  const [errors, setErrors] = useState<Partial<TransactionData>>({});
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [usdInput, setUsdInput] = useState('');

  // Autocomplete state
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [showDropdown, setShowDropdown] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const [isSuggestLoading, setIsSuggestLoading] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const suggestAbortRef = useRef<AbortController | null>(null);

  const convertUsdToSatoshis = useCallback((usdAmount: number, price: number): number => {
    if (price <= 0) return 0;
    return Math.floor((usdAmount / price) * 100000000);
  }, []);

  const convertSatoshisToUsd = useCallback((satoshis: number, price: number): number => {
    if (price <= 0) return 0;
    return (satoshis / 100000000) * price;
  }, []);

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

  // Debounced autocomplete suggestions
  useEffect(() => {
    const q = formData.recipient.trim();
    if (q.length < 1 || IDENTITY_KEY_REGEX.test(q) || BSV_ADDRESS_REGEX.test(q)) {
      setSuggestions([]);
      setShowDropdown(false);
      return;
    }
    setIsSuggestLoading(true);
    const timer = setTimeout(async () => {
      if (suggestAbortRef.current) suggestAbortRef.current.abort();
      const controller = new AbortController();
      suggestAbortRef.current = controller;
      try {
        const resp = await fetch(
          `http://127.0.0.1:31301/wallet/recipient/suggest?q=${encodeURIComponent(q)}&limit=6`,
          { signal: controller.signal }
        );
        const data = await resp.json();
        if (!controller.signal.aborted) {
          setSuggestions(data.suggestions || []);
          setShowDropdown((data.suggestions || []).length > 0);
          setSelectedIndex(-1);
          setIsSuggestLoading(false);
        }
      } catch (err: unknown) {
        if (err instanceof Error && err.name !== 'AbortError') {
          setSuggestions([]);
          setShowDropdown(false);
          setIsSuggestLoading(false);
        }
      }
    }, 200);
    return () => clearTimeout(timer);
  }, [formData.recipient]);

  // Click-outside to close dropdown
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setShowDropdown(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleSelectSuggestion = useCallback((suggestion: Suggestion) => {
    setFormData(prev => ({ ...prev, recipient: suggestion.value }));
    setShowDropdown(false);
    setSuggestions([]);
  }, []);

  const handleRecipientKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!showDropdown || suggestions.length === 0) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex(prev => (prev < suggestions.length - 1 ? prev + 1 : 0));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex(prev => (prev > 0 ? prev - 1 : suggestions.length - 1));
    } else if (e.key === 'Enter' && selectedIndex >= 0) {
      e.preventDefault();
      handleSelectSuggestion(suggestions[selectedIndex]);
    } else if (e.key === 'Escape') {
      setShowDropdown(false);
    }
  }, [showDropdown, suggestions, selectedIndex, handleSelectSuggestion]);

  const validateForm = useCallback((): boolean => {
    const newErrors: Partial<TransactionData> = {};
    const trimmed = formData.recipient.trim();
    if (!trimmed) {
      newErrors.recipient = 'Recipient is required';
    } else if (!BSV_ADDRESS_REGEX.test(trimmed) && !IDENTITY_KEY_REGEX.test(trimmed) && !PAYMAIL_REGEX.test(trimmed)) {
      newErrors.recipient = 'Enter a BSV address, identity key, or paymail';
    } else if (PAYMAIL_REGEX.test(trimmed) && paymailInfo && !paymailInfo.valid) {
      newErrors.recipient = 'Could not resolve this paymail address';
    }
    const amount = parseFloat(formData.amount);
    if (!formData.amount.trim()) {
      newErrors.amount = 'Amount is required';
    } else if (isNaN(amount) || amount <= 0) {
      newErrors.amount = 'Amount must be a positive number';
    } else {
      if (formData.sendMax) {
        if (balance <= 0) newErrors.amount = 'No balance to send';
      } else {
        if (amount < 0.00000546) newErrors.amount = 'Amount must be at least 546 satoshis (0.00000546 BSV)';
        else if (amount * 100000000 > balance) newErrors.amount = 'Insufficient balance';
      }
    }
    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  }, [formData, balance, paymailInfo]);

  const handleInputChange = useCallback((field: keyof TransactionData, value: string) => {
    setFormData(prev => {
      const next = { ...prev, [field]: value };
      if (field === 'amount' && prev.sendMax) next.sendMax = false;
      return next;
    });
    if (errors[field]) setErrors(prev => ({ ...prev, [field]: undefined }));
  }, [errors]);

  const handleBsvAmountChange = useCallback((value: string) => {
    handleInputChange('amount', value);
    const parsed = parseFloat(value);
    if (!isNaN(parsed) && parsed > 0 && bsvPrice > 0) {
      setUsdInput(convertSatoshisToUsd(parsed * 100000000, bsvPrice).toFixed(2));
    } else {
      setUsdInput('');
    }
  }, [handleInputChange, bsvPrice, convertSatoshisToUsd]);

  const handleUsdAmountChange = useCallback((value: string) => {
    setUsdInput(value);
    const parsed = parseFloat(value);
    if (!isNaN(parsed) && parsed > 0 && bsvPrice > 0) {
      const satoshis = convertUsdToSatoshis(parsed, bsvPrice);
      handleInputChange('amount', (satoshis / 100000000).toFixed(8));
    } else {
      handleInputChange('amount', '');
    }
  }, [handleInputChange, bsvPrice, convertUsdToSatoshis]);

  const handleSubmit = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();
    if (!validateForm()) return;
    setIsSubmitting(true);
    try {
      const satoshiAmount = Math.round(parseFloat(formData.amount) * 100000000);
      let result: TransactionResponse;
      if (isPaymail) {
        const resp = await fetch('http://127.0.0.1:31301/wallet/paymail/send', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ paymail: formData.recipient.trim(), amount_satoshis: satoshiAmount }),
        });
        result = await resp.json();
      } else if (isPeerPay) {
        const resp = await fetch('http://127.0.0.1:31301/wallet/peerpay/send', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ recipient_identity_key: formData.recipient.trim(), amount_satoshis: satoshiAmount }),
        });
        result = await resp.json();
      } else {
        result = await sendTransaction({ ...formData, amount: (satoshiAmount / 100000000).toFixed(8) });
      }
      if (result.success === false || result.status === 'failed') {
        try { onTransactionCreated(result); } catch {}
        return;
      }
      setFormData({ recipient: '', amount: '', feeRate: '5', memo: '' });
      setUsdInput('');
      setErrors({});
      try { onTransactionCreated(result); } catch {}
    } catch (err) {
      console.error('Transaction flow failed:', err);
    } finally {
      setIsSubmitting(false);
    }
  }, [formData, validateForm, sendTransaction, onTransactionCreated, isPaymail, isPeerPay]);

  const formatBalance = useCallback((satoshis: number): string => {
    return (satoshis / 100000000).toFixed(8);
  }, []);

  // Grouped suggestions for section labels
  const groupedSuggestions = useMemo(() => {
    const groups: { label: string; items: (Suggestion & { globalIndex: number })[] }[] = [];
    const recent = suggestions.filter(s => s.source === 'recent');
    const handcash = suggestions.filter(s => s.source === 'handcash');
    const identity = suggestions.filter(s => s.source !== 'recent' && s.source !== 'handcash');
    let idx = 0;
    if (recent.length > 0) groups.push({ label: 'Recent', items: recent.map(s => ({ ...s, globalIndex: idx++ })) });
    if (identity.length > 0) groups.push({ label: 'People', items: identity.map(s => ({ ...s, globalIndex: idx++ })) });
    if (handcash.length > 0) groups.push({ label: 'HandCash', items: handcash.map(s => ({ ...s, globalIndex: idx++ })) });
    return groups;
  }, [suggestions]);

  const avatarIcon = (s: Suggestion) => {
    if (s.source === 'handcash') return '$';
    if (s.source === 'recent') return '\u21BB';
    if (s.type === 'identity') return '\u2688';
    return '@';
  };

  const badgeClass = (s: Suggestion) =>
    s.source === 'recent' ? 'recent' : s.source === 'handcash' ? 'handcash' : 'identity';

  const renderSuggestion = (s: Suggestion & { globalIndex: number }) => {
    const displayName = s.display_name || s.value;
    const showValue = s.display_name && s.display_name !== s.value;
    const cls = badgeClass(s);
    const isUnverified = s.unverified === true || s.source === 'handcash';
    return (
      <div
        key={`${s.value}-${s.source}`}
        className={`recipient-suggestion${s.globalIndex === selectedIndex ? ' selected' : ''}`}
        onMouseDown={(e) => { e.preventDefault(); handleSelectSuggestion(s); }}
        onMouseEnter={() => setSelectedIndex(s.globalIndex)}
      >
        {s.avatar_url ? (
          <img src={s.avatar_url} className="suggestion-avatar" alt="" />
        ) : (
          <div className={`suggestion-avatar-placeholder ${cls}`}>{avatarIcon(s)}</div>
        )}
        <div className="suggestion-info">
          <span className="suggestion-name">{displayName}</span>
          {showValue && <span className="suggestion-value">{s.value}</span>}
          {isUnverified && <span className="suggestion-unverified">Will verify on send</span>}
        </div>
        <span className={`suggestion-badge ${cls}`}>{s.source}</span>
      </div>
    );
  };

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
          <div className={`recipient-autocomplete-wrapper${showDropdown && suggestions.length > 0 ? ' dropdown-open' : ''}`} ref={wrapperRef}>
            <input
              id="recipient"
              type="text"
              value={formData.recipient}
              onChange={(e) => handleInputChange('recipient', e.target.value)}
              onKeyDown={handleRecipientKeyDown}
              onFocus={() => { if (suggestions.length > 0) setShowDropdown(true); }}
              placeholder="BSV address, identity key, or paymail"
              className={errors.recipient ? 'error' : ''}
              disabled={isSubmitting || isLoading}
              autoComplete="off"
            />
            {showDropdown && suggestions.length > 0 && (
              <div className="recipient-dropdown">
                {groupedSuggestions.map((group) => (
                  <div key={group.label}>
                    {groupedSuggestions.length > 1 && (
                      <div className="suggestion-section-label">{group.label}</div>
                    )}
                    {group.items.map((s) => renderSuggestion(s))}
                  </div>
                ))}
                {isSuggestLoading && (
                  <div className="recipient-dropdown-loading">
                    <span className="loading-spinner-inline" /> Searching identities...
                  </div>
                )}
              </div>
            )}
          </div>
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
                />
                <HodosButton
                  type="button"
                  variant="secondary"
                  size="small"
                  className="max-button"
                  onClick={() => {
                    const bsvMax = formatBalance(balance);
                    setFormData(prev => ({ ...prev, amount: bsvMax, sendMax: true }));
                    if (bsvPrice > 0) setUsdInput(convertSatoshisToUsd(balance, bsvPrice).toFixed(2));
                    if (errors.amount) setErrors(prev => ({ ...prev, amount: undefined }));
                  }}
                  disabled={isSubmitting || isLoading}
                >
                  MAX
                </HodosButton>
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
          />
        </div>

        <HodosButton
          type="submit"
          variant="primary"
          className={`submit-button${isSubmitting ? ' submitting' : ''}`}
          loading={isSubmitting}
          loadingText="Sending..."
          disabled={isLoading || Object.keys(errors).length > 0}
        >
          {isPaymail ? 'Send to Paymail' : isPeerPay ? 'Send via PeerPay' : 'Send Transaction'}
        </HodosButton>
      </form>
    </div>
  );
};
