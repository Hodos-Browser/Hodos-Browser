// QRScreenCapture.cpp — Phase 2: OS-level screen region capture + QR decode
// Follows the HodosGhostTab pattern (simple_handler.cpp) for the selection overlay.
#ifdef _WIN32

#include "../../include/core/QRScreenCapture.h"
#include "../../include/core/Logger.h"
#include "include/cef_browser.h"
#include "include/cef_process_message.h"

extern "C" {
#include "quirc.h"
}

#include <regex>
#include <string>
#include <sstream>

// Logging macros (same pattern as other core .cpp files)
#define LOG_INFO_QR(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_QR(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_QR(msg) Logger::Log(msg, 3, 2)

// ============================================================================
// Forward declarations — wallet overlay lifecycle (simple_app.cpp)
// ============================================================================
class BrowserWindow;
extern void HideWalletOverlay();
extern void ShowWalletOverlay(int iconRightOffset, BrowserWindow* targetWin);

// QR scan requester — set by simple_handler.cpp before calling StartQRScreenCapture
extern CefRefPtr<CefBrowser> g_qr_scan_requester;

// ============================================================================
// Static state for the selection overlay
// ============================================================================
static HWND   s_capture_hwnd = nullptr;
static bool   s_capture_class_registered = false;
static bool   s_is_dragging = false;
static POINT  s_drag_start  = {0, 0};
static POINT  s_drag_current = {0, 0};
static int    s_vscreen_x = 0, s_vscreen_y = 0;
static int    s_vscreen_w = 0, s_vscreen_h = 0;

// ============================================================================
// BSV pattern classification (mirrors qr-scanner-logic.js regexes)
// ============================================================================

static const std::regex RE_BSV_ADDRESS(R"(^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$)");
static const std::regex RE_IDENTITY_KEY(R"(^(02|03)[0-9a-fA-F]{64}$)");
static const std::regex RE_PAYMAIL(R"(^(\$[a-zA-Z0-9_]+|[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,})$)");
static const std::regex RE_BIP21(R"(^bitcoin:)", std::regex_constants::icase);

// URL-decode a string (for BIP21 label parsing)
static std::string UrlDecode(const std::string& s) {
    std::string result;
    result.reserve(s.size());
    for (size_t i = 0; i < s.size(); ++i) {
        if (s[i] == '%' && i + 2 < s.size()) {
            int hi = 0, lo = 0;
            if (sscanf(s.c_str() + i + 1, "%1x%1x", &hi, &lo) == 2) {
                result += static_cast<char>((hi << 4) | lo);
                i += 2;
                continue;
            }
        }
        if (s[i] == '+') { result += ' '; continue; }
        result += s[i];
    }
    return result;
}

// Escape a string for JSON embedding
static std::string JsonEscape(const std::string& s) {
    std::string out;
    out.reserve(s.size() + 8);
    for (char c : s) {
        switch (c) {
            case '"':  out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n";  break;
            case '\r': out += "\\r";  break;
            case '\t': out += "\\t";  break;
            default:   out += c;      break;
        }
    }
    return out;
}

