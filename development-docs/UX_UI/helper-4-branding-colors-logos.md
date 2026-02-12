# Color Guidelines & Logos

## Purpose

Single reference for Hodos brand colors and logo usage. Other UX_UI docs should point here for palette and asset locations.

**Last Updated:** 2026-02-11

---

## Color Guidelines

### Brand Colors

| Use | Hex | Notes |
|-----|-----|------|
| **Gold (primary)** | `#a67c00` | Primary brand color. Buttons, links, headers, key UI on dark backgrounds |
| **Gold (accent/dark)** | `#a57d2d` | Darker gold variant from Gold icon |
| **Black** | `#000000` | Used in Black icon variant; primary text on light backgrounds |
| **White** | `#ffffff` | Highlights, light backgrounds, text on dark backgrounds |

### Complementary Accent Colors

| Use | Hex | Notes |
|-----|-----|------|
| **Deep Teal** | `#1a6b6a` | Complementary accent for links, interactive elements, info states. Sits opposite gold on the color wheel — professional and distinct |
| **Slate** | `#4a5568` | Warm-neutral gray for secondary UI, borders, muted text |

### Semantic Colors (Functional UI)

| Use | Hex | Notes |
|-----|-----|------|
| **Success** | `#2e7d32` | Forest green. Confirmed transactions, successful operations |
| **Warning** | `#e6a200` | Amber. Pending states, caution prompts. Distinct from brand gold |
| **Error** | `#c62828` | Deep red. Failed transactions, destructive action buttons |
| **Info** | `#1a6b6a` | Same as deep teal. Informational states, links |

### Deprecated

| Use | Hex | Notes |
|-----|-----|------|
| ~~**Blue (old primary)**~~ | ~~`#1a73e8`~~ | **Removed.** This was Google's blue used as a default. Replaced by gold for brand elements and teal for functional accent. Remove from codebase during Phase 3 polish. |

---

## Usage Rules

- **Gold** for primary branding: buttons, links, header accent, primary CTAs on dark backgrounds
- **Teal** for functional accent: links in body text, info badges, interactive highlights
- **Slate** for neutral UI: borders, secondary text, disabled states
- **Semantic colors** for status only: green=success, amber=warning, red=error. Do NOT use gold for "success" to avoid brand confusion
- **Backgrounds**: White or light gray for content areas; dark (near-black) for header/shell if desired
- **Text**: Black or near-black on light backgrounds; white on dark backgrounds

### Dark Mode

**Deferred to a separate sprint.** Dark mode is not in MVP scope. When implemented, gold on dark backgrounds is already spec'd and will be the primary brand expression. The semantic colors above are chosen to work on both light and dark backgrounds.

When dark mode is designed, create a dark palette variant in this document.

---

## Logos

- **Location:** `frontend/public/` only.
- **Files:** `Hodos_Gold_Icon.svg`, `Hodos_Black_Icon.svg`.
- **Use in app:** Reference as `/Hodos_Gold_Icon.svg` or `/Hodos_Black_Icon.svg` (e.g. in `<img src="/Hodos_Gold_Icon.svg" />` or as background/custom element). Control display size with CSS (e.g. `width`/`height` or `background-size`); the SVGs scale to any size.

**Sizing:** Both SVGs use `viewBox="0 0 216 216"` (square). They have no fixed pixel size in the file, so they are correct to use at any display size (e.g. 24x24 for toolbar, 32x32 for buttons, 96x96 for splash). Choose the size that fits each context.

**Which variant:** Prefer **Gold** on dark backgrounds (e.g. dark header, dark modals). Prefer **Black** on light backgrounds (e.g. light toolbar, light wallet UI).

---

**End of Document**
