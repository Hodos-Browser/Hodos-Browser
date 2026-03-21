import React, { useState, useEffect, useCallback } from 'react';
import DomainPermissionsTab from '../DomainPermissionsTab';
import { HodosButton } from '../HodosButton';

interface DefaultLimits {
  defaultPerTxLimitCents: number;
  defaultPerSessionLimitCents: number;
  defaultRateLimitPerMin: number;
  defaultMaxTxPerSession: number;
}

const ApprovedSitesTab: React.FC = () => {
  const [defaults, setDefaults] = useState<DefaultLimits>({
    defaultPerTxLimitCents: 100,      // $1
    defaultPerSessionLimitCents: 1000, // $10
    defaultRateLimitPerMin: 30,
    defaultMaxTxPerSession: 100,
  });
  const [savedDefaults, setSavedDefaults] = useState<DefaultLimits>(defaults);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'saved' | null>(null);
  const [saveResult, setSaveResult] = useState<{ type: 'success' | 'error'; message: string } | null>(null);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [resetting, setResetting] = useState(false);

  const fetchDefaults = useCallback(async () => {
    try {
      setLoading(true);
      const res = await fetch('http://127.0.0.1:31301/wallet/settings');
      if (!res.ok) throw new Error('Failed to fetch settings');
      const data = await res.json();
      const loaded: DefaultLimits = {
        defaultPerTxLimitCents: data.default_per_tx_limit_cents ?? 100,
        defaultPerSessionLimitCents: data.default_per_session_limit_cents ?? 1000,
        defaultRateLimitPerMin: data.default_rate_limit_per_min ?? 30,
        defaultMaxTxPerSession: data.default_max_tx_per_session ?? 100,
      };
      setDefaults(loaded);
      setSavedDefaults(loaded);
    } catch {
      // Use current defaults if endpoint doesn't exist yet
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchDefaults();
  }, [fetchDefaults]);

  const hasChanges = JSON.stringify(defaults) !== JSON.stringify(savedDefaults);

  const handleSaveDefaults = async () => {
    try {
      setSaving(true);
      setSaveResult(null);
      setSaveStatus(null);
      const postRes = await fetch('http://127.0.0.1:31301/wallet/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          default_per_tx_limit_cents: defaults.defaultPerTxLimitCents,
          default_per_session_limit_cents: defaults.defaultPerSessionLimitCents,
          default_rate_limit_per_min: defaults.defaultRateLimitPerMin,
          default_max_tx_per_session: defaults.defaultMaxTxPerSession,
        }),
      });
      if (!postRes.ok) throw new Error('Failed to save defaults');

      // Re-fetch to confirm saved values
      const getRes = await fetch('http://127.0.0.1:31301/wallet/settings');
      if (getRes.ok) {
        const data = await getRes.json();
        const confirmed: DefaultLimits = {
          defaultPerTxLimitCents: data.default_per_tx_limit_cents ?? defaults.defaultPerTxLimitCents,
          defaultPerSessionLimitCents: data.default_per_session_limit_cents ?? defaults.defaultPerSessionLimitCents,
          defaultRateLimitPerMin: data.default_rate_limit_per_min ?? defaults.defaultRateLimitPerMin,
          defaultMaxTxPerSession: data.default_max_tx_per_session ?? defaults.defaultMaxTxPerSession,
        };
        setDefaults(confirmed);
        setSavedDefaults(confirmed);
      } else {
        setSavedDefaults(defaults);
      }

      setSaveStatus('saved');
      setTimeout(() => setSaveStatus(null), 2000);
    } catch (err) {
      setSaveResult({ type: 'error', message: err instanceof Error ? err.message : 'Save failed' });
    } finally {
      setSaving(false);
    }
  };

  const handleResetAll = async () => {
    try {
      setResetting(true);
      const res = await fetch('http://127.0.0.1:31301/domain/permissions/reset-all', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          per_tx_limit_cents: defaults.defaultPerTxLimitCents,
          per_session_limit_cents: defaults.defaultPerSessionLimitCents,
          rate_limit_per_min: defaults.defaultRateLimitPerMin,
          max_tx_per_session: defaults.defaultMaxTxPerSession,
        }),
      });
      if (!res.ok) throw new Error('Failed to reset permissions');
      setShowResetConfirm(false);
      setSaveResult({ type: 'success', message: 'All sites reset to default limits' });
      setTimeout(() => setSaveResult(null), 3000);
    } catch (err) {
      setSaveResult({ type: 'error', message: err instanceof Error ? err.message : 'Reset failed' });
    } finally {
      setResetting(false);
    }
  };

  return (
    <div className="wd-approved-sites">
      {/* Default Limits Section */}
      <div className="wd-defaults-card">
        <div className="wd-defaults-title">Default Limits for New Sites</div>

        {saveResult && (
          <div className={`wd-alert ${saveResult.type}`}>{saveResult.message}</div>
        )}

        {loading ? (
          <div className="wd-loading" style={{ padding: '12px 0' }}>
            <div className="wd-spinner" />
          </div>
        ) : (
          <>
            <div className="wd-defaults-grid">
              <div className="wd-default-field">
                <label>Per-Transaction Limit</label>
                <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                  <span style={{ color: '#9ca3af', fontSize: '14px', fontWeight: 500 }}>$</span>
                  <input
                    type="number"
                    step="1"
                    min="0"
                    value={(defaults.defaultPerTxLimitCents / 100).toFixed(0)}
                    onChange={(e) => setDefaults((d) => ({ ...d, defaultPerTxLimitCents: Math.max(0, parseInt(e.target.value || '0', 10) * 100) }))}
                  />
                </div>
                <div style={{ color: '#9ca3af', fontSize: '11px', marginTop: '3px' }}>
                  Max auto-approved for a single payment
                </div>
              </div>
              <div className="wd-default-field">
                <label>Per-Session Limit</label>
                <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                  <span style={{ color: '#9ca3af', fontSize: '14px', fontWeight: 500 }}>$</span>
                  <input
                    type="number"
                    step="1"
                    min="0"
                    value={(defaults.defaultPerSessionLimitCents / 100).toFixed(0)}
                    onChange={(e) => setDefaults((d) => ({ ...d, defaultPerSessionLimitCents: Math.max(0, parseInt(e.target.value || '0', 10) * 100) }))}
                  />
                </div>
                <div style={{ color: '#9ca3af', fontSize: '11px', marginTop: '3px' }}>
                  Total spending allowed before prompting again
                </div>
              </div>
              <div className="wd-default-field">
                <label>Rate Limit (per minute)</label>
                <input
                  type="number"
                  step="1"
                  min="1"
                  value={defaults.defaultRateLimitPerMin}
                  onChange={(e) => setDefaults((d) => ({ ...d, defaultRateLimitPerMin: Math.max(1, parseInt(e.target.value || '1', 10)) }))}
                />
                <div style={{ color: '#9ca3af', fontSize: '11px', marginTop: '3px' }}>
                  Max payment requests per minute (safety limit)
                </div>
              </div>
              <div className="wd-default-field">
                <label>Max Transactions per Session</label>
                <input
                  type="number"
                  step="1"
                  min="0"
                  value={defaults.defaultMaxTxPerSession}
                  onChange={(e) => setDefaults((d) => ({ ...d, defaultMaxTxPerSession: Math.max(0, parseInt(e.target.value || '0', 10)) }))}
                />
                <div style={{ color: '#9ca3af', fontSize: '11px', marginTop: '3px' }}>
                  Total payments allowed per session before prompting
                </div>
              </div>
            </div>

            <div className="wd-defaults-actions">
              <HodosButton
                variant="primary"
                onClick={handleSaveDefaults}
                disabled={!hasChanges}
                loading={saving}
                loadingText="Saving..."
              >
                Save Defaults
              </HodosButton>
              {saveStatus === 'saved' && (
                <span style={{ color: '#2e7d32', fontSize: '13px', marginLeft: '8px' }}>Saved</span>
              )}
              <HodosButton
                variant="secondary"
                onClick={() => setShowResetConfirm(true)}
              >
                Reset All Sites to Defaults
              </HodosButton>
            </div>
          </>
        )}
      </div>

      {/* Per-Site Permissions */}
      <DomainPermissionsTab />

      {/* Reset Confirmation Modal */}
      {showResetConfirm && (
        <div className="wd-modal-overlay" onClick={() => setShowResetConfirm(false)}>
          <div className="wd-modal" onClick={(e) => e.stopPropagation()}>
            <div className="wd-modal-title">Reset All Site Permissions?</div>
            <div className="wd-modal-body">
              This will update all existing approved sites to the current default limits:
              <br /><br />
              Per-Transaction: ${(defaults.defaultPerTxLimitCents / 100).toFixed(2)}<br />
              Per-Session: ${(defaults.defaultPerSessionLimitCents / 100).toFixed(2)}<br />
              Rate: {defaults.defaultRateLimitPerMin}/min<br />
              Max Tx: {defaults.defaultMaxTxPerSession}/session
            </div>
            <div className="wd-modal-actions">
              <HodosButton variant="secondary" onClick={() => setShowResetConfirm(false)}>
                Cancel
              </HodosButton>
              <HodosButton
                variant="primary"
                onClick={handleResetAll}
                loading={resetting}
                loadingText="Resetting..."
              >
                Reset All Sites
              </HodosButton>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default ApprovedSitesTab;
