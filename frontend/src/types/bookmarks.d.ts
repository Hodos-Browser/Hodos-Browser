export interface BookmarkData {
  id: number;
  url: string;
  title: string;
  folder_id: number | null;  // null = root level
  favicon_url: string;
  position: number;
  created_at: number;    // Unix timestamp ms
  updated_at: number;    // Unix timestamp ms
  last_accessed: number; // Unix timestamp ms
  tags: string[];
}

export interface FolderData {
  id: number;
  name: string;
  parent_id: number | null;  // null = root level
  position: number;
  created_at: number;
  updated_at: number;
  children?: FolderData[];  // Present in tree responses
}

export interface BookmarkAddResponse {
  success: boolean;
  id?: number;
  error?: string;
}

export interface BookmarkUpdateResponse {
  success: boolean;
  error?: string;
}

export interface BookmarkRemoveResponse {
  success: boolean;
  error?: string;
}

export interface BookmarkSearchResponse {
  bookmarks: BookmarkData[];
  total: number;
}

export interface BookmarkGetAllResponse {
  bookmarks: BookmarkData[];
  total: number;
}

export interface BookmarkIsBookmarkedResponse {
  bookmarked: boolean;
}

export interface FolderCreateResponse {
  success: boolean;
  id?: number;
  error?: string;
}

export interface FolderUpdateResponse {
  success: boolean;
  error?: string;
}

export interface FolderRemoveResponse {
  success: boolean;
  error?: string;
}
