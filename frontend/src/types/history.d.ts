export interface HistoryEntry {
  url: string;
  title: string;
  visitCount: number;
  visitTime: number;  // Chromium timestamp (microseconds since 1601)
  transition: number;
}

export interface HistorySearchParams {
  search?: string;
  startTime?: number;  // Chromium timestamp
  endTime?: number;    // Chromium timestamp
  limit?: number;
  offset?: number;
}

export interface HistoryGetParams {
  limit?: number;
  offset?: number;
}

export interface ClearRangeParams {
  startTime: number;  // Chromium timestamp
  endTime: number;    // Chromium timestamp
}
