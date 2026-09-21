# TICKET — the wallet bridge's **plumbing** is advertised to every https site, by name

**Opened** 2026-09-21 from beta.3 Phase 13 Block B (`B2`). **Severity: MEDIUM — privacy, not money.**
⛔ **Not beta.3 scope** unless the owner says otherwise — the sprint is behind and item 1 below is the
only part that is genuinely small. **No code written yet.**
👤 Owner, 2026-09-21: *"agreed on stop advertising the wallet to sites that never ask"* — and separately
*"let's not let it slow us down from getting beta.3 out."*

## What was measured

On `https://example.com/` — an ordinary site, not a dApp, subject asserted `role=tab_1` — a page can
enumerate **six** non-standard globals that stock Chrome does not have:

```
hodosBrowser, cefMessage, __hodos_walletResponse, __hodos_walletResponseChunk,
__hodos_walletCall, CWI
```

## ⚠️ First, the part that is NOT a defect — and the framing correction that goes with it

⛔ **Injecting a wallet provider on every page is standard, not unusual.** MetaMask puts
`window.ethereum` on every page for tens of millions of users; Brave and Phantom do the same. Our own
`BRAVE_WALLET_REFERENCE.md` is already cited in `simple_render_process_handler.cpp` for exactly this.
⇒ **`CWI` / `yours` / `panda` are correct as they are. Leave them alone** — they are the shape dApps
expect, renaming them breaks sites, and "a wallet is present" is not browser-identifying on its own.

⭐ **The gating cascade in front of the shim is already good** and should not be loosened: external
pages only · main frame only · `https://` only, with a TODO for private windows. That is Brave's
posture and it was researched.

⇒ **The defect is narrower than "we inject a bridge".** It is that the **plumbing** — the parts no
dApp asks for by name — is enumerable, and those names say **Hodos**. "A wallet is present" is a
category. "Hodos is present" identifies the browser, and tells every news site and ad network on the
web that this user runs a BSV wallet.

## The three items, cheapest first

### 1. `window.hodosBrowser` on external pages — ⭐ **delete it. Nothing uses it.**

On an external page the object holds exactly one thing: `platform: "windows" | "macos"`, whose stated
purpose is *"so React can conditionally render Windows vs macOS chrome"* — i.e. **internal UI only**.
📏 `grep hodosBrowser cef-native/include/core/CWIShimScript.h` → **no hits**: the shim never touches it.
And the comment on the injection concedes the string is *"not fingerprint-sensitive beyond what the
user-agent already reveals"* — which is an argument for it being **harmless**, not for it being
**needed**.

⇒ Move the `global->SetValue("hodosBrowser", …)` and the `platform` assignment **inside** the existing
`if (isInternalPage || isOverlayBrowser)` block. The gate already exists; this is moving two lines into
it. **Risk to dApps: none** — nothing external reads it.

### 2. `__hodos_walletCall` / `__hodos_walletResponse` / `__hodos_walletResponseChunk` — make them non-enumerable

These **must** stay reachable: the C++ render handler calls `__hodos_walletResponse*` **by name** to
deliver results, and the shim uses `__hodos_walletCall` outbound. So they cannot simply move into a
closure without changing the transport.

⇒ Cheapest correct change: define them with `Object.defineProperty(window, name, {enumerable: false})`
so `Object.getOwnPropertyNames(window)` / `for…in` stop listing them, while `window.__hodos_walletResponse(...)`
still resolves for C++.

⚠️ **Say what this does and does not buy.** It defeats **enumeration**, which is what generic
fingerprinting scripts do. It does **not** hide from a script that tests the name directly
(`'__hodos_walletCall' in window` still answers true). That is a real improvement, not a cloak, and it
must not be written up as one.

### 3. `window.cefMessage` on external pages — ⚠️ needs a look, and it is not only a fingerprint question

`cefMessage.send()` is the render→browser process-message channel and it is injected on **every** page,
external ones included. Two separate questions, and the second is the more important:

- **Fingerprint:** same treatment as item 2 if the shim needs it, or move it inside the internal gate if
  it does not.
- 🚨 **Capability:** an arbitrary web page can call `cefMessage.send('<name>', [...])`. Whether every
  message name is safe to expose to web content is **not established by this ticket** and was not
  investigated — the schedule said stop. ⛔ **Do not close item 3 without answering that**, and treat
  it as a security review rather than a fingerprinting tidy-up.

## Evidence / how to re-measure

`development-docs/0.4.0-beta.3/phase-13-bot-detection/p13_signals.py` — the `hodosGlobals` field.
⛔ Run it with the subject gate (`assert_tab`): the first attempt at this measurement read an
**overlay** and would have reported a different set.

**Negative control for any fix:** the same probe on the same page must still list `CWI` (the provider
is supposed to be there) while the plumbing names disappear. A fix that removes everything has broken
dApps; a fix that removes nothing has done nothing.

## What must not regress

- dApp connect / `createAction` / BRC-121 paid retry — the shim is how they arrive.
- ⭐ The **gold pill** payment indicator chain (`CLAUDE.md`, load-bearing UX safeguard).
- `P13-R1`: the farbling acceptance battery still passes.
