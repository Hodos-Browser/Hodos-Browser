/**
 * Hodos Browser Design Tokens — TypeScript companion
 *
 * Use these for inline styles and MUI sx props where CSS var() isn't available.
 * Values MUST match tokens.css — this is the same source of truth in TS form.
 */

export const tokens = {
  // Brand (theme-invariant)
  gold: '#a67c00',
  goldHover: '#b88d00',
  goldActive: '#8a6800',
  goldLight: '#dfbd69',
  goldDark: '#a57d2d',

  // Backgrounds
  bgPrimary: '#0a0a0b',
  bgElevated: '#141416',
  bgSurface: '#1a1d23',
  bgSurfaceHover: '#1f2937',

  // Text
  textPrimary: '#ffffff',
  textSecondary: '#b0b7c3',
  textMuted: '#6b7280',

  // Borders
  borderDefault: '#363640',
  borderSubtle: '#1f1f23',

  // Semantic
  success: '#2e7d32',
  warning: '#e6a200',
  error: '#c62828',
  info: '#1a6b6a',

  // Typography
  fontUi: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
  fontMono: "'SF Mono', 'Fira Code', 'Cascadia Code', 'Consolas', monospace",

  // Spacing
  spaceXs: '8px',
  spaceSm: '16px',
  spaceMd: '24px',
  spaceLg: '48px',

  // Radius
  radiusSm: '6px',
  radiusMd: '8px',
  radiusLg: '12px',
  radiusXl: '16px',
  radiusPill: '9999px',

  // Shadows
  shadowSm: '0 1px 2px rgba(0, 0, 0, 0.3)',
  shadowMd: '0 4px 12px rgba(0, 0, 0, 0.4)',
  shadowLg: '0 8px 24px rgba(0, 0, 0, 0.5)',
} as const;
