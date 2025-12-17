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

    // Settings panel state now managed in separate overlay process
    const [address, setAddress] = useState('https://metanetapps.com/');
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
    };

    const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter') {
        handleNavigate();
        }
    };

    return (
        <Box
            sx={{
                width: '100%',
                display: 'flex',
                flexDirection: 'column',
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
                paddingX: 1,
                paddingY: 0,
                margin: 0
            }}>
                {/* Back Button */}
                <IconButton onClick={goBack}>
                    <ArrowBackIcon />
                </IconButton>

                {/* Forward Button */}
                <IconButton onClick={goForward}>
                    <ArrowForwardIcon />
                </IconButton>

                {/* Refresh Button */}
                <IconButton onClick={reload}>
                    <RefreshIcon />
                </IconButton>

                {/* Address Bar */}
                <Paper
                    sx={{
                        display: 'flex',
                        alignItems: 'center',
                        flexGrow: 1,
                        mx: 2,
                        height: 40,
                        borderRadius: 20,
                        pl: 2,
                        bgcolor: 'white',
                        boxShadow: 1,
                    }}
                >
                    <InputBase
                        inputRef={addressBarRef}
                        value={address}
                        onChange={(e) => setAddress(e.target.value)}
                        onKeyDown={handleKeyDown}
                        placeholder="Enter address or search"
                        fullWidth
                    />
                </Paper>

                {/* Spacer */}
                <Box flexGrow={1} />

                {/* Wallet Button */}
                <IconButton
                    onClick={() => {
                        console.log("🟢 Wallet button clicked");
                        window.cefMessage?.send('overlay_show_wallet', []);
                        window.hodosBrowser.overlay.toggleInput(true);
                    }}
                    sx={{
                        ml: 1,
                        bgcolor: 'grey.200',
                        borderRadius: '50%',
                        '&:hover': { bgcolor: 'grey.300' }
                    }}
                >
                    <AccountBalanceWalletIcon />
                </IconButton>


                {/* Settings Button */}
                <IconButton
                    onClick={() => {
                        console.log("🔧 Settings button clicked");
                        console.log("🔧 hodosBrowser:", window.hodosBrowser);
                        // console.log("🔧 overlayPanel:", window.hodosBrowser?.overlayPanel);
                        // console.log("🔧 overlayPanel.toggleInput:", window.hodosBrowser?.overlayPanel?.toggleInput);
                        window.cefMessage?.send('overlay_show_settings', []);
                        console.log("🔧 Settings overlay will open in separate process");
                        window.hodosBrowser.overlay.toggleInput(true);
                    }}
                    sx={{
                        ml: 1,
                        bgcolor: 'grey.200',
                        borderRadius: '50%',
                        '&:hover': { bgcolor: 'grey.300' }
                    }}
                >
                    <MoreVertIcon />
                </IconButton>
            </Toolbar>
        </Box>
    );
};

export default MainBrowserView;
