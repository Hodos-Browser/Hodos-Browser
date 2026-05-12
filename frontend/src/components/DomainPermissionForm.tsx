import React, { useState, useEffect, useCallback } from 'react';
import { HodosButton } from './HodosButton';

const FONT_FAMILY = "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif";

const COLORS = {
  gold: '#a67c00',
  textDark: '#f0f0f0',
  textMuted: '#9ca3af',
  borderLight: '#2a2d35',
  borderInput: '#555',         // brighter than borderLight — used for text input + checkbox outlines (Step 0 legibility bump)
  white: '#1a1d23',
  inputBg: '#111827',
  warningBg: 'rgba(166, 124, 0, 0.1)',
  warningBorder: '#a67c00',
  warningText: '#e6a200',
};

export interface DomainPermissionSettings {
  perTxLimitCents: number;
  perSessionLimitCents: number;
  rateLimitPerMin: number;
  maxTxPerSession: number;
  // Phase 1.5 Step 5 — Personal Info Disclosure binary toggle. Maps directly
  // to the V17 domain_permissions.identity_key_disclosure_allowed column.
  // Optional in the interface for backward compat; callers should always pass
  // the user's current selection so the column doesn't silently flip.
  identityKeyDisclosureAllowed?: boolean;
}

// Sub-permission row types fetched from Step 3 GET endpoints. Display-only
// inside the form; per-row revoke buttons call the matching DELETE endpoint.
interface SubPermissionRow {
  id: number;
  kind: 'protocol' | 'basket' | 'counterparty';
  label: string;
  sublabel?: string;
}

interface DomainPermissionFormProps {
  domain: string;
  currentSettings?: DomainPermissionSettings;
  onSave: (settings: DomainPermissionSettings) => void;
  onCancel: () => void;
}

