import React, { useState, useMemo, useEffect, useRef, useCallback } from 'react';
import {
    Avatar,
    Box,
    Toolbar,
    Badge,
    Snackbar,
    Alert,
    CircularProgress,
} from '@mui/material';
import { HodosButton } from '../components/HodosButton';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import ArrowForwardIcon from '@mui/icons-material/ArrowForward';
import RefreshIcon from '@mui/icons-material/Refresh';
import AccountBalanceWalletIcon from '@mui/icons-material/AccountBalanceWallet';
import MoreVertIcon from '@mui/icons-material/MoreVert';
import AccountCircleIcon from '@mui/icons-material/AccountCircle';
import LockIcon from '@mui/icons-material/Lock';
import LockOpenIcon from '@mui/icons-material/LockOpen';
import ErrorOutlineIcon from '@mui/icons-material/ErrorOutline';
import DownloadIcon from '@mui/icons-material/Download';
import SecurityIcon from '@mui/icons-material/Security';
// Settings panel now rendered in separate overlay process
import { useHodosBrowser } from '../hooks/useHodosBrowser';
import { useTabManager } from '../hooks/useTabManager';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { useCookieBlocking } from '../hooks/useCookieBlocking';
import { useAdblock } from '../hooks/useAdblock';
import { useBackgroundBalancePoller } from '../hooks/useBackgroundBalancePoller';
import { useDownloads } from '../hooks/useDownloads';
import { useProfiles } from '../hooks/useProfiles';
import { TabBar } from '../components/TabBar';
import FindBar from '../components/FindBar';
import { isUrl, normalizeUrl, toSearchUrl } from '../utils/urlDetection';

// Map internal localhost URLs to friendly display names
function toDisplayUrl(url: string): string {
    const prefix = 'http://127.0.0.1:5137/';
    if (!url.startsWith(prefix)) return url;
    const path = url.slice(prefix.length);
    if (path === 'newtab') return 'hodos://newtab';
    if (path.startsWith('settings-page')) return 'hodos://settings';
    if (path === 'browser-data') return 'hodos://browser-data';
    if (path === 'wallet') return 'hodos://wallet';
    return url;
}

