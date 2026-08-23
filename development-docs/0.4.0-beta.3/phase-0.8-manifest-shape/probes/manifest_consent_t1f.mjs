// P0.8 T1f — real-source unit harness for the connect-modal consent rule.
//
// Exercises the ACTUAL shipped source, not a re-implementation:
//   frontend/src/utils/manifestConsent.ts  — imported via Node type-stripping
//
// SUBJECT: asserts on WHOSE NUMBER lands in each limit field (`sourceOf`) and on
// the value itself — never on "the function returned something". A modal that
// silently adopts a site's cap and a modal that shows the user's own default
// are both "a modal"; the whole point of P0.8 is that they must be
// distinguishable, so the assertions are about the distinction.
//
// Covers, from PHASE_CONTRACT.md §8:
//   P0.8-A5   BRC-73 spendingAuthorization (monthly satoshis) never becomes a cap
//   P0.8-A10  "Use my defaults" reverts every suggested field
//   P0.8-A12  the pre-fill toggle changes WHICH values are pre-filled, and
//             nothing else — marking survives in both states
//
// Modes:
//   (default)          GREEN — the rules above hold.
//   --negative-control RED   — rewrites the real source so BRC-73's
//                      `monthlySatoshis` is treated as a per-transaction USD
//                      cap (the exact mistake R-CAPS forbids), then proves the
//                      assertions CATCH it. Exits 0 when the breach is
//                      observed, 1 if it is not — the inversion preflight's
//                      -NegativeControl expects.
//
// Exit 0 = all expectations met for the mode; exit 1 = a failure.

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, '../../../..'); // probes -> phase -> 0.4.0-beta.3 -> development-docs -> repo
const NC = process.argv.includes('--negative-control');

const SRC = resolve(REPO, 'frontend/src/utils/manifestConsent.ts');

// The user's own defaults, as shipped ($1.00/tx, $10.00/session, 30/min, 100).
const USER_DEFAULTS = {
  perTxCents: 100,
  perSessionCents: 1000,
  rateLimitPerMin: 30,
  maxTxPerSession: 100,
};

// The `brc73-all-categories.json` fixture's declaration: 5,000,000 satoshis per
// month, and NOTHING in our units.
const BRC73_SPENDING = {
  monthlySatoshis: 5_000_000,
  purpose: 'Up to 5,000,000 satoshis per month for in-app purchases.',
};

// ⛔ A5 MUST also be driven with a SMALL declared amount — BRC-73's own spec
// example, 10,000. MEASURED 2026-08-22: with only the 5,000,000 case, the
// negative control did NOT trip. 5,000,000 is above `usableUsd`'s magnitude
// sanity cap, so even with the unit rule deliberately broken the value was
// rejected for being absurd, and the assertion passed for the WRONG REASON —
// it was measuring the magnitude guard, not the unit separation. 10,000 sits
// inside the sanity range, so it isolates the property A5 actually claims.
const BRC73_SPENDING_SMALL = {
  monthlySatoshis: 10_000,
  purpose: 'For in-app purchases.',
};

// Our own legacy shape's declaration: $100/tx, $1000/session. Same unit and
// period as our caps, so this one IS eligible to pre-fill — behind the opt-in.
const HODOS_LEGACY_SPENDING = { perTransactionUsd: 100, perSessionUsd: 1000 };

// The negative control: treat BRC-73's monthly satoshis as a per-transaction
// USD cap. This is the single edit that would break R-CAPS, and it is applied
// to a COPY of the real source so the harness still exercises real code.
function breakRCaps(src) {
  return src.replace(
    'const perTx = usableUsd(spending.perTransactionUsd);',
    'const perTx = usableUsd(spending.perTransactionUsd ?? spending.monthlySatoshis);',
  );
}

