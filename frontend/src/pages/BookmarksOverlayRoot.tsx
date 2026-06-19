import React, { useEffect, useRef, useState } from 'react';
import { Box, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import StarIcon from '@mui/icons-material/Star';
import StarBorderIcon from '@mui/icons-material/StarBorder';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import PublicIcon from '@mui/icons-material/Public';
import { HodosButton } from '../components/HodosButton';
import { useBookmarks } from '../hooks/useBookmarks';
import type { BookmarkData } from '../types/bookmarks';

function hostOf(url: string): string {
    try { return new URL(url).hostname; } catch { return url; }
}

const BookmarksOverlayRoot: React.FC = () => {
    const { bookmarks, refresh, search, isBookmarked, add, removeByUrl, remove } = useBookmarks();

    // Current page context, injected by C++ on each show via window.setBookmarkContext.
    const [ctx, setCtx] = useState<{ url: string; title: string }>({ url: '', title: '' });
    const [starred, setStarred] = useState(false);
    const [query, setQuery] = useState('');
    const searchRef = useRef<HTMLInputElement>(null);

    // Register the C++ injection hook (mirrors PrivacyShield's window.setShieldDomain).
    useEffect(() => {
        (window as any).setBookmarkContext = (url: string, title: string) => {
            setCtx({ url: url || '', title: title || '' });
        };
        return () => { delete (window as any).setBookmarkContext; };
    }, []);

    // Initial load + focus the search box (delayed per CEF input rules).
    useEffect(() => {
        refresh();
        const t = setTimeout(() => searchRef.current?.focus(), 50);
        return () => clearTimeout(t);
    }, [refresh]);

    // Recompute the current-page star state when the page changes. (Not gated on
    // `bookmarks` — toggleStar sets it explicitly, and depending on the list would
    // cause a one-frame flicker on toggle.)
    useEffect(() => {
        if (ctx.url) isBookmarked(ctx.url).then(setStarred);
        else setStarred(false);
    }, [ctx.url, isBookmarked]);

    const toggleStar = async () => {
        if (!ctx.url) return;
        if (starred) {
            await removeByUrl(ctx.url);
            setStarred(false);
        } else {
            await add(ctx.url, ctx.title || ctx.url);
            setStarred(true);
        }
        // Re-apply the active search filter rather than resetting to the full list.
        await (query.trim() ? search(query) : refresh());
    };

    const onSearchChange = (v: string) => {
        setQuery(v);
        search(v);
    };

    const openBookmark = (url: string) => {
        window.cefMessage?.send('navigate', [url]);
        window.cefMessage?.send('bookmarks_panel_hide');
    };

    return (
        <Box sx={{
            width: '100%',
            height: '100%',
            bgcolor: '#1a1d23',
            borderRadius: '8px',
            boxShadow: '0 4px 20px rgba(0,0,0,0.15)',
            overflow: 'hidden',
            display: 'flex',
            flexDirection: 'column',
        }}>
            {/* Header */}
            <Box sx={{
                p: 1.5,
                borderBottom: '1px solid #2a2d35',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
            }}>
                <Typography variant="subtitle2" sx={{ fontWeight: 600, color: '#f0f0f0' }}>
                    Bookmarks
                </Typography>
                <HodosButton variant="icon" size="small" onClick={() => window.cefMessage?.send('bookmarks_panel_hide')} aria-label="Close">
                    <CloseIcon sx={{ fontSize: 16 }} />
                </HodosButton>
            </Box>

            {/* Current page + star toggle */}
            {ctx.url && (
                <Box sx={{
                    p: 1.5,
                    borderBottom: '1px solid #2a2d35',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 1,
                }}>
                    <Box sx={{ flex: 1, minWidth: 0 }}>
                        <Typography variant="body2" sx={{
                            fontWeight: 500, color: '#f0f0f0',
                            overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                        }} title={ctx.title || ctx.url}>
                            {ctx.title || hostOf(ctx.url)}
                        </Typography>
                        <Typography variant="caption" sx={{
                            color: '#9ca3af',
                            overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'block',
                        }} title={ctx.url}>
                            {ctx.url}
                        </Typography>
                    </Box>
                    <HodosButton
                        variant="icon"
                        size="small"
                        onClick={toggleStar}
                        aria-label={starred ? 'Remove bookmark' : 'Bookmark this page'}
                        title={starred ? 'Remove bookmark' : 'Bookmark this page'}
                        style={{ color: starred ? '#dfbd69' : '#9ca3af' }}
                    >
                        {starred ? <StarIcon sx={{ fontSize: 20 }} /> : <StarBorderIcon sx={{ fontSize: 20 }} />}
                    </HodosButton>
                </Box>
            )}

            {/* Search */}
            <Box sx={{ p: 1.5, pb: 1, borderBottom: '1px solid #2a2d35' }}>
                <input
                    ref={searchRef}
                    type="text"
                    value={query}
                    onChange={(e) => onSearchChange(e.target.value)}
                    placeholder="Search bookmarks…"
                    style={{
                        width: '100%',
                        boxSizing: 'border-box',
                        padding: '6px 8px',
                        background: '#23272f',
                        border: '1px solid #2a2d35',
                        borderRadius: '4px',
                        color: '#f0f0f0',
                        fontSize: '13px',
                        outline: 'none',
                    }}
                    onFocus={(e) => (e.target.style.borderColor = '#dfbd69')}
                    onBlur={(e) => (e.target.style.borderColor = '#2a2d35')}
                />
            </Box>

            {/* List */}
            <Box sx={{ flex: 1, overflow: 'auto', p: 1.5, pt: 0.5 }}>
                {bookmarks.length === 0 && (
                    <Typography variant="body2" sx={{ textAlign: 'center', py: 3, color: '#6b7280' }}>
                        {query ? 'No matching bookmarks' : 'No bookmarks yet'}
                    </Typography>
                )}

                {bookmarks.map((bm: BookmarkData) => (
                    <Box
                        key={bm.id}
                        sx={{
                            py: 0.75,
                            px: 0.5,
                            display: 'flex',
                            alignItems: 'center',
                            gap: 1,
                            borderBottom: '1px solid #2a2d35',
                            '&:last-child': { borderBottom: 'none' },
                            cursor: 'pointer',
                            '&:hover': { backgroundColor: 'rgba(255,255,255,0.04)' },
                            '&:hover .bm-remove': { opacity: 1 },
                        }}
                        onClick={() => openBookmark(bm.url)}
                    >
                        {bm.favicon_url
                            ? <img src={bm.favicon_url} alt="" width={16} height={16} style={{ flexShrink: 0, borderRadius: 2 }} onError={(e) => { (e.currentTarget.style.display = 'none'); }} />
                            : <PublicIcon sx={{ fontSize: 16, color: '#6b7280', flexShrink: 0 }} />}
                        <Box sx={{ flex: 1, minWidth: 0 }}>
                            <Typography variant="body2" sx={{
                                color: '#f0f0f0',
                                overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                            }} title={bm.title || bm.url}>
                                {bm.title || hostOf(bm.url)}
                            </Typography>
                            <Typography variant="caption" sx={{
                                color: '#9ca3af',
                                overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'block',
                            }}>
                                {hostOf(bm.url)}
                            </Typography>
                        </Box>
                        <HodosButton
                            className="bm-remove"
                            variant="icon"
                            size="small"
                            onClick={(e) => { e.stopPropagation(); remove(bm.id); }}
                            aria-label="Remove bookmark"
                            title="Remove bookmark"
                            style={{ opacity: 0, color: '#9ca3af', flexShrink: 0 }}
                        >
                            <DeleteOutlineIcon sx={{ fontSize: 16 }} />
                        </HodosButton>
                    </Box>
                ))}
            </Box>
        </Box>
    );
};

export default BookmarksOverlayRoot;
