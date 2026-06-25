import React, { useState, useEffect, useRef } from 'react';
import DomainPermissionForm from '../components/DomainPermissionForm';
import { walletFetch } from '../services/walletApi';
import type { DomainPermissionSettings } from '../components/DomainPermissionForm';
import { HodosButton } from '../components/HodosButton';
import { prompt as promptTheme } from '../styles/hodosTheme';

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

// Sub-component for editing existing domain permissions (fetches current settings)
const EditPermissionsForm: React.FC<{ domain: string; onClose: () => void }> = ({ domain, onClose }) => {
  const [currentSettings, setCurrentSettings] = useState<DomainPermissionSettings | undefined>();
  const [loading, setLoading] = useState(true);
  const [saved, setSaved] = useState(false);
  const [revoked, setRevoked] = useState(false);

  useEffect(() => {
    const fetchSettings = async () => {
      try {
        const res = await walletFetch(`/domain/permissions?domain=${encodeURIComponent(domain)}`);
        if (res.ok) {
          const data = await res.json();
          // Rust GET returns camelCase (trustLevel, perTxLimitCents, etc.)
          if (data && data.trustLevel === 'approved') {
            setCurrentSettings({
              perTxLimitCents: data.perTxLimitCents ?? 100,
              perSessionLimitCents: data.perSessionLimitCents ?? 1000,
              rateLimitPerMin: data.rateLimitPerMin ?? 30,
              maxTxPerSession: data.maxTxPerSession ?? 100,
              // Phase 1.5 Step 5 — pass through current V17 column value so
              // the form shows the actual setting, not the default.
              identityKeyDisclosureAllowed: data.identityKeyDisclosureAllowed ?? true,
              // Phase 2.6-D Fix #4 — pass through V22 column value.
              bundledScopeGrant: data.bundledScopeGrant ?? false,
            });
          }
        }
      } catch { /* no existing permission */ }
      setLoading(false);
    };
    fetchSettings();
  }, [domain]);

  const handleSave = async (settings: DomainPermissionSettings) => {
    try {
      // POST directly to Rust wallet API (serde rename_all = camelCase)
      await walletFetch('/domain/permissions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          domain,
          trustLevel: 'approved',
          perTxLimitCents: settings.perTxLimitCents,
          perSessionLimitCents: settings.perSessionLimitCents,
          rateLimitPerMin: settings.rateLimitPerMin,
          maxTxPerSession: settings.maxTxPerSession,
          // Phase 1.5 Step 5 — Personal Info Disclosure toggle persistence.
          identityKeyDisclosureAllowed: settings.identityKeyDisclosureAllowed,
          // Phase 2.6-D Fix #4 — Quiet-mode toggle persistence (V22 column).
          bundledScopeGrant: settings.bundledScopeGrant,
        }),
      });
    } catch (err) {
      console.error('Failed to save permissions:', err);
    }
    // Invalidate C++ DomainPermissionCache so changes take effect immediately
    window.cefMessage?.send('domain_permission_invalidate', [domain]);
    setSaved(true);
    setTimeout(onClose, 800);
  };

  const handleRevoke = async () => {
    try {
      await walletFetch(`/domain/permissions?domain=${encodeURIComponent(domain)}`, {
        method: 'DELETE',
      });
    } catch { /* ignore */ }
    // Invalidate C++ DomainPermissionCache so revocation takes effect immediately
    window.cefMessage?.send('domain_permission_invalidate', [domain]);
    setRevoked(true);
    // No auto-dismiss — destructive action gets explicit acknowledgement.
  };

  const cleanDomainShort = domain.replace(/^https?:\/\//, '').replace(/^www\./, '');

  if (saved) {
    return <div style={{ textAlign: 'center', padding: '16px 0', color: '#4ade80', fontSize: '14px' }}>Permissions saved</div>;
  }
  if (revoked) {
    return (
      <div style={{ padding: '4px 0' }}>
        <div style={{ color: '#4ade80', fontSize: '15px', fontWeight: 600, marginBottom: '10px', textAlign: 'center' }}>
          Permissions revoked
        </div>
        <div style={{ color: '#e0e0e0', fontSize: '13px', lineHeight: 1.55, marginBottom: '18px', textAlign: 'center' }}>
          <strong>{cleanDomainShort}</strong> has been removed from your approved sites.
          You'll be asked to approve again the next time it requests a wallet action.
        </div>
        <div style={{ display: 'flex', justifyContent: 'center' }}>
          <HodosButton variant="primary" size="small" onClick={onClose}>
            OK
          </HodosButton>
        </div>
      </div>
    );
  }
  if (loading) {
    return <div style={{ textAlign: 'center', padding: '16px 0', color: '#9ca3af', fontSize: '13px' }}>Loading...</div>;
  }

  return (
    <>
      <DomainPermissionForm
        domain={domain}
        currentSettings={currentSettings}
        onSave={handleSave}
        onCancel={onClose}
      />
      {currentSettings && (
        <div style={{ marginTop: '12px', borderTop: '1px solid #2a2d35', paddingTop: '12px' }}>
          <HodosButton variant="secondary" size="small" onClick={handleRevoke} style={{ color: '#ef4444', borderColor: '#ef4444' }}>
            Revoke All Permissions
          </HodosButton>
        </div>
      )}
    </>
  );
};

// Phase 1.5 Step 5 — small info icon for tooltips on identity-key surfaces.
// Uses onMouseEnter/onMouseLeave + a positioned div rather than the native
// `title` attribute, because CEF doesn't reliably render Chromium's native
// tooltip UI inside overlays. Mirrors the working pattern in
// frontend/src/components/wallet/DashboardTab.tsx's `InfoTooltip`.
// Default copy is the "identify you across the Metanet" framing surfaced
// during Step 5 design — overridable per-callsite if different copy fits.
const InfoIcon: React.FC<{ tooltip?: string; style?: React.CSSProperties }> = ({
  tooltip,
  style,
}) => {
  const [open, setOpen] = React.useState(false);
  const text = tooltip || 'Identify you across the Metanet with your wallet identity key. This key is the same across every BRC-100 site you visit, so granting it lets sites recognize you between visits.';
  return (
    <span
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      style={{
        position: 'relative',
        display: 'inline-flex',
        alignItems: 'center',
        marginLeft: '4px',
        verticalAlign: 'middle',
      }}
    >
      <span
        style={{
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
          fontStyle: 'italic',
          ...style,
        }}
      >
        i
      </span>
      {open && (
        <span
          style={{
            position: 'absolute',
            bottom: 'calc(100% + 6px)',
            left: '50%',
            transform: 'translateX(-50%)',
            zIndex: 9999,
            background: '#0f1117',
            color: COLORS.textDark,
            border: `1px solid ${COLORS.gold}`,
            borderRadius: '6px',
            padding: '8px 10px',
            fontSize: '11px',
            fontWeight: 400,
            lineHeight: 1.45,
            width: '240px',
            textAlign: 'left',
            boxShadow: '0 4px 12px rgba(0, 0, 0, 0.4)',
            fontStyle: 'normal',
            pointerEvents: 'none',
          }}
        >
          {text}
        </span>
      )}
    </span>
  );
};

// Phase 1.5 Step 0 — Hodos wallet attribution header. Renders the
// Hodos_Gold_Wallet_Icon.svg at the top of every auth/payment/cert/
// permission prompt so the user can immediately tell the wallet (not the
// site) is the actor asking. Phase principle #1: Trust on first contact.
// The SVG already contains the "Hodos Wallet" wordmark, so no separate
// text label is rendered. Sits ABOVE the existing favicon + domain row so
// the hierarchy is "Hodos Wallet → talking about → [site]".
const HodosWalletHeader: React.FC = () => (
  <div
    style={{
      display: 'flex',
      alignItems: 'center',
      paddingBottom: '14px',
      marginBottom: '16px',
      borderBottom: `1px solid ${COLORS.borderLight}`,
    }}
  >
    <img
      src="/Hodos_Gold_Wallet_Icon.svg"
      alt="Hodos Wallet"
      height={36}
      style={{ display: 'block', flexShrink: 0, width: 'auto' }}
    />
  </div>
);

