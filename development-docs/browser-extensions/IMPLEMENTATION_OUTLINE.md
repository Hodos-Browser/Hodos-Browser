# Browser Extension Support: Implementation Outline

> **Purpose:** High-level reference for planning implementation sprints.  
> **Not:** A detailed spec. Each phase would need its own deep dive before execution.

---

## Overview

Hodos is built on CEF (Chromium Embedded Framework). CEF has **partial** extension support through the Chrome Runtime integration layer. Full Chrome Web Store compatibility is not feasible—this outline assumes a curated/controlled extension model.

---

## Phase 1: Foundation

**Goal:** Enable basic extension loading and execution.

### 1.1 Chrome Runtime Integration
- [ ] Confirm Hodos uses Chrome Runtime (not Alloy)
- [ ] Verify `enable_extensions=true` in CEF build flags
- [ ] Test current extension loading capability

### 1.2 Extension Loader
- [ ] Implement `LoadExtension()` API wrapper
- [ ] Create extension directory structure (`/extensions/`)
- [ ] Handle extension manifest parsing (Manifest V3)
- [ ] Extension lifecycle management (load/unload/reload)

### 1.3 Service Worker Host
- [ ] Background service worker execution
- [ ] Event-driven lifecycle (start on trigger, terminate on idle)
- [ ] Message passing between service worker and browser

**Deliverable:** Can load an unpacked extension and run its service worker.

---

## Phase 2: Core APIs

**Goal:** Implement essential Chrome Extension APIs.

### 2.1 Priority APIs (Most Extensions Need These)

| API | Purpose | Complexity |
|-----|---------|------------|
| `chrome.storage` | Persistent data storage | Low |
| `chrome.runtime` | Extension lifecycle, messaging | Medium |
| `chrome.tabs` | Tab management | Medium |
| `chrome.scripting` | Content script injection | Medium |

### 2.2 Secondary APIs (Specific Extensions)

| API | Purpose | Needed For |
|-----|---------|------------|
| `chrome.alarms` | Scheduled tasks | Background tasks |
| `chrome.notifications` | System notifications | User alerts |
| `chrome.contextMenus` | Right-click menus | Many extensions |
| `chrome.declarativeNetRequest` | Network request rules | Ad blockers |

### 2.3 Skip/Defer (Native Hodos Features)

| API | Reason |
|-----|--------|
| Wallet/Web3 APIs | Native Hodos wallet |
| Ad blocking APIs | Native Hodos ad engine |

**Deliverable:** Common extensions can execute basic functionality.

---

## Phase 3: UI Integration

**Goal:** Extension UI surfaces in Hodos.

### 3.1 Extension Toolbar
- [ ] Extension icons in toolbar area
- [ ] Badge text/color support
- [ ] Click handler for popup/action

### 3.2 Extension Popups
- [ ] Popup HTML/CSS/JS rendering
- [ ] Popup window management
- [ ] Communication with service worker

### 3.3 Options Pages
- [ ] Extension settings pages
- [ ] Open in new tab functionality

### 3.4 Content Script UI
- [ ] Injected UI elements in web pages
- [ ] Style isolation (shadow DOM)

**Deliverable:** Extensions have full UI capability.

---

## Phase 4: Management

**Goal:** User-facing extension management.

### 4.1 Extensions Page
- [ ] List installed extensions
- [ ] Enable/disable toggle
- [ ] Remove extension
- [ ] View permissions
- [ ] Extension details/version

### 4.2 Permissions UI
- [ ] Permission grant dialog on install
- [ ] Permission review/revoke
- [ ] Host permissions management

### 4.3 Persistence
- [ ] Save installed extensions across sessions
- [ ] Extension settings persistence
- [ ] Extension state restoration on startup

**Deliverable:** Users can manage extensions through Hodos UI.

---

## Phase 5: Distribution

**Goal:** How users get extensions.

### 5.1 Load Unpacked (Developer Mode)
- [ ] Load from local directory
- [ ] Developer tools integration
- [ ] Hot reload for development

### 5.2 Curated Extensions (Recommended)
- [ ] Pre-approved extension list
- [ ] One-click install from Hodos
- [ ] Automatic updates for curated set

### 5.3 External Installation (Optional/Risky)
- [ ] CRX file installation
- [ ] Warning dialogs for unreviewed extensions
- [ ] Security scanning before install

**Deliverable:** Users can install extensions through defined channels.

---

## Phase 6: Security Hardening

**Goal:** Protect users and the native wallet.

### 6.1 Wallet Isolation
- [ ] Extensions cannot access wallet context
- [ ] Separate process/sandbox for wallet operations
- [ ] No extension injection on wallet pages

### 6.2 Permission Enforcement
- [ ] Strict permission checking
- [ ] Deny overly broad permissions by default
- [ ] Host permission warnings for sensitive sites

### 6.3 Monitoring
- [ ] Extension behavior logging
- [ ] Anomaly detection (unusual network, storage)
- [ ] User alerts for suspicious activity

### 6.4 Safe Mode
- [ ] One-click disable all extensions
- [ ] Auto-disable during sensitive operations (wallet transactions)
- [ ] Recovery mode if extension causes issues

**Deliverable:** Extension support that doesn't compromise Hodos security.

---

## Effort Estimates

| Phase | Effort | Dependencies |
|-------|--------|--------------|
| Phase 1: Foundation | 2-3 weeks | CEF build, Rust/C++ bridge |
| Phase 2: Core APIs | 4-6 weeks | Phase 1 |
| Phase 3: UI Integration | 2-3 weeks | Phase 2, UI framework |
| Phase 4: Management | 2-3 weeks | Phase 3 |
| Phase 5: Distribution | 1-2 weeks | Phase 4 |
| Phase 6: Security | 2-4 weeks | All phases |

**Total:** ~15-21 weeks for full implementation

---

## Decision Points

Before starting, decide:

1. **Scope:** Curated only vs. open installation?
2. **Priority:** Which extensions are must-haves?
3. **Timing:** After MVP or later phase?
4. **Resources:** Dedicated sprint or incremental?

---

## Compatibility Testing Candidates

If proceeding, test these first:

| Extension | Category | Why |
|-----------|----------|-----|
| uBlock Origin | Ad blocking | Compare to native |
| Bitwarden | Password manager | High user demand |
| Dark Reader | Accessibility | Simple, useful |
| React DevTools | Developer | Dev audience |

---

## References

- CEF Extension Support: [bitbucket.org/chromiumembedded/cef](https://bitbucket.org/chromiumembedded/cef)
- Chrome Extensions Docs: [developer.chrome.com/docs/extensions](https://developer.chrome.com/docs/extensions)
- Manifest V3 Migration: [developer.chrome.com/docs/extensions/mv3](https://developer.chrome.com/docs/extensions/mv3)

---

*Last updated: 2026-03-04*
