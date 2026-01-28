export interface HistoryEntry {
  url: string;
  title: string;
  visitCount: number;
  lastVisitTime: number;
  frecencyScore: number;
}

export interface Suggestion {
  url: string;
  title: string;
  type: 'history' | 'google';
  score: number;
}