// Classify a QR payload and build a JSON result object.
// Returns empty string if the payload doesn't match any BSV pattern.
static std::string ClassifyAndBuildJson(const std::string& text) {
    // BIP21 URI
    if (std::regex_search(text, RE_BIP21)) {
        // Parse bitcoin:address?amount=X&label=Y
        std::string address, amount, label;
        size_t colon = text.find(':');
        std::string rest = (colon != std::string::npos) ? text.substr(colon + 1) : text;

        size_t q = rest.find('?');
        address = (q != std::string::npos) ? rest.substr(0, q) : rest;

        if (q != std::string::npos) {
            std::string params = rest.substr(q + 1);
            std::istringstream ps(params);
            std::string pair;
            while (std::getline(ps, pair, '&')) {
                size_t eq = pair.find('=');
                if (eq == std::string::npos) continue;
                std::string key = pair.substr(0, eq);
                std::string val = UrlDecode(pair.substr(eq + 1));
                if (key == "amount") amount = val;
                else if (key == "label") label = val;
            }
        }

        std::string json = "{\"type\":\"bip21\",\"value\":\"" + JsonEscape(text) + "\"";
        if (!address.empty()) json += ",\"address\":\"" + JsonEscape(address) + "\"";
        if (!amount.empty())  json += ",\"amount\":" + amount;
        if (!label.empty())   json += ",\"label\":\"" + JsonEscape(label) + "\"";
        json += ",\"source\":\"screen\"}";
        return json;
    }

    // Plain BSV address
    if (std::regex_match(text, RE_BSV_ADDRESS)) {
        return "{\"type\":\"address\",\"value\":\"" + JsonEscape(text) +
               "\",\"address\":\"" + JsonEscape(text) + "\",\"source\":\"screen\"}";
    }

    // Identity key (BRC-100)
    if (std::regex_match(text, RE_IDENTITY_KEY)) {
        return "{\"type\":\"identity_key\",\"value\":\"" + JsonEscape(text) +
               "\",\"source\":\"screen\"}";
    }

    // Paymail
    if (std::regex_match(text, RE_PAYMAIL)) {
        return "{\"type\":\"paymail\",\"value\":\"" + JsonEscape(text) +
               "\",\"source\":\"screen\"}";
    }

    return ""; // Not a BSV pattern
}

// ============================================================================
// Screen capture + QR decode
// ============================================================================

static std::string CaptureAndDecode(RECT sel) {
    int w = sel.right - sel.left;
    int h = sel.bottom - sel.top;
    if (w < 10 || h < 10) return "";

    // BitBlt from screen
    HDC hdcScreen = GetDC(nullptr);
    HDC hdcMem = CreateCompatibleDC(hdcScreen);
    HBITMAP hBitmap = CreateCompatibleBitmap(hdcScreen, w, h);
    HBITMAP hOld = (HBITMAP)SelectObject(hdcMem, hBitmap);
    BitBlt(hdcMem, 0, 0, w, h, hdcScreen, sel.left, sel.top, SRCCOPY);
    SelectObject(hdcMem, hOld);

    // Get pixel data as 32-bit BGRA
    BITMAPINFOHEADER bmi = {};
    bmi.biSize = sizeof(BITMAPINFOHEADER);
    bmi.biWidth = w;
    bmi.biHeight = -h; // top-down
    bmi.biPlanes = 1;
    bmi.biBitCount = 32;
    bmi.biCompression = BI_RGB;

    std::vector<uint8_t> pixels(w * h * 4);
    GetDIBits(hdcMem, hBitmap, 0, h, pixels.data(), (BITMAPINFO*)&bmi, DIB_RGB_COLORS);

    DeleteObject(hBitmap);
    DeleteDC(hdcMem);
    ReleaseDC(nullptr, hdcScreen);

    // Create quirc decoder
    struct quirc* qr = quirc_new();
    if (!qr) {
        LOG_ERROR_QR("quirc_new() failed");
        return "";
    }

    if (quirc_resize(qr, w, h) < 0) {
        LOG_ERROR_QR("quirc_resize() failed");
        quirc_destroy(qr);
        return "";
    }

    // Convert BGRA to grayscale into quirc buffer
    int qw = 0, qh = 0;
    uint8_t* buf = quirc_begin(qr, &qw, &qh);
    for (int y = 0; y < h && y < qh; ++y) {
        for (int x = 0; x < w && x < qw; ++x) {
            int src = (y * w + x) * 4;
            uint8_t b = pixels[src + 0];
            uint8_t g = pixels[src + 1];
            uint8_t r = pixels[src + 2];
            buf[y * qw + x] = static_cast<uint8_t>((r * 299 + g * 587 + b * 114) / 1000);
        }
    }
    quirc_end(qr);

    // Iterate decoded QR codes
    int count = quirc_count(qr);
    LOG_INFO_QR("quirc found " + std::to_string(count) + " QR code(s) in selection");

    std::string bestResult;
    for (int i = 0; i < count; ++i) {
        struct quirc_code code;
        struct quirc_data data;
        quirc_extract(qr, i, &code);

        if (quirc_decode(&code, &data) != QUIRC_SUCCESS) {
            // Try flipped
            quirc_flip(&code);
            if (quirc_decode(&code, &data) != QUIRC_SUCCESS)
                continue;
        }

        std::string payload(reinterpret_cast<char*>(data.payload), data.payload_len);
        LOG_INFO_QR("QR payload: " + payload.substr(0, 200));

        std::string json = ClassifyAndBuildJson(payload);
        if (!json.empty()) {
            bestResult = json;
            break; // Use first BSV match
        }
    }

    quirc_destroy(qr);
    return bestResult;
}

