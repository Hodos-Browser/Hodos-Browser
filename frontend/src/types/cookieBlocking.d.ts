export interface BlockedDomainEntry {
  domain: string;
  is_wildcard: boolean;
  source: string;        // 'user' or 'default'
  created_at: number;    // Unix timestamp ms
}

export interface BlockLogEntry {
  cookie_domain: string;
  page_url: string;
  reason: string;        // 'blocked_domain' or 'third_party'
  blocked_at: number;    // Unix timestamp ms
}

export interface BlockDomainResponse {
  success: boolean;
  domain: string;
}

export interface UnblockDomainResponse {
  success: boolean;
  domain: string;
}

export interface AllowThirdPartyResponse {
  success: boolean;
  domain: string;
}

export interface BlockedCountResponse {
  count: number;
}

export interface ClearBlockLogResponse {
  success: boolean;
}
