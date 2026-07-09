// splash.h - minimal native "Hodos is updating..." window for the update helper (Windows).
//
// The silent-apply supervisor (hodos-update-helper.exe) is the only process alive for the
// whole apply (the bootstrap browser has _exit(0)'d; the new browser hasn't shown yet), so
// it owns the "we're installing, one moment" indicator that removes the otherwise-confusing
// several-second pause on an apply-boot. RAII: construct at the start of the visible install
// work, destruct (auto-close) when RunApplyTransaction returns - so SUCCESS reveals the new
// browser and ROLLBACK reveals the relaunched old one.
//
// Deliberately dependency-light: a self-drawn GDI marquee (no comctl32 / InitCommonControls),
// on its own thread with a message pump. Gated by env HODOS_UPDATE_NO_SPLASH (rigs/CI set it).
#pragma once

#include <windows.h>
#include <atomic>
#include <cstdlib>
#include <string>
#include <thread>

class UpdateSplash {
public:
    // title/subtitle default to the committed-apply wording ("Hodos is updating..."). The
    // silent-apply bootstrap passes NEUTRAL pre-commit wording ("Hodos is starting...") for the
    // splash it raises across the picker-exit wait + verify gates, where it may still DEFER and
    // fall through to normal startup (so "updating" would be a lie). Charset-independent \u
    // escapes so the WIDE literals render correctly in BOTH targets - the shell target compiles
    // this header WITHOUT /utf-8 (only hodos-update-helper has it), which would otherwise mojibake
    // a raw-UTF-8 L"..." literal by decoding the source bytes under the system code page.
    explicit UpdateSplash(const wchar_t* title = L"Hodos is updating\u2026",
                          const wchar_t* subtitle =
                              L"This only takes a moment \u2014 please don\u2019t power off.")
        : title_(title ? title : L""), subtitle_(subtitle ? subtitle : L"") {
        // Suppress in rigs / headless CI (they set this); production apply leaves it unset.
        if (const char* q = std::getenv("HODOS_UPDATE_NO_SPLASH"); q && *q && std::string(q) != "0") {
            return;
        }
        // The splash is cosmetic but is constructed at a brick-sensitive moment (the apply
        // bootstrap, before/after the {app} backup + install). A pathological thread-create
        // failure must NEVER propagate out into the apply path - degrade to "no splash".
        try {
            thread_ = std::thread([this] { Run(); });
        } catch (...) {
            // no splash this cycle; the update proceeds normally (dtor sees no joinable thread)
        }
    }
    ~UpdateSplash() {
        stop_.store(true);  // the ~30fps timer keeps GetMessage responsive, so the loop
                            // notices this within a frame and tears the window down.
        HWND h = hwnd_.load();
        if (h) PostMessageW(h, WM_CLOSE, 0, 0);
        if (thread_.joinable()) thread_.join();
    }
    UpdateSplash(const UpdateSplash&) = delete;
    UpdateSplash& operator=(const UpdateSplash&) = delete;

private:
    std::wstring title_;
    std::wstring subtitle_;
    std::thread thread_;
    std::atomic<bool> stop_{false};
    std::atomic<HWND> hwnd_{nullptr};
    int marquee_ = 0;