// ============================================================================
// Deliver result to wallet overlay via IPC
// ============================================================================

static void DeliverResult(const std::string& json) {
    if (!g_qr_scan_requester || !g_qr_scan_requester->GetMainFrame()) {
        LOG_WARNING_QR("No QR scan requester to deliver result to");
        g_qr_scan_requester = nullptr;
        return;
    }

    CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("qr_screen_capture_result");
    msg->GetArgumentList()->SetString(0, json);
    g_qr_scan_requester->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
    g_qr_scan_requester = nullptr;
    LOG_INFO_QR("Screen capture result delivered to wallet overlay");
}

// ============================================================================
// Selection overlay painting (UpdateLayeredWindow with per-pixel alpha)
// ============================================================================

static void PaintOverlay() {
    if (!s_capture_hwnd) return;

    HDC hdcScreen = GetDC(nullptr);
    HDC hdcMem = CreateCompatibleDC(hdcScreen);

    BITMAPINFO bmi = {};
    bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth = s_vscreen_w;
    bmi.bmiHeader.biHeight = -s_vscreen_h; // top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    void* bits = nullptr;
    HBITMAP hBitmap = CreateDIBSection(hdcMem, &bmi, DIB_RGB_COLORS, &bits, nullptr, 0);
    if (!hBitmap || !bits) {
        DeleteDC(hdcMem);
        ReleaseDC(nullptr, hdcScreen);
        return;
    }

    HBITMAP hOld = (HBITMAP)SelectObject(hdcMem, hBitmap);

    // Fill with semi-transparent black (BGRA, pre-multiplied alpha)
    uint8_t* px = static_cast<uint8_t*>(bits);
    const uint8_t alpha = 153; // ~60% opacity
    for (int i = 0; i < s_vscreen_w * s_vscreen_h; ++i) {
        px[i * 4 + 0] = 0;     // B (pre-multiplied)
        px[i * 4 + 1] = 0;     // G
        px[i * 4 + 2] = 0;     // R
        px[i * 4 + 3] = alpha;  // A
    }

    // If dragging, cut out the selection rectangle (make it transparent)
    if (s_is_dragging) {
        int x1 = (std::min)(s_drag_start.x, s_drag_current.x) - s_vscreen_x;
        int y1 = (std::min)(s_drag_start.y, s_drag_current.y) - s_vscreen_y;
        int x2 = (std::max)(s_drag_start.x, s_drag_current.x) - s_vscreen_x;
        int y2 = (std::max)(s_drag_start.y, s_drag_current.y) - s_vscreen_y;

        // Clamp to bitmap bounds
        x1 = (std::max)(x1, 0); y1 = (std::max)(y1, 0);
        x2 = (std::min)(x2, s_vscreen_w); y2 = (std::min)(y2, s_vscreen_h);

        // Clear selection area to fully transparent
        for (int y = y1; y < y2; ++y) {
            for (int x = x1; x < x2; ++x) {
                int idx = (y * s_vscreen_w + x) * 4;
                px[idx + 0] = 0; px[idx + 1] = 0;
                px[idx + 2] = 0; px[idx + 3] = 0;
            }
        }

        // Draw 2px gold border (#a67c00) around selection
        // Pre-multiplied: R=166*255/255=166, but alpha=255 so just use raw values
        const uint8_t borderR = 166, borderG = 124, borderB = 0, borderA = 255;
        auto drawBorderPixel = [&](int x, int y) {
            if (x < 0 || x >= s_vscreen_w || y < 0 || y >= s_vscreen_h) return;
            int idx = (y * s_vscreen_w + x) * 4;
            px[idx + 0] = borderB; px[idx + 1] = borderG;
            px[idx + 2] = borderR; px[idx + 3] = borderA;
        };

        for (int t = 0; t < 2; ++t) { // 2px border thickness
            for (int x = x1 - t; x <= x2 + t; ++x) {
                drawBorderPixel(x, y1 - 1 - t);
                drawBorderPixel(x, y2 + t);
            }
            for (int y = y1 - t; y <= y2 + t; ++y) {
                drawBorderPixel(x1 - 1 - t, y);
                drawBorderPixel(x2 + t, y);
            }
        }
    }

    // UpdateLayeredWindow with per-pixel alpha
    POINT ptSrc = {0, 0};
    SIZE  sz    = {s_vscreen_w, s_vscreen_h};
    POINT ptDst = {s_vscreen_x, s_vscreen_y};
    BLENDFUNCTION blend = {AC_SRC_OVER, 0, 255, AC_SRC_ALPHA};
    UpdateLayeredWindow(s_capture_hwnd, hdcScreen, &ptDst, &sz, hdcMem, &ptSrc, 0, &blend, ULW_ALPHA);

    SelectObject(hdcMem, hOld);
    DeleteObject(hBitmap);
    DeleteDC(hdcMem);
    ReleaseDC(nullptr, hdcScreen);
}

