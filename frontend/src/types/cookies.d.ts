export interface CookieData {
  name: string;
  value: string;
  domain: string;
  path: string;
  secure: boolean;
  httponly: boolean;
  sameSite: number;      // 0=unspecified, 1=no_restriction, 2=lax, 3=strict
  hasExpires: boolean;
  expires?: number;       // Unix timestamp in milliseconds (only present if hasExpires)
  size: number;           // Approximate size in bytes (name.length + value.length)
}

export interface DomainCookieGroup {
  domain: string;
  cookies: CookieData[];
  totalSize: number;      // Sum of all cookie sizes in this domain
  count: number;          // Number of cookies
}

export interface CookieDeleteResponse {
  success: boolean;
  deleted: number;
}

export interface CacheSizeResponse {
  totalBytes: number;
}
