# Profile Picker — UI Redesign Phase (owner-requested, 2026-07-07)

> **✅ SHIPPED (as of 2026-07-09).** The launcher-window + tile-grid redesign described
> below landed and is live in the shipped releases (beta.23 → beta.26 LIVE). Relevant
> commits: `49c7693` (docs), `0a3f4ce` (launcher window + tiles), `5177d61` (bigger logo
> + gold-bordered tiles), `b1f35af` (final launcher polish), `6d358b8` (close button).
> This doc is retained as design rationale + record. There is no remaining "before
> publish" gate — publish already happened. Any further tweaks are cosmetic-only.
>
> **Note on the same-process refactor:** the `PROFILE_PICKER_SAME_PROCESS_PLAN.md` work
> this redesign defers to is now **SHELVED** (owner decision — revisit way down the road
> with real market feedback; wallet stays SHARED), not merely deferred. As predicted
> below, making the picker read as a distinct designed launcher window took the pressure
> off that risky work.

**Original priority (2026-07-07, now satisfied):** land **this session before publish** —
the current launch picker "looks lazy" and can't be shown to users. Functionality first,
but this is a visible-quality gate.

**Scope boundary:** this is the **visual redesign of the launch picker window** — it is NOT
the same-process refactor (that's D1/D2 in `PROFILE_PICKER_SAME_PROCESS_PLAN.md`, now
**SHELVED**). Crucially, these interact: the owner's point is that if the picker looks like a
**distinct, designed launcher window** (not a full browser window with an empty content
area), then the current two-process "close → reopen" handoff **feels intentional** rather
than broken. So this redesign *substantially reduces the pressure* on the risky same-process
work — the safe way to make D1 feel right.

---

## What's wrong now (owner-observed)

The launch picker (`/profile-picker?mode=window`) and the in-header profile dropdown
(`/profile-picker`) are the **same React component** (`ProfilePickerOverlayRoot.tsx`). In
launch mode the C++ shell makes the header browser fill the **entire browser-sized window**
(`cef_browser_shell.cpp` ~4880–4883: header rect = `0,0,width,height` when `g_picker_mode`),
so a **compact dropdown-list UI gets stretched across a full-screen window** → lots of empty
space, and it reads as "the browser opened weirdly." Owner's specific complaints:

1. Takes up the entire screen — should be a proper, smaller window.
2. Doesn't use our CSS/theme styling.
3. No Hodos logo.
4. Profiles should be **tiles** (Chrome-style), not a list.
5. "Top half black (content area) / bottom white" — not one cohesive window; needs a single
   window theme where everything fits, with arrows/scroll if the user has more than ~4
   profiles. ("Choose a Profile" heading is OK but could be better.)

## Requirements (from owner)

- A **distinct, designed launcher window** — small/centered, not full-screen, no browser
  chrome, clearly "pick something before the app launches."
- Use the app's **theme tokens** (dark `#1a1d23`, gold `#a67c00`, `HodosButton`) — a cohesive
  single-surface window, no empty black/white regions.
- **Hodos logo** (assets exist: `frontend/public/Hodos_Gold_Icon.svg` /
  `Hodos_Gold_Browser_Icon.svg`).
- **Profile tiles** in a responsive grid (avatar + name), Chrome-like, with an **Add profile**
  tile; **scroll or arrows** when there are more than fit.
- Keep the in-header dropdown (`isPickerWindow === false`) as the compact list — only the
  `isPickerWindow === true` branch is redesigned.

---

## Implementation plan

### A. C++ — make the launch picker a small centered window (`cef_browser_shell.cpp`)
Today `g_picker_mode` reuses the full browser window geometry. Change **only the
`g_picker_mode` branch** of the main-window creation to a **fixed, centered, non-maximized
window** (proposed ~**960×640**, centered on the work area; keep it non-resizable or a modest
min-size). The header browser already fills the window in picker mode, so the picker page
just needs the window to be launcher-sized. Leave the normal (`!g_picker_mode`) path
untouched. This is a startup-window change (CLAUDE.md inv #8) → gets its own small design
pass + adversarial review, and must not disturb the picker→`profiles_switch`→spawn→`WM_CLOSE`
flow or the update-gate picker accounting.

> macOS parity: the equivalent picker-window sizing lives in `cef_browser_shell_mac.mm`
> (`NSWindow`); mirror the dimensions there. Build/verify on the Mac.

### B. React — redesign the `isPickerWindow` layout (`ProfilePickerOverlayRoot.tsx`)
Add a dedicated window-mode render branch (the dropdown branch stays as-is):
- Centered card on the dark surface; **Hodos logo** top-center; heading ("Who's using Hodos?"
  or keep "Choose a profile").
- **Tile grid**: responsive, ~3–4 across, each tile ≈ 140×160 (avatar circle w/ color or
  image + name below, hover lift, selected/default markers). `+ Add profile` as the trailing
  tile (opens the existing create form, ideally as a small modal over the grid rather than
  inline-replacing a list row).
- **Overflow**: vertical scroll by default; optional left/right arrows or paging if the owner
  prefers horizontal paging for >4.
- Reuse existing theme tokens + `HodosButton`; keep native `<input>` for the create form
  (CEF rule). Keep the create/edit logic — only the window-mode presentation changes.

### Proposed look (ASCII mockup, window mode)
```
┌──────────────────────────────────────────────────────────┐
│                        [ Hodos logo ]                     │
│                     Who's using Hodos?                    │
│                                                           │
│     ┌────────┐   ┌────────┐   ┌────────┐   ┌────────┐     │
│     │ (WA)   │   │ (PE)   │   │ (JK)   │   │  +     │     │
│     │  Work  │   │Personal│   │  Jake  │   │  Add   │     │
│     └────────┘   └────────┘   └────────┘   └────────┘     │
│                                                           │
│              ‹  · · ·  ›   (arrows/scroll if > row)       │
└──────────────────────────────────────────────────────────┘
   960 × 640, centered, dark themed, gold accents — one surface
```

---

## Testing
- Dev: create 2–4 profiles → launch with no `--profile` → the launcher window is small,
  centered, themed, logo + tiles; selecting a tile launches that profile (existing spawn
  flow) and the launcher closes.
- >4 profiles → overflow scroll/arrows work.
- Add/edit/delete profile from the window still works (native inputs, CEF focus).
- In-header dropdown (the profile button) is unchanged and still a compact list.
- macOS parity build.
- Regression: normal (non-picker) launch window is unchanged.

## Non-goals
- No same-process refactor (`PROFILE_PICKER_SAME_PROCESS_PLAN.md` — now **SHELVED**).
- No change to profile persistence / `profiles.json`.
- Don't touch the `!g_picker_mode` window path.
