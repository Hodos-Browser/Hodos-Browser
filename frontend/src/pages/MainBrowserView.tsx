import React, { useState, useMemo } from 'react';
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
// Settings panel now rendered in separate overlay process
import { useHodosBrowser } from '../hooks/useHodosBrowser';
import { useTabManager } from '../hooks/useTabManager';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { useCookieBlocking } from '../hooks/useCookieBlocking';
import { TabBar } from '../components/TabBar';
import { isUrl, normalizeUrl, toGoogleSearchUrl } from '../utils/urlDetection';


const MainBrowserView: React.FC = () => {
    // Address bar state
    const [address, setAddress] = useState('https://metanetapps.com/');
    const [isEditingAddress, setIsEditingAddress] = useState(false);
    const [autocompleteText, setAutocompleteText] = useState<string>('');

    const { navigate, goBack, goForward, reload } = useHodosBrowser();

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
                const suggestion = event.data.suggestion;
                if (suggestion && address && isEditingAddress) {
                    // Only show autocomplete if suggestion starts with current input
                    if (suggestion.toLowerCase().startsWith(address.toLowerCase())) {
                        setAutocompleteText(suggestion.slice(address.length));
                    } else {
                        setAutocompleteText('');
                    }
                } else {
                    setAutocompleteText('');
                }
            }
        };

        window.addEventListener('message', handleAutocomplete);
        return () => window.removeEventListener('message', handleAutocomplete);
    }, [address, isEditingAddress]);

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
        createTab('http://127.0.0.1:5137/history?tab=cookies');
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
                    <input
                        type="text"
                        value={address}
                        onChange={(e) => {
                            const newValue = e.target.value;
                            setAddress(newValue);
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
                            if (e.key === 'Enter') {
                                handleNavigate(address);
                                setIsEditingAddress(false);
                                setAutocompleteText('');
                                e.currentTarget.blur();
                                // Navigation dismisses overlay
                                window.cefMessage?.send('omnibox_hide', []);
                            } else if (e.key === 'Escape') {
                                // Escape dismisses overlay, keeps current input
                                window.cefMessage?.send('omnibox_hide', []);
                                setIsEditingAddress(false);
                                setAutocompleteText('');
                                e.currentTarget.blur();
                            } else if (e.key === 'Tab' && autocompleteText) {
                                // Tab accepts the autocomplete suggestion
                                e.preventDefault();
                                setAddress(address + autocompleteText);
                                setAutocompleteText('');
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
                        }}
                        placeholder="Search or enter address"
                        style={{
                            width: '100%',
                            height: 36,
                            borderRadius: 20,
                            paddingLeft: 16,
                            paddingRight: 16,
                            backgroundColor: '#f1f3f4',
                            border: '1px solid transparent',
                            fontSize: 14,
                            color: 'rgba(0, 0, 0, 0.87)',
                            outline: 'none',
                        }}
                    />
                    {/* Inline autocomplete text overlay */}
                    {autocompleteText && isEditingAddress && (
                        <span
                            style={{
                                position: 'absolute',
                                left: 16,
                                top: '50%',
                                transform: 'translateY(-50%)',
                                fontSize: 14,
                                color: 'rgba(0, 0, 0, 0.4)',
                                pointerEvents: 'none',
                                userSelect: 'none',
                                whiteSpace: 'pre',
                            }}
                        >
                            <span style={{ visibility: 'hidden' }}>{address}</span>
                            {autocompleteText}
                        </span>
                    )}
                </Box>

                {/* Wallet Button */}
                <IconButton
                    onClick={() => {
                        console.log('Wallet panel toggle clicked');
                        window.cefMessage?.send('toggle_wallet_panel', []);
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
                    onClick={(e) => setShieldMenuAnchor(e.currentTarget)}
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
                    onClick={() => {
                        window.cefMessage?.send('overlay_show_settings', []);
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
        </Box>
    );
};

export default MainBrowserView;
