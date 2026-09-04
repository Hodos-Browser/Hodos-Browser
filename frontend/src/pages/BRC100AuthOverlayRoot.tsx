import React, { useState, useEffect, useCallback, useRef } from 'react';
import DomainPermissionForm from '../components/DomainPermissionForm';
import { walletFetch } from '../services/walletApi';
import type { DomainPermissionSettings } from '../components/DomainPermissionForm';
import { HodosButton } from '../components/HodosButton';
import { prompt as promptTheme } from '../styles/hodosTheme';
// beta.3 Phase 0.8 — the connect modal is a consent surface, so the rule about
// WHOSE numbers each limit field carries lives in a pure module that is
// unit-tested against the real source (T1f). See `R-PROV` in the phase contract.
import {
  resolveConnectLimits,
  useMyDefaults as computeUseMyDefaults,
  useSiteSuggested as computeUseSiteSuggested,
  shouldExpandLimits,
  formatMonthlyAllowance,
  formatCentsUsd,
  isEditableUsdText,
  isEditableIntText,
  usdTextToCents,
  intTextToNumber,
  type ConnectLimits,
  type LimitSources,
  type SiteSuggestedLimits,
} from '../utils/manifestConsent';

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
  // ⛔ The last sentence is not filler. Owner, 2026-08-23, reading a real
  // manifest: a site declared a PROTOCOL named "identity key retrieval"
  // described as "use your public identity key as your passwordless
  // account", and it read as though ticking or unticking it governed
  // identity disclosure. It does not — that row is an ordinary protocol
  // grant (`ProtocolUse`), while identity disclosure is a separate gate
  // (`identity_key_disclosure_allowed` / `CallKind::IdentityKeyReveal`).
  // A site can name a protocol anything, so the wallet cannot recognise
  // "the identity one" from site-authored text; the copy therefore states
  // outright that THIS control is the only one, rather than trying to
  // couple the two. See TICKET_connect_modal_two_views_drift.md.
  const text = tooltip || 'Identify you across the Metanet with your wallet identity key. This key is the same across every BRC-100 site you visit, so granting it lets sites recognize you between visits. This checkbox is the ONLY control over your identity key — no permission a site lists can grant it, whatever the site calls it.';
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

// beta.3 Phase 7b — `describeCounterparty` was deleted with the "Who these
// are with" footnote it served. Owner decision, 2026-09-04: the section asked
// "who?", and the only honest answer we have is a public key nobody can act
// on. The useful part underneath was SCOPE (bounded vs unbounded), not
// identity — and the counterparty is not a lever the user has: the manifest
// declares it, and the engine enforces the narrowing whether or not we print
// it (`is_protocol_granted` matches on counterparty). The identifier is still
// available where identifiers belong — Manage Site Permissions renders it per
// grant via /domain/permissions/protocol.

// Phase 1.5 Step 0 — Hodos wallet attribution header. Renders the
// Hodos_Gold_Wallet_Icon.svg at the top of every auth/payment/cert/
// permission prompt so the user can immediately tell the wallet (not the
// site) is the actor asking. Phase principle #1: Trust on first contact.
// The SVG already contains the "Hodos Wallet" wordmark, so no separate
// text label is rendered. Sits ABOVE the existing favicon + domain row so
// the hierarchy is "Hodos Wallet → talking about → [site]".
const attributionHeaderStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  paddingBottom: '14px',
  marginBottom: '16px',
  borderBottom: `1px solid ${COLORS.borderLight}`,
};

const HodosWalletHeader: React.FC = () => (
  <div style={attributionHeaderStyle}>
    <img
      src="/Hodos_Gold_Wallet_Icon.svg"
      alt="Hodos Wallet"
      height={36}
      style={{ display: 'block', flexShrink: 0, width: 'auto' }}
    />
  </div>
);

// Same attribution header, browser wordmark. Used by the prompts where the actor
// really is the BROWSER, not the wallet — the site-permission prompt (camera, mic,
// location, notifications, clipboard) asks for a Chromium capability and has nothing
// to do with the wallet, so branding it "Hodos Wallet" misattributes who is asking.
//
// ⚠️ Do NOT "unify" these two into one header. The wallet icon on wallet prompts is
// load-bearing (principle #1 above): it is how the user tells the wallet apart from
// the site. Picking the right one per prompt IS the feature.
// beta.3 P0.9 — disclosure for a connect modal that is ALSO answering a parked
// loopback / local-network permission. Rendered only when C++ set
// `grantsLocalAccess=1`, which it does only when such a permission is genuinely
// parked for this host. ⛔ A connect modal that grants only wallet access must
// never show this, and a modal that DOES carry the network grant must never hide
// it — the single consent is only defensible because it says what it covers.
const LocalAccessNotice: React.FC<{ onShown: () => void }> = ({ onShown }) => {
  // ⛔ Fail-closed link. C++ refuses to grant the loopback permission unless the
  // approval carries localAccessAcknowledged, and ONLY this component can set it.
  // So if a connect branch forgets to render the notice, the grant is declined
  // instead of being made silently — which is exactly what happened on the first
  // pass, when domain_approval carried the flag and rendered nothing.
  React.useEffect(() => { onShown(); }, [onShown]);
  return (
  // ⛔ Styling is deliberately INFORMATIONAL, not a warning. Owner call 2026-08-24:
  // loopback access is all-or-nothing everywhere — no browser offers per-app local
  // access — so a site that can reach the wallet can reach other local software by
  // definition. That is industry-standard behaviour, not an alarm. Dressing it in
  // warning colours would train users to dismiss a routine, expected disclosure.
  // It must still be READ, so it keeps its own panel and a bold lead line.
  <div style={{
    display: 'flex',
    gap: '10px',
    alignItems: 'flex-start',
    background: 'rgba(255, 255, 255, 0.04)',
    border: `1px solid ${COLORS.borderLight}`,
    borderRadius: '8px',
    padding: '10px 12px',
    marginBottom: '14px',
  }}>
    <span style={{ fontSize: '16px', lineHeight: '18px', flexShrink: 0 }}>💻</span>
    <div style={{ fontSize: '12px', lineHeight: 1.5, color: COLORS.textDark }}>
      <strong>This also allows access to other apps on this computer.</strong>{' '}
      Connecting lets this site reach software running on your machine, not only your
      Hodos wallet. You can change this later in Site controls.
    </div>
  </div>
  );
};

