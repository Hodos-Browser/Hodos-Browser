import React, { useState, useEffect, useRef, useCallback } from 'react';
import { isUrl, normalizeUrl, toSearchUrl } from '../utils/urlDetection';
import { tokens } from '../theme/tokens';
import { useFavicons, hostOf } from '../hooks/useFavicons';

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

// beta.3 Phase 7b — REMOVED. This returned a third-party favicon URL:
// `google.com/s2/favicons?domain=…` (which redirects to t2.gstatic.com/faviconV2
// carrying the full URL) or, for the non-Google search engine,
// `icons.duckduckgo.com/ip3/…`. Either way, opening a NEW TAB handed a third
// party the user's most-visited sites — and switching search engine only chose
// WHICH third party. Icons now come from our local store via `useFavicons`.
// ⛔ Do not reintroduce a remote favicon URL here, for either engine.

function getDomain(siteUrl: string): string {
    try {
        return new URL(siteUrl).hostname.replace(/^www\./, '');
    } catch {
        return siteUrl;
    }
}

// beta.3 Phase 7b — REMOVED with buildFaviconUrl above. This fetched the
// third-party favicon URL and cached the bytes into localStorage, so the leak
// happened once per new site and then looked cached forever. Icons now come from
// the browser's own favicon store, which holds bytes for pages the user actually
// visited — no fetch from this page at all.

// ── Component ──────────────────────────────────────────────────────

/**
 * A tile's icon: the real favicon when we hold one, otherwise a letter square.
 *
 * ⛔ NEVER render `<img src="">`. That is what shipped in the first cut of the
 * favicon-store change and it is what the owner saw: an empty `src` does not
 * reliably fire `error`, so the `onError` hide never ran and every tile showed a
 * broken/blank image. The regression was invisible to the network test that
 * change was verified with — it proved no request LEAKED, and proved nothing
 * about whether an icon RENDERED.
 *
 * ⭐ The letter square is not a stopgap, it is the permanent fallback. Our store
 * only learns an icon when the user actually VISITS a site, so a freshly created
 * store legitimately has none for historical top sites, and any site can fail to
 * provide one. ⛔ Do not "fix" a missing icon by fetching it from a remote favicon
 * service — that is precisely the leak this whole change removed.
 */
const TileIcon: React.FC<{ src?: string; domain: string }> = ({ src, domain }) => {
    const [failed, setFailed] = useState(false);
    const box: React.CSSProperties = {
        width: 28, height: 28, borderRadius: 4, marginBottom: 8, flexShrink: 0,
    };
    if (src && !failed) {
        return (
            <img src={src} alt="" style={{ ...box, objectFit: 'contain' }}
                 onError={() => setFailed(true)} />
        );
    }
    return (
        <div style={{
            ...box,
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            background: tokens.borderSubtle, color: tokens.textSecondary,
            fontSize: 14, fontWeight: 600, textTransform: 'uppercase',
        }}>
            {(domain || '?').charAt(0)}
        </div>
    );
};

