import React, { useState, useEffect, useRef, useCallback } from 'react';
import { isUrl, normalizeUrl, toSearchUrl } from '../utils/urlDetection';

interface TopSite {
    url: string;
    title: string;
    visitCount: number;
    faviconDataUrl?: string;  // cached base64 favicon
}

// Default tiles for first-time users (BSV ecosystem)
const DEFAULT_TILES: TopSite[] = [
    { url: 'https://coingeek.com/', title: 'CoinGeek', visitCount: 0 },
    { url: 'https://metanetapps.com/', title: 'MetaNet Apps', visitCount: 0 },
];

// ── localStorage tile + favicon cache ──────────────────────────────

const CACHE_KEY = 'ntp_tiles_cache';

function getCachedTiles(): TopSite[] | null {
    try {
        const raw = localStorage.getItem(CACHE_KEY);
        if (raw) {
            const parsed = JSON.parse(raw);
            if (Array.isArray(parsed) && parsed.length > 0) return parsed;
        }
    } catch { /* ignore */ }
    return null;
}

function saveCachedTiles(tiles: TopSite[]) {
    try {
        localStorage.setItem(CACHE_KEY, JSON.stringify(tiles));
    } catch { /* ignore */ }
}

// ── Helpers ────────────────────────────────────────────────────────

function buildFaviconUrl(siteUrl: string, engine: string): string {
    try {
        const domain = new URL(siteUrl).hostname;
        return engine === 'google'
            ? `https://www.google.com/s2/favicons?domain=${domain}&sz=32`
            : `https://icons.duckduckgo.com/ip3/${domain}.ico`;
    } catch {
        return '';
    }
}

function getDomain(siteUrl: string): string {
    try {
        return new URL(siteUrl).hostname.replace(/^www\./, '');
    } catch {
        return siteUrl;
    }
}

/** Fetch a favicon image and return it as a base64 data URL. */
async function fetchFaviconDataUrl(faviconUrl: string): Promise<string> {
    try {
        const res = await fetch(faviconUrl);
        if (!res.ok) return '';
        const blob = await res.blob();
        return new Promise<string>((resolve) => {
            const reader = new FileReader();
            reader.onloadend = () => resolve((reader.result as string) || '');
            reader.onerror = () => resolve('');
            reader.readAsDataURL(blob);
        });
    } catch {
        return '';  // CORS or network failure — fall back to <img> tag
    }
}

// ── Component ──────────────────────────────────────────────────────

