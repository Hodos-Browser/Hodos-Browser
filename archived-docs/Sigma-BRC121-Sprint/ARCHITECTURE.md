# Architecture — Sigma-BRC121 Sprint

Three diagrams showing where this sprint's work lands in the Hodos Browser stack:

1. **Hodos today** — current architecture (no sprint changes).
2. **With Phase 1.5** — BRC-100 Surface Completion. Two missing handlers + three permission-tier overlays. New DB tables flagged for user approval per CLAUDE.md invariant 2.
3. **With Phase 2** — `window.CWI` / `window.yours` / `window.panda` V8 shim layer.

**Phase 3 (1Sat Ordinals) and Phase 4 (Demos + LLM dev guides) are out of scope for these diagrams** — Phase 3 is an additive ordinal-aware classifier on top of the Phase 2 surface; Phase 4 is documentation + demo apps.

**Legend** (used in all three diagrams):

```
┌─────┐                   ╔═════╗                   ┌─NEW─┐
│     │  EXISTING box     ║     ║   PHASE 1.5 box   │     │  PHASE 2 box (NEW)
└─────┘                   ╚═════╝                   └─────┘

(W) = Windows-only    (M) = macOS-only    (W/M) = both with platform split
─→  data flow / call direction
═►  NEW data flow / new call direction
```

---

## 1. Hodos today

What's already shipping. All 26 of the canonical 28 BRC-100 methods are implemented and routed; cross-platform CEF + Rust + React wired up.

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ React Frontend (port 5137 in dev)                                             │
│ ─────────────────────────────────────────                                     │
│  pages/             hooks/             bridge/                                │
│  WalletPanelPage    useHodosBrowser    initWindowBridge.ts                    │
│  SettingsPage       useDownloads        ↓ defines                             │
│  DownloadsOverlay   usePrivacyShield   window.hodosBrowser.*                  │
│  ...                                   window.cefMessage.send()               │
└───────────────────────────────────────┬───────────────────────────────────────┘
                                        │ V8 calls
                                        ↓
┌───────────────────────────────────────────────────────────────────────────────┐
│ CEF C++ Shell                                              (W/M)              │
│ ─────────────────────────────────────────                                     │
│                                                                               │
│   simple_render_process_handler.cpp  (W/M, same file — render process)        │
│   ├── OnContextCreated()                                                      │
│   │   └── injects window.hodosBrowser.* and window.cefMessage                 │
│   └── CefMessageSendHandler   (V8 → IPC)                                      │
│                                                                               │
│   simple_handler.cpp  (W/M)              simple_app.cpp  (W/M)                │
│   ├── OnProcessMessageReceived           ├── OnContextInitialized             │
│   ├── 125+ IPC dispatch types            └── (W) 11 overlay create fns        │
│   └── 12 CefXxxHandler interfaces                                             │
│                                                                               │
│   HttpRequestInterceptor.cpp                                                  │
│   ├── isWalletEndpoint()  (route table for /createAction, /encrypt, ...)      │
│   └── AsyncWalletResourceHandler  (forward to localhost:31301)                │
│                                                                               │
│   ┌─ Platform split ─────────────────────┐                                    │
│   │ (W) cef_browser_shell.cpp            │ (M) cef_browser_shell_mac.mm       │
│   │ WS_POPUP overlay HWNDs               │ NSPanel overlays + delegates       │
│   │ DPAPI key storage                    │ Keychain key storage               │
│   │ WinHTTP for sync calls               │ libcurl (or SyncHttpClient)        │
│   └──────────────────────────────────────┘                                    │
└───────────────────────────────────────┬───────────────────────────────────────┘
                                        │ HTTP /endpoint
                                        ↓ localhost:31301
