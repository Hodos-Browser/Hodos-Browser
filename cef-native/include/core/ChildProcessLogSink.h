#pragma once

namespace hodos {

/// Route this process's Logger output into Chromium's log (cef_debug.log).
///
/// Call ONCE, as early as possible, in every CEF **child** process (renderer, GPU,
/// utility). Harmless but pointless in the browser process, which opens
/// debug_output.log directly via Logger::Initialize().
///
/// ## Why child processes need this at all
///
/// `Logger::Initialize` is only ever called in the browser process. Everywhere else
/// `initialized` stayed false and `Logger::Log` fell through to `std::cout` — which the
/// browser process redirects into `debug_output.log`, but a child process does not. So
/// **every `LOG_*_RENDER` call in the codebase was a silent no-op**, and `[RENDER]` never
/// appeared in the log once. That blindness is why a total farbling failure survived for
/// the entire life of the feature: the only subsystem that could have reported it had no
/// way to write.
///
/// A child cannot just call `Logger::Initialize` instead. Renderers run **sandboxed at
/// UNTRUSTED integrity** and have no write access to `%APPDATA%`; the open would fail, and
/// `Logger::Initialize` swallows that failure, so it would look fixed and stay broken.
/// Chromium's own logging is already brokered across the sandbox boundary, so we hand the
/// formatted line to it and let it reach `settings.log_file`.
///
/// ## Verbosity
///
/// There are ~90 `LOG_DEBUG_RENDER` call sites against ~14 at INFO/WARNING/ERROR, and the
/// DEBUG tier is explicitly documented as high-frequency noise ("every IPC message"). So:
///
/// - INFO / WARNING / ERROR — always emitted.
/// - DEBUG — dropped unless the child was launched with `--hodos-render-verbose`, which
///   `SimpleApp::OnBeforeChildProcessLaunch` appends for dev builds only.
///
/// ⛔ The switch is read from the **command line**, never an env var: a sandboxed child
/// does not reliably inherit our environment. Forgetting that is what killed every
/// renderer during the sandbox work (see `cef-native/CLAUDE.md`, S2).
void InstallChildProcessLogSink();

}  // namespace hodos
