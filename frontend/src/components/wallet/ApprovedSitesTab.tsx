import React, { useState, useEffect, useCallback } from 'react';
import DomainPermissionsTab from '../DomainPermissionsTab';

interface DefaultLimits {
  defaultPerTxLimitCents: number;
  defaultPerSessionLimitCents: number;
  defaultRateLimitPerMin: number;
}

const ApprovedSitesTab: React.FC = () => {
  const [defaults, setDefaults] = useState<DefaultLimits>({
    defaultPerTxLimitCents: 1000,     // $10
    defaultPerSessionLimitCents: 5000, // $50
    defaultRateLimitPerMin: 10,
  });
  const [savedDefaults, setSavedDefaults] = useState<DefaultLimits>(defaults);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
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
        defaultPerTxLimitCents: data.default_per_tx_limit_cents ?? 1000,
        defaultPerSessionLimitCents: data.default_per_session_limit_cents ?? 5000,
        defaultRateLimitPerMin: data.default_rate_limit_per_min ?? 10,
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
      const res = await fetch('http://127.0.0.1:31301/wallet/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          default_per_tx_limit_cents: defaults.defaultPerTxLimitCents,
          default_per_session_limit_cents: defaults.defaultPerSessionLimitCents,
          default_rate_limit_per_min: defaults.defaultRateLimitPerMin,
        }),
      });
      if (!res.ok) throw new Error('Failed to save defaults');
      setSavedDefaults(defaults);
      setSaveResult({ type: 'success', message: 'Default limits saved' });
      setTimeout(() => setSaveResult(null), 3000);
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
                <label>Per-Transaction Limit (USD)</label>
                <input
                  type="number"
                  step="1"
                  min="0"
                  value={(defaults.defaultPerTxLimitCents / 100).toFixed(0)}
                  onChange={(e) => setDefaults((d) => ({ ...d, defaultPerTxLimitCents: Math.max(0, parseInt(e.target.value || '0', 10) * 100) }))}
                />
              </div>
              <div className="wd-default-field">
                <label>Per-Session Limit (USD)</label>
                <input
                  type="number"
                  step="1"
                  min="0"
                  value={(defaults.defaultPerSessionLimitCents / 100).toFixed(0)}
                  onChange={(e) => setDefaults((d) => ({ ...d, defaultPerSessionLimitCents: Math.max(0, parseInt(e.target.value || '0', 10) * 100) }))}
                />
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
              </div>
            </div>

            <div className="wd-defaults-actions">
              <button
                className="wd-btn-primary"
                onClick={handleSaveDefaults}
                disabled={saving || !hasChanges}
              >
                {saving ? 'Saving...' : 'Save Defaults'}
              </button>
              <button
                className="wd-btn-secondary"
                onClick={() => setShowResetConfirm(true)}
              >
                Reset All Sites to Defaults
              </button>
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
              Rate: {defaults.defaultRateLimitPerMin}/min
            </div>
            <div className="wd-modal-actions">
              <button className="wd-btn-secondary" onClick={() => setShowResetConfirm(false)}>
                Cancel
              </button>
              <button
                className="wd-btn-primary"
                onClick={handleResetAll}
                disabled={resetting}
              >
                {resetting ? 'Resetting...' : 'Reset All Sites'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default ApprovedSitesTab;