async function loadModule() {
  if (!NC) return import(pathToFileURL(SRC).href);
  const broken = breakRCaps(readFileSync(SRC, 'utf8'));
  if (broken === readFileSync(SRC, 'utf8')) {
    console.error('[T1f] NEGATIVE CONTROL COULD NOT BE APPLIED — the anchor line moved.');
    console.error('      Update breakRCaps() in this harness; do not assume the rule still holds.');
    process.exit(1);
  }
  const tmp = resolve(tmpdir(), `manifestConsent.nc.${process.pid}.ts`);
  writeFileSync(tmp, broken, 'utf8');
  return import(pathToFileURL(tmp).href);
}

const failures = [];
function check(name, cond, detail) {
  if (cond) {
    console.log(`  ok   ${name}`);
  } else {
    console.log(`  FAIL ${name} — ${detail}`);
    failures.push(name);
  }
}

const M = await loadModule();
const { resolveConnectLimits, useMyDefaults, useSiteSuggested, hasSiteSourcedLimit,
        shouldExpandLimits, formatMonthlyAllowance,
        isEditableUsdText, isEditableIntText, usdTextToCents, intTextToNumber } = M;

console.log(NC ? '=== T1f NEGATIVE CONTROL (R-CAPS deliberately broken) ===' : '=== T1f ===');

// ── P0.8-A5 — BRC-73's monthly satoshis must never become one of our caps ────
// This is the assertion the negative control is designed to trip.
let rCapsHeld = true;
for (const [label, spending] of [['fixture 5,000,000', BRC73_SPENDING],
                                 ['spec example 10,000', BRC73_SPENDING_SMALL]]) {
  for (const prefill of [false, true]) {
    const r = resolveConnectLimits({
      userDefaults: USER_DEFAULTS,
      spending,
      prefillFromManifest: prefill,
    });
    const ok = r.values.perTxCents === 100
            && r.values.perSessionCents === 1000
            && r.sourceOf.perTxCents === 'user'
            && r.sourceOf.perSessionCents === 'user';
    if (!ok) rCapsHeld = false;
    if (!NC) {
      check(
        `A5 monthly satoshis never become a cap (${label}, prefill=${prefill})`,
        ok,
        `got perTx=${r.values.perTxCents} (${r.sourceOf.perTxCents}), ` +
        `perSession=${r.values.perSessionCents} (${r.sourceOf.perSessionCents})`,
      );
    }
  }
}

if (NC) {
  // In negative-control mode the SUCCESS condition is that the breach is seen.
  // Use the SMALL amount: it is inside the magnitude sanity range, so what is
  // being measured is the unit rule and nothing else.
  const r = resolveConnectLimits({
    userDefaults: USER_DEFAULTS,
    spending: BRC73_SPENDING_SMALL,
    prefillFromManifest: true,
  });
  console.log(`  observed perTxCents=${r.values.perTxCents} (source=${r.sourceOf.perTxCents})`);
  if (rCapsHeld) {
    console.log('  FAIL — R-CAPS was deliberately broken and the assertion did NOT notice.');
    console.log('         The A5 test is blind. Fix the test, not the code.');
    process.exit(1);
  }
  console.log('  ok   the A5 assertion CATCHES a monthly-satoshis-as-cap regression');
  console.log('=== T1f negative control: the check is not blind ===');
  process.exit(0);
}

// ── P0.8-A12 — the toggle changes which values are pre-filled, nothing else ──

const off = resolveConnectLimits({
  userDefaults: USER_DEFAULTS,
  spending: HODOS_LEGACY_SPENDING,
  prefillFromManifest: false,
});
check('A12 toggle OFF: limit fields carry the USER\'s defaults',
  off.values.perTxCents === 100 && off.values.perSessionCents === 1000,
  `got ${JSON.stringify(off.values)}`);
check('A12 toggle OFF: no field is marked as the site\'s',
  !hasSiteSourcedLimit(off.sourceOf),
  JSON.stringify(off.sourceOf));
check('A12 toggle OFF: the site\'s suggestion is still surfaced beside the field',
  off.siteSuggests.perTxCents === 10000 && off.siteSuggests.perSessionCents === 100000,
  JSON.stringify(off.siteSuggests));

