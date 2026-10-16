// WaitingPaymentsLine — B5-T1-P7b (owner, live sitting 2026-10-12: rename + info icon + dismiss).
//
// Incoming PeerPay payments the wallet has stored but not credited yet (`peerpay_held_messages`;
// counts from `/wallet/peerpay/status`). Shared by the wallet panel and the full wallet dashboard.
// Dismiss hides the line until one more payment starts waiting. The overdue version ("not on chain
// after an hour") is a fault alarm and cannot be dismissed.
//
// ⚠️ CEF overlay input rules (root CLAUDE.md): native <button> only.

import React, { useEffect, useState } from 'react';

const INFO: string[] = [
  'Someone sent you a payment and your wallet has saved it, but has not added it to your balance yet.',
  'Usually this lasts a few seconds, while the wallet checks the payment on the blockchain. A payment can wait longer if the network refuses it for now, if it is locked until a later time, or if the amount does not match what the sender said.',
  'Nothing is lost while it waits: the wallet keeps re-checking and adds the payment to your balance as soon as it is confirmed. Dismissing this line does not affect the payment.',
];

const plural = (n: number) => `${n} incoming payment${n === 1 ? '' : 's'}`;

const WaitingPaymentsLine: React.FC<{ count: number; overdueCount: number; className: string; textClassName?: string }> = ({
  count, overdueCount, className, textClassName,
}) => {
  // Number of waiting payments when the user dismissed the line; it shows again only above this.
  const [dismissedAt, setDismissedAt] = useState(0);
  const [infoHover, setInfoHover] = useState(false);
  const [infoPinned, setInfoPinned] = useState(false);

  // A payment credited (count fell) lowers the mark, so the next new one shows the line again.
  useEffect(() => {
    if (count < dismissedAt) setDismissedAt(count);
  }, [count, dismissedAt]);

  const overdue = overdueCount > 0;
  if (count <= 0 || (!overdue && count <= dismissedAt)) return null;

  return (
    <div className={className} style={{ flexDirection: 'column', alignItems: 'stretch' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
        <span className={textClassName} style={{ flex: 1 }}>
          {overdue
            ? `${plural(overdueCount)} not on chain after an hour — something is wrong`
            : `${plural(count)} waiting — not yet credited`}
        </span>
        <button
          type="button"
          aria-label="What does this mean?"
          onMouseEnter={() => setInfoHover(true)}
          onMouseLeave={() => setInfoHover(false)}
          onClick={() => setInfoPinned(!infoPinned)}
          style={{ background: 'none', border: '1px solid #b9a36a', borderRadius: '50%', width: 16, height: 16,
                   padding: 0, lineHeight: '14px', fontSize: 10, cursor: 'pointer', color: '#8a6d1d', flexShrink: 0 }}
        >
          i
        </button>
        {!overdue && (
          <button
            type="button"
            aria-label="Dismiss"
            onClick={() => { setDismissedAt(count); setInfoPinned(false); }}
            style={{ background: 'none', border: 'none', padding: '0 2px', fontSize: 14, lineHeight: 1,
                     cursor: 'pointer', color: '#8a6d1d', flexShrink: 0 }}
          >
            ×
          </button>
        )}
      </div>
      {(infoHover || infoPinned) && (
        <div style={{ marginTop: 6, padding: 8, background: '#fffdf5', border: '1px solid #eadfbf', borderRadius: 4,
                      color: '#3c4043', fontSize: 12, lineHeight: 1.4 }}>
          {INFO.map((p) => <div key={p} style={{ marginBottom: 4 }}>{p}</div>)}
          {overdue && (
            <div style={{ marginBottom: 4 }}>
              A payment still waiting after an hour is not normal. It stays saved and is not lost — please report it.
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default WaitingPaymentsLine;
