import { useMemo, useState, useEffect, useRef, useCallback } from 'react';
import WalletPanel from '../components/WalletPanel';

// Reusable 4-digit PIN input (4 boxes, password-masked, numeric-only)
function PinInput({
  digits,
  onChange,
  disabled,
}: {
  digits: string[];
  onChange: (digits: string[]) => void;
  disabled?: boolean;
}) {
  const refs = [
    useRef<HTMLInputElement>(null),
    useRef<HTMLInputElement>(null),
    useRef<HTMLInputElement>(null),
    useRef<HTMLInputElement>(null),
  ];

  const handleChange = (index: number, value: string) => {
    // Only allow single digits
    const digit = value.replace(/\D/g, '').slice(-1);
    const next = [...digits];
    next[index] = digit;
    onChange(next);
    // Auto-advance to next box
    if (digit && index < 3) {
      refs[index + 1].current?.focus();
    }
  };

  const handleKeyDown = (index: number, e: React.KeyboardEvent) => {
    if (e.key === 'Backspace' && !digits[index] && index > 0) {
      e.preventDefault();
      refs[index - 1].current?.focus();
    }
  };

  // Auto-focus first box on mount
  useEffect(() => {
    const timer = setTimeout(() => refs[0].current?.focus(), 50);
    return () => clearTimeout(timer);
  }, []);

  return (
    <div style={{ display: 'flex', gap: '12px', justifyContent: 'center' }}>
      {digits.map((d, i) => (
        <input
          key={i}
          ref={refs[i]}
          type="password"
          inputMode="numeric"
          maxLength={1}
          value={d}
          onChange={e => handleChange(i, e.target.value)}
          onKeyDown={e => handleKeyDown(i, e)}
          disabled={disabled}
          autoComplete="off"
          style={{
            width: '48px',
            height: '56px',
            textAlign: 'center',
            fontSize: '24px',
            fontWeight: 700,
            borderRadius: '8px',
            border: `2px solid ${d ? '#2d5016' : '#e8dcc0'}`,
            background: '#f5f1e8',
            color: '#1a2e0a',
            outline: 'none',
            transition: 'border-color 0.15s',
          }}
          onFocus={e => { e.target.style.borderColor = '#2d5016'; }}
          onBlur={e => { e.target.style.borderColor = d ? '#2d5016' : '#e8dcc0'; }}
        />
      ))}
    </div>
  );
}

