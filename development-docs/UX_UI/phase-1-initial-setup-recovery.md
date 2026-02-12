# Initial Setup/Recovery Interface Implementation Plan

## Overview

**Interface Type**: Modal or Full Shell Window
**Purpose**: Handle wallet creation and recovery for first-time users and users recovering wallets

**Status**: 📋 Planning Phase
**Last Updated**: 2026-01-27

---

## Interface Description

This interface allows users to:
- Create a new wallet (generate mnemonic, first address)
- Recover an existing wallet from mnemonic
- Recover an existing wallet from backup file
- Complete initial wallet setup (backup confirmation, security notices)

**Display Context**:
- Modal (recommended for overlay-style presentation)
- Full shell window (alternative for dedicated setup flow)

---

## Requirements

### Functional Requirements
- [ ] Detect when no wallet exists
- [ ] Present clear create/recover choice
- [ ] Generate secure mnemonic for new wallets
- [ ] Display mnemonic with backup instructions
- [ ] Validate mnemonic format for recovery
- [ ] Handle file import for backup recovery
- [ ] Confirm user has backed up mnemonic
- [ ] Integrate with existing wallet creation/recovery endpoints

### Non-Functional Requirements
- [ ] Secure display of sensitive data (mnemonic)
- [ ] Clear error messaging
- [ ] Progress indicators for recovery operations
- [ ] Accessible to screen readers
- [ ] Responsive design

---

## Frontend Implementation

### Component Structure

**Location**: `frontend/src/components/WalletSetupModal.tsx` (or similar)

**Type**: React Functional Component

**Props**:
```typescript
interface WalletSetupModalProps {
  open: boolean;
  onClose: () => void;
  onComplete: () => void; // Called when wallet is created/recovered
  mode?: 'modal' | 'fullscreen'; // Display mode
}
```

**State Management**:
- Current step in the setup flow
- User input (mnemonic, file selection)
- Loading states
- Error states
- Confirmation states

**UI Flow Steps**:
1. Initial choice: Create New Wallet / Recover Wallet
2. Create flow: Generate mnemonic → Display mnemonic → Backup confirmation → Complete
3. Recover flow: Choose method → Input mnemonic/file → Recovery process → Complete

### Integration Points

- **Wallet Button**: Triggers this interface when no wallet exists
- **Startup Flow**: May trigger on first launch (if implemented)
- **Bridge Methods**: Uses `window.hodosBrowser.wallet.*` methods

---

## CEF-Native Implementation

### Window Management

**If Modal**:
- Use existing overlay window system
- Route: `/wallet-setup` (or similar)
- Overlay HWND: `g_wallet_setup_overlay_hwnd` (to be added)

**If Full Shell Window**:
- Consider if needed or if modal is sufficient
- May require separate window class registration

### Message Handling

**New Messages** (if needed):
- `overlay_show_wallet_setup` - Open setup interface
- Additional messages for file selection, validation, etc.

### Process Isolation

- Follow existing overlay pattern (separate render process)

---

## Rust Wallet Backend

### Existing Endpoints (Review/Verify)

- `GET /wallet/status` - Check if wallet exists
- `POST /wallet/create` - Create new wallet
- `POST /wallet/recover` - Recover from mnemonic
- `POST /wallet/restore` - Restore from backup file

### Required Changes

- [ ] Verify endpoints return appropriate responses
- [ ] Ensure auto-create is disabled (see Startup Flow docs)
- [ ] Add validation for mnemonic format
- [ ] Add progress reporting for recovery operations

---

## Database Considerations

### Current Schema

- Wallet table stores wallet data
- Address table stores addresses
- No specific "setup state" tracking

### Potential Additions

- [ ] Wallet setup completion status
- [ ] First-run flags
- [ ] Setup timestamps
- [ ] Recovery metadata

**Decision Needed**: Do we need to track setup state in database, or is wallet existence sufficient?

---

## Triggers

### Primary Triggers

1. **Wallet Button Click** (when no wallet exists)
   - Location: `frontend/src/pages/MainBrowserView.tsx`
   - Current: Opens wallet overlay directly
   - Change: Check wallet status first, show setup if needed

2. **Startup Flow** (potential)
   - If implemented per Startup Flow docs
   - Check wallet on startup, trigger setup if needed

### Secondary Triggers

- Manual trigger from settings (if needed)
- Recovery from error states

---

## User Interaction Flow

### Create New Wallet Flow

```
1. User clicks Wallet button
   ↓
2. System detects no wallet exists
   ↓
3. Setup interface opens (Create/Recover choice)
   ↓
4. User selects "Create New Wallet"
   ↓
5. System generates mnemonic
   ↓
6. Mnemonic displayed with backup instructions
   ↓
7. User confirms backup (checkbox/button)
   ↓
8. "Continue" button enabled
   ↓
9. User clicks Continue
   ↓
10. Wallet created, setup complete
    ↓
11. Wallet overlay opens automatically
```

### Recover Wallet Flow (Mnemonic)

```
1. User clicks Wallet button
   ↓
2. System detects no wallet exists
   ↓
3. Setup interface opens (Create/Recover choice)
   ↓
4. User selects "Recover Wallet"
   ↓
5. User chooses "From Mnemonic"
   ↓
6. User pastes/types mnemonic
   ↓
7. System validates format
   ↓
8. User clicks "Recover"
   ↓
9. Recovery process runs (with progress)
   ↓
10. Wallet recovered, setup complete
    ↓
11. Wallet overlay opens automatically
```

