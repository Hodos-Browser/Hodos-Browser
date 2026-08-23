/**
 * manifestConsent — deciding whose numbers the connect modal shows.
 *
 * beta.3 Phase 0.8. Pure, no React, no DOM: the connect modal is a consent
 * surface, and the rule about whose values it carries is the security-relevant
 * part, so it lives here where it can be unit-tested against the real source
 * (`development-docs/0.4.0-beta.3/phase-0.8-manifest-shape/probes/manifest_consent_t1f.mjs`,
 * wired into `scripts/preflight.ps1` as T1f).
 *
 * ## The rule (contract §6a, `R-PROV`)
 *
 * The modal carries two different kinds of content and must keep them apart:
 *
 * | Kind                                            | Whose it is  |
 * |-------------------------------------------------|--------------|
 * | protocols / baskets / certificates it asks for   | the site's   |
 * | per-tx, per-session, rate/min, max-tx/session    | **the user's** |
 *
 * ⛔ **A manifest-populated field must never be indistinguishable from the
 * user's own default.** That is a security requirement, not styling: an
 * undifferentiated auto-populated modal lets a site change what the user
 * approves without the user knowing — *worse* than the defect this phase fixes,
 * because today the modal shows nothing rather than the site's numbers dressed
 * as the user's. Every result below therefore carries `sourceOf`, and the modal
 * marks any field whose source is `'site'`.
 *
 * ## Which declared numbers are even eligible
 *
 * 🚨 Only our own legacy shape's `perTransactionUsd` / `perSessionUsd`. They
 * share a unit AND a period with `domain_permissions.per_tx_limit_cents` /
 * `per_session_limit_cents`, so they are convertible.
 *
 * BRC-73's `spendingAuthorization.amount` is **monthly satoshis**: a different
 * unit and a different period from anything we enforce, and we have no monthly
 * concept at all. It is displayed as the site's request and **never** pre-fills
 * anything, in either toggle state (`R-CAPS`, `P0.8-A5`).
 *
 * Nothing here can raise a cap on its own — the values it returns are what the
 * modal *shows*; the user still has to press Connect.
 */

/** The four numeric controls on a `domain_permissions` row. */
export interface ConnectLimits {
  perTxCents: number;
  perSessionCents: number;
  rateLimitPerMin: number;
  maxTxPerSession: number;
}

/** Which of `ConnectLimits` a value came from. */
export type LimitField = keyof ConnectLimits;

/** `'user'` = the user's own default. `'site'` = the site suggested it. */
export type LimitSource = 'user' | 'site';

export type LimitSources = Record<LimitField, LimitSource>;

/**
 * The site's suggestions, already converted into our units where that is
 * possible at all. A field is absent when the site suggested nothing we can
 * express as one of our caps.
 */
export interface SiteSuggestedLimits {
  perTxCents?: number;
  perSessionCents?: number;
}

/** The spending block as it arrives from the C++ modal payload. */
export interface ManifestSpendingPayload {
  /** Our legacy shape only. Whole USD. */
  perTransactionUsd?: number;
  /** Our legacy shape only. Whole USD. */
  perSessionUsd?: number;
  /** BRC-73 `spendingAuthorization.amount` — monthly satoshis. Display only. */
  monthlySatoshis?: number;
  purpose?: string;
}

export interface ResolvedConnectLimits {
  /** What the modal's four inputs start at. */
  values: ConnectLimits;
  /** Whose number each one is. Drives the visible marking. */
  sourceOf: LimitSources;
  /** What the site suggested, shown beside the field whatever the toggle says. */
  siteSuggests: SiteSuggestedLimits;
}

const ALL_FIELDS: LimitField[] = [
  'perTxCents',
  'perSessionCents',
  'rateLimitPerMin',
  'maxTxPerSession',
];

/** Every field marked as the user's. */
function allUserSources(): LimitSources {
  return {
    perTxCents: 'user',
    perSessionCents: 'user',
    rateLimitPerMin: 'user',
    maxTxPerSession: 'user',
  };
}

/**
 * A site-declared figure is usable only if it is a finite, positive, sane
 * number. A manifest is attacker-controlled input: `NaN`, `Infinity`,
 * negatives and absurd magnitudes are all reachable from valid JSON, and any
 * of them reaching an input box is a bug even before it reaches a cap.
 */
