/**
 * Parse a BIP21 bitcoin: URI into its components.
 *
 * Examples:
 *   "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa" → { address: "1A1z..." }
 *   "bitcoin:1A1z...?amount=0.001&label=Test"     → { address: "1A1z...", amount: 0.001, label: "Test" }
 */
export function parseBIP21(uri: string): { address: string; amount?: number; label?: string } | null {
  if (!uri.toLowerCase().startsWith('bitcoin:')) return null;
  const rest = uri.slice(8); // remove "bitcoin:"
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