const on = resolveConnectLimits({
  userDefaults: USER_DEFAULTS,
  spending: HODOS_LEGACY_SPENDING,
  prefillFromManifest: true,
});
check('A12 toggle ON: limit fields carry the SITE\'s values',
  on.values.perTxCents === 10000 && on.values.perSessionCents === 100000,
  `got ${JSON.stringify(on.values)}`);
check('A12 toggle ON: those fields stay MARKED as the site\'s (R-PROV)',
  on.sourceOf.perTxCents === 'site' && on.sourceOf.perSessionCents === 'site',
  JSON.stringify(on.sourceOf));
check('A12 toggle ON: fields the site said nothing about stay the user\'s',
  on.values.rateLimitPerMin === 30 && on.values.maxTxPerSession === 100
    && on.sourceOf.rateLimitPerMin === 'user' && on.sourceOf.maxTxPerSession === 'user',
  JSON.stringify({ v: on.values, s: on.sourceOf }));

// ── Owner requirement: sections must be EXPANDED when a site value is shown ──

check('sections expand when a field carries the site\'s value',
  shouldExpandLimits(on.sourceOf, on.siteSuggests) === true,
  'a user must not approve values hidden behind a collapsed section');
check('sections expand when a suggestion is merely shown beside the field',
  shouldExpandLimits(off.sourceOf, off.siteSuggests) === true,
  'a suggestion the user never sees is no better than one they cannot distinguish');
const plain = resolveConnectLimits({
  userDefaults: USER_DEFAULTS, spending: null, prefillFromManifest: true,
});
check('sections stay collapsed when the site suggested nothing',
  shouldExpandLimits(plain.sourceOf, plain.siteSuggests) === false,
  'no suggestion, no reason to force the section open');

// ── P0.8-A10 — "Use my defaults" reverts every suggested field ───────────────

const reverted = useMyDefaults(USER_DEFAULTS);
check('A10 "Use my defaults" restores every value',
  reverted.values.perTxCents === 100 && reverted.values.perSessionCents === 1000
    && reverted.values.rateLimitPerMin === 30 && reverted.values.maxTxPerSession === 100,
  JSON.stringify(reverted.values));
check('A10 "Use my defaults" clears every site mark',
  !hasSiteSourcedLimit(reverted.sourceOf),
  JSON.stringify(reverted.sourceOf));

// ── "Use this site's suggested limits" — the forward direction ──────────────

const adopted = useSiteSuggested(USER_DEFAULTS, off.siteSuggests);
check('adopt: takes the site\'s figures',
  adopted.values.perTxCents === 10000 && adopted.values.perSessionCents === 100000,
  JSON.stringify(adopted.values));
check('adopt: the adopted fields stay MARKED as the site\'s (R-PROV)',
  adopted.sourceOf.perTxCents === 'site' && adopted.sourceOf.perSessionCents === 'site',
  JSON.stringify(adopted.sourceOf));
check('adopt: fields the site said nothing about are untouched and stay the user\'s',
  adopted.values.rateLimitPerMin === 30 && adopted.sourceOf.rateLimitPerMin === 'user',
  JSON.stringify({ v: adopted.values, s: adopted.sourceOf }));
check('adopt then revert round-trips exactly',
  JSON.stringify(useMyDefaults(USER_DEFAULTS).values) === JSON.stringify(USER_DEFAULTS),
  'Use my defaults must undo an adopt');

// 🚨 R-CAPS: adopting must be IMPOSSIBLE for a BRC-73 monthly figure, because
// it never becomes a suggestion in the first place.
const brc73Suggests = resolveConnectLimits({
  userDefaults: USER_DEFAULTS, spending: BRC73_SPENDING_SMALL, prefillFromManifest: false,
}).siteSuggests;
const adoptedBrc73 = useSiteSuggested(USER_DEFAULTS, brc73Suggests);
check('adopt: a BRC-73 monthly allowance can never be adopted as a cap',
  adoptedBrc73.values.perTxCents === 100 && adoptedBrc73.sourceOf.perTxCents === 'user',
  `got ${adoptedBrc73.values.perTxCents} (${adoptedBrc73.sourceOf.perTxCents})`);

