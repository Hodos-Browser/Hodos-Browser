#include "../../include/core/SingleInstance.h"

#ifdef _WIN32

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

#include <thread>
#include <string>
#include <atomic>

// Logger macros — must match Logger::Log(string, int level, int process) signature
#include "../../include/core/Logger.h"
#define LOG_INFO(msg)    Logger::Log(msg, 1, 0)
#define LOG_WARNING(msg) Logger::Log(msg, 2, 0)
#define LOG_ERROR(msg)   Logger::Log(msg, 3, 0)
#define LOG_DEBUG(msg)   Logger::Log(msg, 0, 0)

// External reference to the primary window HWND (defined in cef_browser_shell.cpp).
// Needed by the listener thread to PostMessage to the UI thread.
extern HWND g_hwnd;

namespace {

// Pipe handle for the server (first instance).
HANDLE g_pipe_handle = INVALID_HANDLE_VALUE;

// Background listener thread.
std::thread g_listener_thread;

// Shutdown flag — when true, listener responds "shutting_down" instead of creating windows.
std::atomic<bool> g_shutting_down{false};

// Profile ID for the listener (needed by StopListenerThread for self-connect).
std::string g_listener_profile_id;

// Build the pipe name for a given profile.
std::string GetPipeName(const std::string& profileId) {
    return "\\\\.\\pipe\\hodos-browser-" + profileId;
}

// Read a complete message from a pipe (synchronous, blocking).
// Returns empty string on failure.
std::string ReadPipeMessage(HANDLE pipe) {
    char buffer[4096];
    DWORD bytesRead = 0;

    BOOL ok = ReadFile(pipe, buffer, sizeof(buffer) - 1, &bytesRead, nullptr);
    if (!ok) {
        DWORD err = GetLastError();
        if (err == ERROR_MORE_DATA && bytesRead > 0) {
            // Message larger than buffer — use what we got
        } else {
            return "";
        }
    }

    if (bytesRead == 0) return "";
    buffer[bytesRead] = '\0';
    return std::string(buffer, bytesRead);
}

// Write a message to a pipe (synchronous).
bool WritePipeMessage(HANDLE pipe, const std::string& msg) {
    DWORD bytesWritten = 0;
    return WriteFile(pipe, msg.c_str(), static_cast<DWORD>(msg.size()),
                     &bytesWritten, nullptr) && bytesWritten == msg.size();
}

// Listener thread function: accepts pipe connections, reads commands,
// posts WM_SINGLE_INSTANCE_NEW_WINDOW to the primary window.
void ListenerThreadFunc(std::string profileId) {
    LOG_INFO("SingleInstance: Listener thread started for profile '" + profileId + "'");
    std::string pipeName = GetPipeName(profileId);

    // Close the original pipe from TryAcquireInstance immediately.
    // That pipe instance never calls ConnectNamedPipe, but per MSDN clients CAN
    // connect to it via CreateFileA anyway — creating a "black hole" where the
    // client's writes/reads block forever because nobody services that instance.
    // We close it here and create fresh listener-owned instances below.
    if (g_pipe_handle != INVALID_HANDLE_VALUE) {
        CloseHandle(g_pipe_handle);
        g_pipe_handle = INVALID_HANDLE_VALUE;
    }

    while (!g_shutting_down.load()) {
        // Create a new SYNCHRONOUS pipe instance for each client connection.
        HANDLE pipe = CreateNamedPipeA(
            pipeName.c_str(),
            PIPE_ACCESS_DUPLEX,  // synchronous — no FILE_FLAG_OVERLAPPED
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            4096,   // output buffer
            4096,   // input buffer
            5000,   // default timeout for WaitNamedPipe clients
            nullptr // default security
        );

        if (pipe == INVALID_HANDLE_VALUE) {
            LOG_ERROR("SingleInstance: Failed to create listener pipe instance, error=" +
                      std::to_string(GetLastError()));
            Sleep(1000);
            continue;
        }

        // Synchronous wait for a client to connect.
        // StopListenerThread() does a self-connect to unblock this call.
        BOOL connected = ConnectNamedPipe(pipe, nullptr);
        if (!connected) {
            DWORD err = GetLastError();
            if (err == ERROR_PIPE_CONNECTED) {
                // Client connected between CreateNamedPipe and ConnectNamedPipe — OK
            } else {
                LOG_WARNING("SingleInstance: ConnectNamedPipe failed, error=" +
                            std::to_string(err));
                CloseHandle(pipe);
                continue;
            }
        }

        // Check if we were unblocked by the shutdown self-connect.
        if (g_shutting_down.load()) {
            DisconnectNamedPipe(pipe);
            CloseHandle(pipe);
            break;
        }

        // Wait for data to arrive (up to 5s) using PeekNamedPipe.
        // Prevents blocking forever if a client connects but never sends.
        bool dataReady = false;
        for (int i = 0; i < 50 && !g_shutting_down.load(); i++) {
            DWORD bytesAvail = 0;
            if (PeekNamedPipe(pipe, nullptr, 0, nullptr, &bytesAvail, nullptr) && bytesAvail > 0) {
                dataReady = true;
                break;
            }
            Sleep(100);
        }

        if (g_shutting_down.load()) {
            DisconnectNamedPipe(pipe);
            CloseHandle(pipe);
            break;
        }

        if (!dataReady) {
            LOG_WARNING("SingleInstance: Client connected but sent no data within 5s");
            DisconnectNamedPipe(pipe);
            CloseHandle(pipe);
            continue;
        }

        // Read the client's command (synchronous — data is available per PeekNamedPipe).
        std::string message = ReadPipeMessage(pipe);

        if (g_shutting_down.load()) {
            WritePipeMessage(pipe, "shutting_down");
            DisconnectNamedPipe(pipe);
            CloseHandle(pipe);
            continue;
        }

        if (!message.empty()) {
            LOG_INFO("SingleInstance: Received command: " + message);

            // Extract URL if present (format: "new_window:https://...")
            std::string url;
            if (message.size() > 11 && message.substr(0, 11) == "new_window:") {
                url = message.substr(11);
            }

            // Post message to the primary window's WndProc.
            // Allocate URL string on heap — WndProc handler will delete it.
            if (g_hwnd && IsWindow(g_hwnd)) {
                std::string* urlPtr = new std::string(url);
                PostMessage(g_hwnd, WM_SINGLE_INSTANCE_NEW_WINDOW, 0,
                            reinterpret_cast<LPARAM>(urlPtr));
                WritePipeMessage(pipe, "ok");
                LOG_INFO("SingleInstance: Posted new_window message to primary window");
            } else {
                // Window not yet created — tell client to retry shortly.
                WritePipeMessage(pipe, "not_ready");
                LOG_INFO("SingleInstance: Window not ready, sent not_ready to client");
            }
        } else {
            LOG_WARNING("SingleInstance: Received empty message from client");
        }

        DisconnectNamedPipe(pipe);
        CloseHandle(pipe);
    }

    LOG_INFO("SingleInstance: Listener thread exiting");
}

}  // anonymous namespace

