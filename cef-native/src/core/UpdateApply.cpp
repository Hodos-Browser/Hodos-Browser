// UpdateApply.cpp — pure (de)serialize for the apply-transaction model (commit 6b.1).
// See UpdateApply.h. No CEF, no process spawning, no filesystem here — lenient
// JSON only, so a corrupt control file degrades to a default rather than throwing.

#include "../../include/core/UpdateApply.h"

#include <algorithm>
#include <cctype>
#include <nlohmann/json.hpp>

namespace hodos {

using json = nlohmann::json;

const char* ApplyPhaseToString(ApplyPhase p) {
    switch (p) {
        case ApplyPhase::Preparing:      return "preparing";
        case ApplyPhase::Armed:          return "armed";
        case ApplyPhase::Installing:     return "installing";
        case ApplyPhase::AwaitingHealth: return "awaiting-health";
        case ApplyPhase::Healthy:        return "healthy";
        case ApplyPhase::RolledBack:     return "rolledback";
        case ApplyPhase::Aborted:        return "aborted";
        case ApplyPhase::None:
        default:                         return "none";
    }
}

ApplyPhase ApplyPhaseFromString(const std::string& s) {
    if (s == "preparing")       return ApplyPhase::Preparing;
    if (s == "armed")           return ApplyPhase::Armed;
    if (s == "installing")      return ApplyPhase::Installing;
    if (s == "awaiting-health") return ApplyPhase::AwaitingHealth;
    if (s == "healthy")         return ApplyPhase::Healthy;
    if (s == "rolledback")      return ApplyPhase::RolledBack;
    if (s == "aborted")         return ApplyPhase::Aborted;
    return ApplyPhase::None;
}

// ---- small lenient getters (never throw on a wrong/missing type) -------------
namespace {
std::string getStr(const json& j, const char* key, const std::string& dflt = "") {
    auto it = j.find(key);
    return (it != j.end() && it->is_string()) ? it->get<std::string>() : dflt;
}
long getLong(const json& j, const char* key, long dflt = 0) {
    auto it = j.find(key);
    return (it != j.end() && it->is_number_integer()) ? it->get<long>() : dflt;
}
int getInt(const json& j, const char* key, int dflt = 0) {
    auto it = j.find(key);
    return (it != j.end() && it->is_number_integer()) ? it->get<int>() : dflt;
}
bool getBool(const json& j, const char* key, bool dflt = false) {
    auto it = j.find(key);
    return (it != j.end() && it->is_boolean()) ? it->get<bool>() : dflt;
}
}  // namespace

// ---- ApplyRecord ------------------------------------------------------------
std::string SerializeApplyRecord(const ApplyRecord& r) {
    json j;
    j["schema"] = r.schema;
    j["phase"] = ApplyPhaseToString(r.phase);
    j["fromBuild"] = r.fromBuild;
    j["toBuild"] = r.toBuild;
    j["installerPath"] = r.installerPath;
    j["rollbackDir"] = r.rollbackDir;
    j["rollbackManifestPath"] = r.rollbackManifestPath;
    j["expectedNewManifestPath"] = r.expectedNewManifestPath;
    j["profileId"] = r.profileId;
    j["stagedAt"] = r.stagedAt;
    j["failureReason"] = r.failureReason;
    return j.dump(2);
}

bool ParseApplyRecord(const std::string& jsonStr, ApplyRecord& out) {
    json j = json::parse(jsonStr, nullptr, /*allow_exceptions=*/false);
    if (j.is_discarded() || !j.is_object()) return false;
    out.schema = getInt(j, "schema", 1);
    out.phase = ApplyPhaseFromString(getStr(j, "phase", "none"));
    out.fromBuild = getLong(j, "fromBuild");
    out.toBuild = getLong(j, "toBuild");
    out.installerPath = getStr(j, "installerPath");
    out.rollbackDir = getStr(j, "rollbackDir");
    out.rollbackManifestPath = getStr(j, "rollbackManifestPath");
    out.expectedNewManifestPath = getStr(j, "expectedNewManifestPath");
    out.profileId = getStr(j, "profileId");
    out.stagedAt = getStr(j, "stagedAt");
    out.failureReason = getStr(j, "failureReason");
    return true;
}

// ---- UpdateState ------------------------------------------------------------
std::string SerializeUpdateState(const UpdateState& s) {
    json j;
    j["schema"] = s.schema;
    j["silent"] = s.silent;
    j["paused"] = s.paused;
    j["highWaterBuild"] = s.highWaterBuild;
    j["signerThumbprint"] = s.signerThumbprint;
    j["lastFailureBuild"] = s.lastFailureBuild;
    j["lastFailureReason"] = s.lastFailureReason;
    j["rescanAfterRollback"] = s.rescanAfterRollback;
    return j.dump(2);
}

bool ParseUpdateState(const std::string& jsonStr, UpdateState& out) {
    json j = json::parse(jsonStr, nullptr, /*allow_exceptions=*/false);
    if (j.is_discarded() || !j.is_object()) return false;
    out.schema = getInt(j, "schema", 1);
    out.silent = getBool(j, "silent");
    out.paused = getBool(j, "paused");
    out.highWaterBuild = getLong(j, "highWaterBuild");
    out.signerThumbprint = getStr(j, "signerThumbprint");
    out.lastFailureBuild = getLong(j, "lastFailureBuild");
    out.lastFailureReason = getStr(j, "lastFailureReason");
    out.rescanAfterRollback = getBool(j, "rescanAfterRollback");
    return true;
}

// ---- FileManifest -----------------------------------------------------------
std::string NormalizeManifestKey(const std::string& relPath) {
    std::string s = relPath;
    for (char& c : s) {
        if (c == '\\') c = '/';
        c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    }
    // strip a leading "./" or "/"
    while (s.rfind("./", 0) == 0) s.erase(0, 2);
    while (!s.empty() && s.front() == '/') s.erase(0, 1);
    return s;
}

std::string SerializeManifest(const FileManifest& m) {
    json j;
    j["schema"] = 1;
    json files = json::object();
    for (const auto& kv : m.entries) files[kv.first] = kv.second;
    j["files"] = files;
    return j.dump(2);
}

bool ParseManifest(const std::string& jsonStr, FileManifest& out) {
    json j = json::parse(jsonStr, nullptr, /*allow_exceptions=*/false);
    if (j.is_discarded() || !j.is_object()) return false;
    out.entries.clear();
    auto it = j.find("files");
    if (it == j.end() || !it->is_object()) return false;
    for (auto& el : it->items()) {
        if (el.value().is_string()) {
            out.entries[NormalizeManifestKey(el.key())] = el.value().get<std::string>();
        }
    }
    return true;
}

}  // namespace hodos