// ── The limit boxes must be TYPEABLE ────────────────────────────────────────
// Reported live 2026-08-23: only the spinner arrows worked. The property is
// that EVERY PREFIX of a valid entry is accepted — you cannot type "25.00" if
// "2", "25" or "25." are rejected along the way.

for (const [entry, accept, toNum, expected] of [
  ['25.00', isEditableUsdText, usdTextToCents, 2500],
  ['0.50',  isEditableUsdText, usdTextToCents, 50],
  ['7',     isEditableUsdText, usdTextToCents, 700],
  ['120',   isEditableIntText, intTextToNumber, 120],
  ['5',     isEditableIntText, intTextToNumber, 5],
]) {
  let everyPrefixOk = true;
  const rejected = [];
  for (let i = 1; i <= entry.length; i++) {
    const prefix = entry.slice(0, i);
    if (!accept(prefix)) { everyPrefixOk = false; rejected.push(prefix); }
  }
  check(`typing "${entry}" — every prefix accepted`, everyPrefixOk,
    `rejected: ${JSON.stringify(rejected)}`);
  check(`typing "${entry}" — final value is ${expected}`, toNum(entry) === expected,
    `got ${toNum(entry)}`);
}

check('a box can be CLEARED to retype (empty string accepted)',
  isEditableUsdText('') && isEditableIntText(''),
  'an empty box must be a legal intermediate state');
check('an emptied box reads as 0, not NaN',
  usdTextToCents('') === 0 && intTextToNumber('') === 0,
  `${usdTextToCents('')} / ${intTextToNumber('')}`);
check('a bare decimal point mid-typing is accepted and parses',
  isEditableUsdText('2.') && usdTextToCents('2.') === 200,
  `accept=${isEditableUsdText('2.')} value=${usdTextToCents('2.')}`);
check('junk is rejected outright',
  !isEditableUsdText('abc') && !isEditableUsdText('1.234') && !isEditableUsdText('-5')
    && !isEditableIntText('1.5') && !isEditableIntText('x'),
  'letters, >2dp, negatives and decimals-in-int must all be refused');

// ── Hostile declared values never reach an input box ────────────────────────

for (const [label, spending] of [
  ['NaN',       { perTransactionUsd: NaN }],
  ['Infinity',  { perTransactionUsd: Infinity }],
  ['negative',  { perTransactionUsd: -5 }],
  ['zero',      { perTransactionUsd: 0 }],
  ['absurd',    { perTransactionUsd: 1e12 }],
  ['string',    { perTransactionUsd: '100' }],
  ['null',      { perTransactionUsd: null }],
]) {
  const r = resolveConnectLimits({
    userDefaults: USER_DEFAULTS, spending, prefillFromManifest: true,
  });
  check(`hostile perTransactionUsd (${label}) is ignored`,
    r.values.perTxCents === 100 && r.sourceOf.perTxCents === 'user',
    `got ${r.values.perTxCents} (${r.sourceOf.perTxCents})`);
}

// ── The monthly allowance is displayed in the unit the site declared ─────────

check('BRC-73 allowance renders as satoshis/month, never converted to a cap',
  formatMonthlyAllowance(BRC73_SPENDING) === '5,000,000 satoshis / month',
  String(formatMonthlyAllowance(BRC73_SPENDING)));
check('no allowance declared renders as nothing',
  formatMonthlyAllowance(HODOS_LEGACY_SPENDING) === null
    && formatMonthlyAllowance(null) === null,
  'expected null');

console.log('');
if (failures.length) {
  console.log(`T1f FAILED: ${failures.length} check(s) — ${failures.join(', ')}`);
  process.exit(1);
}
console.log('T1f PASS — all checks ran and passed.');
process.exit(0);
