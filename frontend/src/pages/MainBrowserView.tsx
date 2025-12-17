import React, { useState, useRef } from 'react';
import {
  Box,
  Toolbar,
  IconButton,
  InputBase,
  Paper,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import ArrowForwardIcon from '@mui/icons-material/ArrowForward';
import RefreshIcon from '@mui/icons-material/Refresh';
import AccountBalanceWalletIcon from '@mui/icons-material/AccountBalanceWallet';
import MoreVertIcon from '@mui/icons-material/MoreVert';
// Settings panel now rendered in separate overlay process
import { useHodosBrowser } from '../hooks/useHodosBrowser';
import { useTabManager } from '../hooks/useTabManager';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { TabBar } from '../components/TabBar';


const MainBrowserView: React.FC = () => {
    console.log("🔍 MainBrowserView rendering");

    // Address bar state
    const [address, setAddress] = useState('https://metanetapps.com/');
    const [isEditingAddress, setIsEditingAddress] = useState(false);
    const addressBarRef = useRef<HTMLInputElement>(null);

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
                console.log('🔗 Address bar synced to active tab:', activeTab.url);
            }
        }
    }, [activeTabId, tabs, isEditingAddress]);

    // Keyboard shortcuts
    useKeyboardShortcuts({
        onNewTab: createTab,
        onCloseTab: closeActiveTab,
        onNextTab: nextTab,
        onPrevTab: prevTab,
        onSwitchToTab: switchToTabByIndex,
        onFocusAddressBar: () => addressBarRef.current?.focus(),
        onReload: reload,
        onToggleDevTools: () => {
            // F12 will be handled by CEF natively
            console.log('DevTools toggle requested');
        },
    });

    const handleNavigate = () => {
        console.log('🧭 Navigating to:', address);
        navigate(address);
        setIsEditingAddress(false);
        // Address will update from tab list sync
    };

    const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter') {
            handleNavigate();
        } else if (e.key === 'Escape') {
            setIsEditingAddress(false);
            // Reset to active tab's URL
            const activeTab = tabs.find(t => t.id === activeTabId);
            if (activeTab) {
                setAddress(activeTab.url);
            }
        }
    };

    const handleAddressFocus = () => {
        setIsEditingAddress(true);
        // Select all text for easy editing
        setTimeout(() => addressBarRef.current?.select(), 0);
    };

    const handleAddressBlur = () => {
        setIsEditingAddress(false);
        // Reset to active tab's URL if user didn't navigate
        const activeTab = tabs.find(t => t.id === activeTabId);
        if (activeTab && activeTab.url !== address) {
            setAddress(activeTab.url);
        }
    };

    return (
        <Box
            sx={{
                width: '100%',
                height: '100%',
                display: 'flex',
                flexDirection: 'column',
                overflow: 'hidden', // Prevent scrolling
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
                bgcolor: 'grey.100',
                borderBottom: '1px solid #ccc',
                minHeight: '48px !important',
                height: '48px',
                flexShrink: 0,
                px: 0.5,
                py: 0,
                margin: 0,
                gap: 0.5,
                overflow: 'hidden', // Prevent scrolling
            }}>
                {/* Back Button */}
                <IconButton onClick={goBack} size="small" sx={{ flexShrink: 0 }}>
                    <ArrowBackIcon fontSize="small" />
                </IconButton>

                {/* Forward Button */}
                <IconButton onClick={goForward} size="small" sx={{ flexShrink: 0 }}>
                    <ArrowForwardIcon fontSize="small" />
                </IconButton>

                {/* Refresh Button */}
                <IconButton onClick={reload} size="small" sx={{ flexShrink: 0 }}>
                    <RefreshIcon fontSize="small" />
                </IconButton>

                {/* Address Bar - grows to fill available space */}
                <Paper
                    sx={{
                        display: 'flex',
                        alignItems: 'center',
                        flex: 1,
                        minWidth: 0, // Allow shrinking below content size
                        height: 32,
                        borderRadius: 16,
                        px: 1.5,
                        bgcolor: 'white',
                        boxShadow: 1,
                    }}
                >
                    <InputBase
                        inputRef={addressBarRef}
                        value={address}
                        onChange={(e) => setAddress(e.target.value)}
                        onKeyDown={handleKeyDown}
                        onFocus={handleAddressFocus}
                        onBlur={handleAddressBlur}
                        placeholder="Search or enter address"
                        fullWidth
                        sx={{
                            fontSize: 14,
                            '& input': {
                                padding: 0,
                            }
                        }}
                    />
                </Paper>

                {/* Wallet Button */}
                <IconButton
                    onClick={() => {
                        console.log("🟢 Wallet button clicked");
                        window.cefMessage?.send('overlay_show_wallet', []);
                        window.hodosBrowser.overlay.toggleInput(true);
                    }}
                    size="small"
                    sx={{
                        flexShrink: 0,
                        bgcolor: 'grey.200',
                        '&:hover': { bgcolor: 'grey.300' }
                    }}
                >
                    <AccountBalanceWalletIcon fontSize="small" />
                </IconButton>

                {/* Settings Button */}
                <IconButton
                    onClick={() => {
                        console.log("🔧 Settings button clicked");
                        window.cefMessage?.send('overlay_show_settings', []);
                        window.hodosBrowser.overlay.toggleInput(true);
                    }}
                    size="small"
                    sx={{
                        flexShrink: 0,
                        bgcolor: 'grey.200',
                        '&:hover': { bgcolor: 'grey.300' }
                    }}
                >
                    <MoreVertIcon fontSize="small" />
                </IconButton>
            </Toolbar>
        </Box>
    );
};

export default MainBrowserView;
