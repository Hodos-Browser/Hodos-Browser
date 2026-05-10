import React from 'react';
import { useSearchParams } from 'react-router-dom';
import { colors, fonts } from '../styles/hodosTheme';

// Phase 1 BRC-121 polish — shown when Async402ResourceHandler exhausts its
// auto-retries (typically due to Cloudflare 431 "Request Header Fields Too
// Large" against the BEEF base64 retry header). The user's nosend tx was
// never broadcast — funds preserved — but we couldn't deliver the bytes
// the user paid (locally) for.
//
// Layout: clean centered card, Hodos icon, plain message, single Retry
// button that re-navigates to the original URL. Unlike the placeholder this
// is a real page (no modal sits over it) so it gets full screen real estate.
const PaymentFailedPage: React.FC = () => {
  const [searchParams] = useSearchParams();
  const domain = searchParams.get('domain') || 'this site';
  const sats = searchParams.get('sats') || '';
  const originalUrl = searchParams.get('originalUrl') || '';
  const status = searchParams.get('status') || '';

  const handleRetry = () => {
    if (originalUrl && window.cefMessage) {
      // Re-navigate to the original URL via the standard navigation IPC.
      window.cefMessage.send('navigate', originalUrl);
    }
  };

  const handleGoBack = () => {
    if (window.cefMessage) {
      window.cefMessage.send('navigate_back', []);
    }
  };

  return (
    <div
      style={{
        minHeight: '100vh',
        background: colors.bgPrimary,
        color: colors.textPrimary,
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'center',
        padding: '28vh 24px 24px',
        fontFamily: fonts.sans,
      }}
    >
      <div
        style={{
          maxWidth: 480,
          width: '100%',
          background: colors.bgSurface,
          border: `1px solid ${colors.borderDefault}`,
          borderRadius: 12,
          padding: '24px 28px 32px',
          textAlign: 'center',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            paddingBottom: 14,
            marginBottom: 20,
            borderBottom: `1px solid ${colors.borderDefault}`,
          }}
        >
          <img
            src="/Hodos_Gold_Wallet_Icon.svg"
            alt="Hodos Wallet"
            height={36}
            style={{ display: 'block', flexShrink: 0, width: 'auto' }}
          />
        </div>
        <h2 style={{ margin: '0 0 8px 0', fontSize: 20, color: colors.textPrimary }}>
          {domain} rejected the payment
        </h2>
        <p
          style={{
            margin: '0 0 20px 0',
            fontSize: 14,
            color: colors.textDim,
            lineHeight: 1.5,
          }}
        >
          Your sats are safe — the transaction was{' '}
          <strong style={{ color: colors.goldBright }}>not broadcast</strong> because
          the site refused our payment headers
          {status && ` (HTTP ${status})`}.
          {sats && (
            <>
              <br />
              The site asked for {sats} sats. You can try again — the same
              site sometimes accepts a second attempt.
            </>
          )}
        </p>
        <div
          style={{
            display: 'flex',
            gap: 10,
            justifyContent: 'center',
            flexWrap: 'wrap',
          }}
        >
          <button
            onClick={handleRetry}
            disabled={!originalUrl}
            style={{
              background: colors.goldBright,
              color: colors.bgPrimary,
              border: 'none',
              padding: '10px 24px',
              borderRadius: 6,
              fontSize: 14,
              fontWeight: 600,
              cursor: originalUrl ? 'pointer' : 'not-allowed',
              opacity: originalUrl ? 1 : 0.5,
            }}
          >
            Try Again
          </button>
          <button
            onClick={handleGoBack}
            style={{
              background: 'transparent',
              color: colors.textSecondary,
              border: `1px solid ${colors.borderInput}`,
              padding: '10px 24px',
              borderRadius: 6,
              fontSize: 14,
              cursor: 'pointer',
            }}
          >
            Go Back
          </button>
        </div>
      </div>
    </div>
  );
};

export default PaymentFailedPage;