// ============================================================================
// WndProc for selection overlay
// ============================================================================

static LRESULT CALLBACK CaptureOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
    case WM_SETCURSOR:
        SetCursor(LoadCursor(nullptr, IDC_CROSS));
        return TRUE;

    case WM_LBUTTONDOWN:
        s_is_dragging = true;
        GetCursorPos(&s_drag_start);
        s_drag_current = s_drag_start;
        SetCapture(hwnd);
        return 0;

    case WM_MOUSEMOVE:
        if (s_is_dragging) {
            GetCursorPos(&s_drag_current);
            PaintOverlay();
        }
        return 0;

    case WM_LBUTTONUP:
        if (s_is_dragging) {
            s_is_dragging = false;
            ReleaseCapture();
            GetCursorPos(&s_drag_current);

            // Build normalized selection rect in screen coordinates
            RECT sel;
            sel.left   = (std::min)(s_drag_start.x, s_drag_current.x);
            sel.top    = (std::min)(s_drag_start.y, s_drag_current.y);
            sel.right  = (std::max)(s_drag_start.x, s_drag_current.x);
            sel.bottom = (std::max)(s_drag_start.y, s_drag_current.y);

            FinishQRScreenCapture(false, sel);
        }
        return 0;

    case WM_RBUTTONDOWN:
        if (s_is_dragging) {
            s_is_dragging = false;
            ReleaseCapture();
        }
        FinishQRScreenCapture(true, {});
        return 0;

    case WM_KEYDOWN:
        if (wParam == VK_ESCAPE) {
            if (s_is_dragging) {
                s_is_dragging = false;
                ReleaseCapture();
            }
            FinishQRScreenCapture(true, {});
        }
        return 0;

    case WM_DESTROY:
        return 0;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

