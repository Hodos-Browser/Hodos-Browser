import React, { useState, useEffect, useCallback } from 'react';
import { HodosButton } from '../HodosButton';

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
          <div style={{ color: '#b0b7c3', fontSize: '14px', fontWeight: 500 }}>No tokens found</div>
          <div style={{ color: '#6b7280', fontSize: '12px', marginTop: '6px' }}>
            Tokens created by apps you interact with will appear here.
          </div>
        </div>
      ) : (
        <>
          <div style={{ color: '#b0b7c3', fontSize: '12px', marginBottom: '12px' }}>
            {tokens.length} token{tokens.length !== 1 ? 's' : ''} across {basketNames.length} app{basketNames.length !== 1 ? 's' : ''}
          </div>

          {basketNames.map((basket) => (
            <div key={basket} style={{ marginBottom: '16px' }}>
              {/* Basket header */}
              <div style={{
                display: 'flex', alignItems: 'center', gap: '8px',
                marginBottom: '8px', paddingBottom: '4px',
                borderBottom: '1px solid #363640',
              }}>
                <span style={{ fontSize: '16px' }}>&#x2B21;</span>
                <span style={{ fontSize: '13px', fontWeight: 600, color: '#ffffff' }}>
                  {formatBasketName(basket)}
                </span>
                <span style={{ fontSize: '11px', color: '#6b7280' }}>
                  ({grouped[basket].length} token{grouped[basket].length !== 1 ? 's' : ''})
                </span>
              </div>

              {/* Token cards */}
              {grouped[basket].map((token) => (
                <div key={`${token.txid}-${token.vout}`} style={{
                  background: '#1a1d23',
                  border: '1px solid #363640',
                  borderRadius: '6px',
                  padding: '12px 14px',
                  marginBottom: '6px',
                  cursor: 'pointer',
                  transition: 'background 0.15s, border-color 0.15s',
                }}
                  onMouseEnter={(e) => { e.currentTarget.style.background = '#1e2128'; e.currentTarget.style.borderColor = '#6b7280'; }}
                  onMouseLeave={(e) => { e.currentTarget.style.background = '#1a1d23'; e.currentTarget.style.borderColor = '#363640'; }}
                >
                  {/* Top row: description + satoshis */}
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '6px' }}>
                    <div style={{ fontSize: '13px', color: '#ffffff', fontWeight: 500 }}>
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
                          fontSize: '10px', color: '#ffffff', background: '#363640',
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
                      <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                        <HodosButton
                          variant="ghost"
                          size="small"
                          className="wd-txid-pill wd-txid-pill-sm"
                          onClick={() => {
                            navigator.clipboard.writeText(token.txid!).catch(() => {});
                          }}
                          title={token.txid}
                        >
                          txid
                        </HodosButton>
                        <HodosButton
                          variant="icon"
                          size="small"
                          className="wd-woc-btn"
                          onClick={() => openOnWoC(token.txid!)}
                          title="View on WhatsOnChain"
                        >
                          <img src="/whatsonchain.png" alt="WoC" width="14" height="14" />
                        </HodosButton>
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
