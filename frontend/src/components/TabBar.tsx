import React, { useState, useCallback, useRef, useEffect } from 'react';
import { Box, IconButton, Tooltip, CircularProgress, Typography } from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import type { Tab } from '../types/TabTypes';
import { TabComponent } from './TabComponent';

const DRAG_THRESHOLD = 5; // px before drag starts

interface TabBarProps {
  tabs: Tab[];
  activeTabId: number;
  isLoading: boolean;
  onCreateTab: () => void;
  onCloseTab: (tabId: number) => void;
  onSwitchTab: (tabId: number) => void;
  onReorderTabs?: (fromIndex: number, toIndex: number) => void;
}

export const TabBar: React.FC<TabBarProps> = ({
  tabs,
  activeTabId,
  isLoading,
  onCreateTab,
  onCloseTab,
  onSwitchTab,
  onReorderTabs,
}) => {
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const [ghostX, setGhostX] = useState(0); // cursor X for floating ghost

  // Refs for pointer-event-based dragging
  const pointerDownRef = useRef<{ index: number; startX: number; pointerId: number; tabRect: DOMRect | null } | null>(null);
  const isDraggingRef = useRef(false);
  const tabRectsRef = useRef<DOMRect[]>([]);
  const tabElsRef = useRef<(HTMLDivElement | null)[]>([]);

  const handleTabClose = (e: React.MouseEvent, tabId: number) => {
    e.stopPropagation();
    onCloseTab(tabId);
  };

  // Store ref for each tab element
  const setTabRef = useCallback((index: number) => (el: HTMLDivElement | null) => {
    tabElsRef.current[index] = el;
  }, []);

  // Snapshot all tab bounding rects when drag starts
  const snapshotRects = useCallback(() => {
    tabRectsRef.current = tabElsRef.current.map(el =>
      el ? el.getBoundingClientRect() : new DOMRect()
    );
  }, []);

  // Find which tab index the pointer X falls on
  const getDropIndex = useCallback((clientX: number): number | null => {
    const rects = tabRectsRef.current;
    for (let i = 0; i < rects.length; i++) {
      const rect = rects[i];
      if (rect.width === 0) continue;
      const midX = rect.left + rect.width / 2;
      if (clientX < midX) return i;
    }
    // Past the last tab
    return rects.length > 0 ? rects.length - 1 : null;
  }, []);

  const handlePointerDown = useCallback((e: React.PointerEvent, index: number) => {
    if (!onReorderTabs || tabs.length < 2) return;
    // Only primary button
    if (e.button !== 0) return;
    const el = tabElsRef.current[index];
    const tabRect = el ? el.getBoundingClientRect() : null;
    pointerDownRef.current = { index, startX: e.clientX, pointerId: e.pointerId, tabRect };
  }, [onReorderTabs, tabs.length]);

  // Track dropIndex in a ref so pointerup can read it synchronously
  const dropIndexRef = useRef<number | null>(null);
  const onReorderTabsRef = useRef(onReorderTabs);
  onReorderTabsRef.current = onReorderTabs;

  // Global pointermove + pointerup via useEffect
  useEffect(() => {
    const handlePointerMove = (e: PointerEvent) => {
      const down = pointerDownRef.current;
      if (!down) return;

      const dx = Math.abs(e.clientX - down.startX);

      // Start dragging after threshold
      if (!isDraggingRef.current && dx >= DRAG_THRESHOLD) {
        isDraggingRef.current = true;
        setDragIndex(down.index);
        snapshotRects();
      }

      if (isDraggingRef.current) {
        const newDrop = getDropIndex(e.clientX);
        dropIndexRef.current = newDrop;
        setDropIndex(newDrop);
        setGhostX(e.clientX);
      }
    };

    const handlePointerUp = () => {
      const down = pointerDownRef.current;
      const drop = dropIndexRef.current;
      if (isDraggingRef.current && down !== null && drop !== null && drop !== down.index) {
        onReorderTabsRef.current?.(down.index, drop);
      }
      pointerDownRef.current = null;
      isDraggingRef.current = false;
      dropIndexRef.current = null;
      setDragIndex(null);
      setDropIndex(null);
    };

    document.addEventListener('pointermove', handlePointerMove);
    document.addEventListener('pointerup', handlePointerUp);
    return () => {
      document.removeEventListener('pointermove', handlePointerMove);
      document.removeEventListener('pointerup', handlePointerUp);
    };
  }, [snapshotRects, getDropIndex]);

  return (
    <Box
      sx={{
        display: 'flex',
        alignItems: 'center',
        backgroundColor: '#dee1e6',
        paddingX: '6px',
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
          <Typography variant="body2" sx={{ color: 'rgba(0, 0, 0, 0.6)', fontSize: 12 }}>
            {isLoading ? 'Loading tabs...' : 'No tabs'}
          </Typography>
        </Box>
      )}

      {/* Render all tabs */}
      {tabs.map((tab, index) => {
        const isActive = tab.id === activeTabId;
        // Show divider if this tab and the next are both inactive
        const nextTab = tabs[index + 1];
        const nextIsActive = nextTab ? nextTab.id === activeTabId : false;
        const showDivider = !isActive && !nextIsActive && index < tabs.length - 1;
        const isDragged = dragIndex === index;

        // Drop indicator: show on the side closest to where the dragged tab is coming from
        let dropIndicator: 'left' | 'right' | null = null;
        if (dropIndex === index && dragIndex !== null && dragIndex !== index) {
          dropIndicator = dragIndex < index ? 'right' : 'left';
        }

        return (
          <TabComponent
            key={tab.id}
            tab={tab}
            isActive={isActive}
            showDivider={showDivider}
            onClose={(e) => handleTabClose(e, tab.id)}
            onClick={() => {
              // Don't switch tabs if we were dragging
              if (!isDraggingRef.current) {
                onSwitchTab(tab.id);
              }
            }}
            tabRef={setTabRef(index)}
            isDragged={isDragged}
            dropIndicator={dropIndicator}
            onPointerDown={onReorderTabs && tabs.length > 1
              ? (e) => handlePointerDown(e, index)
              : undefined
            }
          />
        );
      })}

      {/* Floating ghost tab follows cursor during drag */}
      {dragIndex !== null && tabs[dragIndex] && pointerDownRef.current?.tabRect && (
        <Box
          sx={{
            position: 'fixed',
            top: pointerDownRef.current.tabRect.top,
            left: ghostX - (pointerDownRef.current.tabRect.width / 2),
            width: pointerDownRef.current.tabRect.width,
            height: pointerDownRef.current.tabRect.height,
            opacity: 0.75,
            pointerEvents: 'none',
            zIndex: 9999,
            backgroundColor: '#ffffff',
            borderRadius: '7px',
            boxShadow: '0 2px 8px rgba(0,0,0,0.2)',
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
            px: '10px',
            boxSizing: 'border-box',
          }}
        >
          {/* Ghost favicon */}
          <Box sx={{ flexShrink: 0, width: 14, height: 14, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            {tabs[dragIndex].favicon ? (
              <img src={tabs[dragIndex].favicon} alt="" width={14} height={14} style={{ display: 'block' }} />
            ) : (
              <Box sx={{ width: 14, height: 14, borderRadius: '50%', bgcolor: 'rgba(0,0,0,0.1)' }} />
            )}
          </Box>
          {/* Ghost title */}
          <Typography
            variant="body2"
            sx={{
              flex: 1,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
              fontSize: 12,
              fontWeight: 500,
              color: 'rgba(0, 0, 0, 0.87)',
              lineHeight: 1,
            }}
          >
            {tabs[dragIndex].title || 'New Tab'}
          </Typography>
        </Box>
      )}

      {/* New Tab Button */}
      <Tooltip title="New tab (Ctrl+T)" placement="bottom">
        <IconButton
          onClick={onCreateTab}
          size="small"
          sx={{
            minWidth: 28,
            width: 28,
            height: 28,
            borderRadius: '6px',
            marginLeft: '4px',
            marginBottom: '2px',
            color: 'rgba(0, 0, 0, 0.5)',
            '&:hover': {
              backgroundColor: 'rgba(0, 0, 0, 0.06)',
              color: 'rgba(0, 0, 0, 0.87)',
            },
          }}
        >
          <AddIcon sx={{ fontSize: 18 }} />
        </IconButton>
      </Tooltip>
    </Box>
  );
};