function usableUsd(raw: unknown): number | undefined {
  if (typeof raw !== 'number') return undefined;
  if (!Number.isFinite(raw)) return undefined;
  if (raw <= 0) return undefined;
  // $1,000,000 is far past anything a connect prompt should render, and well
  // inside the range where cents arithmetic is still exact.
  if (raw > 1_000_000) return undefined;
  return Math.round(raw * 100);
}

/**
 * What the site suggested, in our units. ⛔ `monthlySatoshis` is deliberately
 * not consulted — see the module docs.
 */
export function siteSuggestionsFrom(
  spending: ManifestSpendingPayload | null | undefined,
): SiteSuggestedLimits {
  if (!spending) return {};
  const out: SiteSuggestedLimits = {};
  const perTx = usableUsd(spending.perTransactionUsd);
  if (perTx !== undefined) out.perTxCents = perTx;
  const perSession = usableUsd(spending.perSessionUsd);
  if (perSession !== undefined) out.perSessionCents = perSession;
  return out;
}

/**
 * Decide what the connect modal's four limit fields start at, and whose numbers
 * they are.
 *
 * - `prefillFromManifest === false` (the shipped default, behaviour **(b)**):
 *   every value is the user's; the site's suggestion is returned separately so
 *   the modal can show it *beside* the field as information. Adopting it is an
 *   affirmative act.
 * - `prefillFromManifest === true` (behaviour **(a)**, opt-in from
 *   "Default Limits for New Sites"): a field the site suggested starts at the
 *   site's value and is marked `'site'`. Fields the site said nothing about
 *   stay the user's.
 *
 * ⛔ The toggle changes WHICH values are pre-filled and nothing else. Marking
 * is driven by `sourceOf`, which is computed here, so turning the toggle on can
 * never suppress it (`P0.8-A12`).
 */
export function resolveConnectLimits(input: {
  userDefaults: ConnectLimits;
  spending?: ManifestSpendingPayload | null;
  prefillFromManifest: boolean;
}): ResolvedConnectLimits {
  const { userDefaults, spending, prefillFromManifest } = input;
  const siteSuggests = siteSuggestionsFrom(spending);

  const values: ConnectLimits = { ...userDefaults };
  const sourceOf = allUserSources();

  if (prefillFromManifest) {
    if (siteSuggests.perTxCents !== undefined) {
      values.perTxCents = siteSuggests.perTxCents;
      sourceOf.perTxCents = 'site';
    }
    if (siteSuggests.perSessionCents !== undefined) {
      values.perSessionCents = siteSuggests.perSessionCents;
      sourceOf.perSessionCents = 'site';
    }
  }
  // rateLimitPerMin and maxTxPerSession have no counterpart in any manifest
  // shape — neither ours nor BRC-73 — so they are always the user's.

  return { values, sourceOf, siteSuggests };
}

/**
 * The "Use my defaults" control. Reverts every field to the user's own value
 * and clears every `'site'` mark.
 *
 * ⛔ Must revert ALL four, not only the marked ones: a user who has been
 * editing fields expects one button to put the whole block back.
 */
export function useMyDefaults(userDefaults: ConnectLimits): {
  values: ConnectLimits;
  sourceOf: LimitSources;
} {
  return { values: { ...userDefaults }, sourceOf: allUserSources() };
}

/**
 * The "Use this site's suggested limits" control — the forward direction, and
 * the mirror of {@link useMyDefaults}.
 *
 * Contract §6a defines the shipped behaviour (b) as the user's defaults plus
 * *"an explicit 'Use the site's recommended settings' button"*. Most users, on
 * a site whose recommendations are what make it work, will want exactly this;
 * without it they had to retype the site's numbers by hand.
 *
 * ⛔ Adopting is an affirmative act and the fields stay MARKED (`sourceOf`
 * flips to `'site'`), so `R-PROV` still holds and "Use my defaults" still
 * reverts. Only figures already in our unit and period can be adopted at all —
 * BRC-73's monthly satoshis never reach `SiteSuggestedLimits`, so this can
 * never adopt them (`R-CAPS`). Fields the site said nothing about are untouched.
 */
