import React, { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { TransactionForm } from '../TransactionForm';
import type { TransactionResponse } from '../../types/transaction';

const InfoTooltip: React.FC<{ text: string }> = ({ text }) => {
  const [open, setOpen] = useState(false);

  return (
    <div
      className="wd-info-tooltip-wrap"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
    >
      <span
        className={`wd-info-icon${open ? ' active' : ''}`}
        title="More info"
      >
        i
      </span>
      {open && (
        <div className="wd-info-popup">
          {text}
        </div>
      )}
    </div>
  );
};

interface ActivityItem {
  txid: string;
  direction: 'sent' | 'received';
  satoshis: number;
  status: string;
  timestamp: string;
  description?: string;
  labels?: string[];
  price_usd_cents?: number | null;
  source: string;
}

interface DashboardTabProps {
  onNavigateToActivity: () => void;
}

const DashboardTab: React.FC<DashboardTabProps> = ({ onNavigateToActivity }) => {
  // Balance state
  const [balance, setBalance] = useState(0);
  const [bsvPrice, setBsvPrice] = useState(0);
  const [balanceLoading, setBalanceLoading] = useState(true);

  // Receive state
  const [currentAddress, setCurrentAddress] = useState('');
  const [addressLoading, setAddressLoading] = useState(true);
  const [copied, setCopied] = useState(false);
  const [generating, setGenerating] = useState(false);

  // Identity key state
  const [identityKey] = useState<string | null>(
    () => localStorage.getItem('hodos_identity_key')
  );
  const [identityKeyCopied, setIdentityKeyCopied] = useState(false);

  // Recent activity state
  const [recentActions, setRecentActions] = useState<ActivityItem[]>([]);
  const [recentLoading, setRecentLoading] = useState(true);
  const [recentCurrentPrice, setRecentCurrentPrice] = useState<number | null>(null);

  // Transaction success/error state
  const [txResult, setTxResult] = useState<{
    type: 'success' | 'error';
    message: string;
    txid: string | null;
    whatsOnChainUrl: string | null;
  } | null>(null);
  const [txidCopied, setTxidCopied] = useState(false);

  // Notification state (incoming payments)
  const [notification, setNotification] = useState<{ count: number; amount: number } | null>(null);
  const [failureNotification, setFailureNotification] = useState<{ count: number; amount: number } | null>(null);
  const notificationRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const prevNotificationCount = useRef(0);

  // Backup state
  const [backingUp, setBackingUp] = useState(false);
  const [backupStatus, setBackupStatus] = useState<'idle' | 'success' | 'error'>('idle');

  // Refs to track current values — avoids re-renders when poll returns same data
  const balanceRef = useRef(0);
  const bsvPriceRef = useRef(0);
  const initialLoadDone = useRef(false);
  const fetchRecentActivityRef = useRef<() => void>(() => {});

  const fetchBalance = useCallback(async (showLoading = false) => {
    try {
      if (showLoading) setBalanceLoading(true);
      const res = await fetch('http://127.0.0.1:31301/wallet/balance');
      if (!res.ok) throw new Error('Failed to fetch balance');
      const data = await res.json();
      const newBalance = data.balance || 0;
      const newPrice = data.bsvPrice || 0;
      // Only update state if values actually changed
      if (newBalance !== balanceRef.current) {
        const balanceChanged = balanceRef.current !== 0; // skip initial load
        balanceRef.current = newBalance;
        setBalance(newBalance);
        // Balance changed — refresh recent activity to show new transactions
        if (balanceChanged) fetchRecentActivityRef.current();
      }
      if (newPrice !== bsvPriceRef.current) {
        bsvPriceRef.current = newPrice;
        setBsvPrice(newPrice);
      }
    } catch (err) {
      console.error('Failed to fetch balance:', err);
    } finally {
      if (showLoading || !initialLoadDone.current) {
        initialLoadDone.current = true;
        setBalanceLoading(false);
      }
    }
  }, []);

  const fetchAddress = useCallback(async () => {
    try {
      setAddressLoading(true);
      const res = await fetch('http://127.0.0.1:31301/wallet/address/current');
      if (!res.ok) throw new Error('Failed to fetch address');
      const data = await res.json();
      setCurrentAddress(data.address || '');
    } catch (err) {
      console.error('Failed to fetch address:', err);
    } finally {
      setAddressLoading(false);
    }
  }, []);

  const fetchRecentActivity = useCallback(async () => {
    try {
      setRecentLoading(true);
      const res = await fetch('http://127.0.0.1:31301/wallet/activity?page=1&limit=5&filter=all');
      if (!res.ok) throw new Error('Failed to fetch activity');
      const data = await res.json();
      setRecentActions(data.items || []);
      setRecentCurrentPrice(data.current_price_usd_cents ?? null);
    } catch (err) {
      console.error('Failed to fetch recent activity:', err);
    } finally {
      setRecentLoading(false);
    }
  }, []);

  // Keep ref in sync so fetchBalance can call it without circular deps
  fetchRecentActivityRef.current = fetchRecentActivity;

  useEffect(() => {
    fetchBalance(true); // show loading spinner on initial load
    fetchAddress();
    fetchRecentActivity();

    // Poll balance every 10s so dashboard stays current (backend cache is 60s TTL,
    // but invalidate() is called on send/receive so fresh data appears quickly).
    // Background polls don't show loading and only update state if values change.
    const balanceInterval = setInterval(() => fetchBalance(false), 10000);
    return () => clearInterval(balanceInterval);
  }, [fetchBalance, fetchAddress, fetchRecentActivity]);

  // Notification polling (incoming payments + failures)
  useEffect(() => {
    const fetchNotification = () => {
      fetch('http://127.0.0.1:31301/wallet/peerpay/status')
        .then(r => r.json())
        .then((data: { unread_count?: number; unread_amount?: number;
                       receive_count?: number; receive_amount?: number;
                       failure_count?: number; failure_amount?: number }) => {
          const receiveCount = data.receive_count || 0;
          const receiveAmount = data.receive_amount || 0;
          const failureCount = data.failure_count || 0;
          const failureAmount = data.failure_amount || 0;
          const totalCount = (data.unread_count || 0);

          if (receiveCount > 0) {
            setNotification({ count: receiveCount, amount: receiveAmount });
          } else {
            setNotification(null);
          }

          if (failureCount > 0) {
            setFailureNotification({ count: failureCount, amount: failureAmount });
          } else {
            setFailureNotification(null);
          }

          // Auto-refresh balance when new notifications appear
          if (totalCount > prevNotificationCount.current) {
            fetchBalance();
            fetchRecentActivity();
          }
          prevNotificationCount.current = totalCount;
        })
        .catch(() => {});
    };

    fetchNotification();
    notificationRef.current = setInterval(fetchNotification, 10000);
    return () => {
      if (notificationRef.current) clearInterval(notificationRef.current);
    };
  }, [fetchBalance, fetchRecentActivity]);

  const handleGenerateAddress = async () => {
    try {
      setGenerating(true);
      const res = await fetch('http://127.0.0.1:31301/wallet/address/generate', { method: 'POST' });
      if (!res.ok) throw new Error('Failed to generate address');
      const data = await res.json();
      setCurrentAddress(data.address || '');
    } catch (err) {
      console.error('Failed to generate address:', err);
    } finally {
      setGenerating(false);
    }
  };

  const handleCopy = () => {
    if (!currentAddress) return;
    navigator.clipboard.writeText(currentAddress).catch(() => {});
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleCopyIdentityKey = () => {
    if (!identityKey) return;
    navigator.clipboard.writeText(identityKey).catch(() => {});
    setIdentityKeyCopied(true);
    setTimeout(() => setIdentityKeyCopied(false), 2000);
  };

  const handleTransactionCreated = useCallback((result: TransactionResponse) => {
    if (result.success === false || result.status === 'failed') {
      setTxResult({ type: 'error', message: result.error || result.message || 'Transaction failed', txid: null, whatsOnChainUrl: null });
    } else {
      setTxResult({
        type: 'success',
        message: result.message || '',
        txid: result.txid || null,
        whatsOnChainUrl: result.whatsOnChainUrl || null,
      });
      // Balance updates via 10s polling interval — no forced refresh needed.
      // Refresh activity list after a short delay for the tx to appear.
      setTimeout(() => fetchRecentActivity(), 2000);
    }
  }, [fetchRecentActivity]);

  const handleCopyTxid = () => {
    if (!txResult?.txid) return;
    navigator.clipboard.writeText(txResult.txid).catch(() => {});
    setTxidCopied(true);
    setTimeout(() => setTxidCopied(false), 2000);
  };

  const handleViewOnChain = (url: string) => {
    if ((window as any).cefMessage?.send) {
      (window as any).cefMessage.send('tab_create', url);
    }
  };

  const handleDismissNotification = () => {
    setNotification(null);
    setFailureNotification(null);
    prevNotificationCount.current = 0;
    fetch('http://127.0.0.1:31301/wallet/peerpay/dismiss', { method: 'POST' }).catch(() => {});
    if ((window as any).cefMessage?.send) {
      (window as any).cefMessage.send('wallet_payment_dismissed', []);
    }
    // Refresh after dismiss
    setTimeout(() => { fetchBalance(); fetchRecentActivity(); }, 500);
  };

  const handleBackupNow = async () => {
    setBackingUp(true);
    setBackupStatus('idle');
    try {
      const resp = await fetch('http://127.0.0.1:31301/wallet/backup/onchain', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: '{}',
      });
      const data = await resp.json();
      if (data.success) {
        setBackupStatus('success');
      } else if (data.error?.includes('skipped')) {
        setBackupStatus('success'); // No changes = already backed up
      } else {
        setBackupStatus('error');
      }
    } catch {
      setBackupStatus('error');
    }
    setBackingUp(false);
    // Reset status after 5 seconds
    setTimeout(() => setBackupStatus('idle'), 5000);
  };

  // Memoize TransactionForm so balance polls don't reset form state.
  // Only re-renders when balance/price/loading actually change.
  const memoizedTransactionForm = useMemo(() => (
    <TransactionForm
      onTransactionCreated={handleTransactionCreated}
      balance={balance}
      bsvPrice={bsvPrice}
      isLoading={balanceLoading}
    />
  ), [handleTransactionCreated, balance, bsvPrice, balanceLoading]);

  const formatBsv = (sats: number): string => (sats / 100000000).toFixed(8);
  const formatUsd = (sats: number, price: number): string => {
    if (price <= 0) return '--';
    return '$' + ((sats / 100000000) * price).toFixed(2);
  };
  const formatUsdCents = (sats: number, priceInCents: number): string => {
    const price = priceInCents / 100;
    if (price <= 0) return '--';
    return '$' + ((sats / 100000000) * price).toFixed(2);
  };

  const formatTime = (dateStr?: string): string => {
    if (!dateStr) return '';
    try {
      const d = new Date(dateStr);
      const now = new Date();
      const diffMs = now.getTime() - d.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      if (diffMins < 1) return 'Just now';
      if (diffMins < 60) return `${diffMins}m ago`;
      const diffHours = Math.floor(diffMins / 60);
      if (diffHours < 24) return `${diffHours}h ago`;
      const diffDays = Math.floor(diffHours / 24);
      if (diffDays < 7) return `${diffDays}d ago`;
      return d.toLocaleDateString();
    } catch {
      return '';
    }
  };

  return (
    <div className="wd-dashboard">
      {/* Top-Left: Balance Card */}
      <div className="wd-quad-tl">
        <div className="wd-balance-card">
          <div className="wd-balance-top-row">
            <div className="wd-balance-label">Total Balance</div>
            <button
              className="wd-balance-refresh"
              onClick={() => fetchBalance(true)}
              disabled={balanceLoading}
            >
              Refresh
            </button>
          </div>
          {balanceLoading ? (
            <div className="wd-loading" style={{ padding: '12px 0' }}>
              <div className="wd-spinner" />
            </div>
          ) : (
            <>
              <div className="wd-balance-usd-primary">{formatUsd(balance, bsvPrice)}</div>
              <div className="wd-balance-secondary-row">
                <span className="wd-balance-bsv">{formatBsv(balance)} BSV</span>
                {bsvPrice > 0 && (
                  <span className="wd-balance-rate">1 BSV = ${bsvPrice.toFixed(2)} USD</span>
                )}
              </div>
            </>
          )}

          {/* Incoming payment notification banner (green) */}
          {notification && notification.count > 0 && (
            <div className="wd-notification-bar">
              <span className="wd-notification-text">
                Received {notification.count} payment{notification.count > 1 ? 's' : ''}:{' '}
                {formatBsv(notification.amount)} BSV
                {bsvPrice > 0 && ` (~${formatUsd(notification.amount, bsvPrice)})`}
              </span>
              <button className="wd-notification-dismiss" onClick={handleDismissNotification}>
                Dismiss
              </button>
            </div>
          )}

          {/* Failed payment notification banner (red) */}
          {failureNotification && failureNotification.count > 0 && (
            <div className="wd-notification-bar wd-notification-failure">
              <span className="wd-notification-text">
                {failureNotification.count} payment{failureNotification.count > 1 ? 's' : ''} failed to confirm:{' '}
                {formatBsv(failureNotification.amount)} BSV
              </span>
              <button className="wd-notification-dismiss" onClick={handleDismissNotification}>
                Dismiss
              </button>
            </div>
          )}

          {/* Manual backup button */}
          <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: '8px' }}>
            <button
              className="wd-backup-btn"
              onClick={handleBackupNow}
              disabled={backingUp}
              style={{
                background: backupStatus === 'success' ? '#4caf50' : backupStatus === 'error' ? '#c62828' : '#a67c00',
                border: 'none',
                color: '#111',
                fontSize: '12px',
                fontWeight: 600,
                cursor: backingUp ? 'default' : 'pointer',
                padding: '4px 12px',
                borderRadius: '4px',
                transition: 'background 0.15s',
                opacity: backingUp ? 0.6 : 1,
              }}
              onMouseEnter={e => { if (!backingUp) (e.target as HTMLElement).style.background = backupStatus === 'success' ? '#66bb6a' : backupStatus === 'error' ? '#e53935' : '#c49b30'; }}
              onMouseLeave={e => { (e.target as HTMLElement).style.background = backupStatus === 'success' ? '#4caf50' : backupStatus === 'error' ? '#c62828' : '#a67c00'; }}
            >
              {backingUp ? 'Backing up...' : backupStatus === 'success' ? 'Backed up' : backupStatus === 'error' ? 'Backup failed' : 'Backup on Chain'}
            </button>
          </div>
        </div>
      </div>

      {/* Top-Right: Receive Section */}
      <div className="wd-quad-tr">
        <div className="wd-receive-card">
          <div className="wd-receive-split">
            {/* Left: Identity Key */}
            <div className="wd-receive-half">
              <div className="wd-receive-header">
                <div className="wd-receive-title-group">
                  <div className="wd-receive-title-row">
                    <span className="wd-receive-title">Identity Key</span>
                    <InfoTooltip text="Your public identity key enables PeerPay, built on the BRC-29 direct payment standard. When someone sends you BSV, a unique one-time address is derived using elliptic curve Diffie-Hellman (ECDH) key exchange. Only your wallet holds the keys to spend those funds. Payments are delivered via end-to-end encrypted messages, keeping your balance and history private." />
                  </div>
                  <span className="wd-receive-subtitle">(Public Key - use with BRC-100 wallets)</span>
                </div>
              </div>
              {identityKey ? (
                <div className="wd-receive-body">
                  <div className="wd-qr-container">
                    <QRCodeSVG
                      value={identityKey}
                      size={96}
                      level="M"
                      bgColor="#ffffff"
                      fgColor="#000000"
                    />
                  </div>
                  <div className="wd-address-info">
                    <div className="wd-address-display">
                      <code>{identityKey}</code>
                    </div>
                    <div className="wd-address-actions">
                      <button
                        className={`wd-copy-btn${identityKeyCopied ? ' copied' : ''}`}
                        onClick={handleCopyIdentityKey}
                      >
                        {identityKeyCopied ? 'Copied!' : 'Copy Key'}
                      </button>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="wd-empty" style={{ padding: '16px 0' }}>
                  <span className="wd-empty-text">No identity key available</span>
                </div>
              )}
            </div>

            {/* Divider */}
            <div className="wd-receive-divider" />

            {/* Right: Legacy Address */}
            <div className="wd-receive-half">
              <div className="wd-receive-header">
                <div className="wd-receive-title-group">
                  <div className="wd-receive-title-row">
                    <span className="wd-receive-title">Receive Address</span>
                    <InfoTooltip text="Hodos derives a unique address for every transaction using elliptic curve cryptography (secp256k1). Each address has its own key pair, all secured by your recovery phrase. Generate a new address each time you share it — this prevents anyone from linking your transactions or viewing your total balance on-chain." />
                  </div>
                  <span className="wd-receive-subtitle">(P2PKH Address - use with Handcash, RockWallet, etc.)</span>
                </div>
                <button
                  className="wd-receive-generate"
                  onClick={handleGenerateAddress}
                  disabled={generating}
                >
                  {generating ? 'Generating...' : 'New Address'}
                </button>
              </div>
              {addressLoading ? (
                <div className="wd-loading" style={{ padding: '16px 0' }}>
                  <div className="wd-spinner" />
                </div>
              ) : currentAddress ? (
                <div className="wd-receive-body">
                  <div className="wd-qr-container">
                    <QRCodeSVG
                      value={currentAddress}
                      size={96}
                      level="M"
                      bgColor="#ffffff"
                      fgColor="#000000"
                    />
                  </div>
                  <div className="wd-address-info">
                    <div className="wd-address-display">
                      <code>{currentAddress}</code>
                    </div>
                    <div className="wd-address-actions">
                      <button
                        className={`wd-copy-btn${copied ? ' copied' : ''}`}
                        onClick={handleCopy}
                      >
                        {copied ? 'Copied!' : 'Copy Address'}
                      </button>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="wd-empty" style={{ padding: '16px 0' }}>
                  <span className="wd-empty-text">No address generated yet</span>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Bottom-Left: Send Form */}
      <div className="wd-quad-bl">
        <div className="wd-send-card">
          {txResult && (
            <div className={`wd-tx-result ${txResult.type}`}>
              <button className="wd-tx-result-dismiss" onClick={() => { setTxResult(null); setTxidCopied(false); }} title="Dismiss">
                &times;
              </button>
              {txResult.type === 'success' ? (
                <>
                  <div className="wd-tx-result-header">Transaction Sent!</div>
                  {txResult.txid && (
                    <div className="wd-tx-result-txid">
                      TxID: {txResult.txid.substring(0, 16)}...
                    </div>
                  )}
                  <div className="wd-tx-result-actions">
                    {txResult.whatsOnChainUrl && (
                      <button
                        className="wd-tx-result-link"
                        onClick={() => handleViewOnChain(txResult.whatsOnChainUrl!)}
                      >
                        View on WhatsOnChain
                      </button>
                    )}
                    {txResult.txid && (
                      <button
                        className={`wd-tx-copy-btn${txidCopied ? ' copied' : ''}`}
                        onClick={handleCopyTxid}
                      >
                        {txidCopied ? 'Copied!' : 'Copy TxID'}
                      </button>
                    )}
                  </div>
                </>
              ) : (
                <>
                  <div className="wd-tx-result-header">Transaction Failed</div>
                  <div className="wd-tx-result-txid">{txResult.message}</div>
                </>
              )}
            </div>
          )}
          {memoizedTransactionForm}
        </div>
      </div>

      {/* Bottom-Right: Recent Activity */}
      <div className="wd-quad-br">
        <div className="wd-recent-card">
          <div className="wd-recent-header">
            <span className="wd-recent-title">Recent Activity</span>
            <button className="wd-recent-view-all" onClick={onNavigateToActivity}>
              View All
            </button>
          </div>

          {recentLoading ? (
            <div className="wd-loading" style={{ padding: '16px 0' }}>
              <div className="wd-spinner" />
            </div>
          ) : recentActions.length === 0 ? (
            <div className="wd-empty" style={{ padding: '16px 0' }}>
              <span className="wd-empty-text">No transactions yet</span>
              <span className="wd-empty-sub">Send or receive BSV to see activity here</span>
            </div>
          ) : (
            <div className="wd-recent-list">
              {recentActions.map((action, idx) => {
                const txPrice = action.price_usd_cents;
                const hasHistorical = txPrice != null && txPrice > 0;
                const usdAtTx = hasHistorical
                  ? formatUsdCents(action.satoshis, txPrice!)
                  : null;
                const usdNow = recentCurrentPrice
                  ? formatUsdCents(action.satoshis, recentCurrentPrice)
                  : null;

                const handleCopyTxid = (txid: string) => {
                  navigator.clipboard.writeText(txid).catch(() => {});
                };
                const handleOpenWoC = (txid: string) => {
                  const cef = (window as any).cefMessage;
                  if (cef?.send) cef.send('tab_create', `https://whatsonchain.com/tx/${txid}`);
                };

                return (
                  <div key={action.txid || idx} className="wd-recent-item">
                    <div className={`wd-recent-direction ${action.direction}`}>
                      {action.direction === 'sent' ? '\u2191' : '\u2193'}
                    </div>
                    <div className="wd-recent-info">
                      <div className="wd-recent-desc">
                        {action.description || (action.direction === 'sent' ? 'Sent' : 'Received')}
                      </div>
                      <div className="wd-recent-meta">
                        <span className="wd-recent-time">{formatTime(action.timestamp)}</span>
                        <span className={`wd-recent-status ${action.status}`}>{action.status}</span>
                      </div>
                    </div>
                    <div className="wd-recent-center">
                      {action.txid && (
                        <>
                          <button
                            className="wd-txid-pill wd-txid-pill-sm"
                            onClick={(e) => { e.stopPropagation(); handleCopyTxid(action.txid); }}
                            title={action.txid}
                          >
                            txid
                          </button>
                          <button
                            className="wd-woc-btn wd-woc-btn-sm"
                            onClick={(e) => { e.stopPropagation(); handleOpenWoC(action.txid); }}
                            title="View on WhatsOnChain"
                          >
                            <img src="/whatsonchain.png" alt="WoC" width="14" height="14" />
                          </button>
                        </>
                      )}
                      <div className="wd-recent-values">
                        <span className={`wd-recent-usd-primary ${action.direction}`}>
                          {action.direction === 'sent' ? '-' : '+'}{usdAtTx || usdNow || '--'}
                        </span>
                        <span className="wd-recent-bsv">
                          {action.direction === 'sent' ? '-' : '+'}{formatBsv(action.satoshis)} BSV
                        </span>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default DashboardTab;
