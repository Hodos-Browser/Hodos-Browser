cmake_minimum_required(VERSION 3.15)

# ============================================================================
# IMPORTANT: vcpkg configuration MUST come BEFORE project() command
# ============================================================================

# Set vcpkg triplet (platform-specific)
if(NOT DEFINED VCPKG_TARGET_TRIPLET)
    if(WIN32)
        set(VCPKG_TARGET_TRIPLET "x64-windows-static" CACHE STRING "")
    elseif(APPLE)
        # macOS uses arm64 (M1/M2/M3) or x64 (Intel)
        if(CMAKE_SYSTEM_PROCESSOR MATCHES "arm64|aarch64")
            set(VCPKG_TARGET_TRIPLET "arm64-osx" CACHE STRING "")
        else()
            set(VCPKG_TARGET_TRIPLET "x64-osx" CACHE STRING "")
        endif()
    endif()
endif()

# Portable vcpkg toolchain configuration
# Set via environment variable VCPKG_ROOT or pass via command line:
# cmake -DCMAKE_TOOLCHAIN_FILE=/path/to/vcpkg/scripts/buildsystems/vcpkg.cmake
if(NOT DEFINED CMAKE_TOOLCHAIN_FILE)
    if(DEFINED ENV{VCPKG_ROOT})
        set(CMAKE_TOOLCHAIN_FILE "$ENV{VCPKG_ROOT}/scripts/buildsystems/vcpkg.cmake" CACHE STRING "")
        message(STATUS "Using vcpkg from VCPKG_ROOT: $ENV{VCPKG_ROOT}")
    else()
        message(WARNING "vcpkg not found. Set VCPKG_ROOT environment variable or pass -DCMAKE_TOOLCHAIN_FILE=...")
    endif()
else()
    message(STATUS "Using vcpkg toolchain: ${CMAKE_TOOLCHAIN_FILE}")
endif()

message(STATUS "vcpkg triplet: ${VCPKG_TARGET_TRIPLET}")

# ============================================================================
# Now safe to call project()
# ============================================================================

project(HodosBrowserShell)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# macOS-specific settings
if(APPLE)
    set(CMAKE_OSX_DEPLOYMENT_TARGET "10.15" CACHE STRING "Minimum macOS deployment version")
    set(CMAKE_OSX_ARCHITECTURES "${CMAKE_SYSTEM_PROCESSOR}" CACHE STRING "macOS architecture")
    message(STATUS "macOS deployment target: ${CMAKE_OSX_DEPLOYMENT_TARGET}")
    message(STATUS "macOS architecture: ${CMAKE_OSX_ARCHITECTURES}")
endif()

# Paths
set(CEF_ROOT "../cef-binaries")
set(OUTPUT_DIR "$<TARGET_FILE_DIR:HodosBrowserShell>")