const NewTabPage: React.FC = () => {
    const [searchQuery, setSearchQuery] = useState('');
    // Load cached tiles synchronously so the page renders fully on first paint.
    // Fall back to DEFAULT_TILES on fresh installs so users always see content.
    const [topSites, setTopSites] = useState<TopSite[] | null>(() => getCachedTiles() || DEFAULT_TILES);
    // beta.3 Phase 7b — icons from our own store; no third-party request.
    const favicons = useFavicons((topSites || []).map((t) => hostOf(t.url)));
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

    // ⚠️ Phase 11 item 1 (2026-09-18) — this auto-focus was REMOVED for a day and then
    // RESTORED, because removing it made things worse. Read before touching it.
    //
    // The address bar and this box live in **separate CEF browser processes**. Two layers
    // of focus decide where a keystroke goes:
    //   DOM focus    — which element gets keys once they arrive at a browser
    //   NATIVE focus — whether keys arrive at that browser at all
    //
    // 📏 Measured: at startup `TabManager::RegisterTabBrowser` gives NATIVE focus to the
    // TAB. So this box holding DOM focus is what makes "launch and type" work at all today.
    // Removing it, and separately focusing the address bar in `MainBrowserView.tsx`, left
    // the header with DOM focus it could not use — 👤 the owner typed and **nothing
    // happened**. Giving the header native focus here did not fix it either (tried, logged
    // as firing, behaviour unchanged) — so the cause is further down and is not understood.
    //
    // ⛔ Do not remove this again without first establishing where native focus actually
    // lands at startup. See `phase-11-ui-leftovers/README.md` item 1.
    //
    // 🚨 beta.3 Phase 11 item 1 (`P11-I1`) - this page no longer takes the caret.
    //
    // ⛔ The warning above said not to remove this "without first establishing where
    // native focus actually lands at startup". ✅ That has now been established, with
    // `nativefocusprobe.py` reading GetGUIThreadInfo on the browser UI thread, and it
    // turned out to be TWO defects below this line:
    //   1. four overlays were pre-warmed WS_VISIBLE, so each stole the keyboard at
    //      CreateWindowEx and the last one (tab-list, 4.5 s) kept it - a HIDDEN window;
    //   2. the shell window had no WM_SETFOCUS handler at all, and `CEFHostWindow` is
    //      registered with DefWindowProc, so even once the theft stopped the keyboard
    //      sat on windows that discard keystrokes.
    // That is why removing this auto-focus previously made typing stop entirely: this
    // box holding DOM focus was the only thing keeping keys anywhere visible, and the
    // native layer beneath it was broken the whole time.
    //
    // 👤 Owner's target: on launch the caret belongs in the ADDRESS BAR. The shell now
    // hands native focus to the header when the window opens on this page, and
    // MainBrowserView focuses the address bar - so this box must NOT compete, or the
    // user sees two carets (the documented failure of the first attempt).
    //
    // ⚠️ The two fields are functionally identical anyway: this box uses the same
    // isUrl / normalizeUrl / toSearchUrl helpers and the same search-engine setting as
    // the address bar, and the address bar additionally has the omnibox dropdown.

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

                    // beta.3 Phase 7b — the favicon carry-over that used to sit here is
                    // gone: icons no longer live on the tile, they come from the
                    // browser's own favicon store via `useFavicons`.
                    //
                    // ⭐ The tile LIST is still cached, and deliberately so — it is what
                    // makes the new-tab page paint instantly instead of waiting for the
                    // `get_most_visited` round trip. Dropping the write here (which the
                    // favicon removal orphaned) would have left `getCachedTiles()` reading
                    // a cache nothing ever refreshes, i.e. permanently stale tiles.
                    setTopSites(tiles);
                    saveCachedTiles(tiles);
                } catch {
                    setTopSites(DEFAULT_TILES);
                }
            }
        };
        window.addEventListener('message', handler);
        window.cefMessage?.send('get_most_visited');
        return () => window.removeEventListener('message', handler);
    }, []);

    // beta.3 Phase 7b — the pre-fetch/persist effect that used to live here is
    // gone. It existed to hide the latency of a THIRD-PARTY favicon request;
    // `useFavicons` reads our local store over one batched IPC, so there is
    // nothing to pre-warm and nothing to persist.

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
            background: `radial-gradient(ellipse 70% 30% at 50% 58%, rgba(166, 124, 0, 0.18) 0%, transparent 60%), radial-gradient(ellipse 90% 85% at 50% 45%, rgba(255, 255, 255, 0.25) 0%, rgba(255, 255, 255, 0.06) 50%, transparent 80%), ${tokens.bgPrimary}`,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'flex-start',
            fontFamily: tokens.fontUi,
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
                <svg viewBox="0 0 167 54" xmlns="http://www.w3.org/2000/svg" style={{ height: 96, width: 'auto' }}>
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
                            backgroundColor: tokens.bgSurface,
                            border: `1px solid ${tokens.borderDefault}`,
                            fontSize: 16,
                            color: tokens.textPrimary,
                            outline: 'none',
                            caretColor: tokens.gold,
                        }}
                        onFocus={(e) => {
                            e.target.style.borderColor = tokens.gold;
                            e.target.style.backgroundColor = tokens.bgSurfaceHover;
                        }}
                        onBlur={(e) => {
                            e.target.style.borderColor = tokens.borderDefault;
                            e.target.style.backgroundColor = tokens.bgSurface;
                        }}
                    />
                </div>
            </div>

            {/* Most Visited Tiles */}
            {topSites !== null && topSites.length > 0 && (
                <div style={{
                    display: 'grid',
                    gridTemplateColumns: 'repeat(4, 84px)',
                    gap: 14,
                }}>
                    {topSites.map((site, index) => (
                        <div
                            key={index}
                            onClick={() => handleTileClick(site.url)}
                            style={{
                                width: 84,
                                height: 84,
                                borderRadius: 12,
                                background: `radial-gradient(ellipse 80% 50% at 50% 80%, rgba(166, 124, 0, 0.10) 0%, transparent 70%), radial-gradient(ellipse 70% 50% at 50% 30%, rgba(255, 255, 255, 0.08) 0%, transparent 70%), ${tokens.bgSurface}`,
                                border: `1px solid ${tokens.borderSubtle}`,
                                display: 'flex',
                                flexDirection: 'column',
                                alignItems: 'center',
                                justifyContent: 'center',
                                cursor: 'pointer',
                                transition: 'border-color 0.2s, box-shadow 0.2s',
                                padding: 8,
                                boxSizing: 'border-box',
                            }}
                            onMouseEnter={(e) => {
                                (e.currentTarget as HTMLDivElement).style.borderColor = tokens.borderDefault;
                                (e.currentTarget as HTMLDivElement).style.boxShadow = '0 4px 16px rgba(0,0,0,0.4), 0 0 12px rgba(166, 124, 0, 0.06)';
                            }}
                            onMouseLeave={(e) => {
                                (e.currentTarget as HTMLDivElement).style.borderColor = tokens.borderSubtle;
                                (e.currentTarget as HTMLDivElement).style.boxShadow = 'none';
                            }}
                        >
                            <TileIcon
                                src={favicons[hostOf(site.url)] || site.faviconDataUrl}
                                domain={getDomain(site.url)}
                            />
                            <span style={{
                                color: tokens.textSecondary,
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