const DomainPermissionForm: React.FC<DomainPermissionFormProps> = ({
  domain,
  currentSettings,
  onSave,
  onCancel,
}) => {
  const [perTxUsd, setPerTxUsd] = useState(
    currentSettings ? (currentSettings.perTxLimitCents / 100).toFixed(2) : '1.00'
  );
  const [perSessionUsd, setPerSessionUsd] = useState(
    currentSettings ? (currentSettings.perSessionLimitCents / 100).toFixed(2) : '10.00'
  );
  const [rateLimitPerMin, setRateLimitPerMin] = useState(
    String(currentSettings?.rateLimitPerMin ?? 30)
  );
  const [maxTxPerSession, setMaxTxPerSession] = useState(
    String(currentSettings?.maxTxPerSession ?? 100)
  );

  // Phase 1.5 Step 5 — Personal Info Disclosure section state.
  const [allowIdentityKey, setAllowIdentityKey] = useState<boolean>(
    currentSettings?.identityKeyDisclosureAllowed ?? true
  );
  const [subPermissions, setSubPermissions] = useState<SubPermissionRow[]>([]);
  const [subPermsLoading, setSubPermsLoading] = useState<boolean>(false);

  // Fetch granted sub-permissions for this domain. Display-only — revoking
  // hits the Step 3 DELETE endpoint directly. We fetch on mount + after each
  // successful revoke so the list stays accurate.
  const refreshSubPermissions = useCallback(async () => {
    setSubPermsLoading(true);
    const wallet = 'http://127.0.0.1:31301';
    const enc = encodeURIComponent(domain);
    try {
      const [protoRes, basketRes, cpRes] = await Promise.all([
        fetch(`${wallet}/domain/permissions/protocol?domain=${enc}`),
        fetch(`${wallet}/domain/permissions/basket?domain=${enc}`),
        fetch(`${wallet}/domain/permissions/counterparty?domain=${enc}`),
      ]);
      const rows: SubPermissionRow[] = [];
      if (protoRes.ok) {
        const data = await protoRes.json();
        for (const p of data.permissions ?? []) {
          rows.push({
            id: p.id,
            kind: 'protocol',
            label: `Protocol: ${p.protocolName}`,
            sublabel: `level ${p.securityLevel} · key ${p.keyId === '*' ? 'any' : p.keyId}${p.counterparty ? ` · counterparty ${p.counterparty.slice(0, 10)}…` : ''}`,
          });
        }
      }
      if (basketRes.ok) {
        const data = await basketRes.json();
        for (const b of data.permissions ?? []) {
          rows.push({
            id: b.id,
            kind: 'basket',
            label: `Basket: ${b.basket}`,
            sublabel: `${b.access} access`,
          });
        }
      }
      if (cpRes.ok) {
        const data = await cpRes.json();
        for (const cp of data.permissions ?? []) {
          rows.push({
            id: cp.id,
            kind: 'counterparty',
            label: 'Counterparty',
            sublabel: cp.counterparty.slice(0, 24) + (cp.counterparty.length > 24 ? '…' : ''),
          });
        }
      }
      setSubPermissions(rows);
    } catch (err) {
      // Endpoints might 404 on very fresh dev DBs; treat as empty list.
      console.warn('[Hodos] Could not load sub-permissions for', domain, err);
      setSubPermissions([]);
    } finally {
      setSubPermsLoading(false);
    }
  }, [domain]);

  useEffect(() => {
    refreshSubPermissions();
  }, [refreshSubPermissions]);

  const handleRevokeSubPerm = async (row: SubPermissionRow) => {
    const wallet = 'http://127.0.0.1:31301';
    try {
      const res = await fetch(`${wallet}/domain/permissions/${row.kind}?id=${row.id}`, {
        method: 'DELETE',
      });
      if (res.ok) {
        // Refresh list — soft-deleted rows drop from the active view.
        refreshSubPermissions();
      }
    } catch (err) {
      console.error('[Hodos] Failed to revoke sub-permission:', err);
    }
  };

  const isAlwaysNotify = perTxUsd === '0' && perSessionUsd === '0';

  const perTxCents = Math.round(parseFloat(perTxUsd || '0') * 100);
  const perSessionCents = Math.round(parseFloat(perSessionUsd || '0') * 100);
  const rateLimitNum = parseInt(rateLimitPerMin) || 1;

  const showWarning = perTxCents > 500 || perSessionCents > 5000;

  const handleAlwaysNotifyToggle = () => {
    if (isAlwaysNotify) {
      // Restore defaults
      setPerTxUsd('1.00');
      setPerSessionUsd('10.00');
      setMaxTxPerSession('100');
    } else {
      // Set everything to 0
      setPerTxUsd('0');
      setPerSessionUsd('0');
      setMaxTxPerSession('0');
    }
  };

  const handleSave = () => {
    onSave({
      perTxLimitCents: perTxCents,
      perSessionLimitCents: perSessionCents,
      rateLimitPerMin: rateLimitNum,
      maxTxPerSession: parseInt(maxTxPerSession) || 0,
      identityKeyDisclosureAllowed: allowIdentityKey,
    });
  };

  const inputStyle: React.CSSProperties = {
    width: '80px',
    padding: '6px 8px',
    border: `1.5px solid ${COLORS.borderInput}`,
    borderRadius: '6px',
    fontSize: '13px',
    fontFamily: FONT_FAMILY,
    textAlign: 'right',
    outline: 'none',
    background: isAlwaysNotify ? '#0f1117' : COLORS.inputBg,
    color: isAlwaysNotify ? COLORS.textMuted : COLORS.textDark,
  };

  const labelStyle: React.CSSProperties = {
    fontSize: '13px',
    color: COLORS.textDark,
    fontWeight: 500,
  };

  const descStyle: React.CSSProperties = {
    fontSize: '11px',
    color: COLORS.textMuted,
    marginTop: '2px',
  };

  const helpTextStyle: React.CSSProperties = {
    color: '#6b7280',
    fontSize: '12px',
    marginTop: '4px',
  };

  const rowStyle: React.CSSProperties = {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    marginBottom: '14px',
  };

  return (
    <div style={{ fontFamily: FONT_FAMILY }}>
      <div style={{
        fontSize: '13px',
        fontWeight: 600,
        color: COLORS.textDark,
        marginBottom: '14px',
      }}>
        Auto-approve settings for {domain.replace(/^https?:\/\//, '').replace(/^www\./, '')}
      </div>

      {/* Always notify checkbox */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '10px',
        marginBottom: '16px',
        cursor: 'pointer',
        userSelect: 'none',
      }} onClick={handleAlwaysNotifyToggle}>
        <div style={{
          width: '18px',
          height: '18px',
          borderRadius: '4px',
          border: `2px solid ${isAlwaysNotify ? COLORS.gold : COLORS.borderInput}`,
          background: isAlwaysNotify ? COLORS.gold : 'transparent',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          flexShrink: 0,
          transition: 'all 0.15s',
        }}>
          {isAlwaysNotify && (
            <span style={{ color: '#0f1117', fontSize: '12px', fontWeight: 700, lineHeight: 1 }}>&#10003;</span>
          )}
        </div>
        <div>
          <div style={{ fontSize: '13px', color: COLORS.textDark, fontWeight: 600 }}>
            Always notify me
          </div>
          <div style={{ fontSize: '11px', color: COLORS.textMuted }}>
            Ask for confirmation on every payment from this site
          </div>
        </div>
      </div>

      {/* Per-transaction limit */}
      <div style={rowStyle}>
        <div>
          <div style={labelStyle}>Per-transaction limit</div>
          <div style={descStyle}>Payments under this are auto-approved</div>
        </div>
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
            <span style={{ color: '#9ca3af', fontSize: '14px', fontWeight: 500 }}>$</span>
            <input
              type="text"
              inputMode="decimal"
              value={perTxUsd}
              onChange={e => {
                const v = e.target.value;
                if (v === '' || /^\d*\.?\d{0,2}$/.test(v)) setPerTxUsd(v);
              }}
              disabled={isAlwaysNotify}
              style={{ ...inputStyle, flex: 1 }}
            />
          </div>
          <div style={helpTextStyle}>Max auto-approved for a single payment</div>
        </div>
      </div>

      {/* Per-session limit */}
      <div style={rowStyle}>
        <div>
          <div style={labelStyle}>Per-session limit</div>
          <div style={descStyle}>Total spending before requiring approval</div>
        </div>
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
            <span style={{ color: '#9ca3af', fontSize: '14px', fontWeight: 500 }}>$</span>
            <input
              type="text"
              inputMode="decimal"
              value={perSessionUsd}
              onChange={e => {
                const v = e.target.value;
                if (v === '' || /^\d*\.?\d{0,2}$/.test(v)) setPerSessionUsd(v);
              }}
              disabled={isAlwaysNotify}
              style={{ ...inputStyle, flex: 1 }}
            />
          </div>
          <div style={helpTextStyle}>Total spending allowed before prompting again</div>
        </div>
      </div>

      {/* Rate limit */}
      <div style={rowStyle}>
        <div>
          <div style={labelStyle}>Rate limit</div>
          <div style={descStyle}>Max payment requests per minute</div>
        </div>
        <div>
          <input
            type="text"
            inputMode="numeric"
            value={rateLimitPerMin}
            onChange={e => {
              const v = e.target.value;
              if (v === '' || /^\d+$/.test(v)) setRateLimitPerMin(v);
            }}
            style={{ ...inputStyle, width: '60px' }}
          />
          <div style={helpTextStyle}>Max payment requests per minute (safety limit)</div>
        </div>
      </div>

      {/* Max transactions per session */}
      <div style={rowStyle}>
        <div>
          <div style={labelStyle}>Max transactions per session</div>
          <div style={descStyle}>Total payments allowed per session before prompting</div>
        </div>
        <div>
          <input
            type="text"
            inputMode="numeric"
            value={maxTxPerSession}
            onChange={e => {
              const v = e.target.value;
              if (v === '' || /^\d+$/.test(v)) setMaxTxPerSession(v);
            }}
            disabled={isAlwaysNotify}
            style={{ ...inputStyle, width: '60px' }}
          />
          <div style={helpTextStyle}>Total payments allowed per session before prompting</div>
        </div>
      </div>

      {/* Warning banner */}
      {showWarning && (
        <div style={{
          background: COLORS.warningBg,
          border: `1px solid ${COLORS.warningBorder}`,
          borderRadius: '8px',
          padding: '10px 14px',
          marginBottom: '14px',
          fontSize: '12px',
          color: COLORS.warningText,
          lineHeight: 1.5,
        }}>
          High limits set. Payments up to these amounts will be approved automatically without confirmation.
        </div>
      )}

      {/* ─── Phase 1.5 Step 5: Personal Info Disclosure section ────────── */}
      <div style={{
        marginTop: '8px',
        marginBottom: '14px',
        paddingTop: '14px',
        borderTop: `1px solid ${COLORS.borderLight}`,
      }}>
        <div style={{
          fontSize: '13px',
          fontWeight: 600,
          color: COLORS.textDark,
          marginBottom: '4px',
        }}>
          Personal Info Disclosure
        </div>
        <div style={{ fontSize: '11px', color: COLORS.textMuted, marginBottom: '12px' }}>
          What this site can learn about you across the Metanet.
        </div>

        {/* Identity-key binary toggle (V17 column) */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '10px',
            marginBottom: '12px',
            cursor: 'pointer',
            userSelect: 'none',
          }}
          onClick={() => setAllowIdentityKey(!allowIdentityKey)}
        >
          <div style={{
            width: '18px',
            height: '18px',
            borderRadius: '4px',
            border: `2px solid ${allowIdentityKey ? COLORS.gold : COLORS.borderInput}`,
            background: allowIdentityKey ? COLORS.gold : 'transparent',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            flexShrink: 0,
            transition: 'all 0.15s',
          }}>
            {allowIdentityKey && (
              <span style={{ color: '#0f1117', fontSize: '12px', fontWeight: 700, lineHeight: 1 }}>&#10003;</span>
            )}
          </div>
          <div>
            <div style={{ fontSize: '13px', color: COLORS.textDark, fontWeight: 600 }}>
              Allow site to identify you
              <span
                title="Lets this site read your wallet identity key — the same key uniquely identifies you across every BRC-100 site you visit."
                style={{
                  marginLeft: '6px',
                  cursor: 'help',
                  color: COLORS.textMuted,
                  fontSize: '11px',
                  border: `1px solid ${COLORS.textMuted}`,
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
            <div style={{ fontSize: '11px', color: COLORS.textMuted }}>
              When off, the wallet will prompt before sharing your identity key with this site.
            </div>
          </div>
        </div>

        {/* Granted sub-permissions list (read-only with revoke buttons) */}
        <div style={{ fontSize: '12px', color: COLORS.textMuted, marginBottom: '6px' }}>
          Granted permissions
        </div>
        {subPermsLoading ? (
          <div style={{ fontSize: '12px', color: COLORS.textMuted, fontStyle: 'italic', marginBottom: '4px' }}>
            Loading…
          </div>
        ) : subPermissions.length === 0 ? (
          <div style={{ fontSize: '12px', color: COLORS.textMuted, fontStyle: 'italic', marginBottom: '4px' }}>
            No specific protocol, basket, or counterparty grants for this site.
          </div>
        ) : (
          <div style={{
            background: '#0f1117',
            border: `1px solid ${COLORS.borderLight}`,
            borderRadius: '6px',
            padding: '8px 10px',
          }}>
            {subPermissions.map((row) => (
              <div
                key={`${row.kind}-${row.id}`}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  gap: '8px',
                  padding: '6px 0',
                  borderBottom: `1px solid ${COLORS.borderLight}`,
                  fontSize: '12px',
                }}
              >
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ color: COLORS.textDark, fontWeight: 500 }}>{row.label}</div>
                  {row.sublabel && (
                    <div style={{ color: COLORS.textMuted, fontSize: '11px', marginTop: '2px' }}>{row.sublabel}</div>
                  )}
                </div>
                <HodosButton
                  variant="secondary"
                  size="small"
                  onClick={() => handleRevokeSubPerm(row)}
                  style={{ flexShrink: 0, color: '#ef4444', borderColor: '#ef4444' }}
                >
                  Revoke
                </HodosButton>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Action buttons */}
      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', marginTop: '6px' }}>
        <HodosButton
          variant="secondary"
          size="small"
          onClick={onCancel}
        >
          Cancel
        </HodosButton>
        <HodosButton
          variant="primary"
          size="small"
          onClick={handleSave}
        >
          Save
        </HodosButton>
      </div>
    </div>
  );
};

export default DomainPermissionForm;
