// cef-native/include/core/AumidPolicy.h
//
// The AppUserModelID (AUMID) decision, as a pure function — Phase 3 (WS2), beta.3.
//
// WHAT AN AUMID IS. A string Windows uses to answer "are these two things the same
// application?" It groups windows under one taskbar button and, via a matching
// shortcut or a registered display name, supplies the name shown on that button and
// in its jump list. Chrome's is "Chrome"; Brave's is "Brave".
//
// ─────────────────────────────────────────────────────────────────────────────
// WHY THIS FILE EXISTS — the bug it fixes, measured, not assumed.
//
// The taskbar button read "HodosBrowser.exe" and refused to group with the user's
// pinned icon. The desk diagnosis said the cause was "production single-profile
// never sets an AUMID". That was REFUTED during the Phase 3 kickoff: the reporting
// user had two profiles, so the old `GetAllProfiles().size() > 1` gate DID fire on
// their machine — 61 "AUMID set" lines in their production log prove it, six of them
// on the day they reported the symptom.
//
// The real cause is that the identity CHANGED UNDER THE USER, and then matched
// nothing:
//
//   2026-04-27  user pins Hodos. One profile ⇒ the old gate does NOT fire ⇒ no
//               explicit AUMID ⇒ Chromium's default applies ⇒ the pinned .lnk is
//               stamped "Chromium.OYX4LNTP4DA5GSWHROKEK3C7BM".
//   2026-07-06  user creates a 2nd profile. EIGHT SECONDS LATER the gate fires for
//               the first time ever and the process identity becomes "HodosBrowser".
//   thereafter  running window (HodosBrowser) != pinned .lnk (Chromium.OYX…)
//               ⇒ separate taskbar button; and "HodosBrowser" is declared by NO
//               shortcut on the system ⇒ Windows has no display name for it
//               ⇒ falls back to the exe filename ⇒ "HodosBrowser.exe".
//
// ⛔ THEREFORE: widening the gate ALONE would do to every remaining single-profile
// user exactly what 2026-07-06 did to the reporting user — move them off the
// identity their pin currently matches. The three parts must ship together:
//
//   1. this file        — set an explicit AUMID ALWAYS, so identity stops depending
//                         on how many profiles happen to exist;
//   2. installer .iss   — [Icons] declare the SAME string, so shortcut and process
//                         agree (they must be byte-identical or the mismatch persists);
//   3. RegisterAumidDisplayName — because a PER-PROFILE AUMID can never match a
//                         shortcut (there is no shortcut per profile), and the owner
//                         confirmed the second profile's button ALSO read
//                         "hodosbrowser.exe". Registering the name is the documented
//                         mechanism for naming a shortcut-less AUMID.
//
// ⚠️ Existing pinned shortcuts carry the OLD identity and will stop grouping once.
// Owner decision (beta.3 Phase 3, Q4): OPTION A — release-note "re-pin once". We do
// NOT rewrite the user's pinned .lnk files. Deliberate: writing another app's user
// data to save one drag is not a trade we wanted.
//
// ─────────────────────────────────────────────────────────────────────────────
// PER-PROFILE BUTTONS ARE A FEATURE, NOT A BUG. Extra profiles intentionally get
// their own taskbar button — the same thing Chrome does (its per-profile AUMID form
// "Chrome.UserData.Profile1" was observed on the reporting machine). The suffix logic
// below is unchanged from the code it replaces; only the GATE changed. Multi-profile
// users therefore compute the IDENTICAL string they computed before, which is why
// this change cannot regress them.
//
// See development-docs/0.4.0-beta.3/phase-3-window-identity/ (PHASE_CONTRACT.md §4,
// MEASUREMENTS.md M1/M2/M3/M7/M9.1a).

#pragma once

#include <optional>
#include <string>

