import React, { useState, useEffect, useCallback } from 'react';
import { HodosButton } from '../HodosButton';

const SettingsTab: React.FC = () => {
  // Display name
  const [displayName, setDisplayName] = useState('');
  const [savedDisplayName, setSavedDisplayName] = useState('');
  const [nameLoading, setNameLoading] = useState(true);
  const [nameSaving, setNameSaving] = useState(false);
  const [nameResult, setNameResult] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

  // Identity key
  const [identityKey, setIdentityKey] = useState('');
  const [showIdentityKey, setShowIdentityKey] = useState(false);
  const [identityKeyCopied, setIdentityKeyCopied] = useState(false);

  // Mnemonic reveal
  const [mnemonicPin, setMnemonicPin] = useState('');
  const [mnemonic, setMnemonic] = useState<string | null>(null);
  const [mnemonicError, setMnemonicError] = useState<string | null>(null);
  const [revealingMnemonic, setRevealingMnemonic] = useState(false);
  const [showMnemonicForm, setShowMnemonicForm] = useState(false);

  // Export backup (UI not yet wired)
  const [, setShowExportForm] = useState(false);
  const [exportPassword, setExportPassword] = useState('');
  const [exportConfirm, setExportConfirm] = useState('');
  const [, setExportError] = useState<string | null>(null);
  const [, setExporting] = useState(false);
  const [, setExportSuccess] = useState(false);

  // Delete wallet
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deletePin, setDeletePin] = useState('');
  const [deleteStep, setDeleteStep] = useState(1);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [deleteResult, setDeleteResult] = useState<{ backupTxid?: string; backupFailed?: string } | null>(null);
  const [balance, setBalance] = useState(0);

  const fetchSettings = useCallback(async () => {
    try {
      setNameLoading(true);
      const res = await fetch('http://127.0.0.1:31301/wallet/settings');
      if (!res.ok) throw new Error('Failed to fetch settings');
      const data = await res.json();
      setDisplayName(data.sender_display_name || 'Anonymous');
      setSavedDisplayName(data.sender_display_name || 'Anonymous');
    } catch {
      // Defaults if endpoint doesn't exist
      setDisplayName('Anonymous');
      setSavedDisplayName('Anonymous');
    } finally {
      setNameLoading(false);
    }
  }, []);

  const fetchIdentityKey = useCallback(async () => {
    try {
      const res = await fetch('http://127.0.0.1:31301/getPublicKey', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ identityKey: true }),
      });
      if (!res.ok) return;
      const data = await res.json();
      setIdentityKey(data.publicKey || '');
    } catch {
      // Silently fail
    }
  }, []);

  const fetchBalance = useCallback(async () => {
    try {
      const res = await fetch('http://127.0.0.1:31301/wallet/balance');
      if (!res.ok) return;
      const data = await res.json();
      setBalance(data.satoshis || 0);
    } catch {
      // Ignore
    }
  }, []);

  useEffect(() => {
    fetchSettings();
    fetchIdentityKey();
    fetchBalance();
  }, [fetchSettings, fetchIdentityKey, fetchBalance]);

  const handleSaveDisplayName = async () => {
    try {
      setNameSaving(true);
      setNameResult(null);
      const res = await fetch('http://127.0.0.1:31301/wallet/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ sender_display_name: displayName }),
      });
      if (!res.ok) throw new Error('Failed to save');
      setSavedDisplayName(displayName);
      setNameResult({ type: 'success', message: 'Display name saved' });
      setTimeout(() => setNameResult(null), 3000);
    } catch (err) {
      setNameResult({ type: 'error', message: err instanceof Error ? err.message : 'Save failed' });
    } finally {
      setNameSaving(false);
    }
  };

  const handleCopyIdentityKey = () => {
    if (!identityKey) return;
    navigator.clipboard.writeText(identityKey).catch(() => {});
    setIdentityKeyCopied(true);
    setTimeout(() => setIdentityKeyCopied(false), 2000);
  };

  const handleRevealMnemonic = async () => {
    if (!mnemonicPin || mnemonicPin.length < 4) {
      setMnemonicError('PIN must be at least 4 digits');
      return;
    }
    try {
      setRevealingMnemonic(true);
      setMnemonicError(null);
      const res = await fetch('http://127.0.0.1:31301/wallet/reveal-mnemonic', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pin: mnemonicPin }),
      });
      const data = await res.json();
      if (!res.ok) {
        setMnemonicError(data.error || 'Invalid PIN');
        return;
      }
      setMnemonic(data.mnemonic || '');
      setMnemonicPin('');
    } catch (err) {
      setMnemonicError(err instanceof Error ? err.message : 'Failed to reveal mnemonic');
    } finally {
      setRevealingMnemonic(false);
    }
  };

  const handleExportBackup = async () => {
    if (exportPassword.length < 8) {
      setExportError('Password must be at least 8 characters');
      return;
    }
    if (exportPassword !== exportConfirm) {
      setExportError('Passwords do not match');
      return;
    }
    try {
      setExporting(true);
      setExportError(null);
      const res = await fetch('http://127.0.0.1:31301/wallet/export', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ password: exportPassword }),
      });
      const data = await res.json();
      if (!res.ok) {
        setExportError(data.error || 'Export failed');
        return;
      }
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      const date = new Date().toISOString().slice(0, 10);
      a.href = url;
      a.download = `hodos-wallet-backup-${date}.hodos-wallet`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      setExportSuccess(true);
      setExportPassword('');
      setExportConfirm('');
      setTimeout(() => {
        setExportSuccess(false);
        setShowExportForm(false);
      }, 3000);
    } catch {
      setExportError('Failed to connect to wallet server');
    } finally {
      setExporting(false);
    }
  };
  void handleExportBackup; // suppress TS6133 until export UI is wired

  const handleDeleteWallet = async () => {
    try {
      setDeleting(true);
      setDeleteError(null);

      // Verify PIN — 409 "already unlocked" is fine (user already authenticated)
      const unlockRes = await fetch('http://127.0.0.1:31301/wallet/unlock', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pin: deletePin }),
      });
      if (!unlockRes.ok && unlockRes.status !== 409) {
        const unlockData = await unlockRes.json();
        setDeleteError(unlockData.error || 'Invalid PIN');
        return;
      }

      // Delete wallet (backend attempts on-chain backup before deleting)
      const res = await fetch('http://127.0.0.1:31301/wallet/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });
      const data = await res.json();
      if (!res.ok) {
        setDeleteError(data.error || 'Delete failed');
        return;
      }

      // Clear localStorage so header browser and wallet panel show clean state
      localStorage.removeItem('hodos_wallet_exists');
      localStorage.removeItem('hodos_identity_key');
      localStorage.removeItem('hodos:wallet:balance');
      localStorage.removeItem('hodos:wallet:bsvPrice');

      // Show completion screen with backup status
      setDeleteResult({
        backupTxid: data.backup_txid,
        backupFailed: data.backup_failed,
      });
      setDeleteStep(3);
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : 'Delete failed');
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="wd-settings">
      {/* Display Name */}
      <div className="wd-settings-section">
        <div className="wd-section-title">Display Name</div>
        <div className="wd-section-desc">Shown to paymail recipients when you send BSV</div>

        {nameResult && (
          <div className={`wd-alert ${nameResult.type}`}>{nameResult.message}</div>
        )}

        {nameLoading ? (
          <div className="wd-loading" style={{ padding: '12px 0' }}>
            <div className="wd-spinner" />
          </div>
        ) : (
          <>
            <div className="wd-settings-field">
              <label>Name</label>
              <input
                type="text"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
                placeholder="Anonymous"
              />
            </div>
            <HodosButton
              variant="primary"
              onClick={handleSaveDisplayName}
              disabled={displayName === savedDisplayName}
              loading={nameSaving}
              loadingText="Saving..."
            >
              Save
            </HodosButton>
          </>
        )}
      </div>

      {/* Security / Keys */}
      <div className="wd-settings-section">
        <div className="wd-section-title">Security & Keys</div>
        <div className="wd-section-desc">View your recovery phrase and identity key</div>

        {/* Identity Key (public, no PIN needed) */}
        <div style={{ marginBottom: '16px' }}>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center', marginBottom: '8px' }}>
            <HodosButton
              variant="secondary"
              onClick={() => setShowIdentityKey(!showIdentityKey)}
            >
              {showIdentityKey ? 'Hide Identity Key' : 'View Identity Key'}
            </HodosButton>
            {showIdentityKey && identityKey && (
              <HodosButton
                variant="secondary"
                onClick={handleCopyIdentityKey}
                style={identityKeyCopied ? { borderColor: '#2e7d32', color: '#4caf50' } : {}}
              >
                {identityKeyCopied ? 'Copied!' : 'Copy'}
              </HodosButton>
            )}
          </div>
          {showIdentityKey && identityKey && (
            <div className="wd-identity-key-display">
              <code>{identityKey}</code>
            </div>
          )}
        </div>

        {/* Mnemonic (PIN-gated) */}
        <div>
          {!showMnemonicForm && !mnemonic && (
            <HodosButton
              variant="secondary"
              onClick={() => setShowMnemonicForm(true)}
            >
              View Recovery Phrase
            </HodosButton>
          )}

          {showMnemonicForm && !mnemonic && (
            <div>
              <div className="wd-settings-field" style={{ marginBottom: '8px' }}>
                <label>Enter PIN to reveal recovery phrase</label>
                <input
                  type="password"
                  value={mnemonicPin}
                  onChange={(e) => { setMnemonicPin(e.target.value); setMnemonicError(null); }}
                  placeholder="Enter PIN"
                  maxLength={10}
                />
              </div>
              {mnemonicError && (
                <div className="wd-alert error" style={{ marginBottom: '8px' }}>{mnemonicError}</div>
              )}
              <div style={{ display: 'flex', gap: '8px' }}>
                <HodosButton
                  variant="primary"
                  onClick={handleRevealMnemonic}
                  disabled={!mnemonicPin}
                  loading={revealingMnemonic}
                  loadingText="Verifying..."
                >
                  Reveal
                </HodosButton>
                <HodosButton
                  variant="secondary"
                  onClick={() => { setShowMnemonicForm(false); setMnemonicPin(''); setMnemonicError(null); }}
                >
                  Cancel
                </HodosButton>
              </div>
            </div>
          )}

          {mnemonic && (
            <div>
              <div className="wd-mnemonic-display">
                <div className="wd-mnemonic-words">
                  {mnemonic.split(' ').map((word, i) => (
                    <span key={i} className="wd-mnemonic-word">
                      {i + 1}. {word}
                    </span>
                  ))}
                </div>
              </div>
              <HodosButton
                variant="secondary"
                onClick={() => { setMnemonic(null); setShowMnemonicForm(false); }}
                style={{ marginTop: '12px' }}
              >
                Hide Recovery Phrase
              </HodosButton>
            </div>
          )}
        </div>
      </div>

      {/* Export Backup — hidden for now, backend still supports /wallet/export */}

      {/* Delete Wallet */}
      <div className="wd-settings-section danger">
        <div className="wd-section-title" style={{ color: '#ef5350' }}>Delete Wallet</div>
        <div className="wd-section-desc">Permanently delete your wallet and all data</div>

        {balance > 0 && !showDeleteConfirm && (
          <div className="wd-alert" style={{ background: 'rgba(239, 83, 80, 0.1)', border: '1px solid rgba(239, 83, 80, 0.3)', borderRadius: '8px', padding: '12px', marginBottom: '12px', fontSize: '13px', color: '#ccc', lineHeight: '1.5' }}>
            Your wallet has <strong style={{ color: '#ef5350' }}>{(balance / 100000000).toFixed(8)} BSV</strong>. An on-chain backup will be attempted before deletion so you can recover your wallet and funds through the Hodos recovery system using your mnemonic phrase.
          </div>
        )}

        {!showDeleteConfirm ? (
          <HodosButton variant="danger" onClick={() => { setShowDeleteConfirm(true); setDeleteStep(1); setDeletePin(''); setDeleteError(null); setDeleteResult(null); }}>
            Delete Wallet
          </HodosButton>
        ) : (
          <div className="wd-delete-confirm">
            {deleteError && <div className="wd-alert error">{deleteError}</div>}

            {deleteStep === 1 && (
              <>
                <div style={{ fontSize: '14px', color: '#ef5350', marginBottom: '8px' }}>
                  This will permanently delete your wallet. Enter your PIN to confirm.
                </div>
                {balance > 0 && (
                  <div style={{ fontSize: '13px', color: '#aaa', marginBottom: '12px', lineHeight: '1.5' }}>
                    An on-chain backup will be saved before deletion. You can recover your wallet and remaining funds using your mnemonic phrase through the Hodos recovery system.
                  </div>
                )}
                <input
                  type="password"
                  value={deletePin}
                  onChange={(e) => { setDeletePin(e.target.value); setDeleteError(null); }}
                  placeholder="Enter PIN"
                  maxLength={10}
                />
                <div style={{ display: 'flex', gap: '8px', marginTop: '12px' }}>
                  <HodosButton
                    variant="danger"
                    disabled={!deletePin}
                    loading={deleting}
                    loadingText="Backing up & deleting..."
                    onClick={handleDeleteWallet}
                  >
                    Delete Wallet Permanently
                  </HodosButton>
                  <HodosButton variant="secondary" onClick={() => setShowDeleteConfirm(false)}>
                    Cancel
                  </HodosButton>
                </div>
              </>
            )}

            {deleteStep === 3 && deleteResult && (
              <div style={{ textAlign: 'center', padding: '16px 0' }}>
                <div style={{ fontSize: '18px', color: '#4caf50', marginBottom: '16px' }}>
                  Wallet Deleted
                </div>
                {deleteResult.backupTxid && deleteResult.backupTxid !== 'already_current' && (
                  <div style={{ fontSize: '13px', color: '#aaa', marginBottom: '12px', lineHeight: '1.5' }}>
                    On-chain backup saved successfully.<br />
                    <span style={{ color: '#888', fontSize: '12px', wordBreak: 'break-all' }}>
                      Backup txid: {deleteResult.backupTxid}
                    </span><br />
                    You can recover your wallet and funds using your mnemonic phrase.
                  </div>
                )}
                {deleteResult.backupTxid === 'already_current' && (
                  <div style={{ fontSize: '13px', color: '#aaa', marginBottom: '12px', lineHeight: '1.5' }}>
                    On-chain backup is already up to date.<br />
                    You can recover your wallet and funds using your mnemonic phrase.
                  </div>
                )}
                {deleteResult.backupFailed && (
                  <div style={{ fontSize: '13px', color: '#ef5350', marginBottom: '12px', lineHeight: '1.5' }}>
                    {deleteResult.backupFailed}
                  </div>
                )}
                <HodosButton variant="secondary" onClick={() => window.close()}>
                  Done
                </HodosButton>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

export default SettingsTab;
