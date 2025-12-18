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
  const handleTabClose = (e: React.MouseEvent, tabId: number) => {
    e.stopPropagation();
    onCloseTab(tabId);
  };

  return (
    <Box
      sx={{
        display: 'flex',
        alignItems: 'flex-end',
        backgroundColor: '#dee1e6',
        paddingX: 0.5,
        paddingTop: 0.5,
        height: 42,
        overflowX: 'auto',
        overflowY: 'hidden',
        flexShrink: 0,
        // Custom scrollbar styling
        '&::-webkit-scrollbar': {
          height: 3,
        },
        '&::-webkit-scrollbar-track': {
          backgroundColor: 'transparent',
        },
        '&::-webkit-scrollbar-thumb': {
          backgroundColor: 'rgba(0, 0, 0, 0.2)',
          borderRadius: 2,
          '&:hover': {
            backgroundColor: 'rgba(0, 0, 0, 0.3)',
          },
        },
      }}
    >
      {/* Loading indicator or empty state */}
      {tabs.length === 0 && (
        <Box sx={{ display: 'flex', alignItems: 'center', px: 2, height: '100%' }}>
          {isLoading && <CircularProgress size={14} sx={{ mr: 1, color: 'rgba(0, 0, 0, 0.5)' }} />}
          <Typography variant="body2" sx={{ color: 'rgba(0, 0, 0, 0.6)', fontSize: 13 }}>
            {isLoading ? 'Loading tabs...' : 'No tabs'}
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
            minWidth: 32,
            width: 32,
            height: 32,
            borderRadius: '6px',
            marginLeft: 0.5,
            color: 'rgba(0, 0, 0, 0.6)',
            '&:hover': {
              backgroundColor: 'rgba(0, 0, 0, 0.05)',
              color: 'rgba(0, 0, 0, 0.87)',
            },
          }}
        >
          <AddIcon fontSize="small" />
        </IconButton>
      </Tooltip>
    </Box>
  );
};
