# DevTools & remote debugging — security design

**Status:** owner-approved in principle (2026-08-04), **not implemented**. Four decisions below.
**Origin:** surfaced by the 2a history smoke — see `DevOps-CICD/TESTING.md` §14.6/§14.7.
**Product constraint:** DevTools is a **product feature**. BSV developers should be able to
troubleshoot the dApps they build inside Hodos. A dev-only solution is explicitly rejected.

---

## 1. The distinction the whole design rests on

Two capabilities are routinely conflated. They are independent, and separating them is what lets us
keep the feature and close the hole.

| | **DevTools (the UI)** | **Remote debugging port** |
|---|---|---|
| What it is | F12 / Inspect Element — the embedded inspector | A TCP socket speaking the Chrome DevTools Protocol |
| Who can use it | someone at the keyboard | **any process on the machine** |
| How we open it | `CefBrowserHost::ShowDevTools()` — **in-process** | `CefSettings.remote_debugging_port` |
| Authentication | physical presence | **none whatsoever** |
| Needed for the BSV-dev feature? | **yes — this IS the feature** | no |

**Closing the port does not disable DevTools.** Chrome itself ships with the port off and F12 works.
Verified for us below: all four of our DevTools entry points call `ShowDevTools()` and never touch
the port.

## 2. Verified current state

Everything here was read from the tree on 2026-08-04, not inferred.

**DevTools entry points — four, all `ShowDevTools()`, none using the port:**

| Path | Location |
|---|---|
| Three-dot menu → DevTools | `simple_handler.cpp` — menu action `"devtools"` (~2793 → `ShowDevTools` ~2802) |
| `"devtools"` IPC message | `simple_handler.cpp` (~4385 → `ShowDevTools` ~4393) |
| F12 / Ctrl+Shift+I | `SimpleHandler::ShowOrFocusDevTools()` (~8589 → `ShowDevTools` ~8608) |
| Right-click → Inspect Element | `MENU_ID_DEV_TOOLS_INSPECT` (~8626), added at **~8668 and ~8732**, handled ~8841 |

> ⚠️ **~8668 is the non-tab branch.** "Inspect Element" is offered on the header **and on every
> overlay** — including the wallet panel and the BRC-100 auth overlay. That is the concrete gap
> decision **D4** closes.

**The port:**
- `cef_browser_shell.cpp` (near `settings.remote_debugging_port`) sets **9222** for the `Default`
  profile and `9222 + N` for `Profile_N`, **unconditionally in production**. Hardcoded, not a
  setting. The only zero case is picker mode. Dev adds `+100` (dev Default = **9322**).
- `simple_app.cpp :: OnBeforeCommandLineProcessing` appends `--remote-allow-origins=*`
  **unconditionally**, removing the Origin check on the CDP WebSocket.
- Both trace to the **initial commit** (`cc6cf19`, 2025-11-18) — inherited from the old repo, never
  a deliberate security decision. `f9408fd` later adjusted the port only to stop dev/prod colliding.

**Nothing in this repo depends on either.** `grep` over `frontend/e2e`, `scripts/`, `.github/` for
`9222`, `remote-debugging`, `connectOverCDP`, `remote-allow-origins` returns nothing. Removal is
low-risk. (The smoke harness in TESTING.md §14.6 uses the **dev** port, which D2 keeps.)

## 3. Threat model — what actually changes

**What DevTools on ordinary web content does NOT break.** A page with DevTools open can only do what
that page could already do. It can call `window.CWI`, and every one of those calls still goes
through the Rust permission engine. **No permission bypass, no new capability.** This is why D1 is
safe and why a blanket "DevTools is dangerous" reflex is wrong.

**What DevTools on a *privileged* origin DOES break.** The architecture rests on: web content is
untrusted and gated; the internal `127.0.0.1:5137` wallet and overlay pages are trusted, and the
wallet's CORS allowlist admits them precisely because they are ours. Executing arbitrary JavaScript
**inside those origins** is not defeating a gate — it steps around the model the gates are built on.

**What the open port adds.** CDP is unauthenticated. Any local process can enumerate tabs, read page
content, execute JS in **any** origin including the privileged ones, and read cookies from live
authenticated sessions without ever touching DPAPI-encrypted storage. It needs no privilege
escalation and shows nothing in the UI. Demonstrated during the smoke: a script listed the real
browser's open tabs from an unrelated process, with no prompt.

**The attack D4 defends against even after the port is closed.** Once D2/D3 land, DevTools requires
physical presence — which weakens D4's urgency but does not remove its value. The live threat is the
**"paste this into the console" scam**: a long-running, effective social-engineering attack (Chrome
ships a console self-XSS warning for exactly this reason). A victim can be talked into opening
DevTools and pasting code. If the wallet overlay simply cannot be inspected, that attack cannot
reach a wallet-trusted origin. **D4 is the defense against the user being the attack vector.**

## 4. Decisions

### D1 — Keep DevTools enabled in production ✅

No change to any of the four entry points. It is the product feature, and per §3 it grants web
content nothing it did not already have.

### D2 — Close the remote debugging port in release

Gate `settings.remote_debugging_port` so release ships with it **off**. Dev keeps 9322 unchanged
(the smoke harness and our own workflow depend on it).

✅ **RESOLVED (owner, 2026-08-04): shape (a), dev-only.** Wrap in `hodos::IsDevEnv()`. Matches how
the port is already dev/prod-namespaced, and adds no settings surface. Accepted cost: we can no
longer attach to a *user's installed* browser to diagnose — "reproduce it in a dev build" is the
supported path. (Rejected: (b) an explicit default-off setting — more surface for a door we do not
currently need. Revisit only if support hits a case that genuinely cannot be reproduced.)