┌───────────────────────────────────────────────────────────────────────────────┐
│ Rust Wallet (port 31301)                                  (platform-agnostic) │
│ ─────────────────────────────────────────                                     │
│                                                                               │
│   main.rs  →  Actix routes  →  handlers.rs + handlers/certificate_handlers.rs │
│                                                                               │
│   ┌─ Existing BRC-100 surface (26 / 28 canonical methods) ─────────┐          │
│   │ Identity:       getPublicKey                                   │          │
│   │ Crypto (6):     encrypt, decrypt, createHmac, verifyHmac,      │          │
│   │                 createSignature, verifySignature               │          │
│   │ Tx (5):         createAction, signAction, abortAction,         │          │
│   │                 listActions, internalizeAction                 │          │
│   │ Outputs (2):    listOutputs, relinquishOutput                  │          │
│   │ Certs (6):      acquire/list/prove/relinquish + 2 discover     │          │
│   │ Auth (2):       isAuthenticated, waitForAuthentication         │          │
│   │ Chain (4):      getHeight, getHeaderForHeight, getNetwork,     │          │
│   │                 getVersion                                     │          │
│   └────────────────────────────────────────────────────────────────┘          │
│                                                                               │
│   Existing permission gate:                                                   │
│   check_domain_approved(origin)   ← origin-keyed connect/disconnect           │
│   SessionManager  (in C++ side, per-tab spend cap + rate limit)               │
│                                                                               │
│   crypto/   brc42, brc43, signing, keys, brc2, aesgcm, dpapi, pin             │
│   database/ wallets, users, addresses, outputs, transactions, ...             │
│             domain_permissions (in code; replaced JSON file in V24)           │
│             commissions, settings, sync_states, monitor_events                │
│   monitor/  TaskCheckForProofs, TaskSendWaiting, TaskCheckPeerPay, ...        │
└───────────────────────────────────────┬───────────────────────────────────────┘
                                        │ HTTP
                                        ↓
                              ┌─────────────────────┐
                              │ External BSV infra  │
                              │ WhatsOnChain, ARC,  │
                              │ MessageBox, Overlay │
                              └─────────────────────┘

Gaps from canonical BRC-100:
  ✗ revealCounterpartyKeyLinkage  (no handler, no route)
  ✗ revealSpecificKeyLinkage      (no handler, no route)
  ✗ Per-protocol permission tier  (BRC-100 PermissionRequest)
  ✗ Per-counterparty permission tier  (BRC-100 CounterpartyPermissionRequest)
  ✗ Grouped permission requests   (BRC-100 GroupedPermissionRequest)