namespace SingleInstance {

bool TryAcquireInstance(const std::string& profileId) {
    std::string pipeName = GetPipeName(profileId);

    // Try to create the pipe with FILE_FLAG_FIRST_PIPE_INSTANCE.
    // This is atomic — only the first process to create this pipe name succeeds.
    HANDLE pipe = CreateNamedPipeA(
        pipeName.c_str(),
        PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
        PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
        PIPE_UNLIMITED_INSTANCES,
        4096,
        4096,
        5000,   // default timeout for WaitNamedPipe clients
        nullptr
    );

    if (pipe == INVALID_HANDLE_VALUE) {
        DWORD err = GetLastError();
        if (err == ERROR_ACCESS_DENIED || err == ERROR_PIPE_BUSY) {
            // Another instance already owns this pipe.
            LOG_INFO("SingleInstance: Another instance detected (pipe exists)");
            return false;
        }
        // Unexpected error — log but treat as first instance to avoid blocking startup.
        LOG_WARNING("SingleInstance: CreateNamedPipe failed with error=" +
                    std::to_string(err) + ", proceeding as first instance");
        return true;
    }

    // We are the first instance. Store the handle (keeps the pipe name alive).
    g_pipe_handle = pipe;
    LOG_INFO("SingleInstance: Acquired instance lock for profile '" + profileId + "'");
    return true;
}

bool SendToRunningInstance(const std::string& profileId, const std::string& url) {
    std::string pipeName = GetPipeName(profileId);
    std::string command = "new_window:" + url;

    // Allow the server process to call SetForegroundWindow on our behalf.
    AllowSetForegroundWindow(ASFW_ANY);

    for (int attempt = 0; attempt < 10; attempt++) {
        if (attempt > 0) {
            LOG_INFO("SingleInstance: Retry attempt " + std::to_string(attempt) + "/10...");
            Sleep(1000);

            // Check if we can become the new first instance (old process exited).
            if (TryAcquireInstance(profileId)) {
                LOG_INFO("SingleInstance: Old instance exited, becoming new first instance");
                return false;  // Caller should continue normal startup.
            }
        }

        // Wait for a pipe instance with ConnectNamedPipe pending.
        if (!WaitNamedPipeA(pipeName.c_str(), 5000)) {
            LOG_WARNING("SingleInstance: WaitNamedPipe timeout, attempt " +
                        std::to_string(attempt + 1));
            continue;
        }

        // Connect to the pipe (synchronous).
        HANDLE pipe = CreateFileA(
            pipeName.c_str(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            nullptr,
            OPEN_EXISTING,
            0,
            nullptr
        );

        if (pipe == INVALID_HANDLE_VALUE) {
            LOG_WARNING("SingleInstance: Failed to connect to pipe, error=" +
                        std::to_string(GetLastError()));
            continue;
        }

        // Set pipe to message mode for reading.
        DWORD mode = PIPE_READMODE_MESSAGE;
        SetNamedPipeHandleState(pipe, &mode, nullptr, nullptr);

        // Send the command.
        if (!WritePipeMessage(pipe, command)) {
            LOG_WARNING("SingleInstance: Failed to write to pipe");
            CloseHandle(pipe);
            continue;
        }

        // Read the response (synchronous — server should respond promptly).
        std::string response = ReadPipeMessage(pipe);
        CloseHandle(pipe);

        if (response == "ok") {
            LOG_INFO("SingleInstance: Running instance acknowledged new_window request");
            return true;  // Caller should exit.
        }

        if (response == "shutting_down") {
            LOG_INFO("SingleInstance: Running instance is shutting down, will retry...");
            continue;
        }

        if (response == "not_ready") {
            LOG_INFO("SingleInstance: Running instance not ready yet, will retry...");
            continue;
        }

        LOG_WARNING("SingleInstance: Unexpected response: '" + response + "'");
    }

    // All retries exhausted. Caller should try normal startup (AcquireProfileLock will
    // be the final gate — if it succeeds, old process is gone; if it fails, show error).
    LOG_WARNING("SingleInstance: All retries exhausted, falling back to normal startup");
    return false;
}

void StartListenerThread(const std::string& profileId) {
    g_listener_profile_id = profileId;
    g_shutting_down.store(false);
    g_listener_thread = std::thread(ListenerThreadFunc, profileId);
}

void StopListenerThread() {
    g_shutting_down.store(true);

    // Self-connect to the pipe to unblock the listener's synchronous ConnectNamedPipe.
    if (!g_listener_profile_id.empty()) {
        std::string pipeName = GetPipeName(g_listener_profile_id);
        HANDLE dummy = CreateFileA(
            pipeName.c_str(),
            GENERIC_READ | GENERIC_WRITE,
            0, nullptr, OPEN_EXISTING, 0, nullptr);
        if (dummy != INVALID_HANDLE_VALUE) {
            CloseHandle(dummy);
        }
    }

    // Wait for the listener thread to exit.
    if (g_listener_thread.joinable()) {
        g_listener_thread.join();
    }

    // Close the original server pipe handle.
    if (g_pipe_handle != INVALID_HANDLE_VALUE) {
        CloseHandle(g_pipe_handle);
        g_pipe_handle = INVALID_HANDLE_VALUE;
    }

    LOG_INFO("SingleInstance: Listener thread stopped and pipe closed");
}

bool IsShuttingDown() {
    return g_shutting_down.load();
}

void SetShuttingDown() {
    g_shutting_down.store(true);
}

}  // namespace SingleInstance

#elif defined(__APPLE__)

// macOS: Single-instance is handled by NSApplication delegate methods
// (applicationShouldHandleReopen:hasVisibleWindows:) in cef_browser_shell_mac.mm.
// These stubs allow the code to compile cross-platform.

namespace SingleInstance {

bool TryAcquireInstance(const std::string& /*profileId*/) {
    return true;  // Always "first instance" — macOS uses different mechanism.
}

bool SendToRunningInstance(const std::string& /*profileId*/, const std::string& /*url*/) {
    return false;  // Never forwards — macOS handles this natively.
}

void StartListenerThread(const std::string& /*profileId*/) {
    // No-op on macOS.
}

void StopListenerThread() {
    // No-op on macOS.
}

bool IsShuttingDown() {
    return false;
}

void SetShuttingDown() {
    // No-op on macOS.
}

}  // namespace SingleInstance

#endif
