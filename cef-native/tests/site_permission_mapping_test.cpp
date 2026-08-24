// beta.3 Phase 0.9 — the CEF-bit -> Hodos-type boundary.
//
// What this file is actually defending:
//
//  1. The STORED integers must never move. SitePermissionType values are written
//     into site_permissions.db. If someone renumbers them, every existing row
//     silently changes meaning — a stored "Block camera" becomes "Allow
//     notifications". There is no migration and no checksum to catch it, so the
//     only guard is a test that pins the numbers.
//
//  2. 1<<25 must NOT map to anything. cef_types.h declares
//     CEF_PERMISSION_TYPE_LOCAL_NETWORK_ACCESS only under CEF_API_ADDED(13600) and
//     renames it ..._DEPRECATED at 15000 (which is what we compile, having set no
//     explicit api_version). libcef's permission_prompt.cc :: GetCefRequestType has
//     no case emitting it at all. A mapping for it would be dead code that looks
//     load-bearing.
//
//  3. Loopback and LocalNetwork stay DISTINCT (owner decision, 2026-08-23):
//     "a server on this machine" and "a device on your LAN" are different risks
//     and get different copy, so collapsing them would make the consent surface
//     less true than the gate underneath it.
//
// ⛔ The mirrored bit values in SitePermissionMapping.h are proved against the real
// CEF constants by static_assert in simple_handler.cpp — that check CANNOT live
// here, because this target deliberately does not include CEF headers. This file
// tests the mapping's behavior; that static_assert tests its inputs. Both are
// required; neither substitutes for the other.

#include <gtest/gtest.h>

#include "core/SitePermissionMapping.h"

#include <set>
#include <string>

using hodos::siteperm::MaskToTypes;
using hodos::siteperm::PermCode;
namespace sp = hodos::siteperm;

// --- 1. Stored ids are frozen -------------------------------------------------

TEST(SitePermissionType, StoredIntegersAreFrozen) {
    // ⛔ If you are here because this test failed after you renumbered the enum:
    // do not update these numbers. Existing site_permissions.db rows carry the old
    // values and there is no migration. Add new enumerators at the end instead.
    EXPECT_EQ(1, static_cast<int>(SitePermissionType::Camera));
    EXPECT_EQ(2, static_cast<int>(SitePermissionType::Microphone));
    EXPECT_EQ(3, static_cast<int>(SitePermissionType::Location));
    EXPECT_EQ(4, static_cast<int>(SitePermissionType::Notifications));
    EXPECT_EQ(5, static_cast<int>(SitePermissionType::Clipboard));
    EXPECT_EQ(6, static_cast<int>(SitePermissionType::LocalNetwork));
    EXPECT_EQ(7, static_cast<int>(SitePermissionType::Loopback));
}

TEST(SitePermissionState, StoredIntegersAreFrozen) {
    EXPECT_EQ(0, static_cast<int>(SitePermissionState::Ask));
    EXPECT_EQ(1, static_cast<int>(SitePermissionState::Allow));
    EXPECT_EQ(2, static_cast<int>(SitePermissionState::Block));
}

// --- 2. Single-bit mapping ----------------------------------------------------

TEST(MaskToTypes, EachManagedBitMapsToItsType) {
    EXPECT_EQ(std::vector<SitePermissionType>{SitePermissionType::Camera},
              MaskToTypes(sp::kBitCameraStream));
    EXPECT_EQ(std::vector<SitePermissionType>{SitePermissionType::Microphone},
              MaskToTypes(sp::kBitMicStream));
    EXPECT_EQ(std::vector<SitePermissionType>{SitePermissionType::Location},
              MaskToTypes(sp::kBitGeolocation));
    EXPECT_EQ(std::vector<SitePermissionType>{SitePermissionType::Notifications},
              MaskToTypes(sp::kBitNotifications));
    EXPECT_EQ(std::vector<SitePermissionType>{SitePermissionType::Clipboard},
              MaskToTypes(sp::kBitClipboard));
    EXPECT_EQ(std::vector<SitePermissionType>{SitePermissionType::LocalNetwork},
              MaskToTypes(sp::kBitLocalNetwork));
    EXPECT_EQ(std::vector<SitePermissionType>{SitePermissionType::Loopback},
              MaskToTypes(sp::kBitLoopbackNetwork));
}

