#include "../../include/core/ChildProcessLogSink.h"

#include <cstring>
#include <string>

#include "include/base/cef_logging.h"

#include "../../include/core/Logger.h"

#ifdef _WIN32
#include <windows.h>
#elif defined(__APPLE__)
#include <crt_externs.h>  // _NSGetArgc / _NSGetArgv
#endif

namespace hodos {
namespace {

constexpr char kVerboseSwitch[] = "--hodos-render-verbose";

// Resolved once at install time. Reading the command line on every log call would be
// wasteful, and the command line cannot change for the life of the process.
bool g_verbose = false;

bool CommandLineHasVerboseSwitch() {
#ifdef _WIN32
    const wchar_t* cmdline = ::GetCommandLineW();
    return cmdline && wcsstr(cmdline, L"--hodos-render-verbose") != nullptr;
#elif defined(__APPLE__)
    // The helper's argv without threading it through every call site. CEF has not
    // necessarily initialized yet at install time, so CefCommandLine is not an option.
    int* argc = _NSGetArgc();
    char*** argv = _NSGetArgv();
    if (!argc || !argv || !*argv) {
        return false;
    }
    for (int i = 0; i < *argc; ++i) {
        const char* arg = (*argv)[i];
        if (arg && std::strcmp(arg, kVerboseSwitch) == 0) {
            return true;
        }
    }
    return false;
#else
    return false;
#endif
}

void ChromiumLogSink(const char* line, int level) {
    if (!line) {
        return;
    }
    // Levels mirror Logger.h's LogLevel: 0 DEBUG, 1 INFO, 2 WARNING, 3 ERROR_LEVEL.
    switch (level) {
        case 0:
            if (g_verbose) {
                LOG(INFO) << line;
            }
            break;
        case 1:
            LOG(INFO) << line;
            break;
        case 2:
            LOG(WARNING) << line;
            break;
        default:
            LOG(ERROR) << line;
            break;
    }
}

}  // namespace

void InstallChildProcessLogSink() {
    g_verbose = CommandLineHasVerboseSwitch();
    Logger::SetSink(&ChromiumLogSink);
}

}  // namespace hodos
