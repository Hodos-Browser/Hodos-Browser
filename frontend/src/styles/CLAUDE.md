# Frontend Styles
> Centralized theme tokens and design conventions for Hodos Browser UI.

## Overview

This directory holds shared style modules. Today it contains a single file — `hodosTheme.ts` — which centralizes the brand palette, font stacks, and per-prompt-tier theming recipes that were previously inlined across overlays, pages, and components. New code should import tokens from here.

Pre-Phase-1.5, brand colors were inlined as raw hex strings in dozens of files (`#1a1a1a`, `#a67c00`, etc.). Phase 1.5 Step 0 created `hodosTheme.ts` as the canonical source of truth. Migration is gradual — existing inline strings rewrite over time as files are touched. **New code MUST import from `hodosTheme.ts`** rather than re-inlining.

## Files

| File | Purpose |
|------|---------|
| `hodosTheme.ts` | Brand palette, font stacks, prompt-tier theming recipes |

## hodosTheme.ts

### Colors

Three tiers of named tokens:

| Tier | Tokens | Use |
|------|--------|-----|
| **Surfaces** | `bgPrimary`, `bgSurface`, `bgSurfaceAlt`, `bgInputDark` | Page backgrounds, raised cards, input fields |
| **Borders** | `borderSubtle`, `borderDefault`, `borderInput` | Section dividers, card borders, input outlines |
| **Text** | `textPrimary`, `textBright`, `textMuted`, `textSecondary`, `textDim` | Body text, headings, secondary labels |
| **Brand gold** | `goldPrimary` (#a67c00), `goldHover` (#bf9000), `goldBright` (#d4a017) | Hodos gold — titles, accents, hovers, emphasis CTAs |
| **Semantic** | `errorBg`, `errorFg`, `errorBright`, `successFg`, `warningFg` | Status indicators, validation errors |
| **Decorative** | `subduedDark` | Sidebar backgrounds |

### Fonts

| Token | Value |
|-------|-------|
| `fonts.sans` | `'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif` |

### Prompt tiers

Two prompt-tier recipes for BRC-100 auth overlay headers:

| Tier | When | Treatment |
|------|------|-----------|
| `standardAutoApprove` | connect / payment / cert disclosure / rate-limit / no-wallet / edit-permissions prompts | Standard 16px white header, gold accent, default border |
| `privacyPerimeter` | identity-key reveal, key-linkage reveal (Phase 1.5 Step 1) | Heightened: gold-toned header (18px), gold-bordered card, soft gold halo. **Differentiated but NOT red-alarm** — see below |

#### Why `privacyPerimeter` is gold, not red

Per Phase 1.5 Q5 lock (2026-05-09), privacy-perimeter prompts use the wallet's gold accent — not red — to communicate gravity without inducing web2-warning desensitization. Rationale (user words):

> "We are privacy first and want to get their attention, but it is information they already give away every day in web2 without even knowing. We don't want to scare them and we don't want to desensitize them to red warnings that might require more critical analysis. But still differentiated."

So: heightened gold framing, slightly larger header, plain-language copy explaining what's being shared and with whom — but no red border, no alarm icon, no panic-inducing styling.

## Usage

```tsx
import { colors, fonts, prompt } from '../styles/hodosTheme';
// or
import hodosTheme from '../styles/hodosTheme';

const cardStyle = {
  background: colors.bgSurface,
  border: `1px solid ${colors.borderDefault}`,
  color: colors.textPrimary,
  fontFamily: fonts.sans,
};

const perimeterHeader = {
  fontSize: prompt.privacyPerimeter.headerFontSize,
  fontWeight: prompt.privacyPerimeter.headerWeight,
  color: prompt.privacyPerimeter.headerColor,
};
```

## Migration policy

When you touch a file containing inline brand colors or fonts:
1. Replace `#1a1a1a` / `#252525` / `#e0e0e0` / `#a67c00` / `Inter` references with token imports.
2. Don't re-inline tokens you didn't add. Don't open unrelated files just to migrate them — wait until they're touched naturally.
3. New components ALWAYS import from `hodosTheme.ts`. No exceptions.

## Related

- `../components/CLAUDE.md` — components that consume these tokens
- `../components/settings/CLAUDE.md` — settings cards that use the dark theme
- `../components/wallet/CLAUDE.md` — wallet tabs that use `wd-` CSS classes (which mirror these tokens; `WalletPanel.css` should consume from `hodosTheme.ts` over time)
- `../pages/CLAUDE.md` — overlay roots that style cards and modals
- Root `CLAUDE.md` — CEF input rules (native input only) and overlay lifecycle
