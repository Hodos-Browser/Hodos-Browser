import React, { useState, useCallback, useRef, useEffect } from 'react';
import { Box, IconButton, Tooltip, CircularProgress, Typography } from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import type { Tab } from '../types/TabTypes';
import { TabComponent } from './TabComponent';

const DRAG_THRESHOLD = 5; // px before horizontal drag starts
const TEAROFF_THRESHOLD_Y = 40; // px below tab bar to trigger tear-off

interface TabBarProps {
  tabs: Tab[];
  activeTabId: number;
  isLoading: boolean;
  onCreateTab: () => void;
  onCloseTab: (tabId: number) => void;
  onSwitchTab: (tabId: number) => void;
  onReorderTabs?: (fromIndex: number, toIndex: number) => void;
  onTearOff?: (tabId: number, screenX: number, screenY: number) => void;
}

export const TabBar: React.FC<TabBarProps> = ({
  tabs,
  activeTabId,
  isLoading,
  onCreateTab,
  onCloseTab,
  onSwitchTab,
  onReorderTabs,
  onTearOff,
}) => {
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const [ghostX, setGhostX] = useState(0); // cursor X for floating ghost
  const [ghostY, setGhostY] = useState(0); // cursor Y for floating ghost (tear-off)
  const [isTearingOff, setIsTearingOff] = useState(false);

  // Refs for pointer-event-based dragging
  const pointerDownRef = useRef<{ index: number; startX: number; startY: number; pointerId: number; tabRect: DOMRect | null } | null>(null);
  const isDraggingRef = useRef(false);
  const isTearingOffRef = useRef(false);
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
    // Don't start drag when clicking the close button
    if ((e.target as HTMLElement).closest('.tab-close-btn')) return;
    const el = tabElsRef.current[index];
    const tabRect = el ? el.getBoundingClientRect() : null;
    pointerDownRef.current = { index, startX: e.clientX, startY: e.clientY, pointerId: e.pointerId, tabRect };
    // Capture pointer so events continue even when cursor leaves the header HWND
    // (CEF windowed mode: sibling HWNDs steal mouse events without capture)
    if (el) {
      try { el.setPointerCapture(e.pointerId); } catch {}
    }
  }, [onReorderTabs, tabs.length]);

  // Track dropIndex in a ref so pointerup can read it synchronously
  const dropIndexRef = useRef<number | null>(null);
  const onReorderTabsRef = useRef(onReorderTabs);
  onReorderTabsRef.current = onReorderTabs;
  const onTearOffRef = useRef(onTearOff);
  onTearOffRef.current = onTearOff;

  // Helper to release pointer capture safely
  const releaseCapture = useCallback(() => {
    const down = pointerDownRef.current;
    if (down) {
      const el = tabElsRef.current[down.index];
      if (el) {
        try { el.releasePointerCapture(down.pointerId); } catch {}
      }
    }
  }, []);

  // Helper to reset all drag state
  const resetDragState = useCallback(() => {
    releaseCapture();
    // Hide native ghost if it was showing
    if (isTearingOffRef.current) {
      window.cefMessage?.send('tab_ghost_hide');
    }
    pointerDownRef.current = null;
    isDraggingRef.current = false;
    isTearingOffRef.current = false;
    dropIndexRef.current = null;
    setDragIndex(null);
    setDropIndex(null);
    setIsTearingOff(false);
  }, [releaseCapture]);

  // Track last known screen coords for lostpointercapture fallback
  const lastScreenRef = useRef({ x: 0, y: 0 });

  // Global pointermove + pointerup via useEffect
  useEffect(() => {
    const handlePointerMove = (e: PointerEvent) => {
      const down = pointerDownRef.current;
      if (!down) return;

      // Track screen coords for fallback
      lastScreenRef.current = { x: e.screenX, y: e.screenY };

      const dx = Math.abs(e.clientX - down.startX);
      const dy = e.clientY - down.startY;

      // Start dragging after threshold (horizontal or vertical)
      if (!isDraggingRef.current && (dx >= DRAG_THRESHOLD || dy > TEAROFF_THRESHOLD_Y)) {
        isDraggingRef.current = true;
        setDragIndex(down.index);
        snapshotRects();
      }

      if (isDraggingRef.current) {
        // Check for tear-off: pointer moved far enough below the tab bar
        if (dy > TEAROFF_THRESHOLD_Y && tabs.length > 1) {
          if (!isTearingOffRef.current) {
            isTearingOffRef.current = true;
            setIsTearingOff(true);
            // Show native ghost window (floats above all HWNDs)
            const tab = tabs[down.index];
            const rect = down.tabRect;
            if (tab && rect) {
              const dpr = window.devicePixelRatio || 1;
              window.cefMessage?.send('tab_ghost_show',
                tab.title || 'New Tab',
                Math.round(rect.width * dpr),
                Math.round(rect.height * dpr));
            }
          }
        } else if (dy <= TEAROFF_THRESHOLD_Y) {
          // Dragged back into tab bar area — cancel tear-off, resume reorder
          if (isTearingOffRef.current) {
            isTearingOffRef.current = false;
            setIsTearingOff(false);
            window.cefMessage?.send('tab_ghost_hide');
          }
        }

        if (!isTearingOffRef.current) {
          // Normal reorder: update drop index
          const newDrop = getDropIndex(e.clientX);
          dropIndexRef.current = newDrop;
          setDropIndex(newDrop);
        }

        setGhostX(e.clientX);
        setGhostY(e.clientY);
      }
    };

    const handlePointerUp = (e: PointerEvent) => {
      const down = pointerDownRef.current;

      if (isDraggingRef.current && down !== null) {
        if (isTearingOffRef.current) {
          // Tear-off: send IPC with screen coordinates
          const tabId = tabs[down.index]?.id;
          if (tabId !== undefined) {
            onTearOffRef.current?.(tabId, e.screenX, e.screenY);
          }
        } else {
          // Normal reorder
          const drop = dropIndexRef.current;
          if (drop !== null && drop !== down.index) {
            onReorderTabsRef.current?.(down.index, drop);
          }
        }
      }

      resetDragState();
    };

    // Fallback: if pointer capture is lost (e.g. CEF HWND boundary edge case),
    // fire the tear-off immediately using last known screen position
    const handleLostCapture = () => {
      const down = pointerDownRef.current;
      if (isDraggingRef.current && down !== null && isTearingOffRef.current) {
        const tabId = tabs[down.index]?.id;
        if (tabId !== undefined) {
          onTearOffRef.current?.(tabId, lastScreenRef.current.x, lastScreenRef.current.y);
        }
      }
      resetDragState();
    };

    document.addEventListener('pointermove', handlePointerMove);
    document.addEventListener('pointerup', handlePointerUp);
    document.addEventListener('lostpointercapture', handleLostCapture);
    return () => {
      document.removeEventListener('pointermove', handlePointerMove);
      document.removeEventListener('pointerup', handlePointerUp);
      document.removeEventListener('lostpointercapture', handleLostCapture);
    };
  }, [snapshotRects, getDropIndex, tabs, resetDragState]);

  return (
    <Box
      sx={{
        display: 'flex',
        alignItems: 'center',
        backgroundColor: '#111827',
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
          backgroundColor: 'rgba(255, 255, 255, 0.15)',
          borderRadius: 2,
          '&:hover': {
            backgroundColor: 'rgba(255, 255, 255, 0.25)',
          },
        },
      }}
    >
      {/* Loading indicator or empty state */}
      {tabs.length === 0 && (
        <Box sx={{ display: 'flex', alignItems: 'center', px: 2, height: '100%' }}>
          {isLoading && <CircularProgress size={14} sx={{ mr: 1, color: '#9ca3af' }} />}
          <Typography variant="body2" sx={{ color: '#9ca3af', fontSize: 12 }}>
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
        if (!isTearingOff && dropIndex === index && dragIndex !== null && dragIndex !== index) {
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

      {/* Floating ghost tab follows cursor during drag (hidden during tear-off — native ghost takes over) */}
      {dragIndex !== null && !isTearingOff && tabs[dragIndex] && pointerDownRef.current?.tabRect && (
        <Box
          sx={{
            position: 'fixed',
            top: isTearingOff
              ? ghostY - (pointerDownRef.current.tabRect.height / 2)
              : pointerDownRef.current.tabRect.top,
            left: ghostX - (pointerDownRef.current.tabRect.width / 2),
            width: pointerDownRef.current.tabRect.width,
            height: pointerDownRef.current.tabRect.height,
            opacity: isTearingOff ? 0.85 : 0.75,
            pointerEvents: 'none',
            zIndex: 9999,
            backgroundColor: '#1a1d23',
            borderRadius: '7px',
            boxShadow: isTearingOff
              ? '0 8px 24px rgba(0,0,0,0.35)'
              : '0 2px 8px rgba(0,0,0,0.2)',
            transform: isTearingOff ? 'scale(1.05)' : 'scale(1)',
            transition: 'box-shadow 0.15s, transform 0.15s, opacity 0.15s',
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
              <Box sx={{ width: 14, height: 14, borderRadius: '50%', bgcolor: 'rgba(255,255,255,0.1)' }} />
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
              color: '#f0f0f0',
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
            color: '#9ca3af',
            '&:hover': {
              backgroundColor: '#1f2937',
              color: '#f0f0f0',
            },
          }}
        >
          <AddIcon sx={{ fontSize: 18 }} />
        </IconButton>
      </Tooltip>
    </Box>
  );
};
