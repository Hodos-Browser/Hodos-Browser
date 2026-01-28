import React, { useState } from 'react';
import {
  Box,
  Toolbar,
  IconButton,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import ArrowForwardIcon from '@mui/icons-material/ArrowForward';
import RefreshIcon from '@mui/icons-material/Refresh';
import AccountBalanceWalletIcon from '@mui/icons-material/AccountBalanceWallet';
import HistoryIcon from '@mui/icons-material/History';
import SettingsIcon from '@mui/icons-material/Settings';
// Settings panel now rendered in separate overlay process
import { useHodosBrowser } from '../hooks/useHodosBrowser';
import { useTabManager } from '../hooks/useTabManager';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { TabBar } from '../components/TabBar';


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

    const handleNavigate = (url: string) => {
        navigate(url);
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

                {/* Simple Address Bar Input */}
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
                        flex: 1,
                        minWidth: 0,
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

                {/* Wallet Button */}
                <IconButton
                    onClick={() => {
                        console.log('💰 Wallet panel toggle clicked');
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
        </Box>
    );
};

export default MainBrowserView;
