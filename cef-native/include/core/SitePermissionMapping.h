#pragma once

// beta.3 Phase 0.9 — the CEF-bit -> Hodos-type boundary, extracted so it is
// unit-testable without CEF (same reason QRPayloadClassify.h was extracted in
// P0.6: the mapping is pure, the callback it lives in is not).
//
// ⛔ WHY THE BITS ARE MIRRORED HERE RATHER THAN INCLUDED:
// SitePermissionType is deliberately DECOUPLED from cef_permission_request_types_t
// so a Chromium bump that renumbers CEF's bitflags cannot corrupt stored SQLite
// rows. This header therefore carries its own copies of the CEF bit values, and
// simple_handler.cpp — the one translation unit that includes BOTH this header and
// cef_types.h — static_assert's every mirror against the real CEF constant. A
// renumber becomes a COMPILE ERROR instead of a silent mis-grant.
//
// ⚠️ There is no CEF bit for 1<<25 in this map on purpose. cef_types.h declares
// CEF_PERMISSION_TYPE_LOCAL_NETWORK_ACCESS only under CEF_API_ADDED(13600) and
// renames it ..._DEPRECATED at CEF_API_ADDED(15000) — and we build with no
// explicit api_version, i.e. CEF_API_VERSION_EXPERIMENTAL, so the DEPRECATED name
// is the one that exists. More decisively, libcef's own
// permission_prompt.cc :: GetCefRequestType has NO case that emits 1<<25, so it
// can never arrive. Only LOCAL_NETWORK (1<<26) and LOOPBACK_NETWORK (1<<27) do.

#include "SitePermissionType.h"

#include <cstdint>
#include <vector>

namespace hodos {
namespace siteperm {

// Mirrors of cef_permission_request_types_t. Guarded by static_assert in
// simple_handler.cpp — do not "fix" a mismatch here, fix the mapping.
inline constexpr uint32_t kBitCameraPanTiltZoom = 1u << 1;
inline constexpr uint32_t kBitCameraStream      = 1u << 2;
inline constexpr uint32_t kBitClipboard         = 1u << 4;
inline constexpr uint32_t kBitGeolocation       = 1u << 8;
inline constexpr uint32_t kBitMicStream         = 1u << 12;
inline constexpr uint32_t kBitNotifications     = 1u << 15;
inline constexpr uint32_t kBitLocalNetwork      = 1u << 26;
inline constexpr uint32_t kBitLoopbackNetwork   = 1u << 27;

// Map a CEF requested-permissions mask to the Hodos-managed types it covers.
// Bits we do not manage (MIDI, USB, storage access, …) are ignored, and an
// all-unmanaged mask yields an empty vector — the caller's signal to defer to
// Chromium's stock prompt.
//
// PTZ is folded into Camera on purpose so a stored camera decision governs
// pan/tilt/zoom too (pre-existing behavior, preserved).
inline std::vector<SitePermissionType> MaskToTypes(uint32_t mask) {
    std::vector<SitePermissionType> types;
    if (mask & (kBitCameraStream | kBitCameraPanTiltZoom)) types.push_back(SitePermissionType::Camera);
    if (mask & kBitMicStream)      types.push_back(SitePermissionType::Microphone);
    if (mask & kBitGeolocation)    types.push_back(SitePermissionType::Location);
    if (mask & kBitNotifications)  types.push_back(SitePermissionType::Notifications);
    if (mask & kBitClipboard)      types.push_back(SitePermissionType::Clipboard);
    if (mask & kBitLocalNetwork)   types.push_back(SitePermissionType::LocalNetwork);
    if (mask & kBitLoopbackNetwork) types.push_back(SitePermissionType::Loopback);
    return types;
}

// Stable wire code for a type: the `perm=` overlay URL param AND the site-info
// hub's capability code. ⛔ These strings are a contract with
// BRC100AuthOverlayRoot.tsx (PERM map), SiteInfoOverlayRoot.tsx (PERM_META) and
// useSitePermissions.ts — changing one without the others silently degrades the
// prompt to its generic fallback copy.
inline const char* PermCode(SitePermissionType t) {
    switch (t) {
        case SitePermissionType::Camera:        return "camera";
        case SitePermissionType::Microphone:    return "microphone";
        case SitePermissionType::Location:      return "location";
        case SitePermissionType::Notifications: return "notifications";
        case SitePermissionType::Clipboard:     return "clipboard";
        case SitePermissionType::LocalNetwork:  return "local_network";
        case SitePermissionType::Loopback:      return "loopback";
    }
    return "";
}

}  // namespace siteperm
}  // namespace hodos
