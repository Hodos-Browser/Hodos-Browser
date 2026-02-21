import React, { useState, useMemo, useEffect, useRef } from 'react';
import {
    Box,
    Toolbar,
    IconButton,
    Badge,
    Menu,
    MenuItem,
    ListItemIcon,
    ListItemText,
    Divider,
    Snackbar,
    Alert,
    CircularProgress,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import ArrowForwardIcon from '@mui/icons-material/ArrowForward';
import RefreshIcon from '@mui/icons-material/Refresh';
import AccountBalanceWalletIcon from '@mui/icons-material/AccountBalanceWallet';
import HistoryIcon from '@mui/icons-material/History';
import SettingsIcon from '@mui/icons-material/Settings';
import ShieldIcon from '@mui/icons-material/Shield';
import BlockIcon from '@mui/icons-material/Block';
import CookieIcon from '@mui/icons-material/Cookie';
import LockIcon from '@mui/icons-material/Lock';
import LockOpenIcon from '@mui/icons-material/LockOpen';
import ErrorOutlineIcon from '@mui/icons-material/ErrorOutline';
import DownloadIcon from '@mui/icons-material/Download';
// Settings panel now rendered in separate overlay process
import { useHodosBrowser } from '../hooks/useHodosBrowser';
import { useTabManager } from '../hooks/useTabManager';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { useCookieBlocking } from '../hooks/useCookieBlocking';
import { useBackgroundBalancePoller } from '../hooks/useBackgroundBalancePoller';
import { useDownloads } from '../hooks/useDownloads';
import { TabBar } from '../components/TabBar';
import { isUrl, normalizeUrl, toGoogleSearchUrl } from '../utils/urlDetection';


const MainBrowserView: React.FC = () => {
    // Address bar state
    const [address, setAddress] = useState('https://metanetapps.com/');
    const [isEditingAddress, setIsEditingAddress] = useState(false);
    const [autocompleteText, setAutocompleteText] = useState<string>('');
    const [userTypedText, setUserTypedText] = useState('https://metanetapps.com/');
    const addressInputRef = React.useRef<HTMLInputElement>(null);
    const justNavigatedRef = React.useRef(false);
    // Tracks navigation-in-progress to prevent tab sync from reverting address bar
    const pendingNavigationRef = React.useRef(false);
    const preNavTabUrlRef = React.useRef<string>('');
    // Suppress autocomplete after Backspace/Delete so it doesn't re-fill
    const suppressAutocompleteRef = React.useRef(false);

    const { navigate, goBack, goForward, reload } = useHodosBrowser();

    // Keep localStorage balance/price cache warm for wallet overlay
    useBackgroundBalancePoller();

    // Keep wallet-exists cache in sync so the overlay opens instantly with correct state.
    // Runs once on mount — if wallet.db was deleted, clears the stale cache before
    // the user ever opens the wallet panel.
    useEffect(() => {
        fetch('http://localhost:3301/wallet/status')
            .then(r => r.json())
            .then(data => {
                if (data.exists) {
                    localStorage.setItem('hodos_wallet_exists', 'true');
                } else {
                    localStorage.removeItem('hodos_wallet_exists');
                }
                localStorage.removeItem('hodos_wallet_locked');
            })
            .catch(() => {
                // Server not reachable — leave cache as-is (overlay will retry)
            });
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
    } = useTabManager();

    // Cookie blocking
    const {
        blockedCount,
        blockedDomains,
        fetchBlockedCount,
        blockDomain,
        resetBlockedCount,
    } = useCookieBlocking();

    // Downloads — only need icon visibility state; overlay handles controls
    const { downloads, hasDownloads, hasActiveDownloads } = useDownloads();

    // Track download IDs to detect new starts and completions for toast notifications
    const prevDownloadIdsRef = useRef<Set<number>>(new Set());
    const prevCompleteIdsRef = useRef<Set<number>>(new Set());
    const [downloadToast, setDownloadToast] = useState<{ open: boolean; message: string; severity: 'info' | 'success' }>({
        open: false, message: '', severity: 'info'
    });

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

    // Shield menu state
    const [shieldMenuAnchor, setShieldMenuAnchor] = useState<null | HTMLElement>(null);

    // Toast state
    const [toastOpen, setToastOpen] = useState(false);
    const [toastMessage, setToastMessage] = useState('');

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

    // Check if current domain is already blocked
    const isCurrentDomainBlocked = useMemo(() => {
        if (!currentDomain) return true; // Disable if no valid domain
        return blockedDomains.some((d) => d.domain === currentDomain);
    }, [currentDomain, blockedDomains]);

    // Poll for blocked count every 3 seconds
    React.useEffect(() => {
        fetchBlockedCount();
        const interval = setInterval(() => {
            fetchBlockedCount();
        }, 3000);
        return () => clearInterval(interval);
    }, [fetchBlockedCount]);

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
                setAddress(activeTab.url);
            }
        }
    }, [activeTabId, tabs, isEditingAddress]);

    // Reset blocked count on navigation (when active tab URL changes)
    React.useEffect(() => {
        const activeTab = tabs.find(t => t.id === activeTabId);
        if (activeTab?.url) {
            resetBlockedCount().catch(() => {
                // Silently ignore reset errors
            });
        }
    }, [activeTabId, tabs, resetBlockedCount]);

    // Listen for autocomplete suggestions from omnibox overlay
    React.useEffect(() => {
        const handleAutocomplete = (event: MessageEvent) => {
            if (event.data?.type === 'omnibox_autocomplete') {
                // After Backspace/Delete, skip auto-fill but keep suggestions visible
                if (suppressAutocompleteRef.current) {
                    return;
                }
                const suggestion = event.data.suggestion;
                if (suggestion && userTypedText && isEditingAddress) {
                    // Only show autocomplete if suggestion starts with current input
                    if (suggestion.toLowerCase().startsWith(userTypedText.toLowerCase())) {
                        const autocompletePart = suggestion.slice(userTypedText.length);
                        setAutocompleteText(autocompletePart);
                        // Update the full address to include autocomplete
                        setAddress(suggestion);
                    } else {
                        setAutocompleteText('');
                        setAddress(userTypedText);
                    }
                } else {
                    setAutocompleteText('');
                    setAddress(userTypedText);
                }
            }
        };

        window.addEventListener('message', handleAutocomplete);
        return () => window.removeEventListener('message', handleAutocomplete);
    }, [userTypedText, isEditingAddress]);

    // Apply text selection to highlight autocomplete portion
    React.useEffect(() => {
        if (autocompleteText && isEditingAddress && addressInputRef.current) {
            const input = addressInputRef.current;
            const userLength = userTypedText.length;
            const fullLength = address.length;

            // Set selection to highlight the autocomplete part
            // Use setTimeout to ensure this happens after React updates the DOM
            setTimeout(() => {
                if (document.activeElement === input) {
                    input.setSelectionRange(userLength, fullLength);
                }
            }, 0);
        }
    }, [autocompleteText, address, userTypedText, isEditingAddress]);

    // Keyboard shortcuts
    useKeyboardShortcuts({
        onNewTab: createTab,
        onCloseTab: closeActiveTab,
        onNextTab: nextTab,
        onPrevTab: prevTab,
        onSwitchToTab: switchToTabByIndex,
        onFocusAddressBar: () => {}, // TODO: Implement address bar focus functionality
        onReload: reload,
    });

    const handleNavigate = (input: string) => {
        // Detect if input is a URL or search query
        if (isUrl(input)) {
            // It's a URL - normalize and navigate
            const url = normalizeUrl(input);
            navigate(url);
        } else {
            // It's a search query - search Google
            const searchUrl = toGoogleSearchUrl(input);
            navigate(searchUrl);
        }
    };

    const handleQuickBlock = async () => {
        setShieldMenuAnchor(null);
        if (!currentDomain) return;
        try {
            await blockDomain(currentDomain, false);
            setToastMessage(`Blocked: ${currentDomain}`);
            setToastOpen(true);
        } catch {
            setToastMessage('Failed to block domain');
            setToastOpen(true);
        }
    };

    const handleViewCookies = () => {
        setShieldMenuAnchor(null);
        if (window.cefMessage) {
            window.cefMessage.send('cookie_panel_show', '0');
        } else {
            console.error('window.cefMessage not available');
        }
    };

    return (
        <Box
            sx={{
                width: 'calc(100% + 16px)',
                height: 'calc(100% + 16px)',
                display: 'flex',
                flexDirection: 'column',
                overflow: 'hidden',
                margin: '-8px',
                padding: 0,
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
            />

            {/* Top Navigation Bar */}
            <Toolbar sx={{
                bgcolor: '#ffffff',
                borderBottom: '1px solid rgba(0, 0, 0, 0.12)',
                minHeight: '54px !important',
                height: '54px',
                flexShrink: 0,
                px: 1,
                py: 0,
                margin: 0,
                gap: 0.75,
                overflow: 'hidden', // Prevent scrolling
            }}>
                {/* Back Button */}
                <IconButton
                    onClick={goBack}
                    size="small"
                    sx={{
                        flexShrink: 0,
                        color: 'rgba(0, 0, 0, 0.6)',
                        '&:hover': {
                            backgroundColor: 'rgba(0, 0, 0, 0.04)',
                            color: 'rgba(0, 0, 0, 0.87)',
                        }
                    }}
                >
                    <ArrowBackIcon fontSize="small" />
                </IconButton>

                {/* Forward Button */}
                <IconButton
                    onClick={goForward}
                    size="small"
                    sx={{
                        flexShrink: 0,
                        color: 'rgba(0, 0, 0, 0.6)',
                        '&:hover': {
                            backgroundColor: 'rgba(0, 0, 0, 0.04)',
                            color: 'rgba(0, 0, 0, 0.87)',
                        }
                    }}
                >
                    <ArrowForwardIcon fontSize="small" />
                </IconButton>

                {/* Refresh Button */}
                <IconButton
                    onClick={reload}
                    size="small"
                    sx={{
                        flexShrink: 0,
                        color: 'rgba(0, 0, 0, 0.6)',
                        '&:hover': {
                            backgroundColor: 'rgba(0, 0, 0, 0.04)',
                            color: 'rgba(0, 0, 0, 0.87)',
                        }
                    }}
                >
                    <RefreshIcon fontSize="small" />
                </IconButton>

                {/* Address Bar with Inline Autocomplete */}
                <Box sx={{ position: 'relative', flex: 1, minWidth: 0 }}>
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
                                <LockOpenIcon sx={{ fontSize: 16, color: 'rgba(0,0,0,0.4)' }} />
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

                            // Send query to omnibox overlay for suggestions
                            if (newValue.length > 0) {
                                window.cefMessage?.send('omnibox_update_query', [newValue]);
                                window.cefMessage?.send('omnibox_show', [newValue]);
                            } else {
                                window.cefMessage?.send('omnibox_hide', []);
                            }
                        }}
                        onKeyDown={(e) => {
                            if (e.key === 'Backspace' || e.key === 'Delete') {
                                // Suppress autocomplete re-fill after deletion
                                suppressAutocompleteRef.current = true;
                                setAutocompleteText('');
                            } else if (e.key !== 'Shift' && e.key !== 'Control' && e.key !== 'Alt' && e.key !== 'Meta') {
                                // Any non-modifier key clears the suppression
                                suppressAutocompleteRef.current = false;
                            }
                            if (e.key === 'Enter') {
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
                        style={{
                            width: '98%',
                            height: 36,
                            borderRadius: 20,
                            paddingLeft: securityState !== 'none' ? 30 : 16,
                            paddingRight: 16,
                            backgroundColor: '#f1f3f4',
                            border: '1px solid transparent',
                            fontSize: 14,
                            color: 'rgba(0, 0, 0, 0.87)',
                            outline: 'none',
                        }}
                    />
                </Box>

                {/* Download Button - only shown when downloads exist */}
                {hasDownloads && (
                    <IconButton
                        onClick={(e) => {
                            const rect = e.currentTarget.getBoundingClientRect();
                            const headerWidth = window.innerWidth;
                            const iconRightOffset = Math.round(headerWidth - rect.right + rect.width / 2);
                            window.cefMessage?.send('download_panel_show', [iconRightOffset.toString()]);
                        }}
                        size="small"
                        title="Downloads"
                        sx={{
                            flexShrink: 0,
                            position: 'relative',
                            color: allComplete ? '#188038' : hasActiveDownloads ? 'primary.main' : 'rgba(0, 0, 0, 0.6)',
                            '&:hover': {
                                backgroundColor: 'rgba(0, 0, 0, 0.04)',
                                color: allComplete ? '#188038' : 'rgba(0, 0, 0, 0.87)',
                            }
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
                    </IconButton>
                )}

                {/* Wallet Button */}
                <IconButton
                    onClick={(e) => {
                        console.log('Wallet panel toggle clicked');
                        const rect = e.currentTarget.getBoundingClientRect();
                        const dpr = window.devicePixelRatio || 1;
                        const iconRightOffset = Math.round((window.innerWidth - rect.right) * dpr);
                        window.cefMessage?.send('toggle_wallet_panel', iconRightOffset.toString());
                    }}
                    size="small"
                    sx={{
                        flexShrink: 0,
                        color: 'rgba(0, 0, 0, 0.6)',
                        '&:hover': {
                            backgroundColor: 'rgba(0, 0, 0, 0.04)',
                            color: 'rgba(0, 0, 0, 0.87)',
                        }
                    }}
                >
                    <AccountBalanceWalletIcon fontSize="small" />
                </IconButton>

                {/* History Button */}
                <IconButton
                    onClick={() => createTab('http://127.0.0.1:5137/history')}
                    size="small"
                    sx={{
                        flexShrink: 0,
                        color: 'rgba(0, 0, 0, 0.6)',
                        '&:hover': {
                            backgroundColor: 'rgba(0, 0, 0, 0.04)',
                            color: 'rgba(0, 0, 0, 0.87)',
                        }
                    }}
                    title="History"
                >
                    <HistoryIcon fontSize="small" />
                </IconButton>

                {/* Shield Badge - Cookie Blocking */}
                <IconButton
                    onClick={(e) => {
                        console.log('Shield clicked - sending cookie_panel_show');
                        const rect = e.currentTarget.getBoundingClientRect();
                        const dpr = window.devicePixelRatio || 1;
                        const iconRightOffset = Math.round((window.innerWidth - rect.right) * dpr);
                        if (window.cefMessage) {
                            window.cefMessage.send('cookie_panel_show', iconRightOffset.toString());
                        }
                    }}
                    size="small"
                    title="Cookie blocking"
                    sx={{
                        flexShrink: 0,
                        color: blockedCount > 0 ? 'primary.main' : 'rgba(0, 0, 0, 0.6)',
                        '&:hover': {
                            backgroundColor: 'rgba(0, 0, 0, 0.04)',
                        }
                    }}
                >
                    <Badge
                        badgeContent={blockedCount}
                        color="error"
                        max={99}
                        invisible={blockedCount === 0}
                        sx={{ '& .MuiBadge-badge': { fontSize: '0.65rem', minWidth: 16, height: 16 } }}
                    >
                        <ShieldIcon fontSize="small" />
                    </Badge>
                </IconButton>
                <Menu
                    anchorEl={shieldMenuAnchor}
                    open={Boolean(shieldMenuAnchor)}
                    onClose={() => setShieldMenuAnchor(null)}
                >
                    <MenuItem onClick={handleQuickBlock} disabled={isCurrentDomainBlocked}>
                        <ListItemIcon><BlockIcon fontSize="small" /></ListItemIcon>
                        <ListItemText>{currentDomain ? `Block ${currentDomain}` : 'No domain to block'}</ListItemText>
                    </MenuItem>
                    <MenuItem onClick={handleViewCookies}>
                        <ListItemIcon><CookieIcon fontSize="small" /></ListItemIcon>
                        <ListItemText>View Cookies</ListItemText>
                    </MenuItem>
                    <Divider />
                    <MenuItem disabled>
                        <ListItemText>Blocked: {blockedCount} cookies</ListItemText>
                    </MenuItem>
                </Menu>

                {/* Settings Button */}
                <IconButton
                    onClick={(e) => {
                        const rect = e.currentTarget.getBoundingClientRect();
                        const dpr = window.devicePixelRatio || 1;
                        const iconRightOffset = Math.round((window.innerWidth - rect.right) * dpr);
                        window.cefMessage?.send('overlay_show_settings', iconRightOffset.toString());
                        window.hodosBrowser.overlay.toggleInput(true);
                    }}
                    size="small"
                    sx={{
                        flexShrink: 0,
                        color: 'rgba(0, 0, 0, 0.6)',
                        '&:hover': {
                            backgroundColor: 'rgba(0, 0, 0, 0.04)',
                            color: 'rgba(0, 0, 0, 0.87)',
                        }
                    }}
                    title="Settings"
                >
                    <SettingsIcon fontSize="small" />
                </IconButton>
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
