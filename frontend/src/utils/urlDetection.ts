/**
 * Utility functions for detecting whether user input is a URL or search query
 */

/**
 * Determines if the input is a valid URL, localhost, or IP address.
 * Returns true if it should be navigated to directly, false if it should be searched.
 */
export function isUrl(input: string): boolean {
  const trimmed = input.trim();

  // Empty input is not a URL
  if (!trimmed) {
    return false;
  }

  // If it has a protocol, it's a URL
  if (/^https?:\/\//i.test(trimmed)) {
    return true;
  }

  // Check for localhost variants
  if (/^localhost(:\d+)?(\/.*)?$/i.test(trimmed)) {
    return true;
  }

  // Check for IP addresses (IPv4)
  if (/^(\d{1,3}\.){3}\d{1,3}(:\d+)?(\/.*)?$/.test(trimmed)) {
    return true;
  }

  // Check for file:// protocol
  if (/^file:\/\//i.test(trimmed)) {
    return true;
  }

  // Check if it looks like a domain with TLD
  // Must have at least one dot and a valid TLD (2-6 chars)
  if (/^[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)+(\:\d+)?(\/.*)?$/.test(trimmed)) {
    // Verify it has a common TLD or looks domain-like
    const parts = trimmed.split('/')[0].split(':')[0].split('.');
    const tld = parts[parts.length - 1];

    // Common TLDs and check length (2-6 chars is typical for TLDs)
    if (tld.length >= 2 && tld.length <= 6 && /^[a-zA-Z]+$/.test(tld)) {
      return true;
    }
  }

  // Everything else is a search query
  return false;
}

/**
 * Normalizes a URL by adding protocol if missing
 */
export function normalizeUrl(input: string): string {
  const trimmed = input.trim();

  // Already has protocol
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//i.test(trimmed)) {
    return trimmed;
  }

  // Add https:// for domains, localhost, and IPs
  if (isUrl(trimmed)) {
    return `https://${trimmed}`;
  }

  // Not a URL, return as-is (caller should search)
  return trimmed;
}

/**
 * Converts a search query to a Google search URL
 */
export function toGoogleSearchUrl(query: string): string {
  const trimmed = query.trim();
  return `https://www.google.com/search?q=${encodeURIComponent(trimmed)}`;
}

/**
 * Search URL templates by engine
 */
const SEARCH_URLS: Record<string, string> = {
  duckduckgo: 'https://duckduckgo.com/?q=',
  google: 'https://www.google.com/search?q=',
};

/**
 * Converts a search query to a search URL for the given engine
 */
export function toSearchUrl(query: string, engine: string): string {
  const base = SEARCH_URLS[engine] || SEARCH_URLS.duckduckgo;
  return base + encodeURIComponent(query.trim());
}