    static LRESULT CALLBACK WndProc(HWND h, UINT m, WPARAM w, LPARAM l) {
        auto* self = reinterpret_cast<UpdateSplash*>(GetWindowLongPtrW(h, GWLP_USERDATA));
        switch (m) {
            case WM_TIMER:
                if (self) { self->marquee_ = (self->marquee_ + 5) % 300; InvalidateRect(h, nullptr, FALSE); }
                return 0;
            case WM_PAINT: {
                PAINTSTRUCT ps; HDC dc = BeginPaint(h, &ps);
                RECT rc; GetClientRect(h, &rc);
                HBRUSH bg = CreateSolidBrush(RGB(32, 34, 37));
                FillRect(dc, &rc, bg); DeleteObject(bg);
                SetBkMode(dc, TRANSPARENT);

                HFONT ft = CreateFontW(-19, 0, 0, 0, FW_SEMIBOLD, 0, 0, 0, DEFAULT_CHARSET,
                                       0, 0, CLEARTYPE_QUALITY, 0, L"Segoe UI");
                HGDIOBJ of = SelectObject(dc, ft);
                SetTextColor(dc, RGB(236, 236, 238));
                RECT t1 = rc; t1.top += 30;
                const wchar_t* line1 = self ? self->title_.c_str() : L"Hodos is updating\u2026";
                DrawTextW(dc, line1, -1, &t1, DT_CENTER | DT_TOP | DT_SINGLELINE);
                SelectObject(dc, of); DeleteObject(ft);

                HFONT ft2 = CreateFontW(-12, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                                        0, 0, CLEARTYPE_QUALITY, 0, L"Segoe UI");
                of = SelectObject(dc, ft2);
                SetTextColor(dc, RGB(158, 158, 164));
                RECT t2 = rc; t2.top += 60;
                const wchar_t* line2 = self ? self->subtitle_.c_str()
                    : L"This only takes a moment \u2014 please don\u2019t power off.";
                DrawTextW(dc, line2, -1, &t2, DT_CENTER | DT_TOP | DT_SINGLELINE);
                SelectObject(dc, of); DeleteObject(ft2);

                // Indeterminate marquee: a gold block sliding along a dark track.
                const int pad = 44, trackY = rc.bottom - 34, trackH = 6;
                RECT track = { rc.left + pad, trackY, rc.right - pad, trackY + trackH };
                HBRUSH tb = CreateSolidBrush(RGB(58, 61, 66)); FillRect(dc, &track, tb); DeleteObject(tb);
                const int trackW = (rc.right - pad) - (rc.left + pad);
                const int blockW = trackW / 4;
                int bx = rc.left + pad + (self ? (self->marquee_ * trackW / 300) : 0) - blockW / 2;
                RECT block = { bx, trackY, bx + blockW, trackY + trackH };
                if (block.left < rc.left + pad) block.left = rc.left + pad;
                if (block.right > rc.right - pad) block.right = rc.right - pad;
                HBRUSH bb = CreateSolidBrush(RGB(212, 175, 55));  // Hodos gold
                FillRect(dc, &block, bb); DeleteObject(bb);

                EndPaint(h, &ps);
                return 0;
            }
            case WM_DESTROY: PostQuitMessage(0); return 0;
        }
        return DefWindowProcW(h, m, w, l);
    }

    void Run() {
        const wchar_t* kCls = L"HodosUpdateSplash";
        WNDCLASSW wc = {};
        wc.lpfnWndProc = WndProc;
        wc.hInstance = GetModuleHandleW(nullptr);
        wc.hCursor = LoadCursorW(nullptr, IDC_WAIT);
        wc.lpszClassName = kCls;
        RegisterClassW(&wc);

        const int W = 380, H = 168;
        const int sx = GetSystemMetrics(SM_CXSCREEN), sy = GetSystemMetrics(SM_CYSCREEN);
        HWND h = CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW, kCls, L"Hodos",
                                 WS_POPUP | WS_VISIBLE, (sx - W) / 2, (sy - H) / 2, W, H,
                                 nullptr, nullptr, wc.hInstance, nullptr);
        if (!h) { UnregisterClassW(kCls, wc.hInstance); return; }  // headless/CI: no desktop
        SetWindowLongPtrW(h, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(this));
        hwnd_.store(h);
        SetTimer(h, 1, 33, nullptr);  // ~30 fps
        ShowWindow(h, SW_SHOWNOACTIVATE);  // visible but don't steal focus
        UpdateWindow(h);

        MSG msg;
        while (!stop_.load() && GetMessageW(&msg, nullptr, 0, 0) > 0) {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        KillTimer(h, 1);
        hwnd_.store(nullptr);
        if (IsWindow(h)) DestroyWindow(h);
        UnregisterClassW(kCls, wc.hInstance);
    }
};
