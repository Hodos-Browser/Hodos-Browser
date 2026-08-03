# B5 — Wallet-provider injection as a fingerprinting surface (`window.CWI`)

**Opened:** 2026-08-03 · **Owner:** Matthew (Marston Enterprises) · **Status:** OPEN QUESTION — scoping only, no design committed, **no code**
**Sprint position:** 0.4.0, sequenced **after** the Chromium/CEF rebuild (P0–P7). Do not start before the new CEF binaries are staged — it touches `simple_render_process_handler.cpp`, the same file the farbling teardown (TD-1) edits, and the two must not collide.

> **Why this exists.** Surfaced during the 2026-08-03 rebuild kickoff review, while deciding D4 (WebGL `UNMASKED_VENDOR`/`RENDERER`). The conclusion there was that we should *not* fake the GPU strings, partly because the GPU string is not our weak link. **This is our weak link.** It is filed separately so it gets decided on its merits rather than being folded into the farbling work.

---

## 1. The observation, in one line

We inject a wallet provider object into **every external HTTPS main-frame page the user visits**, whether or not that page is a dApp. Any site can detect Hodos with one expression:

```js
typeof window.CWI !== 'undefined'    // → true, on every site, for every Hodos user
```

That is a perfect, zero-cost browser identifier — and it survives every fingerprinting defense we are building in P4.

---

## 2. Current behavior (verified against the working tree, 2026-08-03)

**Injection site:** `cef-native/src/handlers/simple_render_process_handler.cpp :: SimpleRenderProcessHandler::OnContextCreated` — the `if (isExternalPage)` block (~`:894–906`).

**Gating cascade today** (each rejection logged separately — the code is clean and well-commented; this is not a sloppy implementation, it is a deliberate design whose privacy cost was never priced):

