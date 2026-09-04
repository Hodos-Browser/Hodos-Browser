import { useCallback, useEffect, useRef, useState } from 'react';

/**
 * Local favicons, from our own store — never from Google.
 *
 * 🚨 WHY. The omnibox, the new-tab tiles and the bookmarks panel each rendered
 * `https://www.google.com/s2/favicons?domain=<site>`. Measured on the wire, that
 * endpoint REDIRECTS to `t2.gstatic.com/faviconV2?…&url=<full url>`, so Google
 * received every domain the user typed into the address bar, every top site, and
 * every bookmark. From a privacy browser. (The consent modal had the same defect
 * and was fixed first — it can read the live `Tab::favicon_url` instead.)
 *
 * C++ (`FaviconStore`) persists the icon BYTES that `OnFaviconURLChange` +
 * `CefBrowserHost::DownloadImage(is_favicon=true)` already fetch for pages the
 * user actually visits, and serves them back as `data:` URIs — so rendering an
 * icon issues no network request at all.
 *
 * ⛔ A host we have no icon for is simply absent from the map. Callers fall back
 * to their own initial-letter tile. **Never** fall back to a remote lookup; that
 * is the defect being removed, and it would reappear exactly on the sites the
 * user has not visited yet — the most sensitive ones.
 */
export function useFavicons(hosts: string[]): Record<string, string> {
  const [icons, setIcons] = useState<Record<string, string>>({});
  // Keep every icon we have ever been given for this surface's lifetime. Without
  // this, a re-query for a narrower host list would blank the rows still on
  // screen — which reads as "the icons broke", not "the query changed".
  const cacheRef = useRef<Record<string, string>>({});

  const request = useCallback((wanted: string[]) => {
    const missing = wanted.filter((h) => h && !(h in cacheRef.current));
    if (missing.length === 0) return;
    (window as any).onFaviconsResponse = (map: Record<string, string>) => {
      cacheRef.current = { ...cacheRef.current, ...map };
      setIcons(cacheRef.current);
    };
    // Batched: one round trip for every row, not one per row. A per-row trip
    // would be slower than the Google request this replaces, and a privacy fix
    // that makes the UI visibly worse is a privacy fix that gets turned off.
    window.cefMessage?.send('favicon_get', [JSON.stringify(missing)]);
  }, []);

  const key = hosts.join(',');
  useEffect(() => {
    request(hosts);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, request]);

  return icons;
}

/** Host for a URL, or '' — the key `useFavicons` and the C++ store agree on. */
export function hostOf(url: string): string {
  try {
    return new URL(url.includes('://') ? url : `https://${url}`).hostname.toLowerCase();
  } catch {
    return '';
  }
}