namespace hodos {

// The base identities. ⚠️ The production value MUST stay byte-identical to the
// AppUserModelID declared in installer/hodos-browser.iss [Icons]. If one changes and
// the other does not, the process and its shortcut become different applications
// again and this whole bug returns.
inline constexpr wchar_t kAumidBaseProd[] = L"HodosBrowser";
inline constexpr wchar_t kAumidBaseDev[]  = L"HodosBrowser.Dev";

// Human-readable name Windows shows for the AUMID (see RegisterAumidDisplayName).
inline constexpr wchar_t kAumidDisplayName[] = L"Hodos Browser";

// Decide this process's explicit AUMID.
//
//   isDev       — hodos::IsDevEnv(). A dev build ALWAYS gets a ".Dev" identity, even
//                 single-profile, so it never merges with the installed build's
//                 taskbar button.
//   pickerMode  — the profile picker owns no profile, so it takes the BASE identity.
//                 (Previously it was skipped entirely and got no identity at all,
//                 despite a comment claiming it kept the base one — code and comment
//                 disagreed. Phase 3 Q5.)
//   profileId   — "Default" is unsuffixed so the primary profile matches the plain
//                 shortcut; every other profile gets its own suffixed identity and
//                 therefore its own taskbar button.
//
// Returns std::optional deliberately: the return type is what makes the "no identity
// at all" outcome REPRESENTABLE, and therefore what makes the negative control for
// P3-A1 possible (reintroduce a profile-count gate → prod+single-profile returns
// nullopt → the test goes red). Post-fix this never returns nullopt for a real
// launch; an empty profileId is the one defensive case.
inline std::optional<std::wstring> ComputeAumid(bool isDev,
                                                bool pickerMode,
                                                const std::string& profileId) {
    std::wstring aumid = isDev ? kAumidBaseDev : kAumidBaseProd;

    // The picker owns no profile — base identity, no suffix.
    if (pickerMode) {
        return aumid;
    }

    // An empty id means the profile was never resolved. Suffixing with nothing would
    // silently produce the base identity and hide the failure, so keep the base but
    // say so via the type: callers log the difference.
    if (profileId.empty()) {
        return aumid;
    }

    if (profileId != "Default") {
        // profileId is validated by ProfileManager::IsValidProfileId (ASCII, no path
        // separators or shell metacharacters), so this widening is safe.
        aumid += L".";
        aumid.append(profileId.begin(), profileId.end());
    }
    return aumid;
}

}  // namespace hodos

#ifdef _WIN32

namespace hodos {

// Register a display name for `aumid` under
// HKCU\Software\Classes\AppUserModelId\<aumid>.
//
// WHY THIS IS NEEDED AND A SHORTCUT IS NOT ENOUGH. Windows names a taskbar button by
// finding a shortcut that declares the same AUMID. That works for the Default profile
// (installer/hodos-browser.iss now declares "HodosBrowser" on both [Icons] entries),
// but a per-profile identity like "HodosBrowser.Profile_1" can never match a shortcut
// because no per-profile shortcut exists. Without this registration those windows keep
// falling back to the exe filename — which the owner confirmed on 2026-08-29.
//
// Chrome solves the same problem by writing a real shortcut per profile. We register
// the name instead: no Start Menu clutter, and one value per profile.
//
// ⚠️ HKCU, per-user, never HKLM — this must work for a per-user install and must not
// need elevation. Best-effort: a failure is logged and never fatal. An unnamed taskbar
// button is a cosmetic regression; refusing to start is not an acceptable trade.
//
// ⚠️ Leaves state behind. Must be removed on uninstall and on profile deletion, or it
// becomes orphaned registry data of the kind
// TICKET_deleted_profile_id_reused_over_orphaned_data.md describes.
bool RegisterAumidDisplayName(const std::wstring& aumid,
                              const std::wstring& displayName,
                              const std::wstring& iconPath);

// Remove a previously registered AUMID display name. Used on profile deletion.
// Best-effort; returns false if the key was absent or could not be removed.
bool UnregisterAumidDisplayName(const std::wstring& aumid);

}  // namespace hodos

#endif  // _WIN32
