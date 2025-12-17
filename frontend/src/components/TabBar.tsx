import React from 'react';
import { Box, IconButton, Tooltip, CircularProgress, Typography } from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import type { Tab } from '../types/TabTypes';
import { TabComponent } from './TabComponent';

interface TabBarProps {
  tabs: Tab[];
  activeTabId: number;
  isLoading: boolean;
  onCreateTab: () => void;
  onCloseTab: (tabId: number) => void;
  onSwitchTab: (tabId: number) => void;
}

export const TabBar: React.FC<TabBarProps> = ({
  tabs,
  activeTabId,
  isLoading,
  onCreateTab,
  onCloseTab,
  onSwitchTab,
}) => {
  // Debug logging
  React.useEffect(() => {
    console.log('📑 TabBar render:', {
      tabCount: tabs.length,
      activeTabId,
      isLoading,
      tabs: tabs.map(t => ({ id: t.id, title: t.title }))
    });
  }, [tabs, activeTabId, isLoading]);

  const handleTabClose = (e: React.MouseEvent, tabId: number) => {
    e.stopPropagation();
    onCloseTab(tabId);
  };

  return (
    <Box
      sx={{
        display: 'flex',
        alignItems: 'center',
        backgroundColor: '#1e1e1e',
        borderBottom: '1px solid rgba(255, 255, 255, 0.1)',
        height: 36,
        overflowX: 'auto',
        overflowY: 'hidden',
        flexShrink: 0,
        // Custom scrollbar styling
        '&::-webkit-scrollbar': {
          height: 4,
        },
        '&::-webkit-scrollbar-track': {
          backgroundColor: 'transparent',
        },
        '&::-webkit-scrollbar-thumb': {
          backgroundColor: 'rgba(255, 255, 255, 0.2)',
          borderRadius: 2,
          '&:hover': {
            backgroundColor: 'rgba(255, 255, 255, 0.3)',
          },
        },
      }}
    >
      {/* Loading indicator or empty state */}
      {tabs.length === 0 && (
        <Box sx={{ display: 'flex', alignItems: 'center', px: 2, bgcolor: 'red', height: '100%' }}>
          {isLoading && <CircularProgress size={16} sx={{ mr: 1, color: 'white' }} />}
          <Typography variant="body2" sx={{ color: 'white', fontWeight: 'bold' }}>
            {isLoading ? 'Loading tabs...' : 'No tabs - Click + to create'}
          </Typography>
        </Box>
      )}

      {/* Render all tabs */}
      {tabs.map((tab) => (
        <TabComponent
          key={tab.id}
          tab={tab}
          isActive={tab.id === activeTabId}
          onClose={(e) => handleTabClose(e, tab.id)}
          onClick={() => onSwitchTab(tab.id)}
        />
      ))}

      {/* New Tab Button */}
      <Tooltip title="New tab (Ctrl+T)" placement="bottom">
        <IconButton
          onClick={onCreateTab}
          size="small"
          sx={{
            minWidth: 36,
            width: 36,
            height: 36,
            borderRadius: 0,
            color: 'rgba(255, 255, 255, 0.7)',
            '&:hover': {
              backgroundColor: 'rgba(255, 255, 255, 0.05)',
              color: 'white',
            },
          }}
        >
          <AddIcon fontSize="small" />
        </IconButton>
      </Tooltip>
    </Box>
  );
};