### D3 — Drop `--remote-allow-origins=*` from production

Remove the unconditional append in `OnBeforeCommandLineProcessing`. Nothing in the repo needs it
(§2). If a dev workflow turns out to need it, re-add it **inside the dev branch only**.

### D4 — Scope DevTools away from privileged origins

Refuse DevTools on the wallet panel, BRC-100 auth overlay, and the other internal overlays; allow it
freely on web content. Preserves 100% of the BSV-developer use case.

**Implementation shape.** All four entry points converge on `ShowDevTools()`. Rather than guarding
four call sites, **route all four through `SimpleHandler::ShowOrFocusDevTools()`** and put the guard
there — one chokepoint, and the existing `HasDevTools()` de-dup starts applying to every path
instead of just the keyboard shortcut. Also stop adding `MENU_ID_DEV_TOOLS_INSPECT` in the non-tab
branch (~8668) so the menu doesn't offer something the guard will refuse.

**How to decide "privileged" — ✅ RESOLVED (owner, 2026-08-04): role-only.** `SimpleHandler::role_`
is the signal: tab browsers are `tab_<id>`, everything else is a named role (`header`, `wallet`,
`brc100auth`, …). Gate on `role_.rfind("tab_", 0) == 0` — no URL parsing, no origin matching.

Accepted consequence: a tab that navigates to an internal page (`/browser-data`, `/settings-page`)
is still inspectable. That is deliberate — those pages are reachable through normal UI anyway, so
DevTools grants nothing new there, and the simpler gate is far less likely to rot or be bypassed
than URL matching. (Rejected: role + URL — stricter, but the added strictness buys little and the
extra logic is a liability.)

> ⚠️ Do **not** reach for `IsInternalOrigin()` without fixing it first — a known open item records
> that `IsInternalOrigin("") == true`. Reusing it here would make that bug security-relevant.

## 5. Open questions

**Q1 — ✅ RESOLVED: dev-only.** See D2.
**Q2 — ✅ RESOLVED: role-only.** See D4.

### ⭐ Q3 — RESEARCH SPIKE (carried; not blocking) — does CEF support `--remote-debugging-pipe`? Chromium defines the switch
(`content/public/common/content_switches.cc:608` = `kRemoteDebuggingPipe`) but `libcef/` has **no
handling for it** — `CefSettings` only exposes `remote_debugging_port`. Pipe mode is the industry
answer to unauthenticated CDP: it speaks the protocol over an inherited file descriptor, so only the
launching process can reach it — unreachable rather than authenticated. **If it works in a CEF
embedder it is strictly better than a port** and could serve any future automation need without
reopening this hole. Needs a spike.

**Why this spike matters more than it looks.** It is the same question the **native AI assistant**
(the go-to-market wedge, Demo-Day prototype) will have to answer: *how does an in-browser agent get
programmatic control without an unauthenticated socket?* Answering Q3 early gives that work a safe
foundation instead of the tempting-but-wrong "just turn the port back on." Carried in
`../Future-Features/AI_ASSISTANT_SECURITY_NOTES.md` §3.

**Spike shape (~1h):** pass `--remote-debugging-pipe` via `OnBeforeCommandLineProcessing` in a dev
build with `remote_debugging_port = 0`; check whether the browser starts, whether fds 3/4 carry CDP
traffic, and whether CEF strips the switch. Failure is cheap and informative either way.

**Q4 — Is there a future need for authenticated remote CDP at all?** If BSV devs eventually want to
drive Hodos from Playwright/Puppeteer, D2(a) blocks it. Options if so: pipe mode (Q3), a one-time
token shown in Settings (the Jupyter/VS-Code-tunnel model), or **prompt-on-connect** — a modal
saying "A program is trying to control your browser. Allow?", which reuses the overlay prompt system
and permission engine we already have. **Note there is no way to authenticate individual console
commands** — the trust decision belongs at attach time. Deliberately out of scope here.

**Q5 — Does closing the port break the profile-port scheme?** `9222 + N` per profile exists so
multiple instances coexist. With release off and dev on, dev still needs the offset. Confirm the
picker-mode zero case is unaffected.

## 6. Test plan

| Check | Expectation |
|---|---|
| Release build: `Get-NetTCPConnection -LocalPort 9222` | nothing listening |
| Release build: F12, Ctrl+Shift+I, menu → DevTools, right-click → Inspect on a **web page** | all open DevTools |
| Release build: right-click on the **wallet overlay** | no "Inspect Element" offered; DevTools refuses if invoked |
| Dev build: port **9322** still listening, TESTING.md §14.6 harness still runs | unchanged |
| Multi-profile dev: `Profile_2` still gets its own port | unchanged |
| Picker mode | port stays 0 |
| Regression | a page can still call `window.CWI` with DevTools open, and the permission engine still gates it |

⚠️ **Before driving any CDP port, confirm the owning process** — prod is 9222, dev is 9322, one
digit apart. `Get-NetTCPConnection -LocalPort <p> -State Listen` → `Get-CimInstance Win32_Process` →
check `ExecutablePath`. Cross-check `/json/version`: `Chrome/136.x` is the installed production
build, `Chrome/150.x` is the new dev build.

## 7. Sequencing

D2 + D3 are small and independent — one commit, low risk, nothing depends on the removed surface.
D4 is the more interesting change (routing four entry points through one guard) and deserves its
own commit plus the overlay smoke above. None of it blocks the CEF bump; it can land any time after
S0/S1. Q3 (pipe mode) is a spike that can happen whenever, and its answer only *adds* options.
