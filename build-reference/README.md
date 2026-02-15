# Build Reference Files

This directory contains reference copies of build configuration files that are useful for documentation and understanding the build process before downloading large dependencies.

## Files

### `CEF_WRAPPER_CMakeLists.txt`
- **Purpose**: Reference copy of the CMakeLists.txt used to build the CEF wrapper library
- **Actual Location**: `cef-binaries/libcef_dll/wrapper/CMakeLists.txt` (after downloading CEF binaries)
- **Usage**: This file is included in CEF binaries when you download them. This reference copy allows you to see the build configuration before downloading the ~500MB+ CEF binaries.

## Why These Files Exist

The `cef-binaries/` directory is gitignored because CEF binaries are too large for Git (~500MB+). However, it's helpful to have reference copies of important build files so:

1. Developers can understand the build process before downloading dependencies
2. Documentation can reference these files
3. New contributors can see what's expected in the actual CEF binaries directory

## Important Notes

- **Do not modify these reference files** - They are for documentation purposes only
- The actual build files are provided by the CEF binaries download
- If you need to customize the build, modify the files in the `cef-binaries/` directory after downloading
