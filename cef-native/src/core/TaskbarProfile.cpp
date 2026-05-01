#ifdef _WIN32

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif

#include <windows.h>
#include <objbase.h>
#include <shlobj.h>
#include <shobjidl.h>
#include <string>

#include "../../include/core/TaskbarProfile.h"
#include "../../include/core/ProfileManager.h"
#include "../../include/core/Logger.h"

#define LOG_INFO_TP(msg) Logger::Log(msg, 1, 2)
#define LOG_ERROR_TP(msg) Logger::Log(msg, 3, 2)

// Parse hex color string "#RRGGBB" to COLORREF (0x00BBGGRR)
static COLORREF ParseHexColor(const std::string& hex) {
    if (hex.size() < 7 || hex[0] != '#') {
        return RGB(95, 99, 104); // Default gray
    }
    unsigned int r = 0, g = 0, b = 0;
    r = std::stoul(hex.substr(1, 2), nullptr, 16);
    g = std::stoul(hex.substr(3, 2), nullptr, 16);
    b = std::stoul(hex.substr(5, 2), nullptr, 16);
    return RGB(r, g, b);
}

// Create a 16x16 HICON with a colored circle and a white initial letter
static HICON CreateProfileBadgeIcon(const ProfileInfo& profile) {
    const int size = 16;

    // Create 32-bit ARGB DIB section
    BITMAPINFO bmi = {};
    bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth = size;
    bmi.bmiHeader.biHeight = -size; // Top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    void* bits = nullptr;
    HDC screenDC = GetDC(nullptr);
    HDC memDC = CreateCompatibleDC(screenDC);
    HBITMAP hBitmap = CreateDIBSection(memDC, &bmi, DIB_RGB_COLORS, &bits, nullptr, 0);
    HBITMAP oldBitmap = (HBITMAP)SelectObject(memDC, hBitmap);

    if (!bits) {
        SelectObject(memDC, oldBitmap);
        DeleteObject(hBitmap);
        DeleteDC(memDC);
        ReleaseDC(nullptr, screenDC);
        return nullptr;
    }

    // Fill transparent (all zeros = fully transparent ARGB)
    memset(bits, 0, size * size * 4);

    // Draw filled circle with profile color
    COLORREF color = ParseHexColor(profile.color);
    HBRUSH hBrush = CreateSolidBrush(color);
    HPEN hPen = CreatePen(PS_SOLID, 1, color);
    HBRUSH oldBrush = (HBRUSH)SelectObject(memDC, hBrush);
    HPEN oldPen = (HPEN)SelectObject(memDC, hPen);
    Ellipse(memDC, 0, 0, size, size);
    SelectObject(memDC, oldBrush);
    SelectObject(memDC, oldPen);
    DeleteObject(hBrush);
    DeleteObject(hPen);

    // Set alpha to 255 for all non-transparent pixels (GDI doesn't set alpha)
    BYTE* pixelData = (BYTE*)bits;
    for (int i = 0; i < size * size; i++) {
        int offset = i * 4;
        // If any color channel is non-zero, pixel was drawn
        if (pixelData[offset] != 0 || pixelData[offset + 1] != 0 || pixelData[offset + 2] != 0) {
            pixelData[offset + 3] = 255; // Set alpha to opaque
        }
    }

    // Draw initial letter centered in white
    if (!profile.avatarInitial.empty()) {
        SetBkMode(memDC, TRANSPARENT);
        SetTextColor(memDC, RGB(255, 255, 255));

        HFONT hFont = CreateFontW(11, 0, 0, 0, FW_BOLD, FALSE, FALSE, FALSE,
            DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY, DEFAULT_PITCH | FF_SWISS, L"Segoe UI");
        HFONT oldFont = (HFONT)SelectObject(memDC, hFont);

        std::wstring initial(1, (wchar_t)profile.avatarInitial[0]);
        RECT textRect = {0, 0, size, size};
        DrawTextW(memDC, initial.c_str(), 1, &textRect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

        // Fix alpha for text pixels (GDI text doesn't set alpha either)
        for (int i = 0; i < size * size; i++) {
            int offset = i * 4;
            if (pixelData[offset] != 0 || pixelData[offset + 1] != 0 || pixelData[offset + 2] != 0) {
                pixelData[offset + 3] = 255;
            }
        }

        SelectObject(memDC, oldFont);
        DeleteObject(hFont);
    }

    SelectObject(memDC, oldBitmap);

    // Create mask bitmap (all black = fully opaque where color bitmap has alpha)
    HBITMAP hMask = CreateBitmap(size, size, 1, 1, nullptr);
    HDC maskDC = CreateCompatibleDC(screenDC);
    HBITMAP oldMask = (HBITMAP)SelectObject(maskDC, hMask);
    // Set mask: 0 = opaque, 1 = transparent
    for (int y = 0; y < size; y++) {
        for (int x = 0; x < size; x++) {
            int offset = (y * size + x) * 4;
            if (pixelData[offset + 3] > 0) {
                SetPixel(maskDC, x, y, RGB(0, 0, 0));   // Opaque
            } else {
                SetPixel(maskDC, x, y, RGB(255, 255, 255)); // Transparent
            }
        }
    }
    SelectObject(maskDC, oldMask);
    DeleteDC(maskDC);

    // Create icon from color + mask bitmaps
    ICONINFO iconInfo = {};
    iconInfo.fIcon = TRUE;
    iconInfo.hbmMask = hMask;
    iconInfo.hbmColor = hBitmap;
    HICON hIcon = CreateIconIndirect(&iconInfo);

    DeleteObject(hBitmap);
    DeleteObject(hMask);
    DeleteDC(memDC);
    ReleaseDC(nullptr, screenDC);

    return hIcon;
}

// CLSID for TaskbarList COM object
static const CLSID CLSID_TaskbarList_ = {0x56FDF344, 0xFD6D, 0x11D0, {0x95, 0x8A, 0x00, 0x60, 0x97, 0xC9, 0xA0, 0x90}};

// Set overlay icon on the taskbar button
static void SetTaskbarOverlayIcon(HWND hwnd, HICON hBadge, const std::wstring& description) {
    ITaskbarList3* pTaskbar = nullptr;
    HRESULT hr = CoCreateInstance(CLSID_TaskbarList_, nullptr, CLSCTX_INPROC_SERVER,
                                  IID_PPV_ARGS(&pTaskbar));
    if (FAILED(hr) || !pTaskbar) {
        LOG_ERROR_TP("Failed to create ITaskbarList3, hr=" + std::to_string(hr));
        return;
    }

    pTaskbar->HrInit();
    pTaskbar->SetOverlayIcon(hwnd, hBadge, description.c_str());
    pTaskbar->Release();
}

void SetupTaskbarProfile(HWND hwnd, HINSTANCE hInstance) {
    // Skip if only one profile exists — no need for visual differentiation
    auto profiles = ProfileManager::GetInstance().GetAllProfiles();
    if (profiles.size() <= 1) {
        return;
    }

    ProfileInfo profile = ProfileManager::GetInstance().GetCurrentProfile();
    LOG_INFO_TP("Setting up taskbar profile: " + profile.name + " (" + profile.id + ")");

    // AUMID is set process-wide in WinMain (before window creation).
    // Here we set the overlay badge on the taskbar button.
    // The normal Hodos icon (set via WM_SETICON in window creation) stays as-is.
    // The overlay badge appears as a small icon in the bottom-right corner
    // of the taskbar button, drawn by Windows on top of the main icon.

    HICON hBadge = CreateProfileBadgeIcon(profile);
    if (!hBadge) {
        LOG_ERROR_TP("Failed to create profile badge icon");
        return;
    }

    SetTaskbarOverlayIcon(hwnd, hBadge, std::wstring(profile.name.begin(), profile.name.end()));
    DestroyIcon(hBadge);
}

#endif // _WIN32
