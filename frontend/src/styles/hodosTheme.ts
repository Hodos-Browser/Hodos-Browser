// Hodos Browser shared theme tokens.
//
// Step 0 of Phase 1.5 (BRC-100 Surface Completion) centralizes the brand
// palette + font that were previously inlined in every overlay, page, and
// component. New code should import from here. Existing inline strings
// (#1a1a1a, #a67c00, "Inter", etc.) will migrate over time as files are
// touched — this module is the canonical source of truth, not a one-shot
// rewrite.
//
// Token tiers:
//   - colors: brand palette (gold, dark surfaces, text, semantic accents)
//   - fonts:  font stacks
//   - prompt: per-prompt-tier theming. The `privacy_perimeter` tier is
//             a heightened gold framing for identity-key / key-linkage
//             prompts (Phase 1.5 Q5 lock — differentiated but not
//             red-alarm).

export const colors = {
  // Surfaces
  bgPrimary: '#1a1a1a',       // page background, modal backdrop
  bgSurface: '#252525',       // raised cards, dialogs
  bgSurfaceAlt: '#1e1e1e',    // settings cards (slightly darker)
  bgInputDark: '#2a2a2a',     // input fields on dark surfaces

  // Borders
  borderSubtle: '#2a2d35',
  borderDefault: '#333',
  borderInput: '#444',

  // Text
  textPrimary: '#e0e0e0',
  textBright: '#f0f0f0',
  textMuted: '#9ca3af',
  textSecondary: '#888',
  textDim: '#aaa',

  // Brand gold tiers
  goldPrimary: '#a67c00',     // canonical Hodos gold — titles, accents, brand
  goldHover: '#bf9000',       // primary gold hover state
  goldBright: '#d4a017',      // brighter accent gold (used for emphasis,
                              // payment failed CTA, "not broadcast" callout)

  // Semantic accents
  errorBg: 'rgba(211, 47, 47, 0.1)',
  errorFg: '#c62828',
  errorBright: '#ef4444',
  successFg: '#4ade80',
  warningFg: '#d4a017',       // shares brightGold by design

  // Subdued / decorative
  subduedDark: '#111827',
} as const;

export const fonts = {
  // Standard system font stack used across the app.
  sans: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
} as const;

// Prompt-tier theming. Each tier is a recipe of tokens combined into the
// shapes that overlays consume (header color, accent color, framing).
//
// - standardAutoApprove: the default look for connect / payment / cert
//   disclosure / rate-limit / no-wallet / edit-permissions prompts.
// - privacyPerimeter: identity-key reveal + key-linkage reveal. Per Q5
//   (locked 2026-05-09), use heightened gold framing — slightly larger
//   header, more visual weight, plain-language warning copy — but no
//   red border or alarm icon. Privacy-first without scaring users into
//   web2-warning desensitization.
export const prompt = {
  standardAutoApprove: {
    headerColor: colors.textPrimary,
    accentColor: colors.goldPrimary,
    headerFontSize: '16px',
    headerWeight: 700,
    framingBorder: `1px solid ${colors.borderDefault}`,
  },
  privacyPerimeter: {
    headerColor: colors.goldPrimary,         // gold-toned header text
    accentColor: colors.goldPrimary,
    headerFontSize: '18px',                  // slightly larger
    headerWeight: 700,
    framingBorder: `1px solid ${colors.goldPrimary}`,  // gold-bordered card
    framingShadow: '0 0 0 3px rgba(166, 124, 0, 0.12)', // soft gold halo
  },
} as const;

// Convenience re-export so existing files can do
// `import { colors as hodosColors } from '../styles/hodosTheme'`
// or `import hodosTheme from '../styles/hodosTheme'`.
const hodosTheme = { colors, fonts, prompt };
export default hodosTheme;
