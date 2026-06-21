import React, { useEffect, useRef, useState } from 'react';
import { Box, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import PublicIcon from '@mui/icons-material/Public';
import HistoryIcon from '@mui/icons-material/History';
import { HodosButton } from '../components/HodosButton';
import { useTabManager } from '../hooks/useTabManager';

interface ClosedEntry { url: string; title: string; }

declare global {
    interface Window {
        onRecentlyClosedResponse?: (data: ClosedEntry[]) => void;
        tabListRefresh?: () => void;
    }
}

function hostOf(url: string): string {
    try { return new URL(url).hostname; } catch { return url; }
}

const TabListOverlayRoot: React.FC = () => {
    const { tabs, activeTabId, switchToTab, closeTab, refreshTabList } = useTabManager();
    const [query, setQuery] = useState('');
    const [recent, setRecent] = useState<ClosedEntry[]>([]);
    const searchRef = useRef<HTMLInputElement>(null);

    // Recently-closed: fetch via IPC; C++ replies on window.onRecentlyClosedResponse.
    useEffect(() => {
        window.onRecentlyClosedResponse = (data) => {
            setRecent(Array.isArray(data) ? data : []);
        };
        window.cefMessage?.send('get_recently_closed', []);
        return () => { delete window.onRecentlyClosedResponse; };
    }, []);

    // C++ calls this on every (re)show of this keep-alive overlay so both lists are
    // fresh — otherwise open-tabs only update on the 2s poll and recently-closed,
    // fetched once on mount, would stay empty.
    useEffect(() => {
        window.tabListRefresh = () => {
            refreshTabList();
            window.cefMessage?.send('get_recently_closed', []);
        };
        return () => { delete window.tabListRefresh; };
    }, [refreshTabList]);

    // Autofocus the search box (delayed per CEF input rule).
    useEffect(() => {
        const t = setTimeout(() => searchRef.current?.focus(), 50);
        return () => clearTimeout(t);
    }, []);

    const close = () => window.cefMessage?.send('tablist_panel_hide');

    const q = query.trim().toLowerCase();
    const filteredTabs = q
        ? tabs.filter(t => (t.title || '').toLowerCase().includes(q) || (t.url || '').toLowerCase().includes(q))
        : tabs;

    const openTab = (id: number) => {
        switchToTab(id);
        close();
    };
    const closeTabRow = (e: React.MouseEvent, id: number) => {
        e.stopPropagation();
        closeTab(id);
        // The open-tabs list refreshes via useTabManager's optimistic update + poll.
    };
    const reopen = (url: string) => {
        window.cefMessage?.send('reopen_recently_closed', [url]);
        close();
    };

    return (
        <Box sx={{
            width: '100%', height: '100%', bgcolor: '#1a1d23', borderRadius: '8px',
            boxShadow: '0 4px 20px rgba(0,0,0,0.15)', overflow: 'hidden',
            display: 'flex', flexDirection: 'column',
        }}>
            {/* Header */}
            <Box sx={{
                p: 1.5, borderBottom: '1px solid #2a2d35',
                display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            }}>
                <Typography variant="subtitle2" sx={{ fontWeight: 600, color: '#f0f0f0' }}>
                    Tabs
                </Typography>
                <HodosButton variant="icon" size="small" onClick={close} aria-label="Close">
                    <CloseIcon sx={{ fontSize: 16 }} />
                </HodosButton>
            </Box>

            {/* Search */}
            <Box sx={{ p: 1.5, pb: 1, borderBottom: '1px solid #2a2d35' }}>
                <input
                    ref={searchRef}
                    type="text"
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    placeholder="Search tabs…"
                    style={{
                        width: '100%', boxSizing: 'border-box', padding: '6px 8px',
                        background: '#23272f', border: '1px solid #2a2d35', borderRadius: '4px',
                        color: '#f0f0f0', fontSize: '13px', outline: 'none',
                    }}
                    onFocus={(e) => (e.target.style.borderColor = '#dfbd69')}
                    onBlur={(e) => (e.target.style.borderColor = '#2a2d35')}
                />
            </Box>

            {/* Lists */}
            <Box sx={{ flex: 1, overflow: 'auto' }}>
                {/* OPEN TABS */}
                <Typography variant="caption" sx={{ px: 1.5, pt: 1, pb: 0.25, display: 'block', color: '#6b7280', fontWeight: 600, textTransform: 'uppercase', letterSpacing: 0.4 }}>
                    Open tabs ({filteredTabs.length})
                </Typography>
                {filteredTabs.length === 0 && (
                    <Typography variant="body2" sx={{ textAlign: 'center', py: 2, color: '#6b7280' }}>
                        {q ? 'No matching tabs' : 'No tabs'}
                    </Typography>
                )}
                {filteredTabs.map((tab) => (
                    <Box
                        key={tab.id}
                        onClick={() => openTab(tab.id)}
                        sx={{
                            px: 1.5, py: 0.75, display: 'flex', alignItems: 'center', gap: 1,
                            cursor: 'pointer',
                            backgroundColor: tab.id === activeTabId ? 'rgba(223,189,105,0.10)' : 'transparent',
                            borderLeft: tab.id === activeTabId ? '2px solid #dfbd69' : '2px solid transparent',
                            '&:hover': { backgroundColor: 'rgba(255,255,255,0.04)' },
                            '&:hover .tl-x': { opacity: 1 },
                        }}
                    >
                        {tab.favicon
                            ? <img src={tab.favicon} alt="" width={16} height={16} style={{ flexShrink: 0, borderRadius: 2 }} onError={(e) => { (e.currentTarget.style.display = 'none'); }} />
                            : <PublicIcon sx={{ fontSize: 16, color: '#6b7280', flexShrink: 0 }} />}
                        <Box sx={{ flex: 1, minWidth: 0 }}>
                            <Typography variant="body2" sx={{ color: '#f0f0f0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={tab.title || tab.url}>
                                {tab.title || hostOf(tab.url)}
                            </Typography>
                            <Typography variant="caption" sx={{ color: '#9ca3af', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'block' }}>
                                {hostOf(tab.url)}
                            </Typography>
                        </Box>
                        <HodosButton
                            className="tl-x"
                            variant="icon"
                            size="small"
                            onClick={(e) => closeTabRow(e, tab.id)}
                            aria-label="Close tab"
                            title="Close tab"
                            style={{ opacity: 0, color: '#9ca3af', flexShrink: 0 }}
                        >
                            <CloseIcon sx={{ fontSize: 14 }} />
                        </HodosButton>
                    </Box>
                ))}

                {/* RECENTLY CLOSED */}
                {recent.length > 0 && (
                    <>
                        <Typography variant="caption" sx={{ px: 1.5, pt: 1.5, pb: 0.25, display: 'block', color: '#6b7280', fontWeight: 600, textTransform: 'uppercase', letterSpacing: 0.4, borderTop: '1px solid #2a2d35' }}>
                            Recently closed
                        </Typography>
                        {recent.map((c, i) => (
                            <Box
                                key={`${c.url}-${i}`}
                                onClick={() => reopen(c.url)}
                                sx={{
                                    px: 1.5, py: 0.6, display: 'flex', alignItems: 'center', gap: 1,
                                    cursor: 'pointer',
                                    '&:hover': { backgroundColor: 'rgba(255,255,255,0.04)' },
                                }}
                            >
                                <HistoryIcon sx={{ fontSize: 16, color: '#6b7280', flexShrink: 0 }} />
                                <Box sx={{ flex: 1, minWidth: 0 }}>
                                    <Typography variant="body2" sx={{ color: '#f0f0f0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={c.title || c.url}>
                                        {c.title || hostOf(c.url)}
                                    </Typography>
                                    <Typography variant="caption" sx={{ color: '#9ca3af', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'block' }}>
                                        {hostOf(c.url)}
                                    </Typography>
                                </Box>
                            </Box>
                        ))}
                    </>
                )}
            </Box>
        </Box>
    );
};

export default TabListOverlayRoot;
