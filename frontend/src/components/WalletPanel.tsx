import { useState, useEffect, useRef } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { TransactionForm } from './TransactionForm';
import { useBalance } from '../hooks/useBalance';
import { useAddress } from '../hooks/useAddress';
import type { TransactionResponse } from '../types/transaction';
import { HodosButton } from './HodosButton';
import './TransactionComponents.css';
import './WalletPanel.css';

interface SyncStatusData {
  active: boolean;
  phase: string;
  addresses_scanned: number;
  utxos_found: number;
  total_satoshis: number;
  result_seen: boolean;
  error: string | null;
}

interface WalletPanelProps {
  onClose?: () => void;
}

export default function WalletPanel({ onClose }: WalletPanelProps) {
  const { balance, usdValue, bsvPrice, isLoading, isRefreshing, refreshBalance } = useBalance();
  const { currentAddress, isGenerating, generateAndCopy } = useAddress();

  const [showSendForm, setShowSendForm] = useState(false);
  const [transactionResult, setTransactionResult] = useState<TransactionResponse | null>(null);
  const [showReceiveAddress, setShowReceiveAddress] = useState(false);
  const [addressCopiedMessage, setAddressCopiedMessage] = useState<string | null>(null);

  const [copyAgainClicked, setCopyAgainClicked] = useState(false);
  const [copyLinkClicked, setCopyLinkClicked] = useState(false);

  // Identity key state — read from localStorage (cached by MainBrowserView at startup)
  const [identityKey] = useState<string | null>(
    () => localStorage.getItem('hodos_identity_key')
  );
  const [showIdentityKey, setShowIdentityKey] = useState(false);
  const [identityKeyCopied, setIdentityKeyCopied] = useState(false);

  // Keep-alive: reset to balance view on hide (so next open is clean)
  useEffect(() => {
    const handleHidden = (e: MessageEvent) => {
      if (e.data?.type === 'wallet_hidden') {
        setShowSendForm(false);
        setShowReceiveAddress(false);
        setTransactionResult(null);
        setAddressCopiedMessage(null);
        setShowIdentityKey(false);
        setIdentityKeyCopied(false);
      }
    };
    // Keep-alive: refresh balance and PeerPay on re-show
    const handleShown = (e: MessageEvent) => {
      if (e.data?.type === 'wallet_shown') {
        const ppc = e.data.ppc || 0;
        const ppa = e.data.ppa || 0;
        setPeerpayNotification(ppc > 0 ? { count: ppc, amount: ppa } : null);
        refreshBalance();
      }
    };
    window.addEventListener('message', handleHidden);
    window.addEventListener('message', handleShown);
    return () => {
      window.removeEventListener('message', handleHidden);
      window.removeEventListener('message', handleShown);
    };
  }, [refreshBalance]);

  // PeerPay notification state (auto-accept only)
  // Read from URL params for instant display (passed by header via C++)
  const [peerpayNotification, setPeerpayNotification] = useState<{
    count: number;
    amount: number;
  } | null>(() => {
    const params = new URLSearchParams(window.location.search);
    const ppc = parseInt(params.get('ppc') || '0', 10);
    const ppa = parseInt(params.get('ppa') || '0', 10);
    if (ppc > 0) return { count: ppc, amount: ppa };
    return null;
  });

  // Failure notification state (red banner)
  const [failureNotification, setFailureNotification] = useState<{ count: number; amount: number } | null>(null);

  // Self-poll peerpay status every 10s while panel is visible (live updates)
  useEffect(() => {
    const pollStatus = () => {
      fetch('http://127.0.0.1:31301/wallet/peerpay/status')
        .then(r => r.json())
        .then((data: { receive_count?: number; receive_amount?: number;
                       failure_count?: number; failure_amount?: number }) => {
          const rc = data.receive_count || 0;
          const ra = data.receive_amount || 0;
          const fc = data.failure_count || 0;
          const fa = data.failure_amount || 0;
          setPeerpayNotification(rc > 0 ? { count: rc, amount: ra } : null);
          setFailureNotification(fc > 0 ? { count: fc, amount: fa } : null);
        })
        .catch(() => {});
    };
    const interval = setInterval(pollStatus, 10000);
    return () => clearInterval(interval);
  }, []);

  const handleDismissPeerpay = () => {
    setPeerpayNotification(null);
    setFailureNotification(null);
    fetch('http://127.0.0.1:31301/wallet/peerpay/dismiss', { method: 'POST' }).catch(() => {});
    // Notify header to clear the dot
    if (window.cefMessage?.send) {
      window.cefMessage.send('wallet_payment_dismissed', []);
    }
  };

  const handleCopyIdentityKey = async () => {
    if (!identityKey) return;
    try {
      await navigator.clipboard.writeText(identityKey);
      setIdentityKeyCopied(true);
      setTimeout(() => setIdentityKeyCopied(false), 2000);
    } catch {
      // Fallback: show the key so user can manually copy
      setShowIdentityKey(true);
    }
  };

  // Sync status state — seed from localStorage so banner shows instantly on reopen
  const [syncStatus, setSyncStatus] = useState<SyncStatusData | null>(() => {
    if (localStorage.getItem('hodos_sync_active') === 'true') {
      return { active: true, phase: 'scanning', addresses_scanned: 0, utxos_found: 0, total_satoshis: 0, result_seen: false, error: null };
    }
    return null;
  });
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Fetch sync status on mount (deferred to let overlay become interactive first)
  useEffect(() => {
    const fetchStatus = () => {
      fetch('http://127.0.0.1:31301/wallet/sync-status')
        .then(r => r.json())
        .then((data: SyncStatusData) => {
          setSyncStatus(data);
          // Keep localStorage flag in sync
          if (data.active) {
            localStorage.setItem('hodos_sync_active', 'true');
          } else {
            localStorage.removeItem('hodos_sync_active');
          }
        })
        .catch(() => {});
    };
    const timer = setTimeout(fetchStatus, 1500);

    return () => {
      clearTimeout(timer);
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, []);

  // Poll every 3s while sync is active
  useEffect(() => {
    if (syncStatus?.active) {
      if (!pollRef.current) {
        pollRef.current = setInterval(() => {
          fetch('http://127.0.0.1:31301/wallet/sync-status')
            .then(r => r.json())
            .then((data: SyncStatusData) => {
              setSyncStatus(data);
              // When sync completes, refresh balance, auto-dismiss, and stop polling
              if (!data.active) {
                localStorage.removeItem('hodos_sync_active');
                refreshBalance();
                if (!data.error) {
                  // Auto-dismiss successful sync (no "Continue to wallet" needed)
                  fetch('http://127.0.0.1:31301/wallet/sync-status/seen', { method: 'POST' }).catch(() => {});
                }
                if (pollRef.current) {
                  clearInterval(pollRef.current);
                  pollRef.current = null;
                }
              }
            })
            .catch(() => {});
        }, 3000);
      }
    } else {
      if (pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
    }
  }, [syncStatus?.active]);

  const handleDismissSyncSummary = () => {
    fetch('http://127.0.0.1:31301/wallet/sync-status/seen', { method: 'POST' })
      .then(() => setSyncStatus(prev => prev ? { ...prev, result_seen: true } : null))
      .catch(() => {});
  };

  const handleReceiveBrc100 = async () => {
    // Clear other display states
    setShowSendForm(false);
    setShowReceiveAddress(false);
    setTransactionResult(null);
    setAddressCopiedMessage(null);

    // Toggle — if already showing, hide
    if (showIdentityKey) {
      setShowIdentityKey(false);
      return;
    }

    // Show the identity key and auto-copy
    setShowIdentityKey(true);
    if (identityKey) {
      try {
        await navigator.clipboard.writeText(identityKey);
        setIdentityKeyCopied(true);
        setTimeout(() => setIdentityKeyCopied(false), 3000);
      } catch {
        // Fallback — key is shown, user can manually copy
      }
    }
  };

  const handleSendClick = () => {
    // Clear all other display states first
    setShowReceiveAddress(false);
    setAddressCopiedMessage(null);
    setTransactionResult(null);
    setShowIdentityKey(false);
    setIdentityKeyCopied(false);

    // Toggle send form
    setShowSendForm(!showSendForm);
  };

  const handleReceiveClick = async () => {
    console.log('Receive button clicked');

    // Clear all other display states first
    setShowSendForm(false);
    setTransactionResult(null);
    setShowIdentityKey(false);
    setIdentityKeyCopied(false);

    try {
      // Generate address from identity
      const addressData = await generateAndCopy();
      console.log('Address generated and copied:', addressData);

      setShowReceiveAddress(true);
      setAddressCopiedMessage(`Address copied to clipboard: ${addressData.substring(0, 10)}...`);

      // Clear the message after 3 seconds
      setTimeout(() => {
        setAddressCopiedMessage(null);
      }, 3000);
    } catch (error) {
      console.error('Failed to generate address:', error);
      setAddressCopiedMessage(`Error: ${error instanceof Error ? error.message : 'Unknown error'}`);
    } finally {
      // Operation complete
    }
  };

  const handleSendSubmit = (result: TransactionResponse) => {
    // Clear all other states first
    setShowReceiveAddress(false);
    setAddressCopiedMessage(null);
    setTransactionResult(null);

    // Set the transaction result and close the form
    setTransactionResult(result);
    setShowSendForm(false);

    // Only refresh balance if transaction was successful
    if (result.success !== false && result.status !== 'failed') {
      refreshBalance();
    }
  };

  const handleCopyAgain = async () => {
    try {
      await navigator.clipboard.writeText(currentAddress || '');
      setCopyAgainClicked(true);
      setTimeout(() => setCopyAgainClicked(false), 2000);
    } catch (error) {
      console.error('Failed to copy address:', error);
    }
  };

  const handleCopyLink = async () => {
    if (transactionResult?.whatsOnChainUrl) {
      try {
        await navigator.clipboard.writeText(transactionResult.whatsOnChainUrl);
        setCopyLinkClicked(true);
        setTimeout(() => setCopyLinkClicked(false), 2000);
      } catch (error) {
        console.error('Failed to copy link:', error);
      }
    }
  };

  const handleAdvanced = () => {
    console.log('Advanced button clicked - opening wallet page in new tab');
    if (window.cefMessage) {
      window.cefMessage.send('tab_create', 'http://127.0.0.1:5137/wallet');
    }
    onClose?.();
  };

  const handleManageSites = () => {
    console.log('Manage Sites clicked - opening wallet page with Approved Sites tab');
    if (window.cefMessage) {
      window.cefMessage.send('tab_create', 'http://127.0.0.1:5137/wallet?tab=4');
    }
    onClose?.();
  };

  return (
    <div className="wallet-panel-light" onClick={(e) => e.stopPropagation()}>
      {/* Balance Display */}
      <div className="balance-display-light">
        <div className="balance-header-light">
          <div className="balance-brand">
            <svg viewBox="0 0 166.81 54.01" xmlns="http://www.w3.org/2000/svg" xmlnsXlink="http://www.w3.org/1999/xlink" style={{ height: 36, width: 'auto' }}>
              <defs>
                <linearGradient id="wl_lg" x1="32.82" y1="13.97" x2="18.73" y2="10.74" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
                <linearGradient id="wl_lg1" x1="40.33" y1="21.9" x2="32.65" y2="9.65" xlinkHref="#wl_lg"/>
                <linearGradient id="wl_lg2" x1="40.03" y1="32.82" x2="43.26" y2="18.73" xlinkHref="#wl_lg"/>
                <linearGradient id="wl_lg3" x1="32.1" y1="40.33" x2="44.35" y2="32.65" xlinkHref="#wl_lg"/>
                <linearGradient id="wl_lg4" x1="21.18" y1="40.03" x2="35.27" y2="43.26" xlinkHref="#wl_lg"/>
                <linearGradient id="wl_lg5" x1="13.67" y1="32.1" x2="21.35" y2="44.35" xlinkHref="#wl_lg"/>
                <linearGradient id="wl_lg6" x1="13.97" y1="21.18" x2="10.74" y2="35.27" xlinkHref="#wl_lg"/>
                <linearGradient id="wl_lg7" x1="21.9" y1="13.66" x2="9.65" y2="21.35" xlinkHref="#wl_lg"/>
              </defs>
              <g>
                <g>
                  <path fill="#dfbd69" d="m162.87,46.03v-8.928h3.728v-1.152h-8.768v1.152h3.664v8.928z"/>
                  <path fill="#dfbd69" d="m147.69,44.88h-5.84v-3.472h4.528v-1.136h-4.528v-3.168h5.696v-1.152h-7.04v10.08h7.184z"/>
                  <path fill="#dfbd69" d="m122.94,35.95v10.08h7.008v-1.152h-5.616v-8.928z"/>
                  <path fill="#dfbd69" d="m105.36,35.95v10.08h7.008v-1.152h-5.616v-8.928z"/>
                  <path fill="#dfbd69" d="m89.29,35.95l-4.112,10.08h1.36l1.184-2.96h4.592l1.216,2.96h1.472l-4.192-10.08zm0.672,1.536h0.064l1.84,4.48h-3.712z"/>
                  <path fill="#dfbd69" d="m70.37,35.95h-1.296l-2.192,8.352h-0.064l-2.112-8.352h-1.344l2.576,10.08h1.6l2.144-8.048h0.064l2.096,8.048h1.616l2.576-10.08h-1.296l-2.064,8.384h-0.064z"/>
                </g>
                <g>
                  <path fill="#a67c00" d="M63.17,27.98V8.18h4.78v7.83h8.64V8.18h4.78v19.8h-4.78v-8.11h-8.64v8.11z"/>
                  <path fill="#a67c00" d="m94.39,28.36c-5.75,0-10.15-4.18-10.15-10.28s4.4-10.28,10.15-10.28,10.18,4.18,10.18,10.28-4.4,10.28-10.18,10.28zm0-16.38c-3.21,0-5.25,2.42-5.25,6.1s2.04,6.1,5.25,6.1,5.28-2.42,5.28-6.1-2.04-6.1-5.28-6.1z"/>
                  <path fill="#a67c00" d="m107.44,8.18h7.67c6.51,0,10.53,3.71,10.53,9.9s-4.02,9.9-10.53,9.9h-7.67zm7.35,16.03c3.74,0,5.91-2.26,5.91-6.16s-2.17-6.1-5.91-6.1h-2.58v12.26z"/>
                  <path fill="#a67c00" d="m137.7,28.36c-5.75,0-10.15-4.18-10.15-10.28s4.4-10.28,10.15-10.28,10.18,4.18,10.18,10.28-4.4,10.28-10.18,10.28zm0-16.38c-3.21,0-5.25,2.42-5.25,6.1s2.04,6.1,5.25,6.1,5.28-2.42,5.28-6.1-2.04-6.1-5.28-6.1z"/>
                  <path fill="#a67c00" d="m154.04,20.78c0.19,2.74,2.23,3.83,4.68,3.83,2.11,0,3.43-0.82,3.43-2.14s-1.16-1.63-2.89-1.98l-3.71-0.66c-3.14-0.6-5.38-2.36-5.38-5.72,0-3.9,3.05-6.32,7.86-6.32,5.38,0,8.3,2.67,8.39,7.07l-4.4,0.13c-0.13-2.33-1.73-3.46-4.02-3.46-2.01,0-3.14,0.82-3.14,2.17,0,1.13,0.88,1.54,2.33,1.82l3.71,0.66c4.05,0.72,5.91,2.73,5.91,5.97,0,4.09-3.55,6.19-8.08,6.19-5.28,0-9.05-2.61-9.05-7.42l4.37-0.16z"/>
                </g>
              </g>
              <g>
                <path fill="url(#wl_lg)" d="M17.56,23.03c1.02-2.43,2.97-4.46,5.58-5.51,3.22-4.22,7.09-6.68,10.73-8.1C31.49,3.48,26.62,0,26.62,0c0,0-4.46,3.47-7.2,9.71-1.57,3.57-2.57,8.05-1.86,13.32Z"/>
                <path fill="url(#wl_lg1)" d="M23.14,17.51c0.15-0.06,0.3-0.13,0.46-0.19,2.5-0.88,5.1-0.72,7.37,0.24,5.26-0.71,9.75,0.29,13.32,1.86,2.52-5.88,1.54-11.78,1.54-11.78,0,0-5.6-0.7-11.96,1.78-3.63,1.42-7.51,3.88-10.73,8.1Z"/>
                <path fill="url(#wl_lg2)" d="M54,26.62s-3.47-4.46-9.71-7.2c-3.57-1.57-8.06-2.57-13.32-1.86,2.43,1.02,4.45,2.97,5.51,5.57,4.22,3.22,6.69,7.1,8.1,10.73,5.94-2.38,9.42-7.24,9.42-7.24Z"/>
                <path fill="url(#wl_lg3)" d="M36.48,23.14c0.06,0.16,0.13,0.31,0.19,0.47,0.85,2.42,0.76,5.02-0.24,7.37,0.71,5.26-0.29,9.74-1.86,13.31,5.88,2.52,11.78,1.54,11.78,1.54,0,0,0.7-5.6-1.78-11.96-1.42-3.63-3.88-7.51-8.1-10.73Z"/>
                <path fill="url(#wl_lg4)" d="M36.44,30.98c-0.07,0.15-0.12,0.31-0.2,0.46-1.11,2.32-3.02,4.09-5.38,5.05-3.22,4.22-7.09,6.68-10.73,8.1,2.38,5.94,7.24,9.42,7.24,9.42,0,0,4.46-3.47,7.2-9.71,1.57-3.57,2.57-8.05,1.86-13.31Z"/>
                <path fill="url(#wl_lg5)" d="M30.86,36.49c-0.16,0.06-0.31,0.13-0.47,0.19-1.12,0.39-2.26,0.58-3.39,0.58-1.39,0-2.74-0.29-3.99-0.82-5.26,0.71-9.74-0.29-13.31-1.86-2.52,5.88-1.54,11.78-1.54,11.78,0,0,5.6,0.7,11.96-1.78,3.63-1.42,7.51-3.88,10.73-8.1Z"/>
                <path fill="url(#wl_lg6)" d="M23.02,36.44c-2.43-1.03-4.46-2.98-5.51-5.58-4.22-3.22-6.67-7.09-8.09-10.72C3.48,22.52,0,27.38,0,27.38c0,0,3.47,4.46,9.71,7.2,3.57,1.57,8.05,2.57,13.31,1.86Z"/>
                <path fill="url(#wl_lg7)" d="M17.5,30.85c-0.06-0.15-0.13-0.3-0.18-0.46-0.88-2.5-0.72-5.1,0.24-7.37-0.71-5.26,0.29-9.74,1.86-13.32-5.88-2.52-11.78-1.54-11.78-1.54,0,0-0.7,5.6,1.78,11.96,1.42,3.63,3.87,7.5,8.09,10.72Z"/>
                <path fill="#a57d2d" d="M23.6,17.33c-0.16,0.06-0.31,0.13-0.46,0.19-2.6,1.06-4.55,3.08-5.58,5.51-0.95,2.27-1.12,4.87-0.24,7.37,0.05,0.16,0.12,0.31,0.18,0.46,1.06,2.6,3.08,4.56,5.51,5.58,1.25,0.53,2.61,0.82,3.99,0.82,1.12,0,2.27-0.19,3.39-0.58,0.16-0.06,0.31-0.13,0.47-0.19,2.37-0.96,4.27-2.73,5.38-5.05,0.07-0.15,0.13-0.31,0.2-0.46,0.99-2.35,1.09-4.95,0.24-7.37-0.06-0.16-0.13-0.31-0.19-0.47-1.06-2.6-3.08-4.55-5.51-5.57-2.26-0.95-4.87-1.12-7.37-0.24zM35.42,24.04c1.63,4.65-0.81,9.75-5.47,11.38-4.65,1.63-9.75-0.81-11.38-5.47-1.63-4.65,0.81-9.75,5.47-11.38,4.65-1.63,9.75,0.81,11.38,5.47z"/>
              </g>
            </svg>
          </div>
          <HodosButton
            variant="ghost"
            size="small"
            className="refresh-button-light"
            onClick={refreshBalance}
            disabled={isRefreshing}
            title="Refresh balance"
          >
            {isRefreshing ? 'Refreshing...' : 'Refresh'}
          </HodosButton>
        </div>
        <div className="balance-content-light">
          <div className="balance-primary-light">
            <span className="balance-amount-light">
              {isLoading ? '...' : `$${usdValue.toFixed(2)}`}
            </span>
            <span className="balance-currency-light">USD</span>
          </div>
          <div className="balance-secondary-light">
            <span className="balance-usd-light">
              {isLoading ? '...' : (balance / 100000000).toFixed(8)} BSV
            </span>
            {bsvPrice > 0 && (
              <span className="balance-rate-light">
                1 BSV = ${bsvPrice.toFixed(2)} USD
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Sync Status Banner */}
      {syncStatus?.active && (
        <div className="sync-banner-light sync-active">
          <div className="sync-banner-header">
            <div className="sync-spinner" />
            <strong>Syncing with blockchain...</strong>
          </div>
          <p className="sync-banner-detail">
            {syncStatus.addresses_scanned > 0 || syncStatus.utxos_found > 0
              ? `Found ${syncStatus.addresses_scanned} addresses, ${syncStatus.utxos_found} UTXOs so far...`
              : 'Scanning addresses...'}
          </p>
          <p className="sync-banner-hint">You can close this panel and browse — sync continues in the background.</p>
        </div>
      )}

      {syncStatus && !syncStatus.active && !syncStatus.result_seen && syncStatus.error && (
        <div className="sync-banner-light sync-error">
          <strong>Sync completed with error</strong>
          <p className="sync-banner-detail">{syncStatus.error}</p>
          <HodosButton variant="secondary" size="small" className="sync-dismiss-button" onClick={handleDismissSyncSummary}>
            Dismiss
          </HodosButton>
        </div>
      )}

      {/* PeerPay Notification Banner (auto-accept only) */}
      {peerpayNotification && peerpayNotification.count > 0 && (
        <div className="peerpay-banner-light">
          <div className="peerpay-banner-content">
            <span>
              Received {peerpayNotification.count} payment{peerpayNotification.count > 1 ? 's' : ''}:{' '}
              {(peerpayNotification.amount / 100_000_000).toFixed(8)} BSV
              {bsvPrice > 0 && ` (~$${((peerpayNotification.amount / 100_000_000) * bsvPrice).toFixed(2)})`}
            </span>
          </div>
          <div className="peerpay-banner-actions">
            <HodosButton
              variant="secondary"
              size="small"
              className="peerpay-details-button"
              onClick={() => {
                handleDismissPeerpay();
                if (window.cefMessage?.send) {
                  window.cefMessage.send('tab_create', 'http://127.0.0.1:5137/wallet?tab=1');
                }
                onClose?.();
              }}
            >
              Details
            </HodosButton>
            <HodosButton variant="ghost" size="small" className="peerpay-dismiss-button" onClick={handleDismissPeerpay}>
              Dismiss
            </HodosButton>
          </div>
        </div>
      )}

      {/* Failed Payment Banner (red) */}
      {failureNotification && failureNotification.count > 0 && (
        <div className="peerpay-banner-light peerpay-banner-failure">
          <div className="peerpay-banner-content">
            <span>
              {failureNotification.count} payment{failureNotification.count > 1 ? 's' : ''} failed to confirm:{' '}
              {(failureNotification.amount / 100_000_000).toFixed(8)} BSV
            </span>
          </div>
          <div className="peerpay-banner-actions">
            <HodosButton variant="ghost" size="small" className="peerpay-dismiss-button" onClick={handleDismissPeerpay}>
              Dismiss
            </HodosButton>
          </div>
        </div>
      )}

      {/* Action Buttons — two Receive side-by-side, full-width Send below */}
      <div className="wallet-actions-light">
        <div className="wallet-actions-row">
          <HodosButton
            variant="secondary"
            className="wallet-button-light"
            onClick={handleReceiveClick}
            disabled={isGenerating}
          >
            {isGenerating ? 'Generating...' : 'Receive Legacy'}
          </HodosButton>
          <HodosButton
            variant="secondary"
            className="wallet-button-light"
            onClick={handleReceiveBrc100}
          >
            Receive BRC-100
          </HodosButton>
        </div>
        <HodosButton
          variant="primary"
          className="wallet-button-light"
          onClick={handleSendClick}
        >
          {showSendForm ? 'Close' : 'Send'}
        </HodosButton>
      </div>

      {/* BRC-100 Identity Key Display (expanded) */}
      {showIdentityKey && identityKey && (
        <div className="identity-key-display-light">
          <p style={{ color: '#b0b7c3', fontSize: '13px', margin: '0 0 8px' }}>
            {identityKeyCopied ? 'Identity key copied to clipboard!' : 'Your BRC-100 identity key:'}
          </p>
          <code>{identityKey}</code>
          <div className="identity-key-qr-light">
            <QRCodeSVG
              value={identityKey}
              size={128}
              level="M"
              bgColor="#ffffff"
              fgColor="#000000"
            />
          </div>
          <div style={{ display: 'flex', gap: '8px', marginTop: '4px' }}>
            <HodosButton
              variant="secondary"
              size="small"
              onClick={handleCopyIdentityKey}
            >
              {identityKeyCopied ? 'Copied!' : 'Copy Again'}
            </HodosButton>
            <HodosButton
              variant="ghost"
              size="small"
              onClick={() => setShowIdentityKey(false)}
            >
              Close
            </HodosButton>
          </div>
        </div>
      )}

      {/* Dynamic Content Area */}
      <div className="dynamic-content-area-light">
        {showSendForm && (
          <>
            <TransactionForm
              onTransactionCreated={handleSendSubmit}
              balance={balance}
              bsvPrice={bsvPrice}
            />
            <button
              className="submit-button"
              onClick={handleSendClick}
              style={{ margin: '10px 14px 14px', width: 'calc(100% - 28px)' }}
            >
              Close
            </button>
          </>
        )}

        {showReceiveAddress && (
          <div className="receive-address-container-light">
            <h3>Receive Bitcoin SV</h3>
            <p>Address copied to clipboard!</p>
            {currentAddress && (
              <div className="qr-code-container-light">
                <QRCodeSVG
                  value={currentAddress}
                  size={160}
                  level="M"
                  marginSize={4}
                />
              </div>
            )}
            <div className="address-display-light">
              <code>{currentAddress || 'Generating...'}</code>
            </div>
            <div className="address-buttons-light">
              <HodosButton
                variant="secondary"
                size="small"
                className={`copy-button-light ${copyAgainClicked ? 'clicked' : ''}`}
                onClick={handleCopyAgain}
              >
                {copyAgainClicked ? 'Copied!' : 'Copy Again'}
              </HodosButton>
              <HodosButton
                variant="ghost"
                size="small"
                className="close-button-light"
                onClick={() => setShowReceiveAddress(false)}
              >
                Close
              </HodosButton>
            </div>
          </div>
        )}

        {/* Success/Error Modal */}
        {transactionResult && (
          <div className={transactionResult.success === false || transactionResult.status === 'failed' ? 'error-message-light' : 'success-message-light'}>
            {transactionResult.success === false || transactionResult.status === 'failed' ? (
              <>
                <h3>Transaction Failed</h3>
                <div className="transaction-details-light">
                  <p><strong>Error:</strong> {transactionResult.error || transactionResult.message || 'Transaction broadcast failed'}</p>
                  {transactionResult.txid && (
                    <p><strong>TxID:</strong> {transactionResult.txid}</p>
                  )}
                </div>
                <HodosButton variant="ghost" size="small" className="close-button-light" onClick={() => setTransactionResult(null)}>
                  Close
                </HodosButton>
              </>
            ) : (
              <>
                <h3>Transaction Sent!</h3>
                <div className="transaction-details-light">
                  {transactionResult.txid && (
                    <p><strong>TxID:</strong> <span className="txid-display">{transactionResult.txid.substring(0, 16)}...</span></p>
                  )}
                  {transactionResult.message && (
                    <p><strong>Status:</strong> {transactionResult.message}</p>
                  )}
                </div>
                {transactionResult.whatsOnChainUrl && (
                  <div className="whatsonchain-container-light">
                    <a
                      href={transactionResult.whatsOnChainUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="whatsonchain-link-light"
                      onClick={() => onClose?.()}
                    >
                      View on WhatsOnChain
                    </a>
                    <HodosButton
                      variant="secondary"
                      size="small"
                      className={`copy-link-button-light ${copyLinkClicked ? 'clicked' : ''}`}
                      onClick={handleCopyLink}
                    >
                      {copyLinkClicked ? 'Copied!' : 'Copy Link'}
                    </HodosButton>
                  </div>
                )}
                <HodosButton variant="ghost" size="small" className="close-button-light" onClick={() => setTransactionResult(null)}>
                  Close
                </HodosButton>
              </>
            )}
          </div>
        )}

        {!showSendForm && !showReceiveAddress && !transactionResult && (
          <div className="content-placeholder-light">
            {addressCopiedMessage ? (
              <div className="address-copied-message-light">
                {addressCopiedMessage}
              </div>
            ) : (
              <span className="placeholder-text">Click Send or Receive to get started</span>
            )}
          </div>
        )}
      </div>

      {/* Advanced Button */}
      <HodosButton
        variant="ghost"
        size="small"
        className="advanced-button-light"
        onClick={handleAdvanced}
      >
        Advanced
      </HodosButton>
      <HodosButton
        variant="ghost"
        size="small"
        className="manage-sites-link-light"
        onClick={handleManageSites}
      >
        Manage approved sites
      </HodosButton>
    </div>
  );
}