export function useSiteSuggested(
  current: ConnectLimits,
  siteSuggests: SiteSuggestedLimits,
): { values: ConnectLimits; sourceOf: LimitSources } {
  const values: ConnectLimits = { ...current };
  const sourceOf = allUserSources();
  if (siteSuggests.perTxCents !== undefined) {
    values.perTxCents = siteSuggests.perTxCents;
    sourceOf.perTxCents = 'site';
  }
  if (siteSuggests.perSessionCents !== undefined) {
    values.perSessionCents = siteSuggests.perSessionCents;
    sourceOf.perSessionCents = 'site';
  }
  return { values, sourceOf };
}

/** True if any field currently carries a site-supplied value. */
export function hasSiteSourcedLimit(sourceOf: LimitSources): boolean {
  return ALL_FIELDS.some((f) => sourceOf[f] === 'site');
}

/**
 * Should the modal's collapsible limit section start EXPANDED?
 *
 * Owner requirement (contract §6a): *"If we auto-populate fields with the
 * site's suggested settings, the collapsible sections must be expanded on
 * popup — a user must not approve values hidden behind a collapsed section."*
 *
 * Also expands when the site merely *suggested* something we chose not to
 * pre-fill: the suggestion is rendered beside the field, and a suggestion the
 * user never sees is no better than one they cannot distinguish.
 */
export function shouldExpandLimits(
  sourceOf: LimitSources,
  siteSuggests: SiteSuggestedLimits,
): boolean {
  if (hasSiteSourcedLimit(sourceOf)) return true;
  return siteSuggests.perTxCents !== undefined || siteSuggests.perSessionCents !== undefined;
}

/**
 * Text-entry rules for the four limit boxes.
 *
 * 🚨 These exist because the boxes were once numeric controlled inputs whose
 * value was re-derived (`(cents/100).toFixed(2)`) on every keystroke. Typing
 * "25" went 2 → "2.00" → 2.005 → "2.01"; only the spinner arrows worked, and an
 * integer box could not be cleared because `parseInt("")` is NaN. Reported from
 * live testing 2026-08-23.
 *
 * ⭐ The property that matters is **every prefix of a valid entry is
 * accepted** — you cannot type "25.00" if "2", "25", "25." are rejected on the
 * way. That is what the T1f harness asserts, character by character.
 */

/** True if `raw` is a partial-or-complete dollar entry the box should accept. */
export function isEditableUsdText(raw: string): boolean {
  return raw === '' || /^\d*\.?\d{0,2}$/.test(raw);
}

/** True if `raw` is a partial-or-complete whole-number entry. */
export function isEditableIntText(raw: string): boolean {
  return raw === '' || /^\d*$/.test(raw);
}

/** Cents for a (possibly partial) dollar entry. `""`, `"."` and `"2."` → best effort. */
export function usdTextToCents(raw: string): number {
  const n = parseFloat(raw || '0');
  return Number.isFinite(n) ? Math.round(n * 100) : 0;
}

/** Whole number for a (possibly partial) integer entry. */
export function intTextToNumber(raw: string): number {
  const n = parseInt(raw || '0', 10);
  return Number.isFinite(n) ? n : 0;
}

/** `$1.00` from `100`. */
export function formatCentsUsd(cents: number): string {
  return '$' + (cents / 100).toFixed(2);
}

/**
 * Render BRC-73's declared monthly allowance for display. Returns `null` when
 * the site declared none.
 *
 * ⛔ Rendered as satoshis-per-month, the unit and period the site actually
 * declared — never converted into one of our caps. BRC-116 §4.2: *"Wallets MUST
 * compute and display authoritative satoshi amounts from structured numeric
 * fields … rather than relying on free-form description text."* So this reads
 * the number, never the site's prose.
 */
export function formatMonthlyAllowance(
  spending: ManifestSpendingPayload | null | undefined,
): string | null {
  const sats = spending?.monthlySatoshis;
  if (typeof sats !== 'number' || !Number.isFinite(sats) || sats <= 0) return null;
  return `${Math.floor(sats).toLocaleString('en-US')} satoshis / month`;
}
