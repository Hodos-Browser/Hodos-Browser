#pragma once

#include <string>

// The record that must survive the debug log being deleted.
//
// WHY THIS EXISTS. The retention policy for `debug_output.log` is deliberately aggressive —
// 10 MB × 5, nothing older than 30 days, and the pre-beta.3 file cleared on upgrade — because
// it is debug chatter and it was 2.5 GB of plaintext browsing history.
//
// ⛔ But that log was ALSO the only record of several shipped safeguards. The gold-pill
// auto-approve line, the "202 PENDING → modal opened" line and the permission cascade were
// all `LOG_DEBUG_*`, so a production level gate silences every one of them. Deleting the
// debug log and gating DEBUG would, between them, have destroyed the entire money-and-consent
// audit trail — the exact failure the phase brief warns about: fixing the flood by widening
// the blackhole.
//
// So the trail moves somewhere it cannot be rotated away by log noise:
//   * its own file, so debug volume never evicts it;
//   * always written regardless of the debug level;
//   * only decisions worth answering for — payments, permission grants AND denials,
//     profile deletions. Not chatter, so it stays small (a few KB/day).
//
// ⛔ NEVER put a full URL, a page title, a balance, a key or a recovery phrase in here.
// This file is kept LONGER than the debug log, which makes it the worst possible place for
// anything sensitive. Origin only — use hodos::LogSafeUrl().

namespace hodos {

/// Open the audit log. `dir` is AppPaths::GetLogDir(); the file is per-process for the same
/// reason the debug log is (see cef_browser_shell.cpp) — profiles run as separate processes
/// and share this directory.
void InitAuditLog(const std::string& dir);

/// Append one decision. Thread-safe. Silently no-ops if the log could not be opened: an
/// audit-log failure must never break a payment.
///
/// `event`   — stable machine-readable slug, e.g. "payment.auto_approved".
/// `origin`  — the site, ALREADY redacted (LogSafeUrl), or "" if not applicable.
/// `detail`  — short human-readable summary. No secrets, no URLs, no balances.
void AuditEvent(const std::string& event,
                const std::string& origin,
                const std::string& detail);

/// Bytes currently on disk, for the retention sweep. 0 if not open.
unsigned long long AuditLogSize();

void ShutdownAuditLog();

}  // namespace hodos
