import { useState, useCallback } from 'react';
import type {
    BookmarkData,
    BookmarkAddResponse,
    BookmarkRemoveResponse,
    BookmarkSearchResponse,
    BookmarkGetAllResponse,
    BookmarkIsBookmarkedResponse,
} from '../types/bookmarks';

// Typed view of the canonical bookmark bridge (window.hodosBrowser.bookmarks,
// defined in bridge/initWindowBridge.ts and fully typed in types/hodosBrowser.d.ts).
// NOTE: Window.hodosBrowser is declared in two places (types/hodosBrowser.d.ts AND
// bridge/brc100.ts); TS currently resolves the brc100.ts shape, which omits the
// bookmarks namespace. Until those two global declarations are consolidated, we
// reach the typed surface through this local interface + a single cast — the same
// approach initWindowBridge.ts uses. (Follow-up: de-fragment the Window typing.)
interface BookmarksBridge {
    add(url: string, title: string, folderId?: number, tags?: string[]): Promise<BookmarkAddResponse>;
    remove(id: number): Promise<BookmarkRemoveResponse>;
    search(query: string, limit?: number, offset?: number): Promise<BookmarkSearchResponse>;
    getAll(folderId?: number, limit?: number, offset?: number): Promise<BookmarkGetAllResponse>;
    isBookmarked(url: string): Promise<BookmarkIsBookmarkedResponse>;
}

/**
 * useBookmarks — React state wrapper over the canonical bookmark bridge.
 * Mirrors the useHistory pattern: the bridge is the single source of truth;
 * this hook only adds list state + ergonomics. No raw IPC here.
 */
export function useBookmarks() {
    const [bookmarks, setBookmarks] = useState<BookmarkData[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const api = (): BookmarksBridge | undefined =>
        (window.hodosBrowser as unknown as { bookmarks?: BookmarksBridge } | undefined)?.bookmarks;

    const refresh = useCallback(async () => {
        const b = api();
        if (!b) { setError('Bookmark API not available'); return; }
        setLoading(true);
        setError(null);
        // Retry on slow machines: under a saturated UI thread (slow Win10) the getAll IPC
        // round-trip can exceed its timeout and the late response is dropped, otherwise
        // leaving the list silently empty. Re-request a couple of times before giving up.
        for (let attempt = 0; attempt < 3; attempt++) {
            try {
                const res = await b.getAll(undefined, 200, 0);
                setBookmarks(res.bookmarks ?? []);
                setError(null);
                setLoading(false);
                return;
            } catch (err) {
                if (attempt === 2) {
                    setError(err instanceof Error ? err.message : 'Failed to load bookmarks');
                    setLoading(false);
                } else {
                    await new Promise(r => setTimeout(r, 400));
                }
            }
        }
    }, []);

    const search = useCallback(async (query: string) => {
        const b = api();
        if (!b) return;
        setLoading(true);
        setError(null);
        try {
            const res = query.trim()
                ? await b.search(query, 200, 0)
                : await b.getAll(undefined, 200, 0);
            setBookmarks(res.bookmarks ?? []);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Search failed');
        } finally {
            setLoading(false);
        }
    }, []);

    const isBookmarked = useCallback(async (url: string): Promise<boolean> => {
        const b = api();
        if (!b || !url) return false;
        try {
            const res = await b.isBookmarked(url);
            return !!res.bookmarked;
        } catch {
            return false;
        }
    }, []);

    const add = useCallback(async (url: string, title: string): Promise<boolean> => {
        const b = api();
        if (!b || !url) return false;
        try {
            const res = await b.add(url, title || url);
            return !!res.success;
        } catch {
            return false;
        }
    }, []);

    const remove = useCallback(async (id: number): Promise<boolean> => {
        const b = api();
        if (!b) return false;
        try {
            const res = await b.remove(id);
            if (res.success) setBookmarks(prev => prev.filter(x => x.id !== id));
            return !!res.success;
        } catch {
            return false;
        }
    }, []);

    // Remove the bookmark matching a URL (used by the current-page star toggle,
    // where we have a URL but not the bookmark id).
    const removeByUrl = useCallback(async (url: string): Promise<boolean> => {
        const b = api();
        if (!b || !url) return false;
        try {
            const res = await b.search(url, 50, 0);
            const match = (res.bookmarks ?? []).find(x => x.url === url);
            return match ? await remove(match.id) : false;
        } catch {
            return false;
        }
    }, [remove]);

    return { bookmarks, loading, error, refresh, search, isBookmarked, add, remove, removeByUrl };
}