```

---

## 2. With Phase 1.5 — BRC-100 Surface Completion (corrected scope)

Smaller than the original draft. **All additions are child tables of `domain_permissions`** mirroring the existing `cert_field_permissions` pattern, not parallel structures. Existing `domain_permissions` row stays the source of truth for the site's overall trust + spending caps.

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ React Frontend                                                                │
│ ─────────────────────────────────────────                                     │
│  EXISTING:                                                                    │
│   ApprovedSitesTab          (Default Limits + DomainPermissionsTab)           │
│   DomainPermissionForm      (per-site limits + Always notify toggle + warn)   │
│   BRC100AuthOverlayRoot     (connect / payment / cert disclosure)             │
│                                                                               │
│  EXTENSIONS — Phase 1.5:                                                      │
│   DomainPermissionForm  ╔══ + Allow without limits button ══╗                 │
│                          ╔══ + Specific permissions section ══╗               │
│                          ╔══ + Cert fields section (sensitivity-aware) ══╗    │
│   ApprovedSitesTab      ╔══ + Allow without limits global ══╗                 │
│                          ╔══ + Sensitivity classifier editor ══╗              │
│   BRC100AuthOverlayRoot ╔══ + Manifest-driven connect bundle path ══╗         │
│                          ╔══ + Sensitivity-aware cert disclosure ══╗          │
│                                                                               │
│  Existing notification_browser_ overlay — already multiplexes 6 prompt types  │
│  (domain_approval, payment_confirmation, certificate_disclosure,              │
│   rate_limit_exceeded, no_wallet, edit_permissions). Phase 1.5 adds 5 NEW     │
│  type cases to BRC100AuthOverlayRoot.tsx — NO new HWNDs / NSPanels needed:    │
│   ╔═ manifest_connect_bundle    (manifest-driven first-visit) ═╗              │
│   ╔═ identity_key_reveal        (always-prompt privacy perimeter) ═╗          │
│   ╔═ key_linkage_reveal         (always-prompt privacy perimeter) ═╗          │
│   ╔═ protocol_permission_prompt (manifest-less new scope) ═╗                  │
│   ╔═ counterparty_permission_prompt (level-2 new peer) ═╗                     │
└───────────────────────────────────────┬───────────────────────────────────────┘
                                        ↓
┌───────────────────────────────────────────────────────────────────────────────┐
│ CEF C++ Shell                                              (W/M)              │
│ ─────────────────────────────────────────                                     │
│  EXISTING — preserved exactly:                                                │
│   HttpRequestInterceptor (route table + AsyncWalletResourceHandler)           │
│   SessionManager         (per-tab spend cap + rate limit)                     │
│   Right-click MENU_ID_MANAGE_PERMISSIONS → opens DomainPermissionForm         │
│                                                                               │
│  ★★★ PRESERVED: Tab payment badge animation pipeline ★★★                       │
│   Every successful auto-approved payment fires:                               │
│     HttpRequestInterceptor.cpp (2 fire sites):                                │
│       • firePaymentSuccessIpc() — BRC-121 paid retry                          │
│       • AsyncHTTPClient::OnRequestComplete — createAction silent-approve      │
│       both send payment_success_indicator IPC                                 │
│         → simple_render_process_handler.cpp:1051                              │
│           → window.postMessage to header browser                              │
│             → useTabManager.ts:141                                            │
│               → green-dot animation on the tab                                │
│   This is the user's primary visual safeguard against a site abusing          │
│   auto-approve. Phase 1.5 engine MUST keep firing this; Phase 2 shim          │
│   payments MUST also trigger it. Acceptance test required.                    │
│                                                                               │
│  ╔═══ NEW: Permission Engine (Phase 1.5) ════════════════════════════════╗    │
│  ║ Per BRC-100 call:                                                     ║    │
│  ║   1. Fetch domain_permissions row + sub-permission rows               ║    │
│  ║   2. Classify (privacy perimeter? bundle-resolved? new scope?)        ║    │
│  ║   3. Check counters in SessionManager                                 ║    │
│  ║   4. Decide SILENT / PROMPT(kind) / DENY                              ║    │
│  ║                                                                       ║    │
│  ║ On first connect:                                                     ║    │
│  ║   1. Fetch <origin>/.well-known/wallet-manifest.json                  ║    │
│  ║   2. Render bundled connect prompt                                    ║    │
│  ║   3. On accept: write all bundle perms to /wallet/permissions/save    ║    │
│  ╚═══════════════════════════════════════════════════════════════════════╝    │
│                                                                               │
│  HttpRequestInterceptor.cpp                                                   │
│   └── isWalletEndpoint() ╔══ +2 BRC-100 routes ══╗                            │
│                          ║ /revealCounterpartyKeyLinkage                     │
│                          ║ /revealSpecificKeyLinkage   ║                      │
│                          ╔══ +4 permission management routes ══╗              │
│                          ║ /wallet/permissions/check                          │
│                          ║ /wallet/permissions/save                           │
│                          ║ /wallet/permissions/revoke                         │
│                          ║ /wallet/permissions/list   ║                       │
│                          ╚════════════════════════════════╝                   │
└───────────────────────────────────────┬───────────────────────────────────────┘
                                        ↓ localhost:31301
┌───────────────────────────────────────────────────────────────────────────────┐
│ Rust Wallet                                                                   │
│ ─────────────────────────────────────────                                     │
│  EXISTING (untouched bodies):                                                 │
│   26 BRC-100 handlers — bodies unchanged                                      │
│   crypto/brc42.rs, crypto/signing.rs, crypto/keys.rs (invariant 3)            │
│   wallets, users, addresses, outputs, transactions, certificates tables       │
│   domain_permissions, cert_field_permissions tables (shape unchanged)         │
│                                                                               │
│  ╔═══ NEW handlers (Phase 1.5 — additive) ═══════════════════════════════╗   │
│  ║   reveal_counterparty_key_linkage    (handlers.rs)                    ║   │
│  ║   reveal_specific_key_linkage        (handlers.rs)                    ║   │
│  ║   + crypto/key_linkage.rs            (new module)                     ║   │
│  ╚═══════════════════════════════════════════════════════════════════════╝   │
│                                                                               │
│  ╔═══ NEW permission gates (additive — called atop all 28 methods) ══════╗   │
│  ║   check_protocol_approved(origin, protocolID, keyID, counterparty)    ║   │
│  ║   check_basket_approved(origin, basket, access)                       ║   │
│  ║   check_counterparty_approved(origin, counterparty)                   ║   │
│  ║   check_cert_field_approved(origin, certType, field, sensitivity)     ║   │
│  ║                                                                       ║   │
│  ║   Defense in depth: gate also lives in C++. Test fixture asserts      ║   │
│  ║   fresh-origin call triggers gate.                                    ║   │
│  ╚═══════════════════════════════════════════════════════════════════════╝   │
│                                                                               │
│  ┌─ NEW child tables of domain_permissions ⚠️ AWAITS USER WALKTHROUGH ────┐  │
│  │   migrations.rs::v25_subpermissions()                                  │  │
│  │     domain_protocol_permissions     (FK → domain_permissions, CASCADE) │  │
│  │     domain_basket_permissions        (FK → domain_permissions, CASCADE)│  │
│  │     domain_counterparty_permissions  (FK → domain_permissions, CASCADE)│  │
│  │     each with expires_at column (1y default; "never" with warning)     │  │
│  │                                                                        │  │
│  │   ALTER cert_field_permissions ADD COLUMN sensitivity TEXT             │  │
│  │     ('low' | 'medium' | 'high' | 'highest' | 'unknown')                │  │
│  │                                                                        │  │
│  │   No new top-level tables. No audit log. No tier preset table.         │  │
│  │   Mirrors the cert_field_permissions FK pattern exactly.               │  │
│  └────────────────────────────────────────────────────────────────────────┘  │
│                                                                               │
│  Decision: store grants LOCALLY (SQLite). On-chain mirror deferred —          │
│  Babbage's reference is infrastructure debt; UTXO sync isn't robust.          │
└───────────────────────────────────────────────────────────────────────────────┘

Cross-platform parity: every NEW overlay above MUST have both Windows
(WS_POPUP via simple_app.cpp) and macOS (NSPanel via cef_browser_shell_mac.mm)
creation paths before Phase 2 begins.

What is EXPLICITLY UNTOUCHED in Phase 1.5:
  • Existing 26 BRC-100 handler bodies (gate calls added, bodies unchanged)
  • crypto/brc42.rs, crypto/signing.rs, crypto/keys.rs   (invariant 3)
  • Core tables: wallets, users, addresses, outputs, transactions, certificates
  • domain_permissions table SHAPE (we add child tables, don't modify)
  • Right-click MENU_ID_MANAGE_PERMISSIONS context menu
  • Tab payment badge animation pipeline (must stay firing)
  • Per-session counter reset on tab close behavior (per user direction)
  • SessionManager logic in C++
  • V8 injection (Phase 2 territory)
```

