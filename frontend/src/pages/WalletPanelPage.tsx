import { useMemo, useState, useEffect } from 'react';
import WalletPanel from '../components/WalletPanel';

export default function WalletPanelPage() {
  // Read icon position from URL param (physical pixels, passed from toolbar click)
  const paddingRightPx = useMemo(() => {
    const params = new URLSearchParams(window.location.search);
    const iro = parseInt(params.get('iro') || '0', 10);
    if (iro <= 0) return 0;
    const dpr = window.devicePixelRatio || 1;
    return Math.round(iro / dpr);
  }, []);

  // Skip the loading spinner if we already know a wallet exists (from a previous open)
  const cachedExists = localStorage.getItem('hodos_wallet_exists') === 'true';
  const [walletStatus, setWalletStatus] = useState<'loading' | 'exists' | 'no-wallet'>(
    cachedExists ? 'exists' : 'loading'
  );
  const [creating, setCreating] = useState(false);
  const [mnemonic, setMnemonic] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [backedUp, setBackedUp] = useState(false);

  useEffect(() => {
    // If we already cached that wallet exists, skip the fetch
    if (cachedExists) return;

    fetch('http://localhost:3301/wallet/status')
      .then(r => r.json())
      .then(data => {
        if (data.exists) {
          localStorage.setItem('hodos_wallet_exists', 'true');
          setWalletStatus('exists');
        } else {
          setWalletStatus('no-wallet');
        }
      })
      .catch(() => setWalletStatus('no-wallet'));
  }, []);

  const handleClose = () => {
    if (window.hodosBrowser?.overlay?.close) {
      window.hodosBrowser.overlay.close();
    } else if (window.cefMessage?.send) {
      window.cefMessage.send('overlay_close', []);
    }
  };

  const handleBackgroundClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      handleClose();
    }
  };

  const handleCreateWallet = async () => {
    setCreating(true);
    try {
      const res = await fetch('http://localhost:3301/wallet/create', { method: 'POST' });
      const data = await res.json();
      if (data.success && data.mnemonic) {
        setMnemonic(data.mnemonic);
      } else {
        alert(data.error || 'Failed to create wallet');
        setCreating(false);
      }
    } catch (e) {
      alert('Failed to connect to wallet server');
      setCreating(false);
    }
  };

  const handleCopyMnemonic = () => {
    if (mnemonic) {
      navigator.clipboard.writeText(mnemonic).then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      });
    }
  };

  const handleConfirmBackup = () => {
    localStorage.setItem('hodos_wallet_exists', 'true');
    setMnemonic(null);
    setCreating(false);
    setWalletStatus('exists');
  };

  const renderNoWallet = () => (
    <div style={{
      background: '#d4c4a8',
      borderRadius: '12px',
      width: '380px',
      maxHeight: '80vh',
      overflow: 'auto',
      boxShadow: '0 8px 32px rgba(45, 80, 22, 0.3)',
      cursor: 'default',
      fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    }} onClick={e => e.stopPropagation()}>
      {/* Header */}
      <div style={{
        background: '#2d5016',
        color: '#f5f1e8',
        padding: '20px 24px',
        borderRadius: '12px 12px 0 0',
        borderBottom: '2px solid #d4c4a8',
      }}>
        <h2 style={{ margin: 0, fontSize: '20px', fontWeight: 700 }}>Wallet</h2>
      </div>

      {/* Content */}
      <div style={{ padding: '32px 24px', textAlign: 'center' }}>
        {!mnemonic ? (
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F512;</div>
            <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>No Wallet Found</h3>
            <p style={{ color: '#555', fontSize: '14px', margin: '0 0 24px' }}>
              Create a new wallet to get started with Bitcoin SV.
            </p>

            <button
              onClick={handleCreateWallet}
              disabled={creating}
              style={{
                background: '#2d5016',
                color: '#f5f1e8',
                border: 'none',
                borderRadius: '8px',
                padding: '12px 24px',
                fontSize: '15px',
                fontWeight: 600,
                cursor: creating ? 'not-allowed' : 'pointer',
                width: '100%',
                marginBottom: '12px',
                opacity: creating ? 0.7 : 1,
              }}
            >
              {creating ? 'Creating...' : 'Create New Wallet'}
            </button>

            <button
              disabled
              title="Coming in Phase 1"
              style={{
                background: 'transparent',
                color: '#888',
                border: '2px solid #e8dcc0',
                borderRadius: '8px',
                padding: '12px 24px',
                fontSize: '15px',
                fontWeight: 600,
                cursor: 'not-allowed',
                width: '100%',
              }}
            >
              Recover Wallet (Coming Soon)
            </button>
          </>
        ) : (
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x26A0;&#xFE0F;</div>
            <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>Back Up Your Mnemonic</h3>
            <p style={{ color: '#555', fontSize: '13px', margin: '0 0 16px' }}>
              Write down these 12 words in order. This is the only way to recover your wallet.
              <strong> Never share them with anyone.</strong>
            </p>

            <div style={{
              background: '#f5f1e8',
              border: '2px solid #e8dcc0',
              borderRadius: '8px',
              padding: '16px',
              marginBottom: '16px',
              fontFamily: 'monospace',
              fontSize: '14px',
              lineHeight: '1.8',
              color: '#1a2e0a',
              wordBreak: 'break-word',
              userSelect: 'text',
              textAlign: 'left',
            }}>
              {mnemonic.split(' ').map((word, i) => (
                <span key={i} style={{ display: 'inline-block', marginRight: '4px' }}>
                  <span style={{ color: '#888', fontSize: '11px' }}>{i + 1}.</span> {word}
                  {i < 11 ? ' ' : ''}
                </span>
              ))}
            </div>

            <button
              onClick={handleCopyMnemonic}
              style={{
                background: 'transparent',
                color: '#2d5016',
                border: '2px solid #2d5016',
                borderRadius: '8px',
                padding: '8px 16px',
                fontSize: '13px',
                fontWeight: 600,
                cursor: 'pointer',
                marginBottom: '16px',
                width: '100%',
              }}
            >
              {copied ? 'Copied!' : 'Copy to Clipboard'}
            </button>

            <label style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: '8px',
              fontSize: '13px',
              color: '#1a2e0a',
              marginBottom: '16px',
              cursor: 'pointer',
            }}>
              <input
                type="checkbox"
                checked={backedUp}
                onChange={e => setBackedUp(e.target.checked)}
                style={{ width: '16px', height: '16px', accentColor: '#2d5016' }}
              />
              I have backed up my mnemonic
            </label>

            <button
              onClick={handleConfirmBackup}
              disabled={!backedUp}
              style={{
                background: backedUp ? '#2d5016' : '#aaa',
                color: '#f5f1e8',
                border: 'none',
                borderRadius: '8px',
                padding: '12px 24px',
                fontSize: '15px',
                fontWeight: 600,
                cursor: backedUp ? 'pointer' : 'not-allowed',
                width: '100%',
              }}
            >
              Continue to Wallet
            </button>
          </>
        )}
      </div>
    </div>
  );

  const renderLoading = () => (
    <div style={{
      background: '#d4c4a8',
      borderRadius: '12px',
      width: '380px',
      padding: '48px 24px',
      textAlign: 'center',
      boxShadow: '0 8px 32px rgba(45, 80, 22, 0.3)',
      cursor: 'default',
      fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    }} onClick={e => e.stopPropagation()}>
      <div style={{ fontSize: '32px', marginBottom: '16px', animation: 'spin 1s linear infinite' }}>&#x23F3;</div>
      <p style={{ color: '#1a2e0a', fontSize: '14px', margin: 0 }}>Connecting to wallet...</p>
    </div>
  );

  return (
    <div
      onClick={handleBackgroundClick}
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100vw',
        height: '100vh',
        margin: 0,
        padding: 0,
        overflow: 'hidden',
        display: 'flex',
        justifyContent: 'flex-end',
        alignItems: 'flex-start',
        paddingTop: '150px',
        paddingRight: paddingRightPx > 0 ? `${paddingRightPx}px` : '0px',
        boxSizing: 'border-box',
        cursor: 'pointer',
        backgroundColor: 'rgba(0, 0, 0, 0.01)',
      }}
    >
      {walletStatus === 'loading' && renderLoading()}
      {walletStatus === 'no-wallet' && renderNoWallet()}
      {walletStatus === 'exists' && <WalletPanel onClose={handleClose} />}
    </div>
  );
}
