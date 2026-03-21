import React, { useState } from 'react';
import { HodosButton } from './HodosButton';

const FONT_FAMILY = "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif";

const COLORS = {
  gold: '#a67c00',
  textDark: '#111111',
  textMuted: '#666666',
  borderLight: '#d0d0d0',
  white: '#ffffff',
  warningBg: '#fef9e7',
  warningBorder: '#e6a200',
  warningText: '#8a6d3b',
};

export interface DomainPermissionSettings {
  perTxLimitCents: number;
  perSessionLimitCents: number;
  rateLimitPerMin: number;
  maxTxPerSession: number;
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
    });
  };

  const inputStyle: React.CSSProperties = {
    width: '80px',
    padding: '6px 8px',
    border: `1px solid ${COLORS.borderLight}`,
    borderRadius: '6px',
    fontSize: '13px',
    fontFamily: FONT_FAMILY,
    textAlign: 'right',
    outline: 'none',
    background: isAlwaysNotify ? '#f5f5f5' : COLORS.white,
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
        gap: '8px',
        marginBottom: '16px',
        cursor: 'pointer',
        userSelect: 'none',
      }} onClick={handleAlwaysNotifyToggle}>
        <div style={{
          width: '16px',
          height: '16px',
          borderRadius: '3px',
          border: `1.5px solid ${isAlwaysNotify ? '#000000' : COLORS.borderLight}`,
          background: isAlwaysNotify ? '#000000' : COLORS.white,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          flexShrink: 0,
          transition: 'all 0.15s',
        }}>
          {isAlwaysNotify && (
            <span style={{ color: COLORS.white, fontSize: '11px', fontWeight: 700, lineHeight: 1 }}>&#10003;</span>
          )}
        </div>
        <div>
          <div style={{ fontSize: '13px', color: COLORS.textDark, fontWeight: 500 }}>
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
