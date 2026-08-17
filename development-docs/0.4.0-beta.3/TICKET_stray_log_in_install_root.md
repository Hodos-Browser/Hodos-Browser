# TICKET — 44 raw `ofstream("debug_output.log")` writes land a log INSIDE `{app}`, the one place the silent updater forbids

**Filed:** 2026-08-17, investigating the "everything bogged down" incident
**Severity:** 🚨 **threatens the silent auto-update path** + bypasses every logging control
**Status:** OPEN — **beta.3, high**
**Present in:** beta.1 **and beta.2** (`WalletService.cpp` is byte-identical between the two tags)

---

## The finding

Across five shipped files there are **44** raw writes of the form:

```cpp
std::ofstream debugLog("debug_output.log", std::ios::app);
debugLog << "…" << std::endl;
debugLog.close();
```

| File | count |
|---|---|
| `src/core/WalletService.cpp` | **19** |
| `src/core/AddressHandler.cpp` | ✓ |
| `src/handlers/simple_app.cpp` | ✓ |
| `src/handlers/my_overlay_render_handler.cpp` / `.mm` | ✓ |
| **total** | **44** |

The path is **relative**, so it resolves against the process **current working directory** — which for
an app launched from its installed shortcut is `{app}`.

**Measured on the live install:**

```
%LOCALAPPDATA%\HodosBrowser\debug_output.log     9,865 bytes     Aug 17 15:50
```

Written minutes after a fresh **beta.2** install.

## ⛔ Why this is not merely untidy

`cef-native/CLAUDE.md` states the rule and the reason it exists:

> **Log file location:** `%APPDATA%\HodosBrowser\logs\debug_output.log` … It is deliberately
> **outside** the install root: the browser holds the log open for writing, and **a log inside
> `{app}` broke the silent-update backup hash of the `{app}` tree.**

We already hit this once and moved the *Logger's* file out of `{app}`. **These 44 raw writes were
never moved with it**, so the exact condition the rule forbids is back — and it is live in a shipped
build.

The silent updater verifies the installed tree against the **signed `expected-new-manifest.json`**
(`release.yml` generates and Ed25519-signs it from the staged tree; the apply supervisor checks every
installed `{app}` file's sha256 against it). A file that (a) is absent from the signed manifest,
(b) appears inside `{app}` after install, and (c) **grows on every wallet call**, is precisely the
kind of drift that check exists to catch.

⚠️ **Not yet proven to break an apply** — the manifest may only assert files it lists rather than
rejecting unknown ones. **Determine which before closing**: if it rejects extras, every silent update
is at risk; if it ignores them, this is "only" the backup-hash problem we already hit once.

## They also bypass every logging control

These writes never touch `Logger`, so they ignore:

- level gating (the subject of `TICKET_production_debug_logging_unbounded.md`) — they log
  **unconditionally in production**, including full request bodies;
- the resolved log directory (`AppPaths::GetLogDir()`);
- any future rotation or size cap.

And they are **pathologically expensive**: `WalletService::getBalance` alone opens, writes and closes
the file **four times per call** — twice on entry, twice on the result. The incident window recorded
**417 `get_balance` calls in 52 minutes**, i.e. ~1,700 open/write/close cycles on a file in the
install directory, synchronously, on top of the 1.58 GB `%APPDATA%` log.

## What this does and does not explain about the incident

**Established from the logs:**

- Balances were healthy until ~14:31 (`{"balance":85506903,"bsvPrice":15.145}`).
- From **15:18:02** every call returned `{"error":"Failed to fetch total balance"}`, continuously to
  the 15:21 shutdown.
- ⭐ **The BSV price was never the problem** — `bsvPrice: 15.145` was populating fine, and the
  fallback chain (WhatsOnChain → CoinGecko → MEXC, plus SQLite persistence across restarts) is
  intact. The owner's exchange-rate hypothesis is **ruled out by evidence**.
- The failing branch is `makeHttpRequest("GET", "/wallet/balance")` returning no `balance` key —
  a **wallet/indexer** failure, not a price failure.
- `[MAIN] [WARN] freopen failed - stdout: 13, stderr: 13` (errno 13 = `EACCES`) appears repeatedly —
  multiple instances contending for the same log file. Worth pursuing separately: it means
  `std::cout`/`std::cerr` output from those processes went **nowhere**.
- Session scale: **45 tabs across 4+ windows**, ~30,000 log lines in 52 minutes.

**NOT established:** why *web pages* also stopped loading. A wallet-endpoint failure should not stall
`x.com`. The credible mechanism — worth testing, not asserting — is that these synchronous file
writes plus synchronous wallet HTTP run on the browser UI thread, so a slow/failing wallet backend
stalls **everything**, including local DB reads. The uniform **2.0 s** gaps in the log during the
incident look like a timeout, not contention, which supports it.

⛔ **Do not record a cause.** Reproduce it: with a stub that makes `/wallet/balance` hang, confirm
whether page loads stall too. That single experiment settles it.

## Fix

1. **Delete all 44 raw writes.** They are debug scaffolding. Anything worth keeping becomes a
   `LOG_DEBUG_*` call through `Logger`, which resolves the correct directory and will honour the
   level gate.
2. ⛔ **Never write a relative-path file from the browser process.** CWD is not ours to assume —
   it is `{app}` from a shortcut and something else entirely from a jump-list or protocol launch.
3. **Ship a cleanup** removing any `{app}\debug_output.log` left by prior installs.
4. **Add a build-time guard**: fail the build if `ofstream(` + a bare filename appears outside tests.
   A rule this specific, already violated once and re-violated 44 times, needs enforcement rather
   than documentation.

## ⛔ Negative control

- With the guard in place, deliberately add one `ofstream("foo.log")` and confirm the build **fails**.
- After the fix, run a full wallet session and confirm **no** file is created in `{app}` — and that
  the same events **do** appear in `%APPDATA%\…\logs\` (proving logging was moved, not deleted; a
  silent no-op looks identical to a fix).

## Acceptance

- [ ] all 44 raw writes removed; equivalents routed through `Logger`
- [ ] build guard rejects bare-filename `ofstream` in shipped code, **verified by making it fail**
- [ ] `{app}` stays clean across a full session — measured
- [ ] cleanup ships for existing installs
- [ ] answered: does the signed-manifest check reject unknown files in `{app}`?
