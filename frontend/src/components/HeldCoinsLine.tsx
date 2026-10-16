// HeldCoinsLine — B5-T1-P5 (owner decisions 2(d) + 7 ④, wording §12a Q1).
//
// The balance above this line is MONEY: coins the wallet has proven are plain
// payments to its own keys. Anything else it holds (a 1-sat coin, a token, a coin
// whose script has not been read from the chain yet) is listed here, so no coin
// disappears from view. "Treat as money" is the user's explicit, confirmed override
// for the one case the wallet cannot prove on its own (a coin of more than 1 satoshi
// whose script has not been read yet — B5-T2-P1b); the wallet refuses it for anything
// else (1-sat coins, tokens, non-payment scripts, keys that are not ours).
//
// ⚠️ CEF overlay input rules (root CLAUDE.md): native <button> only, no MUI inputs,
// no window.confirm (JS dialogs in overlays go through CefJSDialogHandler) — the
// confirmation is inline.

import React, { useCallback, useEffect, useState } from 'react';
import { walletFetch } from '../services/walletApi';

interface HeldOutput {
  txid: string | null;
  vout: number;
  satoshis: number;
  reason: string;
  canTreatAsMoney: boolean;
}

const REASONS: Record<string, string> = {
  not_observed: 'Waiting for the blockchain to confirm what this coin is',
  not_plain_payment: 'Not a plain payment (token, inscription or other script)',
  one_sat: '1-satoshi coin — usually an NFT or token',
  not_ours: 'Pays a key this wallet does not use',
  wallet_locked: 'Wallet locked — checked after unlock',
  derivation_failed: 'Ownership could not be checked',
  // `list_held` also reports a recognised token waiting to be filed as pending, so this
  // must not promise the balance (B5-T2-P1b, /code-review 176 #8).
  pending: 'Being checked — resolves on its own shortly',
  token_shaped: 'Looks like a token this wallet can’t show yet',
  unreadable: 'Its script could not be read',
};

// B5-T2-P1b: one line for a row without the button, saying why. Temporary states
// (being checked, locked, ownership not checked yet) need none — they resolve on their own.
const whyNoButton = (o: HeldOutput): string | null => {
  if (o.canTreatAsMoney || ['pending', 'wallet_locked', 'derivation_failed'].includes(o.reason)) return null;
  if (o.satoshis <= 1) {
    return 'Never added to your balance: 1-satoshi coins are usually NFTs or tokens, and spending one as money destroys it.';
  }
  if (o.reason === 'not_ours') {
    return 'This wallet can’t spend it, so it can’t join your balance.';
  }
  if (o.reason === 'not_observed') {
    // Withheld by the server's check of the stored script: not a plain payment to this
    // wallet, or ownership not checkable right now (locked).
    return 'Can’t be treated as money yet: the wallet could not confirm it can spend this coin.';
  }
  return 'Kept out of your balance so it is never spent by mistake.';
};

const POLL_MS = 30_000;

// Owner-approved explanation (2026-10-06), shown on hover / click of the ⓘ. The last
// paragraph names "Treat as money" only when a row in the list offers it (B5-T2-P1b).
const INFO: string[] = [
  'Your wallet received these coins but can’t tell yet whether each one is ordinary money or a token (a token is often a 1-satoshi coin; spending it as money destroys it).',
  'They are not in your balance and are never spent automatically. The wallet re-checks them every 30 seconds; most resolve on their own once the blockchain answers.',
];
const INFO_OFFERED = 'If you know a coin is ordinary money, choose Treat as money and it joins your balance. If unsure, leave it — nothing is lost by waiting.';
const INFO_NOT_OFFERED = 'Nothing is lost by leaving them. Each coin in the list says why it is held.';

// Opens in a new browser tab (the overlay pattern `WalletPanel.tsx :: handleManageSites` uses).
const openTab = (url: string) => {
  if (window.cefMessage) window.cefMessage.send('tab_create', url);
  else window.open(url, '_blank', 'noopener');
};

const same = (a: HeldOutput | null, b: HeldOutput) => !!a && a.txid === b.txid && a.vout === b.vout;

const shortOutpoint = (o: HeldOutput) =>
  `${(o.txid ?? '?').slice(0, 10)}…:${o.vout}`;