export default function WalletPanelPage() {
  // Read icon position from URL param (physical pixels, passed from toolbar click)
  const paddingRightPx = useMemo(() => {
    const params = new URLSearchParams(window.location.search);
    const iro = parseInt(params.get('iro') || '0', 10);
    if (iro <= 0) return 0;
    const dpr = window.devicePixelRatio || 1;
    return Math.round(iro / dpr);
  }, []);

  // Cache-first init: check both exists and locked from localStorage
  const cachedExists = localStorage.getItem('hodos_wallet_exists') === 'true';
  const cachedLocked = localStorage.getItem('hodos_wallet_locked') === 'true';
  const initialStatus = cachedExists
    ? (cachedLocked ? 'locked' : 'exists')
    : 'loading';

  const [walletStatus, setWalletStatus] = useState<'loading' | 'exists' | 'no-wallet' | 'locked'>(
    initialStatus as 'loading' | 'exists' | 'no-wallet' | 'locked'
  );
  const [creating, setCreating] = useState(false);
  const [mnemonic, setMnemonic] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [backedUp, setBackedUp] = useState(false);

  // Recovery state
  const [showRecoveryInput, setShowRecoveryInput] = useState(false);
  const [recoveryWords, setRecoveryWords] = useState<string[]>(Array(12).fill(''));
  const [recovering, setRecovering] = useState(false);
  const [recoveryError, setRecoveryError] = useState<string | null>(null);
  const [recoveryResult, setRecoveryResult] = useState<{
    addresses_found: number;
    utxos_found: number;
    total_balance: number;
    message: string;
  } | null>(null);

  // Backup import state
  const [showImportForm, setShowImportForm] = useState(false);
  const [importPassword, setImportPassword] = useState('');
  const [importBackupText, setImportBackupText] = useState('');
  const [importFile, setImportFile] = useState<any>(null);
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [importResult, setImportResult] = useState<{
    transactions: number;
    outputs: number;
    addresses: number;
    certificates: number;
  } | null>(null);

  // PIN state
  const [pinStep, setPinStep] = useState<'create' | 'confirm' | null>(null);
  const [pinDigits, setPinDigits] = useState<string[]>(['', '', '', '']);
  const [confirmPinDigits, setConfirmPinDigits] = useState<string[]>(['', '', '', '']);
  const [pinError, setPinError] = useState<string | null>(null);
  const [pendingPin, setPendingPin] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<'create' | 'recover' | 'import' | null>(null);

  // Unlock state (for locked wallet)
  const [unlockDigits, setUnlockDigits] = useState<string[]>(['', '', '', '']);
  const [unlocking, setUnlocking] = useState(false);
  const [unlockError, setUnlockError] = useState<string | null>(null);

  useEffect(() => {
    // If localStorage says wallet exists and not locked, trust it.
    // If locked, also trust it. Only fetch when no cached state.
    if (cachedExists && !cachedLocked) return;

    fetch('http://localhost:3301/wallet/status')
      .then(r => r.json())
      .then(data => {
        if (data.exists) {
          localStorage.setItem('hodos_wallet_exists', 'true');
          localStorage.setItem('hodos_wallet_locked', data.locked ? 'true' : 'false');
          setWalletStatus(data.locked ? 'locked' : 'exists');
        } else {
          localStorage.removeItem('hodos_wallet_exists');
          localStorage.setItem('hodos_wallet_locked', 'false');
          setWalletStatus('no-wallet');
        }
      })
      .catch(() => setWalletStatus(cachedLocked ? 'locked' : 'no-wallet'));
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

  // --- PIN Flow ---

  // When user clicks "Create New Wallet", start PIN creation flow
  const handleStartCreate = () => {
    setPendingAction('create');
    setPinStep('create');
    setPinDigits(['', '', '', '']);
    setConfirmPinDigits(['', '', '', '']);
    setPinError(null);
    setPendingPin(null);
  };

  // Auto-submit when 4 digits filled (PIN create step)
  const handlePinDigitsChange = useCallback((digits: string[]) => {
    setPinDigits(digits);
    setPinError(null);
    if (digits.every(d => d !== '')) {
      const pin = digits.join('');
      setPendingPin(pin);
      setPinStep('confirm');
      setConfirmPinDigits(['', '', '', '']);
    }
  }, []);

  // Auto-submit when 4 digits filled (PIN confirm step)
  const handleConfirmPinDigitsChange = useCallback((digits: string[]) => {
    setConfirmPinDigits(digits);
    setPinError(null);
    if (digits.every(d => d !== '')) {
      const confirmPin = digits.join('');
      if (confirmPin !== pendingPin) {
        setPinError('PINs did not match. Please start over.');
        setPinStep('create');
        setPinDigits(['', '', '', '']);
        setConfirmPinDigits(['', '', '', '']);
        setPendingPin(null);
        return;
      }
      // PINs match — execute the pending action
      if (pendingAction === 'create') {
        doCreateWallet(confirmPin);
      } else if (pendingAction === 'recover') {
        doRecoverWallet(confirmPin);
      } else if (pendingAction === 'import') {
        doImportBackup(confirmPin);
      }
    }
  }, [pendingPin, pendingAction]);

  // Actually call the create API with PIN
  const doCreateWallet = async (pin: string) => {
    setPinStep(null);
    setCreating(true);
    try {
      const res = await fetch('http://localhost:3301/wallet/create', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pin }),
      });
      const data = await res.json();
      if (data.success && data.mnemonic) {
        setMnemonic(data.mnemonic);
      } else {
        alert(data.error || 'Failed to create wallet');
        setCreating(false);
      }
    } catch {
      alert('Failed to connect to wallet server');
      setCreating(false);
    }
    setPendingAction(null);
    setPendingPin(null);
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
    localStorage.setItem('hodos_wallet_locked', 'false');
    setMnemonic(null);
    setCreating(false);
    setWalletStatus('exists');
  };

  // --- Recovery Flow ---

  const handleWordChange = (index: number, value: string) => {
    const pastedWords = value.trim().split(/\s+/);
    if (pastedWords.length > 1) {
      const newWords = [...recoveryWords];
      for (let i = 0; i < 12; i++) {
        const wi = i - index;
        if (wi >= 0 && wi < pastedWords.length) {
          newWords[i] = pastedWords[wi].toLowerCase();
        }
      }
      setRecoveryWords(newWords);
      setRecoveryError(null);
      const lastFilled = Math.min(index + pastedWords.length - 1, 11);
      const el = document.getElementById(`mnemonic-word-${lastFilled}`);
      if (el) setTimeout(() => el.focus(), 0);
      return;
    }
    const newWords = [...recoveryWords];
    newWords[index] = value.toLowerCase().replace(/\s/g, '');
    setRecoveryWords(newWords);
    setRecoveryError(null);
  };

  const handleWordKeyDown = (index: number, e: React.KeyboardEvent) => {
    if (e.key === ' ' || e.key === 'Tab') {
      if (index < 11) {
        e.preventDefault();
        const el = document.getElementById(`mnemonic-word-${index + 1}`);
        if (el) el.focus();
      }
    } else if (e.key === 'Backspace' && recoveryWords[index] === '' && index > 0) {
      e.preventDefault();
      const el = document.getElementById(`mnemonic-word-${index - 1}`);
      if (el) el.focus();
    }
  };

  // When user clicks "Recover Wallet" with valid words, start PIN creation
  const handleStartRecover = () => {
    const filledWords = recoveryWords.map(w => w.trim().toLowerCase()).filter(w => w.length > 0);
    if (filledWords.length !== 12) {
      setRecoveryError(`Expected 12 words, got ${filledWords.length}. Fill in all boxes.`);
      return;
    }
    const emptyBoxes = recoveryWords.findIndex(w => w.trim() === '');
    if (emptyBoxes !== -1) {
      setRecoveryError(`Word ${emptyBoxes + 1} is empty.`);
      return;
    }

    // Words are valid — start PIN creation
    setPendingAction('recover');
    setPinStep('create');
    setPinDigits(['', '', '', '']);
    setConfirmPinDigits(['', '', '', '']);
    setPinError(null);
    setPendingPin(null);
  };

  // Actually call the recovery API with PIN
  const doRecoverWallet = async (pin: string) => {
    setPinStep(null);
    setPendingAction(null);
    setPendingPin(null);

    const mnemonicPhrase = recoveryWords.map(w => w.trim().toLowerCase()).join(' ');
    setRecovering(true);
    setRecoveryError(null);

    try {
      const res = await fetch('http://localhost:3301/wallet/recover', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          mnemonic: mnemonicPhrase,
          pin,
          confirm: true,
          gap_limit: 20,
        }),
      });

      const data = await res.json();

      if (res.status === 409) {
        setRecoveryError('A wallet already exists. Delete the existing wallet first.');
        setRecovering(false);
        return;
      }

      if (data.success) {
        setRecoveryResult({
          addresses_found: data.addresses_found || 0,
          utxos_found: data.utxos_found || 0,
          total_balance: data.total_balance || 0,
          message: data.message || 'Recovery complete!',
        });
      } else {
        setRecoveryError(data.error || 'Recovery failed');
      }
    } catch {
      setRecoveryError('Failed to connect to wallet server');
    }

    setRecovering(false);
  };

  const handleRecoveryComplete = () => {
    localStorage.setItem('hodos_wallet_exists', 'true');
    localStorage.setItem('hodos_wallet_locked', 'false');
    setRecoveryResult(null);
    setShowRecoveryInput(false);
    setRecoveryWords(Array(12).fill(''));
    setWalletStatus('exists');
  };

  // --- Backup Import Flow ---

  const handleImportFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setImportError(null);
    setImportFile(null);
    setImportBackupText('');
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const text = reader.result as string;
      setImportBackupText(text);
      try {
        const parsed = JSON.parse(text.trim());
        if (parsed.format !== 'hodos-wallet-backup') {
          setImportError('Invalid backup file format');
          return;
        }
        setImportFile(parsed);
      } catch {
        setImportError('Could not parse file. Make sure it is a valid .hodos-wallet file.');
      }
    };
    reader.onerror = () => {
      setImportError('Failed to read file');
    };
    reader.readAsText(file);
  };

  const handleStartImport = () => {
    if (!importFile) {
      setImportError('Please select a valid .hodos-wallet backup file.');
      return;
    }
    if (importPassword.length < 8) {
      setImportError('Backup password must be at least 8 characters.');
      return;
    }
    // Start PIN creation
    setPendingAction('import');
    setPinStep('create');
    setPinDigits(['', '', '', '']);
    setConfirmPinDigits(['', '', '', '']);
    setPinError(null);
    setPendingPin(null);
  };

  const doImportBackup = async (pin: string) => {
    setPinStep(null);
    setPendingAction(null);
    setPendingPin(null);

    setImporting(true);
    setImportError(null);

    try {
      const res = await fetch('http://localhost:3301/wallet/import', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          pin,
          password: importPassword,
          backup: importFile,
        }),
      });

      const data = await res.json();

      if (res.status === 409) {
        setImportError('A wallet already exists. Delete the existing wallet first.');
        setImporting(false);
        return;
      }
      if (res.status === 401) {
        setImportError(data.error || 'Invalid backup password');
        setImporting(false);
        return;
      }
      if (!res.ok) {
        setImportError(data.error || 'Import failed');
        setImporting(false);
        return;
      }

      setImportResult({
        transactions: data.transactions || 0,
        outputs: data.outputs || 0,
        addresses: data.addresses || 0,
        certificates: data.certificates || 0,
      });
    } catch {
      setImportError('Failed to connect to wallet server');
    }

    setImporting(false);
  };

  const handleImportComplete = () => {
    localStorage.setItem('hodos_wallet_exists', 'true');
    localStorage.setItem('hodos_wallet_locked', 'false');
    setImportResult(null);
    setShowImportForm(false);
    setImportPassword('');
    setImportBackupText('');
    setImportFile(null);
    setWalletStatus('exists');
  };

  // --- Unlock Flow ---

  const handleUnlockDigitsChange = useCallback((digits: string[]) => {
    setUnlockDigits(digits);
    setUnlockError(null);
    if (digits.every(d => d !== '')) {
      doUnlock(digits.join(''));
    }
  }, []);

  const doUnlock = async (pin: string) => {
    setUnlocking(true);
    setUnlockError(null);
    try {
      const res = await fetch('http://localhost:3301/wallet/unlock', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pin }),
      });
      const data = await res.json();
      if (res.ok && data.success) {
        localStorage.setItem('hodos_wallet_locked', 'false');
        setWalletStatus('exists');
      } else {
        setUnlockError(data.error || 'Invalid PIN');
        setUnlockDigits(['', '', '', '']);
      }
    } catch {
      setUnlockError('Failed to connect to wallet server');
      setUnlockDigits(['', '', '', '']);
    }
    setUnlocking(false);
  };

  // --- PIN creation/confirm screens (shared between create + recovery) ---

  const renderPinCreate = () => (
    <>
      <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F512;</div>
      <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>Create a PIN</h3>
      <p style={{ color: '#555', fontSize: '13px', margin: '0 0 24px' }}>
        Choose a 4-digit PIN to protect your wallet on this device.
      </p>

      {pinError && (
        <p style={{ color: '#c62828', fontSize: '12px', margin: '0 0 12px', fontWeight: 600 }}>
          {pinError}
        </p>
      )}

      <PinInput key="pin-create" digits={pinDigits} onChange={handlePinDigitsChange} />

      <p style={{ color: '#888', fontSize: '11px', margin: '16px 0 0', fontStyle: 'italic' }}>
        This PIN protects access to your wallet on this device. It is not used to derive your keys or addresses.
      </p>

      <button
        onClick={() => {
          setPinStep(null);
          setPendingAction(null);
          setPinDigits(['', '', '', '']);
          setPinError(null);
        }}
        style={{
          background: 'transparent',
          color: '#2d5016',
          border: '2px solid #2d5016',
          borderRadius: '8px',
          padding: '8px 16px',
          fontSize: '13px',
          fontWeight: 600,
          cursor: 'pointer',
          width: '100%',
          marginTop: '20px',
        }}
      >
        Back
      </button>
    </>
  );

  const renderPinConfirm = () => (
    <>
      <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F512;</div>
      <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>Confirm your PIN</h3>
      <p style={{ color: '#555', fontSize: '13px', margin: '0 0 24px' }}>
        Enter the same 4-digit PIN again to confirm.
      </p>

      <PinInput key="pin-confirm" digits={confirmPinDigits} onChange={handleConfirmPinDigitsChange} />

      {pinError && (
        <p style={{ color: '#c62828', fontSize: '12px', margin: '12px 0 0', fontWeight: 600 }}>
          {pinError}
        </p>
      )}

      <button
        onClick={() => {
          setPinStep('create');
          setPinDigits(['', '', '', '']);
          setConfirmPinDigits(['', '', '', '']);
          setPendingPin(null);
          setPinError(null);
        }}
        style={{
          background: 'transparent',
          color: '#2d5016',
          border: '2px solid #2d5016',
          borderRadius: '8px',
          padding: '8px 16px',
          fontSize: '13px',
          fontWeight: 600,
          cursor: 'pointer',
          width: '100%',
          marginTop: '20px',
        }}
      >
        Back
      </button>
    </>
  );

  // --- Render functions ---

  const renderLocked = () => (
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
        <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F510;</div>
        <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>Wallet Locked</h3>
        <p style={{ color: '#555', fontSize: '13px', margin: '0 0 24px' }}>
          Enter your 4-digit PIN to unlock your wallet.
        </p>

        <PinInput digits={unlockDigits} onChange={handleUnlockDigitsChange} disabled={unlocking} />

        {unlocking && (
          <p style={{ color: '#1a2e0a', fontSize: '13px', margin: '16px 0 0' }}>Unlocking...</p>
        )}

        {unlockError && (
          <p style={{ color: '#c62828', fontSize: '12px', margin: '12px 0 0', fontWeight: 600 }}>
            {unlockError}
          </p>
        )}

        <p style={{ color: '#888', fontSize: '11px', margin: '20px 0 0', fontStyle: 'italic' }}>
          Forgot your PIN? You can recover your wallet using your mnemonic phrase.
        </p>
      </div>
    </div>
  );

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
        {/* PIN creation/confirm overlays (during create or recovery) */}
        {pinStep === 'create' ? renderPinCreate()
        : pinStep === 'confirm' ? renderPinConfirm()
        : recoveryResult ? (
          /* Recovery success */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x2705;</div>
            <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>Wallet Recovered</h3>
            <p style={{ color: '#555', fontSize: '13px', margin: '0 0 16px' }}>
              {recoveryResult.message}
            </p>

            <div style={{
              background: '#f5f1e8',
              border: '2px solid #e8dcc0',
              borderRadius: '8px',
              padding: '16px',
              marginBottom: '16px',
              textAlign: 'left',
            }}>
              <div style={{ fontSize: '13px', color: '#1a2e0a', marginBottom: '8px' }}>
                <strong>Addresses found:</strong> {recoveryResult.addresses_found}
              </div>
              <div style={{ fontSize: '13px', color: '#1a2e0a', marginBottom: '8px' }}>
                <strong>UTXOs found:</strong> {recoveryResult.utxos_found}
              </div>
              <div style={{ fontSize: '13px', color: '#1a2e0a' }}>
                <strong>Balance:</strong> {(recoveryResult.total_balance / 100_000_000).toFixed(8)} BSV
                <span style={{ color: '#888', marginLeft: '8px' }}>
                  ({recoveryResult.total_balance.toLocaleString()} sats)
                </span>
              </div>
            </div>

            <button
              onClick={handleRecoveryComplete}
              style={{
                background: '#2d5016',
                color: '#f5f1e8',
                border: 'none',
                borderRadius: '8px',
                padding: '12px 24px',
                fontSize: '15px',
                fontWeight: 600,
                cursor: 'pointer',
                width: '100%',
              }}
            >
              Continue to Wallet
            </button>
          </>
        ) : showRecoveryInput ? (
          /* Recovery input form */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F50D;</div>
            <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>Recover Wallet</h3>
            <p style={{ color: '#555', fontSize: '13px', margin: '0 0 16px' }}>
              Enter your 12-word mnemonic phrase. You can paste all 12 words into any box.
            </p>

            {/* 12-box mnemonic grid */}
            <div style={{
              display: 'grid',
              gridTemplateColumns: '1fr 1fr 1fr',
              gap: '8px',
              marginBottom: recoveryError ? '8px' : '16px',
            }}>
              {recoveryWords.map((word, i) => (
                <div key={i} style={{ position: 'relative' }}>
                  <span style={{
                    position: 'absolute',
                    left: '8px',
                    top: '50%',
                    transform: 'translateY(-50%)',
                    fontSize: '10px',
                    color: '#999',
                    pointerEvents: 'none',
                    fontFamily: 'monospace',
                  }}>
                    {i + 1}.
                  </span>
                  <input
                    id={`mnemonic-word-${i}`}
                    type="text"
                    value={word}
                    onChange={e => handleWordChange(i, e.target.value)}
                    onKeyDown={e => handleWordKeyDown(i, e)}
                    disabled={recovering}
                    autoComplete="off"
                    spellCheck={false}
                    style={{
                      width: '100%',
                      padding: '10px 8px 10px 28px',
                      borderRadius: '6px',
                      border: `2px solid ${word ? '#2d5016' : '#e8dcc0'}`,
                      background: '#f5f1e8',
                      fontFamily: 'monospace',
                      fontSize: '13px',
                      color: '#1a2e0a',
                      boxSizing: 'border-box',
                      outline: 'none',
                      transition: 'border-color 0.15s',
                    }}
                    onFocus={e => { e.target.style.borderColor = '#2d5016'; }}
                    onBlur={e => { e.target.style.borderColor = word ? '#2d5016' : '#e8dcc0'; }}
                  />
                </div>
              ))}
            </div>

            {recoveryError && (
              <p style={{
                color: '#c62828',
                fontSize: '12px',
                margin: '0 0 12px',
                textAlign: 'left',
              }}>
                {recoveryError}
              </p>
            )}

            {recovering && (
              <div style={{
                background: '#f5f1e8',
                border: '2px solid #e8dcc0',
                borderRadius: '8px',
                padding: '12px 16px',
                marginBottom: '12px',
                textAlign: 'left',
              }}>
                <p style={{ color: '#1a2e0a', fontSize: '13px', fontWeight: 600, margin: '0 0 4px' }}>
                  Scanning blockchain for your addresses...
                </p>
                <p style={{ color: '#666', fontSize: '12px', margin: 0 }}>
                  This may take a minute depending on how many addresses your wallet has used.
                  You can safely close this and check back later — sync will continue in the background.
                </p>
              </div>
            )}

            <button
              onClick={handleStartRecover}
              disabled={recovering || recoveryWords.every(w => w.trim() === '')}
              style={{
                background: '#2d5016',
                color: '#f5f1e8',
                border: 'none',
                borderRadius: '8px',
                padding: '12px 24px',
                fontSize: '15px',
                fontWeight: 600,
                cursor: recovering ? 'not-allowed' : 'pointer',
                width: '100%',
                marginBottom: '12px',
                opacity: (recovering || recoveryWords.every(w => w.trim() === '')) ? 0.7 : 1,
              }}
            >
              {recovering ? 'Recovering...' : 'Recover Wallet'}
            </button>

            <button
              onClick={() => {
                setShowRecoveryInput(false);
                setRecoveryWords(Array(12).fill(''));
                setRecoveryError(null);
              }}
              disabled={recovering}
              style={{
                background: 'transparent',
                color: '#2d5016',
                border: '2px solid #2d5016',
                borderRadius: '8px',
                padding: '8px 16px',
                fontSize: '13px',
                fontWeight: 600,
                cursor: recovering ? 'not-allowed' : 'pointer',
                width: '100%',
              }}
            >
              Back
            </button>
          </>
        ) : importResult ? (
          /* Import success */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x2705;</div>
            <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>Wallet Restored</h3>
            <p style={{ color: '#555', fontSize: '13px', margin: '0 0 16px' }}>
              Your wallet has been restored from the backup file.
            </p>

            <div style={{
              background: '#f5f1e8',
              border: '2px solid #e8dcc0',
              borderRadius: '8px',
              padding: '16px',
              marginBottom: '16px',
              textAlign: 'left',
            }}>
              <div style={{ fontSize: '13px', color: '#1a2e0a', marginBottom: '8px' }}>
                <strong>Addresses:</strong> {importResult.addresses}
              </div>
              <div style={{ fontSize: '13px', color: '#1a2e0a', marginBottom: '8px' }}>
                <strong>Transactions:</strong> {importResult.transactions}
              </div>
              <div style={{ fontSize: '13px', color: '#1a2e0a', marginBottom: '8px' }}>
                <strong>Outputs:</strong> {importResult.outputs}
              </div>
              <div style={{ fontSize: '13px', color: '#1a2e0a' }}>
                <strong>Certificates:</strong> {importResult.certificates}
              </div>
            </div>

            <button
              onClick={handleImportComplete}
              style={{
                background: '#2d5016',
                color: '#f5f1e8',
                border: 'none',
                borderRadius: '8px',
                padding: '12px 24px',
                fontSize: '15px',
                fontWeight: 600,
                cursor: 'pointer',
                width: '100%',
              }}
            >
              Continue to Wallet
            </button>
          </>
        ) : showImportForm ? (
          /* Import from backup form */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F4E5;</div>
            <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>Import from Backup</h3>
            <p style={{ color: '#555', fontSize: '13px', margin: '0 0 12px' }}>
              Select your .hodos-wallet backup file to restore your wallet.
            </p>

            {/* File input */}
            <div style={{
              background: '#f5f1e8',
              border: `2px solid ${importFile ? '#2d5016' : '#e8dcc0'}`,
              borderRadius: '8px',
              padding: '12px',
              marginBottom: '12px',
              textAlign: 'center',
            }}>
              <input
                type="file"
                accept=".hodos-wallet"
                onChange={handleImportFileChange}
                disabled={importing}
                style={{
                  fontSize: '13px',
                  color: '#1a2e0a',
                  width: '100%',
                  cursor: 'pointer',
                }}
              />
              {importFile && (
                <p style={{ fontSize: '11px', color: '#2d5016', margin: '8px 0 0', fontWeight: 600 }}>
                  Valid backup file loaded
                </p>
              )}
            </div>

            {/* Password */}
            <input
              type="password"
              placeholder="Backup password (min 8 characters)"
              value={importPassword}
              onChange={e => { setImportPassword(e.target.value); setImportError(null); }}
              disabled={importing}
              style={{
                width: '100%',
                padding: '10px 12px',
                borderRadius: '6px',
                border: '2px solid #e8dcc0',
                background: '#f5f1e8',
                fontSize: '13px',
                color: '#1a2e0a',
                boxSizing: 'border-box',
                marginBottom: importError ? '8px' : '12px',
                outline: 'none',
              }}
            />

            {importError && (
              <p style={{
                color: '#c62828',
                fontSize: '12px',
                margin: '0 0 12px',
                textAlign: 'left',
              }}>
                {importError}
              </p>
            )}

            {importing && (
              <div style={{
                background: '#f5f1e8',
                border: '2px solid #e8dcc0',
                borderRadius: '8px',
                padding: '12px 16px',
                marginBottom: '12px',
                textAlign: 'left',
              }}>
                <p style={{ color: '#1a2e0a', fontSize: '13px', fontWeight: 600, margin: 0 }}>
                  Importing wallet data...
                </p>
              </div>
            )}

            <button
              onClick={handleStartImport}
              disabled={importing || !importFile}
              style={{
                background: '#2d5016',
                color: '#f5f1e8',
                border: 'none',
                borderRadius: '8px',
                padding: '12px 24px',
                fontSize: '15px',
                fontWeight: 600,
                cursor: importing ? 'not-allowed' : 'pointer',
                width: '100%',
                marginBottom: '12px',
                opacity: (importing || !importFile) ? 0.7 : 1,
              }}
            >
              {importing ? 'Importing...' : 'Import Wallet'}
            </button>

            <button
              onClick={() => {
                setShowImportForm(false);
                setImportPassword('');
                setImportBackupText('');
                setImportFile(null);
                setImportError(null);
              }}
              disabled={importing}
              style={{
                background: 'transparent',
                color: '#2d5016',
                border: '2px solid #2d5016',
                borderRadius: '8px',
                padding: '8px 16px',
                fontSize: '13px',
                fontWeight: 600,
                cursor: importing ? 'not-allowed' : 'pointer',
                width: '100%',
              }}
            >
              Back
            </button>
          </>
        ) : !mnemonic ? (
          /* Default: Create + Recover + Import buttons */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F512;</div>
            <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>No Wallet Found</h3>
            <p style={{ color: '#555', fontSize: '14px', margin: '0 0 24px' }}>
              Create a new wallet to get started with Bitcoin SV.
            </p>

            <button
              onClick={handleStartCreate}
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
              onClick={() => setShowRecoveryInput(true)}
              style={{
                background: 'transparent',
                color: '#2d5016',
                border: '2px solid #2d5016',
                borderRadius: '8px',
                padding: '12px 24px',
                fontSize: '15px',
                fontWeight: 600,
                cursor: 'pointer',
                width: '100%',
                marginBottom: '12px',
              }}
            >
              Recover Existing Wallet
            </button>

            <button
              onClick={() => setShowImportForm(true)}
              style={{
                background: 'transparent',
                color: '#2d5016',
                border: '2px solid #2d5016',
                borderRadius: '8px',
                padding: '12px 24px',
                fontSize: '15px',
                fontWeight: 600,
                cursor: 'pointer',
                width: '100%',
              }}
            >
              Import from Backup
            </button>
          </>
        ) : (
          /* Mnemonic backup (after create) */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x26A0;&#xFE0F;</div>
            <h3 style={{ margin: '0 0 8px', color: '#1a2e0a', fontSize: '18px' }}>Back Up Your Mnemonic</h3>

            <div style={{
              background: '#fff3e0',
              border: '2px solid #e65100',
              borderRadius: '8px',
              padding: '12px 16px',
              marginBottom: '16px',
              textAlign: 'left',
            }}>
              <p style={{ color: '#bf360c', fontSize: '12px', fontWeight: 700, margin: '0 0 6px' }}>
                Your mnemonic is your private key
              </p>
              <ul style={{ color: '#4e342e', fontSize: '12px', margin: 0, paddingLeft: '18px', lineHeight: '1.6' }}>
                <li><strong>Keep it secret.</strong> Anyone with these words can access your coins and identity.</li>
                <li><strong>Keep it safe.</strong> If you lose this mnemonic and something goes wrong, there is no way to recover your wallet.</li>
              </ul>
            </div>

            <p style={{ color: '#555', fontSize: '13px', margin: '0 0 12px' }}>
              Write down these 12 words in order and store them somewhere safe.
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
      {walletStatus === 'locked' && renderLocked()}
      {walletStatus === 'exists' && <WalletPanel onClose={handleClose} />}
    </div>
  );
}
