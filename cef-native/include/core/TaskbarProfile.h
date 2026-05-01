#pragma once

#ifdef _WIN32

#include <windows.h>

// Sets per-profile AUMID, overlay icon badge, and badged window icon on an HWND.
// Call after ShowWindow(). Requires COM initialized (CoInitializeEx).
// Skips badging if only one profile exists.
void SetupTaskbarProfile(HWND hwnd, HINSTANCE hInstance);

#endif
