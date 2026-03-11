import React, { useState, useEffect, useCallback, useRef } from 'react';

const InfoTooltip: React.FC<{ text: string; align?: 'left' | 'right' }> = ({ text, align }) => {
  const [open, setOpen] = useState(false);
  return (
    <div
      className={`wd-info-tooltip-wrap${align === 'right' ? ' wd-info-tooltip-right' : ''}`}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
    >
      <span className={`wd-info-icon${open ? ' active' : ''}`} title="More info">i</span>
      {open && <div className="wd-info-popup">{text}</div>}
    </div>
  );
};

interface ActivityItem {
  txid: string;
  direction: 'sent' | 'received';
  satoshis: number;
  status: string;
  timestamp: string;
  description?: string;
  labels?: string[];
  price_usd_cents?: number | null;
  source: string;
}

interface ActivityResponse {
  items: ActivityItem[];
  total: number;
  page: number;
  page_size: number;
  current_price_usd_cents?: number | null;
}

type DirectionFilter = 'all' | 'sent' | 'received';

const PAGE_SIZE = 10;

const ActivityTab: React.FC = () => {
  const [items, setItems] = useState<ActivityItem[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [currentPrice, setCurrentPrice] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<DirectionFilter>('all');
  const [copiedTxid, setCopiedTxid] = useState<string | null>(null);

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  const fetchActivity = useCallback(async (p: number, f: DirectionFilter) => {
    try {
      setLoading(true);
      setError(null);

      const res = await fetch(
        `http://127.0.0.1:31301/wallet/activity?page=${p}&limit=${PAGE_SIZE}&filter=${f}`
      );

      if (!res.ok) throw new Error(`Failed to fetch: ${res.statusText}`);
      const data: ActivityResponse = await res.json();

      setItems(data.items || []);
      setTotal(data.total || 0);
      setCurrentPrice(data.current_price_usd_cents ?? null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load transactions');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchActivity(page, filter);
  }, [fetchActivity, page, filter]);

  const handleFilterChange = (f: DirectionFilter) => {
    setFilter(f);
    setPage(1);
  };

  const formatBsv = (sats: number): string => (sats / 100000000).toFixed(8);

  const formatUsd = (sats: number, priceInCents: number): string => {
    const price = priceInCents / 100;
    return '$' + ((sats / 100000000) * price).toFixed(2);
  };

  const truncateTxid = (txid: string): string => {
    if (!txid || txid.length <= 16) return txid;
    return `${txid.substring(0, 8)}...${txid.substring(txid.length - 8)}`;
  };

  const formatDate = (iso: string): string => {
    if (!iso) return '';
    try {
      const d = new Date(iso);
      const now = new Date();
      const diffMs = now.getTime() - d.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      if (diffMins < 1) return 'Just now';
      if (diffMins < 60) return `${diffMins}m ago`;
      const diffHours = Math.floor(diffMins / 60);
      if (diffHours < 24) return `${diffHours}h ago`;
      const diffDays = Math.floor(diffHours / 24);
      if (diffDays < 7) return `${diffDays}d ago`;
      return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: d.getFullYear() !== now.getFullYear() ? 'numeric' : undefined });
    } catch {
      return '';
    }
  };

  const handleCopyTxid = (txid: string) => {
    navigator.clipboard.writeText(txid).catch(() => {});
    setCopiedTxid(txid);
    setTimeout(() => setCopiedTxid(null), 2000);
  };

  const handleOpenWoC = (txid: string) => {
    const cef = (window as any).cefMessage;
    if (cef?.send) {
      cef.send('tab_create', `https://whatsonchain.com/tx/${txid}`);
    }
  };

  const startIdx = (page - 1) * PAGE_SIZE + 1;
  const endIdx = Math.min(page * PAGE_SIZE, total);
  const jumpInputRef = useRef<HTMLInputElement>(null);

  // Build page number buttons with ellipsis for large page counts
  const getPageNumbers = (): (number | '...')[] => {
    if (totalPages <= 7) return Array.from({ length: totalPages }, (_, i) => i + 1);
    const pages: (number | '...')[] = [1];
    if (page > 3) pages.push('...');
    const start = Math.max(2, page - 1);
    const end = Math.min(totalPages - 1, page + 1);
    for (let i = start; i <= end; i++) pages.push(i);
    if (page < totalPages - 2) pages.push('...');
    pages.push(totalPages);
    return pages;
  };

  const handleJump = () => {
    const val = parseInt(jumpInputRef.current?.value || '', 10);
    if (val >= 1 && val <= totalPages) setPage(val);
    if (jumpInputRef.current) jumpInputRef.current.value = '';
  };

  if (loading && items.length === 0) {
    return (
      <div className="wd-loading">
        <div className="wd-spinner" />
        <span>Loading transactions...</span>
      </div>
    );
  }

  return (
    <div className="wd-activity">
      {error && <div className="wd-error-banner">{error}</div>}

      {/* Filter buttons + count */}
      <div className="wd-activity-filters">
        {(['all', 'sent', 'received'] as DirectionFilter[]).map((f) => (
          <button
            key={f}
            className={`wd-filter-btn${filter === f ? ' active' : ''}`}
            onClick={() => handleFilterChange(f)}
          >
            {f === 'all' ? 'All' : f === 'sent' ? 'Sent' : 'Received'}
          </button>
        ))}
        {total > 0 && (
          <span style={{ marginLeft: 'auto', fontSize: '12px', color: '#6b7280' }}>
            Showing {startIdx}-{endIdx} of {total}
          </span>
        )}
      </div>

      {items.length === 0 && !loading ? (
        <div className="wd-empty">
          <span className="wd-empty-icon">{filter === 'all' ? '\u{1F4CB}' : filter === 'sent' ? '\u2191' : '\u2193'}</span>
          <span className="wd-empty-text">
            {filter === 'all' ? 'No transactions yet' : `No ${filter} transactions`}
          </span>
          <span className="wd-empty-sub">
            {filter === 'all' ? 'Send or receive BSV to see activity here' : 'Try switching to "All" to see all transactions'}
          </span>
        </div>
      ) : (
        <div className="wd-activity-list">
          {items.map((item, idx) => {
            const txPrice = item.price_usd_cents;
            const hasHistorical = txPrice != null && txPrice > 0;
            const usdAtTx = hasHistorical ? formatUsd(item.satoshis, txPrice!) : null;
            const usdNow = currentPrice ? formatUsd(item.satoshis, currentPrice) : null;
            const showCurrentDiff = hasHistorical && currentPrice && txPrice !== currentPrice;

            return (
              <div key={`${item.txid}-${idx}`} className="wd-activity-item">
                {/* Line 1: Direction + Desc + Date + USD */}
                <div className={`wd-activity-direction ${item.direction}`}>
                  {item.direction === 'sent' ? '\u2191' : '\u2193'}
                </div>
                <div className="wd-activity-info">
                  <div className="wd-activity-desc">
                    {item.description || (item.direction === 'sent' ? 'Sent' : 'Received')}
                  </div>
                  <div className="wd-activity-meta">
                    <span className="wd-activity-date">{formatDate(item.timestamp)}</span>
                    <span className={`wd-activity-status ${item.status}`}>{item.status}</span>
                  </div>
                </div>
                <div className="wd-activity-center">
                  {item.txid && (
                    <>
                      <button
                        className="wd-txid-pill"
                        onClick={(e) => { e.stopPropagation(); handleCopyTxid(item.txid); }}
                        title={copiedTxid === item.txid ? 'Copied!' : truncateTxid(item.txid)}
                      >
                        {copiedTxid === item.txid ? 'Copied' : 'txid'}
                      </button>
                      <button
                        className="wd-woc-btn"
                        onClick={(e) => { e.stopPropagation(); handleOpenWoC(item.txid); }}
                        title="View on WhatsOnChain"
                      >
                        <img src="/whatsonchain.png" alt="WoC" width="20" height="20" />
                      </button>
                    </>
                  )}
                  <span className="wd-activity-info-col">
                    {idx === 0 && (
                      <InfoTooltip align="right" text="Transactions are denominated in BSV — that amount never changes. The bold value shows what it was worth in USD at the time. &quot;now&quot; shows what that same BSV is worth today, since the exchange rate fluctuates." />
                    )}
                  </span>
                  <div className="wd-activity-values">
                    <div className="wd-activity-values-top">
                      <span className="wd-activity-usd-secondary">
                        {showCurrentDiff && usdNow
                          ? `now: ${item.direction === 'sent' ? '-' : '+'}${usdNow}`
                          : ''}
                      </span>
                      <span className={`wd-activity-usd ${item.direction}`}>
                        {item.direction === 'sent' ? '-' : '+'}{usdAtTx || usdNow || '--'}
                      </span>
                    </div>
                    <span className="wd-activity-bsv">
                      {item.direction === 'sent' ? '-' : '+'}{formatBsv(item.satoshis)} BSV
                    </span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="wd-pagination">
          <button
            className="wd-page-btn"
            onClick={() => setPage(1)}
            disabled={page <= 1 || loading}
            title="First page"
          >
            &laquo;
          </button>
          <button
            className="wd-page-btn"
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            disabled={page <= 1 || loading}
            title="Previous page"
          >
            &lt;
          </button>
          {getPageNumbers().map((p, idx) =>
            p === '...' ? (
              <span key={`e${idx}`} className="wd-page-ellipsis">...</span>
            ) : (
              <button
                key={p}
                className={`wd-page-btn${page === p ? ' active' : ''}`}
                onClick={() => setPage(p)}
                disabled={loading}
              >
                {p}
              </button>
            )
          )}
          <button
            className="wd-page-btn"
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            disabled={page >= totalPages || loading}
            title="Next page"
          >
            &gt;
          </button>
          <button
            className="wd-page-btn"
            onClick={() => setPage(totalPages)}
            disabled={page >= totalPages || loading}
            title="Last page"
          >
            &raquo;
          </button>
          {totalPages > 7 && (
            <div className="wd-page-jump">
              <span>Go to</span>
              <input
                ref={jumpInputRef}
                type="number"
                min={1}
                max={totalPages}
                onKeyDown={(e) => { if (e.key === 'Enter') handleJump(); }}
              />
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default ActivityTab;
