import type { HistoryEntryWithFrecency } from './history';

// Re-export for convenience
export type { HistoryEntryWithFrecency };

export interface Suggestion {
  url: string;
  title: string;
  type: 'history' | 'google';
  score: number;
}