const HodosBrowserHeader: React.FC = () => (
  <div style={attributionHeaderStyle}>
    <img
      src="/Hodos_Gold_Browser_Icon.svg"
      alt="Hodos Browser"
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
  // beta.3 Phase 7b — the PAGE'S OWN favicon URL, supplied by C++ from
  // `Tab::favicon_url` (populated by OnFaviconURLChange for the tab strip).
  //
  // 🚨 This replaces `https://www.google.com/s2/favicons?domain=<site>`, which
  // every consent modal rendered — telling Google which site the user was being
  // asked to trust, at the moment of the decision, whether or not they approved
  // and whether or not they read it. From a privacy browser, on its most
  // sensitive surface.
  //
  // ⛔ Empty is normal and must stay renderable: C++ returns "" rather than
  // guessing, because the wrong site's icon on a consent screen is worse than
  // no icon. Empty falls through to the domain-initial avatar. ⛔ Never restore
  // a remote lookup as the fallback.
  const [pageFaviconUrl, setPageFaviconUrl] = useState('');

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
  // beta.3 P0.9 — set by C++ ONLY when this connect modal has claimed a parked
  // loopback / local-network permission, i.e. approving here also grants the site
  // access to other software running on this machine. It must never be set for an
  // ordinary CWI call, or the modal would claim to grant something it does not.
  const [grantsLocalAccess, setGrantsLocalAccess] = useState<boolean>(false);
  // Set only by LocalAccessNotice mounting. Sent with the approval so C++ can
  // fail closed when the disclosure was not actually displayed.
  const [localAccessShown, setLocalAccessShown] = useState<boolean>(false);
  const markLocalAccessShown = useCallback(() => setLocalAccessShown(true), []);
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

  // beta.3 Phase 0.8 V25 — the user's default for QUIET MODE, the widest grant
  // on the connect screen. Was hardcoded `true` with no way to change it.
  // Defaults to true here so a wallet without V25 behaves exactly as before.
  const savedDefaultBundledScopeRef = useRef<boolean>(true);

  // Phase 1.5 Step 5 — manifest_connect_bundle state. Parsed once from the
  // C++-supplied `manifest` query param; sub-permissions start all-selected
  // (matching the "Connect grants everything in the manifest" default per
  // PERMISSION_UX_DESIGN.md §5). Customize subview lets the user untick
  // individual permissions before connecting.
  interface ManifestProtocol {
    securityLevel: number; name: string; keyId: string; purpose: string;
    // BRC-73. BRC-116 §4.1: for Level 2 the wallet MUST identify the
    // counterparty to the user. Empty = unspecified ("any counterparty").
    counterparty?: string;
  }
  interface ManifestBasket { name: string; access: string; purpose: string; }
  interface ManifestCertificate {
    type: string; fields: string[]; purpose: string;
    // BRC-73. BRC-116 §4.4 scopes cert access by type + verifier + fields, so
    // the user is shown who receives the data.
    verifierPublicKey?: string;
  }
  interface ManifestSpending {
    // Our legacy shape only — same unit and period as our caps.
    perTransactionUsd: number;
    perSessionUsd: number;
    // 🚨 BRC-73 spendingAuthorization.amount: MONTHLY SATOSHIS. Displayed as
    // the site's request, never written and never pre-filled (`R-CAPS`).
    monthlySatoshis?: number;
    purpose: string;
  }
  interface ManifestCounterparty { type: string; counterparty: string; purpose: string; }
  interface ManifestData {
    name: string;
    description: string;
    iconUrl: string;
    expiresAt: number;
    version: string;
    /** "metanet" | "babbage" | "hodos-legacy" — which shape the site published. */
    sourceNamespace?: string;
    /** groupPermissions.description — the site's summary. Untrusted text. */
    groupDescription?: string;
    protocols: ManifestProtocol[];
    baskets: ManifestBasket[];
    certificates: ManifestCertificate[];
    spending: ManifestSpending;
    counterparties: ManifestCounterparty[];
  }
  const [manifestData, setManifestData] = useState<ManifestData | null>(null);
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
  // beta.3 Phase 0.8 — the four limit fields, plus WHOSE number each one is.
  //
  // 🚨 These used to be seeded straight from `m.spending.perTransactionUsd`,
  // and the summary view rendered them under the label "Default payment
  // limits" — so a site using our legacy shape set its own caps AND they were
  // presented as the user's. That is `R-PROV` inverted, and it is the defect
  // this phase closes. `sourceOf` now drives a visible mark on every field the
  // site supplied, in both toggle states (`P0.8-A9`, `P0.8-A12`).
  const [manifestLimits, setManifestLimits] = useState<ConnectLimits>({
    perTxCents: 100, perSessionCents: 1000, rateLimitPerMin: 30, maxTxPerSession: 100,
  });
  const [manifestLimitSource, setManifestLimitSource] = useState<LimitSources>({
    perTxCents: 'user', perSessionCents: 'user', rateLimitPerMin: 'user', maxTxPerSession: 'user',
  });
  const [manifestSiteSuggests, setManifestSiteSuggests] = useState<SiteSuggestedLimits>({});
  // Inline "Payment limits" disclosure on the SUMMARY view. Opens automatically
  // whenever a site number is on screen (contract §6a: a user must not approve
  // values hidden behind a collapsed section) — which replaces the first pass's
  // auto-jump into the Customize subview. That jump satisfied the same rule but
  // threw the user off the consent screen to do it, so they never saw the
  // itemised "this site is asking permission to" list with the Connect button.
  const [manifestLimitsOpen, setManifestLimitsOpen] = useState<boolean>(false);
  // 🚨 The four limit inputs are edited as TEXT, not as numbers.
  //
  // They were `<input type="number" value={(cents/100).toFixed(2)}>` — a
  // controlled field that reformats on every keystroke, so the value fought the
  // typist: typing "25" went 2 → "2.00" → 2.005 → "2.01" and the caret jumped.
  // Only the spinner arrows worked, because they emit a complete number each
  // time. The integer fields broke differently: clearing the box gave
  // parseInt("") = NaN, clamped to 0, so it could not be emptied to retype.
  //
  // Same fix the wallet's own "Default Limits for New Sites" already uses
  // (`components/wallet/ApprovedSitesTab.tsx`): keep the raw string the user is
  // typing, validate it with a regex, and derive the number from it. ⛔ Do not
  // "simplify" this back to a numeric controlled input.
  const [manifestLimitText, setManifestLimitText] = useState({
    perTx: '1.00', perSession: '10.00', rate: '30', maxTx: '100',
  });

  // The user's own defaults, fetched from /wallet/settings. Ref (not state)
  // because `applyParams` runs from a JS-injection callback whose closure would
  // otherwise capture a stale value — same reason as savedDefaultIdentityKeyRef.
  const savedUserLimitsRef = useRef<ConnectLimits>({
    perTxCents: 100, perSessionCents: 1000, rateLimitPerMin: 30, maxTxPerSession: 100,
  });
  // V24 `settings.default_prefill_from_manifest`. OFF = behaviour (b): the
  // user's defaults are pre-filled and the site's suggestion is shown beside
  // them. ON = behaviour (a), an informed opt-in.
  const savedPrefillFromManifestRef = useRef<boolean>(false);

  // Apply notification params from a query string (used by both initial load and JS injection)
  const applyParams = (queryString: string) => {
    const params = new URLSearchParams(queryString);
    const type = params.get('type') || '';
    const domain = params.get('domain') || '';

    // b1b — site-permission prompt params.
    setPermCode(params.get('perm') || '');
    // beta.3 Phase 7b — the page's own favicon, supplied by C++.
    // ⛔ Read AND reset here, inside applyParams, which runs on EVERY prompt.
    // This overlay is keep-alive: a field that is set but never re-read shows
    // the PREVIOUS site's icon beside THIS site's name — P0.8 defect 5, same
    // shape, same overlay. `|| ''` is the reset.
    setPageFaviconUrl(params.get('favicon') || '');
    setPermRequestId(params.get('requestId') || '');
    setGrantsLocalAccess(params.get('grantsLocalAccess') === '1');
    setLocalAccessShown(false);   // re-earned on every show, never inherited
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
    // 🚨 beta.3 Phase 0.8: these two were NEVER reset here. The notification
    // overlay is keep-alive and reused for every prompt, so whatever the user
    // chose for the LAST site was still on screen for the NEXT one — a consent
    // checkbox showing a state the user never chose for the site in front of
    // them. Now re-seeded from the V25 user default on every prompt, which
    // fixes the leak and the hardcoded default in one move.
    setAllowBundledScope(savedDefaultBundledScopeRef.current);
    setManifestAllowBundledScope(savedDefaultBundledScopeRef.current);

    // Phase 1.5 Step 5 — manifest_connect_bundle params.
    // Reset every time so a previous site's manifest doesn't leak in.
    setManifestData(null);
    setManifestLimitsOpen(false);
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
        // ⛔ R-PROV. The limit fields start from the USER's defaults unless the
        // user has explicitly opted in to site pre-fill, and either way every
        // site-sourced field is marked. `resolveConnectLimits` is the pure,
        // unit-tested rule (T1f) — do not inline a second copy of it here.
        const resolved = resolveConnectLimits({
          userDefaults: savedUserLimitsRef.current,
          spending: m.spending,
          prefillFromManifest: savedPrefillFromManifestRef.current,
        });
        setManifestLimits(resolved.values);
        setManifestLimitSource(resolved.sourceOf);
        setManifestSiteSuggests(resolved.siteSuggests);
        syncLimitText(resolved.values);
        // Owner requirement (contract §6a): a user must not approve values
        // hidden behind a collapsed section. If the site's numbers are on
        // screen at all, open the section that shows them.
        setManifestLimitsOpen(shouldExpandLimits(resolved.sourceOf, resolved.siteSuggests));
      } catch (e) {
        console.error('[Hodos] Failed to parse manifest from extraParams:', e);
        setManifestData(null);
      }
    }
  };

  useEffect(() => {
    // Register JS injection callbacks for C++ to call (avoids full page navigation)
    (window as any).showNotification = (queryString: string) => {
      // ⛔ Refresh BEFORE applying, never after. The tempting alternative —
      // render immediately from the cached refs and re-resolve when a fresh
      // fetch lands — makes the numbers CHANGE UNDER THE USER'S EYES on a
      // spending-limit consent screen. A user who reads "$10.00" and clicks
      // Approve on "$1.00" (or the reverse) has been misled by us, which is a
      // worse failure than the stale value this fixes.
      //
      // Bounded by a timeout so an unreachable wallet cannot swallow the
      // prompt entirely: on timeout we fall through with the last known good
      // refs, which is exactly the old behaviour and no worse.
      const REFRESH_TIMEOUT_MS = 1200;
      Promise.race([
        refreshWalletDefaults(),
        new Promise((resolve) => setTimeout(resolve, REFRESH_TIMEOUT_MS)),
      ]).then(() => applyParams(queryString));
    };
    (window as any).hideNotification = () => {
      setNotificationType('');
      setNotificationDomain('');
    };

    // Phase 1.5 Step 5 — fetch the user's default for the identity-key bundle
    // checkbox. If they set it to OFF in Approved Sites, fresh-site prompts
    // should start the checkbox unticked. Default to true on any fetch failure
    // so we don't accidentally degrade UX on a wallet that doesn't have V19 yet.
    //
    // 🚨 beta.3 Phase 0.8, MEASURED LIVE 2026-08-23: this used to run ONLY here,
    // in a mount-only effect. The notification overlay is KEEP-ALIVE — it mounts
    // once per browser launch and C++ then drives it by injecting
    // `window.showNotification()` per prompt — so these three refs were a
    // snapshot of the wallet settings AS OF BROWSER START, never refreshed.
    // Any change the user made in "Default Limits for New Sites" was ignored
    // until they restarted the browser. Found because toggling the V24 pre-fill
    // opt-in did nothing; it also silently affected the identity-key default
    // and all four limit values. `refreshWalletDefaults` is now also awaited at
    // the top of every prompt (see `showNotification` above).
    const refreshWalletDefaults = () => walletFetch('/wallet/settings')
      .then((res) => res.ok ? res.json() : null)
      .then((data) => {
        if (!data) return;
        if (typeof data.default_identity_key_disclosure_allowed === 'boolean') {
          const def = data.default_identity_key_disclosure_allowed;
          savedDefaultIdentityKeyRef.current = def;
          setAllowIdentityKey(def);
          setManifestAllowIdentityKey(def);
        }
        // beta.3 Phase 0.8 — the user's OWN four defaults. Before this, the
        // connect modal hardcoded 100/1000/30/100 and ignored whatever the user
        // had set in "Default Limits for New Sites"; now they are what a
        // manifest connect starts from (`R-PROV`).
        const userLimits: ConnectLimits = {
          perTxCents: typeof data.default_per_tx_limit_cents === 'number'
            ? data.default_per_tx_limit_cents : 100,
          perSessionCents: typeof data.default_per_session_limit_cents === 'number'
            ? data.default_per_session_limit_cents : 1000,
          rateLimitPerMin: typeof data.default_rate_limit_per_min === 'number'
            ? data.default_rate_limit_per_min : 30,
          maxTxPerSession: typeof data.default_max_tx_per_session === 'number'
            ? data.default_max_tx_per_session : 100,
        };
        savedUserLimitsRef.current = userLimits;
        savedPrefillFromManifestRef.current =
          data.default_prefill_from_manifest === true;
        if (typeof data.default_bundled_scope_grant === 'boolean') {
          savedDefaultBundledScopeRef.current = data.default_bundled_scope_grant;
        }
        // A notification may already be on screen when this resolves (the
        // fetch is async and applyParams can run first). Re-resolve so the
        // fields show the user's real defaults rather than the hardcoded
        // fallback — but only while nothing is site-sourced yet, so this can
        // never clobber a value the user is already looking at.
        setManifestLimits((cur) => {
          const untouched = cur.perTxCents === 100 && cur.perSessionCents === 1000
            && cur.rateLimitPerMin === 30 && cur.maxTxPerSession === 100;
          if (!untouched) return cur;
          syncLimitText(userLimits);
          return userLimits;
        });
      })
      .catch(() => { /* silent — keep the last known good values */ });

    refreshWalletDefaults();

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
          JSON.stringify({ approved: true, whitelist: true, localAccessAcknowledged: localAccessShown }),
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
          JSON.stringify({ approved: true, whitelist: true, localAccessAcknowledged: localAccessShown }),
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
      const perTx = allowWithoutLimits ? 100000 : manifestLimits.perTxCents;
      const perSession = allowWithoutLimits ? 1000000 : manifestLimits.perSessionCents;
      const rate = allowWithoutLimits ? 1000 : manifestLimits.rateLimitPerMin;
      const maxTx = allowWithoutLimits ? 10000 : manifestLimits.maxTxPerSession;

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
        // ⛔ COUNTERPARTY: write ONLY a concrete compressed public key. Owner
        // approved 2026-08-23 after the measurement below; do not widen this
        // without repeating it.
        //
        // Phase 0.8 introduced a display-vs-reality gap here: the modal showed
        // the Level-2 counterparty but every row landed with `counterparty =
        // NULL` (= ANY counterparty), so the screen promised something
        // narrower than the grant.
        //
        // 🚨 MEASURED LIVE 2026-08-23, and this is WHY 'self'/'anyone' stay
        // NULL rather than being written verbatim. `createHmac` was fired for
        // a protocol the manifest declares as `counterparty: "self"`, and the
        // wallet logged:
        //     🛡️ engine Prompt (scoped) ... endpoint=/createHmac kind=ProtocolUse
        // `ProtocolUse` (not `CounterpartyUse`) means the request arrived with
        // counterparty = None — `handlers.rs :: peek_scoped_grant_scope_protocol`
        // collapses "self", "anyone" and "" to None before the gate sees them.
        // Storing the literal 'self' would then be compared against a NULL
        // parameter in `is_protocol_granted`:
        //     ('self' IS NULL AND NULL IS NULL)  -> false
        //     'self' = NULL                      -> NULL, not true
        //     counterparty IS NULL               -> false
        // i.e. it could NEVER match, and would permanently re-prompt a call
        // the user had already approved. Denying is safer than over-granting,
        // but it is still a regression, and an invisible one.
        //
        // A real 66-hex key DOES arrive intact, so writing it genuinely
        // narrows the grant to match what the modal displayed.
        const cp = (p.counterparty || '').trim();
        const concreteCounterparty = /^0[23][0-9a-fA-F]{64}$/.test(cp) ? cp : undefined;
        writes.push(post('/domain/permissions/protocol', {
          domain,
          securityLevel: p.securityLevel,
          protocolName: p.name,
          keyId: p.keyId || '*',
          ...(concreteCounterparty ? { counterparty: concreteCounterparty } : {}),
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
          localAccessAcknowledged: localAccessShown,
          whitelist: true,
        })]);
      }
      window.cefMessage?.send('overlay_close', []);
    } catch (e) {
      console.error('[Hodos] Error in manifest connect flow:', e);
    }
  };

  // Mirror numeric limits into the editable text drafts. Called whenever the
  // values change from OUTSIDE the inputs (initial resolve, "Use my defaults",
  // "Use this site's suggested limits") — never while the user is typing, which
  // is the whole point.
  const syncLimitText = (v: ConnectLimits) => {
    setManifestLimitText({
      perTx: (v.perTxCents / 100).toFixed(2),
      perSession: (v.perSessionCents / 100).toFixed(2),
      rate: String(v.rateLimitPerMin),
      maxTx: String(v.maxTxPerSession),
    });
  };

  // Typing handler for a dollar field. Accepts a partial number ("", "2", "2.",
  // "2.5") so the box can be cleared and retyped; the numeric state tracks what
  // is currently parseable.
  const onLimitTextUsd = (
    key: 'perTx' | 'perSession',
    field: 'perTxCents' | 'perSessionCents',
    raw: string,
  ) => {
    if (!isEditableUsdText(raw)) return;  // reject the keystroke, keep old text
    setManifestLimitText((t) => ({ ...t, [key]: raw }));
    setLimitField(field, usdTextToCents(raw));
  };

  // Same for a whole-number field.
  const onLimitTextInt = (
    key: 'rate' | 'maxTx',
    field: 'rateLimitPerMin' | 'maxTxPerSession',
    raw: string,
  ) => {
    if (!isEditableIntText(raw)) return;
    setManifestLimitText((t) => ({ ...t, [key]: raw }));
    setLimitField(field, intTextToNumber(raw));
  };

  // On blur, show the value that will actually be submitted — so a box left
  // empty reads "0.00" rather than looking like it kept what was there.
  const onLimitBlurUsd = (key: 'perTx' | 'perSession', cents: number) =>
    setManifestLimitText((t) => ({ ...t, [key]: (cents / 100).toFixed(2) }));
  const onLimitBlurInt = (key: 'rate' | 'maxTx', n: number) =>
    setManifestLimitText((t) => ({ ...t, [key]: String(n) }));

  // Edit one limit field. ⛔ Editing a site-suggested value makes it the USER's
  // choice, so the "suggested by site" mark is cleared for that field — the
  // mark answers "whose number is this?", and once the user has typed over it
  // the answer has changed. It must NOT be sticky, or it would go on accusing
  // the site of a number the user picked.
  const setLimitField = (field: keyof ConnectLimits, value: number) => {
    const safe = Number.isFinite(value) && value >= 0 ? value : 0;
    setManifestLimits((cur) => ({ ...cur, [field]: safe }));
    setManifestLimitSource((cur) => ({ ...cur, [field]: 'user' }));
  };

  // beta.3 Phase 0.8 — the FORWARD direction. Contract §6a's definition of the
  // shipped behaviour (b) is "…show the site's suggestion beside each field as
  // information, plus an explicit **'Use the site's recommended settings'
  // button**". The first pass built only the revert half, so a user who WANTED
  // the site's numbers had to retype them by hand — which is most users, most
  // of the time, on a site whose recommendations are what make it work.
  //
  // ⛔ Adopting the site's numbers is an affirmative act and stays marked: the
  // fields flip to `sourceOf = 'site'`, so `R-PROV` holds and "Use my defaults"
  // remains available. Only figures already in OUR unit and period can appear
  // here at all (`siteSuggestsAnything`) — BRC-73's monthly satoshis never do.
  const handleUseSiteSuggested = () => {
    const adopted = computeUseSiteSuggested(manifestLimits, manifestSiteSuggests);
    setManifestLimits(adopted.values);
    setManifestLimitSource(adopted.sourceOf);
    syncLimitText(adopted.values);
  };

  // beta.3 Phase 0.8 (`P0.8-A10`) — one click puts every limit field back to
  // the user's own default and clears every "suggested by this site" mark.
  // Reverts all four, not only the marked ones: a user who has been editing
  // expects one button to restore the whole block.
  const handleUseMyDefaults = () => {
    const reverted = computeUseMyDefaults(savedUserLimitsRef.current);
    setManifestLimits(reverted.values);
    setManifestLimitSource(reverted.sourceOf);
    syncLimitText(reverted.values);
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
    // ⛔ beta.3 Phase 7a — THE BACKSTOP, and it is a safety property, not styling.
    // Without these two lines the card grows to its content, and a flex item
    // centred in a shorter container overflows BOTH ends with no scrollbar
    // (the backdrop is overflow:hidden), so the overflow is unreachable rather
    // than merely below the fold. Measured at base 887c2cd: 10 declared
    // protocols put Decline/Customize/Connect into a 2-pixel strip that was
    // still clickable and no longer legible — a user choosing between refuse,
    // review and grant with nothing to tell them apart (MEASUREMENTS.md M2).
    //
    // ⚠️ This is SHARED BY EVERY CONSENT BRANCH in this file (~12 modals), which
    // is the point: collapsing one branch's long section fixes that branch,
    // this fixes the class. Do not move it onto a single branch, and do not
    // delete it because "the content fits now" — the content has grown twice.
    // 🚨 THE 88px IS ARITHMETIC, NOT A MARGIN YOU MAY ROUND. This card is
    // content-box (no border-box reset), so `max-height` caps the CONTENT box
    // and the padding sits OUTSIDE it. `calc(100vh - 32px)` was measured
    // producing a 1056px border box in a 1032px viewport — the cap engaged and
    // the buttons still went off-screen. 88 = 32 (breathing room, 16 top +
    // 16 bottom) + 56 (this card's own `padding: '28px 32px'`, top + bottom).
    // ⛔ If you change `padding` above, change this number in the same edit, or
    // re-run `phase-7a-modal-viewport/verify.py` and watch it go red.
    maxHeight: 'calc(100vh - 88px)',
    overflowY: 'auto',
  };

  // ── No wallet notification ──
  if (notificationType === 'no_wallet') {
    return (
      <div style={overlayBackdrop}>
        <div style={cardStyle}>
          <HodosWalletHeader />
          {/* Domain avatar + title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '20px' }}>
            {pageFaviconUrl && !faviconError ? (
              <img
                src={pageFaviconUrl}
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
            {pageFaviconUrl && !faviconError ? (
              <img
                src={pageFaviconUrl}
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
    // `note` is optional and only set for the Phase 0.9 network asks — camera and
    // mic are self-explanatory, "talk to a server on your computer" is not.
    // ⛔ The note deliberately does NOT mention the wallet. Wallet traffic is served
    // by our own resource handler (isWalletEndpoint) and never reaches the network
    // stack, so it never raises this prompt — implying otherwise would tell the user
    // that blocking here protects their wallet, which is false.
    //
    // ⛔ `noOnce` — MEASURED 2026-08-24, do not remove without re-measuring.
    // CEF's cef_permission_request_result_t offers only ACCEPT / DENY / DISMISS /
    // IGNORE. There is NO "grant once". So on the OnShowPermissionPrompt path,
    // answering ACCEPT makes Chromium write a PERSISTENT content setting, and an
    // "Allow this time" button would be a lie: measured on this build, clicking it
    // for example.com produced `loopback_network -> ALLOW` in the profile's
    // Preferences, which survives restart.
    // Camera/mic are exempt because they arrive via OnRequestMediaAccessPermission,
    // whose Continue() grants the single request WITHOUT persisting anything — so
    // for those, "Allow this time" is truthful (confirmed: a stored mic Allow exists
    // in our DB with no matching Chromium content setting).
    const PERM: Record<string, { icon: string; label: string; note?: string; noOnce?: boolean }> = {
      camera:        { icon: '📷', label: 'use your camera' },
      microphone:    { icon: '🎤', label: 'use your microphone' },
      camera_mic:    { icon: '🎥', label: 'use your camera and microphone' },
      location:      { icon: '📍', label: 'know your location' },
      notifications: { icon: '🔔', label: 'show notifications' },
      clipboard:     { icon: '📋', label: 'read your clipboard' },
      loopback:      {
        noOnce: true,
        icon: '💻',
        label: 'connect to a server running on your computer',
        note: 'Most websites never need this. Only allow it if you expect this site to work with software running on this machine.',
      },
      local_network: {
        noOnce: true,
        icon: '🏠',
        label: 'connect to other devices on your local network',
        note: 'This would let the site reach printers, routers and other devices on your network. Most websites never need this.',
      },
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
          <HodosBrowserHeader />
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginBottom: '20px' }}>
            {pageFaviconUrl && !faviconError ? (
              <img
                src={pageFaviconUrl}
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

          <div style={{ fontSize: '40px', textAlign: 'center', marginBottom: perm.note ? '14px' : '22px' }}>{perm.icon}</div>

          {perm.note && (
            <div style={{
              fontSize: '13px',
              color: COLORS.textMuted,
              lineHeight: 1.5,
              marginBottom: '22px',
              textAlign: 'center',
            }}>
              {perm.note}
            </div>
          )}

          {perm.noOnce ? (
            // Two buttons only — offering "Allow this time" here would promise an
            // ephemeral grant the platform cannot give (see `noOnce` above).
            <div style={{ display: 'flex', gap: '12px', marginBottom: '10px' }}>
              <HodosButton variant="secondary" disabled={permSubmitted} onClick={() => decide('block')} style={{ flex: 1 }}>
                Don't allow
              </HodosButton>
              <HodosButton variant="primary" disabled={permSubmitted} onClick={() => decide('allow_always')} style={{ flex: 1 }}>
                Allow
              </HodosButton>
            </div>
          ) : (
            <>
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
            </>
          )}
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
            {pageFaviconUrl && !faviconError ? (
              <img
                src={pageFaviconUrl}
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
            {pageFaviconUrl && !faviconError ? (
              <img
                src={pageFaviconUrl}
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
      {pageFaviconUrl && !faviconError ? (
        <img
          src={pageFaviconUrl}
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
            {pageFaviconUrl && !faviconError ? (
              <img
                src={pageFaviconUrl}
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
    // beta.3 Phase 0.8 — drives the R-PROV marking. `anyLimitFromSite` is true
    // only when a field actually carries a site-supplied value; it is computed
    // from `sourceOf`, which `resolveConnectLimits` owns, so no render path can
    // show a site value without the mark.
    const anyLimitFromSite =
      manifestLimitSource.perTxCents === 'site'
      || manifestLimitSource.perSessionCents === 'site'
      || manifestLimitSource.rateLimitPerMin === 'site'
      || manifestLimitSource.maxTxPerSession === 'site';
    const siteSuggestsAnything =
      manifestSiteSuggests.perTxCents !== undefined
      || manifestSiteSuggests.perSessionCents !== undefined;
    const monthlyAllowance = formatMonthlyAllowance(manifestData.spending);

    // ⭐ ONE implementation of the limits block, used by BOTH the summary
    // disclosure and the Customize subview. Two copies would drift, and the
    // provenance marking is exactly the thing that must not drift.
    const renderLimitFields = () => (
      <>
        <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap', marginBottom: '10px' }}>
          <button type="button" onClick={handleUseMyDefaults} style={useMyDefaultsButton}>
            Use my defaults
          </button>
          {siteSuggestsAnything && !anyLimitFromSite && (
            <button type="button" onClick={handleUseSiteSuggested} style={useMyDefaultsButton}>
              Use this site's suggested limits
            </button>
          )}
        </div>
        {/* R-PROV legend. Short enough to hold one line at 440px — the two
            sentences these replaced wrapped to three lines apiece and read as
            a warning banner rather than a caption (owner, 2026-08-23). The
            asterisk carries the mark; this line says what it means, and one
            of the two ALWAYS renders whenever the site suggested anything, so
            provenance is never silent. */}
        {anyLimitFromSite && (
          <div style={{ fontSize: '12px', color: COLORS.textMuted, marginBottom: '8px' }}>
            <span style={siteSuggestedMark}>***</span> = this site&apos;s numbers, not your defaults.
          </div>
        )}
        {!anyLimitFromSite && siteSuggestsAnything && (
          <div style={{ fontSize: '12px', color: COLORS.textMuted, marginBottom: '8px' }}>
            Your defaults. This site&apos;s suggestion is shown below, not applied.
          </div>
        )}
        {monthlyAllowance && (
          <div style={{ fontSize: '12px', color: COLORS.textMuted, marginBottom: '8px', lineHeight: 1.5 }}>
            This site declares a monthly allowance of <strong>{monthlyAllowance}</strong>.
            Hodos does not enforce monthly limits — the limits below are what will actually apply.
          </div>
        )}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px' }}>
          <label style={limitFieldLabel(manifestLimitSource.perTxCents)}>
            <span>
              Per transaction ($)
              {manifestLimitSource.perTxCents === 'site' && (
                <span style={siteSuggestedMark} title="Suggested by this site, not your default">***</span>
              )}
            </span>
            <input type="text" inputMode="decimal"
              value={manifestLimitText.perTx}
              onChange={(e) => onLimitTextUsd('perTx', 'perTxCents', e.target.value)}
              onBlur={() => onLimitBlurUsd('perTx', manifestLimits.perTxCents)}
              style={limitInputStyle(manifestLimitSource.perTxCents)} />
            {manifestSiteSuggests.perTxCents !== undefined
              && manifestLimitSource.perTxCents !== 'site' && (
              <span style={siteSuggestionHint}>
                site suggests {formatCentsUsd(manifestSiteSuggests.perTxCents)}
              </span>
            )}
          </label>
          <label style={limitFieldLabel(manifestLimitSource.perSessionCents)}>
            <span>
              Per session ($)
              {manifestLimitSource.perSessionCents === 'site' && (
                <span style={siteSuggestedMark} title="Suggested by this site, not your default">***</span>
              )}
            </span>
            <input type="text" inputMode="decimal"
              value={manifestLimitText.perSession}
              onChange={(e) => onLimitTextUsd('perSession', 'perSessionCents', e.target.value)}
              onBlur={() => onLimitBlurUsd('perSession', manifestLimits.perSessionCents)}
              style={limitInputStyle(manifestLimitSource.perSessionCents)} />
            {manifestSiteSuggests.perSessionCents !== undefined
              && manifestLimitSource.perSessionCents !== 'site' && (
              <span style={siteSuggestionHint}>
                site suggests {formatCentsUsd(manifestSiteSuggests.perSessionCents)}
              </span>
            )}
          </label>
          <label style={limitFieldLabel(manifestLimitSource.rateLimitPerMin)}>
            Rate (requests/min)
            <input type="text" inputMode="numeric"
              value={manifestLimitText.rate}
              onChange={(e) => onLimitTextInt('rate', 'rateLimitPerMin', e.target.value)}
              onBlur={() => onLimitBlurInt('rate', manifestLimits.rateLimitPerMin)}
              style={limitInputStyle(manifestLimitSource.rateLimitPerMin)} />
          </label>
          <label style={limitFieldLabel(manifestLimitSource.maxTxPerSession)}>
            Max tx / session
            <input type="text" inputMode="numeric"
              value={manifestLimitText.maxTx}
              onChange={(e) => onLimitTextInt('maxTx', 'maxTxPerSession', e.target.value)}
              onBlur={() => onLimitBlurInt('maxTx', manifestLimits.maxTxPerSession)}
              style={limitInputStyle(manifestLimitSource.maxTxPerSession)} />
          </label>
        </div>
      </>
    );

    // Primary view — bundled summary
    // ⭐ beta.3 Phase 7b — ONE view. There is no `manifestShowCustomize`
    // branch any more: the per-item ticks that lived on a second screen are
    // on this one, all ticked by default, so the fast path is still one
    // click and there is no second wording to drift from.
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
            ) : pageFaviconUrl && !faviconError ? (
              <img
                src={pageFaviconUrl}
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

            {/* ⭐ beta.3 Phase 7b — ORDER IS DELIBERATE (owner, 2026-09-04).
                Quiet mode DOMINATES the list below it: while it is on, every tick
                in that list is inert, because `decide_scoped_grant` returns Silent
                on `bundled_scope_grant` before it ever consults the V18 rows those
                boxes write. So the governing decision comes FIRST, its consequence
                is stated immediately under it, and the thing it governs comes after.
                ⛔ The previous order put the greyed-out list ABOVE the control that
                greyed it — the user met the effect before the cause. */}

          {/* Identity-key bundle checkbox — same pattern as domain_approval Step 1 */}
          <label style={{
            display: 'flex',
            alignItems: 'flex-start',
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
              style={{
                accentColor: COLORS.primary, width: '16px', height: '16px',
                cursor: 'pointer', flexShrink: 0, marginTop: '2px',
              }}
            />
            {/* ⛔ Wording is SHARED with the Customize view — keep them
                identical. Owner-reported 2026-08-23, the same drift as the
                quiet-mode label: one control read two different ways
                depending on which screen you were on, and the shorter one
                dropped "across the Metanet" — the fact that actually matters,
                since this key is the SAME on every BRC-100 site and is
                therefore what lets sites correlate you between them.
                Single <span> so the flex container doesn't shatter it. */}
            <span style={{ lineHeight: 1.45 }}>
              <strong>Identity:</strong> Allow this site to identify you across the Metanet
              <InfoIcon />
            </span>
          </label>

          {/* Phase 2.6-D Fix #4 — bundled scope grant checkbox. Default ON. */}
          {/* ⚠️ The text MUST stay inside a single <span>. This <label> is a
              flex container with `gap: 8px`, so every child element and text
              node becomes its own FLEX ITEM and wraps independently — with
              the gap inserted between each. When the wording gained <strong>
              and <em>, the line shattered into fragments ("mode" under
              "Quiet", "any" on its own). `flexShrink: 0` on the box is the
              other half: without it the checkbox is compressed to a
              different size than its neighbour once the row overflows.
              Both reported by the owner on 2026-08-23. */}
          <label style={{
            display: 'flex',
            alignItems: 'flex-start',
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
              style={{
                accentColor: COLORS.primary, width: '16px', height: '16px',
                cursor: 'pointer', flexShrink: 0, marginTop: '2px',
              }}
            />
            <span style={{ lineHeight: 1.45 }}>
            {/* ⛔ Owner-reported 2026-08-23: this used to read "Allow this
                site to perform wallet operations without asking each time",
                which omits the single most important fact about the control
                — that it also covers protocols and baskets the site NEVER
                DECLARED. The Customize view already said so; the summary,
                which is the screen most users actually read, did not. One
                control must not carry two meanings. Wording is now shared
                with Customize; keep them identical. */}
            <strong>Quiet mode:</strong> let this site use <em>any</em> protocol or
            basket without asking — including ones it did not list above
            <InfoIcon tooltip="When ticked, this site can use ANY protocol or basket without prompting - including ones it did not declare in its manifest. Untick it to approve only the specific items listed above. Protected baskets (change outputs, backup tokens) are never included. Sensitive operations - large payments, identity disclosure, sensitive certificate fields - always prompt regardless. Revoke any time from Manage Site Permissions." />
            </span>
          </label>

          {/* 🚨 Quiet mode makes the ticks above INERT — say so beside them.
              `matrix_c.rs :: decide_scoped_grant` returns Silent on
              `bundled_scope_grant` BEFORE it consults `scoped_grant_exists`, so
              while quiet mode is on the V18 rows these boxes write are never
              read: every ProtocolUse and BasketAccess from this domain goes
              silent, declared or not. (Protected baskets are still excluded.)
              ⛔ Disabling the boxes is deliberate — it is the only state in which
              the two controls cannot contradict each other, so there is no
              hidden cross-toggling. Narrowing the flag itself is an ENGINE
              change: beta.3 Phase 7c. */}
          {manifestAllowBundledScope && (
            <div style={{
              fontSize: '12px', color: COLORS.textDark, marginBottom: '16px',
              lineHeight: 1.5, background: 'rgba(166, 124, 0, 0.10)',
              border: `1px solid ${COLORS.gold}`, borderRadius: '8px', padding: '10px 12px',
            }}>
              <strong>Quiet mode is on</strong>, so this site can use <em>any</em> protocol or
              basket without asking — not just the ones listed above. Untick{' '}
              <strong>Quiet mode</strong> below to choose individually.
            </div>
          )}

          <div style={{ fontSize: '14px', color: COLORS.textDark, marginBottom: '12px' }}>
            This site is asking permission to:
          </div>

          {/* ⭐ beta.3 Phase 7b — THE ONLY connect view.
              The read-only summary list that used to sit here, and the separate
              "Customize" screen that repeated it with ticks, were TWO
              REPRESENTATIONS OF ONE CONSENT — and they drifted three separate
              times, always with the summary being the less alarming one (it
              dropped "including ones it did not list above" from quiet mode and
              "across the Metanet" from identity). The drift then reappeared in
              the TOOLTIPS, which Customize never carried at all.
              ⛔ Do not reintroduce a "short version" of this screen. Summarising
              a consent string reliably drops the qualifying clause, because the
              qualifier is the part that creates the alarm.

              Everything starts ticked, so the fast path is still one click. */}
          <div style={{
            background: COLORS.subduedGold,
            borderRadius: '10px',
            padding: '14px 16px',
            marginBottom: '16px',
            maxHeight: '240px',
            overflowY: 'auto',
          }}>
            {manifestData.protocols.map((p, i) => (
              <label key={`proto-${i}`} style={customizeRowLabel}>
                <input
                  type="checkbox"
                  checked={manifestSelectedProtocols.has(i)}
                  disabled={manifestAllowBundledScope}
                  onChange={() => toggleManifestPerm(manifestSelectedProtocols, setManifestSelectedProtocols, i)}
                  style={customizeCheckbox}
                />
                {/* ⛔ P0.8 lesson 6: keep the row's text in ONE span. This row is
                    display:flex, so every child becomes its own flex item and a
                    split label shatters across lines. */}
                <span>
                  {p.purpose || `Use protocol "${p.name}"`}
                  {/* 🚨 beta.3 Phase 7b — the protocol's OWN identifier, always.
                      `protocolID` is machine-readable and enforced; `description`
                      is the SITE'S free text, and nothing ties them together. A
                      hostile manifest can pair a payment-key scope
                      ([2,"3241645161d8"] — BRC-29) with "Show your profile
                      picture." Printing the id means the label can never FULLY
                      lie: the real scope is on screen for a careful reader or a
                      support person.
                      ⛔ Shown at EVERY security level. It previously appeared only
                      in the level-2 counterparty footnote, so level 0/1
                      protocols — the ones granted for ANY counterparty — put no
                      identifier on screen at all. */}
                  <span style={{
                    color: COLORS.textMuted,
                    fontSize: '11px',
                    marginLeft: '6px',
                    fontFamily: 'monospace',
                    wordBreak: 'break-all',
                  }}>
                    [{p.securityLevel}] {p.name}
                  </span>
                </span>
              </label>
            ))}
            {manifestData.baskets.map((b, i) => (
              <label key={`basket-${i}`} style={customizeRowLabel}>
                <input
                  type="checkbox"
                  checked={manifestSelectedBaskets.has(i) && !isProtectedBasket(b.name)}
                  disabled={isProtectedBasket(b.name) || manifestAllowBundledScope}
                  onChange={() => toggleManifestPerm(manifestSelectedBaskets, setManifestSelectedBaskets, i)}
                  style={customizeCheckbox}
                />
                <span>
                  {b.purpose || `${b.access === 'read_write' ? 'Manage' : 'View'} "${b.name}"`}
                  <span style={{ color: COLORS.textMuted, fontSize: '11px', marginLeft: '6px' }}>
                    ({b.access})
                  </span>
                  {isProtectedBasket(b.name) && (
                    <span style={{ color: COLORS.error, fontWeight: 600, fontSize: '11px', marginLeft: '6px' }}>
                      (protected — never auto-granted)
                    </span>
                  )}
                </span>
              </label>
            ))}
            {manifestData.certificates.map((c, i) => (
              <label key={`cert-${i}`} style={customizeRowLabel}>
                <input
                  type="checkbox"
                  checked={manifestSelectedCertificates.has(i)}
                  onChange={() => toggleManifestPerm(manifestSelectedCertificates, setManifestSelectedCertificates, i)}
                  style={customizeCheckbox}
                />
                <span>
                  {c.purpose || `Read ${c.fields.length} certificate field(s)`}
                  {c.fields.length > 0 && (
                    <span style={{ color: COLORS.textMuted, fontSize: '11px', marginLeft: '6px' }}>
                      ({c.fields.join(', ')})
                    </span>
                  )}
                  {/* BRC-116 §4.4 — the user is shown who receives the data. */}
                  {c.verifierPublicKey && (
                    <span style={{ color: COLORS.textMuted, fontSize: '11px', marginLeft: '6px' }}>
                      shared with {c.verifierPublicKey.slice(0, 10)}…
                    </span>
                  )}
                </span>
              </label>
            ))}
            {manifestData.counterparties.map((cp, i) => (
              <label key={`cp-${i}`} style={customizeRowLabel}>
                <input
                  type="checkbox"
                  checked={manifestSelectedCounterparties.has(i)}
                  onChange={() => toggleManifestPerm(manifestSelectedCounterparties, setManifestSelectedCounterparties, i)}
                  style={customizeCheckbox}
                />
                <span>{cp.purpose || cp.type || `${cp.counterparty.slice(0, 12)}…`}</span>
              </label>
            ))}
            {(manifestData.spending.perTransactionUsd > 0
              || (manifestData.spending.monthlySatoshis ?? 0) > 0) && (
              <div style={permissionItem}>
                <span style={checkmark}>&#10003;</span>
                <span>
                  {manifestData.spending.purpose || 'Send payments'}
                  {' '}
                  {/* ⛔ R-CAPS. Whatever the site declares here is a REQUEST,
                      never a setting. BRC-73's figure is monthly satoshis —
                      shown in the unit and period the site actually declared.
                      The limits that will really apply are below, and they are
                      the user's. */}
                  <span style={{ color: COLORS.textMuted, fontSize: '12px' }}>
                    {manifestData.spending.perTransactionUsd > 0
                      ? `(the site asks for up to $${manifestData.spending.perTransactionUsd}/tx, $${manifestData.spending.perSessionUsd}/session)`
                      : `(the site asks for up to ${formatMonthlyAllowance(manifestData.spending)})`}
                  </span>
                </span>
              </div>
            )}
            {/* ⚠️ Unreachable since beta.3 Phase 0.8: a manifest declaring
                nothing no longer parses as a manifest at all, so the
                interceptor falls back to the plain domain_approval modal.
                Kept as a visible failure mode rather than a silent empty box. */}
            {manifestData.protocols.length === 0 &&
              manifestData.baskets.length === 0 &&
              manifestData.certificates.length === 0 &&
              manifestData.counterparties.length === 0 &&
              manifestData.spending.perTransactionUsd === 0 &&
              (manifestData.spending.monthlySatoshis ?? 0) === 0 && (
              <div style={{ color: COLORS.error, fontSize: '13px', fontWeight: 600 }}>
                This site declared no specific permissions. Nothing here is itemised —
                decline unless you know why you are connecting.
              </div>
            )}
          </div>



          {/* 🚨 THE R-PROV BLOCK. This used to be one static line reading
              "Default payment limits: $X/tx" while X came from the SITE's
              manifest — the site's numbers wearing the word "Default". It now
              says whose numbers these are, in words, differently in each case,
              and it EXPANDS IN PLACE rather than throwing the user into the
              Customize subview to see them (contract §6a). */}
          {/* Owner-directed 2026-08-23: no tinted panel and no tinted border
              for the site-suggested state. The amber wash plus gold outline
              turned a legitimate, user-chosen configuration into what looked
              like an error banner. The WORDS still carry the provenance —
              that is what R-PROV requires, and words survive a screenshot,
              greyscale and a colour-blind reader in a way a wash does not. */}
          <div style={{
            border: '1px solid #e5e7eb',
            borderRadius: '10px', marginBottom: '16px', overflow: 'hidden',
          }}>
            <button
              type="button"
              onClick={() => setManifestLimitsOpen((o) => !o)}
              aria-expanded={manifestLimitsOpen}
              style={{
                width: '100%', display: 'flex', alignItems: 'center', gap: '8px',
                background: 'transparent',
                border: 'none', padding: '10px 12px', cursor: 'pointer',
                font: 'inherit', textAlign: 'left', color: COLORS.textDark,
              }}
            >
              <span style={{
                fontSize: '11px', color: COLORS.textMuted, width: '10px',
                transform: manifestLimitsOpen ? 'rotate(90deg)' : 'none',
                transition: 'transform 0.15s',
              }}>&#9654;</span>
              <span style={{ fontSize: '12px', flex: 1, lineHeight: 1.5 }}>
                {anyLimitFromSite ? (
                  <>
                    {/* ⚠️ NOT COLORS.error ('#c62828'). MEASURED 3.00:1 against
                        the dark card — below WCAG AA, on the one line whose
                        job is to say "these are not your defaults". Same red
                        family as the *** mark (6.10:1), so the two read as
                        one signal instead of two different warnings. */}
                    <span style={{ color: '#f87171', fontWeight: 700 }}>
                      Payment limits suggested by this site:
                    </span>{' '}
                    {formatCentsUsd(manifestLimits.perTxCents)}/tx,{' '}
                    {formatCentsUsd(manifestLimits.perSessionCents)}/session.{' '}
                    <strong>These are not your defaults.</strong>
                  </>
                ) : (
                  <>
                    <strong>Your payment limits:</strong>{' '}
                    {formatCentsUsd(manifestLimits.perTxCents)}/tx,{' '}
                    {formatCentsUsd(manifestLimits.perSessionCents)}/session
                    {siteSuggestsAnything && (
                      <>
                        {' '}— this site suggests{' '}
                        {manifestSiteSuggests.perTxCents !== undefined
                          && `${formatCentsUsd(manifestSiteSuggests.perTxCents)}/tx`}
                        {manifestSiteSuggests.perTxCents !== undefined
                          && manifestSiteSuggests.perSessionCents !== undefined && ', '}
                        {manifestSiteSuggests.perSessionCents !== undefined
                          && `${formatCentsUsd(manifestSiteSuggests.perSessionCents)}/session`}
                        , not applied
                      </>
                    )}
                  </>
                )}
              </span>
              <span style={{ fontSize: '11px', color: COLORS.textMuted, whiteSpace: 'nowrap' }}>
                {manifestLimitsOpen ? 'Hide' : 'Adjust'}
              </span>
            </button>
            {manifestLimitsOpen && (
              <div style={{ padding: '0 12px 12px 12px' }}>
                {renderLimitFields()}
                {/* ⭐ "Allow without limits" lives INSIDE this disclosure —
                    owner decision, 2026-09-04. It raises caps to $1000/tx and
                    $10000/session, and it used to sit behind the Customize click,
                    so most users never met it. Deleting Customize would have
                    promoted the widest spending control on the screen to the
                    front of every connect; folding it in here keeps the
                    capability exactly as reachable as it was (one click) without
                    putting it in front of a user who never asked about limits.
                    ⛔ It belongs with the limits it changes, not beside Connect. */}
                <div style={{
                  marginTop: '12px',
                  paddingTop: '12px',
                  borderTop: `1px solid ${COLORS.borderLight}`,
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
              </div>
            )}
          </div>

          {/* Buttons: Decline / Connect. ⛔ There is no "Customize" button any
              more — the ticks are on this screen. See the list block above for
              why a second view was removed rather than reworded. */}
          {grantsLocalAccess && <LocalAccessNotice onShown={markLocalAccessShown} />}
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', flexWrap: 'wrap' }}>
            <HodosButton variant="secondary" onClick={handleManifestDecline}>
              Decline
            </HodosButton>
            <HodosButton variant="primary" onClick={() => handleManifestConnect(false)}>
              Connect
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
            {pageFaviconUrl && !faviconError ? (
              <img
                src={pageFaviconUrl}
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
            {/* ⭐ beta.3 Phase 7b — VERBATIM the connect modal's wording.
                This view kept the pre-P0.8 label after the connect bundle was
                fixed, and the drift was in the same direction every time: the
                shorter string dropped "across the Metanet", i.e. the clause that
                says this key is the SAME on every BRC-100 site and is therefore
                exactly what lets sites correlate the user between them.
                ⛔ Keep these two identical. A consent label that is merely shorter
                is not merely shorter — the qualifier is the alarm. */}
            <span>
              <strong>Identity:</strong> Allow this site to identify you across the Metanet
            </span>
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
            {/* ⭐ beta.3 Phase 7b — VERBATIM the connect modal's wording, label AND
                tooltip. The old label said "perform wallet operations without
                asking each time" and the old tooltip said the wallet "won't
                prompt for individual grants" — neither said the part that
                matters: it covers protocols the site NEVER DECLARED. */}
            <span>
              <strong>Quiet mode:</strong> Let this site use <em>any</em> protocol or basket
              without asking — including ones it did not list above
            </span>
            <InfoIcon tooltip="When ticked, this site can use ANY protocol or basket without prompting - including ones it did not declare in its manifest. Untick it to approve only the specific items listed above. Protected baskets (change outputs, backup tokens) are never included. Sensitive operations - large payments, identity disclosure, sensitive certificate fields - always prompt regardless. Revoke any time from Manage Site Permissions." />
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

          {/* ⛔ P0.9 — MUST sit outside both approve paths. This branch has TWO ways
              to approve: the Allow button below, and DomainPermissionForm's own save
              (handleAllowAdvanced), which REPLACES that button row. Putting the
              notice next to the buttons would hide it on the Advanced path — which
              is how it went missing from this branch entirely on the first pass. */}
          {grantsLocalAccess && <LocalAccessNotice onShown={markLocalAccessShown} />}

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
            {pageFaviconUrl && !faviconError ? (
              <img
                src={pageFaviconUrl}
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

// beta.3 Phase 0.8 — R-PROV styling. A field carrying a site-suggested value
// must be distinguishable from one carrying the user's own default.
//
// ⛔ Colour alone is STILL not the mechanism. It is now a red asterisk glyph
// plus a legend line, which is why the rule survives the 2026-08-23 restyle:
// an asterisk is a character, not a colour, so it survives greyscale, a
// high-contrast theme, a screenshot and a colour-blind reader exactly as the
// old "suggested by site" pill did. The colour is reinforcement, not signal.
// Each marked control also carries a `title` with the words, so nothing that
// reads the accessibility tree loses the provenance either.
//
// 🚨 The pill it replaced was not merely loud — it was masking a defect. It
// set `background: '#fff8f0'` on the INPUT while leaving `color` at the dark
// theme's '#f0f0f0': near-white text on a near-white box, contrast ≈ 1.07:1.
// The number a user was about to approve as their spending cap was invisible
// exactly when the site had chosen it. The pill stayed dark-on-light and
// perfectly legible, so the screen shouted about a value it was hiding.
// ⛔ Never re-introduce a background override here without setting `color`
// with it — this input inherits a DARK theme (`COLORS.white` is '#1a1d23').
const siteSuggestedMark: React.CSSProperties = {
  color: '#f87171',
  fontWeight: 800,
  fontSize: '15px',
  letterSpacing: '1px',
  marginLeft: '4px',
  lineHeight: 1,
};

const siteSuggestionHint: React.CSSProperties = {
  fontSize: '10px',
  color: '#9ca3af',
  fontStyle: 'italic',
};

const useMyDefaultsButton: React.CSSProperties = {
  background: 'none',
  border: 'none',
  padding: 0,
  font: 'inherit',
  fontSize: '12px',
  fontWeight: 600,
  color: '#1a73e8',
  textDecoration: 'underline',
  cursor: 'pointer',
};

/** Label wrapper for a limit field; tinted when the value came from the site. */
const limitFieldLabel = (source: 'user' | 'site'): React.CSSProperties => ({
  display: 'flex',
  flexDirection: 'column',
  gap: '4px',
  fontSize: '12px',
  // Owner-directed 2026-08-23: one colour in both states. Recolouring the
  // label (dark brown originally, then amber) made a normal, user-chosen
  // state look like a fault. The `***` is the differentiator.
  color: '#9ca3af',
  fontWeight: source === 'site' ? 600 : 400,
});

/** Input styling for a limit field.
 *
 *  Owner-directed 2026-08-23: identical in both provenance states. The red
 *  outline read as an error, not as information — the screen looked like it
 *  was scolding the user for a state they had deliberately chosen. The `***`
 *  beside the label carries the mark instead.
 *
 *  ⛔ If you ever restore a `background` override here you MUST set `color`
 *  with it. This input inherits a DARK theme (`COLORS.white` is '#1a1d23');
 *  the version that set only `background` shipped a 1.07:1 spending cap. */
const limitInputStyle = (_source: 'user' | 'site'): React.CSSProperties => ({
  ...customizeNumberInput,
});

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
