import React, { useState, useEffect, useCallback } from 'react';

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

  // Rescan wallet
  const [rescanning, setRescanning] = useState(false);
  const [rescanResult, setRescanResult] = useState<{
    addresses_scanned: number;
    new_addresses_found: number;
    new_utxos_found: number;
    balance: number;
  } | null>(null);
  const [rescanError, setRescanError] = useState<string | null>(null);

  // Export backup
  const [showExportForm, setShowExportForm] = useState(false);
  const [exportPassword, setExportPassword] = useState('');
  const [exportConfirm, setExportConfirm] = useState('');
  const [exportError, setExportError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportSuccess, setExportSuccess] = useState(false);

  // Delete wallet
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleteInput, setDeleteInput] = useState('');
  const [deletePin, setDeletePin] = useState('');
  const [deleteStep, setDeleteStep] = useState(1);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
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

  const handleRescan = async () => {
    try {
      setRescanning(true);
      setRescanError(null);
      setRescanResult(null);
      const res = await fetch('http://127.0.0.1:31301/wallet/rescan', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });
      const data = await res.json();
      if (!res.ok) {
        setRescanError(data.error || 'Rescan failed');
        return;
      }
      setRescanResult(data);
    } catch (err) {
      setRescanError(err instanceof Error ? err.message : 'Failed to connect to wallet server');
    } finally {
      setRescanning(false);
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

  const handleDeleteWallet = async () => {
    try {
      setDeleting(true);
      setDeleteError(null);

      // Verify PIN first
      const unlockRes = await fetch('http://127.0.0.1:31301/wallet/unlock', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pin: deletePin }),
      });
      if (!unlockRes.ok) {
        const unlockData = await unlockRes.json();
        setDeleteError(unlockData.error || 'Invalid PIN');
        return;
      }

      // Delete wallet
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
      // Close wallet overlay on success
      window.close();
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
            <button
              className="wd-btn-primary"
              onClick={handleSaveDisplayName}
              disabled={nameSaving || displayName === savedDisplayName}
            >
              {nameSaving ? 'Saving...' : 'Save'}
            </button>
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
            <button
              className="wd-btn-secondary"
              onClick={() => setShowIdentityKey(!showIdentityKey)}
            >
              {showIdentityKey ? 'Hide Identity Key' : 'View Identity Key'}
            </button>
            {showIdentityKey && identityKey && (
              <button
                className={`wd-btn-secondary${identityKeyCopied ? '' : ''}`}
                onClick={handleCopyIdentityKey}
                style={identityKeyCopied ? { borderColor: '#2e7d32', color: '#4caf50' } : {}}
              >
                {identityKeyCopied ? 'Copied!' : 'Copy'}
              </button>
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
            <button
              className="wd-btn-secondary"
              onClick={() => setShowMnemonicForm(true)}
            >
              View Recovery Phrase
            </button>
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
                <button
                  className="wd-btn-primary"
                  onClick={handleRevealMnemonic}
                  disabled={revealingMnemonic || !mnemonicPin}
                >
                  {revealingMnemonic ? 'Verifying...' : 'Reveal'}
                </button>
                <button
                  className="wd-btn-secondary"
                  onClick={() => { setShowMnemonicForm(false); setMnemonicPin(''); setMnemonicError(null); }}
                >
                  Cancel
                </button>
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
              <button
                className="wd-btn-secondary"
                onClick={() => { setMnemonic(null); setShowMnemonicForm(false); }}
                style={{ marginTop: '12px' }}
              >
                Hide Recovery Phrase
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Wallet Recovery / Rescan */}
      <div className="wd-settings-section">
        <div className="wd-section-title">Wallet Rescan</div>
        <div className="wd-section-desc">
          Scan the blockchain for all addresses derived from your recovery phrase.
          Use this if you believe your wallet is missing transactions.
        </div>

        {rescanError && (
          <div className="wd-alert error">{rescanError}</div>
        )}

        {rescanResult && (
          <div className="wd-rescan-result">
            <div className="wd-rescan-stat">
              <span className="wd-rescan-stat-label">Addresses scanned</span>
              <span className="wd-rescan-stat-value">{rescanResult.addresses_scanned}</span>
            </div>
            <div className="wd-rescan-stat">
              <span className="wd-rescan-stat-label">New addresses found</span>
              <span className="wd-rescan-stat-value">{rescanResult.new_addresses_found}</span>
            </div>
            <div className="wd-rescan-stat">
              <span className="wd-rescan-stat-label">New UTXOs found</span>
              <span className="wd-rescan-stat-value">{rescanResult.new_utxos_found}</span>
            </div>
            <div className="wd-rescan-stat">
              <span className="wd-rescan-stat-label">Balance</span>
              <span className="wd-rescan-stat-value">
                {(rescanResult.balance / 100000000).toFixed(8)} BSV
              </span>
            </div>
          </div>
        )}

        <button
          className="wd-btn-secondary"
          onClick={handleRescan}
          disabled={rescanning}
        >
          {rescanning ? (
            <>
              <span className="wd-spinner" style={{ width: 14, height: 14, borderWidth: 2, display: 'inline-block', verticalAlign: 'middle', marginRight: 8 }} />
              Scanning...
            </>
          ) : 'Rescan Wallet'}
        </button>
      </div>

      {/* Export Backup */}
      <div className="wd-settings-section">
        <div className="wd-section-title">Export Backup</div>
        <div className="wd-section-desc">Download an encrypted backup of your wallet</div>

        {exportSuccess ? (
          <div className="wd-alert success">Backup downloaded successfully!</div>
        ) : !showExportForm ? (
          <button className="wd-btn-secondary" onClick={() => setShowExportForm(true)}>
            Export Wallet Backup
          </button>
        ) : (
          <div className="wd-export-form">
            <input
              type="password"
              placeholder="Password (min 8 characters)"
              value={exportPassword}
              onChange={(e) => { setExportPassword(e.target.value); setExportError(null); }}
              disabled={exporting}
            />
            <input
              type="password"
              placeholder="Confirm password"
              value={exportConfirm}
              onChange={(e) => { setExportConfirm(e.target.value); setExportError(null); }}
              disabled={exporting}
            />
            {exportError && <div className="wd-alert error">{exportError}</div>}
            <div style={{ display: 'flex', gap: '8px' }}>
              <button
                className="wd-btn-primary"
                onClick={handleExportBackup}
                disabled={exporting || exportPassword.length < 8}
              >
                {exporting ? 'Encrypting...' : 'Download Backup'}
              </button>
              <button
                className="wd-btn-secondary"
                onClick={() => { setShowExportForm(false); setExportPassword(''); setExportConfirm(''); setExportError(null); }}
                disabled={exporting}
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Danger Zone - Delete Wallet */}
      <div className="wd-settings-section danger">
        <div className="wd-section-title" style={{ color: '#ef5350' }}>Danger Zone</div>
        <div className="wd-section-desc">Permanently delete your wallet and all data</div>

        {balance > 0 && (
          <div className="wd-balance-warning">
            You still have {(balance / 100000000).toFixed(8)} BSV in your wallet. Transfer your funds before deleting.
          </div>
        )}

        {!showDeleteConfirm ? (
          <button className="wd-btn-danger" onClick={() => { setShowDeleteConfirm(true); setDeleteStep(1); setDeleteInput(''); setDeletePin(''); setDeleteError(null); }}>
            Delete Wallet
          </button>
        ) : (
          <div className="wd-delete-confirm">
            {deleteError && <div className="wd-alert error">{deleteError}</div>}

            {deleteStep === 1 && (
              <>
                <div style={{ fontSize: '14px', color: '#ef5350', marginBottom: '8px' }}>
                  This will permanently delete your wallet. Type <strong>DELETE</strong> to confirm.
                </div>
                <input
                  type="text"
                  value={deleteInput}
                  onChange={(e) => setDeleteInput(e.target.value)}
                  placeholder="Type DELETE"
                />
                <div style={{ display: 'flex', gap: '8px', marginTop: '12px' }}>
                  <button
                    className="wd-btn-danger"
                    disabled={deleteInput !== 'DELETE'}
                    onClick={() => setDeleteStep(2)}
                  >
                    Continue
                  </button>
                  <button className="wd-btn-secondary" onClick={() => setShowDeleteConfirm(false)}>
                    Cancel
                  </button>
                </div>
              </>
            )}

            {deleteStep === 2 && (
              <>
                <div style={{ fontSize: '14px', color: '#ef5350', marginBottom: '8px' }}>
                  Enter your PIN to verify ownership.
                </div>
                <input
                  type="password"
                  value={deletePin}
                  onChange={(e) => { setDeletePin(e.target.value); setDeleteError(null); }}
                  placeholder="Enter PIN"
                  maxLength={10}
                />
                <div style={{ display: 'flex', gap: '8px', marginTop: '12px' }}>
                  <button
                    className="wd-btn-danger"
                    disabled={deleting || !deletePin}
                    onClick={handleDeleteWallet}
                  >
                    {deleting ? 'Deleting...' : 'Delete Wallet Permanently'}
                  </button>
                  <button className="wd-btn-secondary" onClick={() => setShowDeleteConfirm(false)}>
                    Cancel
                  </button>
                </div>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

export default SettingsTab;