const HeldCoinsLine: React.FC<{ bsvPrice: number; onChanged?: () => void }> = ({ bsvPrice, onChanged }) => {
  const [held, setHeld] = useState<HeldOutput[] | null>(null);
  const [open, setOpen] = useState(false);
  const [confirming, setConfirming] = useState<HeldOutput | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [infoHover, setInfoHover] = useState(false);
  const [infoPinned, setInfoPinned] = useState(false);

  const load = useCallback(async () => {
    try {
      const res = await walletFetch('/wallet/held-outputs');
      if (!res.ok) return; // keep the last list; never show "0 held" we did not measure
      const data = await res.json();
      const list: HeldOutput[] = Array.isArray(data.held) ? data.held : [];
      setHeld(list);
      // A poll that withdraws the button also drops an open confirm, so it never comes
      // back without a fresh click (/code-review 176 #9).
      setConfirming((c) => (c && list.some((o) => same(c, o) && o.canTreatAsMoney) ? c : null));
    } catch {
      /* wallet unreachable — the balance block above already says so */
    }
  }, []);

  useEffect(() => {
    load();
    const t = setInterval(load, POLL_MS);
    return () => clearInterval(t);
  }, [load]);

  const treatAsMoney = async (o: HeldOutput) => {
    setMessage(null);
    try {
      const res = await walletFetch('/wallet/held-outputs/treat-as-money', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ txid: o.txid, vout: o.vout, confirm: true }),
      });
      const data = await res.json().catch(() => ({}));
      if (res.ok && data.success) {
        setMessage(`${o.satoshis.toLocaleString()} sats added to your balance.`);
        onChanged?.();
      } else {
        setMessage(`Not changed: ${data.code ?? res.status}${data.detail ? ` (${data.detail})` : ''}.`);
      }
    } catch (e) {
      setMessage(`Not changed: ${e instanceof Error ? e.message : 'wallet unreachable'}.`);
    } finally {
      setConfirming(null);
      load();
    }
  };

  if (!held || held.length === 0) return null;

  const totalSats = held.reduce((s, o) => s + o.satoshis, 0);
  const usd = bsvPrice > 0 ? (totalSats / 1e8) * bsvPrice : 0;
  const n = held.length;
  const info = [...INFO, held.some((o) => o.canTreatAsMoney) ? INFO_OFFERED : INFO_NOT_OFFERED];

  return (
    <div style={{ marginTop: 8, fontSize: 12, color: '#5f6368' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
        <button
          type="button"
          onClick={() => setOpen(!open)}
          style={{ background: 'none', border: 'none', padding: 0, cursor: 'pointer', color: '#8a6d1d', fontSize: 12 }}
        >
          {n} {n === 1 ? 'item' : 'items'} held · {totalSats.toLocaleString()} sats
          {usd >= 0.01 ? ` ($${usd.toFixed(2)})` : ''}{' '}
          <span style={{ textDecoration: 'underline' }}>{open ? 'Hide ▾' : 'Show ▸'}</span>
        </button>
        <button
          type="button"
          aria-label="What are held coins?"
          onMouseEnter={() => setInfoHover(true)}
          onMouseLeave={() => setInfoHover(false)}
          onClick={() => setInfoPinned(!infoPinned)}
          style={{ background: 'none', border: '1px solid #b9a36a', borderRadius: '50%', width: 16, height: 16,
                   padding: 0, lineHeight: '14px', fontSize: 10, cursor: 'pointer', color: '#8a6d1d' }}
        >
          i
        </button>
      </div>
      {(infoHover || infoPinned) && (
        <div style={{ marginTop: 6, padding: 8, background: '#fffdf5', border: '1px solid #eadfbf', borderRadius: 4,
                      color: '#3c4043', lineHeight: 1.4 }}>
          <div style={{ fontWeight: 600, marginBottom: 4 }}>Coins we couldn’t identify</div>
          {info.map((p) => <div key={p} style={{ marginBottom: 4 }}>{p}</div>)}
        </div>
      )}
      {open && (
        <div style={{ marginTop: 6, maxHeight: 180, overflowY: 'auto' }}>
          <div style={{ marginBottom: 4 }}>
            Not counted in your balance and never spent automatically.
          </div>
          {held.map((o) => {
            const why = whyNoButton(o);
            return (
            <div key={`${o.txid}:${o.vout}`} style={{ padding: '4px 0', borderTop: '1px solid #eee' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8 }}>
                <span style={{ fontFamily: 'monospace' }}>{shortOutpoint(o)}</span>
                <span>{o.satoshis.toLocaleString()} sats</span>
              </div>
              <div>{REASONS[o.reason] ?? o.reason}</div>
              {why && <div style={{ color: '#80868b' }}>{why}</div>}
              {o.txid && (
                <button type="button" onClick={() => openTab(`https://whatsonchain.com/tx/${o.txid}`)}
                        style={{ background: 'none', border: 'none', padding: 0, marginTop: 2, fontSize: 11,
                                 cursor: 'pointer', color: '#1a73e8' }}>
                  View on WhatsOnChain
                </button>
              )}
              {o.canTreatAsMoney && !same(confirming, o) && (
                <button type="button" onClick={() => setConfirming(o)}
                        style={{ marginTop: 2, fontSize: 11, cursor: 'pointer' }}>
                  Treat as money
                </button>
              )}
              {o.canTreatAsMoney && same(confirming, o) && (
                <div style={{ marginTop: 4, padding: 6, background: '#fff8e1', borderRadius: 4 }}>
                  <div>
                    Treat {o.satoshis.toLocaleString()} sats ({shortOutpoint(o)}) as money? Only do this
                    if you know this coin is ordinary money — if it is a token, spending it destroys it.
                  </div>
                  <button type="button" onClick={() => treatAsMoney(o)} style={{ marginRight: 6, marginTop: 4, cursor: 'pointer' }}>
                    Yes, treat as money
                  </button>
                  <button type="button" onClick={() => setConfirming(null)} style={{ marginTop: 4, cursor: 'pointer' }}>
                    Cancel
                  </button>
                </div>
              )}
            </div>
            );
          })}
          {message && <div style={{ marginTop: 4 }}>{message}</div>}
        </div>
      )}
    </div>
  );
};

export default HeldCoinsLine;
