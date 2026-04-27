// QRScreenCapture.h — OS-level screen region capture + QR decode for Phase 2
// Windows only. Called from simple_handler.cpp when DOM scan returns 0 results.
#pragma once
#ifdef _WIN32

#include <windows.h>
#include <string>

// Shows the full-screen selection overlay. User drags a rectangle, then
// BitBlt captures the region and quirc decodes any QR code.
// Must be called from the UI thread (uses GDI).
void StartQRScreenCapture();

// Called internally when selection completes or user presses ESC.
void FinishQRScreenCapture(bool cancelled, RECT selection);

#endif // _WIN32