TEST(MaskToTypes, LoopbackAndLocalNetworkAreDistinct) {
    // The whole point of two stable ids: a grant for one must not imply the other.
    const auto loopback = MaskToTypes(sp::kBitLoopbackNetwork);
    const auto lan      = MaskToTypes(sp::kBitLocalNetwork);
    ASSERT_EQ(1u, loopback.size());
    ASSERT_EQ(1u, lan.size());
    EXPECT_NE(loopback[0], lan[0]);

    // And a request for both yields both, in a stable order.
    const auto both = MaskToTypes(sp::kBitLocalNetwork | sp::kBitLoopbackNetwork);
    ASSERT_EQ(2u, both.size());
    EXPECT_EQ(SitePermissionType::LocalNetwork, both[0]);
    EXPECT_EQ(SitePermissionType::Loopback,     both[1]);
}

TEST(MaskToTypes, PanTiltZoomFoldsIntoCameraAndDoesNotDuplicate) {
    // Pre-existing behavior, preserved: a stored camera decision governs PTZ too.
    EXPECT_EQ(std::vector<SitePermissionType>{SitePermissionType::Camera},
              MaskToTypes(sp::kBitCameraPanTiltZoom));
    EXPECT_EQ(std::vector<SitePermissionType>{SitePermissionType::Camera},
              MaskToTypes(sp::kBitCameraStream | sp::kBitCameraPanTiltZoom));
}

// --- 3. Everything we do NOT manage falls through -----------------------------

TEST(MaskToTypes, UnmanagedBitsYieldNothing) {
    EXPECT_TRUE(MaskToTypes(0u).empty());                 // CEF_PERMISSION_TYPE_NONE
    EXPECT_TRUE(MaskToTypes(1u << 13).empty());           // MIDI_SYSEX
    EXPECT_TRUE(MaskToTypes(1u << 20).empty());           // STORAGE_ACCESS
    EXPECT_TRUE(MaskToTypes(1u << 24).empty());           // FILE_SYSTEM_ACCESS
    EXPECT_TRUE(MaskToTypes(1u << 28).empty());           // SENSORS
}

TEST(MaskToTypes, DeprecatedLocalNetworkAccessBitIsNotMapped) {
    // 1<<25 is CEF_PERMISSION_TYPE_LOCAL_NETWORK_ACCESS_DEPRECATED at our API
    // version, and libcef never emits it. Mapping it would be dead code wearing a
    // security decision's clothes.
    EXPECT_TRUE(MaskToTypes(1u << 25).empty());
}

TEST(MaskToTypes, UnmanagedBitsDoNotContaminateManagedOnes) {
    // A mixed mask must yield exactly the managed subset — no more, no fewer.
    const auto types = MaskToTypes(sp::kBitLoopbackNetwork | (1u << 13) | (1u << 25));
    EXPECT_EQ(std::vector<SitePermissionType>{SitePermissionType::Loopback}, types);
}

// --- 4. Wire codes ------------------------------------------------------------

TEST(PermCode, EveryTypeHasAUniqueNonEmptyCode) {
    const SitePermissionType all[] = {
        SitePermissionType::Camera,        SitePermissionType::Microphone,
        SitePermissionType::Location,      SitePermissionType::Notifications,
        SitePermissionType::Clipboard,     SitePermissionType::LocalNetwork,
        SitePermissionType::Loopback,
    };
    std::set<std::string> seen;
    for (auto t : all) {
        const std::string code = PermCode(t);
        EXPECT_FALSE(code.empty()) << "type " << static_cast<int>(t) << " has no wire code";
        EXPECT_TRUE(seen.insert(code).second) << "duplicate wire code: " << code;
    }
    EXPECT_EQ(7u, seen.size());
}

TEST(PermCode, NewCodesMatchTheReactContract) {
    // ⛔ These exact strings appear in BRC100AuthOverlayRoot.tsx (PERM),
    // SiteInfoOverlayRoot.tsx (CAP_META) and kSitePermCaps. A silent rename here
    // does not break the build — it degrades the prompt to its generic
    // "access a device feature" fallback, which is a consent-surface defect.
    EXPECT_STREQ("loopback",      PermCode(SitePermissionType::Loopback));
    EXPECT_STREQ("local_network", PermCode(SitePermissionType::LocalNetwork));
}
