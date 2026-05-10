import React from 'react';
import { useSearchParams } from 'react-router-dom';
import { colors, fonts } from '../styles/hodosTheme';

// Phase 1 BRC-121 polish — background placeholder shown while the user decides
// on a domain_approval modal for a paywalled article. Replaces the CEF
// "Failed to load" page that would otherwise render behind the modal.
//
// Positioned top-left so the Hodos approval modal (centered) can sit cleanly
// over the rest of the screen. Most users won't even notice this page —
// it's the calm background while the modal does the talking.
const PaymentPendingPage: React.FC = () => {
  const [searchParams] = useSearchParams();
  const domain = searchParams.get('domain') || '';
  const sats = searchParams.get('sats') || '';

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        background: colors.bgPrimary,
        color: colors.textPrimary,
        fontFamily: fonts.sans,
      }}
    >
      <style>{`
        @keyframes hodos-pending-spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
      `}</style>
      <div
        style={{
          position: 'absolute',
          top: 16,
          left: 20,
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          opacity: 0.55,
        }}
      >
        <img
          src="/Hodos_Gold_Icon.svg"
          alt=""
          width={28}
          height={28}
          style={{
            animation: 'hodos-pending-spin 3s linear infinite',
            display: 'block',
          }}
        />
        <div style={{ lineHeight: 1.2 }}>
          <div style={{ fontSize: 13, color: colors.textPrimary }}>
            Waiting for your approval
          </div>
          {domain && (
            <div style={{ fontSize: 11, color: colors.textSecondary, marginTop: 2 }}>
              {domain}
              {sats ? ` · ${sats} sats` : ''}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default PaymentPendingPage;
