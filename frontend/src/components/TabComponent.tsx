import React from 'react';
import { Box, IconButton, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import PublicIcon from '@mui/icons-material/Public';
import CircularProgress from '@mui/material/CircularProgress';
import type { Tab } from '../types/TabTypes';

interface TabComponentProps {
  tab: Tab;
  isActive: boolean;
  showDivider: boolean;
  onClose: (e: React.MouseEvent) => void;
  onClick: () => void;
}

export const TabComponent: React.FC<TabComponentProps> = ({
  tab,
  isActive,
  showDivider,
  onClose,
  onClick,
}) => {
  return (
    <Box
      onClick={onClick}
      sx={{
        display: 'flex',
        alignItems: 'center',
        gap: '6px',
        px: '10px',
        flex: 1,
        minWidth: 48,
        maxWidth: 200,
        height: 32,
        backgroundColor: isActive ? '#ffffff' : 'transparent',
        borderRadius: isActive ? '7px' : '6px',
        cursor: 'pointer',
        transition: 'background-color 0.15s ease',
        position: 'relative',
        userSelect: 'none',

        // Divider pipe between inactive tabs
        '&::after': showDivider ? {
          content: '""',
          position: 'absolute',
          right: 0,
          top: '6px',
          bottom: '6px',
          width: '1px',
          backgroundColor: 'rgba(0, 0, 0, 0.15)',
        } : {},

        '&:hover': {
          backgroundColor: isActive ? '#ffffff' : 'rgba(255, 255, 255, 0.45)',
          '& .tab-close-btn': {
            opacity: 1,
          },
          // Hide dividers on hover
          '&::after': {
            backgroundColor: 'transparent',
          },
        },
      }}
    >
      {/* Favicon or default icon */}
      <Box sx={{ flexShrink: 0, width: 14, height: 14, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        {tab.isLoading ? (
          <CircularProgress
            size={12}
            sx={{ color: 'rgba(0, 0, 0, 0.4)' }}
          />
        ) : tab.favicon ? (
          <img
            src={tab.favicon}
            alt=""
            width={14}
            height={14}
            style={{ display: 'block' }}
            onError={(e) => {
              e.currentTarget.style.display = 'none';
            }}
            onLoad={(e) => {
              e.currentTarget.style.display = 'block';
            }}
          />
        ) : (
          <PublicIcon sx={{ fontSize: 14, color: 'rgba(0, 0, 0, 0.35)' }} />
        )}
      </Box>

      {/* Tab Title */}
      <Typography
        variant="body2"
        sx={{
          flex: 1,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
          fontSize: 12,
          fontWeight: isActive ? 500 : 400,
          color: isActive ? 'rgba(0, 0, 0, 0.87)' : 'rgba(0, 0, 0, 0.55)',
          lineHeight: 1,
        }}
      >
        {tab.title || 'New Tab'}
      </Typography>

      {/* Close Button */}
      <IconButton
        className="tab-close-btn"
        onClick={onClose}
        size="small"
        sx={{
          width: 16,
          height: 16,
          padding: 0,
          opacity: isActive ? 0.4 : 0,
          transition: 'opacity 0.15s ease, background-color 0.15s ease',
          flexShrink: 0,
          '&:hover': {
            backgroundColor: 'rgba(0, 0, 0, 0.1)',
            opacity: 1,
          },
        }}
      >
        <CloseIcon sx={{ fontSize: 12, color: 'rgba(0, 0, 0, 0.6)' }} />
      </IconButton>
    </Box>
  );
};
