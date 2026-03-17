import React, { useState, useEffect } from 'react';
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
  /** Ref callback so TabBar can measure this element's position */
  tabRef?: (el: HTMLDivElement | null) => void;
  isDragged?: boolean;
  dropIndicator?: 'left' | 'right' | null;
  onPointerDown?: (e: React.PointerEvent) => void;
}

export const TabComponent: React.FC<TabComponentProps> = ({
  tab,
  isActive,
  showDivider,
  onClose,
  onClick,
  tabRef,
  isDragged,
  dropIndicator,
  onPointerDown,
}) => {
  // Timeout loading spinner after 8 seconds — some sites (investing.com, yahoo.com)
  // have persistent connections that keep CEF's isLoading=true indefinitely
  const [loadingTimedOut, setLoadingTimedOut] = useState(false);
  useEffect(() => {
    if (tab.isLoading) {
      setLoadingTimedOut(false);
      const timer = setTimeout(() => setLoadingTimedOut(true), 8000);
      return () => clearTimeout(timer);
    } else {
      setLoadingTimedOut(false);
    }
  }, [tab.isLoading, tab.id]);

  const showSpinner = tab.isLoading && !loadingTimedOut;

  return (
    <Box
      ref={tabRef}
      onClick={onClick}
      onPointerDown={onPointerDown}
      sx={{
        display: 'flex',
        alignItems: 'center',
        gap: '6px',
        px: '10px',
        flex: 1,
        minWidth: 48,
        maxWidth: 200,
        height: 32,
        boxSizing: 'border-box',
        opacity: isDragged ? 0.4 : 1,
        backgroundColor: isActive ? '#1a1a2e' : 'transparent',
        borderRadius: isActive ? '7px' : '6px',
        cursor: 'pointer',
        transition: isDragged !== undefined ? 'background-color 0.15s ease' : 'background-color 0.15s ease',
        position: 'relative',
        userSelect: 'none',

        // Gold drop indicator bar
        ...(dropIndicator === 'left' ? {
          '&::before': {
            content: '""',
            position: 'absolute',
            left: -1,
            top: 4,
            bottom: 4,
            width: 2,
            backgroundColor: '#a67c00',
            borderRadius: 1,
            zIndex: 10,
          },
        } : dropIndicator === 'right' ? {
          '&::before': {
            content: '""',
            position: 'absolute',
            right: -1,
            top: 4,
            bottom: 4,
            width: 2,
            backgroundColor: '#a67c00',
            borderRadius: 1,
            zIndex: 10,
          },
        } : {}),

        // Divider pipe between inactive tabs
        '&::after': showDivider ? {
          content: '""',
          position: 'absolute',
          right: 0,
          top: '6px',
          bottom: '6px',
          width: '1px',
          backgroundColor: 'rgba(255, 255, 255, 0.1)',
        } : {},

        '&:hover': {
          backgroundColor: isActive ? '#1a1a2e' : 'rgba(255, 255, 255, 0.08)',
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
        {showSpinner ? (
          <CircularProgress
            size={12}
            sx={{ color: '#9ca3af' }}
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
          <PublicIcon sx={{ fontSize: 14, color: '#6b7280' }} />
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
          color: isActive ? '#f0f0f0' : '#9ca3af',
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
            backgroundColor: 'rgba(255, 255, 255, 0.15)',
            opacity: 1,
          },
        }}
      >
        <CloseIcon sx={{ fontSize: 12, color: '#9ca3af' }} />
      </IconButton>
    </Box>
  );
};
