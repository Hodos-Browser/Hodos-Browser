import { useState, useCallback } from 'react';
import type {
  BlockedDomainEntry,
  BlockLogEntry,
  BlockDomainResponse,
  UnblockDomainResponse,
  AllowThirdPartyResponse,
  BlockedCountResponse,
  ClearBlockLogResponse,
} from '../types/cookieBlocking';

export const useCookieBlocking = () => {
  const [blockedDomains, setBlockedDomains] = useState<BlockedDomainEntry[]>([]);
  const [blockLog, setBlockLog] = useState<BlockLogEntry[]>([]);
  const [blockedCount, setBlockedCount] = useState<number>(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchBlockList = useCallback(() => {
    setLoading(true);
    setError(null);
    return new Promise<BlockedDomainEntry[]>((resolve, reject) => {
      const timeout = setTimeout(() => {
        setLoading(false);
        setBlockedDomains([]);
        resolve([]);
        delete window.onCookieBlocklistResponse;
        delete window.onCookieBlocklistError;
      }, 5000);

      window.onCookieBlocklistResponse = (data: BlockedDomainEntry[]) => {
        clearTimeout(timeout);
        setBlockedDomains(data);
        setLoading(false);
        resolve(data);
        delete window.onCookieBlocklistResponse;
        delete window.onCookieBlocklistError;
      };

      window.onCookieBlocklistError = (errorMsg: string) => {
        clearTimeout(timeout);
        setError(errorMsg);
        setLoading(false);
        reject(new Error(errorMsg));
        delete window.onCookieBlocklistResponse;
        delete window.onCookieBlocklistError;
      };

      window.cefMessage?.send('cookie_get_blocklist', []);
    });
  }, []);

  const blockDomain = useCallback(async (domain: string, isWildcard: boolean): Promise<BlockDomainResponse> => {
    setError(null);
    const result = await new Promise<BlockDomainResponse>((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Block domain timeout'));
        delete window.onCookieBlockDomainResponse;
        delete window.onCookieBlockDomainError;
      }, 5000);

      window.onCookieBlockDomainResponse = (data: BlockDomainResponse) => {
        clearTimeout(timeout);
        resolve(data);
        delete window.onCookieBlockDomainResponse;
        delete window.onCookieBlockDomainError;
      };

      window.onCookieBlockDomainError = (errorMsg: string) => {
        clearTimeout(timeout);
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCookieBlockDomainResponse;
        delete window.onCookieBlockDomainError;
      };

      window.cefMessage?.send('cookie_block_domain', [domain, isWildcard.toString()]);
    });

    // Re-fetch block list after successful block
    await fetchBlockList();
    return result;
  }, [fetchBlockList]);

  const unblockDomain = useCallback(async (domain: string): Promise<UnblockDomainResponse> => {
    setError(null);
    const result = await new Promise<UnblockDomainResponse>((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Unblock domain timeout'));
        delete window.onCookieUnblockDomainResponse;
        delete window.onCookieUnblockDomainError;
      }, 5000);

      window.onCookieUnblockDomainResponse = (data: UnblockDomainResponse) => {
        clearTimeout(timeout);
        resolve(data);
        delete window.onCookieUnblockDomainResponse;
        delete window.onCookieUnblockDomainError;
      };

      window.onCookieUnblockDomainError = (errorMsg: string) => {
        clearTimeout(timeout);
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCookieUnblockDomainResponse;
        delete window.onCookieUnblockDomainError;
      };

      window.cefMessage?.send('cookie_unblock_domain', [domain]);
    });

    // Re-fetch block list after successful unblock
    await fetchBlockList();
    return result;
  }, [fetchBlockList]);

  const allowThirdParty = useCallback((domain: string): Promise<AllowThirdPartyResponse> => {
    setError(null);
    return new Promise<AllowThirdPartyResponse>((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Allow third party timeout'));
        delete window.onCookieAllowThirdPartyResponse;
        delete window.onCookieAllowThirdPartyError;
      }, 5000);

      window.onCookieAllowThirdPartyResponse = (data: AllowThirdPartyResponse) => {
        clearTimeout(timeout);
        resolve(data);
        delete window.onCookieAllowThirdPartyResponse;
        delete window.onCookieAllowThirdPartyError;
      };

      window.onCookieAllowThirdPartyError = (errorMsg: string) => {
        clearTimeout(timeout);
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCookieAllowThirdPartyResponse;
        delete window.onCookieAllowThirdPartyError;
      };

      window.cefMessage?.send('cookie_allow_third_party', [domain]);
    });
  }, []);

  const removeThirdPartyAllow = useCallback((domain: string): Promise<AllowThirdPartyResponse> => {
    setError(null);
    return new Promise<AllowThirdPartyResponse>((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Remove third party allow timeout'));
        delete window.onCookieRemoveThirdPartyAllowResponse;
        delete window.onCookieRemoveThirdPartyAllowError;
      }, 5000);

      window.onCookieRemoveThirdPartyAllowResponse = (data: AllowThirdPartyResponse) => {
        clearTimeout(timeout);
        resolve(data);
        delete window.onCookieRemoveThirdPartyAllowResponse;
        delete window.onCookieRemoveThirdPartyAllowError;
      };

      window.onCookieRemoveThirdPartyAllowError = (errorMsg: string) => {
        clearTimeout(timeout);
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCookieRemoveThirdPartyAllowResponse;
        delete window.onCookieRemoveThirdPartyAllowError;
      };

      window.cefMessage?.send('cookie_remove_third_party_allow', [domain]);
    });
  }, []);

  const fetchBlockLog = useCallback((limit: number = 100, offset: number = 0) => {
    setLoading(true);
    setError(null);
    return new Promise<BlockLogEntry[]>((resolve, reject) => {
      const timeout = setTimeout(() => {
        setLoading(false);
        setBlockLog([]);
        resolve([]);
        delete window.onCookieBlockLogResponse;
        delete window.onCookieBlockLogError;
      }, 5000);

      window.onCookieBlockLogResponse = (data: BlockLogEntry[]) => {
        clearTimeout(timeout);
        setBlockLog(data);
        setLoading(false);
        resolve(data);
        delete window.onCookieBlockLogResponse;
        delete window.onCookieBlockLogError;
      };

      window.onCookieBlockLogError = (errorMsg: string) => {
        clearTimeout(timeout);
        setError(errorMsg);
        setLoading(false);
        reject(new Error(errorMsg));
        delete window.onCookieBlockLogResponse;
        delete window.onCookieBlockLogError;
      };

      window.cefMessage?.send('cookie_get_block_log', [limit.toString(), offset.toString()]);
    });
  }, []);

  const clearBlockLog = useCallback((): Promise<ClearBlockLogResponse> => {
    setError(null);
    return new Promise<ClearBlockLogResponse>((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Clear block log timeout'));
        delete window.onCookieClearBlockLogResponse;
        delete window.onCookieClearBlockLogError;
      }, 5000);

      window.onCookieClearBlockLogResponse = (data: ClearBlockLogResponse) => {
        clearTimeout(timeout);
        setBlockLog([]);
        resolve(data);
        delete window.onCookieClearBlockLogResponse;
        delete window.onCookieClearBlockLogError;
      };

      window.onCookieClearBlockLogError = (errorMsg: string) => {
        clearTimeout(timeout);
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCookieClearBlockLogResponse;
        delete window.onCookieClearBlockLogError;
      };

      window.cefMessage?.send('cookie_clear_block_log', []);
    });
  }, []);

  const fetchBlockedCount = useCallback((): Promise<BlockedCountResponse> => {
    setError(null);
    return new Promise<BlockedCountResponse>((resolve, reject) => {
      const timeout = setTimeout(() => {
        resolve({ count: 0 });
        delete window.onCookieBlockedCountResponse;
        delete window.onCookieBlockedCountError;
      }, 5000);

      window.onCookieBlockedCountResponse = (data: BlockedCountResponse) => {
        clearTimeout(timeout);
        setBlockedCount(data.count);
        resolve(data);
        delete window.onCookieBlockedCountResponse;
        delete window.onCookieBlockedCountError;
      };

      window.onCookieBlockedCountError = (errorMsg: string) => {
        clearTimeout(timeout);
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCookieBlockedCountResponse;
        delete window.onCookieBlockedCountError;
      };

      window.cefMessage?.send('cookie_get_blocked_count', []);
    });
  }, []);

  const resetBlockedCount = useCallback((): Promise<void> => {
    setError(null);
    return new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Reset blocked count timeout'));
        delete window.onCookieResetBlockedCountResponse;
        delete window.onCookieResetBlockedCountError;
      }, 5000);

      window.onCookieResetBlockedCountResponse = () => {
        clearTimeout(timeout);
        setBlockedCount(0);
        resolve();
        delete window.onCookieResetBlockedCountResponse;
        delete window.onCookieResetBlockedCountError;
      };

      window.onCookieResetBlockedCountError = (errorMsg: string) => {
        clearTimeout(timeout);
        setError(errorMsg);
        reject(new Error(errorMsg));
        delete window.onCookieResetBlockedCountResponse;
        delete window.onCookieResetBlockedCountError;
      };

      window.cefMessage?.send('cookie_reset_blocked_count', []);
    });
  }, []);

  return {
    blockedDomains,
    blockLog,
    blockedCount,
    loading,
    error,
    fetchBlockList,
    blockDomain,
    unblockDomain,
    allowThirdParty,
    removeThirdPartyAllow,
    fetchBlockLog,
    clearBlockLog,
    fetchBlockedCount,
    resetBlockedCount,
  };
};