const MainBrowserView: React.FC = () => {
    // Address bar state
    const [address, setAddress] = useState('hodos://newtab');
    const [isEditingAddress, setIsEditingAddress] = useState(false);
    const [autocompleteText, setAutocompleteText] = useState<string>('');
    const [userTypedText, setUserTypedText] = useState('hodos://newtab');
    const addressInputRef = React.useRef<HTMLInputElement>(null);
    const justNavigatedRef = React.useRef(false);
    // Tracks navigation-in-progress to prevent tab sync from reverting address bar
    const pendingNavigationRef = React.useRef(false);
    const preNavTabUrlRef = React.useRef<string>('');
    // Suppress autocomplete after Backspace/Delete so it doesn't re-fill
    const suppressAutocompleteRef = React.useRef(false);
    // Debounce omnibox IPC so typing stays snappy
    const omniboxDebounceRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);

    // Search engine setting — fetched from C++ settings on mount
    const [searchEngine, setSearchEngine] = useState('duckduckgo');

    const { navigate, goBack, goForward, reload } = useHodosBrowser();

    // Keep localStorage balance/price cache warm for wallet overlay
    useBackgroundBalancePoller();

    // Keep wallet-exists cache in sync so the overlay opens instantly with correct state.
    // Also fetch and cache the identity key so the wallet panel has it immediately.
    // Runs once on mount — if wallet.db was deleted, clears the stale cache before
    // the user ever opens the wallet panel.
    useEffect(() => {
        fetch('http://127.0.0.1:31301/wallet/status')
            .then(r => r.json())
            .then(data => {
                if (data.exists) {
                    localStorage.setItem('hodos_wallet_exists', 'true');
                    // Fetch and cache identity key for wallet panel
                    fetch('http://127.0.0.1:31301/getPublicKey', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ identityKey: true }),
                    })
                        .then(r => r.json())
                        .then(keyData => {
                            if (keyData.publicKey) {
                                localStorage.setItem('hodos_identity_key', keyData.publicKey);
                            }
                        })
                        .catch(() => {});
                } else {
                    localStorage.removeItem('hodos_wallet_exists');
                    localStorage.removeItem('hodos_identity_key');
                }
                localStorage.removeItem('hodos_wallet_locked');
            })
            .catch(() => {
                // Server not reachable — leave cache as-is (overlay will retry)
            });
    }, []);

    // Load search engine setting from C++ backend
    useEffect(() => {
        const prevHandler = window.onSettingsResponse;
        window.onSettingsResponse = (data: { browser?: { searchEngine?: string } }) => {
            if (data?.browser?.searchEngine) {
                setSearchEngine(data.browser.searchEngine);
            }
            // Restore previous handler if any (settings overlay also uses this)
            if (prevHandler) prevHandler(data as any);
        };
        if (window.cefMessage?.send) {
            window.cefMessage.send('settings_get_all');
        }
        return () => {
            window.onSettingsResponse = prevHandler;
        };
    }, []);

    // Tab management
    const {
        tabs,
        activeTabId,
        isLoading,
        createTab,
        closeTab,
        switchToTab,
        nextTab,
        prevTab,
        switchToTabByIndex,
        closeActiveTab,
        reorderTabs,
        tearOffTab,
    } = useTabManager();

    // Cookie blocking (badge count + polling)
    const {
        blockedCount,
        fetchBlockedCount,
        resetBlockedCount,
    } = useCookieBlocking();

    // Ad blocking (badge count + site check)
    const {
        blockedCount: adblockBlockedCount,
        adblockEnabled,
        resetBlockedCount: resetAdblockCount,
        checkSiteAdblock,
    } = useAdblock();

    // Current profile info for avatar display
    const { currentProfile } = useProfiles();

    // Shield badge — show dot only when count is actively increasing, fade after settling
    const totalBlocked = adblockBlockedCount + blockedCount;
    const [shieldDotVisible, setShieldDotVisible] = useState(false);
    const prevTotalRef = useRef(0);
    const shieldTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    useEffect(() => {
        if (totalBlocked > prevTotalRef.current && totalBlocked > 0) {
            // Count increased — show dot
            setShieldDotVisible(true);
            // Reset hide timer
            if (shieldTimerRef.current) clearTimeout(shieldTimerRef.current);
            shieldTimerRef.current = setTimeout(() => {
                setShieldDotVisible(false);
            }, 5000);
        } else if (totalBlocked === 0) {
            // Counts reset (navigation) — hide immediately
            setShieldDotVisible(false);
            if (shieldTimerRef.current) {
                clearTimeout(shieldTimerRef.current);
                shieldTimerRef.current = null;
            }
        }
        prevTotalRef.current = totalBlocked;
    }, [totalBlocked]);

    // Downloads — only need icon visibility state; overlay handles controls
    const { downloads, hasDownloads, hasActiveDownloads } = useDownloads();

    // PeerPay notification state — green dot on wallet button
    const [hasUnreadPayments, setHasUnreadPayments] = useState(false);
    const [unreadPaymentCount, setUnreadPaymentCount] = useState(0);
    const [unreadPaymentAmount, setUnreadPaymentAmount] = useState(0);

    // Poll /wallet/peerpay/status every 60s + listen for IPC dismiss events
    useEffect(() => {
        const checkPeerpayStatus = async () => {
            try {
                const resp = await fetch('http://127.0.0.1:31301/wallet/peerpay/status');
                if (resp.ok) {
                    const data = await resp.json();
                    if (data.unread_count > 0) {
                        setHasUnreadPayments(true);
                        setUnreadPaymentCount(data.unread_count);
                        setUnreadPaymentAmount(data.unread_amount || 0);
                    } else {
                        setHasUnreadPayments(false);
                        setUnreadPaymentCount(0);
                        setUnreadPaymentAmount(0);
                    }
                }
            } catch {
                // Wallet server not running — ignore
            }
        };
        // Initial check after short delay (let wallet server start)
        const initialTimer = setTimeout(checkPeerpayStatus, 5000);
        // Periodic check every 60s
        const interval = setInterval(checkPeerpayStatus, 60000);

        const handlePaymentDismissed = (event: MessageEvent) => {
            if (event.data?.type === 'wallet_payment_dismissed') {
                setHasUnreadPayments(false);
                setUnreadPaymentCount(0);
                setUnreadPaymentAmount(0);
            }
        };
        window.addEventListener('message', handlePaymentDismissed);
        return () => {
            clearTimeout(initialTimer);
            clearInterval(interval);
            window.removeEventListener('message', handlePaymentDismissed);
        };
    }, []);

    // Track download IDs to detect new starts and completions for toast notifications
    const prevDownloadIdsRef = useRef<Set<number>>(new Set());
    const prevCompleteIdsRef = useRef<Set<number>>(new Set());
    const [downloadToast, setDownloadToast] = useState<{ open: boolean; message: string; severity: 'info' | 'success' }>({
        open: false, message: '', severity: 'info'
    });

    // Find-in-page state
    const [findBarVisible, setFindBarVisible] = useState(false);
    const [findResult, setFindResult] = useState<{ count: number; activeMatch: number } | null>(null);

    // Listen for find_show and find_result IPC events
    useEffect(() => {
        const handleMessage = (event: MessageEvent) => {
            if (event.data?.type === 'find_show') {
                setFindBarVisible(true);
                setFindResult(null);
            } else if (event.data?.type === 'focus_address_bar') {
                if (addressInputRef.current) {
                    addressInputRef.current.focus();
                    addressInputRef.current.select();
                }
            } else if (event.data?.type === 'find_result') {
                try {
                    const data = typeof event.data.data === 'string'
                        ? JSON.parse(event.data.data)
                        : event.data.data;
                    // Update on every result (not just finalUpdate) for responsiveness
                    setFindResult({
                        count: data.count,
                        activeMatch: data.activeMatchOrdinal,
                    });
                } catch {
                    // Ignore parse errors
                }
            }
        };
        window.addEventListener('message', handleMessage);
        return () => window.removeEventListener('message', handleMessage);
    }, []);

    const handleFindBarClose = useCallback(() => {
        setFindBarVisible(false);
        setFindResult(null);
    }, []);


    useEffect(() => {
        const currentIds = new Set(downloads.map(d => d.id));
        const currentCompleteIds = new Set(downloads.filter(d => d.isComplete).map(d => d.id));

        // Detect new downloads starting
        for (const dl of downloads) {
            if (!prevDownloadIdsRef.current.has(dl.id) && !dl.isComplete && !dl.isCanceled) {
                const name = dl.filename || 'File';
                setDownloadToast({ open: true, message: `Downloading "${name}"`, severity: 'info' });
                break; // Only toast for the first new one
            }
        }

        // Detect downloads completing
        for (const dl of downloads) {
            if (dl.isComplete && !prevCompleteIdsRef.current.has(dl.id) && prevDownloadIdsRef.current.has(dl.id)) {
                const name = dl.filename || 'File';
                setDownloadToast({ open: true, message: `Download complete: "${name}"`, severity: 'success' });
                break;
            }
        }

        prevDownloadIdsRef.current = currentIds;
        prevCompleteIdsRef.current = currentCompleteIds;
    }, [downloads]);

    // Compute overall download progress for icon indicator
    const downloadProgress = useMemo(() => {
        const active = downloads.filter(d => d.isInProgress || d.isPaused);
        if (active.length === 0) return null; // no active downloads
        const totalBytes = active.reduce((sum, d) => sum + d.totalBytes, 0);
        const receivedBytes = active.reduce((sum, d) => sum + d.receivedBytes, 0);
        if (totalBytes <= 0) return -1; // indeterminate
        return Math.round((receivedBytes / totalBytes) * 100);
    }, [downloads]);

    const allComplete = useMemo(
        () => hasDownloads && downloads.every(d => d.isComplete || d.isCanceled),
        [hasDownloads, downloads]
    );

    // Toast state (for misc actions)
    const [toastOpen, setToastOpen] = useState(false);
    const [toastMessage, _setToastMessage] = useState('');

    // Extract current domain from address bar
    const currentDomain = useMemo(() => {
        try {
            const url = new URL(address);
            if (url.protocol === 'http:' || url.protocol === 'https:') {
                return url.hostname;
            }
            return '';
        } catch {
            return '';
        }
    }, [address]);

    // Check adblock site toggle when domain changes
    useEffect(() => {
        if (currentDomain) {
            checkSiteAdblock(currentDomain);
        }
    }, [currentDomain, checkSiteAdblock]);

    // Derive security state from active tab
    const activeTab = useMemo(() => tabs.find(t => t.id === activeTabId), [tabs, activeTabId]);

    const securityState = useMemo((): 'secure' | 'insecure' | 'error' | 'none' => {
        if (activeTab?.hasCertError) return 'error';
        try {
            const url = new URL(address);
            if (url.hostname === '127.0.0.1' || url.hostname === 'localhost' ||
                url.protocol === 'about:' || url.protocol === 'data:') {
                return 'none';
            }
            if (url.protocol === 'https:') return 'secure';
            if (url.protocol === 'http:') return 'insecure';
        } catch {
            // Invalid URL
        }
        return 'none';
    }, [address, activeTab?.hasCertError]);

    // Poll for blocked count + site adblock status every 3 seconds
    React.useEffect(() => {
        fetchBlockedCount();
        if (currentDomain) checkSiteAdblock(currentDomain);
        const interval = setInterval(() => {
            fetchBlockedCount();
            if (currentDomain) checkSiteAdblock(currentDomain);
        }, 10000);
        return () => clearInterval(interval);
    }, [fetchBlockedCount, checkSiteAdblock, currentDomain]);

    // Sync address bar with active tab's URL
    React.useEffect(() => {
        // Only update if user is not currently editing the address bar
        if (!isEditingAddress) {
            const activeTab = tabs.find(t => t.id === activeTabId);
            if (activeTab && activeTab.url) {
                if (pendingNavigationRef.current) {
                    if (activeTab.url === preNavTabUrlRef.current) {
                        // Tab URL hasn't changed yet — skip to avoid flicker
                        return;
                    }
                    // Tab URL changed — navigation complete
                    pendingNavigationRef.current = false;
                    preNavTabUrlRef.current = '';
                }
                setAddress(toDisplayUrl(activeTab.url));
            }
        }
    }, [activeTabId, tabs, isEditingAddress]);

    // Reset blocked counts on navigation (when active tab URL changes)
    React.useEffect(() => {
        const activeTab = tabs.find(t => t.id === activeTabId);
        if (activeTab?.url) {
            resetBlockedCount().catch(() => {});
            resetAdblockCount().catch(() => {});
        }
    }, [activeTabId, tabs, resetBlockedCount, resetAdblockCount]);

    // Listen for autocomplete suggestions from omnibox overlay
    // Only used for arrow-key selection in the dropdown — no inline autofill
    React.useEffect(() => {
        const handleAutocomplete = (event: MessageEvent) => {
            if (event.data?.type === 'omnibox_autocomplete') {
                const suggestion = event.data.suggestion;
                if (suggestion && isEditingAddress) {
                    // Arrow-key selected item — show it in the address bar
                    setAutocompleteText('');
                    setAddress(suggestion);
                } else if (!suggestion && isEditingAddress) {
                    // Arrow back to -1 — revert to typed text
                    setAutocompleteText('');
                    setAddress(userTypedText);
                } else {
                    setAutocompleteText('');
                }
            }
        };

        window.addEventListener('message', handleAutocomplete);
        return () => window.removeEventListener('message', handleAutocomplete);
    }, [userTypedText, isEditingAddress]);

    // Keyboard shortcuts
    useKeyboardShortcuts({
        onNewTab: createTab,
        onCloseTab: closeActiveTab,
        onNextTab: nextTab,
        onPrevTab: prevTab,
        onSwitchToTab: switchToTabByIndex,
        onFocusAddressBar: () => {
            if (addressInputRef.current) {
                addressInputRef.current.focus();
                addressInputRef.current.select();
            }
        },
        onReload: reload,
        onFindInPage: () => {
            setFindBarVisible(true);
            setFindResult(null);
        },
    });

    const handleNavigate = (input: string) => {
        // Handle hodos:// internal URLs
        if (input.startsWith('hodos://')) {
            const page = input.slice('hodos://'.length);
            if (page === 'settings') {
                navigate('http://127.0.0.1:5137/settings-page');
            } else if (page === 'browser-data') {
                navigate('http://127.0.0.1:5137/browser-data');
            } else if (page === 'wallet') {
                navigate('http://127.0.0.1:5137/wallet');
            } else {
                navigate(`http://127.0.0.1:5137/${page}`);
            }
            return;
        }
        // Detect if input is a URL or search query
        if (isUrl(input)) {
            // It's a URL - normalize and navigate
            const url = normalizeUrl(input);
            navigate(url);
        } else {
            // It's a search query - use configured search engine
            const searchUrl = toSearchUrl(input, searchEngine);
            navigate(searchUrl);
        }
    };

    return (
        <Box
            sx={{
                position: 'relative',
                width: 'calc(100% + 16px)',
                height: 'calc(100% + 16px)',
                display: 'flex',
                flexDirection: 'column',
                overflow: 'hidden',
                margin: '-8px',
                padding: 0,
                bgcolor: '#0f1117',
            }}
        >
            {/* Tab Bar */}
            <TabBar
                tabs={tabs}
                activeTabId={activeTabId}
                isLoading={isLoading}
                onCreateTab={createTab}
                onCloseTab={closeTab}
                onSwitchTab={switchToTab}
                onReorderTabs={reorderTabs}
                onTearOff={tearOffTab}
            />

            {/* Top Navigation Bar */}
            <Toolbar sx={{
                bgcolor: '#111827',
                borderBottom: '1px solid rgba(255, 255, 255, 0.05)',
                borderRadius: '0',
                minHeight: '53px !important',
                height: '53px',
                flexShrink: 0,
                px: 1,
                py: 0,
                margin: 0,
                gap: 0.75,
                overflow: 'hidden', // Prevent scrolling
            }}>
                {/* Back Button */}
                <HodosButton
                    variant="icon"
                    size="small"
                    onClick={goBack}
                    aria-label="Back"
                    style={{ flexShrink: 0 }}
                >
                    <ArrowBackIcon fontSize="small" />
                </HodosButton>

                {/* Forward Button */}
                <HodosButton
                    variant="icon"
                    size="small"
                    onClick={goForward}
                    aria-label="Forward"
                    style={{ flexShrink: 0 }}
                >
                    <ArrowForwardIcon fontSize="small" />
                </HodosButton>

                {/* Refresh Button */}
                <HodosButton
                    variant="icon"
                    size="small"
                    onClick={reload}
                    aria-label="Refresh"
                    style={{ flexShrink: 0 }}
                >
                    <RefreshIcon fontSize="small" />
                </HodosButton>

                {/* Spacer to help center address bar */}
                <Box sx={{ flex: 1 }} />

                {/* Address Bar with Inline Autocomplete - centered, constrained width */}
                <Box sx={{ position: 'relative', flex: '0 1 1200px', minWidth: 200, maxWidth: 1200 }}>
                    {securityState !== 'none' && (
                        <Box
                            sx={{
                                position: 'absolute',
                                left: 10,
                                top: '50%',
                                transform: 'translateY(-50%)',
                                display: 'flex',
                                alignItems: 'center',
                                zIndex: 1,
                                pointerEvents: 'none',
                            }}
                        >
                            {securityState === 'secure' && (
                                <LockIcon sx={{ fontSize: 16, color: '#188038' }} />
                            )}
                            {securityState === 'insecure' && (
                                <LockOpenIcon sx={{ fontSize: 16, color: '#6b7280' }} />
                            )}
                            {securityState === 'error' && (
                                <ErrorOutlineIcon sx={{ fontSize: 16, color: '#d93025' }} />
                            )}
                        </Box>
                    )}
                    <input
                        ref={addressInputRef}
                        type="text"
                        value={address}
                        onChange={(e) => {
                            const newValue = e.target.value;

                            // Update both the display value and the user-typed portion
                            setAddress(newValue);
                            setUserTypedText(newValue);
                            setIsEditingAddress(true);
                            setAutocompleteText(''); // Clear autocomplete on input change

                            // Debounce omnibox IPC so typing stays responsive
                            if (omniboxDebounceRef.current) clearTimeout(omniboxDebounceRef.current);
                            if (newValue.length > 0) {
                                omniboxDebounceRef.current = setTimeout(() => {
                                    window.cefMessage?.send('omnibox_update_query', [newValue]);
                                    window.cefMessage?.send('omnibox_show', [newValue]);
                                }, 150);
                            } else {
                                window.cefMessage?.send('omnibox_hide', []);
                            }
                        }}
                        onKeyDown={(e) => {
                            if (e.key === 'Backspace' || e.key === 'Delete') {
                                // Suppress autocomplete re-fill after deletion
                                suppressAutocompleteRef.current = true;
                                setAutocompleteText('');
                            } else if (e.key !== 'Shift' && e.key !== 'Control' && e.key !== 'Alt' && e.key !== 'Meta'
                                && e.key !== 'ArrowDown' && e.key !== 'ArrowUp') {
                                // Any non-modifier, non-arrow key clears the suppression
                                suppressAutocompleteRef.current = false;
                            }
                            if (e.key === 'ArrowDown') {
                                e.preventDefault();
                                window.cefMessage?.send('omnibox_select', 'down');
                            } else if (e.key === 'ArrowUp') {
                                e.preventDefault();
                                window.cefMessage?.send('omnibox_select', 'up');
                            } else if (e.key === 'Enter') {
                                const navigatedAddress = address;
                                // Snapshot old tab URL so tab sync can suppress stale updates
                                const activeTab = tabs.find(t => t.id === activeTabId);
                                preNavTabUrlRef.current = activeTab?.url || '';
                                pendingNavigationRef.current = true;
                                handleNavigate(navigatedAddress);
                                setIsEditingAddress(false);
                                setAutocompleteText('');
                                setUserTypedText(navigatedAddress);
                                // Set ref so onBlur knows not to revert the address
                                justNavigatedRef.current = true;
                                e.currentTarget.blur();
                                window.cefMessage?.send('omnibox_hide', []);
                            } else if (e.key === 'Escape') {
                                // Escape dismisses overlay, keeps current input
                                window.cefMessage?.send('omnibox_hide', []);
                                setIsEditingAddress(false);
                                setAutocompleteText('');
                                setAddress(userTypedText);
                                e.currentTarget.blur();
                            } else if ((e.key === 'Tab' || e.key === 'ArrowRight' || e.key === 'End') && autocompleteText) {
                                // Tab, Right arrow, or End accepts the autocomplete suggestion
                                e.preventDefault();
                                setUserTypedText(address);
                                setAutocompleteText('');
                                // Move cursor to end
                                setTimeout(() => {
                                    if (addressInputRef.current) {
                                        addressInputRef.current.setSelectionRange(address.length, address.length);
                                    }
                                }, 0);
                            }
                        }}
                        onFocus={(e) => {
                            e.target.select();
                            setIsEditingAddress(true);
                            // Preemptive creation: create overlay subprocess on focus but don't show
                            // Only shows when user types (see onChange handler)
                            window.cefMessage?.send('omnibox_create', []);
                        }}
                        onBlur={() => {
                            setIsEditingAddress(false);
                            setAutocompleteText('');
                            // Don't revert address if we just navigated (Enter was pressed)
                            if (justNavigatedRef.current) {
                                justNavigatedRef.current = false;
                            } else {
                                setAddress(userTypedText);
                            }
                        }}
                        placeholder="Search or enter address"
                        autoComplete="off"
                        autoCorrect="off"
                        autoCapitalize="off"
                        spellCheck={false}
                        style={{
                            width: '100%',
                            boxSizing: 'border-box',
                            height: 36,
                            borderRadius: 20,
                            paddingLeft: securityState !== 'none' ? 30 : 16,
                            paddingRight: 110,
                            backgroundColor: '#1a1d23',
                            border: '1px solid #2a2d35',
                            fontSize: 14,
                            color: '#f0f0f0',
                            outline: 'none',
                        }}
                    />
                    {/* Hodos icon + shield icon inside address bar */}
                    <Box
                        sx={{
                            position: 'absolute',
                            right: 2,
                            top: '50%',
                            transform: 'translateY(-50%)',
                            display: 'flex',
                            alignItems: 'center',
                            gap: 0.5,
                            zIndex: 1,
                        }}
                    >
                        <svg viewBox="0 0 167 54" xmlns="http://www.w3.org/2000/svg" style={{ height: 22, width: 'auto', marginRight: 10 }}>
                            <defs>
                                <linearGradient id="ab_lg" x1="32.82" y1="13.97" x2="18.73" y2="10.74" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
                                <linearGradient id="ab_lg1" x1="40.33" y1="21.9" x2="32.65" y2="9.65" xlinkHref="#ab_lg"/>
                                <linearGradient id="ab_lg2" x1="40.03" y1="32.82" x2="43.26" y2="18.73" xlinkHref="#ab_lg"/>
                                <linearGradient id="ab_lg3" x1="32.1" y1="40.33" x2="44.35" y2="32.65" xlinkHref="#ab_lg"/>
                                <linearGradient id="ab_lg4" x1="21.18" y1="40.03" x2="35.27" y2="43.26" xlinkHref="#ab_lg"/>
                                <linearGradient id="ab_lg5" x1="13.67" y1="32.1" x2="21.35" y2="44.35" xlinkHref="#ab_lg"/>
                                <linearGradient id="ab_lg6" x1="13.97" y1="21.18" x2="10.74" y2="35.27" xlinkHref="#ab_lg"/>
                                <linearGradient id="ab_lg7" x1="21.9" y1="13.66" x2="9.65" y2="21.35" xlinkHref="#ab_lg"/>
                            </defs>
                            <g>
                                <g>
                                    <path fill="#dfbd69" d="M67.66,46.01h-4.29v-10.08h4.11c2.5,0,3.52,1.09,3.52,2.69,0,1.17-.83,1.97-1.9,2.16,1.23.21,2.19,1.02,2.19,2.46,0,1.75-1.28,2.77-3.63,2.77ZM64.72,40.28h2.82c1.46,0,2.08-.62,2.08-1.6s-.62-1.62-2.08-1.62h-2.82v3.22ZM64.72,41.35v3.52h2.91c1.5,0,2.29-.66,2.29-1.74s-.78-1.78-2.29-1.78h-2.91Z"/>
                                    <path fill="#dfbd69" d="M85.39,46.01c-.19-.27-.34-1.01-.42-2.19-.06-1.12-.59-1.87-1.97-1.87h-2.85v4.07h-1.36v-10.08h4.1c2.56,0,3.79,1.18,3.79,2.98,0,1.55-1.06,2.37-2.29,2.51,1.23.24,1.82.99,1.92,2.18.13,1.47.18,2.05.51,2.42h-1.44ZM82.81,40.84c1.71,0,2.48-.64,2.48-1.89,0-1.15-.77-1.89-2.48-1.89h-2.66v3.78h2.66Z"/>
                                    <path fill="#dfbd69" d="M98.74,46.2c-2.95,0-5.01-2.13-5.01-5.23s2.06-5.23,5.01-5.23,5.01,2.13,5.01,5.23-2.08,5.23-5.01,5.23ZM98.74,36.91c-2.18,0-3.59,1.58-3.59,4.05s1.41,4.05,3.59,4.05,3.57-1.58,3.57-4.05-1.39-4.05-3.57-4.05Z"/>
                                    <path fill="#dfbd69" d="M119.3,44.31h.06l2.06-8.39h1.3l-2.58,10.08h-1.62l-2.1-8.05h-.06l-2.15,8.05h-1.6l-2.58-10.08h1.34l2.11,8.36h.06l2.19-8.36h1.3l2.24,8.39Z"/>
                                    <path fill="#dfbd69" d="M130.27,42.32c.08,1.84,1.34,2.79,3.04,2.79,1.5,0,2.45-.64,2.45-1.74,0-.93-.61-1.38-1.92-1.63l-2-.38c-1.49-.29-2.51-1.09-2.51-2.66,0-1.76,1.39-2.96,3.6-2.96,2.53,0,3.99,1.34,4,3.62l-1.26.06c-.05-1.67-1.06-2.61-2.72-2.61-1.46,0-2.27.69-2.27,1.81,0,.99.66,1.31,1.82,1.54l1.83.35c1.84.35,2.77,1.15,2.77,2.77,0,1.86-1.6,2.93-3.78,2.93-2.48,0-4.29-1.36-4.29-3.79l1.25-.08Z"/>
                                    <path fill="#dfbd69" d="M151.78,46.01h-7.19v-10.08h7.04v1.15h-5.7v3.17h4.53v1.14h-4.53v3.47h5.84v1.15Z"/>
                                    <path fill="#dfbd69" d="M165.56,46.01c-.19-.27-.34-1.01-.42-2.19-.06-1.12-.59-1.87-1.97-1.87h-2.85v4.07h-1.36v-10.08h4.1c2.56,0,3.79,1.18,3.79,2.98,0,1.55-1.06,2.37-2.29,2.51,1.23.24,1.82.99,1.92,2.18.13,1.47.18,2.05.51,2.42h-1.44ZM162.98,40.84c1.71,0,2.48-.64,2.48-1.89,0-1.15-.77-1.89-2.48-1.89h-2.66v3.78h2.66Z"/>
                                </g>
                                <g>
                                    <path fill="#a67c00" d="M63.17,27.98V8.18h4.78v7.83h8.64v-7.83h4.78v19.8h-4.78v-8.11h-8.64v8.11h-4.78Z"/>
                                    <path fill="#a67c00" d="M94.39,28.36c-5.75,0-10.15-4.18-10.15-10.28s4.4-10.28,10.15-10.28,10.18,4.18,10.18,10.28-4.4,10.28-10.18,10.28ZM94.39,11.98c-3.21,0-5.25,2.42-5.25,6.1s2.04,6.1,5.25,6.1,5.28-2.42,5.28-6.1-2.04-6.1-5.28-6.1Z"/>
                                    <path fill="#a67c00" d="M107.44,8.18h7.67c6.51,0,10.53,3.71,10.53,9.9s-4.02,9.9-10.53,9.9h-7.67V8.18ZM114.79,24.21c3.74,0,5.91-2.26,5.91-6.16s-2.17-6.1-5.91-6.1h-2.58v12.26h2.58Z"/>
                                    <path fill="#a67c00" d="M137.7,28.36c-5.75,0-10.15-4.18-10.15-10.28s4.4-10.28,10.15-10.28,10.18,4.18,10.18,10.28-4.4,10.28-10.18,10.28ZM137.7,11.98c-3.21,0-5.25,2.42-5.25,6.1s2.04,6.1,5.25,6.1,5.28-2.42,5.28-6.1-2.04-6.1-5.28-6.1Z"/>
                                    <path fill="#a67c00" d="M154.04,20.78c.19,2.74,2.23,3.83,4.68,3.83,2.11,0,3.43-.82,3.43-2.14s-1.16-1.63-2.89-1.98l-3.71-.66c-3.14-.6-5.38-2.36-5.38-5.72,0-3.9,3.05-6.32,7.86-6.32,5.38,0,8.3,2.67,8.39,7.07l-4.4.13c-.13-2.33-1.73-3.46-4.02-3.46-2.01,0-3.14.82-3.14,2.17,0,1.13.88,1.54,2.33,1.82l3.71.66c4.05.72,5.91,2.73,5.91,5.97,0,4.09-3.55,6.19-8.08,6.19-5.28,0-9.05-2.61-9.05-7.42l4.37-.16Z"/>
                                </g>
                            </g>
                            <g>
                                <path fill="url(#ab_lg)" d="M17.56,23.03c1.02-2.43,2.97-4.46,5.58-5.51,3.22-4.22,7.09-6.68,10.73-8.1C31.49,3.48,26.62,0,26.62,0c0,0-4.46,3.47-7.2,9.71-1.57,3.57-2.57,8.05-1.86,13.32Z"/>
                                <path fill="url(#ab_lg1)" d="M23.14,17.51c.15-.06.3-.13.46-.19,2.5-.88,5.1-.72,7.37.24,5.26-.71,9.75.29,13.32,1.86,2.52-5.88,1.54-11.78,1.54-11.78,0,0-5.6-.7-11.96,1.78-3.63,1.42-7.51,3.88-10.73,8.1Z"/>
                                <path fill="url(#ab_lg2)" d="M54,26.62s-3.47-4.46-9.71-7.2c-3.57-1.57-8.06-2.57-13.32-1.86,2.43,1.02,4.45,2.97,5.51,5.57,4.22,3.22,6.69,7.1,8.1,10.73,5.94-2.38,9.42-7.24,9.42-7.24Z"/>
                                <path fill="url(#ab_lg3)" d="M36.48,23.14c.06.16.13.31.19.47.85,2.42.76,5.02-.24,7.37.71,5.26-.29,9.74-1.86,13.31,5.88,2.52,11.78,1.54,11.78,1.54,0,0,.7-5.6-1.78-11.96-1.42-3.63-3.88-7.51-8.1-10.73Z"/>
                                <path fill="url(#ab_lg4)" d="M36.44,30.98c-.07.15-.12.31-.2.46-1.11,2.32-3.02,4.09-5.38,5.05-3.22,4.22-7.09,6.68-10.73,8.1,2.38,5.94,7.24,9.42,7.24,9.42,0,0,4.46-3.47,7.2-9.71,1.57-3.57,2.57-8.05,1.86-13.31Z"/>
                                <path fill="url(#ab_lg5)" d="M30.86,36.49c-.16.06-.31.13-.47.19-1.12.39-2.26.58-3.39.58-1.39,0-2.74-.29-3.99-.82-5.26.71-9.74-.29-13.31-1.86-2.52,5.88-1.54,11.78-1.54,11.78,0,0,5.6.7,11.96-1.78,3.63-1.42,7.51-3.88,10.73-8.1Z"/>
                                <path fill="url(#ab_lg6)" d="M23.02,36.44c-2.43-1.03-4.46-2.98-5.51-5.58-4.22-3.22-6.67-7.09-8.09-10.72C3.48,22.52,0,27.38,0,27.38c0,0,3.47,4.46,9.71,7.2,3.57,1.57,8.05,2.57,13.31,1.86Z"/>
                                <path fill="url(#ab_lg7)" d="M17.5,30.85c-.06-.15-.13-.3-.18-.46-.88-2.5-.72-5.1.24-7.37-.71-5.26.29-9.74,1.86-13.32-5.88-2.52-11.78-1.54-11.78-1.54,0,0-.7,5.6,1.78,11.96,1.42,3.63,3.87,7.5,8.09,10.72Z"/>
                                <path fill="#a57d2d" d="M23.6,17.33c-.16.06-.31.13-.46.19-2.6,1.06-4.55,3.08-5.58,5.51-.95,2.27-1.12,4.87-.24,7.37.05.16.12.31.18.46,1.06,2.6,3.08,4.56,5.51,5.58,1.25.53,2.61.82,3.99.82,1.12,0,2.27-.19,3.39-.58.16-.06.31-.13.47-.19,2.37-.96,4.27-2.73,5.38-5.05.07-.15.13-.31.2-.46.99-2.35,1.09-4.95.24-7.37-.06-.16-.13-.31-.19-.47-1.06-2.6-3.08-4.55-5.51-5.57-2.26-.95-4.87-1.12-7.37-.24ZM35.42,24.04c1.63,4.65-.81,9.75-5.47,11.38-4.65,1.63-9.75-.81-11.38-5.47-1.63-4.65.81-9.75,5.47-11.38,4.65-1.63,9.75.81,11.38,5.47Z"/>
                            </g>
                        </svg>
                        <Box sx={{
                            width: '1px',
                            height: 20,
                            bgcolor: 'rgba(255,255,255,0.15)',
                        }} />
                        <HodosButton
                            variant="icon"
                            size="small"
                            onClick={(e) => {
                                const rect = e.currentTarget.getBoundingClientRect();
                                const iconRightOffset = Math.round(window.innerWidth - rect.right + rect.width / 2);
                                if (window.cefMessage) {
                                    window.cefMessage.send('cookie_panel_show', [iconRightOffset.toString(), currentDomain]);
                                }
                            }}
                            aria-label="Privacy Shield"
                            title="Privacy Shield"
                            style={{ color: adblockEnabled ? '#a67c00' : '#6b7280' }}
                        >
                            <Badge
                                variant="dot"
                                invisible={!shieldDotVisible}
                                sx={{
                                    '& .MuiBadge-badge': {
                                        backgroundColor: '#188038',
                                        minWidth: 8,
                                        height: 8,
                                        borderRadius: '50%',
                                    }
                                }}
                            >
                                <SecurityIcon sx={{ fontSize: 20 }} />
                            </Badge>
                        </HodosButton>
                    </Box>
                </Box>

                {/* Spacer to help center address bar */}
                <Box sx={{ flex: 1 }} />

                {/* Download Button */}
                <HodosButton
                    variant="icon"
                    size="small"
                    onClick={(e) => {
                        const rect = e.currentTarget.getBoundingClientRect();
                        const iconRightOffset = Math.round(window.innerWidth - rect.right + rect.width / 2);
                        window.cefMessage?.send('download_panel_show', [iconRightOffset.toString()]);
                    }}
                    aria-label="Downloads"
                    title="Downloads"
                    style={{
                        flexShrink: 0,
                        position: 'relative',
                        color: allComplete ? '#188038' : hasActiveDownloads ? undefined : '#9ca3af',
                    }}
                >
                    <DownloadIcon fontSize="small" />
                    {/* Circular progress ring around icon when downloading */}
                    {hasActiveDownloads && (
                        <CircularProgress
                            size={28}
                            thickness={3}
                            variant={downloadProgress !== null && downloadProgress >= 0 ? 'determinate' : 'indeterminate'}
                            value={downloadProgress !== null && downloadProgress >= 0 ? downloadProgress : undefined}
                            sx={{
                                position: 'absolute',
                                top: '50%',
                                left: '50%',
                                marginTop: '-14px',
                                marginLeft: '-14px',
                                color: 'primary.main',
                            }}
                        />
                    )}
                </HodosButton>

                {/* Wallet Button */}
                <HodosButton
                    variant="icon"
                    size="small"
                    onClick={(e) => {
                        console.log('Wallet panel toggle clicked');
                        const rect = e.currentTarget.getBoundingClientRect();
                        const iconRightOffset = Math.round(window.innerWidth - rect.right + rect.width / 2);
                        window.cefMessage?.send('toggle_wallet_panel', [
                            iconRightOffset.toString(),
                            unreadPaymentCount.toString(),
                            unreadPaymentAmount.toString()
                        ].join(','));
                    }}
                    aria-label="Wallet"
                    style={{ flexShrink: 0 }}
                >
                    <Badge
                        variant="dot"
                        invisible={!hasUnreadPayments}
                        sx={{
                            '& .MuiBadge-badge': {
                                backgroundColor: '#2e7d32',
                                minWidth: 8,
                                height: 8,
                                borderRadius: '50%',
                            }
                        }}
                    >
                        <AccountBalanceWalletIcon fontSize="small" />
                    </Badge>
                </HodosButton>

                {/* Profile Button - shows current profile avatar, triggers overlay */}
                <HodosButton
                    variant="icon"
                    size="small"
                    onClick={(e) => {
                        const rect = e.currentTarget.getBoundingClientRect();
                        const iconRightOffset = Math.round(window.innerWidth - rect.right + rect.width / 2);
                        window.cefMessage?.send('profile_panel_show', [iconRightOffset.toString()]);
                    }}
                    aria-label={currentProfile ? `Profile: ${currentProfile.name}` : 'Profile'}
                    title={currentProfile ? `Profile: ${currentProfile.name}` : 'Profile'}
                    style={{ flexShrink: 0 }}
                >
                    {currentProfile ? (
                        <Avatar
                            src={currentProfile.avatarImage || undefined}
                            sx={{
                                width: 24,
                                height: 24,
                                fontSize: 12,
                                bgcolor: currentProfile.color || '#666',
                            }}
                        >
                            {!currentProfile.avatarImage && currentProfile.avatarInitial}
                        </Avatar>
                    ) : (
                        <AccountCircleIcon fontSize="small" />
                    )}
                </HodosButton>

                {/* Three-dot Menu Button — triggers CEF overlay */}
                <HodosButton
                    variant="icon"
                    size="small"
                    onClick={(e) => {
                        const rect = e.currentTarget.getBoundingClientRect();
                        const iconRightOffset = Math.round(window.innerWidth - rect.right + rect.width / 2);
                        window.cefMessage?.send('menu_show', [iconRightOffset.toString()]);
                    }}
                    aria-label="Menu"
                    title="Menu"
                    style={{ flexShrink: 0 }}
                >
                    <MoreVertIcon fontSize="small" />
                </HodosButton>

                {/* Find Bar - inline in toolbar */}
                {findBarVisible && (
                    <FindBar
                        onClose={handleFindBarClose}
                        findResult={findResult}
                    />
                )}
            </Toolbar>


            {/* Toast for quick-block */}
            <Snackbar
                open={toastOpen}
                autoHideDuration={3000}
                onClose={() => setToastOpen(false)}
                anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
            >
                <Alert
                    onClose={() => setToastOpen(false)}
                    severity="success"
                    variant="filled"
                    sx={{ width: '100%' }}
                >
                    {toastMessage}
                </Alert>
            </Snackbar>

            {/* Toast for download start/complete — positioned at bottom of header */}
            <Snackbar
                open={downloadToast.open}
                autoHideDuration={3000}
                onClose={() => setDownloadToast(prev => ({ ...prev, open: false }))}
                anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
                sx={{
                    // Header HWND is small — position at the bottom edge so toast hangs down
                    '&.MuiSnackbar-root': { bottom: 0 },
                }}
            >
                <Alert
                    onClose={() => setDownloadToast(prev => ({ ...prev, open: false }))}
                    severity={downloadToast.severity}
                    variant="filled"
                    sx={{ width: '100%', fontSize: '0.8rem' }}
                    icon={downloadToast.severity === 'info' ? <DownloadIcon fontSize="small" /> : undefined}
                >
                    {downloadToast.message}
                </Alert>
            </Snackbar>
        </Box>
    );
};

export default MainBrowserView;
