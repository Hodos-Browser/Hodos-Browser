# Wallet Dashboard (Full Wallet) Implementation Plan

**Date**: March 6, 2026
**Project**: Hodos Browser
**Phase**: 4 - Full Wallet Experience
**Status**: 🏗️ Strategic Planning (Prioritizing Dashboard Layout)

---

## 1. Executive Summary

Phase 4 moves the wallet from a simple "Quick Access" overlay (Light Wallet) to a first-class browser experience. We are renaming "Advanced Wallet" to **"Wallet"** and centering the experience on a comprehensive **Dashboard** with a left-side navigation sidebar.

The goal is to provide a beautiful, branded (Gold/Black), and identity-first interface that handles both BRC-100 (Identity) and Legacy (ECC/UTXO) operations.

---

## 2. Interface & Layout (Dashboard-First)

### 2.1 The "Gold & Black" Sidebar (Left)
*   **Branding**: Fixed Sidebar on the left (`width: 260px`).
*   **Theme**: Dark (`#000000`) with Gold (`#a67c00`) highlights for the active state.
*   **Logo**: The "Hodos Gold Wallet" icon in the top-left.
*   **Navigation Items**:
    1.  **Dashboard** (Home) - [Icon: LayoutDashboard]
    2.  **Transactions** - [Icon: History]
    3.  **Addresses** - [Icon: MapPin]
    4.  **Certificates** (BRC-100) - [Icon: ShieldCheck]
    5.  **Vault** (Security & Keys) - [Icon: Lock]

### 2.2 The Dashboard (Homepage) Layout
The Dashboard is the "at-a-glance" command center.

#### **Section A: Balance & Quick Actions (Top)**
*   **Card**: Large, high-contrast card showing `Total BSV Balance` and its `USD Equivalent`.
*   **Buttons**: Large [Send] and [Receive] buttons with Gold accents.

#### **Section B: Dual-Identity QR Widget (Center-Right)**
*   **Identity QR (Static)**: Your BRC-100 **Identity Key (BIP32/BRC-42 pubkey)** QR. This is your "Public Web3 ID."
*   **Legacy QR (Dynamic)**: A QR for a **freshly generated ECC address**.
    *   **Action**: A prominent "Generate New" button to cycle to the next unused address in the derivation path (Privacy-first).
    *   **Label**: Clearly marked as "Single-Use Payment Address."

#### **Section C: Activity & Assets (Bottom-Left)**
*   **Recent Activity**: A list of the last 5 transactions (Filtered by Type: Payment, BRC-100, etc.).
*   **Asset Summary**: (Table) BSV Balance, Total Certificates Count, and Active Sessions.

---

## 3. "The Vault" (Security & Key Management)

This is the high-security section for sensitive operations (Phase 4 final sprint).
*   **Identity Management**: View/Copy the BRC-42 Identity Pubkey and Mnemonic (requires PIN).
*   **Key Export**: Export individual Private Keys for specific ECC addresses (with "Nuclear" warnings).
*   **Data Management**:
    *   **Export Wallet**: Generate encrypted `.hodos-wallet` backup.
    *   **Delete Wallet**: The final "Nuclear Option" with triple confirmation and PIN verification.

---

## 4. Technical Component Architecture

### **Layout Wrapper**
*   `src/layouts/WalletLayout.tsx`: Handles the Sidebar and the Responsive Content area.

### **Pages**
*   `src/pages/wallet/DashboardPage.tsx`: The primary entry point.
*   `src/pages/wallet/TransactionsPage.tsx`: Full searchable/filterable transaction list.
*   `src/pages/wallet/AddressesPage.tsx`: Management of all derived ECC addresses and labels.
*   `src/pages/wallet/CertificatesPage.tsx`: BRC-100 certificate browser.
*   `src/pages/wallet/VaultPage.tsx`: Security and Key management.

---

## 5. Implementation Roadmap (Sprint 4.1: Dashboard)

1.  **[ ] Phase 4.1a: Layout Shell**
    *   Create the `WalletLayout` with the Sidebar.
    *   Implement "Gold on Black" branding for the navigation.
    *   Setup the `/wallet/*` route structure in `App.tsx`.

2.  **[ ] Phase 4.1b: The Dashboard Core**
    *   Integrate `BalanceCache` to show total BSV + USD.
    *   Implement the **Dual QR Widget** (Static Identity vs. Dynamic Legacy).
    *   Implement "Quick Action" buttons for Send/Receive.

3.  **[ ] Phase 4.1c: Transaction Feed**
    *   Connect the Dashboard's "Recent Activity" to the Rust backend history.

---

## 6. UX Best Practices & Considerations

*   **Pixels vs. Percentages**: The Sidebar will be a fixed `rem` or `px` width, while the main dashboard uses a flexible `CSS Grid` (max 3 columns) to look great on 1080p and 4K.
*   **Progressive Disclosure**: Detailed UTXO data is hidden in "Advanced" sub-menus on the Addresses page, keeping the Dashboard clean.
*   **Feedback**: All buttons must have a `100ms` visual press feedback to feel responsive.

---

*This document replaces the old `phase-4-full-wallet.md`. All future Wallet Dashboard tasks should be tracked here.*
