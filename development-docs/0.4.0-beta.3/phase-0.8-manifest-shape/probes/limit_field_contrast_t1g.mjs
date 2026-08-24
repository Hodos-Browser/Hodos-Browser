// P0.8 T1g — the consent screen's limit values must be READABLE.
//
// WHY THIS EXISTS. Round 2 (`8f3982c`) shipped a marked-field style that set
//   background: '#fff8f0'   on the input
// while leaving the dark theme's
//   color: '#f0f0f0'
// in place. Near-white on near-white: contrast 1.07:1. The per-transaction
// spending cap a user was about to approve was INVISIBLE — and invisible
// precisely in the state that matters, the one where the SITE chose the
// number rather than the user. The "suggested by site" pill beside it was
// dark-on-light and perfectly legible, so the screen shouted about a value it
// was simultaneously hiding. Found by the owner reading the real modal, not by
// any test; T1f passed throughout, because T1f guards the consent RULE and
// this is the consent SURFACE.
//
// SUBJECT: the contrast ratio of text against the box it sits in, for the
// limit fields of the connect modal, in BOTH provenance states. Not "does a
// style object exist" — a style object existed the whole time the number was
// unreadable. The numbers are read out of the real .tsx and run through the
// real WCAG 2.1 relative-luminance formula.
//
// Modes:
//   (default)          GREEN — every pair clears WCAG AA (4.5:1).
//   --negative-control RED   — restores the shipped background override in a
//                      scratch copy and proves the assertion catches it.
//                      Exits 0 when the breach is observed, 1 if it is not.
//
// Exit 0 = all expectations met for the mode; exit 1 = a failure.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, '../../../..');
const NC = process.argv.includes('--negative-control');
const SRC = resolve(REPO, 'frontend/src/pages/BRC100AuthOverlayRoot.tsx');

const AA = 4.5;
let src = readFileSync(SRC, 'utf8');

if (NC) {
  // Reinstate the shipped defect: a `background` override with no `color`.
  //
  // ⛔ The injection is VERIFIED, not assumed. An earlier version of this
  // probe pinned to a literal style string; when the modal was restyled on
  // 2026-08-23 the replace silently became a no-op, so the negative control
  // was measuring UNMODIFIED source and would have reported "the defect was
  // not caught" against perfectly good code. A negative control that cannot
  // prove it injected anything is worth exactly as much as no control, so a
  // no-op is a hard error here rather than a confusing red.
  const before = src;
  src = src.replace(
    /(const limitInputStyle[\s\S]*?\.\.\.customizeNumberInput,)/,
    `$1
  ...(source === 'site' ? { background: '#fff8f0' } : {}),`,
  ).replace('(_source:', '(source:');
  if (src === before) {
    console.log('T1g NEGATIVE CONTROL FAIL — could not inject the defect;'
      + ' limitInputStyle no longer has the shape this probe rewrites.'
      + ' FIX THE PROBE: it is not testing anything.');
    process.exit(1);
  }
}

// ── WCAG 2.1 relative luminance + contrast ratio ────────────────────────────
const srgb = (c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
function luminance(hex) {
  const h = hex.replace('#', '');
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
  return 0.2126 * srgb(r) + 0.7152 * srgb(g) + 0.0722 * srgb(b);
}
function contrast(fg, bg) {
  const [a, b] = [luminance(fg), luminance(bg)].sort((x, y) => y - x);
  return (a + 0.05) / (b + 0.05);
}

// ── pull the real values out of the real source ─────────────────────────────
function grab(re, what) {
  const m = src.match(re);
  if (!m) {
    console.log(`  FAIL  could not read ${what} from the source — the probe is`
      + ` stale, which is itself a failure: it can no longer see its subject.`);
    process.exit(1);
  }
  return m[1];
}

const cardBg = grab(/white:\s*'(#[0-9a-fA-F]{6})'/, 'card background (COLORS.white)');
const inputBg = grab(/const customizeNumberInput[\s\S]*?background:\s*'(#[0-9a-fA-F]{6})'/, 'input background');
const inputFg = grab(/const customizeNumberInput[\s\S]*?color:\s*'(#[0-9a-fA-F]{6})'/, 'input text colour');

// The site-state override, if there is one at all. Deliberately tolerant: this
// modal has been restyled twice already and will be again, so this reads
// WHATEVER the override sets rather than pinning to one shape. The invariant
// under test is not "the style looks like X" — it is that a user can READ the
// number they are about to approve as a spending cap.
//
// 🚨 The asymmetry that shipped the bug is encoded here: an override may set
// `background` without `color`, in which case the text is still the dark
// theme's near-white. That is exactly how near-white ended up on near-white.
const overrideBody = (src.match(
  /const limitInputStyle[\s\S]*?source === 'site'[\s\S]*?\?\s*\{([^}]*)\}/,
) || [null, ''])[1];
const overrideBg = overrideBody.match(/background(?:Color)?:\s*'(#[0-9a-fA-F]{6})'/);
const overrideFg = overrideBody.match(/(?<![a-zA-Z])color:\s*'(#[0-9a-fA-F]{6})'/);

