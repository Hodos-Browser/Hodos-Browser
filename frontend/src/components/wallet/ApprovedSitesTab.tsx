import React, { useState, useEffect, useCallback } from 'react';
import { walletFetch } from '../../services/walletApi';
import DomainPermissionsTab from '../DomainPermissionsTab';
import { HodosButton } from '../HodosButton';

interface DefaultLimits {
  defaultPerTxLimitCents: number;
  defaultPerSessionLimitCents: number;
  defaultRateLimitPerMin: number;
  defaultMaxTxPerSession: number;
  // Phase 1.5 Step 5 — V19 column. Controls whether the bundle checkbox on
  // domain_approval / manifest_connect_bundle modals starts ticked for new
  // sites. Default true preserves the Step 1 behavior.
  defaultIdentityKeyDisclosureAllowed: boolean;
  // beta.3 Phase 0.8 — V25 column. How QUIET MODE starts on a fresh site's
  // connect prompt. ⚠️ The widest grant on that screen: it covers protocols
  // and baskets the site never declared. Default true = the behaviour that
  // shipped before this was configurable.
  // beta.3 Phase 0.8 — V24 column. OFF (the default) = the connect modal's
  // limit fields start from the four values above and a site's suggestion is
  // shown beside them. ON = a site's suggested values are pre-filled instead.
  // ⛔ Either way, site-sourced fields stay visibly marked and "Use my
  // defaults" still works — the toggle changes WHICH values are pre-filled and
  // nothing else (`R-PROV`, `P0.8-A12`).
  defaultPrefillFromManifest: boolean;
}