---

## 3. With Phase 2 — `window.CWI` / `window.yours` / `window.panda` V8 shim

Phase 2 stacks on top of Phase 1.5 (it depends on the new permission tiers existing). What changes:

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ React Frontend  (no React-side changes for the shim itself —                  │
│                  shim is pure V8 injection)                                   │
└───────────────────────────────────────┬───────────────────────────────────────┘
                                        ↓
┌───────────────────────────────────────────────────────────────────────────────┐
│ CEF C++ Shell                                              (W/M)              │
│ ─────────────────────────────────────────                                     │
│                                                                               │
│ simple_render_process_handler.cpp  (W/M — same file)                          │
│ ├── OnContextCreated()                                                        │
│ │   ├── (existing) injects window.hodosBrowser.* + window.cefMessage          │
│ │   │                                                                         │
│ │   └── ┌─ NEW — Phase 2 ─────────────────────────────────────────────┐       │
│ │       │ injects:                                                    │       │
│ │       │   window.CWI    (canonical 28-method WalletInterface,       │       │
│ │       │                   non-writable + non-configurable          )│       │
│ │       │   window.yours  (legacy translation surface, writable)      │       │
│ │       │   window.panda  (alias to window.yours, writable)           │       │
│ │       │                                                             │       │
│ │       │ each is a V8 Proxy (Brave-style apply trap)                 │       │
│ │       │                                                             │       │
│ │       │ + bsv:announceProvider CustomEvent emitter                  │       │
│ │       │   (EIP-6963 equivalent — multi-provider discovery)          │       │
│ │       └─────────────────────────────────────────────────────────────┘       │
│ │                                                                             │
│ │            window.CWI.createAction(args)                                    │
│ │                  │                                                          │
│ │                  └─→ window.cefMessage.send('cwi_call', [name, args])       │
│ │                            (canonical pass-through to existing dispatch)    │
│ │                                                                             │
│ │            window.yours.signMessage({message, encoding})                    │
│ │                  │                                                          │
│ │                  └─→ window.cefMessage.send('yours_legacy', [name, args])   │
│ │                            ║                                                │
│ │                            ║  IPC handler in simple_handler.cpp             │
│ │                            ║  applies translation (per SHIM_TRANSLATION_    │
│ │                            ║  SPEC.md), then re-enters as 'cwi_call'        │
│ │                            ║  to share permission gate + SessionManager     │
│ │                            ▼                                                │
│ │                      check_domain_approved → check_protocol_approved        │
│ │                       → SessionManager → forward to localhost:31301         │
│ │                                                                             │
│ │            (No internal fast paths. Read-only methods like                  │
│ │             getAddresses still pass the gate.)                              │
│                                                                               │
│ simple_handler.cpp                                                            │
│ ├── (existing) 125+ IPC dispatch types                                        │
│ ├── (Phase 1.5) protocol/counterparty/grouped permission IPCs                 │
│ └── ┌─ NEW — Phase 2 ─────────────────────────────────────────────┐           │
│     │ 'cwi_call' dispatch          (canonical, ~28 method names)  │           │
│     │ 'yours_legacy' dispatch      (legacy translation entry)     │           │
│     │ 'announce_provider'          (bsv:announceProvider trigger) │           │
│     │                                                             │           │
│     │ Auto-approve OFF by default for yours_legacy paths          │           │
│     │ regardless of domain whitelist (per SHIM_TRANSLATION_       │           │
│     │ SPEC.md "Auto-approve under the shim")                      │           │
│     └─────────────────────────────────────────────────────────────┘           │
│                                                                               │
│ Platform split — same shim code, different overlay creation paths             │
│   (W) cef_browser_shell.cpp WS_POPUP for any prompt overlays the shim needs   │
│   (M) cef_browser_shell_mac.mm NSPanel equivalents                            │
│                                                                               │
└───────────────────────────────────────┬───────────────────────────────────────┘
                                        ↓ localhost:31301 (UNCHANGED from 1.5)