// ============================================================================
// Window class registration (lazy, like EnsureGhostWindowClass)
// ============================================================================

static void EnsureCaptureWindowClass() {
    if (s_capture_class_registered) return;
    WNDCLASSEXW wc = {};
    wc.cbSize = sizeof(WNDCLASSEXW);
    wc.lpfnWndProc = CaptureOverlayWndProc;
    wc.hInstance = GetModuleHandle(nullptr);
    wc.hCursor = LoadCursor(nullptr, IDC_CROSS);
    wc.lpszClassName = L"HodosQRCapture";
    RegisterClassExW(&wc);
    s_capture_class_registered = true;
}

// ============================================================================
// Public API
// ============================================================================

void StartQRScreenCapture() {
    // Destroy any existing selection overlay
    if (s_capture_hwnd) {
        DestroyWindow(s_capture_hwnd);
        s_capture_hwnd = nullptr;
    }

    EnsureCaptureWindowClass();

    // Get virtual screen bounds (all monitors)
    s_vscreen_x = GetSystemMetrics(SM_XVIRTUALSCREEN);
    s_vscreen_y = GetSystemMetrics(SM_YVIRTUALSCREEN);
    s_vscreen_w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
    s_vscreen_h = GetSystemMetrics(SM_CYVIRTUALSCREEN);

    LOG_INFO_QR("Starting QR screen capture: virtual screen " +
                std::to_string(s_vscreen_w) + "x" + std::to_string(s_vscreen_h) +
                " at (" + std::to_string(s_vscreen_x) + "," + std::to_string(s_vscreen_y) + ")");

    s_is_dragging = false;

    s_capture_hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
        L"HodosQRCapture", nullptr, WS_POPUP,
        s_vscreen_x, s_vscreen_y, s_vscreen_w, s_vscreen_h,
        nullptr, nullptr, GetModuleHandle(nullptr), nullptr);

    if (!s_capture_hwnd) {
        LOG_ERROR_QR("Failed to create QR capture overlay window");
        // Reopen wallet and deliver error
        ShowWalletOverlay(-1, nullptr);
        DeliverResult("{\"status\":\"not_found\"}");
        return;
    }

    // Initial paint (full dark overlay)
    PaintOverlay();

    ShowWindow(s_capture_hwnd, SW_SHOW);
    SetForegroundWindow(s_capture_hwnd);
}

void FinishQRScreenCapture(bool cancelled, RECT selection) {
    // Destroy overlay
    if (s_capture_hwnd) {
        DestroyWindow(s_capture_hwnd);
        s_capture_hwnd = nullptr;
    }

    if (cancelled) {
        LOG_INFO_QR("QR screen capture cancelled by user");
        ShowWalletOverlay(-1, nullptr);
        DeliverResult("{\"status\":\"cancelled\"}");
        return;
    }

    int w = selection.right - selection.left;
    int h = selection.bottom - selection.top;
    LOG_INFO_QR("Capturing region: " + std::to_string(w) + "x" + std::to_string(h) +
                " at (" + std::to_string(selection.left) + "," + std::to_string(selection.top) + ")");

    if (w < 10 || h < 10) {
        LOG_WARNING_QR("Selection too small (" + std::to_string(w) + "x" + std::to_string(h) + ")");
        ShowWalletOverlay(-1, nullptr);
        DeliverResult("{\"status\":\"not_found\"}");
        return;
    }

    std::string resultJson = CaptureAndDecode(selection);

    // Reopen wallet overlay
    ShowWalletOverlay(-1, nullptr);

    if (resultJson.empty()) {
        LOG_INFO_QR("No BSV QR code found in selection");
        DeliverResult("{\"status\":\"not_found\"}");
    } else {
        LOG_INFO_QR("BSV QR code found: " + resultJson.substr(0, 200));
        DeliverResult("{\"status\":\"found\",\"result\":" + resultJson + "}");
    }
}

#endif // _WIN32
