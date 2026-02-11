# Color Guidelines & Logos

## Purpose

Single reference for Hodos brand colors and logo usage. Other UX_UI docs should point here for palette and asset locations.

**Last Updated:** 2026-02-11

---

## Color Guidelines

### Brand colors (from logo assets)

| Use | Hex | Notes |
|-----|-----|------|
| **Gold (primary)** | `#a67c00` | Primary gold from Hodos Gold icon gradients |
| **Gold (accent/dark)** | `#a57d2d` | Darker gold used in Gold icon |
| **Black** | `#000000` | Used in Black icon variant |
| **White** | `#ffffff` | Highlights and light backgrounds |

Use **Gold** for primary branding on dark backgrounds (e.g. header, splash). Use **Black** icon on light backgrounds. When integrating with MUI or app theme, prefer these for brand elements and use theme colors for neutral UI (backgrounds, borders, text) unless a screen is brand-heavy.

### UI usage (concise)

- **Primary accent / brand:** `#a67c00` (gold) for buttons, links, or key UI on dark themes.
- **Backgrounds:** White or light gray for content; dark for header/shell if desired.
- **Text:** Black or near-black on light; white on dark.
- **Success / warning / error:** Use theme or existing app conventions; keep gold for brand-only so it doesn’t double as “success.”

---

## Logos

- **Location:** `frontend/public/` only.
- **Files:** `Hodos_Gold_Icon.svg`, `Hodos_Black_Icon.svg`.
- **Use in app:** Reference as `/Hodos_Gold_Icon.svg` or `/Hodos_Black_Icon.svg` (e.g. in `<img src="/Hodos_Gold_Icon.svg" />` or as background/custom element). Control display size with CSS (e.g. `width`/`height` or `background-size`); the SVGs scale to any size.

**Sizing:** Both SVGs use `viewBox="0 0 216 216"` (square). They have no fixed pixel size in the file, so they are correct to use at any display size (e.g. 24×24 for toolbar, 32×32 for buttons, 96×96 for splash). Choose the size that fits each context.

**Which variant:** Prefer **Gold** on dark backgrounds (e.g. dark header, dark modals). Prefer **Black** on light backgrounds (e.g. light toolbar, light wallet UI).

---

**End of Document**