┌───────────────────────────────────────────────────────────────────────────────┐
│ Rust Wallet  —  no changes from Phase 1.5                                     │
│ The shim sends translated args through existing handlers and existing         │
│ permission gates. Wallet does not see "yours.signMessage" — only              │
│ "createSignature with protocolID [1, 'yours-legacy-message']".                │
└───────────────────────────────────────────────────────────────────────────────┘

╔══════════════════════════════════════════════════════════════════════════════╗
║  Phase 3 (1Sat Ordinals) — outside this sprint                               ║
║  ──────────────────────                                                      ║
║  Ordinal flows route through window.CWI.createAction({ basket: '1sat' })     ║
║  with no new wallet entrypoint. Phase 3 may add basket-aware classification  ║
║  inside create_action_internal (mirroring Yours's processCWICreateAction).   ║
║  DB shape + UI changes are deferred to a separate conversation — see         ║
║  AUDIT_RESULTS.md "Open questions for the ordinal conversation".             ║
╚══════════════════════════════════════════════════════════════════════════════╝

Cross-platform parity: V8 injection code runs in the render process, which is
platform-agnostic on the C++ side (same .cpp file builds for W and M). The
overlay subprocess creation — used if the shim triggers a permission prompt —
is platform-specific and must be kept in lockstep.

Phase 2 acceptance criteria must include:
  • Treechat login on Windows build  (uses window.panda)
  • Treechat login on macOS build    (same)
  • 1sat.market basic flow on both   (uses window.yours during Yours v4 era)
  • Babbage MetaNet App Catalog app on both  (uses window.CWI)
  • Per-CLAUDE.md auth-category test sites: x.com, google.com, github.com
```

---

## Risk surface — where to look first if something breaks

| If you see... | Most likely culprit | File |
|---|---|---|
| Treechat login no longer silent | `signMessage` security level mismatch | `SHIM_TRANSLATION_SPEC.md` §`signMessage` |
| Funds sent to wallet land on wrong key | `getAddresses` returning identity-key P2PKH | `SHIM_TRANSLATION_SPEC.md` §`getAddresses` |
| Two wallets fighting over `window.CWI` | non-writable descriptor + extension conflict | Phase 2 V8 injection ordering |
| Auto-approve firing where prompts expected | shim path not gated correctly | `simple_handler.cpp` `yours_legacy` dispatch |
| Win build works, Mac broken | overlay creation parity gap | `cef_browser_shell_mac.mm` |
| BRC-100-conforming app sees auto-grants | per-protocol gate not wired in | Phase 1.5 `check_protocol_approved` calls |
| **Auto-approved payments don't trigger green-dot animation** | `payment_success_indicator` IPC not firing through new engine | `HttpRequestInterceptor.cpp` `AsyncHTTPClient::OnRequestComplete` + `firePaymentSuccessIpc` (silent-approve path must keep sending it) |
| **Shim payments (window.yours.sendBsv) don't trigger animation** | shim not routing through canonical IPC + indicator path | Phase 2 acceptance test should catch this |
| Right-click "Manage Site Permissions" stops working after UI change | `DomainPermissionForm` route or IPC dispatch broken | `simple_handler.cpp:6989` + `MENU_ID_MANAGE_PERMISSIONS` |

---

## Files for sprint coordination

| Doc | Read when... |
|---|---|
| `README.md` | Picking up the sprint cold |
| `YOURS_CWI_MIGRATION.md` | Translating any legacy method |
| `phase-0.1-brc100-audit/AUDIT_RESULTS.md` | Wiring up Phase 1.5 |
| `phase-0.2-window-yours-shim-design/SHIM_TRANSLATION_SPEC.md` | Implementing Phase 2 V8 shim |
| `BRAVE_WALLET_REFERENCE.md` | Picking property descriptors / V8 Proxy patterns |
| `AUTO_APPROVE_RATIONALE.md` | Defending the demo against "why isn't this Brave's model?" |
| `OPEN_QUESTIONS.md` | Surfacing scope decisions still pending |
