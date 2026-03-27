import React, { useState, useEffect } from 'react';
import DomainPermissionForm from '../components/DomainPermissionForm';
import type { DomainPermissionSettings } from '../components/DomainPermissionForm';
import { HodosButton } from '../components/HodosButton';

const PermissionDialogPage: React.FC = () => {
  const params = new URLSearchParams(window.location.search);
  const domain = params.get('domain') || '';

  const [currentSettings, setCurrentSettings] = useState<DomainPermissionSettings | undefined>();
  const [loading, setLoading] = useState(true);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (!domain) return;
    const fetchSettings = async () => {
      try {
        const res = await fetch(`http://127.0.0.1:31301/domain/permissions?domain=${encodeURIComponent(domain)}`);
        if (res.ok) {
          const data = await res.json();
          if (data && data.trust_level === 'approved') {
            setCurrentSettings({
              perTxLimitCents: data.per_tx_limit_cents ?? 100,
              perSessionLimitCents: data.per_session_limit_cents ?? 1000,
              rateLimitPerMin: data.rate_limit_per_min ?? 30,
              maxTxPerSession: data.max_tx_per_session ?? 100,
            });
          }
        }
      } catch {
        // No existing permission
      }
      setLoading(false);
    };
    fetchSettings();
  }, [domain]);

  const handleSave = async (settings: DomainPermissionSettings) => {
    try {
      await fetch('http://127.0.0.1:31301/domain/permissions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          domain,
          trust_level: 'approved',
          per_tx_limit_cents: settings.perTxLimitCents,
          per_session_limit_cents: settings.perSessionLimitCents,
          rate_limit_per_min: settings.rateLimitPerMin,
          max_tx_per_session: settings.maxTxPerSession,
        }),
      });
      setSaved(true);
      window.cefMessage?.send('domain_permission_invalidate', domain);
      // Close this tab after a brief delay
      setTimeout(() => {
        window.cefMessage?.send('tab_close_self');
      }, 1000);
    } catch (err) {
      console.error('Failed to save permissions:', err);
    }
  };

  const handleRevoke = async () => {
    try {
      await fetch(`http://127.0.0.1:31301/domain/permissions?domain=${encodeURIComponent(domain)}`, {
        method: 'DELETE',
      });
      window.cefMessage?.send('domain_permission_invalidate', domain);
      window.cefMessage?.send('tab_close_self');
    } catch (err) {
      console.error('Failed to revoke permissions:', err);
    }
  };

  const handleCancel = () => {
    // Navigate back or close tab
    if (window.history.length > 1) {
      window.history.back();
    } else {
      window.cefMessage?.send('tab_close_self');
    }
  };

  if (!domain) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', backgroundColor: '#0f1117', color: '#9ca3af' }}>
        No domain specified
      </div>
    );
  }

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      minHeight: '100vh',
      backgroundColor: '#0f1117',
      padding: '24px',
    }}>
      <div style={{
        backgroundColor: '#1a1d23',
        borderRadius: '12px',
        padding: '24px 28px',
        width: 440,
        border: '1px solid #2a2d35',
        boxShadow: '0 8px 32px rgba(0, 0, 0, 0.6)',
      }}>
        {saved ? (
          <div style={{ textAlign: 'center', padding: '20px 0', color: '#4ade80', fontSize: '14px' }}>
            Permissions saved
          </div>
        ) : loading ? (
          <div style={{ textAlign: 'center', padding: '20px 0', color: '#9ca3af', fontSize: '13px' }}>
            Loading...
          </div>
        ) : (
          <>
            <DomainPermissionForm
              domain={domain}
              currentSettings={currentSettings}
              onSave={handleSave}
              onCancel={handleCancel}
            />
            {currentSettings && (
              <div style={{ marginTop: '12px', borderTop: '1px solid #2a2d35', paddingTop: '12px' }}>
                <HodosButton
                  variant="secondary"
                  size="small"
                  onClick={handleRevoke}
                  style={{ color: '#ef4444', borderColor: '#ef4444' }}
                >
                  Revoke All Permissions
                </HodosButton>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
};

export default PermissionDialogPage;
