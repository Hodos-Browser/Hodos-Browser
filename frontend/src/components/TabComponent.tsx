import React, { useState, useEffect } from 'react';
import { Box, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import { HodosButton } from './HodosButton';
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
        position: 'relative',
        flex: 1,
        minWidth: 48,
        maxWidth: 200,
        height: '100%',
        display: 'flex',
        alignItems: 'flex-end',
        opacity: isDragged ? 0.4 : 1,
        cursor: 'pointer',
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
      }}
    >
      {/* Tab body */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: '6px',
          px: isActive ? '10px' : '8px',
          mx: isActive ? 0 : '2px',
          width: isActive ? '100%' : 'calc(100% - 8px)',
          height: isActive ? 36 : 30,
          boxSizing: 'border-box',
          backgroundColor: isActive ? '#111827' : 'transparent',
          borderRadius: isActive ? '8px 8px 0 0' : '6px',
          marginBottom: isActive ? 0 : '6px',
          position: 'relative',
          transition: 'background-color 0.15s ease',

          // Divider pill on the LEFT side of inactive tabs
          '&::after': showDivider && !isActive ? {
            content: '""',
            position: 'absolute',
            left: -1,
            top: '6px',
            bottom: '6px',
            width: '2px',
            backgroundColor: 'rgba(255, 255, 255, 0.12)',
            borderRadius: '1px',
            transition: 'opacity 0.1s ease',
          } : {},

          '&:hover': {
            backgroundColor: isActive ? '#111827' : 'rgba(255, 255, 255, 0.06)',
            '& .tab-close-btn': {
              opacity: 1,
            },
            // Hide own left divider on hover
            '&::after': {
              opacity: 0,
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

        {/* Close Button — opacity controlled via CSS class, not inline style,
             so parent's sx hover rule ('& .tab-close-btn': { opacity: 1 }) can override it */}
        <HodosButton
          className={`tab-close-btn ${isActive ? 'tab-close-active' : 'tab-close-inactive'}`}
          variant="icon"
          size="small"
          onClick={onClose}
          aria-label="Close tab"
          style={{
            width: 16,
            height: 16,
            padding: 0,
            flexShrink: 0,
          }}
        >
          <CloseIcon sx={{ fontSize: 12, color: '#9ca3af' }} />
        </HodosButton>
      </Box>

      {/* Inverted corner - left */}
      {isActive && (
        <Box
          sx={{
            position: 'absolute',
            bottom: 0,
            left: -12,
            width: 12,
            height: 12,
            overflow: 'hidden',
            '&::after': {
              content: '""',
              position: 'absolute',
              width: 24,
              height: 24,
              borderRadius: '50%',
              boxShadow: '12px 12px 0 #111827',
              bottom: 0,
              left: -12,
            },
          }}
        />
      )}

      {/* Inverted corner - right */}
      {isActive && (
        <Box
          sx={{
            position: 'absolute',
            bottom: 0,
            right: -12,
            width: 12,
            height: 12,
            overflow: 'hidden',
            '&::after': {
              content: '""',
              position: 'absolute',
              width: 24,
              height: 24,
              borderRadius: '50%',
              boxShadow: '-12px 12px 0 #111827',
              bottom: 0,
              right: -12,
            },
          }}
        />
      )}
    </Box>
  );
};
