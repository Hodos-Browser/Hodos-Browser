# TICKET — production ships DEBUG logging with no level gate and no rotation: 1.58 GB of plaintext browsing history

**Filed:** 2026-08-17, while investigating "my installed browser just stopped working"
**Severity:** 🚨 **privacy** + unbounded disk growth + a plausible performance/hang mechanism
**Status:** OPEN — **beta.3 candidate, high**
**Affects:** every installed user, every release to date

---

## Measured on the owner's installed build

```
%APPDATA%\HodosBrowser\logs\debug_output.log
  first line : 2026-07-06 14:15:39   "Logger initialized for MAIN"
  size       : 1,700,205,765 bytes  (1.58 GB)
  growth     : ~38 MB/day average over 42 days, and superlinear under heavy use
```

Alongside it, `cef_debug.log` at 17 MB. Neither is capped.

## Three defects, one cause

### 1. 🚨 It is a complete plaintext browsing history, kept forever

Every resource request is written at DEBUG:

```
[BROWSER] [DEBUG] 🌐 Resource request: https://video.twimg.com/amplify_video/2089357218063785984/... (role: tab_1)
[BROWSER] [DEBUG] 🌐 Method: GET, Connection: , Upgrade:
```

⛔ **That is every URL the user has visited since 2026-07-06, in the clear, never pruned** — on a
browser whose entire product thesis is privacy (farbling, ad/cookie blocking, no telemetry). Anyone
with read access to `%APPDATA%` — malware, a support bundle, a shared machine, a backup — gets the
user's full browsing history. It also outlives the user's own history deletion: clearing browser
history does **not** touch this file.

### 2. There is no level gate — DEBUG ships to users

`src/core/Logger.cpp :: Logger::Log` formats and writes unconditionally:

```cpp
LogLevel logLevel = static_cast<LogLevel>(level);
std::string logEntry = "[" + GetTimestamp() + "] [" + GetProcessName(processType) + "] ["
                     + GetLogLevelName(logLevel) + "] " + message;
```

There is **no comparison against a minimum level**, and **no `IsDevEnv()` check anywhere in the
file**. `LOG_DEBUG_*` is as expensive and as permanent in production as `LOG_ERROR`.

⚠️ Note the asymmetry: the **Rust wallet already got this right** — `flexi_logger`, prod=warn, with
rotation (`wallet_r00023.log`, `wallet_rCURRENT.log`). The C++ side never did.

### 3. No rotation, no cap, no retention

Nothing in `Logger.cpp` or `Logger.h` mentions rotate / max size / truncate. The file grows until
the disk does. The owner's machine had 948 GB free so it merely wasted 1.58 GB — a user on a 128 GB
laptop would eventually be in real trouble, silently.

## ⚠️ Possible link to the reported symptom — hypothesis, NOT established

The trigger for this investigation was *"my installed browser just stopped working."* Every health
signal read normal at the time (process alive, wallet answering `get_balance`, tabs loading, no
ERROR or WARNING in the last 80 MB), so **no cause was proven**.

But a credible mechanism exists and should be tested rather than assumed: the log is written
**synchronously on the browser process**, and a video-heavy page emits dozens of segment requests per
second — the captured tail shows exactly that on an X/Twitter video timeline. Every one triggers two
formatted writes to a 1.58 GB file. Under that I/O load a UI stall is plausible.

⛔ **Do not record this as the cause.** It is a hypothesis with a mechanism, and it is testable:
truncate the log, reproduce the workload, and compare. If it reproduces with a small log, this is not
it.

## Fix

1. **Gate by level, with production defaulting to WARNING.** Mirror what the Rust side already does.
   Cheapest correct form: an early return in `Logger::Log` before any string formatting — building
   the entry and then discarding it keeps the CPU cost of DEBUG in production.
2. **Stop logging full URLs at all in production.** Even at DEBUG this is the wrong data to persist
   in a privacy browser. If a URL is needed for debugging, log the **origin** or a hash, never the
   full path and query — query strings carry tokens, search terms and session identifiers.
3. **Rotate + cap.** Size-based rotation with a small retained set, matching the wallet's scheme so
   there is one mental model.
4. **Decide a retention policy** and state it in the privacy-facing docs. Right now the answer to
   "what does Hodos keep about my browsing?" is *"everything, forever, in plaintext"*, and that is
   not what the product claims.
5. **Ship a one-time cleanup** for existing installs — users already carry these files, and an
   upgrade that silently leaves 1.58 GB of history behind has not fixed their problem.

## ⛔ Negative control

A level gate that has never been observed to suppress anything has not been shown to work:

- with the gate on, a `LOG_DEBUG_*` call must produce **no line** in the production log **and** must
  not build the string (verify by instrumentation or a debugger, not by absence alone — absence is
  also what a broken logger looks like);
- the same call in a dev build must still appear, proving the gate discriminates rather than
  disabling logging outright;
- `LOG_ERROR` must survive in both.

That last one matters: the failure mode to avoid is silencing the log entirely and calling it fixed.
This project has already had **renderer logging be a silent no-op for the entire life of a feature**
(`cef-native/CLAUDE.md`, child-process logging), which is exactly how a total farbling failure went
unreported.

## Acceptance

- [ ] production defaults to WARNING; DEBUG costs nothing (no formatting) when suppressed
- [ ] full URLs no longer persisted in production logs
- [ ] rotation + size cap, matching the wallet's model
- [ ] retention policy documented in privacy-facing docs
- [ ] one-time cleanup for existing installs
- [ ] all three negative controls measured