// Every colour the field label can take, whatever the expression shape: a
// ternary yields two, a constant yields one, and both must clear AA against the
// card. Pinning to the ternary is what made the first version of this probe go
// stale the moment the owner asked for one colour instead of two.
const labelBlock = grab(/(const limitFieldLabel[\s\S]*?\}\);)/, 'limitFieldLabel block');
const uniqueLabelColours = [...new Set(
  [...labelBlock.matchAll(/'(#[0-9a-fA-F]{6})'/g)].map((m) => m[1]),
)];
if (uniqueLabelColours.length === 0) {
  console.log('  FAIL  limitFieldLabel declares no colour the probe can read —'
    + ' the probe has gone stale, which is itself a failure.');
  process.exit(1);
}

// The *** is the ONLY provenance signal since the 2026-08-23 restyle, so it has
// to be legible too: an unreadable mark is an unmarked field.
const markFg = grab(/const siteSuggestedMark[\s\S]*?color:\s*'(#[0-9a-fA-F]{6})'/, 'siteSuggestedMark colour');

// The summary expander's provenance heading. It shipped as COLORS.error
// ('#c62828') = 3.00:1 against the dark card: below AA, on the single line
// whose entire job is to say the numbers are not the user's. Same defect class
// as the input, smaller blast radius, and equally invisible to T1f.
const headingFg = grab(
  /color:\s*'(#[0-9a-fA-F]{6})',\s*fontWeight:\s*700\s*\}\}>\s*Payment limits suggested/,
  'summary provenance heading colour',
);

const cases = [
  { name: 'limit value, user default', fg: inputFg, bg: inputBg },
  {
    name: 'limit value, SITE-suggested',
    fg: overrideFg ? overrideFg[1] : inputFg,
    bg: overrideBg ? overrideBg[1] : inputBg,
  },
  ...uniqueLabelColours.map((c) => ({ name: `field label (${c})`, fg: c, bg: cardBg })),
  { name: 'the *** provenance mark', fg: markFg, bg: cardBg },
  { name: '"suggested by this site" heading', fg: headingFg, bg: cardBg },
];

let failures = 0;
console.log(`T1g — limit-field contrast${NC ? ' (NEGATIVE CONTROL)' : ''}`);
for (const c of cases) {
  const ratio = contrast(c.fg, c.bg);
  const ok = ratio >= AA;
  if (!ok) failures++;
  console.log(`  ${ok ? 'ok  ' : 'FAIL'} ${c.name}: ${c.fg} on ${c.bg} = ${ratio.toFixed(2)}:1`
    + `${ok ? '' : `  <-- below WCAG AA ${AA}:1, the value is not readable`}`);
}

if (NC) {
  if (failures > 0) {
    console.log('\nT1g NEGATIVE CONTROL PASS — the shipped defect was caught.');
    process.exit(0);
  }
  console.log('\nT1g NEGATIVE CONTROL FAIL — the defect was reinstated and the'
    + ' assertions did NOT catch it. This probe proves nothing.');
  process.exit(1);
}

if (failures > 0) {
  console.log(`\nT1g FAIL — ${failures} pair(s) below AA.`);
  process.exit(1);
}
console.log('\nT1g PASS — every limit field is readable in both provenance states.');
process.exit(0);