const BRC100AuthOverlayRoot: React.FC = () => {
  const [notificationType, setNotificationType] = useState<string>('');
  const [notificationDomain, setNotificationDomain] = useState<string>('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showModifyLimits, setShowModifyLimits] = useState(false);
  const [faviconError, setFaviconError] = useState(false);

  // Payment/rate-limit params
  const [paymentSatoshis, setPaymentSatoshis] = useState<number>(0);
  const [paymentCents, setPaymentCents] = useState<number>(0);
  const [exceededLimit, setExceededLimit] = useState<string>('');
  const [perTxLimit, setPerTxLimit] = useState<number>(10);
  const [perSessionLimit, setPerSessionLimit] = useState<number>(300);
  const [sessionSpent, setSessionSpent] = useState<number>(0);
  const [rateLimit, setRateLimit] = useState<number>(10);
  const [maxTxPerSession, setMaxTxPerSession] = useState<number>(100);

  // Certificate disclosure params
  const [certFields, setCertFields] = useState<string[]>([]);
  const [selectedFields, setSelectedFields] = useState<string[]>([]);
  const [certType, setCertType] = useState<string>('');
  const [certifier, setCertifier] = useState<string>('');
  const [rememberFields, setRememberFields] = useState<boolean>(true);

  // Phase 1.5 Step 1 — privacy-perimeter prompt params (identity_key_reveal,
  // key_linkage_reveal). Step 2 wires persistence into domain_permissions;
  // for Step 1 the "Always allow" checkbox state ships into IPC and lands in
  // the in-memory C++ cache.
  const [linkageKind, setLinkageKind] = useState<string>(''); // 'counterparty' | 'specific'
  const [linkageVerifier, setLinkageVerifier] = useState<string>('');
  const [linkageProtocol, setLinkageProtocol] = useState<string>('');
  const [linkageKeyId, setLinkageKeyId] = useState<string>('');
  const [rememberPrivacy, setRememberPrivacy] = useState<boolean>(false);

  // Phase 1.5 Step 6 Commit E — scoped permission prompts (protocol_permission_prompt,
  // basket_permission_prompt, counterparty_permission_prompt). Three fields per
  // scope kind, populated by applyParams from the C++ extraParams query string.
  const [scopedProtocolLevel, setScopedProtocolLevel] = useState<number>(2);
  const [scopedProtocolName, setScopedProtocolName] = useState<string>('');
  const [scopedProtocolKeyId, setScopedProtocolKeyId] = useState<string>('*');
  const [scopedProtocolCounterparty, setScopedProtocolCounterparty] = useState<string>('');
  const [scopedBasket, setScopedBasket] = useState<string>('');
  const [scopedBasketAccess, setScopedBasketAccess] = useState<string>('read');
  const [scopedCounterparty, setScopedCounterparty] = useState<string>('');

  // b1b — site-permission prompt (camera/mic/location/notifications/clipboard).
  // `permCode` selects the icon + wording; `permRequestId` keys the C++ callback.
  const [permCode, setPermCode] = useState<string>('');
  const [permRequestId, setPermRequestId] = useState<string>('');
  const [permSubmitted, setPermSubmitted] = useState<boolean>(false);  // one decision per prompt

  // Phase 1.5 Step 1 — "Allow this site to identify you" checkbox in the
  // domain_approval modal. Defaults ON so the common case (user trusts the
  // site enough to approve it at all) avoids a second sequential popup for
  // the identity-key reveal. Power users can untick to keep the prompt.
  const [allowIdentityKey, setAllowIdentityKey] = useState<boolean>(true);

  // Phase 2.6-D Fix #4 — "Allow this site to perform wallet operations
  // without prompting each time" checkbox. Defaults ON for UX (per CLAUDE.md:
  // UX wins ties when no privacy/security cost is at stake). When ticked,
  // the engine silences ProtocolUse + BasketAccess prompts for this domain.
  // CounterpartyUse is already silent for approved domains via Fix #3.
  // Protected baskets (default/backup-*/admin *) still prompt regardless —
  // those forced-prompt paths run before this flag is consulted.
  const [allowBundledScope, setAllowBundledScope] = useState<boolean>(true);

  // Phase 1.5 Step 5 — user's saved default for the bundle checkbox (V19
  // settings column). applyParams reads this ref so each fresh notification
  // initializes to the user's preference rather than hardcoded true. Ref
  // (not state) because applyParams is invoked from a JS-injection callback
  // whose closure would otherwise capture stale state.
  const savedDefaultIdentityKeyRef = useRef<boolean>(true);

  // Phase 1.5 Step 5 — manifest_connect_bundle state. Parsed once from the
  // C++-supplied `manifest` query param; sub-permissions start all-selected
  // (matching the "Connect grants everything in the manifest" default per
  // PERMISSION_UX_DESIGN.md §5). Customize subview lets the user untick
  // individual permissions before connecting.
  interface ManifestProtocol { securityLevel: number; name: string; keyId: string; purpose: string; }
  interface ManifestBasket { name: string; access: string; purpose: string; }
  interface ManifestCertificate { type: string; fields: string[]; purpose: string; }
  interface ManifestSpending { perTransactionUsd: number; perSessionUsd: number; purpose: string; }
  interface ManifestCounterparty { type: string; counterparty: string; purpose: string; }
  interface ManifestData {
    name: string;
    description: string;
    iconUrl: string;
    expiresAt: number;
    version: string;
    protocols: ManifestProtocol[];
    baskets: ManifestBasket[];
    certificates: ManifestCertificate[];
    spending: ManifestSpending;
    counterparties: ManifestCounterparty[];
  }
  const [manifestData, setManifestData] = useState<ManifestData | null>(null);
  const [manifestShowCustomize, setManifestShowCustomize] = useState<boolean>(false);
  const [manifestSelectedProtocols, setManifestSelectedProtocols] = useState<Set<number>>(new Set());
  const [manifestSelectedBaskets, setManifestSelectedBaskets] = useState<Set<number>>(new Set());
  const [manifestSelectedCertificates, setManifestSelectedCertificates] = useState<Set<number>>(new Set());
  const [manifestSelectedCounterparties, setManifestSelectedCounterparties] = useState<Set<number>>(new Set());
  const [manifestAllowIdentityKey, setManifestAllowIdentityKey] = useState<boolean>(true);
  // Phase 2.6-D Fix #4 — bundled scope grant for the manifest connect path.
  // Same semantics as the domain_approval modal's allowBundledScope: when
  // ticked, the engine silences ProtocolUse + BasketAccess prompts for this
  // domain (CounterpartyUse silent by Fix #3, protected baskets always
  // prompt). Default ON.
  const [manifestAllowBundledScope, setManifestAllowBundledScope] = useState<boolean>(true);
  // Customize subview payment caps (start from manifest's recommendation, fall back to wallet defaults).
  const [manifestPerTxCents, setManifestPerTxCents] = useState<number>(100);
  const [manifestPerSessionCents, setManifestPerSessionCents] = useState<number>(1000);
  const [manifestRateLimit, setManifestRateLimit] = useState<number>(30);
  const [manifestMaxTxPerSession, setManifestMaxTxPerSession] = useState<number>(100);

  // Apply notification params from a query string (used by both initial load and JS injection)
  const applyParams = (queryString: string) => {
    const params = new URLSearchParams(queryString);
    const type = params.get('type') || '';
    const domain = params.get('domain') || '';

    // b1b — site-permission prompt params.
    setPermCode(params.get('perm') || '');
    setPermRequestId(params.get('requestId') || '');
    setPermSubmitted(false);  // fresh prompt → re-enable buttons

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
    setMaxTxPerSession(100);

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

    const maxTxParam = params.get('maxTxPerSession');
    if (maxTxParam) setMaxTxPerSession(parseInt(maxTxParam));

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

    // Phase 1.5 Step 6 Commit E — scoped permission params. Each prompt type
    // uses a different subset of fields; reading all of them up-front is fine
    // because unused state stays at the default empty values.
    const protoLevelParam = params.get('protocolLevel');
    setScopedProtocolLevel(protoLevelParam ? parseInt(protoLevelParam) : 2);
    setScopedProtocolName(params.get('protocolName') || '');
    setScopedProtocolKeyId(params.get('protocolKeyId') || '*');
    setScopedProtocolCounterparty(params.get('protocolCounterparty') || '');
    setScopedBasket(params.get('basket') || '');
    setScopedBasketAccess(params.get('basketAccess') || 'read');
    setScopedCounterparty(params.get('counterparty') || '');

    // Phase 1.5 Step 1 — privacy-perimeter params
    setLinkageKind(params.get('kind') || '');
    setLinkageVerifier(params.get('verifier') || '');
    setLinkageProtocol(params.get('protocol') || '');
    setLinkageKeyId(params.get('keyID') || '');
    setRememberPrivacy(false);

    // Domain-approval bundle: initialize from the user's saved default (V19),
    // not hardcoded true. Ref keeps applyParams non-stale across re-renders.
    setAllowIdentityKey(savedDefaultIdentityKeyRef.current);

    // Phase 1.5 Step 5 — manifest_connect_bundle params.
    // Reset every time so a previous site's manifest doesn't leak in.
    setManifestData(null);
    setManifestShowCustomize(false);
    setManifestAllowIdentityKey(savedDefaultIdentityKeyRef.current);
    const manifestParam = params.get('manifest');
    if (manifestParam) {
      try {
        const m: ManifestData = JSON.parse(manifestParam);
        setManifestData(m);
        // All permissions ticked by default — matches "Connect grants everything"
        setManifestSelectedProtocols(new Set(m.protocols.map((_, i) => i)));
        setManifestSelectedBaskets(new Set(m.baskets.map((_, i) => i)));
        setManifestSelectedCertificates(new Set(m.certificates.map((_, i) => i)));
        setManifestSelectedCounterparties(new Set(m.counterparties.map((_, i) => i)));
        // Use manifest's recommended caps if present, otherwise wallet defaults.
        if (m.spending && m.spending.perTransactionUsd > 0) {
          setManifestPerTxCents(m.spending.perTransactionUsd * 100);
        } else {
          setManifestPerTxCents(100); // $1
        }
        if (m.spending && m.spending.perSessionUsd > 0) {
          setManifestPerSessionCents(m.spending.perSessionUsd * 100);
        } else {
          setManifestPerSessionCents(1000); // $10
        }
        setManifestRateLimit(30);
        setManifestMaxTxPerSession(100);
      } catch (e) {
        console.error('[Hodos] Failed to parse manifest from extraParams:', e);
        setManifestData(null);
      }
    }
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

    // Phase 1.5 Step 5 — fetch the user's default for the identity-key bundle
    // checkbox. If they set it to OFF in Approved Sites, fresh-site prompts
    // should start the checkbox unticked. Default to true on any fetch failure
    // so we don't accidentally degrade UX on a wallet that doesn't have V19 yet.
    walletFetch('/wallet/settings')
      .then((res) => res.ok ? res.json() : null)
      .then((data) => {
        if (data && typeof data.default_identity_key_disclosure_allowed === 'boolean') {
          const def = data.default_identity_key_disclosure_allowed;
          savedDefaultIdentityKeyRef.current = def;
          setAllowIdentityKey(def);
          setManifestAllowIdentityKey(def);
        }
      })
      .catch(() => { /* silent — keep defaults */ });

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

  // Reset favicon error state whenever the domain changes
  useEffect(() => {
    setFaviconError(false);
  }, [notificationDomain]);

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
    } else if (exceededLimit === 'price_unavailable') {
      // Engine fell back to Prompt because the BSV/USD price feed is down,
      // so spending caps can't be evaluated automatically. The cents
      // display above will show $0.00 (no price → no conversion); the
      // satoshi amount below it is the real number to verify.
      return `The BSV/USD price feed is currently unavailable, so we can't evaluate spending caps automatically. Verify the satoshi amount above before approving, or deny to retry once the price feed is back.`;
    }
    return 'This payment exceeds your auto-approve limits for this site.';
  };

  // ── Domain Approval: Allow ──
  const handleAllow = () => {
    try {
      if (window.cefMessage) {
        // Set domain permission to "approved" (sets cache + DB write).
        // Phase 1.5 Step 1: bundle identityKeyDisclosureAllowed via the
        // "Allow this site to identify you" checkbox state.
        // Phase 2.6-D Fix #4: bundle bundledScopeGrant via the "Allow this
        // site to perform wallet operations" checkbox state.
        window.cefMessage.send('add_domain_permission', [
          JSON.stringify({
            domain: notificationDomain,
            identityKeyDisclosureAllowed: allowIdentityKey,
            bundledScopeGrant: allowBundledScope,
          }),
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
            maxTxPerSession: settings.maxTxPerSession,
            identityKeyDisclosureAllowed: allowIdentityKey,
            // Phase 2.6-D Fix #4 — bundle the V22 column write through the
            // advanced path too.
            bundledScopeGrant: allowBundledScope,
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
  //
  // Phase 2.6-E — collapsed the racy two-IPC dance into a single
  // brc100_auth_response that bundles the new limits. The pre-2.6-E flow
  // fired `add_domain_permission_advanced` (whose C++ handler drained
  // PendingRequestManager via popAllForDomain) followed by
  // `brc100_auth_response`, and the drain popped the very request the
  // second IPC was about to look up by id → silent no-replay → user
  // had to manually retry. Now C++ updates the perm row + caches in
  // response to the `modifyLimits` payload BEFORE calling handleAuthResponse
  // for the same request id, so the X-User-Approved replay always lands.
  const handleModifyLimitsAndApprove = (settings: DomainPermissionSettings) => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({
            approved: true,
            modifyLimits: {
              perTxLimitCents: settings.perTxLimitCents,
              perSessionLimitCents: settings.perSessionLimitCents,
              rateLimitPerMin: settings.rateLimitPerMin,
              maxTxPerSession: settings.maxTxPerSession,
            },
          }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error modifying limits:', error);
    }
  };

  // ── Phase 1.5 Step 6 Commit E — scoped permission handlers ──
  // Three buttons across all three scoped-prompt types:
  //   handleScopedAllowOnce      → just approve; no DB write. The approve →
  //                                re-issue flow in simple_handler delivers
  //                                this one call's response; future calls
  //                                re-prompt.
  //   handleScopedAlwaysAllow    → fire grant_scoped_permission IPC first
  //                                (writes V18 row + invalidates
  //                                SubPermissionCache), then approve. Future
  //                                same-scope calls find the persistent grant
  //                                and pass silently.
  //   handleScopedDeny           → reject; existing CefURLRequest reply path
  //                                returns the timeout error to the page.
  const scopedKindFromNotificationType = (): 'protocol' | 'basket' | 'counterparty' | null => {
    if (notificationType === 'protocol_permission_prompt') return 'protocol';
    if (notificationType === 'basket_permission_prompt') return 'basket';
    if (notificationType === 'counterparty_permission_prompt') return 'counterparty';
    return null;
  };

  const buildScopedGrantPayload = () => {
    const kind = scopedKindFromNotificationType();
    if (!kind) return null;
    const base = { domain: notificationDomain, kind } as Record<string, unknown>;
    if (kind === 'protocol') {
      base.protocolLevel = scopedProtocolLevel;
      base.protocolName = scopedProtocolName;
      base.protocolKeyId = scopedProtocolKeyId;
      if (scopedProtocolCounterparty) {
        base.protocolCounterparty = scopedProtocolCounterparty;
      }
    } else if (kind === 'basket') {
      base.basket = scopedBasket;
      base.basketAccess = scopedBasketAccess;
    } else if (kind === 'counterparty') {
      base.counterparty = scopedCounterparty;
    }
    return base;
  };

  const handleScopedAllowOnce = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: true }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error allowing scoped permission once:', error);
    }
  };

  const handleScopedAlwaysAllow = () => {
    try {
      const payload = buildScopedGrantPayload();
      if (window.cefMessage && payload) {
        // Write V18 row first so the cache invalidation lands before any
        // future same-scope call hits the engine.
        window.cefMessage.send('grant_scoped_permission', [JSON.stringify(payload)]);
        // Then approve this request so the page gets its response.
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: true }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error always-allowing scoped permission:', error);
    }
  };

  const handleScopedDeny = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: false }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error denying scoped permission:', error);
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

  // ── Phase 1.5 Step 1 privacy-perimeter handlers ──
  // Mirror handleCert{Approve,Deny}: fire the "remember" IPC if the user
  // checked the box, then fire brc100_auth_response to unblock the
  // AsyncWalletResourceHandler on the C++ side. Approve replays the original
  // request through Rust; deny returns a typed error to the page.

  const handleIdentityKeyApprove = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('approve_identity_key_reveal', [
          JSON.stringify({
            domain: notificationDomain,
            remember: rememberPrivacy,
          }),
        ]);
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: true }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error approving identity-key reveal:', error);
    }
  };

  const handleIdentityKeyDeny = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: false }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error denying identity-key reveal:', error);
    }
  };

  const handleKeyLinkageApprove = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('approve_key_linkage_reveal', [
          JSON.stringify({
            domain: notificationDomain,
            remember: rememberPrivacy,
          }),
        ]);
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: true }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error approving key-linkage reveal:', error);
    }
  };

  const handleKeyLinkageDeny = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [
          JSON.stringify({ approved: false }),
        ]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (error) {
      console.error('Error denying key-linkage reveal:', error);
    }
  };

  // ── Phase 1.5 Step 5 — manifest_connect_bundle handlers ──

  // Toggle a permission in/out of a Set<number> (immutably).
  const toggleManifestPerm = (
    set: Set<number>,
    setter: React.Dispatch<React.SetStateAction<Set<number>>>,
    idx: number,
  ) => {
    const next = new Set(set);
    if (next.has(idx)) next.delete(idx);
    else next.add(idx);
    setter(next);
  };

  // Guardrail: never auto-grant sensitive baskets even if a dApp lists them
  // in its manifest. The user can still grant these explicitly through the
  // form later, but a Connect-button click won't silently hand them over.
  // Aligns with @bsv/wallet-toolbox's "admin "-prefix convention.
  const isProtectedBasket = (name: string): boolean => {
    if (!name) return false;
    if (name === 'default') return true;                // change outputs
    if (name.startsWith('backup-')) return true;        // backup tokens etc.
    if (name.startsWith('admin ')) return true;         // toolbox admin baskets
    return false;
  };

  const handleManifestConnect = async (allowWithoutLimits: boolean = false) => {
    if (!manifestData) return;
    const domain = notificationDomain;
    try {
      // 1. Parent domain_permissions row — trust + payment caps + identity-key.
      // "Allow without limits" only raises PAYMENT caps; scoped grants stay
      // exactly as the user ticked them (and protected baskets stay blocked).
      const perTx = allowWithoutLimits ? 100000 : manifestPerTxCents;
      const perSession = allowWithoutLimits ? 1000000 : manifestPerSessionCents;
      const rate = allowWithoutLimits ? 1000 : manifestRateLimit;
      const maxTx = allowWithoutLimits ? 10000 : manifestMaxTxPerSession;

      if (window.cefMessage) {
        window.cefMessage.send('add_domain_permission_advanced', [JSON.stringify({
          domain,
          perTxLimitCents: perTx,
          perSessionLimitCents: perSession,
          rateLimitPerMin: rate,
          maxTxPerSession: maxTx,
          identityKeyDisclosureAllowed: manifestAllowIdentityKey,
          // Phase 2.6-D Fix #4 — bundle the V22 column write. Manifest path
          // also writes per-scope V18 rows below; the bundle flag is an
          // extra silencer above those (engine returns SilentBundledScopeGrant
          // before consulting V18).
          bundledScopeGrant: manifestAllowBundledScope,
        })]);
      }

      // 2. Scoped sub-permissions via Step 3 endpoints. Fail-tolerant —
      // each row is independent, partial-write doesn't corrupt anything.
      const walletBase = '';
      const post = (path: string, body: object) => walletFetch(walletBase + path, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });

      const writes: Promise<unknown>[] = [];

      manifestData.protocols.forEach((p, i) => {
        if (!manifestSelectedProtocols.has(i)) return;
        writes.push(post('/domain/permissions/protocol', {
          domain,
          securityLevel: p.securityLevel,
          protocolName: p.name,
          keyId: p.keyId || '*',
        }));
      });

      manifestData.baskets.forEach((b, i) => {
        if (!manifestSelectedBaskets.has(i)) return;
        if (isProtectedBasket(b.name)) {
          console.warn(`[Hodos] Refused to auto-grant protected basket: ${b.name} (manifest from ${domain})`);
          return;
        }
        writes.push(post('/domain/permissions/basket', {
          domain,
          basket: b.name,
          access: b.access,
        }));
      });

      manifestData.counterparties.forEach((cp, i) => {
        if (!manifestSelectedCounterparties.has(i)) return;
        // Type-only category entries (no specific pubkey) are out-of-scope
        // for Step 5 grants — Step 6 engine will handle them lazily.
        if (!cp.counterparty) return;
        writes.push(post('/domain/permissions/counterparty', {
          domain,
          counterparty: cp.counterparty,
        }));
      });

      // Cert fields go through the existing IPC (which writes via the
      // wallet's /domain/permissions/certificate endpoint).
      manifestData.certificates.forEach((c, i) => {
        if (!manifestSelectedCertificates.has(i)) return;
        if (!c.type || c.fields.length === 0) return;
        if (window.cefMessage) {
          window.cefMessage.send('approve_cert_fields', [JSON.stringify({
            domain,
            certType: c.type,
            fields: c.fields,
            remember: true,
          })]);
        }
      });

      await Promise.allSettled(writes);

      // 3. Unblock the AsyncWalletResourceHandler queue. Same IPC the
      // existing domain_approval flow uses; PendingRequestManager drains
      // all queued requests on approval.
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [JSON.stringify({
          approved: true,
          whitelist: true,
        })]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (e) {
      console.error('[Hodos] Error in manifest connect flow:', e);
    }
  };

  const handleManifestDecline = () => {
    try {
      if (window.cefMessage) {
        window.cefMessage.send('brc100_auth_response', [JSON.stringify({
          approved: false,
          whitelist: false,
        })]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (e) {
      console.error('[Hodos] Error declining manifest:', e);
    }
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
          <HodosWalletHeader />
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '20px' }}>
            {!faviconError ? (
              <img
                src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=32`}
                width={32}
                height={32}
                style={{ borderRadius: 4, flexShrink: 0 }}
                onError={() => setFaviconError(true)}
                alt=""
              />
            ) : (
              <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            )}
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
          <HodosWalletHeader />
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '22px' }}>
            {!faviconError ? (
              <img
                src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=32`}
                width={32}
                height={32}
                style={{ borderRadius: 4, flexShrink: 0 }}
                onError={() => setFaviconError(true)}
                alt=""
              />
            ) : (
              <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            )}
            <div>
              <div style={{ fontSize: '16px', fontWeight: 700, color: COLORS.textDark }}>
                {cleanDomain}
              </div>
              <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '2px' }}>
                is requesting a payment
              </div>
            </div>
          </div>

          {/* Amount display
              When BSV/USD price is unavailable, the engine sends cents=0 —
              the big "$0.00" misleads users into thinking the payment is free.
              In that case, lead with satoshis (the real number) and show
              "Price unavailable" in the USD slot. */}
          <div style={{
            background: COLORS.subduedGold,
            borderRadius: '10px',
            padding: '18px 20px',
            marginBottom: '18px',
            textAlign: 'center',
          }}>
            {exceededLimit === 'price_unavailable' ? (
              <>
                <div style={{
                  fontSize: '28px',
                  fontWeight: 700,
                  color: COLORS.textDark,
                  marginBottom: '4px',
                }}>
                  {formatSatoshis(paymentSatoshis)}
                </div>
                <div style={{
                  fontSize: '14px',
                  color: COLORS.textMuted,
                }}>
                  USD price unavailable
                </div>
              </>
            ) : (
              <>
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
              </>
            )}
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
                  maxTxPerSession: maxTxPerSession,
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

  // ── Site permission prompt (b1b): camera / mic / location / notifications /
  // clipboard. Replaces Chromium's stock prompt with the Hodos-branded one;
  // the choice is resolved + persisted via the permission_response IPC. ──
  if (notificationType === 'permission_request') {
    const PERM: Record<string, { icon: string; label: string }> = {
      camera:        { icon: '📷', label: 'use your camera' },
      microphone:    { icon: '🎤', label: 'use your microphone' },
      camera_mic:    { icon: '🎥', label: 'use your camera and microphone' },
      location:      { icon: '📍', label: 'know your location' },
      notifications: { icon: '🔔', label: 'show notifications' },
      clipboard:     { icon: '📋', label: 'read your clipboard' },
    };
    const perm = PERM[permCode] || { icon: '🔐', label: 'access a device feature' };
    const decide = (decision: 'allow_once' | 'allow_always' | 'block') => {
      if (permSubmitted) return;       // exactly one decision per prompt
      setPermSubmitted(true);
      window.cefMessage?.send('permission_response', [JSON.stringify({ requestId: permRequestId, decision })]);
      setNotificationType('');  // optimistic hide; C++ also hides the overlay
    };
    return (
      <div style={overlayBackdrop}>
        <div style={cardStyle}>
          <HodosWalletHeader />
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '20px' }}>
            {!faviconError ? (
              <img
                src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=32`}
                width={32}
                height={32}
                style={{ borderRadius: 4, flexShrink: 0 }}
                onError={() => setFaviconError(true)}
                alt=""
              />
            ) : (
              <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            )}
            <div>
              <div style={{ fontSize: '16px', fontWeight: 700, color: COLORS.textDark }}>
                {cleanDomain}
              </div>
              <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '2px' }}>
                wants to {perm.label}
              </div>
            </div>
          </div>

          <div style={{ fontSize: '40px', textAlign: 'center', marginBottom: '22px' }}>{perm.icon}</div>

          <div style={{ display: 'flex', gap: '12px', marginBottom: '10px' }}>
            <HodosButton variant="secondary" disabled={permSubmitted} onClick={() => decide('allow_once')} style={{ flex: 1 }}>
              Allow this time
            </HodosButton>
            <HodosButton variant="primary" disabled={permSubmitted} onClick={() => decide('allow_always')} style={{ flex: 1 }}>
              Allow every visit
            </HodosButton>
          </div>
          <HodosButton variant="secondary" disabled={permSubmitted} onClick={() => decide('block')} style={{ width: '100%' }}>
            Don't allow
          </HodosButton>
        </div>
      </div>
    );
  }

  // ── Rate limit / session-tx-count / price-unavailable notification ──
  // C++ uses one overlay type ("rate_limit_exceeded") for three engine
  // outcomes; `exceededLimit` URL param differentiates the banner copy.
  if (notificationType === 'rate_limit_exceeded') {
    const limitCopy = (() => {
      if (exceededLimit === 'session_tx_count') {
        return {
          subtitle: 'has reached its session transaction limit',
          explanation: (
            <>
              This site has used all {maxTxPerSession} transactions allowed per
              session. You can approve this request, deny it, or adjust the
              session limit for this site.
            </>
          ),
        };
      }
      if (exceededLimit === 'price_unavailable') {
        return {
          subtitle: 'is requesting a payment',
          explanation: (
            <>
              The BSV/USD price is currently unavailable, so spending caps
              cannot be evaluated automatically. Review the satoshi amount
              above before approving, or deny to retry once the price feed
              is back.
            </>
          ),
        };
      }
      // Default — rate_limit branch (and any unrecognized exceededLimit value).
      return {
        subtitle: 'is making frequent requests',
        explanation: (
          <>
            This site is sending payment requests faster than your rate limit
            of {rateLimit} per minute. You can approve this request, deny it,
            or adjust your limits for this site.
          </>
        ),
      };
    })();
    return (
      <div style={overlayBackdrop}>
        <div style={cardStyle}>
          <HodosWalletHeader />
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '22px' }}>
            {!faviconError ? (
              <img
                src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=32`}
                width={32}
                height={32}
                style={{ borderRadius: 4, flexShrink: 0 }}
                onError={() => setFaviconError(true)}
                alt=""
              />
            ) : (
              <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            )}
            <div>
              <div style={{ fontSize: '16px', fontWeight: 700, color: COLORS.textDark }}>
                {cleanDomain}
              </div>
              <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '2px' }}>
                {limitCopy.subtitle}
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
            {limitCopy.explanation}
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
                  maxTxPerSession: maxTxPerSession,
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

  // ── Phase 1.5 Step 6 Commit E — scoped permission prompts ──
  // Shared modal for protocol_permission_prompt, basket_permission_prompt,
  // and counterparty_permission_prompt. Three buttons: Allow once / Always
  // allow for site / Deny. Differs from payment_confirmation in that the
  // grant is scope-tuple-keyed (V18 child tables) rather than spending-cap-
  // keyed (domain_permissions columns).
  if (notificationType === 'protocol_permission_prompt'
      || notificationType === 'basket_permission_prompt'
      || notificationType === 'counterparty_permission_prompt') {
    // Per-kind copy + scope display.
    const scopedCopy = (() => {
      if (notificationType === 'protocol_permission_prompt') {
        const tag = scopedProtocolCounterparty
          ? ` (with counterparty ${scopedProtocolCounterparty.slice(0, 12)}…)`
          : '';
        return {
          title: 'Protocol access',
          subtitle: 'wants permission to use a protocol',
          scopeLabel: 'Protocol',
          scopeValue: `${scopedProtocolName} (level ${scopedProtocolLevel}${tag})`,
          explanation:
            scopedProtocolLevel === 2
              ? `${cleanDomain} wants to derive a key for a specific counterparty. Each (site, protocol, counterparty) tuple is isolated by default — granting it here lets this site use this protocol without re-prompting.`
              : `${cleanDomain} wants to use protocol "${scopedProtocolName}" to derive a site-specific key. The derived key is bound to this origin and protocol — it does not link back to your identity key.`,
        };
      }
      if (notificationType === 'basket_permission_prompt') {
        return {
          title: 'Basket access',
          subtitle: `wants ${scopedBasketAccess === 'read_write' ? 'read + write' : 'read'} access to a basket`,
          scopeLabel: 'Basket',
          scopeValue: `${scopedBasket} (${scopedBasketAccess === 'read_write' ? 'read + write' : 'read-only'})`,
          explanation: scopedBasketAccess === 'read_write'
            ? `${cleanDomain} wants to read AND modify the "${scopedBasket}" basket. Granting this lets the site insert, list, and remove UTXOs in that basket — payment caps still gate any spend.`
            : `${cleanDomain} wants to view the "${scopedBasket}" basket contents. Read-only — the site cannot move or spend any UTXOs in this basket.`,
        };
      }
      // counterparty_permission_prompt
      return {
        title: 'Counterparty access',
        subtitle: 'wants to derive keys with a specific counterparty',
        scopeLabel: 'Counterparty',
        scopeValue: scopedCounterparty.length > 24
          ? `${scopedCounterparty.slice(0, 24)}…`
          : scopedCounterparty,
        explanation: `${cleanDomain} wants permission to derive shared keys with the counterparty above. This is required for encrypted messaging or P2P payments via this site.`,
      };
    })();

    return (
      <div style={overlayBackdrop}>
        <div style={cardStyle}>
          <HodosWalletHeader />
          {/* Domain row */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '18px' }}>
            {!faviconError ? (
              <img
                src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=32`}
                width={32}
                height={32}
                style={{ borderRadius: 4, flexShrink: 0 }}
                onError={() => setFaviconError(true)}
                alt=""
              />
            ) : (
              <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            )}
            <div>
              <div style={{ fontSize: '16px', fontWeight: 700, color: COLORS.textDark }}>
                {cleanDomain}
              </div>
              <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '2px' }}>
                {scopedCopy.subtitle}
              </div>
            </div>
          </div>

          {/* Scope summary box */}
          <div style={{
            background: COLORS.subduedGold,
            borderRadius: '10px',
            padding: '14px 16px',
            marginBottom: '18px',
          }}>
            <div style={{ fontSize: '11px', color: COLORS.textMuted, marginBottom: '4px', textTransform: 'uppercase', letterSpacing: '0.5px' }}>
              {scopedCopy.scopeLabel}
            </div>
            <div style={{ fontSize: '15px', fontWeight: 600, color: COLORS.textDark, wordBreak: 'break-all' }}>
              {scopedCopy.scopeValue}
            </div>
          </div>

          {/* Explanation */}
          <div style={{
            fontSize: '13px',
            color: COLORS.textMuted,
            lineHeight: 1.5,
            marginBottom: '22px',
          }}>
            {scopedCopy.explanation}
          </div>

          {/* Buttons — three actions: Deny, Allow once, Always allow */}
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', flexWrap: 'wrap' }}>
            <HodosButton variant="secondary" onClick={handleScopedDeny}>
              Deny
            </HodosButton>
            <HodosButton variant="secondary" onClick={handleScopedAllowOnce}>
              Allow once
            </HodosButton>
            <HodosButton variant="primary" onClick={handleScopedAlwaysAllow}>
              Always allow for this site
            </HodosButton>
          </div>
        </div>
      </div>
    );
  }

  // ── Phase 1.5 Step 1 — shared privacy-perimeter card style ──
  // Layered on top of the standard cardStyle. Gold border + soft halo via
  // hodosTheme.prompt.privacyPerimeter (the existing tier that had no callers
  // before Step 1 -- this is its first user).
  const privacyPerimeterCardStyle: React.CSSProperties = {
    ...cardStyle,
    border: promptTheme.privacyPerimeter.framingBorder,
    boxShadow: `${promptTheme.privacyPerimeter.framingShadow}, ${cardStyle.boxShadow}`,
  };

  const privacyPerimeterHeaderStyle: React.CSSProperties = {
    fontSize: promptTheme.privacyPerimeter.headerFontSize,
    fontWeight: promptTheme.privacyPerimeter.headerWeight,
    color: promptTheme.privacyPerimeter.headerColor,
    marginBottom: '12px',
  };

  const renderPrivacyPerimeterDomainRow = (subtitle: string) => (
    <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '14px' }}>
      {!faviconError ? (
        <img
          src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=32`}
          width={32}
          height={32}
          style={{ borderRadius: 4, flexShrink: 0 }}
          onError={() => setFaviconError(true)}
          alt=""
        />
      ) : (
        <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
      )}
      <div>
        <div style={{ fontSize: '15px', fontWeight: 700, color: COLORS.textDark }}>
          {cleanDomain}
        </div>
        <div style={{ fontSize: '12px', color: COLORS.textMuted, marginTop: '2px' }}>
          {subtitle}
        </div>
      </div>
    </div>
  );

  const renderPrivacyPerimeterCheckbox = () => (
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
        checked={rememberPrivacy}
        onChange={(e) => setRememberPrivacy(e.target.checked)}
        style={{ accentColor: COLORS.primary, width: '16px', height: '16px', cursor: 'pointer' }}
      />
      Always allow for this site
    </label>
  );

  // ── Identity key reveal (Phase 1.5 Step 1) ──
  // Fires when an external site calls getPublicKey({ identityKey: true }) and
  // the per-domain "Always allow" cache is empty. Locked copy: minimal +
  // neutral; gold privacy-perimeter framing (NOT red).
  if (notificationType === 'identity_key_reveal') {
    return (
      <div style={overlayBackdrop}>
        <div style={privacyPerimeterCardStyle}>
          <HodosWalletHeader />
          {renderPrivacyPerimeterDomainRow('is requesting access to private wallet data')}

          <h2 style={privacyPerimeterHeaderStyle}>
            Identity key request
            <InfoIcon style={{ fontSize: '13px', width: '16px', height: '16px' }} />
          </h2>

          <p style={{
            margin: '0 0 18px',
            fontSize: '14px',
            color: COLORS.textDark,
            lineHeight: 1.6,
          }}>
            <strong>{cleanDomain}</strong> is requesting your wallet identity
            key. This key can be used to identify you across sites.
          </p>

          {renderPrivacyPerimeterCheckbox()}

          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px' }}>
            <HodosButton variant="secondary" onClick={handleIdentityKeyDeny}>
              Deny
            </HodosButton>
            <HodosButton variant="primary" onClick={handleIdentityKeyApprove}>
              Approve
            </HodosButton>
          </div>
        </div>
      </div>
    );
  }

  // ── Key linkage reveal (Phase 1.5 Step 1) ──
  // Fires for /revealCounterpartyKeyLinkage and /revealSpecificKeyLinkage.
  // Verifier hex is truncated to first 4 + "..." + last 2 chars per locked copy.
  if (notificationType === 'key_linkage_reveal') {
    const truncateVerifier = (key: string): string => {
      if (!key) return 'an unknown verifier';
      if (key.length <= 8) return key;
      return key.slice(0, 4) + '...' + key.slice(-2);
    };
    const verifierLabel = truncateVerifier(linkageVerifier);
    const isSpecific = linkageKind === 'specific';

    return (
      <div style={overlayBackdrop}>
        <div style={privacyPerimeterCardStyle}>
          <HodosWalletHeader />
          {renderPrivacyPerimeterDomainRow('is requesting a key-linkage proof')}

          <h2 style={privacyPerimeterHeaderStyle}>Key linkage proof request</h2>

          <p style={{
            margin: '0 0 12px',
            fontSize: '14px',
            color: COLORS.textDark,
            lineHeight: 1.6,
          }}>
            <strong>{cleanDomain}</strong> is requesting a linkage proof to{' '}
            <span style={{
              fontFamily: 'monospace',
              fontSize: '12px',
              background: '#0f1117',
              padding: '2px 6px',
              borderRadius: '4px',
            }}>
              {verifierLabel}
            </span>
            . This proves two of your keys are related.
          </p>

          {isSpecific && (linkageProtocol || linkageKeyId) && (
            <p style={{
              margin: '0 0 18px',
              fontSize: '12px',
              color: COLORS.textMuted,
              lineHeight: 1.5,
            }}>
              {linkageProtocol && <>Protocol: <strong>{linkageProtocol}</strong>. </>}
              {linkageKeyId && <>Key ID: <strong>{linkageKeyId}</strong>.</>}
            </p>
          )}

          {renderPrivacyPerimeterCheckbox()}

          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px' }}>
            <HodosButton variant="secondary" onClick={handleKeyLinkageDeny}>
              Deny
            </HodosButton>
            <HodosButton variant="primary" onClick={handleKeyLinkageApprove}>
              Approve
            </HodosButton>
          </div>
        </div>
      </div>
    );
  }

  // ── Certificate disclosure notification ──
  if (notificationType === 'certificate_disclosure') {
    return (
      <div style={overlayBackdrop}>
        <div style={cardStyle}>
          <HodosWalletHeader />
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '22px' }}>
            {!faviconError ? (
              <img
                src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=32`}
                width={32}
                height={32}
                style={{ borderRadius: 4, flexShrink: 0 }}
                onError={() => setFaviconError(true)}
                alt=""
              />
            ) : (
              <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            )}
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

  // ── Phase 1.5 Step 5 — manifest_connect_bundle ──
  // Bundled connect prompt that consumes the dApp's wallet-manifest.json
  // permissions list. Three buttons: Connect (primary, grants everything
  // ticked with the user's default limits), Customize (toggle individual
  // permissions + adjust caps), Decline (block this domain in-session).
  if (notificationType === 'manifest_connect_bundle' && manifestData) {
    const formatUsd = (cents: number) => '$' + (cents / 100).toFixed(2);

    // Primary view — bundled summary
    if (!manifestShowCustomize) {
      return (
        <div style={overlayBackdrop}>
          <div style={{ ...cardStyle, maxWidth: '480px' }}>
            <HodosWalletHeader />

            {/* App branding row */}
            <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '16px' }}>
              {manifestData.iconUrl && !faviconError ? (
                <img
                  src={manifestData.iconUrl}
                  width={48}
                  height={48}
                  style={{ borderRadius: 8, flexShrink: 0 }}
                  onError={() => setFaviconError(true)}
                  alt=""
                />
              ) : !faviconError ? (
                <img
                  src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=48`}
                  width={48}
                  height={48}
                  style={{ borderRadius: 8, flexShrink: 0 }}
                  onError={() => setFaviconError(true)}
                  alt=""
                />
              ) : (
                <div style={{ ...avatarStyle, width: 48, height: 48 }}>
                  {getDomainInitial(notificationDomain)}
                </div>
              )}
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: '17px', fontWeight: 700, color: COLORS.textDark }}>
                  {manifestData.name || cleanDomain}
                </div>
                <div style={{ fontSize: '12px', color: COLORS.textMuted, marginTop: '2px' }}>
                  {cleanDomain}
                </div>
                {manifestData.description && (
                  <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '4px', lineHeight: 1.4 }}>
                    {manifestData.description}
                  </div>
                )}
              </div>
            </div>

            <div style={{ fontSize: '14px', color: COLORS.textDark, marginBottom: '12px' }}>
              This site is asking permission to:
            </div>

            {/* Permissions list with plain-language `purpose` strings */}
            <div style={{
              background: COLORS.subduedGold,
              borderRadius: '10px',
              padding: '14px 16px',
              marginBottom: '16px',
              maxHeight: '240px',
              overflowY: 'auto',
            }}>
              {manifestData.protocols.map((p, i) => (
                <div key={`proto-${i}`} style={permissionItem}>
                  <span style={checkmark}>&#10003;</span>
                  <span>{p.purpose || `Use protocol "${p.name}"`}</span>
                </div>
              ))}
              {manifestData.baskets.map((b, i) => (
                <div key={`basket-${i}`} style={permissionItem}>
                  <span style={checkmark}>&#10003;</span>
                  <span>
                    {b.purpose || `${b.access === 'read_write' ? 'Manage' : 'View'} "${b.name}"`}
                    {isProtectedBasket(b.name) && (
                      <span style={{ color: COLORS.error, fontWeight: 600, fontSize: '11px', marginLeft: '6px' }}>
                        (protected — won't auto-grant)
                      </span>
                    )}
                  </span>
                </div>
              ))}
              {manifestData.certificates.map((c, i) => (
                <div key={`cert-${i}`} style={permissionItem}>
                  <span style={checkmark}>&#10003;</span>
                  <span>{c.purpose || `Read ${c.fields.length} certificate field(s)`}</span>
                </div>
              ))}
              {manifestData.spending.perTransactionUsd > 0 && (
                <div style={permissionItem}>
                  <span style={checkmark}>&#10003;</span>
                  <span>
                    {manifestData.spending.purpose || 'Send payments'}
                    {' '}
                    <span style={{ color: COLORS.textMuted, fontSize: '12px' }}>
                      (up to ${manifestData.spending.perTransactionUsd}/tx, ${manifestData.spending.perSessionUsd}/session)
                    </span>
                  </span>
                </div>
              )}
              {manifestData.counterparties.map((cp, i) => (
                <div key={`cp-${i}`} style={permissionItem}>
                  <span style={checkmark}>&#10003;</span>
                  <span>{cp.purpose || 'Communicate with specific peers'}</span>
                </div>
              ))}
              {manifestData.protocols.length === 0 &&
                manifestData.baskets.length === 0 &&
                manifestData.certificates.length === 0 &&
                manifestData.counterparties.length === 0 &&
                manifestData.spending.perTransactionUsd === 0 && (
                <div style={{ color: COLORS.textMuted, fontSize: '13px', fontStyle: 'italic' }}>
                  No specific permissions declared.
                </div>
              )}
            </div>

            {/* Identity-key bundle checkbox — same pattern as domain_approval Step 1 */}
            <label style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              fontSize: '13px',
              color: COLORS.textDark,
              cursor: 'pointer',
              marginBottom: '10px',
              userSelect: 'none',
            }}>
              <input
                type="checkbox"
                checked={manifestAllowIdentityKey}
                onChange={(e) => setManifestAllowIdentityKey(e.target.checked)}
                style={{ accentColor: COLORS.primary, width: '16px', height: '16px', cursor: 'pointer' }}
              />
              Allow this site to identify you
              <InfoIcon />
            </label>

            {/* Phase 2.6-D Fix #4 — bundled scope grant checkbox. Default ON. */}
            <label style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              fontSize: '13px',
              color: COLORS.textDark,
              cursor: 'pointer',
              marginBottom: '16px',
              userSelect: 'none',
            }}>
              <input
                type="checkbox"
                checked={manifestAllowBundledScope}
                onChange={(e) => setManifestAllowBundledScope(e.target.checked)}
                style={{ accentColor: COLORS.primary, width: '16px', height: '16px', cursor: 'pointer' }}
              />
              Allow this site to perform wallet operations without asking each time
              <InfoIcon tooltip="When ticked, the wallet won't prompt you for individual protocol, basket, or counterparty grants on this site after the first connect. You can revoke this at any time from Manage Site Permissions. Sensitive operations (large payments, identity disclosure, sensitive certificate fields) always prompt regardless." />
            </label>

            {/* Default limits hint */}
            <div style={{ fontSize: '12px', color: COLORS.textMuted, marginBottom: '18px', lineHeight: 1.5 }}>
              Default payment limits: {formatUsd(manifestPerTxCents)}/tx, {formatUsd(manifestPerSessionCents)}/session.
              {' '}You can adjust these in Customize.
            </div>

            {/* Buttons: Decline / Customize / Connect */}
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', flexWrap: 'wrap' }}>
              <HodosButton variant="secondary" onClick={handleManifestDecline}>
                Decline
              </HodosButton>
              <HodosButton variant="secondary" onClick={() => setManifestShowCustomize(true)}>
                Customize
              </HodosButton>
              <HodosButton variant="primary" onClick={() => handleManifestConnect(false)}>
                Connect
              </HodosButton>
            </div>
          </div>
        </div>
      );
    }

    // Customize subview — per-permission checkboxes + payment-cap inputs
    return (
      <div style={overlayBackdrop}>
        <div style={{ ...cardStyle, maxWidth: '520px' }}>
          <HodosWalletHeader />

          <div style={{ fontSize: '15px', fontWeight: 700, color: COLORS.textDark, marginBottom: '4px' }}>
            Customize permissions for {manifestData.name || cleanDomain}
          </div>
          <div style={{ fontSize: '12px', color: COLORS.textMuted, marginBottom: '14px' }}>
            Untick anything you don't want to grant.
          </div>

          {/* Per-permission checkboxes */}
          <div style={{
            background: COLORS.subduedGold,
            borderRadius: '10px',
            padding: '12px 14px',
            marginBottom: '14px',
            maxHeight: '260px',
            overflowY: 'auto',
          }}>
            {manifestData.protocols.map((p, i) => (
              <label key={`cust-proto-${i}`} style={customizeRowLabel}>
                <input
                  type="checkbox"
                  checked={manifestSelectedProtocols.has(i)}
                  onChange={() => toggleManifestPerm(manifestSelectedProtocols, setManifestSelectedProtocols, i)}
                  style={customizeCheckbox}
                />
                <span><strong>Protocol:</strong> {p.purpose || p.name}</span>
              </label>
            ))}
            {manifestData.baskets.map((b, i) => (
              <label key={`cust-basket-${i}`} style={customizeRowLabel}>
                <input
                  type="checkbox"
                  checked={manifestSelectedBaskets.has(i) && !isProtectedBasket(b.name)}
                  disabled={isProtectedBasket(b.name)}
                  onChange={() => toggleManifestPerm(manifestSelectedBaskets, setManifestSelectedBaskets, i)}
                  style={customizeCheckbox}
                />
                <span>
                  <strong>Basket {b.access}:</strong> {b.purpose || b.name}
                  {isProtectedBasket(b.name) && (
                    <span style={{ color: COLORS.error, fontSize: '11px', marginLeft: '6px' }}>
                      (protected, never auto-granted)
                    </span>
                  )}
                </span>
              </label>
            ))}
            {manifestData.certificates.map((c, i) => (
              <label key={`cust-cert-${i}`} style={customizeRowLabel}>
                <input
                  type="checkbox"
                  checked={manifestSelectedCertificates.has(i)}
                  onChange={() => toggleManifestPerm(manifestSelectedCertificates, setManifestSelectedCertificates, i)}
                  style={customizeCheckbox}
                />
                <span><strong>Certificate fields:</strong> {c.purpose || c.fields.join(', ')}</span>
              </label>
            ))}
            {manifestData.counterparties.map((cp, i) => (
              <label key={`cust-cp-${i}`} style={customizeRowLabel}>
                <input
                  type="checkbox"
                  checked={manifestSelectedCounterparties.has(i)}
                  onChange={() => toggleManifestPerm(manifestSelectedCounterparties, setManifestSelectedCounterparties, i)}
                  style={customizeCheckbox}
                />
                <span><strong>Counterparty:</strong> {cp.purpose || cp.type || cp.counterparty.slice(0, 12) + '…'}</span>
              </label>
            ))}
          </div>

          {/* Identity-key toggle */}
          <label style={{ ...customizeRowLabel, marginBottom: '10px' }}>
            <input
              type="checkbox"
              checked={manifestAllowIdentityKey}
              onChange={(e) => setManifestAllowIdentityKey(e.target.checked)}
              style={customizeCheckbox}
            />
            <span>
              <strong>Identity:</strong> Allow this site to identify you across the Metanet
            </span>
          </label>

          {/* Phase 2.6-D Fix #4 — bundled scope grant toggle. Same as the
              domain_approval modal's allowBundledScope checkbox. */}
          <label style={{ ...customizeRowLabel, marginBottom: '14px' }}>
            <input
              type="checkbox"
              checked={manifestAllowBundledScope}
              onChange={(e) => setManifestAllowBundledScope(e.target.checked)}
              style={customizeCheckbox}
            />
            <span>
              <strong>Quiet mode:</strong> Don't prompt for individual protocol or basket grants while connected
            </span>
          </label>

          {/* Payment limit inputs */}
          <div style={{ fontSize: '13px', fontWeight: 600, color: COLORS.textDark, marginBottom: '8px' }}>
            Payment limits
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px', marginBottom: '14px' }}>
            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '12px', color: COLORS.textMuted }}>
              Per transaction ($)
              <input
                type="number"
                min={0}
                step={0.01}
                value={(manifestPerTxCents / 100).toFixed(2)}
                onChange={(e) => setManifestPerTxCents(Math.round(parseFloat(e.target.value || '0') * 100))}
                style={customizeNumberInput}
              />
            </label>
            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '12px', color: COLORS.textMuted }}>
              Per session ($)
              <input
                type="number"
                min={0}
                step={0.01}
                value={(manifestPerSessionCents / 100).toFixed(2)}
                onChange={(e) => setManifestPerSessionCents(Math.round(parseFloat(e.target.value || '0') * 100))}
                style={customizeNumberInput}
              />
            </label>
            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '12px', color: COLORS.textMuted }}>
              Rate (requests/min)
              <input
                type="number"
                min={0}
                value={manifestRateLimit}
                onChange={(e) => setManifestRateLimit(parseInt(e.target.value || '0'))}
                style={customizeNumberInput}
              />
            </label>
            <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '12px', color: COLORS.textMuted }}>
              Max tx / session
              <input
                type="number"
                min={0}
                value={manifestMaxTxPerSession}
                onChange={(e) => setManifestMaxTxPerSession(parseInt(e.target.value || '0'))}
                style={customizeNumberInput}
              />
            </label>
          </div>

          {/* Allow without limits — payment caps only, scoped grants unaffected */}
          <div style={{
            background: 'rgba(166, 124, 0, 0.08)',
            border: `1px solid ${COLORS.gold}`,
            borderRadius: '8px',
            padding: '10px 12px',
            marginBottom: '16px',
            fontSize: '12px',
            color: COLORS.textMuted,
            lineHeight: 1.5,
          }}>
            <strong style={{ color: COLORS.textDark }}>Trust this site fully?</strong>
            <br />
            "Allow without limits" raises payment caps to $1000/tx and $10000/session.
            Sensitive baskets (default change outputs, backup tokens) stay protected
            either way.
            <div style={{ marginTop: '8px' }}>
              <HodosButton variant="secondary" size="small" onClick={() => handleManifestConnect(true)}>
                Allow without limits
              </HodosButton>
            </div>
          </div>

          {/* Buttons: Back / Connect with current selections */}
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px' }}>
            <HodosButton variant="secondary" onClick={() => setManifestShowCustomize(false)}>
              Back
            </HodosButton>
            <HodosButton variant="primary" onClick={() => handleManifestConnect(false)}>
              Connect with these
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
          <HodosWalletHeader />
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '22px' }}>
            {!faviconError ? (
              <img
                src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=32`}
                width={32}
                height={32}
                style={{ borderRadius: 4, flexShrink: 0 }}
                onError={() => setFaviconError(true)}
                alt=""
              />
            ) : (
              <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            )}
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

          {/* Phase 1.5 Step 1 \u2014 bundled identity-key grant. Default ON; users
              who untick will be re-prompted via the privacy-perimeter modal
              the first time the site requests the identity key. */}
          <label style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            fontSize: '13px',
            color: COLORS.textDark,
            cursor: 'pointer',
            marginBottom: '10px',
            userSelect: 'none',
          }}>
            <input
              type="checkbox"
              checked={allowIdentityKey}
              onChange={(e) => setAllowIdentityKey(e.target.checked)}
              style={{ accentColor: COLORS.primary, width: '16px', height: '16px', cursor: 'pointer' }}
            />
            Allow this site to identify you
            <InfoIcon />
          </label>

          {/* Phase 2.6-D Fix #4 \u2014 bundled scope grant. Default ON; users who
              untick get the per-call permission prompts the engine would
              otherwise show (protocol/basket scopes the first time each one
              is touched). Protected baskets always prompt regardless. */}
          <label style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            fontSize: '13px',
            color: COLORS.textDark,
            cursor: 'pointer',
            marginBottom: '14px',
            userSelect: 'none',
          }}>
            <input
              type="checkbox"
              checked={allowBundledScope}
              onChange={(e) => setAllowBundledScope(e.target.checked)}
              style={{ accentColor: COLORS.primary, width: '16px', height: '16px', cursor: 'pointer' }}
            />
            Allow this site to perform wallet operations without asking each time
            <InfoIcon tooltip="When ticked, the wallet won't prompt you for individual protocol, basket, or counterparty grants on this site after the first connect. You can revoke this at any time from Manage Site Permissions. Sensitive operations (large payments, identity disclosure, sensitive certificate fields) always prompt regardless." />
          </label>

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
                // Phase 1.5 Step 5 bugfix — the parent domain_approval modal
                // already shows the "Allow this site to identify you" bundle
                // checkbox; rendering the form's own toggle here would create
                // two unsynced controls for the same setting.
                hideDisclosureSection={true}
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

  // ── Edit permissions (right-click "Manage Site Permissions") ──
  if (notificationType === 'edit_permissions') {
    return (
      <div style={overlayBackdrop} onClick={() => window.cefMessage?.send('overlay_close', [])}>
        <div style={cardStyle} onClick={(e) => e.stopPropagation()}>
          <HodosWalletHeader />
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '22px' }}>
            {!faviconError ? (
              <img
                src={`https://www.google.com/s2/favicons?domain=${notificationDomain}&sz=32`}
                width={32}
                height={32}
                style={{ borderRadius: 4, flexShrink: 0 }}
                onError={() => setFaviconError(true)}
                alt=""
              />
            ) : (
              <div style={avatarStyle}>{getDomainInitial(notificationDomain)}</div>
            )}
            <div>
              <div style={{ fontSize: '16px', fontWeight: 700, color: COLORS.textDark }}>
                {cleanDomain}
              </div>
              <div style={{ fontSize: '13px', color: COLORS.textMuted, marginTop: '2px' }}>
                Site permissions
              </div>
            </div>
          </div>

          <EditPermissionsForm
            domain={notificationDomain}
            onClose={() => window.cefMessage?.send('overlay_close', [])}
          />
        </div>
      </div>
    );
  }

  // ── Idle / no notification: render nothing (fully transparent) ──
  return null;
};

// ── Shared styles ──

const overlayBackdrop: React.CSSProperties = {
  position: 'fixed',
  inset: 0,
  backgroundColor: 'rgba(0, 0, 0, 0.45)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  overflow: 'hidden',
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

// Phase 1.5 Step 5 — Customize subview shared styles for manifest_connect_bundle.
const customizeRowLabel: React.CSSProperties = {
  display: 'flex',
  alignItems: 'flex-start',
  gap: '10px',
  fontSize: '13px',
  color: '#f0f0f0',
  lineHeight: 1.45,
  marginBottom: '8px',
  cursor: 'pointer',
  userSelect: 'none',
};

const customizeCheckbox: React.CSSProperties = {
  accentColor: '#a67c00',
  width: '16px',
  height: '16px',
  cursor: 'pointer',
  flexShrink: 0,
  marginTop: '2px',
};

const customizeNumberInput: React.CSSProperties = {
  background: '#0f1117',
  border: '1px solid #2a2d35',
  borderRadius: '6px',
  padding: '6px 8px',
  color: '#f0f0f0',
  fontSize: '13px',
  fontFamily: 'inherit',
};

export default BRC100AuthOverlayRoot;
