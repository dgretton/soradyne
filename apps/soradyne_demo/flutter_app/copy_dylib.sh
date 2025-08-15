#!/bin/bash
# Copy the Rust library to the app bundle after build

# Source path for the dylib
DYLIB_SOURCE="../../packages/soradyne_core/target/release/libsoradyne.dylib"

# Destination paths for both Debug and Release builds
DEBUG_APP_BUNDLE="build/macos/Build/Products/Debug/soradyne_app.app/Contents/MacOS/"
DEBUG_FRAMEWORKS_DIR="build/macos/Build/Products/Debug/soradyne_app.app/Contents/Frameworks/"
RELEASE_APP_BUNDLE="build/macos/Build/Products/Release/soradyne_app.app/Contents/MacOS/"
RELEASE_FRAMEWORKS_DIR="build/macos/Build/Products/Release/soradyne_app.app/Contents/Frameworks/"

if [ -f "$DYLIB_SOURCE" ]; then
    # Copy to Debug build if it exists
    if [ -d "$DEBUG_APP_BUNDLE" ]; then
        cp "$DYLIB_SOURCE" "$DEBUG_APP_BUNDLE"
        echo "Copied libsoradyne.dylib to Debug app bundle MacOS directory"
    fi
    
    if [ -d "$DEBUG_FRAMEWORKS_DIR" ]; then
        cp "$DYLIB_SOURCE" "$DEBUG_FRAMEWORKS_DIR"
        echo "Copied libsoradyne.dylib to Debug app bundle Frameworks directory"
    fi
    
    # Copy to Release build if it exists
    if [ -d "$RELEASE_APP_BUNDLE" ]; then
        cp "$DYLIB_SOURCE" "$RELEASE_APP_BUNDLE"
        echo "Copied libsoradyne.dylib to Release app bundle MacOS directory"
    fi
    
    if [ -d "$RELEASE_FRAMEWORKS_DIR" ]; then
        cp "$DYLIB_SOURCE" "$RELEASE_FRAMEWORKS_DIR"
        echo "Copied libsoradyne.dylib to Release app bundle Frameworks directory"
    fi
else
    echo "libsoradyne.dylib not found at: $DYLIB_SOURCE"
    echo "Make sure to build the Rust library first with: cargo build --release"
fi
