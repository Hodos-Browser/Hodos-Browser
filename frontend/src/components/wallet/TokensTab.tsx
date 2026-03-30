import React, { useState, useEffect, useCallback } from 'react';

interface TokenOutput {
  outputId: number;
  txid: string | null;
  vout: number;
  satoshis: number;
  description: string | null;
  createdAt: number;
  spendable: boolean;
  basket: string;
  tags?: string[];
}

const formatBasketName = (name: string) =>
  name.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());

const formatDate = (timestamp: number) => {
  const d = new Date(timestamp * 1000);
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
};

const truncateTxid = (txid: string) =>
  txid.length > 16 ? `${txid.slice(0, 8)}...${txid.slice(-8)}` : txid;

const TokensTab: React.FC = () => {
  const [tokens, setTokens] = useState<TokenOutput[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchTokens = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await fetch('http://127.0.0.1:31301/wallet/tokens');
      if (!res.ok) throw new Error(`Failed to fetch tokens: ${res.statusText}`);
      const data = await res.json();
      setTokens(data.tokens || []);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load tokens');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchTokens();
  }, [fetchTokens]);

  const openOnWoC = (txid: string) => {
    const url = `https://whatsonchain.com/tx/${txid}`;
    if ((window as any).cefMessage) {
      (window as any).cefMessage.send('tab_create', url);
    } else {
      window.open(url, '_blank');
    }
  };

  // Group tokens by basket
  const grouped = tokens.reduce<Record<string, TokenOutput[]>>((acc, token) => {
    const key = token.basket;
    if (!acc[key]) acc[key] = [];
    acc[key].push(token);
    return acc;
  }, {});

  const basketNames = Object.keys(grouped).sort();

  if (loading) {
    return (
      <div className="wd-loading" style={{ padding: '40px 0' }}>
        <div className="wd-spinner" />
      </div>
    );
  }

  return (
    <div className="wd-tokens">
      {error && (
        <div className="wd-alert error">{error}</div>
      )}

      {tokens.length === 0 ? (
        <div className="wd-empty-state" style={{ textAlign: 'center', padding: '40px 20px' }}>
          <div style={{ fontSize: '36px', marginBottom: '12px', opacity: 0.4 }}>&#x2B21;</div>
          <div style={{ color: '#9ca3af', fontSize: '14px', fontWeight: 500 }}>No tokens found</div>
          <div style={{ color: '#6b7280', fontSize: '12px', marginTop: '6px' }}>
            Tokens created by apps you interact with will appear here.
          </div>
        </div>
      ) : (
        <>
          <div style={{ color: '#9ca3af', fontSize: '12px', marginBottom: '12px' }}>
            {tokens.length} token{tokens.length !== 1 ? 's' : ''} across {basketNames.length} app{basketNames.length !== 1 ? 's' : ''}
          </div>

          {basketNames.map((basket) => (
            <div key={basket} style={{ marginBottom: '16px' }}>
              {/* Basket header */}
              <div style={{
                display: 'flex', alignItems: 'center', gap: '8px',
                marginBottom: '8px', paddingBottom: '4px',
                borderBottom: '1px solid #2d2d2d',
              }}>
                <span style={{ fontSize: '16px' }}>&#x2B21;</span>
                <span style={{ fontSize: '13px', fontWeight: 600, color: '#e5e7eb' }}>
                  {formatBasketName(basket)}
                </span>
                <span style={{ fontSize: '11px', color: '#6b7280' }}>
                  ({grouped[basket].length} token{grouped[basket].length !== 1 ? 's' : ''})
                </span>
              </div>

              {/* Token cards */}
              {grouped[basket].map((token) => (
                <div key={`${token.txid}-${token.vout}`} style={{
                  background: '#1a1a1a',
                  border: '1px solid #2d2d2d',
                  borderRadius: '8px',
                  padding: '10px 14px',
                  marginBottom: '6px',
                }}>
                  {/* Top row: description + satoshis */}
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '6px' }}>
                    <div style={{ fontSize: '13px', color: '#e5e7eb', fontWeight: 500 }}>
                      {token.description || 'Token output'}
                    </div>
                    <div style={{ fontSize: '12px', color: '#a67c00', fontWeight: 600, whiteSpace: 'nowrap', marginLeft: '12px' }}>
                      {token.satoshis.toLocaleString()} sats
                    </div>
                  </div>

                  {/* Tags */}
                  {token.tags && token.tags.length > 0 && (
                    <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', marginBottom: '6px' }}>
                      {token.tags.map((tag) => (
                        <span key={tag} style={{
                          fontSize: '10px', color: '#e5e7eb', background: '#374151',
                          borderRadius: '4px', padding: '1px 6px',
                        }}>
                          {tag}
                        </span>
                      ))}
                    </div>
                  )}

                  {/* Bottom row: date + txid + WoC link */}
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div style={{ fontSize: '11px', color: '#6b7280' }}>
                      {formatDate(token.createdAt)}
                    </div>
                    {token.txid && (
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <span style={{ fontSize: '10px', color: '#6b7280', fontFamily: 'monospace' }}>
                          {truncateTxid(token.txid)}:{token.vout}
                        </span>
                        <button
                          onClick={() => openOnWoC(token.txid!)}
                          title="View on WhatsOnChain"
                          style={{
                            background: 'none', border: 'none', cursor: 'pointer',
                            color: '#a67c00', fontSize: '11px', padding: '0 2px',
                            textDecoration: 'underline',
                          }}
                        >
                          WoC &#x2197;
                        </button>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          ))}
        </>
      )}
    </div>
  );
};

export default TokensTab;
