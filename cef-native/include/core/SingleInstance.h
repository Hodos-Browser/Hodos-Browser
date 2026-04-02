#pragma once

#include <string>
#include <atomic>

// Custom message posted by the pipe listener thread to the primary window's WndProc.
// wParam = 0, lParam = pointer to heap-allocated std::string (URL or empty).
// The WndProc handler MUST delete the string after use.
#define WM_SINGLE_INSTANCE_NEW_WINDOW  (WM_APP + 1)

// Single-instance manager using named pipes (Windows).
//
// Flow:
//   1. TryAcquireInstance() — attempts to create a named pipe server.
//      Returns true if this is the first instance (pipe created).
//      Returns false if another instance already owns the pipe.
//
//   2. If first instance: call StartListenerThread() after the main window
//      is created. The listener accepts client connections and posts
//      WM_SINGLE_INSTANCE_NEW_WINDOW to the primary window.
//
//   3. If second instance: call SendToRunningInstance(). This connects
//      to the pipe, sends a "new_window" command, waits for a response,
//      and returns. The caller should exit after this returns true.
//      Handles shutdown-relaunch: if server responds "shutting_down",
//      retries until the old process exits and this becomes the new server.
//
// Thread safety:
//   - TryAcquireInstance / SendToRunningInstance run on the main thread
//     before any windows exist.
//   - The listener thread posts messages via PostMessage (safe cross-thread).
//   - StopListenerThread blocks until the listener exits.

namespace SingleInstance {

// Try to become the pipe server for this profile.
// Returns true if this is the first instance (server created).
// Returns false if another instance already owns the pipe.
bool TryAcquireInstance(const std::string& profileId);

// Send a "new_window" command to the running instance.
// If the running instance is shutting down, retries until it exits,
// then returns false (caller should continue as new first instance).
// Returns true if the running instance acknowledged the request (caller should exit).
// Returns false if forwarding failed (caller should continue startup).
bool SendToRunningInstance(const std::string& profileId, const std::string& url = "");

// Start the background pipe listener thread.
// Can be called BEFORE g_hwnd is created — will respond "not_ready" to clients
// until the main window exists. The thread posts WM_SINGLE_INSTANCE_NEW_WINDOW
// to g_hwnd when a client connects and g_hwnd is valid.
void StartListenerThread(const std::string& profileId);

// Stop the listener thread (called during shutdown).
// Sets the shutdown flag so the listener responds "shutting_down" to new clients,
// then signals the thread to exit and waits for it to finish.
void StopListenerThread();

// Check if the listener is in shutdown mode.
// Used by the listener to decide whether to create windows or respond "shutting_down".
bool IsShuttingDown();

// Set the shutdown flag. Called from ShutdownApplication().
void SetShuttingDown();

}  // namespace SingleInstance
