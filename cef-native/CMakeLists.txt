cmake_minimum_required(VERSION 3.15)

# ============================================================================
# IMPORTANT: vcpkg configuration MUST come BEFORE project() command
# ============================================================================

# Set vcpkg triplet (Windows uses x64-windows-static for static linking)
if(NOT DEFINED VCPKG_TARGET_TRIPLET)
    if(WIN32)
        set(VCPKG_TARGET_TRIPLET "x64-windows-static" CACHE STRING "")
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

# Paths
set(CEF_ROOT "../cef-binaries")
set(WRAPPER_LIB_PATH "${CEF_ROOT}/libcef_dll/wrapper/build/Release")
set(RELEASE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../cef-binaries/Release")
set(RESOURCES_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../cef-binaries/Resources")
set(OUTPUT_DIR "$<TARGET_FILE_DIR:HodosBrowserShell>")

set(OPENSSL_USE_STATIC_LIBS TRUE)
# set(OPENSSL_MSVC_STATIC_RT TRUE)

# Find required packages (vcpkg will locate them automatically)
find_package(OpenSSL REQUIRED)
find_package(nlohmann_json CONFIG REQUIRED)
find_package(unofficial-sqlite3 CONFIG REQUIRED)


message(STATUS "OpenSSL include dir: ${OPENSSL_INCLUDE_DIR}")
message(STATUS "OpenSSL libraries: ${OPENSSL_LIBRARIES}")

# Output
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

# Shell source
set(SOURCES
    cef_browser_shell.cpp
    src/handlers/simple_handler.cpp
    src/handlers/simple_render_process_handler.cpp
    src/handlers/simple_app.cpp
    src/handlers/my_overlay_render_handler.cpp
    src/core/WalletService.cpp
    src/core/IdentityHandler.cpp
    src/core/NavigationHandler.cpp
    src/core/AddressHandler.cpp
    src/core/BRC100Bridge.cpp
    src/core/BRC100Handler.cpp
    src/core/HttpRequestInterceptor.cpp
    src/core/WebSocketServerHandler.cpp
    # Tab management (Phase 1)
    include/core/Tab.h
    include/core/TabManager.h
    src/core/TabManager.cpp
    # History management
    include/core/HistoryManager.h
    src/core/HistoryManager.cpp
    # Add other source files here
)

# Create executable
add_executable(HodosBrowserShell WIN32 ${SOURCES})

set_target_properties(HodosBrowserShell PROPERTIES
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
)

# Include CEF headers and project headers
# Note: vcpkg packages (OpenSSL, nlohmann_json, sqlite3) automatically add their include paths
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

# Link libs
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

# Delay-load
set_property(TARGET HodosBrowserShell PROPERTY LINK_FLAGS "/DELAYLOAD:libcef.dll")

# Required macros
add_definitions(-DUNICODE -D_UNICODE)

# Copy all runtime files after building
add_custom_command(TARGET HodosBrowserShell POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E echo "Copying CEF runtime files..."

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

# add_subdirectory(src/core) # Removed - wallet library no longer needed
# add_subdirectory(tests) # Removed - test files deleted
