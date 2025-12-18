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
        px: 1.5,
        py: 0.75,
        minWidth: 160,
        maxWidth: 220,
        height: 34,
        backgroundColor: isActive ? '#ffffff' : 'rgba(255, 255, 255, 0.5)',
        borderTopLeftRadius: 8,
        borderTopRightRadius: 8,
        marginRight: 0.25,
        cursor: 'pointer',
        transition: 'all 0.15s ease',
        position: 'relative',
        border: '1px solid rgba(0, 0, 0, 0.1)',
        borderBottom: isActive ? '1px solid #ffffff' : '1px solid rgba(0, 0, 0, 0.1)',
        '&:hover': {
          backgroundColor: isActive ? '#ffffff' : 'rgba(255, 255, 255, 0.75)',
          '& .tab-close-btn': {
            opacity: 1,
          },
        },
      }}
    >
      {/* Favicon or default icon */}
      {tab.favicon ? (
        <img
          src={tab.favicon}
          alt=""
          width={14}
          height={14}
          style={{ flexShrink: 0 }}
        />
      ) : (
        <PublicIcon sx={{ fontSize: 14, color: 'rgba(0, 0, 0, 0.4)', flexShrink: 0 }} />
      )}

      {/* Loading indicator */}
      {tab.isLoading && (
        <CircularProgress
          size={12}
          sx={{ color: 'rgba(0, 0, 0, 0.4)', flexShrink: 0 }}
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
          fontSize: 12.5,
          fontWeight: isActive ? 500 : 400,
          color: isActive ? 'rgba(0, 0, 0, 0.87)' : 'rgba(0, 0, 0, 0.6)',
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
          width: 18,
          height: 18,
          padding: 0,
          marginLeft: 0.5,
          opacity: isActive ? 0.5 : 0,
          transition: 'all 0.15s ease',
          flexShrink: 0,
          '&:hover': {
            backgroundColor: 'rgba(0, 0, 0, 0.08)',
            opacity: 1,
          },
        }}
      >
        <CloseIcon sx={{ fontSize: 13, color: 'rgba(0, 0, 0, 0.6)' }} />
      </IconButton>
    </Box>
  );
};