const NewTabPage: React.FC = () => {
    const [searchQuery, setSearchQuery] = useState('');
    // Load cached tiles synchronously so the page renders fully on first paint
    const [topSites, setTopSites] = useState<TopSite[] | null>(() => getCachedTiles());
    const [searchEngine, setSearchEngine] = useState('duckduckgo');
    const searchInputRef = useRef<HTMLInputElement>(null);

    // Set document title and suppress scrollbars
    useEffect(() => {
        document.title = 'New Tab';
        document.documentElement.style.overflow = 'hidden';
        document.body.style.overflow = 'hidden';
        document.body.style.margin = '0';
        return () => {
            document.documentElement.style.overflow = '';
            document.body.style.overflow = '';
        };
    }, []);

    // Auto-focus search bar on mount
    useEffect(() => {
        const timer = setTimeout(() => {
            searchInputRef.current?.focus();
        }, 100);
        return () => clearTimeout(timer);
    }, []);

    // Fetch search engine setting
    useEffect(() => {
        (window as any).onSettingsResponse = (data: { browser?: { searchEngine?: string } }) => {
            if (data?.browser?.searchEngine) {
                setSearchEngine(data.browser.searchEngine);
            }
        };
        window.cefMessage?.send('settings_get_all');
        return () => {
            (window as any).onSettingsResponse = undefined;
        };
    }, []);

    // Fetch fresh most-visited data from C++ (background refresh)
    // Merges cached favicons so tiles with known favicons render instantly
    useEffect(() => {
        const handler = (event: MessageEvent) => {
            if (event.data?.type === 'most_visited_response') {
                try {
                    const data = typeof event.data.data === 'string'
                        ? JSON.parse(event.data.data)
                        : event.data.data;

                    let tiles: TopSite[];
                    if (Array.isArray(data) && data.length > 0) {
                        tiles = data;
                    } else {
                        tiles = DEFAULT_TILES;
                    }

                    // Carry over cached favicons so we don't re-fetch known icons
                    const cached = getCachedTiles();
                    if (cached) {
                        const faviconMap = new Map<string, string>();
                        cached.forEach(t => {
                            if (t.faviconDataUrl) faviconMap.set(t.url, t.faviconDataUrl);
                        });
                        tiles = tiles.map(t => ({
                            ...t,
                            faviconDataUrl: faviconMap.get(t.url),
                        }));
                    }

                    setTopSites(tiles);
                } catch {
                    setTopSites(DEFAULT_TILES);
                }
            }
        };
        window.addEventListener('message', handler);
        window.cefMessage?.send('get_most_visited');
        return () => window.removeEventListener('message', handler);
    }, []);

    // Pre-fetch favicons as base64 and persist to cache
    // Runs whenever topSites changes; skips tiles that already have a cached favicon
    useEffect(() => {
        if (!topSites || topSites.length === 0) return;
        // undefined = not yet fetched;  '' = fetched but failed
        const needsFetch = topSites.some(s => s.faviconDataUrl === undefined);
        if (!needsFetch) return;

        let cancelled = false;
        (async () => {
            const updated = await Promise.all(topSites.map(async (tile) => {
                if (tile.faviconDataUrl !== undefined) return tile;
                const url = buildFaviconUrl(tile.url, searchEngine);
                if (!url) return { ...tile, faviconDataUrl: '' };
                const dataUrl = await fetchFaviconDataUrl(url);
                return { ...tile, faviconDataUrl: dataUrl };
            }));
            if (!cancelled) {
                setTopSites(updated);
                saveCachedTiles(updated);
            }
        })();
        return () => { cancelled = true; };
    }, [topSites, searchEngine]);

    const handleSearch = useCallback(() => {
        const input = searchQuery.trim();
        if (!input) return;

        let url: string;
        if (isUrl(input)) {
            url = normalizeUrl(input);
        } else {
            url = toSearchUrl(input, searchEngine);
        }
        window.location.href = url;
    }, [searchQuery, searchEngine]);

    const handleTileClick = useCallback((url: string) => {
        window.location.href = url;
    }, []);

    return (
        <div style={{
            width: '100vw',
            height: '100vh',
            backgroundColor: '#1a1a1a',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'flex-start',
            fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
            overflow: 'hidden',
            margin: 0,
            padding: 0,
            paddingTop: '20vh',
            position: 'fixed',
            top: 0,
            left: 0,
        }}>
            {/* Logo */}
            <div style={{ marginBottom: 32 }}>
                <svg viewBox="0 0 167 54" xmlns="http://www.w3.org/2000/svg" style={{ height: 54, width: 'auto' }}>
                    <defs>
                        <linearGradient id="ntp_lg" x1="32.82" y1="13.97" x2="18.73" y2="10.74" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
                        <linearGradient id="ntp_lg1" x1="40.33" y1="21.9" x2="32.65" y2="9.65" xlinkHref="#ntp_lg"/>
                        <linearGradient id="ntp_lg2" x1="40.03" y1="32.82" x2="43.26" y2="18.73" xlinkHref="#ntp_lg"/>
                        <linearGradient id="ntp_lg3" x1="32.1" y1="40.33" x2="44.35" y2="32.65" xlinkHref="#ntp_lg"/>
                        <linearGradient id="ntp_lg4" x1="21.18" y1="40.03" x2="35.27" y2="43.26" xlinkHref="#ntp_lg"/>
                        <linearGradient id="ntp_lg5" x1="13.67" y1="32.1" x2="21.35" y2="44.35" xlinkHref="#ntp_lg"/>
                        <linearGradient id="ntp_lg6" x1="13.97" y1="21.18" x2="10.74" y2="35.27" xlinkHref="#ntp_lg"/>
                        <linearGradient id="ntp_lg7" x1="21.9" y1="13.66" x2="9.65" y2="21.35" xlinkHref="#ntp_lg"/>
                    </defs>
                    <g>
                        <g>
                            <path fill="#dfbd69" d="M67.66,46.01h-4.29v-10.08h4.11c2.5,0,3.52,1.09,3.52,2.69,0,1.17-.83,1.97-1.9,2.16,1.23.21,2.19,1.02,2.19,2.46,0,1.75-1.28,2.77-3.63,2.77ZM64.72,40.28h2.82c1.46,0,2.08-.62,2.08-1.6s-.62-1.62-2.08-1.62h-2.82v3.22ZM64.72,41.35v3.52h2.91c1.5,0,2.29-.66,2.29-1.74s-.78-1.78-2.29-1.78h-2.91Z"/>
                            <path fill="#dfbd69" d="M85.39,46.01c-.19-.27-.34-1.01-.42-2.19-.06-1.12-.59-1.87-1.97-1.87h-2.85v4.07h-1.36v-10.08h4.1c2.56,0,3.79,1.18,3.79,2.98,0,1.55-1.06,2.37-2.29,2.51,1.23.24,1.82.99,1.92,2.18.13,1.47.18,2.05.51,2.42h-1.44ZM82.81,40.84c1.71,0,2.48-.64,2.48-1.89,0-1.15-.77-1.89-2.48-1.89h-2.66v3.78h2.66Z"/>
                            <path fill="#dfbd69" d="M98.74,46.2c-2.95,0-5.01-2.13-5.01-5.23s2.06-5.23,5.01-5.23,5.01,2.13,5.01,5.23-2.08,5.23-5.01,5.23ZM98.74,36.91c-2.18,0-3.59,1.58-3.59,4.05s1.41,4.05,3.59,4.05,3.57-1.58,3.57-4.05-1.39-4.05-3.57-4.05Z"/>
                            <path fill="#dfbd69" d="M119.3,44.31h.06l2.06-8.39h1.3l-2.58,10.08h-1.62l-2.1-8.05h-.06l-2.15,8.05h-1.6l-2.58-10.08h1.34l2.11,8.36h.06l2.19-8.36h1.3l2.24,8.39Z"/>
                            <path fill="#dfbd69" d="M130.27,42.32c.08,1.84,1.34,2.79,3.04,2.79,1.5,0,2.45-.64,2.45-1.74,0-.93-.61-1.38-1.92-1.63l-2-.38c-1.49-.29-2.51-1.09-2.51-2.66,0-1.76,1.39-2.96,3.6-2.96,2.53,0,3.99,1.34,4,3.62l-1.26.06c-.05-1.67-1.06-2.61-2.72-2.61-1.46,0-2.27.69-2.27,1.81,0,.99.66,1.31,1.82,1.54l1.83.35c1.84.35,2.77,1.15,2.77,2.77,0,1.86-1.6,2.93-3.78,2.93-2.48,0-4.29-1.36-4.29-3.79l1.25-.08Z"/>
                            <path fill="#dfbd69" d="M151.78,46.01h-7.19v-10.08h7.04v1.15h-5.7v3.17h4.53v1.14h-4.53v3.47h5.84v1.15Z"/>
                            <path fill="#dfbd69" d="M165.56,46.01c-.19-.27-.34-1.01-.42-2.19-.06-1.12-.59-1.87-1.97-1.87h-2.85v4.07h-1.36v-10.08h4.1c2.56,0,3.79,1.18,3.79,2.98,0,1.55-1.06,2.37-2.29,2.51,1.23.24,1.82.99,1.92,2.18.13,1.47.18,2.05.51,2.42h-1.44ZM162.98,40.84c1.71,0,2.48-.64,2.48-1.89,0-1.15-.77-1.89-2.48-1.89h-2.66v3.78h2.66Z"/>
                        </g>
                        <g>
                            <path fill="#a67c00" d="M63.17,27.98V8.18h4.78v7.83h8.64v-7.83h4.78v19.8h-4.78v-8.11h-8.64v8.11h-4.78Z"/>
                            <path fill="#a67c00" d="M94.39,28.36c-5.75,0-10.15-4.18-10.15-10.28s4.4-10.28,10.15-10.28,10.18,4.18,10.18,10.28-4.4,10.28-10.18,10.28ZM94.39,11.98c-3.21,0-5.25,2.42-5.25,6.1s2.04,6.1,5.25,6.1,5.28-2.42,5.28-6.1-2.04-6.1-5.28-6.1Z"/>
                            <path fill="#a67c00" d="M107.44,8.18h7.67c6.51,0,10.53,3.71,10.53,9.9s-4.02,9.9-10.53,9.9h-7.67V8.18ZM114.79,24.21c3.74,0,5.91-2.26,5.91-6.16s-2.17-6.1-5.91-6.1h-2.58v12.26h2.58Z"/>
                            <path fill="#a67c00" d="M137.7,28.36c-5.75,0-10.15-4.18-10.15-10.28s4.4-10.28,10.15-10.28,10.18,4.18,10.18,10.28-4.4,10.28-10.18,10.28ZM137.7,11.98c-3.21,0-5.25,2.42-5.25,6.1s2.04,6.1,5.25,6.1,5.28-2.42,5.28-6.1-2.04-6.1-5.28-6.1Z"/>
                            <path fill="#a67c00" d="M154.04,20.78c.19,2.74,2.23,3.83,4.68,3.83,2.11,0,3.43-.82,3.43-2.14s-1.16-1.63-2.89-1.98l-3.71-.66c-3.14-.6-5.38-2.36-5.38-5.72,0-3.9,3.05-6.32,7.86-6.32,5.38,0,8.3,2.67,8.39,7.07l-4.4.13c-.13-2.33-1.73-3.46-4.02-3.46-2.01,0-3.14.82-3.14,2.17,0,1.13.88,1.54,2.33,1.82l3.71.66c4.05.72,5.91,2.73,5.91,5.97,0,4.09-3.55,6.19-8.08,6.19-5.28,0-9.05-2.61-9.05-7.42l4.37-.16Z"/>
                        </g>
                    </g>
                    <g>
                        <path fill="url(#ntp_lg)" d="M17.56,23.03c1.02-2.43,2.97-4.46,5.58-5.51,3.22-4.22,7.09-6.68,10.73-8.1C31.49,3.48,26.62,0,26.62,0c0,0-4.46,3.47-7.2,9.71-1.57,3.57-2.57,8.05-1.86,13.32Z"/>
                        <path fill="url(#ntp_lg1)" d="M23.14,17.51c.15-.06.3-.13.46-.19,2.5-.88,5.1-.72,7.37.24,5.26-.71,9.75.29,13.32,1.86,2.52-5.88,1.54-11.78,1.54-11.78,0,0-5.6-.7-11.96,1.78-3.63,1.42-7.51,3.88-10.73,8.1Z"/>
                        <path fill="url(#ntp_lg2)" d="M54,26.62s-3.47-4.46-9.71-7.2c-3.57-1.57-8.06-2.57-13.32-1.86,2.43,1.02,4.45,2.97,5.51,5.57,4.22,3.22,6.69,7.1,8.1,10.73,5.94-2.38,9.42-7.24,9.42-7.24Z"/>
                        <path fill="url(#ntp_lg3)" d="M36.48,23.14c.06.16.13.31.19.47.85,2.42.76,5.02-.24,7.37.71,5.26-.29,9.74-1.86,13.31,5.88,2.52,11.78,1.54,11.78,1.54,0,0,.7-5.6-1.78-11.96-1.42-3.63-3.88-7.51-8.1-10.73Z"/>
                        <path fill="url(#ntp_lg4)" d="M36.44,30.98c-.07.15-.12.31-.2.46-1.11,2.32-3.02,4.09-5.38,5.05-3.22,4.22-7.09,6.68-10.73,8.1,2.38,5.94,7.24,9.42,7.24,9.42,0,0,4.46-3.47,7.2-9.71,1.57-3.57,2.57-8.05,1.86-13.31Z"/>
                        <path fill="url(#ntp_lg5)" d="M30.86,36.49c-.16.06-.31.13-.47.19-1.12.39-2.26.58-3.39.58-1.39,0-2.74-.29-3.99-.82-5.26.71-9.74-.29-13.31-1.86-2.52,5.88-1.54,11.78-1.54,11.78,0,0,5.6.7,11.96-1.78,3.63-1.42,7.51-3.88,10.73-8.1Z"/>
                        <path fill="url(#ntp_lg6)" d="M23.02,36.44c-2.43-1.03-4.46-2.98-5.51-5.58-4.22-3.22-6.67-7.09-8.09-10.72C3.48,22.52,0,27.38,0,27.38c0,0,3.47,4.46,9.71,7.2,3.57,1.57,8.05,2.57,13.31,1.86Z"/>
                        <path fill="url(#ntp_lg7)" d="M17.5,30.85c-.06-.15-.13-.3-.18-.46-.88-2.5-.72-5.1.24-7.37-.71-5.26.29-9.74,1.86-13.32-5.88-2.52-11.78-1.54-11.78-1.54,0,0-.7,5.6,1.78,11.96,1.42,3.63,3.87,7.5,8.09,10.72Z"/>
                        <path fill="#a57d2d" d="M23.6,17.33c-.16.06-.31.13-.46.19-2.6,1.06-4.55,3.08-5.58,5.51-.95,2.27-1.12,4.87-.24,7.37.05.16.12.31.18.46,1.06,2.6,3.08,4.56,5.51,5.58,1.25.53,2.61.82,3.99.82,1.12,0,2.27-.19,3.39-.58.16-.06.31-.13.47-.19,2.37-.96,4.27-2.73,5.38-5.05.07-.15.13-.31.2-.46.99-2.35,1.09-4.95.24-7.37-.06-.16-.13-.31-.19-.47-1.06-2.6-3.08-4.55-5.51-5.57-2.26-.95-4.87-1.12-7.37-.24ZM35.42,24.04c1.63,4.65-.81,9.75-5.47,11.38-4.65,1.63-9.75-.81-11.38-5.47-1.63-4.65.81-9.75,5.47-11.38,4.65-1.63,9.75.81,11.38,5.47Z"/>
                    </g>
                </svg>
            </div>

            {/* Search Bar */}
            <div style={{ width: '100%', maxWidth: 584, padding: '0 24px', boxSizing: 'border-box', marginBottom: 40 }}>
                <div style={{ position: 'relative' }}>
                    <div style={{
                        position: 'absolute',
                        left: 16,
                        top: '50%',
                        transform: 'translateY(-50%)',
                        color: 'rgba(255,255,255,0.5)',
                        fontSize: 18,
                        pointerEvents: 'none',
                    }}>
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                            <path d="M15.5 14h-.79l-.28-.27A6.471 6.471 0 0016 9.5 6.5 6.5 0 109.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z" fill="currentColor"/>
                        </svg>
                    </div>
                    <input
                        ref={searchInputRef}
                        type="text"
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        onKeyDown={(e) => {
                            if (e.key === 'Enter') {
                                handleSearch();
                            }
                        }}
                        placeholder="Search or enter address"
                        style={{
                            width: '100%',
                            boxSizing: 'border-box',
                            height: 46,
                            borderRadius: 24,
                            paddingLeft: 44,
                            paddingRight: 16,
                            backgroundColor: '#2d2d2d',
                            border: '1px solid #3d3d3d',
                            fontSize: 16,
                            color: '#e8e8e8',
                            outline: 'none',
                            caretColor: '#a67c00',
                        }}
                        onFocus={(e) => {
                            e.target.style.borderColor = '#a67c00';
                            e.target.style.backgroundColor = '#333';
                        }}
                        onBlur={(e) => {
                            e.target.style.borderColor = '#3d3d3d';
                            e.target.style.backgroundColor = '#2d2d2d';
                        }}
                    />
                </div>
            </div>

            {/* Most Visited Tiles */}
            {topSites !== null && topSites.length > 0 && (
                <div style={{
                    display: 'grid',
                    gridTemplateColumns: 'repeat(4, 96px)',
                    gap: 16,
                }}>
                    {topSites.map((site, index) => (
                        <div
                            key={index}
                            onClick={() => handleTileClick(site.url)}
                            style={{
                                width: 96,
                                height: 96,
                                borderRadius: 12,
                                backgroundColor: '#2d2d2d',
                                display: 'flex',
                                flexDirection: 'column',
                                alignItems: 'center',
                                justifyContent: 'center',
                                cursor: 'pointer',
                                transition: 'background-color 0.15s',
                                padding: 8,
                                boxSizing: 'border-box',
                            }}
                            onMouseEnter={(e) => {
                                (e.currentTarget as HTMLDivElement).style.backgroundColor = '#3a3a3a';
                            }}
                            onMouseLeave={(e) => {
                                (e.currentTarget as HTMLDivElement).style.backgroundColor = '#2d2d2d';
                            }}
                        >
                            <img
                                src={site.faviconDataUrl || buildFaviconUrl(site.url, searchEngine)}
                                alt=""
                                style={{
                                    width: 32,
                                    height: 32,
                                    borderRadius: 4,
                                    marginBottom: 8,
                                    objectFit: 'contain',
                                }}
                                onError={(e) => {
                                    (e.target as HTMLImageElement).style.display = 'none';
                                }}
                            />
                            <span style={{
                                color: '#ccc',
                                fontSize: 11,
                                textAlign: 'center',
                                overflow: 'hidden',
                                textOverflow: 'ellipsis',
                                whiteSpace: 'nowrap',
                                width: '100%',
                            }}>
                                {getDomain(site.url)}
                            </span>
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
};

export default NewTabPage;
