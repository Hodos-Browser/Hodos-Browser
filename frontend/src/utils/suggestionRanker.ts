import type { HistoryEntry, Suggestion } from '../types/omnibox';

/**
 * Extract domain from URL for deduplication
 */
function extractDomain(url: string): string {
  try {
    const parsed = new URL(url);
    return parsed.hostname.toLowerCase();
  } catch {
    return url.toLowerCase();
  }
}

/**
 * Merge history and Google suggestions with deduplication and ranking.
 * History suggestions are prioritized over Google suggestions.
 * Deduplication is at domain level - history wins.
 * Results are capped at 6.
 */
export function rankAndMergeSuggestions(
  historyResults: HistoryEntry[],
  googleResults: string[],
  _query: string
): Suggestion[] {
  // Convert history entries to suggestions
  const historySuggestions: Suggestion[] = historyResults.map(entry => ({
    url: entry.url,
    title: entry.title || entry.url,
    type: 'history' as const,
    score: entry.frecencyScore
  }));

  // Convert Google strings to suggestions (search URLs)
  const googleSuggestions: Suggestion[] = googleResults.map((text, index) => ({
    url: `https://www.google.com/search?q=${encodeURIComponent(text)}`,
    title: text,
    type: 'google' as const,
    score: 0 - (index * 0.01) // Slightly lower than history, preserve Google's order
  }));

  // Deduplicate by domain (history wins)
  const seen = new Set<string>();
  const deduped: Suggestion[] = [];

  // Add history first (they win in deduplication)
  for (const suggestion of historySuggestions) {
    const domain = extractDomain(suggestion.url);
    if (!seen.has(domain)) {
      seen.add(domain);
      deduped.push(suggestion);
    }
  }

  // Add Google suggestions that don't duplicate history domains
  for (const suggestion of googleSuggestions) {
    // For Google search suggestions, use the search term as the "domain" for dedup
    const searchTerm = suggestion.title.toLowerCase();
    // Also check if any history URL contains this search term as domain
    const isDuplicate = historySuggestions.some(h => {
      const hDomain = extractDomain(h.url);
      return hDomain.includes(searchTerm) || searchTerm.includes(hDomain.replace('www.', ''));
    });

    if (!isDuplicate && !seen.has(searchTerm)) {
      seen.add(searchTerm);
      deduped.push(suggestion);
    }
  }

  // Sort by score descending (history scores are positive, Google scores are negative/zero)
  deduped.sort((a, b) => b.score - a.score);

  // Cap at 6 results
  return deduped.slice(0, 6);
}

/**
 * Get the best autocomplete suggestion (top result, if it matches query prefix)
 */
export function getAutocompleteSuggestion(
  suggestions: Suggestion[],
  query: string
): string | null {
  if (suggestions.length === 0 || !query) return null;

  const top = suggestions[0];
  const queryLower = query.toLowerCase();

  // For history, check if URL or title starts with query
  if (top.type === 'history') {
    // Try URL without protocol
    const urlWithoutProtocol = top.url.replace(/^https?:\/\/(www\.)?/, '');
    if (urlWithoutProtocol.toLowerCase().startsWith(queryLower)) {
      return urlWithoutProtocol;
    }
    // Try title
    if (top.title.toLowerCase().startsWith(queryLower)) {
      return top.title;
    }
  }

  // For Google suggestions, the title is the search term
  if (top.type === 'google' && top.title.toLowerCase().startsWith(queryLower)) {
    return top.title;
  }

  return null;
}