| # | Gate | Effect |
|---|------|--------|
| 1 | External pages only | Hodos internal UI + overlays excluded |
| 2 | Main frame only | All iframes skipped (matches Yours Wallet + Brave) |
| 3 | `https://` only | Insecure pages skipped (matches Brave's "no provider on insecure pages") |
| 4 | *(TODO, pre-existing)* | Private/incognito — **no such mode exists in Hodos yet**; the comment already reserves the gate |

**What gets injected**, in order:
1. `WALLET_CALL_BRIDGE_SCRIPT` → `window.__hodos_walletCall` / `window.__hodos_walletResponse`
2. `CWI_SHIM_SCRIPT` → `window.CWI`, `window.yours`, `window.panda`

**What it is NOT:** this is not a security hole. Every shim method rides the `wallet_call` IPC bridge and is gated by the Rust permission engine (`permission_service` → `hodos_permission_engine`), identical to canonical BRC-100 calls. There are no bypass paths. **The issue is disclosure, not authorization.**

---

## 3. Why it matters

**3a. It defeats the farbling work on its own.** P4 spends a full phase moving canvas/WebGL/audio farbling into Blink so our fingerprint stops being detectable and starts covering workers. All of that is moot for identification purposes if a site can read `window.CWI` first. We would be hardening a side door while the front door is labelled.

**3b. It is a financial-interest signal, leaked passively.** `window.CWI` does not merely say "unusual browser." It says *this user has a Bitcoin SV wallet*. That is disclosed to every news site, ad network, and analytics script on every HTTPS page — not just to dApps the user chose to interact with. For a browser whose stated moat is being a surveillance-free wallet-native economic layer, passively broadcasting wallet ownership to the entire web is the wrong default.

**3c. Our small user base makes it a strong identifier, not a weak one.** The same scale argument that ruled out copying Brave's `"Brave"` GPU constant applies in reverse here: the smaller the Hodos population, the more information "is a Hodos user" carries.

**3d. It is trivially usable for discrimination.** A site can serve different content, block, or price-differentiate on wallet ownership with no user interaction and no consent prompt.

---

## 4. The tension — this is a real trade-off, not a free win

**dApp discovery requires the object to be present before the page asks for it.** That is the whole point of provider injection: a dApp does `if (window.CWI) { ... }` on load. Remove it and the dApp concludes no wallet is installed. Whatever we do here must not turn every first visit into a broken experience or a popup.

Per `CLAUDE.md`: **UX is the highest-level product goal, and UX wins ties. Minimize prompts.** Any option that trades a real privacy gain for a prompt on every dApp visit is probably the wrong trade. Note also that every mainstream wallet (MetaMask, Brave Wallet, Yours) injects unconditionally — we would be deviating from the ecosystem norm, and deviation has a compatibility cost.

---

## 5. Options to evaluate — NOT a recommendation

Listed for the design session. None is chosen; several may combine.

| # | Option | Sketch | Cost / open risk |
|---|--------|--------|------------------|
| **A** | **Status quo** | Keep unconditional injection; document the disclosure honestly | Zero work, zero breakage. Accepts 3a–3d permanently. The baseline every other option must beat |
| **B** | **Per-site opt-in, reusing `domain_permissions`** | No injection on first visit. User enables the wallet for a site via a toolbar affordance; the existing `domain_permissions` row + `check_domain_approved` already model exactly this trust state | **Strong reuse anchor** — the table, the Rust gate, the right-click "Manage Site Permissions" revoke flow and the `DomainPermissionForm` UI all already exist. Cost: first visit to any dApp appears wallet-less until the user acts. Needs a discoverable, non-annoying affordance — this is the make-or-break UX question |
| **C** | **Announce/request handshake** | Don't expose a global. Page dispatches a request event; we respond only if it does. Modelled on EIP-6963, which the EVM ecosystem adopted to solve the multi-wallet collision problem | Only helps against *passive* scripts — any site can fire the event to probe. Reduces casual detection, not determined detection. Requires dApp-side adoption, so it cannot be the only mechanism without breaking every existing BSV dApp |
| **D** | **Allowlist of known dApp origins** | Ship a curated list; inject only there | Violates `feedback_no_site_specific_code` (no host literals in the wallet). Unmaintainable. **Listed to be explicitly rejected**, not considered |
| **E** | **Private-mode gate only** | Implement the pre-existing TODO #4 when a private mode ships | Correct and already scoped, but orthogonal — does nothing for normal browsing, which is the actual exposure |

**Likely shape:** B as the mechanism, with the UX affordance as the real design work, and E landing alongside whenever private mode arrives. To be confirmed at design time, not now.

---

## 6. Open questions for the design session

1. **What is the affordance?** A wallet icon that lights up when a page *would* use the wallet requires detecting the page's intent — which is the same detection problem inverted. Is a persistent, always-available "enable wallet here" control good enough?
2. **Can we detect dApp intent without exposing anything?** E.g. the site declares a `.well-known/wallet-manifest.json` — we already fetch and parse these (`rust-wallet/src/manifest.rs :: fetch_manifest`, `ManifestFetcher::ParseFromJson`). Could manifest presence gate injection? *Note: a manifest fetch is itself an outbound signal — price that before assuming it's free.*
3. **What breaks?** Enumerate the BSV dApps that would see a wallet-less browser on first visit. The BRC-121 test site and the standard BSV basket (whatsonchain.com) are the minimum smoke set.
4. **Does `window.__hodos_walletCall` need the same treatment?** It is injected alongside the shim and is equally detectable.
5. **Is partial mitigation worth it?** If a determined site can always detect us, does raising the bar against *casual* scripts (ad networks, analytics) still deliver most of the real-world benefit? Probably yes — but say so explicitly rather than letting perfect kill good.
6. **Ecosystem cost.** Are we the only BSV browser doing this? What does deviating cost in dApp-developer goodwill?

---

## 7. Scope guardrails

- **No change to the permission engine, the DB schema, or any crypto path.** This is an *injection-gating* question only. Invariants #2/#3 hold.
- **Must not disturb the gold pill**, the right-click revoke flow, "Always notify", the privacy-perimeter gates, or the per-session counters.
- **Sequenced after the CEF rebuild.** It edits `simple_render_process_handler.cpp`, which TD-1 (farbling teardown) also edits. Landing both at once risks a merge that silently drops one.
- Reuse before building: `domain_permissions`, `check_domain_approved`, `DomainPermissionForm`, `MENU_ID_MANAGE_PERMISSIONS` and the manifest fetcher all already exist.

---

## 8. What this is not

Not a bug report, not a vulnerability, and not a criticism of the current implementation — the gating cascade is deliberate, documented and correct for its stated goal (dApp compatibility). This item asks a question that goal never had to answer: **what is the right default when the user is not on a dApp at all?**

---

*Filed from the 2026-08-03 Chromium/CEF rebuild kickoff review. Sequenced after P7. Feeds no other doc yet; if it produces a design, that design lands here.*