const ApprovedSitesTab: React.FC = () => {
  const [defaults, setDefaults] = useState<DefaultLimits>({
    defaultPerTxLimitCents: 100,      // $1
    defaultPerSessionLimitCents: 1000, // $10
    defaultRateLimitPerMin: 30,
    defaultMaxTxPerSession: 100,
    defaultIdentityKeyDisclosureAllowed: true,
    defaultPrefillFromManifest: false,
  });
  const [savedDefaults, setSavedDefaults] = useState<DefaultLimits>(defaults);
  const [perTxUsd, setPerTxUsd] = useState('1.00');
  const [perSessionUsd, setPerSessionUsd] = useState('10.00');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'saved' | null>(null);
  const [saveResult, setSaveResult] = useState<{ type: 'success' | 'error'; message: string } | null>(null);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [resetting, setResetting] = useState(false);

  const fetchDefaults = useCallback(async () => {
    try {
      setLoading(true);
      const res = await walletFetch('/wallet/settings');
      if (!res.ok) throw new Error('Failed to fetch settings');
      const data = await res.json();
      const loaded: DefaultLimits = {
        defaultPerTxLimitCents: data.default_per_tx_limit_cents ?? 100,
        defaultPerSessionLimitCents: data.default_per_session_limit_cents ?? 1000,
        defaultRateLimitPerMin: data.default_rate_limit_per_min ?? 30,
        defaultMaxTxPerSession: data.default_max_tx_per_session ?? 100,
        defaultIdentityKeyDisclosureAllowed: data.default_identity_key_disclosure_allowed ?? true,
        defaultPrefillFromManifest: data.default_prefill_from_manifest ?? false,
      };
      setDefaults(loaded);
      setSavedDefaults(loaded);
      setPerTxUsd((loaded.defaultPerTxLimitCents / 100).toFixed(2));
      setPerSessionUsd((loaded.defaultPerSessionLimitCents / 100).toFixed(2));
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
      const postRes = await walletFetch('/wallet/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          default_per_tx_limit_cents: defaults.defaultPerTxLimitCents,
          default_per_session_limit_cents: defaults.defaultPerSessionLimitCents,
          default_rate_limit_per_min: defaults.defaultRateLimitPerMin,
          default_max_tx_per_session: defaults.defaultMaxTxPerSession,
          default_identity_key_disclosure_allowed: defaults.defaultIdentityKeyDisclosureAllowed,
          default_prefill_from_manifest: defaults.defaultPrefillFromManifest,
        }),
      });
      if (!postRes.ok) throw new Error('Failed to save defaults');

      // Re-fetch to confirm saved values
      const getRes = await walletFetch('/wallet/settings');
      if (getRes.ok) {
        const data = await getRes.json();
        const confirmed: DefaultLimits = {
          defaultPerTxLimitCents: data.default_per_tx_limit_cents ?? defaults.defaultPerTxLimitCents,
          defaultPerSessionLimitCents: data.default_per_session_limit_cents ?? defaults.defaultPerSessionLimitCents,
          defaultRateLimitPerMin: data.default_rate_limit_per_min ?? defaults.defaultRateLimitPerMin,
          defaultMaxTxPerSession: data.default_max_tx_per_session ?? defaults.defaultMaxTxPerSession,
          defaultIdentityKeyDisclosureAllowed: data.default_identity_key_disclosure_allowed ?? defaults.defaultIdentityKeyDisclosureAllowed,
          defaultPrefillFromManifest: data.default_prefill_from_manifest ?? defaults.defaultPrefillFromManifest,
        };
        setDefaults(confirmed);
        setSavedDefaults(confirmed);
        setPerTxUsd((confirmed.defaultPerTxLimitCents / 100).toFixed(2));
        setPerSessionUsd((confirmed.defaultPerSessionLimitCents / 100).toFixed(2));
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
      const res = await walletFetch('/domain/permissions/reset-all', {
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
      // Reset-all touches every domain in the table, so clear the entire C++
      // DomainPermissionCache. The IPC handler treats no-domain-arg as a
      // full clear (see simple_handler.cpp::domain_permission_invalidate).
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (window as any).cefMessage?.send('domain_permission_invalidate', []);
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
                  <span style={{ color: '#9ca3af', fontSize: '13px', fontWeight: 500 }}>$</span>
                  <input
                    type="text"
                    inputMode="decimal"
                    value={perTxUsd}
                    onChange={(e) => {
                      const v = e.target.value;
                      if (v === '' || /^\d*\.?\d{0,2}$/.test(v)) {
                        setPerTxUsd(v);
                        setDefaults((d) => ({ ...d, defaultPerTxLimitCents: Math.max(0, Math.round(parseFloat(v || '0') * 100)) }));
                      }
                    }}
                  />
                </div>
                <div style={{ color: '#9ca3af', fontSize: '10px', marginTop: '2px' }}>
                  Max auto-approved per payment
                </div>
              </div>
              <div className="wd-default-field">
                <label>Per-Session Limit</label>
                <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                  <span style={{ color: '#9ca3af', fontSize: '13px', fontWeight: 500 }}>$</span>
                  <input
                    type="text"
                    inputMode="decimal"
                    value={perSessionUsd}
                    onChange={(e) => {
                      const v = e.target.value;
                      if (v === '' || /^\d*\.?\d{0,2}$/.test(v)) {
                        setPerSessionUsd(v);
                        setDefaults((d) => ({ ...d, defaultPerSessionLimitCents: Math.max(0, Math.round(parseFloat(v || '0') * 100)) }));
                      }
                    }}
                  />
                </div>
                <div style={{ color: '#9ca3af', fontSize: '10px', marginTop: '2px' }}>
                  Total spending before prompting
                </div>
              </div>
              <div className="wd-default-field">
                <label>Rate Limit (/min)</label>
                <input
                  type="number"
                  step="1"
                  min="1"
                  value={defaults.defaultRateLimitPerMin}
                  onChange={(e) => setDefaults((d) => ({ ...d, defaultRateLimitPerMin: Math.max(1, parseInt(e.target.value || '1', 10)) }))}
                />
                <div style={{ color: '#9ca3af', fontSize: '10px', marginTop: '2px' }}>
                  Max requests per minute
                </div>
              </div>
              <div className="wd-default-field">
                <label>Max Tx/Session</label>
                <input
                  type="number"
                  step="1"
                  min="0"
                  value={defaults.defaultMaxTxPerSession}
                  onChange={(e) => setDefaults((d) => ({ ...d, defaultMaxTxPerSession: Math.max(0, parseInt(e.target.value || '0', 10)) }))}
                />
                <div style={{ color: '#9ca3af', fontSize: '10px', marginTop: '2px' }}>
                  Payments per session before prompting
                </div>
              </div>
            </div>

            {/* Phase 1.5 Step 5 — default identity-key bundle toggle (V19 column) */}
            {/* beta.3 Phase 0.8 UI follow-up (owner-requested 2026-08-23) — the
                two "how should a FRESH site's prompt start?" toggles share one
                horizontal line, pre-fill pushed to the right.

                ⚠️ `flexWrap: 'wrap'` is load-bearing, not decoration. Two
                checkbox+label pairs side by side is the exact shape that clips
                at 125%/1366 and 150%/1366 (DPI_RESOLUTION_TEST_MATRIX.md cells
                #4/#6/#9), and both labels here are long. Wrapping to two rows
                is the correct degradation; truncating a consent label is not.
                `marginLeft: 'auto'` right-justifies only while they share a
                line — once wrapped the second falls to the left, as it should. */}
            <div style={{
              marginTop: '14px',
              paddingTop: '14px',
              borderTop: '1px solid #2a2d35',
              display: 'flex',
              flexWrap: 'wrap',
              alignItems: 'flex-start',
              gap: '12px 24px',
            }}>
            <div style={{
              flex: '1 1 220px',
              display: 'flex',
              alignItems: 'center',
              gap: '10px',
              cursor: 'pointer',
              userSelect: 'none',
            }} onClick={() => setDefaults((d) => ({ ...d, defaultIdentityKeyDisclosureAllowed: !d.defaultIdentityKeyDisclosureAllowed }))}>
              <div style={{
                width: '18px',
                height: '18px',
                borderRadius: '4px',
                border: `2px solid ${defaults.defaultIdentityKeyDisclosureAllowed ? '#a67c00' : '#555'}`,
                background: defaults.defaultIdentityKeyDisclosureAllowed ? '#a67c00' : 'transparent',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                flexShrink: 0,
                transition: 'all 0.15s',
              }}>
                {defaults.defaultIdentityKeyDisclosureAllowed && (
                  <span style={{ color: '#0f1117', fontSize: '12px', fontWeight: 700, lineHeight: 1 }}>&#10003;</span>
                )}
              </div>
              <div>
                <div style={{ fontSize: '13px', color: '#f0f0f0', fontWeight: 600 }}>
                  Allow identity-key disclosure by default for new sites
                  <span
                    title="When a fresh site asks to connect, the 'Allow this site to identify you' checkbox starts ticked. Untick to make new sites prompt separately before sharing your identity key."
                    style={{
                      marginLeft: '6px',
                      cursor: 'help',
                      color: '#9ca3af',
                      fontSize: '11px',
                      border: '1px solid #9ca3af',
                      borderRadius: '50%',
                      width: '14px',
                      height: '14px',
                      display: 'inline-flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      fontWeight: 600,
                      lineHeight: 1,
                      verticalAlign: 'middle',
                    }}
                  >i</span>
                </div>
                <div style={{ color: '#9ca3af', fontSize: '11px', marginTop: '2px' }}>
                  Controls the default state of the bundle checkbox on the first-visit connect prompt.
                </div>
              </div>
            </div>

            {/* beta.3 Phase 0.8 — the pre-fill opt-in (V24). Grouped with the
                identity-key default because both answer "how should a FRESH
                site's connect prompt start?", and both are one-line toggles.
                ⛔ Ships OFF: the user is opting into letting a *site* choose
                the starting numbers on their own consent prompt, so the detail
                lives in the tooltip but the risk is named in the label. */}
            <div style={{
              // Owner-directed 2026-08-23: three toggles share the row and are
              // evenly spaced, so each takes an equal share and wraps together.
              flex: '1 1 220px',
              display: 'flex',
              alignItems: 'center',
              gap: '10px',
              cursor: 'pointer',
              userSelect: 'none',
            }} onClick={() => setDefaults((d) => ({ ...d, defaultPrefillFromManifest: !d.defaultPrefillFromManifest }))}>
              <div style={{
                width: '18px',
                height: '18px',
                borderRadius: '4px',
                border: `2px solid ${defaults.defaultPrefillFromManifest ? '#a67c00' : '#555'}`,
                background: defaults.defaultPrefillFromManifest ? '#a67c00' : 'transparent',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                flexShrink: 0,
                transition: 'all 0.15s',
              }}>
                {defaults.defaultPrefillFromManifest && (
                  <span style={{ color: '#0f1117', fontSize: '12px', fontWeight: 700, lineHeight: 1 }}>&#10003;</span>
                )}
              </div>
              <div style={{ fontSize: '13px', color: '#f0f0f0', fontWeight: 600 }}>
                Pre-fill new-site limits with the site&apos;s recommended settings
                <span
                  title={"Off by default. When on, a site that publishes recommended spending limits starts "
                    + "the connect prompt with ITS numbers instead of yours. They stay clearly marked as the "
                    + "site's suggestion and you can always revert with \u201cUse my defaults\u201d, but you "
                    + "would be letting a site choose the starting point. Leave this off to start every new "
                    + "site from your own limits above and adopt a site's numbers only when you choose to."}
                  style={{
                    marginLeft: '6px',
                    cursor: 'help',
                    color: '#9ca3af',
                    fontSize: '11px',
                    border: '1px solid #9ca3af',
                    borderRadius: '50%',
                    width: '14px',
                    height: '14px',
                    display: 'inline-flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontWeight: 600,
                    lineHeight: 1,
                    verticalAlign: 'middle',
                  }}
                >i</span>
              </div>
            </div>

            {/* [P7c] DELETED the "Start new sites in quiet mode" default.
                It set the initial state of a connect-screen checkbox that no
                longer exists, for an engine flag that is no longer read. Its
                tooltip promised a site could use "ANY protocol or basket ...
                including ones it never declared" - which is exactly the
                behaviour Phase 7c removed, so keeping the control would have
                let the user set a preference the wallet cannot honour.
                New sites now grant exactly what their manifest declared and
                the user left ticked; a site with no manifest grants nothing
                and prompts on first use. There is no default to choose. */}
            </div>

            {/* Owner-reported 2026-08-23: the Save/Reset row sat too close to
                the toggles above it and read as part of them. */}
            <div className="wd-defaults-actions" style={{ marginTop: '20px' }}>
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
