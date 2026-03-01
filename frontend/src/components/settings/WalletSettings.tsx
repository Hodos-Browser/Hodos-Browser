import React from 'react';
import { Switch, Typography, Box } from '@mui/material';
import { SettingsCard, SettingRow } from './SettingsCard';
import { useSettings } from '../../hooks/useSettings';

const WalletSettings: React.FC = () => {
  const { settings, updateSetting } = useSettings();

  const formatCents = (cents: number): string => {
    return `$${(cents / 100).toFixed(2)}`;
  };

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 3, color: '#e0e0e0' }}>
        Wallet
      </Typography>

      <SettingsCard title="Auto-Approve">
        <SettingRow
          label="Enable auto-approve"
          description="Automatically approve transactions within spending limits"
          control={
            <Switch
              checked={settings.wallet.autoApproveEnabled}
              onChange={(e) => updateSetting('wallet.autoApproveEnabled', e.target.checked)}
              size="small"
            />
          }
        />
      </SettingsCard>

      <SettingsCard title="Spending Limits">
        <SettingRow
          label="Per-transaction limit"
          description={`Currently: ${formatCents(settings.wallet.defaultPerTxLimitCents)}`}
          control={
            <input
              type="number"
              value={settings.wallet.defaultPerTxLimitCents}
              onChange={(e) => updateSetting('wallet.defaultPerTxLimitCents', parseInt(e.target.value) || 0)}
              style={{
                width: 100,
                padding: '6px 10px',
                border: '1px solid #444',
                borderRadius: 4,
                backgroundColor: '#2a2a2a',
                color: '#e0e0e0',
                fontSize: '0.85rem',
                outline: 'none',
                textAlign: 'right',
              }}
              onFocus={(e) => (e.target.style.borderColor = '#a67c00')}
              onBlur={(e) => (e.target.style.borderColor = '#444')}
            />
          }
        />
        <SettingRow
          label="Per-session limit"
          description={`Currently: ${formatCents(settings.wallet.defaultPerSessionLimitCents)}`}
          control={
            <input
              type="number"
              value={settings.wallet.defaultPerSessionLimitCents}
              onChange={(e) => updateSetting('wallet.defaultPerSessionLimitCents', parseInt(e.target.value) || 0)}
              style={{
                width: 100,
                padding: '6px 10px',
                border: '1px solid #444',
                borderRadius: 4,
                backgroundColor: '#2a2a2a',
                color: '#e0e0e0',
                fontSize: '0.85rem',
                outline: 'none',
                textAlign: 'right',
              }}
              onFocus={(e) => (e.target.style.borderColor = '#a67c00')}
              onBlur={(e) => (e.target.style.borderColor = '#444')}
            />
          }
        />
        <SettingRow
          label="Rate limit (per minute)"
          description="Maximum number of auto-approved transactions per minute"
          control={
            <input
              type="number"
              value={settings.wallet.defaultRateLimitPerMin}
              onChange={(e) => updateSetting('wallet.defaultRateLimitPerMin', parseInt(e.target.value) || 0)}
              style={{
                width: 100,
                padding: '6px 10px',
                border: '1px solid #444',
                borderRadius: 4,
                backgroundColor: '#2a2a2a',
                color: '#e0e0e0',
                fontSize: '0.85rem',
                outline: 'none',
                textAlign: 'right',
              }}
              onFocus={(e) => (e.target.style.borderColor = '#a67c00')}
              onBlur={(e) => (e.target.style.borderColor = '#444')}
            />
          }
        />
      </SettingsCard>
    </Box>
  );
};

export default WalletSettings;
