# Frontend Layer

## Responsibility

React/TypeScript UI for wallet interactions, overlays, and browser chrome. This layer never handles private keys or signing—all sensitive operations are delegated to the Rust wallet via the C++ bridge. Communicates with C++ CEF shell via `window.hodosBrowser` and `window.cefMessage` APIs injected by V8.

## Build & Run (Windows)

```powershell
cd frontend
npm install          # First time only
npm run dev          # Dev server on localhost:5137
npm run build        # Production build (runs tsc then vite build)
npm run lint         # ESLint check
```

The CEF browser loads from `localhost:5137` during development.

## Invariants

1. **Never call Rust wallet directly** — all wallet operations go through `window.hodosBrowser.*`
2. **Do not modify `bridge/initWindowBridge.ts`** without understanding V8 injection in C++
3. **Do not add new routes** without corresponding overlay HWND setup in C++ (`cef_browser_shell.cpp`)
4. **No cryptographic operations in frontend** — signing, key derivation, and encryption happen in Rust only

## Entry Points

| File | Purpose |
|------|---------|
| `src/main.tsx` | React entry; imports `BrowserRouter`, renders `<App />`, imports `bridge/initWindowBridge` |
| `src/App.tsx` | Router with routes: `/`, `/settings`, `/wallet`, `/backup`, `/brc100-auth`; registers `window.showBRC100AuthApprovalModal` |

## Extension Points

| To Add | Where |
|--------|-------|
| New overlay page | Create `src/pages/FooOverlayRoot.tsx`, add route in `App.tsx`, add HWND in C++ |
| New wallet API call | Add method in `src/hooks/useHodosBrowser.ts` using `window.hodosBrowser.*` or `cefMessage.send()` |
| New component | Add to `src/components/`, import from page |

## Key Files

| File | Identifiers |
|------|-------------|
| `src/hooks/useHodosBrowser.ts` | `useHodosBrowser()`, `getIdentity`, `generateAddress`, `navigate`, `markBackedUp` |
| `src/hooks/useDownloads.ts` | `useDownloads()` hook; `DownloadItem` interface; IPC: `download_state_update`, `download_cancel/pause/resume/open/show_folder/clear_completed/get_state` |
| `src/pages/DownloadsOverlayRoot.tsx` | Download panel overlay; active/completed list with progress bars; pause/resume/cancel; open/show-in-folder; clear completed (auto-closes overlay) |
| `src/components/FindBar.tsx` | Find-in-page bar; IPC: `find_text`, `find_stop`; receives `find_show`/`find_result` events |
| `src/hooks/useKeyboardShortcuts.ts` | Global keyboard shortcuts (Ctrl+T/W/F, tab switching, reload, etc.) |
| `src/bridge/initWindowBridge.ts` | `window.hodosBrowser.navigation`, `window.hodosBrowser.overlay`, `cefMessage.send()` |
| `src/bridge/brc100.ts` | `brc100` object with BRC-100 protocol methods |
| `src/types/hodosBrowser.d.ts` | TypeScript declarations for `window.hodosBrowser` |
