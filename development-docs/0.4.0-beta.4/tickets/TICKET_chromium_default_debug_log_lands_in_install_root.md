# TICKET — Chromium's default `debug.log` lands in `{app}`, because two lines log before `CefInitialize`

**Filed:** 2026-09-23, by the `I4` row of `INSTALL_TEST_BATCH.md`, run against the **real signed
`v0.4.0-beta.3` draft installer**. **Severity:** 🟡 low — hygiene, not privacy and not update-integrity.
👤 **Owner accepted it for beta.3 and asked for it to be carried here** rather than fixing on the RC.

> ⭐ **This is NOT `TICKET_stray_log_in_install_root.md`.** That one was `debug_output.log`, written by
> 52 raw `ofstream` calls carrying **wallet financial data**, and 📏 **it is FIXED** — verified absent
> from the install root on this same signed build. Different file, different mechanism, different
> severity. ⛔ Do not merge the two; closing this one does not re-open that one.

## Measured

📏 Install `v0.4.0-beta.3`, snapshot `{app}` **before first run** (566 files), browse a few sites and
open the wallet, snapshot again (567 files). The diff is exactly one entry:

```
> debug.log|1845
```

Contents — **10 lines, 1,845 bytes**, two shapes only:

```
[0923/145557.538:INFO:...\ChildProcessLogSink.cpp:61] [BROWSER] [INFO] SimpleApp constructor called!
[0923/145557.538:INFO:...\ChildProcessLogSink.cpp:61] [BROWSER] [INFO] Render process handler created: true
```

📏 Scanned for `balance|satoshi|mnemonic|seed|privkey|address|transaction|password|token` and a base58
address pattern: **zero matches.** Two lines per process launch, nothing else.

## Mechanism

Both lines are emitted from `SimpleApp`'s constructor / render-process-handler creation, which run
**before `CefInitialize` applies `settings.log_file`** (`cef_browser_shell.cpp:5408`). With no log file
configured yet, Chromium's logging falls back to its default — `debug.log` in the process **working
directory**, which for an installed build is `{app}`.

## Why it is low, stated so nobody re-escalates it

| concern | verdict |
|---|---|
| privacy — wallet/key data in the install root | ❌ **no.** Measured: zero matches. That was the *other* ticket's defect, and it is fixed |
| silently aborts silent auto-update | ❌ **no.** Already refuted under `TICKET_stray_log_in_install_root.md` §0: `VerifyTreeAgainstManifest` iterates only `m.entries` and **never enumerates the directory**, so an unknown file is invisible to it |
| a beta.3 regression | ❌ **no.** 📏 The dev build carries a **644 KB** one that has been accumulating for months |
| unbounded growth | ⚠️ **yes, and this is the whole reason for the ticket.** `Logger` has rotation (10 MB × 5) and retention (30 days / 200 MB); this file has **neither**, and `PruneOldLogs` only matches `debug_output*`. It grows forever, ~2 lines per launch |

## The fix

Give Chromium a log file **before** the first `LOG()` in `SimpleApp`'s constructor, or pass `--log-file`
to every process on the command line. ⚠️ Check the child processes too: they are sandboxed at UNTRUSTED
and **cannot open a file under `%APPDATA%`** — which is exactly why `ChildProcessLogSink` exists and why
`Logger::Initialize` must not simply be called in a child (`cef-native/CLAUDE.md`). So the destination
has to be one a sandboxed child can actually write, or the two lines have to move after init.

⭐ **Cheapest correct fix may be neither:** these two lines say nothing a developer needs in production.
Dropping them to `LOG_DEBUG_*` removes the file on release builds without touching logging policy.

## First step

Confirm whether any *other* line reaches this file on a longer-lived install — 10 lines over 5 launches
is the whole sample so far. If the answer stays "these two shapes", the debug-tier fix above closes it.
