# Panels Components (Legacy)
> Legacy wallet panel and backup modal — superseded by the overlay architecture.

## Overview

These components are **no longer imported or used** anywhere in the active codebase. They represent an earlier iteration of the wallet UI that used MUI `Drawer` and `Modal` patterns before the project migrated to the CEF overlay subprocess model (see root CLAUDE.md "UI Architecture Rules").

**Do not build on these components.** Use the current overlay-based equivalents instead.

## Status

| Component | Status | Replaced By |
|-----------|--------|-------------|
| `BackupModal` | **Dead code** — not imported anywhere | `pages/BackupOverlayRoot.tsx` (CEF overlay subprocess) |
| `WalletPanelContent` | **Dead code** — only imported by `WalletPanelLayout` (also dead) | `components/wallet/DashboardTab.tsx` + `components/WalletPanel.tsx` → `pages/WalletPanelPage.tsx` |
| `WalletPanelLayout` | **Dead code** — not imported anywhere | `pages/WalletPanelPage.tsx` (CEF overlay subprocess) |

## Component Details

### BackupModal (`BackupModal.tsx`, 217 lines)
- **Purpose:** MUI `Modal` dialog prompting user to back up their seed phrase.
- **Props:** `open: boolean`, `onClose: () => void`, `wallet: { address, mnemonic, version, backedUp }`
- **Behavior:** Shows wallet address and version as read-only fields. Mnemonic is hidden behind a "Show Seed Phrase" toggle (`Collapse`). User must check a confirmation checkbox before the "Done" button enables. Clicking "Done" sends `mark_wallet_backed_up` IPC via `window.cefMessage.send()` and closes.
- **Contains debug artifacts:** A fixed-position blue debug div that renders unconditionally.
- **Why replaced:** MUI `Modal` doesn't work well in CEF overlay subprocesses. The new `BackupOverlayRoot.tsx` uses native `<input>` elements and runs as its own CEF subprocess window with proper focus handling.

### WalletPanelContent (`WalletPanelContent.tsx`, 364 lines)
- **Purpose:** Main wallet dashboard showing balance, send/receive actions, and a navigation grid.
- **Exports:** `WalletPanel` (default export, despite filename saying "Content")
- **Hooks used:** `useBalance()` (balance, USD value, BSV price, refresh), `useAddress()` (address generation and copy)
- **Sections:**
  - **Balance display** — BSV amount (satoshis → BSV with 8 decimals) and USD equivalent, with refresh button
  - **Action buttons** — "Receive" (generates address via `generateAndCopy()`, copies to clipboard) and "Send" (toggles `TransactionForm`)
  - **Navigation grid** — 6 buttons: Certificates, History, Settings, Tokens, Baskets, Exchange (all just trigger `clearAllStates()` with click animation, no actual navigation)
  - **Dynamic content area** — conditionally renders: `TransactionForm`, receive address display, or transaction result (success/error with WhatsOnChain link)
- **Key dependency:** `TransactionForm` from `../TransactionForm` and `TransactionResponse` type from `../../types/transaction`
- **Why replaced:** The current wallet UI (`components/wallet/`) uses a tabbed sidebar layout (`WalletSidebar`) with dedicated tab components (`DashboardTab`, `ActivityTab`, `CertificatesTab`, `SettingsTab`, `ApprovedSitesTab`) and runs inside the wallet overlay subprocess.

### WalletPanelLayout (`WalletPanelLayout.tsx`, 54 lines)
- **Purpose:** MUI `Drawer` wrapper that slides in from the right (36% width) with Hodos gold (`#a67c00`) background. Contains a header with wallet icon and close button, then renders `WalletPanel` (from `WalletPanelContent`).
- **Props:** `open: boolean`, `onClose: () => void`
- **Why replaced:** MUI `Drawer` approach was replaced by dedicated CEF overlay windows (`WalletPanelPage.tsx` routed via `App.tsx` at `/wallet`).

## Cleanup Candidate

All three files can be safely deleted. They have:
- No imports from any active code
- No route references in `App.tsx`
- Been fully replaced by the overlay architecture

## Related

- **Current wallet UI:** `../wallet/CLAUDE.md`
- **Current backup overlay:** `../../pages/BackupOverlayRoot.tsx`
- **Current wallet overlay page:** `../../pages/WalletPanelPage.tsx`
- **Overlay architecture rules:** Root `CLAUDE.md` → "UI Architecture Rules"
- **CEF input patterns:** Root `CLAUDE.md` → "CEF Input Patterns"
