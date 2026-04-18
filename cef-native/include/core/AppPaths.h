#pragma once
#include <cstdlib>
#include <string>
#include <iostream>

namespace AppPaths {

inline std::string GetAppDirName() {
    const char* dev = std::getenv("HODOS_DEV");
    if (dev && std::string(dev) == "1") {
        return "HodosBrowserDev";
    }
    return "HodosBrowser";
}

/// Safeguard: if running from a dev build directory, require HODOS_DEV=1.
/// The installed app runs from Program Files / app bundle, so this won't trigger for users.
/// Returns true if safe to proceed, false if the process should exit.
inline bool EnforceDevSafeguard(const std::string& exe_path) {
    bool is_dev_build = (exe_path.find("build\\bin\\Release") != std::string::npos)
                     || (exe_path.find("build/bin/Release") != std::string::npos)
                     || (exe_path.find("build\\bin\\Debug") != std::string::npos)
                     || (exe_path.find("build/bin/Debug") != std::string::npos)
                     || (exe_path.find("build/bin/HodosBrowser") != std::string::npos);
    if (!is_dev_build) return true;

    const char* dev = std::getenv("HODOS_DEV");
    if (dev && std::string(dev) == "1") return true;

    std::cerr << "========================================================" << std::endl;
    std::cerr << "  DEV SAFEGUARD: HODOS_DEV=1 is not set!" << std::endl;
    std::cerr << "  Running a dev build without it would use the" << std::endl;
    std::cerr << "  production database and risk corrupting real data." << std::endl;
    std::cerr << std::endl;
    std::cerr << "  Use the launcher script instead:" << std::endl;
    std::cerr << "    Windows: .\\win_build_run.sh" << std::endl;
    std::cerr << "    Mac:     ./mac_build_run.sh" << std::endl;
    std::cerr << "========================================================" << std::endl;
    return false;
}

} // namespace AppPaths