### Recover Wallet Flow (File)

```
[Similar to mnemonic flow, but with file selection step]
```

---

## Design Considerations

**Reference**: [Design Principles](./helper-2-design-philosophy.md)

Key considerations:
- [ ] Security of mnemonic display
- [ ] Clear instructions for users
- [ ] Error handling and messaging
- [ ] Loading states during recovery
- [ ] Accessibility requirements

---

## Testing Requirements

### Unit Tests
- Component rendering
- State transitions
- Form validation
- Error handling

### Integration Tests
- Wallet creation flow
- Recovery flows
- Error scenarios
- Bridge communication

### User Acceptance Tests
- First-time user experience
- Recovery scenarios
- Error recovery
- Accessibility

---

## Dependencies

### External Dependencies
- Existing wallet creation/recovery endpoints
- Bridge methods for wallet operations
- Overlay window system

### Internal Dependencies
- Wallet status checking
- Mnemonic validation
- File import capabilities

---

## Coordination with Wallet Backup & Recovery Plan

Phase-1 (this document) is the **UX/UI** for create and recover flows. The **backend** for backup and recovery (file format, encryption, on-chain, sync) is defined in [WALLET_BACKUP_AND_RECOVERY_PLAN.md](../WALLET_BACKUP_AND_RECOVERY_PLAN.md).

### When to Build What

- **Phase B1 (local file export/import)** from the backup plan should be implemented **before or in parallel with** Phase-1 so that “Recover from backup file” has a real backend. The Phase-1 UI will call e.g. `POST /wallet/import` and `POST /wallet/export`; those are implemented in Phase B1.
- **Phase B2 (on-chain backup)** and **Phase B3 (cloud sync scaffolding)** can follow after Phase-1 and B1 are done.

We do **not** merge the two plans into one: the backup plan stays the single source of truth for data model, encryption, and recovery logic; Phase-1 stays the single source of truth for screens, flows, and triggers. Cross-reference only.

### MVP Scope (from backup plan; relevant to Phase-1)

- **Cloud sync**: Scaffolding only in MVP. No cloud implementation or testing. Plan for it; don’t build it in MVP.
- **Local file backup**: Plan and **test** local file export and recover in MVP. Phase-1 “Recover from file” must use the real B1 import path and be tested end-to-end.
- **External wallet import (TypeScript BSV/SDK)**: As part of **planning** for Phase-1, export a backup from another wallet built with the TypeScript BSV/SDK and mirror that format in our export/import (hopefully just JSON). Handle **camelCase (TS/JSON) ↔ snake_case (Rust/DB)** in file recovery so we can import their exports and they can import ours. During planning, obtain a sample export from the other wallet and use it to validate our import/export format.
- **On-chain backup**: Novel to our wallet; we will only test our own. Likely same JSON (subset) encrypted and stored (e.g. PushDrop token with self-counterparty, or OP_RETURN). **Compress** the data before putting it in the token/OP_RETURN to save space and cost; decompress after decrypt on recovery.

---

## Related Documentation

- [Startup Flow and Wallet Checks](./phase-0-startup-flow-and-wallet-checks.md) - Startup wallet detection
- [UI/UX Enhancement Guide](./helper-1-implementation-guide-checklist.md) - Frontend architecture
- [Design Principles](./helper-2-design-philosophy.md) - Design guidelines
- [Wallet Initialization Flow](./helper-1-implementation-guide-checklist.md#wallet-initialization-flow) - Detailed flow
- [Wallet Backup & Recovery Plan](../WALLET_BACKUP_AND_RECOVERY_PLAN.md) - Backup data model, local file, on-chain, cloud scaffolding

---

## Decisions (2026-02-11)

> **Encryption Key Decision (resolved)**
>
> - **On-chain backup encryption**: Derived from mnemonic via HKDF-SHA256. Anyone with the mnemonic can decrypt — this is intentional for mnemonic-only recovery.
> - **Local file backup encryption**: User-provided PIN/passphrase. The wallet creation flow should include a PIN creation step. This PIN is used when exporting/importing `.bsv-wallet` files.
> - **Implication for Phase 1 UI**: The "Create New Wallet" flow needs a PIN creation step after mnemonic display/confirmation. The "Recover from file" flow needs a PIN entry field.

> **File Picker in CEF Overlays**
>
> Phase 0 planning must test whether `<input type="file">` works in CEF overlay subprocesses. If not, a C++ bridge method (`window.hodosBrowser.openFileDialog()`) will be needed to open a native file dialog and return the selected path to the frontend. This must be resolved before implementing "Recover from file."

## Open Questions

1. ~~Modal vs Full Shell Window - which is preferred?~~ (Use modal/overlay — consistent with existing patterns)
2. Should we track setup completion in database?
3. What validation is needed for mnemonic format?
4. How to handle partial recovery scenarios?
5. Should recovery progress be cancellable?

---

## Implementation Notes

- This interface overlaps with existing Wallet Initialization Flow documentation
- Review existing commented-out wallet setup code in `App.tsx`
- Consider reusing patterns from HTTP Interceptor Flow for modals

---

**End of Document**
