import React, { useState, useEffect } from 'react';
import DomainPermissionForm from '../components/DomainPermissionForm';
import type { DomainPermissionSettings } from '../components/DomainPermissionForm';
import { HodosButton } from '../components/HodosButton';

const FONT_FAMILY = "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif";

// Brand colors (matching WalletPanel.css)
const COLORS = {
  primary: '#a67c00',
  primaryHover: '#bf9000',
  gold: '#a67c00',
  subduedGold: '#111827',
  textDark: '#f0f0f0',
  textLight: '#f0f0f0',
  textMuted: '#9ca3af',
  borderLight: '#2a2d35',
  white: '#1a1d23',
  error: '#c62828',
  errorBg: 'rgba(211, 47, 47, 0.1)',
};

const BRC100AuthOverlayRoot: React.FC = () => {
  const [notificationType, setNotificationType] = useState<string>('');
  const [notificationDomain, setNotificationDomain] = useState<string>('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showModifyLimits, setShowModifyLimits] = useState(false);

  // Payment/rate-limit params
  const [paymentSatoshis, setPaymentSatoshis] = useState<number>(0);
  const [paymentCents, setPaymentCents] = useState<number>(0);
  const [exceededLimit, setExceededLimit] = useState<string>('');
  const [perTxLimit, setPerTxLimit] = useState<number>(10);
  const [perSessionLimit, setPerSessionLimit] = useState<number>(300);
  const [sessionSpent, setSessionSpent] = useState<number>(0);
  const [rateLimit, setRateLimit] = useState<number>(10);

  // Certificate disclosure params
  const [certFields, setCertFields] = useState<string[]>([]);
  const [selectedFields, setSelectedFields] = useState<string[]>([]);
  const [certType, setCertType] = useState<string>('');
  const [certifier, setCertifier] = useState<string>('');
  const [rememberFields, setRememberFields] = useState<boolean>(true);

  // Apply notification params from a query string (used by both initial load and JS injection)
  const applyParams = (queryString: string) => {
    const params = new URLSearchParams(queryString);
    const type = params.get('type') || '';
    const domain = params.get('domain') || '';

    // Reset UI state for fresh notification
    setShowAdvanced(false);
    setShowModifyLimits(false);

    // Reset payment defaults
    setPaymentSatoshis(0);
    setPaymentCents(0);
    setExceededLimit('');
    setPerTxLimit(10);
    setPerSessionLimit(300);
    setSessionSpent(0);
    setRateLimit(10);

    // Apply params
    setNotificationType(type);
    setNotificationDomain(domain);

    const satoshis = params.get('satoshis');
    const cents = params.get('cents');
    if (satoshis) setPaymentSatoshis(parseInt(satoshis));
    if (cents) setPaymentCents(parseInt(cents));

    const exceeded = params.get('exceededLimit');
    if (exceeded) setExceededLimit(exceeded);

    const txLimit = params.get('perTxLimit');
    if (txLimit) setPerTxLimit(parseInt(txLimit));

    const sessLimit = params.get('perSessionLimit');
    if (sessLimit) setPerSessionLimit(parseInt(sessLimit));

    const sessSpent = params.get('sessionSpent');
    if (sessSpent) setSessionSpent(parseInt(sessSpent));

    const rateLimitParam = params.get('rateLimit');
    if (rateLimitParam) setRateLimit(parseInt(rateLimitParam));

    // Certificate disclosure params
    const fieldsParam = params.get('fields');
    if (fieldsParam) {
      const fields = fieldsParam.split(',').filter(f => f.length > 0);
      setCertFields(fields);
      setSelectedFields([...fields]); // All selected by default
    } else {
      setCertFields([]);
      setSelectedFields([]);
    }
    const certTypeParam = params.get('certType');
    setCertType(certTypeParam || '');
    const certifierParam = params.get('certifier');
    setCertifier(certifierParam || '');
    setRememberFields(true);
  };

  useEffect(() => {
    // Register JS injection callbacks for C++ to call (avoids full page navigation)
    (window as any).showNotification = (queryString: string) => {
      applyParams(queryString);
    };
    (window as any).hideNotification = () => {
      setNotificationType('');
      setNotificationDomain('');
    };

    // Initial load: parse URL params (backward compat + first page load)
    const search = window.location.search;
    if (search) {
      applyParams(search.startsWith('?') ? search.substring(1) : search);
    }

    return () => {
      delete (window as any).showNotification;
      delete (window as any).hideNotification;
    };
  }, []);

  const formatDomain = (domain: string) => {
    return domain.replace(/^https?:\/\//, '').replace(/^www\./, '');
  };

  const getDomainInitial = (domain: string) => {
    const clean = formatDomain(domain);
    return clean.charAt(0).toUpperCase();
  };

  const formatSatoshis = (sats: number): string => {
    if (sats >= 100_000_000) {
      return (sats / 100_000_000).toFixed(8) + ' BSV';
    } else if (sats >= 1000) {
      return (sats / 1000).toFixed(3) + 'k sats';
    }
    return sats.toLocaleString() + ' sats';
  };

  const formatUsdCents = (cents: number): string => {
    return '$' + (cents / 100).toFixed(2);
  };

  const getLimitExplanation = (): string => {
    if (exceededLimit === 'per_tx') {
      return `This payment of ${formatUsdCents(paymentCents)} exceeds your per-transaction limit of ${formatUsdCents(perTxLimit)} for this site.`;
    } else if (exceededLimit === 'per_session') {
      return `You've spent ${formatUsdCents(sessionSpent)} this session. This payment of ${formatUsdCents(paymentCents)} would exceed your session limit of ${formatUsdCents(perSessionLimit)}.`;
    } else if (exceededLimit === 'both') {
      return `This payment of ${formatUsdCents(paymentCents)} exceeds both your per-transaction limit (${formatUsdCents(perTxLimit)}) and session limit (${formatUsdCents(perSessionLimit)}).`;
    }
    return 'This payment exceeds your auto-approve limits for this site.';
  };

  // ── Domain Approval: Allow ──
  const handleAllow = () => {
    try {
      if (window.cefMessage) {
        // Set domain permission to "approved" (sets cache + DB write)
        window.cefMessage.send('add_domain_permission', [
          JSON.stringify({ domain: notificationDomain }),
        ]);
        // Tell the interceptor to forward the pending request
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: true, whitelist: true }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error handling domain approval:', error);
    }
  };

  // ── Domain Approval: Allow with Advanced Settings ──
  const handleAllowAdvanced = (settings: DomainPermissionSettings) => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('add_domain_permission_advanced', [
          JSON.stringify({
            domain: notificationDomain,
            perTxLimitCents: settings.perTxLimitCents,
            perSessionLimitCents: settings.perSessionLimitCents,
            rateLimitPerMin: settings.rateLimitPerMin,
          }),
        ]);
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: true, whitelist: true }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error handling advanced domain approval:', error);
    }
  };

  // ── Domain Approval: Block ──
  const handleBlock = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: false, whitelist: false }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error handling domain rejection:', error);
    }
  };

  // ── Payment Confirmation: Approve ──
  const handlePaymentApprove = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: true }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error approving payment:', error);
    }
  };

  // ── Payment Confirmation: Deny ──
  const handlePaymentDeny = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: false }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error denying payment:', error);
    }
  };

  // ── Payment/Rate-Limit: Modify Limits + Approve ──
  const handleModifyLimitsAndApprove = (settings: DomainPermissionSettings) => {
    try {
      if (window.cefMessage) {
        // Update the site's limits in DB + cache
        window.cefMessage.send('add_domain_permission_advanced', [
          JSON.stringify({
            domain: notificationDomain,
            perTxLimitCents: settings.perTxLimitCents,
            perSessionLimitCents: settings.perSessionLimitCents,
            rateLimitPerMin: settings.rateLimitPerMin,
          }),
        ]);
        // Approve this request
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: true }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error modifying limits:', error);
    }
  };

  // ── Certificate Disclosure: Share (selected fields only) ──
  const handleCertApprove = () => {
    if (selectedFields.length === 0) return; // Nothing selected
    try {
      if (window.cefMessage) {
        // Persist field approval if "Remember" is checked
        if (rememberFields && selectedFields.length > 0 && certType) {
          window.cefMessage.send('approve_cert_fields', [
            JSON.stringify({
              domain: notificationDomain,
              certType: certType,
              fields: selectedFields,
              remember: true,
            }),
          ]);
        }
        // Forward the proveCertificate request to Rust with only selected fields
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: true, selectedFields }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error approving cert disclosure:', error);
    }
  };

  // ── Certificate Disclosure: Deny ──
  const handleCertDeny = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: false }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error denying cert disclosure:', error);
    }
  };

  // Toggle a field in the selected set
  const toggleField = (field: string) => {
    setSelectedFields(prev =>
      prev.includes(field)
        ? prev.filter(f => f !== field)
        : [...prev, field]
    );
  };

  // Format field name for display: underscores → spaces, capitalize first letter
  const formatFieldName = (field: string): string => {
    return field
      .replace(/_/g, ' ')
      .replace(/\b\w/g, c => c.toUpperCase());
  };

  // Truncate certifier pubkey for display
  const truncatePubkey = (key: string): string => {
    if (key.length <= 16) return key;
    return key.slice(0, 8) + '...' + key.slice(-8);
  };

  // ── No Wallet: Set Up ──
  const handleNoWalletSetup = () => {
    window.cefMessage?.send('toggle_wallet_panel', ['0']);
    window.cefMessage?.send('overlay_close', []);
  };

  // ── No Wallet: Dismiss ──
  const handleNoWalletDismiss = () => {
    window.cefMessage?.send('overlay_close', []);
  };

  const cleanDomain = formatDomain(notificationDomain);

  // ── Shared card wrapper ──
  const cardStyle: React.CSSProperties = {
    background: COLORS.white,
    borderRadius: '14px',
    boxShadow: `
      0 0 0 1px ${COLORS.borderLight},
      0 4px 6px rgba(0,0,0,0.07),
      0 12px 40px rgba(0,0,0,0.15),
      0 0 80px rgba(212,196,168,0.12)
    `,
    padding: '28px 32px',
    maxWidth: '440px',
    width: '90%',
    fontFamily: FONT_FAMILY,
  };

  // ── No wallet notification ──
  if (notificationType === 'no_wallet') {
    return (
      <div style={overlayBackdrop}>
        <div style={cardStyle}>
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '20px' }}>
            <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            <div>
              <div style={{ fontSize: '16px', fontWeight: 700, color: COLORS.textDark }}>
                {cleanDomain}
              </div>
              <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '2px' }}>
                wants to connect
              </div>
            </div>
          </div>

          {/* Explanation */}
          <p style={{
            margin: '0 0 10px',
            fontSize: '14px',
            color: COLORS.textMuted,
            lineHeight: 1.6,
          }}>
            This site needs a wallet to work. A wallet lets you sign in to sites,
            make payments, and manage your data — all without creating accounts or passwords.
          </p>
          <p style={{
            margin: '0 0 24px',
            fontSize: '14px',
            color: COLORS.textMuted,
            lineHeight: 1.6,
          }}>
            You don't have a wallet yet. Would you like to set one up?
          </p>

          {/* Buttons */}
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px' }}>
            <HodosButton variant="secondary" onClick={handleNoWalletDismiss}>
              Not now
            </HodosButton>
            <HodosButton variant="primary" onClick={handleNoWalletSetup}>
              Set up wallet
            </HodosButton>
          </div>
        </div>
      </div>
    );
  }

  // ── Payment confirmation notification ──
  if (notificationType === 'payment_confirmation') {
    return (
      <div style={overlayBackdrop}>
        <div style={cardStyle}>
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '22px' }}>
            <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            <div>
              <div style={{ fontSize: '16px', fontWeight: 700, color: COLORS.textDark }}>
                {cleanDomain}
              </div>
              <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '2px' }}>
                is requesting a payment
              </div>
            </div>
          </div>

          {/* Amount display */}
          <div style={{
            background: COLORS.subduedGold,
            borderRadius: '10px',
            padding: '18px 20px',
            marginBottom: '18px',
            textAlign: 'center',
          }}>
            <div style={{
              fontSize: '28px',
              fontWeight: 700,
              color: COLORS.textDark,
              marginBottom: '4px',
            }}>
              {formatUsdCents(paymentCents)}
            </div>
            <div style={{
              fontSize: '14px',
              color: COLORS.textMuted,
            }}>
              {formatSatoshis(paymentSatoshis)}
            </div>
          </div>

          {/* Limit explanation */}
          <div style={{
            fontSize: '13px',
            color: COLORS.textMuted,
            lineHeight: 1.5,
            marginBottom: showModifyLimits ? '14px' : '22px',
          }}>
            {getLimitExplanation()}
          </div>

          {/* Modify limits form (collapsible) */}
          {showModifyLimits ? (
            <div style={{
              border: `1px solid ${COLORS.borderLight}`,
              borderRadius: '10px',
              padding: '16px',
              marginBottom: '22px',
            }}>
              <DomainPermissionForm
                domain={notificationDomain}
                currentSettings={{
                  perTxLimitCents: perTxLimit,
                  perSessionLimitCents: perSessionLimit,
                  rateLimitPerMin: rateLimit,
                }}
                onSave={(settings) => handleModifyLimitsAndApprove(settings)}
                onCancel={() => setShowModifyLimits(false)}
              />
            </div>
          ) : (
            <>
              {/* Buttons */}
              <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px' }}>
                <HodosButton variant="secondary" onClick={handlePaymentDeny}>
                  Deny
                </HodosButton>
                <HodosButton variant="secondary" onClick={() => setShowModifyLimits(true)}>
                  Modify Limits
                </HodosButton>
                <HodosButton variant="primary" onClick={handlePaymentApprove}>
                  Approve
                </HodosButton>
              </div>
            </>
          )}
        </div>
      </div>
    );
  }

  // ── Rate limit exceeded notification ──
  if (notificationType === 'rate_limit_exceeded') {
    return (
      <div style={overlayBackdrop}>
        <div style={cardStyle}>
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '22px' }}>
            <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            <div>
              <div style={{ fontSize: '16px', fontWeight: 700, color: COLORS.textDark }}>
                {cleanDomain}
              </div>
              <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '2px' }}>
                is making frequent requests
              </div>
            </div>
          </div>

          {/* Amount display (if there is a payment) */}
          {paymentSatoshis > 0 && (
            <div style={{
              background: COLORS.subduedGold,
              borderRadius: '10px',
              padding: '18px 20px',
              marginBottom: '18px',
              textAlign: 'center',
            }}>
              <div style={{
                fontSize: '28px',
                fontWeight: 700,
                color: COLORS.textDark,
                marginBottom: '4px',
              }}>
                {formatUsdCents(paymentCents)}
              </div>
              <div style={{
                fontSize: '14px',
                color: COLORS.textMuted,
              }}>
                {formatSatoshis(paymentSatoshis)}
              </div>
            </div>
          )}

          {/* Explanation */}
          <div style={{
            fontSize: '13px',
            color: COLORS.textMuted,
            lineHeight: 1.5,
            marginBottom: showModifyLimits ? '14px' : '22px',
          }}>
            This site is sending payment requests faster than your rate limit
            of {rateLimit} per minute. You can approve this request, deny it,
            or adjust your limits for this site.
          </div>

          {/* Modify limits form (collapsible) */}
          {showModifyLimits ? (
            <div style={{
              border: `1px solid ${COLORS.borderLight}`,
              borderRadius: '10px',
              padding: '16px',
              marginBottom: '22px',
            }}>
              <DomainPermissionForm
                domain={notificationDomain}
                currentSettings={{
                  perTxLimitCents: perTxLimit,
                  perSessionLimitCents: perSessionLimit,
                  rateLimitPerMin: rateLimit,
                }}
                onSave={(settings) => handleModifyLimitsAndApprove(settings)}
                onCancel={() => setShowModifyLimits(false)}
              />
            </div>
          ) : (
            <>
              {/* Buttons */}
              <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px' }}>
                <HodosButton variant="secondary" onClick={handlePaymentDeny}>
                  Deny
                </HodosButton>
                <HodosButton variant="secondary" onClick={() => setShowModifyLimits(true)}>
                  Modify Limits
                </HodosButton>
                <HodosButton variant="primary" onClick={handlePaymentApprove}>
                  Approve
                </HodosButton>
              </div>
            </>
          )}
        </div>
      </div>
    );
  }

  // ── Certificate disclosure notification ──
  if (notificationType === 'certificate_disclosure') {
    return (
      <div style={overlayBackdrop}>
        <div style={cardStyle}>
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '22px' }}>
            <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            <div>
              <div style={{ fontSize: '16px', fontWeight: 700, color: COLORS.textDark }}>
                {cleanDomain}
              </div>
              <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '2px' }}>
                is requesting identity verification
              </div>
            </div>
          </div>

          {/* Fields being requested — individually selectable */}
          <div style={{
            background: COLORS.subduedGold,
            borderRadius: '10px',
            padding: '14px 16px',
            marginBottom: '18px',
          }}>
            <div style={{ fontSize: '13px', fontWeight: 600, color: COLORS.textDark, marginBottom: '10px' }}>
              Select which fields to share:
            </div>
            {certFields.map((field, idx) => (
              <label
                key={idx}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '10px',
                  fontSize: '13px',
                  color: '#f0f0f0',
                  lineHeight: 1.5,
                  marginBottom: '8px',
                  cursor: 'pointer',
                  userSelect: 'none',
                }}
              >
                <input
                  type="checkbox"
                  checked={selectedFields.includes(field)}
                  onChange={() => toggleField(field)}
                  style={{
                    accentColor: COLORS.primary,
                    width: '16px',
                    height: '16px',
                    cursor: 'pointer',
                    flexShrink: 0,
                  }}
                />
                <span>{formatFieldName(field)}</span>
              </label>
            ))}
          </div>

          {/* Certifier info */}
          {certifier && (
            <div style={{
              fontSize: '12px',
              color: COLORS.textMuted,
              lineHeight: 1.5,
              marginBottom: '14px',
            }}>
              Verified by:{' '}
              <span style={{
                fontFamily: 'monospace',
                fontSize: '11px',
                background: '#0f1117',
                padding: '2px 6px',
                borderRadius: '4px',
              }}>
                {truncatePubkey(certifier)}
              </span>
            </div>
          )}

          {/* Remember checkbox */}
          <label style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            fontSize: '13px',
            color: COLORS.textMuted,
            cursor: 'pointer',
            marginBottom: '22px',
            userSelect: 'none',
          }}>
            <input
              type="checkbox"
              checked={rememberFields}
              onChange={(e) => setRememberFields(e.target.checked)}
              style={{ accentColor: COLORS.primary, width: '16px', height: '16px', cursor: 'pointer' }}
            />
            Remember for this site
          </label>

          {/* Buttons */}
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px' }}>
            <HodosButton variant="secondary" onClick={handleCertDeny}>
              Deny
            </HodosButton>
            <HodosButton
              variant="primary"
              onClick={handleCertApprove}
              disabled={selectedFields.length === 0}
            >
              {selectedFields.length === certFields.length
                ? 'Share All'
                : selectedFields.length > 0
                  ? `Share ${selectedFields.length} of ${certFields.length}`
                  : 'Share'}
            </HodosButton>
          </div>
        </div>
      </div>
    );
  }

  // ── Domain approval notification ──
  if (notificationType === 'domain_approval') {
    return (
      <div style={overlayBackdrop}>
        <div style={cardStyle}>
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '22px' }}>
            <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            <div>
              <div style={{ fontSize: '16px', fontWeight: 700, color: COLORS.textDark }}>
                {cleanDomain}
              </div>
              <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '2px' }}>
                wants to connect to your wallet
              </div>
            </div>
          </div>

          {/* What this means */}
          <div style={{
            fontSize: '14px',
            color: COLORS.textDark,
            lineHeight: 1.6,
            marginBottom: '16px',
          }}>
            If you allow this, the site can:
          </div>

          <div style={{
            background: COLORS.subduedGold,
            borderRadius: '10px',
            padding: '14px 16px',
            marginBottom: '18px',
          }}>
            <div style={permissionItem}>
              <span style={checkmark}>&#10003;</span>
              <span>Verify your identity to sign you in</span>
            </div>
            <div style={permissionItem}>
              <span style={checkmark}>&#10003;</span>
              <span>Request payments — small ones auto-approved, large ones ask you</span>
            </div>
            <div style={permissionItem}>
              <span style={checkmark}>&#10003;</span>
              <span>Store and access data you share with it</span>
            </div>
          </div>

          {/* Reassurance */}
          <div style={{
            fontSize: '12px',
            color: COLORS.textMuted,
            lineHeight: 1.5,
            marginBottom: '16px',
          }}>
            You can disconnect this site at any time from your browser settings.
            Payments above $0.10 will ask for your confirmation.
          </div>

          {/* Advanced settings toggle */}
          <div
            onClick={() => setShowAdvanced(!showAdvanced)}
            style={{
              fontSize: '12px',
              color: COLORS.textMuted,
              cursor: 'pointer',
              marginBottom: showAdvanced ? '14px' : '22px',
              userSelect: 'none',
            }}
          >
            {showAdvanced ? '\u25BC' : '\u25B6'} Advanced settings
          </div>

          {/* Collapsible advanced section */}
          {showAdvanced && (
            <div style={{
              border: `1px solid ${COLORS.borderLight}`,
              borderRadius: '10px',
              padding: '16px',
              marginBottom: '22px',
            }}>
              <DomainPermissionForm
                domain={notificationDomain}
                onSave={(settings) => handleAllowAdvanced(settings)}
                onCancel={() => setShowAdvanced(false)}
              />
            </div>
          )}

          {/* Buttons (hidden when advanced form is showing — it has its own save/cancel) */}
          {!showAdvanced && (
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px' }}>
              <HodosButton variant="secondary" onClick={handleBlock}>
                Block
              </HodosButton>
              <HodosButton variant="primary" onClick={handleAllow}>
                Allow
              </HodosButton>
            </div>
          )}
        </div>
      </div>
    );
  }

  // ── Idle / no notification: render nothing (fully transparent) ──
  return null;
};

// ── Shared styles ──

const overlayBackdrop: React.CSSProperties = {
  width: '100vw',
  height: '100vh',
  backgroundColor: 'rgba(0, 0, 0, 0.45)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
};

const avatarStyle: React.CSSProperties = {
  width: '44px',
  height: '44px',
  borderRadius: '50%',
  backgroundColor: '#111827',
  color: '#a67c00',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  fontSize: '18px',
  fontWeight: 700,
  fontFamily: "'Inter', sans-serif",
  flexShrink: 0,
};

const permissionItem: React.CSSProperties = {
  display: 'flex',
  alignItems: 'flex-start',
  gap: '10px',
  fontSize: '13px',
  color: '#f0f0f0',
  lineHeight: 1.5,
  marginBottom: '8px',
};

const checkmark: React.CSSProperties = {
  color: '#2e7d32',
  fontWeight: 700,
  fontSize: '14px',
  marginTop: '1px',
  flexShrink: 0,
};

export default BRC100AuthOverlayRoot;
