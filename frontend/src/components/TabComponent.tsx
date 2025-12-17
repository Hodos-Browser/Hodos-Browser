import React from 'react';
import { Box, IconButton, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import PublicIcon from '@mui/icons-material/Public';
import CircularProgress from '@mui/material/CircularProgress';
import type { Tab } from '../types/TabTypes';

interface TabComponentProps {
  tab: Tab;
  isActive: boolean;
  onClose: (e: React.MouseEvent) => void;
  onClick: () => void;
}

export const TabComponent: React.FC<TabComponentProps> = ({
  tab,
  isActive,
  onClose,
  onClick,
}) => {
  return (
    <Box
      onClick={onClick}
      sx={{
        display: 'flex',
        alignItems: 'center',
        gap: 1,
        px: 2,
        py: 0.75,
        minWidth: 180,
        maxWidth: 240,
        height: 36,
        backgroundColor: isActive ? '#2d2d2d' : 'transparent',
        borderRight: '1px solid rgba(255, 255, 255, 0.1)',
        cursor: 'pointer',
        transition: 'background-color 0.2s',
        position: 'relative',
        '&:hover': {
          backgroundColor: isActive ? '#2d2d2d' : 'rgba(255, 255, 255, 0.05)',
          '& .tab-close-btn': {
            opacity: 1,
          },
        },
        // Active tab indicator
        '&::after': isActive ? {
          content: '""',
          position: 'absolute',
          bottom: 0,
          left: 0,
          right: 0,
          height: 2,
          backgroundColor: 'primary.main',
        } : {},
      }}
    >
      {/* Favicon or default icon */}
      {tab.favicon ? (
        <img
          src={tab.favicon}
          alt=""
          width={16}
          height={16}
          style={{ flexShrink: 0 }}
        />
      ) : (
        <PublicIcon sx={{ fontSize: 16, color: 'grey.500', flexShrink: 0 }} />
      )}

      {/* Loading indicator */}
      {tab.isLoading && (
        <CircularProgress
          size={12}
          sx={{ color: 'primary.main', flexShrink: 0 }}
        />
      )}

      {/* Tab Title */}
      <Typography
        variant="body2"
        sx={{
          flex: 1,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
          fontSize: 13,
          color: isActive ? 'white' : 'rgba(255, 255, 255, 0.7)',
          userSelect: 'none',
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
          width: 20,
          height: 20,
          padding: 0,
          opacity: isActive ? 1 : 0,
          transition: 'opacity 0.2s',
          flexShrink: 0,
          '&:hover': {
            backgroundColor: 'rgba(255, 255, 255, 0.1)',
          },
        }}
      >
        <CloseIcon sx={{ fontSize: 14, color: 'rgba(255, 255, 255, 0.7)' }} />
      </IconButton>
    </Box>
  );
};
