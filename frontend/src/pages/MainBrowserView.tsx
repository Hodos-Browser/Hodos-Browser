import React, { useState } from 'react';
import {
  Box,
  Toolbar,
  IconButton,
  Paper,
  InputBase,
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
    // Address bar state - TODO Phase 3: Remove when Omnibox manages state
    const [address] = useState('https://metanetapps.com/');

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

    // TODO Phase 3: Re-enable tab sync when Omnibox integrates with tab state
    // Sync address bar with active tab's URL
    // React.useEffect(() => {
    //     // Only update if user is not currently editing the address bar
    //     if (!isEditingAddress) {
    //         const activeTab = tabs.find(t => t.id === activeTabId);
    //         if (activeTab && activeTab.url) {
    //             setAddress(activeTab.url);
    //         }
    //     }
    // }, [activeTabId, tabs, isEditingAddress]);

    // Keyboard shortcuts
    // TODO Phase 5: Restore Ctrl+L focus when Omnibox exposes focus method
    useKeyboardShortcuts({
        onNewTab: createTab,
        onCloseTab: closeActiveTab,
        onNextTab: nextTab,
        onPrevTab: prevTab,
        onSwitchToTab: switchToTabByIndex,
        onFocusAddressBar: () => {}, // Temporarily disabled until Omnibox exposes focus method
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

                {/* Address Bar Placeholder - clicking shows omnibox overlay */}
                <Paper
                    onClick={() => {
                        console.log('🔍 Address bar clicked, showing omnibox overlay');
                        window.cefMessage?.send('show_omnibox_overlay', []);
                    }}
                    sx={{
                        display: 'flex',
                        alignItems: 'center',
                        flex: 1,
                        minWidth: 0,
                        height: 36,
                        borderRadius: 20,
                        px: 2,
                        bgcolor: '#f1f3f4',
                        boxShadow: 'none',
                        border: '1px solid transparent',
                        cursor: 'pointer',
                        '&:hover': {
                            bgcolor: '#ffffff',
                            border: '1px solid rgba(0, 0, 0, 0.1)',
                        },
                    }}
                >
                    <InputBase
                        value={address}
                        placeholder="Search or enter address"
                        readOnly
                        fullWidth
                        sx={{
                            fontSize: 14,
                            color: 'rgba(0, 0, 0, 0.87)',
                            cursor: 'pointer',
                            '& input': {
                                padding: 0,
                                cursor: 'pointer',
                                '&::placeholder': {
                                    color: 'rgba(0, 0, 0, 0.4)',
                                    opacity: 1,
                                },
                            },
                        }}
                    />
                </Paper>

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
