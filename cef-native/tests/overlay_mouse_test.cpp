// Unit tests for hodos::PhysicalToView — beta.3 Phase 1, row P1-A3.
//
// The bug this guards against: the ~15 overlays are windowless CEF browsers whose view is
// reported to CEF in LOGICAL pixels, while their WndProcs receive PHYSICAL client pixels.
// Both are `int`. Nothing but this conversion distinguishes them.
//
// These tests are deliberately CEF-free and window-free (the JsStringEscape.h precedent),
// so they run in hodos_tests without a browser, a message loop or a monitor.

#include <gtest/gtest.h>

#include "../include/core/OverlayMouse.h"

namespace {

struct Pt {
    int x = 0;
    int y = 0;
};

Pt Convert(int px, int py, unsigned int dpi) {
    Pt p;
    hodos::PhysicalToView(px, py, dpi, p.x, p.y);
    return p;
}

}  // namespace

// 96 DPI is 100% scaling: the conversion must be the identity, or every non-scaled machine
// regresses. This is also why the defect survived so long — at 100% it is a no-op, so every
// check anyone ran on an unscaled monitor passed.
TEST(OverlayMouse, IdentityAt96Dpi) {
    EXPECT_EQ(Convert(0, 0, 96).x, 0);
    EXPECT_EQ(Convert(200, 400, 96).x, 200);
    EXPECT_EQ(Convert(200, 400, 96).y, 400);
    EXPECT_EQ(Convert(1919, 1079, 96).y, 1079);
}

// 125% — the scale on the display that produced the original report.
TEST(OverlayMouse, ScalesAt125Percent) {
    EXPECT_EQ(Convert(200, 400, 120).x, 160);
    EXPECT_EQ(Convert(200, 400, 120).y, 320);
    // The owner's actual failing click, from the probe log of 2026-08-25 09:58:40.
    // Pre-fix it was delivered as (228,411) and hit the balance panel; the Advanced button
    // sits at logical (176,330), proven by the same click succeeding at 100%.
    EXPECT_EQ(Convert(228, 411, 120).x, 182);
    EXPECT_EQ(Convert(228, 411, 120).y, 329);
}

TEST(OverlayMouse, ScalesAt150And175Percent) {
    EXPECT_EQ(Convert(300, 600, 144).x, 200);
    EXPECT_EQ(Convert(300, 600, 144).y, 400);
    EXPECT_EQ(Convert(350, 700, 168).x, 200);
    EXPECT_EQ(Convert(350, 700, 168).y, 400);
}

// A point below viewHeight*scale used to land outside the view entirely — elementFromPoint
// returned NONE and the click hit nothing. That is the "modal buttons unclickable on a
// small screen" report. After conversion the point must fall INSIDE the view.
TEST(OverlayMouse, PointNearBottomLandsInsideTheView) {
    const int physH = 1000;
    const unsigned int dpi = 120;
    const int viewH = static_cast<int>(physH / (dpi / 96.0));  // 800, as GetViewRect reports
    EXPECT_EQ(viewH, 800);

    EXPECT_GE(Convert(200, physH - 1, dpi).y, 0);
    EXPECT_LT(Convert(200, physH - 1, dpi).y, viewH)
        << "the bottom-most physical row must map inside the view, or the bottom strip of "
           "every overlay stays dead";
}

// Rounds to nearest rather than truncating, so clicks are not systematically biased toward
// the overlay's top-left. Half a logical pixel of bias is invisible on one control and
// compounds across a tall panel.
TEST(OverlayMouse, RoundsToNearestNotTowardZero) {
    // 411 * 96 / 120 = 328.8 -> 329, not 328.
    EXPECT_EQ(Convert(0, 411, 120).y, 329);
    // 5 * 96 / 120 = 4.0 exactly.
    EXPECT_EQ(Convert(0, 5, 120).y, 4);
}

// ScreenToClient can legitimately return a negative coordinate while the pointer is outside
// the window during a drag or a wheel event. The conversion must stay symmetric there
// rather than collapsing toward zero.
TEST(OverlayMouse, HandlesNegativeCoordinatesSymmetrically) {
    EXPECT_EQ(Convert(-200, -400, 120).x, -160);
    EXPECT_EQ(Convert(-200, -400, 120).y, -320);
}

// A DPI of 0 means GetDpiForWindow failed (an invalid or destroyed HWND). Treat it as 100%
// and pass the coordinate through unchanged: an unconverted click is a bug, but a click
// multiplied by a garbage scale is a click somewhere arbitrary on a live money-path modal.
TEST(OverlayMouse, ZeroDpiFailsSafeToIdentity) {
    EXPECT_EQ(Convert(200, 400, 0).x, 200);
    EXPECT_EQ(Convert(200, 400, 0).y, 400);
}