# Platform-specific paths
if(APPLE)
    # macOS: Wrapper built via main CEF CMakeLists.txt
    set(WRAPPER_LIB_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../cef-binaries/build/libcef_dll_wrapper")
    set(CEF_FRAMEWORK_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../cef-binaries/Release/Chromium Embedded Framework.framework")
    set(RESOURCES_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../cef-binaries/Resources")
elseif(WIN32)
    # Windows: Wrapper built separately in wrapper subdirectory
    set(WRAPPER_LIB_PATH "${CEF_ROOT}/libcef_dll/wrapper/build/Release")
    set(RELEASE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../cef-binaries/Release")
    set(RESOURCES_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../cef-binaries/Resources")
endif()

set(OPENSSL_USE_STATIC_LIBS TRUE)
# set(OPENSSL_MSVC_STATIC_RT TRUE)

# Find required packages (platform-specific)
find_package(OpenSSL REQUIRED)

if(APPLE)
    # macOS: Use Homebrew packages (header-only for nlohmann_json)
    find_path(NLOHMANN_JSON_INCLUDE_DIR
        NAMES nlohmann/json.hpp
        PATHS /opt/homebrew/include /usr/local/include
        REQUIRED)

    find_library(SQLITE3_LIBRARY
        NAMES sqlite3
        PATHS /opt/homebrew/lib /usr/local/lib
        REQUIRED)

    message(STATUS "nlohmann_json include: ${NLOHMANN_JSON_INCLUDE_DIR}")
    message(STATUS "sqlite3 library: ${SQLITE3_LIBRARY}")

elseif(WIN32)
    # Windows: Use vcpkg packages
    find_package(nlohmann_json CONFIG REQUIRED)
    find_package(unofficial-sqlite3 CONFIG REQUIRED)
endif()


message(STATUS "OpenSSL include dir: ${OPENSSL_INCLUDE_DIR}")
message(STATUS "OpenSSL libraries: ${OPENSSL_LIBRARIES}")

# Output
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

# Shell source - Cross-platform files
set(SOURCES
    # CEF handlers (cross-platform)
    src/handlers/simple_handler.cpp
    src/handlers/simple_render_process_handler.cpp
    src/handlers/simple_app.cpp
)

# Platform-specific handler files
if(APPLE)
    list(APPEND SOURCES src/handlers/my_overlay_render_handler.mm)
elseif(WIN32)
    list(APPEND SOURCES src/handlers/my_overlay_render_handler.cpp)
endif()

# Platform-specific sources
if(APPLE)
    # macOS-specific files
    list(APPEND SOURCES cef_browser_shell_mac.mm)
    message(STATUS "Adding macOS-specific source: cef_browser_shell_mac.mm")

    # TODO: Port these to macOS or create macOS equivalents
    # - TabManager (tab system)
    # - WalletService (HTTP client)
    # - HistoryManager (browser history)
    # - HTTP/WebSocket handlers

elseif(WIN32)
    # Windows-specific files
    list(APPEND SOURCES
        cef_browser_shell.cpp
        src/core/WalletService.cpp
        src/core/IdentityHandler.cpp
        src/core/NavigationHandler.cpp
        src/core/AddressHandler.cpp
        src/core/BRC100Bridge.cpp
        src/core/BRC100Handler.cpp
        src/core/HttpRequestInterceptor.cpp
        src/core/WebSocketServerHandler.cpp
        include/core/Tab.h
        include/core/TabManager.h
        src/core/TabManager.cpp
        include/core/HistoryManager.h
        src/core/HistoryManager.cpp
    )
    message(STATUS "Adding Windows-specific source: cef_browser_shell.cpp + core modules")
endif()

# Create executable (platform-specific)
if(APPLE)
    add_executable(HodosBrowserShell MACOSX_BUNDLE ${SOURCES})
    set_target_properties(HodosBrowserShell PROPERTIES
        MACOSX_BUNDLE_INFO_PLIST "${CMAKE_CURRENT_SOURCE_DIR}/Info.plist"
        MACOSX_BUNDLE_BUNDLE_NAME "HodosBrowser"
        MACOSX_BUNDLE_BUNDLE_VERSION "1.0"
        MACOSX_BUNDLE_SHORT_VERSION_STRING "1.0"
    )
elseif(WIN32)
    add_executable(HodosBrowserShell WIN32 ${SOURCES})
    set_target_properties(HodosBrowserShell PROPERTIES
        MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    )
else()
    add_executable(HodosBrowserShell ${SOURCES})
endif()

# Include CEF headers and project headers
target_include_directories(HodosBrowserShell PRIVATE
    ${CEF_ROOT}
    ${CEF_ROOT}/include
    ${CEF_ROOT}/include/base
    ${CMAKE_CURRENT_SOURCE_DIR}/include/core
    ${CMAKE_CURRENT_SOURCE_DIR}/include/handlers
    ${CMAKE_CURRENT_SOURCE_DIR}/src/core
    ${CMAKE_CURRENT_SOURCE_DIR}/src/handlers
    ${CMAKE_CURRENT_SOURCE_DIR}/tests
)

# Platform-specific includes
if(APPLE)
    target_include_directories(HodosBrowserShell PRIVATE
        ${NLOHMANN_JSON_INCLUDE_DIR}
    )
endif()

# Platform-specific linking
if(APPLE)
    # macOS: Link CEF Framework and system frameworks
    target_link_directories(HodosBrowserShell PRIVATE
        "${WRAPPER_LIB_PATH}"
    )

    # Find macOS frameworks
    find_library(COCOA_LIBRARY Cocoa)
    find_library(APPKIT_LIBRARY AppKit)
    find_library(FOUNDATION_LIBRARY Foundation)
    find_library(COREGRAPHICS_LIBRARY CoreGraphics)
    find_library(QUARTZCORE_LIBRARY QuartzCore)

    # Link CEF Framework (special handling for .framework bundle)
    # Note: Framework name has spaces, use find_library to handle properly
    find_library(CEF_LIBRARY
        NAMES "Chromium Embedded Framework"
        PATHS "${CEF_ROOT}/Release"
        NO_DEFAULT_PATH
    )

    if(NOT CEF_LIBRARY)
        message(FATAL_ERROR "CEF framework not found at ${CEF_ROOT}/Release")
    endif()

    message(STATUS "CEF framework found: ${CEF_LIBRARY}")

    target_link_libraries(HodosBrowserShell PRIVATE
        "${WRAPPER_LIB_PATH}/libcef_dll_wrapper.a"
        "${CEF_LIBRARY}"
        ${COCOA_LIBRARY}
        ${APPKIT_LIBRARY}
        ${FOUNDATION_LIBRARY}
        ${COREGRAPHICS_LIBRARY}
        ${QUARTZCORE_LIBRARY}
        OpenSSL::SSL
        OpenSSL::Crypto
        ${SQLITE3_LIBRARY}
    )

    # macOS-specific compiler flags
    target_compile_options(HodosBrowserShell PRIVATE
        -Wno-deprecated-declarations
        -Wno-objc-missing-property-synthesis
    )

    # Set rpath for CEF framework lookup at runtime
    set_target_properties(HodosBrowserShell PROPERTIES
        BUILD_WITH_INSTALL_RPATH TRUE
        INSTALL_RPATH "@executable_path/../Frameworks"
        INSTALL_RPATH_USE_LINK_PATH TRUE
    )

elseif(WIN32)
    # Windows: Link Windows DLLs and libraries
    target_link_directories(HodosBrowserShell PRIVATE
        "${CEF_ROOT}/Release"
        "${WRAPPER_LIB_PATH}"
    )

    target_link_libraries(HodosBrowserShell PRIVATE
        libcef
        libcef_dll_wrapper.lib
        cef_sandbox
        user32
        gdi32
        ole32
        oleaut32
        comdlg32
        shlwapi
        uuid
        winmm
        dbghelp
        delayimp
        shell32
        advapi32
        dwmapi
        version
        winhttp
        OpenSSL::SSL
        OpenSSL::Crypto
        nlohmann_json::nlohmann_json
        unofficial::sqlite3::sqlite3
    )

    # Windows delay-load
    set_property(TARGET HodosBrowserShell PROPERTY LINK_FLAGS "/DELAYLOAD:libcef.dll")

    # Windows-specific macros
    add_definitions(-DUNICODE -D_UNICODE)
endif()

# Platform-specific post-build steps
if(WIN32)
    # Windows: Copy CEF runtime files after building
    add_custom_command(TARGET HodosBrowserShell POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E echo "Copying CEF runtime files (Windows)..."

        # Copy everything from Release/
        COMMAND ${CMAKE_COMMAND} -E copy_directory
            "${RELEASE_DIR}"
            "${OUTPUT_DIR}"

        # Copy everything from Resources/
        COMMAND ${CMAKE_COMMAND} -E copy_directory
            "${RESOURCES_DIR}"
            "${OUTPUT_DIR}"

        COMMAND ${CMAKE_COMMAND} -E echo "CEF files copied."
        COMMENT "Copying all CEF binaries and resources"
    )
elseif(APPLE)
    # macOS: Copy CEF framework into app bundle
    add_custom_command(TARGET HodosBrowserShell POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E echo "Setting up macOS app bundle..."

        # Create Frameworks directory in app bundle
        COMMAND ${CMAKE_COMMAND} -E make_directory
            "$<TARGET_FILE_DIR:HodosBrowserShell>/../Frameworks"

        # Copy CEF framework into app bundle
        COMMAND ${CMAKE_COMMAND} -E copy_directory
            "${CEF_FRAMEWORK_PATH}"
            "$<TARGET_FILE_DIR:HodosBrowserShell>/../Frameworks/Chromium Embedded Framework.framework"

        # Copy CEF resources (inside framework on macOS)
        COMMAND ${CMAKE_COMMAND} -E copy_directory
            "${CEF_FRAMEWORK_PATH}/Resources"
            "$<TARGET_FILE_DIR:HodosBrowserShell>/../Resources"

        COMMAND ${CMAKE_COMMAND} -E echo "macOS app bundle setup complete."
        COMMENT "Setting up macOS application bundle"
    )
endif()

# add_subdirectory(src/core) # Removed - wallet library no longer needed
# add_subdirectory(tests) # Removed - test files deleted
