import { useState, useEffect, useRef, useCallback } from 'react';
import WalletPanel from '../components/WalletPanel';
import { HodosButton } from '../components/HodosButton';
import { tokens } from '../theme/tokens';

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
            border: `2px solid ${d ? tokens.gold : tokens.borderDefault}`,
            background: tokens.bgElevated,
            color: tokens.textPrimary,
            outline: 'none',
            transition: 'border-color 0.15s',
          }}
          onFocus={e => { e.target.style.borderColor = tokens.gold; }}
          onBlur={e => { e.target.style.borderColor = d ? tokens.gold : tokens.borderDefault; }}
        />
      ))}
    </div>
  );
}

export default function WalletPanelPage() {
  const [, setPanelHeight] = useState<number | null>(null);

  useEffect(() => {
    document.body.style.margin = '0';
    document.body.style.overflow = 'hidden';
    document.body.style.background = 'transparent';
    document.documentElement.style.background = 'transparent';
  }, []);

  // Listen for HWND dimensions from C++ (on show and on resize)
  // Set a CSS variable so .wallet-panel-light can use it for max-height
  useEffect(() => {
    const handler = (e: MessageEvent) => {
      if (e.data?.type === 'wallet_shown' || e.data?.type === 'wallet_resize') {
        if (e.data.panelHeight > 0) {
          setPanelHeight(e.data.panelHeight);
          document.documentElement.style.setProperty('--wallet-hwnd-height', `${e.data.panelHeight}px`);
        }
      }
    };
    window.addEventListener('message', handler);
    return () => window.removeEventListener('message', handler);
  }, []);

  // Read icon position from URL param (physical pixels, passed from toolbar click)
  // Keep-alive: padding is now handled by C++ positioning, not CSS
  // const paddingRightPx = useMemo(() => { ... }, []);

  // Cache-first init: check exists from localStorage
  const cachedExists = localStorage.getItem('hodos_wallet_exists') === 'true';
  const initialStatus = cachedExists ? 'exists' : 'loading';

  const [walletStatus, setWalletStatus] = useState<'loading' | 'exists' | 'no-wallet' | 'locked'>(
    initialStatus as 'loading' | 'exists' | 'no-wallet' | 'locked'
  );

  // Fetch and cache identity key in localStorage (called after wallet creation/recovery).
  // Returns a promise so callers can await it before rendering components that read the key.
  const cacheIdentityKey = async () => {
    try {
      const r = await fetch('http://127.0.0.1:31301/getPublicKey', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ identityKey: true }),
      });
      const data = await r.json();
      if (data.publicKey) localStorage.setItem('hodos_identity_key', data.publicKey);
    } catch {
      // Silently fail — identity key will be fetched on next wallet panel open
    }
  };
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
    transactions: number;
    outputs: number;
    addresses: number;
    certificates: number;
    total_balance: number;
    spendable_utxos: number;
    fromBackup: boolean;
  } | null>(null);

  // Backup import state
  const [showImportForm, setShowImportForm] = useState(false);
  const [importPassword, setImportPassword] = useState('');
  const [_importBackupText, setImportBackupText] = useState('');
  const [importFile, setImportFile] = useState<any>(null);
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [importResult, setImportResult] = useState<{
    transactions: number;
    outputs: number;
    addresses: number;
    certificates: number;
  } | null>(null);

  // PIN state (for initial wallet creation/recovery only — not for unlock)
  const [pinStep, setPinStep] = useState<'create' | 'confirm' | null>(null);
  const [pinDigits, setPinDigits] = useState<string[]>(['', '', '', '']);
  const [confirmPinDigits, setConfirmPinDigits] = useState<string[]>(['', '', '', '']);
  const [pinError, setPinError] = useState<string | null>(null);
  const [pendingPin, setPendingPin] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<'create' | 'recover' | 'import' | null>(null);

  // Unlock state (fallback when DPAPI fails — e.g., DB moved to another machine)
  const [unlockPinDigits, setUnlockPinDigits] = useState<string[]>(['', '', '', '']);
  const [unlocking, setUnlocking] = useState(false);
  const [unlockError, setUnlockError] = useState<string | null>(null);

  const [cancelling, setCancelling] = useState(false);

  // Centbee recovery state
  const [showCentbeeRecovery, setShowCentbeeRecovery] = useState(false);
  const [centbeeWords, setCentbeeWords] = useState<string[]>(Array(12).fill(''));
  const [centbeePinDigits, setCentbeePinDigits] = useState<string[]>(['', '', '', '']);
  const [centbeeRecovering, setCentbeeRecovering] = useState(false);
  const [centbeeError, setCentbeeError] = useState<string | null>(null);
  const [centbeeProgress, setCentbeeProgress] = useState<string | null>(null);
  const [centbeeResult, setCentbeeResult] = useState<{
    utxos_found: number;
    total_balance: number;
    sweep_txids: string[];
    total_fees: number;
    brc42_balance: number;
    message: string;
  } | null>(null);

  // Prevent close during mnemonic display or PIN creation steps.
  // C++ sets g_wallet_overlay_prevent_close=true at overlay creation (synchronous).
  // React clears it (wallet_allow_close) when user reaches a safe state.
  // This avoids race conditions — C++ flag is always set before focus-loss events.
  const preventClose = mnemonic !== null || (pinStep !== null && pendingAction !== null);

  // Send IPC to C++ when preventClose changes
  useEffect(() => {
    const msg = preventClose ? 'wallet_prevent_close' : 'wallet_allow_close';
    if (window.cefMessage?.send) {
      window.cefMessage.send(msg, []);
    }
  }, [preventClose]);

  useEffect(() => {
    // If localStorage says wallet exists, trust it and skip the fetch
    if (cachedExists) return;

    console.log('[WalletPanel] Fetching wallet status from backend...');
    fetch('http://127.0.0.1:31301/wallet/status')
      .then(r => r.json())
      .then(data => {
        console.log('[WalletPanel] Wallet status response:', JSON.stringify(data));
        if (data.exists && data.locked) {
          localStorage.setItem('hodos_wallet_exists', 'true');
          cacheIdentityKey();
          setWalletStatus('locked');
        } else if (data.exists) {
          localStorage.setItem('hodos_wallet_exists', 'true');
          cacheIdentityKey();
          setWalletStatus('exists');
        } else {
          localStorage.removeItem('hodos_wallet_exists');
          localStorage.removeItem('hodos_identity_key');
          setWalletStatus('no-wallet');
          console.log('[WalletPanel] No wallet found — showing create/recover UI');
        }
      })
      .catch((err) => {
        console.error('[WalletPanel] Failed to fetch wallet status:', err);
        setWalletStatus('no-wallet');
      });
  }, []);

  // Keep-alive: reset UI state on hide (so next open is clean)
  useEffect(() => {
    const handleHidden = (e: MessageEvent) => {
      if (e.data?.type === 'wallet_hidden') {
        console.log('[WalletPanel] wallet_hidden — resetting UI state');
        setShowRecoveryInput(false);
        setRecoveryWords(Array(12).fill(''));
        setRecoveryError(null);
        setRecoveryResult(null);
        setShowImportForm(false);
        setShowCentbeeRecovery(false);
        setCentbeeWords(Array(12).fill(''));
        setCentbeePinDigits(['', '', '', '']);
        setCentbeeError(null);
        setCentbeeProgress(null);
        setCentbeeResult(null);
        setMnemonic(null);
        setPinStep(null);
        setPendingAction(null);
      }
    };
    // Keep-alive: re-fetch wallet status when overlay is shown again
    const handleShown = (e: MessageEvent) => {
      if (e.data?.type === 'wallet_shown') {
        console.log('[WalletPanel] wallet_shown — refreshing status');
        fetch('http://127.0.0.1:31301/wallet/status')
          .then(r => r.json())
          .then(data => {
            if (data.exists && data.locked) {
              setWalletStatus('locked');
            } else if (data.exists) {
              setWalletStatus('exists');
            } else {
              setWalletStatus('no-wallet');
            }
          })
          .catch(() => {});
      }
    };
    window.addEventListener('message', handleHidden);
    window.addEventListener('message', handleShown);
    return () => {
      window.removeEventListener('message', handleHidden);
      window.removeEventListener('message', handleShown);
    };
  }, []);

  const handleClose = () => {
    // Reset all setup/recovery form state so reopening the panel is clean
    setShowRecoveryInput(false);
    setRecoveryWords(Array(12).fill(''));
    setShowCentbeeRecovery(false);
    setCentbeeWords(Array(12).fill(''));
    setCentbeePinDigits(['', '', '', '']);
    setCentbeeError(null);
    setCentbeeProgress(null);
    setCentbeeResult(null);

    if (window.hodosBrowser?.overlay?.close) {
      window.hodosBrowser.overlay.close();
    } else if (window.cefMessage?.send) {
      window.cefMessage.send('overlay_close', []);
    }
  };

  // Keep-alive: click-outside is now handled by C++ mouse hook + WM_ACTIVATE
  // const handleBackgroundClick = (e: React.MouseEvent) => { ... };

  // --- PIN Flow (used during create, recover, import) ---

  const handleStartCreate = () => {
    console.log('[WalletPanel] handleStartCreate called — transitioning to PIN create screen');
    setPendingAction('create');
    setPinStep('create');
    setPinDigits(['', '', '', '']);
    setConfirmPinDigits(['', '', '', '']);
    setPinError(null);
    setPendingPin(null);
  };

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
      if (pendingAction === 'create') doCreateWallet(confirmPin);
      else if (pendingAction === 'recover') doRecoverWallet(confirmPin);
      else if (pendingAction === 'import') doImportBackup(confirmPin);
    }
  }, [pendingPin, pendingAction]);

  const doCreateWallet = async (pin: string) => {
    setPinStep(null);
    setCreating(true);
    try {
      const res = await fetch('http://127.0.0.1:31301/wallet/create', {
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

  const handleConfirmBackup = async () => {
    localStorage.setItem('hodos_wallet_exists', 'true');
    await cacheIdentityKey();

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
    setPendingAction('recover');
    setPinStep('create');
    setPinDigits(['', '', '', '']);
    setConfirmPinDigits(['', '', '', '']);
    setPinError(null);
    setPendingPin(null);
  };

  const doRecoverWallet = async (pin: string) => {
    setPinStep(null);
    setPendingAction(null);
    setPendingPin(null);

    const mnemonicPhrase = recoveryWords.map(w => w.trim().toLowerCase()).join(' ');
    setRecovering(true);
    setRecoveryError(null);

    try {
      const res = await fetch('http://127.0.0.1:31301/wallet/recover/onchain', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          mnemonic: mnemonicPhrase,
          pin,
        }),
      });

      const data = await res.json();

      if (res.status === 409) {
        setRecoveryError('A wallet already exists. Delete the existing wallet first.');
        setRecovering(false);
        return;
      }

      if (data.success && data.backup_found) {
        const restored = data.restored || {};
        setRecoveryResult({
          transactions: restored.transactions || 0,
          outputs: restored.outputs || 0,
          addresses: restored.addresses || 0,
          certificates: restored.certificates || 0,
          total_balance: data.balance_satoshis || 0,
          spendable_utxos: data.spendable_utxos || 0,
          fromBackup: true,
        });
      } else if (data.backup_found === false) {
        // No backup token — fall back to mnemonic-only recovery (gap-limit chain scan)
        try {
          const fallbackRes = await fetch('http://127.0.0.1:31301/wallet/recover', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ mnemonic: mnemonicPhrase, pin, confirm: true }),
          });
          const fb = await fallbackRes.json();
          if (fallbackRes.status === 409) {
            setRecoveryError('A wallet already exists. Delete the existing wallet first.');
          } else if (fb.success) {
            setRecoveryResult({
              transactions: 0,
              outputs: fb.utxos_found || 0,
              addresses: fb.addresses_found || 0,
              certificates: 0,
              total_balance: fb.total_balance || 0,
              spendable_utxos: fb.utxos_found || 0,
              fromBackup: false,
            });
          } else {
            setRecoveryError(fb.error || fb.message || 'Mnemonic recovery failed.');
          }
        } catch {
          setRecoveryError('Failed to connect to wallet server during chain scan fallback.');
        }
      } else {
        setRecoveryError(data.error || 'Recovery failed.');
      }
    } catch {
      setRecoveryError('Failed to connect to wallet server');
    }

    setRecovering(false);
  };

  const handleRecoveryComplete = async () => {
    localStorage.setItem('hodos_wallet_exists', 'true');
    // Clear stale balance cache so wallet panel fetches fresh balance from backend
    localStorage.removeItem('hodos:wallet:balance');
    // Await identity key cache so WalletPanel reads it from localStorage on mount
    await cacheIdentityKey();

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
      const res = await fetch('http://127.0.0.1:31301/wallet/import', {
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

  const handleImportComplete = async () => {
    localStorage.setItem('hodos_wallet_exists', 'true');
    await cacheIdentityKey();

    setImportResult(null);
    setShowImportForm(false);
    setImportPassword('');
    setImportBackupText('');
    setImportFile(null);
    setWalletStatus('exists');
  };

  // --- Centbee Recovery Flow ---

  const handleCentbeeWordChange = (index: number, value: string) => {
    const pastedWords = value.trim().split(/\s+/);
    if (pastedWords.length > 1) {
      const newWords = [...centbeeWords];
      for (let i = 0; i < 12; i++) {
        const wi = i - index;
        if (wi >= 0 && wi < pastedWords.length) {
          newWords[i] = pastedWords[wi].toLowerCase();
        }
      }
      setCentbeeWords(newWords);
      setCentbeeError(null);
      const lastFilled = Math.min(index + pastedWords.length - 1, 11);
      const el = document.getElementById(`centbee-word-${lastFilled}`);
      if (el) setTimeout(() => el.focus(), 0);
      return;
    }
    const newWords = [...centbeeWords];
    newWords[index] = value.toLowerCase().replace(/\s/g, '');
    setCentbeeWords(newWords);
    setCentbeeError(null);
  };

  const handleCentbeeWordKeyDown = (index: number, e: React.KeyboardEvent) => {
    if (e.key === ' ' || e.key === 'Tab') {
      if (index < 11) {
        e.preventDefault();
        const el = document.getElementById(`centbee-word-${index + 1}`);
        if (el) el.focus();
      }
    } else if (e.key === 'Backspace' && centbeeWords[index] === '' && index > 0) {
      e.preventDefault();
      const el = document.getElementById(`centbee-word-${index - 1}`);
      if (el) el.focus();
    }
  };

  const handleCentbeeRecover = async () => {
    // Validate words
    const filledWords = centbeeWords.map(w => w.trim().toLowerCase()).filter(w => w.length > 0);
    if (filledWords.length !== 12) {
      setCentbeeError(`Expected 12 words, got ${filledWords.length}. Fill in all boxes.`);
      return;
    }
    const emptyBox = centbeeWords.findIndex(w => w.trim() === '');
    if (emptyBox !== -1) {
      setCentbeeError(`Word ${emptyBox + 1} is empty.`);
      return;
    }

    // Validate PIN
    const pin = centbeePinDigits.join('');
    if (pin.length !== 4 || !/^\d{4}$/.test(pin)) {
      setCentbeeError('Enter your 4-digit Centbee PIN.');
      return;
    }

    const mnemonicPhrase = centbeeWords.map(w => w.trim().toLowerCase()).join(' ');
    setCentbeeRecovering(true);
    setCentbeeError(null);
    setCentbeeProgress('Scanning Centbee addresses for funds...');

    try {
      const res = await fetch('http://127.0.0.1:31301/wallet/recover-external', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          mnemonic: mnemonicPhrase,
          passphrase: pin,
          wallet_type: 'centbee',
          gap_limit: 25,
          confirm: true,
        }),
      });

      const data = await res.json();

      if (res.status === 409) {
        setCentbeeError('A wallet already exists. Delete the existing wallet first.');
        setCentbeeRecovering(false);
        setCentbeeProgress(null);
        return;
      }

      if (data.success) {
        setCentbeeResult({
          utxos_found: data.utxos_found || 0,
          total_balance: data.total_balance || 0,
          sweep_txids: data.sweep_txids || [],
          total_fees: data.total_fees || 0,
          brc42_balance: data.brc42_balance || 0,
          message: data.message || 'Migration complete!',
        });
      } else {
        setCentbeeError(data.error || 'No funds found — check your mnemonic and Centbee PIN.');
      }
    } catch {
      setCentbeeError('Failed to connect to wallet server');
    }

    setCentbeeRecovering(false);
    setCentbeeProgress(null);
  };

  const handleCentbeeComplete = async () => {
    localStorage.setItem('hodos_wallet_exists', 'true');
    await cacheIdentityKey();

    setCentbeeResult(null);
    setShowCentbeeRecovery(false);
    setCentbeeWords(Array(12).fill(''));
    setCentbeePinDigits(['', '', '', '']);
    setWalletStatus('exists');
  };

  // --- PIN creation/confirm screens (shared between create, recovery, import) ---

  const renderPinCreate = () => (
    <>
      <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F512;</div>
      <h3 style={{ margin: '0 0 8px', color: tokens.textPrimary, fontSize: '18px' }}>Create a PIN</h3>
      <p style={{ color: tokens.textSecondary, fontSize: '13px', margin: '0 0 24px' }}>
        Choose a 4-digit PIN to protect your wallet. You'll need this PIN to view your mnemonic or perform sensitive operations.
      </p>

      {pinError && (
        <p style={{ color: tokens.error, fontSize: '12px', margin: '0 0 12px', fontWeight: 600 }}>
          {pinError}
        </p>
      )}

      <PinInput key="pin-create" digits={pinDigits} onChange={handlePinDigitsChange} />

      <p style={{ color: tokens.textMuted, fontSize: '11px', margin: '16px 0 0', fontStyle: 'italic' }}>
        Your wallet will unlock automatically when you start the browser. The PIN is used to encrypt your keys and for sensitive operations.
      </p>

      <HodosButton
        variant="secondary"
        onClick={() => {
          setPinStep(null);
          setPendingAction(null);
          setPinDigits(['', '', '', '']);
          setPinError(null);
        }}
        style={{ width: '100%', marginTop: '20px' }}
      >
        Back
      </HodosButton>
    </>
  );

  const renderPinConfirm = () => (
    <>
      <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F512;</div>
      <h3 style={{ margin: '0 0 8px', color: tokens.textPrimary, fontSize: '18px' }}>Confirm your PIN</h3>
      <p style={{ color: tokens.textSecondary, fontSize: '13px', margin: '0 0 24px' }}>
        Enter the same 4-digit PIN again to confirm.
      </p>

      <PinInput key="pin-confirm" digits={confirmPinDigits} onChange={handleConfirmPinDigitsChange} />

      {pinError && (
        <p style={{ color: tokens.error, fontSize: '12px', margin: '12px 0 0', fontWeight: 600 }}>
          {pinError}
        </p>
      )}

      <HodosButton
        variant="secondary"
        onClick={() => {
          setPinStep('create');
          setPinDigits(['', '', '', '']);
          setConfirmPinDigits(['', '', '', '']);
          setPendingPin(null);
          setPinError(null);
        }}
        style={{ width: '100%', marginTop: '20px' }}
      >
        Back
      </HodosButton>
    </>
  );

  // --- Render functions ---

  const renderNoWallet = () => (
    <div style={{
      background: tokens.bgSurface,
      borderRadius: '12px',
      width: '380px',
      maxHeight: '80vh',
      overflow: 'auto',
      border: `2px solid ${tokens.gold}`,
      cursor: 'default',
      fontFamily: tokens.fontUi,
    }} onClick={e => { console.log('[WalletPanel] renderNoWallet container clicked'); e.stopPropagation(); }}>
      {/* Header */}
      <div style={{
        background: '#000000',
        color: tokens.textPrimary,
        padding: '12px 24px',
        borderRadius: '12px 12px 0 0',
        borderBottom: `2px solid ${tokens.gold}`,
        display: 'flex',
        alignItems: 'center',
        gap: '12px',
      }}>
        <img src="/Hodos_Gold_Wallet_Icon.svg" alt="Hodos Wallet" style={{ height: '28px', width: 'auto' }} />
      </div>

      {/* Content */}
      <div style={{
        padding: '32px 24px',
        textAlign: 'center',
        background: `radial-gradient(ellipse 80% 60% at 50% 50%, rgba(255, 255, 255, 0.06) 0%, transparent 70%), ${tokens.bgSurface}`,
      }}>
        {pinStep === 'create' ? renderPinCreate()
        : pinStep === 'confirm' ? renderPinConfirm()
        : recoveryResult ? (
          /* Recovery success */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x2705;</div>
            <h3 style={{ margin: '0 0 8px', color: tokens.textPrimary, fontSize: '18px' }}>Wallet Recovered</h3>
            <p style={{ color: tokens.textSecondary, fontSize: '13px', margin: '0 0 16px' }}>
              {recoveryResult.fromBackup
                ? 'Your wallet has been restored from on-chain backup.'
                : 'No backup token found. Wallet recovered by scanning the blockchain for self-derived addresses only. Counterparty-derived outputs and identity certificates cannot be recovered without a backup.'}
            </p>

            <div style={{
              background: tokens.bgElevated,
              border: `2px solid ${tokens.borderDefault}`,
              borderRadius: '8px',
              padding: '16px',
              marginBottom: '16px',
              textAlign: 'left',
            }}>
              <div style={{ fontSize: '13px', color: tokens.textPrimary, marginBottom: '8px' }}>
                <strong>Addresses:</strong> {recoveryResult.addresses}
              </div>
              <div style={{ fontSize: '13px', color: tokens.textPrimary, marginBottom: '8px' }}>
                <strong>Transactions:</strong> {recoveryResult.transactions}
              </div>
              <div style={{ fontSize: '13px', color: tokens.textPrimary, marginBottom: '8px' }}>
                <strong>Outputs:</strong> {recoveryResult.spendable_utxos} spendable
              </div>
              {recoveryResult.certificates > 0 && (
                <div style={{ fontSize: '13px', color: tokens.textPrimary, marginBottom: '8px' }}>
                  <strong>Tokens / Certificates:</strong> {recoveryResult.certificates}
                </div>
              )}
              <div style={{ fontSize: '13px', color: tokens.textPrimary }}>
                <strong>Balance:</strong> {(recoveryResult.total_balance / 100_000_000).toFixed(8)} BSV
                <span style={{ color: tokens.textMuted, marginLeft: '8px' }}>
                  ({recoveryResult.total_balance.toLocaleString()} sats)
                </span>
              </div>
            </div>

            <HodosButton
              variant="primary"
              onClick={handleRecoveryComplete}
              style={{ width: '100%' }}
            >
              Continue to Wallet
            </HodosButton>
          </>
        ) : showRecoveryInput ? (
          /* Recovery input form */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F50D;</div>
            <h3 style={{ margin: '0 0 8px', color: tokens.textPrimary, fontSize: '18px' }}>Recover Wallet</h3>
            <p style={{ color: tokens.textSecondary, fontSize: '13px', margin: '0 0 16px' }}>
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
                    color: tokens.textMuted,
                    pointerEvents: 'none',
                    fontFamily: tokens.fontMono,
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
                      border: `2px solid ${word ? tokens.gold : tokens.borderDefault}`,
                      background: tokens.bgElevated,
                      fontFamily: tokens.fontMono,
                      fontSize: '13px',
                      color: tokens.textPrimary,
                      boxSizing: 'border-box',
                      outline: 'none',
                      transition: 'border-color 0.15s',
                    }}
                    onFocus={e => { e.target.style.borderColor = tokens.gold; }}
                    onBlur={e => { e.target.style.borderColor = word ? tokens.gold : tokens.borderDefault; }}
                  />
                </div>
              ))}
            </div>

            {recoveryError && (
              <p style={{
                color: tokens.error,
                fontSize: '12px',
                margin: '0 0 12px',
                textAlign: 'left',
              }}>
                {recoveryError}
              </p>
            )}

            {recovering && (
              <div style={{
                background: tokens.bgElevated,
                border: `2px solid ${tokens.borderDefault}`,
                borderRadius: '8px',
                padding: '12px 16px',
                marginBottom: '12px',
                textAlign: 'left',
              }}>
                <p style={{ color: tokens.textPrimary, fontSize: '13px', fontWeight: 600, margin: '0 0 4px' }}>
                  Searching for on-chain backup...
                </p>
                <p style={{ color: tokens.textMuted, fontSize: '12px', margin: 0 }}>
                  Looking up your wallet backup on the blockchain and restoring data.
                  This may take several minutes depending on wallet size.
                </p>
              </div>
            )}

            <HodosButton
              variant="primary"
              onClick={handleStartRecover}
              disabled={recovering || recoveryWords.every(w => w.trim() === '')}
              loading={recovering}
              loadingText="Recovering..."
              style={{ width: '100%', marginBottom: '12px' }}
            >
              Recover Wallet
            </HodosButton>

            <HodosButton
              variant="secondary"
              onClick={() => {
                setShowRecoveryInput(false);
                setRecoveryWords(Array(12).fill(''));
                setRecoveryError(null);
              }}
              disabled={recovering}
              style={{ width: '100%' }}
            >
              Back
            </HodosButton>
          </>
        ) : importResult ? (
          /* Import success */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x2705;</div>
            <h3 style={{ margin: '0 0 8px', color: tokens.textPrimary, fontSize: '18px' }}>Wallet Restored</h3>
            <p style={{ color: tokens.textSecondary, fontSize: '13px', margin: '0 0 16px' }}>
              Your wallet has been restored from the backup file.
            </p>

            <div style={{
              background: tokens.bgElevated,
              border: `2px solid ${tokens.borderDefault}`,
              borderRadius: '8px',
              padding: '16px',
              marginBottom: '16px',
              textAlign: 'left',
            }}>
              <div style={{ fontSize: '13px', color: tokens.textPrimary, marginBottom: '8px' }}>
                <strong>Addresses:</strong> {importResult.addresses}
              </div>
              <div style={{ fontSize: '13px', color: tokens.textPrimary, marginBottom: '8px' }}>
                <strong>Transactions:</strong> {importResult.transactions}
              </div>
              <div style={{ fontSize: '13px', color: tokens.textPrimary, marginBottom: '8px' }}>
                <strong>Outputs:</strong> {importResult.outputs}
              </div>
              <div style={{ fontSize: '13px', color: tokens.textPrimary }}>
                <strong>Certificates:</strong> {importResult.certificates}
              </div>
            </div>

            <HodosButton
              variant="primary"
              onClick={handleImportComplete}
              style={{ width: '100%' }}
            >
              Continue to Wallet
            </HodosButton>
          </>
        ) : showImportForm ? (
          /* Import from backup form */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F4E5;</div>
            <h3 style={{ margin: '0 0 8px', color: tokens.textPrimary, fontSize: '18px' }}>Import from Backup</h3>
            <p style={{ color: tokens.textSecondary, fontSize: '13px', margin: '0 0 12px' }}>
              Select your .hodos-wallet backup file to restore your wallet.
            </p>

            {/* File input */}
            <div style={{
              background: tokens.bgElevated,
              border: `2px solid ${importFile ? tokens.gold : tokens.borderDefault}`,
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
                  color: tokens.textPrimary,
                  width: '100%',
                  cursor: 'pointer',
                }}
              />
              {importFile && (
                <p style={{ fontSize: '11px', color: tokens.gold, margin: '8px 0 0', fontWeight: 600 }}>
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
                border: `2px solid ${tokens.borderDefault}`,
                background: tokens.bgElevated,
                fontSize: '13px',
                color: tokens.textPrimary,
                boxSizing: 'border-box',
                marginBottom: importError ? '8px' : '12px',
                outline: 'none',
              }}
            />

            {importError && (
              <p style={{
                color: tokens.error,
                fontSize: '12px',
                margin: '0 0 12px',
                textAlign: 'left',
              }}>
                {importError}
              </p>
            )}

            {importing && (
              <div style={{
                background: tokens.bgElevated,
                border: `2px solid ${tokens.borderDefault}`,
                borderRadius: '8px',
                padding: '12px 16px',
                marginBottom: '12px',
                textAlign: 'left',
              }}>
                <p style={{ color: tokens.textPrimary, fontSize: '13px', fontWeight: 600, margin: 0 }}>
                  Importing wallet data...
                </p>
              </div>
            )}

            <HodosButton
              variant="primary"
              onClick={handleStartImport}
              disabled={importing || !importFile}
              loading={importing}
              loadingText="Importing..."
              style={{ width: '100%', marginBottom: '12px' }}
            >
              Import Wallet
            </HodosButton>

            <HodosButton
              variant="secondary"
              onClick={() => {
                setShowImportForm(false);
                setImportPassword('');
                setImportBackupText('');
                setImportFile(null);
                setImportError(null);
              }}
              disabled={importing}
              style={{ width: '100%' }}
            >
              Back
            </HodosButton>
          </>
        ) : centbeeResult ? (
          /* Centbee migration success */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x2705;</div>
            <h3 style={{ margin: '0 0 8px', color: tokens.textPrimary, fontSize: '18px' }}>Centbee Migration Complete</h3>
            <p style={{ color: tokens.textSecondary, fontSize: '13px', margin: '0 0 16px' }}>
              {centbeeResult.message}
            </p>

            <div style={{
              background: tokens.bgElevated,
              border: `2px solid ${tokens.borderDefault}`,
              borderRadius: '8px',
              padding: '16px',
              marginBottom: '16px',
              textAlign: 'left',
            }}>
              <div style={{ fontSize: '13px', color: tokens.textPrimary, marginBottom: '8px' }}>
                <strong>UTXOs found:</strong> {centbeeResult.utxos_found}
              </div>
              <div style={{ fontSize: '13px', color: tokens.textPrimary, marginBottom: '8px' }}>
                <strong>Original balance:</strong> {(centbeeResult.total_balance / 100_000_000).toFixed(8)} BSV
                <span style={{ color: tokens.textMuted, marginLeft: '8px' }}>
                  ({centbeeResult.total_balance.toLocaleString()} sats)
                </span>
              </div>
              <div style={{ fontSize: '13px', color: tokens.textPrimary, marginBottom: '8px' }}>
                <strong>Fees:</strong> {centbeeResult.total_fees.toLocaleString()} sats
              </div>
              <div style={{ fontSize: '13px', color: tokens.textPrimary }}>
                <strong>BRC-42 balance:</strong> {(centbeeResult.brc42_balance / 100_000_000).toFixed(8)} BSV
                <span style={{ color: tokens.textMuted, marginLeft: '8px' }}>
                  ({centbeeResult.brc42_balance.toLocaleString()} sats)
                </span>
              </div>
            </div>

            {/* Migration notice */}
            <div style={{
              background: 'rgba(166, 124, 0, 0.1)',
              border: '2px solid #f9a825',
              borderRadius: '8px',
              padding: '12px 16px',
              marginBottom: '16px',
              textAlign: 'left',
            }}>
              <p style={{ color: tokens.gold, fontSize: '12px', fontWeight: 700, margin: '0 0 6px' }}>
                Migration Notice
              </p>
              <p style={{ color: tokens.textSecondary, fontSize: '12px', margin: 0, lineHeight: '1.5' }}>
                Your wallet has been migrated to BRC-42 derivation. Your mnemonic is the same &mdash; only the address derivation scheme changed.
              </p>
            </div>

            <HodosButton
              variant="primary"
              onClick={handleCentbeeComplete}
              style={{ width: '100%' }}
            >
              Continue to Wallet
            </HodosButton>
          </>
        ) : showCentbeeRecovery ? (
          /* Centbee recovery form */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F4F1;</div>
            <h3 style={{ margin: '0 0 8px', color: tokens.textPrimary, fontSize: '18px' }}>Recover from Centbee</h3>
            <p style={{ color: tokens.textSecondary, fontSize: '13px', margin: '0 0 12px' }}>
              Enter your Centbee mnemonic and PIN. Your funds will be swept from Centbee addresses to BRC-42.
            </p>

            {/* 12-box mnemonic grid */}
            <div style={{
              display: 'grid',
              gridTemplateColumns: '1fr 1fr 1fr',
              gap: '8px',
              marginBottom: '12px',
            }}>
              {centbeeWords.map((word, i) => (
                <div key={i} style={{ position: 'relative' }}>
                  <span style={{
                    position: 'absolute',
                    left: '8px',
                    top: '50%',
                    transform: 'translateY(-50%)',
                    fontSize: '10px',
                    color: tokens.textMuted,
                    pointerEvents: 'none',
                    fontFamily: tokens.fontMono,
                  }}>
                    {i + 1}.
                  </span>
                  <input
                    id={`centbee-word-${i}`}
                    type="text"
                    value={word}
                    onChange={e => handleCentbeeWordChange(i, e.target.value)}
                    onKeyDown={e => handleCentbeeWordKeyDown(i, e)}
                    disabled={centbeeRecovering}
                    autoComplete="off"
                    spellCheck={false}
                    style={{
                      width: '100%',
                      padding: '10px 8px 10px 28px',
                      borderRadius: '6px',
                      border: `2px solid ${word ? tokens.gold : tokens.borderDefault}`,
                      background: tokens.bgElevated,
                      fontFamily: tokens.fontMono,
                      fontSize: '13px',
                      color: tokens.textPrimary,
                      boxSizing: 'border-box',
                      outline: 'none',
                      transition: 'border-color 0.15s',
                    }}
                    onFocus={e => { e.target.style.borderColor = tokens.gold; }}
                    onBlur={e => { e.target.style.borderColor = word ? tokens.gold : tokens.borderDefault; }}
                  />
                </div>
              ))}
            </div>

            {/* Centbee PIN */}
            <p style={{ color: tokens.textPrimary, fontSize: '13px', fontWeight: 600, margin: '0 0 8px', textAlign: 'left' }}>
              Centbee PIN
            </p>
            <PinInput digits={centbeePinDigits} onChange={d => { setCentbeePinDigits(d); setCentbeeError(null); }} disabled={centbeeRecovering} />
            <p style={{ color: tokens.textMuted, fontSize: '11px', margin: '8px 0 12px', fontStyle: 'italic' }}>
              This is the PIN you set in Centbee — it's needed to derive your addresses.
            </p>

            {centbeeError && (
              <p style={{
                color: tokens.error,
                fontSize: '12px',
                margin: '0 0 12px',
                textAlign: 'left',
              }}>
                {centbeeError}
              </p>
            )}

            {centbeeRecovering && centbeeProgress && (
              <div style={{
                background: tokens.bgElevated,
                border: `2px solid ${tokens.borderDefault}`,
                borderRadius: '8px',
                padding: '12px 16px',
                marginBottom: '12px',
                textAlign: 'left',
              }}>
                <p style={{ color: tokens.textPrimary, fontSize: '13px', fontWeight: 600, margin: '0 0 4px' }}>
                  {centbeeProgress}
                </p>
                <p style={{ color: tokens.textMuted, fontSize: '12px', margin: 0 }}>
                  This may take a minute. Scanning addresses and sweeping funds to your new wallet.
                </p>
              </div>
            )}

            <HodosButton
              variant="primary"
              onClick={handleCentbeeRecover}
              disabled={centbeeRecovering || centbeeWords.every(w => w.trim() === '')}
              loading={centbeeRecovering}
              loadingText="Recovering..."
              style={{ width: '100%', marginBottom: '12px' }}
            >
              Recover from Centbee
            </HodosButton>

            <HodosButton
              variant="secondary"
              onClick={() => {
                setShowCentbeeRecovery(false);
                setCentbeeWords(Array(12).fill(''));
                setCentbeePinDigits(['', '', '', '']);
                setCentbeeError(null);
              }}
              disabled={centbeeRecovering}
              style={{ width: '100%' }}
            >
              Back
            </HodosButton>
          </>
        ) : !mnemonic ? (
          /* Default: Create + Recover + Import + Centbee buttons */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x1F512;</div>
            <h3 style={{ margin: '0 0 8px', color: tokens.textPrimary, fontSize: '18px' }}>No Wallet Found</h3>
            <p style={{ color: tokens.textSecondary, fontSize: '14px', margin: '0 0 24px' }}>
              Create a new wallet to get started with Bitcoin SV.
            </p>

            <HodosButton
              variant="primary"
              onClick={handleStartCreate}
              onMouseDown={() => console.log('[WalletPanel] Create button onMouseDown fired')}
              onMouseEnter={() => console.log('[WalletPanel] Create button onMouseEnter fired')}
              disabled={creating}
              loading={creating}
              loadingText="Creating..."
              style={{ width: '100%', marginBottom: '12px' }}
            >
              Create New Wallet
            </HodosButton>

            <HodosButton
              variant="secondary"
              onClick={() => setShowRecoveryInput(true)}
              style={{ width: '100%', marginBottom: '12px' }}
            >
              Recover Hodos Wallet
            </HodosButton>

            <HodosButton
              variant="secondary"
              onClick={() => setShowCentbeeRecovery(true)}
              style={{ width: '100%', marginBottom: '12px' }}
            >
              Recover from Centbee
            </HodosButton>

            {/* Import from Backup File — hidden for now, backend still supports it */}
          </>
        ) : (
          /* Mnemonic backup (after create) */
          <>
            <div style={{ fontSize: '48px', marginBottom: '16px' }}>&#x26A0;&#xFE0F;</div>
            <h3 style={{ margin: '0 0 8px', color: tokens.textPrimary, fontSize: '18px' }}>Back Up Your Mnemonic</h3>

            <div style={{
              background: 'rgba(166, 124, 0, 0.15)',
              border: '2px solid #e65100',
              borderRadius: '8px',
              padding: '12px 16px',
              marginBottom: '16px',
              textAlign: 'left',
            }}>
              <p style={{ color: '#e65100', fontSize: '12px', fontWeight: 700, margin: '0 0 6px' }}>
                Your mnemonic is your private key
              </p>
              <ul style={{ color: tokens.textSecondary, fontSize: '12px', margin: 0, paddingLeft: '18px', lineHeight: '1.6' }}>
                <li><strong>Keep it secret.</strong> Anyone with these words can access your coins and identity.</li>
                <li><strong>Keep it safe.</strong> If you lose this mnemonic and something goes wrong, there is no way to recover your wallet.</li>
              </ul>
            </div>

            <p style={{ color: tokens.textSecondary, fontSize: '13px', margin: '0 0 12px' }}>
              Write down these 12 words in order and store them somewhere safe.
            </p>

            <div style={{
              background: tokens.bgElevated,
              border: `2px solid ${tokens.borderDefault}`,
              borderRadius: '8px',
              padding: '16px',
              marginBottom: '16px',
              fontFamily: tokens.fontMono,
              fontSize: '14px',
              lineHeight: '1.8',
              color: tokens.textPrimary,
              wordBreak: 'break-word',
              userSelect: 'text',
              textAlign: 'left',
            }}>
              {mnemonic.split(' ').map((word, i) => (
                <span key={i} style={{ display: 'inline-block', marginRight: '4px' }}>
                  <span style={{ color: tokens.textMuted, fontSize: '11px' }}>{i + 1}.</span> {word}
                  {i < 11 ? ' ' : ''}
                </span>
              ))}
            </div>

            <HodosButton
              variant="secondary"
              onClick={handleCopyMnemonic}
              style={{ width: '100%', marginBottom: '16px' }}
            >
              {copied ? 'Copied!' : 'Copy to Clipboard'}
            </HodosButton>

            <label style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: '8px',
              fontSize: '13px',
              color: tokens.textPrimary,
              marginBottom: '16px',
              cursor: 'pointer',
            }}>
              <input
                type="checkbox"
                checked={backedUp}
                onChange={e => setBackedUp(e.target.checked)}
                style={{ width: '16px', height: '16px', accentColor: tokens.gold }}
              />
              I have backed up my mnemonic
            </label>

            <HodosButton
              variant="primary"
              onClick={handleConfirmBackup}
              disabled={!backedUp}
              style={{ width: '100%' }}
            >
              Continue to Wallet
            </HodosButton>

            {/* Cancel — safe here because wallet was just created, no funds possible */}
            <HodosButton
              variant="ghost"
              onClick={() => {
                if (cancelling) return;
                setCancelling(true);
                // Wait for React to paint "Cancelling..." before sending IPC
                // (wallet_delete_cancel blocks C++ UI thread during WinHTTP call,
                //  which prevents CEF from compositing any paint updates)
                setTimeout(() => {
                  if (window.cefMessage?.send) {
                    window.cefMessage.send('wallet_delete_cancel', []);
                    window.cefMessage.send('wallet_allow_close', []);
                  }
                  localStorage.removeItem('hodos_wallet_exists');
                  handleClose();
                }, 150);
              }}
              disabled={cancelling}
              loading={cancelling}
              loadingText="Cancelling..."
              style={{ width: '100%', marginTop: '12px' }}
            >
              Cancel
            </HodosButton>
          </>
        )}
      </div>
    </div>
  );

  // Handle unlock (fallback when DPAPI fails)
  const handleUnlock = async () => {
    const pin = unlockPinDigits.join('');
    if (pin.length !== 4) return;
    setUnlocking(true);
    setUnlockError(null);
    try {
      const res = await fetch('http://127.0.0.1:31301/wallet/unlock', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pin }),
      });
      const data = await res.json();
      if (!res.ok) {
        setUnlockError(data.error || 'Unlock failed');
        setUnlockPinDigits(['', '', '', '']);
        setUnlocking(false);
        return;
      }
      cacheIdentityKey();
      setWalletStatus('exists');
    } catch (e: any) {
      setUnlockError(e.message || 'Connection failed');
      setUnlocking(false);
    }
  };

  // Auto-submit unlock when 4 digits entered
  useEffect(() => {
    if (walletStatus === 'locked' && unlockPinDigits.every(d => d !== '') && !unlocking) {
      handleUnlock();
    }
  }, [unlockPinDigits, walletStatus]);

  const renderLocked = () => (
    <div style={{
      background: tokens.bgSurface,
      borderRadius: '12px',
      width: '380px',
      padding: '32px 24px',
      textAlign: 'center',
      cursor: 'default',
      fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    }} onClick={e => e.stopPropagation()}>
      <div style={{ fontSize: '32px', marginBottom: '8px' }}>&#x1F512;</div>
      <h3 style={{ margin: '0 0 8px 0', fontSize: '18px', color: tokens.textPrimary }}>Wallet Locked</h3>
      <p style={{ color: tokens.textSecondary, fontSize: '13px', margin: '0 0 20px 0', lineHeight: '1.4' }}>
        Auto-unlock was unavailable. Enter your PIN to unlock.
      </p>
      <PinInput digits={unlockPinDigits} onChange={setUnlockPinDigits} disabled={unlocking} />
      {unlockError && (
        <p style={{ color: tokens.error, fontSize: '13px', marginTop: '12px' }}>{unlockError}</p>
      )}
      {unlocking && (
        <p style={{ color: tokens.textSecondary, fontSize: '13px', marginTop: '12px' }}>Unlocking...</p>
      )}
    </div>
  );

  const renderLoading = () => (
    <div style={{
      background: tokens.bgSurface,
      borderRadius: '12px',
      width: '380px',
      padding: '48px 24px',
      textAlign: 'center',
      cursor: 'default',
      fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    }} onClick={e => e.stopPropagation()}>
      <div style={{ fontSize: '32px', marginBottom: '16px', animation: 'spin 1s linear infinite' }}>&#x23F3;</div>
      <p style={{ color: tokens.textPrimary, fontSize: '14px', margin: 0 }}>Connecting to wallet...</p>
    </div>
  );

  return (
    <div
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        margin: 0,
        padding: 0,
        overflow: 'auto',
        boxSizing: 'border-box',
        backgroundColor: 'transparent',
      }}
    >
      {walletStatus === 'loading' && renderLoading()}
      {walletStatus === 'locked' && renderLocked()}
      {walletStatus === 'no-wallet' && renderNoWallet()}
      {walletStatus === 'exists' && <WalletPanel onClose={handleClose} />}
    </div>
  );
}
