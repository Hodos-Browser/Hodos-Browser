/**
 * Parse a BIP21 payment URI into its components.
 *
 * Accepts an ALLOWLIST of payment schemes: `bitcoin:` and `bsv:`. `bsv:` is the
 * better scheme for this chain — `bitcoin:` is claimed by BTC wallets, so a
 * `bitcoin:` URI carrying a BSV address is the ambiguous one. The scheme is a
 * meaningful signal of intent-to-pay (the money path), so it stays an allowlist;
 * do NOT widen to "any scheme". See TICKET_qr_bsv_uri_scheme_rejected.md.
 *
 * Examples:
 *   "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa" → { address: "1A1z..." }
 *   "bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=0.11828417" → { address: "16ce...", amount: 0.11828417 }
 */
export function parseBIP21(uri: string): { address: string; amount?: number; label?: string } | null {
  const lower = uri.toLowerCase();
  if (!lower.startsWith('bitcoin:') && !lower.startsWith('bsv:')) return null;
  // Strip the scheme by the FIRST ':', never a fixed offset — "bitcoin:" (8) and
  // "bsv:" (4) differ in length, so slice(8) would truncate a bsv: address and
  // fail closed. See TICKET §The trap.
  const rest = uri.slice(uri.indexOf(':') + 1);
  const qIdx = rest.indexOf('?');
  const address = qIdx >= 0 ? rest.slice(0, qIdx) : rest;
  if (!address) return null;

  const params = new URLSearchParams(qIdx >= 0 ? rest.slice(qIdx + 1) : '');
  const amountStr = params.get('amount');

  return {
    address,
    amount: amountStr ? parseFloat(amountStr) : undefined,
    label: params.get('label') || undefined,
  };
}
