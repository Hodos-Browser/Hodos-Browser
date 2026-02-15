# UI/UX Enhancement Implementation Guide

## Purpose

This document provides comprehensive documentation for AI assistants specializing in frontend UI/UX development to understand the Hodos Browser frontend architecture, ensuring new UI components integrate seamlessly with the existing codebase without breaking functionality.

**Document Version:** 1.0
**Last Updated:** 2026-01-27
**Target Audience:** Frontend UI/UX AI Assistants

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [File Structure](#file-structure)
3. [Technology Stack](#technology-stack)
4. [React Component Architecture](#react-component-architecture)
5. [TypeScript Type System](#typescript-type-system)
6. [CSS & Styling](#css--styling)
7. [Communication Bridges](#communication-bridges)
8. [Window Management System](#window-management-system)
9. [Hooks & State Management](#hooks--state-management)
10. [Pages & Routing](#pages--routing)
11. [Critical Integration Points](#critical-integration-points)
12. [Development Guidelines](#development-guidelines)

---

## Architecture Overview

### Three-Layer Architecture

```
┌─────────────────────────────────────────┐
│      React Frontend (TypeScript)       │
│  • Components, Pages, Hooks            │
│  • Material-UI Components              │
│  • React Router                        │
└──────────────┬──────────────────────────┘
               │
               │ window.hodosBrowser.*
               │ window.cefMessage.send()
               │
┌──────────────▼──────────────────────────┐
│      CEF-Native Bridge (C++)           │
│  • Process-per-overlay architecture    │
│  • V8 JavaScript injection             │
│  • Message routing                     │
│  • Window management (HWND)             │
└──────────────┬──────────────────────────┘
               │
               │ HTTP (localhost:3301)
               │
┌──────────────▼──────────────────────────┐
│      Rust Wallet Backend                │
│  • Actix-web HTTP server               │
│  • Wallet operations                   │
│  • Database (wallet.db)                │
│  • Blockchain integration              │
└─────────────────────────────────────────┘
```

### Key Architectural Principles

1. **Process Isolation**: Each overlay window runs in a separate CEF render process
2. **Message-Based Communication**: All frontend-backend communication uses message passing
3. **Type Safety**: Full TypeScript coverage with strict type definitions
4. **Separation of Concerns**: Clear boundaries between UI, bridge, and backend layers

---

## File Structure

### Frontend Directory Layout

```
frontend/
├── src/
│   ├── main.tsx                    # React entry point
│   ├── App.tsx                     # Root component with routing
│   ├── App.css                     # Global app styles
│   ├── index.css                   # Global reset & base styles
│   │
│   ├── bridge/                     # Communication bridges
│   │   ├── initWindowBridge.ts     # Main window.hodosBrowser API setup
│   │   └── brc100.ts               # BRC-100 protocol bridge
│   │
│   ├── components/                 # Reusable React components
│   │   ├── panels/                 # Panel layouts (overlays)
│   │   │   ├── WalletPanelLayout.tsx
│   │   │   ├── SettingsPanelLayout.tsx
│   │   │   ├── WalletPanelContent.tsx
│   │   │   └── BackupModal.tsx
│   │   ├── layout/                 # Layout components
│   │   ├── AddressManager.tsx
│   │   ├── BalanceDisplay.tsx
│   │   ├── BRC100AuthModal.tsx
│   │   ├── HistoryPanel.tsx
│   │   ├── SettingsMenu.tsx
│   │   ├── TabBar.tsx
│   │   ├── TabComponent.tsx
│   │   ├── TransactionForm.tsx
│   │   ├── TransactionHistory.tsx
│   │   ├── TransactionComponents.css
│   │   └── WalletPanel.css
│   │
│   ├── hooks/                      # Custom React hooks
│   │   ├── useAddress.ts
│   │   ├── useBalance.ts
│   │   ├── useBitcoinBrowser.ts
│   │   ├── useHistory.ts
│   │   ├── useHodosBrowser.ts      # Main browser API hook
│   │   ├── useKeyboardShortcuts.ts
│   │   ├── useTabManager.ts
│   │   ├── useTransaction.ts
│   │   └── useWallet.ts            # Wallet operations hook
│   │
│   ├── pages/                      # Route-level page components
│   │   ├── MainBrowserView.tsx     # Main browser UI (header)
│   │   ├── HistoryPage.tsx
│   │   ├── SendPage.tsx
│   │   ├── WalletOverlayRoot.tsx   # Wallet overlay page
│   │   ├── SettingsOverlayRoot.tsx # Settings overlay page
│   │   ├── BackupOverlayRoot.tsx   # Backup overlay page
│   │   └── BRC100AuthOverlayRoot.tsx
│   │
│   └── types/                      # TypeScript type definitions
│       ├── address.d.ts
│       ├── history.d.ts
│       ├── hodosBrowser.d.ts        # Main API types
│       ├── identity.d.ts
│       ├── TabTypes.ts
│       └── transaction.d.ts
│
├── package.json                    # Dependencies & scripts
├── vite.config.ts                  # Vite build configuration
├── tsconfig.json                   # TypeScript configuration
└── index.html                      # HTML entry point
```

### Key File Responsibilities

- **`main.tsx`**: Initializes React, sets up BrowserRouter, imports bridge initialization
- **`App.tsx`**: Defines routes, manages global state (BRC-100 auth modal), handles initialization
- **`bridge/initWindowBridge.ts`**: Sets up `window.hodosBrowser.*` API methods
- **`bridge/brc100.ts`**: BRC-100 protocol bridge implementation
- **`pages/*OverlayRoot.tsx`**: Root components for overlay windows (wallet, settings, backup)
- **`components/panels/*.tsx`**: Panel layout components used in overlays

---

## Technology Stack

### Core Dependencies

```json
{
  "dependencies": {
    "@emotion/react": "^11.14.0",        // CSS-in-JS runtime
    "@emotion/styled": "^11.14.0",      // Styled components
    "@mui/icons-material": "^7.1.1",   // Material-UI icons
    "@mui/material": "^7.1.1",         // Material-UI component library
    "react": "^19.1.0",                // React framework
    "react-dom": "^19.1.0",            // React DOM renderer
    "react-router-dom": "^7.6.1"       // Client-side routing
  },
  "devDependencies": {
    "@vitejs/plugin-react": "^4.4.1",  // Vite React plugin
    "typescript": "~5.8.3",             // TypeScript compiler
    "vite": "^6.3.5"                   // Build tool & dev server
  }
}
```

### Build System

- **Vite**: Fast build tool and dev server
- **TypeScript**: Type-safe JavaScript
- **ESLint**: Code linting
- **Development Server**: `http://127.0.0.1:5137` (configured in `vite.config.ts`)

### UI Framework

- **Material-UI (MUI) v7**: Primary component library
- **Emotion**: CSS-in-JS styling (used by MUI)
- **React Router v7**: Client-side routing

### Styling Approach

1. **Material-UI `sx` prop**: Primary styling method (inline styles with theme support)
2. **CSS Modules**: Component-specific stylesheets (`.css` files)
3. **Global CSS**: `index.css` and `App.css` for global styles

---

## React Component Architecture

### Component Hierarchy

```
App.tsx (Root)
├── Routes (React Router)
│   ├── MainBrowserView (path="/")
│   │   ├── TabBar
│   │   └── Toolbar (navigation, address bar, buttons)
│   ├── HistoryPage (path="/history")
│   ├── WalletOverlayRoot (path="/wallet")
│   │   └── WalletPanelLayout
│   ├── SettingsOverlayRoot (path="/settings")
│   │   └── SettingsPanelLayout
│   ├── BackupOverlayRoot (path="/backup")
│   └── BRC100AuthOverlayRoot (path="/brc100-auth")
└── BRC100AuthModal (global modal)
```

### Component Patterns

#### 1. Page Components (`pages/`)
- Route-level components that handle page-level logic
- Manage overlay state and lifecycle
- Set up window event handlers
- Example: `WalletOverlayRoot.tsx`

```typescript
const WalletOverlayRoot: React.FC = () => {
  const [walletOpen, setWalletOpen] = useState(true);

  useEffect(() => {
    // Set up window triggers
    window.triggerPanel = (panelName: string) => {
      if (panelName === 'wallet') setWalletOpen(true);
    };
  }, []);

  return <WalletPanelLayout open={walletOpen} onClose={handleClose} />;
};
```

#### 2. Panel Components (`components/panels/`)
- Layout components for overlay windows
- Handle panel-specific UI and interactions
- Use Material-UI components for structure
- Example: `WalletPanelLayout.tsx`, `SettingsPanelLayout.tsx`

#### 3. Feature Components (`components/`)
- Reusable UI components
- Business logic components (e.g., `TransactionForm`, `BalanceDisplay`)
- Use custom hooks for data fetching

#### 4. Layout Components (`components/layout/`)
- Shared layout structures
- Navigation components
- Header/footer components

### Component Communication Patterns

1. **Props Down**: Parent → Child via props
2. **Hooks Up**: Child → Parent via custom hooks that call parent callbacks
3. **Global State**: `window.hodosBrowser.*` for backend communication
4. **Event Handlers**: `window.triggerPanel`, `window.onAddressGenerated`, etc.

---

## TypeScript Type System

### Global Window Types

Defined in `src/types/hodosBrowser.d.ts`:

```typescript
interface Window {
  hodosBrowser: {
    // Wallet API
    wallet: {
      getStatus(): Promise<{ exists: boolean; needsBackup: boolean }>;
      create(): Promise<WalletCreateResponse>;
      load(): Promise<WalletLoadResponse>;
      getInfo(): Promise<WalletInfo>;
      generateAddress(): Promise<AddressData>;
      getCurrentAddress(): Promise<AddressData>;
      getAddresses(): Promise<AddressData[]>;
      markBackedUp(): Promise<{ success: boolean }>;
      getBalance(): Promise<{ balance: number }>;
      sendTransaction(data: any): Promise<TransactionResponse>;
      getTransactionHistory(): Promise<TransactionHistoryEntry[]>;
    };

    // Address API
    address: {
      generate(): Promise<AddressData>;
    };

    // Navigation API
    navigation: {
      navigate(path: string): void;
    };

    // Overlay API
    overlay: {
      show(): void;
      hide(): void;
      close(): void;
      toggleInput(enable: boolean): void;
    };

    // History API
    history: {
      get(params?: HistoryGetParams): HistoryEntry[];
      search(params: HistorySearchParams): HistoryEntry[];
      delete(url: string): boolean;
      clearAll(): boolean;
      clearRange(params: ClearRangeParams): boolean;
    };

    // BRC-100 API (injected by C++ backend)
    brc100?: {
      status(): Promise<BRC100Status>;
      isAvailable(): Promise<boolean>;
      // ... other BRC-100 methods
    };
  };

  // CEF message bridge
  cefMessage?: {
    send(message: string, args: any[]): void;
  };

  // Event handlers (set by bridge, called by C++ backend)
  onAddressGenerated?: (data: AddressData) => void;
  onAddressError?: (error: string) => void;
  onSendTransactionResponse?: (data: TransactionResponse) => void;
  onSendTransactionError?: (error: string) => void;
  onGetBalanceResponse?: (data: { balance: number }) => void;
  onGetBalanceError?: (error: string) => void;
  // ... more event handlers

  // Panel trigger (set by overlay roots)
  triggerPanel?: (panelName: string) => void;

  // System flags
  allSystemsReady?: boolean;
  __overlayReady?: boolean;
}
```

### Type Definitions Location

- **`types/hodosBrowser.d.ts`**: Main API types
- **`types/address.d.ts`**: Address-related types
- **`types/transaction.d.ts`**: Transaction types
- **`types/history.d.ts`**: History types
- **`types/identity.d.ts`**: Identity types
- **`types/TabTypes.ts`**: Tab management types

### Type Safety Guidelines

1. **Always use types**: Import types from `types/` directory
2. **Extend global types**: Add to `Window` interface in `hodosBrowser.d.ts`
3. **Type assertions**: Use sparingly, prefer proper typing
4. **Promise types**: All async operations return typed Promises

---

## CSS & Styling

### Styling Methods

#### 1. Material-UI `sx` Prop (Preferred)

```typescript
<Box
  sx={{
    width: '100%',
    height: '100%',
    display: 'flex',
    flexDirection: 'column',
    bgcolor: '#ffffff',
    '&:hover': {
      bgcolor: '#f5f5f5',
    },
  }}
>
```

**Advantages:**
- Theme-aware
- Type-safe
- Responsive breakpoints
- Pseudo-selectors support

#### 2. Component-Specific CSS Files

Located in `components/`:
- `TransactionComponents.css`
- `WalletPanel.css`

**Usage:**
```typescript
import './WalletPanel.css';

<div className="wallet-panel">
```

#### 3. Global CSS

- **`index.css`**: Global reset, base styles, root variables
- **`App.css`**: App-level styles

### CSS Architecture Principles

1. **Global Reset**: `index.css` resets all margins/padding for CEF browser
2. **Component Scoping**: Use CSS modules or scoped class names
3. **Theme Consistency**: Use MUI theme colors when possible
4. **Responsive Design**: Use MUI breakpoints (`sx` prop)

### Current Color Scheme

- **Background**: `#ffffff` (white)
- **Text**: `rgba(0, 0, 0, 0.87)` (dark gray)
- **Secondary Text**: `rgba(0, 0, 0, 0.6)` (medium gray)
- **Borders**: `rgba(0, 0, 0, 0.12)` (light gray)
- **Hover**: `rgba(0, 0, 0, 0.04)` (very light gray)
- **Primary Accent**: `#1a73e8` (blue, for focus states)

### Important CSS Constraints

```css
/* index.css - Critical for CEF browser */
html, body {
  margin: 0 !important;
  padding: 0 !important;
  width: 100%;
  height: 100%;
  overflow: hidden;
  position: fixed;
}

#root {
  width: 100%;
  height: 100%;
  margin: 0 !important;
  padding: 0 !important;
  overflow: hidden;
  position: absolute;
}
```

**⚠️ DO NOT MODIFY** these base styles without understanding CEF browser constraints.

---

## Communication Bridges

### Bridge Architecture

```
React Component
    ↓
window.hodosBrowser.wallet.getBalance()
    ↓
bridge/initWindowBridge.ts (sets up Promise wrapper)
    ↓
window.cefMessage.send('get_balance', [])
    ↓
CEF Render Process Handler (C++)
    ↓
Browser Process Handler (C++)
    ↓
Rust Wallet HTTP Server (localhost:3301)
    ↓
Response callback: window.onGetBalanceResponse(data)
    ↓
Promise resolves in React component
```

### Main Bridge: `initWindowBridge.ts`

**Purpose**: Sets up `window.hodosBrowser.*` API methods that wrap CEF message passing.

**Pattern**: All methods return Promises and use callback handlers:

```typescript
window.hodosBrowser.wallet.getBalance = () => {
  return new Promise((resolve, reject) => {
    // Set up response handlers
    window.onGetBalanceResponse = (data: any) => {
      resolve(data);
      delete window.onGetBalanceResponse;
      delete window.onGetBalanceError;
    };

    window.onGetBalanceError = (error: string) => {
      reject(new Error(error));
      delete window.onGetBalanceResponse;
      delete window.onGetBalanceError;
    };

    // Send message to C++ backend
    window.cefMessage?.send('get_balance', []);
  });
};
```

### BRC-100 Bridge: `brc100.ts`

**Purpose**: Type-safe wrapper for BRC-100 protocol methods.

**Pattern**: Singleton class that calls `window.hodosBrowser.brc100.*` methods (injected by C++).

### Message Types

**Frontend → Backend** (via `cefMessage.send()`):
- `'navigate'` - Navigate webview
- `'overlay_show_wallet'` - Show wallet overlay
- `'overlay_show_settings'` - Show settings overlay
- `'overlay_show_backup'` - Show backup overlay
- `'overlay_close'` - Close overlay window
- `'address_generate'` - Generate new address
- `'get_balance'` - Get wallet balance
- `'send_transaction'` - Send transaction
- `'get_transaction_history'` - Get transaction history
- `'wallet_status_check'` - Check wallet status
- `'create_wallet'` - Create new wallet
- `'load_wallet'` - Load existing wallet
- `'get_wallet_info'` - Get wallet information
- `'get_current_address'` - Get current address
- `'get_addresses'` - Get all addresses
- `'mark_wallet_backed_up'` - Mark wallet as backed up

**Backend → Frontend** (via callback handlers):
- `window.onAddressGenerated(data)`
- `window.onAddressError(error)`
- `window.onGetBalanceResponse(data)`
- `window.onGetBalanceError(error)`
- `window.onSendTransactionResponse(data)`
- `window.onSendTransactionError(error)`
- `window.onGetTransactionHistoryResponse(data)`
- `window.onGetTransactionHistoryError(error)`
- `window.onWalletStatusResponse(data)`
- `window.onWalletStatusError(error)`
- `window.onCreateWalletResponse(data)`
- `window.onCreateWalletError(error)`
- `window.onLoadWalletResponse(data)`
- `window.onLoadWalletError(error)`
- `window.onGetWalletInfoResponse(data)`
- `window.onGetWalletInfoError(error)`
- `window.onGetCurrentAddressResponse(data)`
- `window.onGetCurrentAddressError(error)`
- `window.onGetAddressesResponse(data)`
- `window.onGetAddressesError(error)`
- `window.onMarkWalletBackedUpResponse(data)`
- `window.onMarkWalletBackedUpError(error)`

### Adding New Bridge Methods

1. **Add to `initWindowBridge.ts`**:
   ```typescript
   window.hodosBrowser.wallet.newMethod = () => {
     return new Promise((resolve, reject) => {
       window.onNewMethodResponse = (data: any) => {
         resolve(data);
         delete window.onNewMethodResponse;
         delete window.onNewMethodError;
       };

       window.onNewMethodError = (error: string) => {
         reject(new Error(error));
         delete window.onNewMethodResponse;
         delete window.onNewMethodError;
       };

       window.cefMessage?.send('new_method', []);
     });
   };
   ```

2. **Add types to `types/hodosBrowser.d.ts`**:
   ```typescript
   wallet: {
     // ... existing methods
     newMethod(): Promise<NewMethodResponse>;
   };
   ```

3. **Add callback handler types**:
   ```typescript
   onNewMethodResponse?: (data: NewMethodResponse) => void;
   onNewMethodError?: (error: string) => void;
   ```

4. **Implement in C++ backend** (outside frontend scope, but must match)

---

## Window Management System

### Window Types

#### 1. Main Browser Window (Header)
- **Route**: `/` (MainBrowserView)
- **Purpose**: Browser UI (tabs, address bar, navigation buttons)
- **HWND**: `g_header_hwnd` (created at startup)
- **CEF Handler**: `SimpleHandler("header")`

#### 2. WebView Window
- **Purpose**: Renders web content
- **HWND**: `g_webview_hwnd` (created at startup)
- **CEF Handler**: `SimpleHandler("webview")`
- **Not React**: This is a native CEF browser for web content

#### 3. Overlay Windows (Process-Per-Overlay)
- **Routes**: `/wallet`, `/settings`, `/backup`, `/brc100-auth`
- **Purpose**: Modal panels, settings, wallet UI
- **HWND**: Created dynamically (`g_wallet_overlay_hwnd`, etc.)
- **CEF Handler**: `SimpleHandler("overlay")`, `SimpleHandler("wallet")`, etc.
- **Process Isolation**: Each overlay runs in separate CEF render process

### Overlay Window Lifecycle

1. **Creation**: Frontend sends `cefMessage.send('overlay_show_wallet', [])`
2. **C++ Backend**: Creates HWND, CEF browser, loads React app at route
3. **React Mount**: Overlay root component mounts (e.g., `WalletOverlayRoot`)
4. **Panel Display**: Component renders panel layout
5. **Close**: User clicks close → `cefMessage.send('overlay_close', [])` → C++ destroys HWND

### Opening Overlays

**From Main Browser View:**
```typescript
// Wallet button click
window.cefMessage?.send('overlay_show_wallet', []);
window.hodosBrowser.overlay.toggleInput(true);

// Settings button click
window.cefMessage?.send('overlay_show_settings', []);
window.hodosBrowser.overlay.toggleInput(true);
```

**From Code:**
```typescript
// Programmatic overlay opening
window.hodosBrowser.overlay.show(); // Opens settings by default
```

### Closing Overlays

**From Overlay Component:**
```typescript
const handleClose = () => {
  window.cefMessage?.send('overlay_close', []);
};
```

### Window Positioning

- **Overlays**: Positioned relative to main window
- **Managed by C++**: Frontend doesn't control window position
- **Auto-repositioning**: C++ handles window movement/resizing

---

## Hooks & State Management

### Custom Hooks Location

All hooks in `src/hooks/`:

### Key Hooks

#### `useWallet.ts`
**Purpose**: Wallet operations and state management

**Methods:**
- `checkWalletStatus()` - Check if wallet exists and needs backup
- `createWallet()` - Create new wallet
- `loadWallet()` - Load existing wallet
- `getWalletInfo()` - Get wallet information
- `generateAddress()` - Generate new address
- `getCurrentAddress()` - Get current address
- `markBackedUp()` - Mark wallet as backed up

**State:**
- `address: string | null`
- `mnemonic: string | null`
- `isInitialized: boolean`
- `backedUp: boolean`
- `version: string | null`

#### `useHodosBrowser.ts`
**Purpose**: Browser navigation and address generation

**Methods:**
- `getIdentity()` - Get identity (legacy, may be deprecated)
- `markBackedUp()` - Mark as backed up
- `generateAddress()` - Generate address (handles overlay vs main browser)
- `navigate(path)` - Navigate webview
- `goBack()` - Browser back
- `goForward()` - Browser forward
- `reload()` - Reload page

#### `useTabManager.ts`
**Purpose**: Tab management for browser

**State:**
- `tabs: Tab[]`
- `activeTabId: number`
- `isLoading: boolean`

**Methods:**
- `createTab(url?)` - Create new tab
- `closeTab(tabId)` - Close tab
- `switchToTab(tabId)` - Switch to tab
- `nextTab()` - Next tab
- `prevTab()` - Previous tab
- `switchToTabByIndex(index)` - Switch by index
- `closeActiveTab()` - Close active tab

#### `useBalance.ts`
**Purpose**: Wallet balance management

#### `useTransaction.ts`
**Purpose**: Transaction operations

#### `useHistory.ts`
**Purpose**: Browser history management

#### `useAddress.ts`
**Purpose**: Address management

#### `useKeyboardShortcuts.ts`
**Purpose**: Keyboard shortcut handling

**Usage:**
```typescript
useKeyboardShortcuts({
  onNewTab: createTab,
  onCloseTab: closeActiveTab,
  onNextTab: nextTab,
  onPrevTab: prevTab,
  onSwitchToTab: switchToTabByIndex,
  onFocusAddressBar: () => addressBarRef.current?.focus(),
  onReload: reload,
});
```

### State Management Pattern

**No Global State Library**: React's built-in state management only
- `useState` for component state
- `useEffect` for side effects
- Custom hooks for shared logic
- `window.hodosBrowser.*` for backend state (read-only from frontend)

---

## Pages & Routing

### Routing Setup

**Location**: `App.tsx`

**Router**: React Router v7 (`BrowserRouter`)

### Routes

```typescript
<Routes>
  <Route path="/" element={<MainBrowserView />} />
  <Route path="/history" element={<HistoryPage />} />
  <Route path="/settings" element={<SettingsOverlayRoot />} />
  <Route path="/wallet" element={<WalletOverlayRoot />} />
  <Route path="/backup" element={<BackupOverlayRoot />} />
  <Route path="/brc100-auth" element={<BRC100AuthOverlayRoot />} />
</Routes>
```

### Route Types

1. **Main Routes** (`/`, `/history`): Regular pages in main browser
2. **Overlay Routes** (`/wallet`, `/settings`, `/backup`, `/brc100-auth`): Overlay windows

### Page Components

#### `MainBrowserView.tsx`
- **Route**: `/`
- **Purpose**: Main browser UI
- **Features**: Tab bar, navigation toolbar, address bar, action buttons
- **Components**: `TabBar`, `Toolbar` (MUI), navigation buttons

#### `HistoryPage.tsx`
- **Route**: `/history`
- **Purpose**: Browser history display
- **Components**: `HistoryPanel`

#### `WalletOverlayRoot.tsx`
- **Route**: `/wallet`
- **Purpose**: Wallet overlay window root
- **Components**: `WalletPanelLayout`
- **Lifecycle**: Sets up `window.triggerPanel` handler

#### `SettingsOverlayRoot.tsx`
- **Route**: `/settings`
- **Purpose**: Settings overlay window root
- **Components**: `SettingsPanelLayout`, `BRC100AuthModal`
- **Lifecycle**: Handles BRC-100 auth requests

#### `BackupOverlayRoot.tsx`
- **Route**: `/backup`
- **Purpose**: Wallet backup overlay
- **Components**: `BackupModal`

#### `BRC100AuthOverlayRoot.tsx`
- **Route**: `/brc100-auth`
- **Purpose**: BRC-100 authentication overlay
- **Components**: `BRC100AuthModal`

### Navigation

**Programmatic Navigation:**
```typescript
// Navigate webview (not React router)
window.hodosBrowser.navigation.navigate('https://example.com');

// Navigate React route (for overlays)
// Use React Router's useNavigate() hook
```

---

## Critical Integration Points

### ⚠️ DO NOT BREAK

These are critical integration points that must be preserved:

#### 1. Window Object APIs

**`window.hodosBrowser.*`**: Must exist and match type definitions
- Used by all components for backend communication
- Injected by C++ backend, wrapped by `initWindowBridge.ts`
- **DO NOT** modify method signatures without updating C++ backend

**`window.cefMessage.send()`**: Must be available
- Used for all CEF message passing
- Injected by C++ render process handler
- **DO NOT** change message format without updating C++ backend

#### 2. Callback Handlers

**`window.on*Response` / `window.on*Error`**: Must match C++ expectations
- Set by bridge methods in `initWindowBridge.ts`
- Called by C++ backend via `ExecuteJavaScript()`
- **DO NOT** change callback names or signatures

#### 3. Overlay Window Triggers

**`window.triggerPanel(panelName)`**: Used by C++ to trigger panels
- Set by overlay root components (`*OverlayRoot.tsx`)
- Called by C++ when overlay loads
- **DO NOT** remove or change signature

#### 4. Route Structure

**Overlay routes**: Must match C++ overlay window creation
- `/wallet` → Wallet overlay
- `/settings` → Settings overlay
- `/backup` → Backup overlay
- `/brc100-auth` → BRC-100 auth overlay
- **DO NOT** change route paths without updating C++ backend

#### 5. CSS Constraints

**Global reset in `index.css`**: Critical for CEF browser
- `html, body { overflow: hidden; position: fixed; }`
- `#root { overflow: hidden; position: absolute; }`
- **DO NOT** modify without understanding CEF browser constraints

#### 6. Process Isolation

**Overlay windows**: Each runs in separate CEF render process
- Direct V8 function calls work in overlays
- Message passing required for main browser
- **DO NOT** assume shared state between windows

---

## Development Guidelines

### Adding New UI Components

1. **Create component file** in `components/` or `components/panels/`
2. **Use TypeScript** with proper types
3. **Use Material-UI** components when possible
4. **Style with `sx` prop** (preferred) or CSS modules
5. **Export component** for use in pages

### Adding New Pages

1. **Create page component** in `pages/`
2. **Add route** in `App.tsx`
3. **If overlay**: Create `*OverlayRoot.tsx` component
4. **Set up lifecycle** (useEffect for initialization)
5. **Handle window triggers** if needed

### Adding New Hooks

1. **Create hook file** in `hooks/`
2. **Use `useCallback`** for stable function references
3. **Handle errors** with try/catch
4. **Return typed values** matching hook purpose
5. **Document hook** with JSDoc comments

### Adding New Bridge Methods

1. **Add method** to `initWindowBridge.ts`
2. **Add types** to `types/hodosBrowser.d.ts`
3. **Add callback handlers** to Window interface
4. **Update C++ backend** (outside frontend scope)
5. **Test** message passing flow

### Styling Guidelines

1. **Prefer MUI `sx` prop** for component styles
2. **Use theme colors** when possible
3. **Follow existing patterns** (see `MainBrowserView.tsx` for examples)
4. **Keep CSS files minimal** (only when `sx` prop insufficient)
5. **Test in overlay windows** (different rendering context)

### Type Safety

1. **Always type function parameters and return values**
2. **Import types from `types/` directory**
3. **Extend global types** in `hodosBrowser.d.ts`
4. **Avoid `any` type** (use `unknown` if type is truly unknown)
5. **Use type guards** for runtime type checking

### Error Handling

1. **Wrap async operations** in try/catch
2. **Display user-friendly error messages**
3. **Log errors** to console for debugging
4. **Handle promise rejections** in bridge methods
5. **Clean up callbacks** after use

### Testing Considerations

1. **Test in both main browser and overlay windows**
2. **Verify message passing** works correctly
3. **Check type safety** with TypeScript compiler
4. **Test error cases** (network failures, invalid data)
5. **Verify window lifecycle** (open/close overlays)

---

## Common Patterns & Examples

### Pattern 1: Wallet Operation with Loading State

```typescript
const [loading, setLoading] = useState(false);
const [error, setError] = useState<string | null>(null);

const handleGetBalance = async () => {
  setLoading(true);
  setError(null);
  try {
    const result = await window.hodosBrowser.wallet.getBalance();
    console.log('Balance:', result.balance);
  } catch (err) {
    setError(err instanceof Error ? err.message : 'Unknown error');
  } finally {
    setLoading(false);
  }
};
```

### Pattern 2: Overlay Component with Close Handler

```typescript
const MyOverlayRoot: React.FC = () => {
  const [open, setOpen] = useState(true);

  const handleClose = () => {
    setOpen(false);
    window.cefMessage?.send('overlay_close', []);
  };

  return (
    <MyPanelLayout open={open} onClose={handleClose} />
  );
};
```

### Pattern 3: Using Custom Hooks

```typescript
const MyComponent: React.FC = () => {
  const { balance, getBalance } = useBalance();
  const { address, generateAddress } = useAddress();

  useEffect(() => {
    getBalance();
    generateAddress();
  }, []);

  return (
    <Box>
      <Typography>Balance: {balance}</Typography>
      <Typography>Address: {address}</Typography>
    </Box>
  );
};
```

### Pattern 4: Material-UI Styling

```typescript
<Paper
  sx={{
    display: 'flex',
    alignItems: 'center',
    flex: 1,
    minWidth: 0,
    height: 36,
    borderRadius: 20,
    px: 2,
    bgcolor: '#f1f3f4',
    boxShadow: 'none',
    border: '1px solid transparent',
    '&:hover': {
      bgcolor: '#ffffff',
      border: '1px solid rgba(0, 0, 0, 0.1)',
    },
    '&:focus-within': {
      bgcolor: '#ffffff',
      border: '1px solid #1a73e8',
      boxShadow: '0 0 0 2px rgba(26, 115, 232, 0.1)',
    },
  }}
>
```

---

## Wallet Initialization Flow

### Overview

**⚠️ IMPLEMENTATION STATUS**: This section describes the **PLANNED** wallet initialization flow. Most of this functionality has NOT been implemented yet.

**Current State**:
- ✅ Frontend wallet check on startup is commented out (as intended)
- ❌ Wallet button does NOT check wallet existence (opens overlay directly)
- ❌ WalletSetupModal component does NOT exist yet
- ❌ Wallet button click handler does NOT check wallet status
- ✅ Rust wallet server auto-creates wallet if none exists (needs to be changed)

**Planned Implementation**: The wallet initialization flow will handle checking if a wallet exists, creating new wallets, and recovering existing wallets. This flow will be **user-initiated** (via Wallet button click) and **non-blocking** (browser startup continues regardless of wallet status).

**Related Documentation**: For detailed information about the browser startup sequence and wallet file checks, see [Startup Flow and Wallet Checks](./phase-0-startup-flow-and-wallet-checks.md).

### Architecture Principles (Planned)

1. **Non-blocking startup**: Browser launches without wallet; wallet is optional
2. **File-based check**: Startup checks for wallet database file existence (fast, no server required)
3. **On-demand server**: Wallet server starts only when wallet exists at startup OR when user chooses to create/recover
4. **User-driven creation**: Wallet creation/recovery only happens when user explicitly chooses via modal

---

### Flow 1: Browser Startup

**⚠️ NOT YET IMPLEMENTED** - This flow needs to be implemented in C++ startup code.

**Location**: C++ backend (browser process startup)
**Detailed Documentation**: See [Startup Flow and Wallet Checks](./phase-0-startup-flow-and-wallet-checks.md#startup-sequence)

**Planned Steps** (to be implemented):
1. Check if wallet database file exists: `%APPDATA%/HodosBrowser/wallet/wallet.db`
2. If file exists:
   - Validate database (check if it's not corrupted)
   - If valid → Start wallet server (Rust Actix-web on port 3301)
   - If invalid → Log error, continue without wallet server
3. If file does not exist:
   - Continue browser startup (no wallet server)
   - Browser is fully functional without wallet

**Result**:
- Wallet server may or may not be running
- Browser is always ready to use
- Wallet status stored for later checks

**Frontend Impact**: None (this happens in C++ before React loads)

---

### Flow 2: Wallet Button Click

**⚠️ NOT YET IMPLEMENTED** - The wallet button currently opens the wallet overlay directly without checking if a wallet exists.

**Location**: `frontend/src/pages/MainBrowserView.tsx`
**Trigger**: User clicks Wallet button in toolbar

**Current Implementation** (lines 229-246):
```typescript
<IconButton
  onClick={() => {
    window.cefMessage?.send('overlay_show_wallet', []);
    window.hodosBrowser.overlay.toggleInput(true);
  }}
>
  <AccountBalanceWalletIcon />
</IconButton>
```

**Planned Implementation** (to be implemented):
```typescript
<IconButton
  onClick={async () => {
    try {
      // Ensure wallet server is running (start if needed)
      await ensureWalletServerRunning();

      // Check wallet status
      const status = await window.hodosBrowser.wallet.getStatus();

      if (status.exists) {
        // Wallet exists - open wallet overlay
        window.cefMessage?.send('overlay_show_wallet', []);
        window.hodosBrowser.overlay.toggleInput(true);
      } else {
        // Wallet doesn't exist - show setup modal
        setWalletSetupModalOpen(true);
      }
    } catch (error) {
      console.error('Failed to check wallet status:', error);
      // Optionally show error message to user
    }
  }}
>
```

**Frontend Components Involved**:
- `MainBrowserView.tsx`: Wallet button click handler
- `WalletSetupModal.tsx`: New modal component (to be created)

---

### Flow 3: Wallet Setup Modal (No Wallet Exists)

**⚠️ NOT YET IMPLEMENTED** - This component needs to be created.

**Location**: `frontend/src/components/WalletSetupModal.tsx` (⚠️ TO BE CREATED)
**Trigger**: Wallet button clicked AND `wallet.getStatus()` returns `exists: false`

**Modal States**:
1. **Initial Choice**: "Create New Wallet" or "Recover Wallet"
2. **Create Flow**: Show mnemonic → Responsibility notice → Continue button
3. **Recover Flow**: Choose "From Mnemonic" or "From File" → Input → Recover

**Component Structure**:
```typescript
type WalletSetupStep =
  | 'choice'           // Initial: Create or Recover?
  | 'create-mnemonic'  // Create: Show mnemonic + notice
  | 'recover-choice'   // Recover: Mnemonic or File?
  | 'recover-mnemonic' // Recover: Input mnemonic
  | 'recover-file';    // Recover: File picker

const WalletSetupModal: React.FC<{
  open: boolean;
  onClose: () => void;
  onComplete: () => void; // Called when wallet is created/recovered
}> = ({ open, onClose, onComplete }) => {
  const [step, setStep] = useState<WalletSetupStep>('choice');
  const [mnemonic, setMnemonic] = useState<string>('');
  const [confirmedBackup, setConfirmedBackup] = useState(false);
  // ... other state
}
```

---

### Flow 4: Create New Wallet

**Location**: `frontend/src/components/WalletSetupModal.tsx`
**Trigger**: User clicks "Create New Wallet" in modal

**Frontend Steps**:
1. User selects "Create New Wallet"
2. Call `window.hodosBrowser.wallet.create()` → sends `create_wallet` message
3. Receive response with `mnemonic`, `address`, `version`
4. Update modal to step `'create-mnemonic'`
5. Display mnemonic (with copy button)
6. Show responsibility notice (about securing mnemonic)
7. Show checkbox: "I have securely backed up my mnemonic"
8. "Continue" button disabled until checkbox checked AND user acknowledges
9. On "Continue" click:
   - Close modal
   - Call `onComplete()` callback
   - Wallet overlay can now be opened (wallet exists)

**Bridge Method**: `frontend/src/bridge/initWindowBridge.ts`
```typescript
window.hodosBrowser.wallet.create = () => {
  return new Promise((resolve, reject) => {
    window.onCreateWalletResponse = (data: any) => {
      resolve(data);
      delete window.onCreateWalletResponse;
      delete window.onCreateWalletError;
    };
    window.onCreateWalletError = (error: string) => {
      reject(new Error(error));
      delete window.onCreateWalletResponse;
      delete window.onCreateWalletError;
    };
    window.cefMessage?.send('create_wallet', []);
  });
};
```

**C++ Handler**: `cef-native/src/handlers/simple_handler.cpp` (line 878)
- Receives `"create_wallet"` message
- Calls `WalletService::createWallet()`
- Returns response via `create_wallet_response` message

**Rust Endpoint**: `POST /wallet/create` (via `WalletService`)
- Creates wallet in database
- Generates mnemonic
- Creates first address
- Returns: `{ success: true, mnemonic: "...", address: "...", version: "..." }`

---

### Flow 5: Recover from Mnemonic

**Location**: `frontend/src/components/WalletSetupModal.tsx`
**Trigger**: User selects "Recover Wallet" → "From Mnemonic"

**Frontend Steps**:
1. User selects "Recover Wallet"
2. Modal updates to step `'recover-choice'`
3. User clicks "From Mnemonic"
4. Modal updates to step `'recover-mnemonic'`
5. Show textarea for mnemonic input (12-24 words)
6. User pastes/types mnemonic
7. Validate mnemonic format (basic check)
8. User clicks "Recover" button
9. Call `window.hodosBrowser.wallet.recoverFromMnemonic(mnemonic)`
10. Show loading state
11. On success:
    - Close modal
    - Call `onComplete()` callback
    - Wallet now exists (can open wallet overlay)

**New Bridge Method** (⚠️ TO BE ADDED):
```typescript
// frontend/src/bridge/initWindowBridge.ts
window.hodosBrowser.wallet.recoverFromMnemonic = (mnemonic: string) => {
  return new Promise((resolve, reject) => {
    window.onRecoverWalletResponse = (data: any) => {
      resolve(data);
      delete window.onRecoverWalletResponse;
      delete window.onRecoverWalletError;
    };
    window.onRecoverWalletError = (error: string) => {
      reject(new Error(error));
      delete window.onRecoverWalletResponse;
      delete window.onRecoverWalletError;
    };
    window.cefMessage?.send('wallet_recover', [JSON.stringify({ mnemonic, confirm: true })]);
  });
};
```

**C++ Handler** (⚠️ TO BE ADDED): `cef-native/src/handlers/simple_handler.cpp`
- Receives `"wallet_recover"` message with JSON body
- Calls `WalletService::recoverWallet(mnemonic)`
- Returns response via `wallet_recover_response` message

**Rust Endpoint**: `POST /wallet/recover` (already exists in `rust-wallet/src/handlers.rs`)
- Requires `{ mnemonic: "...", confirm: true }`
- Recovers wallet from mnemonic
- Scans blockchain for addresses/UTXOs
- Returns: `{ success: true, addresses_found: N, utxos_found: N, total_balance: N }`

---

### Flow 6: Recover from File

**Location**: `frontend/src/components/WalletSetupModal.tsx`
**Trigger**: User selects "Recover Wallet" → "From File"

**Frontend Steps**:
1. User selects "Recover Wallet"
2. Modal updates to step `'recover-choice'`
3. User clicks "From File"
4. Modal updates to step `'recover-file'`
5. Show file input (or trigger native file picker)
6. User selects backup file (`wallet.db` or backup file)
7. Read file path (or upload file data to backend)
8. User clicks "Restore" button
9. Call `window.hodosBrowser.wallet.restoreFromFile(filePath)`
10. Show loading state
11. On success:
    - Close modal
    - Call `onComplete()` callback
    - Wallet now exists (can open wallet overlay)

**New Bridge Method** (⚠️ TO BE ADDED):
```typescript
// frontend/src/bridge/initWindowBridge.ts
window.hodosBrowser.wallet.restoreFromFile = (filePath: string) => {
  return new Promise((resolve, reject) => {
    window.onRestoreWalletResponse = (data: any) => {
      resolve(data);
      delete window.onRestoreWalletResponse;
      delete window.onRestoreWalletError;
    };
    window.onRestoreWalletError = (error: string) => {
      reject(new Error(error));
      delete window.onRestoreWalletResponse;
      delete window.onRestoreWalletError;
    };
    window.cefMessage?.send('wallet_restore', [JSON.stringify({ backup_path: filePath, confirm: true })]);
  });
};
```

**C++ Handler** (⚠️ TO BE ADDED): `cef-native/src/handlers/simple_handler.cpp`
- Receives `"wallet_restore"` message with JSON body
- Calls `WalletService::restoreWallet(backupPath)`
- Returns response via `wallet_restore_response` message

**Rust Endpoint**: `POST /wallet/restore` (already exists in `rust-wallet/src/handlers.rs`)
- Requires `{ backup_path: "...", confirm: true }`
- Restores database from backup file
- Returns: `{ success: true, message: "..." }`

---

### Modal Component Structure

**⚠️ TO BE CREATED**

**New File**: `frontend/src/components/WalletSetupModal.tsx` (⚠️ TO BE CREATED)

**Props**:
```typescript
interface WalletSetupModalProps {
  open: boolean;
  onClose: () => void;
  onComplete: () => void; // Called when wallet created/recovered successfully
}
```

**State Management**:
```typescript
const [step, setStep] = useState<WalletSetupStep>('choice');
const [mnemonic, setMnemonic] = useState<string>('');
const [inputMnemonic, setInputMnemonic] = useState<string>('');
const [selectedFile, setSelectedFile] = useState<File | null>(null);
const [confirmedBackup, setConfirmedBackup] = useState(false);
const [loading, setLoading] = useState(false);
const [error, setError] = useState<string | null>(null);
```

**Modal Steps Rendering**:
- `'choice'`: Two buttons (Create / Recover)
- `'create-mnemonic'`: Mnemonic display + notice + checkbox + Continue button
- `'recover-choice'`: Two buttons (From Mnemonic / From File)
- `'recover-mnemonic'`: Textarea + Recover button
- `'recover-file'`: File input + Restore button

**Integration with MainBrowserView**:
```typescript
// frontend/src/pages/MainBrowserView.tsx
const [walletSetupModalOpen, setWalletSetupModalOpen] = useState(false);

// In wallet button click handler:
if (!status.exists) {
  setWalletSetupModalOpen(true);
}

// Render modal:
<WalletSetupModal
  open={walletSetupModalOpen}
  onClose={() => setWalletSetupModalOpen(false)}
  onComplete={() => {
    setWalletSetupModalOpen(false);
    // Now open wallet overlay
    window.cefMessage?.send('overlay_show_wallet', []);
  }}
/>
```

---

### State Flow Diagram

```
Browser Startup
    ↓
Check wallet.db exists?
    ├─ YES → Validate DB → Start wallet server
    └─ NO  → Continue without wallet server
    ↓
Browser Ready (wallet server may or may not be running)

User Clicks Wallet Button
    ↓
Check wallet.getStatus()
    ├─ exists: true → Open Wallet Overlay
    └─ exists: false → Show WalletSetupModal
        ↓
        Modal Step: 'choice'
        ├─ "Create New" → Modal Step: 'create-mnemonic'
        │   ├─ Call wallet.create()
        │   ├─ Show mnemonic + notice
        │   ├─ User checks "I backed up"
        │   └─ Click "Continue" → onComplete() → Open Wallet Overlay
        │
        └─ "Recover" → Modal Step: 'recover-choice'
            ├─ "From Mnemonic" → Modal Step: 'recover-mnemonic'
            │   ├─ User inputs mnemonic
            │   ├─ Call wallet.recoverFromMnemonic()
            │   └─ Success → onComplete() → Open Wallet Overlay
            │
            └─ "From File" → Modal Step: 'recover-file'
                ├─ User selects file
                ├─ Call wallet.restoreFromFile()
                └─ Success → onComplete() → Open Wallet Overlay
```

---

### API Methods Required

**Existing**:
- `window.hodosBrowser.wallet.getStatus()` → Returns `{ exists: boolean }`
- `window.hodosBrowser.wallet.create()` → Creates wallet, returns mnemonic

**To Be Added**:
- `window.hodosBrowser.wallet.recoverFromMnemonic(mnemonic: string)` → Recovers from mnemonic
- `window.hodosBrowser.wallet.restoreFromFile(filePath: string)` → Restores from backup file

**Backend Messages** (C++):
- `'wallet_status_check'` (existing)
- `'create_wallet'` (existing)
- `'wallet_recover'` (to be added)
- `'wallet_restore'` (to be added)

---

### Important Notes

1. **Server Startup**: ⚠️ Wallet server startup on browser launch is NOT YET IMPLEMENTED. Currently, the wallet server must be started manually or runs separately. Planned: Wallet server can be started on-demand when user chooses to create/recover (even if it wasn't running at browser startup)

2. **Auto-Creation**: ⚠️ CURRENTLY, Rust wallet auto-creates wallet on server startup (lines 106-120 in `rust-wallet/src/main.rs`). This needs to be disabled/conditional so that:
   - Server can start without creating wallet
   - Wallet only created when `/wallet/create` endpoint is explicitly called

3. **Error Handling**: Modal should handle:
   - Invalid mnemonic format
   - Invalid backup file
   - Network/server errors
   - Wallet recovery failures

4. **State Persistence**: After successful create/recover:
   - Wallet server is running
   - Wallet exists in database
   - `wallet.getStatus()` will return `exists: true`
   - User can immediately use wallet features

5. **Modal UX**:
   - Show clear loading states during async operations
   - Display helpful error messages
   - Allow user to go back to previous steps
   - Prevent closing modal during critical operations (mnemonic backup)

---

## Next Steps for UI/UX Enhancement

### Questions to Answer

1. **What windows/modals/panels are part of the user experience?**
   - List all UI surfaces (main browser, overlays, modals, panels)
   - Document their purpose and trigger mechanisms
   - Map user flows between surfaces

2. **How are windows triggered?**
   - User actions (button clicks, keyboard shortcuts)
   - System events (wallet creation, BRC-100 auth requests)
   - Programmatic triggers (from backend, from other windows)

3. **What is the visual design system?**
   - Color palette
   - Typography
   - Spacing system
   - Component library extensions
   - Animation/transition guidelines

4. **What are the user flows?**
   - Onboarding flow
   - Wallet creation flow
   - Transaction flow
   - Settings management flow
   - BRC-100 authentication flow

### Documentation to Create

1. **UI Component Inventory**: List all existing components with screenshots/descriptions
2. **User Flow Diagrams**: Visual representation of user journeys
3. **Design System Documentation**: Colors, typography, spacing, components
4. **Window/Modal Catalog**: All UI surfaces with triggers and purposes
5. **Interaction Patterns**: Common UI patterns used throughout the app

---

## Related Documentation

- **[Startup Flow and Wallet Checks](./phase-0-startup-flow-and-wallet-checks.md)**: Complete browser startup sequence and wallet file checks
- **[HTTP Interceptor Flow Guide](./HTTP_INTERCEPTOR_FLOW_GUIDE.md)**: HTTP request interception and domain whitelisting
- **[UX Design Considerations](./helper-3-ux-considerations.md)**: User experience design principles and patterns

---

## Summary

This guide provides a comprehensive overview of the Hodos Browser frontend architecture. Key takeaways:

1. **Three-layer architecture**: React → CEF-Native → Rust Wallet
2. **Message-based communication**: All backend communication via `window.hodosBrowser.*` and `window.cefMessage.send()`
3. **Process isolation**: Overlay windows run in separate CEF processes
4. **Type safety**: Full TypeScript coverage with strict types
5. **Material-UI**: Primary component library with `sx` prop styling
6. **Critical integration points**: Must preserve window APIs, callbacks, routes, and CSS constraints
7. **Non-blocking startup**: Browser launches without wallet; wallet is optional and user-initiated

When implementing UI/UX enhancements:
- ✅ Follow existing patterns
- ✅ Use Material-UI components
- ✅ Maintain type safety
- ✅ Preserve bridge APIs
- ✅ Test in both main browser and overlay windows
- ❌ Don't break window object APIs
- ❌ Don't change message formats without backend updates
- ❌ Don't modify critical CSS constraints

---

## Implementation Checklist (Per Interface)

For each phase/interface, the implementation plan should cover:

### Frontend (React/TypeScript)
- [ ] Component structure and props
- [ ] State management
- [ ] UI/UX flow
- [ ] Styling approach
- [ ] Integration with existing components

### CEF-Native (C++)
- [ ] Window/overlay management
- [ ] Message handling
- [ ] Process isolation
- [ ] Event triggers

### Rust Wallet Backend
- [ ] API endpoints
- [ ] Database schema changes
- [ ] Business logic
- [ ] Error handling

### Database
- [ ] Schema changes
- [ ] Data models
- [ ] Migration scripts
- [ ] Indexes and queries

### Testing
- [ ] Unit tests
- [ ] Integration tests
- [ ] User acceptance testing
- [ ] Cross-browser/platform testing

---

**End of Document**
